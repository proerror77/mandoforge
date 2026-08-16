use std::collections::{BTreeSet, HashMap, HashSet};

use axum::http::HeaderMap;
use chrono::Utc;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::*;

pub(crate) fn merge_semantic_scopes(base: &Value, override_scopes: &Value) -> Value {
    let mut merged = base.as_object().cloned().unwrap_or_default();
    if let Some(object) = override_scopes.as_object() {
        for (key, value) in object {
            merged.insert(key.clone(), value.clone());
        }
    }
    Value::Object(merged)
}

pub(crate) async fn build_context_packet(
    state: &AppState,
    session_id: Uuid,
) -> Result<ContextPacket, AppError> {
    let session = state.get_session(session_id).await?;
    let mut context_workflow_run = None;
    let context_task_grant =
        if let Some((run, grant)) = active_task_grant_for_session(state, session_id).await? {
            if grant.status != "active" {
                record_task_grant_denied(
                    state,
                    session_id,
                    Some(&grant),
                    Some(run.id),
                    "context_packet",
                    "task grant is not active",
                )
                .await?;
                return Err(AppError::forbidden("task grant is not active"));
            }
            context_workflow_run = Some(run);
            Some(grant)
        } else {
            None
        };
    let agent = state.get_agent(session.agent_id).await?;
    let agent_version = state.agent_version_for_session(session_id).await?;
    let mut context_agent_version = agent_version.clone();
    if let Some(grant) = context_task_grant.as_ref() {
        let effective_tools = task_grant_effective_tools(&agent_version, grant);
        context_agent_version.tools = effective_tools.clone();
        context_agent_version.tool_names = effective_tools;
    }
    let handoff_assignment_context =
        effective_handoff_assignment_context(state, session_id).await?;
    let base_semantic_scopes = handoff_assignment_context
        .as_ref()
        .map(|assignment| assignment.semantic_scopes.clone())
        .unwrap_or_else(|| agent_version.semantic_scopes.clone());
    let effective_semantic_scopes = context_task_grant
        .as_ref()
        .map(|grant| merge_semantic_scopes(&base_semantic_scopes, &grant.semantic_scopes))
        .unwrap_or(base_semantic_scopes);
    let ontology_release = if let Some(snapshot) = context_workflow_run
        .as_ref()
        .and_then(|run| run.runtime_envelope.get("ontology_release"))
    {
        Some(snapshot.clone())
    } else {
        active_ontology_release_metadata_for_scopes(state, &effective_semantic_scopes).await?
    };
    let events = state.list_events(session_id).await?;
    let policy = state.policy_for_session(session_id).await;
    let (effective_runtime_profile_id, runtime_profile_source, runtime_profile_lookup) =
        resolve_context_packet_runtime_profile(
            state,
            &session,
            &agent_version,
            handoff_assignment_context.as_ref(),
        )
        .await?;
    let runtime_profile = runtime_profile_lookup
        .as_ref()
        .and_then(|(_, result)| result.as_ref().ok())
        .map(context_packet_runtime_profile);
    let runtime_profile_error = runtime_profile_lookup
        .as_ref()
        .and_then(|(profile_id, result)| result.as_ref().err().map(|error| (*profile_id, error)));
    let version = state.next_context_packet_version(session_id).await?;
    let mut retrieved_objects =
        retrieve_context_packet_semantic_objects(state, &effective_semantic_scopes).await?;
    if let Some(grant) = context_task_grant.as_ref() {
        retrieved_objects = apply_task_grant_memory_scope_to_context_objects(
            retrieved_objects,
            &grant.memory_scope,
        );
        record_task_grant_checked(state, grant, session_id, "context_packet").await?;
    }
    let mut source_refs = build_context_packet_source_refs(
        &session,
        &agent,
        &agent_version,
        runtime_profile.as_ref(),
        runtime_profile_error.map(|(profile_id, _)| profile_id),
        runtime_profile_source,
        events.len(),
        &retrieved_objects,
        handoff_assignment_context.as_ref(),
        context_task_grant.as_ref(),
    );
    if let Some(run) = context_workflow_run.as_ref() {
        source_refs.push(context_source_ref(
            "workflow_run",
            run.id,
            "pinned_snapshot",
        ));
    }
    if let Some(step_id) = context_task_grant
        .as_ref()
        .and_then(|grant| grant.workflow_step_run_id)
    {
        source_refs.push(context_source_ref(
            "workflow_step_run",
            step_id,
            "task_grant_bound",
        ));
    }
    if let Some(release_id) = ontology_release
        .as_ref()
        .and_then(|release| release.get("id"))
        .and_then(Value::as_str)
    {
        source_refs.push(ContextPacketSourceRef {
            source_type: "ontology_release".to_string(),
            source_id: release_id.to_string(),
            freshness: "pinned_snapshot".to_string(),
        });
    }
    let last_user_message = events
        .iter()
        .rev()
        .find(|event| event.event_type == "user.message")
        .and_then(|event| event.payload.get("message"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let freshness_warnings = build_context_freshness_warnings(
        &agent,
        &context_agent_version,
        &effective_semantic_scopes,
        effective_runtime_profile_id,
        runtime_profile_lookup.as_ref(),
        last_user_message.as_deref(),
    );

    let generated_at = Utc::now();
    let context_layers = context_packet_layer_projection(
        &session,
        &context_agent_version,
        runtime_profile.as_ref(),
        context_workflow_run.as_ref(),
        context_task_grant.as_ref(),
        ontology_release.as_ref(),
    );
    let task = json!({
        "title": session.title,
        "status": session.status.as_str(),
        "objective": context_task_grant.as_ref().map(|grant| grant.objective.as_str()),
        "workflow_run_id": context_workflow_run.as_ref().map(|run| run.id),
        "workflow_step_run_id": context_task_grant.as_ref().and_then(|grant| grant.workflow_step_run_id),
        "last_user_message": last_user_message,
        "event_count": events.len(),
        "created_at": session.created_at,
        "updated_at": session.updated_at,
    });
    let policy_reminders = build_context_policy_reminders(&policy, &context_agent_version);
    let tool_policy = context_task_grant
        .as_ref()
        .map(|grant| {
            json!({
                "agent_version_policy": agent_version.approval_policy.clone(),
                "task_grant_approval_policy": grant.approval_policy.clone(),
                "task_grant_tool_scope": grant.tool_scope.clone(),
                "task_grant_connector_scope": grant.connector_scope.clone(),
                "task_grant_external_effects": grant.external_effects.clone()
            })
        })
        .unwrap_or_else(|| agent_version.approval_policy.clone());
    let mut replay_summary = json!({
        "version": version,
        "source_ref_count": source_refs.len(),
        "retrieved_object_count": retrieved_objects.len(),
        "policy_reminder_count": policy_reminders.len(),
        "freshness_warning_count": freshness_warnings.len(),
        "semantic_scope_keys": semantic_scope_keys(&effective_semantic_scopes),
        "effective_context_source": if context_task_grant.is_some() {
            "task_grant"
        } else if handoff_assignment_context.is_some() {
            "agent_handoff_assignment"
        } else {
            "agent_version"
        },
        "runtime_profile_source": runtime_profile_source,
        "task_grant_authority": context_task_grant.as_ref().map(task_grant_context_authority),
        "context_layers": context_layers,
    });
    if let Some(ontology_release) = ontology_release {
        replay_summary["ontology_release"] = ontology_release;
    }
    Ok(ContextPacket {
        id: Uuid::new_v4(),
        session_id,
        agent_id: agent.id,
        agent_version_id: session.agent_version_id.or(Some(agent_version.id)),
        version,
        generated_at,
        task,
        agent: ContextPacketAgent {
            id: agent.id,
            name: agent.name.clone(),
            kind: agent.kind.clone(),
            agent_role: agent.agent_role.clone(),
            release_state: agent.release_state.clone(),
            tools: context_agent_version.tools.clone(),
            mcp_server_ids: agent_version.mcp_server_ids.clone(),
            skill_ids: agent_version.skill_ids.clone(),
            workflow_pack_ids: agent_version.workflow_pack_ids.clone(),
            remote_computer_profile: agent_version.remote_computer_profile.clone(),
        },
        runtime_profile,
        semantic_scopes: effective_semantic_scopes,
        tool_policy,
        policy_reminders,
        freshness_warnings,
        source_refs,
        retrieved_objects,
        replay_summary,
        audit_trace_id: None,
        created_at: generated_at,
    })
}

pub(crate) fn task_grant_effective_tools(
    agent_version: &AgentVersion,
    grant: &TaskGrant,
) -> Vec<String> {
    agent_version
        .tools
        .iter()
        .chain(agent_version.tool_names.iter())
        .filter(|tool| !tool.trim().is_empty() && task_grant_allows_tool(grant, tool))
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) fn task_grant_context_authority(grant: &TaskGrant) -> Value {
    json!({
        "task_grant_id": grant.id,
        "workflow_run_id": grant.workflow_run_id,
        "workflow_step_run_id": grant.workflow_step_run_id,
        "parent_grant_id": grant.parent_grant_id,
        "status": grant.status,
        "risk_level": grant.risk_level,
        "tool_scope": grant.tool_scope.clone(),
        "connector_scope": grant.connector_scope.clone(),
        "approval_policy": grant.approval_policy.clone(),
        "external_effects": grant.external_effects.clone(),
        "budgets": {
            "max_turns": grant.max_turns,
            "max_tool_calls": grant.max_tool_calls,
            "max_runtime_seconds": grant.max_runtime_seconds,
            "max_cost_usd_micros": grant.max_cost_usd_micros,
            "turns_used": grant.turns_used,
            "tool_calls_used": grant.tool_calls_used,
            "cost_usd_micros_used": grant.cost_usd_micros_used
        }
    })
}

pub(crate) fn context_packet_layer_projection(
    session: &Session,
    agent_version: &AgentVersion,
    runtime_profile: Option<&ContextPacketRuntimeProfile>,
    workflow_run: Option<&WorkflowRun>,
    task_grant: Option<&TaskGrant>,
    ontology_release: Option<&Value>,
) -> Value {
    json!({
        "work_surfaces": {
            "source_event_id": workflow_run.and_then(|run| run.source_event_id),
            "source_schedule_id": workflow_run.and_then(|run| run.source_schedule_id),
        },
        "collaboration": {
            "source_work_item_id": workflow_run.and_then(|run| run.source_work_item_id),
        },
        "manager_agent": {
            "source_handoff_id": task_grant.and_then(|grant| grant.source_handoff_id),
            "issuer_subject": task_grant.map(|grant| grant.issuer_subject.as_str()),
            "grantee_agent_id": task_grant.and_then(|grant| grant.grantee_agent_id),
        },
        "managed_runtime": {
            "session_id": session.id,
            "agent_id": session.agent_id,
            "agent_version_id": session.agent_version_id,
            "workflow_run_id": workflow_run.map(|run| run.id),
            "workflow_step_run_id": task_grant.and_then(|grant| grant.workflow_step_run_id),
        },
        "governance": {
            "task_grant_id": task_grant.map(|grant| grant.id),
            "parent_grant_id": task_grant.and_then(|grant| grant.parent_grant_id),
            "policy_revision_id": task_grant.and_then(|grant| grant.policy_revision_id),
            "risk_level": task_grant.map(|grant| grant.risk_level.as_str()),
        },
        "ontology_action_contract": {
            "ontology_release": ontology_release.cloned(),
        },
        "environment_scheduling": {
            "environment_id": session.environment_id,
        },
        "execution_substrate": {
            "runtime_profile_id": runtime_profile.map(|profile| profile.id),
            "runtime_type": runtime_profile.map(|profile| profile.runtime_type.as_str()),
            "provider": agent_version.provider,
            "model": agent_version.model,
            "tools": agent_version.tools,
        },
    })
}

pub(crate) async fn effective_handoff_assignment_context(
    state: &AppState,
    session_id: Uuid,
) -> Result<Option<AgentHandoffAssignment>, AppError> {
    Ok(state
        .list_agent_handoff_assignments(Some(session_id))
        .await?
        .into_iter()
        .filter(|assignment| assignment.specialist_session_id == session_id)
        .max_by_key(|assignment| assignment.created_at))
}

pub(crate) async fn resolve_context_packet_runtime_profile(
    state: &AppState,
    session: &Session,
    agent_version: &AgentVersion,
    handoff_assignment: Option<&AgentHandoffAssignment>,
) -> Result<
    (
        Option<Uuid>,
        Option<&'static str>,
        Option<(Uuid, Result<AgentRuntimeProfile, AppError>)>,
    ),
    AppError,
> {
    if let Some(environment_id) = session.environment_id {
        let environment = state.get_environment(environment_id).await?;
        if let Some(profile_id) = environment.runtime_profile_id {
            return Ok((
                Some(profile_id),
                Some("environment"),
                Some((
                    profile_id,
                    state.get_agent_runtime_profile(profile_id).await,
                )),
            ));
        }
    }
    if let Some(profile_id) =
        handoff_assignment.and_then(|assignment| assignment.runtime_profile_id)
    {
        return Ok((
            Some(profile_id),
            Some("handoff"),
            Some((
                profile_id,
                state.get_agent_runtime_profile(profile_id).await,
            )),
        ));
    }
    let Some(profile_id) = agent_version.runtime_profile_id else {
        return Ok((None, None, None));
    };
    let snapshot_present = agent_version
        .runtime_profile_snapshot
        .as_object()
        .is_some_and(|snapshot| !snapshot.is_empty());
    let (source, profile) = if snapshot_present {
        (
            "agent_version_snapshot",
            serde_json::from_value::<AgentRuntimeProfile>(
                agent_version.runtime_profile_snapshot.clone(),
            )
            .map_err(|error| {
                AppError::bad_request(format!(
                    "agent version runtime profile snapshot is invalid: {error}"
                ))
            }),
        )
    } else {
        (
            "agent_version_profile_fallback",
            state.get_agent_runtime_profile(profile_id).await,
        )
    };
    Ok((Some(profile_id), Some(source), Some((profile_id, profile))))
}

pub(crate) fn context_packet_runtime_profile(
    profile: &AgentRuntimeProfile,
) -> ContextPacketRuntimeProfile {
    ContextPacketRuntimeProfile {
        id: profile.id,
        name: profile.name.clone(),
        runtime_type: profile.runtime_type.clone(),
        remote_computer_required: profile.remote_computer_required,
        status: profile.status.clone(),
    }
}

pub(crate) fn apply_task_grant_memory_scope_to_context_objects(
    objects: Vec<ContextPacketSemanticObject>,
    memory_scope: &Value,
) -> Vec<ContextPacketSemanticObject> {
    let allowed_object_types = memory_scope
        .get("allowed_object_types")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();
    let allowed_object_ids = memory_scope
        .get("allowed_object_ids")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();
    let minimum_trust_level = memory_scope
        .get("minimum_trust_level")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let mut filtered = objects
        .into_iter()
        .filter(|object| {
            allowed_object_types.is_empty() || allowed_object_types.contains(&object.object_type)
        })
        .filter(|object| {
            allowed_object_ids.is_empty() || allowed_object_ids.contains(&object.id.to_string())
        })
        .filter(|object| {
            minimum_trust_level
                .is_none_or(|minimum| memory_trust_level_satisfies(&object.trust_level, minimum))
        })
        .collect::<Vec<_>>();
    if let Some(max_objects) = memory_scope.get("max_objects").and_then(Value::as_u64) {
        filtered.truncate(max_objects as usize);
    }
    filtered
}

pub(crate) fn task_grant_memory_scope_mode(memory_scope: &Value) -> &str {
    memory_scope
        .get("mode")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("snapshot_only")
}

pub(crate) fn context_semantic_object_satisfies_memory_scope(
    object: &ContextPacketSemanticObject,
    memory_scope: &Value,
) -> bool {
    let allowed_object_types = memory_scope
        .get("allowed_object_types")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();
    let allowed_object_ids = memory_scope
        .get("allowed_object_ids")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();
    let minimum_trust_level = memory_scope
        .get("minimum_trust_level")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    (allowed_object_types.is_empty() || allowed_object_types.contains(object.object_type.as_str()))
        && (allowed_object_ids.is_empty()
            || allowed_object_ids.contains(object.id.to_string().as_str()))
        && minimum_trust_level
            .is_none_or(|minimum| memory_trust_level_satisfies(&object.trust_level, minimum))
}

pub(crate) fn memory_trust_level_satisfies(actual: &str, minimum: &str) -> bool {
    match (memory_trust_rank(actual), memory_trust_rank(minimum)) {
        (Some(actual), Some(minimum)) => actual >= minimum,
        _ => actual == minimum,
    }
}

pub(crate) fn memory_trust_rank(value: &str) -> Option<i32> {
    match value.trim().to_ascii_lowercase().as_str() {
        "none" | "unverified" => Some(0),
        "verified" | "source_attested" => Some(1),
        "human_verified" => Some(2),
        "system_verified" => Some(3),
        _ => None,
    }
}

pub(crate) fn build_context_policy_reminders(
    policy: &PolicyConfig,
    agent_version: &AgentVersion,
) -> Vec<String> {
    let tools = agent_version
        .tools
        .iter()
        .chain(agent_version.tool_names.iter())
        .filter(|tool| !tool.trim().is_empty())
        .cloned()
        .collect::<BTreeSet<_>>();
    if tools.is_empty() {
        return vec![format!(
            "agent version {} has no enabled tools; runtime actions should stay unavailable",
            agent_version.version
        )];
    }
    tools
        .into_iter()
        .map(|tool| {
            let decision = policy.evaluate_tool_for_agent_version(&tool, agent_version);
            format!(
                "{}: {} ({}) - {}",
                tool, decision.decision, decision.risk_level, decision.reason
            )
        })
        .collect()
}

pub(crate) fn build_context_freshness_warnings(
    agent: &Agent,
    agent_version: &AgentVersion,
    effective_semantic_scopes: &Value,
    effective_runtime_profile_id: Option<Uuid>,
    runtime_profile_lookup: Option<&(Uuid, Result<AgentRuntimeProfile, AppError>)>,
    last_user_message: Option<&str>,
) -> Vec<String> {
    let mut warnings = Vec::new();
    let missing_scopes = missing_semantic_scope_keys(effective_semantic_scopes);
    if !missing_scopes.is_empty() {
        warnings.push(format!(
            "semantic_scopes missing required keys: {}",
            missing_scopes.join(", ")
        ));
    }
    if value_is_empty_object(&agent_version.approval_policy) {
        warnings.push(
            "tool_policy is empty; runtime policy reminders are the only tool governance context"
                .to_string(),
        );
    }
    if agent_version.workflow_pack_ids.is_empty() {
        warnings
            .push("no workflow_pack_ids are bound; domain workflow context is generic".to_string());
    }
    if effective_runtime_profile_id.is_none() {
        warnings.push(
            "no runtime_profile_id is bound; execution may rely on provider or environment fallback"
                .to_string(),
        );
    }
    if let Some((profile_id, result)) = runtime_profile_lookup {
        match result {
            Ok(profile) if profile.status != "enabled" => warnings.push(format!(
                "runtime profile {} is {}; execution should fail closed until enabled",
                profile.name, profile.status
            )),
            Err(error) => warnings.push(format!(
                "runtime profile {profile_id} is unavailable: {}",
                error.message
            )),
            _ => {}
        }
    }
    if agent.release_state != "active" {
        warnings.push(format!(
            "agent release_state is {}; treat packet as non-production unless promoted",
            agent.release_state
        ));
    }
    if last_user_message.is_none() {
        warnings.push(
            "session has no user.message event; task context is based on the session title only"
                .to_string(),
        );
    }
    warnings
}

pub(crate) fn build_context_packet_source_refs(
    session: &Session,
    agent: &Agent,
    agent_version: &AgentVersion,
    runtime_profile: Option<&ContextPacketRuntimeProfile>,
    missing_runtime_profile_id: Option<Uuid>,
    runtime_profile_source: Option<&str>,
    event_count: usize,
    retrieved_objects: &[ContextPacketSemanticObject],
    handoff_assignment_context: Option<&AgentHandoffAssignment>,
    context_task_grant: Option<&TaskGrant>,
) -> Vec<ContextPacketSourceRef> {
    let mut source_refs = vec![
        context_source_ref("session", session.id, "current"),
        context_source_ref(
            "agent",
            agent.id,
            &format!("release_state:{}", agent.release_state),
        ),
        context_source_ref(
            "agent_version",
            agent_version.id,
            &format!("version:{}", agent_version.version),
        ),
        ContextPacketSourceRef {
            source_type: "session_events".to_string(),
            source_id: format!("{}:{event_count}", session.id),
            freshness: "current_snapshot".to_string(),
        },
        ContextPacketSourceRef {
            source_type: "policy".to_string(),
            source_id: "runtime_policy".to_string(),
            freshness: "current".to_string(),
        },
    ];
    if let Some(profile) = runtime_profile {
        source_refs.push(context_source_ref(
            "agent_runtime_profile",
            profile.id,
            &format!(
                "source:{}:status:{}",
                runtime_profile_source.unwrap_or("unknown"),
                profile.status
            ),
        ));
    }
    if let Some(profile_id) = missing_runtime_profile_id {
        source_refs.push(context_source_ref(
            "agent_runtime_profile",
            profile_id,
            &format!(
                "source:{}:unavailable",
                runtime_profile_source.unwrap_or("unknown")
            ),
        ));
    }
    if let Some(assignment) = handoff_assignment_context {
        source_refs.push(context_source_ref(
            "agent_handoff_assignment",
            assignment.id,
            &format!("status:{}", assignment.status),
        ));
        source_refs.push(context_source_ref(
            "agent_handoff_event",
            assignment.agent_handoff_event_id,
            "assignment_context",
        ));
    }
    if let Some(grant) = context_task_grant {
        source_refs.push(context_source_ref(
            "task_grant",
            grant.id,
            &format!("status:{}", grant.status),
        ));
    }
    for path in [
        "AGENTS.md",
        "README.md",
        "docs/mandoforge-roadmap-v2.md",
        "tasks/todo.md",
    ] {
        if project_file_path(path).is_some() {
            source_refs.push(ContextPacketSourceRef {
                source_type: "repo_doc".to_string(),
                source_id: path.to_string(),
                freshness: "workspace_current".to_string(),
            });
        }
    }
    for object in retrieved_objects {
        source_refs.push(ContextPacketSourceRef {
            source_type: "semantic_object".to_string(),
            source_id: object.id.to_string(),
            freshness: format!("{}:{}", object.freshness, object.trust_level),
        });
        if let Some(source_id) = object.source_id {
            source_refs.push(ContextPacketSourceRef {
                source_type: "semantic_source".to_string(),
                source_id: source_id.to_string(),
                freshness: object.freshness.clone(),
            });
        }
    }
    source_refs
}

pub(crate) async fn retrieve_context_packet_semantic_objects(
    state: &AppState,
    semantic_scopes: &Value,
) -> Result<Vec<ContextPacketSemanticObject>, AppError> {
    let registry = semantic_retrieval_backend_registry_from_env();
    if registry.effective_backend != "scope_rank" {
        return Err(AppError::bad_request(format!(
            "semantic retrieval backend {} is not executable; object/link/context packet scope_rank remains required",
            registry.effective_backend
        )));
    }
    let mut objects = state
        .list_semantic_objects()
        .await?
        .into_iter()
        .filter(|object| object.status == "active")
        .filter(|object| semantic_object_matches_scope(object, semantic_scopes))
        .map(context_packet_semantic_object_from_store_object)
        .collect::<Vec<_>>();
    objects.sort_by(|left, right| {
        semantic_object_rank(right)
            .cmp(&semantic_object_rank(left))
            .then_with(|| {
                semantic_object_scope_specificity(right, semantic_scopes)
                    .cmp(&semantic_object_scope_specificity(left, semantic_scopes))
            })
            .then_with(|| left.object_key.cmp(&right.object_key))
    });
    objects.truncate(12);
    Ok(objects)
}

pub(crate) fn context_packet_semantic_object_from_store_object(
    object: SemanticObject,
) -> ContextPacketSemanticObject {
    ContextPacketSemanticObject {
        id: object.id,
        object_type: object.object_type,
        object_key: object.object_key,
        title: object.title,
        summary: object.summary,
        source_id: object.source_id,
        source_uri: object.source_uri,
        trust_level: object.trust_level,
        freshness: object.freshness,
        semantic_scopes: object.semantic_scopes,
        provenance: object.provenance,
    }
}

pub(crate) fn semantic_retrieval_backend_registry_from_env() -> SemanticRetrievalBackendRegistry {
    semantic_retrieval_backend_registry_from_lookup(|key| std::env::var(key).ok())
}

pub(crate) fn semantic_retrieval_backend_registry_from_lookup<F>(
    lookup: F,
) -> SemanticRetrievalBackendRegistry
where
    F: Fn(&str) -> Option<String>,
{
    let selected_backend = lookup("MANDOFORGE_SEMANTIC_RETRIEVAL_BACKEND")
        .map(|backend| backend.trim().to_ascii_lowercase())
        .filter(|backend| !backend.is_empty())
        .unwrap_or_else(|| "scope_rank".to_string());
    let specs = [
        (
            "scope_rank",
            "object_link_context_packet",
            Vec::<&str>::new(),
            "active",
        ),
        (
            "pgvector",
            "vector",
            vec!["MANDOFORGE_PGVECTOR_DSN"],
            "reserved",
        ),
        (
            "qdrant",
            "vector",
            vec!["MANDOFORGE_QDRANT_URL"],
            "reserved",
        ),
        (
            "weaviate",
            "vector",
            vec!["MANDOFORGE_WEAVIATE_URL"],
            "reserved",
        ),
    ];
    let known_backends = specs
        .iter()
        .map(|(backend, _, _, _)| *backend)
        .collect::<BTreeSet<_>>();
    let mut fail_closed = !known_backends.contains(selected_backend.as_str());
    let mut backends = specs
        .iter()
        .map(|(backend, backend_type, required_env_vars, base_status)| {
            let missing_env_vars = required_env_vars
                .iter()
                .filter(|key| lookup(key).is_none())
                .map(|key| (*key).to_string())
                .collect::<Vec<_>>();
            let configured = missing_env_vars.is_empty();
            let selected = selected_backend == *backend;
            let effective = *backend == "scope_rank";
            let mut blocking_reasons = Vec::new();
            if *backend != "scope_rank" {
                blocking_reasons.push(
                    "optional retrieval backend is reserved; scope_rank remains the executable semantic layer".to_string(),
                );
                if !configured {
                    blocking_reasons.push(format!(
                        "missing required env vars: {}",
                        missing_env_vars.join(", ")
                    ));
                }
            }
            if selected && *backend != "scope_rank" {
                fail_closed = true;
                blocking_reasons.push(
                    "selected backend is not enabled for context packet execution".to_string(),
                );
            }
            let status = if *backend == "scope_rank" {
                "active"
            } else if configured {
                "configured_reserved"
            } else {
                base_status
            };
            SemanticRetrievalBackendStatus {
                backend: (*backend).to_string(),
                backend_type: (*backend_type).to_string(),
                status: status.to_string(),
                selected,
                effective,
                configured,
                required_env_vars: required_env_vars
                    .iter()
                    .map(|key| (*key).to_string())
                    .collect(),
                missing_env_vars,
                object_link_context_packet_required: true,
                blocking_reasons,
            }
        })
        .collect::<Vec<_>>();
    if !known_backends.contains(selected_backend.as_str()) {
        backends.push(SemanticRetrievalBackendStatus {
            backend: selected_backend.clone(),
            backend_type: "unknown".to_string(),
            status: "blocked".to_string(),
            selected: true,
            effective: false,
            configured: false,
            required_env_vars: Vec::new(),
            missing_env_vars: Vec::new(),
            object_link_context_packet_required: true,
            blocking_reasons: vec![
                "unknown semantic retrieval backend selection; falling back to scope_rank"
                    .to_string(),
            ],
        });
    }
    SemanticRetrievalBackendRegistry {
        selected_backend,
        effective_backend: "scope_rank".to_string(),
        fail_closed,
        object_model_required: true,
        backends,
    }
}

pub(crate) fn semantic_object_matches_agent_scope(object: &SemanticObject, agent: &Agent) -> bool {
    semantic_object_matches_scope(object, &agent.semantic_scopes)
}

pub(crate) fn semantic_object_matches_scope(
    object: &SemanticObject,
    semantic_scopes: &Value,
) -> bool {
    let Some(object_scopes) = object.semantic_scopes.as_object() else {
        return false;
    };
    let Some(agent_scopes) = semantic_scopes.as_object() else {
        return false;
    };
    let mut matched_scope_count = 0usize;
    for (key, object_value) in object_scopes {
        let Some(object_scope) = object_value.as_str().map(str::trim) else {
            continue;
        };
        if object_scope.is_empty() {
            continue;
        }
        matched_scope_count += 1;
        let matches = agent_scopes
            .get(key)
            .and_then(Value::as_str)
            .is_some_and(|agent_scope| agent_scope.trim() == object_scope);
        if !matches {
            return false;
        }
    }
    matched_scope_count > 0
}

pub(crate) fn semantic_object_rank(object: &ContextPacketSemanticObject) -> i32 {
    let freshness_score = match object.freshness.as_str() {
        "current" => 30,
        "unknown" => 10,
        "stale" => 5,
        "expired" => 0,
        _ => 0,
    };
    let trust_score = match object.trust_level.as_str() {
        "system_verified" => 30,
        "human_verified" => 25,
        "source_attested" => 15,
        "unverified" => 5,
        _ => 0,
    };
    freshness_score + trust_score
}

pub(crate) fn semantic_object_scope_specificity(
    object: &ContextPacketSemanticObject,
    semantic_scopes: &Value,
) -> usize {
    let Some(object_scopes) = object.semantic_scopes.as_object() else {
        return 0;
    };
    let Some(context_scopes) = semantic_scopes.as_object() else {
        return 0;
    };
    object_scopes
        .iter()
        .filter(|(key, object_value)| {
            let Some(object_scope) = object_value.as_str().map(str::trim) else {
                return false;
            };
            if object_scope.is_empty() {
                return false;
            }
            context_scopes
                .get(*key)
                .and_then(Value::as_str)
                .is_some_and(|context_scope| context_scope.trim() == object_scope)
        })
        .count()
}

pub(crate) fn render_execution_context(
    packet: &ContextPacket,
    input: RenderContextPacketRequest,
) -> RenderedExecutionContext {
    let max_prompt_tokens = bounded_render_budget(input.max_prompt_tokens, 1_500, 256, 8_000);
    let max_objects = bounded_render_budget(input.max_objects, 5, 1, 20);
    let max_summary_chars = bounded_render_budget(input.max_summary_chars, 280, 32, 1_200);
    let max_policy_reminders = bounded_render_budget(input.max_policy_reminders, 3, 0, 12);
    let allow_on_demand_fetch = input.allow_on_demand_fetch.unwrap_or(true);
    let _allow_full_content = input.allow_full_content.unwrap_or(false);
    let task = rendered_context_task_projection(packet, max_summary_chars);
    let context_layers = packet
        .replay_summary
        .get("context_layers")
        .map(compact_rendered_context_layers)
        .unwrap_or_else(empty_json_object);

    let mut must_follow = packet
        .policy_reminders
        .iter()
        .take(max_policy_reminders)
        .cloned()
        .collect::<Vec<_>>();
    if must_follow.len() < max_policy_reminders {
        must_follow.extend(
            packet
                .freshness_warnings
                .iter()
                .take(max_policy_reminders - must_follow.len())
                .map(|warning| format!("Freshness warning: {warning}")),
        );
    }

    let mut omitted = RenderedContextOmissions {
        policy_reminders_omitted: packet
            .policy_reminders
            .len()
            .saturating_sub(max_policy_reminders),
        source_refs_not_rendered: packet.source_refs.len(),
        full_content_not_rendered: packet.retrieved_objects.len(),
        ..Default::default()
    };

    let mut estimated_tokens_used = estimate_rendered_context_base_tokens(packet, &must_follow)
        + estimate_tokens(&task.to_string())
        + estimate_tokens(&context_layers.to_string());
    let mut relevant_objects = Vec::new();
    for object in &packet.retrieved_objects {
        if relevant_objects.len() >= max_objects {
            omitted.object_limit_exceeded += 1;
            continue;
        }
        let rendered = RenderedSemanticObject {
            id: object.id,
            object_type: object.object_type.clone(),
            object_key: object.object_key.clone(),
            title: truncate_for_execution_context(&object.title, max_summary_chars),
            summary: truncate_for_execution_context(&object.summary, max_summary_chars),
            trust_level: object.trust_level.clone(),
            freshness: object.freshness.clone(),
            source_uri: object.source_uri.clone(),
        };
        let object_tokens = estimate_tokens(&format!(
            "{} {} {} {} {}",
            rendered.object_type,
            rendered.object_key,
            rendered.title,
            rendered.summary,
            rendered.source_uri.clone().unwrap_or_default()
        ));
        if estimated_tokens_used + object_tokens > max_prompt_tokens {
            omitted.token_budget_exceeded += 1;
            continue;
        }
        estimated_tokens_used += object_tokens;
        relevant_objects.push(rendered);
    }

    let mut available_tools = if allow_on_demand_fetch {
        vec![
            "semantic_object.fetch".to_string(),
            "semantic_object.search".to_string(),
            "semantic_link.expand".to_string(),
            "ontology_type.lookup".to_string(),
        ]
    } else {
        Vec::new()
    };
    if packet
        .agent
        .tools
        .iter()
        .any(|tool| tool == "ontology.action.execute")
    {
        available_tools.push("ontology.action.execute".to_string());
    }
    let fetchable_object_ids = if allow_on_demand_fetch {
        packet
            .retrieved_objects
            .iter()
            .map(|object| object.id)
            .collect()
    } else {
        Vec::new()
    };

    RenderedExecutionContext {
        context_packet_id: packet.id,
        session_id: packet.session_id,
        agent_id: packet.agent_id,
        context_packet_version: packet.version,
        task,
        context_layers,
        ontology_scope: render_ontology_scope(&packet.semantic_scopes),
        role: packet.agent.agent_role.clone(),
        must_follow,
        relevant_objects,
        fetchable_object_ids,
        omitted,
        budget: RenderedContextBudget {
            max_prompt_tokens,
            estimated_tokens_used,
            max_objects,
            max_summary_chars,
            max_policy_reminders,
        },
        available_tools,
        full_content_included: false,
    }
}

pub(crate) fn rendered_context_task_projection(
    packet: &ContextPacket,
    max_summary_chars: usize,
) -> Value {
    let mut task = serde_json::Map::new();
    for key in ["title", "objective"] {
        if let Some(value) = packet.task.get(key).and_then(Value::as_str) {
            task.insert(
                key.to_string(),
                Value::String(truncate_for_execution_context(value, max_summary_chars)),
            );
        }
    }
    for key in ["status", "workflow_run_id", "workflow_step_run_id"] {
        if let Some(value) = packet.task.get(key).filter(|value| !value.is_null()) {
            task.insert(key.to_string(), value.clone());
        }
    }
    Value::Object(task)
}

pub(crate) fn compact_rendered_context_layers(layers: &Value) -> Value {
    let Some(layers) = layers.as_object() else {
        return empty_json_object();
    };
    Value::Object(
        layers
            .iter()
            .map(|(name, layer)| {
                let mut layer = compact_non_null_json(layer).unwrap_or_else(empty_json_object);
                if let Some(layer) = layer.as_object_mut() {
                    match name.as_str() {
                        "managed_runtime" => {
                            layer.remove("session_id");
                            layer.remove("agent_id");
                        }
                        "execution_substrate" => {
                            layer.remove("tools");
                        }
                        _ => {}
                    }
                }
                (name.clone(), layer)
            })
            .collect(),
    )
}

fn compact_non_null_json(value: &Value) -> Option<Value> {
    match value {
        Value::Null => None,
        Value::Object(object) => Some(Value::Object(
            object
                .iter()
                .filter_map(|(key, value)| {
                    compact_non_null_json(value).map(|value| (key.clone(), value))
                })
                .collect(),
        )),
        Value::Array(values) => Some(Value::Array(
            values.iter().filter_map(compact_non_null_json).collect(),
        )),
        _ => Some(value.clone()),
    }
}

pub(crate) async fn render_execution_context_for_packet(
    state: &AppState,
    packet: &ContextPacket,
    input: RenderContextPacketRequest,
) -> Result<RenderedExecutionContext, AppError> {
    let mut rendered = render_execution_context(packet, input);
    let release_metadata = match packet.replay_summary.get("ontology_release") {
        Some(snapshot) => Some(snapshot.clone()),
        None => active_ontology_release_metadata_for_scopes(state, &packet.semantic_scopes).await?,
    };
    if let Some(release_metadata) = release_metadata {
        if !rendered.ontology_scope.is_object() {
            rendered.ontology_scope = json!({});
        }
        if let Some(scope) = rendered.ontology_scope.as_object_mut() {
            scope.insert("ontology_release".to_string(), release_metadata);
        }
    }
    Ok(rendered)
}

pub(crate) async fn active_ontology_release_metadata_for_scopes(
    state: &AppState,
    semantic_scopes: &Value,
) -> Result<Option<Value>, AppError> {
    let Some(scopes) = semantic_scopes.as_object() else {
        return Ok(None);
    };
    let Some(scope) = scopes
        .get("domain_scope")
        .and_then(Value::as_str)
        .map(str::trim)
    else {
        return Ok(None);
    };
    if scope.is_empty() {
        return Ok(None);
    }
    if let Some(release) = state.active_ontology_release_for_domain(scope).await? {
        return Ok(Some(ontology_release_runtime_metadata(&release)));
    }
    Ok(Some(Value::Null))
}

pub(crate) fn ontology_release_runtime_metadata(release: &OntologyRelease) -> Value {
    let catalog_digest = release
        .evidence_refs
        .as_array()
        .and_then(|entries| {
            entries.iter().find(|entry| {
                entry.get("schema").and_then(Value::as_str)
                    == Some(crate::ONTOLOGY_RELEASE_CATALOG_SCHEMA)
            })
        })
        .and_then(|entry| entry.get("digest"))
        .cloned()
        .unwrap_or(Value::Null);
    json!({
        "id": release.id,
        "version": release.version,
        "domain_scope": release.domain_scope,
        "status": release.status,
        "release_class": release.release_class,
        "object_count": release.object_count,
        "relation_count": release.relation_count,
        "action_count": release.action_count,
        "catalog_digest": catalog_digest,
        "source_run_id": release.source_run_id,
        "parent_release_id": release.parent_release_id,
        "rollback_target_release_id": release.rollback_target_release_id,
        "gate_status": release.gate_result.get("status").and_then(Value::as_str),
        "promoted_at": release.promoted_at,
        "pinned_by": "active_ontology_release",
    })
}

pub(crate) fn bounded_render_budget(
    requested: Option<usize>,
    default: usize,
    minimum: usize,
    maximum: usize,
) -> usize {
    requested.unwrap_or(default).clamp(minimum, maximum)
}

pub(crate) fn render_ontology_scope(semantic_scopes: &Value) -> Value {
    let Some(scopes) = semantic_scopes.as_object() else {
        return json!({});
    };
    let rendered = scopes
        .iter()
        .filter(|(key, _)| key.ends_with("_scope"))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<serde_json::Map<_, _>>();
    Value::Object(rendered)
}

pub(crate) fn truncate_for_execution_context(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let keep = max_chars.saturating_sub(3);
    let mut output = trimmed.chars().take(keep).collect::<String>();
    output.push_str("...");
    output
}

pub(crate) fn estimate_rendered_context_base_tokens(
    packet: &ContextPacket,
    must_follow: &[String],
) -> usize {
    estimate_tokens(&format!(
        "{} {} {} {}",
        packet.id,
        packet.agent.agent_role,
        packet.semantic_scopes,
        must_follow.join(" ")
    ))
}

pub(crate) fn estimate_tokens(value: &str) -> usize {
    value.chars().count().div_ceil(4).max(1)
}

pub(crate) fn ensure_context_packet_allows_semantic_object(
    packet: &ContextPacket,
    object_id: Uuid,
) -> Result<(), AppError> {
    if packet
        .retrieved_objects
        .iter()
        .any(|object| object.id == object_id)
    {
        Ok(())
    } else {
        Err(AppError::forbidden(
            "semantic object is not available in this context packet",
        ))
    }
}

pub(crate) fn context_packet_semantic_object_ids(packet: &ContextPacket) -> HashSet<Uuid> {
    packet
        .retrieved_objects
        .iter()
        .map(|object| object.id)
        .collect()
}

pub(crate) async fn fetch_semantic_object_for_context(
    state: &AppState,
    packet: &ContextPacket,
    object_id: Uuid,
    include_content: bool,
) -> Result<FetchSemanticObjectResponse, AppError> {
    ensure_context_packet_allows_semantic_object(packet, object_id)?;
    let object = state.get_semantic_object(object_id).await?;
    let response = FetchSemanticObjectResponse {
        context_packet_id: packet.id,
        object: FetchableSemanticObject {
            id: object.id,
            object_type: object.object_type,
            object_key: object.object_key,
            title: object.title,
            summary: object.summary,
            content: include_content.then_some(object.content),
            semantic_scopes: object.semantic_scopes,
            source_uri: object.source_uri,
            trust_level: object.trust_level,
            freshness: object.freshness,
        },
        content_included: include_content,
        fetch_policy: json!({
            "boundary": "context_packet",
            "context_packet_id": packet.id,
            "allowed_by_retrieved_objects": true,
            "full_content_requires_explicit_request": true,
        }),
    };
    record_semantic_object_fetch(state, packet, &response).await?;
    Ok(response)
}

pub(crate) async fn search_semantic_objects_for_context(
    state: &AppState,
    packet: &ContextPacket,
    grant: Option<&TaskGrant>,
    input: SearchSemanticObjectsRequest,
) -> Result<SearchSemanticObjectsResponse, AppError> {
    if input.context_packet_id != packet.id {
        return Err(AppError::forbidden(
            "semantic search context_packet_id does not match the verified context packet",
        ));
    }
    let max_results = input.max_results.unwrap_or(10).clamp(1, 25);
    let query = input
        .query
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let object_type = input
        .object_type
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let scoped_lookup_allowed = grant
        .map(|grant| task_grant_memory_scope_mode(&grant.memory_scope) == "scoped_lookup")
        .unwrap_or(false);
    let boundary = if scoped_lookup_allowed {
        "task_grant_memory_scope"
    } else {
        "context_packet"
    };
    let mut candidates = if scoped_lookup_allowed {
        let Some(grant) = grant else {
            return Err(AppError::forbidden(
                "semantic scoped lookup requires an active TaskGrant",
            ));
        };
        let mut objects = state
            .list_semantic_objects()
            .await?
            .into_iter()
            .filter(|object| object.status == "active")
            .filter(|object| semantic_object_matches_scope(object, &packet.semantic_scopes))
            .map(context_packet_semantic_object_from_store_object)
            .filter(|object| {
                context_semantic_object_satisfies_memory_scope(object, &grant.memory_scope)
            })
            .collect::<Vec<_>>();
        objects.sort_by(|left, right| {
            semantic_object_rank(right)
                .cmp(&semantic_object_rank(left))
                .then_with(|| {
                    semantic_object_scope_specificity(right, &packet.semantic_scopes).cmp(
                        &semantic_object_scope_specificity(left, &packet.semantic_scopes),
                    )
                })
                .then_with(|| left.object_key.cmp(&right.object_key))
        });
        objects
    } else {
        packet.retrieved_objects.clone()
    };
    let before_filter_count = candidates.len();
    if let Some(object_type) = object_type.as_deref() {
        candidates.retain(|object| object.object_type == object_type);
    }
    if let Some(query) = query.as_deref() {
        let query = query.to_ascii_lowercase();
        candidates.retain(|object| {
            [
                object.object_type.as_str(),
                object.object_key.as_str(),
                object.title.as_str(),
                object.summary.as_str(),
            ]
            .into_iter()
            .any(|value| value.to_ascii_lowercase().contains(&query))
        });
    }
    let matched_before_limit = candidates.len();
    let results = candidates
        .into_iter()
        .take(max_results)
        .map(|object| RenderedSemanticObject {
            id: object.id,
            object_type: object.object_type,
            object_key: object.object_key,
            title: object.title,
            summary: truncate_for_execution_context(&object.summary, 280),
            trust_level: object.trust_level,
            freshness: object.freshness,
            source_uri: object.source_uri,
        })
        .collect::<Vec<_>>();
    let response = SearchSemanticObjectsResponse {
        context_packet_id: packet.id,
        boundary: boundary.to_string(),
        query,
        object_type,
        results,
        omitted: json!({
            "before_filter_count": before_filter_count,
            "matched_before_limit": matched_before_limit,
            "max_results": max_results,
            "scoped_lookup_allowed": scoped_lookup_allowed,
        }),
    };
    record_semantic_objects_searched(state, packet, &response).await?;
    Ok(response)
}

pub(crate) async fn expand_semantic_links_for_context(
    state: &AppState,
    packet: &ContextPacket,
    input: ExpandSemanticLinksRequest,
) -> Result<ExpandSemanticLinksResponse, AppError> {
    ensure_context_packet_allows_semantic_object(packet, input.object_id)?;
    let max_links = input.max_links.unwrap_or(10).clamp(1, 50);
    let allowed_object_ids = context_packet_semantic_object_ids(packet);
    let object_id = input.object_id.to_string();
    let relation_type = input
        .relation_type
        .as_deref()
        .map(normalized_ontology_token)
        .filter(|value| !value.is_empty());
    let mut outside_packet_count = 0usize;
    let mut relation_filtered_count = 0usize;
    let mut links = Vec::new();
    for link in state
        .list_semantic_links()
        .await?
        .into_iter()
        .filter(|link| link.status == "active")
        .filter(|link| {
            (link.from_entity_type == "semantic_object" && link.from_entity_id == object_id)
                || (link.to_entity_type == "semantic_object" && link.to_entity_id == object_id)
        })
    {
        if relation_type
            .as_ref()
            .is_some_and(|expected| normalized_ontology_token(&link.relation_type) != *expected)
        {
            relation_filtered_count += 1;
            continue;
        }
        let endpoints_allowed = [link.from_entity_id.as_str(), link.to_entity_id.as_str()]
            .into_iter()
            .filter_map(|value| Uuid::parse_str(value).ok())
            .all(|id| allowed_object_ids.contains(&id));
        if !endpoints_allowed {
            outside_packet_count += 1;
            continue;
        }
        if links.len() < max_links {
            links.push(link);
        }
    }
    let response = ExpandSemanticLinksResponse {
        context_packet_id: packet.id,
        object_id: input.object_id,
        links,
        omitted: json!({
            "outside_context_packet": outside_packet_count,
            "relation_type_filtered": relation_filtered_count,
            "max_links": max_links,
        }),
    };
    record_semantic_links_expanded(state, packet, &response).await?;
    Ok(response)
}

pub(crate) async fn record_semantic_object_fetch(
    state: &AppState,
    packet: &ContextPacket,
    response: &FetchSemanticObjectResponse,
) -> Result<(), AppError> {
    let details = json!({
        "context_packet_id": packet.id,
        "context_packet_version": packet.version,
        "semantic_object_id": response.object.id,
        "object_key": response.object.object_key,
        "content_included": response.content_included,
    });
    state
        .append_event(
            "tool",
            Some(response.object.id),
            packet.session_id,
            "semantic_object.fetched",
            details.clone(),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(packet.session_id),
            "tool",
            Some(response.object.id),
            "semantic_object.fetched",
            "semantic_object",
            Some(response.object.id),
            details,
        ))
        .await?;
    Ok(())
}

pub(crate) async fn record_semantic_objects_searched(
    state: &AppState,
    packet: &ContextPacket,
    response: &SearchSemanticObjectsResponse,
) -> Result<(), AppError> {
    let details = json!({
        "context_packet_id": packet.id,
        "context_packet_version": packet.version,
        "boundary": response.boundary,
        "query": response.query,
        "object_type": response.object_type,
        "result_count": response.results.len(),
        "result_ids": response.results.iter().map(|object| object.id).collect::<Vec<_>>(),
        "omitted": response.omitted,
    });
    state
        .append_event(
            "tool",
            Some(packet.id),
            packet.session_id,
            "semantic_objects.searched",
            details.clone(),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(packet.session_id),
            "tool",
            Some(packet.id),
            "semantic_objects.searched",
            "context_packet",
            Some(packet.id),
            details,
        ))
        .await?;
    Ok(())
}

pub(crate) async fn record_semantic_links_expanded(
    state: &AppState,
    packet: &ContextPacket,
    response: &ExpandSemanticLinksResponse,
) -> Result<(), AppError> {
    let details = json!({
        "context_packet_id": packet.id,
        "context_packet_version": packet.version,
        "semantic_object_id": response.object_id,
        "link_count": response.links.len(),
        "link_ids": response.links.iter().map(|link| link.id).collect::<Vec<_>>(),
        "omitted": response.omitted.clone(),
    });
    state
        .append_event(
            "tool",
            Some(response.object_id),
            packet.session_id,
            "semantic_links.expanded",
            details.clone(),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(packet.session_id),
            "tool",
            Some(response.object_id),
            "semantic_links.expanded",
            "semantic_object",
            Some(response.object_id),
            details,
        ))
        .await?;
    Ok(())
}

pub(crate) async fn evaluate_semantic_context_gate(
    state: &AppState,
    agent: &Agent,
    task_grant: Option<&TaskGrant>,
    input: &ExecuteTool,
    risk_level: &str,
    decision: &str,
) -> Result<Option<Value>, AppError> {
    if risk_level != "high" || decision == "denied" {
        return Ok(None);
    }

    let mut evaluated_object_count = 0usize;
    let mut blockers = Vec::new();
    let referenced_object_ids = semantic_object_ref_ids_from_tool_args(&input.args);
    if let Some(grant) = task_grant {
        let Some(context_packet_id) = grant.context_packet_id else {
            return Ok(Some(json!({
                "status": "blocked",
                "risk_level": risk_level,
                "boundary": "task_grant_context_packet",
                "task_grant_id": grant.id,
                "context_packet_id": Value::Null,
                "referenced_object_count": referenced_object_ids.len(),
                "evaluated_object_count": 0,
                "blocked_object_count": 1,
                "required_freshness": "current",
                "blocked_trust_levels": ["unverified"],
                "blockers": [{
                    "id": grant.id,
                    "reasons": ["missing_context_packet"],
                }],
            })));
        };
        let packet = state.get_context_packet(context_packet_id).await?;
        let packet_objects = packet
            .retrieved_objects
            .iter()
            .map(|object| (object.id, object))
            .collect::<HashMap<_, _>>();
        let evaluated_objects = if referenced_object_ids.is_empty() {
            packet.retrieved_objects.iter().collect::<Vec<_>>()
        } else {
            let mut objects = Vec::new();
            for object_id in &referenced_object_ids {
                if let Some(object) = packet_objects.get(object_id) {
                    objects.push(*object);
                } else {
                    blockers.push(json!({
                        "id": object_id,
                        "reasons": ["outside_context_packet"],
                    }));
                }
            }
            objects
        };
        for object in evaluated_objects {
            evaluated_object_count += 1;
            append_semantic_context_gate_blocker(&mut blockers, object);
        }
        return Ok(Some(json!({
            "status": if blockers.is_empty() { "passed" } else { "blocked" },
            "risk_level": risk_level,
            "boundary": "task_grant_context_packet",
            "context_packet_id": packet.id,
            "context_packet_version": packet.version,
            "referenced_object_count": referenced_object_ids.len(),
            "evaluated_object_count": evaluated_object_count,
            "blocked_object_count": blockers.len(),
            "required_freshness": "current",
            "blocked_trust_levels": ["unverified"],
            "blockers": blockers,
        })));
    }

    for object in state
        .list_semantic_objects()
        .await?
        .into_iter()
        .filter(|object| object.status == "active")
        .filter(|object| semantic_object_matches_agent_scope(object, agent))
    {
        evaluated_object_count += 1;
        append_semantic_context_gate_blocker(&mut blockers, &object);
    }

    Ok(Some(json!({
        "status": if blockers.is_empty() { "passed" } else { "blocked" },
        "risk_level": risk_level,
        "boundary": "agent_semantic_scope",
        "referenced_object_count": referenced_object_ids.len(),
        "evaluated_object_count": evaluated_object_count,
        "blocked_object_count": blockers.len(),
        "required_freshness": "current",
        "blocked_trust_levels": ["unverified"],
        "blockers": blockers,
    })))
}

trait SemanticContextGateObject {
    fn semantic_id(&self) -> Uuid;
    fn semantic_object_type(&self) -> &str;
    fn semantic_object_key(&self) -> &str;
    fn semantic_title(&self) -> &str;
    fn semantic_trust_level(&self) -> &str;
    fn semantic_freshness(&self) -> &str;
}

impl SemanticContextGateObject for SemanticObject {
    fn semantic_id(&self) -> Uuid {
        self.id
    }

    fn semantic_object_type(&self) -> &str {
        &self.object_type
    }

    fn semantic_object_key(&self) -> &str {
        &self.object_key
    }

    fn semantic_title(&self) -> &str {
        &self.title
    }

    fn semantic_trust_level(&self) -> &str {
        &self.trust_level
    }

    fn semantic_freshness(&self) -> &str {
        &self.freshness
    }
}

impl SemanticContextGateObject for ContextPacketSemanticObject {
    fn semantic_id(&self) -> Uuid {
        self.id
    }

    fn semantic_object_type(&self) -> &str {
        &self.object_type
    }

    fn semantic_object_key(&self) -> &str {
        &self.object_key
    }

    fn semantic_title(&self) -> &str {
        &self.title
    }

    fn semantic_trust_level(&self) -> &str {
        &self.trust_level
    }

    fn semantic_freshness(&self) -> &str {
        &self.freshness
    }
}

fn append_semantic_context_gate_blocker<T: SemanticContextGateObject + ?Sized>(
    blockers: &mut Vec<Value>,
    object: &T,
) {
    let stale = object.semantic_freshness() != "current";
    let untrusted = object.semantic_trust_level() == "unverified";
    if stale || untrusted {
        let mut reasons = Vec::new();
        if stale {
            reasons.push("freshness_not_current");
        }
        if untrusted {
            reasons.push("trust_unverified");
        }
        blockers.push(json!({
            "id": object.semantic_id(),
            "object_type": object.semantic_object_type(),
            "object_key": object.semantic_object_key(),
            "title": object.semantic_title(),
            "trust_level": object.semantic_trust_level(),
            "freshness": object.semantic_freshness(),
            "reasons": reasons,
        }));
    }
}

pub(crate) fn semantic_object_ref_ids_from_tool_args(args: &Value) -> BTreeSet<Uuid> {
    let mut ids = BTreeSet::new();
    collect_semantic_object_ref_ids(args, None, &mut ids);
    ids
}

pub(crate) fn collect_semantic_object_ref_ids(
    value: &Value,
    key_hint: Option<&str>,
    ids: &mut BTreeSet<Uuid>,
) {
    match value {
        Value::String(raw) if semantic_ref_key_hint(key_hint) => {
            if let Ok(id) = Uuid::parse_str(raw) {
                ids.insert(id);
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_semantic_object_ref_ids(value, key_hint, ids);
            }
        }
        Value::Object(object) => {
            for (key, value) in object {
                collect_semantic_object_ref_ids(value, Some(key), ids);
            }
        }
        _ => {}
    }
}

pub(crate) fn semantic_ref_key_hint(key_hint: Option<&str>) -> bool {
    let Some(key) = key_hint else {
        return false;
    };
    let normalized = key.trim().to_ascii_lowercase();
    normalized.contains("semantic_object")
        || normalized == "object_id"
        || normalized == "object_ids"
        || normalized == "semantic_ref"
        || normalized == "semantic_refs"
}

pub(crate) fn semantic_context_gate_block_reason(gate: &Value) -> String {
    let count = gate
        .get("blocked_object_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    format!(
        "high-risk tool blocked by semantic context gate: {count} stale or untrusted semantic object(s)"
    )
}

pub(crate) fn context_packet_replay_details(packet: &ContextPacket) -> Value {
    json!({
        "context_packet_id": packet.id,
        "version": packet.version,
        "agent_id": packet.agent_id,
        "agent_version_id": packet.agent_version_id,
        "runtime_profile_id": packet.runtime_profile.as_ref().map(|profile| profile.id),
        "semantic_scope_keys": semantic_scope_keys(&packet.semantic_scopes),
        "policy_reminder_count": packet.policy_reminders.len(),
        "freshness_warning_count": packet.freshness_warnings.len(),
        "source_ref_count": packet.source_refs.len(),
        "retrieved_object_count": packet.retrieved_objects.len(),
        "source_refs": packet.source_refs,
        "retrieved_objects": packet.retrieved_objects.iter().map(|object| {
            json!({
                "id": object.id,
                "object_type": object.object_type,
                "object_key": object.object_key,
                "title": object.title,
                "trust_level": object.trust_level,
                "freshness": object.freshness,
                "source_id": object.source_id,
                "source_uri": object.source_uri,
            })
        }).collect::<Vec<_>>(),
    })
}

pub(crate) fn context_source_ref(
    source_type: &str,
    source_id: Uuid,
    freshness: &str,
) -> ContextPacketSourceRef {
    ContextPacketSourceRef {
        source_type: source_type.to_string(),
        source_id: source_id.to_string(),
        freshness: freshness.to_string(),
    }
}

pub(crate) fn semantic_scope_keys(scopes: &Value) -> Vec<String> {
    scopes
        .as_object()
        .map(|object| object.keys().cloned().collect())
        .unwrap_or_default()
}

const REQUIRED_SEMANTIC_SCOPE_KEYS: [&str; 6] = [
    "project_scope",
    "repo_scope",
    "service_scope",
    "workflow_scope",
    "policy_scope",
    "memory_scope",
];

const REQUIRED_DOMAIN_SEMANTIC_SCOPE_KEYS: [&str; 3] =
    ["domain_scope", "workflow_scope", "share_policy"];

pub(crate) fn missing_semantic_scope_keys(scopes: &Value) -> Vec<String> {
    let required_keys: &[&str] =
        if scopes.get("domain_scope").is_some() || scopes.get("share_policy").is_some() {
            &REQUIRED_DOMAIN_SEMANTIC_SCOPE_KEYS
        } else {
            &REQUIRED_SEMANTIC_SCOPE_KEYS
        };
    required_keys
        .iter()
        .filter(|key| {
            scopes
                .get(*key)
                .and_then(Value::as_str)
                .map(|value| value.trim().is_empty())
                .unwrap_or(true)
        })
        .map(|key| (*key).to_string())
        .collect()
}

pub(crate) fn value_is_empty_object(value: &Value) -> bool {
    match value.as_object() {
        Some(object) => object.is_empty(),
        None => true,
    }
}

#[async_trait]
impl ToolExecutor for FileReadTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "file.read",
            risk: "low",
            description: "Read files inside the session workspace",
        }
    }

    async fn execute(
        &self,
        _state: &AppState,
        _input: &ExecuteTool,
        _tool_call: &ToolCall,
    ) -> Result<Value, AppError> {
        Ok(generic_file_read_summary())
    }
}

#[async_trait]
impl ToolExecutor for SqlSchemaTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "sql.get_schema",
            risk: "low",
            description: "Return generic demo SQL schema",
        }
    }

    async fn execute(
        &self,
        _state: &AppState,
        _input: &ExecuteTool,
        _tool_call: &ToolCall,
    ) -> Result<Value, AppError> {
        Ok(generic_schema())
    }
}

#[async_trait]
impl ToolExecutor for SqlQueryTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "sql.query",
            risk: "medium",
            description: "Execute read-only SQL against generic demo data",
        }
    }

    async fn execute(
        &self,
        state: &AppState,
        input: &ExecuteTool,
        _tool_call: &ToolCall,
    ) -> Result<Value, AppError> {
        let sql = input
            .args
            .get("sql")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let policy = state.policy_for_session(input.session_id).await;
        ensure_read_only_sql_with_policy(sql, &policy.sql_policy)?;
        match &state.store {
            StoreBackend::Postgres(pool) => {
                execute_postgres_sql_query(pool, sql, policy.sql_policy.max_rows).await
            }
            StoreBackend::Memory(_) => Ok(json!({"rows": generic_diagnostics(), "row_count": 4})),
        }
    }
}

#[async_trait]
impl ToolExecutor for ShellExecTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "shell.exec",
            risk: "high",
            description: "Run a shell command after policy approval",
        }
    }

    async fn execute(
        &self,
        state: &AppState,
        input: &ExecuteTool,
        _tool_call: &ToolCall,
    ) -> Result<Value, AppError> {
        let command = input
            .args
            .get("command")
            .or_else(|| input.args.get("cmd"))
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::bad_request("shell.exec requires command"))?;
        let workspace = state.workspace_root.join(input.session_id.to_string());
        tokio::fs::create_dir_all(&workspace)
            .await
            .map_err(|error| {
                AppError::bad_request(format!("failed to prepare session workspace: {error}"))
            })?;
        if !inline_shell_exec_allowed_for_tool() {
            return Err(AppError::bad_request(
                "inline shell.exec is disabled; use approved execution jobs or set MANDOFORGE_ALLOW_INLINE_SHELL_EXEC=1 for local development",
            ));
        }
        let runner = shell_runner();
        let output = run_shell_command(&runner, &workspace, command, Duration::from_secs(30))
            .await
            .map_err(|error| {
                AppError::bad_request(format!("failed to execute shell.exec: {error}"))
            })?
            .ok_or_else(|| AppError::bad_request("shell.exec timed out"))?;
        let stdout = truncate_output(&String::from_utf8_lossy(&output.stdout), 64 * 1024);
        let stderr = truncate_output(&String::from_utf8_lossy(&output.stderr), 64 * 1024);
        let result = json!({
            "command": command,
            "runner": runner,
            "workspace": workspace.display().to_string(),
            "exit_code": output.status.code(),
            "stdout": stdout.text,
            "stderr": stderr.text,
            "stdout_original_bytes": stdout.original_bytes,
            "stderr_original_bytes": stderr.original_bytes,
            "stdout_truncated": stdout.truncated,
            "stderr_truncated": stderr.truncated,
        });
        if output.status.success() {
            Ok(result)
        } else {
            Err(AppError::bad_request(format!(
                "shell.exec exited unsuccessfully: {:?}",
                result
            )))
        }
    }
}

#[async_trait]
impl ToolExecutor for ArtifactCreateTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "artifact.create",
            risk: "low",
            description: "Create a session artifact from normalized tool output",
        }
    }

    async fn execute(
        &self,
        state: &AppState,
        input: &ExecuteTool,
        tool_call: &ToolCall,
    ) -> Result<Value, AppError> {
        let name = input
            .args
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("artifact.json")
            .to_string();
        let artifact_type = input
            .args
            .get("artifact_type")
            .or_else(|| input.args.get("type"))
            .and_then(Value::as_str)
            .unwrap_or("json")
            .to_string();
        let path = input
            .args
            .get("path")
            .and_then(Value::as_str)
            .map(str::to_string);
        let content = input
            .args
            .get("content")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let artifact = Artifact {
            id: Uuid::new_v4(),
            session_id: input.session_id,
            artifact_type,
            name,
            path,
            content,
            created_at: Utc::now(),
        };
        let artifact = state.insert_artifact(artifact).await?;
        state
            .append_event(
                "system",
                Some(artifact.id),
                input.session_id,
                "artifact.created",
                json!({"artifact_id": artifact.id, "name": artifact.name, "path": artifact.path, "artifact_type": artifact.artifact_type, "tool_call_id": tool_call.id}),
            )
            .await?;
        state
            .append_audit_log(new_audit_log(
                Some(input.session_id),
                "tool",
                Some(tool_call.id),
                "artifact.created",
                "artifact",
                Some(artifact.id),
                json!({"name": artifact.name, "artifact_type": artifact.artifact_type}),
            ))
            .await?;
        Ok(
            json!({"artifact_id": artifact.id, "name": artifact.name, "path": artifact.path, "artifact_type": artifact.artifact_type}),
        )
    }
}

#[async_trait]
impl ToolExecutor for ApprovalRequestTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "approval.request",
            risk: "low",
            description: "Create an approval request linked to the current tool call",
        }
    }

    async fn execute(
        &self,
        state: &AppState,
        input: &ExecuteTool,
        tool_call: &ToolCall,
    ) -> Result<Value, AppError> {
        let action = input
            .args
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or("manual.approval")
            .to_string();
        let risk_level = input
            .args
            .get("risk_level")
            .or_else(|| input.args.get("risk"))
            .and_then(Value::as_str)
            .unwrap_or("medium")
            .to_string();
        let reason = input
            .args
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or("Tool requested human approval.")
            .to_string();
        let mut evidence = input
            .args
            .get("evidence")
            .cloned()
            .unwrap_or_else(|| json!({}));
        if let Some(approver_subject) = input
            .args
            .get("approver_subject")
            .or_else(|| input.args.get("delegated_approver"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
        {
            if let Value::Object(map) = &mut evidence {
                map.insert("approver_subject".to_string(), json!(approver_subject));
            } else {
                evidence = json!({
                    "details": evidence,
                    "approver_subject": approver_subject,
                });
            }
        }
        if let Some(group_id) = input
            .args
            .get("approver_group_id")
            .or_else(|| input.args.get("delegated_approver_group_id"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(Uuid::parse_str)
            .transpose()
            .map_err(|_| AppError::bad_request("approver_group_id must be a UUID"))?
        {
            let group = state.get_approval_group(group_id).await?;
            if group.status != "active" {
                return Err(AppError::bad_request("approval group is not active"));
            }
            merge_approval_evidence(
                &mut evidence,
                json!({
                    "approver_group_id": group.id,
                    "approver_group_name": group.name
                }),
            );
        }
        let created_at = Utc::now();
        let approval = Approval {
            id: Uuid::new_v4(),
            session_id: input.session_id,
            tool_call_id: Some(tool_call.id),
            action,
            risk_level,
            reason,
            evidence,
            decision_payload: json!({}),
            status: "pending".to_string(),
            expires_at: approval_expires_at(created_at, input.args.get("expires_in_seconds")),
            created_at,
            decided_at: None,
        };
        let approval = state.insert_approval(approval).await?;
        state
            .append_event(
                "system",
                Some(approval.id),
                input.session_id,
                "approval.requested",
                json!({"approval_id": approval.id, "action": approval.action, "risk_level": approval.risk_level, "reason": approval.reason, "evidence": approval.evidence, "expires_at": approval.expires_at, "tool_call_id": tool_call.id}),
            )
            .await?;
        state
            .append_audit_log(new_audit_log(
                Some(input.session_id),
                "tool",
                Some(tool_call.id),
                "approval.requested",
                "approval",
                Some(approval.id),
                json!({"action": approval.action, "risk_level": approval.risk_level, "expires_at": approval.expires_at}),
            ))
            .await?;
        set_managed_session_status(
            state,
            input.session_id,
            SessionStatus::RequiresAction,
            "tool approval requested",
        )
        .await?;
        Ok(json!({"status": "approval_requested", "approval_id": approval.id}))
    }
}

#[async_trait]
impl ToolExecutor for McpCallTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "mcp.call",
            risk: "high",
            description: "Call an allowlisted MCP Gateway server tool through the audited Tool Router",
        }
    }

    async fn execute(
        &self,
        state: &AppState,
        _input: &ExecuteTool,
        _tool_call: &ToolCall,
    ) -> Result<Value, AppError> {
        if let Some(task_grant_id) = _input.task_grant_id {
            let grant = state.get_task_grant(task_grant_id).await?;
            if task_grant_requires_approval_commit_token(&grant, "mcp.call") {
                return Err(AppError::forbidden(
                    "commit_write MCP calls require ApprovalCommitToken approved execution",
                ));
            }
        }
        let config = state
            .mcp_gateway_config
            .as_ref()
            .ok_or_else(|| AppError::bad_request("MCP gateway is not configured"))?;
        let request: McpCallRequest = serde_json::from_value(_input.args.clone())?;
        let scoped_server = state
            .mcp_server_for_session_tool(_input.session_id, &request.server, &request.tool)
            .await?;
        let secret_refs_resolved = if let Some(server) = scoped_server.as_ref() {
            resolve_mcp_runtime_secret_refs(server).await?
        } else {
            0
        };
        let response = state.mcp_gateway_client.call(config, request).await?;
        Ok(json!({
            "status": "called",
            "secret_refs_resolved_count": secret_refs_resolved,
            "result": response.result,
        }))
    }
}

#[async_trait]
impl ToolExecutor for SemanticObjectFetchTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "semantic_object.fetch",
            risk: "low",
            description: "Fetch a context-packet-visible semantic object",
        }
    }

    async fn execute(
        &self,
        state: &AppState,
        input: &ExecuteTool,
        _tool_call: &ToolCall,
    ) -> Result<Value, AppError> {
        let packet = context_packet_for_tool_invocation(state, input).await?;
        let object_id = uuid_tool_arg(
            &input.args,
            &["object_id", "semantic_object_id", "id"],
            "semantic_object.fetch requires object_id",
        )?;
        let include_content = input
            .args
            .get("include_content")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let response =
            fetch_semantic_object_for_context(state, &packet, object_id, include_content).await?;
        serde_json::to_value(response).map_err(|error| {
            AppError::bad_request(format!(
                "failed to serialize semantic object fetch: {error}"
            ))
        })
    }
}

#[async_trait]
impl ToolExecutor for SemanticObjectSearchTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "semantic_object.search",
            risk: "low",
            description: "Search semantic objects inside the current context packet or scoped TaskGrant memory",
        }
    }

    async fn execute(
        &self,
        state: &AppState,
        input: &ExecuteTool,
        _tool_call: &ToolCall,
    ) -> Result<Value, AppError> {
        let (packet, grant) = context_packet_and_grant_for_tool_invocation(state, input).await?;
        let request = SearchSemanticObjectsRequest {
            context_packet_id: packet.id,
            query: input
                .args
                .get("query")
                .and_then(Value::as_str)
                .map(str::to_string),
            object_type: input
                .args
                .get("object_type")
                .and_then(Value::as_str)
                .map(str::to_string),
            max_results: input
                .args
                .get("max_results")
                .and_then(Value::as_u64)
                .map(|value| value as usize),
        };
        let response =
            search_semantic_objects_for_context(state, &packet, grant.as_ref(), request).await?;
        serde_json::to_value(response).map_err(|error| {
            AppError::bad_request(format!(
                "failed to serialize semantic object search: {error}"
            ))
        })
    }
}

#[async_trait]
impl ToolExecutor for SemanticLinkExpandTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "semantic_link.expand",
            risk: "low",
            description: "Expand semantic links inside the current context packet",
        }
    }

    async fn execute(
        &self,
        state: &AppState,
        input: &ExecuteTool,
        _tool_call: &ToolCall,
    ) -> Result<Value, AppError> {
        let packet = context_packet_for_tool_invocation(state, input).await?;
        let object_id = uuid_tool_arg(
            &input.args,
            &["object_id", "semantic_object_id", "id"],
            "semantic_link.expand requires object_id",
        )?;
        let request = ExpandSemanticLinksRequest {
            context_packet_id: packet.id,
            object_id,
            relation_type: input
                .args
                .get("relation_type")
                .and_then(Value::as_str)
                .map(str::to_string),
            max_links: input
                .args
                .get("max_links")
                .and_then(Value::as_u64)
                .map(|value| value as usize),
        };
        let response = expand_semantic_links_for_context(state, &packet, request).await?;
        serde_json::to_value(response).map_err(|error| {
            AppError::bad_request(format!(
                "failed to serialize semantic link expansion: {error}"
            ))
        })
    }
}

#[async_trait]
impl ToolExecutor for OntologyTypeLookupTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "ontology_type.lookup",
            risk: "low",
            description: "Look up ontology object and relation type definitions",
        }
    }

    async fn execute(
        &self,
        state: &AppState,
        input: &ExecuteTool,
        _tool_call: &ToolCall,
    ) -> Result<Value, AppError> {
        let packet = context_packet_for_tool_invocation(state, input).await?;
        let requested_name = input
            .args
            .get("type_name")
            .or_else(|| input.args.get("name"))
            .and_then(Value::as_str)
            .map(normalized_ontology_token)
            .filter(|value| !value.is_empty());
        let kind = input
            .args
            .get("kind")
            .and_then(Value::as_str)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .unwrap_or("all");
        let registry = ontology_registry();
        let include_object_types = matches!(kind, "all" | "object_type" | "object_types");
        let include_relation_types = matches!(kind, "all" | "relation_type" | "relation_types");
        if !include_object_types && !include_relation_types {
            return Err(AppError::bad_request(
                "ontology_type.lookup kind must be all, object_type, or relation_type",
            ));
        }
        let object_types = if include_object_types {
            registry
                .object_types
                .into_iter()
                .filter(|object_type| {
                    requested_name
                        .as_ref()
                        .is_none_or(|name| normalized_ontology_token(&object_type.name) == *name)
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let relation_types = if include_relation_types {
            registry
                .relation_types
                .into_iter()
                .filter(|relation_type| {
                    requested_name
                        .as_ref()
                        .is_none_or(|name| normalized_ontology_token(&relation_type.name) == *name)
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let result = json!({
            "status": "ok",
            "context_packet_id": packet.id,
            "context_packet_version": packet.version,
            "registry_version": registry.version,
            "registry_scope": {
                "type": "core",
                "domain": packet.semantic_scopes.get("domain_scope").and_then(Value::as_str).unwrap_or("global"),
                "workflow_scope": packet.semantic_scopes.get("workflow_scope").and_then(Value::as_str),
                "memory_scope": packet.semantic_scopes.get("memory_scope").and_then(Value::as_str),
                "release_model": "core registry scoped by ContextPacket; active domain ontology release metadata is pinned when available"
            },
            "ontology_scope": render_ontology_scope(&packet.semantic_scopes),
            "requested_name": requested_name,
            "kind": kind,
            "object_types": object_types,
            "relation_types": relation_types,
        });
        record_ontology_type_lookup(state, &packet, &result).await?;
        Ok(result)
    }
}

#[async_trait]
impl ToolExecutor for OntologyActionExecuteTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "ontology.action.execute",
            risk: "medium",
            description: "Validate a pinned ontology action contract and create an auditable proposal",
        }
    }

    async fn execute(
        &self,
        state: &AppState,
        input: &ExecuteTool,
        tool_call: &ToolCall,
    ) -> Result<Value, AppError> {
        let (packet, grant) = context_packet_and_grant_for_tool_invocation(state, input).await?;
        let grant = grant.ok_or_else(|| {
            AppError::forbidden("ontology action execution requires a workflow TaskGrant")
        })?;
        let release_snapshot = packet
            .replay_summary
            .get("ontology_release")
            .filter(|release| release.is_object())
            .ok_or_else(|| AppError::forbidden("context packet has no pinned ontology release"))?;
        let release_id = release_snapshot
            .get("id")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .ok_or_else(|| {
                AppError::forbidden("pinned ontology release id is missing or invalid")
            })?;
        let grant_release_id = grant
            .approval_policy
            .get("ontology_release_snapshot")
            .and_then(|release| release.get("id"))
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok());
        if grant_release_id != Some(release_id) {
            return Err(AppError::forbidden(
                "context packet ontology release is outside the TaskGrant authority",
            ));
        }
        let release = state.get_ontology_release(release_id).await?;
        if matches!(release.status.as_str(), "rolled_back" | "archived") {
            return Err(AppError::forbidden(
                "pinned ontology release has been revoked for new action proposals",
            ));
        }
        if release_snapshot.get("version").and_then(Value::as_str) != Some(release.version.as_str())
            || release_snapshot.get("domain_scope").and_then(Value::as_str)
                != Some(release.domain_scope.as_str())
        {
            return Err(AppError::forbidden(
                "pinned ontology release metadata does not match the release registry",
            ));
        }
        let action_name = input
            .args
            .get("action")
            .or_else(|| input.args.get("action_name"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| AppError::bad_request("ontology action requires action"))?;
        let (spec, contract_digest) = ontology_action_tool_spec_for_release(&release, action_name)?;
        let parameters = input
            .args
            .get("parameters")
            .cloned()
            .unwrap_or_else(empty_json_object);
        validate_ontology_action_parameters(&spec.input_schema, &parameters)?;
        if !spec.read_only && spec.execution_mode != "proposal_only" {
            return Err(AppError::forbidden(
                "ontology action side effects are disabled unless the published contract is proposal_only",
            ));
        }

        let artifact = state
            .insert_artifact(Artifact {
                id: Uuid::new_v4(),
                session_id: input.session_id,
                artifact_type: "ontology_action_proposal".to_string(),
                name: format!("{}-proposal.json", workflow_slug(action_name)),
                path: None,
                content: json!({
                    "status": "draft",
                    "ontology_release_id": release.id,
                    "ontology_version": release.version,
                    "domain_scope": release.domain_scope,
                    "action": spec.name,
                    "action_contract_id": spec.id,
                    "contract_digest": contract_digest,
                    "target_object": spec.target_object,
                    "parameters": parameters,
                    "effects": spec.effects,
                    "executor": spec.executor,
                    "approval_required": spec.approval_required,
                    "transaction_profile": spec.transaction_profile,
                    "execution_mode": spec.execution_mode,
                    "commit_status": "blocked_pending_explicit_production_policy",
                    "context_packet_id": packet.id,
                    "task_grant_id": grant.id,
                }),
                created_at: Utc::now(),
            })
            .await?;
        state
            .append_event(
                "system",
                Some(artifact.id),
                input.session_id,
                "artifact.created",
                json!({
                    "artifact_id": artifact.id,
                    "name": artifact.name,
                    "artifact_type": artifact.artifact_type,
                    "tool_call_id": tool_call.id,
                }),
            )
            .await?;
        state
            .append_audit_log(new_audit_log(
                Some(input.session_id),
                "tool",
                Some(tool_call.id),
                "artifact.created",
                "artifact",
                Some(artifact.id),
                json!({
                    "name": artifact.name,
                    "artifact_type": artifact.artifact_type,
                    "source": "ontology_action_proposal",
                }),
            ))
            .await?;
        let details = json!({
            "artifact_id": artifact.id,
            "ontology_release_id": release.id,
            "action": spec.name,
            "action_contract_id": spec.id,
            "contract_digest": contract_digest,
            "execution_mode": spec.execution_mode,
            "declared_audit_event": spec.audit_event,
            "tool_call_id": tool_call.id,
            "task_grant_id": grant.id,
            "context_packet_id": packet.id,
        });
        state
            .append_event(
                "system",
                Some(artifact.id),
                input.session_id,
                "ontology_action.proposal_created",
                details.clone(),
            )
            .await?;
        state
            .append_audit_log(new_audit_log(
                Some(input.session_id),
                "tool",
                Some(tool_call.id),
                "ontology_action.proposal_created",
                "artifact",
                Some(artifact.id),
                details,
            ))
            .await?;
        Ok(json!({
            "status": "proposal_created",
            "artifact_id": artifact.id,
            "ontology_release_id": release.id,
            "action": spec.name,
            "execution_mode": spec.execution_mode,
            "approval_required": spec.approval_required,
            "commit_status": "blocked_pending_explicit_production_policy",
        }))
    }
}

pub(crate) fn validate_ontology_action_parameters(
    input_schema: &Value,
    parameters: &Value,
) -> Result<(), AppError> {
    let parameters = parameters
        .as_object()
        .ok_or_else(|| AppError::bad_request("ontology action parameters must be a JSON object"))?;
    if input_schema.get("type").and_then(Value::as_str) == Some("object") {
        let properties = input_schema
            .get("properties")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                AppError::bad_request("ontology action object input_schema must declare properties")
            })?;
        for name in parameters.keys() {
            if !properties.contains_key(name) {
                return Err(AppError::bad_request(format!(
                    "ontology action parameter {name} is not declared by the published contract"
                )));
            }
        }
        return validate_handoff_payload_schema(
            &Value::Object(parameters.clone()),
            Some(input_schema),
        );
    }
    let declarations = input_schema.as_object().ok_or_else(|| {
        AppError::bad_request("ontology action input_schema must be a JSON object")
    })?;
    for name in parameters.keys() {
        if !declarations.contains_key(name) {
            return Err(AppError::bad_request(format!(
                "ontology action parameter {name} is not declared by the published contract"
            )));
        }
    }
    for (name, declaration) in declarations {
        let (expected_type, required) = match declaration {
            Value::String(expected_type) => (Some(expected_type.as_str()), true),
            Value::Object(declaration) => (
                declaration.get("type").and_then(Value::as_str),
                declaration
                    .get("required")
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
            ),
            _ => (None, true),
        };
        let Some(value) = parameters.get(name) else {
            if required {
                return Err(AppError::bad_request(format!(
                    "ontology action missing required parameter {name}"
                )));
            }
            continue;
        };
        let Some(expected_type) = expected_type else {
            return Err(AppError::bad_request(format!(
                "ontology action parameter {name} has no supported type declaration"
            )));
        };
        let matches_type = match expected_type {
            "string" => value.is_string(),
            "decimal" | "number" => value.is_number(),
            "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
            "boolean" => value.is_boolean(),
            "object" => value.is_object(),
            "array" => value.is_array(),
            _ => {
                return Err(AppError::bad_request(format!(
                    "ontology action parameter {name} has unsupported type {expected_type}"
                )));
            }
        };
        if !matches_type {
            return Err(AppError::bad_request(format!(
                "ontology action parameter {name} must be {expected_type}"
            )));
        }
    }
    Ok(())
}

pub(crate) async fn context_packet_for_tool_invocation(
    state: &AppState,
    input: &ExecuteTool,
) -> Result<ContextPacket, AppError> {
    context_packet_and_grant_for_tool_invocation(state, input)
        .await
        .map(|(packet, _)| packet)
}

pub(crate) async fn context_packet_and_grant_for_tool_invocation(
    state: &AppState,
    input: &ExecuteTool,
) -> Result<(ContextPacket, Option<TaskGrant>), AppError> {
    let context_packet_id = uuid_tool_arg(
        &input.args,
        &["context_packet_id"],
        "ontology tools require context_packet_id",
    )?;
    let packet = state.get_context_packet(context_packet_id).await?;
    if packet.session_id != input.session_id {
        return Err(AppError::forbidden(
            "context_packet_id does not belong to this session",
        ));
    }
    if let Some(task_grant_id) = input.task_grant_id {
        let grant = state.get_task_grant(task_grant_id).await?;
        match grant.context_packet_id {
            Some(bound_packet_id) if bound_packet_id == packet.id => {}
            Some(_) => {
                return Err(AppError::forbidden(
                    "context_packet_id is outside the active TaskGrant boundary",
                ));
            }
            None => {
                return Err(AppError::forbidden(
                    "ontology tools require a TaskGrant-bound context_packet_id",
                ));
            }
        }
        return Ok((packet, Some(grant)));
    }
    Ok((packet, None))
}

pub(crate) fn uuid_tool_arg(
    args: &Value,
    keys: &[&str],
    missing_message: &str,
) -> Result<Uuid, AppError> {
    for key in keys {
        if let Some(value) = args.get(*key).and_then(Value::as_str) {
            return Uuid::parse_str(value)
                .map_err(|_| AppError::bad_request(format!("{key} must be a UUID")));
        }
    }
    Err(AppError::bad_request(missing_message))
}

pub(crate) async fn record_ontology_type_lookup(
    state: &AppState,
    packet: &ContextPacket,
    result: &Value,
) -> Result<(), AppError> {
    let details = json!({
        "context_packet_id": packet.id,
        "context_packet_version": packet.version,
        "registry_version": result.get("registry_version").cloned().unwrap_or(Value::Null),
        "registry_scope": result.get("registry_scope").cloned().unwrap_or(Value::Null),
        "requested_name": result.get("requested_name").cloned().unwrap_or(Value::Null),
        "kind": result.get("kind").cloned().unwrap_or(Value::Null),
        "object_type_count": result
            .get("object_types")
            .and_then(Value::as_array)
            .map(|values| values.len())
            .unwrap_or(0),
        "relation_type_count": result
            .get("relation_types")
            .and_then(Value::as_array)
            .map(|values| values.len())
            .unwrap_or(0),
    });
    state
        .append_event(
            "tool",
            Some(packet.id),
            packet.session_id,
            "ontology_type.looked_up",
            details.clone(),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(packet.session_id),
            "tool",
            Some(packet.id),
            "ontology_type.looked_up",
            "context_packet",
            Some(packet.id),
            details,
        ))
        .await?;
    Ok(())
}

pub(crate) fn tool_registry() -> HashMap<&'static str, Box<dyn ToolExecutor>> {
    let tools: Vec<Box<dyn ToolExecutor>> = vec![
        Box::new(ArtifactCreateTool),
        Box::new(ApprovalRequestTool),
        Box::new(FileReadTool),
        Box::new(McpCallTool),
        Box::new(OntologyActionExecuteTool),
        Box::new(OntologyTypeLookupTool),
        Box::new(SemanticLinkExpandTool),
        Box::new(SemanticObjectFetchTool),
        Box::new(SemanticObjectSearchTool),
        Box::new(ShellExecTool),
        Box::new(SqlSchemaTool),
        Box::new(SqlQueryTool),
    ];
    tools
        .into_iter()
        .map(|tool| (tool.descriptor().name, tool))
        .collect()
}

pub(crate) fn inline_shell_exec_allowed_for_tool() -> bool {
    std::env::var("MANDOFORGE_ALLOW_INLINE_SHELL_EXEC")
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

pub(crate) fn tool_descriptors() -> Vec<ToolDescriptor> {
    let mut descriptors: Vec<_> = tool_registry()
        .into_values()
        .map(|tool| tool.descriptor())
        .collect();
    descriptors.extend([
        ToolDescriptor {
            name: "file.write",
            risk: "medium",
            description: "Write files inside the session workspace after approval",
        },
        ToolDescriptor {
            name: "codex.exec",
            risk: "high",
            description: "Run Codex CLI in a session workspace",
        },
        ToolDescriptor {
            name: "agent_cli.exec",
            risk: "high",
            description: "Run an allowlisted coding-agent CLI profile in a session workspace",
        },
        ToolDescriptor {
            name: "native.connector.call",
            risk: "high",
            description: "Commit a native connector side effect through scoped approval-token binding",
        },
    ]);
    descriptors.sort_by_key(|descriptor| descriptor.name);
    descriptors
}

pub(crate) async fn authorize_tool_execution(
    state: &AppState,
    headers: &HeaderMap,
    tool_name: &str,
) -> Result<(), AppError> {
    let principal = principal_from_request(state, headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::ToolsExecute,
        resource_type: format!("tool:{tool_name}"),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await
}

pub(crate) async fn principal_from_request(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Principal, AppError> {
    let tenant_id = resolve_request_tenant_id(state, headers)?;
    if worker_token_authenticated(headers) {
        return Ok(Principal {
            tenant_id,
            subject_id: configured_worker_subject(),
            roles: vec![Role::Worker],
        });
    }

    let dev_token_authenticated = dev_admin_token_authenticated(headers);
    if dev_token_authenticated {
        return Ok(Principal {
            tenant_id,
            subject_id: header_value(headers, "x-mandoforge-subject")
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("dev-admin")
                .to_string(),
            roles: vec![Role::Admin],
        });
    }

    let insecure_dev_auth = insecure_dev_auth_enabled();
    let trusted_subject_header = trusted_subject_header_enabled();
    let Some(explicit_subject) = header_value(headers, "x-mandoforge-subject") else {
        if insecure_dev_auth {
            let roles = if let Some(value) = header_value(headers, "x-mandoforge-roles") {
                parse_roles_header(value)?
            } else {
                vec![Role::Operator]
            };
            return Ok(Principal {
                tenant_id,
                subject_id: "demo-operator".to_string(),
                roles,
            });
        }
        return Err(AppError::unauthorized(
            "x-mandoforge-subject header is required",
        ));
    };
    if !insecure_dev_auth && !trusted_subject_header {
        return Err(AppError::unauthorized(
            "x-mandoforge-subject is only accepted from trusted identity ingress",
        ));
    }
    let subject_id = explicit_subject.trim().to_string();
    if subject_id.is_empty() {
        return Err(AppError::bad_request(
            "x-mandoforge-subject header cannot be empty",
        ));
    }
    let roles = if insecure_dev_auth {
        if let Some(value) = header_value(headers, "x-mandoforge-roles") {
            parse_roles_header(value)?
        } else {
            state.membership_roles_for_subject(&subject_id).await?
        }
    } else if header_value(headers, "x-mandoforge-roles").is_some() {
        return Err(AppError::forbidden(
            "x-mandoforge-roles is only accepted in explicit insecure dev auth mode",
        ));
    } else {
        state.membership_roles_for_subject(&subject_id).await?
    };
    if roles.is_empty() {
        return Err(AppError::forbidden("principal has no roles"));
    }

    Ok(Principal {
        tenant_id,
        subject_id,
        roles,
    })
}

pub(crate) fn dev_admin_token_authenticated(headers: &HeaderMap) -> bool {
    let Some(expected) = std::env::var("MANDOFORGE_DEV_ADMIN_TOKEN")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        return false;
    };
    let Some(header) = header_value(headers, "authorization") else {
        return false;
    };
    let Some(token) = header.trim().strip_prefix("Bearer ") else {
        return false;
    };
    token.trim() == expected
}

pub(crate) fn worker_token_authenticated(headers: &HeaderMap) -> bool {
    let Some(expected) = std::env::var("MANDOFORGE_WORKER_TOKEN")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        return false;
    };
    let Some(header) = header_value(headers, "authorization") else {
        return false;
    };
    let Some(token) = header.trim().strip_prefix("Bearer ") else {
        return false;
    };
    token.trim() == expected
}

pub(crate) fn insecure_dev_auth_enabled() -> bool {
    std::env::var("MANDOFORGE_INSECURE_DEV_AUTH")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

pub(crate) fn trusted_tenant_header_enabled() -> bool {
    std::env::var("MANDOFORGE_TRUST_X_MANDOFORGE_TENANT_ID")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

pub(crate) fn trusted_subject_header_enabled() -> bool {
    std::env::var("MANDOFORGE_TRUST_X_MANDOFORGE_SUBJECT")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

#[allow(dead_code)]
pub(crate) async fn legacy_principal_from_request(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Principal, AppError> {
    let tenant_id = resolve_request_tenant_id(state, headers)?;
    let explicit_subject = header_value(headers, "x-mandoforge-subject");
    let subject_id = explicit_subject.unwrap_or("demo-operator").to_string();
    let roles = if let Some(value) = header_value(headers, "x-mandoforge-roles") {
        parse_roles_header(value)?
    } else if explicit_subject.is_some() {
        state.membership_roles_for_subject(&subject_id).await?
    } else {
        vec![Role::Operator]
    };
    if roles.is_empty() {
        return Err(AppError::forbidden("principal has no roles"));
    }

    Ok(Principal {
        tenant_id,
        subject_id,
        roles,
    })
}

pub(crate) fn resolve_request_tenant_id(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Uuid, AppError> {
    let requested_tenant_id = header_value(headers, "x-mandoforge-tenant-id")
        .map(|value| {
            Uuid::parse_str(value.trim())
                .map_err(|_| AppError::bad_request("x-mandoforge-tenant-id must be a valid UUID"))
        })
        .transpose()?;

    match (state.tenant_runtime_mode, requested_tenant_id) {
        (TenantRuntimeMode::TenantRouted, Some(tenant_id))
            if insecure_dev_auth_enabled()
                || trusted_tenant_header_enabled()
                || (worker_token_authenticated(headers)
                    && state.process_role == ProcessRole::Worker
                    && tenant_id == state.configured_tenant_id()) =>
        {
            Ok(tenant_id)
        }
        (TenantRuntimeMode::TenantRouted, Some(_)) => Err(AppError::forbidden(
            "x-mandoforge-tenant-id is only accepted from trusted tenant-routing ingress",
        )),
        (TenantRuntimeMode::TenantRouted, None) => Err(AppError::forbidden(
            "tenant-routed runtime requires x-mandoforge-tenant-id from trusted ingress",
        )),
        (TenantRuntimeMode::SingleRuntimeTenant, Some(tenant_id)) => {
            if tenant_id != state.configured_tenant_id() {
                return Err(AppError::forbidden(
                    "x-mandoforge-tenant-id does not match this runtime tenant",
                ));
            }
            Ok(tenant_id)
        }
        (TenantRuntimeMode::SingleRuntimeTenant, None) => Ok(state.configured_tenant_id()),
    }
}

fn configured_worker_subject() -> String {
    std::env::var("MANDOFORGE_WORKER_SUBJECT")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            std::env::var("WORKER_SUBJECT")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_else(|| "mandoforge-worker".to_string())
}

pub(crate) fn subject_from_headers(headers: &HeaderMap) -> Result<String, AppError> {
    let Some(subject) = header_value(headers, "x-mandoforge-subject") else {
        return Err(AppError::bad_request(
            "x-mandoforge-subject header is required",
        ));
    };
    let subject = subject.trim();
    if subject.is_empty() {
        return Err(AppError::bad_request(
            "x-mandoforge-subject header cannot be empty",
        ));
    }
    Ok(subject.to_string())
}

pub(crate) fn header_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

pub(crate) fn parse_roles_header(value: &str) -> Result<Vec<Role>, AppError> {
    value
        .split(',')
        .map(str::trim)
        .filter(|role| !role.is_empty())
        .map(|role| match role {
            "admin" => Ok(Role::Admin),
            "operator" => Ok(Role::Operator),
            "worker" => Ok(Role::Worker),
            "approver" => Ok(Role::Approver),
            "viewer" => Ok(Role::Viewer),
            other => Err(AppError::bad_request(format!(
                "unsupported x-mandoforge-roles value: {other}"
            ))),
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolInvocationOrigin {
    ManualRoute,
    SessionLoop,
}

pub(crate) fn task_grant_requires_approval_commit_token(
    grant: &TaskGrant,
    tool_name: &str,
) -> bool {
    grant.connector_scope.get("mode").and_then(Value::as_str) == Some("commit_write")
        && (tool_name == "mcp.call"
            || native_connector_tool_name(tool_name)
            || json_string_array_contains(grant.tool_scope.get("external_write"), tool_name))
}

pub(crate) fn approval_commit_binding_for_invocation(
    tool_name: &str,
    args: &Value,
    grant: Option<&TaskGrant>,
) -> Result<Option<ApprovalCommitBinding>, AppError> {
    let Some(grant) = grant else {
        return Ok(None);
    };
    if !task_grant_requires_approval_commit_token(grant, tool_name) {
        return Ok(None);
    }
    Ok(Some(approval_commit_binding_for_args(tool_name, args)?))
}

pub(crate) fn approval_commit_binding_for_args(
    tool_name: &str,
    args: &Value,
) -> Result<ApprovalCommitBinding, AppError> {
    Ok(ApprovalCommitBinding {
        normalized_args_hash: normalized_json_sha256(args),
        target_binding: connector_target_binding(tool_name, args)?,
    })
}

pub(crate) fn normalized_json_sha256(value: &Value) -> String {
    let text = serde_json::to_string(value).unwrap_or_else(|_| "null".to_string());
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let digest = hasher.finalize();
    let hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

pub(crate) fn mcp_call_target_binding(args: &Value) -> Result<Value, AppError> {
    let request: McpCallRequest = serde_json::from_value(args.clone())?;
    let mut binding = serde_json::Map::new();
    binding.insert("server".to_string(), json!(request.server));
    binding.insert("tool".to_string(), json!(request.tool));
    if let Some(object) = request.args.as_object() {
        binding.insert(
            "payload_digest".to_string(),
            json!(normalized_json_sha256(&request.args)),
        );
        for key in [
            "side_effect_class",
            "account",
            "channel",
            "recipient",
            "resource_id",
            "campaign_id",
            "ad_account_id",
            "amount",
            "amount_usd",
            "currency",
            "spend_limit",
            "spend_limit_usd",
            "context_packet_id",
        ] {
            if let Some(value) = object.get(key) {
                binding.insert(key.to_string(), value.clone());
            }
        }
        if let Some(content_digest) = object.get("content_digest") {
            binding.insert("content_digest".to_string(), content_digest.clone());
        } else if let Some(content) = [
            "content", "body", "text", "message", "post", "creative", "caption",
        ]
        .iter()
        .find_map(|key| object.get(*key))
        {
            binding.insert(
                "content_digest".to_string(),
                json!(normalized_json_sha256(content)),
            );
        }
    }
    Ok(Value::Object(binding))
}

pub(crate) fn connector_target_binding(tool_name: &str, args: &Value) -> Result<Value, AppError> {
    if tool_name == "mcp.call" {
        return mcp_call_target_binding(args);
    }
    native_connector_target_binding(tool_name, args)
}

pub(crate) fn native_connector_tool_name(tool_name: &str) -> bool {
    tool_name == "native.connector.call" || tool_name.starts_with("native.")
}

pub(crate) fn native_connector_target_binding(
    tool_name: &str,
    args: &Value,
) -> Result<Value, AppError> {
    let target = native_connector_call_target(args)?;
    let object = args
        .as_object()
        .ok_or_else(|| AppError::bad_request("native connector args must be a JSON object"))?;
    let payload = object.get("payload").unwrap_or(args);
    let mut binding = serde_json::Map::new();
    binding.insert("tool".to_string(), json!(tool_name));
    binding.insert("connector_id".to_string(), json!(target.connector_id));
    binding.insert("operation".to_string(), json!(target.operation));
    binding.insert(
        "side_effect_class".to_string(),
        json!(target.side_effect_class),
    );
    binding.insert(
        "payload_digest".to_string(),
        json!(normalized_json_sha256(payload)),
    );
    for key in [
        "account",
        "resource_id",
        "campaign_id",
        "ad_account_id",
        "amount",
        "amount_usd",
        "currency",
        "spend_limit",
        "spend_limit_usd",
        "context_packet_id",
    ] {
        if let Some(value) = object.get(key) {
            binding.insert(key.to_string(), value.clone());
        }
    }
    if let Some(content_digest) = object.get("content_digest") {
        binding.insert("content_digest".to_string(), content_digest.clone());
    } else if let Some(content) = payload.as_object().and_then(|payload| {
        [
            "content", "body", "text", "message", "post", "creative", "caption",
        ]
        .iter()
        .find_map(|key| payload.get(*key))
    }) {
        binding.insert(
            "content_digest".to_string(),
            json!(normalized_json_sha256(content)),
        );
    }
    Ok(Value::Object(binding))
}

pub(crate) async fn refresh_tool_call_commit_binding_if_required(
    state: &AppState,
    tool_call: ToolCall,
) -> Result<ToolCall, AppError> {
    let Some(task_grant_id) = tool_call.task_grant_id else {
        return Ok(tool_call);
    };
    let grant = state.get_task_grant(task_grant_id).await?;
    if !task_grant_requires_approval_commit_token(&grant, &tool_call.tool_name) {
        return Ok(tool_call);
    }
    let binding = approval_commit_binding_for_args(&tool_call.tool_name, &tool_call.args)?;
    state
        .update_tool_call_commit_binding(
            tool_call.id,
            Some(binding.normalized_args_hash),
            binding.target_binding,
        )
        .await
}

pub(crate) async fn execute_tool_invocation(
    state: &AppState,
    name: &str,
    input: ExecuteTool,
    origin: ToolInvocationOrigin,
) -> Result<Value, AppError> {
    let task_grant = enforce_task_grant_for_tool_invocation(state, name, &input).await?;
    let agent_version = state.agent_version_for_session(input.session_id).await?;
    let policy = state.policy_for_session(input.session_id).await;
    let mut policy_decision =
        policy.evaluate_tool_for_agent_version_with_args(name, &input.args, &agent_version);
    let commit_binding =
        approval_commit_binding_for_invocation(name, &input.args, task_grant.as_ref())?;
    if commit_binding.is_some() && policy_decision.decision != "denied" {
        policy_decision.decision = "requires_approval";
        policy_decision.risk_level = "critical".to_string();
        policy_decision.reason =
            "commit_write connector calls require ApprovalCommitToken exact digest binding"
                .to_string();
    }
    let session = state.get_session(input.session_id).await?;
    let agent = state.get_agent(session.agent_id).await?;
    let semantic_context_gate = evaluate_semantic_context_gate(
        state,
        &agent,
        task_grant.as_ref(),
        &input,
        &policy_decision.risk_level,
        policy_decision.decision,
    )
    .await?;
    let semantic_context_gate_blocked = semantic_context_gate
        .as_ref()
        .and_then(|gate| gate.get("status"))
        .and_then(Value::as_str)
        == Some("blocked");
    let mut policy_decision_payload = json!({
        "decision": policy_decision.decision,
        "reason": policy_decision.reason.clone(),
        "agent_version_id": agent_version.id,
        "agent_version": agent_version.version,
    });
    if let Some(gate) = semantic_context_gate.clone() {
        policy_decision_payload["semantic_context_gate"] = gate;
    }
    if let Some(binding) = commit_binding.as_ref() {
        policy_decision_payload["approval_commit_binding"] = json!({
            "normalized_args_hash": binding.normalized_args_hash,
            "target_binding": binding.target_binding
        });
    }
    let call_event = state
        .append_event(
            "tool",
            None,
            input.session_id,
            "tool.call",
            json!({"tool": name, "args": input.args.clone(), "agent_version_id": agent_version.id, "agent_version": agent_version.version}),
        )
        .await?;
    state
        .append_event(
            "agent",
            Some(call_event.id),
            input.session_id,
            "agent.tool_use",
            json!({
                "event_id": call_event.id,
                "tool": name,
                "args": input.args.clone(),
                "agent_version_id": agent_version.id,
                "agent_version": agent_version.version
            }),
        )
        .await?;
    let tool_call = state
        .insert_tool_call(ToolCall {
            id: Uuid::new_v4(),
            session_id: input.session_id,
            event_id: Some(call_event.id),
            tool_name: name.to_string(),
            args: input.args.clone(),
            task_grant_id: task_grant.as_ref().map(|grant| grant.id),
            normalized_args_hash: commit_binding
                .as_ref()
                .map(|binding| binding.normalized_args_hash.clone()),
            target_binding: commit_binding
                .as_ref()
                .map(|binding| binding.target_binding.clone())
                .unwrap_or_else(empty_json_object),
            status: if semantic_context_gate_blocked {
                "denied"
            } else {
                match policy_decision.decision {
                    "allowed" => "running",
                    "requires_approval" => "waiting_approval",
                    "denied" => "denied",
                    _ => "denied",
                }
            }
            .to_string(),
            risk_level: policy_decision.risk_level.clone(),
            policy_decision: policy_decision_payload,
            result: None,
            error: None,
            started_at: if policy_decision.decision == "allowed" && !semantic_context_gate_blocked {
                Some(Utc::now())
            } else {
                None
            },
            completed_at: None,
            created_at: Utc::now(),
        })
        .await?;

    if let Some(gate) = semantic_context_gate
        .as_ref()
        .filter(|gate| gate.get("status").and_then(Value::as_str) == Some("blocked"))
    {
        let reason = semantic_context_gate_block_reason(gate);
        let result = json!({
            "status": "denied",
            "reason": reason,
            "semantic_context_gate": gate,
        });
        state
            .append_event(
                "system",
                Some(tool_call.id),
                input.session_id,
                "semantic_context.gate_failed",
                json!({"tool_call_id": tool_call.id, "tool": name, "content": result}),
            )
            .await?;
        state
            .update_tool_call_status(tool_call.id, "denied", Some(result.clone()), None)
            .await?;
        state
            .append_audit_log(new_audit_log(
                Some(input.session_id),
                "system",
                Some(tool_call.id),
                "semantic_context.gate_failed",
                "tool_call",
                Some(tool_call.id),
                json!({
                    "tool": name,
                    "risk_level": policy_decision.risk_level.clone(),
                    "status": "denied",
                    "semantic_context_gate": gate,
                }),
            ))
            .await?;
        return Err(AppError::forbidden(reason));
    }

    if policy_decision.decision == "denied" {
        let result = json!({"status": "denied", "reason": policy_decision.reason.clone()});
        state
            .append_event(
                "system",
                Some(tool_call.id),
                input.session_id,
                "policy.denied",
                json!({"tool_call_id": tool_call.id, "tool": name, "content": result}),
            )
            .await?;
        state
            .update_tool_call_status(tool_call.id, "denied", Some(result.clone()), None)
            .await?;
        state
            .append_audit_log(new_audit_log(
                Some(input.session_id),
                "system",
                Some(tool_call.id),
                "policy.denied",
                "tool_call",
                Some(tool_call.id),
                json!({"tool": name, "risk_level": policy_decision.risk_level.clone(), "status": "denied"}),
            ))
            .await?;
        return Err(AppError::forbidden(
            result["reason"].as_str().unwrap_or("tool denied"),
        ));
    }

    if policy_decision.decision == "requires_approval" {
        let created_at = Utc::now();
        let approval = Approval {
            id: Uuid::new_v4(),
            session_id: input.session_id,
            tool_call_id: Some(tool_call.id),
            action: name.to_string(),
            risk_level: policy_decision.risk_level.clone(),
            reason: policy_decision.reason.clone(),
            evidence: json!({
                "tool": name,
                "args": input.args,
                "task_grant_id": task_grant.as_ref().map(|grant| grant.id),
                "approval_commit_binding": commit_binding.as_ref().map(|binding| json!({
                    "normalized_args_hash": binding.normalized_args_hash,
                    "target_binding": binding.target_binding
                }))
            }),
            decision_payload: json!({}),
            status: "pending".to_string(),
            expires_at: approval_expires_at(created_at, None),
            created_at,
            decided_at: None,
        };
        let approval = state.insert_approval(approval).await?;
        let result = json!({
            "status": "approval_required",
            "approval_id": approval.id,
            "reason": policy_decision.reason.clone()
        });
        state
            .append_event(
                "system",
                Some(tool_call.id),
                input.session_id,
                "policy.requires_approval",
                json!({"tool_call_id": tool_call.id, "tool": name, "approval_id": approval.id, "content": result}),
            )
            .await?;
        state
            .update_tool_call_status(tool_call.id, "waiting_approval", Some(result.clone()), None)
            .await?;
        state
            .append_audit_log(new_audit_log(
                Some(input.session_id),
                "system",
                Some(tool_call.id),
                "policy.requires_approval",
                "tool_call",
                Some(tool_call.id),
                json!({"tool": name, "risk_level": policy_decision.risk_level.clone(), "status": "waiting_approval", "approval_id": approval.id}),
            ))
            .await?;
        state
            .append_event(
                "system",
                Some(approval.id),
                input.session_id,
                "approval.requested",
                json!({"approval_id": approval.id, "action": approval.action, "risk_level": approval.risk_level, "reason": approval.reason, "evidence": approval.evidence, "expires_at": approval.expires_at}),
            )
            .await?;
        state
            .append_audit_log(new_audit_log(
                Some(input.session_id),
                "system",
                None,
                "approval.requested",
                "approval",
                Some(approval.id),
                json!({"tool_call_id": approval.tool_call_id, "action": approval.action, "risk_level": approval.risk_level, "expires_at": approval.expires_at}),
            ))
            .await?;
        set_managed_session_status(
            state,
            input.session_id,
            SessionStatus::RequiresAction,
            "tool approval required",
        )
        .await?;
        return Ok(result);
    }

    state
        .append_event(
            "system",
            Some(tool_call.id),
            input.session_id,
            "policy.allowed",
            json!({"tool_call_id": tool_call.id, "tool": name, "risk_level": policy_decision.risk_level.clone()}),
        )
        .await?;

    let registry = tool_registry();
    let Some(executor) = registry.get(name) else {
        let error_payload = json!({"error": "unknown tool"});
        state
            .append_event(
                "tool",
                Some(tool_call.id),
                input.session_id,
                "tool.error",
                json!({"tool_call_id": tool_call.id, "tool": name, "content": error_payload}),
            )
            .await?;
        state
            .update_tool_call_status(tool_call.id, "failed", None, Some(error_payload.clone()))
            .await?;
        state
            .append_audit_log(new_audit_log(
                Some(input.session_id),
                "tool",
                Some(tool_call.id),
                "tool.failed",
                "tool_call",
                Some(tool_call.id),
                json!({"tool": name, "error": error_payload}),
            ))
            .await?;
        return Err(AppError::not_found("unknown tool"));
    };
    let result = match executor.execute(state, &input, &tool_call).await {
        Ok(result) => result,
        Err(error) => {
            let error_payload = json!({"error": error.message.clone()});
            state
                .append_event(
                    "tool",
                    Some(tool_call.id),
                    input.session_id,
                    "tool.error",
                    json!({"tool_call_id": tool_call.id, "tool": name, "content": error_payload}),
                )
                .await?;
            state
                .update_tool_call_status(tool_call.id, "failed", None, Some(error_payload.clone()))
                .await?;
            state
                .append_audit_log(new_audit_log(
                    Some(input.session_id),
                    "tool",
                    Some(tool_call.id),
                    "tool.failed",
                    "tool_call",
                    Some(tool_call.id),
                    json!({"tool": name, "error": error_payload}),
                ))
                .await?;
            return Err(error);
        }
    };
    let status = if result.get("status").and_then(Value::as_str) == Some("approval_required") {
        "waiting_approval"
    } else {
        "completed"
    };
    let event_type = if status == "waiting_approval" {
        "policy.requires_approval"
    } else {
        "tool.result"
    };
    let result_origin = match origin {
        ToolInvocationOrigin::ManualRoute => "manual",
        ToolInvocationOrigin::SessionLoop => "session_loop",
    };
    let result_event = state
        .append_event(
            if status == "waiting_approval" {
                "system"
            } else {
                "tool"
            },
            Some(tool_call.id),
            input.session_id,
            event_type,
            json!({"tool_call_id": tool_call.id, "tool": name, "origin": result_origin, "content": result}),
        )
        .await?;
    if status == "completed" && origin == ToolInvocationOrigin::ManualRoute {
        project_session_event_to_loop(state, &result_event).await?;
    }
    state
        .append_event(
            "agent",
            Some(tool_call.id),
            input.session_id,
            "agent.tool_result",
            json!({"tool_call_id": tool_call.id, "tool": name, "status": status, "content": result}),
        )
        .await?;
    state
        .update_tool_call_status(tool_call.id, status, Some(result.clone()), None)
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(input.session_id),
            "tool",
            Some(tool_call.id),
            if status == "waiting_approval" {
                "tool.waiting_approval"
            } else {
                "tool.completed"
            },
            "tool_call",
            Some(tool_call.id),
            json!({"tool": name, "risk_level": policy_decision.risk_level, "status": status}),
        ))
        .await?;
    Ok(result)
}
