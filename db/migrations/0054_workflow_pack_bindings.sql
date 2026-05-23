CREATE TABLE IF NOT EXISTS workflow_pack_bindings (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    installation_id UUID NOT NULL REFERENCES workflow_pack_installations(id),
    pack_id TEXT NOT NULL,
    pack_version TEXT NOT NULL,
    binding_type TEXT NOT NULL,
    binding_key TEXT NOT NULL,
    source_path TEXT,
    target_kind TEXT NOT NULL,
    target_id UUID,
    status TEXT NOT NULL,
    materialized_payload JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- tracked tenant table: 'workflow_pack_bindings'

CREATE INDEX IF NOT EXISTS idx_workflow_pack_bindings_tenant_installation
    ON workflow_pack_bindings (tenant_id, installation_id, binding_type, binding_key);

CREATE INDEX IF NOT EXISTS idx_workflow_pack_bindings_tenant_status
    ON workflow_pack_bindings (tenant_id, status, updated_at DESC);

ALTER TABLE workflow_pack_bindings ENABLE ROW LEVEL SECURITY;
ALTER TABLE workflow_pack_bindings FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_workflow_pack_bindings ON workflow_pack_bindings;
CREATE POLICY tenant_isolation_workflow_pack_bindings ON workflow_pack_bindings
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());
