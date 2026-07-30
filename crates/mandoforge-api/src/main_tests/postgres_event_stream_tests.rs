use super::*;

fn tenant_request(method: &str, uri: &str, tenant_id: Uuid, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .header("x-mandoforge-subject", "admin-1")
        .header("x-mandoforge-roles", "admin")
        .header("x-mandoforge-tenant-id", tenant_id.to_string())
        .body(Body::from(body.to_string()))
        .expect("valid tenant request")
}

#[tokio::test]
#[ignore = "requires MANDOFORGE_TEST_POSTGRES_URL"]
async fn postgres_session_stream_delivers_live_events_without_cross_tenant_leakage() {
    let database_url = std::env::var("MANDOFORGE_TEST_POSTGRES_URL")
        .expect("MANDOFORGE_TEST_POSTGRES_URL is required");
    let bootstrap_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .expect("connect bootstrap postgres");
    run_migrations(&bootstrap_pool)
        .await
        .expect("run migrations");

    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();
    for (tenant_id, label) in [(tenant_a, "a"), (tenant_b, "b")] {
        sqlx::query("INSERT INTO tenants (id, name, slug) VALUES ($1, $2, $3)")
            .bind(tenant_id)
            .bind(format!("SSE tenant {label}"))
            .bind(format!("sse-{label}-{}", tenant_id.simple()))
            .execute(&bootstrap_pool)
            .await
            .expect("insert test tenant");
    }
    drop(bootstrap_pool);

    let tenant_setting = format!("SET mandoforge.tenant_id = '{tenant_a}'");
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .after_connect(move |connection, _| {
            let tenant_setting = tenant_setting.clone();
            Box::pin(async move {
                connection.execute(tenant_setting.as_str()).await?;
                Ok(())
            })
        })
        .before_acquire(move |connection, _| {
            Box::pin(async move {
                let tenant_id = current_request_tenant_id(tenant_a);
                connection
                    .execute(format!("SET mandoforge.tenant_id = '{tenant_id}'").as_str())
                    .await?;
                Ok(true)
            })
        })
        .connect(&database_url)
        .await
        .expect("connect tenant-routed postgres");

    let mut state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    state.process_role = ProcessRole::Api;
    state.store = StoreBackend::Postgres(pool.clone());
    state.execution_queue = ExecutionQueue::postgres(pool, tenant_a);
    state.tenant_id = tenant_a;
    state.tenant_runtime_mode = TenantRuntimeMode::TenantRouted;
    let app = build_router(state);

    let agent_a: Agent = request_json(
        app.clone(),
        tenant_request("POST", "/api/agents", tenant_a, json!({"name": "SSE A"})),
    )
    .await;
    let agent_b: Agent = request_json(
        app.clone(),
        tenant_request("POST", "/api/agents", tenant_b, json!({"name": "SSE B"})),
    )
    .await;
    let session_a: Session = request_json(
        app.clone(),
        tenant_request(
            "POST",
            "/api/sessions",
            tenant_a,
            json!({"agent_id": agent_a.id, "title": "SSE tenant A"}),
        ),
    )
    .await;
    let session_b: Session = request_json(
        app.clone(),
        tenant_request(
            "POST",
            "/api/sessions",
            tenant_b,
            json!({"agent_id": agent_b.id, "title": "SSE tenant B"}),
        ),
    )
    .await;
    let baseline: Vec<SessionEvent> = request_json(
        app.clone(),
        tenant_request(
            "POST",
            &format!("/api/sessions/{}/events", session_a.id),
            tenant_a,
            json!({"events": [{"type": "user.message", "payload": {"message": "baseline-a"}}]}),
        ),
    )
    .await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/sessions/{}/stream?after_seq={}",
                    session_a.id, baseline[0].seq
                ))
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .header("x-mandoforge-tenant-id", tenant_a.to_string())
                .body(Body::empty())
                .expect("valid stream request"),
        )
        .await
        .expect("open postgres stream");
    assert_eq!(response.status(), StatusCode::OK);

    let _: Vec<SessionEvent> = request_json(
        app.clone(),
        tenant_request(
            "POST",
            &format!("/api/sessions/{}/events", session_b.id),
            tenant_b,
            json!({"events": [{"type": "user.message", "payload": {"message": "tenant-b-secret"}}]}),
        ),
    )
    .await;
    let live_a: Vec<SessionEvent> = request_json(
        app,
        tenant_request(
            "POST",
            &format!("/api/sessions/{}/events", session_a.id),
            tenant_a,
            json!({"events": [{"type": "user.message", "payload": {"message": "tenant-a-live"}}]}),
        ),
    )
    .await;

    let body = sse_response_until_contains(
        response,
        &[&format!("id: {}\n", live_a[0].seq), "tenant-a-live"],
    )
    .await;
    assert!(body.contains("tenant-a-live"));
    assert!(!body.contains("tenant-b-secret"));
}
