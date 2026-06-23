use super::*;

#[test]
fn remote_computer_execution_transport_state_is_gate_driven() {
    assert_eq!(
        remote_computer_execution_transport_state("reserved", true),
        (false, "reserved")
    );
    assert_eq!(
        remote_computer_execution_transport_state("kubernetes", false),
        (false, "blocked")
    );
    assert_eq!(
        remote_computer_execution_transport_state("k8s", true),
        (true, "enabled")
    );
}

#[test]
fn worker_queue_backend_readiness_reports_jetstream_durable_semantics() {
    let worker_readiness = worker_queue_backend_readiness("nats_jetstream");

    assert_eq!(worker_readiness.kind, "nats_jetstream");
    assert!(worker_readiness.durable);
    assert!(worker_readiness.broker_handoff);
    assert!(worker_readiness.jetstream_enabled);
    assert!(worker_readiness.semantics.contains("durable pull-consumer"));
}

#[tokio::test]
async fn queue_backed_worker_invalid_remote_computer_contract_does_not_leave_job_running() {
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
    let environment: Environment = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/environments",
            json!({
                "name": "Disabled Remote Environment",
                "environment_type": "remote_computer",
                "remote_computer_profile": {"pool": "disabled", "profile": "workspace-write"},
                "worker_queue_binding": {"queue": "managed-agent"},
                "release_state": "active",
                "status": "disabled"
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
                "title": "invalid remote computer contract"
            }),
        ),
    )
    .await;
    let approval_result: Value = request_json(
        app.clone(),
        json_request(
            "POST",
            "/api/tools/file.write/execute",
            json!({
                "session_id": session.id,
                "args": {"path": "invalid-remote-contract.md", "content": "blocked"}
            }),
        ),
    )
    .await;
    let approval_id = approval_result["approval_id"]
        .as_str()
        .expect("approval id");
    let approved: Approval = request_json(
        app.clone(),
        approve_request(format!("/api/approvals/{approval_id}/approve")),
    )
    .await;
    let job_id = request_json::<Vec<execution_queue::ExecutionJob>>(
        app.clone(),
        Request::builder()
            .uri("/api/execution-jobs")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await
    .into_iter()
    .find(|job| job.approval_id == approved.id)
    .expect("execution job queued")
    .id;

    let requeued: execution_queue::ExecutionJob = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/execution-jobs/{job_id}/run"))
            .header("x-mandoforge-worker-id", "invalid-contract-worker")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert_eq!(requeued.status, ExecutionJobStatus::Queued);
    assert_eq!(requeued.attempt_count, 1);
    assert!(
        requeued
            .last_error
            .as_deref()
            .unwrap_or_default()
            .contains("not active and enabled")
    );

    let jobs: Vec<execution_queue::ExecutionJob> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/execution-jobs")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let persisted = jobs
        .iter()
        .find(|job| job.id == job_id)
        .expect("persisted execution job");
    assert_ne!(persisted.status, ExecutionJobStatus::Running);

    let events: Vec<SessionEvent> = request_json(
        app,
        Request::builder()
            .uri(format!("/api/sessions/{}/events", session.id))
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(events.iter().any(|event| {
        event.event_type == "environment.remote_computer_contract_invalid"
            && event.payload["error"]
                .as_str()
                .unwrap_or_default()
                .contains("not active and enabled")
    }));
    assert!(events.iter().any(|event| {
        event.event_type == "execution.retry_queued"
            && event.payload["stage"] == json!("environment_contract")
            && event.payload["last_error"]
                .as_str()
                .unwrap_or_default()
                .contains("not active and enabled")
    }));
}

#[tokio::test]
async fn queue_backed_worker_rejects_mismatched_active_remote_computer_assignment() {
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
    let environment: Environment = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/environments",
            json!({
                "name": "Strict Remote Pool",
                "environment_type": "remote_computer",
                "remote_computer_profile": {"pool": "strict", "profile": "workspace-write"},
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
                "title": "mismatched remote assignment"
            }),
        ),
    )
    .await;
    let relative_path = format!("mismatched-remote-{}.md", Uuid::new_v4());
    let approval_result: Value = request_json(
        app.clone(),
        json_request(
            "POST",
            "/api/tools/file.write/execute",
            json!({
                "session_id": session.id,
                "args": {"path": relative_path, "content": "must not run"}
            }),
        ),
    )
    .await;
    let approval_id = approval_result["approval_id"]
        .as_str()
        .expect("approval id");
    let approved: Approval = request_json(
        app.clone(),
        approve_request(format!("/api/approvals/{approval_id}/approve")),
    )
    .await;
    let job_id = request_json::<Vec<execution_queue::ExecutionJob>>(
        app.clone(),
        Request::builder()
            .uri("/api/execution-jobs")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await
    .into_iter()
    .find(|job| job.approval_id == approved.id)
    .expect("execution job queued")
    .id;

    let computer: RemoteComputer = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/remote-computers")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({
                    "name": "wrong-pool-remote-computer",
                    "profile": "read-only",
                    "pod_name": "wrong-pool-pod",
                    "metadata": {"pool": "wrong"}
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    let lease: RemoteComputerLease = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/remote-computers/{}/leases", computer.id))
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({
                    "session_id": session.id,
                    "worker_id": "wrong-pool-worker",
                    "lease_seconds": 900
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    let assignment: RemoteComputerJobAssignment = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!(
                "/api/execution-jobs/{job_id}/remote-computer-lease"
            ))
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "operator-1")
            .header("x-mandoforge-roles", "operator")
            .body(Body::from(
                json!({
                    "lease_id": lease.id,
                    "assigned_by": "operator-1",
                    "metadata": {"handoff_mode": "manual-test"}
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    assert_eq!(assignment.status, "assigned");

    let requeued: execution_queue::ExecutionJob = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/execution-jobs/{job_id}/run"))
            .header("x-mandoforge-worker-id", "mismatch-worker")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert_eq!(requeued.status, ExecutionJobStatus::Queued);
    assert!(
        requeued
            .last_error
            .as_deref()
            .unwrap_or_default()
            .contains("does not match the remote_computer environment contract")
    );

    let jobs: Vec<execution_queue::ExecutionJob> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/execution-jobs")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let persisted = jobs
        .iter()
        .find(|job| job.id == job_id)
        .expect("persisted execution job");
    assert_ne!(persisted.status, ExecutionJobStatus::Running);

    let assignments: Vec<RemoteComputerJobAssignment> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/remote-computer-job-assignments")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let released_assignment = assignments
        .iter()
        .find(|listed| listed.id == assignment.id)
        .expect("released assignment");
    assert_eq!(released_assignment.status, "released");
    assert_eq!(
        released_assignment.metadata["stage"],
        json!("remote_computer_assignment_validation")
    );
    assert_eq!(
        released_assignment.metadata["execution_job_status"],
        json!("queued")
    );

    let events: Vec<SessionEvent> = request_json(
        app,
        Request::builder()
            .uri(format!("/api/sessions/{}/events", session.id))
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(events.iter().any(|event| {
        event.event_type == "remote_computer.execution_handoff_released"
            && event.payload["assignment_id"] == json!(assignment.id)
    }));
    assert!(events.iter().any(|event| {
        event.event_type == "execution.retry_queued"
            && event.payload["stage"] == json!("remote_computer_assignment_validation")
            && event.payload["assignment_id"] == json!(assignment.id)
    }));
    assert!(!test_workspace_root().join(relative_path).exists());
}

#[tokio::test]
async fn queue_backed_worker_auto_assigns_active_remote_computer_lease() {
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
    let environment: Environment = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/environments",
            json!({
                "name": "Whiskey Remote Pool",
                "environment_type": "remote_computer",
                "remote_computer_profile": {
                    "pool": "whiskey",
                    "profile": "workspace-write"
                },
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
                "title": "auto remote computer handoff"
            }),
        ),
    )
    .await;

    let relative_path = format!("auto-remote-{}.md", Uuid::new_v4());
    let content = "# Auto Remote\n\nWorker drain.";
    let approval_result: Value = request_json(
        app.clone(),
        json_request(
            "POST",
            "/api/tools/file.write/execute",
            json!({
                "session_id": session.id,
                "args": {
                    "path": relative_path,
                    "content": content
                }
            }),
        ),
    )
    .await;
    let approval_id = approval_result["approval_id"]
        .as_str()
        .expect("approval id");

    let approved: Approval = request_json(
        app.clone(),
        approve_request(format!("/api/approvals/{approval_id}/approve")),
    )
    .await;

    let job_id = request_json::<Vec<execution_queue::ExecutionJob>>(
        app.clone(),
        Request::builder()
            .uri("/api/execution-jobs")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await
    .into_iter()
    .find(|job| job.approval_id == approved.id)
    .expect("execution job queued")
    .id;

    let computer: RemoteComputer = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/remote-computers")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({
                    "name": "auto-assign-remote-computer",
                    "profile": "workspace-write",
                    "pod_name": "agent-remote-computer-auto",
                    "metadata": {"pool": "whiskey"}
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    let lease: RemoteComputerLease = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/remote-computers/{}/leases", computer.id))
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({
                    "session_id": session.id,
                    "worker_id": "remote-computer-pool",
                    "lease_seconds": 900
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;

    let requeued: execution_queue::ExecutionJob = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/execution-jobs/{job_id}/run"))
            .header("x-mandoforge-worker-id", "remote-worker-1")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert_eq!(requeued.status, ExecutionJobStatus::Queued);
    assert_eq!(
        requeued.last_error.as_deref(),
        Some("remote_computer environment requires enabled Remote Computer execution transport")
    );

    let assignments: Vec<RemoteComputerJobAssignment> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/remote-computer-job-assignments")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let assignment = assignments
        .iter()
        .find(|assignment| assignment.execution_job_id == job_id)
        .expect("auto remote computer assignment");
    assert_eq!(assignment.lease_id, lease.id);
    assert_eq!(assignment.status, "released");
    assert_eq!(assignment.assigned_by.as_deref(), Some("remote-worker-1"));
    assert_eq!(
        assignment.metadata["handoff_mode"],
        "environment-worker-lease"
    );
    assert_eq!(
        assignment.metadata["environment_contract"]["environment_id"],
        json!(environment.id)
    );
    assert_eq!(assignment.metadata["execution_job_status"], "queued");

    let events: Vec<SessionEvent> = request_json(
        app,
        Request::builder()
            .uri(format!("/api/sessions/{}/events", session.id))
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(events.iter().any(|event| {
        event.event_type == "remote_computer.execution_handoff_assigned"
            && event.payload["assignment_id"] == json!(assignment.id)
            && event.payload["execution_enabled"] == json!(false)
    }));
    assert!(events.iter().any(|event| {
        event.event_type == "remote_computer.execution_handoff_acknowledged"
            && event.payload["assignment_id"] == json!(assignment.id)
    }));
    assert!(events.iter().any(|event| {
        event.event_type == "remote_computer.execution_transport_planned"
            && event.payload["assignment_id"] == json!(assignment.id)
            && event.payload["execution_enabled"] == json!(false)
    }));
    assert!(events.iter().any(|event| {
        event.event_type == "remote_computer.execution_handoff_released"
            && event.payload["assignment_id"] == json!(assignment.id)
            && event.payload["status"] == json!("released")
    }));
    assert!(events.iter().any(|event| {
            event.event_type == "execution.retry_queued"
                && event.payload["environment_id"] == json!(environment.id)
                && event.payload["last_error"]
                    == json!(
                        "remote_computer environment requires enabled Remote Computer execution transport"
                    )
        }));
}

#[tokio::test]
async fn queue_backed_worker_claims_available_warm_pool_remote_computer() {
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
    let environment: Environment = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/environments",
            json!({
                "name": "Warm Pool Remote Environment",
                "environment_type": "remote_computer",
                "remote_computer_profile": {
                    "pool": "warm",
                    "profile": "workspace-write"
                },
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
                "title": "warm pool claim"
            }),
        ),
    )
    .await;
    let approval_result: Value = request_json(
        app.clone(),
        json_request(
            "POST",
            "/api/tools/file.write/execute",
            json!({
                "session_id": session.id,
                "args": {"path": "warm-pool.md", "content": "warm pool"}
            }),
        ),
    )
    .await;
    let approval_id = approval_result["approval_id"]
        .as_str()
        .expect("approval id");
    let approved: Approval = request_json(
        app.clone(),
        approve_request(format!("/api/approvals/{approval_id}/approve")),
    )
    .await;
    let job_id = request_json::<Vec<execution_queue::ExecutionJob>>(
        app.clone(),
        Request::builder()
            .uri("/api/execution-jobs")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await
    .into_iter()
    .find(|job| job.approval_id == approved.id)
    .expect("execution job queued")
    .id;

    let computer: RemoteComputer = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/remote-computers")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({
                    "name": "warm-pool-remote-computer",
                    "profile": "workspace-write",
                    "pod_name": "agent-remote-computer-warm",
                    "metadata": {"warm_pool": true, "pool": "warm"}
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    assert_eq!(computer.status, "available");
    let newer_computer: RemoteComputer = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/remote-computers")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({
                    "name": "warm-pool-remote-computer-newer",
                    "profile": "workspace-write",
                    "pod_name": "agent-remote-computer-warm-newer",
                    "metadata": {"warm_pool": true, "pool": "warm"}
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    assert_eq!(newer_computer.status, "available");

    let requeued: execution_queue::ExecutionJob = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/execution-jobs/{job_id}/run"))
            .header("x-mandoforge-worker-id", "warm-pool-worker")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert_eq!(requeued.status, ExecutionJobStatus::Queued);

    let leases: Vec<RemoteComputerLease> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/remote-computer-leases")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let lease = leases
        .iter()
        .find(|lease| lease.remote_computer_id == computer.id)
        .expect("oldest warm pool lease was claimed");
    assert!(
        leases
            .iter()
            .all(|lease| lease.remote_computer_id != newer_computer.id),
        "newer warm pool computer should remain idle until older candidates are claimed"
    );
    assert_eq!(lease.session_id, Some(session.id));
    assert_eq!(lease.worker_id.as_deref(), Some("warm-pool-worker"));
    assert_eq!(
        lease.metadata["handoff_mode"],
        "environment-warm-pool-lease"
    );
    assert_eq!(
        lease.metadata["environment_contract"]["environment_id"],
        json!(environment.id)
    );

    let assignments: Vec<RemoteComputerJobAssignment> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/remote-computer-job-assignments")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let assignment = assignments
        .iter()
        .find(|assignment| assignment.execution_job_id == job_id)
        .expect("warm pool job assignment");
    assert_eq!(assignment.lease_id, lease.id);
    assert_eq!(assignment.status, "released");

    let events: Vec<SessionEvent> = request_json(
        app,
        Request::builder()
            .uri(format!("/api/sessions/{}/events", session.id))
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(events.iter().any(|event| {
        event.event_type == "remote_computer.warm_pool_claimed"
            && event.payload["remote_computer_id"] == json!(computer.id)
            && event.payload["environment_contract"]["environment_id"] == json!(environment.id)
    }));
}

#[tokio::test]
async fn queue_backed_worker_reuses_session_remote_computer_lease_for_multiple_jobs() {
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
    let environment: Environment = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/environments",
            json!({
                "name": "Reusable Session Remote Environment",
                "environment_type": "remote_computer",
                "remote_computer_profile": {
                    "pool": "reuse",
                    "profile": "workspace-write"
                },
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
                "title": "session lease reuse"
            }),
        ),
    )
    .await;
    let computer: RemoteComputer = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/remote-computers")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({
                    "name": "reuse-session-remote-computer",
                    "profile": "workspace-write",
                    "pod_name": "agent-remote-computer-reuse",
                    "workspace_path": "/workspace",
                    "metadata": {"pool": "reuse"}
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    let lease: RemoteComputerLease = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/remote-computers/{}/leases", computer.id))
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({
                    "session_id": session.id,
                    "worker_id": "remote-computer-pool",
                    "lease_seconds": 900
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;

    let mut job_ids = Vec::new();
    for index in 0..2 {
        let approval_result: Value = request_json(
            app.clone(),
            json_request(
                "POST",
                "/api/tools/file.write/execute",
                json!({
                    "session_id": session.id,
                    "args": {
                        "path": format!("reuse-{index}.md"),
                        "content": format!("reuse {index}")
                    }
                }),
            ),
        )
        .await;
        let approval_id = approval_result["approval_id"]
            .as_str()
            .expect("approval id");
        let approved: Approval = request_json(
            app.clone(),
            approve_request(format!("/api/approvals/{approval_id}/approve")),
        )
        .await;
        let job_id = request_json::<Vec<execution_queue::ExecutionJob>>(
            app.clone(),
            Request::builder()
                .uri("/api/execution-jobs")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .into_iter()
        .find(|job| job.approval_id == approved.id)
        .expect("execution job queued")
        .id;
        job_ids.push(job_id);
        let requeued: execution_queue::ExecutionJob = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/execution-jobs/{job_id}/run"))
                .header("x-mandoforge-worker-id", "reuse-worker")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert_eq!(requeued.status, ExecutionJobStatus::Queued);
    }

    let assignments: Vec<RemoteComputerJobAssignment> = request_json(
        app,
        Request::builder()
            .uri("/api/remote-computer-job-assignments")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let expected_workspace = format!("/workspace/sessions/{}", session.id);
    for job_id in job_ids {
        let assignment = assignments
            .iter()
            .find(|assignment| assignment.execution_job_id == job_id)
            .expect("remote computer assignment");
        assert_eq!(assignment.lease_id, lease.id);
        assert_eq!(assignment.remote_computer_id, computer.id);
        assert_eq!(
            assignment.metadata["session_workspace_path"],
            json!(expected_workspace)
        );
    }
}

#[tokio::test]
async fn queue_backed_worker_does_not_claim_second_pod_when_session_assignment_active() {
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
    let environment: Environment = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/environments",
            json!({
                "name": "Active Session Remote Environment",
                "environment_type": "remote_computer",
                "remote_computer_profile": {
                    "pool": "single-session",
                    "profile": "workspace-write"
                },
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
                "title": "active assignment blocks second pod"
            }),
        ),
    )
    .await;
    let mut job_ids = Vec::new();
    for index in 0..2 {
        let approval_result: Value = request_json(
            app.clone(),
            json_request(
                "POST",
                "/api/tools/file.write/execute",
                json!({
                    "session_id": session.id,
                    "args": {
                        "path": format!("active-assignment-{index}.md"),
                        "content": format!("active assignment {index}")
                    }
                }),
            ),
        )
        .await;
        let approval_id = approval_result["approval_id"]
            .as_str()
            .expect("approval id");
        let approved: Approval = request_json(
            app.clone(),
            approve_request(format!("/api/approvals/{approval_id}/approve")),
        )
        .await;
        let job_id = request_json::<Vec<execution_queue::ExecutionJob>>(
            app.clone(),
            Request::builder()
                .uri("/api/execution-jobs")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .into_iter()
        .find(|job| job.approval_id == approved.id)
        .expect("execution job queued")
        .id;
        job_ids.push(job_id);
    }

    let computer: RemoteComputer = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/remote-computers")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({
                    "name": "active-session-remote-computer",
                    "profile": "workspace-write",
                    "pod_name": "agent-remote-computer-active",
                    "metadata": {"pool": "single-session"}
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    let second_computer: RemoteComputer = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/remote-computers")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({
                    "name": "second-warm-pool-remote-computer",
                    "profile": "workspace-write",
                    "pod_name": "agent-remote-computer-second",
                    "metadata": {"warm_pool": true, "pool": "single-session"}
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    let lease: RemoteComputerLease = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/remote-computers/{}/leases", computer.id))
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({
                    "session_id": session.id,
                    "worker_id": "remote-computer-pool",
                    "lease_seconds": 900
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    let active_assignment: RemoteComputerJobAssignment = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!(
                "/api/execution-jobs/{}/remote-computer-lease",
                job_ids[0]
            ))
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "operator-1")
            .header("x-mandoforge-roles", "operator")
            .body(Body::from(
                json!({
                    "lease_id": lease.id,
                    "assigned_by": "operator-1",
                    "metadata": {"handoff_mode": "manual-active-test"}
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    assert_eq!(active_assignment.status, "assigned");

    let requeued: execution_queue::ExecutionJob = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/execution-jobs/{}/run", job_ids[1]))
            .header("x-mandoforge-worker-id", "active-session-worker")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert_eq!(requeued.status, ExecutionJobStatus::Queued);
    assert!(
        requeued
            .last_error
            .as_deref()
            .unwrap_or_default()
            .contains("session already has an active Remote Computer assignment")
    );

    let assignments: Vec<RemoteComputerJobAssignment> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/remote-computer-job-assignments")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(
        assignments
            .iter()
            .all(|assignment| assignment.execution_job_id != job_ids[1])
    );
    let leases: Vec<RemoteComputerLease> = request_json(
        app,
        Request::builder()
            .uri("/api/remote-computer-leases")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(
        leases
            .iter()
            .all(|lease| lease.remote_computer_id != second_computer.id)
    );
}
