use super::*;

async fn postgres_workflow_store_state() -> AppState {
    let database_url = std::env::var("MANDOFORGE_TEST_POSTGRES_URL")
        .expect("MANDOFORGE_TEST_POSTGRES_URL is required");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect(&database_url)
        .await
        .expect("connect test postgres");
    run_migrations(&pool).await.expect("run migrations");
    let mut state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    seed_demo_tenant(&pool, state.tenant_id)
        .await
        .expect("seed tenant");
    state.store = StoreBackend::Postgres(pool);
    state
}

fn postgres_workflow_run(
    workflow_definition_id: Uuid,
    primary_session_id: Uuid,
    status: &str,
    now: DateTime<Utc>,
) -> WorkflowRun {
    WorkflowRun {
        id: Uuid::new_v4(),
        workflow_definition_id,
        pack_installation_id: None,
        source_event_id: None,
        source_work_item_id: None,
        source_schedule_id: None,
        status: status.to_string(),
        primary_session_id,
        root_task_grant_id: None,
        input_payload: json!({}),
        input_digest: format!("postgres-workflow-store-{}", Uuid::new_v4()),
        execution_strategy: "managed_graph".to_string(),
        runtime_adapter: None,
        runtime_mode: None,
        delegation_status: None,
        external_run_ref: None,
        runtime_event_cursor: None,
        runtime_envelope: json!({}),
        started_at: None,
        completed_at: None,
        audit_trace_id: None,
        created_at: now,
        updated_at: now,
    }
}

fn postgres_workflow_step(
    workflow_run_id: Uuid,
    step_key: &str,
    now: DateTime<Utc>,
) -> WorkflowStepRun {
    WorkflowStepRun {
        id: Uuid::new_v4(),
        workflow_run_id,
        step_key: step_key.to_string(),
        step_type: "agent".to_string(),
        agent_id: None,
        agent_version_id: None,
        session_id: None,
        thread_id: None,
        handoff_id: None,
        task_grant_id: None,
        environment_id: None,
        status: "queued".to_string(),
        input_payload: json!({}),
        output_payload: json!({}),
        artifact_ids: Vec::new(),
        approval_ids: Vec::new(),
        tool_call_ids: Vec::new(),
        claimed_by_worker: None,
        claim_owner_version: 0,
        lease_expires_at: None,
        context_packet_id: None,
        started_at: None,
        completed_at: None,
        scheduled_at: None,
        created_at: now,
        updated_at: now,
    }
}

#[tokio::test]
#[ignore = "requires MANDOFORGE_TEST_POSTGRES_URL"]
async fn postgres_terminal_workflow_run_rejects_new_step_reservations() {
    let state = postgres_workflow_store_state().await;
    state.seed_demo_agent().await.expect("seed demo agent");
    let agent = state
        .list_agents()
        .await
        .expect("list postgres agents")
        .into_iter()
        .next()
        .expect("seeded postgres agent");
    let now = Utc::now();
    let definition = state
        .create_workflow_definition(WorkflowDefinition {
            id: Uuid::new_v4(),
            pack_installation_id: None,
            pack_id: None,
            pack_version: None,
            name: "Postgres terminal step guard".to_string(),
            entrypoint: format!("postgres-terminal-step-{}", Uuid::new_v4()),
            trigger_type: "manual".to_string(),
            default_agent_id: agent.id,
            default_environment_id: None,
            input_schema_ref: None,
            output_schema_ref: None,
            step_graph: json!({}),
            handoff_rules: json!({}),
            execution_strategy: "managed_graph".to_string(),
            runtime_adapter: None,
            runtime_mode: None,
            runtime_capability_contract: json!({}),
            event_ingestion_policy: default_event_ingestion_policy(),
            approval_policy_ref: None,
            eval_gate_refs: Vec::new(),
            release_state: "released".to_string(),
            created_at: now,
            updated_at: now,
            archived_at: None,
        })
        .await
        .expect("create postgres workflow definition");
    let session = state
        .create_session(CreateSession {
            agent_id: agent.id,
            environment_id: None,
            title: "Postgres terminal step guard".to_string(),
            message: None,
        })
        .await
        .expect("create postgres workflow session");
    let run = state
        .create_workflow_run(postgres_workflow_run(
            definition.id,
            session.id,
            "queued",
            now,
        ))
        .await
        .expect("create postgres workflow run");
    state
        .create_workflow_step_run_if_key_absent(postgres_workflow_step(run.id, "initial-step", now))
        .await
        .expect("reserve initial postgres step")
        .expect("initial postgres step is created");
    state
        .update_workflow_run_status(run.id, "completed".to_string(), Some(now), Some(Utc::now()))
        .await
        .expect("complete postgres workflow run");

    let error = state
        .create_workflow_step_run_if_key_absent(postgres_workflow_step(
            run.id,
            "late-step",
            Utc::now(),
        ))
        .await
        .expect_err("terminal postgres workflow run must reject new steps");
    assert_eq!(error.status, StatusCode::FORBIDDEN);
    assert_eq!(error.message, "workflow run is not executable");
    let error = state
        .create_workflow_step_run(postgres_workflow_step(
            run.id,
            "late-internal-step",
            Utc::now(),
        ))
        .await
        .expect_err("terminal postgres workflow run must reject internal steps");
    assert_eq!(error.status, StatusCode::FORBIDDEN);
    assert_eq!(error.message, "workflow run is not executable");
    assert_eq!(
        state
            .list_workflow_step_runs(run.id)
            .await
            .expect("list postgres workflow steps")
            .len(),
        1
    );
}

#[tokio::test]
#[ignore = "requires MANDOFORGE_TEST_POSTGRES_URL"]
async fn postgres_claim_owner_migration_blocks_legacy_claim_replay() {
    let state = postgres_workflow_store_state().await;
    state.seed_demo_agent().await.expect("seed demo agent");
    let agent = state
        .list_agents()
        .await
        .expect("list postgres agents")
        .into_iter()
        .next()
        .expect("seeded postgres agent");
    let now = Utc::now();
    let definition = state
        .create_workflow_definition(WorkflowDefinition {
            id: Uuid::new_v4(),
            pack_installation_id: None,
            pack_id: None,
            pack_version: None,
            name: "Postgres legacy claim migration".to_string(),
            entrypoint: format!("postgres-legacy-claim-{}", Uuid::new_v4()),
            trigger_type: "manual".to_string(),
            default_agent_id: agent.id,
            default_environment_id: None,
            input_schema_ref: None,
            output_schema_ref: None,
            step_graph: json!({}),
            handoff_rules: json!({}),
            execution_strategy: "managed_graph".to_string(),
            runtime_adapter: None,
            runtime_mode: None,
            runtime_capability_contract: json!({}),
            event_ingestion_policy: default_event_ingestion_policy(),
            approval_policy_ref: None,
            eval_gate_refs: Vec::new(),
            release_state: "released".to_string(),
            created_at: now,
            updated_at: now,
            archived_at: None,
        })
        .await
        .expect("create postgres workflow definition");
    let session = state
        .create_session(CreateSession {
            agent_id: agent.id,
            environment_id: None,
            title: "Postgres legacy claim migration".to_string(),
            message: None,
        })
        .await
        .expect("create postgres workflow session");
    let run = state
        .create_workflow_run(postgres_workflow_run(
            definition.id,
            session.id,
            "running",
            now,
        ))
        .await
        .expect("create postgres workflow run");
    let root_grant = issue_root_task_grant_for_workflow_run(&state, &run, &definition, &session)
        .await
        .expect("create postgres root task grant");
    let run = state
        .update_workflow_run_root_task_grant(run.id, root_grant.id)
        .await
        .expect("bind postgres root task grant");
    let mut step_input = postgres_workflow_step(run.id, "legacy-running-step", now);
    step_input.agent_id = Some(agent.id);
    step_input.session_id = Some(session.id);
    step_input.task_grant_id = Some(root_grant.id);
    let step = state
        .create_workflow_step_run_if_key_absent(step_input)
        .await
        .expect("create postgres workflow step")
        .expect("postgres workflow step created");
    let mut child_grant = root_grant.clone();
    child_grant.id = Uuid::new_v4();
    child_grant.parent_grant_id = Some(root_grant.id);
    let child_grant = state
        .create_task_grant(child_grant)
        .await
        .expect("create postgres child task grant");
    let StoreBackend::Postgres(pool) = &state.store else {
        panic!("test state must use postgres store");
    };
    let mut transaction = pool
        .begin()
        .await
        .expect("begin migration test transaction");
    sqlx::query(
        "ALTER TABLE workflow_step_runs DROP CONSTRAINT workflow_step_runs_claim_owner_version_check",
    )
    .execute(&mut *transaction)
    .await
    .expect("simulate pre-migration claim schema");
    sqlx::query(
        "UPDATE workflow_step_runs
         SET status = 'running', claimed_by_worker = 'subject:alice', claim_owner_version = 0,
             lease_expires_at = now() + interval '5 minutes'
         WHERE tenant_id = $1 AND id = $2",
    )
    .bind(state.current_tenant_id())
    .bind(step.id)
    .execute(&mut *transaction)
    .await
    .expect("insert legacy active claim");
    sqlx::raw_sql(include_str!(
        "../../../../db/migrations/0079_workflow_step_claim_owner_version.sql"
    ))
    .execute(&mut *transaction)
    .await
    .expect("apply claim owner migration");

    let migrated = sqlx::query_as::<_, (String, Option<String>, i16, Value)>(
        "SELECT status, claimed_by_worker, claim_owner_version, output_payload
         FROM workflow_step_runs WHERE tenant_id = $1 AND id = $2",
    )
    .bind(state.current_tenant_id())
    .bind(step.id)
    .fetch_one(&mut *transaction)
    .await
    .expect("read migrated claim");
    assert_eq!(migrated.0, "failed");
    assert_eq!(migrated.1, None);
    assert_eq!(migrated.2, 0);
    assert_eq!(migrated.3["claim_migration"]["status"], "outcome_unknown");

    let replayed = sqlx::query_scalar::<_, Uuid>(
        "UPDATE workflow_step_runs
         SET status = 'running', claimed_by_worker = 'subject:alice', claim_owner_version = 1
         WHERE tenant_id = $1 AND id = $2
           AND ((status = 'queued' AND claimed_by_worker IS NULL AND claim_owner_version = 0)
             OR (status = 'running' AND claim_owner_version = 1 AND lease_expires_at <= now()))
         RETURNING id",
    )
    .bind(state.current_tenant_id())
    .bind(step.id)
    .fetch_optional(&mut *transaction)
    .await
    .expect("attempt replay claim");
    assert_eq!(replayed, None);

    let run_status = sqlx::query_scalar::<_, String>(
        "SELECT status FROM workflow_runs WHERE tenant_id = $1 AND id = $2",
    )
    .bind(state.current_tenant_id())
    .bind(run.id)
    .fetch_one(&mut *transaction)
    .await
    .expect("read migrated workflow run");
    assert_eq!(run_status, "requires_action");

    for grant_id in [root_grant.id, child_grant.id] {
        let grant_status = sqlx::query_scalar::<_, String>(
            "SELECT status FROM task_grants WHERE tenant_id = $1 AND id = $2",
        )
        .bind(state.current_tenant_id())
        .bind(grant_id)
        .fetch_one(&mut *transaction)
        .await
        .expect("read migrated task grant");
        assert_eq!(grant_status, "cancelled");
    }
    transaction
        .commit()
        .await
        .expect("commit migration test transaction");

    let error = enforce_task_grant_for_tool_invocation(
        &state,
        "file.write",
        &ExecuteTool {
            session_id: session.id,
            task_grant_id: Some(root_grant.id),
            args: json!({"path": "should-not-run"}),
        },
    )
    .await
    .expect_err("migrated root task grant must not authorize tool execution");
    assert_eq!(error.status, StatusCode::FORBIDDEN);
    assert_eq!(error.message, "task grant is not active");
}
