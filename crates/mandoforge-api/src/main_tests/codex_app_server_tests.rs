use super::*;

fn ready_codex_app_server_control_summary(
    generated_at: DateTime<Utc>,
) -> CodexAppServerControlPlaneSummary {
    let config = CodexAppServerConfig {
        endpoint: "http://codex-app-server.test".to_string(),
        timeout_seconds: 5,
    };
    let stale_poll_audit = AuditLog {
        id: Uuid::new_v4(),
        session_id: None,
        actor_type: "system".to_string(),
        actor_id: None,
        action: "codex_app_server.stale_poll_due_run".to_string(),
        resource_type: "codex_app_server".to_string(),
        resource_id: None,
        details: json!({
            "candidate_count": 0,
            "failed_count": 0
        }),
        created_at: generated_at,
    };
    let deployment_audit = AuditLog {
        id: Uuid::new_v4(),
        session_id: None,
        actor_type: "user".to_string(),
        actor_id: None,
        action: "codex_app_server.deployment_validation".to_string(),
        resource_type: "codex_app_server".to_string(),
        resource_id: None,
        details: json!({
            "status": "healthy",
            "healthy": true,
            "configured": true,
            "endpoint_configured": true,
            "timeout_seconds": 5,
            "issues": []
        }),
        created_at: generated_at,
    };
    build_codex_app_server_control_plane_summary(
        Some(&config),
        &[],
        &[stale_poll_audit, deployment_audit],
        generated_at,
    )
}

#[test]
fn builds_codex_app_server_turn_trace_summary() {
    let base_time = "2026-05-13T00:00:00Z"
        .parse::<DateTime<Utc>>()
        .expect("valid time");
    let runs = vec![
        CodexAppServerRun {
            id: Uuid::new_v4(),
            operation: "turn.create".to_string(),
            thread_id: Some("thread-1".to_string()),
            turn_id: Some("turn-1".to_string()),
            command_id: None,
            status: "running".to_string(),
            request: json!({"message": "inspect"}),
            response: json!({"turn_id": "turn-1"}),
            error: None,
            created_at: base_time,
        },
        CodexAppServerRun {
            id: Uuid::new_v4(),
            operation: "command.execute".to_string(),
            thread_id: Some("thread-1".to_string()),
            turn_id: Some("turn-1".to_string()),
            command_id: Some("command-1".to_string()),
            status: "running".to_string(),
            request: json!({"command": "ls"}),
            response: json!({"command_id": "command-1"}),
            error: None,
            created_at: base_time + chrono::Duration::seconds(1),
        },
        CodexAppServerRun {
            id: Uuid::new_v4(),
            operation: "turn.poll".to_string(),
            thread_id: Some("thread-1".to_string()),
            turn_id: Some("turn-1".to_string()),
            command_id: None,
            status: "completed".to_string(),
            request: json!({}),
            response: json!({"status": "completed"}),
            error: None,
            created_at: base_time + chrono::Duration::seconds(2),
        },
        CodexAppServerRun {
            id: Uuid::new_v4(),
            operation: "turn.poll".to_string(),
            thread_id: Some("thread-1".to_string()),
            turn_id: Some("turn-2".to_string()),
            command_id: None,
            status: "poll_failed".to_string(),
            request: json!({}),
            response: json!({}),
            error: Some(json!({"message": "timeout"})),
            created_at: base_time + chrono::Duration::seconds(3),
        },
    ];
    let summary = build_codex_app_server_trace_summary(&runs);
    assert_eq!(summary.run_count, 4);
    assert_eq!(summary.turn_count, 2);
    assert_eq!(summary.failed_turn_count, 1);
    assert_eq!(summary.active_turn_count, 1);
    assert_eq!(summary.by_operation["turn.poll"], 2);
    assert_eq!(summary.by_failure_domain["none"], 1);
    assert_eq!(summary.by_failure_domain["poll"], 1);
    let turn_1 = summary
        .traces
        .iter()
        .find(|trace| trace.turn_id.as_deref() == Some("turn-1"))
        .expect("turn-1 trace");
    assert_eq!(turn_1.run_count, 3);
    assert_eq!(turn_1.command_count, 1);
    assert_eq!(turn_1.poll_count, 1);
    assert!(turn_1.terminal);
    assert_eq!(turn_1.duration_seconds, 2);
    assert_eq!(turn_1.command_ids, vec!["command-1".to_string()]);
    assert_eq!(turn_1.next_action, "complete");
    assert_eq!(turn_1.dashboard.failure_domain, "none");
    assert_eq!(turn_1.dashboard.command_count, 1);
    assert_eq!(turn_1.dashboard.poll_count, 1);
    assert_eq!(turn_1.latest_error, None);
    assert!(turn_1.operations.contains(&"command.execute".to_string()));
    let session_id = Uuid::new_v4();
    let artifact_id = Uuid::new_v4();
    let events = vec![SessionEvent {
        id: Uuid::new_v4(),
        session_id,
        seq: 1,
        parent_event_id: None,
        actor_type: "worker".to_string(),
        actor_id: None,
        event_type: "codex.task.event".to_string(),
        payload: json!({
            "runner": "app-server",
            "run_id": runs[3].id,
            "turn_id": "turn-2",
            "attempt": 1,
            "status": "poll_failed",
            "terminal": false,
            "error": "timeout"
        }),
        created_at: base_time + chrono::Duration::seconds(4),
    }];
    let audit_logs = vec![AuditLog {
        id: Uuid::new_v4(),
        session_id: Some(session_id),
        actor_type: "worker".to_string(),
        actor_id: Some(artifact_id),
        action: "codex_app_server.artifact_synced".to_string(),
        resource_type: "artifact".to_string(),
        resource_id: Some(artifact_id),
        details: json!({
            "name": "codex-report.md",
            "path": "artifacts/codex-report.md",
            "artifact_type": "markdown",
            "turn_id": "turn-1",
            "command_id": "command-1"
        }),
        created_at: base_time + chrono::Duration::seconds(5),
    }];
    let turn_1_detail = build_codex_app_server_trace_detail(&runs, "turn-1", &events, &audit_logs)
        .expect("turn-1 detail should exist");
    assert_eq!(turn_1_detail.trace.trace_key, "turn-1");
    assert_eq!(turn_1_detail.runs.len(), 3);
    assert_eq!(turn_1_detail.status_timeline.len(), 3);
    assert_eq!(turn_1_detail.by_status["running"], 2);
    assert_eq!(turn_1_detail.by_operation["command.execute"], 1);
    assert_eq!(turn_1_detail.terminal_count, 1);
    assert_eq!(turn_1_detail.non_terminal_count, 2);
    assert_eq!(turn_1_detail.command_ids, vec!["command-1".to_string()]);
    assert_eq!(turn_1_detail.dashboard.artifact_sync_count, 1);
    assert_eq!(
        turn_1_detail.dashboard.operator_action,
        "inspect_artifact_lineage"
    );
    assert_eq!(turn_1_detail.artifact_lineage.len(), 1);
    assert_eq!(turn_1_detail.artifact_lineage[0].artifact_id, artifact_id);
    assert!(
        turn_1_detail
            .evidence
            .iter()
            .any(|item| item.kind == "artifact_sync")
    );
    assert_eq!(turn_1_detail.latest_response["status"], "completed");
    let turn_2 = summary
        .traces
        .iter()
        .find(|trace| trace.turn_id.as_deref() == Some("turn-2"))
        .expect("turn-2 trace");
    assert_eq!(turn_2.error_count, 1);
    assert_eq!(turn_2.latest_status, "poll_failed");
    assert_eq!(turn_2.next_action, "inspect_poll_failure");
    assert_eq!(turn_2.dashboard.failure_domain, "poll");
    assert_eq!(turn_2.duration_seconds, 0);
    assert_eq!(
        turn_2.latest_error.as_ref().expect("latest error")["message"],
        "timeout"
    );
    let turn_2_detail = build_codex_app_server_trace_detail(&runs, "turn-2", &events, &audit_logs)
        .expect("turn-2 detail should exist");
    assert_eq!(turn_2_detail.dashboard.failure_domain, "poll");
    assert_eq!(turn_2_detail.dashboard.poll_count, 2);
    assert!(turn_2_detail.dashboard.stuck);
    assert!(
        turn_2_detail
            .evidence
            .iter()
            .any(|item| item.kind == "poll" && item.message == "timeout")
    );

    let control = build_codex_app_server_control_plane_summary(
        Some(&CodexAppServerConfig {
            endpoint: "http://codex-app-server.test".to_string(),
            timeout_seconds: 5,
        }),
        &runs,
        &[],
        base_time + chrono::Duration::seconds(4),
    );
    assert_eq!(control.production_ops.status, "blocked");
    assert!(control.production_ops.production_blocked);
    assert_eq!(control.production_ops.failed_turn_count, 1);
    assert!(
        control
            .attention_items
            .iter()
            .any(|item| { item.kind == "production_ops_blocked" && item.severity == "critical" })
    );
}

#[test]
fn codex_app_server_production_ops_requires_fresh_stale_poll_supervision() {
    let base_time = "2026-05-13T00:00:00Z"
        .parse::<DateTime<Utc>>()
        .expect("valid time");
    let runs = vec![CodexAppServerRun {
        id: Uuid::new_v4(),
        operation: "turn.poll".to_string(),
        thread_id: Some("thread-1".to_string()),
        turn_id: Some("turn-ready".to_string()),
        command_id: None,
        status: "completed".to_string(),
        request: json!({}),
        response: json!({"status": "completed"}),
        error: None,
        created_at: base_time,
    }];
    let config = CodexAppServerConfig {
        endpoint: "http://codex-app-server.test".to_string(),
        timeout_seconds: 5,
    };
    let without_supervision = build_codex_app_server_control_plane_summary(
        Some(&config),
        &runs,
        &[],
        base_time + chrono::Duration::minutes(5),
    );
    assert_eq!(without_supervision.production_ops.status, "blocked");
    assert!(without_supervision.production_ops.production_blocked);
    assert!(
        without_supervision
            .production_ops
            .message
            .contains("stale-turn supervision has not run")
    );
    assert_eq!(without_supervision.deployment_readiness.status, "blocked");
    assert!(
        without_supervision
            .deployment_readiness
            .message
            .contains("deployment validation has not run")
    );

    let audit_logs = vec![
        AuditLog {
            id: Uuid::new_v4(),
            session_id: None,
            actor_type: "system".to_string(),
            actor_id: None,
            action: "codex_app_server.stale_poll_due_run".to_string(),
            resource_type: "codex_app_server".to_string(),
            resource_id: None,
            details: json!({
                "candidate_count": 0,
                "polled_count": 0,
                "terminal_count": 0,
                "skipped_count": 0,
                "failed_count": 0
            }),
            created_at: base_time + chrono::Duration::minutes(1),
        },
        AuditLog {
            id: Uuid::new_v4(),
            session_id: None,
            actor_type: "user".to_string(),
            actor_id: Some(Uuid::new_v4()),
            action: "codex_app_server.deployment_validation".to_string(),
            resource_type: "codex_app_server".to_string(),
            resource_id: None,
            details: json!({
                "status": "healthy",
                "healthy": true,
                "configured": true,
                "endpoint_configured": true,
                "timeout_seconds": 5,
                "issues": []
            }),
            created_at: base_time + chrono::Duration::minutes(2),
        },
    ];
    let ready = build_codex_app_server_control_plane_summary(
        Some(&config),
        &runs,
        &audit_logs,
        base_time + chrono::Duration::minutes(5),
    );
    assert_eq!(ready.production_ops.status, "ready");
    assert!(!ready.production_ops.production_blocked);
    assert_eq!(ready.production_ops.latest_stale_poll_failed_count, 0);
    assert_eq!(ready.production_ops.latest_stale_poll_candidate_count, 0);
    assert_eq!(ready.deployment_readiness.status, "ready");
    assert!(!ready.deployment_readiness.production_blocked);
    assert!(ready.deployment_readiness.deployment_validated);
    assert!(ready.deployment_readiness.latest_validation_healthy);
    assert_eq!(ready.status, "ready");
}

#[tokio::test]
async fn codex_artifact_sync_rejects_missing_workspace_file() {
    let codex_client = Arc::new(RecordingCodexAppServerClient::default());
    let app = test_app_with_codex_app_server(codex_client).await;

    let agents: Vec<Agent> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/agents")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let session: Session = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/sessions")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({"agent_id": agents[0].id, "title": "codex artifact sync missing file"})
                    .to_string(),
            ))
            .expect("valid request"),
    )
    .await;

    let (status, body) = request_value(
        app,
        Request::builder()
            .method("POST")
            .uri("/api/codex-app-server/artifacts/sync")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({
                    "session_id": session.id,
                    "turn_id": "turn-1",
                    "command_id": "command-1",
                    "artifacts": [{
                        "name": "codex-report.md",
                        "artifact_type": "markdown",
                        "path": "artifacts/codex-report.md",
                        "content": {"markdown": "# Codex Report"},
                        "metadata": {"source": "mock"}
                    }]
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body["error"]
            .as_str()
            .unwrap_or_default()
            .contains("existing file inside the session workspace")
    );
}

#[tokio::test]
async fn codex_app_server_deployment_controller_executes_external_boundary() {
    let payloads = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("codex app server deployment listener");
    let controller_addr = listener.local_addr().expect("codex deployment addr");
    let controller = Router::new()
        .route(
            "/codex-app-server-deployment",
            post(mock_codex_app_server_deployment_controller),
        )
        .with_state(payloads.clone());
    let controller_server = tokio::spawn(async move {
        axum::serve(listener, controller)
            .await
            .expect("mock codex app server deployment controller");
    });
    let lookup = |key: &str| match key {
        "MANDOFORGE_CODEX_APP_SERVER_DEPLOYMENT_CONTROLLER_URL" => Some(format!(
            "http://{controller_addr}/codex-app-server-deployment"
        )),
        "MANDOFORGE_CODEX_APP_SERVER_DEPLOYMENT_CONTROLLER_TOKEN" => {
            Some("codex-deployment-token".to_string())
        }
        _ => None,
    };
    let config = CodexAppServerConfig {
        endpoint: "http://codex-app-server.test".to_string(),
        timeout_seconds: 7,
    };

    let execution =
        execute_codex_app_server_deployment_controller(&lookup, "admin-1", Utc::now(), &config)
            .await
            .expect("codex deployment controller");

    assert_eq!(execution["status"], "validated");
    assert_eq!(execution["deployment_id"], "codex-app-server-deployment-1");
    let payloads = payloads.lock().await;
    assert_eq!(payloads.len(), 1);
    assert_eq!(
        payloads[0]["type"],
        "mandoforge.codex_app_server_deployment"
    );
    assert_eq!(payloads[0]["subject"], "admin-1");
    assert_eq!(payloads[0]["app_server"]["timeout_seconds"], 7);
    assert_eq!(
        payloads[0]["app_server"]["endpoint"],
        "http://codex-app-server.test"
    );

    controller_server.abort();
}

#[tokio::test]
async fn codex_app_server_ops_controller_executes_external_boundary() {
    let payloads = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("codex app server ops listener");
    let controller_addr = listener.local_addr().expect("codex ops addr");
    let controller = Router::new()
        .route(
            "/codex-app-server-ops",
            post(mock_codex_app_server_ops_controller),
        )
        .with_state(payloads.clone());
    let controller_server = tokio::spawn(async move {
        axum::serve(listener, controller)
            .await
            .expect("mock codex app server ops controller");
    });
    let checked_at = Utc::now();
    let summary = ready_codex_app_server_control_summary(checked_at);
    let lookup = |key: &str| match key {
        "MANDOFORGE_CODEX_APP_SERVER_OPS_CONTROLLER_URL" => {
            Some(format!("http://{controller_addr}/codex-app-server-ops"))
        }
        "MANDOFORGE_CODEX_APP_SERVER_OPS_CONTROLLER_TOKEN" => Some("codex-ops-token".to_string()),
        _ => None,
    };

    let execution =
        execute_codex_app_server_ops_controller(&lookup, Some("admin-1"), checked_at, &summary)
            .await
            .expect("codex app server ops controller");

    assert_eq!(execution["status"], "validated");
    assert_eq!(execution["ops_id"], "codex-ops-1");
    let payloads = payloads.lock().await;
    assert_eq!(payloads.len(), 1);
    assert_eq!(payloads[0]["type"], "mandoforge.codex_app_server_ops");
    assert_eq!(payloads[0]["subject"], "admin-1");
    assert_eq!(payloads[0]["control_plane"]["configured"], true);
    assert!(payloads[0]["control_plane"]["endpoint"].is_null());
    assert!(payloads[0]["secret"].is_null());

    controller_server.abort();
}

#[test]
fn codex_app_server_production_ops_requires_controller_when_configured() {
    let base_time = "2026-05-13T00:00:00Z"
        .parse::<DateTime<Utc>>()
        .expect("valid time");
    let runs = vec![CodexAppServerRun {
        id: Uuid::new_v4(),
        operation: "turn.poll".to_string(),
        thread_id: Some("thread-1".to_string()),
        turn_id: Some("turn-ready".to_string()),
        command_id: None,
        status: "completed".to_string(),
        request: json!({}),
        response: json!({"status": "completed"}),
        error: None,
        created_at: base_time,
    }];
    let trace_summary = build_codex_app_server_trace_summary(&runs);
    let stale_poll_audit = AuditLog {
        id: Uuid::new_v4(),
        session_id: None,
        actor_type: "system".to_string(),
        actor_id: None,
        action: "codex_app_server.stale_poll_due_run".to_string(),
        resource_type: "codex_app_server".to_string(),
        resource_id: None,
        details: json!({
            "candidate_count": 0,
            "failed_count": 0
        }),
        created_at: base_time + chrono::Duration::minutes(1),
    };
    let missing = build_codex_app_server_production_ops_readiness(
        true,
        &trace_summary,
        0,
        std::slice::from_ref(&stale_poll_audit),
        base_time + chrono::Duration::minutes(5),
        true,
        false,
    );
    assert_eq!(missing.status, "blocked");
    assert!(missing.production_blocked);
    assert!(missing.message.contains("ops controller is required"));

    let generated_at = base_time + chrono::Duration::minutes(5);
    let mut validated_audit = new_audit_log(
        None,
        "user",
        None,
        "codex_app_server.ops_validation",
        "codex_app_server",
        None,
        json!({
            "status": "validated",
            "controller_configured": true,
            "controller_execution": {
                "attempted": true,
                "status": "validated",
                "ops_id": "codex-ops-1"
            }
        }),
    );
    validated_audit.created_at = generated_at;
    let ready = build_codex_app_server_production_ops_readiness(
        true,
        &trace_summary,
        0,
        &[stale_poll_audit, validated_audit],
        generated_at,
        true,
        true,
    );
    assert_eq!(ready.status, "ready");
    assert!(!ready.production_blocked);
    assert!(ready.latest_controller_validated);
    assert!(ready.controller_evidence_fresh);
    assert_eq!(ready.latest_controller_age_hours, Some(0));
    assert_eq!(ready.latest_controller_status.as_deref(), Some("validated"));

    let mut stale_controller_audit = new_audit_log(
        None,
        "user",
        None,
        "codex_app_server.ops_validation",
        "codex_app_server",
        None,
        json!({
            "status": "validated",
            "controller_configured": true,
            "controller_execution": {
                "attempted": true,
                "status": "validated",
                "ops_id": "codex-ops-stale"
            }
        }),
    );
    stale_controller_audit.created_at = generated_at - chrono::Duration::hours(25);
    let fresh_stale_poll_audit = AuditLog {
        id: Uuid::new_v4(),
        session_id: None,
        actor_type: "system".to_string(),
        actor_id: None,
        action: "codex_app_server.stale_poll_due_run".to_string(),
        resource_type: "codex_app_server".to_string(),
        resource_id: None,
        details: json!({
            "candidate_count": 0,
            "failed_count": 0
        }),
        created_at: generated_at,
    };
    let stale_controller = build_codex_app_server_production_ops_readiness(
        true,
        &trace_summary,
        0,
        &[fresh_stale_poll_audit, stale_controller_audit],
        generated_at,
        true,
        true,
    );
    assert_eq!(stale_controller.status, "blocked");
    assert!(stale_controller.production_blocked);
    assert!(!stale_controller.controller_evidence_fresh);
    assert_eq!(stale_controller.latest_controller_age_hours, Some(25));
    assert!(
        stale_controller
            .message
            .contains("Codex App Server ops controller evidence is stale")
    );
}

#[test]
fn codex_app_server_deployment_readiness_requires_controller_when_configured() {
    let generated_at = Utc::now();
    let missing = build_codex_app_server_deployment_readiness(true, &[], generated_at, true, false);
    assert_eq!(missing.status, "blocked");
    assert!(missing.production_blocked);
    assert!(missing.controller_required);
    assert!(!missing.controller_configured);
    assert!(missing.blocking_reasons.iter().any(|reason| {
        reason == "Codex App Server deployment controller is required but not configured"
    }));

    let without_controller = build_codex_app_server_deployment_readiness(
        true,
        &[new_audit_log(
            None,
            "user",
            None,
            "codex_app_server.deployment_validation",
            "codex_app_server",
            None,
            json!({
                "status": "healthy",
                "healthy": true,
                "configured": true,
                "endpoint_configured": true,
                "timeout_seconds": 5,
                "controller_required": true,
                "controller_configured": true,
                "controller_execution": {
                    "attempted": false,
                    "status": "skipped",
                    "reason": "controller_not_configured"
                },
                "issues": []
            }),
        )],
        generated_at,
        true,
        true,
    );
    assert_eq!(without_controller.status, "blocked");
    assert!(without_controller.production_blocked);
    assert!(!without_controller.latest_controller_validated);

    let mut controller_audit = new_audit_log(
        None,
        "user",
        None,
        "codex_app_server.deployment_validation",
        "codex_app_server",
        None,
        json!({
            "status": "healthy",
            "healthy": true,
            "configured": true,
            "endpoint_configured": true,
            "timeout_seconds": 5,
            "controller_required": true,
            "controller_configured": true,
            "controller_execution": {
                "attempted": true,
                "status": "validated",
                "deployment_id": "codex-app-server-deployment-1"
            },
            "issues": []
        }),
    );
    controller_audit.created_at = generated_at;
    let ready = build_codex_app_server_deployment_readiness(
        true,
        &[controller_audit.clone()],
        generated_at,
        true,
        true,
    );
    assert_eq!(ready.status, "ready");
    assert!(!ready.production_blocked);
    assert_eq!(ready.latest_controller_status.as_deref(), Some("validated"));
    assert!(ready.latest_controller_validated);
    assert!(ready.controller_evidence_fresh);
    assert_eq!(ready.latest_controller_age_hours, Some(0));
    assert_eq!(ready.controller_execution_count, 1);
    assert_eq!(ready.controller_failed_count, 0);

    let mut stale_controller_audit = controller_audit;
    stale_controller_audit.created_at = generated_at - chrono::Duration::hours(25);
    let stale = build_codex_app_server_deployment_readiness(
        true,
        &[stale_controller_audit],
        generated_at,
        true,
        true,
    );
    assert_eq!(stale.status, "blocked");
    assert!(stale.production_blocked);
    assert!(stale.latest_controller_validated);
    assert!(!stale.controller_evidence_fresh);
    assert_eq!(stale.latest_controller_age_hours, Some(25));
    assert!(
        stale
            .blocking_reasons
            .iter()
            .any(|reason| { reason == "Codex App Server deployment controller evidence is stale" })
    );
}

async fn mock_codex_app_server_deployment_controller(
    State(payloads): State<Arc<tokio::sync::Mutex<Vec<Value>>>>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Json<Value> {
    assert_eq!(
        headers
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer codex-deployment-token")
    );
    assert_eq!(payload["type"], "mandoforge.codex_app_server_deployment");
    payloads.lock().await.push(payload);
    Json(json!({
        "status": "validated",
        "deployment_id": "codex-app-server-deployment-1",
        "message": "Codex App Server deployment controller validated control plane",
        "steps": [
            {"name": "health", "status": "passed"},
            {"name": "turn-control", "status": "passed"}
        ]
    }))
}

async fn mock_codex_app_server_ops_controller(
    State(payloads): State<Arc<tokio::sync::Mutex<Vec<Value>>>>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Json<Value> {
    assert_eq!(
        headers
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer codex-ops-token")
    );
    payloads.lock().await.push(payload);
    Json(json!({
        "status": "validated",
        "ops_id": "codex-ops-1",
        "message": "Codex App Server production ops validated",
        "checks": [
            {"name": "stale_poll_supervision", "status": "passed"},
            {"name": "turn_trace_health", "status": "passed"},
            {"name": "artifact_sync_ops", "status": "passed"}
        ]
    }))
}
