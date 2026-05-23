CREATE TABLE IF NOT EXISTS workflow_pack_runtime_objects (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    installation_id UUID NOT NULL REFERENCES workflow_pack_installations(id),
    binding_id UUID NOT NULL REFERENCES workflow_pack_bindings(id),
    pack_id TEXT NOT NULL,
    pack_version TEXT NOT NULL,
    object_type TEXT NOT NULL,
    object_key TEXT NOT NULL,
    runtime_kind TEXT NOT NULL,
    status TEXT NOT NULL,
    spec JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- tracked tenant table: 'workflow_pack_runtime_objects'

CREATE INDEX IF NOT EXISTS idx_workflow_pack_runtime_objects_tenant_installation
    ON workflow_pack_runtime_objects (tenant_id, installation_id, object_type, object_key);

CREATE INDEX IF NOT EXISTS idx_workflow_pack_runtime_objects_tenant_status
    ON workflow_pack_runtime_objects (tenant_id, status, updated_at DESC);

ALTER TABLE workflow_pack_runtime_objects ENABLE ROW LEVEL SECURITY;
ALTER TABLE workflow_pack_runtime_objects FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_workflow_pack_runtime_objects ON workflow_pack_runtime_objects;
CREATE POLICY tenant_isolation_workflow_pack_runtime_objects ON workflow_pack_runtime_objects
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());
