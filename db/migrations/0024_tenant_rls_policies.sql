CREATE OR REPLACE FUNCTION mandoforge_current_tenant_id()
RETURNS UUID
LANGUAGE SQL
STABLE
AS $$
    SELECT NULLIF(current_setting('mandoforge.tenant_id', true), '')::uuid
$$;

DO $$
DECLARE
    table_name TEXT;
BEGIN
    FOREACH table_name IN ARRAY ARRAY[
        'tenants',
        'agents',
        'providers',
        'sessions',
        'session_events',
        'tool_definitions',
        'workspaces',
        'artifacts',
        'tool_calls',
        'approvals',
        'audit_logs',
        'execution_jobs',
        'organizations',
        'teams',
        'projects',
        'memberships',
        'provider_access',
        'mcp_servers',
        'eval_datasets',
        'eval_cases',
        'eval_runs',
        'usage_rollups',
        'agent_releases',
        'secret_records',
        'policy_revisions',
        'approval_groups',
        'approval_escalation_rules',
        'cost_alert_routes',
        'codex_app_server_runs',
        'tenant_invitations',
        'remote_computers',
        'remote_computer_leases',
        'remote_computer_session_attachments',
        'remote_computer_job_assignments',
        'approval_notification_channel_policies'
    ] LOOP
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', table_name);
        EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', table_name);
    END LOOP;
END $$;

DROP POLICY IF EXISTS tenant_isolation_tenants ON tenants;
CREATE POLICY tenant_isolation_tenants ON tenants
    USING (id = mandoforge_current_tenant_id())
    WITH CHECK (id = mandoforge_current_tenant_id());

DROP POLICY IF EXISTS tenant_isolation_agent_versions ON agent_versions;
ALTER TABLE agent_versions ENABLE ROW LEVEL SECURITY;
ALTER TABLE agent_versions FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation_agent_versions ON agent_versions
    USING (
        EXISTS (
            SELECT 1
            FROM agents
            WHERE agents.id = agent_versions.agent_id
              AND agents.tenant_id = mandoforge_current_tenant_id()
        )
    )
    WITH CHECK (
        EXISTS (
            SELECT 1
            FROM agents
            WHERE agents.id = agent_versions.agent_id
              AND agents.tenant_id = mandoforge_current_tenant_id()
        )
    );

DO $$
DECLARE
    table_name TEXT;
BEGIN
    FOREACH table_name IN ARRAY ARRAY[
        'agents',
        'providers',
        'sessions',
        'session_events',
        'tool_definitions',
        'workspaces',
        'artifacts',
        'tool_calls',
        'approvals',
        'audit_logs',
        'execution_jobs',
        'organizations',
        'teams',
        'projects',
        'memberships',
        'provider_access',
        'mcp_servers',
        'eval_datasets',
        'eval_cases',
        'eval_runs',
        'usage_rollups',
        'agent_releases',
        'secret_records',
        'policy_revisions',
        'approval_groups',
        'approval_escalation_rules',
        'cost_alert_routes',
        'codex_app_server_runs',
        'tenant_invitations',
        'remote_computers',
        'remote_computer_leases',
        'remote_computer_session_attachments',
        'remote_computer_job_assignments',
        'approval_notification_channel_policies'
    ] LOOP
        EXECUTE format('DROP POLICY IF EXISTS tenant_isolation_%I ON %I', table_name, table_name);
        EXECUTE format(
            'CREATE POLICY tenant_isolation_%I ON %I USING (tenant_id = mandoforge_current_tenant_id()) WITH CHECK (tenant_id = mandoforge_current_tenant_id())',
            table_name,
            table_name
        );
    END LOOP;
END $$;
