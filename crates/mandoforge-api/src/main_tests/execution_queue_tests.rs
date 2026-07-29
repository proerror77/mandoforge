use super::*;
use chrono::{Duration as ChronoDuration, Utc};

#[test]
fn in_memory_store_requires_explicit_local_opt_in() {
    assert!(!allow_in_memory_store_from_lookup(|_| None));
    assert!(allow_in_memory_store_from_lookup(|key| {
        (key == "MANDOFORGE_ALLOW_IN_MEMORY_STORE").then(|| "true".to_string())
    }));
    assert!(!allow_in_memory_store_from_lookup(|_| Some(
        "false".to_string()
    )));
}

#[test]
fn selects_execution_queue_backend_fail_closed() {
    assert_eq!(
        select_execution_queue_backend(None, false).expect("auto memory"),
        ExecutionQueueBackendSelection::Memory
    );
    assert_eq!(
        select_execution_queue_backend(Some("auto"), true).expect("auto postgres"),
        ExecutionQueueBackendSelection::Postgres
    );
    assert_eq!(
        select_execution_queue_backend(Some("memory"), true).expect("forced memory"),
        ExecutionQueueBackendSelection::Memory
    );
    assert!(
        select_execution_queue_backend(Some("postgres"), false).is_err(),
        "forced postgres queue should require DATABASE_URL"
    );
    assert!(
        select_execution_queue_backend(Some("broker"), true).is_err(),
        "generic broker queue should remain reserved"
    );
}

#[test]
fn rejects_process_local_broker_queue_backends() {
    for requested in ["redis", "nats", "nats_jetstream", "jetstream"] {
        let error = select_execution_queue_backend(Some(requested), true)
            .expect_err("process-local broker queue should be rejected");
        let message = error.to_string();
        assert!(
            message.contains("process-local"),
            "error should explain the process-local risk: {message}"
        );
        assert!(
            message.contains("postgres") && message.contains("auto"),
            "error should name the safe replacement: {message}"
        );
    }
}

#[tokio::test]
async fn api_process_rejects_worker_execution_entrypoints() {
    let _env_guard = env_lock().lock().expect("env lock");
    let _insecure_auth = EnvVarGuard::set("MANDOFORGE_INSECURE_DEV_AUTH", "1");
    let mut state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    state.process_role = ProcessRole::Api;
    let mut headers = HeaderMap::new();
    headers.insert("x-mandoforge-subject", "worker-1".parse().unwrap());
    headers.insert("x-mandoforge-roles", "worker".parse().unwrap());
    headers.insert("x-mandoforge-worker-id", "worker-1".parse().unwrap());

    let session_error =
        handlers::execution_jobs::worker_run_session_loop_job(&state, Uuid::new_v4(), &headers)
            .await
            .expect_err("API process must reject session-loop execution");
    assert_eq!(session_error.status, StatusCode::FORBIDDEN);

    let execution_error =
        handlers::execution_jobs::worker_run_execution_job(&state, Uuid::new_v4(), &headers)
            .await
            .expect_err("API process must reject execution-job execution");
    assert_eq!(execution_error.status, StatusCode::FORBIDDEN);

    let task_board_error = handlers::workflows::worker_get_task_board(&state, &headers)
        .await
        .expect_err("API process must reject daemon task-board polling");
    assert_eq!(task_board_error.status, StatusCode::FORBIDDEN);

    let workflow_error = handlers::workflows::worker_run_workflow_step_run(
        &state,
        Uuid::new_v4(),
        &headers,
        RunWorkflowStepRun {
            agent_id: None,
            worker_id: None,
            lease_seconds: None,
        },
    )
    .await
    .expect_err("API process must reject daemon workflow execution");
    assert_eq!(workflow_error.status, StatusCode::FORBIDDEN);

    state.process_role = ProcessRole::Worker;
    handlers::workflows::worker_get_task_board(&state, &headers)
        .await
        .expect("worker process may poll the task board");
}

#[tokio::test]
async fn api_process_http_run_routes_require_insecure_dev_auth() {
    let _env_guard = env_lock().lock().expect("env lock");
    let _insecure_auth = EnvVarGuard::set("MANDOFORGE_INSECURE_DEV_AUTH", "0");
    let _worker_token = EnvVarGuard::set("MANDOFORGE_WORKER_TOKEN", "worker-token");
    let mut state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    state.process_role = ProcessRole::Api;
    let app = build_router(state);

    for (uri, json_body) in [
        (format!("/api/execution-jobs/{}/run", Uuid::new_v4()), false),
        (
            format!("/api/session-loop-jobs/{}/run", Uuid::new_v4()),
            false,
        ),
        (
            format!("/api/workflow-step-runs/{}/run", Uuid::new_v4()),
            true,
        ),
    ] {
        let mut request = Request::builder()
            .method("POST")
            .uri(uri)
            .header("authorization", "Bearer worker-token")
            .header("x-mandoforge-worker-id", "worker-a");
        if json_body {
            request = request.header("content-type", "application/json");
        }
        let response = app
            .clone()
            .oneshot(
                request
                    .body(if json_body {
                        Body::from("{}")
                    } else {
                        Body::empty()
                    })
                    .expect("valid request"),
            )
            .await
            .expect("run route response");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    drop(_insecure_auth);
    let _insecure_auth = EnvVarGuard::set("MANDOFORGE_INSECURE_DEV_AUTH", "1");
    let mut state = test_state_with_worker(Arc::new(InlineExecutionWorker));
    state.process_role = ProcessRole::Api;
    let app = build_router(state);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/execution-jobs/{}/run", Uuid::new_v4()))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("local compatibility response");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn expired_execution_cancellation_can_be_recovered_after_lease_expiry() {
    let queue = ExecutionQueue::default();
    let job = queue
        .enqueue(ExecutionJobRequest {
            session_id: Uuid::new_v4(),
            environment_id: None,
            approval_id: Uuid::new_v4(),
            tool_call_id: Uuid::new_v4(),
            tool_name: "shell.exec".to_string(),
            max_attempts: None,
        })
        .await
        .expect("queue job");

    queue.start(job.id, "dead-worker").await.expect("start job");
    queue.cancel(job.id).await.expect("request cancellation");

    queue
        .acknowledge_canceled_expired(job.id, Utc::now())
        .await
        .expect_err("live lease should not be recoverable");

    let recovered = queue
        .acknowledge_canceled_expired(job.id, Utc::now() + ChronoDuration::minutes(10))
        .await
        .expect("expired cancel request should recover");
    assert_eq!(recovered.status, ExecutionJobStatus::Canceled);
    assert!(recovered.completed_at.is_some());
}
