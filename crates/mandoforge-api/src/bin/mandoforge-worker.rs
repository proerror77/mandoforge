use std::{env, time::Duration};

use anyhow::{Context, Result, bail};
use reqwest::StatusCode;
use serde::{Deserialize, de::DeserializeOwned};
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

#[tokio::main]
async fn main() -> Result<()> {
    let base_url = env::var("BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:8787".to_string());
    let worker_id = env::var("WORKER_ID")
        .unwrap_or_else(|_| format!("mandoforge-worker-{}", std::process::id()));
    let worker_subject =
        env::var("WORKER_SUBJECT").unwrap_or_else(|_| "mandoforge-worker".to_string());
    let worker_roles = env::var("WORKER_ROLES").unwrap_or_else(|_| "admin".to_string());
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
    let poll_interval = env::var("POLL_INTERVAL_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(2);
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
                .worker_auth(&worker_subject, &worker_roles, api_token.as_deref()),
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
                .worker_auth(&worker_subject, &worker_roles, api_token.as_deref()),
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

        if run_once {
            println!("mandoforge worker processed {processed} job(s)");
            return Ok(());
        }
        if poll_interval == 0 {
            bail!("POLL_INTERVAL_SECONDS=0 requires RUN_ONCE=1 or MAX_JOBS > 0");
        }
        sleep(Duration::from_secs(poll_interval)).await;
    }
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

trait WorkerAuthRequestBuilder {
    fn worker_auth(
        self,
        subject: &str,
        roles: &str,
        token: Option<&str>,
    ) -> reqwest::RequestBuilder;
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
    use axum::{Router, routing::get};
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
}
