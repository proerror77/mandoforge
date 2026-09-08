use super::*;

struct HarnessTestProvider {
    fail: bool,
}

#[async_trait::async_trait]
impl ProviderClient for HarnessTestProvider {
    fn name(&self) -> &'static str {
        "harness-test-provider"
    }

    async fn complete(&self, _context: HarnessContext) -> Result<ProviderResponse, AppError> {
        if self.fail {
            return Err(AppError::bad_request("provider harness test failure"));
        }
        Ok(ProviderResponse {
            plan: vec!["return the governed response".to_string()],
            tool_calls: Vec::new(),
            final_message: Some("harness completed".to_string()),
            usage: None,
        })
    }
}

async fn harness_test_session() -> (AppState, Session) {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    state.seed_demo_agent().await.expect("seed demo agent");
    let agent = state
        .list_agents()
        .await
        .expect("list agents")
        .into_iter()
        .next()
        .expect("seeded agent");
    let session = state
        .create_session(CreateSession {
            agent_id: agent.id,
            environment_id: None,
            title: "provider harness test".to_string(),
            message: None,
        })
        .await
        .expect("create session");
    (state, session)
}

async fn postgres_harness_test_session() -> (AppState, Session) {
    let database_url = std::env::var("MANDOFORGE_TEST_POSTGRES_URL")
        .expect("MANDOFORGE_TEST_POSTGRES_URL is required");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url)
        .await
        .expect("connect test postgres");
    run_migrations(&pool).await.expect("run migrations");
    let mut state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    seed_demo_tenant(&pool, state.tenant_id)
        .await
        .expect("seed tenant");
    state.store = StoreBackend::Postgres(pool.clone());
    state.execution_queue = ExecutionQueue::postgres(pool, state.tenant_id);
    state.seed_demo_agent().await.expect("seed demo agent");
    let agent = state
        .list_agents()
        .await
        .expect("list agents")
        .into_iter()
        .next()
        .expect("seeded agent");
    let session = state
        .create_session(CreateSession {
            agent_id: agent.id,
            environment_id: None,
            title: "postgres provider harness test".to_string(),
            message: None,
        })
        .await
        .expect("create session");
    (state, session)
}

fn waiting_ontology_action_call(session_id: Uuid) -> ToolCall {
    ToolCall {
        id: Uuid::new_v4(),
        session_id,
        event_id: None,
        tool_name: "ontology.action.execute".to_string(),
        args: json!({}),
        task_grant_id: None,
        normalized_args_hash: None,
        target_binding: empty_json_object(),
        status: "waiting_approval".to_string(),
        risk_level: "medium".to_string(),
        policy_decision: json!({"decision": "requires_approval"}),
        result: None,
        error: None,
        started_at: None,
        completed_at: None,
        created_at: Utc::now(),
    }
}

fn harness_task_grant(session_id: Uuid, agent_id: Uuid) -> TaskGrant {
    let now = Utc::now();
    TaskGrant {
        id: Uuid::new_v4(),
        workflow_run_id: Uuid::new_v4(),
        workflow_step_run_id: None,
        session_id: Some(session_id),
        parent_grant_id: None,
        source_event_id: None,
        source_handoff_id: None,
        issuer_subject: "test".to_string(),
        grantee_agent_id: Some(agent_id),
        grantee_session_id: Some(session_id),
        agent_class: None,
        objective: "test atomic invocation".to_string(),
        risk_level: "low".to_string(),
        status: "active".to_string(),
        expires_at: None,
        max_turns: None,
        max_tool_calls: Some(1),
        max_runtime_seconds: None,
        max_cost_usd_micros: None,
        turns_used: 0,
        tool_calls_used: 0,
        cost_usd_micros_used: 0,
        semantic_scopes: empty_json_object(),
        memory_scope: empty_json_object(),
        tool_scope: json!({"read": ["file.read"]}),
        connector_scope: empty_json_object(),
        approval_policy: empty_json_object(),
        external_effects: empty_json_object(),
        context_packet_id: None,
        policy_revision_id: None,
        immutable_args_hash: None,
        audit_trace_id: None,
        created_at: now,
        updated_at: now,
    }
}

async fn persisted_harness_task_grant(state: &AppState, session: &Session) -> TaskGrant {
    let now = Utc::now();
    let definition = state
        .create_workflow_definition(WorkflowDefinition {
            id: Uuid::new_v4(),
            pack_installation_id: None,
            pack_id: None,
            pack_version: None,
            name: "Provider harness TaskGrant".to_string(),
            entrypoint: format!("provider-harness-{}", Uuid::new_v4()),
            trigger_type: "manual".to_string(),
            default_agent_id: session.agent_id,
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
        .expect("create provider harness workflow definition");
    let run = state
        .create_workflow_run(WorkflowRun {
            id: Uuid::new_v4(),
            workflow_definition_id: definition.id,
            pack_installation_id: None,
            source_event_id: None,
            source_work_item_id: None,
            source_schedule_id: None,
            status: "running".to_string(),
            primary_session_id: session.id,
            root_task_grant_id: None,
            input_payload: json!({}),
            input_digest: format!("provider-harness-{}", Uuid::new_v4()),
            execution_strategy: "managed_graph".to_string(),
            runtime_adapter: None,
            runtime_mode: None,
            delegation_status: None,
            external_run_ref: None,
            runtime_event_cursor: None,
            runtime_envelope: json!({}),
            started_at: Some(now),
            completed_at: None,
            audit_trace_id: None,
            created_at: now,
            updated_at: now,
        })
        .await
        .expect("create provider harness workflow run");
    let mut grant = harness_task_grant(session.id, session.agent_id);
    grant.workflow_run_id = run.id;
    let step_id = Uuid::new_v4();
    grant.workflow_step_run_id = Some(step_id);
    grant.tool_scope = json!({"write": ["ontology.action.execute"]});
    let step = WorkflowStepRun {
        id: step_id,
        workflow_run_id: run.id,
        step_key: format!("provider-harness-{}", Uuid::new_v4()),
        step_type: "agent".to_string(),
        agent_id: Some(session.agent_id),
        agent_version_id: None,
        session_id: Some(session.id),
        thread_id: None,
        handoff_id: None,
        task_grant_id: Some(grant.id),
        environment_id: None,
        status: "running".to_string(),
        input_payload: empty_json_object(),
        output_payload: empty_json_object(),
        artifact_ids: Vec::new(),
        approval_ids: Vec::new(),
        tool_call_ids: Vec::new(),
        claimed_by_worker: None,
        claim_owner_version: 0,
        lease_expires_at: None,
        context_packet_id: None,
        started_at: Some(now),
        completed_at: None,
        scheduled_at: None,
        created_at: now,
        updated_at: now,
    };
    state
        .create_workflow_step_run_with_task_grant(step, grant)
        .await
        .expect("create provider harness step and TaskGrant")
        .1
}

async fn set_harness_task_grant_expiry(
    state: &AppState,
    grant_id: Uuid,
    expires_at: Option<DateTime<Utc>>,
) {
    match &state.store {
        StoreBackend::Memory(store) => {
            let mut store = store.write().await;
            let grant = store
                .task_grants
                .get_mut(&grant_id)
                .expect("provider harness TaskGrant");
            grant.expires_at = expires_at;
            grant.updated_at = Utc::now();
        }
        StoreBackend::Postgres(pool) => {
            sqlx::query(
                "UPDATE task_grants SET expires_at = $1, updated_at = now() WHERE tenant_id = $2 AND id = $3",
            )
            .bind(expires_at)
            .bind(state.tenant_id)
            .bind(grant_id)
            .execute(pool)
            .await
            .expect("update provider harness TaskGrant expiry");
        }
    }
}

async fn set_harness_workflow_run_status(state: &AppState, grant: &TaskGrant, status: &str) {
    match &state.store {
        StoreBackend::Memory(store) => {
            let mut store = store.write().await;
            let run = store
                .workflow_runs
                .get_mut(&grant.workflow_run_id)
                .expect("provider harness workflow run");
            run.status = status.to_string();
            run.updated_at = Utc::now();
        }
        StoreBackend::Postgres(pool) => {
            sqlx::query(
                "UPDATE workflow_runs SET status = $1, updated_at = now() WHERE tenant_id = $2 AND id = $3",
            )
            .bind(status)
            .bind(state.tenant_id)
            .bind(grant.workflow_run_id)
            .execute(pool)
            .await
            .expect("update provider harness workflow run status");
        }
    }
}

async fn set_harness_workflow_step_status(state: &AppState, grant: &TaskGrant, status: &str) {
    let step_id = grant
        .workflow_step_run_id
        .expect("provider harness workflow step");
    match &state.store {
        StoreBackend::Memory(store) => {
            let mut store = store.write().await;
            let step = store
                .workflow_step_runs
                .get_mut(&step_id)
                .expect("provider harness workflow step");
            step.status = status.to_string();
            step.updated_at = Utc::now();
        }
        StoreBackend::Postgres(pool) => {
            sqlx::query(
                "UPDATE workflow_step_runs SET status = $1, updated_at = now() WHERE tenant_id = $2 AND id = $3",
            )
            .bind(status)
            .bind(state.tenant_id)
            .bind(step_id)
            .execute(pool)
            .await
            .expect("update provider harness workflow step status");
        }
    }
}

async fn persisted_harness_ontology_release(state: &AppState) -> OntologyRelease {
    let now = Utc::now();
    state
        .create_ontology_release(OntologyRelease {
            id: Uuid::new_v4(),
            version: "v1".to_string(),
            domain_scope: format!("provider-harness-{}", Uuid::new_v4()),
            source_run_id: None,
            parent_release_id: None,
            rollback_target_release_id: None,
            status: ONTOLOGY_RELEASE_STATUS_ACTIVE.to_string(),
            release_class: "repo_controlled".to_string(),
            object_count: 0,
            relation_count: 0,
            action_count: 0,
            migration_policy: empty_json_object(),
            gate_result: json!({"status": "passed"}),
            materialized_object_ids: json!([]),
            materialized_link_ids: json!([]),
            evidence_refs: json!([]),
            promoted_by: Some("provider-harness-test".to_string()),
            promoted_at: Some(now),
            rolled_back_by: None,
            rolled_back_at: None,
            archived_at: None,
            created_at: now,
            updated_at: now,
        })
        .await
        .expect("create provider harness ontology release")
}

async fn set_harness_ontology_release_status(state: &AppState, release_id: Uuid, status: &str) {
    match &state.store {
        StoreBackend::Memory(store) => {
            let mut store = store.write().await;
            let release = store
                .ontology_releases
                .get_mut(&release_id)
                .expect("provider harness ontology release");
            release.status = status.to_string();
            release.updated_at = Utc::now();
        }
        StoreBackend::Postgres(pool) => {
            sqlx::query(
                "UPDATE ontology_releases SET status = $1, updated_at = now() WHERE tenant_id = $2 AND id = $3",
            )
            .bind(status)
            .bind(state.tenant_id)
            .bind(release_id)
            .execute(pool)
            .await
            .expect("update provider harness ontology release status");
        }
    }
}

fn pending_tool_approval(tool_call: &ToolCall, expires_at: Option<DateTime<Utc>>) -> Approval {
    Approval {
        id: Uuid::new_v4(),
        session_id: tool_call.session_id,
        tool_call_id: Some(tool_call.id),
        action: tool_call.tool_name.clone(),
        risk_level: tool_call.risk_level.clone(),
        reason: "test pending approval".to_string(),
        evidence: empty_json_object(),
        decision_payload: empty_json_object(),
        status: "pending".to_string(),
        expires_at,
        created_at: Utc::now(),
        decided_at: None,
    }
}

async fn assert_approved_ontology_proposal_commits_as_one_record_set(
    state: &AppState,
    session: &Session,
) {
    let grant = persisted_harness_task_grant(state, session).await;
    let release = persisted_harness_ontology_release(state).await;
    let mut waiting_call = waiting_ontology_action_call(session.id);
    waiting_call.task_grant_id = Some(grant.id);
    let tool_call = state
        .insert_tool_call(waiting_call)
        .await
        .expect("waiting ontology action");
    let approval = state
        .insert_approval(pending_tool_approval(&tool_call, None))
        .await
        .expect("pending ontology approval");
    state
        .decide_approval(approval.id, "approved")
        .await
        .expect("approve ontology action");
    let queued = state
        .execution_queue
        .enqueue(ExecutionJobRequest {
            session_id: session.id,
            environment_id: None,
            approval_id: approval.id,
            tool_call_id: tool_call.id,
            tool_name: tool_call.tool_name.clone(),
            max_attempts: None,
        })
        .await
        .expect("queue ontology action");
    let running = state
        .execution_queue
        .start(queued.id, "ontology-proposal-worker")
        .await
        .expect("claim ontology action");
    let executing = state
        .execution_queue
        .begin_executing_started(
            running.id,
            "ontology-proposal-worker",
            running.claim_generation,
        )
        .await
        .expect("begin ontology action commit");
    let artifact = Artifact {
        id: Uuid::new_v4(),
        session_id: session.id,
        artifact_type: "ontology_action_proposal".to_string(),
        name: "atomic-proposal.json".to_string(),
        path: None,
        content: json!({
            "status": "draft",
            "ontology_release_id": release.id,
        }),
        created_at: Utc::now(),
    };
    let result = json!({
        "status": "proposal_created",
        "approval": "approved",
        "artifact_id": artifact.id,
    });
    let proposal_details = json!({
        "artifact_id": artifact.id,
        "tool_call_id": tool_call.id,
    });
    set_harness_task_grant_expiry(
        state,
        grant.id,
        Some(Utc::now() - chrono::Duration::seconds(1)),
    )
    .await;
    let error = state
        .commit_approved_ontology_action_proposal(
            &executing,
            approval.id,
            artifact.clone(),
            proposal_details.clone(),
            result.clone(),
        )
        .await
        .expect_err("expired TaskGrant must block proposal commit");
    assert!(error.execution_outcome_known);
    assert!(!error.execution_retry_safe);
    assert!(
        state
            .list_artifacts(session.id)
            .await
            .expect("artifacts after denied proposal")
            .iter()
            .all(|stored| stored.id != artifact.id)
    );
    assert_eq!(
        state
            .get_tool_call(tool_call.id)
            .await
            .expect("waiting ontology action after denied proposal")
            .status,
        "waiting_approval"
    );
    set_harness_task_grant_expiry(state, grant.id, None).await;
    set_harness_ontology_release_status(state, release.id, "rolled_back").await;
    let error = state
        .commit_approved_ontology_action_proposal(
            &executing,
            approval.id,
            artifact.clone(),
            proposal_details.clone(),
            result.clone(),
        )
        .await
        .expect_err("revoked ontology release must block proposal commit");
    assert!(error.execution_outcome_known);
    assert!(!error.execution_retry_safe);
    assert!(error.message.contains("revoked"));
    set_harness_ontology_release_status(state, release.id, ONTOLOGY_RELEASE_STATUS_ACTIVE).await;
    set_harness_workflow_run_status(state, &grant, "canceled").await;
    let error = state
        .commit_approved_ontology_action_proposal(
            &executing,
            approval.id,
            artifact.clone(),
            proposal_details.clone(),
            result.clone(),
        )
        .await
        .expect_err("terminal workflow run must block proposal commit");
    assert!(error.execution_outcome_known);
    assert!(error.message.contains("workflow run is not active"));
    set_harness_workflow_run_status(state, &grant, "running").await;
    set_harness_workflow_step_status(state, &grant, "canceled").await;
    let error = state
        .commit_approved_ontology_action_proposal(
            &executing,
            approval.id,
            artifact.clone(),
            proposal_details.clone(),
            result.clone(),
        )
        .await
        .expect_err("terminal workflow step must block proposal commit");
    assert!(error.execution_outcome_known);
    assert!(error.message.contains("workflow step run is terminal"));
    set_harness_workflow_step_status(state, &grant, "running").await;
    assert!(
        state
            .list_artifacts(session.id)
            .await
            .expect("artifacts after terminal workflow denials")
            .iter()
            .all(|stored| stored.id != artifact.id)
    );
    assert_eq!(
        state
            .get_tool_call(tool_call.id)
            .await
            .expect("waiting ontology action after terminal workflow denials")
            .status,
        "waiting_approval"
    );
    if let StoreBackend::Postgres(pool) = &state.store {
        let mut invocation_lock = pool.begin().await.expect("begin invocation lock");
        sqlx::query(
            "SELECT pg_advisory_xact_lock(hashtextextended($1::uuid::text || ':' || $2::uuid::text, 0))",
        )
        .bind(state.tenant_id)
        .bind(session.id)
        .execute(&mut *invocation_lock)
        .await
        .expect("lock session in invocation order");
        let commit_state = state.clone();
        let commit_job = executing.clone();
        let commit_artifact = artifact.clone();
        let commit_details = proposal_details.clone();
        let commit_result = result.clone();
        let commit = tokio::spawn(async move {
            commit_state
                .commit_approved_ontology_action_proposal(
                    &commit_job,
                    approval.id,
                    commit_artifact,
                    commit_details,
                    commit_result,
                )
                .await
        });
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM task_grants WHERE tenant_id = $1 AND id = $2 FOR UPDATE",
        )
        .bind(state.tenant_id)
        .bind(grant.id)
        .fetch_one(&mut *invocation_lock)
        .await
        .expect("session-first invocation lock order must not deadlock with proposal commit");
        invocation_lock
            .commit()
            .await
            .expect("release invocation locks");
        tokio::time::timeout(std::time::Duration::from_secs(10), commit)
            .await
            .expect("proposal commit must not hang behind invocation locks")
            .expect("proposal commit task")
            .expect("commit approved ontology proposal");
    } else {
        state
            .commit_approved_ontology_action_proposal(
                &executing,
                approval.id,
                artifact.clone(),
                proposal_details.clone(),
                result.clone(),
            )
            .await
            .expect("commit approved ontology proposal");
    }

    let completed = state
        .get_tool_call(tool_call.id)
        .await
        .expect("completed ontology action");
    assert_eq!(completed.status, "completed");
    assert_eq!(completed.result, Some(result.clone()));
    assert!(
        state
            .list_artifacts(session.id)
            .await
            .expect("ontology proposal artifacts")
            .iter()
            .any(|stored| stored.id == artifact.id)
    );
    let events = state
        .list_events(session.id)
        .await
        .expect("proposal events");
    assert!(events.iter().any(|event| {
        event.event_type == "ontology_action.proposal_created"
            && event.payload["artifact_id"] == json!(artifact.id)
    }));
    assert!(events.iter().any(|event| {
        event.event_type == "tool.result"
            && event.payload["execution_job_id"] == json!(executing.id)
            && event.payload["content"]["approval"] == json!("approved")
    }));
    let audits = state
        .list_audit_logs(Some(session.id))
        .await
        .expect("proposal audits");
    assert!(audits.iter().any(|audit| {
        audit.action == "tool.completed" && audit.details["approval_id"] == json!(approval.id)
    }));

    state
        .execution_queue
        .mark_outcome_unknown_started(
            executing.id,
            "ontology-proposal-worker",
            executing.claim_generation,
            "simulate lost claim before retry",
        )
        .await
        .expect("move claim out of executing state");
    let error = state
        .commit_approved_ontology_action_proposal(
            &executing,
            approval.id,
            artifact,
            proposal_details,
            result,
        )
        .await
        .expect_err("lost pre-commit claim must not be ambiguous");
    assert!(error.execution_retry_safe);
    assert!(!error.execution_outcome_known);
}

#[tokio::test]
async fn approved_ontology_proposal_commits_as_one_memory_record_set() {
    let (state, session) = harness_test_session().await;
    assert_approved_ontology_proposal_commits_as_one_record_set(&state, &session).await;
}

#[tokio::test]
#[ignore = "requires MANDOFORGE_TEST_POSTGRES_URL"]
async fn approved_ontology_proposal_commits_as_one_postgres_record_set() {
    let (state, session) = postgres_harness_test_session().await;
    assert_approved_ontology_proposal_commits_as_one_record_set(&state, &session).await;
}

async fn record_legacy_expiry_evidence(state: &AppState, approval: &Approval) {
    let expired = state
        .decide_approval(approval.id, "expired")
        .await
        .expect("simulate legacy approval expiry");
    state
        .append_event(
            "system",
            Some(expired.id),
            expired.session_id,
            "approval.expired",
            json!({
                "approval_id": expired.id,
                "decision": "expired",
                "expires_at": expired.expires_at,
            }),
        )
        .await
        .expect("legacy approval expiry event");
    state
        .append_audit_log(new_audit_log(
            Some(expired.session_id),
            "system",
            Some(expired.id),
            "approval.expired",
            "approval",
            Some(expired.id),
            json!({
                "tool_call_id": expired.tool_call_id,
                "decision": "expired",
                "expires_at": expired.expires_at,
            }),
        ))
        .await
        .expect("legacy approval expiry audit");
}

#[tokio::test]
async fn provider_harness_records_one_authoritative_success_response() {
    let (state, session) = harness_test_session().await;
    let provider = HarnessTestProvider { fail: false };

    run_provider_harness(&state, session.id, &provider, "harness-test", None, None)
        .await
        .expect("provider response");

    let events = state.list_events(session.id).await.expect("events");
    assert_eq!(
        events
            .iter()
            .filter(|event| event.event_type == "llm.request")
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| event.event_type == "llm.response")
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| event.event_type == "span.model_request_start")
            .count(),
        1
    );
    let end = events
        .iter()
        .find(|event| event.event_type == "span.model_request_end")
        .expect("completed span end");
    assert_eq!(end.payload["status"], json!("completed"));
    assert!(!events.iter().any(|event| event.event_type == "llm.error"));
    assert!(
        state
            .list_audit_logs(Some(session.id))
            .await
            .expect("audit logs")
            .iter()
            .all(|audit| audit.action != "provider.request_failed")
    );
}

#[tokio::test]
async fn provider_harness_failure_is_audited_and_cannot_execute_tools() {
    let (state, session) = harness_test_session().await;
    let provider = HarnessTestProvider { fail: true };

    let error = run_provider_harness(&state, session.id, &provider, "harness-test", None, None)
        .await
        .expect_err("provider failure");
    assert_eq!(error.message, "provider harness test failure");

    let events = state.list_events(session.id).await.expect("events");
    assert_eq!(
        events
            .iter()
            .filter(|event| event.event_type == "llm.request")
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| event.event_type == "llm.error")
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| event.event_type == "llm.response")
            .count(),
        0
    );
    let end = events
        .iter()
        .find(|event| event.event_type == "span.model_request_end")
        .expect("failed span end");
    assert_eq!(end.payload["status"], json!("failed"));
    let audits = state
        .list_audit_logs(Some(session.id))
        .await
        .expect("audit logs");
    assert_eq!(
        audits
            .iter()
            .filter(|audit| audit.action == "provider.request_failed")
            .count(),
        1
    );
    assert!(
        state
            .list_tool_calls(Some(session.id))
            .await
            .expect("tool calls")
            .is_empty()
    );
}

#[test]
fn provider_tool_names_require_agent_and_task_grant_for_mcp() {
    let agent_version = AgentVersion {
        id: Uuid::new_v4(),
        agent_id: Uuid::new_v4(),
        version: 1,
        provider: "mock".to_string(),
        model: "test".to_string(),
        system_prompt: String::new(),
        tools: vec![
            "file.read".to_string(),
            "mcp.call".to_string(),
            "native.connector.call".to_string(),
            "custom.unknown".to_string(),
        ],
        tool_names: Vec::new(),
        runtime_config: json!({}),
        approval_policy: json!({}),
        runtime_profile_id: None,
        runtime_profile_snapshot: json!({}),
        mcp_server_ids: Vec::new(),
        skill_ids: Vec::new(),
        workflow_pack_ids: Vec::new(),
        remote_computer_profile: json!({}),
        semantic_scopes: json!({}),
        created_at: Utc::now(),
    };
    let grant = TaskGrant {
        id: Uuid::new_v4(),
        workflow_run_id: Uuid::new_v4(),
        workflow_step_run_id: None,
        session_id: None,
        parent_grant_id: None,
        source_event_id: None,
        source_handoff_id: None,
        issuer_subject: "test".to_string(),
        grantee_agent_id: Some(agent_version.agent_id),
        grantee_session_id: None,
        agent_class: None,
        objective: "test".to_string(),
        risk_level: "low".to_string(),
        status: "active".to_string(),
        expires_at: None,
        max_turns: None,
        max_tool_calls: None,
        max_runtime_seconds: None,
        max_cost_usd_micros: None,
        turns_used: 0,
        tool_calls_used: 0,
        cost_usd_micros_used: 0,
        semantic_scopes: json!({}),
        memory_scope: json!({}),
        tool_scope: json!({"read": ["file.read", "mcp.call", "native.connector.call", "custom.unknown"]}),
        connector_scope: json!({}),
        approval_policy: json!({}),
        external_effects: json!({}),
        context_packet_id: None,
        policy_revision_id: None,
        immutable_args_hash: None,
        audit_trace_id: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let with_grant = provider_tool_names_for_grant_and_agent_version(Some(&grant), &agent_version);
    assert!(with_grant.iter().any(|tool| tool == "file.read"));
    assert!(with_grant.iter().any(|tool| tool == "mcp.call"));
    assert!(
        !with_grant
            .iter()
            .any(|tool| tool == "native.connector.call")
    );
    assert!(!with_grant.iter().any(|tool| tool == "custom.unknown"));

    let without_grant = provider_tool_names_for_grant_and_agent_version(None, &agent_version);
    assert!(without_grant.iter().any(|tool| tool == "file.read"));
    assert!(without_grant.iter().any(|tool| tool == "complete_task"));
    assert!(!without_grant.iter().any(|tool| tool == "mcp.call"));
    assert!(
        !without_grant
            .iter()
            .any(|tool| tool == "native.connector.call")
    );
}

#[tokio::test]
async fn complete_task_is_explicit_validated_and_terminal() {
    assert!(
        provider_completion_request(&[ProviderToolCall {
            tool_name: "complete_task".to_string(),
            args: json!({"status": "completed", "summary": "objective satisfied"}),
        }])
        .expect("valid completion")
        .is_some()
    );
    assert!(
        provider_completion_request(&[
            ProviderToolCall {
                tool_name: "complete_task".to_string(),
                args: json!({"status": "completed", "summary": "too early"}),
            },
            ProviderToolCall {
                tool_name: "file.read".to_string(),
                args: json!({"paths": ["README.md"]}),
            },
        ])
        .is_err()
    );

    let (state, session) = harness_test_session().await;
    let completed =
        apply_provider_completion(&state, session.id, None, "completed", "objective satisfied")
            .await
            .expect("complete session");
    assert!(matches!(completed.status, SessionStatus::Terminated));
    let events = state.list_events(session.id).await.expect("events");
    assert!(
        events
            .iter()
            .any(|event| event.event_type == "session.goal.completed")
    );
    assert!(events.iter().any(|event| {
        event.event_type == "tool.result" && event.payload["tool"] == "complete_task"
    }));
    assert!(
        events
            .iter()
            .any(|event| event.event_type == "session.status_terminated")
    );
    let tool_calls = state
        .list_tool_calls(Some(session.id))
        .await
        .expect("tool calls");
    let completion_call = tool_calls
        .iter()
        .find(|call| call.tool_name == "complete_task")
        .expect("durable completion tool call");
    assert_eq!(completion_call.status, "completed");
    assert_eq!(
        completion_call.result.as_ref(),
        Some(&json!({
            "status": "completed",
            "summary": "objective satisfied",
        }))
    );
    assert!(
        state
            .list_audit_logs(Some(session.id))
            .await
            .expect("audit logs")
            .iter()
            .any(|audit| audit.action == "session.goal.completed")
    );

    let event_count = events.len();
    let error = execute_tool_invocation(
        &state,
        "file.read",
        ExecuteTool {
            session_id: session.id,
            task_grant_id: None,
            args: json!({"paths": ["README.md"]}),
        },
        ToolInvocationOrigin::ManualRoute,
    )
    .await
    .expect_err("terminal session must reject new tools");
    assert!(error.message.contains("terminal session"));
    assert_eq!(
        state.list_events(session.id).await.expect("events").len(),
        event_count
    );
}

#[tokio::test]
async fn complete_task_defers_while_durable_actions_are_unresolved() {
    let (state, session) = harness_test_session().await;
    let tool_call = waiting_ontology_action_call(session.id);
    let tool_call = state
        .insert_tool_call(tool_call)
        .await
        .expect("waiting tool call");
    let approval = state
        .insert_approval(pending_tool_approval(&tool_call, None))
        .await
        .expect("pending approval");

    let deferred = apply_provider_completion(&state, session.id, None, "completed", "too early")
        .await
        .expect("pending approval must defer completion without failing the session");
    assert!(matches!(deferred.status, SessionStatus::RequiresAction));

    state
        .decide_approval(approval.id, "rejected")
        .await
        .expect("resolve approval only");
    let deferred =
        apply_provider_completion(&state, session.id, None, "completed", "still too early")
            .await
            .expect("waiting tool call must independently defer completion");
    assert!(matches!(deferred.status, SessionStatus::RequiresAction));
    assert!(!matches!(
        state.get_session(session.id).await.expect("session").status,
        SessionStatus::Terminated
    ));
    assert!(
        state
            .list_tool_calls(Some(session.id))
            .await
            .expect("tool calls")
            .iter()
            .all(|call| call.tool_name != "complete_task")
    );
    assert!(
        state
            .list_events(session.id)
            .await
            .expect("events")
            .iter()
            .all(|event| event.payload["tool"] != "complete_task")
    );
}

#[tokio::test]
async fn expired_approval_resolves_waiting_tool_before_completion() {
    let (state, session) = harness_test_session().await;
    let tool_call = state
        .insert_tool_call(waiting_ontology_action_call(session.id))
        .await
        .expect("waiting tool call");
    let approval = state
        .insert_approval(pending_tool_approval(
            &tool_call,
            Some(Utc::now() - ChronoDuration::seconds(1)),
        ))
        .await
        .expect("expired pending approval");

    expire_approval_record(&state, approval.id)
        .await
        .expect("expire approval");
    assert_eq!(
        state
            .get_tool_call(tool_call.id)
            .await
            .expect("resolved tool call")
            .status,
        "denied"
    );
    assert_eq!(
        build_harness_context(&state, session.id, None, None)
            .await
            .expect("expired approval context")
            .rejected_tool_result_count,
        1
    );
    let completed = apply_provider_completion(
        &state,
        session.id,
        None,
        "completed",
        "expired work resolved",
    )
    .await
    .expect("resolved expiration must not wedge completion");
    assert!(matches!(completed.status, SessionStatus::Terminated));
}

#[tokio::test]
async fn due_run_repairs_legacy_expiry_without_duplicate_evidence() {
    let (state, session) = harness_test_session().await;
    let tool_call = state
        .insert_tool_call(waiting_ontology_action_call(session.id))
        .await
        .expect("waiting tool call");
    let approval = state
        .insert_approval(pending_tool_approval(
            &tool_call,
            Some(Utc::now() - ChronoDuration::seconds(1)),
        ))
        .await
        .expect("expired pending approval");
    record_legacy_expiry_evidence(&state, &approval).await;
    assert_eq!(
        state
            .get_tool_call(tool_call.id)
            .await
            .expect("waiting tool call")
            .status,
        "waiting_approval"
    );

    let first_run = execute_due_approval_escalations(&state)
        .await
        .expect("scheduled legacy expiry repair");
    assert_eq!(first_run.expired_count, 1);
    let second_run = execute_due_approval_escalations(&state)
        .await
        .expect("idempotent scheduled expiry repair");
    assert_eq!(second_run.expired_count, 0);
    assert_eq!(
        state
            .get_tool_call(tool_call.id)
            .await
            .expect("resolved tool call")
            .status,
        "denied"
    );
    let events = state.list_events(session.id).await.expect("events");
    assert_eq!(
        events
            .iter()
            .filter(|event| event.event_type == "approval.expired")
            .count(),
        1
    );
    assert_eq!(
        state
            .list_audit_logs(Some(session.id))
            .await
            .expect("audits")
            .iter()
            .filter(|audit| audit.action == "approval.expired")
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| {
                event.event_type == "tool.result" && event.actor_id == Some(tool_call.id)
            })
            .count(),
        1
    );
}

#[tokio::test]
async fn completion_and_tool_invocation_start_are_session_atomic() {
    let (state, session) = harness_test_session().await;
    let agent_version = state
        .agent_version_for_session(session.id)
        .await
        .expect("agent version");
    let grant = persisted_harness_task_grant(&state, &session).await;
    let mut running_call = waiting_ontology_action_call(session.id);
    running_call.event_id = Some(Uuid::new_v4());
    running_call.tool_name = "file.read".to_string();
    running_call.task_grant_id = Some(grant.id);
    running_call.status = "running".to_string();
    running_call.policy_decision = json!({"decision": "allowed"});
    running_call.started_at = Some(Utc::now());
    let tool_call_id = running_call.id;
    let call_event_id = running_call.event_id.expect("call event id");
    let completion_state = state.clone();
    let insertion_state = state.clone();
    let (completion, insertion) = tokio::join!(
        apply_provider_completion(
            &completion_state,
            session.id,
            None,
            "completed",
            "atomic completion"
        ),
        insertion_state.commit_tool_invocation_start(
            running_call,
            agent_version.id,
            agent_version.version,
        ),
    );

    let completion = completion.expect("completion must terminate or defer");
    let reserved = state
        .get_task_grant(grant.id)
        .await
        .expect("task grant after race");
    let events = state.list_events(session.id).await.expect("events");
    if insertion.is_ok() {
        assert!(matches!(completion.status, SessionStatus::RequiresAction));
        assert_eq!(reserved.tool_calls_used, 1);
        assert!(
            events
                .iter()
                .any(|event| { event.event_type == "tool.call" && event.id == call_event_id })
        );
        assert!(events.iter().any(|event| {
            event.event_type == "task_grant.checked" && event.actor_id == Some(grant.id)
        }));
    } else {
        assert!(matches!(completion.status, SessionStatus::Terminated));
        assert_eq!(reserved.tool_calls_used, 0);
        assert!(events.iter().all(|event| {
            event.id != call_event_id
                && !(event.event_type == "task_grant.checked" && event.actor_id == Some(grant.id))
        }));
        assert!(state.get_tool_call(tool_call_id).await.is_err());
    }
    let session = state.get_session(session.id).await.expect("session");
    let unresolved = state
        .list_tool_calls(Some(session.id))
        .await
        .expect("tool calls")
        .iter()
        .any(|call| matches!(call.status.as_str(), "running" | "waiting_approval"));
    assert!(!matches!(session.status, SessionStatus::Terminated) || !unresolved);
}

async fn assert_terminal_workflow_blocks_tool_invocation_start(
    state: &AppState,
    session: &Session,
) {
    let agent_version = state
        .agent_version_for_session(session.id)
        .await
        .expect("agent version");
    let grant = persisted_harness_task_grant(state, session).await;
    let candidate = || {
        let mut tool_call = waiting_ontology_action_call(session.id);
        tool_call.event_id = Some(Uuid::new_v4());
        tool_call.task_grant_id = Some(grant.id);
        tool_call.status = "running".to_string();
        tool_call.policy_decision = json!({"decision": "allowed"});
        tool_call.started_at = Some(Utc::now());
        tool_call
    };

    set_harness_workflow_run_status(state, &grant, "canceled").await;
    let run_denied = candidate();
    let error = state
        .commit_tool_invocation_start(run_denied.clone(), agent_version.id, agent_version.version)
        .await
        .expect_err("terminal workflow run must block tool invocation commit");
    assert!(error.message.contains("workflow run is not active"));
    set_harness_workflow_run_status(state, &grant, "running").await;

    set_harness_workflow_step_status(state, &grant, "canceled").await;
    let step_denied = candidate();
    let error = state
        .commit_tool_invocation_start(step_denied.clone(), agent_version.id, agent_version.version)
        .await
        .expect_err("terminal workflow step must block tool invocation commit");
    assert!(error.message.contains("workflow step run is terminal"));
    set_harness_workflow_step_status(state, &grant, "running").await;

    assert_eq!(
        state
            .get_task_grant(grant.id)
            .await
            .expect("TaskGrant after denied invocation commits")
            .tool_calls_used,
        0
    );
    assert!(state.get_tool_call(run_denied.id).await.is_err());
    assert!(state.get_tool_call(step_denied.id).await.is_err());
    let events = state.list_events(session.id).await.expect("session events");
    assert!(events.iter().all(|event| {
        event.id != run_denied.event_id.expect("run denial event id")
            && event.id != step_denied.event_id.expect("step denial event id")
    }));
}

#[tokio::test]
async fn terminal_workflow_blocks_tool_invocation_start_in_memory() {
    let (state, session) = harness_test_session().await;
    assert_terminal_workflow_blocks_tool_invocation_start(&state, &session).await;
}

#[tokio::test]
#[ignore = "requires MANDOFORGE_TEST_POSTGRES_URL"]
async fn terminal_workflow_blocks_tool_invocation_start_in_postgres() {
    let (state, session) = postgres_harness_test_session().await;
    assert_terminal_workflow_blocks_tool_invocation_start(&state, &session).await;
}

#[tokio::test]
async fn deferred_context_refresh_survives_consumed_user_message() {
    let (state, session) = harness_test_session().await;
    let grant = persisted_harness_task_grant(&state, &session).await;
    state
        .update_workflow_run_root_task_grant(grant.workflow_run_id, grant.id)
        .await
        .expect("bind root TaskGrant");
    let original = generate_and_persist_context_packet(&state, session.id)
        .await
        .expect("original context packet");
    state
        .update_task_grant_context_packet(grant.id, original.id)
        .await
        .expect("bind original context packet");
    let message = state
        .append_event(
            "user",
            None,
            session.id,
            "user.message",
            json!({"message": "refresh after current work resolves"}),
        )
        .await
        .expect("user message");

    let blocked = build_harness_context(&state, session.id, Some(message.seq), Some(message.seq))
        .await
        .expect("blocked context refresh");
    assert_eq!(blocked.context_packet_id, Some(original.id));
    assert!(
        state
            .list_events(session.id)
            .await
            .expect("events")
            .iter()
            .any(|event| {
                event.event_type == CONTEXT_PACKET_REFRESH_DEFERRED_EVENT
                    && event.payload["blockers"] == json!(["workflow_step_running"])
            })
    );

    set_harness_workflow_step_status(&state, &grant, "queued").await;
    let refreshed = build_harness_context(&state, session.id, None, None)
        .await
        .expect("deferred context refresh");
    let refreshed_id = refreshed.context_packet_id.expect("refreshed packet id");
    assert_ne!(refreshed_id, original.id);
    let reused = build_harness_context(&state, session.id, None, None)
        .await
        .expect("reuse completed refresh");
    assert_eq!(reused.context_packet_id, Some(refreshed_id));

    let events = state.list_events(session.id).await.expect("refresh events");
    assert_eq!(
        events
            .iter()
            .filter(|event| event.event_type == CONTEXT_PACKET_REFRESH_DEFERRED_EVENT)
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| event.event_type == CONTEXT_PACKET_REFRESH_COMPLETED_EVENT)
            .count(),
        1
    );
    assert!(events.iter().any(|event| {
        event.event_type == CONTEXT_PACKET_REFRESH_COMPLETED_EVENT
            && event.payload["context_packet_id"] == json!(refreshed_id)
    }));
}

async fn assert_tool_result_publishes_after_terminal_status(state: &AppState, session: &Session) {
    let mut running_call = waiting_ontology_action_call(session.id);
    running_call.status = "running".to_string();
    running_call.started_at = Some(Utc::now());
    let tool_call = state
        .insert_tool_call(running_call)
        .await
        .expect("running tool call");
    let mut changes = crate::store_events::subscribe_session_events(state)
        .await
        .expect("subscribe to tool result");
    let tool_call_id = tool_call.id;
    let commit_state = state.clone();
    let commit = tokio::spawn(async move {
        commit_state
            .commit_tool_invocation_result(
                tool_call_id,
                "completed",
                json!({"status": "ok"}),
                "manual",
            )
            .await
    });

    assert!(
        tokio::time::timeout(
            std::time::Duration::from_secs(20),
            changes.wait_for_session_change(session.id),
        )
        .await
        .expect("tool result notification")
        .expect("tool result change")
    );
    assert_eq!(
        state
            .get_tool_call(tool_call_id)
            .await
            .expect("tool call visible with result")
            .status,
        "completed"
    );
    commit
        .await
        .expect("tool result commit task")
        .expect("tool result commit");
}

async fn assert_decline_publishes_after_resolved_state(state: &AppState, session: &Session) {
    let tool_call = state
        .insert_tool_call(waiting_ontology_action_call(session.id))
        .await
        .expect("waiting tool call");
    let approval = state
        .insert_approval(pending_tool_approval(&tool_call, None))
        .await
        .expect("pending approval");
    let mut changes = crate::store_events::subscribe_session_events(state)
        .await
        .expect("subscribe to approval decline");
    let approval_id = approval.id;
    let commit_state = state.clone();
    let commit = tokio::spawn(async move {
        commit_state
            .decline_approval_and_tool_call(approval_id, "rejected", "user")
            .await
    });

    assert!(
        tokio::time::timeout(
            std::time::Duration::from_secs(20),
            changes.wait_for_session_change(session.id),
        )
        .await
        .expect("approval decline notification")
        .expect("approval decline change")
    );
    assert_eq!(
        state
            .get_approval(approval_id)
            .await
            .expect("declined approval visible with evidence")
            .status,
        "rejected"
    );
    assert_eq!(
        state
            .get_tool_call(tool_call.id)
            .await
            .expect("denied tool visible with evidence")
            .status,
        "denied"
    );
    commit
        .await
        .expect("approval decline commit task")
        .expect("approval decline commit");
}

async fn assert_concurrent_approval_decision_has_one_winner(state: &AppState, session: &Session) {
    let tool_call = state
        .insert_tool_call(waiting_ontology_action_call(session.id))
        .await
        .expect("waiting tool call");
    let approval = state
        .insert_approval(pending_tool_approval(&tool_call, None))
        .await
        .expect("pending approval");
    let approve_state = state.clone();
    let reject_state = state.clone();
    let (approved, rejected) = tokio::join!(
        approve_state.decide_approval(approval.id, "approved"),
        reject_state.decline_approval_and_tool_call(approval.id, "rejected", "user"),
    );

    assert_ne!(approved.is_ok(), rejected.is_ok());
    let final_approval = state
        .get_approval(approval.id)
        .await
        .expect("final approval");
    let final_tool_call = state
        .get_tool_call(tool_call.id)
        .await
        .expect("final tool call");
    let has_rejection_evidence = state
        .list_events(session.id)
        .await
        .expect("decision events")
        .iter()
        .any(|event| {
            event.event_type == "approval.rejected" && event.actor_id == Some(approval.id)
        });
    if approved.is_ok() {
        assert_eq!(final_approval.status, "approved");
        assert_eq!(final_tool_call.status, "waiting_approval");
        assert!(!has_rejection_evidence);
    } else {
        assert_eq!(final_approval.status, "rejected");
        assert_eq!(final_tool_call.status, "denied");
        assert!(has_rejection_evidence);
    }
}

async fn assert_approval_modification_is_atomic_with_decision(state: &AppState, session: &Session) {
    let mut tool_call = waiting_ontology_action_call(session.id);
    tool_call.tool_name = "file.write".to_string();
    tool_call.args = json!({"path": "before.md", "content": "before"});
    let tool_call = state
        .insert_tool_call(tool_call)
        .await
        .expect("waiting tool call");
    let approval = state
        .insert_approval(pending_tool_approval(&tool_call, None))
        .await
        .expect("pending approval");
    let modified_args = json!({"path": "after.md", "content": "after"});
    let modify_state = state.clone();
    let approve_state = state.clone();
    let (modified, approved) = tokio::join!(
        modify_state.modify_approval(
            approval.id,
            modified_args.clone(),
            Some("authorized edit".to_string()),
        ),
        approve_state.decide_approval(approval.id, "approved"),
    );

    assert!(approved.is_ok());
    let final_approval = state
        .get_approval(approval.id)
        .await
        .expect("final approval");
    let final_tool_call = state
        .get_tool_call(tool_call.id)
        .await
        .expect("final tool call");
    assert_eq!(final_approval.status, "approved");
    if modified.is_ok() {
        assert_eq!(final_tool_call.args, modified_args);
        assert_eq!(
            final_approval.decision_payload["modified_args"],
            final_tool_call.args
        );
    } else {
        assert_eq!(final_tool_call.args, tool_call.args);
        assert_eq!(final_approval.decision_payload, empty_json_object());
    }
}

async fn assert_expiry_losing_a_decision_race_is_a_noop(state: &AppState, session: &Session) {
    let tool_call = state
        .insert_tool_call(waiting_ontology_action_call(session.id))
        .await
        .expect("waiting tool call");
    let approval = state
        .insert_approval(pending_tool_approval(&tool_call, None))
        .await
        .expect("pending approval");
    state
        .decide_approval(approval.id, "approved")
        .await
        .expect("approve before stale expiry attempt");

    let (unchanged, unchanged_tool, events) = state
        .decline_approval_and_tool_call(approval.id, "expired", "system")
        .await
        .expect("stale expiry must not abort a batch");
    assert_eq!(unchanged.status, "approved");
    assert!(unchanged_tool.is_none());
    assert!(events.is_empty());
    assert_eq!(
        state
            .get_tool_call(tool_call.id)
            .await
            .expect("unchanged approved tool")
            .status,
        "waiting_approval"
    );
}

#[tokio::test]
async fn atomic_runtime_transitions_publish_only_after_commit() {
    let (state, session) = harness_test_session().await;
    assert_tool_result_publishes_after_terminal_status(&state, &session).await;
    assert_decline_publishes_after_resolved_state(&state, &session).await;
    assert_concurrent_approval_decision_has_one_winner(&state, &session).await;
    assert_approval_modification_is_atomic_with_decision(&state, &session).await;
    assert_expiry_losing_a_decision_race_is_a_noop(&state, &session).await;
}

#[tokio::test]
async fn idempotent_decline_replay_does_not_publish_again() {
    let (state, session) = harness_test_session().await;
    let tool_call = state
        .insert_tool_call(waiting_ontology_action_call(session.id))
        .await
        .expect("waiting tool call");
    let approval = state
        .insert_approval(pending_tool_approval(&tool_call, None))
        .await
        .expect("pending approval");
    state
        .decline_approval_and_tool_call(approval.id, "rejected", "user")
        .await
        .expect("initial approval decline");

    let mut changes = crate::store_events::subscribe_session_events(&state)
        .await
        .expect("subscribe after initial decline");
    state
        .decline_approval_and_tool_call(approval.id, "rejected", "user")
        .await
        .expect("idempotent approval decline replay");

    assert!(
        tokio::time::timeout(
            std::time::Duration::from_millis(50),
            changes.wait_for_session_change(session.id),
        )
        .await
        .is_err(),
        "idempotent replay must not publish duplicate session changes"
    );
}

#[tokio::test]
#[ignore = "requires MANDOFORGE_TEST_POSTGRES_URL"]
async fn postgres_completion_and_tool_invocation_start_are_session_atomic() {
    let (state, session) = postgres_harness_test_session().await;
    let agent_version = state
        .agent_version_for_session(session.id)
        .await
        .expect("agent version");
    let mut running_call = waiting_ontology_action_call(session.id);
    running_call.event_id = Some(Uuid::new_v4());
    running_call.tool_name = "file.read".to_string();
    running_call.status = "running".to_string();
    running_call.policy_decision = json!({"decision": "allowed"});
    running_call.started_at = Some(Utc::now());
    let completion_state = state.clone();
    let insertion_state = state.clone();
    let (completion, insertion) = tokio::join!(
        apply_provider_completion(
            &completion_state,
            session.id,
            None,
            "completed",
            "atomic postgres completion"
        ),
        insertion_state.commit_tool_invocation_start(
            running_call,
            agent_version.id,
            agent_version.version,
        ),
    );

    let completion = completion.expect("completion must terminate or defer");
    if insertion.is_ok() {
        assert!(matches!(completion.status, SessionStatus::RequiresAction));
    } else {
        assert!(matches!(completion.status, SessionStatus::Terminated));
    }
    if let Ok((inserted, _)) = insertion {
        state
            .update_tool_call_status(inserted.id, "denied", Some(json!({"resolved": true})), None)
            .await
            .expect("resolve inserted tool call");
        apply_provider_completion(
            &state,
            session.id,
            None,
            "completed",
            "completed after resolving race",
        )
        .await
        .expect("complete after resolving inserted tool");
    }
    let session = state.get_session(session.id).await.expect("session");
    let unresolved = state
        .list_tool_calls(Some(session.id))
        .await
        .expect("tool calls")
        .iter()
        .any(|call| matches!(call.status.as_str(), "running" | "waiting_approval"));
    assert!(!matches!(session.status, SessionStatus::Terminated) || !unresolved);
    assert!(matches!(session.status, SessionStatus::Terminated));
    assert!(
        state
            .list_events(session.id)
            .await
            .expect("events")
            .iter()
            .any(|event| event.event_type == "session.status_terminated")
    );
    assert!(
        state
            .list_audit_logs(Some(session.id))
            .await
            .expect("audit logs")
            .iter()
            .any(|audit| audit.action == "session.goal.completed")
    );
}

#[tokio::test]
#[ignore = "requires MANDOFORGE_TEST_POSTGRES_URL"]
async fn postgres_runtime_transitions_publish_only_after_commit() {
    let (state, session) = postgres_harness_test_session().await;
    assert_tool_result_publishes_after_terminal_status(&state, &session).await;
    assert_decline_publishes_after_resolved_state(&state, &session).await;
    assert_concurrent_approval_decision_has_one_winner(&state, &session).await;
    assert_approval_modification_is_atomic_with_decision(&state, &session).await;
    assert_expiry_losing_a_decision_race_is_a_noop(&state, &session).await;
}

#[tokio::test]
#[ignore = "requires MANDOFORGE_TEST_POSTGRES_URL"]
async fn postgres_due_run_repairs_legacy_expiry_without_duplicate_evidence() {
    let (state, session) = postgres_harness_test_session().await;
    let tool_call = state
        .insert_tool_call(waiting_ontology_action_call(session.id))
        .await
        .expect("waiting tool call");
    let approval = state
        .insert_approval(pending_tool_approval(
            &tool_call,
            Some(Utc::now() - ChronoDuration::seconds(1)),
        ))
        .await
        .expect("expired pending approval");
    record_legacy_expiry_evidence(&state, &approval).await;

    let first_run = execute_due_approval_escalations(&state)
        .await
        .expect("scheduled legacy expiry repair");
    assert_eq!(first_run.expired_count, 1);
    let second_run = execute_due_approval_escalations(&state)
        .await
        .expect("idempotent scheduled expiry repair");
    assert_eq!(second_run.expired_count, 0);
    assert_eq!(
        state
            .get_tool_call(tool_call.id)
            .await
            .expect("resolved tool call")
            .status,
        "denied"
    );
    assert_eq!(
        state
            .list_events(session.id)
            .await
            .expect("events")
            .iter()
            .filter(|event| event.event_type == "approval.expired")
            .count(),
        1
    );
    assert_eq!(
        state
            .list_audit_logs(Some(session.id))
            .await
            .expect("audits")
            .iter()
            .filter(|audit| audit.action == "approval.expired")
            .count(),
        1
    );
}

#[tokio::test]
async fn workflow_turn_end_does_not_complete_an_unfinished_task() {
    let (state, session) = harness_test_session().await;
    let grant = persisted_harness_task_grant(&state, &session).await;
    let run = state
        .update_workflow_run_root_task_grant(grant.workflow_run_id, grant.id)
        .await
        .unwrap();
    let mut step = state
        .get_workflow_step_run(grant.workflow_step_run_id.unwrap())
        .await
        .unwrap();
    step.claimed_by_worker = Some("worker-turn-test".to_string());
    step.claim_owner_version = WORKFLOW_STEP_CLAIM_OWNER_VERSION;
    let step = state.update_workflow_step_run(step).await.unwrap();
    let job = state
        .enqueue_session_loop_job(session.id, None, "test turn")
        .await
        .unwrap();
    state
        .start_session_loop_job(job.id, "worker-turn-test")
        .await
        .unwrap();
    let completed = state
        .complete_session_loop_job(job.id, "worker-turn-test")
        .await
        .unwrap();
    let refs = collect_session_runtime_refs(&state, session.id)
        .await
        .unwrap();
    let updated = update_workflow_step_after_worker_session(
        &state,
        &run,
        &step,
        &session,
        &completed,
        "worker-turn-test",
        refs,
        None,
        false,
    )
    .await
    .unwrap();
    assert_ne!(
        updated.status, "completed",
        "finishing one idle turn must not close the business task"
    );
    assert!(updated.completed_at.is_none());
    assert_eq!(
        state.get_task_grant(grant.id).await.unwrap().status,
        "active"
    );
}

#[tokio::test]
async fn long_history_context_retains_goal_and_current_result() {
    let (state, session) = harness_test_session().await;
    let user = state
        .append_event(
            "user",
            None,
            session.id,
            "user.message",
            json!({"message": "Check the order status"}),
        )
        .await
        .unwrap();
    state
        .append_event(
            "user",
            None,
            session.id,
            "session.goal.created",
            json!({"objective": "Check the order status"}),
        )
        .await
        .unwrap();
    for _ in 0..2000 {
        state
            .append_event(
                "agent",
                None,
                session.id,
                "llm.request",
                json!({"context": "x".repeat(4096)}),
            )
            .await
            .unwrap();
    }
    let result = state
        .append_event(
            "user",
            None,
            session.id,
            "user.custom_tool_result",
            json!({"order_status": "shipped"}),
        )
        .await
        .unwrap();
    let start = std::time::Instant::now();
    let mut has_goal = false;
    for _ in 0..10 {
        let context = build_harness_context(&state, session.id, Some(result.seq), Some(result.seq))
            .await
            .unwrap();
        assert_eq!(
            context.last_user_message.as_deref(),
            Some("Check the order status")
        );
        assert_eq!(context.event_count, 2003);
        assert_eq!(context.pending_event_count, 1);
        assert_eq!(context.recent_custom_tool_results.len(), 1);
        has_goal = context.latest_goal_event.is_some();
    }
    eprintln!(
        "long-history context, 2000 diagnostic payloads, 10 reads: {:?}; initial event {}",
        start.elapsed(),
        user.seq
    );
    assert!(has_goal, "tool-result-only turns must retain the task goal");
}
async fn assert_context_reads_are_session_scoped(state: &AppState, session: &Session) {
    let other = state
        .create_session(CreateSession {
            agent_id: session.agent_id,
            environment_id: None,
            title: "unrelated session".into(),
            message: None,
        })
        .await
        .unwrap();
    let mut job_ids = Vec::new();
    for target in [session, &other] {
        let call = state
            .insert_tool_call(waiting_ontology_action_call(target.id))
            .await
            .unwrap();
        let approval = state
            .insert_approval(pending_tool_approval(&call, None))
            .await
            .unwrap();
        let job = state
            .execution_queue
            .enqueue(ExecutionJobRequest {
                session_id: target.id,
                environment_id: None,
                approval_id: approval.id,
                tool_call_id: call.id,
                tool_name: call.tool_name.clone(),
                max_attempts: None,
            })
            .await
            .unwrap();
        job_ids.push(job.id);
    }
    let relevant = state
        .execution_queue
        .list_for_session(session.id)
        .await
        .unwrap();
    assert_eq!(relevant.len(), 1);
    assert_eq!(relevant[0].id, job_ids[0]);
    let user = state
        .append_event(
            "user",
            None,
            session.id,
            "user.message",
            json!({"message":"original task"}),
        )
        .await
        .unwrap();
    state
        .append_event(
            "user",
            None,
            session.id,
            "session.goal.created",
            json!({"objective":"original task"}),
        )
        .await
        .unwrap();
    for _ in 0..30 {
        state
            .append_event(
                "agent",
                None,
                session.id,
                "llm.request",
                json!({"context":"diagnostic only"}),
            )
            .await
            .unwrap();
    }
    let result = state
        .append_event(
            "user",
            None,
            session.id,
            "user.custom_tool_result",
            json!({"answer":42}),
        )
        .await
        .unwrap();
    state
        .append_event(
            "user",
            None,
            other.id,
            "user.message",
            json!({"message":"must not enter the current task"}),
        )
        .await
        .unwrap();
    state
        .append_event(
            "user",
            None,
            session.id,
            "user.message",
            json!({"message":"future task change"}),
        )
        .await
        .unwrap();
    state
        .append_event(
            "user",
            None,
            session.id,
            "session.goal.updated",
            json!({"objective":"future objective"}),
        )
        .await
        .unwrap();
    let history = state
        .load_harness_events(session.id, Some(result.seq), Some(result.seq))
        .await
        .unwrap();
    assert_eq!(history.event_count, 35);
    assert_eq!(history.pending_event_count, 1);
    assert_eq!(
        history.events.len(),
        3,
        "only current input and durable task markers are read"
    );
    assert_eq!(history.events[0].id, user.id);
    let context = build_harness_context(state, session.id, Some(result.seq), Some(result.seq))
        .await
        .unwrap();
    assert_eq!(context.last_user_message.as_deref(), Some("original task"));
    assert_eq!(
        context.latest_goal_event.as_ref().unwrap()["payload"]["objective"],
        "original task"
    );

    let reconciliation_session = state
        .create_session(CreateSession {
            agent_id: session.agent_id,
            environment_id: None,
            title: "reconciliation only".into(),
            message: None,
        })
        .await
        .unwrap();
    let session = &reconciliation_session;
    let grant = persisted_harness_task_grant(state, session).await;
    state
        .update_workflow_run_root_task_grant(grant.workflow_run_id, grant.id)
        .await
        .unwrap();
    let mut primary = state
        .get_workflow_step_run(grant.workflow_step_run_id.unwrap())
        .await
        .unwrap();
    primary.claimed_by_worker = Some("primary-worker".into());
    primary.claim_owner_version = WORKFLOW_STEP_CLAIM_OWNER_VERSION;
    let primary = state.update_workflow_step_run(primary).await.unwrap();
    let mut child = primary.clone();
    child.id = Uuid::new_v4();
    child.step_key = "other-session-step".into();
    child.session_id = Some(other.id);
    child.claimed_by_worker = Some("other-worker".into());
    state.create_workflow_step_run(child.clone()).await.unwrap();
    assert_eq!(
        state
            .list_active_workflow_steps_for_session(session.id)
            .await
            .unwrap()
            .iter()
            .map(|step| step.id)
            .collect::<Vec<_>>(),
        vec![primary.id]
    );
    let job = state
        .enqueue_session_loop_job(session.id, None, "context test")
        .await
        .unwrap();
    state
        .start_session_loop_job(job.id, "primary-worker")
        .await
        .unwrap();
    let job = state
        .complete_session_loop_job(job.id, "primary-worker")
        .await
        .unwrap();
    let updated =
        reconcile_workflow_steps_after_session_loop_job(state, session, &job, "primary-worker")
            .await
            .unwrap();
    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0].status, "requires_action");
    assert_eq!(
        state
            .get_workflow_step_run(child.id)
            .await
            .unwrap()
            .claimed_by_worker
            .as_deref(),
        Some("other-worker")
    );
    assert_eq!(
        state.get_workflow_step_run(child.id).await.unwrap().status,
        "running"
    );
}

#[tokio::test]
async fn context_queries_read_only_current_session_and_preserve_task_markers() {
    let (state, session) = harness_test_session().await;
    assert_context_reads_are_session_scoped(&state, &session).await;
}

#[tokio::test]
#[ignore = "requires MANDOFORGE_TEST_POSTGRES_URL"]
async fn postgres_context_queries_preserve_session_and_tenant_boundaries() {
    let (state, session) = postgres_harness_test_session().await;
    assert_context_reads_are_session_scoped(&state, &session).await;
    let StoreBackend::Postgres(pool) = &state.store else {
        unreachable!()
    };
    let other_tenant = Uuid::new_v4();
    sqlx::query("INSERT INTO tenants (id, name, slug) VALUES ($1, 'Context isolation test', $2)")
        .bind(other_tenant)
        .bind(format!("context-isolation-{other_tenant}"))
        .execute(pool)
        .await
        .unwrap();
    let mut isolated = state.clone();
    isolated.tenant_id = other_tenant;
    isolated.execution_queue = ExecutionQueue::postgres(pool.clone(), other_tenant);
    assert!(
        isolated
            .execution_queue
            .list_for_session(session.id)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        isolated
            .load_harness_events(session.id, None, None)
            .await
            .unwrap()
            .event_count,
        0
    );
    assert!(
        isolated
            .list_active_workflow_steps_for_session(session.id)
            .await
            .unwrap()
            .is_empty()
    );
}

async fn shared_workflow_completion_fixture(
    state: &AppState,
    session: &Session,
) -> (WorkflowRun, TaskGrant, WorkflowStepRun) {
    let grant = persisted_harness_task_grant(state, session).await;
    let run = state
        .update_workflow_run_root_task_grant(grant.workflow_run_id, grant.id)
        .await
        .unwrap();
    let mut step = state
        .get_workflow_step_run(grant.workflow_step_run_id.unwrap())
        .await
        .unwrap();
    step.claimed_by_worker = Some("workflow-worker".into());
    step.claim_owner_version = WORKFLOW_STEP_CLAIM_OWNER_VERSION;
    let step = state.update_workflow_step_run(step).await.unwrap();
    let mut definition = state
        .get_workflow_definition(run.workflow_definition_id)
        .await
        .unwrap();
    definition.step_graph = json!({"steps": [
        {"key": step.step_key, "type": "agent", "start": true},
        {"key": "followup", "type": "agent", "depends_on": [step.step_key]}
    ]});
    state.update_workflow_definition(definition).await.unwrap();
    (run, grant, step)
}

#[tokio::test]
async fn workflow_step_completion_keeps_shared_session_open_until_last_step() {
    let (state, session) = harness_test_session().await;
    let (run, grant, mut step) = shared_workflow_completion_fixture(&state, &session).await;
    for turn in 0..2 {
        let event = state
            .append_event(
                "user",
                None,
                session.id,
                "user.message",
                json!({"message": format!("step {turn}")}),
            )
            .await
            .unwrap();
        let job = state
            .enqueue_session_loop_job(session.id, Some(event.id), "workflow.step.run")
            .await
            .unwrap();
        let job = state
            .start_session_loop_job(job.id, "workflow-worker")
            .await
            .unwrap();
        let completed_session = apply_provider_completion(
            &state,
            session.id,
            Some(grant.id),
            "completed",
            "step objective satisfied",
        )
        .await
        .unwrap();
        assert_eq!(
            completed_session.status,
            SessionStatus::Idle,
            "step completion must leave shared conversation available for graph continuation"
        );
        let completed_job = state
            .complete_session_loop_job(job.id, "workflow-worker")
            .await
            .unwrap();
        let updated = update_workflow_step_after_worker_session(
            &state,
            &run,
            &step,
            &completed_session,
            &completed_job,
            "workflow-worker",
            collect_session_runtime_refs(&state, session.id)
                .await
                .unwrap(),
            None,
            false,
        )
        .await
        .unwrap();
        assert_eq!(updated.status, "completed");
        if turn == 0 {
            assert_ne!(
                state.get_workflow_run(run.id).await.unwrap().status,
                "completed"
            );
            assert_eq!(
                state.get_task_grant(grant.id).await.unwrap().status,
                "active"
            );
            step = state
                .list_workflow_step_runs(run.id)
                .await
                .unwrap()
                .into_iter()
                .find(|step| step.step_key == "followup")
                .unwrap();
            step.status = "running".into();
            step.claimed_by_worker = Some("workflow-worker".into());
            step.claim_owner_version = WORKFLOW_STEP_CLAIM_OWNER_VERSION;
            state
                .claim_workflow_step_run_if_available(step.clone(), Utc::now())
                .await
                .unwrap();
        }
    }
    assert_eq!(
        state.get_workflow_run(run.id).await.unwrap().status,
        "completed"
    );
    assert_eq!(
        state.get_session(session.id).await.unwrap().status,
        SessionStatus::Terminated
    );
}

#[tokio::test]
async fn workflow_completion_records_evidence_before_session_finalization() {
    let (state, session) = harness_test_session().await;
    let (run, _, _) = shared_workflow_completion_fixture(&state, &session).await;
    let completed = update_workflow_run_status_and_record(&state, &run, "completed")
        .await
        .unwrap();
    let events = state.list_events(session.id).await.unwrap();
    let completion = events
        .iter()
        .find(|e| e.event_type == "workflow.run.completed")
        .unwrap();
    let terminal = events
        .iter()
        .find(|e| e.event_type == "session.status_terminated")
        .unwrap();
    assert!(
        completion.seq < terminal.seq,
        "completion evidence must precede fallible finalization"
    );
    update_workflow_run_status_and_record(&state, &completed, "completed")
        .await
        .unwrap();
    assert_eq!(
        state
            .list_events(session.id)
            .await
            .unwrap()
            .iter()
            .filter(|e| e.event_type == "workflow.run.completed")
            .count(),
        1
    );
}

#[tokio::test]
async fn workflow_continuation_preserves_attempt_failure() {
    let (state, session) = harness_test_session().await;
    let (_, _, step) = shared_workflow_completion_fixture(&state, &session).await;
    let job = state
        .enqueue_session_loop_job(session.id, None, "continuation")
        .await
        .unwrap();
    let job = state
        .start_session_loop_job(job.id, "workflow-worker")
        .await
        .unwrap();
    let result = Err(AppError::bad_request("provider continuation unavailable"));
    settle_session_loop_attempt(&state, &job, "workflow-worker", &result, None)
        .await
        .unwrap();
    let step = state.get_workflow_step_run(step.id).await.unwrap();
    assert_eq!(step.status, "failed");
    assert_eq!(
        step.output_payload["worker_execution"]["error"],
        "provider continuation unavailable"
    );
}

#[tokio::test]
async fn workflow_completion_retries_cleanup_for_terminal_sessions() {
    let (state, session) = harness_test_session().await;
    let (run, _, _) = shared_workflow_completion_fixture(&state, &session).await;
    let computer = state
        .create_remote_computer(CreateRemoteComputer {
            id: None,
            name: "workflow-cleanup".into(),
            profile: Some("workspace-write".into()),
            namespace: None,
            pod_name: Some("workflow-cleanup-pod".into()),
            workspace_path: None,
            state_mount_path: None,
            metadata: Some(json!({"warm_pool":true})),
        })
        .await
        .unwrap();
    let lease = state
        .create_remote_computer_lease(
            computer.id,
            CreateRemoteComputerLease {
                session_id: Some(session.id),
                worker_id: Some("workflow-worker".into()),
                lease_seconds: Some(60),
                metadata: Some(json!({"on_demand":false})),
            },
        )
        .await
        .unwrap();
    // Reproduce an interrupted finalization: terminal status persisted, lease still active.
    let StoreBackend::Memory(inner) = &state.store else {
        unreachable!()
    };
    inner
        .write()
        .await
        .sessions
        .get_mut(&session.id)
        .unwrap()
        .status = SessionStatus::Terminated;
    let completed = state
        .update_workflow_run_status(run.id, "completed".into(), run.started_at, Some(Utc::now()))
        .await
        .unwrap();
    update_workflow_run_status_and_record(&state, &completed, "completed")
        .await
        .unwrap();
    let leases = state.list_remote_computer_leases().await.unwrap();
    assert_eq!(
        leases.iter().find(|l| l.id == lease.id).unwrap().status,
        "released"
    );
}

async fn assert_workflow_completion_recovers_before_queue_ack(
    state: &AppState,
    session: &Session,
    step_was_saved: bool,
) {
    let (run, grant, mut step) = shared_workflow_completion_fixture(state, session).await;
    let event = state
        .append_event(
            "user",
            None,
            session.id,
            "user.message",
            json!({"message":"complete first step"}),
        )
        .await
        .unwrap();
    let job = state
        .enqueue_session_loop_job(session.id, Some(event.id), "workflow.step.run")
        .await
        .unwrap();
    state
        .start_session_loop_job(job.id, "workflow-worker")
        .await
        .unwrap();
    apply_provider_completion(
        state,
        session.id,
        Some(grant.id),
        "completed",
        "first step done",
    )
    .await
    .unwrap();
    if step_was_saved {
        step.status = "completed".into();
        step.output_payload = json!({"worker_execution": {"session_loop_job_id": job.id}});
        state
            .update_claimed_workflow_step_run(step.clone(), "workflow-worker")
            .await
            .unwrap();
    }
    let expired = Utc::now() - chrono::Duration::seconds(1);
    match &state.store {
        StoreBackend::Memory(inner) => {
            let mut store = inner.write().await;
            store
                .session_loop_jobs
                .get_mut(&job.id)
                .unwrap()
                .lease_expires_at = Some(expired);
            store
                .workflow_step_runs
                .get_mut(&step.id)
                .unwrap()
                .lease_expires_at = Some(expired);
        }
        StoreBackend::Postgres(pool) => {
            sqlx::query("UPDATE session_loop_jobs SET lease_expires_at = $2 WHERE id = $1")
                .bind(job.id)
                .bind(expired)
                .execute(pool)
                .await
                .unwrap();
            sqlx::query("UPDATE workflow_step_runs SET lease_expires_at = $2 WHERE id = $1")
                .bind(step.id)
                .bind(expired)
                .execute(pool)
                .await
                .unwrap();
        }
    }
    let claimed = state
        .start_session_loop_job(job.id, "replacement-worker")
        .await
        .unwrap();
    let result = run_session_loop(state, &claimed).await;
    assert!(
        result.is_ok(),
        "recorded completion must bypass the model: {result:?}"
    );
    let settled = settle_session_loop_attempt(state, &claimed, "replacement-worker", &result, None)
        .await
        .unwrap();
    assert_eq!(settled.job.status, SessionLoopJobStatus::Completed);
    assert_eq!(
        state.get_workflow_step_run(step.id).await.unwrap().status,
        "completed"
    );
    assert_eq!(
        state.list_workflow_step_runs(run.id).await.unwrap().len(),
        2
    );
    assert_eq!(
        state.list_tool_calls(Some(session.id)).await.unwrap().len(),
        1
    );
    assert!(
        state
            .list_events(session.id)
            .await
            .unwrap()
            .iter()
            .all(|event| event.event_type != "llm.request")
    );
}

#[tokio::test]
async fn workflow_completion_recovers_without_reexecuting_model_or_tools() {
    for step_was_saved in [false, true] {
        let (state, session) = harness_test_session().await;
        assert_workflow_completion_recovers_before_queue_ack(&state, &session, step_was_saved)
            .await;
    }
}

#[tokio::test]
#[ignore = "requires MANDOFORGE_TEST_POSTGRES_URL"]
async fn postgres_workflow_completion_recovers_without_reexecuting_effects() {
    for step_was_saved in [false, true] {
        let (state, session) = postgres_harness_test_session().await;
        assert_workflow_completion_recovers_before_queue_ack(&state, &session, step_was_saved)
            .await;
    }
}

async fn assert_workflow_session_claims_are_exclusive(state: &AppState, session: &Session) {
    let (_, _, mut first) = shared_workflow_completion_fixture(state, session).await;
    first.status = "queued".into();
    first.claimed_by_worker = None;
    first.claim_owner_version = 0;
    state.update_workflow_step_run(first.clone()).await.unwrap();
    let mut second = first.clone();
    second.id = Uuid::new_v4();
    second.step_key = "parallel-same-session".into();
    state
        .create_workflow_step_run(second.clone())
        .await
        .unwrap();
    let now = Utc::now();
    first.status = "running".into();
    first.claimed_by_worker = Some("worker-a".into());
    first.claim_owner_version = WORKFLOW_STEP_CLAIM_OWNER_VERSION;
    first.lease_expires_at = Some(now + chrono::Duration::minutes(5));
    second.status = "running".into();
    second.claimed_by_worker = Some("worker-b".into());
    second.claim_owner_version = WORKFLOW_STEP_CLAIM_OWNER_VERSION;
    second.lease_expires_at = first.lease_expires_at;
    let (a, b) = tokio::join!(
        state.claim_workflow_step_run_if_available(first, now),
        state.claim_workflow_step_run_if_available(second, now)
    );
    assert_ne!(
        a.is_ok(),
        b.is_ok(),
        "only one step may own a shared conversation"
    );
    let winner = a.or(b).unwrap();
    let job = state
        .enqueue_session_loop_job(session.id, None, "claim test")
        .await
        .unwrap();
    assert!(
        state
            .start_session_loop_job(job.id, "unrelated-worker")
            .await
            .is_err()
    );
    assert!(
        state
            .start_session_loop_job(job.id, winner.claimed_by_worker.as_deref().unwrap())
            .await
            .is_ok()
    );
    let running_job = state.get_session_loop_job(job.id).await.unwrap();
    let mut legacy = winner.clone();
    legacy.claim_owner_version = 0;
    legacy.claimed_by_worker = None;
    legacy.lease_expires_at = Some(now - chrono::Duration::seconds(1));
    state.update_workflow_step_run(legacy).await.unwrap();
    assert!(
        state
            .adopt_workflow_step_for_loop(
                winner.id,
                &running_job,
                winner.claimed_by_worker.as_deref().unwrap()
            )
            .await
            .is_err(),
        "a valid loop claim must not upgrade an unversioned legacy workflow claim"
    );
}

#[tokio::test]
async fn workflow_and_loop_claims_share_the_session_execution_boundary() {
    let (state, session) = harness_test_session().await;
    assert_workflow_session_claims_are_exclusive(&state, &session).await;
}

#[tokio::test]
#[ignore = "requires MANDOFORGE_TEST_POSTGRES_URL"]
async fn postgres_workflow_and_loop_claims_share_the_session_execution_boundary() {
    let (state, session) = postgres_harness_test_session().await;
    assert_workflow_session_claims_are_exclusive(&state, &session).await;
}

#[tokio::test]
#[ignore = "requires MANDOFORGE_TEST_POSTGRES_URL"]
async fn postgres_native_worker_runs_independent_sessions_concurrently() {
    let _env = env_lock().lock().unwrap();
    let (state, seed_session) = postgres_harness_test_session().await;
    let environment = state.create_environment(serde_json::from_value(json!({"name":format!("native worker {}", Uuid::new_v4()), "release_state":"active"})).unwrap()).await.unwrap();
    let mut sessions = Vec::new();
    for title in ["long", "short"] {
        let session = state
            .create_session(CreateSession {
                agent_id: seed_session.agent_id,
                environment_id: Some(environment.id),
                title: title.into(),
                message: None,
            })
            .await
            .unwrap();
        let event = state
            .append_event(
                "user",
                None,
                session.id,
                "user.message",
                json!({"message":title}),
            )
            .await
            .unwrap();
        state
            .enqueue_session_loop_job(session.id, Some(event.id), "native worker benchmark")
            .await
            .unwrap();
        sessions.push(session);
    }
    let finished = Arc::new(tokio::sync::Notify::new());
    let order = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let observed = order.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, Router::new().route("/v1/chat/completions", axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
            let finished = finished.clone(); let order = order.clone();
            async move {
                let context: Value = serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
                let task = context["last_user_message"].as_str().unwrap();
                if task == "long" { tokio::time::timeout(std::time::Duration::from_secs(5), finished.notified()).await.unwrap(); }
                order.lock().await.push(task.to_string());
                if task == "short" { finished.notify_one(); }
                axum::Json(json!({"choices":[{"message":{"role":"assistant","tool_calls":[{"id":"complete", "type":"function", "function":{"name":"complete_task","arguments":"{\"status\":\"completed\",\"summary\":\"task done\"}"}}]}}],"usage":{"prompt_tokens":5,"completion_tokens":5,"total_tokens":10}}))
            }
        }))).await.unwrap();
    });
    let _database = EnvVarGuard::set(
        "DATABASE_URL",
        &std::env::var("MANDOFORGE_TEST_POSTGRES_URL").unwrap(),
    );
    let _worker_token = EnvVarGuard::set("MANDOFORGE_WORKER_TOKEN", "native-worker-test-token");
    let _worker_id = EnvVarGuard::set("WORKER_ID", "native-worker-test");
    let _environment = EnvVarGuard::set("WORKER_ENVIRONMENT_ID", &environment.id.to_string());
    let _pool = EnvVarGuard::remove("WORKER_POOL");
    let _queue = EnvVarGuard::remove("WORKER_QUEUE");
    let _concurrency = EnvVarGuard::set("WORKER_CONCURRENCY", "2");
    let _once = EnvVarGuard::set("RUN_ONCE", "1");
    let _max_jobs = EnvVarGuard::set("MAX_JOBS", "2");
    let _provider = EnvVarGuard::set("MANDOFORGE_PROVIDER_BASE_URL", &format!("http://{address}"));
    let _key = EnvVarGuard::set("MANDOFORGE_PROVIDER_API_KEY", "local-test-only");
    let result = crate::worker_daemon::run_worker_daemon(state.clone()).await;
    server.abort();
    result.unwrap();
    assert_eq!(*observed.lock().await, vec!["short", "long"]);
    for session in sessions {
        assert_eq!(
            state.get_session(session.id).await.unwrap().status,
            SessionStatus::Terminated
        );
    }
}

#[tokio::test]
async fn operator_task_catalog_exposes_only_visible_published_capabilities() {
    let (state, session) = harness_test_session().await;
    let grant = persisted_harness_task_grant(&state, &session).await;
    let run = state.get_workflow_run(grant.workflow_run_id).await.unwrap();
    let visible = state
        .get_workflow_definition(run.workflow_definition_id)
        .await
        .unwrap();
    let mut draft = visible.clone();
    draft.id = Uuid::new_v4();
    draft.name = "draft capability".into();
    draft.release_state = "draft".into();
    state
        .create_workflow_definition(draft.clone())
        .await
        .unwrap();
    let mut hidden_agent = state.get_agent(session.agent_id).await.unwrap();
    hidden_agent.id = Uuid::new_v4();
    hidden_agent.team_id = Some(Uuid::new_v4());
    let StoreBackend::Memory(inner) = &state.store else {
        unreachable!()
    };
    inner
        .write()
        .await
        .agents
        .insert(hidden_agent.id, hidden_agent.clone());
    let mut hidden = visible.clone();
    hidden.id = Uuid::new_v4();
    hidden.name = "private team capability".into();
    hidden.default_agent_id = hidden_agent.id;
    state
        .create_workflow_definition(hidden.clone())
        .await
        .unwrap();
    let app = build_router(state.clone());
    let operator_headers = [
        ("x-mandoforge-subject", "catalog-operator"),
        ("x-mandoforge-roles", "operator"),
    ];
    let catalog: Vec<Value> = request_json(
        app.clone(),
        json_request_with_headers(
            "GET",
            "/api/workflow-definitions",
            json!({}),
            &operator_headers,
        ),
    )
    .await;
    assert_eq!(catalog.len(), 1);
    assert_eq!(catalog[0]["id"], json!(visible.id));
    assert!(catalog[0].get("step_graph").is_none());
    assert!(catalog[0].get("handoff_rules").is_none());
    let admin: Vec<WorkflowDefinition> = request_json(
        app.clone(),
        json_request_with_headers(
            "GET",
            "/api/workflow-definitions",
            json!({}),
            &[
                ("x-mandoforge-subject", "admin"),
                ("x-mandoforge-roles", "admin"),
            ],
        ),
    )
    .await;
    assert_eq!(admin.len(), 3);
    let before = state.list_sessions().await.unwrap().len();
    let response = app
        .clone()
        .oneshot(json_request_with_headers(
            "POST",
            "/api/workflow-runs",
            json!({"workflow_definition_id":hidden.id}),
            &operator_headers,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(state.list_sessions().await.unwrap().len(), before);
    let response = app
        .oneshot(json_request_with_headers(
            "GET",
            &format!("/api/workflow-definitions/{}", visible.id),
            json!({}),
            &operator_headers,
        ))
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "full workflow configuration remains admin-only"
    );
}

#[tokio::test]
async fn cancellation_after_model_completion_does_not_advance_the_workflow() {
    let (state, session) = harness_test_session().await;
    let (run, grant, _) = shared_workflow_completion_fixture(&state, &session).await;
    let event = state
        .append_event(
            "user",
            None,
            session.id,
            "user.message",
            json!({"message":"first step"}),
        )
        .await
        .unwrap();
    let job = state
        .enqueue_session_loop_job(session.id, Some(event.id), "workflow.step.run")
        .await
        .unwrap();
    let job = state
        .start_session_loop_job(job.id, "workflow-worker")
        .await
        .unwrap();
    apply_provider_completion(
        &state,
        session.id,
        Some(grant.id),
        "completed",
        "first step done",
    )
    .await
    .unwrap();
    append_incoming_session_event(
        &state,
        session.id,
        serde_json::from_value(
            json!({"type":"user.interrupt","payload":{"reason":"stop before next step"}}),
        )
        .unwrap(),
    )
    .await
    .unwrap();
    assert!(
        successful_provider_completion_for_job(&state, &job)
            .await
            .unwrap()
            .is_none()
    );
    let stopped = state.get_session(session.id).await.unwrap();
    settle_session_loop_attempt(&state, &job, "workflow-worker", &Ok(stopped), None)
        .await
        .unwrap();
    assert_ne!(
        state.get_workflow_run(run.id).await.unwrap().status,
        "completed"
    );
    assert!(
        state
            .list_workflow_step_runs(run.id)
            .await
            .unwrap()
            .iter()
            .all(|step| step.step_key != "followup"
                || !matches!(step.status.as_str(), "queued" | "running"))
    );
}

async fn assert_pending_approval_preserves_the_queued_model_window(
    state: &AppState,
    session: &Session,
) {
    let call = state
        .insert_tool_call(waiting_ontology_action_call(session.id))
        .await
        .unwrap();
    let approval = state
        .insert_approval(pending_tool_approval(&call, None))
        .await
        .unwrap();
    let event = state
        .append_event(
            "tool",
            None,
            session.id,
            "tool.result",
            json!({"tool":"file.read","content":{"summary":"read before approval"}}),
        )
        .await
        .unwrap();
    let queued = state
        .enqueue_session_loop_job(session.id, Some(event.id), "tool.result")
        .await
        .unwrap();
    for _ in 0..3 {
        assert!(
            state
                .start_session_loop_job(queued.id, "waiting-worker")
                .await
                .is_err(),
            "pending approval must not invoke another model turn"
        );
    }
    let waiting = state.get_session_loop_job(queued.id).await.unwrap();
    assert_eq!(waiting.status, SessionLoopJobStatus::Queued);
    assert_eq!(waiting.attempt_count, 0);
    assert_eq!(
        waiting.pending_event_seq_start,
        queued.pending_event_seq_start
    );
    assert_eq!(
        state
            .list_approvals()
            .await
            .unwrap()
            .iter()
            .filter(|approval| approval.session_id == session.id)
            .count(),
        1
    );
    let (_, _, events) = state
        .decline_approval_and_tool_call(approval.id, "rejected", "user")
        .await
        .unwrap();
    state
        .enqueue_session_loop_job(
            session.id,
            events.last().map(|event| event.id),
            "approval.rejected",
        )
        .await
        .unwrap();
    let resumed = state
        .start_session_loop_job(queued.id, "waiting-worker")
        .await
        .unwrap();
    assert_eq!(resumed.attempt_count, 1);
    assert_eq!(
        resumed.pending_event_seq_start,
        queued.pending_event_seq_start
    );
    assert!(resumed.pending_event_seq_end > queued.pending_event_seq_end);
}

#[tokio::test]
async fn pending_approval_does_not_repeat_model_work_or_consume_the_cursor() {
    let (state, session) = harness_test_session().await;
    assert_pending_approval_preserves_the_queued_model_window(&state, &session).await;
}

#[tokio::test]
#[ignore = "requires MANDOFORGE_TEST_POSTGRES_URL"]
async fn postgres_pending_approval_does_not_repeat_model_work_or_consume_the_cursor() {
    let (state, session) = postgres_harness_test_session().await;
    assert_pending_approval_preserves_the_queued_model_window(&state, &session).await;
}

#[tokio::test]
async fn workflow_task_details_reach_both_runtime_paths_and_provider_context() {
    let (state, session) = harness_test_session().await;
    let grant = persisted_harness_task_grant(&state, &session).await;
    let mut run = state
        .update_workflow_run_root_task_grant(grant.workflow_run_id, grant.id)
        .await
        .unwrap();
    let detail = format!(
        "{} important details after the short display title",
        "task detail ".repeat(30)
    );
    run.input_payload =
        json!({"objective":detail,"order_id":"order-35","items":["first","last-item"]});
    run.runtime_envelope = json!({"private_note":"INTERNAL-CONTROL-ONLY"});
    let mut step = state
        .get_workflow_step_run(grant.workflow_step_run_id.unwrap())
        .await
        .unwrap();
    step.input_payload = workflow_graph_step_input_payload(
        &run,
        &json!({"key":"inspect","type":"agent","input":{"objective":"Inspect the order","limit":3}}),
        json!({}),
    );
    let message = workflow_step_worker_message(&step, &grant);
    let delegated = delegated_runtime_turn_message(&state, &run, &step)
        .await
        .unwrap();
    for text in [&message, &delegated] {
        assert!(
            text.contains(&detail),
            "full user instructions must survive the display summary"
        );
        assert!(text.contains("order-35") && text.contains("last-item"));
        assert!(text.contains("not execution authority"));
        assert!(!text.contains("INTERNAL-CONTROL-ONLY"));
    }
    let event = append_user_message_event(&state, session.id, message.clone())
        .await
        .unwrap();
    run_provider_harness(
        &state,
        session.id,
        &HarnessTestProvider { fail: false },
        "input-test",
        Some(event.seq),
        Some(event.seq),
    )
    .await
    .unwrap();
    let events = state.list_events(session.id).await.unwrap();
    let request = events
        .iter()
        .find(|event| event.event_type == "llm.request")
        .unwrap();
    assert_eq!(
        request.payload["context"]["last_user_message"],
        json!(message)
    );
}
