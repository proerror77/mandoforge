use super::*;

fn ready_approval_notification_routing(
    generated_at: DateTime<Utc>,
) -> ApprovalNotificationRoutingSummary {
    ApprovalNotificationRoutingSummary {
        status: "healthy".to_string(),
        generated_at,
        webhook_configured: true,
        slack_configured: false,
        email_relay_configured: false,
        channel_count: 1,
        persisted_policy_count: 1,
        active_policy_count: 1,
        channel_policies: vec![],
        pending_approval_count: 0,
        delegated_pending_count: 0,
        group_pending_count: 0,
        routable_pending_count: 0,
        unroutable_pending_count: 0,
        approval_group_count: 0,
        escalation_rule_count: 0,
        attention_items: vec![],
    }
}

#[tokio::test]
async fn approval_notification_deployment_controller_executes_external_boundary() {
    let payloads = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("approval notification deployment listener");
    let controller_addr = listener
        .local_addr()
        .expect("approval notification deployment addr");
    let controller = Router::new()
        .route(
            "/approval-notification-deployment",
            post(mock_approval_notification_deployment_controller),
        )
        .with_state(payloads.clone());
    let controller_server = tokio::spawn(async move {
        axum::serve(listener, controller)
            .await
            .expect("mock approval notification deployment controller");
    });
    let lookup = |key: &str| match key {
        "MANDOFORGE_APPROVAL_NOTIFICATION_DEPLOYMENT_CONTROLLER_URL" => Some(format!(
            "http://{controller_addr}/approval-notification-deployment"
        )),
        "MANDOFORGE_APPROVAL_NOTIFICATION_DEPLOYMENT_CONTROLLER_TOKEN" => {
            Some("approval-notification-token".to_string())
        }
        _ => None,
    };
    let routing = ApprovalNotificationRoutingSummary {
        status: "warning".to_string(),
        generated_at: Utc::now(),
        webhook_configured: true,
        slack_configured: false,
        email_relay_configured: false,
        channel_count: 1,
        persisted_policy_count: 1,
        active_policy_count: 1,
        channel_policies: vec![],
        pending_approval_count: 2,
        delegated_pending_count: 2,
        group_pending_count: 0,
        routable_pending_count: 2,
        unroutable_pending_count: 0,
        approval_group_count: 0,
        escalation_rule_count: 0,
        attention_items: vec![],
    };

    let execution = execute_approval_notification_deployment_controller(
        &lookup,
        "admin-1",
        Utc::now(),
        &routing,
    )
    .await
    .expect("approval notification deployment controller");

    assert_eq!(execution["status"], "validated");
    assert_eq!(
        execution["deployment_id"],
        "approval-notification-deployment-1"
    );
    let payloads = payloads.lock().await;
    assert_eq!(payloads.len(), 1);
    assert_eq!(
        payloads[0]["type"],
        "mandoforge.approval_notification_deployment"
    );
    assert_eq!(payloads[0]["subject"], "admin-1");
    assert_eq!(payloads[0]["routing"]["channel_count"], 1);
    assert_eq!(payloads[0]["routing"]["routable_pending_count"], 2);

    controller_server.abort();
}

#[tokio::test]
async fn approval_notification_ops_controller_executes_external_boundary() {
    let payloads = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("approval notification ops listener");
    let controller_addr = listener
        .local_addr()
        .expect("approval notification ops addr");
    let controller = Router::new()
        .route(
            "/approval-notification-ops",
            post(mock_approval_notification_ops_controller),
        )
        .with_state(payloads.clone());
    let controller_server = tokio::spawn(async move {
        axum::serve(listener, controller)
            .await
            .expect("mock approval notification ops controller");
    });
    let generated_at = Utc::now();
    let routing = ready_approval_notification_routing(generated_at);
    let delivery_audit = new_audit_log(
        None,
        "system",
        None,
        "approval.notification_delivery_run",
        "approval_notifications",
        None,
        json!({
            "status": "delivered",
            "subject": "admin-1",
            "delivered_count": 1,
            "reserved_count": 0,
            "failed_count": 0,
            "skipped_count": 0,
            "ran_at": generated_at,
        }),
    );
    let delivery_summary =
        build_approval_notification_delivery_run_summary(&[delivery_audit], generated_at, &routing);
    let lookup = |key: &str| match key {
        "MANDOFORGE_APPROVAL_NOTIFICATION_OPS_CONTROLLER_URL" => Some(format!(
            "http://{controller_addr}/approval-notification-ops"
        )),
        "MANDOFORGE_APPROVAL_NOTIFICATION_OPS_CONTROLLER_TOKEN" => {
            Some("approval-notification-ops-token".to_string())
        }
        _ => None,
    };

    let execution = execute_approval_notification_ops_controller(
        &lookup,
        Some("admin-1"),
        generated_at,
        &routing,
        &delivery_summary,
    )
    .await
    .expect("approval notification ops controller");

    assert_eq!(execution["status"], "validated");
    assert_eq!(execution["ops_id"], "approval-notification-ops-1");
    let payloads = payloads.lock().await;
    assert_eq!(payloads.len(), 1);
    assert_eq!(payloads[0]["type"], "mandoforge.approval_notification_ops");
    assert_eq!(payloads[0]["subject"], "admin-1");
    assert_eq!(payloads[0]["routing"]["channel_count"], 1);
    assert_eq!(payloads[0]["delivery"]["latest_run_status"], "delivered");
    assert!(payloads[0]["secret"].is_null());

    controller_server.abort();
}

#[test]
fn approval_notification_production_ops_requires_controller_when_configured() {
    let generated_at = Utc::now();
    let routing = ready_approval_notification_routing(generated_at);
    let delivery_run = ApprovalNotificationDeliveryRunRecord {
        id: Uuid::new_v4(),
        status: "delivered".to_string(),
        subject: Some("admin-1".to_string()),
        candidate_count: 1,
        delivered_count: 1,
        reserved_count: 0,
        failed_count: 0,
        skipped_count: 0,
        ran_at: generated_at,
    };
    let missing = build_approval_notification_production_ops_readiness(
        Some(&delivery_run),
        &routing,
        &[],
        generated_at,
        true,
        false,
    );
    assert_eq!(missing.status, "blocked");
    assert!(missing.production_blocked);
    assert!(missing.message.contains("controller is required"));

    let validated_audit = new_audit_log(
        None,
        "user",
        None,
        "approval.notification_ops_validation_run",
        "approval_notifications",
        None,
        json!({
            "status": "validated",
            "controller_configured": true,
            "controller_execution": {
                "attempted": true,
                "status": "validated",
                "ops_id": "approval-notification-ops-1"
            },
            "checked_at": generated_at,
        }),
    );
    let ready = build_approval_notification_production_ops_readiness(
        Some(&delivery_run),
        &routing,
        &[validated_audit],
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

    let mut stale_audit = new_audit_log(
        None,
        "user",
        None,
        "approval.notification_ops_validation_run",
        "approval_notifications",
        None,
        json!({
            "status": "validated",
            "controller_configured": true,
            "controller_execution": {
                "attempted": true,
                "status": "validated",
                "ops_id": "approval-notification-ops-stale"
            },
            "checked_at": generated_at - chrono::Duration::hours(25),
        }),
    );
    stale_audit.created_at = generated_at - chrono::Duration::hours(25);
    let stale = build_approval_notification_production_ops_readiness(
        Some(&delivery_run),
        &routing,
        &[stale_audit],
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
            .message
            .contains("approval notification ops controller evidence is stale")
    );
}

#[test]
fn approval_notification_deployment_readiness_requires_controller_when_configured() {
    let generated_at = Utc::now();
    let routing = ApprovalNotificationRoutingSummary {
        status: "healthy".to_string(),
        generated_at,
        webhook_configured: true,
        slack_configured: false,
        email_relay_configured: false,
        channel_count: 1,
        persisted_policy_count: 1,
        active_policy_count: 1,
        channel_policies: vec![],
        pending_approval_count: 0,
        delegated_pending_count: 0,
        group_pending_count: 0,
        routable_pending_count: 0,
        unroutable_pending_count: 0,
        approval_group_count: 0,
        escalation_rule_count: 0,
        attention_items: vec![],
    };
    let missing =
        build_approval_notification_deployment_readiness(&[], &routing, generated_at, true, false);
    assert_eq!(missing.status, "blocked");
    assert!(missing.production_blocked);
    assert!(missing.controller_required);
    assert!(!missing.controller_configured);
    assert!(missing.blocking_reasons.iter().any(|reason| {
        reason == "approval notification deployment controller is required but not configured"
    }));

    let stale_without_controller = build_approval_notification_deployment_readiness(
        &[new_audit_log(
            None,
            "user",
            None,
            "approval.notification_deployment_validation_run",
            "approval_notifications",
            None,
            json!({
                "status": "healthy",
                "pending_approval_count": 0,
                "routable_pending_count": 0,
                "unroutable_pending_count": 0,
                "channel_count": 1,
                "persisted_policy_count": 1,
                "active_policy_count": 1,
                "routing_status": "healthy",
                "controller_required": true,
                "controller_configured": true,
                "controller_execution": {
                    "attempted": false,
                    "status": "skipped",
                    "reason": "controller_not_configured"
                },
                "checked_at": generated_at,
            }),
        )],
        &routing,
        generated_at,
        true,
        true,
    );
    assert_eq!(stale_without_controller.status, "blocked");
    assert!(stale_without_controller.production_blocked);
    assert!(!stale_without_controller.latest_controller_validated);

    let mut controller_audit = new_audit_log(
        None,
        "user",
        None,
        "approval.notification_deployment_validation_run",
        "approval_notifications",
        None,
        json!({
            "status": "healthy",
            "pending_approval_count": 0,
            "routable_pending_count": 0,
            "unroutable_pending_count": 0,
            "channel_count": 1,
            "persisted_policy_count": 1,
            "active_policy_count": 1,
            "routing_status": "healthy",
            "controller_required": true,
            "controller_configured": true,
            "controller_execution": {
                "attempted": true,
                "status": "validated",
                "deployment_id": "approval-notification-deployment-1"
            },
            "checked_at": generated_at,
        }),
    );
    controller_audit.created_at = generated_at;
    let ready = build_approval_notification_deployment_readiness(
        &[controller_audit.clone()],
        &routing,
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
    let stale = build_approval_notification_deployment_readiness(
        &[stale_controller_audit],
        &routing,
        generated_at,
        true,
        true,
    );
    assert_eq!(stale.status, "blocked");
    assert!(stale.production_blocked);
    assert!(stale.latest_controller_validated);
    assert!(!stale.controller_evidence_fresh);
    assert_eq!(stale.latest_controller_age_hours, Some(25));
    assert!(stale.blocking_reasons.iter().any(|reason| {
        reason == "approval notification deployment controller evidence is stale"
    }));
}

async fn mock_approval_notification_deployment_controller(
    State(payloads): State<Arc<tokio::sync::Mutex<Vec<Value>>>>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Json<Value> {
    assert_eq!(
        headers
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer approval-notification-token")
    );
    assert_eq!(
        payload["type"],
        "mandoforge.approval_notification_deployment"
    );
    payloads.lock().await.push(payload);
    Json(json!({
        "status": "validated",
        "deployment_id": "approval-notification-deployment-1",
        "steps": ["routing-checked", "delivery-provider-checked"]
    }))
}

async fn mock_approval_notification_ops_controller(
    State(payloads): State<Arc<tokio::sync::Mutex<Vec<Value>>>>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Json<Value> {
    assert_eq!(
        headers
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer approval-notification-ops-token")
    );
    payloads.lock().await.push(payload);
    Json(json!({
        "status": "validated",
        "ops_id": "approval-notification-ops-1",
        "message": "approval notification production ops validated",
        "checks": [
            {"name": "routing", "status": "passed"},
            {"name": "delivery_run", "status": "passed"},
            {"name": "provider_delivery_ops", "status": "passed"}
        ]
    }))
}
