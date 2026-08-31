use super::*;

fn on_demand_test_identity(resource_name: &str, pod_name: &str) -> RemoteComputerRuntimeIdentity {
    RemoteComputerRuntimeIdentity::new(
        RemoteComputerSubstrate::AgentSandbox,
        "agent-os-test".to_string(),
        resource_name.to_string(),
        pod_name.to_string(),
        Some(resource_name.to_string()),
        Some(format!("{resource_name}-sandbox")),
        None,
    )
}

async fn pooled_cleanup_test_lease(state: &AppState, name: &str) -> RemoteComputerLease {
    let computer = state
        .create_remote_computer(CreateRemoteComputer {
            id: None,
            name: name.to_string(),
            profile: Some("workspace-write".to_string()),
            namespace: None,
            pod_name: Some(format!("{name}-pod")),
            workspace_path: None,
            state_mount_path: None,
            metadata: Some(json!({"warm_pool": true})),
        })
        .await
        .expect("create pooled cleanup computer");
    state
        .create_remote_computer_lease(
            computer.id,
            CreateRemoteComputerLease {
                session_id: None,
                worker_id: Some("cleanup-worker".to_string()),
                lease_seconds: Some(60),
                metadata: Some(json!({"on_demand": false})),
            },
        )
        .await
        .expect("create pooled cleanup lease")
}

#[tokio::test]
async fn remote_computer_lease_rejects_non_positive_duration() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let computer = state
        .create_remote_computer(CreateRemoteComputer {
            id: None,
            name: "lease-duration-test".to_string(),
            profile: None,
            namespace: None,
            pod_name: None,
            workspace_path: None,
            state_mount_path: None,
            metadata: None,
        })
        .await
        .expect("create remote computer");

    let error = state
        .create_remote_computer_lease(
            computer.id,
            CreateRemoteComputerLease {
                session_id: None,
                worker_id: Some("worker-1".to_string()),
                lease_seconds: Some(0),
                metadata: None,
            },
        )
        .await
        .expect_err("zero lease duration should fail");
    assert_eq!(error.status, StatusCode::BAD_REQUEST);

    assert!(
        error
            .message
            .contains("remote computer lease_seconds must be positive")
    );
}

#[tokio::test]
async fn remote_computer_lease_rejects_client_cleanup_markers() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let computer = state
        .create_remote_computer(CreateRemoteComputer {
            id: None,
            name: "cleanup-marker-test".to_string(),
            profile: None,
            namespace: None,
            pod_name: None,
            workspace_path: None,
            state_mount_path: None,
            metadata: None,
        })
        .await
        .expect("create remote computer");
    let mut cleanup_metadata = json!({});
    cleanup_metadata[REMOTE_COMPUTER_RUNTIME_CLEANUP_MARKER] = json!(true);

    let error = state
        .create_remote_computer_lease(
            computer.id,
            CreateRemoteComputerLease {
                session_id: None,
                worker_id: Some("worker-1".to_string()),
                lease_seconds: Some(60),
                metadata: Some(cleanup_metadata.clone()),
            },
        )
        .await
        .expect_err("client cleanup marker must not be accepted at lease creation");
    assert_eq!(error.status, StatusCode::BAD_REQUEST);

    let mut retry_metadata = json!({});
    retry_metadata[REMOTE_COMPUTER_RUNTIME_CLEANUP_RETRY_MARKER] = json!(true);
    let error = state
        .create_remote_computer_lease(
            computer.id,
            CreateRemoteComputerLease {
                session_id: None,
                worker_id: Some("worker-1".to_string()),
                lease_seconds: Some(60),
                metadata: Some(retry_metadata.clone()),
            },
        )
        .await
        .expect_err("client cleanup retry marker must not be accepted at lease creation");
    assert_eq!(error.status, StatusCode::BAD_REQUEST);

    let mut claim_metadata = json!({});
    claim_metadata[REMOTE_COMPUTER_RUNTIME_CLEANUP_CLAIM_UNTIL_MARKER] = json!(Utc::now());
    let error = state
        .create_remote_computer_lease(
            computer.id,
            CreateRemoteComputerLease {
                session_id: None,
                worker_id: Some("worker-1".to_string()),
                lease_seconds: Some(60),
                metadata: Some(claim_metadata),
            },
        )
        .await
        .expect_err("client cleanup claim marker must not be accepted at lease creation");
    assert_eq!(error.status, StatusCode::BAD_REQUEST);

    let lease = state
        .create_remote_computer_lease(
            computer.id,
            CreateRemoteComputerLease {
                session_id: None,
                worker_id: Some("worker-1".to_string()),
                lease_seconds: Some(60),
                metadata: None,
            },
        )
        .await
        .expect("create clean lease");
    let error = state
        .update_remote_computer_lease_status(
            lease.id,
            "released",
            UpdateRemoteComputerLease {
                reason: Some("spoof cleanup".to_string()),
                metadata: Some(cleanup_metadata),
            },
        )
        .await
        .expect_err("client cleanup marker must not be accepted at lease transition");
    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    let error = state
        .update_remote_computer_lease_status(
            lease.id,
            "released",
            UpdateRemoteComputerLease {
                reason: Some("spoof cleanup retry".to_string()),
                metadata: Some(retry_metadata),
            },
        )
        .await
        .expect_err("client cleanup retry marker must not be accepted at lease transition");
    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        state
            .list_remote_computer_leases()
            .await
            .expect("list leases")[0]
            .status,
        "leased"
    );
}

#[tokio::test]
async fn remote_computer_rejects_duplicate_caller_supplied_id() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let id = Uuid::new_v4();
    state
        .create_remote_computer(CreateRemoteComputer {
            id: Some(id),
            name: "duplicate-id-test".to_string(),
            profile: None,
            namespace: None,
            pod_name: None,
            workspace_path: None,
            state_mount_path: None,
            metadata: None,
        })
        .await
        .expect("create remote computer");

    let error = state
        .create_remote_computer(CreateRemoteComputer {
            id: Some(id),
            name: "duplicate-id-overwrite".to_string(),
            profile: None,
            namespace: None,
            pod_name: None,
            workspace_path: None,
            state_mount_path: None,
            metadata: None,
        })
        .await
        .expect_err("duplicate id should fail");

    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert!(error.message.contains("already exists"));
}

#[tokio::test]
async fn remote_computer_rejects_second_active_lease_until_released() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let computer = state
        .create_remote_computer(CreateRemoteComputer {
            id: None,
            name: "single-active-lease-test".to_string(),
            profile: None,
            namespace: None,
            pod_name: Some("single-active-lease-pod".to_string()),
            workspace_path: None,
            state_mount_path: None,
            metadata: None,
        })
        .await
        .expect("create remote computer");

    let first = state
        .create_remote_computer_lease(
            computer.id,
            CreateRemoteComputerLease {
                session_id: Some(Uuid::new_v4()),
                worker_id: Some("worker-1".to_string()),
                lease_seconds: Some(60),
                metadata: None,
            },
        )
        .await
        .expect("first lease");

    let second = state
        .create_remote_computer_lease(
            computer.id,
            CreateRemoteComputerLease {
                session_id: Some(Uuid::new_v4()),
                worker_id: Some("worker-2".to_string()),
                lease_seconds: Some(60),
                metadata: None,
            },
        )
        .await
        .expect_err("second active lease should fail");
    assert_eq!(second.status, StatusCode::BAD_REQUEST);
    assert!(
        second
            .message
            .contains("Remote computer is not available for lease")
            || second
                .message
                .contains("Remote computer already has an active lease")
    );

    state
        .update_remote_computer_lease_status(
            first.id,
            "released",
            UpdateRemoteComputerLease {
                reason: Some("test complete".to_string()),
                metadata: None,
            },
        )
        .await
        .expect("release first lease");

    let third = state
        .create_remote_computer_lease(
            computer.id,
            CreateRemoteComputerLease {
                session_id: Some(Uuid::new_v4()),
                worker_id: Some("worker-3".to_string()),
                lease_seconds: Some(60),
                metadata: None,
            },
        )
        .await
        .expect("lease after release");
    assert_eq!(third.status, "leased");

    let repeated_release = state
        .update_remote_computer_lease_status(
            first.id,
            "released",
            UpdateRemoteComputerLease {
                reason: Some("late duplicate release".to_string()),
                metadata: None,
            },
        )
        .await
        .expect("duplicate release is idempotent");
    assert_eq!(repeated_release.status, "released");

    let computer_after_duplicate_release = state
        .list_remote_computers()
        .await
        .expect("remote computers")
        .into_iter()
        .find(|candidate| candidate.id == computer.id)
        .expect("remote computer");
    assert_eq!(computer_after_duplicate_release.status, "leased");
}

#[tokio::test]
async fn on_demand_cleanup_failure_keeps_lease_and_record_retryable() {
    let _lock = env_lock().lock().expect("env lock");
    let _mode = EnvVarGuard::set("MANDOFORGE_REMOTE_COMPUTER_RUNNER", "reserved");
    let _mutation = EnvVarGuard::set("MANDOFORGE_REMOTE_COMPUTER_MUTATION_ENABLED", "false");
    let _live = EnvVarGuard::set("MANDOFORGE_REMOTE_COMPUTER_LIVE_MUTATION_ENABLED", "false");
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let identity = on_demand_test_identity("retryable-claim", "retryable-pod");
    let computer = state
        .create_remote_computer(CreateRemoteComputer {
            id: None,
            name: identity.resource_name.clone(),
            profile: Some("agent-sandbox".to_string()),
            namespace: Some(identity.namespace.clone()),
            pod_name: Some(identity.pod_name.clone()),
            workspace_path: None,
            state_mount_path: None,
            metadata: Some(metadata_with_remote_computer_runtime_identity(
                &json!({"on_demand": true}),
                &identity,
            )),
        })
        .await
        .expect("create on-demand computer");
    let lease = state
        .create_remote_computer_lease(
            computer.id,
            CreateRemoteComputerLease {
                session_id: None,
                worker_id: Some("cleanup-worker".to_string()),
                lease_seconds: Some(60),
                metadata: Some(json!({"on_demand": true})),
            },
        )
        .await
        .expect("create lease");

    let error = cleanup_remote_computer_lease_runtime(
        &state,
        &lease,
        None,
        "test_cleanup_failure",
        "released",
    )
    .await
    .expect_err("closed mutation gates must fail cleanup");
    assert_eq!(error.status, StatusCode::INTERNAL_SERVER_ERROR);

    let persisted_lease = state
        .list_remote_computer_leases()
        .await
        .expect("list leases")
        .into_iter()
        .find(|candidate| candidate.id == lease.id)
        .expect("persisted lease");
    assert_eq!(persisted_lease.status, "leased");
    assert!(
        persisted_lease
            .lease_expires_at
            .is_some_and(|expires_at| expires_at <= Utc::now())
    );
    assert_eq!(
        persisted_lease.metadata[REMOTE_COMPUTER_RUNTIME_CLEANUP_RETRY_MARKER],
        json!(true)
    );
    assert_eq!(
        persisted_lease.metadata["runtime_cleanup_retry_reason"],
        json!("test_cleanup_failure")
    );
    assert_eq!(
        persisted_lease.metadata["runtime_cleanup_retry_assignment_status"],
        json!("released")
    );
    assert!(
        persisted_lease.metadata["runtime_cleanup_assignment_id"].is_null(),
        "cleanup without an assignment must preserve an explicit null binding"
    );
    let heartbeat_error = state
        .update_remote_computer_lease_status(
            lease.id,
            "leased",
            UpdateRemoteComputerLease {
                reason: None,
                metadata: Some(json!({"heartbeat": true})),
            },
        )
        .await
        .expect_err("heartbeats must not erase a pending cleanup retry");
    assert!(heartbeat_error.message.contains("cleanup is pending"));
    assert_eq!(
        state
            .list_remote_computer_leases()
            .await
            .expect("list leases after rejected heartbeat")
            .into_iter()
            .find(|candidate| candidate.id == lease.id)
            .expect("persisted retry lease")
            .metadata["runtime_cleanup_retry_reason"],
        json!("test_cleanup_failure")
    );
    let persisted_computer = state
        .list_remote_computers()
        .await
        .expect("list computers")
        .into_iter()
        .find(|candidate| candidate.id == computer.id)
        .expect("persisted computer");
    assert_eq!(persisted_computer.status, "leased");
}

#[tokio::test]
async fn cleanup_claim_rejects_stale_intent_without_overwriting_retry() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let lease = pooled_cleanup_test_lease(&state, "cleanup-intent-fence").await;
    let claimed = state
        .claim_remote_computer_lease_runtime_cleanup(
            lease.id,
            "execution_job_cancel",
            "canceled",
            None,
        )
        .await
        .expect("claim cancellation cleanup")
        .expect("cancellation cleanup owner");
    let mut retry_metadata = claimed.metadata;
    retry_metadata
        .as_object_mut()
        .expect("cleanup metadata object")
        .remove(REMOTE_COMPUTER_RUNTIME_CLEANUP_CLAIM_UNTIL_MARKER);
    state
        .schedule_remote_computer_lease_cleanup_retry(lease.id, retry_metadata)
        .await
        .expect("schedule cancellation cleanup retry");

    let stale_claim = state
        .claim_remote_computer_lease_runtime_cleanup(
            lease.id,
            "expired_lease_reclaim",
            "released",
            Some(Uuid::new_v4()),
        )
        .await
        .expect("stale cleanup claim result");
    assert!(stale_claim.is_none());
    let persisted = state
        .list_remote_computer_leases()
        .await
        .expect("list leases")
        .into_iter()
        .find(|candidate| candidate.id == lease.id)
        .expect("persisted lease");
    assert_eq!(
        persisted.metadata["runtime_cleanup_retry_reason"],
        json!("execution_job_cancel")
    );
    assert_eq!(
        persisted.metadata["runtime_cleanup_retry_assignment_status"],
        json!("canceled")
    );
    assert!(persisted.metadata["runtime_cleanup_assignment_id"].is_null());

    assert!(
        state
            .claim_remote_computer_lease_runtime_cleanup(
                lease.id,
                "execution_job_cancel",
                "canceled",
                None,
            )
            .await
            .expect("resume cancellation cleanup")
            .is_some()
    );
}

#[tokio::test]
async fn equivalent_active_cleanup_claim_is_reported_in_progress() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let lease = pooled_cleanup_test_lease(&state, "equivalent-cleanup-claim").await;
    state
        .claim_remote_computer_lease_runtime_cleanup(
            lease.id,
            "expired_lease_reclaim",
            "released",
            None,
        )
        .await
        .expect("claim stale cleanup")
        .expect("stale cleanup owner");

    let result = cleanup_remote_computer_lease_runtime(
        &state,
        &lease,
        None,
        "provider_terminal_cleanup",
        "released",
    )
    .await
    .expect("equivalent cleanup owner must not fail terminal completion");
    assert_eq!(result["status"], "runtime_cleanup_in_progress");
    assert_eq!(
        state
            .list_remote_computer_leases()
            .await
            .expect("list leases")
            .into_iter()
            .find(|candidate| candidate.id == lease.id)
            .expect("persisted lease")
            .metadata["runtime_cleanup_retry_reason"],
        json!("expired_lease_reclaim")
    );
}

#[tokio::test]
async fn pooled_cleanup_releases_lease_without_deleting_runtime() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let computer = state
        .create_remote_computer(CreateRemoteComputer {
            id: None,
            name: "pooled-cleanup".to_string(),
            profile: Some("workspace-write".to_string()),
            namespace: None,
            pod_name: Some("pooled-cleanup-pod".to_string()),
            workspace_path: None,
            state_mount_path: None,
            metadata: Some(json!({"warm_pool": true})),
        })
        .await
        .expect("create pooled computer");
    let lease = state
        .create_remote_computer_lease(
            computer.id,
            CreateRemoteComputerLease {
                session_id: None,
                worker_id: Some("pool-worker".to_string()),
                lease_seconds: Some(60),
                metadata: Some(json!({"on_demand": false})),
            },
        )
        .await
        .expect("create pooled lease");

    let cleanup =
        cleanup_remote_computer_lease_runtime(&state, &lease, None, "pooled_release", "released")
            .await
            .expect("pooled cleanup");
    assert_eq!(cleanup["runtime"]["delete_attempted"], false);
    assert_eq!(cleanup["lease_status"], "released");

    let persisted_computer = state
        .list_remote_computers()
        .await
        .expect("list computers")
        .into_iter()
        .find(|candidate| candidate.id == computer.id)
        .expect("pooled computer remains");
    assert_eq!(persisted_computer.status, "available");
    assert_eq!(
        persisted_computer.pod_name.as_deref(),
        Some("pooled-cleanup-pod")
    );
}

#[tokio::test]
async fn attention_on_demand_record_rebinds_and_preserves_lease_history() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let original_identity = on_demand_test_identity("rebind-claim", "rebind-pod-old");
    let computer = state
        .create_remote_computer(CreateRemoteComputer {
            id: None,
            name: original_identity.resource_name.clone(),
            profile: Some("agent-sandbox".to_string()),
            namespace: Some(original_identity.namespace.clone()),
            pod_name: Some(original_identity.pod_name.clone()),
            workspace_path: None,
            state_mount_path: None,
            metadata: Some(metadata_with_remote_computer_runtime_identity(
                &json!({"on_demand": true, "generation": 1}),
                &original_identity,
            )),
        })
        .await
        .expect("create computer");
    let lease = state
        .create_remote_computer_lease(
            computer.id,
            CreateRemoteComputerLease {
                session_id: None,
                worker_id: Some("old-worker".to_string()),
                lease_seconds: Some(60),
                metadata: Some(json!({"on_demand": true})),
            },
        )
        .await
        .expect("create lease");
    state
        .update_remote_computer_lease_status(
            lease.id,
            "failed",
            UpdateRemoteComputerLease {
                reason: Some("old runtime deleted".to_string()),
                metadata: None,
            },
        )
        .await
        .expect("mark old lease failed");

    let replacement_identity = on_demand_test_identity("rebind-claim", "rebind-pod-new");
    let rebound = state
        .rebind_on_demand_remote_computer(
            computer.id,
            &replacement_identity,
            metadata_with_remote_computer_runtime_identity(
                &json!({"on_demand": true, "generation": 2}),
                &replacement_identity,
            ),
        )
        .await
        .expect("rebind attention record");
    assert_eq!(rebound.status, "available");
    assert_eq!(rebound.pod_name.as_deref(), Some("rebind-pod-new"));
    assert_eq!(rebound.metadata["generation"], 2);
    let attention = state
        .mark_remote_computer_attention_if_unleased(computer.id, "replacement lease failed")
        .await
        .expect("return rebound runtime to attention");
    assert_eq!(attention.status, "attention");
    assert_eq!(
        attention.metadata["runtime_attention_reason"],
        "replacement lease failed"
    );

    let historical_lease = state
        .list_remote_computer_leases()
        .await
        .expect("list leases")
        .into_iter()
        .find(|candidate| candidate.id == lease.id)
        .expect("historical lease");
    assert_eq!(historical_lease.status, "failed");
}

#[tokio::test]
#[ignore = "requires MANDOFORGE_TEST_POSTGRES_URL"]
async fn postgres_attention_rebind_matches_memory_store_lifecycle() {
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
    state.store = StoreBackend::Postgres(pool.clone());

    let original_identity = on_demand_test_identity("pg-rebind-claim", "pg-rebind-pod-old");
    let computer = state
        .create_remote_computer(CreateRemoteComputer {
            id: None,
            name: original_identity.resource_name.clone(),
            profile: Some("agent-sandbox".to_string()),
            namespace: Some(original_identity.namespace.clone()),
            pod_name: Some(original_identity.pod_name.clone()),
            workspace_path: None,
            state_mount_path: None,
            metadata: Some(metadata_with_remote_computer_runtime_identity(
                &json!({"on_demand": true}),
                &original_identity,
            )),
        })
        .await
        .expect("create postgres computer");
    let historical_lease = state
        .create_remote_computer_lease(
            computer.id,
            CreateRemoteComputerLease {
                session_id: None,
                worker_id: Some("pg-old-worker".to_string()),
                lease_seconds: Some(60),
                metadata: Some(json!({"on_demand": true})),
            },
        )
        .await
        .expect("create postgres lease");
    state
        .update_remote_computer_lease_status(
            historical_lease.id,
            "failed",
            UpdateRemoteComputerLease {
                reason: Some("old runtime deleted".to_string()),
                metadata: None,
            },
        )
        .await
        .expect("mark postgres lease failed");

    let replacement_identity = on_demand_test_identity("pg-rebind-claim", "pg-rebind-pod-new");
    let rebound = state
        .rebind_on_demand_remote_computer(
            computer.id,
            &replacement_identity,
            metadata_with_remote_computer_runtime_identity(
                &json!({"on_demand": true}),
                &replacement_identity,
            ),
        )
        .await
        .expect("rebind postgres computer");
    assert_eq!(rebound.status, "available");
    assert_eq!(rebound.pod_name.as_deref(), Some("pg-rebind-pod-new"));
    let attention = state
        .mark_remote_computer_attention_if_unleased(computer.id, "replacement lease failed")
        .await
        .expect("mark postgres computer attention");
    assert_eq!(attention.status, "attention");

    let available = state
        .rebind_on_demand_remote_computer(
            computer.id,
            &replacement_identity,
            metadata_with_remote_computer_runtime_identity(
                &json!({"on_demand": true}),
                &replacement_identity,
            ),
        )
        .await
        .expect("rebind postgres computer again");
    assert_eq!(available.status, "available");
    let active_lease = state
        .create_remote_computer_lease(
            computer.id,
            CreateRemoteComputerLease {
                session_id: None,
                worker_id: Some("pg-active-worker".to_string()),
                lease_seconds: Some(60),
                metadata: Some(json!({"on_demand": true})),
            },
        )
        .await
        .expect("create active postgres lease");
    sqlx::query("UPDATE remote_computers SET status = 'attention' WHERE id = $1")
        .bind(computer.id)
        .execute(&pool)
        .await
        .expect("force attention conflict fixture");
    let conflict = state
        .rebind_on_demand_remote_computer(
            computer.id,
            &replacement_identity,
            metadata_with_remote_computer_runtime_identity(
                &json!({"on_demand": true}),
                &replacement_identity,
            ),
        )
        .await
        .expect_err("active lease must block postgres rebind");
    assert_eq!(conflict.status, StatusCode::BAD_REQUEST);

    sqlx::query("DELETE FROM remote_computer_leases WHERE remote_computer_id = $1")
        .bind(computer.id)
        .execute(&pool)
        .await
        .expect("clean postgres leases");
    sqlx::query("DELETE FROM remote_computers WHERE id = $1")
        .bind(computer.id)
        .execute(&pool)
        .await
        .expect("clean postgres computer");
    drop(active_lease);
    pool.close().await;
}
