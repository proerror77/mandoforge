use super::*;

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
    assert_eq!(
        select_execution_queue_backend(Some("redis"), true).expect("redis queue"),
        ExecutionQueueBackendSelection::Redis
    );
    assert_eq!(
        select_execution_queue_backend(Some("nats"), true).expect("nats queue"),
        ExecutionQueueBackendSelection::Nats
    );
    assert_eq!(
        select_execution_queue_backend(Some("nats_jetstream"), true).expect("nats jetstream queue"),
        ExecutionQueueBackendSelection::NatsJetstream
    );
    assert_eq!(
        select_execution_queue_backend(Some("jetstream"), true).expect("jetstream alias"),
        ExecutionQueueBackendSelection::NatsJetstream
    );
    assert!(
        select_execution_queue_backend(Some("broker"), true).is_err(),
        "generic broker queue should remain reserved"
    );
}
