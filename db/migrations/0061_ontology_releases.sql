CREATE TABLE IF NOT EXISTS ontology_releases (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    version TEXT NOT NULL,
    domain_scope TEXT NOT NULL,
    source_run_id UUID,
    parent_release_id UUID REFERENCES ontology_releases(id) ON DELETE SET NULL,
    rollback_target_release_id UUID REFERENCES ontology_releases(id) ON DELETE SET NULL,
    status TEXT NOT NULL,
    release_class TEXT NOT NULL,
    object_count INTEGER NOT NULL DEFAULT 0,
    relation_count INTEGER NOT NULL DEFAULT 0,
    action_count INTEGER NOT NULL DEFAULT 0,
    migration_policy JSONB NOT NULL DEFAULT '{}'::jsonb,
    gate_result JSONB NOT NULL DEFAULT '{}'::jsonb,
    materialized_object_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
    materialized_link_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
    evidence_refs JSONB NOT NULL DEFAULT '[]'::jsonb,
    promoted_by TEXT,
    promoted_at TIMESTAMPTZ,
    rolled_back_by TEXT,
    rolled_back_at TIMESTAMPTZ,
    archived_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (tenant_id, domain_scope, version)
);

CREATE INDEX IF NOT EXISTS idx_ontology_releases_tenant_domain_status
    ON ontology_releases (tenant_id, domain_scope, status);

CREATE UNIQUE INDEX IF NOT EXISTS idx_ontology_releases_one_active_per_domain
    ON ontology_releases (tenant_id, lower(domain_scope))
    WHERE status = 'active';

CREATE INDEX IF NOT EXISTS idx_ontology_releases_source_run
    ON ontology_releases (tenant_id, source_run_id);

ALTER TABLE ontology_releases ENABLE ROW LEVEL SECURITY;
ALTER TABLE ontology_releases FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_ontology_releases ON ontology_releases;
CREATE POLICY tenant_isolation_ontology_releases ON ontology_releases
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());
