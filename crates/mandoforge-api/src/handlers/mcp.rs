use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, patch, post},
};
use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    AppError, AppState, AuthorizationRequest, CreateMcpServerRecord,
    McpServerDeploymentValidationRun, McpServerHealth, McpServerHealthRun, McpServerRecord,
    McpServerRolloutDueRun, McpServerRolloutResponse, McpServerRolloutRunSummary,
    McpServerRolloutSummary, McpServerScheduledHealthRun, Permission, RequestMcpServerRollout,
    UpdateMcpServerRecord, UpdateMcpServerStatus, apply_mcp_server_rollout_inner,
    authorize_request, build_mcp_server_rollout, build_mcp_server_rollout_run_summary,
    build_mcp_server_rollout_summary, enforce_resource_scope, execute_due_mcp_server_health_checks,
    execute_due_mcp_server_rollouts, execute_mcp_server_deployment_controller,
    execute_mcp_server_rollout_rollback_controller, mcp_config_without_rollout_metadata,
    mcp_pending_rollout, mcp_server_deployment_controller_configured,
    mcp_server_deployment_controller_required, mcp_server_health,
    mcp_server_rollout_rollback_controller_configured,
    mcp_server_rollout_rollback_controller_required, new_audit_log, normalize_mcp_config,
    normalize_mcp_name, normalize_mcp_status, normalize_mcp_tool_allowlist,
    normalize_mcp_transport, principal_from_request,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/teams/{id}/mcp-servers",
            get(list_mcp_servers).post(create_mcp_server),
        )
        .route(
            "/api/teams/{team_id}/mcp-servers/{server_id}",
            patch(update_mcp_server),
        )
        .route(
            "/api/teams/{team_id}/mcp-servers/{server_id}/status",
            patch(update_mcp_server_status),
        )
        .route(
            "/api/teams/{team_id}/mcp-servers/{server_id}/health",
            get(get_mcp_server_health),
        )
        .route(
            "/api/teams/{team_id}/mcp-servers/health/run",
            post(run_mcp_server_health_checks),
        )
        .route(
            "/api/teams/{team_id}/mcp-servers/health/run-due",
            post(run_due_mcp_server_health_checks),
        )
        .route(
            "/api/teams/{team_id}/mcp-servers/deployment/validate",
            post(validate_mcp_server_deployment),
        )
        .route(
            "/api/teams/{team_id}/mcp-servers/rollouts/run-due",
            post(run_due_mcp_server_rollouts),
        )
        .route(
            "/api/teams/{team_id}/mcp-servers/rollouts/summary",
            get(get_mcp_server_rollout_summary),
        )
        .route(
            "/api/teams/{team_id}/mcp-servers/rollouts/runs",
            get(get_mcp_server_rollout_runs),
        )
        .route(
            "/api/teams/{team_id}/mcp-servers/{server_id}/rollouts",
            post(request_mcp_server_rollout),
        )
        .route(
            "/api/teams/{team_id}/mcp-servers/{server_id}/rollouts/{rollout_id}/apply",
            post(apply_mcp_server_rollout),
        )
        .route(
            "/api/teams/{team_id}/mcp-servers/{server_id}/rollouts/{rollout_id}/rollback",
            post(rollback_mcp_server_rollout),
        )
        .route(
            "/api/teams/{team_id}/mcp-servers/{server_id}/discover",
            post(discover_mcp_server_tools),
        )
}

async fn list_mcp_servers(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<McpServerRecord>>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "team", Some(id)).await?;
    Ok(Json(state.list_mcp_servers(id).await?))
}

async fn create_mcp_server(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(mut input): Json<CreateMcpServerRecord>,
) -> Result<Json<McpServerRecord>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "team", Some(id)).await?;
    input.name = normalize_mcp_name(&input.name)?;
    input.transport = normalize_mcp_transport(&input.transport)?;
    input.config = normalize_mcp_config(input.config)?;
    input.tool_allowlist = normalize_mcp_tool_allowlist(input.tool_allowlist)?;
    let server = state.create_mcp_server(id, input).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "system",
            None,
            "mcp.server_saved",
            "mcp_server",
            Some(server.id),
            json!({
                "team_id": id,
                "name": server.name,
                "transport": server.transport,
                "status": server.status,
                "tool_allowlist": server.tool_allowlist,
            }),
        ))
        .await?;
    Ok(Json(server))
}

async fn update_mcp_server(
    State(state): State<AppState>,
    Path((team_id, server_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(mut input): Json<UpdateMcpServerRecord>,
) -> Result<Json<McpServerRecord>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "team", Some(team_id)).await?;
    if let Some(transport) = input.transport.as_deref() {
        input.transport = Some(normalize_mcp_transport(transport)?);
    }
    if let Some(config) = input.config.take() {
        input.config = Some(normalize_mcp_config(config)?);
    }
    if let Some(tool_allowlist) = input.tool_allowlist.take() {
        input.tool_allowlist = Some(normalize_mcp_tool_allowlist(tool_allowlist)?);
    }
    let server = state.update_mcp_server(team_id, server_id, input).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "system",
            None,
            "mcp.server_updated",
            "mcp_server",
            Some(server.id),
            json!({
                "team_id": team_id,
                "name": server.name,
                "transport": server.transport,
                "status": server.status,
                "tool_allowlist": server.tool_allowlist,
            }),
        ))
        .await?;
    Ok(Json(server))
}

async fn update_mcp_server_status(
    State(state): State<AppState>,
    Path((team_id, server_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(input): Json<UpdateMcpServerStatus>,
) -> Result<Json<McpServerRecord>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "team", Some(team_id)).await?;
    let status = normalize_mcp_status(&input.status)?;
    let server = state
        .update_mcp_server_status(team_id, server_id, &status)
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "system",
            None,
            "mcp.server_status_updated",
            "mcp_server",
            Some(server.id),
            json!({
                "team_id": team_id,
                "name": server.name,
                "status": server.status,
            }),
        ))
        .await?;
    Ok(Json(server))
}

async fn get_mcp_server_health(
    State(state): State<AppState>,
    Path((team_id, server_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<McpServerHealth>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "team", Some(team_id)).await?;
    let server = state.get_mcp_server(team_id, server_id).await?;
    let health = mcp_server_health(&state, &server).await;
    state
        .append_audit_log(new_audit_log(
            None,
            "system",
            None,
            "mcp.server_health_checked",
            "mcp_server",
            Some(server.id),
            json!({
                "team_id": team_id,
                "name": server.name,
                "healthy": health.healthy,
                "issues": health.issues,
            }),
        ))
        .await?;
    Ok(Json(health))
}

async fn run_mcp_server_health_checks(
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<McpServerHealthRun>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "team", Some(team_id)).await?;
    let checked_at = Utc::now();
    let servers = state.list_mcp_servers(team_id).await?;
    let mut results = Vec::with_capacity(servers.len());
    for server in servers {
        results.push(mcp_server_health(&state, &server).await);
    }
    let healthy_count = results.iter().filter(|health| health.healthy).count();
    let run = McpServerHealthRun {
        team_id,
        server_count: results.len(),
        healthy_count,
        unhealthy_count: results.len().saturating_sub(healthy_count),
        results,
        checked_at,
    };
    state
        .append_audit_log(new_audit_log(
            None,
            "system",
            None,
            "mcp.server_health_run",
            "team",
            Some(team_id),
            json!({
                "team_id": team_id,
                "server_count": run.server_count,
                "healthy_count": run.healthy_count,
                "unhealthy_count": run.unhealthy_count,
            }),
        ))
        .await?;
    Ok(Json(run))
}

async fn run_due_mcp_server_health_checks(
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<McpServerScheduledHealthRun>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "team", Some(team_id)).await?;
    Ok(Json(
        execute_due_mcp_server_health_checks(&state, team_id).await?,
    ))
}

async fn validate_mcp_server_deployment(
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<McpServerDeploymentValidationRun>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "team".to_string(),
        resource_id: Some(team_id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;

    let checked_at = Utc::now();
    let servers = state.list_mcp_servers(team_id).await?;
    let mut results = Vec::with_capacity(servers.len());
    for server in servers {
        results.push(mcp_server_health(&state, &server).await);
    }
    let healthy_count = results.iter().filter(|health| health.healthy).count();
    let unhealthy_count = results.len().saturating_sub(healthy_count);
    let lookup = |key: &str| std::env::var(key).ok();
    let controller_required = mcp_server_deployment_controller_required(&lookup);
    let controller_configured = mcp_server_deployment_controller_configured(&lookup);
    let mut healthy = !results.is_empty() && unhealthy_count == 0;
    let mut issues = Vec::new();
    if results.is_empty() {
        issues.push("MCP connector deployment validation covered no connectors");
    }
    if unhealthy_count > 0 {
        issues.push("MCP connector deployment validation found unhealthy connectors");
    }
    let mut controller_execution = json!({
        "attempted": false,
        "status": "skipped",
        "reason": if controller_configured {
            "mcp_health_not_ready"
        } else {
            "controller_not_configured"
        }
    });
    if healthy && controller_configured {
        match execute_mcp_server_deployment_controller(
            &lookup,
            team_id,
            &principal.subject_id,
            checked_at,
            &results,
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
                    issues.push("MCP connector deployment controller did not validate");
                }
            }
            Err(error) => {
                healthy = false;
                issues.push("MCP connector deployment controller failed");
                controller_execution = json!({
                    "attempted": true,
                    "status": "failed",
                    "error": error.message
                });
            }
        }
    }
    if healthy && controller_required && !controller_configured {
        healthy = false;
        issues.push("MCP connector deployment controller is required but not configured");
    }
    let status = if healthy { "healthy" } else { "blocked" }.to_string();
    let run = McpServerDeploymentValidationRun {
        team_id,
        server_count: results.len(),
        healthy_count,
        unhealthy_count,
        results,
        controller_required,
        controller_configured,
        controller_execution,
        checked_at,
        status,
    };
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "mcp.server_deployment_validation_run",
            "team",
            Some(team_id),
            json!({
                "team_id": team_id,
                "subject": principal.subject_id,
                "server_count": run.server_count,
                "healthy_count": run.healthy_count,
                "unhealthy_count": run.unhealthy_count,
                "status": run.status,
                "controller_required": run.controller_required,
                "controller_configured": run.controller_configured,
                "controller_execution": run.controller_execution,
                "issues": issues,
            }),
        ))
        .await?;
    Ok(Json(run))
}

async fn run_due_mcp_server_rollouts(
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<McpServerRolloutDueRun>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "team", Some(team_id)).await?;
    Ok(Json(
        execute_due_mcp_server_rollouts(&state, team_id).await?,
    ))
}

async fn get_mcp_server_rollout_summary(
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<McpServerRolloutSummary>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "team", Some(team_id)).await?;
    Ok(Json(build_mcp_server_rollout_summary(
        team_id,
        state.list_mcp_servers(team_id).await?,
        Utc::now(),
    )))
}

async fn get_mcp_server_rollout_runs(
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<McpServerRolloutRunSummary>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "team", Some(team_id)).await?;
    let audit_logs = state.list_audit_logs(None).await?;
    let rollout_summary = build_mcp_server_rollout_summary(
        team_id,
        state.list_mcp_servers(team_id).await?,
        Utc::now(),
    );
    Ok(Json(build_mcp_server_rollout_run_summary(
        team_id,
        &audit_logs,
        Utc::now(),
        &rollout_summary,
    )))
}

async fn request_mcp_server_rollout(
    State(state): State<AppState>,
    Path((team_id, server_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(mut input): Json<RequestMcpServerRollout>,
) -> Result<Json<McpServerRolloutResponse>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "team".to_string(),
        resource_id: Some(team_id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let server = state.get_mcp_server(team_id, server_id).await?;
    if mcp_pending_rollout(&server).is_some() {
        return Err(AppError::bad_request(
            "MCP server already has a pending rollout",
        ));
    }
    let rollout =
        build_mcp_server_rollout(&state, &server, &principal.subject_id, &mut input).await?;
    let mut config = server.config.as_object().cloned().unwrap_or_default();
    config.insert("pending_rollout".to_string(), rollout.rollout.clone());
    let updated = state
        .update_mcp_server(
            team_id,
            server_id,
            UpdateMcpServerRecord {
                transport: None,
                config: Some(Value::Object(config)),
                tool_allowlist: None,
            },
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "mcp.server_rollout_requested",
            "mcp_server",
            Some(server_id),
            json!({
                "subject": principal.subject_id,
                "team_id": team_id,
                "name": server.name,
                "rollout": rollout.rollout,
            }),
        ))
        .await?;
    Ok(Json(McpServerRolloutResponse {
        server: updated,
        rollout: rollout.rollout,
        preflight_health: Some(rollout.preflight_health),
    }))
}

async fn apply_mcp_server_rollout(
    State(state): State<AppState>,
    Path((team_id, server_id, rollout_id)): Path<(Uuid, Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<McpServerRolloutResponse>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "team".to_string(),
        resource_id: Some(team_id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    Ok(Json(
        apply_mcp_server_rollout_inner(
            &state,
            team_id,
            server_id,
            rollout_id,
            &principal.subject_id,
        )
        .await?,
    ))
}

async fn rollback_mcp_server_rollout(
    State(state): State<AppState>,
    Path((team_id, server_id, rollout_id)): Path<(Uuid, Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<McpServerRolloutResponse>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "team".to_string(),
        resource_id: Some(team_id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let server = state.get_mcp_server(team_id, server_id).await?;
    let last_rollout =
        server.config.get("last_rollout").cloned().ok_or_else(|| {
            AppError::bad_request("MCP server has no applied rollout to rollback")
        })?;
    if last_rollout.get("id").and_then(Value::as_str) != Some(&rollout_id.to_string()) {
        return Err(AppError::bad_request(
            "MCP server last rollout does not match requested rollout id",
        ));
    }
    if last_rollout.get("status").and_then(Value::as_str) != Some("applied") {
        return Err(AppError::bad_request(
            "MCP server last rollout is not rollbackable",
        ));
    }
    let lookup = |key: &str| std::env::var(key).ok();
    let controller_required = mcp_server_rollout_rollback_controller_required(&lookup);
    let controller_configured = mcp_server_rollout_rollback_controller_configured(&lookup);
    let mut controller_execution = json!({
        "attempted": false,
        "status": "skipped",
        "reason": "controller_not_configured"
    });
    if controller_required && !controller_configured {
        return Err(AppError::bad_request(
            "MCP rollout rollback controller is required but not configured",
        ));
    }
    if controller_configured {
        controller_execution = execute_mcp_server_rollout_rollback_controller(
            &lookup,
            &principal.subject_id,
            Utc::now(),
            &server,
            &last_rollout,
        )
        .await?;
        if controller_execution.get("status").and_then(Value::as_str) != Some("rolled_back") {
            return Err(AppError::bad_request(
                "MCP rollout rollback controller did not confirm rollback",
            ));
        }
    }
    let snapshot = last_rollout
        .get("previous_snapshot")
        .cloned()
        .ok_or_else(|| AppError::bad_request("MCP server rollout missing previous snapshot"))?;
    let mut config = snapshot
        .get("config")
        .cloned()
        .ok_or_else(|| AppError::bad_request("MCP server rollout snapshot missing config"))?;
    config = mcp_config_without_rollout_metadata(&config);
    let mut last_rollout = last_rollout;
    last_rollout["status"] = json!("rolled_back");
    last_rollout["rolled_back_by"] = json!(principal.subject_id.clone());
    last_rollout["rolled_back_at"] = json!(Utc::now());
    last_rollout["rollback_controller_required"] = json!(controller_required);
    last_rollout["rollback_controller_configured"] = json!(controller_configured);
    last_rollout["rollback_controller_execution"] = controller_execution.clone();
    let mut config_map = config.as_object().cloned().unwrap_or_default();
    config_map.insert("last_rollout".to_string(), last_rollout.clone());
    let target_status = snapshot
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::bad_request("MCP server rollout snapshot missing status"))?;
    state
        .update_mcp_server(
            team_id,
            server_id,
            UpdateMcpServerRecord {
                transport: Some(
                    snapshot
                        .get("transport")
                        .and_then(Value::as_str)
                        .unwrap_or(server.transport.as_str())
                        .to_string(),
                ),
                config: Some(Value::Object(config_map)),
                tool_allowlist: Some(
                    snapshot
                        .get("tool_allowlist")
                        .cloned()
                        .and_then(|value| serde_json::from_value(value).ok())
                        .unwrap_or_else(|| server.tool_allowlist.clone()),
                ),
            },
        )
        .await?;
    let updated = state
        .update_mcp_server_status(team_id, server_id, target_status)
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "mcp.server_rollout_rolled_back",
            "mcp_server",
            Some(server_id),
            json!({
                "subject": principal.subject_id,
                "team_id": team_id,
                "name": updated.name,
                "rollout": last_rollout,
                "controller_required": controller_required,
                "controller_configured": controller_configured,
                "controller_execution": controller_execution,
            }),
        ))
        .await?;
    let health = mcp_server_health(&state, &updated).await;
    Ok(Json(McpServerRolloutResponse {
        server: updated,
        rollout: last_rollout,
        preflight_health: Some(health),
    }))
}

async fn discover_mcp_server_tools(
    State(state): State<AppState>,
    Path((team_id, server_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<McpServerRecord>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "team", Some(team_id)).await?;
    let config = state
        .mcp_gateway_config
        .as_ref()
        .ok_or_else(|| AppError::bad_request("MCP gateway is not configured"))?;
    let server = state.get_mcp_server(team_id, server_id).await?;
    let tools = state
        .mcp_gateway_client
        .discover_tools(config, &server.name)
        .await?;
    let mut tool_allowlist: Vec<_> = tools
        .into_iter()
        .map(|tool| tool.name)
        .filter(|name| !name.trim().is_empty())
        .collect();
    tool_allowlist.sort();
    tool_allowlist.dedup();
    Ok(Json(
        state
            .update_mcp_server_tool_allowlist(team_id, server_id, tool_allowlist)
            .await?,
    ))
}
