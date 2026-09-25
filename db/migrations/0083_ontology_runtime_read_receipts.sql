-- Server-issued evidence of an authorized Ontology read. No public write API.
CREATE TABLE IF NOT EXISTS ontology_runtime_read_receipts (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE RESTRICT,
    session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE RESTRICT,
    task_grant_id UUID NOT NULL REFERENCES task_grants(id) ON DELETE RESTRICT,
    application_id UUID NOT NULL REFERENCES ontology_sdk_applications(id) ON DELETE RESTRICT,
    source_tool_call_id UUID NOT NULL REFERENCES tool_calls(id) ON DELETE RESTRICT,
    kind TEXT NOT NULL CHECK (kind IN ('context', 'action_result')),
    expires_at TIMESTAMPTZ NOT NULL,
    payload JSONB NOT NULL CHECK (jsonb_typeof(payload) = 'object'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_ontology_runtime_receipts_session
    ON ontology_runtime_read_receipts (tenant_id, session_id, created_at);
CREATE OR REPLACE FUNCTION mandoforge_check_ontology_runtime_receipt_tenant()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM sessions WHERE id = NEW.session_id AND tenant_id = NEW.tenant_id)
       OR NOT EXISTS (SELECT 1 FROM task_grants WHERE id = NEW.task_grant_id AND tenant_id = NEW.tenant_id)
       OR NOT EXISTS (SELECT 1 FROM ontology_sdk_applications WHERE id = NEW.application_id AND tenant_id = NEW.tenant_id)
       OR NOT EXISTS (SELECT 1 FROM tool_calls WHERE id = NEW.source_tool_call_id AND tenant_id = NEW.tenant_id AND session_id = NEW.session_id AND task_grant_id = NEW.task_grant_id)
    THEN RAISE EXCEPTION 'ontology read receipt authority mismatch'; END IF;
    RETURN NEW;
END;
$$;
DROP TRIGGER IF EXISTS trg_ontology_runtime_receipt_tenant ON ontology_runtime_read_receipts;
CREATE TRIGGER trg_ontology_runtime_receipt_tenant BEFORE INSERT ON ontology_runtime_read_receipts
    FOR EACH ROW EXECUTE FUNCTION mandoforge_check_ontology_runtime_receipt_tenant();
CREATE OR REPLACE FUNCTION mandoforge_immutable_ontology_runtime_receipt()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'ontology read receipts are immutable'; END;
$$;
DROP TRIGGER IF EXISTS trg_immutable_ontology_runtime_receipt ON ontology_runtime_read_receipts;
CREATE TRIGGER trg_immutable_ontology_runtime_receipt BEFORE UPDATE OR DELETE ON ontology_runtime_read_receipts
    FOR EACH ROW EXECUTE FUNCTION mandoforge_immutable_ontology_runtime_receipt();
ALTER TABLE ontology_runtime_read_receipts ENABLE ROW LEVEL SECURITY;
ALTER TABLE ontology_runtime_read_receipts FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation_ontology_runtime_receipts ON ontology_runtime_read_receipts;
CREATE POLICY tenant_isolation_ontology_runtime_receipts ON ontology_runtime_read_receipts
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());
