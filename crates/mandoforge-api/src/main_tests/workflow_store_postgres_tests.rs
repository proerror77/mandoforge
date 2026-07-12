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
async fn postgres_dynamic_workflow_review_compare_and_set_blocks_stale_status() {
    let state = postgres_workflow_store_state().await;
    let now = Utc::now();
    let plan = state
        .create_dynamic_workflow_plan(DynamicWorkflowPlan {
            id: Uuid::new_v4(),
            source_work_item_id: None,
            source_session_id: None,
            objective: "Verify PostgreSQL review compare-and-set".to_string(),
            status: "proposed".to_string(),
            phases: json!([]),
            agent_fleet_policy: json!({}),
            governance: json!({}),
            validation: json!({}),
            materialization: json!({}),
            analysis: json!({}),
            review: json!({}),
            workflow_definition_id: None,
            workflow_run_id: None,
            audit_trace_id: None,
            created_at: now,
            updated_at: now,
            reviewed_at: None,
            materialized_at: None,
        })
        .await
        .expect("create postgres dynamic workflow plan");

    let approved = state
        .update_dynamic_workflow_plan_review(
            plan.id,
            "proposed",
            "approved".to_string(),
            json!({"approved_by": "postgres-reviewer"}),
            None,
            now,
        )
        .await
        .expect("approve postgres dynamic workflow plan");
    assert_eq!(approved.status, "approved");

    let error = state
        .update_dynamic_workflow_plan_review(
            plan.id,
            "proposed",
            "reviewed".to_string(),
            json!({"note": "stale operator review"}),
            None,
            Utc::now(),
        )
        .await
        .expect_err("stale review status must conflict");
    assert_eq!(error.status, StatusCode::CONFLICT);
    assert_eq!(
        state
            .get_dynamic_workflow_plan(plan.id)
            .await
            .expect("read persisted postgres plan")
            .status,
        "approved"
    );

    let first_claim_audit = state
        .append_audit_log(new_audit_log(
            None,
            "system",
            None,
            "dynamic_workflow_plan.materialization_claim_requested",
            "dynamic_workflow_plan",
            Some(plan.id),
            json!({"attempt": 1}),
        ))
        .await
        .expect("create postgres materialization claim audit");
    state
        .claim_dynamic_workflow_plan_materialization(plan.id, first_claim_audit.id, Utc::now())
        .await
        .expect("claim postgres materialization");
    state
        .fail_dynamic_workflow_plan_materialization(plan.id, first_claim_audit.id, None, Utc::now())
        .await
        .expect("fail postgres materialization");
    let reapproved = state
        .update_dynamic_workflow_plan_review(
            plan.id,
            "materialization_failed",
            "approved".to_string(),
            json!({"approved_by": "postgres-retry-reviewer"}),
            None,
            Utc::now(),
        )
        .await
        .expect("reapprove failed postgres materialization");
    assert_eq!(reapproved.status, "approved");
    let retry_claim_audit = state
        .append_audit_log(new_audit_log(
            None,
            "system",
            None,
            "dynamic_workflow_plan.materialization_claim_requested",
            "dynamic_workflow_plan",
            Some(plan.id),
            json!({"attempt": 2}),
        ))
        .await
        .expect("create postgres retry claim audit");
    let reclaimed = state
        .claim_dynamic_workflow_plan_materialization(plan.id, retry_claim_audit.id, Utc::now())
        .await
        .expect("reclaim reapproved postgres materialization");
    assert_eq!(reclaimed.status, "materializing");
    let late_review_audit = state
        .append_audit_log(new_audit_log(
            None,
            "postgres-retry-reviewer",
            None,
            "dynamic_workflow_plan.reviewed",
            "dynamic_workflow_plan",
            Some(plan.id),
            json!({"status": "approved"}),
        ))
        .await
        .expect("create late postgres review audit");
    let still_claimed = state
        .update_dynamic_workflow_plan_audit_trace_if_unchanged(
            plan.id,
            &reapproved.status,
            reapproved.updated_at,
            Some(late_review_audit.id),
            late_review_audit.created_at,
        )
        .await
        .expect("late postgres review audit attachment converges");
    assert_eq!(still_claimed.status, "materializing");
    assert_eq!(still_claimed.audit_trace_id, Some(retry_claim_audit.id));
    let retry_failed = state
        .fail_dynamic_workflow_plan_materialization(plan.id, retry_claim_audit.id, None, Utc::now())
        .await
        .expect("postgres retry claim remains valid after late review audit");
    assert_eq!(retry_failed.status, "materialization_failed");
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
