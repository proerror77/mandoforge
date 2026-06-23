use super::*;

#[tokio::test]
async fn migration_paths_include_stage2_migrations_in_order() {
    let paths = migration_paths().await.expect("migration paths");
    let names: Vec<_> = paths
        .iter()
        .filter_map(|path| path.file_name().and_then(|name| name.to_str()))
        .collect();

    assert!(names.contains(&"0001_core.sql"));
    assert!(names.contains(&"0003_stage2_governance.sql"));
    assert!(names.contains(&"0004_usage_rollups.sql"));
    assert!(names.contains(&"0005_approval_expiry.sql"));
    assert!(names.contains(&"0006_agent_releases.sql"));
    assert!(names.contains(&"0007_secret_records.sql"));
    assert!(names.contains(&"0008_policy_revisions.sql"));
    assert!(names.contains(&"0009_policy_revision_gates.sql"));
    assert!(names.contains(&"0010_approval_groups.sql"));
    assert!(names.contains(&"0011_cost_alert_routes.sql"));
    assert!(names.contains(&"0012_codex_app_server_runs.sql"));
    assert!(names.contains(&"0013_execution_job_retries.sql"));
    assert!(names.contains(&"0014_tenant_lifecycle_archive.sql"));
    assert!(names.contains(&"0015_tenant_invitations.sql"));
    assert!(names.contains(&"0016_organization_owner.sql"));
    assert!(names.contains(&"0017_agent_release_workflows.sql"));
    assert!(names.contains(&"0018_agent_release_automation.sql"));
    assert!(names.contains(&"0019_remote_computers.sql"));
    assert!(names.contains(&"0020_remote_computer_attachments.sql"));
    assert!(names.contains(&"0021_remote_computer_job_assignments.sql"));
    assert!(names.contains(&"0022_approval_notification_channel_policies.sql"));
    assert!(names.contains(&"0023_approval_notification_retries.sql"));
    assert!(names.contains(&"0024_tenant_rls_policies.sql"));
    assert!(names.contains(&"0025_remote_computer_state_locks.sql"));
    assert!(names.contains(&"0026_remote_computer_sidecar_heartbeats.sql"));
    assert!(names.contains(&"0027_agent_handoff_events.sql"));
    assert!(names.contains(&"0028_workflow_pack_installations.sql"));
    assert!(names.contains(&"0029_workflow_pack_profile_assets.sql"));
    assert!(names.contains(&"0030_agent_runtime_profiles.sql"));
    assert!(names.contains(&"0031_managed_agent_registry_fields.sql"));
    assert!(names.contains(&"0032_manager_agent_plans.sql"));
    assert!(names.contains(&"0033_agent_handoff_assignment_fields.sql"));
    assert!(names.contains(&"0034_agent_handoff_assignments.sql"));
    assert!(names.contains(&"0035_semantic_kernel.sql"));
    assert!(names.contains(&"0036_context_packets.sql"));
    assert!(names.contains(&"0037_memory_writeback_candidates.sql"));
    assert!(names.contains(&"0038_environments.sql"));
    assert!(names.contains(&"0039_session_loop_jobs.sql"));
    assert!(names.contains(&"0040_session_threads.sql"));
    assert!(names.contains(&"0041_session_loop_event_cursor.sql"));
    assert!(names.contains(&"0042_session_managed_statuses.sql"));
    assert!(names.contains(&"0050_managed_workflows.sql"));
    assert!(names.contains(&"0051_task_grants.sql"));
    assert!(names.contains(&"0052_approval_commit_tokens.sql"));
    assert!(names.contains(&"0053_workflow_transitions.sql"));
    assert!(names.contains(&"0054_workflow_pack_bindings.sql"));
    assert!(names.contains(&"0055_workflow_step_run_schedule.sql"));
    assert!(names.contains(&"0056_workflow_pack_runtime_objects.sql"));
    assert!(names.contains(&"0058_delegated_runtime_workflow_envelope.sql"));
    assert!(names.contains(&"0059_dynamic_workflow_plans.sql"));
    assert!(names.contains(&"0061_ontology_releases.sql"));
    assert!(names.contains(&"0064_agent_version_snapshot_fields.sql"));
    assert!(names.contains(&"0065_ontology_onboarding_runs.sql"));
    assert!(names.contains(&"0066_workflow_schedules.sql"));
    assert!(names.contains(&"0067_task_grants_constraints.sql"));
    assert!(names.contains(&"0068_remote_computer_active_lease_unique.sql"));
    assert!(names.contains(&"0069_ontology_release_workflow_triggers.sql"));
    assert!(names.contains(&"0070_ontology_release_workflow_trigger_skipped_status.sql"));
    assert!(names.contains(&"0071_ontology_release_current_status_unique.sql"));
    assert!(
        names.windows(2).all(|window| window[0] <= window[1]),
        "migrations should run lexicographically: {names:?}"
    );
}

#[test]
fn tenant_rls_migration_covers_tracked_tables() {
    let migration = [
        include_str!("../../../../db/migrations/0024_tenant_rls_policies.sql"),
        include_str!("../../../../db/migrations/0025_remote_computer_state_locks.sql"),
        include_str!("../../../../db/migrations/0026_remote_computer_sidecar_heartbeats.sql"),
        include_str!("../../../../db/migrations/0027_agent_handoff_events.sql"),
        include_str!("../../../../db/migrations/0028_workflow_pack_installations.sql"),
        include_str!("../../../../db/migrations/0029_workflow_pack_profile_assets.sql"),
        include_str!("../../../../db/migrations/0030_agent_runtime_profiles.sql"),
        include_str!("../../../../db/migrations/0032_manager_agent_plans.sql"),
        include_str!("../../../../db/migrations/0034_agent_handoff_assignments.sql"),
        include_str!("../../../../db/migrations/0035_semantic_kernel.sql"),
        include_str!("../../../../db/migrations/0036_context_packets.sql"),
        include_str!("../../../../db/migrations/0037_memory_writeback_candidates.sql"),
        include_str!("../../../../db/migrations/0038_environments.sql"),
        include_str!("../../../../db/migrations/0039_session_loop_jobs.sql"),
        include_str!("../../../../db/migrations/0040_session_threads.sql"),
        include_str!("../../../../db/migrations/0050_managed_workflows.sql"),
        include_str!("../../../../db/migrations/0051_task_grants.sql"),
        include_str!("../../../../db/migrations/0052_approval_commit_tokens.sql"),
        include_str!("../../../../db/migrations/0053_workflow_transitions.sql"),
        include_str!("../../../../db/migrations/0054_workflow_pack_bindings.sql"),
        include_str!("../../../../db/migrations/0056_workflow_pack_runtime_objects.sql"),
        include_str!("../../../../db/migrations/0059_dynamic_workflow_plans.sql"),
        include_str!("../../../../db/migrations/0061_ontology_releases.sql"),
        include_str!("../../../../db/migrations/0065_ontology_onboarding_runs.sql"),
        include_str!("../../../../db/migrations/0066_workflow_schedules.sql"),
        include_str!("../../../../db/migrations/0069_ontology_release_workflow_triggers.sql"),
    ]
    .join("\n");
    assert!(migration.contains("mandoforge_current_tenant_id"));
    assert!(migration.contains("FORCE ROW LEVEL SECURITY"));
    for table in tenant_isolation_tracked_tables() {
        assert!(
            migration.contains(&format!("'{table}'"))
                || migration.contains(&format!("TABLE {table}"))
                || (table == "agent_versions"
                    && migration.contains("tenant_isolation_agent_versions")),
            "RLS migration should mention tracked tenant table {table}"
        );
    }
}
