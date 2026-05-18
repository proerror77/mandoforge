ALTER TABLE agents ADD COLUMN IF NOT EXISTS runtime_profile_id UUID REFERENCES agent_runtime_profiles(id);
ALTER TABLE agents ADD COLUMN IF NOT EXISTS agent_role TEXT NOT NULL DEFAULT 'specialist';
ALTER TABLE agents ADD COLUMN IF NOT EXISTS tool_policy JSONB NOT NULL DEFAULT '{}';
ALTER TABLE agents ADD COLUMN IF NOT EXISTS mcp_server_ids JSONB NOT NULL DEFAULT '[]';
ALTER TABLE agents ADD COLUMN IF NOT EXISTS skill_ids JSONB NOT NULL DEFAULT '[]';
ALTER TABLE agents ADD COLUMN IF NOT EXISTS workflow_pack_ids JSONB NOT NULL DEFAULT '[]';
ALTER TABLE agents ADD COLUMN IF NOT EXISTS remote_computer_profile JSONB NOT NULL DEFAULT '{}';
ALTER TABLE agents ADD COLUMN IF NOT EXISTS semantic_scopes JSONB NOT NULL DEFAULT '{}';
ALTER TABLE agents ADD COLUMN IF NOT EXISTS release_state TEXT NOT NULL DEFAULT 'draft';

CREATE INDEX IF NOT EXISTS idx_agents_tenant_runtime_profile
    ON agents (tenant_id, runtime_profile_id)
    WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_agents_tenant_role_release
    ON agents (tenant_id, agent_role, release_state)
    WHERE archived_at IS NULL;
