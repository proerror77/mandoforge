ALTER TABLE workflow_step_runs
    ADD COLUMN IF NOT EXISTS claim_owner_version SMALLINT NOT NULL DEFAULT 0;

-- Legacy claim owners were untyped caller-controlled strings. Their external
-- side-effect outcome is unknown during a rolling upgrade, so fail closed and
-- require manual reconciliation instead of automatically replaying the step.
UPDATE task_grants AS task_grant
SET status = 'cancelled',
    updated_at = now()
WHERE task_grant.status = 'active'
  AND EXISTS (
      SELECT 1
      FROM workflow_step_runs AS step
      WHERE step.tenant_id = task_grant.tenant_id
        AND step.workflow_run_id = task_grant.workflow_run_id
        AND step.status = 'running'
        AND step.claimed_by_worker IS NOT NULL
        AND step.claim_owner_version = 0
  );

UPDATE workflow_runs AS run
SET status = 'requires_action',
    runtime_envelope = COALESCE(runtime_envelope, '{}'::jsonb) || jsonb_build_object(
        'claim_migration',
        jsonb_build_object(
            'status', 'outcome_unknown',
            'action', 'manual_reconciliation_required'
        )
    ),
    updated_at = now()
WHERE run.status IN ('queued', 'running', 'requires_action')
  AND EXISTS (
      SELECT 1
      FROM workflow_step_runs AS step
      WHERE step.tenant_id = run.tenant_id
        AND step.workflow_run_id = run.id
        AND step.status = 'running'
        AND step.claimed_by_worker IS NOT NULL
        AND step.claim_owner_version = 0
  );

UPDATE workflow_step_runs
SET status = CASE WHEN status = 'running' THEN 'failed' ELSE status END,
    output_payload = CASE
        WHEN status = 'running' THEN COALESCE(output_payload, '{}'::jsonb) || jsonb_build_object(
            'claim_migration',
            jsonb_build_object(
                'status', 'outcome_unknown',
                'action', 'manual_reconciliation_required'
            )
        )
        ELSE output_payload
    END,
    claimed_by_worker = NULL,
    claim_owner_version = 0,
    lease_expires_at = NULL,
    completed_at = CASE WHEN status = 'running' THEN COALESCE(completed_at, now()) ELSE completed_at END,
    updated_at = now()
WHERE claimed_by_worker IS NOT NULL
  AND claim_owner_version = 0;

ALTER TABLE workflow_step_runs
    DROP CONSTRAINT IF EXISTS workflow_step_runs_claim_owner_version_check;

ALTER TABLE workflow_step_runs
    ADD CONSTRAINT workflow_step_runs_claim_owner_version_check
    CHECK (
        (claimed_by_worker IS NULL AND claim_owner_version = 0)
        OR (claimed_by_worker IS NOT NULL AND claim_owner_version = 1)
    );
