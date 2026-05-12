ALTER TABLE agent_releases
    ADD COLUMN IF NOT EXISTS automation_policy JSONB NOT NULL DEFAULT '{}';
