use super::*;

#[tokio::test]
async fn remote_computer_lease_rejects_non_positive_duration() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let computer = state
        .create_remote_computer(CreateRemoteComputer {
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
async fn remote_computer_rejects_second_active_lease_until_released() {
    let state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    let computer = state
        .create_remote_computer(CreateRemoteComputer {
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
