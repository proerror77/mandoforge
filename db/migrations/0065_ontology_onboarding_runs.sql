CREATE TABLE IF NOT EXISTS ontology_onboarding_runs (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    industry TEXT NOT NULL,
    source_mode TEXT NOT NULL,
    domain_scope TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending_review',
    dataset_count INTEGER NOT NULL DEFAULT 0,
    profile_count INTEGER NOT NULL DEFAULT 0,
    proposal_count INTEGER NOT NULL DEFAULT 0,
    approved_count INTEGER NOT NULL DEFAULT 0,
    materialized_count INTEGER NOT NULL DEFAULT 0,
    actor_subject TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_ontology_onboarding_runs_tenant_status
    ON ontology_onboarding_runs (tenant_id, status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_ontology_onboarding_runs_tenant_domain
    ON ontology_onboarding_runs (tenant_id, domain_scope, created_at DESC);

ALTER TABLE ontology_onboarding_runs ENABLE ROW LEVEL SECURITY;
ALTER TABLE ontology_onboarding_runs FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_ontology_onboarding_runs ON ontology_onboarding_runs;
CREATE POLICY tenant_isolation_ontology_onboarding_runs ON ontology_onboarding_runs
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());

-- Do not attach ontology_releases.source_run_id yet: the current runtime still
-- reconstructs onboarding runs from semantic proposal objects instead of
-- writing ontology_onboarding_runs rows. Add the FK only after the store layer
-- persists onboarding runs.
