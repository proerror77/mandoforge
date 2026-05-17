CREATE TABLE IF NOT EXISTS workflow_pack_profile_assets (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    installation_id UUID NOT NULL REFERENCES workflow_pack_installations(id),
    profile_id TEXT NOT NULL,
    content TEXT NOT NULL,
    version INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    archived_at TIMESTAMPTZ
);

-- tracked tenant table: 'workflow_pack_profile_assets'

CREATE INDEX IF NOT EXISTS idx_workflow_pack_profile_assets_tenant_installation
    ON workflow_pack_profile_assets (tenant_id, installation_id, profile_id, version DESC);

CREATE UNIQUE INDEX IF NOT EXISTS idx_workflow_pack_profile_assets_active_profile
    ON workflow_pack_profile_assets (tenant_id, installation_id, profile_id)
    WHERE archived_at IS NULL;

ALTER TABLE workflow_pack_profile_assets ENABLE ROW LEVEL SECURITY;
ALTER TABLE workflow_pack_profile_assets FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_workflow_pack_profile_assets ON workflow_pack_profile_assets;
CREATE POLICY tenant_isolation_workflow_pack_profile_assets ON workflow_pack_profile_assets
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());
