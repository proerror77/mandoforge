use super::*;

#[test]
fn remote_computer_reclaim_run_defaults_new_replay_counter() {
    let run: RemoteComputerReclaimRun = serde_json::from_value(json!({
        "generated_at": "2026-09-01T00:00:00Z",
        "status": "noop",
        "stale_attachment_count": 0,
        "reclaimed_attachment_count": 0,
        "expired_lease_count": 0,
        "reclaimed_lease_count": 0,
        "attachments": [],
        "leases": [],
        "execution_enabled": false
    }))
    .expect("pre-upgrade reclaim run");

    assert_eq!(run.replayed_cleanup_evidence_count, 0);
}

#[tokio::test]
#[ignore = "requires MANDOFORGE_TEST_POSTGRES_URL"]
async fn postgres_existing_audit_log_ids_queries_requested_ids() {
    let database_url = std::env::var("MANDOFORGE_TEST_POSTGRES_URL")
        .expect("MANDOFORGE_TEST_POSTGRES_URL is required");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .expect("connect test postgres");
    run_migrations(&pool).await.expect("run migrations");
    let mut state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    seed_demo_tenant(&pool, state.tenant_id)
        .await
        .expect("seed tenant");
    state.store = StoreBackend::Postgres(pool);
    let audit = new_audit_log(
        None,
        "system",
        None,
        "remote_computer.cleanup_probe",
        "remote_computer",
        None,
        json!({"status": "completed"}),
    );
    let audit_id = audit.id;
    state.append_audit_log(audit).await.expect("append audit");

    let existing = state
        .existing_audit_log_ids(&[audit_id, Uuid::new_v4()])
        .await
        .expect("query requested audit ids");
    assert_eq!(existing, std::collections::HashSet::from([audit_id]));
}

#[tokio::test]
async fn terminal_session_releases_active_pooled_remote_computer_lease() {
    let app = test_app().await;
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
            json!({"agent_id": agents[0].id, "title": "terminal runtime cleanup"}),
        ),
    )
    .await;
    let computer: RemoteComputer = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/remote-computers",
            json!({
                "name": "terminal-pooled-computer",
                "profile": "workspace-write",
                "pod_name": "terminal-pooled-pod",
                "metadata": {"warm_pool": true}
            }),
            &[("x-mandoforge-roles", "admin")],
        ),
    )
    .await;
    let lease: RemoteComputerLease = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            &format!("/api/remote-computers/{}/leases", computer.id),
            json!({
                "session_id": session.id,
                "worker_id": "terminal-cleanup-worker",
                "lease_seconds": 900,
                "metadata": {"on_demand": false}
            }),
            &[("x-mandoforge-roles", "admin")],
        ),
    )
    .await;
    assert_eq!(lease.status, "leased");

    let _: Vec<SessionEvent> = request_json(
        app.clone(),
        json_request(
            "POST",
            &format!("/api/sessions/{}/events", session.id),
            json!({
                "events": [{
                    "type": "user.interrupt",
                    "payload": {"reason": "operator stop"}
                }]
            }),
        ),
    )
    .await;

    let leases: Vec<RemoteComputerLease> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/remote-computer-leases")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let released = leases
        .iter()
        .find(|candidate| candidate.id == lease.id)
        .expect("released lease");
    assert_eq!(released.status, "released");
    let computers: Vec<RemoteComputer> = request_json(
        app,
        Request::builder()
            .uri("/api/remote-computers")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert_eq!(
        computers
            .iter()
            .find(|candidate| candidate.id == computer.id)
            .expect("pooled computer")
            .status,
        "available"
    );
}

#[tokio::test]
async fn terminal_session_defers_failed_runtime_cleanup_for_stale_reclaim() {
    let _lock = env_lock().lock().expect("env lock");
    let _mode = EnvVarGuard::set("MANDOFORGE_REMOTE_COMPUTER_RUNNER", "reserved");
    let _mutation = EnvVarGuard::set("MANDOFORGE_REMOTE_COMPUTER_MUTATION_ENABLED", "false");
    let _live = EnvVarGuard::set("MANDOFORGE_REMOTE_COMPUTER_LIVE_MUTATION_ENABLED", "false");
    let app = test_app().await;
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
            json!({"agent_id": agents[0].id, "title": "terminal cleanup retry"}),
        ),
    )
    .await;
    let identity = RemoteComputerRuntimeIdentity::new(
        RemoteComputerSubstrate::AgentSandbox,
        "agent-os-test".to_string(),
        "terminal-retry-claim".to_string(),
        "terminal-retry-pod".to_string(),
        Some("terminal-retry-claim".to_string()),
        Some("terminal-retry-sandbox".to_string()),
        None,
    );
    let computer: RemoteComputer = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            "/api/remote-computers",
            json!({
                "name": identity.resource_name,
                "profile": "agent-sandbox",
                "namespace": identity.namespace,
                "pod_name": identity.pod_name,
                "metadata": metadata_with_remote_computer_runtime_identity(
                    &json!({"on_demand": true}),
                    &identity,
                )
            }),
            &[("x-mandoforge-roles", "admin")],
        ),
    )
    .await;
    let lease: RemoteComputerLease = request_json(
        app.clone(),
        json_request_with_headers(
            "POST",
            &format!("/api/remote-computers/{}/leases", computer.id),
            json!({
                "session_id": session.id,
                "worker_id": "terminal-retry-worker",
                "lease_seconds": 900,
                "metadata": {"on_demand": true}
            }),
            &[("x-mandoforge-roles", "admin")],
        ),
    )
    .await;

    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/api/sessions/{}/events", session.id),
            json!({
                "events": [{"type": "user.interrupt", "payload": {"reason": "stop"}}]
            }),
        ))
        .await
        .expect("interrupt response");
    assert_eq!(response.status(), StatusCode::OK);

    let persisted_session: Session = request_json(
        app.clone(),
        Request::builder()
            .uri(format!("/api/sessions/{}", session.id))
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert_eq!(persisted_session.status, SessionStatus::Terminated);
    let events: Vec<SessionEvent> = request_json(
        app.clone(),
        Request::builder()
            .uri(format!("/api/sessions/{}/events", session.id))
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(
        events
            .iter()
            .any(|event| event.event_type == "remote_computer.runtime_cleanup_failed")
    );
    let leases: Vec<RemoteComputerLease> = request_json(
        app,
        Request::builder()
            .uri("/api/remote-computer-leases")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let retryable_lease = leases
        .iter()
        .find(|candidate| candidate.id == lease.id)
        .expect("retryable lease");
    assert_eq!(retryable_lease.status, "leased");
    assert!(
        retryable_lease
            .lease_expires_at
            .is_some_and(|expires_at| expires_at <= Utc::now())
    );
}

#[tokio::test]
async fn stale_reclaim_replays_missing_cleanup_evidence_after_lease_transition() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    state.seed_demo_agent().await.expect("seed demo agent");
    let agent = state
        .list_agents()
        .await
        .expect("list agents")
        .into_iter()
        .next()
        .expect("seeded agent");
    let session = state
        .create_session(CreateSession {
            agent_id: agent.id,
            environment_id: None,
            title: "cleanup evidence retry".to_string(),
            message: None,
        })
        .await
        .expect("create session");
    let computer = state
        .create_remote_computer(CreateRemoteComputer {
            id: None,
            name: "cleanup-evidence-retry".to_string(),
            profile: Some("workspace-write".to_string()),
            namespace: None,
            pod_name: Some("cleanup-evidence-retry-pod".to_string()),
            workspace_path: None,
            state_mount_path: None,
            metadata: Some(json!({"warm_pool": true})),
        })
        .await
        .expect("create computer");
    let lease = state
        .create_remote_computer_lease(
            computer.id,
            CreateRemoteComputerLease {
                session_id: Some(session.id),
                worker_id: Some("cleanup-evidence-retry-worker".to_string()),
                lease_seconds: Some(60),
                metadata: None,
            },
        )
        .await
        .expect("create lease");
    let mut cleanup_metadata = json!({
        "runtime_cleanup_reason": "terminal cleanup",
        "runtime_cleanup": {
            "delete_attempted": false,
            "delete_status": "not_required"
        }
    });
    cleanup_metadata[REMOTE_COMPUTER_RUNTIME_CLEANUP_MARKER] = json!(true);
    state
        .update_remote_computer_lease_status(
            lease.id,
            "released",
            UpdateRemoteComputerLease {
                reason: Some("competing release".to_string()),
                metadata: None,
            },
        )
        .await
        .expect("competing status transition");
    state
        .transition_remote_computer_lease_after_runtime_cleanup(
            lease.id,
            "released",
            UpdateRemoteComputerLease {
                reason: Some("terminal cleanup".to_string()),
                metadata: Some(cleanup_metadata),
            },
        )
        .await
        .expect("persist lease transition without evidence");

    let replayed = execute_remote_computer_stale_reclaim(&state)
        .await
        .expect("replay cleanup evidence");
    assert_eq!(replayed.status, "completed");
    assert_eq!(replayed.replayed_cleanup_evidence_count, 1);

    assert!(
        state
            .list_events(session.id)
            .await
            .expect("list events")
            .iter()
            .any(|event| event.event_type == "remote_computer.runtime_cleanup_completed")
    );
    assert!(
        state
            .list_audit_logs(Some(session.id))
            .await
            .expect("list audit logs")
            .iter()
            .any(|audit| audit.action == "remote_computer.runtime_cleanup_completed")
    );
    let repeated = execute_remote_computer_stale_reclaim(&state)
        .await
        .expect("repeat cleanup evidence sweep");
    assert_eq!(repeated.status, "noop");
    assert_eq!(repeated.replayed_cleanup_evidence_count, 0);
}

#[tokio::test]
async fn expired_pooled_lease_reclaim_uses_runtime_cleanup_convergence() {
    let _lock = env_lock().lock().expect("env lock");
    let _runner = EnvVarGuard::set("MANDOFORGE_REMOTE_COMPUTER_RUNNER", "reserved");
    let _mutation = EnvVarGuard::set("MANDOFORGE_REMOTE_COMPUTER_MUTATION_ENABLED", "false");
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let computer = state
        .create_remote_computer(CreateRemoteComputer {
            id: None,
            name: "expired-pooled-computer".to_string(),
            profile: Some("workspace-write".to_string()),
            namespace: None,
            pod_name: Some("expired-pooled-pod".to_string()),
            workspace_path: None,
            state_mount_path: None,
            metadata: Some(json!({"warm_pool": true})),
        })
        .await
        .expect("create computer");
    let lease = state
        .create_remote_computer_lease(
            computer.id,
            CreateRemoteComputerLease {
                session_id: None,
                worker_id: Some("expired-worker".to_string()),
                lease_seconds: Some(60),
                metadata: Some(json!({"on_demand": false})),
            },
        )
        .await
        .expect("create lease");
    let failing_identity = RemoteComputerRuntimeIdentity::new(
        RemoteComputerSubstrate::AgentSandbox,
        "agent-os-test".to_string(),
        "expired-on-demand-claim".to_string(),
        "expired-on-demand-pod".to_string(),
        Some("expired-on-demand-claim".to_string()),
        Some("expired-on-demand-sandbox".to_string()),
        None,
    );
    let failing_computer = state
        .create_remote_computer(CreateRemoteComputer {
            id: None,
            name: "expired-on-demand-computer".to_string(),
            profile: Some("agent-sandbox".to_string()),
            namespace: Some(failing_identity.namespace.clone()),
            pod_name: Some(failing_identity.pod_name.clone()),
            workspace_path: None,
            state_mount_path: None,
            metadata: Some(metadata_with_remote_computer_runtime_identity(
                &json!({"on_demand": true}),
                &failing_identity,
            )),
        })
        .await
        .expect("create failing on-demand computer");
    let failing_lease = state
        .create_remote_computer_lease(
            failing_computer.id,
            CreateRemoteComputerLease {
                session_id: None,
                worker_id: Some("expired-on-demand-worker".to_string()),
                lease_seconds: Some(60),
                metadata: Some(json!({"on_demand": true})),
            },
        )
        .await
        .expect("create failing on-demand lease");
    let StoreBackend::Memory(inner) = &state.store else {
        panic!("test requires memory store");
    };
    let mut store = inner.write().await;
    for lease_id in [lease.id, failing_lease.id] {
        store
            .remote_computer_leases
            .get_mut(&lease_id)
            .expect("persisted lease")
            .lease_expires_at = Some(Utc::now() - chrono::Duration::seconds(1));
    }
    drop(store);

    let run = execute_remote_computer_stale_reclaim(&state)
        .await
        .expect("reclaim expired lease");
    assert_eq!(run.status, "attention");
    assert_eq!(run.expired_lease_count, 2);
    assert_eq!(run.reclaimed_lease_count, 1);
    assert_eq!(run.leases[0].status, "released");
    assert_eq!(
        state
            .list_remote_computer_leases()
            .await
            .expect("list leases")
            .into_iter()
            .find(|candidate| candidate.id == failing_lease.id)
            .expect("retryable on-demand lease")
            .status,
        "leased"
    );
    assert_eq!(
        state
            .list_remote_computers()
            .await
            .expect("list computers")
            .into_iter()
            .find(|candidate| candidate.id == computer.id)
            .expect("pooled computer")
            .status,
        "available"
    );
}

#[tokio::test]
async fn remote_computer_leases_are_audited_without_executing_tools() {
    let app = test_app().await;
    let agents: Vec<Agent> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/agents")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let session: Session = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/sessions")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({"agent_id": agents[0].id, "title": "remote computer lease"}).to_string(),
            ))
            .expect("valid request"),
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
                    "name": "remote-computer-test",
                    "profile": "workspace-write",
                    "pod_name": "agent-remote-computer-0"
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    assert_eq!(computer.status, "available");

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
                    "worker_id": "remote-manager-1",
                    "lease_seconds": 120,
                    "metadata": {"execution_enabled": false}
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    assert_eq!(lease.status, "leased");
    assert_eq!(lease.session_id, Some(session.id));
    assert!(lease.lease_expires_at.is_some());

    let heartbeat: RemoteComputerLease = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!(
                "/api/remote-computer-leases/{}/heartbeat",
                lease.id
            ))
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({"metadata": {"heartbeat": true}}).to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    assert_eq!(heartbeat.status, "leased");
    assert_eq!(heartbeat.metadata["heartbeat"], true);

    let released: RemoteComputerLease = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/remote-computer-leases/{}/release", lease.id))
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({"reason": "readiness skeleton release"}).to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    assert_eq!(released.status, "released");
    assert_eq!(released.metadata["reason"], "readiness skeleton release");

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
    assert_eq!(leases.len(), 1);

    let events: Vec<SessionEvent> = request_json(
        app.clone(),
        Request::builder()
            .uri(format!("/api/sessions/{}/events", session.id))
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let event_types: Vec<_> = events
        .iter()
        .map(|event| event.event_type.as_str())
        .collect();
    assert!(event_types.contains(&"remote_computer.leased"));
    assert!(event_types.contains(&"remote_computer.heartbeat"));
    assert!(event_types.contains(&"remote_computer.released"));

    let tool_calls: Vec<ToolCall> = request_json(
        app.clone(),
        Request::builder()
            .uri(format!("/api/sessions/{}/tool-calls", session.id))
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(
        tool_calls.is_empty(),
        "remote computer lease lifecycle must not execute tools"
    );

    let audit_logs: Vec<AuditLog> = request_json(
        app,
        Request::builder()
            .uri(format!("/api/sessions/{}/audit-logs", session.id))
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(
        audit_logs
            .iter()
            .any(|log| log.action == "remote_computer.leased")
    );
    assert!(
        audit_logs
            .iter()
            .any(|log| log.action == "remote_computer.released")
    );
}

#[tokio::test]
async fn remote_computer_artifact_sync_records_artifacts_events_and_audit() {
    let app = test_app().await;
    let agents: Vec<Agent> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/agents")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let session: Session = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/sessions")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({"agent_id": agents[0].id, "title": "remote artifact sync"}).to_string(),
            ))
            .expect("valid request"),
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
                    "name": "artifact-sync-remote-computer",
                    "profile": "workspace-write",
                    "pod_name": "agent-remote-computer-artifacts"
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
                    "worker_id": "artifact-sync-worker"
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    let _attachment: RemoteComputerAttachment = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/remote-computer-leases/{}/attach", lease.id))
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({
                    "session_id": session.id,
                    "attached_by": "artifact-sync-test"
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;

    let synced: RemoteComputerArtifactSyncResponse = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/remote-computers/artifacts/sync")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "operator-1")
            .header("x-mandoforge-roles", "operator")
            .body(Body::from(
                json!({
                    "session_id": session.id,
                    "remote_computer_id": computer.id,
                    "artifacts": [{
                        "name": "diagnostics.md",
                        "artifact_type": "markdown",
                        "path": "artifacts/diagnostics.md",
                        "content": {"markdown": "# Diagnostics"},
                        "metadata": {"source_path": "/workspace/artifacts/diagnostics.md"}
                    }]
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    assert_eq!(synced.artifact_count, 1);
    assert_eq!(synced.remote_computer_id, computer.id);
    assert_eq!(synced.artifacts[0].name, "diagnostics.md");

    let events: Vec<SessionEvent> = request_json(
        app.clone(),
        Request::builder()
            .uri(format!("/api/sessions/{}/events", session.id))
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(events.iter().any(|event| {
        event.event_type == "artifact.created"
            && event.payload["source"] == json!("remote_computer")
            && event.payload["remote_computer_id"] == json!(computer.id)
    }));

    let audit_logs: Vec<AuditLog> = request_json(
        app,
        Request::builder()
            .uri("/api/audit-logs")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(audit_logs.iter().any(|log| {
        log.action == "remote_computer.artifact_synced"
            && log.details["remote_computer_id"] == json!(computer.id)
    }));
}

#[tokio::test]
async fn remote_computer_artifact_sync_accepts_completed_assignment_with_active_lease() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    state.seed_demo_agent().await.expect("seed demo agent");
    let app = build_router(state.clone());
    let agents: Vec<Agent> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/agents")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let session: Session = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/sessions")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({"agent_id": agents[0].id, "title": "remote artifact completed assignment"})
                    .to_string(),
            ))
            .expect("valid request"),
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
                    "name": "artifact-sync-completed-assignment",
                    "profile": "workspace-write",
                    "pod_name": "agent-remote-computer-completed-artifacts"
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
                    "worker_id": "artifact-sync-completed-worker"
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    let assignment = state
        .create_remote_computer_job_assignment(
            Uuid::new_v4(),
            session.id,
            CreateRemoteComputerJobAssignment {
                lease_id: lease.id,
                assigned_by: Some("worker".to_string()),
                metadata: Some(json!({"test": "completed_artifact_sync"})),
            },
        )
        .await
        .expect("assignment");
    state
        .update_remote_computer_job_assignment_status(
            assignment.id,
            "completed",
            json!({"completed": true}),
        )
        .await
        .expect("complete assignment");

    let synced: RemoteComputerArtifactSyncResponse = request_json(
        app,
        Request::builder()
            .method("POST")
            .uri("/api/remote-computers/artifacts/sync")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "operator-1")
            .header("x-mandoforge-roles", "operator")
            .body(Body::from(
                json!({
                    "session_id": session.id,
                    "remote_computer_id": computer.id,
                    "assignment_id": assignment.id,
                    "artifacts": [{
                        "name": "final.md",
                        "artifact_type": "markdown",
                        "path": "artifacts/final.md",
                        "content": {"markdown": "# Final"},
                        "metadata": {"source_path": "/workspace/artifacts/final.md"}
                    }]
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;

    assert_eq!(synced.assignment_id, Some(assignment.id));
    assert_eq!(synced.artifact_count, 1);
}

#[tokio::test]
async fn remote_computer_artifact_sync_requires_session_binding_for_non_admin() {
    let app = test_app().await;
    let agents: Vec<Agent> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/agents")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let session: Session = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/sessions")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({"agent_id": agents[0].id, "title": "remote artifact unauthorized"})
                    .to_string(),
            ))
            .expect("valid request"),
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
                    "name": "artifact-sync-unbound-remote-computer",
                    "profile": "workspace-write"
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;

    let (status, body) = request_value(
        app,
        Request::builder()
            .method("POST")
            .uri("/api/remote-computers/artifacts/sync")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "operator-1")
            .header("x-mandoforge-roles", "operator")
            .body(Body::from(
                json!({
                    "session_id": session.id,
                    "remote_computer_id": computer.id,
                    "artifacts": [{
                        "name": "diagnostics.md",
                        "artifact_type": "markdown",
                        "path": "artifacts/diagnostics.md",
                        "content": {"markdown": "# Diagnostics"}
                    }]
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(
        body["error"]
            .as_str()
            .is_some_and(|message| message.contains("no active Remote Computer lease binding")),
        "{body:?}"
    );
}

#[tokio::test]
async fn remote_computer_artifact_discovery_scans_workspace_artifacts() {
    let app = test_app().await;
    let agents: Vec<Agent> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/agents")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let session: Session = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/sessions")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({"agent_id": agents[0].id, "title": "remote artifact discovery"}).to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    let workspace_path = test_workspace_root().join(format!("remote-discovery-{}", Uuid::new_v4()));
    let artifacts_path = workspace_path.join("artifacts").join("reports");
    tokio::fs::create_dir_all(&artifacts_path)
        .await
        .expect("create artifact directory");
    tokio::fs::write(artifacts_path.join("diagnostics.md"), "# Diagnostics\n")
        .await
        .expect("write artifact");

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
                    "name": "artifact-discovery-remote-computer",
                    "profile": "workspace-write",
                    "pod_name": "agent-remote-computer-discovery",
                    "workspace_path": workspace_path
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
                    "worker_id": "artifact-discovery-worker"
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    let _attachment: RemoteComputerAttachment = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/remote-computer-leases/{}/attach", lease.id))
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({
                    "session_id": session.id,
                    "attached_by": "artifact-discovery-test"
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;

    let discovered: RemoteComputerArtifactSyncResponse = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/remote-computers/artifacts/discover")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "operator-1")
            .header("x-mandoforge-roles", "operator")
            .body(Body::from(
                json!({
                    "session_id": session.id,
                    "remote_computer_id": computer.id,
                    "artifact_dir": "artifacts",
                    "max_files": 10
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    assert_eq!(discovered.artifact_count, 1);
    assert_eq!(discovered.artifacts[0].name, "diagnostics.md");
    assert_eq!(discovered.artifacts[0].artifact_type, "markdown");
    assert_eq!(
        discovered.artifacts[0].path.as_deref(),
        Some("artifacts/reports/diagnostics.md")
    );
    assert_eq!(
        discovered.artifacts[0].content["source"],
        json!("remote_computer_artifact_discovery")
    );

    let events: Vec<SessionEvent> = request_json(
        app.clone(),
        Request::builder()
            .uri(format!("/api/sessions/{}/events", session.id))
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(events.iter().any(|event| {
        event.event_type == "artifact.created"
            && event.payload["source"] == json!("remote_computer_artifact_discovery")
            && event.payload["remote_computer_id"] == json!(computer.id)
    }));

    let audit_logs: Vec<AuditLog> = request_json(
        app,
        Request::builder()
            .uri("/api/audit-logs")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(audit_logs.iter().any(|log| {
        log.action == "remote_computer.artifact_discovered"
            && log.details["remote_computer_id"] == json!(computer.id)
    }));

    let _ = tokio::fs::remove_dir_all(&workspace_path).await;
}

#[tokio::test]
async fn remote_computer_state_locks_prevent_concurrent_state_writes() {
    let app = test_app().await;
    let agents: Vec<Agent> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/agents")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let session: Session = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/sessions")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({"agent_id": agents[0].id, "title": "remote state lock"}).to_string(),
            ))
            .expect("valid request"),
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
                    "name": "state-lock-remote-computer",
                    "profile": "workspace-write"
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
                    "worker_id": "state-lock-worker"
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;

    let lock: RemoteComputerStateLock = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/remote-computers/state-locks")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "operator-1")
            .header("x-mandoforge-roles", "operator")
            .body(Body::from(
                json!({
                    "lock_key": "memory/session-notes.md",
                    "remote_computer_id": computer.id,
                    "lease_id": lease.id,
                    "session_id": session.id,
                    "owner": "state-lock-worker",
                    "lease_seconds": 60
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    assert_eq!(lock.status, "held");
    assert_eq!(lock.lock_key, "memory/session-notes.md");

    let (status, value) = request_value(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/remote-computers/state-locks")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "operator-1")
            .header("x-mandoforge-roles", "operator")
            .body(Body::from(
                json!({
                    "lock_key": "memory/session-notes.md",
                    "session_id": session.id,
                    "owner": "other-worker"
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        value["error"],
        json!("Remote Computer state lock is already held")
    );

    let (status, value) = request_value(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!(
                "/api/remote-computers/state-locks/{}/release",
                lock.id
            ))
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "operator-1")
            .header("x-mandoforge-roles", "operator")
            .header("x-mandoforge-worker-id", "other-worker")
            .body(Body::from(json!({"reason": "wrong worker"}).to_string()))
            .expect("valid request"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        value["error"],
        json!("Remote Computer state lock owner does not match worker identity")
    );

    let released: RemoteComputerStateLock = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!(
                "/api/remote-computers/state-locks/{}/release",
                lock.id
            ))
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "operator-1")
            .header("x-mandoforge-roles", "operator")
            .header("x-mandoforge-worker-id", "state-lock-worker")
            .body(Body::from(json!({"reason": "done"}).to_string()))
            .expect("valid request"),
    )
    .await;
    assert_eq!(released.status, "released");
    assert!(released.released_at.is_some());

    let locks: Vec<RemoteComputerStateLock> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/remote-computers/state-locks")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(locks.iter().any(|existing| existing.id == lock.id));

    let events: Vec<SessionEvent> = request_json(
        app.clone(),
        Request::builder()
            .uri(format!("/api/sessions/{}/events", session.id))
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(events.iter().any(|event| {
        event.event_type == "remote_computer.state_lock_acquired"
            && event.payload["lock_key"] == json!("memory/session-notes.md")
    }));
    assert!(events.iter().any(|event| {
        event.event_type == "remote_computer.state_lock_released"
            && event.payload["lock_id"] == json!(lock.id)
    }));

    let (status, value) = request_value(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/remote-computers/state-locks")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "operator-1")
            .header("x-mandoforge-roles", "operator")
            .body(Body::from(
                json!({
                    "lock_key": "memory/missing-session.md",
                    "owner": "state-lock-worker"
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        value["error"],
        json!("Remote Computer state lock requires session_id for scoped access")
    );
}

#[tokio::test]
async fn remote_computer_sidecar_heartbeats_are_persisted_and_audited() {
    let app = test_app().await;
    let agents: Vec<Agent> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/agents")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let session: Session = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/sessions")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({"agent_id": agents[0].id, "title": "sidecar heartbeat"}).to_string(),
            ))
            .expect("valid request"),
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
                    "name": "sidecar-heartbeat-remote-computer",
                    "profile": "workspace-write"
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
                    "worker_id": "sidecar-heartbeat-worker"
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    let approval_required: Value = request_json(
        app.clone(),
        json_request(
            "POST",
            "/api/tools/agent_cli.exec/execute",
            json!({
                "session_id": session.id,
                "args": {
                    "profile": "heartbeat-worker",
                    "task": "Report heartbeat artifacts"
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
    let job = request_json::<Vec<execution_queue::ExecutionJob>>(
        app.clone(),
        Request::builder()
            .uri("/api/execution-jobs")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await
    .into_iter()
    .find(|job| job.approval_id == approved.id && job.tool_name == "agent_cli.exec")
    .expect("heartbeat execution job queued");
    let assignment: RemoteComputerJobAssignment = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!(
                "/api/execution-jobs/{}/remote-computer-lease",
                job.id
            ))
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "operator-1")
            .header("x-mandoforge-roles", "operator")
            .body(Body::from(
                json!({
                    "lease_id": lease.id,
                    "assigned_by": "operator-1"
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;

    let heartbeat: RemoteComputerSidecarHeartbeat = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/remote-computers/sidecars/heartbeats")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "operator-1")
            .header("x-mandoforge-roles", "operator")
            .body(Body::from(
                json!({
                    "remote_computer_id": computer.id,
                    "session_id": session.id,
                    "assignment_id": assignment.id,
                    "sidecar_name": "artifact-discovery",
                    "status": "enabled",
                    "metadata": {"artifact_dir": "/workspace/artifacts"}
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    assert_eq!(heartbeat.remote_computer_id, computer.id);
    assert_eq!(heartbeat.sidecar_name, "artifact-discovery");
    assert_eq!(heartbeat.status, "enabled");

    let other_session: Session = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/sessions")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({"agent_id": agents[0].id, "title": "sidecar heartbeat other session"})
                    .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    let (status, value) = request_value(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/remote-computers/sidecars/heartbeats")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "operator-1")
            .header("x-mandoforge-roles", "operator")
            .body(Body::from(
                json!({
                    "remote_computer_id": computer.id,
                    "session_id": other_session.id,
                    "assignment_id": assignment.id,
                    "sidecar_name": "artifact-discovery",
                    "status": "enabled"
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        value["error"],
        json!("Remote Computer sidecar heartbeat assignment does not belong to the session")
    );

    let heartbeats: Vec<RemoteComputerSidecarHeartbeat> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/remote-computers/sidecars/heartbeats")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(
        heartbeats
            .iter()
            .any(|existing| existing.id == heartbeat.id)
    );

    let events: Vec<SessionEvent> = request_json(
        app.clone(),
        Request::builder()
            .uri(format!("/api/sessions/{}/events", session.id))
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(events.iter().any(|event| {
        event.event_type == "remote_computer.sidecar_heartbeat"
            && event.payload["heartbeat_id"] == json!(heartbeat.id)
    }));

    let audit_logs: Vec<AuditLog> = request_json(
        app,
        Request::builder()
            .uri("/api/audit-logs")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(audit_logs.iter().any(|log| {
        log.action == "remote_computer.sidecar_heartbeat" && log.resource_id == Some(heartbeat.id)
    }));
}

#[tokio::test]
async fn remote_computer_readiness_flags_missing_and_stale_sidecar_heartbeats() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let computer = state
        .create_remote_computer(CreateRemoteComputer {
            id: None,
            name: "sidecar-supervision-remote-computer".to_string(),
            profile: Some("workspace-write".to_string()),
            namespace: None,
            pod_name: Some("agent-remote-computer-supervision".to_string()),
            workspace_path: None,
            state_mount_path: None,
            metadata: None,
        })
        .await
        .expect("create remote computer");

    let readiness_without_heartbeat = build_remote_computer_readiness(&state)
        .await
        .expect("readiness without heartbeat");
    assert_eq!(
        readiness_without_heartbeat.sidecar_supervision.status,
        "attention"
    );
    assert_eq!(
        readiness_without_heartbeat
            .sidecar_supervision
            .missing_heartbeat_count,
        1
    );
    assert_eq!(
        readiness_without_heartbeat.sidecar_recovery.status,
        "blocked"
    );
    assert_eq!(
        readiness_without_heartbeat.sidecar_recovery.unhealthy_count,
        1
    );
    assert_eq!(
        readiness_without_heartbeat
            .sidecar_recovery
            .replaceable_pod_count,
        1
    );
    assert_eq!(
        readiness_without_heartbeat
            .sidecar_recovery
            .blocked_reason
            .as_deref(),
        Some("replacement_gate_disabled")
    );
    assert!(
        readiness_without_heartbeat
            .attention_items
            .iter()
            .any(|item| item.kind == "artifact_discovery_sidecar_heartbeat_missing")
    );
    assert!(
        readiness_without_heartbeat
            .attention_items
            .iter()
            .any(|item| item.kind == "sidecar_replacement_blocked")
    );

    let heartbeat = state
        .record_remote_computer_sidecar_heartbeat(CreateRemoteComputerSidecarHeartbeat {
            remote_computer_id: computer.id,
            session_id: None,
            assignment_id: None,
            sidecar_name: Some("artifact-discovery".to_string()),
            status: Some("enabled".to_string()),
            metadata: None,
        })
        .await
        .expect("record heartbeat");
    if let StoreBackend::Memory(inner) = &state.store {
        let mut store = inner.write().await;
        let stored = store
            .remote_computer_sidecar_heartbeats
            .get_mut(&heartbeat.id)
            .expect("stored heartbeat");
        stored.observed_at = Utc::now() - ChronoDuration::seconds(600);
    }

    let readiness_with_stale_heartbeat = build_remote_computer_readiness(&state)
        .await
        .expect("readiness with stale heartbeat");
    assert_eq!(
        readiness_with_stale_heartbeat.sidecar_supervision.status,
        "attention"
    );
    assert_eq!(
        readiness_with_stale_heartbeat
            .sidecar_supervision
            .missing_heartbeat_count,
        0
    );
    assert_eq!(
        readiness_with_stale_heartbeat
            .sidecar_supervision
            .stale_heartbeat_count,
        1
    );
    assert_eq!(
        readiness_with_stale_heartbeat.sidecar_recovery.status,
        "blocked"
    );
    assert_eq!(
        readiness_with_stale_heartbeat
            .sidecar_recovery
            .unhealthy_count,
        1
    );
    assert!(
        readiness_with_stale_heartbeat
            .attention_items
            .iter()
            .any(|item| item.kind == "artifact_discovery_sidecar_heartbeat_stale")
    );
}

#[tokio::test]
async fn remote_computer_sidecar_recovery_run_is_audited_and_fail_closed() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let computer = state
        .create_remote_computer(CreateRemoteComputer {
            id: None,
            name: "sidecar-recovery-remote-computer".to_string(),
            profile: Some("workspace-write".to_string()),
            namespace: None,
            pod_name: Some("agent-remote-computer-recovery".to_string()),
            workspace_path: None,
            state_mount_path: None,
            metadata: None,
        })
        .await
        .expect("create remote computer");

    let run = execute_remote_computer_sidecar_recovery(&state)
        .await
        .expect("run sidecar recovery");
    assert_eq!(run.status, "blocked");
    assert_eq!(run.unhealthy_count, 1);
    assert_eq!(run.planned_replacement_count, 1);
    assert_eq!(run.attempted_replacement_count, 0);
    assert_eq!(run.blocked_replacement_count, 1);
    assert!(!run.replacement_enabled);
    assert!(!run.execution_enabled);
    assert_eq!(run.targets[0].remote_computer_id, computer.id);
    assert_eq!(
        run.targets[0].reason,
        "missing_artifact_discovery_heartbeat"
    );
    assert!(run.runner_responses.is_empty());

    let audit_logs = state.list_audit_logs(None).await.expect("audit logs");
    assert!(audit_logs.iter().any(|log| {
        log.action == "remote_computer.sidecar_recovery_run"
            && log.details["status"] == json!("blocked")
            && log.details["unhealthy_count"] == json!(1)
            && log.details["execution_enabled"] == json!(false)
    }));
}

#[tokio::test]
async fn remote_computer_state_sync_controller_executes_external_boundary() {
    let payloads = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("state sync listener");
    let controller_addr = listener.local_addr().expect("state sync addr");
    let controller = Router::new()
        .route(
            "/state-sync-validation",
            post(mock_remote_state_sync_controller),
        )
        .with_state(payloads.clone());
    let controller_server = tokio::spawn(async move {
        axum::serve(listener, controller)
            .await
            .expect("mock state sync controller");
    });
    let lookup = |key: &str| match key {
        "MANDOFORGE_REMOTE_COMPUTER_STATE_SYNC_CONTROLLER_URL" => {
            Some(format!("http://{controller_addr}/state-sync-validation"))
        }
        "MANDOFORGE_REMOTE_COMPUTER_STATE_SYNC_CONTROLLER_TOKEN" => {
            Some("state-sync-token".to_string())
        }
        _ => None,
    };
    let state_filesystem = ready_remote_state_filesystem();

    let execution = execute_remote_computer_state_sync_controller(
        &lookup,
        "admin-1",
        Utc::now(),
        &state_filesystem,
    )
    .await
    .expect("state sync controller");

    assert_eq!(execution["status"], "validated");
    assert_eq!(execution["state_sync_id"], "state-sync-1");
    let payloads = payloads.lock().await;
    assert_eq!(payloads.len(), 1);
    assert_eq!(
        payloads[0]["type"],
        "mandoforge.remote_computer_state_sync_validation"
    );
    assert_eq!(payloads[0]["subject"], "admin-1");
    assert_eq!(payloads[0]["provider"], "juicefs");
    assert_eq!(
        payloads[0]["state_layout_paths"].as_array().unwrap().len(),
        6
    );

    controller_server.abort();
}

#[test]
fn remote_computer_state_sync_readiness_requires_controller_when_configured() {
    let generated_at = Utc::now();
    let state_filesystem = ready_remote_state_filesystem();
    let missing_controller = build_remote_computer_production_state_sync_readiness(
        &state_filesystem,
        &[],
        generated_at,
        true,
        false,
    );
    assert_eq!(missing_controller.status, "blocked");
    assert!(missing_controller.production_blocked);
    assert!(
        missing_controller
            .blocking_reasons
            .iter()
            .any(|reason| { reason == "state sync controller is required but not configured" })
    );

    let mut audit = new_audit_log(
        None,
        "user",
        None,
        "remote_computer.production_state_sync_validation",
        "remote_computer_state_sync",
        None,
        json!({
            "status": "validated",
            "controller_required": true,
            "controller_configured": true,
            "controller_execution": {
                "attempted": true,
                "status": "validated",
                "state_sync_id": "state-sync-1"
            }
        }),
    );
    audit.created_at = generated_at;
    let ready = build_remote_computer_production_state_sync_readiness(
        &state_filesystem,
        &[audit],
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

    let mut stale_audit = new_audit_log(
        None,
        "user",
        None,
        "remote_computer.production_state_sync_validation",
        "remote_computer_state_sync",
        None,
        json!({
            "status": "validated",
            "controller_required": true,
            "controller_configured": true,
            "controller_execution": {
                "attempted": true,
                "status": "validated",
                "state_sync_id": "state-sync-1"
            }
        }),
    );
    stale_audit.created_at = generated_at - chrono::Duration::hours(25);
    let stale = build_remote_computer_production_state_sync_readiness(
        &state_filesystem,
        &[stale_audit],
        generated_at,
        true,
        true,
    );
    assert_eq!(stale.status, "blocked");
    assert!(stale.production_blocked);
    assert_eq!(stale.latest_controller_status.as_deref(), Some("validated"));
    assert!(stale.latest_controller_validated);
    assert!(!stale.controller_evidence_fresh);
    assert_eq!(stale.latest_controller_age_hours, Some(25));
    assert!(
        stale
            .blocking_reasons
            .iter()
            .any(|reason| reason == "state sync controller evidence is stale")
    );
}

#[test]
fn remote_computer_sidecar_recovery_requires_validation_controller_when_configured() {
    let remote_computer_id = Uuid::new_v4();
    let targets = vec![RemoteComputerSidecarRecoveryTarget {
        remote_computer_id,
        name: "remote-computer-1".to_string(),
        pod_name: Some("agent-remote-computer-1".to_string()),
        reason: "stale_artifact_discovery_heartbeat".to_string(),
        latest_observed_at: Some(Utc::now()),
    }];
    let runner = RemoteComputerRunnerReadiness {
        mode: "kubernetes".to_string(),
        configured: true,
        status: "ready".to_string(),
        namespace: "default".to_string(),
        pod_template_path: "deploy/k8s/agent-remote-computer.yaml".to_string(),
        service_account: "mandoforge-remote-computer".to_string(),
        client_configured: true,
        api_server_configured: true,
        bearer_token_configured: true,
        mutation_enabled: true,
        live_mutation_enabled: true,
        dry_run_only: false,
        supported_operations: vec!["live_delete".to_string(), "live_create".to_string()],
        message: "ready".to_string(),
    };
    let lookup = |key: &str| match key {
        "MANDOFORGE_REMOTE_COMPUTER_SIDECAR_REPLACEMENT_ENABLED" => Some("true".to_string()),
        "MANDOFORGE_REMOTE_COMPUTER_SIDECAR_VALIDATION_REQUIRED" => Some("true".to_string()),
        _ => None,
    };

    let readiness =
        build_remote_computer_sidecar_recovery_readiness_with_lookup(&targets, &runner, &lookup);

    assert_eq!(readiness.status, "blocked");
    assert!(readiness.replacement_enabled);
    assert!(readiness.validation_controller_required);
    assert!(!readiness.validation_controller_configured);
    assert_eq!(
        readiness.blocked_reason.as_deref(),
        Some("validation_controller_required")
    );
}

#[tokio::test]
async fn remote_computer_sidecar_validation_controller_confirms_replacement() {
    let payloads = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("sidecar validation listener");
    let controller_addr = listener.local_addr().expect("sidecar validation addr");
    let controller = Router::new()
        .route(
            "/sidecar-validation",
            post(mock_remote_sidecar_validation_controller),
        )
        .with_state(payloads.clone());
    let controller_server = tokio::spawn(async move {
        axum::serve(listener, controller)
            .await
            .expect("mock sidecar validation controller");
    });
    let remote_computer_id = Uuid::new_v4();
    let run = RemoteComputerSidecarRecoveryRun {
        generated_at: Utc::now(),
        status: "completed".to_string(),
        replacement_enabled: true,
        validation_controller_required: true,
        validation_controller_configured: true,
        runner_status: "ready".to_string(),
        unhealthy_count: 1,
        planned_replacement_count: 1,
        attempted_replacement_count: 1,
        blocked_replacement_count: 0,
        targets: vec![RemoteComputerSidecarRecoveryTarget {
            remote_computer_id,
            name: "remote-computer-1".to_string(),
            pod_name: Some("agent-remote-computer-1".to_string()),
            reason: "stale_heartbeat".to_string(),
            latest_observed_at: Some(Utc::now()),
        }],
        runner_responses: vec![
            RemoteComputerRunnerDryRunResponse {
                status: "mutation_ok".to_string(),
                operation: "live_delete".to_string(),
                configured: true,
                would_create_pod: false,
                would_delete_pod: true,
                live_probe_attempted: false,
                live_probe_status_code: None,
                live_mutation_attempted: true,
                live_mutation_status_code: Some(200),
                kubernetes_api_path: Some(
                    "/api/v1/namespaces/default/pods/agent-remote-computer-1".to_string(),
                ),
                namespace: Some("default".to_string()),
                pod_name: Some("agent-remote-computer-1".to_string()),
                pod_template_path: Some("deploy/k8s/agent-remote-computer.yaml".to_string()),
                execution_enabled: true,
                message: "deleted".to_string(),
                request: json!({"remote_computer_id": remote_computer_id, "operation": "live_delete"}),
                exec_result: None,
            },
            RemoteComputerRunnerDryRunResponse {
                status: "mutation_ok".to_string(),
                operation: "live_create".to_string(),
                configured: true,
                would_create_pod: true,
                would_delete_pod: false,
                live_probe_attempted: false,
                live_probe_status_code: None,
                live_mutation_attempted: true,
                live_mutation_status_code: Some(201),
                kubernetes_api_path: Some("/api/v1/namespaces/default/pods".to_string()),
                namespace: Some("default".to_string()),
                pod_name: Some("agent-remote-computer-1".to_string()),
                pod_template_path: Some("deploy/k8s/agent-remote-computer.yaml".to_string()),
                execution_enabled: true,
                message: "created".to_string(),
                request: json!({"remote_computer_id": remote_computer_id, "operation": "live_create"}),
                exec_result: None,
            },
        ],
        validation_result: json!({}),
        execution_enabled: false,
        message: "completed".to_string(),
    };
    let lookup = |key: &str| match key {
        "MANDOFORGE_REMOTE_COMPUTER_SIDECAR_VALIDATION_URL" => {
            Some(format!("http://{controller_addr}/sidecar-validation"))
        }
        "MANDOFORGE_REMOTE_COMPUTER_SIDECAR_VALIDATION_TOKEN" => Some("sidecar-token".to_string()),
        _ => None,
    };

    let validation = execute_remote_computer_sidecar_validation_controller(&lookup, &run)
        .await
        .expect("sidecar validation");

    assert_eq!(validation["status"], "validated");
    assert_eq!(validation["replacement_pods_healthy"], true);
    let payloads = payloads.lock().await;
    assert_eq!(payloads.len(), 1);
    assert_eq!(
        payloads[0]["type"],
        "mandoforge.remote_computer_sidecar_replacement_validation"
    );
    assert_eq!(payloads[0]["attempted_replacement_count"], 1);
    assert_eq!(
        payloads[0]["targets"][0]["remote_computer_id"],
        remote_computer_id.to_string()
    );

    controller_server.abort();
}

#[tokio::test]
async fn remote_computer_runner_boundary_is_reserved_and_dry_run_only() {
    let app = test_app().await;
    let agents: Vec<Agent> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/agents")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let session: Session = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/sessions")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({"agent_id": agents[0].id, "title": "remote computer runner dry run"})
                    .to_string(),
            ))
            .expect("valid request"),
    )
    .await;

    let readiness: RemoteComputerRunnerReadiness = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/remote-computers/runner/readiness")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert_eq!(readiness.status, "reserved");
    assert!(!readiness.configured);
    assert!(readiness.message.contains("no Pods are created or deleted"));

    let dry_run: RemoteComputerRunnerDryRunResponse = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/remote-computers/runner/dry-run")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({
                    "operation": "create",
                    "session_id": session.id,
                    "pod_name": "agent-remote-computer-dry-run",
                    "metadata": {"reason": "test reserved runner"}
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    assert_eq!(dry_run.status, "reserved");
    assert_eq!(dry_run.operation, "create");
    assert!(!dry_run.configured);
    assert!(!dry_run.would_create_pod);
    assert!(!dry_run.would_delete_pod);
    assert!(!dry_run.execution_enabled);

    let exec_dry_run: RemoteComputerRunnerDryRunResponse = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/remote-computers/runner/dry-run")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({
                    "operation": "exec",
                    "session_id": session.id,
                    "pod_name": "agent-remote-computer-dry-run",
                    "metadata": {
                        "sandbox_runtime_request": {
                            "version": "v1",
                            "session_id": session.id,
                            "workspace_path": format!("/workspace/sessions/{}", session.id),
                            "timeout_seconds": 30,
                            "environment": {},
                            "operation": {
                                "type": "shell",
                                "command": "pwd && echo SENSITIVE_DRY_RUN_SENTINEL"
                            }
                        }
                    }
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    assert_eq!(exec_dry_run.status, "reserved");
    assert_eq!(exec_dry_run.operation, "exec");
    assert!(!exec_dry_run.execution_enabled);
    assert_eq!(
        exec_dry_run.request["metadata"]["sandbox_runtime"]["operation"],
        "shell"
    );
    assert_eq!(
        exec_dry_run.request["metadata"]["sandbox_runtime"]["redacted"],
        true
    );
    assert!(
        exec_dry_run.request["metadata"]["sandbox_runtime"]["stdin_bytes"]
            .as_u64()
            .is_some_and(|bytes| bytes > 0)
    );
    assert!(
        !exec_dry_run
            .request
            .to_string()
            .contains("SENSITIVE_DRY_RUN_SENTINEL")
    );

    let (exec_mutate_status, exec_mutate_body) = request_value(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/remote-computers/runner/mutate")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({
                    "operation": "live_exec",
                    "session_id": session.id,
                    "pod_name": "agent-remote-computer-dry-run",
                    "metadata": {"command": "pwd"}
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    assert_eq!(exec_mutate_status, StatusCode::BAD_REQUEST);
    assert!(
        exec_mutate_body["error"]
            .as_str()
            .is_some_and(|message| message.contains("approved execution job")),
        "{exec_mutate_body:?}"
    );

    let mutate: RemoteComputerRunnerDryRunResponse = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/remote-computers/runner/mutate")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({
                    "operation": "live_create",
                    "session_id": session.id,
                    "pod_name": "agent-remote-computer-dry-run",
                    "metadata": {"reason": "test reserved runner mutation"}
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    assert_eq!(mutate.status, "blocked");
    assert_eq!(mutate.operation, "live_create");
    assert!(!mutate.live_mutation_attempted);
    assert!(!mutate.execution_enabled);

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
    assert!(leases.is_empty(), "runner dry-run must not create leases");

    let jobs: Vec<execution_queue::ExecutionJob> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/execution-jobs")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(
        jobs.is_empty(),
        "runner dry-run must not enqueue execution jobs"
    );

    let tool_calls: Vec<ToolCall> = request_json(
        app.clone(),
        Request::builder()
            .uri(format!("/api/sessions/{}/tool-calls", session.id))
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(
        tool_calls.is_empty(),
        "runner dry-run must not execute tools"
    );

    let audit_logs: Vec<AuditLog> = request_json(
        app,
        Request::builder()
            .uri(format!("/api/sessions/{}/audit-logs", session.id))
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(
        audit_logs
            .iter()
            .any(|log| log.action == "remote_computer.runner_dry_run")
    );
    assert!(
        audit_logs
            .iter()
            .any(|log| log.action == "remote_computer.runner_mutate")
    );
}

#[tokio::test]
async fn remote_computer_session_attachments_are_persisted_and_audited_without_execution() {
    let app = test_app().await;
    let agents: Vec<Agent> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/agents")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let session: Session = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/sessions")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({"agent_id": agents[0].id, "title": "remote computer attach"}).to_string(),
            ))
            .expect("valid request"),
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
                    "name": "remote-computer-attach-test",
                    "profile": "workspace-write",
                    "pod_name": "agent-remote-computer-attach"
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
                    "worker_id": "remote-manager-attach",
                    "lease_seconds": 120
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;

    let attachment: RemoteComputerAttachment = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/remote-computer-leases/{}/attach", lease.id))
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({
                    "session_id": session.id,
                    "attached_by": "remote-manager-attach",
                    "stale_after_seconds": -1,
                    "metadata": {"execution_enabled": false}
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    assert_eq!(attachment.status, "attached");
    assert_eq!(attachment.session_id, session.id);
    assert_eq!(attachment.lease_id, lease.id);
    assert!(attachment.stale_after.is_some());

    let stale: Vec<RemoteComputerAttachment> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/remote-computer-attachments/stale")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].id, attachment.id);

    let released: RemoteComputerAttachment = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!(
                "/api/remote-computer-attachments/{}/release",
                attachment.id
            ))
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({"reason": "attach state verified"}).to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    assert_eq!(released.status, "released");
    assert_eq!(released.metadata["reason"], "attach state verified");
    assert!(released.released_at.is_some());

    let stale_after_release: Vec<RemoteComputerAttachment> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/remote-computer-attachments/stale")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(stale_after_release.is_empty());

    let events: Vec<SessionEvent> = request_json(
        app.clone(),
        Request::builder()
            .uri(format!("/api/sessions/{}/events", session.id))
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let event_types: Vec<_> = events
        .iter()
        .map(|event| event.event_type.as_str())
        .collect();
    assert!(event_types.contains(&"remote_computer.attached"));
    assert!(event_types.contains(&"remote_computer.detached"));

    let tool_calls: Vec<ToolCall> = request_json(
        app.clone(),
        Request::builder()
            .uri(format!("/api/sessions/{}/tool-calls", session.id))
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(
        tool_calls.is_empty(),
        "remote computer attach lifecycle must not execute tools"
    );

    let jobs: Vec<execution_queue::ExecutionJob> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/execution-jobs")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(
        jobs.is_empty(),
        "remote computer attach lifecycle must not enqueue jobs"
    );

    let audit_logs: Vec<AuditLog> = request_json(
        app,
        Request::builder()
            .uri(format!("/api/sessions/{}/audit-logs", session.id))
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(
        audit_logs
            .iter()
            .any(|log| log.action == "remote_computer.attached")
    );
    assert!(
        audit_logs
            .iter()
            .any(|log| log.action == "remote_computer.detached")
    );
}

#[tokio::test]
async fn remote_computer_stale_reclaim_is_audited_without_execution() {
    let app = test_app().await;
    let agents: Vec<Agent> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/agents")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let session: Session = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/sessions")
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({"agent_id": agents[0].id, "title": "remote computer reclaim"}).to_string(),
            ))
            .expect("valid request"),
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
                    "name": "remote-computer-reclaim-test",
                    "profile": "workspace-write"
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
                    "worker_id": "remote-manager-reclaim",
                    "lease_seconds": 30
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    let attachment: RemoteComputerAttachment = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/api/remote-computer-leases/{}/attach", lease.id))
            .header("content-type", "application/json")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::from(
                json!({
                    "session_id": session.id,
                    "attached_by": "remote-manager-reclaim",
                    "stale_after_seconds": -1
                })
                .to_string(),
            ))
            .expect("valid request"),
    )
    .await;
    assert_eq!(attachment.status, "attached");

    let run: RemoteComputerReclaimRun = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/remote-computers/reclaim-stale")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert_eq!(run.status, "completed");
    assert_eq!(run.stale_attachment_count, 1);
    assert_eq!(run.reclaimed_attachment_count, 1);
    assert_eq!(run.expired_lease_count, 0);
    assert_eq!(run.reclaimed_lease_count, 0);
    assert!(!run.execution_enabled);
    assert_eq!(run.attachments[0].status, "released");
    assert!(run.leases.is_empty());

    let tool_calls: Vec<ToolCall> = request_json(
        app.clone(),
        Request::builder()
            .uri(format!("/api/sessions/{}/tool-calls", session.id))
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(
        tool_calls.is_empty(),
        "stale reclaim must not execute tools"
    );

    let jobs: Vec<execution_queue::ExecutionJob> = request_json(
        app.clone(),
        Request::builder()
            .uri("/api/execution-jobs")
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(jobs.is_empty(), "stale reclaim must not enqueue jobs");

    let events: Vec<SessionEvent> = request_json(
        app.clone(),
        Request::builder()
            .uri(format!("/api/sessions/{}/events", session.id))
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    let event_types: Vec<_> = events
        .iter()
        .map(|event| event.event_type.as_str())
        .collect();
    assert!(event_types.contains(&"remote_computer.attachment_reclaimed"));
    assert!(!event_types.contains(&"remote_computer.lease_reclaimed"));

    let audit_logs: Vec<AuditLog> = request_json(
        app,
        Request::builder()
            .uri(format!("/api/sessions/{}/audit-logs", session.id))
            .header("x-mandoforge-subject", "admin-1")
            .header("x-mandoforge-roles", "admin")
            .body(Body::empty())
            .expect("valid request"),
    )
    .await;
    assert!(
        audit_logs
            .iter()
            .any(|log| log.action == "remote_computer.attachment_reclaimed")
    );
    assert!(
        !audit_logs
            .iter()
            .any(|log| log.action == "remote_computer.lease_reclaimed")
    );
}
