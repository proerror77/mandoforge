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
async fn execution_job_environment_is_snapshotted_at_enqueue() {
    let state = test_state_with_worker(Arc::new(QueueBackedExecutionWorker));
    state.seed_demo_agent().await.expect("seed demo agent");
    let app = build_router(state.clone());
    let agents = state.list_agents().await.expect("list seeded agents");
    let agent = agents.first().expect("seeded agent");
    let environment_a: Environment = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/environments",
            json!({
                "name": "Snapshot Queue A",
                "environment_type": "local",
                "worker_queue_binding": {"queue": "snapshot-a"},
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
    let environment_b: Environment = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/environments",
            json!({
                "name": "Snapshot Queue B",
                "environment_type": "local",
                "worker_queue_binding": {"queue": "snapshot-b"},
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
    let session: Session = request_json(
        app.clone(),
        json_request(
            "POST",
            "/api/sessions",
            json!({
                "agent_id": agent.id,
                "environment_id": environment_a.id,
                "title": "execution environment snapshot"
            }),
        ),
    )
    .await;

    let approval_required: Value = request_json(
        app.clone(),
        json_request(
            "POST",
            "/api/tools/file.write/execute",
            json!({
                "session_id": session.id,
                "args": {
                    "path": "snapshot.md",
                    "content": "queued"
                }
            }),
        ),
    )
    .await;
    let approval_id = approval_required["approval_id"]
        .as_str()
        .expect("approval id");
    let approved: Approval = request_json(
        app.clone(),
        approve_request(format!("/api/approvals/{approval_id}/approve")),
    )
    .await;
    assert_eq!(approved.status, "approved");

    if let StoreBackend::Memory(inner) = &state.store {
        let mut store = inner.write().await;
        store
            .sessions
            .get_mut(&session.id)
            .expect("stored session")
            .environment_id = Some(environment_b.id);
    }

    let execution_jobs_for_a: Vec<Value> = request_json(
        app,
        Request::builder()
            .uri("/api/execution-jobs")
            .header("x-mandoforge-subject", "worker-a")
            .header("x-mandoforge-roles", "admin")
            .header("x-mandoforge-environment-id", environment_a.id.to_string())
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(
        execution_jobs_for_a.iter().any(|job| {
            job["session_id"] == json!(session.id)
                && job["environment_id"] == json!(environment_a.id)
        }),
        "execution job must keep the enqueue-time environment snapshot: {execution_jobs_for_a:?}"
    );
}

#[tokio::test]
async fn worker_scope_required_blocks_unscoped_environment_job_claim() {
    let _scope_required = EnvVarGuard::set("MANDOFORGE_REQUIRE_WORKER_ENVIRONMENT_SCOPE", "1");
    let state = test_state_with_worker(Arc::new(QueueBackedExecutionWorker));
    state.seed_demo_agent().await.expect("seed demo agent");
    let app = build_router(state.clone());
    let agents = state.list_agents().await.expect("list seeded agents");
    let agent = agents.first().expect("seeded agent");
    let environment: Environment = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/environments",
            json!({
                "name": "Scoped Worker Queue",
                "environment_type": "local",
                "worker_queue_binding": {"queue": "scoped-worker"},
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
    let session: Session = request_json(
        app.clone(),
        json_request(
            "POST",
            "/api/sessions",
            json!({
                "agent_id": agent.id,
                "environment_id": environment.id,
                "title": "scoped worker claim"
            }),
        ),
    )
    .await;

    let approval_required: Value = request_json(
        app.clone(),
        json_request(
            "POST",
            "/api/tools/file.write/execute",
            json!({
                "session_id": session.id,
                "args": {
                    "path": "scoped.md",
                    "content": "queued"
                }
            }),
        ),
    )
    .await;
    let approval_id = approval_required["approval_id"]
        .as_str()
        .expect("approval id");
    let approved: Approval = request_json(
        app.clone(),
        approve_request(format!("/api/approvals/{approval_id}/approve")),
    )
    .await;
    let job = state
        .execution_queue
        .list()
        .await
        .expect("execution jobs")
        .into_iter()
        .find(|job| job.approval_id == approved.id)
        .expect("approved execution job");

    let (status, body) = request_value(
        app,
        Request::builder()
            .method("POST")
            .uri(format!("/api/execution-jobs/{}/run", job.id))
            .header("x-mandoforge-subject", "worker-1")
            .header("x-mandoforge-roles", "worker")
            .header("x-mandoforge-worker-id", "worker-1")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body["error"]
            .as_str()
            .is_some_and(|message| message.contains("x-mandoforge-environment-id"))
    );
}

#[tokio::test]
async fn worker_environment_header_filters_session_loop_and_execution_jobs() {
    let app = test_app_with_worker(Arc::new(QueueBackedExecutionWorker)).await;
    let agents: Vec<Agent> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/agents")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let agent = agents.first().expect("seeded agent");
    let environment_a: Environment = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/environments",
            json!({
                "name": "Worker Queue A",
                "environment_type": "local",
                "worker_queue_binding": {"queue": "worker-a"},
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
    let environment_b: Environment = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/environments",
            json!({
                "name": "Worker Queue B",
                "environment_type": "local",
                "worker_queue_binding": {"queue": "worker-b"},
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

    let session_a: Session = request_json(
        app.clone(),
        json_request(
            "POST",
            "/api/sessions",
            json!({
                "agent_id": agent.id,
                "environment_id": environment_a.id,
                "title": "queue A session"
            }),
        ),
    )
    .await;
    let session_b: Session = request_json(
        app.clone(),
        json_request(
            "POST",
            "/api/sessions",
            json!({
                "agent_id": agent.id,
                "environment_id": environment_b.id,
                "title": "queue B session"
            }),
        ),
    )
    .await;

    for session in [&session_a, &session_b] {
        let _: Vec<SessionEvent> = request_json(
            app.clone(),
            json_request(
                "POST",
                &format!("/api/sessions/{}/events", session.id),
                json!({
                    "events": [{
                        "type": "user.message",
                        "payload": {"message": "start loop"}
                    }]
                }),
            ),
        )
        .await;
    }

    let loop_jobs_for_a: Vec<SessionLoopJob> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/session-loop-jobs")
            .header("x-mandoforge-subject", "worker-a")
            .header("x-mandoforge-roles", "admin")
            .header("x-mandoforge-environment-id", environment_a.id.to_string())
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(
        loop_jobs_for_a
            .iter()
            .all(|job| job.environment_id == Some(environment_a.id)),
        "environment A worker should only see A loop jobs: {loop_jobs_for_a:?}"
    );
    assert!(
        loop_jobs_for_a
            .iter()
            .any(|job| job.session_id == session_a.id)
    );
    assert!(
        !loop_jobs_for_a
            .iter()
            .any(|job| job.session_id == session_b.id)
    );
    let loop_jobs_for_pool_a: Vec<SessionLoopJob> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/session-loop-jobs")
            .header("x-mandoforge-subject", "worker-a")
            .header("x-mandoforge-roles", "admin")
            .header("x-mandoforge-worker-pool", "worker-a")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(
        loop_jobs_for_pool_a
            .iter()
            .any(|job| job.session_id == session_a.id)
    );
    assert!(
        !loop_jobs_for_pool_a
            .iter()
            .any(|job| job.session_id == session_b.id),
        "worker pool A should not see B loop jobs: {loop_jobs_for_pool_a:?}"
    );
    let loop_job_a = loop_jobs_for_a
        .iter()
        .find(|job| job.session_id == session_a.id)
        .expect("environment A loop job");
    let completed_loop_a: SessionLoopJob = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/session-loop-jobs/{}/run", loop_job_a.id))
            .header("x-mandoforge-worker-id", "k-agent-worker-a")
            .header("x-mandoforge-subject", "worker-a")
            .header("x-mandoforge-roles", "admin")
            .header("x-mandoforge-environment-id", environment_a.id.to_string())
            .header("x-mandoforge-worker-pool", "worker-a")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert_eq!(completed_loop_a.status, SessionLoopJobStatus::Completed);
    assert_eq!(
        completed_loop_a.worker_id.as_deref(),
        Some("k-agent-worker-a")
    );
    let events_a: Vec<SessionEvent> = request_json(
        app.clone(),
        Request::builder()
            .uri(format!("/api/sessions/{}/events", session_a.id))
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(events_a.iter().any(|event| {
        event.event_type == "k_agent.claimed"
            && event.payload["session_loop_job_id"] == json!(loop_job_a.id)
            && event.payload["environment_id"] == json!(environment_a.id)
            && event.payload["worker_id"] == json!("k-agent-worker-a")
            && event.payload["worker_pool"] == json!("worker-a")
            && event.payload["lease_expires_at"].as_str().is_some()
            && event.payload["dispatch_surface"] == json!("session_loop_job")
            && event.payload["authority_boundary"] == json!("environment_scheduling_only")
    }));
    let completed_event = events_a
        .iter()
        .find(|event| event.event_type == "k_agent.completed")
        .expect("k_agent.completed event");
    assert_eq!(
        completed_event.payload["session_loop_job_id"],
        json!(loop_job_a.id)
    );
    assert_eq!(
        completed_event.payload["environment_id"],
        json!(environment_a.id)
    );
    assert_eq!(
        completed_event.payload["worker_id"],
        json!("k-agent-worker-a")
    );
    assert_eq!(completed_event.payload["worker_pool"], json!("worker-a"));
    assert_eq!(
        completed_event.payload["attempt_count"],
        json!(completed_loop_a.attempt_count)
    );
    assert_eq!(
        completed_event.payload["pending_event_seq_start"],
        json!(completed_loop_a.pending_event_seq_start)
    );
    assert_eq!(
        completed_event.payload["pending_event_seq_end"],
        json!(completed_loop_a.pending_event_seq_end)
    );
    assert_eq!(
        completed_event.payload["processed_event_seq"],
        json!(completed_loop_a.processed_event_seq)
    );
    assert_eq!(completed_event.payload["job_status"], json!("completed"));
    assert_eq!(
        completed_event.payload["session_status"],
        json!("requires_action")
    );
    assert_eq!(
        completed_event.payload["dispatch_surface"],
        json!("session_loop_job")
    );
    assert_eq!(
        completed_event.payload["authority_boundary"],
        json!("environment_scheduling_only")
    );
    assert_eq!(
        completed_event.payload["return_contract"],
        json!("session_loop_job_return_evidence")
    );

    let loop_job_b = session_loop_jobs_for_session(app.clone(), session_b.id)
        .await
        .into_iter()
        .find(|job| job.status == SessionLoopJobStatus::Queued)
        .expect("environment B loop job");
    let (status, error) = request_value(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/session-loop-jobs/{}/run", loop_job_b.id))
            .header("x-mandoforge-worker-id", "worker-a")
            .header("x-mandoforge-subject", "worker-a")
            .header("x-mandoforge-roles", "admin")
            .header("x-mandoforge-environment-id", environment_a.id.to_string())
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        error["error"],
        json!("job not claimable for worker environment")
    );
    let (status, error) = request_value(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/session-loop-jobs/{}/run", loop_job_b.id))
            .header("x-mandoforge-worker-id", "worker-pool-a")
            .header("x-mandoforge-subject", "worker-pool-a")
            .header("x-mandoforge-roles", "admin")
            .header("x-mandoforge-worker-pool", "worker-a")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(error["error"], json!("job not claimable for worker pool"));

    for session in [&session_a, &session_b] {
        let approval_required: Value = request_json(
            app.clone(),
            json_request(
                "POST",
                "/api/tools/file.write/execute",
                json!({
                    "session_id": session.id,
                    "args": {
                        "path": "environment-queue.md",
                        "content": "queued"
                    }
                }),
            ),
        )
        .await;
        let approval_id = approval_required["approval_id"]
            .as_str()
            .expect("approval id");
        let approved: Approval = request_json(
            app.clone(),
            approve_request(format!("/api/approvals/{approval_id}/approve")),
        )
        .await;
        assert_eq!(approved.status, "approved");
    }

    let execution_jobs_for_a: Vec<execution_queue::ExecutionJob> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/execution-jobs")
            .header("x-mandoforge-subject", "worker-a")
            .header("x-mandoforge-roles", "admin")
            .header("x-mandoforge-environment-id", environment_a.id.to_string())
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(
        execution_jobs_for_a
            .iter()
            .all(|job| job.environment_id == Some(environment_a.id)),
        "environment A worker should only see A execution jobs: {execution_jobs_for_a:?}"
    );
    assert!(
        execution_jobs_for_a
            .iter()
            .any(|job| job.session_id == session_a.id)
    );
    assert!(
        !execution_jobs_for_a
            .iter()
            .any(|job| job.session_id == session_b.id),
        "environment A worker should not see B execution jobs: {execution_jobs_for_a:?}"
    );
    let execution_jobs_for_pool_a: Vec<execution_queue::ExecutionJob> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/execution-jobs")
            .header("x-mandoforge-subject", "worker-a")
            .header("x-mandoforge-roles", "admin")
            .header("x-mandoforge-worker-pool", "worker-a")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(
        execution_jobs_for_pool_a
            .iter()
            .all(|job| job.environment_id == Some(environment_a.id)),
        "worker pool A should only see execution jobs bound to A: {execution_jobs_for_pool_a:?}"
    );
    assert!(
        execution_jobs_for_pool_a
            .iter()
            .any(|job| job.session_id == session_a.id)
    );
    assert!(
        !execution_jobs_for_pool_a
            .iter()
            .any(|job| job.session_id == session_b.id),
        "worker pool A should not see B execution jobs: {execution_jobs_for_pool_a:?}"
    );

    let execution_job_a = execution_jobs_for_a
        .iter()
        .find(|job| job.session_id == session_a.id)
        .expect("environment A execution job");
    let completed_execution_job_a: execution_queue::ExecutionJob = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/execution-jobs/{}/run", execution_job_a.id))
            .header("x-mandoforge-worker-id", "k-agent-execution-worker-a")
            .header("x-mandoforge-subject", "worker-a")
            .header("x-mandoforge-roles", "admin")
            .header("x-mandoforge-environment-id", environment_a.id.to_string())
            .header("x-mandoforge-worker-pool", "worker-a")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert_eq!(
        completed_execution_job_a.status,
        ExecutionJobStatus::Completed
    );

    let execution_events_a: Vec<SessionEvent> = request_json(
        app.clone(),
        Request::builder()
            .uri(format!("/api/sessions/{}/events", session_a.id))
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let execution_claimed_event = execution_events_a
        .iter()
        .find(|event| {
            event.event_type == "k_agent.claimed"
                && event.payload["execution_job_id"] == json!(execution_job_a.id)
        })
        .expect("execution job k_agent.claimed event");
    assert_eq!(
        execution_claimed_event.payload["environment_id"],
        json!(environment_a.id)
    );
    assert_eq!(
        execution_claimed_event.payload["worker_id"],
        json!("k-agent-execution-worker-a")
    );
    assert_eq!(
        execution_claimed_event.payload["worker_pool"],
        json!("worker-a")
    );
    assert_eq!(
        execution_claimed_event.payload["job_status"],
        json!("running")
    );
    assert_eq!(
        execution_claimed_event.payload["attempt_count"],
        json!(completed_execution_job_a.attempt_count)
    );
    assert!(execution_claimed_event.payload["lease_expires_at"].is_string());
    assert_eq!(
        execution_claimed_event.payload["dispatch_surface"],
        json!("execution_job")
    );
    assert_eq!(
        execution_claimed_event.payload["authority_boundary"],
        json!("environment_scheduling_only")
    );
    assert_eq!(
        execution_claimed_event.payload["return_contract"],
        json!("execution_job_return_evidence")
    );
    let execution_completed_event = execution_events_a
        .iter()
        .find(|event| {
            event.event_type == "k_agent.completed"
                && event.payload["execution_job_id"] == json!(execution_job_a.id)
        })
        .expect("execution job k_agent.completed event");
    assert_eq!(
        execution_completed_event.payload["job_status"],
        json!("completed")
    );
    assert_eq!(
        execution_completed_event.payload["tool"],
        json!(execution_job_a.tool_name)
    );
    assert_eq!(
        execution_completed_event.payload["attempt_count"],
        json!(completed_execution_job_a.attempt_count)
    );
    assert_eq!(
        execution_completed_event.payload["dispatch_surface"],
        json!("execution_job")
    );
    assert_eq!(
        execution_completed_event.payload["authority_boundary"],
        json!("environment_scheduling_only")
    );
    assert_eq!(
        execution_completed_event.payload["return_contract"],
        json!("execution_job_return_evidence")
    );
    let k_agent_execution_event_count = execution_events_a
        .iter()
        .filter(|event| {
            event.payload["execution_job_id"] == json!(execution_job_a.id)
                && event.event_type.starts_with("k_agent.")
        })
        .count();
    let (status, error) = request_value(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/execution-jobs/{}/run", execution_job_a.id))
            .header(
                "x-mandoforge-worker-id",
                "k-agent-execution-worker-a-duplicate",
            )
            .header("x-mandoforge-subject", "worker-a")
            .header("x-mandoforge-roles", "admin")
            .header("x-mandoforge-environment-id", environment_a.id.to_string())
            .header("x-mandoforge-worker-pool", "worker-a")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(error["error"], json!("execution job not found"));
    let execution_events_after_duplicate: Vec<SessionEvent> = request_json(
        app.clone(),
        Request::builder()
            .uri(format!("/api/sessions/{}/events", session_a.id))
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert_eq!(
        execution_events_after_duplicate
            .iter()
            .filter(|event| {
                event.payload["execution_job_id"] == json!(execution_job_a.id)
                    && event.event_type.starts_with("k_agent.")
            })
            .count(),
        k_agent_execution_event_count,
        "duplicate execution job claim should not append fake K Agent evidence"
    );

    let execution_job_b = request_json::<Vec<execution_queue::ExecutionJob>>(
        app.clone(),
        Request::builder()
            .uri("/api/execution-jobs")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await
    .into_iter()
    .find(|job| job.session_id == session_b.id)
    .expect("environment B execution job");
    let (status, error) = request_value(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/execution-jobs/{}/run", execution_job_b.id))
            .header("x-mandoforge-worker-id", "worker-a")
            .header("x-mandoforge-subject", "worker-a")
            .header("x-mandoforge-roles", "admin")
            .header("x-mandoforge-environment-id", environment_a.id.to_string())
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        error["error"],
        json!("job not claimable for worker environment")
    );
    let (status, error) = request_value(
        app,
        Request::builder()
            .method("POST")
            .uri(format!("/api/execution-jobs/{}/run", execution_job_b.id))
            .header("x-mandoforge-worker-id", "worker-pool-a")
            .header("x-mandoforge-subject", "worker-pool-a")
            .header("x-mandoforge-roles", "admin")
            .header("x-mandoforge-worker-pool", "worker-a")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(error["error"], json!("job not claimable for worker pool"));
}

#[tokio::test]
async fn k_agent_failed_event_preserves_session_loop_cursor_on_runtime_failure() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    state.seed_demo_agent().await.expect("seed demo agent");
    let app = build_router(state.clone());

    let provider: ProviderRecord = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/providers")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({
                    "provider_type": "mock",
                    "name": "openai-compatible",
                    "default_model": "gpt-5.5-mini",
                    "config": {}
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    let disabled: ProviderRecord = request_json(
        app.clone(),
        Request::builder()
            .method("PATCH")
            .uri(format!("/api/providers/{}/status", provider.id))
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({
                    "status": "disabled",
                    "emergency": true,
                    "reason": "force session-loop K Agent failure evidence"
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    assert_eq!(disabled.status, "disabled");

    let agents = state.list_agents().await.expect("list seeded agents");
    let agent = agents.first().expect("seeded agent");
    let environment: Environment = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/environments",
            json!({
                "name": "K Agent Failure Return",
                "environment_type": "local",
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
    let session: Session = request_json(
        app.clone(),
        json_request(
            "POST",
            "/api/sessions",
            json!({
                "agent_id": agent.id,
                "environment_id": environment.id,
                "title": "failed K Agent return evidence"
            }),
        ),
    )
    .await;
    let _: Vec<SessionEvent> = request_json(
        app.clone(),
        json_request(
            "POST",
            &format!("/api/sessions/{}/events", session.id),
            json!({
                "events": [{
                    "type": "user.message",
                    "payload": {"message": "trigger disabled provider"}
                }]
            }),
        ),
    )
    .await;

    let loop_job = session_loop_jobs_for_session(app.clone(), session.id)
        .await
        .into_iter()
        .find(|job| job.status == SessionLoopJobStatus::Queued)
        .expect("queued session loop job");
    let (status, error) = request_value(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/session-loop-jobs/{}/run", loop_job.id))
            .header("x-mandoforge-worker-id", "k-agent-failure-worker")
            .header("x-mandoforge-subject", "worker-a")
            .header("x-mandoforge-roles", "admin")
            .header("x-mandoforge-environment-id", environment.id.to_string())
            .header("x-mandoforge-worker-pool", "managed-agent")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(
        error["error"]
            .as_str()
            .unwrap_or_default()
            .contains("provider openai-compatible is not active")
    );

    let failed_loop_job = session_loop_jobs_for_session(app.clone(), session.id)
        .await
        .into_iter()
        .find(|job| job.id == loop_job.id)
        .expect("failed session loop job");
    assert_eq!(failed_loop_job.status, SessionLoopJobStatus::Failed);
    assert_eq!(
        failed_loop_job.processed_event_seq,
        loop_job.processed_event_seq
    );

    let events: Vec<SessionEvent> = request_json(
        app.clone(),
        Request::builder()
            .uri(format!("/api/sessions/{}/events", session.id))
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let failed_event = events
        .iter()
        .find(|event| event.event_type == "k_agent.failed")
        .expect("k_agent.failed event");
    assert_eq!(
        failed_event.payload["session_loop_job_id"],
        json!(loop_job.id)
    );
    assert_eq!(
        failed_event.payload["environment_id"],
        json!(environment.id)
    );
    assert_eq!(
        failed_event.payload["worker_id"],
        json!("k-agent-failure-worker")
    );
    assert_eq!(failed_event.payload["worker_pool"], json!("managed-agent"));
    assert_eq!(
        failed_event.payload["attempt_count"],
        json!(failed_loop_job.attempt_count)
    );
    assert_eq!(
        failed_event.payload["pending_event_seq_start"],
        json!(loop_job.pending_event_seq_start)
    );
    assert_eq!(
        failed_event.payload["pending_event_seq_end"],
        json!(loop_job.pending_event_seq_end)
    );
    assert_eq!(
        failed_event.payload["processed_event_seq"],
        json!(loop_job.processed_event_seq)
    );
    assert_eq!(failed_event.payload["job_status"], json!("failed"));
    assert_eq!(failed_event.payload["session_status"], json!("failed"));
    assert!(
        failed_event.payload["error"]
            .as_str()
            .unwrap_or_default()
            .contains("provider openai-compatible is not active")
    );
    assert_eq!(failed_event.payload["terminal_session"], json!(false));
    assert_eq!(
        failed_event.payload["dispatch_surface"],
        json!("session_loop_job")
    );
    assert_eq!(
        failed_event.payload["authority_boundary"],
        json!("environment_scheduling_only")
    );
    assert_eq!(
        failed_event.payload["return_contract"],
        json!("session_loop_job_return_evidence")
    );
}

#[tokio::test]
async fn k_agent_heartbeat_extends_session_loop_lease() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    state.seed_demo_agent().await.expect("seed demo agent");
    let app = build_router(state.clone());
    let agents = state.list_agents().await.expect("list seeded agents");
    let agent = agents.first().expect("seeded agent");
    let environment: Environment = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/environments",
            json!({
                "name": "K Agent Heartbeat",
                "environment_type": "local",
                "worker_queue_binding": {"queue": "heartbeat-pool"},
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
    let session: Session = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/sessions",
            json!({
                "agent_id": agent.id,
                "environment_id": environment.id,
                "title": "k agent heartbeat",
                "message": "enqueue heartbeat loop job"
            }),
            &[
                ("x-mandoforge-subject", "admin-1"),
                ("x-mandoforge-roles", "admin"),
            ],
        ),
    )
    .await;
    let queued = session_loop_jobs_for_session(app.clone(), session.id)
        .await
        .into_iter()
        .find(|job| job.status == SessionLoopJobStatus::Queued)
        .expect("queued session loop job");
    let running = state
        .start_session_loop_job(queued.id, "k-agent-heartbeat")
        .await
        .expect("start loop job");
    assert_eq!(running.status, SessionLoopJobStatus::Running);
    assert_eq!(running.attempt_count, 1);

    let heartbeat: SessionLoopJob = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/session-loop-jobs/{}/heartbeat", running.id))
            .header("x-mandoforge-worker-id", "k-agent-heartbeat")
            .header("x-mandoforge-subject", "worker-heartbeat")
            .header("x-mandoforge-roles", "admin")
            .header("x-mandoforge-environment-id", environment.id.to_string())
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert_eq!(heartbeat.status, SessionLoopJobStatus::Running);
    assert_eq!(heartbeat.worker_id.as_deref(), Some("k-agent-heartbeat"));
    assert_eq!(heartbeat.attempt_count, 1);
    assert!(heartbeat.lease_expires_at.is_some());

    match &state.store {
        StoreBackend::Memory(inner) => {
            let mut store = inner.write().await;
            let job = store
                .session_loop_jobs
                .get_mut(&running.id)
                .expect("stored session loop job");
            job.lease_expires_at = Some(Utc::now() - chrono::Duration::seconds(1));
        }
        StoreBackend::Postgres(_) => unreachable!("test uses memory store"),
    }
    let (status, error) = request_value(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/session-loop-jobs/{}/heartbeat", running.id))
            .header("x-mandoforge-worker-id", "k-agent-heartbeat")
            .header("x-mandoforge-subject", "worker-heartbeat")
            .header("x-mandoforge-roles", "admin")
            .header("x-mandoforge-environment-id", environment.id.to_string())
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(error["error"], json!("session loop job not found"));

    let events: Vec<SessionEvent> = request_json(
        app,
        Request::builder()
            .uri(format!("/api/sessions/{}/events", session.id))
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(events.iter().any(|event| {
        event.event_type == "k_agent.heartbeat"
            && event.payload["session_loop_job_id"] == json!(running.id)
            && event.payload["environment_id"] == json!(environment.id)
            && event.payload["worker_id"] == json!("k-agent-heartbeat")
            && event.payload["lease_expires_at"].as_str().is_some()
            && event.payload["dispatch_surface"] == json!("session_loop_job")
            && event.payload["authority_boundary"] == json!("environment_scheduling_only")
    }));
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
