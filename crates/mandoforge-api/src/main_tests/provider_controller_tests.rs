use super::*;

#[tokio::test]
async fn provider_runtime_production_mode_blocks_env_fallback() {
    let _lock = env_lock().lock().expect("env lock");
    let _provider_runtime_env = EnvVarGuard::set("MANDOFORGE_PROVIDER_RUNTIME_ENV", "production");
    let _provider_base_url = EnvVarGuard::remove("MANDOFORGE_PROVIDER_BASE_URL");
    let _provider_api_key = EnvVarGuard::remove("MANDOFORGE_PROVIDER_API_KEY");
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    state.seed_demo_agent().await.expect("seed demo agent");
    let agent = state
        .list_agents()
        .await
        .expect("agents")
        .into_iter()
        .next()
        .expect("agent");
    let session = state
        .create_session(CreateSession {
            agent_id: agent.id,
            environment_id: None,
            title: "production provider fallback".to_string(),
            message: None,
        })
        .await
        .expect("session");

    let error = match provider_client_for_session(&state, session.id).await {
        Ok(_) => panic!("production mode accepts provider fallback"),
        Err(error) => error,
    };

    assert_eq!(error.status, StatusCode::FORBIDDEN);
    assert!(
        error
            .message
            .contains("production provider runtime requires stored active provider")
    );
}

#[tokio::test]
async fn provider_runtime_production_mode_blocks_mock_provider() {
    let _lock = env_lock().lock().expect("env lock");
    let _provider_runtime_env = EnvVarGuard::set("MANDOFORGE_PROVIDER_RUNTIME_ENV", "production");
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    state.seed_demo_agent().await.expect("seed demo agent");
    state
        .create_provider(CreateProviderRecord {
            provider_type: "mock".to_string(),
            name: "openai-compatible".to_string(),
            base_url: None,
            default_model: Some("gpt-5.5".to_string()),
            config: empty_json_object(),
        })
        .await
        .expect("mock provider");
    let agent = state
        .list_agents()
        .await
        .expect("agents")
        .into_iter()
        .next()
        .expect("agent");
    let session = state
        .create_session(CreateSession {
            agent_id: agent.id,
            environment_id: None,
            title: "production mock provider".to_string(),
            message: None,
        })
        .await
        .expect("session");

    let error = match provider_client_for_session(&state, session.id).await {
        Ok(_) => panic!("production mode accepts mock provider"),
        Err(error) => error,
    };

    assert_eq!(error.status, StatusCode::FORBIDDEN);
    assert!(
        error
            .message
            .contains("uses mock runtime in production mode")
    );
}

#[tokio::test]
async fn provider_runtime_status_reports_production_mode() {
    let _lock = env_lock().lock().expect("env lock");
    let _provider_runtime_env = EnvVarGuard::set("MANDOFORGE_PROVIDER_RUNTIME_ENV", "production");
    let _release_enforcement = EnvVarGuard::set("MANDOFORGE_AGENT_RELEASE_ENFORCEMENT", "required");
    let _release_environment =
        EnvVarGuard::set("MANDOFORGE_AGENT_RELEASE_ENVIRONMENT", "production");
    let app = build_router(test_state_with_worker(Arc::new(InlineExecutionWorker)));

    let runtime: Value = request_json(
        app,
        Request::builder()
            .method("GET")
            .uri("/api/providers/runtime")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .unwrap(),
    )
    .await;

    assert_eq!(runtime["mode"], json!("production"));
    assert_eq!(runtime["production_mode"], json!(true));
    assert_eq!(runtime["agent_release_enforcement_required"], json!(true));
    assert_eq!(runtime["agent_release_environment"], json!("production"));
    assert!(
        runtime["contract"]
            .as_str()
            .unwrap_or_default()
            .contains("forbids mock providers")
    );
}

#[tokio::test]
async fn provider_deployment_controller_executes_external_boundary() {
    let payloads = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("provider deployment listener");
    let controller_addr = listener.local_addr().expect("provider deployment addr");
    let controller = Router::new()
        .route(
            "/provider-deployment",
            post(mock_provider_deployment_controller),
        )
        .with_state(payloads.clone());
    let controller_server = tokio::spawn(async move {
        axum::serve(listener, controller)
            .await
            .expect("mock provider deployment controller");
    });
    let lookup = |key: &str| match key {
        "MANDOFORGE_PROVIDER_DEPLOYMENT_CONTROLLER_URL" => {
            Some(format!("http://{controller_addr}/provider-deployment"))
        }
        "MANDOFORGE_PROVIDER_DEPLOYMENT_CONTROLLER_TOKEN" => {
            Some("provider-deployment-token".to_string())
        }
        _ => None,
    };
    let provider_id = Uuid::new_v4();
    let results = vec![ProviderHealth {
        provider_id,
        name: "deployment-mock".to_string(),
        status: "healthy".to_string(),
        healthy: true,
        issues: vec![],
        checks: json!({"kind": "mock"}),
        checked_at: Utc::now(),
    }];

    let execution =
        execute_provider_deployment_controller(&lookup, "admin-1", Utc::now(), &results)
            .await
            .expect("provider deployment controller");

    assert_eq!(execution["status"], "validated");
    assert_eq!(execution["deployment_id"], "provider-deployment-1");
    let payloads = payloads.lock().await;
    assert_eq!(payloads.len(), 1);
    assert_eq!(payloads[0]["type"], "mandoforge.provider_deployment");
    assert_eq!(payloads[0]["subject"], "admin-1");
    assert_eq!(payloads[0]["provider_count"], 1);
    assert_eq!(
        payloads[0]["providers"][0]["provider_id"],
        provider_id.to_string()
    );
    assert!(payloads[0]["providers"][0]["checks"].is_null());

    controller_server.abort();
}

#[test]
fn provider_deployment_readiness_requires_controller_when_configured() {
    let generated_at = Utc::now();
    let missing = build_provider_deployment_readiness(&[], generated_at, true, false);
    assert_eq!(missing.status, "blocked");
    assert!(missing.production_blocked);
    assert!(missing.controller_required);
    assert!(!missing.controller_configured);
    assert!(missing.blocking_reasons.iter().any(|reason| {
        reason == "provider deployment controller is required but not configured"
    }));

    let without_controller = build_provider_deployment_readiness(
        &[new_audit_log(
            None,
            "user",
            None,
            "provider.deployment_validation_run",
            "providers",
            None,
            json!({
                "status": "healthy",
                "provider_count": 1,
                "healthy_count": 1,
                "unhealthy_count": 0,
                "controller_required": true,
                "controller_configured": true,
                "controller_execution": {
                    "attempted": false,
                    "status": "skipped",
                    "reason": "controller_not_configured"
                }
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
        "provider.deployment_validation_run",
        "providers",
        None,
        json!({
            "status": "healthy",
            "provider_count": 1,
            "healthy_count": 1,
            "unhealthy_count": 0,
            "controller_required": true,
            "controller_configured": true,
            "controller_execution": {
                "attempted": true,
                "status": "validated",
                "deployment_id": "provider-deployment-1"
            }
        }),
    );
    controller_audit.created_at = generated_at;
    let ready =
        build_provider_deployment_readiness(&[controller_audit.clone()], generated_at, true, true);
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
    let stale =
        build_provider_deployment_readiness(&[stale_controller_audit], generated_at, true, true);
    assert_eq!(stale.status, "blocked");
    assert!(stale.production_blocked);
    assert!(stale.latest_controller_validated);
    assert!(!stale.controller_evidence_fresh);
    assert_eq!(stale.latest_controller_age_hours, Some(25));
    assert!(
        stale
            .blocking_reasons
            .iter()
            .any(|reason| { reason == "provider deployment controller evidence is stale" })
    );
}

#[tokio::test]
async fn provider_production_rollout_executes_external_controller() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let provider = state
        .create_provider(CreateProviderRecord {
            provider_type: "mock".to_string(),
            name: "controller-backed-provider".to_string(),
            base_url: None,
            default_model: Some("gpt-5.5-mini".to_string()),
            config: json!({"budget": {"daily_request_limit": 10}}),
        })
        .await
        .expect("create provider");
    let gate_run = execute_provider_policy_gate(&state, Some("admin-1".to_string()), "user")
        .await
        .expect("policy gate run");
    assert_eq!(gate_run.run.status, "passed");

    let controller_payloads = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("controller listener");
    let controller_addr = listener.local_addr().expect("controller addr");
    let controller = Router::new()
        .route("/provider-rollout", post(mock_provider_rollout_controller))
        .with_state(controller_payloads.clone());
    let controller_server = tokio::spawn(async move {
        axum::serve(listener, controller)
            .await
            .expect("mock provider rollout controller");
    });
    let lookup = |key: &str| match key {
        "MANDOFORGE_PROVIDER_ROLLOUT_CONTROLLER_URL" => {
            Some(format!("http://{controller_addr}/provider-rollout"))
        }
        "MANDOFORGE_PROVIDER_ROLLOUT_TOKEN" => Some("rollout-token".to_string()),
        _ => None,
    };

    let rollout = execute_provider_production_rollout_with_lookup(
        &state,
        "admin-1".to_string(),
        RunProviderProductionRollout {
            environment: Some("production".to_string()),
            reason: Some("external controller test".to_string()),
            provider_ids: vec![provider.id],
        },
        lookup,
    )
    .await
    .expect("provider production rollout");

    assert_eq!(rollout.status, "applied");
    assert_eq!(rollout.provider_count, 1);
    assert!(rollout.controller_configured);
    assert_eq!(rollout.controller_execution["status"], "applied");
    assert_eq!(
        rollout.controller_execution["deployment_id"],
        "provider-rollout-1"
    );

    let payloads = controller_payloads.lock().await;
    assert_eq!(payloads.len(), 1);
    assert_eq!(
        payloads[0]["type"],
        "mandoforge.provider_production_rollout"
    );
    assert_eq!(payloads[0]["environment"], "production");
    assert_eq!(payloads[0]["provider_ids"][0], provider.id.to_string());
    assert_eq!(payloads[0]["latest_gate_run_status"], "passed");
    assert_eq!(
        payloads[0]["providers"][0]["name"],
        "controller-backed-provider"
    );

    let audit_logs = state.list_audit_logs(None).await.expect("audit logs");
    assert!(audit_logs.iter().any(|log| {
        log.action == "provider.production_rollout_applied"
            && log.details["status"] == "applied"
            && log.details["controller_configured"] == true
            && log.details["controller_execution"]["status"] == "applied"
    }));

    controller_server.abort();
}

#[tokio::test]
async fn provider_production_rollback_executes_external_controller() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let provider = state
        .create_provider(CreateProviderRecord {
            provider_type: "mock".to_string(),
            name: "rollback-provider".to_string(),
            base_url: None,
            default_model: Some("gpt-5.5-mini".to_string()),
            config: json!({"budget": {"daily_request_limit": 10}}),
        })
        .await
        .expect("create provider");
    let gate_run = execute_provider_policy_gate(&state, Some("admin-1".to_string()), "user")
        .await
        .expect("policy gate run");
    assert_eq!(gate_run.run.status, "passed");

    let rollout_payloads = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let rollout_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("rollout listener");
    let rollout_addr = rollout_listener.local_addr().expect("rollout addr");
    let rollout_controller = Router::new()
        .route("/provider-rollout", post(mock_provider_rollout_controller))
        .with_state(rollout_payloads.clone());
    let rollout_server = tokio::spawn(async move {
        axum::serve(rollout_listener, rollout_controller)
            .await
            .expect("mock provider rollout controller");
    });
    let rollout_lookup = |key: &str| match key {
        "MANDOFORGE_PROVIDER_ROLLOUT_CONTROLLER_URL" => {
            Some(format!("http://{rollout_addr}/provider-rollout"))
        }
        "MANDOFORGE_PROVIDER_ROLLOUT_TOKEN" => Some("rollout-token".to_string()),
        _ => None,
    };
    let rollout = execute_provider_production_rollout_with_lookup(
        &state,
        "admin-1".to_string(),
        RunProviderProductionRollout {
            environment: Some("production".to_string()),
            reason: Some("rollback source rollout".to_string()),
            provider_ids: vec![provider.id],
        },
        rollout_lookup,
    )
    .await
    .expect("provider production rollout");
    assert_eq!(rollout.status, "applied");

    let rollback_payloads = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let rollback_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("rollback listener");
    let rollback_addr = rollback_listener.local_addr().expect("rollback addr");
    let rollback_controller = Router::new()
        .route(
            "/provider-rollback",
            post(mock_provider_rollout_rollback_controller),
        )
        .with_state(rollback_payloads.clone());
    let rollback_server = tokio::spawn(async move {
        axum::serve(rollback_listener, rollback_controller)
            .await
            .expect("mock provider rollback controller");
    });
    let rollback_lookup = |key: &str| match key {
        "MANDOFORGE_PROVIDER_ROLLOUT_ROLLBACK_CONTROLLER_URL" => {
            Some(format!("http://{rollback_addr}/provider-rollback"))
        }
        "MANDOFORGE_PROVIDER_ROLLOUT_ROLLBACK_TOKEN" => Some("provider-rollback-token".to_string()),
        _ => None,
    };

    let rollback = execute_provider_production_rollback_with_lookup(
        &state,
        "admin-1".to_string(),
        RunProviderProductionRollback {
            reason: Some("external rollback controller test".to_string()),
        },
        rollback_lookup,
    )
    .await
    .expect("provider production rollback");

    assert_eq!(rollback.status, "rolled_back");
    assert_eq!(rollback.provider_count, 1);
    assert_eq!(rollback.provider_ids, vec![provider.id]);
    assert_eq!(rollback.controller_execution["status"], "rolled_back");
    assert_eq!(
        rollback.controller_execution["rollback_id"],
        "provider-rollback-1"
    );
    assert_eq!(rollback.source_rollout_id, Some(rollout.id));

    let payloads = rollback_payloads.lock().await;
    assert_eq!(payloads.len(), 1);
    assert_eq!(
        payloads[0]["type"],
        "mandoforge.provider_production_rollout_rollback"
    );
    assert_eq!(payloads[0]["environment"], "production");
    assert_eq!(payloads[0]["provider_ids"][0], provider.id.to_string());
    assert_eq!(payloads[0]["source_rollout"]["id"], rollout.id.to_string());
    assert_eq!(
        payloads[0]["source_rollout"]["controller_execution"]["status"],
        "applied"
    );

    let audit_logs = state.list_audit_logs(None).await.expect("audit logs");
    assert!(audit_logs.iter().any(|log| {
        log.action == "provider.production_rollout_rolled_back"
            && log.details["status"] == "rolled_back"
            && log.details["source_rollout_id"] == rollout.id.to_string()
            && log.details["controller_execution"]["status"] == "rolled_back"
    }));

    rollout_server.abort();
    rollback_server.abort();
}

async fn mock_provider_rollout_controller(
    State(payloads): State<Arc<tokio::sync::Mutex<Vec<Value>>>>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Json<Value> {
    assert_eq!(
        headers
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer rollout-token")
    );
    payloads.lock().await.push(payload);
    Json(json!({
        "status": "applied",
        "deployment_id": "provider-rollout-1",
        "message": "rollout controller applied provider config",
        "steps": [
            {"name": "preflight", "status": "passed"},
            {"name": "apply", "status": "applied"},
            {"name": "verify", "status": "passed"}
        ]
    }))
}

async fn mock_provider_rollout_rollback_controller(
    State(payloads): State<Arc<tokio::sync::Mutex<Vec<Value>>>>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Json<Value> {
    assert_eq!(
        headers
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer provider-rollback-token")
    );
    payloads.lock().await.push(payload);
    Json(json!({
        "status": "rolled_back",
        "rollback_id": "provider-rollback-1",
        "message": "provider rollout rollback accepted",
        "steps": [
            {"name": "restore-provider-config", "status": "rolled_back"},
            {"name": "verify-provider-policy", "status": "passed"}
        ]
    }))
}

async fn mock_provider_deployment_controller(
    State(payloads): State<Arc<tokio::sync::Mutex<Vec<Value>>>>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Json<Value> {
    assert_eq!(
        headers
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer provider-deployment-token")
    );
    assert_eq!(payload["type"], "mandoforge.provider_deployment");
    payloads.lock().await.push(payload);
    Json(json!({
        "status": "validated",
        "deployment_id": "provider-deployment-1",
        "message": "provider deployment controller validated active providers",
        "steps": [
            {"name": "provider-health", "status": "passed"},
            {"name": "policy-bindings", "status": "passed"}
        ]
    }))
}
