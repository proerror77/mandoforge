use super::*;

#[tokio::test]
async fn environment_resource_can_bind_session() {
    let app = test_app().await;
    let profile: AgentRuntimeProfile = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/agent-runtime-profiles",
            json!({
                "name": "codex-safe",
                "runtime_type": "agent_cli",
                "command": "codex",
                "default_args": ["exec", "--json"],
                "env": {},
                "timeout_seconds": 120,
                "remote_computer_required": false,
                "status": "enabled"
            }),
            &[
                ("x-mandoforge-subject", "admin-1"),
                ("x-mandoforge-roles", "admin"),
            ],
        ),
    )
    .await;

    let environment: Environment = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/environments",
            json!({
                "name": "Whiskey Remote",
                "environment_type": "remote_computer",
                "runtime_profile_id": profile.id,
                "remote_computer_profile": {"pool": "whiskey"},
                "worker_queue_binding": {"queue": "managed-agent"},
                "release_state": "active",
                "status": "enabled"
            }),
            &[
                ("x-mandoforge-subject", "admin-1"),
                ("x-mandoforge-roles", "admin"),
            ],
        ),
    )
    .await;
    assert_eq!(environment.environment_type, "remote_computer");
    assert_eq!(environment.runtime_profile_id, Some(profile.id));

    let agents: Vec<Agent> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/agents")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let session: Session = request_json(
        app.clone(),
        json_request(
            "POST",
            "/api/sessions",
            json!({
                "agent_id": agents[0].id,
                "environment_id": environment.id,
                "title": "environment bound run"
            }),
        ),
    )
    .await;
    assert_eq!(session.environment_id, Some(environment.id));

    let events: Vec<SessionEvent> = request_json(
        app.clone(),
        Request::builder()
            .uri(format!("/api/sessions/{}/events", session.id))
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(events.iter().any(|event| {
        event.event_type == "session.environment_bound"
            && event.payload["environment_id"] == json!(environment.id)
    }));

    let listed: Vec<Environment> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/environments")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(listed.iter().any(|item| item.id == environment.id));
}

#[tokio::test]
async fn session_rejects_unknown_environment() {
    let app = test_app().await;
    let agents: Vec<Agent> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/agents")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let (status, error) = request_value(
        app,
        json_request(
            "POST",
            "/api/sessions",
            json!({
                "agent_id": agents[0].id,
                "environment_id": Uuid::new_v4(),
                "title": "missing environment"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(
        error["error"]
            .as_str()
            .unwrap_or_default()
            .contains("environment not found")
    );
}
