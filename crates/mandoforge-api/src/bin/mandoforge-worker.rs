use std::{env, time::Duration};

use anyhow::{Context, Result, bail};
use reqwest::StatusCode;
use serde::Deserialize;
use tokio::time::sleep;

#[derive(Debug, Deserialize)]
struct ExecutionJob {
    id: String,
    status: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let base_url = env::var("BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:8787".to_string());
    let worker_id = env::var("WORKER_ID")
        .unwrap_or_else(|_| format!("mandoforge-worker-{}", std::process::id()));
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
    client
        .get(format!("{base_url}/healthz"))
        .send()
        .await
        .context("call API healthz")?
        .error_for_status()
        .context("API healthz failed")?;

    let mut processed = 0usize;
    loop {
        let jobs: Vec<ExecutionJob> = client
            .get(format!("{base_url}/api/execution-jobs"))
            .send()
            .await
            .context("list execution jobs")?
            .error_for_status()
            .context("list execution jobs failed")?
            .json()
            .await
            .context("parse execution jobs")?;

        for job in jobs
            .into_iter()
            .filter(|job| job.status == "queued" || job.status == "running")
        {
            let response = client
                .post(format!("{base_url}/api/execution-jobs/{}/run", job.id))
                .header("x-mandoforge-worker-id", &worker_id)
                .send()
                .await
                .with_context(|| format!("run execution job {}", job.id))?;
            if response.status() == StatusCode::NOT_FOUND
                || response.status() == StatusCode::BAD_REQUEST
            {
                eprintln!("execution job not claimable: {}", job.id);
                continue;
            }
            response
                .error_for_status()
                .with_context(|| format!("run execution job {} failed", job.id))?;
            processed += 1;
            println!("execution job completed: {}", job.id);
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
