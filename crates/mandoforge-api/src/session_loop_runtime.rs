use std::collections::HashSet;

use axum::http::HeaderMap;
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::*;

pub(crate) async fn build_harness_context(
    state: &AppState,
    session_id: Uuid,
    pending_event_seq_start: Option<i64>,
    pending_event_seq_end: Option<i64>,
) -> Result<HarnessContext, AppError> {
    let events = state.list_events(session_id).await?;
    let pending_events = events
        .iter()
        .filter(|event| {
            pending_event_seq_start.is_some_and(|start| event.seq >= start)
                && pending_event_seq_end.is_some_and(|end| event.seq <= end)
        })
        .collect::<Vec<_>>();
    let context_events: Vec<&SessionEvent> =
        if pending_event_seq_start.is_some() && pending_event_seq_end.is_some() {
            pending_events.clone()
        } else {
            events.iter().collect()
        };
    let last_user_message = if context_events
        .iter()
        .any(|event| event.event_type == "user.message")
    {
        context_events
            .iter()
            .rev()
            .find(|event| event.event_type == "user.message")
            .and_then(|event| event.payload.get("message"))
            .and_then(Value::as_str)
            .map(str::to_string)
    } else {
        events
            .iter()
            .rev()
            .find(|event| event.event_type == "user.message")
            .and_then(|event| event.payload.get("message"))
            .and_then(Value::as_str)
            .map(str::to_string)
    };
    let approved_event_result_count = context_events
        .iter()
        .filter(|event| {
            event.event_type == "tool.result"
                && event
                    .payload
                    .get("content")
                    .and_then(|content| content.get("approval"))
                    .and_then(Value::as_str)
                    == Some("approved")
        })
        .count();
    let rejected_event_result_count = context_events
        .iter()
        .filter(|event| {
            event.event_type == "tool.result"
                && event
                    .payload
                    .get("content")
                    .and_then(|content| content.get("approval"))
                    .and_then(Value::as_str)
                    == Some("rejected")
        })
        .count();
    let manual_tool_result_count = context_events
        .iter()
        .filter(|event| {
            event.event_type == "tool.result"
                && event.payload.get("origin").and_then(Value::as_str) == Some("manual")
        })
        .count();
    let execution_completed_events = context_events
        .iter()
        .filter(|event| event.event_type == "execution.completed")
        .rev()
        .take(10)
        .map(|event| {
            json!({
                "event_id": event.id,
                "created_at": event.created_at,
                "payload": event.payload,
            })
        })
        .collect::<Vec<_>>();
    let recent_custom_tool_results = context_events
        .iter()
        .filter(|event| event.event_type == "user.custom_tool_result")
        .rev()
        .take(10)
        .map(|event| {
            json!({
                "event_id": event.id,
                "created_at": event.created_at,
                "payload": event.payload,
            })
        })
        .collect::<Vec<_>>();
    let recent_goal_events = context_events
        .iter()
        .filter(|event| is_session_goal_event(&event.event_type))
        .rev()
        .take(10)
        .map(|event| {
            json!({
                "event_id": event.id,
                "event_type": event.event_type,
                "created_at": event.created_at,
                "payload": event.payload,
            })
        })
        .collect::<Vec<_>>();
    let latest_goal_event = recent_goal_events.first().cloned();
    let (task_grant_id, context_packet_id, rendered_context_packet, provider_tool_names) =
        build_provider_context_packet(state, session_id).await?;
    Ok(HarnessContext {
        session_id,
        task_grant_id,
        context_packet_id,
        rendered_context_packet,
        provider_tool_names,
        event_count: events.len(),
        pending_event_seq_start,
        pending_event_seq_end,
        pending_event_count: pending_events.len(),
        last_user_message,
        latest_goal_event,
        approved_tool_result_count: approved_event_result_count,
        rejected_tool_result_count: rejected_event_result_count,
        manual_tool_result_count,
        execution_completed_count: execution_completed_events.len(),
        custom_tool_result_count: recent_custom_tool_results.len(),
        recent_custom_tool_results,
        recent_execution_completed: execution_completed_events,
        recent_goal_events,
    })
}

pub(crate) async fn build_provider_context_packet(
    state: &AppState,
    session_id: Uuid,
) -> Result<(Option<Uuid>, Option<Uuid>, Option<Value>, Vec<String>), AppError> {
    let active_task_grant = active_task_grant_for_session(state, session_id).await?;
    let task_grant_id = active_task_grant.as_ref().map(|(_, grant)| grant.id);
    let mut context_task_grant = active_task_grant.as_ref().map(|(_, grant)| grant.clone());
    let agent_version = state.agent_version_for_session(session_id).await?;
    let mut provider_tool_names = provider_tool_names_for_grant_and_agent_version(
        context_task_grant.as_ref(),
        &agent_version,
    );
    let mut packet = if let Some((_, grant)) = active_task_grant.as_ref() {
        if let Some(context_packet_id) = grant.context_packet_id {
            Some(state.get_context_packet(context_packet_id).await?)
        } else {
            None
        }
    } else {
        None
    };
    if packet.is_none() {
        packet = state
            .list_context_packets(session_id)
            .await?
            .into_iter()
            .max_by_key(|packet| packet.version);
    }
    if packet.is_none() && active_task_grant.is_some() {
        let generated_packet = generate_and_persist_context_packet(state, session_id).await?;
        if let Some((_, grant)) = active_task_grant.as_ref() {
            let grant = state
                .update_task_grant_context_packet(grant.id, generated_packet.id)
                .await?;
            record_task_grant_checked(state, &grant, session_id, "session_loop.context_packet")
                .await?;
            context_task_grant = Some(grant);
            provider_tool_names = provider_tool_names_for_grant_and_agent_version(
                context_task_grant.as_ref(),
                &agent_version,
            );
        }
        packet = Some(generated_packet);
    }
    let Some(packet) = packet else {
        return Ok((task_grant_id, None, None, provider_tool_names));
    };
    let context_packet_id = packet.id;
    let mut rendered = render_execution_context_for_packet(
        state,
        &packet,
        RenderContextPacketRequest {
            max_prompt_tokens: Some(1_500),
            max_objects: Some(5),
            max_summary_chars: Some(280),
            max_policy_reminders: Some(3),
            allow_full_content: Some(false),
            allow_on_demand_fetch: Some(true),
        },
    )
    .await?;
    if let Some(grant) = context_task_grant.as_ref() {
        rendered
            .available_tools
            .retain(|tool| task_grant_allows_tool(grant, tool));
        if !rendered
            .available_tools
            .iter()
            .any(|tool| tool == "semantic_object.fetch")
        {
            rendered.fetchable_object_ids.clear();
        }
    }
    let rendered = serde_json::to_value(rendered).map_err(|error| {
        AppError::bad_request(format!(
            "failed to serialize rendered context packet: {error}"
        ))
    })?;
    Ok((
        task_grant_id,
        Some(context_packet_id),
        Some(rendered),
        provider_tool_names,
    ))
}

pub(crate) fn provider_tool_names_for_grant_and_agent_version(
    grant: Option<&TaskGrant>,
    agent_version: &AgentVersion,
) -> Vec<String> {
    let names = default_provider_tool_names();
    let agent_allowed = agent_version
        .tools
        .iter()
        .chain(agent_version.tool_names.iter())
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let names = names
        .into_iter()
        .filter(|tool| agent_allowed.contains(tool.as_str()))
        .collect::<Vec<_>>();
    if let Some(grant) = grant {
        names
            .into_iter()
            .filter(|tool| task_grant_allows_tool(grant, tool))
            .collect()
    } else {
        names
    }
}

pub(crate) async fn run_provider_harness(
    state: &AppState,
    session_id: Uuid,
    provider: &dyn ProviderClient,
    provider_label: &str,
    pending_event_seq_start: Option<i64>,
    pending_event_seq_end: Option<i64>,
) -> Result<ProviderResponse, AppError> {
    let context = build_harness_context(
        state,
        session_id,
        pending_event_seq_start,
        pending_event_seq_end,
    )
    .await?;
    let span_id = Uuid::new_v4();
    state
        .append_event(
            "agent",
            Some(span_id),
            session_id,
            "span.model_request_start",
            json!({
                "span_id": span_id,
                "provider": provider_label,
                "client": provider.name(),
                "context": context
            }),
        )
        .await?;
    state
        .append_event(
            "agent",
            Some(span_id),
            session_id,
            "llm.request",
            json!({"span_id": span_id, "provider": provider_label, "client": provider.name(), "context": context}),
        )
        .await?;
    let response = provider.complete(context).await?;
    state
        .append_event(
            "agent",
            Some(span_id),
            session_id,
            "llm.response",
            json!({"span_id": span_id, "provider": provider_label, "client": provider.name(), "tool_calls": &response.tool_calls, "final_message": &response.final_message, "usage": &response.usage}),
        )
        .await?;
    state
        .append_event(
            "agent",
            Some(span_id),
            session_id,
            "span.model_request_end",
            json!({
                "span_id": span_id,
                "provider": provider_label,
                "client": provider.name(),
                "tool_call_count": response.tool_calls.len(),
                "final_message_present": response.final_message.is_some(),
                "usage": response.usage
            }),
        )
        .await?;
    Ok(response)
}

pub(crate) async fn provider_client_for_session(
    state: &AppState,
    session_id: Uuid,
) -> Result<(String, Box<dyn ProviderClient>), AppError> {
    let session = state.get_session(session_id).await?;
    let agent = state.get_agent(session.agent_id).await?;
    if let Some(provider) = state.provider_by_name(&agent.provider).await? {
        if provider.status != "active" {
            return Err(AppError::forbidden(format!(
                "provider {} is not active",
                provider.name
            )));
        }
        enforce_provider_budget(state, &provider).await?;
        let provider_type = provider.provider_type.trim().to_ascii_lowercase();
        if matches!(provider_type.as_str(), "mock" | "mock_openai_compatible") {
            return Ok((provider.name, Box::new(MockProviderClient)));
        }
        if matches!(
            provider_type.as_str(),
            "openai_compatible" | "openai-compatible"
        ) {
            let base_url = provider.base_url.clone().ok_or_else(|| {
                AppError::bad_request("stored openai-compatible provider requires base_url")
            })?;
            let model = agent
                .model
                .trim()
                .is_empty()
                .then(|| provider.default_model.clone())
                .flatten()
                .unwrap_or(agent.model);
            let api_key = stored_provider_api_key(&provider).await?;
            return Ok((
                provider.name,
                Box::new(OpenAiCompatibleProviderClient::from_parts(
                    base_url, api_key, model,
                )?),
            ));
        }
        return Err(AppError::bad_request(format!(
            "provider type {} is not supported",
            provider.provider_type
        )));
    }
    let fallback = provider_client_from_env().await?;
    Ok((agent.provider, fallback))
}

pub(crate) async fn stored_provider_api_key(provider: &ProviderRecord) -> Result<String, AppError> {
    if let Some(env_key) = provider.config.get("api_key_env").and_then(Value::as_str) {
        let value = std::env::var(env_key).map_err(|_| {
            AppError::bad_request(format!(
                "stored provider {} requires env var {env_key}",
                provider.name
            ))
        })?;
        return Ok(value);
    }
    if let Some(value) = provider.config.get("api_key_ref").and_then(Value::as_str) {
        let secret_provider = secret_provider_from_env()?;
        return provider::provider_api_key_from_stored_value(value, secret_provider.as_ref()).await;
    }
    Err(AppError::bad_request(format!(
        "stored provider {} requires config.api_key_env or config.api_key_ref",
        provider.name
    )))
}

pub(crate) async fn enforce_provider_budget(
    state: &AppState,
    provider: &ProviderRecord,
) -> Result<(), AppError> {
    let since = Utc::now() - chrono::Duration::hours(24);
    if let Some(limit) = provider_daily_request_limit(provider) {
        let used = state
            .provider_request_count_since(&provider.name, since)
            .await?;
        if used >= limit {
            return Err(AppError::forbidden(format!(
                "provider {} exceeded daily request budget {limit}",
                provider.name
            )));
        }
    }
    if let Some(limit) = provider_daily_cost_limit_cents(provider) {
        let used = provider_estimated_cost_cents_since(state, provider, since).await?;
        let next_request_cost = provider_per_request_cost_cents(provider);
        if used + next_request_cost > limit {
            return Err(AppError::forbidden(format!(
                "provider {} exceeded daily cost budget {limit:.2} cents",
                provider.name
            )));
        }
    }
    Ok(())
}

pub(crate) fn provider_daily_request_limit(provider: &ProviderRecord) -> Option<i64> {
    provider
        .config
        .get("budget")
        .and_then(|budget| budget.get("daily_request_limit"))
        .and_then(Value::as_i64)
        .filter(|limit| *limit >= 0)
}

pub(crate) fn provider_daily_cost_limit_cents(provider: &ProviderRecord) -> Option<f64> {
    provider
        .config
        .get("budget")
        .and_then(|budget| budget.get("daily_cost_limit_cents"))
        .and_then(Value::as_f64)
        .filter(|limit| *limit >= 0.0)
}

pub(crate) fn provider_per_request_cost_cents(provider: &ProviderRecord) -> f64 {
    provider
        .config
        .get("pricing")
        .and_then(|pricing| pricing.get("per_request_cents"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
}

pub(crate) fn provider_prompt_token_price_cents(provider: &ProviderRecord) -> Option<f64> {
    provider
        .config
        .get("pricing")
        .and_then(|pricing| pricing.get("per_1k_prompt_tokens_cents"))
        .and_then(Value::as_f64)
}

pub(crate) fn provider_completion_token_price_cents(provider: &ProviderRecord) -> Option<f64> {
    provider
        .config
        .get("pricing")
        .and_then(|pricing| pricing.get("per_1k_completion_tokens_cents"))
        .and_then(Value::as_f64)
}

pub(crate) async fn provider_estimated_cost_cents_since(
    state: &AppState,
    provider: &ProviderRecord,
    since: DateTime<Utc>,
) -> Result<f64, AppError> {
    let mut cost = 0.0;
    for session in state.list_sessions().await? {
        for event in state.list_events(session.id).await? {
            if event.created_at < since {
                continue;
            }
            if event.payload.get("provider").and_then(Value::as_str) != Some(provider.name.as_str())
            {
                continue;
            }
            if event.event_type == "llm.request" {
                cost += provider_per_request_cost_cents(provider);
            }
            if event.event_type == "llm.response" {
                let prompt_tokens = event
                    .payload
                    .get("usage")
                    .and_then(|usage| usage.get("prompt_tokens"))
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                let completion_tokens = event
                    .payload
                    .get("usage")
                    .and_then(|usage| usage.get("completion_tokens"))
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                cost +=
                    token_cost_cents(prompt_tokens, provider_prompt_token_price_cents(provider));
                cost += token_cost_cents(
                    completion_tokens,
                    provider_completion_token_price_cents(provider),
                );
            }
        }
    }
    Ok(cost)
}

pub(crate) async fn provider_client_from_env() -> Result<Box<dyn ProviderClient>, AppError> {
    if let Some(provider) = OpenAiCompatibleProviderClient::from_env().await? {
        Ok(Box::new(provider))
    } else {
        Ok(Box::new(MockProviderClient))
    }
}

pub(crate) async fn enqueue_session_loop(
    state: &AppState,
    id: Uuid,
    trigger_event_id: Option<Uuid>,
    reason: &str,
) -> Result<SessionLoopJob, AppError> {
    if !session_accepts_worker_execution(state, id).await? {
        return Err(AppError::bad_request(
            "session is terminal and cannot enqueue session loop work",
        ));
    }
    let job = state
        .enqueue_session_loop_job(id, trigger_event_id, reason)
        .await?;
    state
        .append_event(
            "system",
            Some(job.id),
            id,
            "session.loop.queued",
            json!({
                "session_loop_job_id": job.id,
                "environment_id": job.environment_id,
                "reason": job.reason,
                "status": job.status
            }),
        )
        .await?;
    Ok(job)
}

pub(crate) async fn project_session_event_to_loop(
    state: &AppState,
    event: &SessionEvent,
) -> Result<Option<SessionLoopJob>, AppError> {
    let Some(reason) = session_loop_reason_for_event(&event.event_type) else {
        return Ok(None);
    };
    if matches!(
        event.event_type.as_str(),
        "approval.approved" | "approval.rejected" | "execution.completed"
    ) {
        set_managed_session_status(
            state,
            event.session_id,
            SessionStatus::Idle,
            "durable event projected to session loop",
        )
        .await?;
    }
    enqueue_session_loop(state, event.session_id, Some(event.id), reason)
        .await
        .map(Some)
}

pub(crate) async fn run_session_loop(
    state: &AppState,
    job: &SessionLoopJob,
) -> Result<Session, AppError> {
    let id = job.session_id;
    if !session_accepts_worker_execution(state, id).await? {
        return Err(AppError::bad_request(
            "session is terminal and cannot run session loop work",
        ));
    }
    ensure_primary_session_thread(state, id).await?;
    set_managed_session_status(state, id, SessionStatus::Running, "session loop started").await?;
    state
        .append_audit_log(new_audit_log(
            Some(id),
            "system",
            None,
            "session.started",
            "session",
            Some(id),
            json!({"status": "running"}),
        ))
        .await?;

    let (provider_label, provider) = provider_client_for_session(state, id).await?;
    let provider_response = run_provider_harness(
        state,
        id,
        provider.as_ref(),
        &provider_label,
        job.pending_event_seq_start,
        job.pending_event_seq_end,
    )
    .await?;

    state
        .append_event(
            "agent",
            None,
            id,
            "agent.plan",
            json!({
                "steps": provider_response.plan
            }),
        )
        .await?;

    let session_task_grant_id = active_task_grant_for_session(state, id)
        .await?
        .map(|(_, grant)| grant.id);
    let mut waiting_for_approval = false;
    for tool_call in provider_response.tool_calls {
        let result = execute_tool_invocation(
            state,
            &tool_call.tool_name,
            ExecuteTool {
                session_id: id,
                task_grant_id: session_task_grant_id,
                args: tool_call.args,
            },
            ToolInvocationOrigin::SessionLoop,
        )
        .await?;
        if result.get("status").and_then(Value::as_str) == Some("approval_required") {
            waiting_for_approval = true;
            break;
        }
    }

    let artifact = Artifact {
        id: Uuid::new_v4(),
        session_id: id,
        artifact_type: "markdown".to_string(),
        name: "diagnostics.md".to_string(),
        path: None,
        content: json!({
            "markdown": "# Runtime Diagnostics\n\nThe generic runtime processed recent platform events, confirmed approval gating for shell execution, and produced a replayable diagnostics artifact."
        }),
        created_at: Utc::now(),
    };
    let artifact = state.insert_artifact(artifact).await?;
    state
        .append_event(
        "system",
        Some(artifact.id),
        id,
        "artifact.created",
        json!({"artifact_id": artifact.id, "name": artifact.name, "artifact_type": artifact.artifact_type}),
    )
    .await?;
    state
        .append_audit_log(new_audit_log(
            Some(id),
            "system",
            None,
            "artifact.created",
            "artifact",
            Some(artifact.id),
            json!({"name": artifact.name, "artifact_type": artifact.artifact_type}),
        ))
        .await?;

    state
        .append_event(
        "agent",
        None,
        id,
            "llm.response",
            json!({
                "final_report": {
                "summary": "Generic Runtime Diagnostics Demo reached the approval gate and produced a replayable artifact.",
                "files_read": ["README.md", "config/policy.stage1.yaml"],
                "sql_tables": ["generic_demo.platform_events", "generic_demo.sample_documents", "generic_demo.sample_metrics"],
                "policy_events": ["policy.requires_approval for shell.exec"],
                "artifacts": ["diagnostics.md"],
                "next_steps": [
                    "Add live external provider transport behind the ProviderClient trait",
                    "Add Docker-backed sandbox execution for shell workers",
                    "Run Postgres-backed sql.query integration verification"
                ]
            }
        }),
    )
    .await?;

    if let Some(final_message) = provider_response.final_message {
        state
            .append_event(
                "agent",
                None,
                id,
                "agent.final",
                json!({"message": final_message}),
            )
            .await?;
    }

    let session = if waiting_for_approval {
        set_managed_session_status(
            state,
            id,
            SessionStatus::RequiresAction,
            "tool approval required",
        )
        .await?
    } else {
        let session =
            set_managed_session_status(state, id, SessionStatus::Idle, "provider tool loop idled")
                .await?;
        state
            .append_event(
                "system",
                None,
                id,
                "session.loop.idle",
                json!({"reason": "provider tool loop idled"}),
            )
            .await?;
        session
    };
    Ok(session)
}

pub(crate) async fn authorize_session_run(
    state: &AppState,
    headers: &HeaderMap,
    session_id: Uuid,
) -> Result<(), AppError> {
    authorize_request(
        state,
        headers,
        Permission::SessionsRun,
        "session",
        Some(session_id),
    )
    .await
}
