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
