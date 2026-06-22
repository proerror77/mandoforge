use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::authorization::Permission;
use crate::{
    AppError, AppState, CreateRemoteComputer, CreateRemoteComputerAttachment,
    CreateRemoteComputerLease, CreateRemoteComputerSidecarHeartbeat, CreateRemoteComputerStateLock,
    ReleaseRemoteComputerStateLock, RemoteComputer, RemoteComputerAttachment,
    RemoteComputerJobAssignment, RemoteComputerLease,
    RemoteComputerArtifactDiscoverRequest, RemoteComputerArtifactSyncRequest,
    RemoteComputerArtifactSyncResponse, RemoteComputerReadinessReport, RemoteComputerReclaimRun,
    RemoteComputerRunnerDryRunRequest, RemoteComputerRunnerDryRunResponse,
    RemoteComputerRunnerReadiness, RemoteComputerSidecarHeartbeat,
    RemoteComputerSidecarRecoveryRun, RemoteComputerStateLock,
    RemoteComputerStateSyncValidationRun,
    acquire_remote_computer_state_lock as acquire_remote_computer_state_lock_impl,
    attach_remote_computer_lease as attach_remote_computer_lease_impl,
    authorize_request,
    create_remote_computer_lease as create_remote_computer_lease_impl,
    discover_remote_computer_artifacts as discover_remote_computer_artifacts_impl,
    dry_run_remote_computer_runner as dry_run_remote_computer_runner_impl,
    get_remote_computer_production_path as get_remote_computer_production_path_impl,
    get_remote_computer_readiness as get_remote_computer_readiness_impl,
    get_remote_computer_runner_readiness as get_remote_computer_runner_readiness_impl,
    heartbeat_remote_computer_lease as heartbeat_remote_computer_lease_impl,
    list_remote_computer_attachments as list_remote_computer_attachments_impl,
    list_remote_computer_job_assignments as list_remote_computer_job_assignments_impl,
    list_remote_computer_leases as list_remote_computer_leases_impl,
    list_stale_remote_computer_attachments as list_stale_remote_computer_attachments_impl,
    list_remote_computer_sidecar_heartbeats as list_remote_computer_sidecar_heartbeats_impl,
    list_remote_computer_state_locks as list_remote_computer_state_locks_impl,
    mutate_remote_computer_runner as mutate_remote_computer_runner_impl, new_audit_log,
    record_remote_computer_sidecar_heartbeat as record_remote_computer_sidecar_heartbeat_impl,
    reclaim_stale_remote_computers as reclaim_stale_remote_computers_impl,
    release_remote_computer_state_lock as release_remote_computer_state_lock_impl,
    release_remote_computer_attachment as release_remote_computer_attachment_impl,
    release_remote_computer_lease as release_remote_computer_lease_impl,
    run_remote_computer_sidecar_recovery as run_remote_computer_sidecar_recovery_impl,
    sync_remote_computer_artifacts as sync_remote_computer_artifacts_impl,
    fail_remote_computer_lease as fail_remote_computer_lease_impl,
    validate_remote_computer_state_sync as validate_remote_computer_state_sync_impl,
    UpdateRemoteComputerAttachment, UpdateRemoteComputerLease,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/remote-computers/readiness",
            get(get_remote_computer_readiness),
        )
        .route(
            "/api/remote-computers/production-path",
            get(get_remote_computer_production_path),
        )
        .route(
            "/api/remote-computers/state-sync/validate",
            post(validate_remote_computer_state_sync),
        )
        .route(
            "/api/remote-computers/artifacts/sync",
            post(sync_remote_computer_artifacts),
        )
        .route(
            "/api/remote-computers/artifacts/discover",
            post(discover_remote_computer_artifacts),
        )
        .route(
            "/api/remote-computers/runner/readiness",
            get(get_remote_computer_runner_readiness),
        )
        .route(
            "/api/remote-computers/runner/dry-run",
            post(dry_run_remote_computer_runner),
        )
        .route(
            "/api/remote-computers/runner/mutate",
            post(mutate_remote_computer_runner),
        )
        .route(
            "/api/remote-computers/state-locks",
            get(list_remote_computer_state_locks).post(acquire_remote_computer_state_lock),
        )
        .route(
            "/api/remote-computers/state-locks/{id}/release",
            post(release_remote_computer_state_lock),
        )
        .route(
            "/api/remote-computers/sidecars/heartbeats",
            get(list_remote_computer_sidecar_heartbeats)
                .post(record_remote_computer_sidecar_heartbeat),
        )
        .route(
            "/api/remote-computers/sidecars/recovery/run",
            post(run_remote_computer_sidecar_recovery),
        )
        .route(
            "/api/remote-computers/reclaim-stale",
            post(reclaim_stale_remote_computers),
        )
        .route(
            "/api/remote-computers",
            get(list_remote_computers).post(create_remote_computer),
        )
        .route(
            "/api/remote-computers/{id}/leases",
            post(create_remote_computer_lease),
        )
        .route(
            "/api/remote-computer-leases/{id}/attach",
            post(attach_remote_computer_lease),
        )
        .route(
            "/api/remote-computer-leases",
            get(list_remote_computer_leases),
        )
        .route(
            "/api/remote-computer-attachments",
            get(list_remote_computer_attachments),
        )
        .route(
            "/api/remote-computer-job-assignments",
            get(list_remote_computer_job_assignments),
        )
        .route(
            "/api/remote-computer-attachments/stale",
            get(list_stale_remote_computer_attachments),
        )
        .route(
            "/api/remote-computer-attachments/{id}/release",
            post(release_remote_computer_attachment),
        )
        .route(
            "/api/remote-computer-leases/{id}/heartbeat",
            post(heartbeat_remote_computer_lease),
        )
        .route(
            "/api/remote-computer-leases/{id}/release",
            post(release_remote_computer_lease),
        )
        .route(
            "/api/remote-computer-leases/{id}/fail",
            post(fail_remote_computer_lease),
        )
}

async fn get_remote_computer_readiness(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<RemoteComputerReadinessReport>, AppError> {
    get_remote_computer_readiness_impl(state, headers).await
}

async fn get_remote_computer_production_path(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    get_remote_computer_production_path_impl(state, headers).await
}

async fn validate_remote_computer_state_sync(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<RemoteComputerStateSyncValidationRun>, AppError> {
    validate_remote_computer_state_sync_impl(state, headers).await
}

async fn sync_remote_computer_artifacts(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RemoteComputerArtifactSyncRequest>,
) -> Result<Json<RemoteComputerArtifactSyncResponse>, AppError> {
    sync_remote_computer_artifacts_impl(state, headers, input).await
}

async fn discover_remote_computer_artifacts(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RemoteComputerArtifactDiscoverRequest>,
) -> Result<Json<RemoteComputerArtifactSyncResponse>, AppError> {
    discover_remote_computer_artifacts_impl(state, headers, input).await
}

async fn get_remote_computer_runner_readiness(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<RemoteComputerRunnerReadiness>, AppError> {
    get_remote_computer_runner_readiness_impl(state, headers).await
}

async fn dry_run_remote_computer_runner(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RemoteComputerRunnerDryRunRequest>,
) -> Result<Json<RemoteComputerRunnerDryRunResponse>, AppError> {
    dry_run_remote_computer_runner_impl(state, headers, input).await
}

async fn mutate_remote_computer_runner(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RemoteComputerRunnerDryRunRequest>,
) -> Result<Json<RemoteComputerRunnerDryRunResponse>, AppError> {
    mutate_remote_computer_runner_impl(state, headers, input).await
}

async fn list_remote_computer_state_locks(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<RemoteComputerStateLock>>, AppError> {
    list_remote_computer_state_locks_impl(state, headers).await
}

async fn acquire_remote_computer_state_lock(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateRemoteComputerStateLock>,
) -> Result<Json<RemoteComputerStateLock>, AppError> {
    acquire_remote_computer_state_lock_impl(state, headers, input).await
}

async fn release_remote_computer_state_lock(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ReleaseRemoteComputerStateLock>,
) -> Result<Json<RemoteComputerStateLock>, AppError> {
    release_remote_computer_state_lock_impl(state, id, headers, input).await
}

async fn list_remote_computer_sidecar_heartbeats(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<RemoteComputerSidecarHeartbeat>>, AppError> {
    list_remote_computer_sidecar_heartbeats_impl(state, headers).await
}

async fn record_remote_computer_sidecar_heartbeat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateRemoteComputerSidecarHeartbeat>,
) -> Result<Json<RemoteComputerSidecarHeartbeat>, AppError> {
    record_remote_computer_sidecar_heartbeat_impl(state, headers, input).await
}

async fn run_remote_computer_sidecar_recovery(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<RemoteComputerSidecarRecoveryRun>, AppError> {
    run_remote_computer_sidecar_recovery_impl(state, headers).await
}

async fn reclaim_stale_remote_computers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<RemoteComputerReclaimRun>, AppError> {
    reclaim_stale_remote_computers_impl(state, headers).await
}

async fn list_remote_computers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<RemoteComputer>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "remote_computers",
        None,
    )
    .await?;
    Ok(Json(state.list_remote_computers().await?))
}

async fn create_remote_computer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateRemoteComputer>,
) -> Result<Json<RemoteComputer>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "remote_computers",
        None,
    )
    .await?;
    let record = state.create_remote_computer(input).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "system",
            None,
            "remote_computer.created",
            "remote_computer",
            Some(record.id),
            json!({
                "name": record.name,
                "profile": record.profile,
                "namespace": record.namespace,
                "pod_name": record.pod_name,
                "execution_enabled": false
            }),
        ))
        .await?;
    Ok(Json(record))
}

async fn create_remote_computer_lease(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateRemoteComputerLease>,
) -> Result<Json<RemoteComputerLease>, AppError> {
    create_remote_computer_lease_impl(state, id, headers, input).await
}

async fn attach_remote_computer_lease(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateRemoteComputerAttachment>,
) -> Result<Json<RemoteComputerAttachment>, AppError> {
    attach_remote_computer_lease_impl(state, id, headers, input).await
}

async fn list_remote_computer_leases(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<RemoteComputerLease>>, AppError> {
    list_remote_computer_leases_impl(state, headers).await
}

async fn list_remote_computer_attachments(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<RemoteComputerAttachment>>, AppError> {
    list_remote_computer_attachments_impl(state, headers).await
}

async fn list_remote_computer_job_assignments(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<RemoteComputerJobAssignment>>, AppError> {
    list_remote_computer_job_assignments_impl(state, headers).await
}

async fn list_stale_remote_computer_attachments(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<RemoteComputerAttachment>>, AppError> {
    list_stale_remote_computer_attachments_impl(state, headers).await
}

async fn release_remote_computer_attachment(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateRemoteComputerAttachment>,
) -> Result<Json<RemoteComputerAttachment>, AppError> {
    release_remote_computer_attachment_impl(state, id, headers, input).await
}

async fn heartbeat_remote_computer_lease(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateRemoteComputerLease>,
) -> Result<Json<RemoteComputerLease>, AppError> {
    heartbeat_remote_computer_lease_impl(state, id, headers, input).await
}

async fn release_remote_computer_lease(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateRemoteComputerLease>,
) -> Result<Json<RemoteComputerLease>, AppError> {
    release_remote_computer_lease_impl(state, id, headers, input).await
}

async fn fail_remote_computer_lease(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateRemoteComputerLease>,
) -> Result<Json<RemoteComputerLease>, AppError> {
    fail_remote_computer_lease_impl(state, id, headers, input).await
}
