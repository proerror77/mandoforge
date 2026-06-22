-- Add CHECK constraints to safety-critical free-text fields on task_grants.
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'task_grants_risk_level_check'
    ) THEN
        ALTER TABLE task_grants
            ADD CONSTRAINT task_grants_risk_level_check
            CHECK (risk_level IN ('low', 'medium', 'high', 'critical'));
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'task_grants_status_check'
    ) THEN
        ALTER TABLE task_grants
            ADD CONSTRAINT task_grants_status_check
            CHECK (status IN ('active', 'revoked', 'expired', 'completed', 'cancelled'));
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'task_grants_agent_class_check'
    ) THEN
        ALTER TABLE task_grants
            ADD CONSTRAINT task_grants_agent_class_check
            CHECK (agent_class IS NULL OR agent_class IN ('specialist', 'manager', 'reviewer', 'observer', 'executor'));
    END IF;
END $$;

-- Add missing performance indexes
CREATE INDEX IF NOT EXISTS idx_task_grants_tenant_grantee_session
    ON task_grants (tenant_id, grantee_session_id)
    WHERE grantee_session_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_task_grants_tenant_grantee_agent
    ON task_grants (tenant_id, grantee_agent_id)
    WHERE grantee_agent_id IS NOT NULL;

-- Add missing index on squad_members for uniqueness
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'unique_squad_member'
    ) THEN
        ALTER TABLE squad_members
            ADD CONSTRAINT unique_squad_member UNIQUE (squad_id, teammate_id);
    END IF;
END $$;
