use anyhow::{Result, bail};
use futures_util::{StreamExt, stream::FuturesUnordered};
use std::{
    collections::{HashSet, VecDeque},
    time::Duration,
};
use tokio::time::{Instant, sleep};

#[derive(Debug)]
pub(crate) enum WorkKind {
    SessionLoop,
    Execution,
    WorkflowStep { agent_id: String },
}

#[derive(Debug)]
pub(crate) struct Work {
    pub(crate) id: String,
    pub(crate) session_id: Option<String>,
    pub(crate) kind: WorkKind,
}

impl Work {
    pub(crate) fn route(&self) -> &'static str {
        match self.kind {
            WorkKind::SessionLoop => "session-loop-jobs",
            WorkKind::Execution => "execution-jobs",
            WorkKind::WorkflowStep { .. } => "workflow-step-runs",
        }
    }

    pub(crate) fn key(&self) -> String {
        format!("{}/{}", self.route(), self.id)
    }
}

#[async_trait::async_trait]
pub(crate) trait WorkerBackend {
    async fn discover(&self) -> Result<VecDeque<Work>>;
    async fn run(&self, work: &Work) -> Result<bool>;
    async fn wait_for_work(&self, interval: Duration) {
        sleep(interval).await;
    }
}

pub(crate) fn work_attempt_result(status: &str) -> Result<bool> {
    match status {
        "queued" | "running" | "executing" | "finalizing" | "completed" | "requires_action"
        | "cancel_requested" | "canceled" | "skipped" | "scheduled" => Ok(true),
        "failed" | "outcome_unknown" => bail!("work ended with status {status}"),
        _ => bail!("unsupported work result status {status}"),
    }
}

pub(crate) fn concurrency_from_lookup(lookup: &impl Fn(&str) -> Option<String>) -> Result<usize> {
    let concurrency = lookup("WORKER_CONCURRENCY")
        .unwrap_or_else(|| "4".into())
        .parse::<usize>()?;
    if !(1..=32).contains(&concurrency) {
        bail!("WORKER_CONCURRENCY must be between 1 and 32");
    }
    Ok(concurrency)
}

pub(crate) fn interleave_work(
    mut lanes: [VecDeque<Work>; 3],
    occupied: &HashSet<String>,
) -> VecDeque<Work> {
    let mut work = VecDeque::new();
    while lanes.iter().any(|lane| !lane.is_empty()) {
        for lane in &mut lanes {
            if let Some(item) = lane.pop_front()
                && item
                    .session_id
                    .as_ref()
                    .is_none_or(|id| !occupied.contains(id))
            {
                work.push_back(item);
            }
        }
    }
    work
}

pub(crate) async fn run_worker<B: WorkerBackend + Sync>(
    worker: &B,
    concurrency: usize,
    interval: Duration,
    max_jobs: usize,
    run_once: bool,
) -> Result<usize> {
    let mut pending = match worker.discover().await {
        Ok(work) => work,
        Err(error) if run_once => return Err(error),
        Err(error) => {
            eprintln!("worker discovery failed: {error:#}");
            VecDeque::new()
        }
    };
    let mut active = FuturesUnordered::new();
    let mut active_ids = HashSet::new();
    let mut active_sessions = HashSet::new();
    let mut exclusive = false;
    let mut processed = 0;
    let mut last_failure = None;
    let mut refresh_after_completion = false;
    let mut refresh_at = Instant::now() + interval;
    loop {
        while active.len() < concurrency
            && (max_jobs == 0 || processed + active.len() < max_jobs)
            && !exclusive
        {
            let index = pending.iter().position(|work| match &work.session_id {
                Some(session) => !active_sessions.contains(session),
                None => active.is_empty(),
            });
            let Some(index) = index else {
                break;
            };
            // Let an older unscoped operation drain the active set before
            // admitting newer operations; unscoped work must not starve.
            if pending
                .iter()
                .take(index)
                .any(|work| work.session_id.is_none())
            {
                break;
            }
            let work = pending.remove(index).expect("selected pending work");
            if !active_ids.insert(work.key()) {
                continue;
            }
            if let Some(session) = &work.session_id {
                active_sessions.insert(session.clone());
            } else {
                exclusive = true;
            }
            active.push(async move {
                let result = worker.run(&work).await;
                (work, result)
            });
        }
        if active.is_empty() {
            if (max_jobs > 0 && processed >= max_jobs) || (run_once && pending.is_empty()) {
                if let Some(error) = last_failure {
                    return Err(error);
                }
                return Ok(processed);
            }
            if !pending.is_empty() {
                continue;
            }
            if !refresh_after_completion {
                worker.wait_for_work(interval).await;
            }
            refresh_after_completion = false;
            pending = worker.discover().await.unwrap_or_else(|error| {
                eprintln!("worker discovery failed: {error:#}");
                VecDeque::new()
            });
            refresh_at = Instant::now() + interval;
            continue;
        }
        tokio::select! {
            Some((work, result)) = active.next() => {
                active_ids.remove(&work.key());
                if result.is_err() {
                    // Do not dispatch a queued sibling until fresh leases show
                    // whether a failed HTTP call is still running remotely.
                    pending.retain(|pending| work.session_id.is_some() && pending.session_id != work.session_id);
                }
                if let Some(session) = work.session_id { active_sessions.remove(&session); } else { exclusive = false; }
                match result {
                    Ok(true) => { processed += 1; refresh_after_completion = true; },
                    Ok(false) => {},
                    Err(error) => { processed += 1; eprintln!("worker attempt failed: {error:#}"); last_failure = Some(error); },
                }
            }
            _ = tokio::time::sleep_until(refresh_at), if !run_once => {
                let mut seen = active_ids.clone();
                pending = worker.discover().await.unwrap_or_else(|error| {
                    eprintln!("worker discovery failed: {error:#}"); VecDeque::new()
                }).into_iter().filter(|work| seen.insert(work.key())).collect();
                refresh_at = Instant::now() + interval;
            }
        }
    }
}
