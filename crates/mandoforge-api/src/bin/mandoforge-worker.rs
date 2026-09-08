use std::{
    collections::{HashMap, HashSet, VecDeque},
    env,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::time::{Instant, sleep};

#[path = "../worker_scheduler.rs"]
mod worker_scheduler;
use worker_scheduler::{
    Work, WorkKind, WorkerBackend, concurrency_from_lookup, interleave_work, run_worker,
    work_attempt_result,
};

#[derive(Debug, Deserialize)]
struct QueueJob {
    id: String,
    session_id: String,
    status: String,
    lease_expires_at: Option<DateTime<Utc>>,
}

impl QueueJob {
    fn claimable(&self) -> bool {
        self.status == "queued"
            || (self.status == "running"
                && self
                    .lease_expires_at
                    .is_none_or(|deadline| deadline <= Utc::now()))
    }

    fn executing(&self) -> bool {
        matches!(
            self.status.as_str(),
            "executing" | "finalizing" | "cancel_requested"
        ) || (self.status == "running" && !self.claimable())
    }
}

#[derive(Debug, Deserialize)]
struct TaskBoardSnapshot {
    items: Vec<TaskBoardItem>,
}

#[derive(Debug, Deserialize)]
struct TaskBoardItem {
    workflow_step_run_id: String,
    workflow_run_id: Option<String>,
    agent_id: Option<String>,
    claimable: bool,
}

#[derive(Debug, Deserialize)]
struct WorkflowStepIdentity {
    id: String,
    session_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct RunWorkflowStepRunRequest<'a> {
    agent_id: &'a str,
    worker_id: &'a str,
    lease_seconds: i64,
}

struct Worker {
    client: reqwest::Client,
    base_url: String,
    id: String,
    subject: String,
    roles: String,
    token: Option<String>,
    environment_id: Option<String>,
    pool: Option<String>,
}

impl Worker {
    fn request(&self, method: reqwest::Method, endpoint: &str) -> reqwest::RequestBuilder {
        self.client
            .request(method, format!("{}{endpoint}", self.base_url))
            .header("x-mandoforge-worker-id", &self.id)
            .worker_auth(&self.subject, &self.roles, self.token.as_deref())
            .worker_environment(self.environment_id.as_deref())
            .worker_pool(self.pool.as_deref())
    }
}

#[async_trait::async_trait]
impl WorkerBackend for Worker {
    async fn discover(&self) -> Result<VecDeque<Work>> {
        let (loops, executions, board) = tokio::join!(
            fetch_job_item::<Vec<QueueJob>>(
                self.request(reqwest::Method::GET, "/api/session-loop-jobs"),
                "session loop jobs"
            ),
            fetch_job_item::<Vec<QueueJob>>(
                self.request(reqwest::Method::GET, "/api/execution-jobs"),
                "execution jobs"
            ),
            fetch_job_item::<TaskBoardSnapshot>(
                self.request(reqwest::Method::GET, "/api/task-board"),
                "task board"
            ),
        );
        let (Some(loops), Some(executions), Some(board)) = (loops, executions, board) else {
            // Without all queue snapshots, same-session exclusion is unknown.
            bail!("worker queue snapshot is incomplete");
        };
        // A timed-out HTTP request may still be executing at the server. Its
        // live lease remains authoritative even after this process loses it.
        let occupied: HashSet<_> = loops
            .iter()
            .chain(&executions)
            .filter(|job| job.executing())
            .map(|job| job.session_id.clone())
            .collect();
        let loop_work = loops
            .into_iter()
            .filter(QueueJob::claimable)
            .map(|job| Work {
                id: job.id,
                session_id: Some(job.session_id),
                kind: WorkKind::SessionLoop,
            });
        let execution_work = executions
            .into_iter()
            .filter(QueueJob::claimable)
            .map(|job| Work {
                id: job.id,
                session_id: Some(job.session_id),
                kind: WorkKind::Execution,
            });
        let mut step_work = Vec::new();
        let mut step_identities = HashMap::new();
        for item in board.items.into_iter().filter(|item| item.claimable) {
            let Some(agent_id) = item.agent_id.filter(|id| !id.trim().is_empty()) else {
                continue;
            };
            // Resolve identities using the existing per-run steps endpoint.
            // Fetch each run once; an unresolved identity executes exclusively.
            if let Some(run_id) = &item.workflow_run_id
                && !step_identities.contains_key(run_id)
            {
                let steps = fetch_job_list::<WorkflowStepIdentity>(
                    self.request(
                        reqwest::Method::GET,
                        &format!("/api/workflow-runs/{run_id}/steps"),
                    ),
                    "workflow step identities",
                )
                .await;
                step_identities.insert(run_id.clone(), steps);
            }
            let session_id = item
                .workflow_run_id
                .as_ref()
                .and_then(|run_id| step_identities.get(run_id))
                .and_then(|steps| {
                    steps
                        .iter()
                        .find(|step| step.id == item.workflow_step_run_id)
                })
                .and_then(|step| step.session_id.clone());
            step_work.push(Work {
                id: item.workflow_step_run_id,
                session_id,
                kind: WorkKind::WorkflowStep { agent_id },
            });
        }
        Ok(interleave_work(
            [
                loop_work.collect(),
                execution_work.collect(),
                step_work.into(),
            ],
            &occupied,
        ))
    }

    async fn run(&self, work: &Work) -> Result<bool> {
        let mut request = self
            .request(
                reqwest::Method::POST,
                &format!("/api/{}/{}/run", work.route(), work.id),
            )
            // The API renews leases and enforces operation deadlines. A normal
            // long operation must not be abandoned by the 60s discovery client.
            .timeout(Duration::from_secs(86_400));
        if let WorkKind::WorkflowStep { agent_id } = &work.kind {
            request = request.json(&RunWorkflowStepRunRequest {
                agent_id,
                worker_id: &self.id,
                lease_seconds: 600,
            });
        }
        let response = request
            .send()
            .await
            .with_context(|| format!("run {}", work.key()))?;
        if matches!(
            response.status(),
            StatusCode::NOT_FOUND | StatusCode::BAD_REQUEST | StatusCode::CONFLICT
        ) {
            eprintln!("work no longer claimable: {}", work.key());
            return Ok(false);
        }
        let payload: serde_json::Value = response.error_for_status()?.json().await?;
        let result = if matches!(work.kind, WorkKind::WorkflowStep { .. }) {
            &payload["step"]
        } else {
            &payload
        };
        let status = result["status"]
            .as_str()
            .context("run response has no status")?;
        if result["id"].as_str() != Some(work.id.as_str()) {
            bail!("run response identity does not match {}", work.key());
        }
        println!("work attempt finished: {} status={status}", work.key());
        work_attempt_result(status)
    }

    async fn wait_for_work(&self, interval: Duration) {
        let start = Instant::now();
        let result = self
            .request(
                reqwest::Method::GET,
                &format!("/api/queue/notify-wait?timeout_ms={}", interval.as_millis()),
            )
            .timeout(interval + Duration::from_secs(5))
            .send()
            .await;
        if result.is_ok_and(|response| response.status() == StatusCode::OK) {
            return;
        }
        // A 204 already waited. Back off only the remaining interval when the
        // endpoint failed early, avoiding both double waiting and a busy loop.
        sleep(interval.saturating_sub(start.elapsed())).await;
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let token = env::var("MANDOFORGE_WORKER_TOKEN")
        .or_else(|_| env::var("MANDOFORGE_DEV_ADMIN_TOKEN"))
        .ok()
        .filter(|v| !v.trim().is_empty());
    let insecure_dev_auth = env::var("MANDOFORGE_INSECURE_DEV_AUTH")
        .ok()
        .is_some_and(|v| matches!(v.trim(), "1" | "true" | "TRUE" | "yes" | "YES"));
    if token.is_none() && !insecure_dev_auth {
        bail!(
            "mandoforge-worker requires MANDOFORGE_WORKER_TOKEN, MANDOFORGE_DEV_ADMIN_TOKEN, or MANDOFORGE_INSECURE_DEV_AUTH=true"
        );
    }
    let concurrency = concurrency_from_lookup(&|key| env::var(key).ok())?;
    let interval = env::var("POLL_INTERVAL_SECONDS")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| v.is_finite())
        .unwrap_or(2.0)
        .clamp(0.1, 29.0);
    let max_jobs = env::var("MAX_JOBS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    let run_once = env::var("RUN_ONCE").is_ok_and(|v| v == "1");
    let worker = Worker {
        client: reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .context("build worker http client")?,
        base_url: env::var("BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:8787".to_string()),
        id: env::var("WORKER_ID")
            .unwrap_or_else(|_| format!("mandoforge-worker-{}", std::process::id())),
        subject: env::var("WORKER_SUBJECT").unwrap_or_else(|_| "mandoforge-worker".to_string()),
        roles: env::var("WORKER_ROLES").unwrap_or_else(|_| "worker".to_string()),
        token,
        environment_id: env::var("WORKER_ENVIRONMENT_ID")
            .ok()
            .filter(|v| !v.trim().is_empty()),
        pool: env::var("WORKER_POOL")
            .or_else(|_| env::var("WORKER_QUEUE"))
            .ok()
            .filter(|v| !v.trim().is_empty()),
    };
    wait_for_api(&worker.client, &worker.base_url).await?;
    let processed = run_worker(
        &worker,
        concurrency,
        Duration::from_secs_f64(interval),
        max_jobs,
        run_once,
    )
    .await?;
    println!("mandoforge worker processed {processed} job(s)");
    Ok(())
}

async fn fetch_job_list<T>(request: reqwest::RequestBuilder, label: &str) -> Vec<T>
where
    T: DeserializeOwned,
{
    fetch_job_item::<Vec<T>>(request, label)
        .await
        .unwrap_or_default()
}

async fn fetch_job_item<T>(request: reqwest::RequestBuilder, label: &str) -> Option<T>
where
    T: DeserializeOwned,
{
    let response = match request.send().await {
        Ok(response) => response,
        Err(error) => {
            eprintln!("get {label} failed: {error}");
            return None;
        }
    };
    let response = match response.error_for_status() {
        Ok(response) => response,
        Err(error) => {
            eprintln!("get {label} failed: {error}");
            return None;
        }
    };
    match response.json().await {
        Ok(item) => Some(item),
        Err(error) => {
            eprintln!("parse {label} failed: {error}");
            None
        }
    }
}

trait WorkerAuthRequestBuilder {
    fn worker_auth(
        self,
        subject: &str,
        roles: &str,
        token: Option<&str>,
    ) -> reqwest::RequestBuilder;

    fn worker_environment(self, environment_id: Option<&str>) -> reqwest::RequestBuilder;

    fn worker_pool(self, worker_pool: Option<&str>) -> reqwest::RequestBuilder;
}

impl WorkerAuthRequestBuilder for reqwest::RequestBuilder {
    fn worker_auth(
        self,
        subject: &str,
        roles: &str,
        token: Option<&str>,
    ) -> reqwest::RequestBuilder {
        let request = self.header("x-mandoforge-subject", subject);
        let request = if let Some(token) = token {
            request.bearer_auth(token)
        } else {
            request
        };
        request.header("x-mandoforge-roles", roles)
    }

    fn worker_environment(self, environment_id: Option<&str>) -> reqwest::RequestBuilder {
        if let Some(environment_id) = environment_id {
            self.header("x-mandoforge-environment-id", environment_id)
        } else {
            self
        }
    }

    fn worker_pool(self, worker_pool: Option<&str>) -> reqwest::RequestBuilder {
        if let Some(worker_pool) = worker_pool {
            self.header("x-mandoforge-worker-pool", worker_pool)
        } else {
            self
        }
    }
}

async fn wait_for_api(client: &reqwest::Client, base_url: &str) -> Result<()> {
    let mut last_error = None;
    for _ in 0..60 {
        match client.get(format!("{base_url}/healthz")).send().await {
            Ok(response) if response.status().is_success() => return Ok(()),
            Ok(response) => {
                last_error = Some(format!("API healthz returned {}", response.status()));
            }
            Err(error) => {
                last_error = Some(error.to_string());
            }
        }
        sleep(Duration::from_secs(1)).await;
    }
    bail!(
        "API healthz did not become ready: {}",
        last_error.unwrap_or_else(|| "unknown error".to_string())
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Json, Router,
        extract::Path,
        routing::{get, post},
    };
    use serde_json::json;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use tokio::net::TcpListener;

    fn test_worker(base_url: String) -> Worker {
        Worker {
            client: reqwest::Client::new(),
            base_url,
            id: "worker".into(),
            subject: "worker".into(),
            roles: "worker".into(),
            token: None,
            environment_id: None,
            pool: None,
        }
    }

    async fn serve_once(route: Router) -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock worker server");
        let addr = listener.local_addr().expect("mock worker server addr");
        tokio::spawn(async move {
            axum::serve(listener, route)
                .await
                .expect("serve mock worker");
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn fetch_job_list_returns_empty_on_server_error() {
        let base_url = serve_once(Router::new().route(
            "/api/session-loop-jobs",
            get(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "boom") }),
        ))
        .await;
        let client = reqwest::Client::new();

        let jobs: Vec<QueueJob> = fetch_job_list(
            client.get(format!("{base_url}/api/session-loop-jobs")),
            "session loop jobs",
        )
        .await;

        assert!(jobs.is_empty());
    }

    #[tokio::test]
    async fn fetch_job_list_returns_empty_on_invalid_json() {
        let base_url = serve_once(Router::new().route(
            "/api/execution-jobs",
            get(|| async { (StatusCode::OK, "not-json") }),
        ))
        .await;
        let client = reqwest::Client::new();

        let jobs: Vec<QueueJob> = fetch_job_list(
            client.get(format!("{base_url}/api/execution-jobs")),
            "execution jobs",
        )
        .await;

        assert!(jobs.is_empty());
    }

    #[tokio::test]
    async fn process_workflow_step_jobs_runs_claimable_task_board_item() {
        let run_count = Arc::new(AtomicUsize::new(0));
        let run_count_for_route = Arc::clone(&run_count);
        let step_id = "00000000-0000-4000-8000-000000000010";
        let agent_id = "00000000-0000-4000-8000-000000000011";
        let base_url = serve_once(
            Router::new()
                .route("/api/session-loop-jobs", get(|| async { Json(json!([])) }))
                .route("/api/execution-jobs", get(|| async { Json(json!([])) }))
                .route(
                    "/api/task-board",
                    get(move || async move {
                        Json(json!({
                            "items": [{
                                "workflow_step_run_id": step_id,
                                "agent_id": agent_id,
                                "claimable": true,
                                "status": "running"
                            }]
                        }))
                    }),
                )
                .route(
                    "/api/workflow-step-runs/{id}/run",
                    post(
                        move |Path(id): Path<String>, Json(body): Json<serde_json::Value>| {
                            let run_count = Arc::clone(&run_count_for_route);
                            async move {
                                assert_eq!(id, step_id);
                                assert_eq!(body["agent_id"], json!(agent_id));
                                assert_eq!(body["worker_id"], json!("worker-test-1"));
                                run_count.fetch_add(1, Ordering::SeqCst);
                                Json(json!({
                                    "step": {
                                        "id": step_id,
                                        "status": "requires_action"
                                    }
                                }))
                            }
                        },
                    ),
                ),
        )
        .await;
        let client = reqwest::Client::new();

        let processed = run_worker(
            &Worker {
                client,
                base_url,
                id: "worker-test-1".into(),
                subject: "worker-subject".into(),
                roles: "admin".into(),
                token: None,
                environment_id: None,
                pool: None,
            },
            4,
            Duration::from_millis(10),
            0,
            true,
        )
        .await
        .expect("process workflow step jobs");

        assert_eq!(processed, 1);
        assert_eq!(run_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn independent_short_task_finishes_before_long_task() {
        let started = std::time::Instant::now();
        let finishes = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let recorded = finishes.clone();
        let base_url = serve_once(Router::new()
            .route("/api/session-loop-jobs", get(|| async { Json(json!([])) }))
            .route("/api/execution-jobs", get(|| async { Json(json!([])) }))
            .route("/api/task-board", get(|| async { Json(json!({"items": [
                {"workflow_step_run_id": "long", "workflow_run_id": "long", "agent_id": "agent", "claimable": true},
                {"workflow_step_run_id": "short", "workflow_run_id": "short", "agent_id": "agent", "claimable": true}
            ]})) }))
            .route("/api/workflow-runs/{id}/steps", get(|Path(id): Path<String>| async move { Json(json!([{"id": id, "session_id": id}])) }))
            .route("/api/workflow-step-runs/{id}/run", post(move |Path(id): Path<String>| {
                let recorded = recorded.clone();
                async move {
                    if id == "long" { sleep(Duration::from_millis(200)).await; }
                    recorded.lock().await.push((id.clone(), started.elapsed()));
                    Json(json!({"step": {"id": id, "status": "completed"}}))
                }
            }))).await;
        run_worker(
            &test_worker(base_url),
            4,
            Duration::from_millis(10),
            0,
            true,
        )
        .await
        .unwrap();
        let finishes = finishes.lock().await;
        eprintln!("task scheduling baseline/readback: {finishes:?}");
        assert_eq!(
            finishes[0].0, "short",
            "an independent short task must not wait for the long task"
        );
    }

    #[tokio::test]
    async fn worker_serializes_same_session_across_queue_kinds_and_bounds_other_sessions() {
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let sessions = Arc::new(tokio::sync::Mutex::new(HashSet::new()));
        let count = Arc::new(AtomicUsize::new(0));
        let run = {
            let (active, peak, sessions, count) = (
                active.clone(),
                peak.clone(),
                sessions.clone(),
                count.clone(),
            );
            move |Path((kind, id)): Path<(String, String)>| {
                let (active, peak, sessions, count) = (
                    active.clone(),
                    peak.clone(),
                    sessions.clone(),
                    count.clone(),
                );
                async move {
                    let session = if id == "loop" || id == "execution" {
                        "shared"
                    } else {
                        id.as_str()
                    };
                    assert!(
                        sessions.lock().await.insert(session.to_string()),
                        "overlapping execution in one session"
                    );
                    let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(current, Ordering::SeqCst);
                    sleep(Duration::from_millis(30)).await;
                    sessions.lock().await.remove(session);
                    active.fetch_sub(1, Ordering::SeqCst);
                    count.fetch_add(1, Ordering::SeqCst);
                    if kind == "workflow-step-runs" {
                        Json(json!({"step": {"id": id, "status": "completed"}}))
                    } else {
                        Json(json!({"id": id, "status": "completed"}))
                    }
                }
            }
        };
        let base_url = serve_once(
            Router::new()
                .route(
                    "/api/session-loop-jobs",
                    get(|| async {
                        Json(json!([
                            {"id":"loop", "session_id":"shared", "status":"queued"},
                            {"id":"independent", "session_id":"independent", "status":"queued"}
                        ]))
                    }),
                )
                .route(
                    "/api/execution-jobs",
                    get(|| async {
                        Json(json!([
                            {"id":"execution", "session_id":"shared", "status":"queued"}
                        ]))
                    }),
                )
                .route(
                    "/api/task-board",
                    get(|| async { Json(json!({"items": []})) }),
                )
                .route("/api/{kind}/{id}/run", post(run)),
        )
        .await;
        let completed = run_worker(
            &test_worker(base_url),
            2,
            Duration::from_millis(10),
            0,
            true,
        )
        .await
        .unwrap();
        assert_eq!(completed, 3);
        assert_eq!(count.load(Ordering::SeqCst), 3);
        assert_eq!(peak.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn worker_discovers_new_short_task_while_long_task_is_running() {
        let long_started = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let short_finished = Arc::new(tokio::sync::Notify::new());
        let snapshot_started = long_started.clone();
        let run_started = long_started.clone();
        let base_url = serve_once(
            Router::new()
                .route(
                    "/api/session-loop-jobs",
                    get(move || {
                        let started = snapshot_started.clone();
                        async move {
                            Json(if started.load(Ordering::SeqCst) {
                                json!([
                                    {"id":"short", "session_id":"short", "status":"queued"}
                                ])
                            } else {
                                json!([
                                    {"id":"long", "session_id":"long", "status":"queued"}
                                ])
                            })
                        }
                    }),
                )
                .route("/api/execution-jobs", get(|| async { Json(json!([])) }))
                .route(
                    "/api/task-board",
                    get(|| async { Json(json!({"items": []})) }),
                )
                .route(
                    "/api/session-loop-jobs/{id}/run",
                    post(move |Path(id): Path<String>| {
                        let started = run_started.clone();
                        let finished = short_finished.clone();
                        async move {
                            if id == "long" {
                                started.store(true, Ordering::SeqCst);
                                tokio::time::timeout(Duration::from_secs(2), finished.notified())
                                    .await
                                    .expect("new short task was starved");
                            } else {
                                finished.notify_one();
                            }
                            Json(json!({"id": id, "status":"completed"}))
                        }
                    }),
                ),
        )
        .await;
        assert_eq!(
            run_worker(
                &test_worker(base_url),
                2,
                Duration::from_millis(10),
                2,
                false
            )
            .await
            .unwrap(),
            2
        );
    }

    #[tokio::test]
    async fn worker_max_jobs_is_an_exact_dispatch_limit() {
        let count = Arc::new(AtomicUsize::new(0));
        let calls = count.clone();
        let base_url = serve_once(
            Router::new()
                .route(
                    "/api/session-loop-jobs",
                    get(|| async {
                        Json(json!([
                            {"id":"one", "session_id":"one", "status":"queued"},
                            {"id":"two", "session_id":"two", "status":"queued"},
                            {"id":"three", "session_id":"three", "status":"queued"}
                        ]))
                    }),
                )
                .route("/api/execution-jobs", get(|| async { Json(json!([])) }))
                .route(
                    "/api/task-board",
                    get(|| async { Json(json!({"items": []})) }),
                )
                .route(
                    "/api/session-loop-jobs/{id}/run",
                    post(move |Path(id): Path<String>| {
                        calls.fetch_add(1, Ordering::SeqCst);
                        async move { Json(json!({"id": id, "status":"completed"})) }
                    }),
                ),
        )
        .await;
        assert_eq!(
            run_worker(
                &test_worker(base_url),
                4,
                Duration::from_millis(10),
                2,
                true
            )
            .await
            .unwrap(),
            2
        );
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn timed_out_notification_wait_does_not_wait_twice() {
        let base_url = serve_once(Router::new().route(
            "/api/queue/notify-wait",
            get(|| async {
                sleep(Duration::from_millis(300)).await;
                StatusCode::NO_CONTENT
            }),
        ))
        .await;
        let worker = test_worker(base_url);
        let started = Instant::now();
        tokio::time::timeout(
            Duration::from_millis(500),
            worker.wait_for_work(Duration::from_millis(300)),
        )
        .await
        .expect("notification timeout must not be followed by a second full wait");
        eprintln!("notification idle wait readback: {:?}", started.elapsed());
    }

    #[tokio::test]
    async fn ambiguous_run_failure_defers_queued_sibling_and_reports_failure() {
        let calls = Arc::new(AtomicUsize::new(0));
        let recorded = calls.clone();
        let base_url = serve_once(
            Router::new()
                .route(
                    "/api/session-loop-jobs",
                    get(|| async {
                        Json(json!([
                            {"id":"one", "session_id":"shared", "status":"queued"},
                            {"id":"two", "session_id":"shared", "status":"queued"}
                        ]))
                    }),
                )
                .route("/api/execution-jobs", get(|| async { Json(json!([])) }))
                .route(
                    "/api/task-board",
                    get(|| async { Json(json!({"items": []})) }),
                )
                .route(
                    "/api/session-loop-jobs/{id}/run",
                    post(move || {
                        recorded.fetch_add(1, Ordering::SeqCst);
                        async { StatusCode::INTERNAL_SERVER_ERROR }
                    }),
                ),
        )
        .await;
        assert!(
            run_worker(
                &test_worker(base_url),
                4,
                Duration::from_millis(10),
                0,
                true
            )
            .await
            .is_err()
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn worker_does_not_dispatch_beside_a_live_remote_lease() {
        let base_url = serve_once(Router::new()
            .route("/api/session-loop-jobs", get(|| async { Json(json!([
                {"id":"new", "session_id":"shared", "status":"queued"}
            ])) }))
            .route("/api/execution-jobs", get(|| async { Json(json!([
                {"id":"existing", "session_id":"shared", "status":"running", "lease_expires_at": "2099-01-01T00:00:00Z"}
            ])) }))
            .route("/api/task-board", get(|| async { Json(json!({"items": []})) }))).await;
        assert!(test_worker(base_url).discover().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn worker_drains_new_continuation_without_an_idle_poll_delay() {
        let first_finished = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let discovery = first_finished.clone();
        let completed = first_finished.clone();
        let base_url = serve_once(Router::new()
            .route("/api/session-loop-jobs", get(move || {
                let first_finished = discovery.clone();
                async move { Json(json!([{"id": if first_finished.load(Ordering::SeqCst) {"second"} else {"first"}, "session_id":"one-session", "status":"queued"}])) }
            }))
            .route("/api/execution-jobs", get(|| async { Json(json!([])) }))
            .route("/api/task-board", get(|| async { Json(json!({"items":[]})) }))
            .route("/api/session-loop-jobs/{id}/run", post(move |Path(id): Path<String>| {
                completed.store(true, Ordering::SeqCst);
                async move { Json(json!({"id":id,"status":"completed"})) }
            }))).await;
        let worker = test_worker(base_url);
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            run_worker(&worker, 1, Duration::from_secs(3), 2, false),
        )
        .await
        .expect("a ready continuation should not incur the three-second idle interval")
        .unwrap();
        assert_eq!(result, 2);
    }

    #[tokio::test]
    async fn failed_work_result_fails_a_bounded_batch_instead_of_waiting_forever() {
        for status in ["failed", "outcome_unknown", "unexpected_status"] {
            let base_url = serve_once(
            Router::new()
                .route(
                    "/api/session-loop-jobs",
                    get(|| async {
                        Json(
                            json!([{"id":"failed", "session_id":"one-session", "status":"queued"}]),
                        )
                    }),
                )
                .route("/api/execution-jobs", get(|| async { Json(json!([])) }))
                .route(
                    "/api/task-board",
                    get(|| async { Json(json!({"items":[]})) }),
                )
                .route(
                    "/api/session-loop-jobs/{id}/run",
                    post(move |Path(id): Path<String>| async move {
                        Json(json!({"id":id,"status":status}))
                    }),
                ),
        )
        .await;
            let worker = test_worker(base_url);
            let result = tokio::time::timeout(
                Duration::from_secs(1),
                run_worker(&worker, 2, Duration::from_secs(3), 1, false),
            )
            .await
            .expect("failed attempts also consume the bounded dispatch budget");
            assert!(result.is_err());
        }
    }

    #[test]
    fn worker_environment_adds_environment_header_when_configured() {
        let request = reqwest::Client::new()
            .get("http://127.0.0.1/api/session-loop-jobs")
            .worker_environment(Some("00000000-0000-4000-8000-000000000001"))
            .build()
            .expect("build request");

        assert_eq!(
            request
                .headers()
                .get("x-mandoforge-environment-id")
                .and_then(|value| value.to_str().ok()),
            Some("00000000-0000-4000-8000-000000000001")
        );
    }

    #[test]
    fn worker_pool_adds_worker_pool_header_when_configured() {
        let request = reqwest::Client::new()
            .get("http://127.0.0.1/api/session-loop-jobs")
            .worker_pool(Some("managed-agent-a"))
            .build()
            .expect("build request");

        assert_eq!(
            request
                .headers()
                .get("x-mandoforge-worker-pool")
                .and_then(|value| value.to_str().ok()),
            Some("managed-agent-a")
        );
    }
}
