use super::*;

#[tokio::test]
async fn github_binding_routes_require_admin_and_secret_reference() {
    let app = test_app().await;
    let viewer_headers = [
        ("x-mandoforge-subject", "viewer-1"),
        ("x-mandoforge-roles", "viewer"),
    ];
    let (list_status, _) = request_value(
        app.clone(),
        Request::builder()
            .uri("/api/github/project-bindings")
            .header("x-mandoforge-subject", "viewer-1")
            .header("x-mandoforge-roles", "viewer")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert_eq!(list_status, StatusCode::FORBIDDEN);

    let (write_status, _) = request_value(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/github/project-bindings",
            json!({
                "repo_full_name": "example/private",
                "pack_installation_id": Uuid::new_v4(),
                "webhook_secret_ref": "GITHUB_WEBHOOK_SECRET"
            }),
            &viewer_headers,
        ),
    )
    .await;
    assert_eq!(write_status, StatusCode::FORBIDDEN);

    let (missing_secret_status, error) = request_value(
        app,
        json_request_with_headers(
            "POST",
            "/api/github/project-bindings",
            json!({
                "repo_full_name": "example/private",
                "pack_installation_id": Uuid::new_v4()
            }),
            &[
                ("x-mandoforge-subject", "admin-1"),
                ("x-mandoforge-roles", "admin"),
            ],
        ),
    )
    .await;
    assert_eq!(missing_secret_status, StatusCode::BAD_REQUEST);
    assert_eq!(error["error"], json!("webhook_secret_ref is required"));
}

#[tokio::test]
async fn github_webhook_missing_secret_fails_closed_without_workflow_run() {
    let _env = env_lock().lock().expect("env lock");
    let secret_name = "MANDOFORGE_TEST_MISSING_GITHUB_WEBHOOK_SECRET";
    let _secret = EnvVarGuard::remove(secret_name);
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let now = Utc::now();
    state
        .upsert_project_github_binding(ProjectGitHubBinding {
            id: Uuid::new_v4(),
            repo_full_name: "example/private".to_string(),
            pack_installation_id: Uuid::new_v4(),
            webhook_secret_ref: secret_name.to_string(),
            active: true,
            created_at: now,
            updated_at: now,
        })
        .await
        .expect("store binding");
    let app = build_router(state.clone());

    let (status, error) = request_value(
        app,
        Request::builder()
            .method("POST")
            .uri("/api/webhooks/github")
            .header("content-type", "application/json")
            .header("x-github-event", "issues")
            .body(Body::from(
                json!({"repository": {"full_name": "example/private"}}).to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(error["error"], json!("webhook secret not configured"));
    assert!(
        state
            .list_workflow_runs()
            .await
            .expect("list workflow runs")
            .is_empty()
    );
}
