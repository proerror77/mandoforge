use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::authorization::{AuthorizationRequest, Permission, Role};
use crate::remote_computer_runner::{
    RemoteComputerRunnerConfig, remote_computer_runner_for_config,
};
use crate::{
    AppError, AppState, Artifact, CreateRemoteComputer, CreateRemoteComputerAttachment,
    CreateRemoteComputerLease, CreateRemoteComputerSidecarHeartbeat, CreateRemoteComputerStateLock,
    ReleaseRemoteComputerStateLock, RemoteComputer, RemoteComputerArtifactDiscoverRequest,
    RemoteComputerArtifactSyncRequest, RemoteComputerArtifactSyncResponse,
    RemoteComputerAttachment, RemoteComputerJobAssignment, RemoteComputerLease,
    RemoteComputerReadinessReport, RemoteComputerReclaimRun, RemoteComputerRunnerDryRunRequest,
    RemoteComputerRunnerDryRunResponse, RemoteComputerRunnerReadiness,
    RemoteComputerSidecarHeartbeat, RemoteComputerSidecarRecoveryRun, RemoteComputerStateLock,
    RemoteComputerStateSyncValidationRun, UpdateRemoteComputerAttachment,
    UpdateRemoteComputerLease, artifact_type_from_path,
    authorize_remote_computer_state_lock_release, authorize_request,
    build_remote_computer_execution_transport_readiness,
    build_remote_computer_production_path_payload, build_remote_computer_readiness,
    build_remote_computer_runner_readiness, build_worker_readiness, dedupe_strings,
    discover_artifact_files, enforce_resource_scope,
    ensure_remote_computer_heartbeat_refs_match_session,
    ensure_remote_computer_lock_refs_match_session, execute_remote_computer_sidecar_recovery,
    execute_remote_computer_stale_reclaim, execute_remote_computer_state_sync_controller,
    new_audit_log, normalize_codex_artifact_path, normalize_remote_computer_artifact_dir,
    principal_from_request, record_remote_computer_attachment_event,
    record_remote_computer_lease_event, record_remote_computer_sidecar_heartbeat_event,
    record_remote_computer_state_lock_event, remote_computer_runner_request_is_exec,
    remote_computer_runner_response_for_audit, remote_computer_state_sync_base_issues,
    remote_computer_state_sync_controller_configured,
    remote_computer_state_sync_controller_required, visible_session_ids_for_principal,
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
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "remote_computers",
        None,
    )
    .await?;
    Ok(Json(build_remote_computer_readiness(&state).await?))
}

async fn get_remote_computer_production_path(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "remote_computer_production_path",
        None,
    )
    .await?;
    let generated_at = Utc::now();
    let readiness = build_remote_computer_readiness(&state).await?;
    let execution_transport = build_remote_computer_execution_transport_readiness(&state).await?;
    let worker_readiness = build_worker_readiness(&state).await?;
    let audit_logs = state.list_audit_logs(None).await?;
    Ok(Json(build_remote_computer_production_path_payload(
        generated_at,
        readiness,
        execution_transport,
        worker_readiness,
        &audit_logs,
    )))
}

async fn validate_remote_computer_state_sync(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<RemoteComputerStateSyncValidationRun>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "remote_computer_state_sync".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;

    let checked_at = Utc::now();
    let lookup = |key: &str| std::env::var(key).ok();
    let readiness = build_remote_computer_readiness(&state).await?;
    let state_filesystem = readiness.state_filesystem;
    let controller_required = remote_computer_state_sync_controller_required(&lookup);
    let controller_configured = remote_computer_state_sync_controller_configured(&lookup);
    let mut issues = remote_computer_state_sync_base_issues(&state_filesystem);
    if controller_required && !controller_configured {
        issues.push("state sync controller is required but not configured".to_string());
    }
    let mut controller_execution = json!({
        "attempted": false,
        "status": "skipped",
        "reason": if controller_configured {
            if issues.is_empty() {
                "controller_not_required"
            } else {
                "state_sync_not_ready"
            }
        } else {
            "controller_not_configured"
        }
    });
    if controller_configured && issues.is_empty() {
        match execute_remote_computer_state_sync_controller(
            &lookup,
            &principal.subject_id,
            checked_at,
            &state_filesystem,
        )
        .await
        {
            Ok(execution) => {
                if execution.get("status").and_then(Value::as_str) != Some("validated") {
                    issues.push("state sync controller did not validate".to_string());
                }
                controller_execution = execution;
            }
            Err(error) => {
                issues.push("state sync controller failed".to_string());
                controller_execution = json!({
                    "attempted": true,
                    "status": "failed",
                    "error": error.message
                });
            }
        }
    }
    if controller_required
        && controller_execution.get("status").and_then(Value::as_str) != Some("validated")
    {
        issues.push("state sync controller evidence is missing or not validated".to_string());
    }
    dedupe_strings(&mut issues);
    let status = if issues.is_empty() {
        "validated"
    } else {
        "blocked"
    }
    .to_string();
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "remote_computer.production_state_sync_validation",
            "remote_computer_state_sync",
            None,
            json!({
                "subject": principal.subject_id,
                "status": status,
                "controller_required": controller_required,
                "controller_configured": controller_configured,
                "controller_execution": controller_execution,
                "provider": state_filesystem.provider,
                "distributed_filesystem_configured": state_filesystem.distributed_filesystem_configured,
                "production_profile_present": state_filesystem.production_profile_present,
                "state_contract_present": state_filesystem.state_contract_present,
                "lock_manager_configured": state_filesystem.lock_manager_configured,
                "conflict_policy": state_filesystem.conflict_policy,
                "issues": issues,
                "checked_at": checked_at,
            }),
        ))
        .await?;
    Ok(Json(RemoteComputerStateSyncValidationRun {
        status,
        checked_at,
        controller_required,
        controller_configured,
        controller_execution,
        issues,
    }))
}

async fn authorize_remote_computer_artifact_access(
    state: &AppState,
    headers: &HeaderMap,
    remote_computer_id: Uuid,
    session_id: Uuid,
    assignment_id: Option<Uuid>,
    operation: &str,
) -> Result<(), AppError> {
    authorize_request(
        state,
        headers,
        Permission::SessionsRun,
        "remote_computer",
        Some(remote_computer_id),
    )
    .await?;

    let principal = principal_from_request(state, headers).await?;
    if principal.roles.contains(&Role::Admin) {
        return Ok(());
    }

    let visible_session_ids = visible_session_ids_for_principal(state, &principal).await?;
    if !visible_session_ids.contains(&session_id) {
        return Err(AppError::forbidden(format!(
            "principal {} cannot {} Remote Computer artifacts for session {}",
            principal.subject_id, operation, session_id
        )));
    }

    let leases = state.list_remote_computer_leases().await?;
    let lease_is_active = |lease_id: Uuid| {
        leases.iter().any(|lease| {
            lease.id == lease_id
                && lease.remote_computer_id == remote_computer_id
                && lease
                    .session_id
                    .is_none_or(|leased_session_id| leased_session_id == session_id)
                && lease.status == "leased"
                && lease
                    .lease_expires_at
                    .is_none_or(|lease_expires_at| lease_expires_at > Utc::now())
        })
    };

    if let Some(assignment_id) = assignment_id {
        let assignment = state
            .list_remote_computer_job_assignments()
            .await?
            .into_iter()
            .find(|assignment| assignment.id == assignment_id)
            .ok_or_else(|| {
                AppError::forbidden("Remote Computer artifact assignment is not valid")
            })?;
        if assignment.remote_computer_id == remote_computer_id
            && assignment.session_id == session_id
            && assignment.status == "assigned"
            && lease_is_active(assignment.lease_id)
        {
            return Ok(());
        }
        return Err(AppError::forbidden(
            "Remote Computer artifact assignment is not active for this session",
        ));
    }

    let has_active_assignment = state
        .list_remote_computer_job_assignments()
        .await?
        .into_iter()
        .any(|assignment| {
            assignment.remote_computer_id == remote_computer_id
                && assignment.session_id == session_id
                && assignment.status == "assigned"
                && lease_is_active(assignment.lease_id)
        });
    if has_active_assignment {
        return Ok(());
    }

    let has_active_attachment = state
        .list_remote_computer_attachments()
        .await?
        .into_iter()
        .any(|attachment| {
            attachment.remote_computer_id == remote_computer_id
                && attachment.session_id == session_id
                && attachment.status == "attached"
                && attachment
                    .stale_after
                    .is_none_or(|stale_after| stale_after > Utc::now())
                && lease_is_active(attachment.lease_id)
        });
    if has_active_attachment {
        return Ok(());
    }

    Err(AppError::forbidden(format!(
        "principal {} has no active Remote Computer lease binding for artifact {}",
        principal.subject_id, operation
    )))
}

async fn sync_remote_computer_artifacts(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RemoteComputerArtifactSyncRequest>,
) -> Result<Json<RemoteComputerArtifactSyncResponse>, AppError> {
    authorize_remote_computer_artifact_access(
        &state,
        &headers,
        input.remote_computer_id,
        input.session_id,
        input.assignment_id,
        "sync",
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
    authorize_remote_computer_artifact_access(
        &state,
        &headers,
        input.remote_computer_id,
        input.session_id,
        input.assignment_id,
        "discover",
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
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "remote_computer_runner",
        None,
    )
    .await?;
    Ok(Json(build_remote_computer_runner_readiness()))
}

async fn dry_run_remote_computer_runner(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RemoteComputerRunnerDryRunRequest>,
) -> Result<Json<RemoteComputerRunnerDryRunResponse>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "remote_computer_runner",
        input.remote_computer_id,
    )
    .await?;
    let config = RemoteComputerRunnerConfig::from_env();
    let runner = remote_computer_runner_for_config(&config);
    let session_id = input.session_id;
    let remote_computer_id = input.remote_computer_id;
    let response = runner.dry_run(&config, input).await;
    state
        .append_audit_log(new_audit_log(
            session_id,
            "system",
            None,
            "remote_computer.runner_dry_run",
            "remote_computer_runner",
            remote_computer_id,
            json!({
                "config": config,
                "response": remote_computer_runner_response_for_audit(&response),
                "execution_enabled": response.execution_enabled
            }),
        ))
        .await?;
    Ok(Json(response))
}

async fn mutate_remote_computer_runner(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RemoteComputerRunnerDryRunRequest>,
) -> Result<Json<RemoteComputerRunnerDryRunResponse>, AppError> {
    if remote_computer_runner_request_is_exec(&input) {
        return Err(AppError::bad_request(
            "remote computer runner mutate does not accept direct exec operations; use an approved execution job",
        ));
    }
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "remote_computer_runner",
        input.remote_computer_id,
    )
    .await?;
    let config = RemoteComputerRunnerConfig::from_env();
    let runner = remote_computer_runner_for_config(&config);
    let session_id = input.session_id;
    let remote_computer_id = input.remote_computer_id;
    let response = runner.mutate(&config, input).await;
    state
        .append_audit_log(new_audit_log(
            session_id,
            "system",
            None,
            "remote_computer.runner_mutate",
            "remote_computer_runner",
            remote_computer_id,
            json!({
                "config": config,
                "response": remote_computer_runner_response_for_audit(&response),
                "execution_enabled": response.execution_enabled
            }),
        ))
        .await?;
    Ok(Json(response))
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
