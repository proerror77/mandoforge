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
    assert!(names.contains(&"0061_ontology_releases.sql"));
    assert!(names.contains(&"0064_agent_version_snapshot_fields.sql"));
    assert!(names.contains(&"0065_ontology_onboarding_runs.sql"));
    assert!(names.contains(&"0066_workflow_schedules.sql"));
    assert!(names.contains(&"0067_task_grants_constraints.sql"));
    assert!(names.contains(&"0068_remote_computer_active_lease_unique.sql"));
    assert!(names.contains(&"0069_ontology_release_workflow_triggers.sql"));
    assert!(names.contains(&"0070_ontology_release_workflow_trigger_skipped_status.sql"));
    assert!(names.contains(&"0071_ontology_release_current_status_unique.sql"));
    assert!(names.contains(&"0072_agent_version_runtime_snapshot.sql"));
    assert!(names.contains(&"0073_task_grant_budget_usage.sql"));
    assert!(names.contains(&"0074_task_grant_root_unique.sql"));
    assert!(names.contains(&"0075_agent_release_promoted_unique.sql"));
    assert!(names.contains(&"0076_drop_dynamic_workflow_plans.sql"));
    assert!(names.contains(&"0077_session_event_loop_projection.sql"));
    assert!(
        names.windows(2).all(|window| window[0] <= window[1]),
        "migrations should run lexicographically: {names:?}"
    );
}

#[test]
fn migration_checksum_is_content_addressed() {
    assert_eq!(
        db_bootstrap::migration_checksum("SELECT 1;"),
        db_bootstrap::migration_checksum("SELECT 1;")
    );
    assert_ne!(
        db_bootstrap::migration_checksum("SELECT 1;"),
        db_bootstrap::migration_checksum("SELECT 2;")
    );
}

#[tokio::test]
#[ignore = "requires MANDOFORGE_TEST_POSTGRES_URL"]
async fn postgres_migration_ledger_is_idempotent_and_rejects_checksum_drift() {
    let database_url = std::env::var("MANDOFORGE_TEST_POSTGRES_URL")
        .expect("MANDOFORGE_TEST_POSTGRES_URL is required");
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .expect("connect test postgres");
    let suffix = Uuid::new_v4().simple().to_string();
    let table_name = format!("mandoforge_migration_ledger_{suffix}");
    let filename = format!("9999_migration_ledger_{suffix}.sql");
    let directory = std::env::temp_dir().join(format!("mandoforge-migrations-{suffix}"));
    fs::create_dir_all(&directory).expect("create temporary migration directory");
    let path = directory.join(&filename);
    fs::write(
        &path,
        format!("CREATE TABLE {table_name} (id INTEGER PRIMARY KEY);"),
    )
    .expect("write test migration");

    run_migrations_from_paths(&pool, vec![path.clone()])
        .await
        .expect("apply migration once");
    run_migrations_from_paths(&pool, vec![path.clone()])
        .await
        .expect("reapply unchanged migration");

    fs::write(
        &path,
        format!("ALTER TABLE {table_name} ADD COLUMN changed BOOLEAN;"),
    )
    .expect("mutate test migration");
    let error = run_migrations_from_paths(&pool, vec![path])
        .await
        .expect_err("changed migration must fail closed");
    assert!(error.to_string().contains("migration checksum mismatch"));

    sqlx::query(&format!("DROP TABLE IF EXISTS {table_name}"))
        .execute(&pool)
        .await
        .expect("drop test table");
    sqlx::query("DELETE FROM schema_migrations WHERE filename = $1")
        .bind(filename)
        .execute(&pool)
        .await
        .expect("remove test ledger row");
    fs::remove_dir_all(directory).expect("remove temporary migration directory");
}

#[tokio::test]
#[ignore = "requires MANDOFORGE_TEST_POSTGRES_URL"]
async fn postgres_session_event_trigger_and_explicit_projection_converge() -> Result<()> {
    let database_url = std::env::var("MANDOFORGE_TEST_POSTGRES_URL")
        .expect("MANDOFORGE_TEST_POSTGRES_URL is required");
    let bootstrap_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await?;
    run_migrations(&bootstrap_pool).await?;
    drop(bootstrap_pool);

    let tenant_id = Uuid::new_v4();
    let agent_id = Uuid::new_v4();
    let session_id = Uuid::new_v4();
    let tenant_setting = format!("SET mandoforge.tenant_id = '{tenant_id}'");
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .after_connect(move |connection, _| {
            let tenant_setting = tenant_setting.clone();
            Box::pin(async move {
                connection.execute(tenant_setting.as_str()).await?;
                Ok(())
            })
        })
        .connect(&database_url)
        .await?;

    let outcome: Result<()> = async {
        sqlx::query("INSERT INTO tenants (id, name, slug) VALUES ($1, $2, $3)")
            .bind(tenant_id)
            .bind("Session event projection test")
            .bind(format!("session-event-projection-{}", tenant_id.simple()))
            .execute(&pool)
            .await?;
        sqlx::query(
            "INSERT INTO agents (id, tenant_id, name, kind, provider, model, system_prompt)
             VALUES ($1, $2, $3, 'orchestrator', 'test', 'test', '')",
        )
        .bind(agent_id)
        .bind(tenant_id)
        .bind("Session event projection test agent")
        .execute(&pool)
        .await?;
        sqlx::query(
            "INSERT INTO sessions (id, tenant_id, agent_id, title, status)
             VALUES ($1, $2, $3, $4, 'idle')",
        )
        .bind(session_id)
        .bind(tenant_id)
        .bind(agent_id)
        .bind("Session event projection test session")
        .execute(&pool)
        .await?;

        let mut state = test_state_with_worker(Arc::new(InlineExecutionWorker));
        state.store = StoreBackend::Postgres(pool.clone());
        state.execution_queue = ExecutionQueue::postgres(pool.clone(), tenant_id);
        state.tenant_id = tenant_id;

        state
            .append_event(
                "user",
                None,
                session_id,
                "user.message",
                json!({"message": "one"}),
            )
            .await
            .map_err(|error| anyhow::anyhow!(error.message))?;
        let second_event = state
            .append_event(
                "user",
                None,
                session_id,
                "user.message",
                json!({"message": "two"}),
            )
            .await
            .map_err(|error| anyhow::anyhow!(error.message))?;
        let explicit_job = project_session_event_to_loop(&state, &second_event)
            .await
            .map_err(|error| anyhow::anyhow!(error.message))?
            .ok_or_else(|| anyhow::anyhow!("user event was not explicitly projected"))?;

        let jobs = state
            .list_session_loop_jobs()
            .await
            .map_err(|error| anyhow::anyhow!(error.message))?;
        anyhow::ensure!(
            jobs.len() == 1,
            "expected one converged queued job: {jobs:?}"
        );
        let first_job = &jobs[0];
        anyhow::ensure!(first_job.id == explicit_job.id);
        anyhow::ensure!(first_job.status == SessionLoopJobStatus::Queued);
        anyhow::ensure!(first_job.pending_event_seq_start == Some(1));
        anyhow::ensure!(first_job.pending_event_seq_end == Some(2));

        state
            .start_session_loop_job(first_job.id, "projection-test-worker")
            .await
            .map_err(|error| anyhow::anyhow!(error.message))?;
        let next_event = state
            .append_event(
                "user",
                None,
                session_id,
                "user.message",
                json!({"message": "three"}),
            )
            .await
            .map_err(|error| anyhow::anyhow!(error.message))?;

        let jobs = state
            .list_session_loop_jobs()
            .await
            .map_err(|error| anyhow::anyhow!(error.message))?;
        let running = jobs
            .iter()
            .filter(|job| job.status == SessionLoopJobStatus::Running)
            .collect::<Vec<_>>();
        let queued = jobs
            .iter()
            .filter(|job| job.status == SessionLoopJobStatus::Queued)
            .collect::<Vec<_>>();
        anyhow::ensure!(running.len() == 1, "expected one running job: {jobs:?}");
        anyhow::ensure!(queued.len() == 1, "expected one queued job: {jobs:?}");
        anyhow::ensure!(
            queued[0].pending_event_seq_start
                == running[0].pending_event_seq_end.map(|seq| seq + 1)
        );
        anyhow::ensure!(queued[0].pending_event_seq_end == Some(next_event.seq));
        Ok(())
    }
    .await;

    let cleanup: Result<()> = async {
        let mut transaction = pool.begin().await?;
        for table in ["session_loop_jobs", "session_events", "sessions", "agents"] {
            sqlx::query(&format!("DELETE FROM {table} WHERE tenant_id = $1"))
                .bind(tenant_id)
                .execute(&mut *transaction)
                .await?;
        }
        sqlx::query("DELETE FROM tenants WHERE id = $1")
            .bind(tenant_id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }
    .await;
    cleanup?;
    outcome
}

#[test]
fn dynamic_workflow_cleanup_migration_is_restart_safe() {
    let migration = include_str!("../../../../db/migrations/0076_drop_dynamic_workflow_plans.sql");

    assert!(migration.contains("SET execution_strategy = 'native_steps'"));
    assert!(
        migration.contains("WHERE execution_strategy IN ('native_dynamic', 'dynamic_workflow')")
    );
    assert!(migration.contains("SET runtime_mode = 'normal'"));
    assert!(migration.contains("WHERE runtime_mode = 'dynamic_workflow'"));
    assert_eq!(migration.matches("UPDATE workflow_definitions").count(), 2);
    assert_eq!(migration.matches("UPDATE workflow_runs").count(), 2);
    assert!(migration.contains("DROP TABLE IF EXISTS dynamic_workflow_plans"));
}

#[test]
fn session_event_loop_projection_migration_is_restart_safe() {
    let migration =
        include_str!("../../../../db/migrations/0077_session_event_loop_projection.sql");

    assert!(migration.contains("CREATE OR REPLACE FUNCTION project_session_event_to_loop_job"));
    assert!(migration.contains("AFTER INSERT ON session_events"));
    assert!(migration.contains("ON CONFLICT (tenant_id, session_id)"));
    assert!(!migration.contains("NEW.event_type = 'approval.approved'"));
    assert!(migration.contains("NEW.event_type = 'approval.rejected'"));
    assert!(migration.contains("NEW.event_type = 'execution.completed'"));
    assert!(migration.contains("NEW.event_type = 'tool.result'"));
    assert!(migration.contains("FROM execution_jobs"));
    assert!(migration.contains("tool_call_id = NEW.actor_id"));
    assert!(migration.contains("status IN ('queued', 'running', 'cancel_requested')"));
}

#[test]
fn promoted_agent_release_uniqueness_migration_is_restart_safe() {
    let migration =
        include_str!("../../../../db/migrations/0075_agent_release_promoted_unique.sql");

    assert!(migration.contains("row_number() OVER"));
    assert!(migration.contains("SET status = 'superseded'"));
    assert!(migration.contains("CREATE UNIQUE INDEX IF NOT EXISTS"));
    assert!(migration.contains("uq_agent_releases_promoted_target"));
    assert!(migration.contains("lower(environment)"));
    assert!(migration.contains("WHERE status = 'promoted'"));
    assert!(migration.contains("workflow_pack_installation_ids"));
    assert!(migration.contains("jsonb_build_array"));
    assert!(migration.contains("jsonb_array_elements_text"));
    assert!(migration.contains("WHEN automation_policy ->> 'source' = 'workflow_pack_release'"));
}

#[test]
fn project_github_bindings_migration_is_restart_safe() {
    let migration = include_str!("../../../../db/migrations/0062_project_github_bindings.sql");

    assert!(migration.contains("CREATE TABLE IF NOT EXISTS project_github_bindings"));
    assert!(migration.contains("CREATE INDEX IF NOT EXISTS ix_pgb_tenant"));
    assert!(migration.contains("CREATE INDEX IF NOT EXISTS ix_pgb_repo"));
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
