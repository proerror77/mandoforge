ALTER TABLE agent_handoff_events
    ADD COLUMN IF NOT EXISTS manager_plan_id UUID REFERENCES manager_agent_plans(id),
    ADD COLUMN IF NOT EXISTS semantic_scopes JSONB NOT NULL DEFAULT '{}',
    ADD COLUMN IF NOT EXISTS runtime_profile_id UUID REFERENCES agent_runtime_profiles(id),
    ADD COLUMN IF NOT EXISTS remote_computer_required BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN IF NOT EXISTS review_status TEXT NOT NULL DEFAULT 'pending_review',
    ADD COLUMN IF NOT EXISTS human_escalation_status TEXT NOT NULL DEFAULT 'none';

CREATE INDEX IF NOT EXISTS idx_agent_handoff_events_tenant_manager_plan
    ON agent_handoff_events (tenant_id, manager_plan_id)
    WHERE manager_plan_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_agent_handoff_events_tenant_runtime_profile
    ON agent_handoff_events (tenant_id, runtime_profile_id)
    WHERE runtime_profile_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_agent_handoff_events_tenant_review_status
    ON agent_handoff_events (tenant_id, review_status, human_escalation_status);
