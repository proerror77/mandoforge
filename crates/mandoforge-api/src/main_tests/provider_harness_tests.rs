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
        apply_provider_completion(&state, session.id, "completed", "objective satisfied")
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
}
