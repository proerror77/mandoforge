DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM task_grants
        WHERE parent_grant_id IS NULL
        GROUP BY tenant_id, workflow_run_id
        HAVING COUNT(*) > 1
    ) THEN
        RAISE EXCEPTION 'cannot enforce unique root task grants: duplicate workflow roots exist';
    END IF;
END $$;

CREATE UNIQUE INDEX IF NOT EXISTS ux_task_grants_tenant_workflow_root
    ON task_grants (tenant_id, workflow_run_id)
    WHERE parent_grant_id IS NULL;
