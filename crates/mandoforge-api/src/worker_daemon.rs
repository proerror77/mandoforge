use std::time::Duration;

use anyhow::{Context, Result, bail};
use axum::http::{HeaderMap, HeaderValue};
use chrono::Utc;
use tokio::time::sleep;

use crate::{
    AppState, RunWorkflowStepRun, StoreBackend,
    handlers::{execution_jobs, workflows},
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkerDaemonConfig {
    worker_id: String,
    worker_environment_id: Option<String>,
    worker_pool: Option<String>,
    worker_token: String,
    poll_interval: Duration,
    max_jobs: usize,
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
            run_once,
            lease_seconds,
        })
    }

    fn worker_headers(&self) -> Result<HeaderMap> {
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

    let headers = config.worker_headers()?;
    let mut processed = 0usize;

    loop {
        processed += process_session_loop_jobs(
            &state,
            &headers,
            remaining_job_budget(config.max_jobs, processed),
        )
        .await;
        if should_stop(&config, processed) {
            return Ok(());
        }

        processed += process_execution_jobs(
            &state,
            &headers,
            remaining_job_budget(config.max_jobs, processed),
        )
        .await;
        if should_stop(&config, processed) {
            return Ok(());
        }

        processed += process_workflow_step_jobs(
            &state,
            &headers,
            &config,
            remaining_job_budget(config.max_jobs, processed),
        )
        .await;
        if should_stop(&config, processed) {
            return Ok(());
        }

        if config.run_once {
            return Ok(());
        }

        sleep(config.poll_interval).await;
    }
}

fn should_stop(config: &WorkerDaemonConfig, processed: usize) -> bool {
    config.max_jobs != 0 && processed >= config.max_jobs
}

fn remaining_job_budget(max_jobs: usize, processed: usize) -> usize {
    if max_jobs == 0 {
        usize::MAX
    } else {
        max_jobs.saturating_sub(processed)
    }
}

async fn process_session_loop_jobs(
    state: &AppState,
    headers: &HeaderMap,
    job_budget: usize,
) -> usize {
    if job_budget == 0 {
        return 0;
    }
    let jobs = match execution_jobs::worker_list_session_loop_jobs(state, headers).await {
        Ok(jobs) => jobs,
        Err(error) => {
            eprintln!("list session loop jobs failed: {}", error.message);
            return 0;
        }
    };
    let mut processed = 0usize;
    let now = Utc::now();
    for job in jobs
        .into_iter()
        .filter(|job| session_loop_job_ready_for_worker(job, now))
    {
        if processed >= job_budget {
            break;
        }
        match execution_jobs::worker_run_session_loop_job(state, job.id, headers).await {
            Ok(updated) => {
                processed += 1;
                println!(
                    "session loop job attempt finished: {} status={:?}",
                    updated.id, updated.status
                );
            }
            Err(error) if worker_claim_rejected(&error) => {
                eprintln!("session loop job not claimable: {}", job.id);
            }
            Err(error) => {
                eprintln!("run session loop job {} failed: {}", job.id, error.message);
            }
        }
    }
    processed
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

async fn process_execution_jobs(state: &AppState, headers: &HeaderMap, job_budget: usize) -> usize {
    if job_budget == 0 {
        return 0;
    }
    let jobs = match execution_jobs::worker_list_execution_jobs(state, headers).await {
        Ok(jobs) => jobs,
        Err(error) => {
            eprintln!("list execution jobs failed: {}", error.message);
            return 0;
        }
    };
    let mut processed = 0usize;
    let now = Utc::now();
    for job in jobs
        .into_iter()
        .filter(|job| execution_job_ready_for_worker(job, now))
    {
        if processed >= job_budget {
            break;
        }
        if job.status == crate::ExecutionJobStatus::CancelRequested {
            match execution_jobs::worker_recover_execution_cancellation(state, job.id, headers)
                .await
            {
                Ok(updated) => {
                    processed += 1;
                    println!(
                        "execution cancellation recovered: {} status={:?}",
                        updated.id, updated.status
                    );
                }
                Err(error) if worker_claim_rejected(&error) => {
                    eprintln!("execution cancellation not recoverable: {}", job.id);
                }
                Err(error) => {
                    eprintln!(
                        "recover execution cancellation {} failed: {}",
                        job.id, error.message
                    );
                }
            }
            continue;
        }
        match execution_jobs::worker_run_execution_job(state, job.id, headers).await {
            Ok(updated) => {
                processed += 1;
                println!(
                    "execution job attempt finished: {} status={:?}",
                    updated.id, updated.status
                );
            }
            Err(error) if worker_claim_rejected(&error) => {
                eprintln!("execution job not claimable: {}", job.id);
            }
            Err(error) => {
                eprintln!("run execution job {} failed: {}", job.id, error.message);
            }
        }
    }
    processed
}

fn execution_job_ready_for_worker(
    job: &crate::execution_queue::ExecutionJob,
    now: chrono::DateTime<Utc>,
) -> bool {
    job.status == crate::ExecutionJobStatus::Queued
        || (matches!(
            job.status,
            crate::ExecutionJobStatus::Running | crate::ExecutionJobStatus::CancelRequested
        ) && job
            .lease_expires_at
            .is_none_or(|lease_expires_at| lease_expires_at <= now))
}

async fn process_workflow_step_jobs(
    state: &AppState,
    headers: &HeaderMap,
    config: &WorkerDaemonConfig,
    job_budget: usize,
) -> usize {
    if job_budget == 0 {
        return 0;
    }
    let board = match workflows::worker_get_task_board(state, headers).await {
        Ok(board) => board,
        Err(error) => {
            eprintln!("get task board failed: {}", error.message);
            return 0;
        }
    };
    let mut processed = 0usize;
    for item in board
        .items
        .into_iter()
        .filter(|item| item.claimable)
        .filter(|item| item.status == "queued" || item.status == "scheduled")
    {
        if processed >= job_budget {
            break;
        }
        let Some(agent_id) = item.agent_id else {
            eprintln!(
                "workflow step not runnable: {} missing agent_id",
                item.workflow_step_run_id
            );
            continue;
        };
        match workflows::worker_run_workflow_step_run(
            state,
            item.workflow_step_run_id,
            headers,
            RunWorkflowStepRun {
                agent_id: Some(agent_id),
                worker_id: Some(config.worker_id.clone()),
                lease_seconds: Some(config.lease_seconds),
            },
        )
        .await
        {
            Ok(updated) => {
                processed += 1;
                println!(
                    "workflow step attempt finished: {} status={}",
                    updated.step.id, updated.step.status
                );
            }
            Err(error) if worker_claim_rejected(&error) => {
                eprintln!("workflow step not claimable: {}", item.workflow_step_run_id);
            }
            Err(error) => {
                eprintln!(
                    "run workflow step {} failed: {}",
                    item.workflow_step_run_id, error.message
                );
            }
        }
    }
    processed
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
    fn max_jobs_budget_is_hard_bounded() {
        assert_eq!(remaining_job_budget(0, 42), usize::MAX);
        assert_eq!(remaining_job_budget(3, 1), 2);
        assert_eq!(remaining_job_budget(3, 3), 0);
        assert_eq!(remaining_job_budget(3, 4), 0);
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
            &execution_job(crate::ExecutionJobStatus::CancelRequested, expired),
            now
        ));
        assert!(!execution_job_ready_for_worker(
            &execution_job(crate::ExecutionJobStatus::Running, active),
            now
        ));
        assert!(!execution_job_ready_for_worker(
            &execution_job(crate::ExecutionJobStatus::Completed, None),
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

        let headers = config.worker_headers().expect("worker headers");
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
