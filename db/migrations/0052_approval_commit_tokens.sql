ALTER TABLE tool_calls
    ADD COLUMN IF NOT EXISTS task_grant_id UUID REFERENCES task_grants(id),
    ADD COLUMN IF NOT EXISTS normalized_args_hash TEXT,
    ADD COLUMN IF NOT EXISTS target_binding JSONB NOT NULL DEFAULT '{}';

CREATE TABLE IF NOT EXISTS approval_commit_tokens (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    approval_id UUID NOT NULL UNIQUE REFERENCES approvals(id),
    tool_call_id UUID NOT NULL REFERENCES tool_calls(id),
    task_grant_id UUID NOT NULL REFERENCES task_grants(id),
    session_id UUID NOT NULL REFERENCES sessions(id),
    tool_name TEXT NOT NULL,
    normalized_args_hash TEXT NOT NULL,
    target_binding JSONB NOT NULL DEFAULT '{}',
    approver_subject TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'issued',
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- tracked tenant table: 'approval_commit_tokens'

CREATE INDEX IF NOT EXISTS idx_approval_commit_tokens_tenant_tool_call
    ON approval_commit_tokens (tenant_id, tool_call_id);

CREATE INDEX IF NOT EXISTS idx_approval_commit_tokens_tenant_status
    ON approval_commit_tokens (tenant_id, status, expires_at);

ALTER TABLE approval_commit_tokens ENABLE ROW LEVEL SECURITY;
ALTER TABLE approval_commit_tokens FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_approval_commit_tokens ON approval_commit_tokens;
CREATE POLICY tenant_isolation_approval_commit_tokens ON approval_commit_tokens
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());
