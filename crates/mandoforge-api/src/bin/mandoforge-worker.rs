use std::{env, time::Duration};

use anyhow::{Context, Result, bail};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::time::sleep;

#[derive(Debug, Deserialize)]
struct ExecutionJob {
    id: String,
    status: String,
}

#[derive(Debug, Deserialize)]
struct SessionLoopJob {
    id: String,
    status: String,
}

#[derive(Debug, Deserialize)]
struct TaskBoardSnapshot {
    items: Vec<TaskBoardItem>,
}

#[derive(Debug, Deserialize)]
struct TaskBoardItem {
    workflow_step_run_id: String,
    agent_id: Option<String>,
    claimable: bool,
}

#[derive(Debug, Deserialize)]
struct RunWorkflowStepRunResponse {
    step: WorkflowStepRunSummary,
}

#[derive(Debug, Deserialize)]
struct WorkflowStepRunSummary {
    id: String,
    status: String,
}

#[derive(Debug, Serialize)]
struct RunWorkflowStepRunRequest<'a> {
    agent_id: &'a str,
    worker_id: &'a str,
    lease_seconds: i64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let base_url = env::var("BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:8787".to_string());
    let worker_id = env::var("WORKER_ID")
        .unwrap_or_else(|_| format!("mandoforge-worker-{}", std::process::id()));
    let worker_subject =
        env::var("WORKER_SUBJECT").unwrap_or_else(|_| "mandoforge-worker".to_string());
    let worker_roles = env::var("WORKER_ROLES").unwrap_or_else(|_| "worker".to_string());
    let worker_environment_id = env::var("WORKER_ENVIRONMENT_ID")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let worker_pool = env::var("WORKER_POOL")
        .or_else(|_| env::var("WORKER_QUEUE"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let api_token = env::var("MANDOFORGE_WORKER_TOKEN")
        .or_else(|_| env::var("MANDOFORGE_DEV_ADMIN_TOKEN"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let insecure_dev_auth = env::var("MANDOFORGE_INSECURE_DEV_AUTH")
        .ok()
        .is_some_and(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"));
    if api_token.is_none() && !insecure_dev_auth {
        bail!(
            "mandoforge-worker requires MANDOFORGE_WORKER_TOKEN, MANDOFORGE_DEV_ADMIN_TOKEN, or MANDOFORGE_INSECURE_DEV_AUTH=true"
        );
    }
    let poll_interval_ms = env::var("POLL_INTERVAL_SECONDS")
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .map(|secs| (secs * 1000.0) as u64)
        .unwrap_or(2_000)
        .max(100);
    let max_jobs = env::var("MAX_JOBS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let run_once = env::var("RUN_ONCE").is_ok_and(|value| value == "1");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .context("build worker http client")?;
    wait_for_api(&client, &base_url).await?;

    let mut processed = 0usize;
    loop {
        let session_loop_jobs: Vec<SessionLoopJob> = fetch_job_list(
            client
                .get(format!("{base_url}/api/session-loop-jobs"))
                .worker_auth(&worker_subject, &worker_roles, api_token.as_deref())
                .worker_environment(worker_environment_id.as_deref())
                .worker_pool(worker_pool.as_deref()),
            "session loop jobs",
        )
        .await;

        for job in session_loop_jobs
            .into_iter()
            .filter(|job| job.status == "queued" || job.status == "running")
        {
            let response = client
                .post(format!("{base_url}/api/session-loop-jobs/{}/run", job.id))
                .header("x-mandoforge-worker-id", &worker_id)
                .worker_auth(&worker_subject, &worker_roles, api_token.as_deref())
                .worker_environment(worker_environment_id.as_deref())
                .worker_pool(worker_pool.as_deref())
                .send()
                .await
                .with_context(|| format!("run session loop job {}", job.id))?;
            if response.status() == StatusCode::NOT_FOUND
                || response.status() == StatusCode::BAD_REQUEST
            {
                eprintln!("session loop job not claimable: {}", job.id);
                continue;
            }
            let updated: SessionLoopJob = match response.error_for_status() {
                Ok(response) => match response.json().await {
                    Ok(updated) => updated,
                    Err(error) => {
                        eprintln!(
                            "parse session loop job {} run response failed: {error}",
                            job.id
                        );
                        continue;
                    }
                },
                Err(error) => {
                    eprintln!("run session loop job {} failed: {error}", job.id);
                    continue;
                }
            };
            processed += 1;
            println!(
                "session loop job attempt finished: {} status={}",
                updated.id, updated.status
            );
            if max_jobs != 0 && processed >= max_jobs {
                println!("mandoforge worker processed {processed} job(s)");
                return Ok(());
            }
        }

        let jobs: Vec<ExecutionJob> = fetch_job_list(
            client
                .get(format!("{base_url}/api/execution-jobs"))
                .worker_auth(&worker_subject, &worker_roles, api_token.as_deref())
                .worker_environment(worker_environment_id.as_deref())
                .worker_pool(worker_pool.as_deref()),
            "execution jobs",
        )
        .await;

        for job in jobs
            .into_iter()
            .filter(|job| job.status == "queued" || job.status == "running")
        {
            let response = client
                .post(format!("{base_url}/api/execution-jobs/{}/run", job.id))
                .header("x-mandoforge-worker-id", &worker_id)
                .worker_auth(&worker_subject, &worker_roles, api_token.as_deref())
                .worker_environment(worker_environment_id.as_deref())
                .worker_pool(worker_pool.as_deref())
                .send()
                .await
                .with_context(|| format!("run execution job {}", job.id))?;
            if response.status() == StatusCode::NOT_FOUND
                || response.status() == StatusCode::BAD_REQUEST
            {
                eprintln!("execution job not claimable: {}", job.id);
                continue;
            }
            let updated: ExecutionJob = match response.error_for_status() {
                Ok(response) => match response.json().await {
                    Ok(updated) => updated,
                    Err(error) => {
                        eprintln!(
                            "parse execution job {} run response failed: {error}",
                            job.id
                        );
                        continue;
                    }
                },
                Err(error) => {
                    eprintln!("run execution job {} failed: {error}", job.id);
                    continue;
                }
            };
            processed += 1;
            println!(
                "execution job attempt finished: {} status={}",
                updated.id, updated.status
            );
            if max_jobs != 0 && processed >= max_jobs {
                println!("mandoforge worker processed {processed} job(s)");
                return Ok(());
            }
        }

        let workflow_step_processed = process_workflow_step_jobs(
            &client,
            &base_url,
            &worker_id,
            &worker_subject,
            &worker_roles,
            api_token.as_deref(),
            worker_environment_id.as_deref(),
            worker_pool.as_deref(),
        )
        .await?;
        processed += workflow_step_processed;
        if workflow_step_processed > 0 {
            println!("workflow step attempts finished: {workflow_step_processed}");
        }
        if max_jobs != 0 && processed >= max_jobs {
            println!("mandoforge worker processed {processed} job(s)");
            return Ok(());
        }

        if run_once {
            println!("mandoforge worker processed {processed} job(s)");
            return Ok(());
        }
        if poll_interval_ms == 0 {
            bail!("POLL_INTERVAL_SECONDS=0 requires RUN_ONCE=1 or MAX_JOBS > 0");
        }
        // Try the push-notify wait endpoint first. If it returns 200 a job was
        // just enqueued and we loop immediately. On timeout (204) or any error
        // we fall through and do a normal poll cycle — this keeps the worker
        // correct even when the endpoint is unavailable.
        let notified = client
            .get(format!(
                "{base_url}/api/queue/notify-wait?timeout_ms={poll_interval_ms}"
            ))
            .worker_auth(&worker_subject, &worker_roles, api_token.as_deref())
            .worker_environment(worker_environment_id.as_deref())
            .worker_pool(worker_pool.as_deref())
            .timeout(Duration::from_millis(poll_interval_ms + 5_000))
            .send()
            .await
            .ok()
            .map(|r| r.status() == StatusCode::OK)
            .unwrap_or(false);
        if !notified {
            // Endpoint not available or timed out — brief sleep to avoid
            // hammering the API if the wait endpoint is down.
            sleep(Duration::from_millis(poll_interval_ms)).await;
        }
    }
}

async fn process_workflow_step_jobs(
    client: &reqwest::Client,
    base_url: &str,
    worker_id: &str,
    worker_subject: &str,
    worker_roles: &str,
    api_token: Option<&str>,
    worker_environment_id: Option<&str>,
    worker_pool: Option<&str>,
) -> Result<usize> {
    let Some(board) = fetch_job_item::<TaskBoardSnapshot>(
        client
            .get(format!("{base_url}/api/task-board"))
            .worker_auth(worker_subject, worker_roles, api_token)
            .worker_environment(worker_environment_id)
            .worker_pool(worker_pool),
        "task board",
    )
    .await
    else {
        return Ok(0);
    };
    let mut processed = 0usize;
    for item in board.items.into_iter().filter(|item| item.claimable) {
        let Some(agent_id) = item
            .agent_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            eprintln!(
                "workflow step not runnable: {} missing agent_id",
                item.workflow_step_run_id
            );
            continue;
        };
        let response = client
            .post(format!(
                "{base_url}/api/workflow-step-runs/{}/run",
                item.workflow_step_run_id
            ))
            .header("x-mandoforge-worker-id", worker_id)
            .worker_auth(worker_subject, worker_roles, api_token)
            .worker_environment(worker_environment_id)
            .worker_pool(worker_pool)
            .json(&RunWorkflowStepRunRequest {
                agent_id,
                worker_id,
                lease_seconds: 600,
            })
            .send()
            .await
            .with_context(|| format!("run workflow step {}", item.workflow_step_run_id))?;
        if response.status() == StatusCode::NOT_FOUND
            || response.status() == StatusCode::BAD_REQUEST
        {
            eprintln!("workflow step not claimable: {}", item.workflow_step_run_id);
            continue;
        }
        let updated: RunWorkflowStepRunResponse = match response.error_for_status() {
            Ok(response) => match response.json().await {
                Ok(updated) => updated,
                Err(error) => {
                    eprintln!(
                        "parse workflow step {} run response failed: {error}",
                        item.workflow_step_run_id
                    );
                    continue;
                }
            },
            Err(error) => {
                eprintln!(
                    "run workflow step {} failed: {error}",
                    item.workflow_step_run_id
                );
                continue;
            }
        };
        processed += 1;
        println!(
            "workflow step attempt finished: {} status={}",
            updated.step.id, updated.step.status
        );
    }
    Ok(processed)
}

async fn fetch_job_list<T>(request: reqwest::RequestBuilder, label: &str) -> Vec<T>
where
    T: DeserializeOwned,
{
    let response = match request.send().await {
        Ok(response) => response,
        Err(error) => {
            eprintln!("list {label} failed: {error}");
            return Vec::new();
        }
    };
    let response = match response.error_for_status() {
        Ok(response) => response,
        Err(error) => {
            eprintln!("list {label} failed: {error}");
            return Vec::new();
        }
    };
    match response.json().await {
        Ok(jobs) => jobs,
        Err(error) => {
            eprintln!("parse {label} failed: {error}");
            Vec::new()
        }
    }
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

        let jobs: Vec<SessionLoopJob> = fetch_job_list(
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

        let jobs: Vec<ExecutionJob> = fetch_job_list(
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

        let processed = process_workflow_step_jobs(
            &client,
            &base_url,
            "worker-test-1",
            "worker-subject",
            "admin",
            None,
            None,
            None,
        )
        .await
        .expect("process workflow step jobs");

        assert_eq!(processed, 1);
        assert_eq!(run_count.load(Ordering::SeqCst), 1);
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
