CREATE TABLE IF NOT EXISTS workflow_pack_installations (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    pack_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    version TEXT NOT NULL,
    manifest_path TEXT NOT NULL,
    manifest JSONB NOT NULL,
    validation_report JSONB NOT NULL,
    status TEXT NOT NULL,
    eval_gate_status TEXT NOT NULL DEFAULT 'pending',
    release_gate_status TEXT NOT NULL DEFAULT 'pending',
    gate_evidence JSONB NOT NULL DEFAULT '{}',
    staged_at TIMESTAMPTZ,
    released_at TIMESTAMPTZ,
    archived_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- tracked tenant table: 'workflow_pack_installations'

CREATE INDEX IF NOT EXISTS idx_workflow_pack_installations_tenant_pack
    ON workflow_pack_installations (tenant_id, pack_id, version);

CREATE INDEX IF NOT EXISTS idx_workflow_pack_installations_tenant_status
    ON workflow_pack_installations (tenant_id, status, created_at DESC);

ALTER TABLE workflow_pack_installations ENABLE ROW LEVEL SECURITY;
ALTER TABLE workflow_pack_installations FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_workflow_pack_installations ON workflow_pack_installations;
CREATE POLICY tenant_isolation_workflow_pack_installations ON workflow_pack_installations
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());
