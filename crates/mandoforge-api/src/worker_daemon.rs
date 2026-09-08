use std::{
    collections::{HashSet, VecDeque},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use axum::http::{HeaderMap, HeaderValue};
use chrono::Utc;
use uuid::Uuid;

use crate::{
    AppState, RunWorkflowStepRun, StoreBackend,
    handlers::{execution_jobs, workflows},
};

use crate::worker_scheduler::{
    Work, WorkKind, WorkerBackend, concurrency_from_lookup, interleave_work, run_worker,
    work_attempt_result,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProcessRole {
    Api,
    Worker,
}

impl ProcessRole {
    pub(crate) fn from_env() -> Result<Self> {
        Self::from_lookup(&|key| std::env::var(key).ok())
    }

    fn from_lookup<F>(lookup: &F) -> Result<Self>
    where
        F: Fn(&str) -> Option<String>,
    {
        let value = lookup("MANDOFORGE_PROCESS_ROLE")
            .unwrap_or_else(|| "api".to_string())
            .trim()
            .to_ascii_lowercase();
        match value.as_str() {
            "" | "api" => Ok(Self::Api),
            "worker" => Ok(Self::Worker),
            other => bail!("unsupported MANDOFORGE_PROCESS_ROLE={other}; use api or worker"),
        }
    }

    pub(crate) fn seeds_demo_data(self) -> bool {
        self == Self::Api
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkerDaemonConfig {
    worker_id: String,
    worker_environment_id: Option<String>,
    worker_pool: Option<String>,
    worker_token: String,
    poll_interval: Duration,
    max_jobs: usize,
    concurrency: usize,
    run_once: bool,
    lease_seconds: i64,
}

impl WorkerDaemonConfig {
    fn from_env() -> Result<Self> {
        Self::from_lookup(&|key| std::env::var(key).ok())
    }

    fn from_lookup<F>(lookup: &F) -> Result<Self>
    where
        F: Fn(&str) -> Option<String>,
    {
        let database_url = lookup("DATABASE_URL")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        if database_url.is_none() {
            bail!("MANDOFORGE_PROCESS_ROLE=worker requires DATABASE_URL");
        }
        let worker_token = required_env(lookup, "MANDOFORGE_WORKER_TOKEN")?;
        let worker_id = required_env(lookup, "WORKER_ID")?;
        if crate::execution::remote_computer_pod_execution_requested_from_lookup(lookup) {
            bail!(
                "MANDOFORGE_PROCESS_ROLE=worker cannot own Kubernetes Remote Computer live execution; deploy a separate narrow Kubernetes bridge with scoped RBAC, and do not grant Kubernetes API credentials to the queue worker"
            );
        }
        let worker_environment_id = optional_env(lookup, "WORKER_ENVIRONMENT_ID");
        let worker_pool =
            optional_env(lookup, "WORKER_POOL").or_else(|| optional_env(lookup, "WORKER_QUEUE"));
        let poll_interval = lookup("POLL_INTERVAL_SECONDS")
            .and_then(|value| value.trim().parse::<f64>().ok())
            .map(|seconds| Duration::from_millis((seconds * 1000.0) as u64))
            .unwrap_or_else(|| Duration::from_millis(2_000))
            .max(Duration::from_millis(100));
        let max_jobs = lookup("MAX_JOBS")
            .and_then(|value| value.trim().parse::<usize>().ok())
            .unwrap_or(0);
        let run_once = lookup("RUN_ONCE").is_some_and(|value| value.trim() == "1");
        let lease_seconds = lookup("MANDOFORGE_WORKER_LEASE_SECONDS")
            .map(|value| {
                value
                    .trim()
                    .parse::<i64>()
                    .context("MANDOFORGE_WORKER_LEASE_SECONDS must be an integer")
            })
            .transpose()?
            .unwrap_or(600);
        if !(180..=86_400).contains(&lease_seconds) {
            bail!("MANDOFORGE_WORKER_LEASE_SECONDS must be between 180 and 86400");
        }

        Ok(Self {
            worker_id,
            worker_environment_id,
            worker_pool,
            worker_token,
            poll_interval,
            max_jobs,
            concurrency: concurrency_from_lookup(lookup)?,
            run_once,
            lease_seconds,
        })
    }

    fn worker_headers(&self, tenant_id: Uuid) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_str(&format!("Bearer {}", self.worker_token))
                .context("worker token is not a valid header value")?,
        );
        headers.insert(
            "x-mandoforge-worker-id",
            HeaderValue::from_str(&self.worker_id)
                .context("worker id is not a valid header value")?,
        );
        headers.insert(
            "x-mandoforge-tenant-id",
            HeaderValue::from_str(&tenant_id.to_string())
                .context("worker tenant id is not a valid header value")?,
        );
        if let Some(environment_id) = self.worker_environment_id.as_deref() {
            headers.insert(
                "x-mandoforge-environment-id",
                HeaderValue::from_str(environment_id)
                    .context("worker environment id is not a valid header value")?,
            );
        }
        if let Some(worker_pool) = self.worker_pool.as_deref() {
            headers.insert(
                "x-mandoforge-worker-pool",
                HeaderValue::from_str(worker_pool)
                    .context("worker pool is not a valid header value")?,
            );
        }
        Ok(headers)
    }
}

fn required_env<F>(lookup: &F, key: &str) -> Result<String>
where
    F: Fn(&str) -> Option<String>,
{
    lookup(key)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .with_context(|| format!("{key} is required"))
}

fn optional_env<F>(lookup: &F, key: &str) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    lookup(key)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) async fn run_worker_daemon(state: AppState) -> Result<()> {
    let config = WorkerDaemonConfig::from_env()?;
    if !matches!(state.store, StoreBackend::Postgres(_)) {
        bail!("MANDOFORGE_PROCESS_ROLE=worker requires Postgres-backed state");
    }
    let headers = config.worker_headers(state.configured_tenant_id())?;
    let worker = ManagedWorker {
        state,
        headers,
        worker_id: config.worker_id.clone(),
        lease_seconds: config.lease_seconds,
    };
    let processed = run_worker(
        &worker,
        config.concurrency,
        config.poll_interval,
        config.max_jobs,
        config.run_once,
    )
    .await?;
    println!("mandoforge worker processed {processed} job(s)");
    Ok(())
}

struct ManagedWorker {
    state: AppState,
    headers: HeaderMap,
    worker_id: String,
    lease_seconds: i64,
}

#[async_trait::async_trait]
impl WorkerBackend for ManagedWorker {
    async fn discover(&self) -> Result<VecDeque<Work>> {
        let (loops, executions, board) = tokio::try_join!(
            execution_jobs::worker_list_session_loop_jobs(&self.state, &self.headers),
            execution_jobs::worker_list_execution_jobs(&self.state, &self.headers),
            workflows::worker_get_task_board(&self.state, &self.headers),
        )
        .map_err(|error| anyhow::anyhow!("worker discovery: {}", error.message))?;
        let now = Utc::now();
        let occupied: HashSet<_> = loops
            .iter()
            .filter(|job| {
                job.status == crate::SessionLoopJobStatus::Running
                    && !session_loop_job_ready_for_worker(job, now)
            })
            .map(|job| job.session_id.to_string())
            .chain(
                executions
                    .iter()
                    .filter(|job| {
                        matches!(
                            job.status,
                            crate::ExecutionJobStatus::Running
                                | crate::ExecutionJobStatus::Executing
                                | crate::ExecutionJobStatus::Finalizing
                                | crate::ExecutionJobStatus::CancelRequested
                        ) && !execution_job_ready_for_worker(job, now)
                    })
                    .map(|job| job.session_id.to_string()),
            )
            .collect();
        let loop_work = loops
            .into_iter()
            .filter(|job| session_loop_job_ready_for_worker(job, now))
            .map(|job| Work {
                id: job.id.to_string(),
                session_id: Some(job.session_id.to_string()),
                kind: WorkKind::SessionLoop,
            })
            .collect();
        let execution_work = executions
            .into_iter()
            .filter(|job| execution_job_ready_for_worker(job, now))
            .map(|job| Work {
                id: job.id.to_string(),
                session_id: Some(job.session_id.to_string()),
                kind: WorkKind::Execution,
            })
            .collect();
        let mut step_work = VecDeque::new();
        for item in board.items.into_iter().filter(|item| item.claimable) {
            let Some(agent_id) = item.agent_id else {
                continue;
            };
            let step = self
                .state
                .get_workflow_step_run(item.workflow_step_run_id)
                .await
                .map_err(|error| anyhow::anyhow!("workflow identity: {}", error.message))?;
            let session_id = match step.session_id {
                Some(id) => id,
                None => {
                    self.state
                        .get_workflow_run(step.workflow_run_id)
                        .await
                        .map_err(|error| anyhow::anyhow!("workflow identity: {}", error.message))?
                        .primary_session_id
                }
            };
            step_work.push_back(Work {
                id: step.id.to_string(),
                session_id: Some(session_id.to_string()),
                kind: WorkKind::WorkflowStep {
                    agent_id: agent_id.to_string(),
                },
            });
        }
        Ok(interleave_work(
            [loop_work, execution_work, step_work],
            &occupied,
        ))
    }

    async fn run(&self, work: &Work) -> Result<bool> {
        let id = Uuid::parse_str(&work.id)?;
        let result = match &work.kind {
            WorkKind::SessionLoop => {
                execution_jobs::worker_run_session_loop_job(&self.state, id, &self.headers)
                    .await
                    .map(|job| job.status.as_str().to_string())
            }
            WorkKind::Execution => {
                let job =
                    self.state.execution_queue.get(id).await.map_err(|error| {
                        anyhow::anyhow!("execution identity: {}", error.message)
                    })?;
                // Preserve recovery semantics: uncertain writes are reconciled,
                // never sent through the normal executor a second time.
                let result = match job.status {
                    crate::ExecutionJobStatus::Executing => {
                        execution_jobs::worker_record_execution_outcome_unknown(
                            &self.state,
                            id,
                            &self.headers,
                        )
                        .await
                    }
                    crate::ExecutionJobStatus::CancelRequested => {
                        execution_jobs::worker_recover_execution_cancellation(
                            &self.state,
                            id,
                            &self.headers,
                        )
                        .await
                    }
                    _ => {
                        execution_jobs::worker_run_execution_job(&self.state, id, &self.headers)
                            .await
                    }
                };
                result.map(|job| job.status.as_str().to_string())
            }
            WorkKind::WorkflowStep { agent_id } => workflows::worker_run_workflow_step_run(
                &self.state,
                id,
                &self.headers,
                RunWorkflowStepRun {
                    agent_id: Some(Uuid::parse_str(agent_id)?),
                    worker_id: Some(self.worker_id.clone()),
                    lease_seconds: Some(self.lease_seconds),
                },
            )
            .await
            .map(|response| response.step.status),
        };
        match result {
            Ok(status) => {
                println!("work attempt finished: {} status={status}", work.key());
                work_attempt_result(&status)
            }
            Err(error) if worker_claim_rejected(&error) => Ok(false),
            Err(error) => bail!("{}: {}", work.key(), error.message),
        }
    }
}

fn session_loop_job_ready_for_worker(
    job: &crate::SessionLoopJob,
    now: chrono::DateTime<Utc>,
) -> bool {
    job.status == crate::SessionLoopJobStatus::Queued
        || (job.status == crate::SessionLoopJobStatus::Running
            && job
                .lease_expires_at
                .is_none_or(|lease_expires_at| lease_expires_at <= now))
}

fn execution_job_ready_for_worker(
    job: &crate::execution_queue::ExecutionJob,
    now: chrono::DateTime<Utc>,
) -> bool {
    job.status == crate::ExecutionJobStatus::Queued
        || (job.status == crate::ExecutionJobStatus::Completed
            && job.finalization_details["stage"] == "completion_pending")
        || (job.status == crate::ExecutionJobStatus::Failed
            && job.finalization_details["stage"] == "failure_pending")
        || (matches!(
            job.status,
            crate::ExecutionJobStatus::Running
                | crate::ExecutionJobStatus::Executing
                | crate::ExecutionJobStatus::Finalizing
                | crate::ExecutionJobStatus::CancelRequested
        ) && job
            .lease_expires_at
            .is_none_or(|lease_expires_at| lease_expires_at <= now))
}

fn worker_claim_rejected(error: &crate::AppError) -> bool {
    matches!(
        error.status,
        axum::http::StatusCode::BAD_REQUEST | axum::http::StatusCode::NOT_FOUND
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn execution_job(
        status: crate::ExecutionJobStatus,
        lease_expires_at: Option<chrono::DateTime<Utc>>,
    ) -> crate::execution_queue::ExecutionJob {
        crate::execution_queue::ExecutionJob {
            id: uuid::Uuid::new_v4(),
            session_id: uuid::Uuid::new_v4(),
            environment_id: None,
            approval_id: uuid::Uuid::new_v4(),
            tool_call_id: uuid::Uuid::new_v4(),
            tool_name: "shell.exec".to_string(),
            status,
            enqueued_at: Utc::now(),
            started_at: None,
            completed_at: None,
            worker_id: None,
            lease_expires_at,
            claim_generation: 1,
            finalization_details: serde_json::json!({}),
            attempt_count: 0,
            max_attempts: 3,
            last_error: None,
        }
    }

    #[test]
    fn process_role_defaults_to_api() {
        let role = ProcessRole::from_lookup(&|_| None).expect("default process role");
        assert_eq!(role, ProcessRole::Api);
    }

    #[test]
    fn worker_process_role_does_not_seed_demo_data() {
        assert!(ProcessRole::Api.seeds_demo_data());
        assert!(!ProcessRole::Worker.seeds_demo_data());
    }

    #[test]
    fn worker_concurrency_is_bounded_for_both_transports() {
        assert_eq!(concurrency_from_lookup(&|_| None).unwrap(), 4);
        assert_eq!(concurrency_from_lookup(&|_| Some("1".into())).unwrap(), 1);
        for value in ["0", "33", "unlimited"] {
            assert!(concurrency_from_lookup(&|_| Some(value.into())).is_err());
        }
    }

    #[test]
    fn daemon_revisits_only_available_execution_jobs() {
        let now = Utc::now();
        let expired = Some(now - chrono::Duration::seconds(1));
        let active = Some(now + chrono::Duration::seconds(1));

        assert!(execution_job_ready_for_worker(
            &execution_job(crate::ExecutionJobStatus::Queued, None),
            now
        ));
        assert!(execution_job_ready_for_worker(
            &execution_job(crate::ExecutionJobStatus::Running, expired),
            now
        ));
        assert!(execution_job_ready_for_worker(
            &execution_job(crate::ExecutionJobStatus::Executing, expired),
            now
        ));
        assert!(execution_job_ready_for_worker(
            &execution_job(crate::ExecutionJobStatus::Finalizing, expired),
            now
        ));
        assert!(execution_job_ready_for_worker(
            &execution_job(crate::ExecutionJobStatus::CancelRequested, expired),
            now
        ));
        assert!(!execution_job_ready_for_worker(
            &execution_job(crate::ExecutionJobStatus::Running, active),
            now
        ));
        assert!(!execution_job_ready_for_worker(
            &execution_job(crate::ExecutionJobStatus::Executing, active),
            now
        ));
        assert!(!execution_job_ready_for_worker(
            &execution_job(crate::ExecutionJobStatus::Finalizing, active),
            now
        ));
        let mut completion_pending = execution_job(crate::ExecutionJobStatus::Completed, None);
        completion_pending.finalization_details =
            serde_json::json!({"stage": "completion_pending"});
        assert!(execution_job_ready_for_worker(&completion_pending, now));
        let mut failure_pending = execution_job(crate::ExecutionJobStatus::Failed, None);
        failure_pending.finalization_details = serde_json::json!({"stage": "failure_pending"});
        assert!(execution_job_ready_for_worker(&failure_pending, now));
        assert!(!execution_job_ready_for_worker(
            &execution_job(crate::ExecutionJobStatus::Completed, None),
            now
        ));
        assert!(!execution_job_ready_for_worker(
            &execution_job(crate::ExecutionJobStatus::OutcomeUnknown, None),
            now
        ));
    }

    #[test]
    fn worker_config_requires_database_token_and_worker_id() {
        let error = WorkerDaemonConfig::from_lookup(&|key| match key {
            "DATABASE_URL" => Some("postgres://db".to_string()),
            "MANDOFORGE_WORKER_TOKEN" => Some("worker-token".to_string()),
            _ => None,
        })
        .expect_err("missing WORKER_ID should fail");
        assert!(error.to_string().contains("WORKER_ID"));
    }

    #[test]
    fn worker_config_builds_headers_from_worker_scope() {
        let config = WorkerDaemonConfig::from_lookup(&|key| match key {
            "DATABASE_URL" => Some("postgres://db".to_string()),
            "MANDOFORGE_WORKER_TOKEN" => Some("worker-token".to_string()),
            "WORKER_ID" => Some("worker-a".to_string()),
            "WORKER_POOL" => Some("isolated".to_string()),
            "WORKER_ENVIRONMENT_ID" => Some("00000000-0000-4000-8000-000000000001".to_string()),
            _ => None,
        })
        .expect("worker config");

        let tenant_id = uuid::Uuid::new_v4();
        let tenant_id_header = tenant_id.to_string();
        let headers = config.worker_headers(tenant_id).expect("worker headers");
        assert_eq!(
            headers
                .get("authorization")
                .and_then(|value| value.to_str().ok()),
            Some("Bearer worker-token")
        );
        assert_eq!(
            headers
                .get("x-mandoforge-worker-id")
                .and_then(|value| value.to_str().ok()),
            Some("worker-a")
        );
        assert_eq!(
            headers
                .get("x-mandoforge-worker-pool")
                .and_then(|value| value.to_str().ok()),
            Some("isolated")
        );
        assert_eq!(
            headers
                .get("x-mandoforge-tenant-id")
                .and_then(|value| value.to_str().ok()),
            Some(tenant_id_header.as_str())
        );
    }

    #[test]
    fn worker_config_rejects_kubernetes_live_execution_without_bridge() {
        let error = WorkerDaemonConfig::from_lookup(&|key| match key {
            "DATABASE_URL" => Some("postgres://db".to_string()),
            "MANDOFORGE_WORKER_TOKEN" => Some("worker-token".to_string()),
            "WORKER_ID" => Some("worker-a".to_string()),
            "MANDOFORGE_REMOTE_COMPUTER_EXECUTION_TRANSPORT" => Some("kubernetes".to_string()),
            "MANDOFORGE_REMOTE_COMPUTER_EXECUTION_ENABLED"
            | "MANDOFORGE_REMOTE_COMPUTER_MUTATION_ENABLED"
            | "MANDOFORGE_REMOTE_COMPUTER_LIVE_MUTATION_ENABLED" => Some("true".to_string()),
            _ => None,
        })
        .expect_err("worker must not own Kubernetes live execution");

        assert!(error.to_string().contains("narrow Kubernetes bridge"));
    }

    #[test]
    fn worker_config_rejects_lease_shorter_than_heartbeat_window() {
        let error = WorkerDaemonConfig::from_lookup(&|key| match key {
            "DATABASE_URL" => Some("postgres://db".to_string()),
            "MANDOFORGE_WORKER_TOKEN" => Some("worker-token".to_string()),
            "WORKER_ID" => Some("worker-a".to_string()),
            "MANDOFORGE_WORKER_LEASE_SECONDS" => Some("60".to_string()),
            _ => None,
        })
        .expect_err("short worker lease should fail");

        assert!(error.to_string().contains("between 180 and 86400"));
    }
}
