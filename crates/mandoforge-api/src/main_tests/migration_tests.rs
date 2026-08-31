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
    assert!(names.contains(&"0078_execution_completion_projection.sql"));
    assert!(names.contains(&"0079_workflow_step_claim_owner_version.sql"));
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

#[test]
fn workflow_step_claim_owner_migration_fails_legacy_running_claims_closed() {
    let migration =
        include_str!("../../../../db/migrations/0079_workflow_step_claim_owner_version.sql");
    assert!(migration.contains("SET status = CASE WHEN status = 'running' THEN 'failed'"));
    assert!(migration.contains("'status', 'outcome_unknown'"));
    assert!(migration.contains("'action', 'manual_reconciliation_required'"));
    assert!(migration.contains("SET status = 'requires_action'"));
    assert!(migration.contains("SET status = 'cancelled'"));
    assert!(migration.contains("step.workflow_run_id = task_grant.workflow_run_id"));
    assert!(!migration.contains("root_task_grant_id IS DISTINCT FROM task_grant.id"));
    assert!(!migration.contains("THEN 'queued'"));
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
    run_migrations(&pool).await.expect("apply migrations");
    verify_migrations_applied(&pool, TenantRuntimeMode::SingleRuntimeTenant)
        .await
        .expect("runtime database must verify the exact migration ledger");
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
async fn postgres_migration_runner_rejects_tenant_scoped_data_visibility() -> Result<()> {
    let database_url = std::env::var("MANDOFORGE_TEST_POSTGRES_URL")
        .expect("MANDOFORGE_TEST_POSTGRES_URL is required");
    let admin_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await?;
    run_migrations(&admin_pool).await?;

    let suffix = Uuid::new_v4().simple().to_string();
    let role_name = format!("mandoforge_scoped_migration_{}", &suffix[..16]);
    let table_name = format!("mandoforge_scoped_migration_{suffix}");
    let policy_name = format!("tenant_isolation_{table_name}");
    let filename = format!("9998_scoped_migration_{suffix}.sql");
    let directory = std::env::temp_dir().join(format!("mandoforge-migrations-{suffix}"));
    fs::create_dir_all(&directory)?;
    let path = directory.join(&filename);
    fs::write(&path, format!("UPDATE {table_name} SET migrated = TRUE;"))?;

    sqlx::raw_sql(&format!(
        "CREATE ROLE {role_name} NOLOGIN NOBYPASSRLS;
         CREATE TABLE {table_name} (tenant_id UUID NOT NULL, migrated BOOLEAN NOT NULL DEFAULT FALSE);
         ALTER TABLE {table_name} ENABLE ROW LEVEL SECURITY;
         ALTER TABLE {table_name} FORCE ROW LEVEL SECURITY;
         CREATE POLICY {policy_name} ON {table_name}
             USING (tenant_id = mandoforge_current_tenant_id())
             WITH CHECK (tenant_id = mandoforge_current_tenant_id());
         GRANT USAGE, CREATE ON SCHEMA public TO {role_name};
         GRANT SELECT, UPDATE ON {table_name} TO {role_name};
         GRANT SELECT, INSERT ON schema_migrations TO {role_name};"
    ))
    .execute(&admin_pool)
    .await?;
    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();
    sqlx::query(&format!(
        "INSERT INTO {table_name} (tenant_id) VALUES ($1), ($2)"
    ))
    .bind(tenant_a)
    .bind(tenant_b)
    .execute(&admin_pool)
    .await?;

    let role_setting = format!("SET ROLE {role_name}");
    let tenant_setting = format!("SET mandoforge.tenant_id = '{tenant_a}'");
    let scoped_pool = PgPoolOptions::new()
        .max_connections(1)
        .after_connect(move |connection, _| {
            let role_setting = role_setting.clone();
            let tenant_setting = tenant_setting.clone();
            Box::pin(async move {
                connection.execute(role_setting.as_str()).await?;
                connection.execute(tenant_setting.as_str()).await?;
                Ok(())
            })
        })
        .connect(&database_url)
        .await?;
    let error = run_migrations_from_paths(&scoped_pool, vec![path])
        .await
        .expect_err("global migration must not be recorded through one tenant's RLS view");
    let error_chain = format!("{error:#}");
    anyhow::ensure!(
        error_chain.contains("row-level security"),
        "unexpected migration failure: {error_chain}"
    );
    scoped_pool.close().await;

    let migrated_count: i64 =
        sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table_name} WHERE migrated"))
            .fetch_one(&admin_pool)
            .await?;
    anyhow::ensure!(migrated_count == 0);
    let ledger_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM schema_migrations WHERE filename = $1")
            .bind(&filename)
            .fetch_one(&admin_pool)
            .await?;
    anyhow::ensure!(ledger_count == 0);

    sqlx::raw_sql(&format!(
        "DROP OWNED BY {role_name};
         DROP ROLE {role_name};
         DROP TABLE {table_name};"
    ))
    .execute(&admin_pool)
    .await?;
    fs::remove_dir_all(directory)?;
    Ok(())
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

        let exporter = Arc::new(RecordingTelemetryExporter::default());
        let mut state = test_state_with_worker(Arc::new(InlineExecutionWorker));
        state.store = StoreBackend::Postgres(pool.clone());
        state.execution_queue = ExecutionQueue::postgres(pool.clone(), tenant_id);
        state.tenant_id = tenant_id;
        state.observability_config.otlp_endpoint = Some("http://otel.test".to_string());
        state.telemetry_exporter = exporter.clone();

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
            .map_err(|error| anyhow::anyhow!(error.message))?;
        anyhow::ensure!(
            explicit_job.is_none(),
            "database trigger projection should suppress explicit replay"
        );

        let jobs = state
            .list_session_loop_jobs()
            .await
            .map_err(|error| anyhow::anyhow!(error.message))?;
        anyhow::ensure!(
            jobs.len() == 1,
            "expected one converged queued job: {jobs:?}"
        );
        let first_job = &jobs[0];
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

        let interleaved_session_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO sessions (id, tenant_id, agent_id, title, status)
             VALUES ($1, $2, $3, 'Interleaved execution projection', 'idle')",
        )
        .bind(interleaved_session_id)
        .bind(tenant_id)
        .bind(agent_id)
        .execute(&pool)
        .await?;
        let tool_call_ids = [Uuid::new_v4(), Uuid::new_v4()];
        let approval_ids = [Uuid::new_v4(), Uuid::new_v4()];
        for index in 0..2 {
            sqlx::query(
                "INSERT INTO tool_calls
                    (id, tenant_id, session_id, tool_name, args, status, risk_level, policy_decision)
                 VALUES ($1, $2, $3, 'shell.exec', '{}'::jsonb, 'running', 'medium', '{}'::jsonb)",
            )
            .bind(tool_call_ids[index])
            .bind(tenant_id)
            .bind(interleaved_session_id)
            .execute(&pool)
            .await?;
            sqlx::query(
                "INSERT INTO approvals
                    (id, tenant_id, session_id, tool_call_id, action, risk_level, reason, status)
                 VALUES ($1, $2, $3, $4, 'shell.exec', 'medium', 'interleaving test', 'approved')",
            )
            .bind(approval_ids[index])
            .bind(tenant_id)
            .bind(interleaved_session_id)
            .bind(tool_call_ids[index])
            .execute(&pool)
            .await?;
        }
        let mut executing_jobs = Vec::new();
        let mut result_events = Vec::new();
        for index in 0..2 {
            let job = state
                .execution_queue
                .enqueue(ExecutionJobRequest {
                    session_id: interleaved_session_id,
                    environment_id: None,
                    approval_id: approval_ids[index],
                    tool_call_id: tool_call_ids[index],
                    tool_name: "shell.exec".to_string(),
                    max_attempts: None,
                })
                .await
                .map_err(|error| anyhow::anyhow!(error.message))?;
            let running = state
                .execution_queue
                .start(job.id, "postgres-interleaving-worker")
                .await
                .map_err(|error| anyhow::anyhow!(error.message))?;
            let executing = state
                .execution_queue
                .begin_executing_started(
                    job.id,
                    "postgres-interleaving-worker",
                    running.claim_generation,
                )
                .await
                .map_err(|error| anyhow::anyhow!(error.message))?;
            let result = state
                .append_event_once_for_execution_claim(
                    &executing,
                    ExecutionJobStatus::Executing,
                    Uuid::new_v4(),
                    "tool",
                    Some(tool_call_ids[index]),
                    interleaved_session_id,
                    "tool.result",
                    json!({
                        "execution_job_id": executing.id,
                        "tool_call_id": tool_call_ids[index],
                        "tool": executing.tool_name,
                        "content": {"approval": "approved", "index": index},
                        "execution_outcome_known": true,
                        "attempt_count": executing.attempt_count,
                        "claim_generation": executing.claim_generation
                    }),
                )
                .await
                .map_err(|error| anyhow::anyhow!(error.message))?;
            executing_jobs.push(executing);
            result_events.push(result);
        }
        for (index, executing) in executing_jobs.iter().enumerate() {
            let finalizing = state
                .execution_queue
                .begin_finalizing_started(
                    executing.id,
                    "postgres-interleaving-worker",
                    executing.claim_generation,
                    None,
                    json!({"stage": "tool_execution"}),
                )
                .await
                .map_err(|error| anyhow::anyhow!(error.message))?;
            let finished = state
                .execution_queue
                .finish_finalizing_started(
                    executing.id,
                    "postgres-interleaving-worker",
                    finalizing.claim_generation,
                    false,
                )
                .await
                .map_err(|error| anyhow::anyhow!(error.message))?;
            execution::publish_execution_completion_tail(&state, &finished)
                .await
                .map_err(|error| anyhow::anyhow!(error.message))?;
            let interleaved_jobs = state
                .list_session_loop_jobs()
                .await
                .map_err(|error| anyhow::anyhow!(error.message))?
                .into_iter()
                .filter(|job| job.session_id == interleaved_session_id)
                .collect::<Vec<_>>();
            if index == 0 {
                anyhow::ensure!(
                    interleaved_jobs.is_empty(),
                    "first completion must stay blocked by the second result: {interleaved_jobs:?}"
                );
            } else {
                anyhow::ensure!(interleaved_jobs.len() == 1);
                let completion_seq = state
                    .list_events(interleaved_session_id)
                    .await
                    .map_err(|error| anyhow::anyhow!(error.message))?
                    .into_iter()
                    .filter(|event| event.event_type == "execution.completed")
                    .map(|event| event.seq)
                    .max()
                    .context("latest completion event")?;
                anyhow::ensure!(
                    interleaved_jobs[0].pending_event_seq_start == Some(result_events[0].seq)
                );
                anyhow::ensure!(
                    interleaved_jobs[0].pending_event_seq_end == Some(completion_seq)
                );
            }
        }
        let completion_telemetry_count = exporter
            .events
            .lock()
            .await
            .iter()
            .filter(|event| event.name == "execution.completed")
            .count();
        anyhow::ensure!(completion_telemetry_count == 2);

        let failure_session_id = Uuid::new_v4();
        let failure_tool_call_id = Uuid::new_v4();
        let failure_approval_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO sessions (id, tenant_id, agent_id, title, status)
             VALUES ($1, $2, $3, 'Failed execution projection', 'requires_action')",
        )
        .bind(failure_session_id)
        .bind(tenant_id)
        .bind(agent_id)
        .execute(&pool)
        .await?;
        sqlx::query(
            "INSERT INTO tool_calls
                (id, tenant_id, session_id, tool_name, args, status, risk_level, policy_decision)
             VALUES ($1, $2, $3, 'shell.exec', '{}'::jsonb, 'running', 'medium', '{}'::jsonb)",
        )
        .bind(failure_tool_call_id)
        .bind(tenant_id)
        .bind(failure_session_id)
        .execute(&pool)
        .await?;
        sqlx::query(
            "INSERT INTO approvals
                (id, tenant_id, session_id, tool_call_id, action, risk_level, reason, status)
             VALUES ($1, $2, $3, $4, 'shell.exec', 'medium', 'failure test', 'approved')",
        )
        .bind(failure_approval_id)
        .bind(tenant_id)
        .bind(failure_session_id)
        .bind(failure_tool_call_id)
        .execute(&pool)
        .await?;
        let failure_job = state
            .execution_queue
            .enqueue(ExecutionJobRequest {
                session_id: failure_session_id,
                environment_id: None,
                approval_id: failure_approval_id,
                tool_call_id: failure_tool_call_id,
                tool_name: "shell.exec".to_string(),
                max_attempts: None,
            })
            .await
            .map_err(|error| anyhow::anyhow!(error.message))?;
        let failure_running = state
            .execution_queue
            .start(failure_job.id, "postgres-failure-worker")
            .await
            .map_err(|error| anyhow::anyhow!(error.message))?;
        let failure_executing = state
            .execution_queue
            .begin_executing_started(
                failure_job.id,
                "postgres-failure-worker",
                failure_running.claim_generation,
            )
            .await
            .map_err(|error| anyhow::anyhow!(error.message))?;
        let failure_finalizing = state
            .execution_queue
            .begin_finalizing_started(
                failure_job.id,
                "postgres-failure-worker",
                failure_executing.claim_generation,
                Some("known failure"),
                json!({"stage": "tool_known_failure"}),
            )
            .await
            .map_err(|error| anyhow::anyhow!(error.message))?;
        let failed = state
            .execution_queue
            .finish_finalizing_started(
                failure_job.id,
                "postgres-failure-worker",
                failure_finalizing.claim_generation,
                false,
            )
            .await
            .map_err(|error| anyhow::anyhow!(error.message))?;
        anyhow::ensure!(failed.status == ExecutionJobStatus::Failed);
        let failure_event = state
            .list_events(failure_session_id)
            .await
            .map_err(|error| anyhow::anyhow!(error.message))?
            .into_iter()
            .find(|event| execution_failure_event_matches_job(event, &failed))
            .context("trusted execution failure event")?;
        let failure_jobs = state
            .list_session_loop_jobs()
            .await
            .map_err(|error| anyhow::anyhow!(error.message))?
            .into_iter()
            .filter(|job| job.session_id == failure_session_id)
            .collect::<Vec<_>>();
        anyhow::ensure!(failure_jobs.len() == 1, "expected one failure projection");
        anyhow::ensure!(failure_jobs[0].trigger_event_id == Some(failure_event.id));
        anyhow::ensure!(failure_jobs[0].reason == "approved execution failed");
        execution::publish_execution_failure_tail(&state, &failed)
            .await
            .map_err(|error| anyhow::anyhow!(error.message))?;
        let failure_telemetry_count = exporter
            .events
            .lock()
            .await
            .iter()
            .filter(|event| event.name == "execution.failed")
            .count();
        anyhow::ensure!(failure_telemetry_count == 1);
        Ok(())
    }
    .await;

    let cleanup: Result<()> = async {
        let mut transaction = pool.begin().await?;
        for table in [
            "session_loop_jobs",
            "execution_jobs",
            "approvals",
            "tool_calls",
            "session_events",
            "sessions",
            "agents",
        ] {
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

#[tokio::test]
#[ignore = "requires MANDOFORGE_TEST_POSTGRES_URL"]
async fn postgres_execution_completion_migration_backfills_safe_legacy_range() -> Result<()> {
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
    let tool_call_ids = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
    let approval_ids = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
    let execution_job_ids = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
    let completion_event_ids = [Uuid::new_v4(), Uuid::new_v4()];
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
            .bind("Legacy execution completion test")
            .bind(format!("legacy-execution-completion-{}", tenant_id.simple()))
            .execute(&pool)
            .await?;
        sqlx::query(
            "INSERT INTO agents (id, tenant_id, name, kind, provider, model, system_prompt)
             VALUES ($1, $2, $3, 'orchestrator', 'test', 'test', '')",
        )
        .bind(agent_id)
        .bind(tenant_id)
        .bind("Legacy execution completion test agent")
        .execute(&pool)
        .await?;
        sqlx::query(
            "INSERT INTO sessions (id, tenant_id, agent_id, title, status)
             VALUES ($1, $2, $3, $4, 'idle')",
        )
        .bind(session_id)
        .bind(tenant_id)
        .bind(agent_id)
        .bind("Legacy execution completion test session")
        .execute(&pool)
        .await?;
        for (index, tool_call_id) in tool_call_ids.iter().enumerate() {
            let status = if index < 2 { "completed" } else { "running" };
            sqlx::query(
                "INSERT INTO tool_calls
                    (id, tenant_id, session_id, tool_name, args, status, risk_level, policy_decision)
                 VALUES ($1, $2, $3, 'shell.exec', '{}'::jsonb, $4, 'medium', '{}'::jsonb)",
            )
            .bind(tool_call_id)
            .bind(tenant_id)
            .bind(session_id)
            .bind(status)
            .execute(&pool)
            .await?;
            sqlx::query(
                "INSERT INTO approvals
                    (id, tenant_id, session_id, tool_call_id, action, risk_level, reason, status)
                 VALUES ($1, $2, $3, $4, 'shell.exec', 'medium', 'legacy test', 'approved')",
            )
            .bind(approval_ids[index])
            .bind(tenant_id)
            .bind(session_id)
            .bind(tool_call_id)
            .execute(&pool)
            .await?;
        }
        for index in 0..2 {
            sqlx::query(
                "INSERT INTO execution_jobs
                    (id, tenant_id, session_id, approval_id, tool_call_id, tool_name, status,
                     completed_at, worker_id, claim_generation, finalization_details,
                     attempt_count, max_attempts)
                 VALUES ($1, $2, $3, $4, $5, 'shell.exec', 'completed', now(), 'legacy-worker',
                         $6, '{\"stage\":\"completion_published\"}'::jsonb, 1, 3)",
            )
            .bind(execution_job_ids[index])
            .bind(tenant_id)
            .bind(session_id)
            .bind(approval_ids[index])
            .bind(tool_call_ids[index])
            .bind(7_i64 + index as i64)
            .execute(&pool)
            .await?;
        }
        sqlx::query(
            "INSERT INTO execution_jobs
                (id, tenant_id, session_id, approval_id, tool_call_id, tool_name, status,
                 worker_id, claim_generation, finalization_details, attempt_count, max_attempts)
             VALUES ($1, $2, $3, $4, $5, 'shell.exec', 'executing', 'active-worker',
                     9, '{}'::jsonb, 1, 3)",
        )
        .bind(execution_job_ids[2])
        .bind(tenant_id)
        .bind(session_id)
        .bind(approval_ids[2])
        .bind(tool_call_ids[2])
        .execute(&pool)
        .await?;
        for index in 0..2 {
            let result_seq = 1_i64 + (index as i64 * 2);
            let completion_seq = result_seq + 1;
            sqlx::query(
                "INSERT INTO session_events
                    (id, tenant_id, session_id, seq, actor_type, actor_id, event_type, payload)
                 VALUES
                    ($1, $2, $3, $4, 'tool', $5, 'tool.result', $6),
                    ($7, $2, $3, $8, 'worker', $9, 'execution.completed', $10)",
            )
            .bind(Uuid::new_v4())
            .bind(tenant_id)
            .bind(session_id)
            .bind(result_seq)
            .bind(tool_call_ids[index])
            .bind(json!({
                "tool_call_id": tool_call_ids[index],
                "content": {"approval": "approved"}
            }))
            .bind(completion_event_ids[index])
            .bind(completion_seq)
            .bind(execution_job_ids[index])
            .bind(json!({
                "execution_job_id": execution_job_ids[index],
                "approval_id": approval_ids[index],
                "tool_call_id": tool_call_ids[index],
                "tool": "shell.exec",
                "status": "completed",
                "worker_id": "legacy-worker",
                "reason": "approved execution completed"
            }))
            .execute(&pool)
            .await?;
        }
        sqlx::query(
            "INSERT INTO session_events
                (id, tenant_id, session_id, seq, actor_type, actor_id, event_type, payload)
             VALUES ($1, $2, $3, 5, 'tool', $4, 'tool.result', $5)",
        )
        .bind(Uuid::new_v4())
        .bind(tenant_id)
        .bind(session_id)
        .bind(tool_call_ids[2])
        .bind(json!({
            "tool_call_id": tool_call_ids[2],
            "content": {"approval": "approved", "value": "still hidden"}
        }))
        .execute(&pool)
        .await?;

        let pre_migration_jobs = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM session_loop_jobs WHERE tenant_id = $1 AND session_id = $2",
        )
        .bind(tenant_id)
        .bind(session_id)
        .fetch_one(&pool)
        .await?;
        anyhow::ensure!(pre_migration_jobs == 0);

        sqlx::raw_sql(include_str!(
            "../../../../db/migrations/0078_execution_completion_projection.sql"
        ))
        .execute(&pool)
        .await?;

        for (index, completion_event_id) in completion_event_ids.iter().enumerate() {
            let payload = sqlx::query_scalar::<_, Value>(
                "SELECT payload FROM session_events WHERE tenant_id = $1 AND id = $2",
            )
            .bind(tenant_id)
            .bind(completion_event_id)
            .fetch_one(&pool)
            .await?;
            anyhow::ensure!(payload["attempt_count"] == json!(1));
            anyhow::ensure!(payload["claim_generation"] == json!(7_i64 + index as i64));
        }

        let migrated_ranges = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM session_loop_jobs
             WHERE tenant_id = $1 AND session_id = $2 AND status = 'queued'",
        )
        .bind(tenant_id)
        .bind(session_id)
        .fetch_one(&pool)
        .await?;
        anyhow::ensure!(migrated_ranges == 1);

        sqlx::query(
            "INSERT INTO session_events
                (id, tenant_id, session_id, seq, actor_type, event_type, payload)
             VALUES ($1, $2, $3, 6, 'user', 'user.message', jsonb_build_object('message', 'continue'))",
        )
        .bind(Uuid::new_v4())
        .bind(tenant_id)
        .bind(session_id)
        .execute(&pool)
        .await?;
        let range = sqlx::query_as::<_, (Option<i64>, Option<i64>)>(
            "SELECT pending_event_seq_start, pending_event_seq_end
             FROM session_loop_jobs
             WHERE tenant_id = $1 AND session_id = $2 AND status = 'queued'",
        )
        .bind(tenant_id)
        .bind(session_id)
        .fetch_one(&pool)
        .await?;
        anyhow::ensure!(range == (Some(1), Some(4)), "unexpected recovery range: {range:?}");
        Ok(())
    }
    .await;

    let cleanup: Result<()> = async {
        let mut transaction = pool.begin().await?;
        for table in [
            "session_loop_jobs",
            "execution_jobs",
            "approvals",
            "tool_calls",
            "session_events",
            "sessions",
            "agents",
        ] {
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
fn execution_completion_projection_migration_is_restart_safe() {
    let migration =
        include_str!("../../../../db/migrations/0078_execution_completion_projection.sql");

    assert!(migration.contains("CREATE OR REPLACE FUNCTION project_session_event_to_loop_job"));
    assert!(migration.contains("UPDATE remote_computer_job_assignments AS assignment"));
    assert!(migration.contains("ADD COLUMN IF NOT EXISTS claim_generation BIGINT"));
    assert!(migration.contains("ADD COLUMN IF NOT EXISTS finalization_details JSONB"));
    assert!(migration.contains("jsonb_typeof(assignment.metadata) = 'object'"));
    assert!(migration.contains("jsonb_build_object('legacy_metadata', assignment.metadata)"));
    assert!(migration.contains("UPDATE session_events AS event"));
    assert!(migration.contains("WITH backfilled_completion_events AS"));
    assert!(migration.contains("unresolved_boundaries AS"));
    assert!(migration.contains("backfilled_failure_events AS"));
    assert!(migration.contains("safe_outcome_events AS"));
    assert!(migration.contains("outcome_recovery_ranges AS"));
    assert!(migration.contains("SELECT DISTINCT ON (tenant_id, session_id)"));
    assert!(migration.contains("outcome_seq AS pending_end"));
    assert!(!migration.contains("tail.seq AS pending_end"));
    assert!(migration.contains("INSERT INTO session_loop_jobs"));
    assert!(migration.contains("GREATEST(tool_result_seq, high_watermark + 1)"));
    assert!(migration.contains("NOT event.payload ? 'attempt_count'"));
    assert!(migration.contains("NOT event.payload ? 'claim_generation'"));
    assert!(migration.contains("event.actor_id = job.id"));
    assert!(migration.contains("event.payload ->> 'execution_job_id' = job.id::text"));
    assert!(migration.contains("event.payload ->> 'tool_call_id' = job.tool_call_id::text"));
    assert!(migration.contains("'execution_attempt_count'"));
    assert!(migration.contains("'execution_claim_generation'"));
    assert!(migration.contains("NEW.event_type = 'tool.result'"));
    assert!(migration.contains("FROM execution_jobs"));
    assert!(migration.contains("tool_call_id = NEW.actor_id"));
    assert!(!migration.contains("AND status IN"));
    assert!(migration.contains("NEW.actor_type = 'worker'"));
    assert!(migration.contains("NEW.payload ->> 'status' = 'completed'"));
    assert!(migration.contains("NEW.payload ->> 'status' = 'failed'"));
    assert!(migration.contains("status = 'completed'"));
    assert!(migration.contains("claim_generation::text = NEW.payload ->> 'claim_generation'"));
    assert!(migration.contains("tool_call_id::text = NEW.payload ->> 'tool_call_id'"));
    assert!(migration.contains("seq > COALESCE(projected_high_watermark, 0)"));
    assert!(migration.contains("cursor_high_watermark := execution_tool_result_seq - 1"));
    assert!(migration.contains("SELECT MIN(result.seq)"));
    assert!(migration.contains("JOIN execution_jobs AS completed_job"));
    assert!(migration.contains("completed_job.id = NEW.actor_id"));
    assert!(migration.contains("hidden_result.seq <= NEW.seq"));
    assert!(migration.contains("hidden_job.status <> 'completed'"));
    assert!(migration.contains("completion.event_type = 'execution.completed'"));
    assert!(
        migration.contains(
            "completion.payload ->> 'claim_generation' = hidden_job.claim_generation::text"
        )
    );
    assert!(migration.contains("CREATE OR REPLACE FUNCTION record_terminal_execution_event"));
    assert!(migration.contains("pg_advisory_xact_lock"));
    assert!(migration.contains("INSERT INTO session_events"));
    assert!(migration.contains("'execution.completed'"));
    assert!(migration.contains("'execution.failed'"));
    assert!(migration.contains("'claim_generation', NEW.claim_generation"));
    assert!(migration.contains("AFTER UPDATE OF status ON execution_jobs"));
    assert!(migration.contains("EXECUTE FUNCTION record_terminal_execution_event()"));
    assert!(!migration.contains("IF completion_event_seq IS NULL"));
    assert!(!migration.contains("UPDATE sessions"));
    assert!(!migration.contains("UPDATE session_threads"));
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
