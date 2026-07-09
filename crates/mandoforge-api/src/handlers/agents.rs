use std::collections::BTreeMap;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::get,
};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    Agent, AgentRuntimeProfile, AgentRuntimeProfileReleaseGate, AgentVersion, AppError, AppState,
    AuthorizationRequest, CreateAgent, CreateAgentRuntimeProfile, CreateEnvironment, Environment,
    Permission, UpdateAgentRuntimeProfile, UpdateEnvironment, authorize_request,
    evaluate_agent_runtime_profile_release_gate, new_audit_log, principal_from_request,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/agents", get(list_agents).post(create_agent))
        .route("/api/agents/{id}/versions", get(list_agent_versions))
        .route(
            "/api/agents/{id}/versions/{version}",
            get(get_agent_version),
        )
        .route(
            "/api/agents/{id}/versions/{version}/capability-readback",
            get(get_agent_version_capability_readback),
        )
        .route(
            "/api/agent-runtime-profiles",
            get(list_agent_runtime_profiles).post(create_agent_runtime_profile),
        )
        .route(
            "/api/agent-runtime-profile-release-gates",
            get(list_agent_runtime_profile_release_gates),
        )
        .route(
            "/api/agent-runtime-profiles/{id}",
            get(get_agent_runtime_profile)
                .patch(update_agent_runtime_profile)
                .delete(archive_agent_runtime_profile),
        )
        .route(
            "/api/agent-runtime-profiles/{id}/release-gate",
            get(get_agent_runtime_profile_release_gate),
        )
        .route(
            "/api/agent-runtime-profiles/{id}/capability-readback",
            get(get_agent_runtime_profile_capability_readback),
        )
        .route(
            "/api/environments",
            get(list_environments).post(create_environment),
        )
        .route(
            "/api/environments/{id}",
            get(get_environment)
                .patch(update_environment)
                .delete(archive_environment),
        )
}

async fn list_agents(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Agent>>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::AgentsRead,
        resource_type: "agents".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    Ok(Json(state.list_agents_visible_to(&principal).await?))
}

async fn create_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateAgent>,
) -> Result<Json<Agent>, AppError> {
    authorize_request(&state, &headers, Permission::AgentsWrite, "agents", None).await?;
    Ok(Json(state.create_agent(input).await?))
}

async fn list_agent_versions(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<AgentVersion>>, AppError> {
    authorize_request(&state, &headers, Permission::AgentsRead, "agent", Some(id)).await?;
    Ok(Json(state.list_agent_versions(id).await?))
}

async fn get_agent_version(
    State(state): State<AppState>,
    Path((id, version)): Path<(Uuid, i32)>,
    headers: HeaderMap,
) -> Result<Json<AgentVersion>, AppError> {
    authorize_request(&state, &headers, Permission::AgentsRead, "agent", Some(id)).await?;
    Ok(Json(state.get_agent_version(id, version).await?))
}

async fn get_agent_version_capability_readback(
    State(state): State<AppState>,
    Path((id, version)): Path<(Uuid, i32)>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    authorize_request(&state, &headers, Permission::AgentsRead, "agent", Some(id)).await?;
    let agent = state.get_agent(id).await?;
    let agent_version = state.get_agent_version(id, version).await?;
    let eval_runs: Vec<_> = state
        .list_eval_runs(None)
        .await?
        .into_iter()
        .filter(|run| run.agent_id == id && run.agent_version_id == agent_version.id)
        .collect();
    let releases: Vec<_> = state
        .list_agent_releases(id)
        .await?
        .into_iter()
        .filter(|release| release.agent_version_id == agent_version.id)
        .collect();
    let best_eval_score = eval_runs
        .iter()
        .filter_map(|run| run.score)
        .max_by(f64::total_cmp);
    let latest_eval_run_id = eval_runs.first().map(|run| run.id);
    Ok(Json(json!({
        "product_object": "AgentVersion",
        "agent": {
            "id": agent.id,
            "name": agent.name,
            "kind": agent.kind,
            "release_state": agent.release_state,
            "runtime_profile_id": agent.runtime_profile_id
        },
        "version": {
            "id": agent_version.id,
            "agent_id": agent_version.agent_id,
            "version": agent_version.version,
            "model": agent_version.model,
            "created_at": agent_version.created_at
        },
        "runtime_contract": {
            "runtime_config": agent_version.runtime_config,
            "semantic_scopes": agent_version.semantic_scopes,
            "mcp_server_ids": agent_version.mcp_server_ids,
            "skill_ids": agent_version.skill_ids,
            "workflow_pack_ids": agent_version.workflow_pack_ids
        },
        "tool_contract": {
            "tools": agent_version.tools,
            "tool_names": agent_version.tool_names,
            "tool_count": agent_version.tool_names.len()
        },
        "policy_contract": {
            "approval_policy": agent_version.approval_policy
        },
        "eval_evidence": {
            "run_count": eval_runs.len(),
            "latest_run_id": latest_eval_run_id,
            "best_score": best_eval_score,
            "run_ids": eval_runs.iter().map(|run| run.id).collect::<Vec<_>>(),
            "runs_by_status": count_values(eval_runs.iter().map(|run| run.status.as_str()))
        },
        "release_evidence": {
            "release_count": releases.len(),
            "release_ids": releases.iter().map(|release| release.id).collect::<Vec<_>>(),
            "releases_by_status": count_values(releases.iter().map(|release| release.status.as_str()))
        },
        "evidence_sources": [
            "agent_version",
            "eval_runs",
            "agent_releases"
        ],
        "authority_boundary": "read-only AgentVersion capability readback; versions define runtime, tool, and policy contracts while release, approval, deployment validation, and rollback remain separate governance gates"
    })))
}

async fn list_agent_runtime_profiles(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AgentRuntimeProfile>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "agent_runtime_profiles",
        None,
    )
    .await?;
    Ok(Json(state.list_agent_runtime_profiles().await?))
}

async fn list_agent_runtime_profile_release_gates(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AgentRuntimeProfileReleaseGate>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "agent_runtime_profiles",
        None,
    )
    .await?;
    let profiles = state.list_agent_runtime_profiles().await?;
    Ok(Json(
        profiles
            .iter()
            .map(evaluate_agent_runtime_profile_release_gate)
            .collect(),
    ))
}

async fn create_agent_runtime_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateAgentRuntimeProfile>,
) -> Result<Json<AgentRuntimeProfile>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsWrite,
        "agent_runtime_profiles",
        None,
    )
    .await?;
    let profile = state.create_agent_runtime_profile(input).await?;
    let principal = principal_from_request(&state, &headers).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "agent_runtime_profile.created",
            "agent_runtime_profile",
            Some(profile.id),
            json!({
                "subject": principal.subject_id,
                "name": profile.name,
                "runtime_type": profile.runtime_type,
                "remote_computer_required": profile.remote_computer_required,
                "status": profile.status
            }),
        ))
        .await?;
    Ok(Json(profile))
}

async fn get_agent_runtime_profile(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<AgentRuntimeProfile>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "agent_runtime_profile",
        Some(id),
    )
    .await?;
    Ok(Json(state.get_agent_runtime_profile(id).await?))
}

async fn get_agent_runtime_profile_release_gate(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<AgentRuntimeProfileReleaseGate>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "agent_runtime_profile",
        Some(id),
    )
    .await?;
    let profile = state.get_agent_runtime_profile(id).await?;
    Ok(Json(evaluate_agent_runtime_profile_release_gate(&profile)))
}

async fn get_agent_runtime_profile_capability_readback(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "agent_runtime_profile",
        Some(id),
    )
    .await?;
    let profile = state.get_agent_runtime_profile(id).await?;
    let release_gate = evaluate_agent_runtime_profile_release_gate(&profile);
    let bound_environments: Vec<_> = state
        .list_environments()
        .await?
        .into_iter()
        .filter(|environment| environment.runtime_profile_id == Some(profile.id))
        .collect();
    let bound_agents: Vec<_> = state
        .list_agents()
        .await?
        .into_iter()
        .filter(|agent| agent.runtime_profile_id == Some(profile.id))
        .collect();
    Ok(Json(json!({
        "product_object": "EnvironmentProfile",
        "runtime_profile": {
            "id": profile.id,
            "name": profile.name,
            "runtime_type": profile.runtime_type,
            "command": profile.command,
            "default_args": profile.default_args,
            "timeout_seconds": profile.timeout_seconds,
            "remote_computer_required": profile.remote_computer_required,
            "status": profile.status,
            "created_at": profile.created_at,
            "updated_at": profile.updated_at,
            "archived_at": profile.archived_at
        },
        "release_gate": release_gate,
        "environment_bindings": bound_environments
            .iter()
            .map(|environment| json!({
                "id": environment.id,
                "name": environment.name,
                "environment_type": environment.environment_type,
                "release_state": environment.release_state,
                "status": environment.status
            }))
            .collect::<Vec<_>>(),
        "agent_bindings": bound_agents
            .iter()
            .map(|agent| json!({
                "id": agent.id,
                "name": agent.name,
                "kind": agent.kind,
                "agent_role": agent.agent_role,
                "release_state": agent.release_state
            }))
            .collect::<Vec<_>>(),
        "summary": {
            "environment_binding_count": bound_environments.len(),
            "agent_binding_count": bound_agents.len(),
            "environments_by_status": count_values(bound_environments.iter().map(|environment| environment.status.as_str())),
            "agents_by_release_state": count_values(bound_agents.iter().map(|agent| agent.release_state.as_str()))
        },
        "evidence_sources": [
            "agent_runtime_profile",
            "agent_runtime_profile_release_gate",
            "environments",
            "agents"
        ],
        "authority_boundary": "read-only EnvironmentProfile capability readback; runtime profiles select managed runtime bindings while execution remains gated by Managed Runtime, Tool Router, Policy, Approval, and Audit"
    })))
}

async fn update_agent_runtime_profile(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateAgentRuntimeProfile>,
) -> Result<Json<AgentRuntimeProfile>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsWrite,
        "agent_runtime_profile",
        Some(id),
    )
    .await?;
    let before = state.get_agent_runtime_profile(id).await?;
    let profile = state.update_agent_runtime_profile(id, input).await?;
    let principal = principal_from_request(&state, &headers).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "agent_runtime_profile.updated",
            "agent_runtime_profile",
            Some(profile.id),
            json!({
                "subject": principal.subject_id,
                "name": profile.name,
                "runtime_type": profile.runtime_type,
                "before": {
                    "status": before.status,
                    "command": before.command,
                    "default_args": before.default_args,
                    "env": before.env,
                    "timeout_seconds": before.timeout_seconds,
                    "remote_computer_required": before.remote_computer_required
                },
                "after": {
                    "status": profile.status,
                    "command": profile.command,
                    "default_args": profile.default_args,
                    "env": profile.env,
                    "timeout_seconds": profile.timeout_seconds,
                    "remote_computer_required": profile.remote_computer_required
                }
            }),
        ))
        .await?;
    Ok(Json(profile))
}

async fn archive_agent_runtime_profile(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<AgentRuntimeProfile>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsWrite,
        "agent_runtime_profile",
        Some(id),
    )
    .await?;
    let profile = state.archive_agent_runtime_profile(id).await?;
    let principal = principal_from_request(&state, &headers).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "agent_runtime_profile.archived",
            "agent_runtime_profile",
            Some(profile.id),
            json!({
                "subject": principal.subject_id,
                "name": profile.name,
                "runtime_type": profile.runtime_type,
                "status": profile.status,
                "archived_at": profile.archived_at
            }),
        ))
        .await?;
    Ok(Json(profile))
}

async fn list_environments(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Environment>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "environments",
        None,
    )
    .await?;
    Ok(Json(state.list_environments().await?))
}

async fn create_environment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateEnvironment>,
) -> Result<Json<Environment>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsWrite,
        "environments",
        None,
    )
    .await?;
    let environment = state.create_environment(input).await?;
    let principal = principal_from_request(&state, &headers).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "environment.created",
            "environment",
            Some(environment.id),
            json!({
                "subject": principal.subject_id,
                "name": environment.name,
                "environment_type": environment.environment_type,
                "runtime_profile_id": environment.runtime_profile_id,
                "release_state": environment.release_state,
                "status": environment.status
            }),
        ))
        .await?;
    Ok(Json(environment))
}

async fn get_environment(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Environment>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "environment",
        Some(id),
    )
    .await?;
    Ok(Json(state.get_environment(id).await?))
}

async fn update_environment(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateEnvironment>,
) -> Result<Json<Environment>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsWrite,
        "environment",
        Some(id),
    )
    .await?;
    let before = state.get_environment(id).await?;
    let environment = state.update_environment(id, input).await?;
    let principal = principal_from_request(&state, &headers).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "environment.updated",
            "environment",
            Some(environment.id),
            json!({
                "subject": principal.subject_id,
                "name": environment.name,
                "environment_type": environment.environment_type,
                "before": {
                    "name": before.name,
                    "environment_type": before.environment_type,
                    "runtime_profile_id": before.runtime_profile_id,
                    "release_state": before.release_state,
                    "status": before.status
                },
                "after": {
                    "name": environment.name,
                    "environment_type": environment.environment_type,
                    "runtime_profile_id": environment.runtime_profile_id,
                    "release_state": environment.release_state,
                    "status": environment.status
                }
            }),
        ))
        .await?;
    Ok(Json(environment))
}

async fn archive_environment(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Environment>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsWrite,
        "environment",
        Some(id),
    )
    .await?;
    let environment = state.archive_environment(id).await?;
    let principal = principal_from_request(&state, &headers).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "environment.archived",
            "environment",
            Some(environment.id),
            json!({
                "subject": principal.subject_id,
                "name": environment.name,
                "environment_type": environment.environment_type,
                "release_state": environment.release_state,
                "status": environment.status,
                "archived_at": environment.archived_at
            }),
        ))
        .await?;
    Ok(Json(environment))
}

fn count_values<'a>(values: impl Iterator<Item = &'a str>) -> BTreeMap<String, usize> {
    values.fold(BTreeMap::new(), |mut counts, value| {
        *counts.entry(value.to_string()).or_default() += 1;
        counts
    })
}
