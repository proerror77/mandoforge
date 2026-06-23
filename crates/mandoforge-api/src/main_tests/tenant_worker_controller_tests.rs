use super::*;

#[tokio::test]
async fn worker_load_validation_controller_executes_external_boundary() {
    let payloads = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("worker load validation listener");
    let controller_addr = listener.local_addr().expect("worker load validation addr");
    let controller = Router::new()
        .route(
            "/worker-load-validation",
            post(mock_worker_load_validation_controller),
        )
        .with_state(payloads.clone());
    let controller_server = tokio::spawn(async move {
        axum::serve(listener, controller)
            .await
            .expect("mock worker load validation controller");
    });
    let checked_at = Utc::now();
    let queue_backend = worker_queue_backend_readiness("nats_jetstream");
    let k8s = worker_k8s_readiness_from_manifests();
    let autoscaling = worker_autoscaling_readiness_from_manifests(&[
        "deploy/k8s/worker-hpa.yaml",
        "deploy/k8s/worker-keda.yaml",
        "deploy/k8s/keda.yaml",
    ]);
    let lookup = |key: &str| match key {
        "MANDOFORGE_WORKER_LOAD_VALIDATION_CONTROLLER_URL" => {
            Some(format!("http://{controller_addr}/worker-load-validation"))
        }
        "MANDOFORGE_WORKER_LOAD_VALIDATION_CONTROLLER_TOKEN" => {
            Some("worker-load-token".to_string())
        }
        _ => None,
    };

    let execution = execute_worker_load_validation_controller(
        &lookup,
        checked_at,
        &queue_backend,
        "queue",
        &k8s,
        &autoscaling,
    )
    .await
    .expect("worker load validation controller");

    assert_eq!(execution["status"], "validated");
    assert_eq!(execution["validation_id"], "worker-load-validation-1");
    assert_eq!(execution["load_validated"], true);
    assert_eq!(execution["isolated_worker_pool_configured"], true);
    let payloads = payloads.lock().await;
    assert_eq!(payloads.len(), 1);
    assert_eq!(payloads[0]["type"], "mandoforge.worker_load_validation");
    assert_eq!(payloads[0]["queue_backend"]["kind"], "nats_jetstream");
    assert_eq!(payloads[0]["worker_mode"], "queue");
    assert_eq!(
        payloads[0]["isolated_worker_pool_manifest_configured"],
        true
    );
    assert!(payloads[0]["secret"].is_null());

    controller_server.abort();
}

#[tokio::test]
async fn tenant_production_routing_controller_executes_external_boundary() {
    let payloads = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("tenant routing listener");
    let controller_addr = listener.local_addr().expect("tenant routing addr");
    let controller = Router::new()
        .route("/tenant-routing", post(mock_tenant_routing_controller))
        .with_state(payloads.clone());
    let controller_server = tokio::spawn(async move {
        axum::serve(listener, controller)
            .await
            .expect("mock tenant routing controller");
    });
    let checked_at = Utc::now();
    let readiness = ready_tenant_isolation_readiness(checked_at);
    let lookup = |key: &str| match key {
        "MANDOFORGE_TENANT_ROUTING_CONTROLLER_URL" => {
            Some(format!("http://{controller_addr}/tenant-routing"))
        }
        "MANDOFORGE_TENANT_ROUTING_CONTROLLER_TOKEN" => Some("tenant-routing-token".to_string()),
        _ => None,
    };

    let execution = execute_tenant_production_routing_controller(
        &lookup,
        Some("admin-1"),
        checked_at,
        &readiness,
    )
    .await
    .expect("tenant routing controller");

    assert_eq!(execution["status"], "validated");
    assert_eq!(execution["validation_id"], "tenant-routing-1");
    assert_eq!(execution["target_kind"], "multi_tenant_deployment");
    assert_eq!(execution["tenant_count"], 3);
    assert_eq!(execution["tenant_samples"], json!(["tenant-a", "tenant-b"]));
    assert_eq!(execution["rls_enforced"], true);
    assert_eq!(execution["rls_table_count"], 12);
    assert_eq!(execution["rls_forced_table_count"], 12);
    assert_eq!(execution["tenant_context_validated"], true);
    assert_eq!(execution["cross_tenant_negative_tests"], true);
    assert_eq!(execution["cross_tenant_negative_test_count"], 3);
    let payloads = payloads.lock().await;
    assert_eq!(payloads.len(), 1);
    assert_eq!(
        payloads[0]["type"],
        "mandoforge.tenant_production_routing_validation"
    );
    assert_eq!(payloads[0]["subject"], "admin-1");
    assert_eq!(payloads[0]["runtime_tenant_mode"], "tenant_routed");
    assert!(payloads[0]["secret"].is_null());

    controller_server.abort();
}

#[test]
fn tenant_production_routing_readiness_requires_controller_when_configured() {
    let generated_at = Utc::now();
    let rls = ready_tenant_rls_readiness();
    let missing_controller = build_tenant_production_routing_readiness(
        "tenant_routed",
        true,
        true,
        &rls,
        &[],
        true,
        false,
        generated_at,
    );
    assert_eq!(missing_controller.status, "blocked");
    assert!(missing_controller.blocking_reasons.iter().any(|reason| {
        reason == "tenant production routing controller is required but not configured"
    }));

    let validated_audit = new_audit_log(
        None,
        "user",
        None,
        "tenant.production_routing_validation_run",
        "tenant_isolation",
        None,
        json!({
            "status": "validated",
            "controller_configured": true,
            "controller_execution": {
                "attempted": true,
                "status": "validated",
                "validation_id": "tenant-routing-1"
            },
            "checked_at": generated_at,
        }),
    );
    let ready = build_tenant_production_routing_readiness(
        "tenant_routed",
        true,
        true,
        &rls,
        &[validated_audit],
        true,
        true,
        generated_at,
    );
    assert_eq!(ready.status, "ready");
    assert!(!ready.production_blocked);
    assert!(ready.latest_controller_validated);
    assert!(ready.controller_evidence_fresh);
    assert_eq!(ready.latest_controller_age_hours, Some(0));
    assert_eq!(ready.latest_controller_status.as_deref(), Some("validated"));

    let mut stale_audit = new_audit_log(
        None,
        "user",
        None,
        "tenant.production_routing_validation_run",
        "tenant_isolation",
        None,
        json!({
            "status": "validated",
            "controller_configured": true,
            "controller_execution": {
                "attempted": true,
                "status": "validated",
                "validation_id": "tenant-routing-stale"
            },
            "checked_at": generated_at - chrono::Duration::hours(25),
        }),
    );
    stale_audit.created_at = generated_at - chrono::Duration::hours(25);
    let stale = build_tenant_production_routing_readiness(
        "tenant_routed",
        true,
        true,
        &rls,
        &[stale_audit],
        true,
        true,
        generated_at,
    );
    assert_eq!(stale.status, "blocked");
    assert!(!stale.controller_evidence_fresh);
    assert_eq!(stale.latest_controller_age_hours, Some(25));
    assert!(
        stale
            .blocking_reasons
            .iter()
            .any(|reason| { reason == "tenant production routing controller evidence is stale" })
    );
}

#[test]
fn worker_load_validation_evidence_requires_controller_when_configured() {
    let generated_at = Utc::now();
    let env_only_audit = new_audit_log(
        None,
        "system",
        None,
        "worker.load_validation_run",
        "execution_worker",
        None,
        json!({
            "status": "validated",
            "load_validated": true,
            "isolated_worker_pool_configured": true,
            "controller_configured": false,
            "controller_execution": {
                "attempted": false,
                "status": "skipped"
            },
            "checked_at": generated_at,
        }),
    );
    let blocked = worker_load_validation_evidence_from_audit_logs(
        &[env_only_audit],
        generated_at,
        true,
        false,
        true,
    );
    assert_eq!(blocked.status, "attention");
    assert!(!blocked.load_validated);
    assert!(
        blocked
            .message
            .contains("controller is required but not configured")
    );

    let mut controller_audit = new_audit_log(
        None,
        "system",
        None,
        "worker.load_validation_run",
        "execution_worker",
        None,
        json!({
            "status": "validated",
            "load_validated": true,
            "isolated_worker_pool_configured": true,
            "controller_configured": true,
            "controller_execution": {
                "attempted": true,
                "status": "validated",
                "validation_id": "worker-load-validation-1"
            },
            "checked_at": generated_at,
        }),
    );
    controller_audit.created_at = generated_at;
    let ready = worker_load_validation_evidence_from_audit_logs(
        &[controller_audit],
        generated_at,
        true,
        true,
        true,
    );
    assert_eq!(ready.status, "validated");
    assert!(ready.load_validated);
    assert!(ready.latest_controller_validated);
    assert!(ready.controller_evidence_fresh);
    assert_eq!(ready.latest_controller_age_hours, Some(0));
    assert_eq!(ready.latest_controller_status.as_deref(), Some("validated"));

    let mut stale_controller_audit = new_audit_log(
        None,
        "system",
        None,
        "worker.load_validation_run",
        "execution_worker",
        None,
        json!({
            "status": "validated",
            "load_validated": true,
            "isolated_worker_pool_configured": true,
            "controller_configured": true,
            "controller_execution": {
                "attempted": true,
                "status": "validated",
                "validation_id": "worker-load-validation-stale"
            },
            "checked_at": generated_at - chrono::Duration::hours(25),
        }),
    );
    stale_controller_audit.created_at = generated_at - chrono::Duration::hours(25);
    let stale = worker_load_validation_evidence_from_audit_logs(
        &[stale_controller_audit],
        generated_at,
        true,
        true,
        true,
    );
    assert_eq!(stale.status, "attention");
    assert!(!stale.load_validated);
    assert!(stale.latest_controller_validated);
    assert!(!stale.controller_evidence_fresh);
    assert_eq!(stale.latest_controller_age_hours, Some(25));
    assert!(
        stale
            .message
            .contains("worker load validation controller evidence is stale")
    );
}

fn ready_tenant_rls_readiness() -> TenantIsolationRlsReadiness {
    TenantIsolationRlsReadiness {
        required_for_production: true,
        enabled: true,
        forced: true,
        migration_asset_present: true,
        tenant_context_configured: true,
        enabled_table_count: 2,
        forced_table_count: 2,
        tracked_table_count: 2,
        status: "configured".to_string(),
    }
}

fn ready_tenant_isolation_readiness(generated_at: DateTime<Utc>) -> TenantIsolationReadinessReport {
    let rls = ready_tenant_rls_readiness();
    let production_routing = build_tenant_production_routing_readiness(
        "tenant_routed",
        true,
        true,
        &rls,
        &[],
        false,
        true,
        generated_at,
    );
    TenantIsolationReadinessReport {
        generated_at,
        status: "ready".to_string(),
        readiness_score: 100,
        runtime_tenant_id: Uuid::new_v4(),
        runtime_tenant_mode: "tenant_routed".to_string(),
        header_fail_closed: true,
        membership_scope_enforced: true,
        production_routing,
        scoped_counts: TenantIsolationScopedCounts {
            organizations: 1,
            teams: 1,
            projects: 1,
            memberships: 1,
            invitations: 0,
        },
        table_coverage: vec![TenantIsolationTableCoverage {
            table: "agents".to_string(),
            tenant_id_required: true,
            store_filters_tenant: true,
            rls_required_for_production: true,
            rls_enabled: true,
            rls_forced: true,
        }],
        rls,
        attention_items: vec![],
        runbook_actions: vec![],
    }
}

async fn mock_worker_load_validation_controller(
    State(payloads): State<Arc<tokio::sync::Mutex<Vec<Value>>>>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Json<Value> {
    assert_eq!(
        headers
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer worker-load-token")
    );
    payloads.lock().await.push(payload);
    Json(json!({
        "status": "validated",
        "validation_id": "worker-load-validation-1",
        "message": "worker autoscaling and isolated pool validated",
        "target_kind": "k8s_cluster",
        "cluster_id": "test-cluster-1",
        "cluster_profile": "test-multi-node",
        "node_count": 3,
        "worker_pool": "mandoforge-worker-isolated",
        "load_validated": true,
        "isolated_worker_pool_configured": true,
        "observed_replicas": {
            "min": 1,
            "max": 4,
            "scaled_from": 1,
            "scaled_to": 3
        },
        "checks": [
            {"name": "queue_pressure", "status": "passed"},
            {"name": "keda_scale_out", "status": "passed"},
            {"name": "isolated_worker_pool", "status": "passed"}
        ]
    }))
}

async fn mock_tenant_routing_controller(
    State(payloads): State<Arc<tokio::sync::Mutex<Vec<Value>>>>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Json<Value> {
    assert_eq!(
        headers
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer tenant-routing-token")
    );
    payloads.lock().await.push(payload);
    Json(json!({
        "status": "validated",
        "validation_id": "tenant-routing-1",
        "message": "tenant routing target validated",
        "target_kind": "multi_tenant_deployment",
        "deployment_id": "tenant-routing-deployment-1",
        "environment": "enterprise-prod",
        "tenant_count": 3,
        "tenant_samples": ["tenant-a", "tenant-b"],
        "rls_enforced": true,
        "rls_table_count": 12,
        "rls_forced_table_count": 12,
        "tenant_context_validated": true,
        "cross_tenant_negative_tests": true,
        "cross_tenant_negative_test_count": 3,
        "checks": [
            {"name": "runtime_routing", "status": "passed"},
            {"name": "header_fail_closed", "status": "passed"},
            {"name": "rls_context", "status": "passed"}
        ]
    }))
}
