-- Prevent double-provisioning race: two concurrent workers for the same session
-- cannot both insert an on-demand Pod record with the same pod_name in the same tenant.
CREATE UNIQUE INDEX IF NOT EXISTS idx_remote_computers_tenant_pod_name
    ON remote_computers (tenant_id, pod_name)
    WHERE pod_name IS NOT NULL;
