CREATE TABLE IF NOT EXISTS ontology_release_workflow_triggers (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    ontology_release_id UUID NOT NULL REFERENCES ontology_releases(id) ON DELETE CASCADE,
    workflow_definition_id UUID NOT NULL REFERENCES workflow_definitions(id) ON DELETE CASCADE,
    workflow_run_id UUID REFERENCES workflow_runs(id) ON DELETE SET NULL,
    status TEXT NOT NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    claimed_at TIMESTAMPTZ,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (tenant_id, ontology_release_id, workflow_definition_id)
);

CREATE INDEX IF NOT EXISTS idx_ontology_release_workflow_triggers_tenant_status
    ON ontology_release_workflow_triggers (tenant_id, status, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_ontology_release_workflow_triggers_claims
    ON ontology_release_workflow_triggers (tenant_id, status, claimed_at);

CREATE INDEX IF NOT EXISTS idx_ontology_release_workflow_triggers_release
    ON ontology_release_workflow_triggers (tenant_id, ontology_release_id);

CREATE INDEX IF NOT EXISTS idx_workflow_runs_ontology_release_trigger
    ON workflow_runs (
        tenant_id,
        workflow_definition_id,
        ((input_payload->>'ontology_release_id'))
    )
    WHERE input_payload->>'trigger' = 'ontology_release.promoted';

ALTER TABLE ontology_release_workflow_triggers ENABLE ROW LEVEL SECURITY;
ALTER TABLE ontology_release_workflow_triggers FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_ontology_release_workflow_triggers
    ON ontology_release_workflow_triggers;
CREATE POLICY tenant_isolation_ontology_release_workflow_triggers
    ON ontology_release_workflow_triggers
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());
