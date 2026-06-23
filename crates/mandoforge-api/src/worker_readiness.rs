use crate::WorkerQueueBackendReadiness;

pub(crate) fn worker_queue_backend_readiness(kind: &str) -> WorkerQueueBackendReadiness {
    match kind {
        "postgres" => WorkerQueueBackendReadiness {
            kind: "postgres".to_string(),
            durable: true,
            broker_handoff: false,
            jetstream_enabled: false,
            semantics: "Postgres-backed durable execution_jobs table with lease/retry semantics"
                .to_string(),
        },
        "redis" => WorkerQueueBackendReadiness {
            kind: "redis".to_string(),
            durable: true,
            broker_handoff: true,
            jetstream_enabled: false,
            semantics: "Redis Streams broker-backed queue with XADD/XREADGROUP/XACK handoff"
                .to_string(),
        },
        "nats" => WorkerQueueBackendReadiness {
            kind: "nats".to_string(),
            durable: false,
            broker_handoff: true,
            jetstream_enabled: false,
            semantics: "Core NATS queue subscription handoff; not JetStream durable".to_string(),
        },
        "nats_jetstream" => WorkerQueueBackendReadiness {
            kind: "nats_jetstream".to_string(),
            durable: true,
            broker_handoff: true,
            jetstream_enabled: true,
            semantics: "NATS JetStream durable stream with request/reply publish ack, durable pull-consumer drain, explicit ack, and redelivery semantics".to_string(),
        },
        _ => WorkerQueueBackendReadiness {
            kind: "memory".to_string(),
            durable: false,
            broker_handoff: false,
            jetstream_enabled: false,
            semantics: "process-local in-memory queue for local demo and tests".to_string(),
        },
    }
}
