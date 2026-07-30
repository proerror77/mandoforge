CREATE OR REPLACE FUNCTION project_session_event_to_loop_job()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    projection_reason TEXT;
    session_environment_id UUID;
    cursor_high_watermark BIGINT;
    pending_start BIGINT;
BEGIN
    projection_reason := CASE
        WHEN NEW.event_type IN (
            'user.message',
            'user.custom_tool_result',
            'tool.result',
            'session.goal.created',
            'session.goal.updated',
            'session.goal.completed',
            'session.goal.blocked'
        ) THEN NEW.event_type
        WHEN NEW.event_type = 'approval.approved' THEN 'approval approved'
        WHEN NEW.event_type = 'approval.rejected' THEN 'approval rejected'
        WHEN NEW.event_type = 'execution.completed' THEN 'approved execution completed'
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

    SELECT MAX(GREATEST(
        COALESCE(processed_event_seq, 0),
        CASE
            WHEN status IN ('queued', 'running') THEN COALESCE(pending_event_seq_end, 0)
            ELSE 0
        END
    ))
    INTO cursor_high_watermark
    FROM session_loop_jobs
    WHERE tenant_id = NEW.tenant_id
      AND session_id = NEW.session_id;

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

DROP TRIGGER IF EXISTS trg_project_session_event_to_loop_job ON session_events;
CREATE TRIGGER trg_project_session_event_to_loop_job
AFTER INSERT ON session_events
FOR EACH ROW
EXECUTE FUNCTION project_session_event_to_loop_job();
