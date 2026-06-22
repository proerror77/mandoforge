use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::codex_app_server::{
    CodexCommandRequest, CodexCommandResponse, CodexInterruptResponse, CodexThreadRequest,
    CodexThreadResponse, CodexTurnRequest, CodexTurnResponse,
};
use crate::{
    AppError, AppState, Artifact, AuthorizationRequest, CodexAppServerControlPlaneSummary,
    CodexAppServerOpsValidationRun, CodexAppServerPollRequest, CodexAppServerPollResponse,
    CodexAppServerRun, CodexAppServerStalePollRequest, CodexAppServerStalePollRun,
    CodexAppServerTraceDetail, CodexAppServerTraceSummary, CodexArtifactSyncRequest,
    CodexArtifactSyncResponse, Permission, authorize_request,
    build_codex_app_server_control_plane_summary, build_codex_app_server_trace_detail,
    build_codex_app_server_trace_summary, codex_app_server_config,
    codex_app_server_deployment_controller_configured,
    codex_app_server_deployment_controller_required, codex_app_server_ops_controller_configured,
    codex_app_server_ops_controller_required, dedupe_strings, enforce_resource_scope,
    execute_codex_app_server_deployment_controller, execute_codex_app_server_ops_controller,
    execute_stale_codex_app_server_polls, new_audit_log, normalize_codex_artifact_path,
    poll_codex_app_server_run_inner, principal_from_request,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/codex-app-server/health",
            get(get_codex_app_server_health),
        )
        .route(
            "/api/codex-app-server/deployment/validate",
            post(validate_codex_app_server_deployment),
        )
        .route(
            "/api/codex-app-server/ops/validate",
            post(validate_codex_app_server_ops),
        )
        .route(
            "/api/codex-app-server/runs",
            get(list_codex_app_server_runs),
        )
        .route(
            "/api/codex-app-server/traces",
            get(get_codex_app_server_traces),
        )
        .route(
            "/api/codex-app-server/control-plane/summary",
            get(get_codex_app_server_control_plane_summary),
        )
        .route(
            "/api/codex-app-server/traces/{trace_key}",
            get(get_codex_app_server_trace_detail),
        )
        .route(
            "/api/codex-app-server/runs/{run_id}/poll",
            post(poll_codex_app_server_run),
        )
        .route(
            "/api/codex-app-server/runs/poll-stale",
            post(poll_stale_codex_app_server_runs),
        )
        .route("/api/codex-app-server/threads", post(create_codex_thread))
        .route(
            "/api/codex-app-server/threads/{thread_id}/turns",
            post(create_codex_turn),
        )
        .route(
            "/api/codex-app-server/turns/{turn_id}/interrupt",
            post(interrupt_codex_turn),
        )
        .route(
            "/api/codex-app-server/turns/{turn_id}/commands",
            post(execute_codex_command),
        )
        .route(
            "/api/codex-app-server/artifacts/sync",
            post(sync_codex_artifacts),
        )
}

async fn get_codex_app_server_health(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "codex_app_server",
        None,
    )
    .await?;
    let checked_at = Utc::now();
    let Some(config) = state.codex_app_server_config.as_ref() else {
        return Ok(Json(json!({
            "status": "reserved",
            "healthy": false,
            "issues": ["Codex App Server is disabled until MANDOFORGE_CODEX_APP_SERVER_URL is configured"],
            "checks": {"provider": "reserved"},
            "checked_at": checked_at,
        })));
    };
    match state.codex_app_server_client.health_check(config).await {
        Ok(()) => Ok(Json(json!({
            "status": "healthy",
            "healthy": true,
            "issues": [],
            "checks": {
                "endpoint_configured": true,
                "timeout_seconds": config.timeout_seconds,
            },
            "checked_at": checked_at,
        }))),
        Err(error) => Ok(Json(json!({
            "status": "unhealthy",
            "healthy": false,
            "issues": [error.message],
            "checks": {
                "endpoint_configured": true,
                "timeout_seconds": config.timeout_seconds,
            },
            "checked_at": checked_at,
        }))),
    }
}

async fn validate_codex_app_server_deployment(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "codex_app_server".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let checked_at = Utc::now();
    let mut issues = Vec::new();
    let mut healthy = false;
    let mut timeout_seconds = None;
    let configured = state.codex_app_server_config.is_some();
    let lookup = |key: &str| std::env::var(key).ok();
    let controller_required = codex_app_server_deployment_controller_required(&lookup);
    let controller_configured = codex_app_server_deployment_controller_configured(&lookup);
    let mut controller_execution = json!({
        "attempted": false,
        "status": "skipped",
        "reason": if controller_configured {
            "app_server_health_not_ready"
        } else {
            "controller_not_configured"
        }
    });

    if let Some(config) = state.codex_app_server_config.as_ref() {
        timeout_seconds = Some(config.timeout_seconds);
        match state.codex_app_server_client.health_check(config).await {
            Ok(()) => {
                healthy = true;
                if controller_configured {
                    match execute_codex_app_server_deployment_controller(
                        &lookup,
                        &principal.subject_id,
                        checked_at,
                        config,
                    )
                    .await
                    {
                        Ok(execution) => {
                            let controller_status = execution
                                .get("status")
                                .and_then(Value::as_str)
                                .unwrap_or("failed")
                                .to_string();
                            controller_execution = execution;
                            if controller_status != "validated" {
                                healthy = false;
                                issues.push(
                                    "Codex App Server deployment controller did not validate"
                                        .to_string(),
                                );
                            }
                        }
                        Err(error) => {
                            healthy = false;
                            issues
                                .push("Codex App Server deployment controller failed".to_string());
                            controller_execution = json!({
                                "attempted": true,
                                "status": "failed",
                                "error": error.message
                            });
                        }
                    }
                }
            }
            Err(error) => {
                issues.push(error.message);
            }
        }
    } else {
        issues.push(
            "Codex App Server is disabled until MANDOFORGE_CODEX_APP_SERVER_URL is configured"
                .to_string(),
        );
    }
    if healthy && controller_required && !controller_configured {
        healthy = false;
        issues.push(
            "Codex App Server deployment controller is required but not configured".to_string(),
        );
    }

    let status = if healthy { "healthy" } else { "blocked" };
    let result = json!({
        "status": status,
        "healthy": healthy,
        "configured": configured,
        "endpoint_configured": configured,
        "timeout_seconds": timeout_seconds,
        "controller_required": controller_required,
        "controller_configured": controller_configured,
        "controller_execution": controller_execution,
        "issues": issues,
        "subject": principal.subject_id,
        "checked_at": checked_at,
    });
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "codex_app_server.deployment_validation",
            "codex_app_server",
            None,
            result.clone(),
        ))
        .await?;
    Ok(Json(result))
}

async fn validate_codex_app_server_ops(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<CodexAppServerOpsValidationRun>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "codex_app_server".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let checked_at = Utc::now();
    let runs = state.list_codex_app_server_runs().await?;
    let audit_logs = state.list_audit_logs(None).await?;
    let summary = build_codex_app_server_control_plane_summary(
        state.codex_app_server_config.as_ref(),
        &runs,
        &audit_logs,
        checked_at,
    );
    let lookup = |key: &str| std::env::var(key).ok();
    let controller_required = codex_app_server_ops_controller_required(&lookup);
    let controller_configured = codex_app_server_ops_controller_configured(&lookup);
    let mut controller_execution = json!({
        "attempted": false,
        "status": "skipped",
        "reason": if controller_configured {
            "controller_not_attempted"
        } else {
            "codex_app_server_ops_controller_not_configured"
        }
    });
    let mut issues = Vec::new();
    let production_blocked_only_by_missing_controller_evidence = controller_required
        && controller_configured
        && summary.production_ops.configured
        && summary.production_ops.failed_turn_count == 0
        && summary.production_ops.stale_candidate_count == 0
        && summary.production_ops.latest_stale_poll_at.is_some()
        && summary.production_ops.latest_stale_poll_failed_count == 0
        && summary
            .production_ops
            .latest_stale_poll_age_hours
            .map_or(true, |age_hours| age_hours < 24)
        && !summary.production_ops.latest_controller_validated;
    if summary.production_ops.production_blocked
        && !production_blocked_only_by_missing_controller_evidence
    {
        issues.push(summary.production_ops.message.clone());
    }
    if controller_configured {
        match execute_codex_app_server_ops_controller(
            &lookup,
            Some(principal.subject_id.as_str()),
            checked_at,
            &summary,
        )
        .await
        {
            Ok(execution) => {
                if execution.get("status").and_then(Value::as_str) != Some("validated") {
                    issues.push("Codex App Server ops controller did not validate".to_string());
                }
                controller_execution = execution;
            }
            Err(error) => {
                issues.push("Codex App Server ops controller failed".to_string());
                controller_execution = json!({
                    "attempted": true,
                    "status": "failed",
                    "error": error.message
                });
            }
        }
    } else if controller_required {
        issues.push("Codex App Server ops controller is required but not configured".to_string());
    }
    if controller_required
        && controller_execution.get("status").and_then(Value::as_str) != Some("validated")
    {
        issues.push(
            "Codex App Server ops controller evidence is missing or not validated".to_string(),
        );
    }
    dedupe_strings(&mut issues);
    let status = if issues.is_empty() {
        "validated"
    } else {
        "blocked"
    }
    .to_string();
    let run = CodexAppServerOpsValidationRun {
        status,
        configured: summary.configured,
        production_ops_status: summary.production_ops.status,
        controller_required,
        controller_configured,
        controller_execution,
        checked_at,
    };
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "codex_app_server.ops_validation",
            "codex_app_server",
            None,
            json!({
                "subject": principal.subject_id,
                "status": run.status,
                "configured": run.configured,
                "production_ops_status": run.production_ops_status,
                "controller_required": run.controller_required,
                "controller_configured": run.controller_configured,
                "controller_execution": run.controller_execution,
                "issues": issues,
                "checked_at": run.checked_at,
            }),
        ))
        .await?;
    Ok(Json(run))
}

async fn list_codex_app_server_runs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<CodexAppServerRun>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "codex_app_server",
        None,
    )
    .await?;
    Ok(Json(state.list_codex_app_server_runs().await?))
}

async fn get_codex_app_server_traces(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<CodexAppServerTraceSummary>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "codex_app_server",
        None,
    )
    .await?;
    let runs = state.list_codex_app_server_runs().await?;
    Ok(Json(build_codex_app_server_trace_summary(&runs)))
}

async fn get_codex_app_server_control_plane_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<CodexAppServerControlPlaneSummary>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "codex_app_server",
        None,
    )
    .await?;
    let runs = state.list_codex_app_server_runs().await?;
    let audit_logs = state.list_audit_logs(None).await?;
    Ok(Json(build_codex_app_server_control_plane_summary(
        state.codex_app_server_config.as_ref(),
        &runs,
        &audit_logs,
        Utc::now(),
    )))
}

async fn get_codex_app_server_trace_detail(
    State(state): State<AppState>,
    Path(trace_key): Path<String>,
    headers: HeaderMap,
) -> Result<Json<CodexAppServerTraceDetail>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "codex_app_server",
        None,
    )
    .await?;
    let runs = state.list_codex_app_server_runs().await?;
    let audit_logs = state.list_audit_logs(None).await?;
    let mut events = Vec::new();
    for session in state.list_sessions().await? {
        events.extend(state.list_events(session.id).await?);
    }
    build_codex_app_server_trace_detail(&runs, &trace_key, &events, &audit_logs).map(Json)
}

async fn poll_codex_app_server_run(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CodexAppServerPollRequest>,
) -> Result<Json<CodexAppServerPollResponse>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "codex_app_server".to_string(),
        resource_id: Some(run_id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    Ok(Json(
        poll_codex_app_server_run_inner(&state, run_id, input, "user", principal.subject_id)
            .await?,
    ))
}

async fn poll_stale_codex_app_server_runs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CodexAppServerStalePollRequest>,
) -> Result<Json<CodexAppServerStalePollRun>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "codex_app_server".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    Ok(Json(
        execute_stale_codex_app_server_polls(&state, input, "user", &principal.subject_id).await?,
    ))
}

async fn create_codex_thread(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CodexThreadRequest>,
) -> Result<Json<CodexThreadResponse>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "codex_app_server",
        None,
    )
    .await?;
    let config = codex_app_server_config(&state)?;
    let response = state
        .codex_app_server_client
        .create_thread(config, input.clone())
        .await?;
    state
        .record_codex_app_server_run(
            "thread.create",
            Some(response.thread_id.clone()),
            None,
            None,
            serde_json::to_value(&input)?,
            serde_json::to_value(&response)?,
        )
        .await?;
    Ok(Json(response))
}

async fn create_codex_turn(
    State(state): State<AppState>,
    Path(thread_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<CodexTurnRequest>,
) -> Result<Json<CodexTurnResponse>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "codex_app_server",
        None,
    )
    .await?;
    let config = codex_app_server_config(&state)?;
    let response = state
        .codex_app_server_client
        .create_turn(config, &thread_id, input.clone())
        .await?;
    state
        .record_codex_app_server_run(
            "turn.create",
            Some(thread_id),
            Some(response.turn_id.clone()),
            None,
            serde_json::to_value(&input)?,
            serde_json::to_value(&response)?,
        )
        .await?;
    Ok(Json(response))
}

async fn interrupt_codex_turn(
    State(state): State<AppState>,
    Path(turn_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<CodexInterruptResponse>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "codex_app_server",
        None,
    )
    .await?;
    let config = codex_app_server_config(&state)?;
    let response = state
        .codex_app_server_client
        .interrupt_turn(config, &turn_id)
        .await?;
    state
        .record_codex_app_server_run(
            "turn.interrupt",
            None,
            Some(turn_id),
            None,
            json!({}),
            serde_json::to_value(&response)?,
        )
        .await?;
    Ok(Json(response))
}

async fn execute_codex_command(
    State(state): State<AppState>,
    Path(turn_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<CodexCommandRequest>,
) -> Result<Json<CodexCommandResponse>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "codex_app_server",
        None,
    )
    .await?;
    let config = codex_app_server_config(&state)?;
    let response = state
        .codex_app_server_client
        .execute_command(config, &turn_id, input.clone())
        .await?;
    state
        .record_codex_app_server_run(
            "command.execute",
            None,
            Some(turn_id),
            Some(response.command_id.clone()),
            serde_json::to_value(&input)?,
            serde_json::to_value(&response)?,
        )
        .await?;
    Ok(Json(response))
}

async fn sync_codex_artifacts(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CodexArtifactSyncRequest>,
) -> Result<Json<CodexArtifactSyncResponse>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "session",
        Some(input.session_id),
    )
    .await?;
    if input.artifacts.is_empty() {
        return Err(AppError::bad_request("at least one artifact is required"));
    }
    if input.artifacts.len() > 50 {
        return Err(AppError::bad_request(
            "Codex artifact sync accepts at most 50 artifacts per request",
        ));
    }
    state.get_session(input.session_id).await?;

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
                    "source": "codex_app_server",
                    "turn_id": input.turn_id,
                    "command_id": input.command_id,
                    "metadata": artifact_input.metadata,
                }),
            )
            .await?;
        state
            .append_audit_log(new_audit_log(
                Some(input.session_id),
                "worker",
                Some(artifact.id),
                "codex_app_server.artifact_synced",
                "artifact",
                Some(artifact.id),
                json!({
                    "name": artifact.name,
                    "path": artifact.path,
                    "artifact_type": artifact.artifact_type,
                    "turn_id": input.turn_id,
                    "command_id": input.command_id,
                }),
            ))
            .await?;
        artifacts.push(artifact);
    }

    Ok(Json(CodexArtifactSyncResponse {
        session_id: input.session_id,
        turn_id: input.turn_id,
        command_id: input.command_id,
        artifact_count: artifacts.len(),
        artifacts,
    }))
}
