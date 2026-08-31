ALTER TABLE execution_jobs
    ADD COLUMN IF NOT EXISTS claim_generation BIGINT NOT NULL DEFAULT 0;

ALTER TABLE execution_jobs
    ADD COLUMN IF NOT EXISTS finalization_details JSONB NOT NULL DEFAULT '{}'::jsonb;

UPDATE remote_computer_job_assignments AS assignment
SET metadata = (
        CASE
            WHEN jsonb_typeof(assignment.metadata) = 'object' THEN assignment.metadata
            ELSE jsonb_build_object('legacy_metadata', assignment.metadata)
        END
    ) || jsonb_build_object(
        'execution_attempt_count',
        CASE
            WHEN job.status = 'queued'
              OR (job.status = 'cancel_requested' AND job.worker_id IS NULL)
                THEN job.attempt_count + 1
            ELSE job.attempt_count
        END,
        'execution_claim_generation', job.claim_generation
    ),
    updated_at = NOW()
FROM execution_jobs AS job
WHERE assignment.tenant_id = job.tenant_id
  AND assignment.execution_job_id = job.id
  AND assignment.status = 'assigned';

WITH backfilled_completion_events AS (
    UPDATE session_events AS event
    SET payload = event.payload || jsonb_build_object(
            'attempt_count', job.attempt_count,
            'claim_generation', job.claim_generation
        )
    FROM execution_jobs AS job
    WHERE event.tenant_id = job.tenant_id
      AND event.session_id = job.session_id
      AND event.event_type = 'execution.completed'
      AND event.actor_type = 'worker'
      AND event.actor_id = job.id
      AND event.payload ->> 'status' = 'completed'
      AND event.payload ->> 'execution_job_id' = job.id::text
      AND event.payload ->> 'tool_call_id' = job.tool_call_id::text
      AND job.status = 'completed'
      AND (
          NOT event.payload ? 'attempt_count'
          OR event.payload ->> 'attempt_count' = job.attempt_count::text
      )
      AND (
          NOT event.payload ? 'claim_generation'
          OR event.payload ->> 'claim_generation' = job.claim_generation::text
      )
      AND (
          NOT event.payload ? 'attempt_count'
          OR NOT event.payload ? 'claim_generation'
      )
    RETURNING event.id, event.tenant_id, event.session_id, event.seq, event.payload
), backfilled_failure_events AS (
    UPDATE session_events AS event
    SET payload = event.payload || jsonb_build_object(
            'status', 'failed',
            'attempt_count', job.attempt_count,
            'claim_generation', job.claim_generation
        )
    FROM execution_jobs AS job
    WHERE event.tenant_id = job.tenant_id
      AND event.session_id = job.session_id
      AND event.event_type = 'execution.failed'
      AND event.actor_type = 'worker'
      AND event.actor_id = job.id
      AND event.payload ->> 'execution_job_id' = job.id::text
      AND event.payload ->> 'tool_call_id' = job.tool_call_id::text
      AND job.status = 'failed'
      AND (
          NOT event.payload ? 'status'
          OR event.payload ->> 'status' = 'failed'
      )
      AND (
          NOT event.payload ? 'attempt_count'
          OR event.payload ->> 'attempt_count' = job.attempt_count::text
      )
      AND (
          NOT event.payload ? 'claim_generation'
          OR event.payload ->> 'claim_generation' = job.claim_generation::text
      )
      AND (
          NOT event.payload ? 'status'
          OR NOT event.payload ? 'attempt_count'
          OR NOT event.payload ? 'claim_generation'
      )
    RETURNING event.id, event.tenant_id, event.session_id, event.seq, event.payload
), outcome_candidates AS (
    SELECT
        completion.id AS outcome_event_id,
        completion.tenant_id,
        completion.session_id,
        session.environment_id,
        tool_result.seq AS tool_result_seq,
        completion.seq AS outcome_seq,
        'approved execution completed' AS reason
    FROM backfilled_completion_events AS completion
    JOIN sessions AS session
      ON session.tenant_id = completion.tenant_id
     AND session.id = completion.session_id
     AND session.status NOT IN ('terminated', 'failed')
    JOIN LATERAL (
        SELECT MAX(seq) AS seq
        FROM session_events
        WHERE tenant_id = completion.tenant_id
          AND session_id = completion.session_id
          AND event_type = 'tool.result'
          AND actor_id::text = completion.payload ->> 'tool_call_id'
          AND seq < completion.seq
    ) AS tool_result ON tool_result.seq IS NOT NULL
    UNION ALL
    SELECT
        failure.id AS outcome_event_id,
        failure.tenant_id,
        failure.session_id,
        session.environment_id,
        failure.seq AS tool_result_seq,
        failure.seq AS outcome_seq,
        'approved execution failed' AS reason
    FROM backfilled_failure_events AS failure
    JOIN sessions AS session
      ON session.tenant_id = failure.tenant_id
     AND session.id = failure.session_id
     AND session.status NOT IN ('terminated', 'failed')
), unresolved_boundaries AS (
    SELECT
        hidden_result.tenant_id,
        hidden_result.session_id,
        MIN(hidden_result.seq) AS seq
    FROM session_events AS hidden_result
    JOIN execution_jobs AS hidden_job
      ON hidden_job.tenant_id = hidden_result.tenant_id
     AND hidden_job.session_id = hidden_result.session_id
     AND hidden_job.tool_call_id = hidden_result.actor_id
    WHERE hidden_result.event_type = 'tool.result'
      AND (
          hidden_job.status <> 'completed'
          OR (
              NOT EXISTS (
                  SELECT 1
                  FROM session_events AS completion
                  WHERE completion.tenant_id = hidden_job.tenant_id
                    AND completion.session_id = hidden_job.session_id
                    AND completion.event_type = 'execution.completed'
                    AND completion.actor_type = 'worker'
                    AND completion.actor_id = hidden_job.id
                    AND completion.payload ->> 'status' = 'completed'
                    AND completion.payload ->> 'execution_job_id' = hidden_job.id::text
                    AND completion.payload ->> 'tool_call_id' = hidden_job.tool_call_id::text
                    AND completion.payload ->> 'attempt_count' = hidden_job.attempt_count::text
                    AND completion.payload ->> 'claim_generation' = hidden_job.claim_generation::text
                    AND completion.seq > hidden_result.seq
              )
              AND NOT EXISTS (
                  SELECT 1
                  FROM backfilled_completion_events AS completion
                  WHERE completion.tenant_id = hidden_job.tenant_id
                    AND completion.session_id = hidden_job.session_id
                    AND completion.payload ->> 'status' = 'completed'
                    AND completion.payload ->> 'execution_job_id' = hidden_job.id::text
                    AND completion.payload ->> 'tool_call_id' = hidden_job.tool_call_id::text
                    AND completion.payload ->> 'attempt_count' = hidden_job.attempt_count::text
                    AND completion.payload ->> 'claim_generation' = hidden_job.claim_generation::text
                    AND completion.seq > hidden_result.seq
              )
          )
      )
    GROUP BY hidden_result.tenant_id, hidden_result.session_id
), safe_outcome_events AS (
    SELECT
        candidate.*,
        COALESCE(projected.high_watermark, 0) AS high_watermark
    FROM outcome_candidates AS candidate
    LEFT JOIN unresolved_boundaries AS unresolved
      ON unresolved.tenant_id = candidate.tenant_id
     AND unresolved.session_id = candidate.session_id
    LEFT JOIN LATERAL (
        SELECT MAX(GREATEST(
            COALESCE(processed_event_seq, 0),
            CASE
                WHEN status IN ('queued', 'running') THEN COALESCE(pending_event_seq_end, 0)
                ELSE 0
            END
        )) AS high_watermark
        FROM session_loop_jobs
        WHERE tenant_id = candidate.tenant_id
          AND session_id = candidate.session_id
    ) AS projected ON TRUE
    WHERE unresolved.seq IS NULL
       OR candidate.outcome_seq < unresolved.seq
), outcome_recovery_ranges AS (
    SELECT DISTINCT ON (tenant_id, session_id)
        outcome_event_id,
        tenant_id,
        session_id,
        environment_id,
        MIN(tool_result_seq) OVER (
            PARTITION BY tenant_id, session_id
        ) AS tool_result_seq,
        outcome_seq AS pending_end,
        high_watermark,
        reason
    FROM safe_outcome_events
    ORDER BY tenant_id, session_id, outcome_seq DESC
)
INSERT INTO session_loop_jobs (
    id,
    tenant_id,
    session_id,
    environment_id,
    status,
    trigger_event_id,
    pending_event_seq_start,
    pending_event_seq_end,
    processed_event_seq,
    reason,
    enqueued_at,
    attempt_count,
    max_attempts
)
SELECT
    gen_random_uuid(),
    tenant_id,
    session_id,
    environment_id,
    'queued',
    outcome_event_id,
    GREATEST(tool_result_seq, high_watermark + 1),
    pending_end,
    GREATEST(tool_result_seq, high_watermark + 1) - 1,
    reason,
    NOW(),
    0,
    3
FROM outcome_recovery_ranges
WHERE high_watermark < pending_end
  AND GREATEST(tool_result_seq, high_watermark + 1) <= pending_end
ON CONFLICT (tenant_id, session_id)
WHERE status = 'queued'
DO UPDATE SET
    trigger_event_id = EXCLUDED.trigger_event_id,
    pending_event_seq_start = CASE
        WHEN session_loop_jobs.pending_event_seq_start IS NULL
            THEN EXCLUDED.pending_event_seq_start
        WHEN EXCLUDED.pending_event_seq_start IS NULL
            THEN session_loop_jobs.pending_event_seq_start
        ELSE LEAST(
            session_loop_jobs.pending_event_seq_start,
            EXCLUDED.pending_event_seq_start
        )
    END,
    pending_event_seq_end = NULLIF(GREATEST(
        COALESCE(session_loop_jobs.pending_event_seq_end, 0),
        COALESCE(EXCLUDED.pending_event_seq_end, 0)
    ), 0),
    reason = EXCLUDED.reason;

CREATE OR REPLACE FUNCTION project_session_event_to_loop_job()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    projection_reason TEXT;
    session_environment_id UUID;
    projected_high_watermark BIGINT;
    cursor_high_watermark BIGINT;
    pending_start BIGINT;
    execution_tool_result_seq BIGINT;
BEGIN
    projection_reason := CASE
        WHEN NEW.event_type IN (
            'user.message',
            'user.custom_tool_result',
            'session.goal.created',
            'session.goal.updated',
            'session.goal.completed',
            'session.goal.blocked'
        ) THEN NEW.event_type
        WHEN NEW.event_type = 'tool.result'
          AND NOT EXISTS (
              SELECT 1
              FROM execution_jobs
              WHERE tenant_id = NEW.tenant_id
                AND session_id = NEW.session_id
                AND tool_call_id = NEW.actor_id
          ) THEN NEW.event_type
        WHEN NEW.event_type = 'approval.rejected' THEN 'approval rejected'
        WHEN NEW.event_type = 'execution.completed'
          AND NEW.actor_type = 'worker'
          AND NEW.actor_id IS NOT NULL
          AND NEW.payload ->> 'status' = 'completed'
          AND NEW.payload ->> 'execution_job_id' = NEW.actor_id::text
          AND EXISTS (
              SELECT 1
              FROM execution_jobs
              WHERE tenant_id = NEW.tenant_id
                AND id = NEW.actor_id
                AND session_id = NEW.session_id
                AND tool_call_id::text = NEW.payload ->> 'tool_call_id'
                AND attempt_count::text = NEW.payload ->> 'attempt_count'
                AND claim_generation::text = NEW.payload ->> 'claim_generation'
                AND status = 'completed'
          ) THEN 'approved execution completed'
        WHEN NEW.event_type = 'execution.failed'
          AND NEW.actor_type = 'worker'
          AND NEW.actor_id IS NOT NULL
          AND NEW.payload ->> 'status' = 'failed'
          AND NEW.payload ->> 'execution_job_id' = NEW.actor_id::text
          AND EXISTS (
              SELECT 1
              FROM execution_jobs
              WHERE tenant_id = NEW.tenant_id
                AND id = NEW.actor_id
                AND session_id = NEW.session_id
                AND tool_call_id::text = NEW.payload ->> 'tool_call_id'
                AND attempt_count::text = NEW.payload ->> 'attempt_count'
                AND claim_generation::text = NEW.payload ->> 'claim_generation'
                AND status = 'failed'
          ) THEN 'approved execution failed'
        ELSE NULL
    END;
    IF projection_reason IS NULL THEN
        RETURN NEW;
    END IF;

    SELECT environment_id
    INTO session_environment_id
    FROM sessions
    WHERE tenant_id = NEW.tenant_id
      AND id = NEW.session_id
      AND status NOT IN ('terminated', 'failed');
    IF NOT FOUND THEN
        RETURN NEW;
    END IF;

    -- Do not advance a session cursor past a result whose execution has not
    -- reached its durable completion boundary. The matching completion event
    -- releases the whole deferred range in one enqueue.
    IF EXISTS (
        SELECT 1
        FROM session_events AS hidden_result
        JOIN execution_jobs AS hidden_job
          ON hidden_job.tenant_id = hidden_result.tenant_id
         AND hidden_job.session_id = hidden_result.session_id
         AND hidden_job.tool_call_id = hidden_result.actor_id
        WHERE hidden_result.tenant_id = NEW.tenant_id
          AND hidden_result.session_id = NEW.session_id
          AND hidden_result.event_type = 'tool.result'
          AND hidden_result.seq <= NEW.seq
          AND (
              hidden_job.status <> 'completed'
              OR NOT EXISTS (
                  SELECT 1
                  FROM session_events AS completion
                  WHERE completion.tenant_id = hidden_job.tenant_id
                    AND completion.session_id = hidden_job.session_id
                    AND completion.event_type = 'execution.completed'
                    AND completion.actor_type = 'worker'
                    AND completion.actor_id = hidden_job.id
                    AND completion.payload ->> 'status' = 'completed'
                    AND completion.payload ->> 'execution_job_id' = hidden_job.id::text
                    AND completion.payload ->> 'tool_call_id' = hidden_job.tool_call_id::text
                    AND completion.payload ->> 'attempt_count' = hidden_job.attempt_count::text
                    AND completion.payload ->> 'claim_generation' = hidden_job.claim_generation::text
              )
          )
    ) THEN
        RETURN NEW;
    END IF;

    SELECT MAX(GREATEST(
        COALESCE(processed_event_seq, 0),
        CASE
            WHEN status IN ('queued', 'running') THEN COALESCE(pending_event_seq_end, 0)
            ELSE 0
        END
    ))
    INTO projected_high_watermark
    FROM session_loop_jobs
    WHERE tenant_id = NEW.tenant_id
      AND session_id = NEW.session_id;

    cursor_high_watermark := projected_high_watermark;

    IF cursor_high_watermark IS NULL OR cursor_high_watermark = 0 THEN
        SELECT MAX(seq)
        INTO cursor_high_watermark
        FROM session_events
        WHERE tenant_id = NEW.tenant_id
          AND session_id = NEW.session_id
          AND seq < NEW.seq;
    END IF;

    pending_start := COALESCE(cursor_high_watermark, 0) + 1;
    IF pending_start > NEW.seq THEN
        pending_start := NULL;
    END IF;

    IF NEW.event_type = 'execution.completed' THEN
        SELECT MIN(result.seq)
        INTO execution_tool_result_seq
        FROM session_events AS result
        JOIN execution_jobs AS completed_job
          ON completed_job.tenant_id = result.tenant_id
         AND completed_job.session_id = result.session_id
         AND completed_job.tool_call_id = result.actor_id
        WHERE result.tenant_id = NEW.tenant_id
          AND result.session_id = NEW.session_id
          AND result.event_type = 'tool.result'
          AND result.seq > COALESCE(projected_high_watermark, 0)
          AND result.seq < NEW.seq
          AND completed_job.status = 'completed'
          AND (
              completed_job.id = NEW.actor_id
              OR EXISTS (
                  SELECT 1
                  FROM session_events AS completion
                  WHERE completion.tenant_id = completed_job.tenant_id
                    AND completion.session_id = completed_job.session_id
                    AND completion.event_type = 'execution.completed'
                    AND completion.actor_type = 'worker'
                    AND completion.actor_id = completed_job.id
                    AND completion.payload ->> 'status' = 'completed'
                    AND completion.payload ->> 'execution_job_id' = completed_job.id::text
                    AND completion.payload ->> 'tool_call_id' = completed_job.tool_call_id::text
                    AND completion.payload ->> 'attempt_count' = completed_job.attempt_count::text
                    AND completion.payload ->> 'claim_generation' = completed_job.claim_generation::text
                    AND completion.seq > result.seq
                    AND completion.seq < NEW.seq
              )
          );

        IF execution_tool_result_seq IS NOT NULL
          AND (pending_start IS NULL OR execution_tool_result_seq < pending_start) THEN
            pending_start := execution_tool_result_seq;
            cursor_high_watermark := execution_tool_result_seq - 1;
        END IF;
    END IF;

    INSERT INTO session_loop_jobs (
        id,
        tenant_id,
        session_id,
        environment_id,
        status,
        trigger_event_id,
        pending_event_seq_start,
        pending_event_seq_end,
        processed_event_seq,
        reason,
        enqueued_at,
        attempt_count,
        max_attempts
    )
    VALUES (
        gen_random_uuid(),
        NEW.tenant_id,
        NEW.session_id,
        session_environment_id,
        'queued',
        NEW.id,
        pending_start,
        NEW.seq,
        cursor_high_watermark,
        projection_reason,
        NOW(),
        0,
        3
    )
    ON CONFLICT (tenant_id, session_id)
    WHERE status = 'queued'
    DO UPDATE SET
        trigger_event_id = EXCLUDED.trigger_event_id,
        pending_event_seq_start = COALESCE(
            session_loop_jobs.pending_event_seq_start,
            EXCLUDED.pending_event_seq_start
        ),
        pending_event_seq_end = NULLIF(GREATEST(
            COALESCE(session_loop_jobs.pending_event_seq_end, 0),
            COALESCE(EXCLUDED.pending_event_seq_end, 0)
        ), 0),
        reason = EXCLUDED.reason;

    RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION record_terminal_execution_event()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    outcome_event_seq BIGINT;
    outcome_event_type TEXT;
    outcome_reason TEXT;
BEGIN
    IF OLD.status IS NOT DISTINCT FROM NEW.status
      OR NEW.status NOT IN ('completed', 'failed') THEN
        RETURN NEW;
    END IF;

    outcome_event_type := CASE NEW.status
        WHEN 'completed' THEN 'execution.completed'
        ELSE 'execution.failed'
    END;
    outcome_reason := CASE NEW.status
        WHEN 'completed' THEN 'approved execution completed'
        ELSE 'approved execution failed'
    END;

    PERFORM pg_advisory_xact_lock(
        hashtextextended(NEW.tenant_id::text || ':' || NEW.session_id::text, 0)
    );

    SELECT COALESCE(MAX(seq), 0) + 1
    INTO outcome_event_seq
    FROM session_events
    WHERE tenant_id = NEW.tenant_id
      AND session_id = NEW.session_id;

    INSERT INTO session_events (
        id,
        tenant_id,
        session_id,
        seq,
        actor_type,
        actor_id,
        event_type,
        payload,
        created_at
    )
    VALUES (
        gen_random_uuid(),
        NEW.tenant_id,
        NEW.session_id,
        outcome_event_seq,
        'worker',
        NEW.id,
        outcome_event_type,
        jsonb_build_object(
            'execution_job_id', NEW.id,
            'approval_id', NEW.approval_id,
            'tool_call_id', NEW.tool_call_id,
            'tool', NEW.tool_name,
            'status', NEW.status,
            'worker_id', NEW.worker_id,
            'attempt_count', NEW.attempt_count,
            'claim_generation', NEW.claim_generation,
            'max_attempts', NEW.max_attempts,
            'last_error', NEW.last_error,
            'reason', outcome_reason
        ),
        NOW()
    );

    PERFORM pg_notify(
        'mf_session_events',
        NEW.tenant_id::text || ':' || NEW.session_id::text || ':' || outcome_event_seq::text
    );

    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_record_completed_execution_event ON execution_jobs;
DROP TRIGGER IF EXISTS trg_record_terminal_execution_event ON execution_jobs;
DROP FUNCTION IF EXISTS record_completed_execution_event();
CREATE TRIGGER trg_record_terminal_execution_event
AFTER UPDATE OF status ON execution_jobs
FOR EACH ROW
EXECUTE FUNCTION record_terminal_execution_event();
