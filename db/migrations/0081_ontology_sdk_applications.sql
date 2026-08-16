CREATE TABLE IF NOT EXISTS ontology_sdk_applications (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    subject TEXT NOT NULL CHECK (length(btrim(subject)) > 0),
    ontology_release_id UUID NOT NULL REFERENCES ontology_releases(id) ON DELETE RESTRICT,
    release_version TEXT NOT NULL CHECK (length(btrim(release_version)) > 0),
    domain_scope TEXT NOT NULL CHECK (length(btrim(domain_scope)) > 0),
    catalog_digest TEXT NOT NULL CHECK (length(btrim(catalog_digest)) > 0),
    subset_manifest JSONB NOT NULL CHECK (jsonb_typeof(subset_manifest) = 'object'),
    subset_digest TEXT NOT NULL CHECK (length(btrim(subset_digest)) > 0),
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'suspended', 'archived')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, subject, ontology_release_id, subset_digest)
);

CREATE INDEX IF NOT EXISTS idx_ontology_sdk_applications_tenant_subject
    ON ontology_sdk_applications (tenant_id, subject, created_at);

CREATE OR REPLACE FUNCTION mandoforge_check_ontology_sdk_application_tenant()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    release_tenant_id UUID;
BEGIN
    SELECT tenant_id
    INTO release_tenant_id
    FROM ontology_releases
    WHERE id = NEW.ontology_release_id;
    IF release_tenant_id IS NULL OR release_tenant_id <> NEW.tenant_id THEN
        RAISE EXCEPTION 'ontology SDK application release tenant mismatch';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_check_ontology_sdk_application_tenant
    ON ontology_sdk_applications;
CREATE TRIGGER trg_check_ontology_sdk_application_tenant
BEFORE INSERT OR UPDATE ON ontology_sdk_applications
FOR EACH ROW
EXECUTE FUNCTION mandoforge_check_ontology_sdk_application_tenant();

CREATE OR REPLACE FUNCTION mandoforge_prevent_ontology_sdk_application_mutation()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'ontology SDK applications are immutable';
END;
$$;

DROP TRIGGER IF EXISTS trg_prevent_ontology_sdk_application_mutation
    ON ontology_sdk_applications;
CREATE TRIGGER trg_prevent_ontology_sdk_application_mutation
BEFORE UPDATE OR DELETE ON ontology_sdk_applications
FOR EACH ROW
EXECUTE FUNCTION mandoforge_prevent_ontology_sdk_application_mutation();

ALTER TABLE ontology_sdk_applications ENABLE ROW LEVEL SECURITY;
ALTER TABLE ontology_sdk_applications FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_ontology_sdk_applications
    ON ontology_sdk_applications;
CREATE POLICY tenant_isolation_ontology_sdk_applications
    ON ontology_sdk_applications
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());
