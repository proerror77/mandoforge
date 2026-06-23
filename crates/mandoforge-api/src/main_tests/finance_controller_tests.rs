use super::*;

#[tokio::test]
async fn finance_close_controller_executes_external_boundary() {
    let payloads = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("finance close listener");
    let controller_addr = listener.local_addr().expect("finance close addr");
    let controller = Router::new()
        .route("/finance-close", post(mock_finance_close_controller))
        .with_state(payloads.clone());
    let controller_server = tokio::spawn(async move {
        axum::serve(listener, controller)
            .await
            .expect("mock finance close controller");
    });
    let generated_at = Utc::now();
    let before = ready_finance_operations_summary(generated_at, "attention");
    let after = ready_finance_operations_summary(generated_at, "ready");
    let lookup = |key: &str| match key {
        "MANDOFORGE_FINANCE_CLOSE_CONTROLLER_URL" => {
            Some(format!("http://{controller_addr}/finance-close"))
        }
        "MANDOFORGE_FINANCE_CLOSE_CONTROLLER_TOKEN" => Some("finance-token".to_string()),
        _ => None,
    };

    let execution = execute_usage_finance_close_controller(
        &lookup,
        Some("admin-1"),
        generated_at,
        &before,
        &after,
        true,
        None,
        None,
    )
    .await
    .expect("finance close controller");

    assert_eq!(execution["status"], "closed");
    assert_eq!(execution["close_id"], "finance-close-1");
    assert_eq!(execution["steps"].as_array().expect("steps").len(), 3);
    let payloads = payloads.lock().await;
    assert_eq!(payloads.len(), 1);
    assert_eq!(payloads[0]["type"], "mandoforge.finance_close");
    assert_eq!(payloads[0]["subject"], "admin-1");
    assert_eq!(payloads[0]["rollup_created"], true);
    assert_eq!(payloads[0]["after"]["production_close"]["status"], "ready");

    controller_server.abort();
}

#[tokio::test]
async fn finance_reconciliation_controller_executes_external_boundary() {
    let payloads = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("finance reconciliation listener");
    let controller_addr = listener.local_addr().expect("finance reconciliation addr");
    let controller = Router::new()
        .route(
            "/finance-reconciliation",
            post(mock_finance_reconciliation_controller),
        )
        .with_state(payloads.clone());
    let controller_server = tokio::spawn(async move {
        axum::serve(listener, controller)
            .await
            .expect("mock finance reconciliation controller");
    });
    let generated_at = Utc::now();
    let summary = ready_finance_operations_summary(generated_at, "ready");
    let lookup = |key: &str| match key {
        "MANDOFORGE_FINANCE_RECONCILIATION_CONTROLLER_URL" => {
            Some(format!("http://{controller_addr}/finance-reconciliation"))
        }
        "MANDOFORGE_FINANCE_RECONCILIATION_CONTROLLER_TOKEN" => {
            Some("finance-reconcile-token".to_string())
        }
        _ => None,
    };

    let execution = execute_usage_finance_reconciliation_controller(
        &lookup,
        Some("admin-1"),
        generated_at,
        &summary,
    )
    .await
    .expect("finance reconciliation controller");

    assert_eq!(execution["status"], "reconciled");
    assert_eq!(execution["reconciliation_id"], "finance-reconciliation-1");
    assert_eq!(execution["checks"].as_array().expect("checks").len(), 3);
    let payloads = payloads.lock().await;
    assert_eq!(payloads.len(), 1);
    assert_eq!(payloads[0]["type"], "mandoforge.finance_reconciliation");
    assert_eq!(payloads[0]["subject"], "admin-1");
    assert_eq!(
        payloads[0]["summary"]["production_close"]["status"],
        "ready"
    );
    assert!(payloads[0]["csv"].is_null());
    assert!(payloads[0]["secret"].is_null());

    controller_server.abort();
}

#[test]
fn finance_production_close_readiness_requires_controller_when_configured() {
    let generated_at = Utc::now();
    let mut dashboard = UsageFinanceDashboardSummary {
        generated_at,
        current_cost_cents: 0.0,
        current_total_tokens: 0,
        current_tool_calls: 0,
        comparison_basis: "current".to_string(),
        budget_pressure_status: "normal".to_string(),
        budget_pressure_count: 0,
        critical_budget_count: 0,
        warning_budget_count: 0,
        alert_count: 0,
        critical_alert_count: 0,
        warning_alert_count: 0,
        alert_route_count: 1,
        active_alert_route_count: 1,
        rollup_count: 1,
        latest_rollup_at: Some(generated_at),
        latest_rollup_age_hours: Some(0),
        finance_export_target_configured: true,
        finance_export_schedule_enabled: true,
        forecast_7d_cost_cents: Some(0.0),
        forecast_30d_cost_cents: Some(0.0),
        top_provider_by_cost: None,
        recommendations: vec![],
        attention_items: vec![],
    };
    let export = UsageFinanceOperationAudit {
        action: "usage.finance_export_delivered".to_string(),
        status: "delivered".to_string(),
        subject: Some("admin-1".to_string()),
        created_at: generated_at,
    };
    let delivery = UsageFinanceOperationAudit {
        action: "usage.cost_alerts_delivered".to_string(),
        status: "delivered".to_string(),
        subject: Some("admin-1".to_string()),
        created_at: generated_at,
    };

    let missing_controller = build_usage_finance_production_close_readiness(
        &dashboard,
        &[],
        &[],
        "fresh",
        "no_alerts",
        Some(&export),
        Some(&delivery),
        generated_at,
        true,
        false,
        false,
        false,
    );
    assert_eq!(missing_controller.status, "blocked");
    assert!(missing_controller.production_blocked);
    assert!(
        missing_controller
            .blocking_reasons
            .iter()
            .any(|reason| { reason == "finance close controller is required but not configured" })
    );

    let mut audit = new_audit_log(
        None,
        "user",
        None,
        "usage.finance_operations_run",
        "usage_finance_operations",
        None,
        json!({
            "status": "completed",
            "close_controller_configured": true,
            "close_controller_execution": {
                "attempted": true,
                "status": "closed",
                "close_id": "finance-close-1"
            }
        }),
    );
    audit.created_at = generated_at;
    dashboard.attention_items.clear();
    let ready = build_usage_finance_production_close_readiness(
        &dashboard,
        &[],
        &[audit.clone()],
        "fresh",
        "no_alerts",
        Some(&export),
        Some(&delivery),
        generated_at,
        true,
        true,
        false,
        false,
    );
    assert_eq!(ready.status, "ready");
    assert!(!ready.production_blocked);
    assert!(ready.latest_close_controller_closed);
    assert_eq!(
        ready.latest_close_controller_status.as_deref(),
        Some("closed")
    );
    assert_eq!(ready.latest_close_controller_age_hours, Some(0));
    assert!(ready.close_controller_evidence_fresh);

    let mut stale_audit = audit.clone();
    stale_audit.created_at = generated_at - chrono::Duration::hours(25);
    let stale = build_usage_finance_production_close_readiness(
        &dashboard,
        &[],
        &[stale_audit],
        "fresh",
        "no_alerts",
        Some(&export),
        Some(&delivery),
        generated_at,
        true,
        true,
        false,
        false,
    );
    assert_eq!(stale.status, "blocked");
    assert!(stale.latest_close_controller_closed);
    assert_eq!(stale.latest_close_controller_age_hours, Some(25));
    assert!(!stale.close_controller_evidence_fresh);
    assert!(
        stale
            .blocking_reasons
            .iter()
            .any(|reason| { reason == "finance close controller evidence is stale" })
    );
}

#[test]
fn finance_production_close_readiness_requires_reconciliation_when_configured() {
    let generated_at = Utc::now();
    let dashboard = UsageFinanceDashboardSummary {
        generated_at,
        current_cost_cents: 0.0,
        current_total_tokens: 0,
        current_tool_calls: 0,
        comparison_basis: "current".to_string(),
        budget_pressure_status: "normal".to_string(),
        budget_pressure_count: 0,
        critical_budget_count: 0,
        warning_budget_count: 0,
        alert_count: 0,
        critical_alert_count: 0,
        warning_alert_count: 0,
        alert_route_count: 1,
        active_alert_route_count: 1,
        rollup_count: 1,
        latest_rollup_at: Some(generated_at),
        latest_rollup_age_hours: Some(0),
        finance_export_target_configured: true,
        finance_export_schedule_enabled: true,
        forecast_7d_cost_cents: Some(0.0),
        forecast_30d_cost_cents: Some(0.0),
        top_provider_by_cost: None,
        recommendations: vec![],
        attention_items: vec![],
    };
    let export = UsageFinanceOperationAudit {
        action: "usage.finance_export_delivered".to_string(),
        status: "delivered".to_string(),
        subject: Some("admin-1".to_string()),
        created_at: generated_at,
    };
    let delivery = UsageFinanceOperationAudit {
        action: "usage.cost_alerts_delivered".to_string(),
        status: "delivered".to_string(),
        subject: Some("admin-1".to_string()),
        created_at: generated_at,
    };

    let missing_controller = build_usage_finance_production_close_readiness(
        &dashboard,
        &[],
        &[],
        "fresh",
        "no_alerts",
        Some(&export),
        Some(&delivery),
        generated_at,
        false,
        false,
        true,
        false,
    );
    assert_eq!(missing_controller.status, "blocked");
    assert!(missing_controller.blocking_reasons.iter().any(|reason| {
        reason == "finance reconciliation controller is required but not configured"
    }));

    let mut reconciled_audit = new_audit_log(
        None,
        "user",
        None,
        "usage.finance_reconciliation_run",
        "usage_finance_operations",
        None,
        json!({
            "status": "reconciled",
            "reconciliation_controller_configured": true,
            "reconciliation_controller_execution": {
                "attempted": true,
                "status": "reconciled",
                "reconciliation_id": "finance-reconciliation-1"
            }
        }),
    );
    reconciled_audit.created_at = generated_at;
    let ready = build_usage_finance_production_close_readiness(
        &dashboard,
        &[],
        &[reconciled_audit.clone()],
        "fresh",
        "no_alerts",
        Some(&export),
        Some(&delivery),
        generated_at,
        false,
        false,
        true,
        true,
    );
    assert_eq!(ready.status, "ready");
    assert!(!ready.production_blocked);
    assert!(ready.latest_reconciliation_reconciled);
    assert_eq!(
        ready.latest_reconciliation_status.as_deref(),
        Some("reconciled")
    );
    assert_eq!(ready.latest_reconciliation_age_hours, Some(0));
    assert!(ready.reconciliation_evidence_fresh);

    let mut stale_reconciled_audit = reconciled_audit.clone();
    stale_reconciled_audit.created_at = generated_at - chrono::Duration::hours(25);
    let stale = build_usage_finance_production_close_readiness(
        &dashboard,
        &[],
        &[stale_reconciled_audit],
        "fresh",
        "no_alerts",
        Some(&export),
        Some(&delivery),
        generated_at,
        false,
        false,
        true,
        true,
    );
    assert_eq!(stale.status, "blocked");
    assert!(stale.latest_reconciliation_reconciled);
    assert_eq!(stale.latest_reconciliation_age_hours, Some(25));
    assert!(!stale.reconciliation_evidence_fresh);
    assert!(
        stale
            .blocking_reasons
            .iter()
            .any(|reason| { reason == "finance reconciliation controller evidence is stale" })
    );
}

#[test]
fn finance_close_controller_can_bootstrap_its_required_evidence() {
    let generated_at = Utc::now();
    let dashboard = UsageFinanceDashboardSummary {
        generated_at,
        current_cost_cents: 0.0,
        current_total_tokens: 0,
        current_tool_calls: 0,
        comparison_basis: "current".to_string(),
        budget_pressure_status: "normal".to_string(),
        budget_pressure_count: 0,
        critical_budget_count: 0,
        warning_budget_count: 0,
        alert_count: 0,
        critical_alert_count: 0,
        warning_alert_count: 0,
        alert_route_count: 1,
        active_alert_route_count: 1,
        rollup_count: 1,
        latest_rollup_at: Some(generated_at),
        latest_rollup_age_hours: Some(0),
        finance_export_target_configured: true,
        finance_export_schedule_enabled: true,
        forecast_7d_cost_cents: Some(0.0),
        forecast_30d_cost_cents: Some(0.0),
        top_provider_by_cost: None,
        recommendations: vec![],
        attention_items: vec![],
    };
    let export = UsageFinanceOperationAudit {
        action: "usage.finance_export_delivered".to_string(),
        status: "delivered".to_string(),
        subject: Some("admin-1".to_string()),
        created_at: generated_at,
    };
    let delivery = UsageFinanceOperationAudit {
        action: "usage.cost_alerts_delivered".to_string(),
        status: "delivered".to_string(),
        subject: Some("admin-1".to_string()),
        created_at: generated_at,
    };

    let bootstrap_ready = build_usage_finance_production_close_readiness(
        &dashboard,
        &[],
        &[],
        "fresh",
        "no_alerts",
        Some(&export),
        Some(&delivery),
        generated_at,
        true,
        true,
        true,
        true,
    );
    assert_eq!(bootstrap_ready.status, "blocked");
    assert!(
        bootstrap_ready
            .blocking_reasons
            .iter()
            .any(|reason| { reason == "finance close controller has no recent closed evidence" })
    );
    assert!(bootstrap_ready.blocking_reasons.iter().any(|reason| {
        reason == "finance reconciliation controller has no recent reconciled evidence"
    }));
    assert!(finance_close_controller_prerequisites_ready(
        &bootstrap_ready
    ));

    let export_blocked = build_usage_finance_production_close_readiness(
        &dashboard,
        &[],
        &[],
        "fresh",
        "no_alerts",
        None,
        Some(&delivery),
        generated_at,
        true,
        true,
        true,
        true,
    );
    assert!(
        export_blocked
            .blocking_reasons
            .iter()
            .any(|reason| { reason == "finance export has no recent delivered audit evidence" })
    );
    assert!(!finance_close_controller_prerequisites_ready(
        &export_blocked
    ));
}

fn ready_finance_operations_summary(
    generated_at: DateTime<Utc>,
    status: &str,
) -> UsageFinanceOperationsSummary {
    UsageFinanceOperationsSummary {
        generated_at,
        status: status.to_string(),
        readiness_score: 100,
        open_alert_count: 0,
        acknowledged_alert_count: 0,
        unacknowledged_alert_count: 0,
        active_alert_route_count: 1,
        rollup_status: "fresh".to_string(),
        export_status: "delivered".to_string(),
        alert_delivery_status: "delivered".to_string(),
        last_finance_export: Some(UsageFinanceOperationAudit {
            action: "usage.finance_export_delivered".to_string(),
            status: "delivered".to_string(),
            subject: Some("admin-1".to_string()),
            created_at: generated_at,
        }),
        last_alert_delivery: Some(UsageFinanceOperationAudit {
            action: "usage.cost_alert_delivery_run".to_string(),
            status: "delivered".to_string(),
            subject: Some("admin-1".to_string()),
            created_at: generated_at,
        }),
        last_alert_acknowledgement: Some(UsageFinanceOperationAudit {
            action: "usage.alert_acknowledged".to_string(),
            status: "acknowledged".to_string(),
            subject: Some("admin-1".to_string()),
            created_at: generated_at,
        }),
        last_accounting_reconciliation: None,
        production_close: UsageFinanceProductionCloseReadiness {
            status: "ready".to_string(),
            production_blocked: false,
            rollup_fresh: true,
            export_target_configured: true,
            export_recent: true,
            alert_delivery_ready: true,
            critical_alerts_acknowledged: true,
            failed_delivery_evidence: false,
            close_controller_required: false,
            close_controller_configured: false,
            latest_close_controller_status: None,
            latest_close_controller_age_hours: None,
            close_controller_evidence_fresh: false,
            latest_close_controller_closed: false,
            reconciliation_controller_required: false,
            reconciliation_controller_configured: false,
            latest_reconciliation_status: None,
            latest_reconciliation_age_hours: None,
            reconciliation_evidence_fresh: false,
            latest_reconciliation_reconciled: false,
            blocking_reasons: vec![],
            message: "ready".to_string(),
        },
        runbook_actions: vec![],
        attention_items: vec![],
    }
}

async fn mock_finance_close_controller(
    State(payloads): State<Arc<tokio::sync::Mutex<Vec<Value>>>>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Json<Value> {
    assert_eq!(
        headers
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer finance-token")
    );
    payloads.lock().await.push(payload);
    Json(json!({
        "status": "closed",
        "close_id": "finance-close-1",
        "message": "finance close accepted",
        "steps": [
            {"name": "rollup", "status": "verified"},
            {"name": "export", "status": "delivered"},
            {"name": "alerts", "status": "acknowledged"}
        ]
    }))
}

async fn mock_finance_reconciliation_controller(
    State(payloads): State<Arc<tokio::sync::Mutex<Vec<Value>>>>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Json<Value> {
    assert_eq!(
        headers
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer finance-reconcile-token")
    );
    payloads.lock().await.push(payload);
    Json(json!({
        "status": "reconciled",
        "reconciliation_id": "finance-reconciliation-1",
        "message": "accounting system reconciliation accepted",
        "checks": [
            {"name": "rollup_totals", "status": "matched"},
            {"name": "export_delivery", "status": "matched"},
            {"name": "alert_acknowledgement", "status": "matched"}
        ]
    }))
}
