use super::*;

#[tokio::test]
async fn worker_readiness_blocks_enabled_host_shell_runner() {
    let _env_guard = env_lock().lock().expect("env lock");
    let _host_shell = EnvVarGuard::set("MANDOFORGE_ALLOW_HOST_SHELL_EXEC", "1");
    let _runner = EnvVarGuard::remove("MANDOFORGE_SHELL_RUNNER");
    let state = test_state_with_worker(Arc::new(QueueBackedExecutionWorker));

    let report = build_worker_readiness(&state)
        .await
        .expect("worker readiness");

    assert!(
        report.attention_items.iter().any(|item| {
            item.kind == "host_shell_runner_enabled" && item.severity == "critical"
        })
    );
    assert!(
        report
            .production_ops
            .blocking_reasons
            .iter()
            .any(|reason| reason.contains("host shell runner is enabled"))
    );
}
