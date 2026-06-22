use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::authorization::Permission;
use crate::{
    AppError, AppState, Artifact, CreateRemoteComputer, CreateRemoteComputerAttachment,
    CreateRemoteComputerLease, CreateRemoteComputerSidecarHeartbeat, CreateRemoteComputerStateLock,
    ReleaseRemoteComputerStateLock, RemoteComputer, RemoteComputerAttachment,
    RemoteComputerJobAssignment, RemoteComputerLease,
    RemoteComputerArtifactDiscoverRequest, RemoteComputerArtifactSyncRequest,
    RemoteComputerArtifactSyncResponse, RemoteComputerReadinessReport, RemoteComputerReclaimRun,
    RemoteComputerRunnerDryRunRequest, RemoteComputerRunnerDryRunResponse,
    RemoteComputerRunnerReadiness, RemoteComputerSidecarHeartbeat,
    RemoteComputerSidecarRecoveryRun, RemoteComputerStateLock,
    RemoteComputerStateSyncValidationRun,
    authorize_remote_computer_state_lock_release, authorize_request,
    artifact_type_from_path,
    discover_artifact_files,
    dry_run_remote_computer_runner as dry_run_remote_computer_runner_impl,
    get_remote_computer_production_path as get_remote_computer_production_path_impl,
    get_remote_computer_readiness as get_remote_computer_readiness_impl,
    get_remote_computer_runner_readiness as get_remote_computer_runner_readiness_impl,
    ensure_remote_computer_heartbeat_refs_match_session,
    ensure_remote_computer_lock_refs_match_session,
    execute_remote_computer_sidecar_recovery, execute_remote_computer_stale_reclaim,
    mutate_remote_computer_runner as mutate_remote_computer_runner_impl, new_audit_log,
    record_remote_computer_attachment_event, record_remote_computer_lease_event,
    record_remote_computer_sidecar_heartbeat_event, record_remote_computer_state_lock_event,
    normalize_codex_artifact_path, normalize_remote_computer_artifact_dir,
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
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRun,
        "remote_computer",
        Some(input.remote_computer_id),
    )
    .await?;
    if input.artifacts.is_empty() {
        return Err(AppError::bad_request("at least one artifact is required"));
    }
    if input.artifacts.len() > 50 {
        return Err(AppError::bad_request(
            "Remote Computer artifact sync accepts at most 50 artifacts per request",
        ));
    }
    state.get_session(input.session_id).await?;
    let remote_computer = state
        .list_remote_computers()
        .await?
        .into_iter()
        .find(|computer| computer.id == input.remote_computer_id)
        .ok_or_else(|| AppError::not_found("Remote computer not found"))?;
    if let Some(assignment_id) = input.assignment_id {
        let assignment = state
            .list_remote_computer_job_assignments()
            .await?
            .into_iter()
            .find(|assignment| assignment.id == assignment_id)
            .ok_or_else(|| AppError::not_found("Remote computer job assignment not found"))?;
        if assignment.remote_computer_id != input.remote_computer_id
            || assignment.session_id != input.session_id
        {
            return Err(AppError::bad_request(
                "Remote Computer artifact sync assignment does not match session or remote computer",
            ));
        }
    }

    let mut artifacts = Vec::with_capacity(input.artifacts.len());
    for artifact_input in input.artifacts {
        let name = artifact_input.name.trim();
        if name.is_empty() {
            return Err(AppError::bad_request("artifact name is required"));
        }
        let artifact_type = artifact_input.artifact_type.trim();
        if artifact_type.is_empty() {
            return Err(AppError::bad_request("artifact_type is required"));
        }
        let path = normalize_codex_artifact_path(artifact_input.path.as_deref())?;
        let artifact = Artifact {
            id: Uuid::new_v4(),
            session_id: input.session_id,
            artifact_type: artifact_type.to_string(),
            name: name.to_string(),
            path,
            content: artifact_input.content,
            created_at: Utc::now(),
        };
        let artifact = state.insert_artifact(artifact).await?;
        state
            .append_event(
                "worker",
                Some(artifact.id),
                input.session_id,
                "artifact.created",
                json!({
                    "artifact_id": artifact.id,
                    "name": artifact.name,
                    "path": artifact.path,
                    "artifact_type": artifact.artifact_type,
                    "source": "remote_computer",
                    "remote_computer_id": remote_computer.id,
                    "assignment_id": input.assignment_id,
                    "workspace_path": remote_computer.workspace_path,
                    "metadata": artifact_input.metadata,
                }),
            )
            .await?;
        state
            .append_audit_log(new_audit_log(
                Some(input.session_id),
                "worker",
                Some(artifact.id),
                "remote_computer.artifact_synced",
                "artifact",
                Some(artifact.id),
                json!({
                    "name": artifact.name,
                    "path": artifact.path,
                    "artifact_type": artifact.artifact_type,
                    "remote_computer_id": remote_computer.id,
                    "assignment_id": input.assignment_id,
                }),
            ))
            .await?;
        artifacts.push(artifact);
    }

    Ok(Json(RemoteComputerArtifactSyncResponse {
        session_id: input.session_id,
        remote_computer_id: input.remote_computer_id,
        assignment_id: input.assignment_id,
        artifact_count: artifacts.len(),
        artifacts,
    }))
}

async fn discover_remote_computer_artifacts(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RemoteComputerArtifactDiscoverRequest>,
) -> Result<Json<RemoteComputerArtifactSyncResponse>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRun,
        "remote_computer",
        Some(input.remote_computer_id),
    )
    .await?;
    if input.max_files == 0 || input.max_files > 50 {
        return Err(AppError::bad_request(
            "Remote Computer artifact discovery accepts between 1 and 50 files per request",
        ));
    }
    state.get_session(input.session_id).await?;
    let remote_computer = state
        .list_remote_computers()
        .await?
        .into_iter()
        .find(|computer| computer.id == input.remote_computer_id)
        .ok_or_else(|| AppError::not_found("Remote computer not found"))?;
    if let Some(assignment_id) = input.assignment_id {
        let assignment = state
            .list_remote_computer_job_assignments()
            .await?
            .into_iter()
            .find(|assignment| assignment.id == assignment_id)
            .ok_or_else(|| AppError::not_found("Remote computer job assignment not found"))?;
        if assignment.remote_computer_id != input.remote_computer_id
            || assignment.session_id != input.session_id
        {
            return Err(AppError::bad_request(
                "Remote Computer artifact discovery assignment does not match session or remote computer",
            ));
        }
    }

    let artifact_dir = normalize_remote_computer_artifact_dir(&input.artifact_dir)?;
    let workspace_path = std::path::PathBuf::from(&remote_computer.workspace_path);
    let discovery_root = workspace_path.join(&artifact_dir);
    let root_metadata = tokio::fs::metadata(&discovery_root)
        .await
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                AppError::not_found("Remote Computer artifact discovery directory not found")
            } else {
                AppError::from(error)
            }
        })?;
    if !root_metadata.is_dir() {
        return Err(AppError::bad_request(
            "Remote Computer artifact discovery path must be a directory",
        ));
    }

    let discovered_files =
        discover_artifact_files(&discovery_root, input.max_files, 1_048_576).await?;
    if discovered_files.is_empty() {
        return Err(AppError::bad_request(
            "Remote Computer artifact discovery found no files",
        ));
    }

    let mut artifacts = Vec::with_capacity(discovered_files.len());
    for discovered in discovered_files {
        let relative_path = discovered
            .path
            .strip_prefix(&workspace_path)
            .unwrap_or(&discovered.path)
            .to_string_lossy()
            .replace('\\', "/");
        let name = discovered
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("artifact")
            .to_string();
        let artifact = Artifact {
            id: Uuid::new_v4(),
            session_id: input.session_id,
            artifact_type: artifact_type_from_path(&discovered.path),
            name,
            path: Some(relative_path.clone()),
            content: json!({
                "text": discovered.content,
                "bytes": discovered.bytes,
                "source": "remote_computer_artifact_discovery",
            }),
            created_at: Utc::now(),
        };
        let artifact = state.insert_artifact(artifact).await?;
        state
            .append_event(
                "worker",
                Some(artifact.id),
                input.session_id,
                "artifact.created",
                json!({
                    "artifact_id": artifact.id,
                    "name": artifact.name,
                    "path": artifact.path,
                    "artifact_type": artifact.artifact_type,
                    "source": "remote_computer_artifact_discovery",
                    "remote_computer_id": remote_computer.id,
                    "assignment_id": input.assignment_id,
                    "workspace_path": remote_computer.workspace_path,
                    "artifact_dir": artifact_dir,
                }),
            )
            .await?;
        state
            .append_audit_log(new_audit_log(
                Some(input.session_id),
                "worker",
                Some(artifact.id),
                "remote_computer.artifact_discovered",
                "artifact",
                Some(artifact.id),
                json!({
                    "name": artifact.name,
                    "path": artifact.path,
                    "artifact_type": artifact.artifact_type,
                    "remote_computer_id": remote_computer.id,
                    "assignment_id": input.assignment_id,
                    "artifact_dir": artifact_dir,
                }),
            ))
            .await?;
        artifacts.push(artifact);
    }

    Ok(Json(RemoteComputerArtifactSyncResponse {
        session_id: input.session_id,
        remote_computer_id: input.remote_computer_id,
        assignment_id: input.assignment_id,
        artifact_count: artifacts.len(),
        artifacts,
    }))
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
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "remote_computer_state_locks",
        None,
    )
    .await?;
    Ok(Json(state.list_remote_computer_state_locks().await?))
}

async fn acquire_remote_computer_state_lock(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateRemoteComputerStateLock>,
) -> Result<Json<RemoteComputerStateLock>, AppError> {
    let session_id = input.session_id.ok_or_else(|| {
        AppError::bad_request("Remote Computer state lock requires session_id for scoped access")
    })?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRun,
        "session",
        Some(session_id),
    )
    .await?;
    ensure_remote_computer_lock_refs_match_session(&state, &input, session_id).await?;
    let lock = state.acquire_remote_computer_state_lock(input).await?;
    record_remote_computer_state_lock_event(&state, &lock, "remote_computer.state_lock_acquired")
        .await?;
    Ok(Json(lock))
}

async fn release_remote_computer_state_lock(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ReleaseRemoteComputerStateLock>,
) -> Result<Json<RemoteComputerStateLock>, AppError> {
    let existing = state
        .list_remote_computer_state_locks()
        .await?
        .into_iter()
        .find(|lock| lock.id == id)
        .ok_or_else(|| AppError::not_found("Remote Computer state lock not found"))?;
    authorize_remote_computer_state_lock_release(&state, &headers, &existing).await?;
    let lock = state.release_remote_computer_state_lock(id, input).await?;
    record_remote_computer_state_lock_event(&state, &lock, "remote_computer.state_lock_released")
        .await?;
    Ok(Json(lock))
}

async fn list_remote_computer_sidecar_heartbeats(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<RemoteComputerSidecarHeartbeat>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "remote_computer_sidecar_heartbeats",
        None,
    )
    .await?;
    Ok(Json(state.list_remote_computer_sidecar_heartbeats().await?))
}

async fn record_remote_computer_sidecar_heartbeat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateRemoteComputerSidecarHeartbeat>,
) -> Result<Json<RemoteComputerSidecarHeartbeat>, AppError> {
    let session_id = input.session_id.ok_or_else(|| {
        AppError::bad_request("Remote Computer sidecar heartbeat requires session_id")
    })?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRun,
        "session",
        Some(session_id),
    )
    .await?;
    ensure_remote_computer_heartbeat_refs_match_session(&state, &input, session_id).await?;
    let heartbeat = state
        .record_remote_computer_sidecar_heartbeat(input)
        .await?;
    record_remote_computer_sidecar_heartbeat_event(&state, &heartbeat).await?;
    Ok(Json(heartbeat))
}

async fn run_remote_computer_sidecar_recovery(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<RemoteComputerSidecarRecoveryRun>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "remote_computer_sidecar_recovery",
        None,
    )
    .await?;
    Ok(Json(
        execute_remote_computer_sidecar_recovery(&state).await?,
    ))
}

async fn reclaim_stale_remote_computers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<RemoteComputerReclaimRun>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "remote_computers",
        None,
    )
    .await?;
    Ok(Json(execute_remote_computer_stale_reclaim(&state).await?))
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
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "remote_computer",
        Some(id),
    )
    .await?;
    let lease = state.create_remote_computer_lease(id, input).await?;
    record_remote_computer_lease_event(&state, &lease, "remote_computer.leased").await?;
    Ok(Json(lease))
}

async fn attach_remote_computer_lease(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateRemoteComputerAttachment>,
) -> Result<Json<RemoteComputerAttachment>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "remote_computer_lease",
        Some(id),
    )
    .await?;
    let attachment = state.create_remote_computer_attachment(id, input).await?;
    record_remote_computer_attachment_event(&state, &attachment, "remote_computer.attached")
        .await?;
    Ok(Json(attachment))
}

async fn list_remote_computer_leases(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<RemoteComputerLease>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "remote_computer_leases",
        None,
    )
    .await?;
    Ok(Json(state.list_remote_computer_leases().await?))
}

async fn list_remote_computer_attachments(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<RemoteComputerAttachment>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "remote_computer_attachments",
        None,
    )
    .await?;
    Ok(Json(state.list_remote_computer_attachments().await?))
}

async fn list_remote_computer_job_assignments(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<RemoteComputerJobAssignment>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "remote_computer_job_assignments",
        None,
    )
    .await?;
    Ok(Json(state.list_remote_computer_job_assignments().await?))
}

async fn list_stale_remote_computer_attachments(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<RemoteComputerAttachment>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "remote_computer_attachments",
        None,
    )
    .await?;
    Ok(Json(state.list_stale_remote_computer_attachments().await?))
}

async fn release_remote_computer_attachment(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateRemoteComputerAttachment>,
) -> Result<Json<RemoteComputerAttachment>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "remote_computer_attachment",
        Some(id),
    )
    .await?;
    let attachment = state.release_remote_computer_attachment(id, input).await?;
    record_remote_computer_attachment_event(&state, &attachment, "remote_computer.detached")
        .await?;
    Ok(Json(attachment))
}

async fn heartbeat_remote_computer_lease(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateRemoteComputerLease>,
) -> Result<Json<RemoteComputerLease>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "remote_computer_lease",
        Some(id),
    )
    .await?;
    let lease = state
        .update_remote_computer_lease_status(id, "leased", input)
        .await?;
    record_remote_computer_lease_event(&state, &lease, "remote_computer.heartbeat").await?;
    Ok(Json(lease))
}

async fn release_remote_computer_lease(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateRemoteComputerLease>,
) -> Result<Json<RemoteComputerLease>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "remote_computer_lease",
        Some(id),
    )
    .await?;
    let lease = state
        .update_remote_computer_lease_status(id, "released", input)
        .await?;
    record_remote_computer_lease_event(&state, &lease, "remote_computer.released").await?;
    Ok(Json(lease))
}

async fn fail_remote_computer_lease(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateRemoteComputerLease>,
) -> Result<Json<RemoteComputerLease>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "remote_computer_lease",
        Some(id),
    )
    .await?;
    let lease = state
        .update_remote_computer_lease_status(id, "failed", input)
        .await?;
    record_remote_computer_lease_event(&state, &lease, "remote_computer.failed").await?;
    Ok(Json(lease))
}
