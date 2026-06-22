-- Adds mcp_server_ids, skill_ids, workflow_pack_ids, semantic_scopes to agent_versions
-- so that AgentVersion is a self-contained immutable snapshot of all agent capabilities.
ALTER TABLE agent_versions
    ADD COLUMN IF NOT EXISTS mcp_server_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
    ADD COLUMN IF NOT EXISTS skill_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
    ADD COLUMN IF NOT EXISTS workflow_pack_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
    ADD COLUMN IF NOT EXISTS semantic_scopes JSONB NOT NULL DEFAULT '{}'::jsonb;
