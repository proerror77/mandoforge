use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::session_loop_job_from_row;
use crate::{AppError, AppState, SessionLoopJob, SessionLoopJobStatus};

impl AppState {
    pub(crate) async fn enqueue_session_loop_job(
        &self,
        session_id: Uuid,
        trigger_event_id: Option<Uuid>,
        reason: &str,
    ) -> Result<SessionLoopJob, AppError> {
        self.enqueue_session_loop_job_from(session_id, trigger_event_id, reason, None)
            .await
    }

    pub(crate) async fn enqueue_session_loop_job_from(
        &self,
        session_id: Uuid,
        trigger_event_id: Option<Uuid>,
        reason: &str,
        pending_event_seq_start_floor: Option<i64>,
    ) -> Result<SessionLoopJob, AppError> {
        let session = self.get_session(session_id).await?;
        let pending_event_seq_end = self
            .session_loop_trigger_event_seq(session_id, trigger_event_id)
            .await?;
        let mut cursor_high_watermark = match self
            .session_loop_cursor_high_watermark(session_id)
            .await?
        {
            Some(seq) => Some(seq),
            None => match (pending_event_seq_start_floor, pending_event_seq_end) {
                (Some(seq_start), _) => Some(seq_start.saturating_sub(1)),
                (None, Some(seq_end)) => self.session_event_seq_before(session_id, seq_end).await?,
                (None, None) => None,
            },
        };
        if let Some(seq_start) = pending_event_seq_start_floor
            && cursor_high_watermark.is_some_and(|high_watermark| high_watermark >= seq_start)
        {
            cursor_high_watermark = Some(seq_start.saturating_sub(1));
        }
        let pending_event_seq_start = pending_event_seq_end.and_then(|seq_end| {
            let seq_start = pending_event_seq_start_floor
                .unwrap_or_else(|| cursor_high_watermark.unwrap_or(0) + 1);
            (seq_start <= seq_end).then_some(seq_start)
        });
        let now = Utc::now();
        let job = SessionLoopJob {
            id: Uuid::new_v4(),
            session_id,
            environment_id: session.environment_id,
            status: SessionLoopJobStatus::Queued,
            trigger_event_id,
            pending_event_seq_start,
            pending_event_seq_end,
            processed_event_seq: cursor_high_watermark,
            reason: reason.to_string(),
            enqueued_at: now,
            started_at: None,
            completed_at: None,
            worker_id: None,
            lease_expires_at: None,
            attempt_count: 0,
            max_attempts: 3,
            last_error: None,
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                if let Some(existing) = store.session_loop_jobs.values_mut().find(|existing| {
                    existing.session_id == session_id
                        && existing.status == SessionLoopJobStatus::Queued
                }) {
                    existing.trigger_event_id = trigger_event_id;
                    if let Some(seq_start) = pending_event_seq_start
                        && existing
                            .pending_event_seq_start
                            .is_none_or(|existing_start| seq_start < existing_start)
                    {
                        existing.pending_event_seq_start = Some(seq_start);
                        existing.processed_event_seq = Some(seq_start.saturating_sub(1));
                    }
                    existing.pending_event_seq_end =
                        max_optional_i64(existing.pending_event_seq_end, pending_event_seq_end);
                    existing.reason = reason.to_string();
                    return Ok(existing.clone());
                }
                store.session_loop_jobs.insert(job.id, job.clone());
                Ok(job)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "INSERT INTO session_loop_jobs
                        (id, tenant_id, session_id, environment_id, status, trigger_event_id,
                         pending_event_seq_start, pending_event_seq_end, processed_event_seq, reason,
                         enqueued_at, started_at, completed_at, worker_id, lease_expires_at,
                         attempt_count, max_attempts, last_error)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, NULL, NULL, NULL, NULL, $12, $13, NULL)
                     ON CONFLICT (tenant_id, session_id)
                     WHERE status = 'queued'
                     DO UPDATE SET trigger_event_id = EXCLUDED.trigger_event_id,
                                   pending_event_seq_start = CASE
                                       WHEN session_loop_jobs.pending_event_seq_start IS NULL THEN EXCLUDED.pending_event_seq_start
                                       WHEN EXCLUDED.pending_event_seq_start IS NULL THEN session_loop_jobs.pending_event_seq_start
                                       ELSE LEAST(session_loop_jobs.pending_event_seq_start, EXCLUDED.pending_event_seq_start)
                                   END,
                                   pending_event_seq_end = NULLIF(GREATEST(COALESCE(session_loop_jobs.pending_event_seq_end, 0), COALESCE(EXCLUDED.pending_event_seq_end, 0)), 0),
                                   processed_event_seq = CASE
                                       WHEN EXCLUDED.pending_event_seq_start IS NOT NULL
                                        AND (session_loop_jobs.pending_event_seq_start IS NULL
                                             OR EXCLUDED.pending_event_seq_start < session_loop_jobs.pending_event_seq_start)
                                           THEN EXCLUDED.processed_event_seq
                                       ELSE session_loop_jobs.processed_event_seq
                                   END,
                                   reason = EXCLUDED.reason
                     RETURNING id, session_id, environment_id, status, trigger_event_id,
                               pending_event_seq_start, pending_event_seq_end, processed_event_seq, reason,
                               enqueued_at, started_at, completed_at, worker_id, lease_expires_at,
                               attempt_count, max_attempts, last_error",
                )
                .bind(job.id)
                .bind(self.current_tenant_id())
                .bind(job.session_id)
                .bind(job.environment_id)
                .bind(job.status.as_str())
                .bind(job.trigger_event_id)
                .bind(job.pending_event_seq_start)
                .bind(job.pending_event_seq_end)
                .bind(job.processed_event_seq)
                .bind(&job.reason)
                .bind(job.enqueued_at)
                .bind(job.attempt_count)
                .bind(job.max_attempts)
                .fetch_one(pool)
                .await?;
                session_loop_job_from_row(row)
            }
        }
    }

    pub(crate) async fn list_session_loop_jobs(&self) -> Result<Vec<SessionLoopJob>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut jobs: Vec<_> = inner
                    .read()
                    .await
                    .session_loop_jobs
                    .values()
                    .cloned()
                    .collect();
                jobs.sort_by_key(|job| job.enqueued_at);
                Ok(jobs)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, session_id, environment_id, status, trigger_event_id,
                            pending_event_seq_start, pending_event_seq_end, processed_event_seq, reason,
                            enqueued_at, started_at, completed_at, worker_id, lease_expires_at,
                            attempt_count, max_attempts, last_error
                     FROM session_loop_jobs
                     WHERE tenant_id = $1
                     ORDER BY enqueued_at ASC",
                )
                .bind(self.current_tenant_id())
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(session_loop_job_from_row).collect()
            }
        }
    }

    pub(crate) async fn get_session_loop_job(&self, id: Uuid) -> Result<SessionLoopJob, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .session_loop_jobs
                .get(&id)
                .cloned()
                .ok_or_else(|| AppError::not_found("session loop job not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, session_id, environment_id, status, trigger_event_id,
                            pending_event_seq_start, pending_event_seq_end, processed_event_seq, reason,
                            enqueued_at, started_at, completed_at, worker_id, lease_expires_at,
                            attempt_count, max_attempts, last_error
                     FROM session_loop_jobs
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("session loop job not found"))?;
                session_loop_job_from_row(row)
            }
        }
    }

    pub(crate) async fn start_session_loop_job(
        &self,
        id: Uuid,
        worker_id: &str,
    ) -> Result<SessionLoopJob, AppError> {
        self.update_session_loop_job_status(
            id,
            SessionLoopJobStatus::Running,
            Some(worker_id),
            None,
        )
        .await
    }

    pub(crate) async fn complete_session_loop_job(
        &self,
        id: Uuid,
        worker_id: &str,
    ) -> Result<SessionLoopJob, AppError> {
        self.update_session_loop_job_status(
            id,
            SessionLoopJobStatus::Completed,
            Some(worker_id),
            None,
        )
        .await
    }

    pub(crate) async fn renew_session_loop_job_lease(
        &self,
        id: Uuid,
        worker_id: &str,
        lease_seconds: i64,
    ) -> Result<SessionLoopJob, AppError> {
        if !(1..=86_400).contains(&lease_seconds) {
            return Err(AppError::bad_request(
                "session loop lease_seconds must be between 1 and 86400",
            ));
        }
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let job = store
                    .session_loop_jobs
                    .get_mut(&id)
                    .filter(|job| {
                        job.status == SessionLoopJobStatus::Running
                            && job.worker_id.as_deref() == Some(worker_id)
                    })
                    .ok_or_else(|| AppError::not_found("session loop job not found"))?;
                job.lease_expires_at = Some(Utc::now() + chrono::Duration::seconds(lease_seconds));
                Ok(job.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE session_loop_jobs
                     SET lease_expires_at = now() + $1 * interval '1 second'
                     WHERE tenant_id = $2 AND id = $3 AND status = 'running' AND worker_id = $4
                     RETURNING id, session_id, environment_id, status, trigger_event_id,
                               pending_event_seq_start, pending_event_seq_end, processed_event_seq, reason,
                               enqueued_at, started_at, completed_at, worker_id, lease_expires_at,
                               attempt_count, max_attempts, last_error",
                )
                .bind(lease_seconds)
                .bind(self.current_tenant_id())
                .bind(id)
                .bind(worker_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("session loop job not found"))?;
                session_loop_job_from_row(row)
            }
        }
    }

    pub(crate) async fn fail_session_loop_job(
        &self,
        id: Uuid,
        worker_id: &str,
        error: &str,
    ) -> Result<SessionLoopJob, AppError> {
        self.update_session_loop_job_status(
            id,
            SessionLoopJobStatus::Failed,
            Some(worker_id),
            Some(error.to_string()),
        )
        .await
    }

    pub(crate) async fn discard_session_loop_job(
        &self,
        id: Uuid,
        error: &str,
    ) -> Result<SessionLoopJob, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let job = store
                    .session_loop_jobs
                    .get_mut(&id)
                    .ok_or_else(|| AppError::not_found("session loop job not found"))?;
                if !matches!(
                    job.status,
                    SessionLoopJobStatus::Queued | SessionLoopJobStatus::Running
                ) {
                    return Err(AppError::not_found("session loop job not found"));
                }
                let worker_id = job.worker_id.clone();
                apply_session_loop_job_status(
                    job,
                    SessionLoopJobStatus::Failed,
                    worker_id.as_deref(),
                    Some(error.to_string()),
                );
                Ok(job.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE session_loop_jobs
                     SET status = 'failed',
                         completed_at = COALESCE(completed_at, now()),
                         lease_expires_at = NULL,
                         last_error = $1
                     WHERE tenant_id = $2 AND id = $3 AND status IN ('queued', 'running')
                     RETURNING id, session_id, environment_id, status, trigger_event_id,
                               pending_event_seq_start, pending_event_seq_end, processed_event_seq, reason,
                               enqueued_at, started_at, completed_at, worker_id, lease_expires_at,
                               attempt_count, max_attempts, last_error",
                )
                .bind(error)
                .bind(self.current_tenant_id())
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("session loop job not found"))?;
                session_loop_job_from_row(row)
            }
        }
    }

    async fn update_session_loop_job_status(
        &self,
        id: Uuid,
        status: SessionLoopJobStatus,
        worker_id: Option<&str>,
        last_error: Option<String>,
    ) -> Result<SessionLoopJob, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                if status == SessionLoopJobStatus::Running {
                    let job = store
                        .session_loop_jobs
                        .get(&id)
                        .ok_or_else(|| AppError::not_found("session loop job not found"))?;
                    let lease_expired = matches!(job.status, SessionLoopJobStatus::Running)
                        && job
                            .lease_expires_at
                            .is_none_or(|expires_at| expires_at <= Utc::now());
                    let other_running_job = store.session_loop_jobs.values().any(|existing| {
                        existing.id != id
                            && existing.session_id == job.session_id
                            && existing.status == SessionLoopJobStatus::Running
                    });
                    let claimable = match job.status {
                        SessionLoopJobStatus::Queued => !other_running_job,
                        SessionLoopJobStatus::Running => lease_expired,
                        SessionLoopJobStatus::Completed | SessionLoopJobStatus::Failed => false,
                    };
                    if !claimable {
                        return Err(AppError::not_found("session loop job not found"));
                    }
                } else if matches!(
                    status,
                    SessionLoopJobStatus::Completed | SessionLoopJobStatus::Failed
                ) {
                    let job = store
                        .session_loop_jobs
                        .get(&id)
                        .ok_or_else(|| AppError::not_found("session loop job not found"))?;
                    if job.status != SessionLoopJobStatus::Running
                        || job.worker_id.as_deref() != worker_id
                    {
                        return Err(AppError::not_found("session loop job not found"));
                    }
                }
                let job = store
                    .session_loop_jobs
                    .get_mut(&id)
                    .ok_or_else(|| AppError::not_found("session loop job not found"))?;
                apply_session_loop_job_status(job, status, worker_id, last_error);
                Ok(job.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = match status {
                    SessionLoopJobStatus::Running => sqlx::query(
                        "UPDATE session_loop_jobs
                         SET status = 'running',
                             started_at = COALESCE(started_at, now()),
                             completed_at = NULL,
                             worker_id = $1,
                             lease_expires_at = now() + interval '5 minutes',
                             attempt_count = attempt_count + 1
                         WHERE tenant_id = $2
                           AND id = $3
                           AND (
                               (
                                   status = 'queued'
                                   AND NOT EXISTS (
                                       SELECT 1
                                       FROM session_loop_jobs running_job
                                       WHERE running_job.tenant_id = session_loop_jobs.tenant_id
                                         AND running_job.session_id = session_loop_jobs.session_id
                                         AND running_job.id <> session_loop_jobs.id
                                         AND running_job.status = 'running'
                                   )
                               )
                               OR (status = 'running' AND (lease_expires_at IS NULL OR lease_expires_at <= now()))
                           )
                         RETURNING id, session_id, environment_id, status, trigger_event_id,
                                   pending_event_seq_start, pending_event_seq_end, processed_event_seq, reason,
                                   enqueued_at, started_at, completed_at, worker_id, lease_expires_at,
                                   attempt_count, max_attempts, last_error",
                    )
                    .bind(worker_id.unwrap_or("session-loop-worker"))
                    .bind(self.current_tenant_id())
                    .bind(id)
                    .fetch_optional(pool)
                    .await?,
                    SessionLoopJobStatus::Completed | SessionLoopJobStatus::Failed => sqlx::query(
                        "UPDATE session_loop_jobs
                         SET status = $1,
                             completed_at = COALESCE(completed_at, now()),
                             lease_expires_at = NULL,
                             processed_event_seq = CASE
                                 WHEN $1 = 'completed' THEN COALESCE(pending_event_seq_end, processed_event_seq)
                                 ELSE processed_event_seq
                             END,
                             last_error = COALESCE($2, last_error)
                         WHERE tenant_id = $3 AND id = $4 AND status = 'running' AND worker_id = $5
                         RETURNING id, session_id, environment_id, status, trigger_event_id,
                                   pending_event_seq_start, pending_event_seq_end, processed_event_seq, reason,
                                   enqueued_at, started_at, completed_at, worker_id, lease_expires_at,
                                   attempt_count, max_attempts, last_error",
                    )
                    .bind(status.as_str())
                    .bind(last_error)
                    .bind(self.current_tenant_id())
                    .bind(id)
                    .bind(worker_id.unwrap_or("session-loop-worker"))
                    .fetch_optional(pool)
                    .await?,
                    SessionLoopJobStatus::Queued => sqlx::query(
                        "UPDATE session_loop_jobs
                         SET status = 'queued',
                             started_at = NULL,
                             completed_at = NULL,
                             worker_id = NULL,
                             lease_expires_at = NULL,
                             last_error = NULL
                         WHERE tenant_id = $1 AND id = $2
                         RETURNING id, session_id, environment_id, status, trigger_event_id,
                                   pending_event_seq_start, pending_event_seq_end, processed_event_seq, reason,
                                   enqueued_at, started_at, completed_at, worker_id, lease_expires_at,
                                   attempt_count, max_attempts, last_error",
                    )
                    .bind(self.current_tenant_id())
                    .bind(id)
                    .fetch_optional(pool)
                    .await?,
                }
                .ok_or_else(|| AppError::not_found("session loop job not found"))?;
                session_loop_job_from_row(row)
            }
        }
    }
}

fn apply_session_loop_job_status(
    job: &mut SessionLoopJob,
    status: SessionLoopJobStatus,
    worker_id: Option<&str>,
    last_error: Option<String>,
) {
    job.status = status;
    match job.status {
        SessionLoopJobStatus::Queued => {
            job.started_at = None;
            job.completed_at = None;
            job.worker_id = None;
            job.lease_expires_at = None;
            job.last_error = None;
        }
        SessionLoopJobStatus::Running => {
            job.started_at = Some(Utc::now());
            job.completed_at = None;
            job.worker_id = worker_id.map(str::to_string);
            job.lease_expires_at = Some(Utc::now() + chrono::Duration::minutes(5));
            job.attempt_count += 1;
        }
        SessionLoopJobStatus::Completed | SessionLoopJobStatus::Failed => {
            job.completed_at = Some(Utc::now());
            job.lease_expires_at = None;
            if job.status == SessionLoopJobStatus::Completed {
                job.processed_event_seq = job.pending_event_seq_end.or(job.processed_event_seq);
            }
            if let Some(last_error) = last_error {
                job.last_error = Some(last_error);
            }
        }
    }
}

impl AppState {
    async fn session_loop_trigger_event_seq(
        &self,
        session_id: Uuid,
        trigger_event_id: Option<Uuid>,
    ) -> Result<Option<i64>, AppError> {
        if let Some(trigger_event_id) = trigger_event_id
            && let Some(seq) = self.session_event_seq(session_id, trigger_event_id).await?
        {
            return Ok(Some(seq));
        }
        self.latest_session_event_seq(session_id).await
    }

    async fn session_event_seq(
        &self,
        session_id: Uuid,
        event_id: Uuid,
    ) -> Result<Option<i64>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                Ok(inner
                    .read()
                    .await
                    .events
                    .get(&session_id)
                    .and_then(|events| {
                        events
                            .iter()
                            .find(|event| event.id == event_id)
                            .map(|event| event.seq)
                    }))
            }
            StoreBackend::Postgres(pool) => {
                let seq = sqlx::query_scalar::<_, i64>(
                    "SELECT seq
                     FROM session_events
                     WHERE tenant_id = $1 AND session_id = $2 AND id = $3",
                )
                .bind(self.current_tenant_id())
                .bind(session_id)
                .bind(event_id)
                .fetch_optional(pool)
                .await?;
                Ok(seq)
            }
        }
    }

    async fn latest_session_event_seq(&self, session_id: Uuid) -> Result<Option<i64>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => Ok(inner
                .read()
                .await
                .events
                .get(&session_id)
                .and_then(|events| events.iter().map(|event| event.seq).max())),
            StoreBackend::Postgres(pool) => {
                let seq = sqlx::query_scalar::<_, Option<i64>>(
                    "SELECT MAX(seq)
                     FROM session_events
                     WHERE tenant_id = $1 AND session_id = $2",
                )
                .bind(self.current_tenant_id())
                .bind(session_id)
                .fetch_one(pool)
                .await?;
                Ok(seq)
            }
        }
    }

    async fn session_event_seq_before(
        &self,
        session_id: Uuid,
        seq: i64,
    ) -> Result<Option<i64>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                Ok(inner
                    .read()
                    .await
                    .events
                    .get(&session_id)
                    .and_then(|events| {
                        events
                            .iter()
                            .filter(|event| event.seq < seq)
                            .map(|event| event.seq)
                            .max()
                    }))
            }
            StoreBackend::Postgres(pool) => {
                let previous_seq = sqlx::query_scalar::<_, Option<i64>>(
                    "SELECT MAX(seq)
                     FROM session_events
                     WHERE tenant_id = $1 AND session_id = $2 AND seq < $3",
                )
                .bind(self.current_tenant_id())
                .bind(session_id)
                .bind(seq)
                .fetch_one(pool)
                .await?;
                Ok(previous_seq)
            }
        }
    }

    async fn session_loop_cursor_high_watermark(
        &self,
        session_id: Uuid,
    ) -> Result<Option<i64>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => Ok(inner
                .read()
                .await
                .session_loop_jobs
                .values()
                .filter(|job| job.session_id == session_id)
                .filter_map(|job| {
                    [
                        job.processed_event_seq,
                        job.pending_event_seq_end.filter(|_| {
                            matches!(
                                job.status,
                                SessionLoopJobStatus::Queued | SessionLoopJobStatus::Running
                            )
                        }),
                    ]
                    .into_iter()
                    .flatten()
                    .max()
                })
                .max()),
            StoreBackend::Postgres(pool) => {
                let seq = sqlx::query_scalar::<_, Option<i64>>(
                    "SELECT MAX(GREATEST(
                         COALESCE(processed_event_seq, 0),
                         CASE
                             WHEN status IN ('queued', 'running') THEN COALESCE(pending_event_seq_end, 0)
                             ELSE 0
                         END
                     ))
                     FROM session_loop_jobs
                     WHERE tenant_id = $1 AND session_id = $2",
                )
                .bind(self.current_tenant_id())
                .bind(session_id)
                .fetch_one(pool)
                .await?
                .filter(|seq| *seq > 0);
                Ok(seq)
            }
        }
    }

    pub(crate) async fn session_loop_projection_covers(
        &self,
        session_id: Uuid,
        event_seq: i64,
    ) -> Result<bool, AppError> {
        Ok(self
            .session_loop_cursor_high_watermark(session_id)
            .await?
            .is_some_and(|high_watermark| high_watermark >= event_seq))
    }

    pub(crate) async fn session_loop_projection_covers_range(
        &self,
        session_id: Uuid,
        seq_start: i64,
        seq_end: i64,
    ) -> Result<bool, AppError> {
        if seq_start > seq_end {
            return Ok(true);
        }
        let mut ranges = self
            .list_session_loop_jobs()
            .await?
            .into_iter()
            .filter(|job| job.session_id == session_id)
            .filter_map(|job| {
                let range_start = job.pending_event_seq_start?;
                let range_end = match job.status {
                    SessionLoopJobStatus::Queued | SessionLoopJobStatus::Running => {
                        max_optional_i64(job.processed_event_seq, job.pending_event_seq_end)
                    }
                    SessionLoopJobStatus::Completed | SessionLoopJobStatus::Failed => {
                        job.processed_event_seq
                    }
                }?;
                (range_end >= range_start).then_some((range_start, range_end))
            })
            .collect::<Vec<_>>();
        ranges.sort_unstable();
        let mut next_seq = seq_start;
        for (range_start, range_end) in ranges {
            if range_end < next_seq {
                continue;
            }
            if range_start > next_seq {
                break;
            }
            if range_end >= seq_end {
                return Ok(true);
            }
            next_seq = range_end.saturating_add(1);
        }
        Ok(false)
    }
}

fn max_optional_i64(left: Option<i64>, right: Option<i64>) -> Option<i64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}
