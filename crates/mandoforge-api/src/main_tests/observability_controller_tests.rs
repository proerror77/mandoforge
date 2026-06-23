use super::*;

#[tokio::test]
async fn observability_remediation_controller_executes_external_boundary() {
    let payloads = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("observability remediation listener");
    let controller_addr = listener
        .local_addr()
        .expect("observability remediation addr");
    let controller = Router::new()
        .route(
            "/observability-remediation",
            post(mock_observability_remediation_controller),
        )
        .with_state(payloads.clone());
    let controller_server = tokio::spawn(async move {
        axum::serve(listener, controller)
            .await
            .expect("mock observability remediation controller");
    });
    let before = ObservabilityBackpressure {
        status: "attention".to_string(),
        queued_jobs: 2,
        running_jobs: 1,
        failed_jobs: 0,
        retryable_jobs: 1,
        pending_approvals: 3,
        waiting_approval_sessions: 1,
        failed_sessions: 0,
        failed_tool_calls: 0,
        oldest_queued_job_age_seconds: Some(420),
    };
    let after = ObservabilityBackpressure {
        status: "healthy".to_string(),
        queued_jobs: 0,
        running_jobs: 1,
        failed_jobs: 0,
        retryable_jobs: 0,
        pending_approvals: 0,
        waiting_approval_sessions: 0,
        failed_sessions: 0,
        failed_tool_calls: 0,
        oldest_queued_job_age_seconds: None,
    };
    let actions = vec![
        "approval_escalation_due_run".to_string(),
        "worker_drain_required".to_string(),
    ];
    let lookup = |key: &str| match key {
        "MANDOFORGE_OBSERVABILITY_REMEDIATION_CONTROLLER_URL" => Some(format!(
            "http://{controller_addr}/observability-remediation"
        )),
        "MANDOFORGE_OBSERVABILITY_REMEDIATION_CONTROLLER_TOKEN" => {
            Some("remediation-token".to_string())
        }
        _ => None,
    };

    let execution =
        execute_observability_remediation_controller(&lookup, &before, &after, &actions)
            .await
            .expect("observability remediation controller");

    assert_eq!(execution["status"], "remediated");
    assert_eq!(execution["remediation_id"], "observability-remediation-1");
    assert_eq!(execution["steps"].as_array().expect("steps").len(), 2);
    let payloads = payloads.lock().await;
    assert_eq!(payloads.len(), 1);
    assert_eq!(payloads[0]["type"], "mandoforge.observability_remediation");
    assert_eq!(payloads[0]["actions"][0], "approval_escalation_due_run");
    assert_eq!(payloads[0]["actions"][1], "worker_drain_required");
    assert_eq!(payloads[0]["before"]["pending_approvals"], 3);
    assert_eq!(payloads[0]["after"]["pending_approvals"], 0);

    controller_server.abort();
}

#[tokio::test]
async fn observability_collector_deployment_controller_executes_external_boundary() {
    let payloads = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("collector deployment listener");
    let controller_addr = listener.local_addr().expect("collector deployment addr");
    let controller = Router::new()
        .route(
            "/collector-deployment",
            post(mock_observability_collector_deployment_controller),
        )
        .with_state(payloads.clone());
    let controller_server = tokio::spawn(async move {
        axum::serve(listener, controller)
            .await
            .expect("mock collector deployment controller");
    });
    let lookup = |key: &str| match key {
        "MANDOFORGE_OBSERVABILITY_COLLECTOR_DEPLOYMENT_CONTROLLER_URL" => {
            Some(format!("http://{controller_addr}/collector-deployment"))
        }
        "MANDOFORGE_OBSERVABILITY_COLLECTOR_DEPLOYMENT_CONTROLLER_TOKEN" => {
            Some("collector-token".to_string())
        }
        _ => None,
    };
    let config = ObservabilityConfig {
        service_name: "mandoforge-api-test".to_string(),
        otlp_endpoint: Some("http://otel-collector:4318".to_string()),
        collector_health_endpoint: None,
        sample_ratio: 1.0,
    };

    let execution = execute_observability_collector_deployment_controller(
        &lookup,
        "admin-1",
        Utc::now(),
        &config,
    )
    .await
    .expect("collector deployment controller");

    assert_eq!(execution["status"], "validated");
    assert_eq!(execution["deployment_id"], "collector-deployment-1");
    let payloads = payloads.lock().await;
    assert_eq!(payloads.len(), 1);
    assert_eq!(
        payloads[0]["type"],
        "mandoforge.observability_collector_deployment"
    );
    assert_eq!(payloads[0]["subject"], "admin-1");
    assert_eq!(payloads[0]["service_name"], "mandoforge-api-test");
    assert_eq!(
        payloads[0]["signal_paths"]["logs"],
        "http://otel-collector:4318/v1/logs"
    );

    controller_server.abort();
}

#[tokio::test]
async fn observability_collector_cluster_controller_executes_external_boundary() {
    let payloads = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("collector cluster listener");
    let controller_addr = listener.local_addr().expect("collector cluster addr");
    let controller = Router::new()
        .route(
            "/collector-cluster",
            post(mock_observability_collector_cluster_controller),
        )
        .with_state(payloads.clone());
    let controller_server = tokio::spawn(async move {
        axum::serve(listener, controller)
            .await
            .expect("mock collector cluster controller");
    });
    let lookup = |key: &str| match key {
        "MANDOFORGE_OBSERVABILITY_COLLECTOR_CLUSTER_CONTROLLER_URL" => {
            Some(format!("http://{controller_addr}/collector-cluster"))
        }
        "MANDOFORGE_OBSERVABILITY_COLLECTOR_CLUSTER_CONTROLLER_TOKEN" => {
            Some("collector-cluster-token".to_string())
        }
        _ => None,
    };
    let config = ObservabilityConfig {
        service_name: "mandoforge-api-test".to_string(),
        otlp_endpoint: Some("http://otel-collector:4318".to_string()),
        collector_health_endpoint: None,
        sample_ratio: 1.0,
    };
    let audit = new_audit_log(
        None,
        "user",
        None,
        "observability.collector_deployment_validation",
        "observability",
        None,
        json!({
            "status": "healthy",
            "healthy": true,
            "controller_execution": {
                "attempted": true,
                "status": "validated",
                "deployment_id": "collector-deployment-1"
            }
        }),
    );
    let deployment_readiness = build_observability_collector_deployment_readiness(
        true,
        true,
        false,
        false,
        &[audit],
        Utc::now(),
    );

    let execution = execute_observability_collector_cluster_controller(
        &lookup,
        "admin-1",
        Utc::now(),
        &config,
        &deployment_readiness,
    )
    .await
    .expect("collector cluster controller");

    assert_eq!(execution["status"], "validated");
    assert_eq!(
        execution["cluster_rollout_id"],
        "collector-cluster-rollout-1"
    );
    let payloads = payloads.lock().await;
    assert_eq!(payloads.len(), 1);
    assert_eq!(
        payloads[0]["type"],
        "mandoforge.observability_collector_cluster_rollout"
    );
    assert_eq!(payloads[0]["subject"], "admin-1");
    assert_eq!(payloads[0]["service_name"], "mandoforge-api-test");
    assert_eq!(payloads[0]["otlp_enabled"], true);
    assert_eq!(
        payloads[0]["deployment_readiness"]["deployment_validated"],
        true
    );

    controller_server.abort();
}

#[test]
fn observability_remediation_supervision_blocks_when_required_without_controller_evidence() {
    let generated_at = Utc::now();
    let missing =
        build_observability_remediation_supervision_readiness(true, false, &[], generated_at);
    assert_eq!(missing.status, "blocked");
    assert!(missing.production_blocked);
    assert!(missing.required);
    assert!(!missing.controller_configured);
    assert!(
        missing
            .blocking_reasons
            .iter()
            .any(|reason| { reason == "remediation controller is required but not configured" })
    );
    assert!(
        missing
            .blocking_reasons
            .iter()
            .any(|reason| { reason == "remediation controller has no audited execution evidence" })
    );

    let audit = new_audit_log(
        None,
        "system",
        None,
        "observability.remediation_run",
        "observability",
        None,
        json!({
            "controller_configured": true,
            "controller_execution": {
                "attempted": true,
                "status": "remediated",
                "remediation_id": "observability-remediation-1"
            }
        }),
    );
    let ready =
        build_observability_remediation_supervision_readiness(true, true, &[audit], generated_at);
    assert_eq!(ready.status, "ready");
    assert!(!ready.production_blocked);
    assert!(ready.latest_controller_remediated);
    assert_eq!(
        ready.latest_controller_status.as_deref(),
        Some("remediated")
    );

    let optional =
        build_observability_remediation_supervision_readiness(false, false, &[], generated_at);
    assert_eq!(optional.status, "optional");
    assert!(!optional.production_blocked);
}

#[test]
fn observability_collector_deployment_readiness_requires_controller_when_configured() {
    let generated_at = Utc::now();
    let missing_controller = build_observability_collector_deployment_readiness(
        true,
        true,
        true,
        false,
        &[],
        generated_at,
    );
    assert_eq!(missing_controller.status, "blocked");
    assert!(missing_controller.production_blocked);
    assert!(missing_controller.blocking_reasons.iter().any(|reason| {
        reason == "collector deployment controller is required but not configured"
    }));

    let mut audit = new_audit_log(
        None,
        "user",
        None,
        "observability.collector_deployment_validation",
        "observability",
        None,
        json!({
            "status": "healthy",
            "healthy": true,
            "controller_required": true,
            "controller_configured": true,
            "controller_execution": {
                "attempted": true,
                "status": "validated",
                "deployment_id": "collector-deployment-1"
            }
        }),
    );
    audit.created_at = generated_at;
    let ready = build_observability_collector_deployment_readiness(
        true,
        true,
        true,
        true,
        &[audit.clone()],
        generated_at,
    );
    assert_eq!(ready.status, "ready");
    assert!(!ready.production_blocked);
    assert!(ready.deployment_validated);
    assert!(ready.latest_controller_validated);
    assert!(ready.controller_evidence_fresh);
    assert_eq!(ready.latest_controller_age_hours, Some(0));
    assert_eq!(ready.latest_controller_status.as_deref(), Some("validated"));

    let mut stale_audit = audit;
    stale_audit.created_at = generated_at - chrono::Duration::hours(25);
    let stale = build_observability_collector_deployment_readiness(
        true,
        true,
        true,
        true,
        &[stale_audit],
        generated_at,
    );
    assert_eq!(stale.status, "blocked");
    assert!(stale.production_blocked);
    assert!(!stale.controller_evidence_fresh);
    assert_eq!(stale.latest_controller_age_hours, Some(25));
    assert!(
        stale
            .blocking_reasons
            .iter()
            .any(|reason| { reason == "collector deployment controller evidence is stale" })
    );
}

#[test]
fn observability_collector_cluster_readiness_requires_controller_when_configured() {
    let generated_at = Utc::now();
    let mut deployment_audit = new_audit_log(
        None,
        "user",
        None,
        "observability.collector_deployment_validation",
        "observability",
        None,
        json!({
            "status": "healthy",
            "healthy": true,
            "controller_execution": {
                "attempted": true,
                "status": "validated",
                "deployment_id": "collector-deployment-1"
            }
        }),
    );
    deployment_audit.created_at = generated_at;
    let deployment_readiness = build_observability_collector_deployment_readiness(
        true,
        true,
        false,
        false,
        &[deployment_audit],
        generated_at,
    );

    let missing_controller = build_observability_collector_cluster_rollout_readiness(
        &deployment_readiness,
        &[],
        generated_at,
        true,
        false,
    );
    assert_eq!(missing_controller.status, "blocked");
    assert!(missing_controller.production_blocked);
    assert!(missing_controller.blocking_reasons.iter().any(|reason| {
        reason == "collector cluster rollout controller is required but not configured"
    }));

    let mut rollout_audit = new_audit_log(
        None,
        "user",
        None,
        "observability.collector_cluster_rollout_validation",
        "observability",
        None,
        json!({
            "status": "validated",
            "controller_required": true,
            "controller_configured": true,
            "controller_execution": {
                "attempted": true,
                "status": "validated",
                "cluster_rollout_id": "collector-cluster-rollout-1"
            },
            "deployment_validated": true
        }),
    );
    rollout_audit.created_at = generated_at;
    let ready = build_observability_collector_cluster_rollout_readiness(
        &deployment_readiness,
        &[rollout_audit.clone()],
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
    assert!(ready.deployment_validated);

    let mut stale_rollout_audit = rollout_audit;
    stale_rollout_audit.created_at = generated_at - chrono::Duration::hours(25);
    let stale = build_observability_collector_cluster_rollout_readiness(
        &deployment_readiness,
        &[stale_rollout_audit],
        generated_at,
        true,
        true,
    );
    assert_eq!(stale.status, "blocked");
    assert!(stale.production_blocked);
    assert!(!stale.controller_evidence_fresh);
    assert_eq!(stale.latest_controller_age_hours, Some(25));
    assert!(
        stale
            .blocking_reasons
            .iter()
            .any(|reason| { reason == "collector cluster rollout controller evidence is stale" })
    );
}

async fn mock_observability_remediation_controller(
    State(payloads): State<Arc<tokio::sync::Mutex<Vec<Value>>>>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Json<Value> {
    assert_eq!(
        headers
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer remediation-token")
    );
    payloads.lock().await.push(payload);
    Json(json!({
        "status": "remediated",
        "remediation_id": "observability-remediation-1",
        "message": "remediation controller accepted backpressure actions",
        "steps": [
            {"name": "approval-escalation", "status": "processed"},
            {"name": "worker-drain", "status": "queued"}
        ]
    }))
}

async fn mock_observability_collector_deployment_controller(
    State(payloads): State<Arc<tokio::sync::Mutex<Vec<Value>>>>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Json<Value> {
    assert_eq!(
        headers
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer collector-token")
    );
    payloads.lock().await.push(payload);
    Json(json!({
        "status": "validated",
        "deployment_id": "collector-deployment-1",
        "message": "collector deployment controller validated OTLP collector",
        "steps": [
            {"name": "collector-health", "status": "checked"},
            {"name": "signal-paths", "status": "validated"}
        ]
    }))
}

async fn mock_observability_collector_cluster_controller(
    State(payloads): State<Arc<tokio::sync::Mutex<Vec<Value>>>>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Json<Value> {
    assert_eq!(
        headers
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer collector-cluster-token")
    );
    payloads.lock().await.push(payload);
    Json(json!({
        "status": "validated",
        "cluster_rollout_id": "collector-cluster-rollout-1",
        "message": "collector cluster controller validated rollout in target cluster",
        "steps": [
            {"name": "k8s-rollout-status", "status": "validated"},
            {"name": "collector-pod-health", "status": "validated"}
        ]
    }))
}
