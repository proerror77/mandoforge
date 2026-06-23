use super::*;

#[tokio::test]
async fn scheduler_deployment_controller_executes_external_boundary() {
    let payloads = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("scheduler deployment listener");
    let controller_addr = listener.local_addr().expect("scheduler deployment addr");
    let controller = Router::new()
        .route(
            "/scheduler-deployment",
            post(mock_scheduler_deployment_controller),
        )
        .with_state(payloads.clone());
    let controller_server = tokio::spawn(async move {
        axum::serve(listener, controller)
            .await
            .expect("mock scheduler deployment controller");
    });
    let checked_at = Utc::now();
    let lookup = |key: &str| match key {
        "MANDOFORGE_SCHEDULER_DEPLOYMENT_CONTROLLER_URL" => {
            Some(format!("http://{controller_addr}/scheduler-deployment"))
        }
        "MANDOFORGE_SCHEDULER_DEPLOYMENT_CONTROLLER_TOKEN" => {
            Some("scheduler-deploy-token".to_string())
        }
        _ => None,
    };
    let readiness =
        scheduler_deployment_readiness_from_manifests(&[], checked_at, &|key| match key {
            "MANDOFORGE_SCHEDULER_TOKEN" => Some("scheduler-token".to_string()),
            _ => None,
        });

    let execution =
        execute_scheduler_deployment_controller(&lookup, Some("admin-1"), checked_at, &readiness)
            .await
            .expect("scheduler deployment controller");

    assert_eq!(execution["status"], "validated");
    let payloads = payloads.lock().await;
    assert_eq!(payloads.len(), 1);
    assert_eq!(
        payloads[0]["type"],
        "mandoforge.scheduler_deployment_validation"
    );
    assert_eq!(payloads[0]["subject"], "admin-1");
    assert_eq!(payloads[0]["readiness"]["status"], "ready");

    controller_server.abort();
}

#[test]
fn scheduler_deployment_readiness_requires_controller_when_configured() {
    let generated_at = Utc::now();
    let missing_controller =
        scheduler_deployment_readiness_from_manifests(&[], generated_at, &|key| match key {
            "MANDOFORGE_SCHEDULER_TOKEN" => Some("scheduler-token".to_string()),
            "MANDOFORGE_SCHEDULER_DEPLOYMENT_CONTROLLER_REQUIRED" => Some("true".to_string()),
            _ => None,
        });
    assert_eq!(missing_controller.status, "blocked");
    assert!(missing_controller.controller_required);
    assert!(!missing_controller.controller_configured);
    assert!(missing_controller.blocking_reasons.iter().any(|reason| {
        reason == "scheduler deployment controller is required but not configured"
    }));

    let mut audit = new_audit_log(
        None,
        "user",
        None,
        "scheduler.deployment_validation_run",
        "scheduler",
        None,
        json!({
            "status": "validated",
            "controller_required": true,
            "controller_configured": true,
            "controller_execution": {
                "attempted": true,
                "status": "validated"
            }
        }),
    );
    audit.created_at = generated_at;
    let ready =
        scheduler_deployment_readiness_from_manifests(&[audit.clone()], generated_at, &|key| {
            match key {
                "MANDOFORGE_SCHEDULER_TOKEN" => Some("scheduler-token".to_string()),
                "MANDOFORGE_SCHEDULER_DEPLOYMENT_CONTROLLER_REQUIRED" => Some("true".to_string()),
                "MANDOFORGE_SCHEDULER_DEPLOYMENT_CONTROLLER_URL" => {
                    Some("http://controller.example/validate".to_string())
                }
                _ => None,
            }
        });
    assert_eq!(ready.status, "ready");
    assert!(ready.latest_controller_validated);
    assert!(ready.controller_evidence_fresh);
    assert_eq!(ready.latest_controller_age_hours, Some(0));

    let mut stale_audit = audit.clone();
    stale_audit.created_at = generated_at - chrono::Duration::hours(25);
    let stale = scheduler_deployment_readiness_from_manifests(
        &[stale_audit],
        generated_at,
        &|key| match key {
            "MANDOFORGE_SCHEDULER_TOKEN" => Some("scheduler-token".to_string()),
            "MANDOFORGE_SCHEDULER_DEPLOYMENT_CONTROLLER_REQUIRED" => Some("true".to_string()),
            "MANDOFORGE_SCHEDULER_DEPLOYMENT_CONTROLLER_URL" => {
                Some("http://controller.example/validate".to_string())
            }
            _ => None,
        },
    );
    assert_eq!(stale.status, "blocked");
    assert!(stale.latest_controller_validated);
    assert!(!stale.controller_evidence_fresh);
    assert_eq!(stale.latest_controller_age_hours, Some(25));
    assert!(
        stale
            .blocking_reasons
            .iter()
            .any(|reason| { reason == "scheduler deployment controller evidence is stale" })
    );
}

async fn mock_scheduler_deployment_controller(
    State(payloads): State<Arc<tokio::sync::Mutex<Vec<Value>>>>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Json<Value> {
    assert_eq!(
        headers
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer scheduler-deploy-token")
    );
    payloads.lock().await.push(payload);
    Json(json!({
        "status": "validated",
        "deployment_id": "scheduler-deployment-1",
        "message": "scheduler deployment accepted",
        "checks": [
            {"name": "cronjob", "status": "validated"},
            {"name": "service_account", "status": "validated"},
            {"name": "shared_token", "status": "validated"}
        ]
    }))
}
