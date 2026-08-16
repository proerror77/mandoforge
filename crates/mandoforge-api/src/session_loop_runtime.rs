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
    let agent_version = state.agent_version_for_session(session_id).await?;
    let events = state.list_events(session_id).await?;
    let execution_jobs = state.execution_queue.list().await?;
    let hidden_execution_tool_calls: HashSet<_> = execution_jobs
        .iter()
        .filter(|job| {
            job.session_id == session_id
                && !events
                    .iter()
                    .any(|event| execution_completion_event_matches_job(event, job))
        })
        .map(|job| job.tool_call_id)
        .collect();
    let pending_events = events
        .iter()
        .filter(|event| {
            pending_event_seq_start.is_some_and(|start| event.seq >= start)
                && pending_event_seq_end.is_some_and(|end| event.seq <= end)
                && !(event.event_type == "tool.result"
                    && event
                        .actor_id
                        .is_some_and(|actor_id| hidden_execution_tool_calls.contains(&actor_id)))
                && (event.event_type != "execution.completed"
                    || execution_jobs
                        .iter()
                        .any(|job| execution_completion_event_matches_job(event, job)))
                && (event.event_type != "execution.failed"
                    || execution_jobs
                        .iter()
                        .any(|job| execution_failure_event_matches_job(event, job)))
        })
        .collect::<Vec<_>>();
    let context_events: Vec<&SessionEvent> =
        if pending_event_seq_start.is_some() && pending_event_seq_end.is_some() {
            pending_events.clone()
        } else {
            events
                .iter()
                .filter(|event| {
                    !(event.event_type == "tool.result"
                        && event.actor_id.is_some_and(|actor_id| {
                            hidden_execution_tool_calls.contains(&actor_id)
                        }))
                        && (event.event_type != "execution.completed"
                            || execution_jobs
                                .iter()
                                .any(|job| execution_completion_event_matches_job(event, job)))
                        && (event.event_type != "execution.failed"
                            || execution_jobs
                                .iter()
                                .any(|job| execution_failure_event_matches_job(event, job)))
                })
                .collect()
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
    let execution_failed_events = context_events
        .iter()
        .filter(|event| event.event_type == "execution.failed")
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
        agent_version_id: agent_version.id,
        agent_version: agent_version.version,
        system_prompt: agent_version.system_prompt,
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
        execution_failed_count: execution_failed_events.len(),
        custom_tool_result_count: recent_custom_tool_results.len(),
        recent_custom_tool_results,
        recent_execution_completed: execution_completed_events,
        recent_execution_failed: execution_failed_events,
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
    let mut names: Vec<String> = if let Some(grant) = grant {
        names
            .into_iter()
            .filter(|tool| task_grant_allows_tool(grant, tool))
            .collect()
    } else {
        names
            .into_iter()
            .filter(|tool| tool != "mcp.call")
            .collect()
    };
    if !names.iter().any(|tool| tool == "complete_task") {
        names.push("complete_task".to_string());
    }
    names
}

pub(crate) fn provider_completion_request(
    tool_calls: &[ProviderToolCall],
) -> Result<Option<(String, String)>, AppError> {
    let Some(call) = tool_calls
        .iter()
        .find(|call| call.tool_name == "complete_task")
    else {
        return Ok(None);
    };
    if tool_calls.len() != 1 {
        return Err(AppError::bad_request(
            "complete_task must be the only provider tool call",
        ));
    }
    let status = call
        .args
        .get("status")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if !matches!(status, "completed" | "blocked") {
        return Err(AppError::bad_request(
            "complete_task status must be completed or blocked",
        ));
    }
    let summary = call
        .args
        .get("summary")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if summary.is_empty() {
        return Err(AppError::bad_request(
            "complete_task summary must not be empty",
        ));
    }
    Ok(Some((status.to_string(), summary.to_string())))
}

pub(crate) async fn apply_provider_completion(
    state: &AppState,
    session_id: Uuid,
    status: &str,
    summary: &str,
) -> Result<Session, AppError> {
    state
        .append_event(
            "agent",
            None,
            session_id,
            "agent.tool_use",
            json!({"tool": "complete_task", "args": {"status": status, "summary": summary}}),
        )
        .await?;
    state
        .append_event(
            "tool",
            None,
            session_id,
            "tool.result",
            json!({"tool": "complete_task", "origin": "session_loop", "content": {"status": status, "summary": summary}}),
        )
        .await?;
    let event_type = if status == "completed" {
        "session.goal.completed"
    } else {
        "session.goal.blocked"
    };
    state
        .append_event(
            "agent",
            None,
            session_id,
            event_type,
            json!({"objective": summary, "summary": summary, "reason": summary}),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(session_id),
            "agent",
            None,
            event_type,
            "session",
            Some(session_id),
            json!({"status": status, "summary": summary}),
        ))
        .await?;
    set_managed_session_status(
        state,
        session_id,
        if status == "completed" {
            SessionStatus::Terminated
        } else {
            SessionStatus::RequiresAction
        },
        summary,
    )
    .await
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
    let response = match provider.complete(context).await {
        Ok(response) => response,
        Err(error) => {
            let error_message = error.message.clone();
            state
                .append_event(
                    "agent",
                    Some(span_id),
                    session_id,
                    "llm.error",
                    json!({
                        "span_id": span_id,
                        "provider": provider_label,
                        "client": provider.name(),
                        "status": "failed",
                        "error": error_message.clone(),
                    }),
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
                        "status": "failed",
                    }),
                )
                .await?;
            state
                .append_audit_log(new_audit_log(
                    Some(session_id),
                    "agent",
                    Some(span_id),
                    "provider.request_failed",
                    "provider_request",
                    Some(span_id),
                    json!({
                        "provider": provider_label,
                        "client": provider.name(),
                        "status": "failed",
                        "error": error_message,
                    }),
                ))
                .await?;
            return Err(error);
        }
    };
    state
        .append_event(
            "agent",
            Some(span_id),
            session_id,
            "llm.response",
            json!({"span_id": span_id, "provider": provider_label, "client": provider.name(), "plan": &response.plan, "tool_calls": &response.tool_calls, "final_message": &response.final_message, "usage": &response.usage}),
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
                "status": "completed",
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
    let agent_version = state.agent_version_for_session(session_id).await?;
    if let Some(provider) = state.provider_by_name(&agent_version.provider).await? {
        if provider.status != "active" {
            return Err(AppError::forbidden(format!(
                "provider {} is not active",
                provider.name
            )));
        }
        enforce_provider_budget(state, &provider).await?;
        let provider_type = provider.provider_type.trim().to_ascii_lowercase();
        if matches!(provider_type.as_str(), "mock" | "mock_openai_compatible") {
            if provider_runtime_production_mode() {
                return Err(AppError::forbidden(format!(
                    "provider {} uses mock runtime in production mode",
                    provider.name
                )));
            }
            return Ok((provider.name, Box::new(MockProviderClient)));
        }
        if matches!(
            provider_type.as_str(),
            "openai_compatible" | "openai-compatible"
        ) {
            let base_url = provider.base_url.clone().ok_or_else(|| {
                AppError::bad_request("stored openai-compatible provider requires base_url")
            })?;
            let model = agent_version
                .model
                .trim()
                .is_empty()
                .then(|| provider.default_model.clone())
                .flatten()
                .unwrap_or(agent_version.model);
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
    if provider_runtime_production_mode() {
        return Err(AppError::forbidden(format!(
            "production provider runtime requires stored active provider {}",
            agent_version.provider
        )));
    }
    let fallback = provider_client_from_env().await?;
    Ok((agent_version.provider, fallback))
}

pub(crate) fn provider_runtime_production_mode() -> bool {
    std::env::var("MANDOFORGE_PROVIDER_RUNTIME_ENV")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "production" | "prod"
            )
        })
        .unwrap_or(false)
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

async fn task_grant_cost_metering_provider(
    state: &AppState,
    provider_name: &str,
) -> Result<ProviderRecord, AppError> {
    let provider = state
        .provider_by_name(provider_name)
        .await?
        .ok_or_else(|| {
            AppError::forbidden("task grant cost budget requires a stored provider with pricing")
        })?;
    let metered = provider
        .config
        .get("pricing")
        .and_then(Value::as_object)
        .is_some_and(|pricing| {
            [
                "per_request_cents",
                "per_1k_prompt_tokens_cents",
                "per_1k_completion_tokens_cents",
            ]
            .iter()
            .any(|key| pricing.get(*key).and_then(Value::as_f64).is_some())
        });
    if !metered {
        return Err(AppError::forbidden(
            "task grant cost budget requires a stored provider with pricing",
        ));
    }
    Ok(provider)
}

fn provider_uses_token_pricing(provider: &ProviderRecord) -> bool {
    provider_prompt_token_price_cents(provider).is_some()
        || provider_completion_token_price_cents(provider).is_some()
}

fn provider_response_cost_usd_micros(
    provider: &ProviderRecord,
    usage: Option<&crate::provider::ProviderTokenUsage>,
) -> Result<i64, AppError> {
    let mut cost_cents = provider_per_request_cost_cents(provider);
    if let Some(usage) = usage {
        cost_cents += token_cost_cents(
            usage.prompt_tokens,
            provider_prompt_token_price_cents(provider),
        );
        cost_cents += token_cost_cents(
            usage.completion_tokens,
            provider_completion_token_price_cents(provider),
        );
    }
    if !cost_cents.is_finite() || cost_cents < 0.0 {
        return Err(AppError::bad_request(
            "provider pricing produced an invalid task grant cost",
        ));
    }
    let micros = (cost_cents * 10_000.0).ceil();
    if micros > i64::MAX as f64 {
        return Err(AppError::bad_request(
            "provider pricing exceeds task grant cost range",
        ));
    }
    Ok(micros as i64)
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
    enqueue_session_loop_from(state, id, trigger_event_id, reason, None).await
}

async fn enqueue_session_loop_from(
    state: &AppState,
    id: Uuid,
    trigger_event_id: Option<Uuid>,
    reason: &str,
    pending_event_seq_start_floor: Option<i64>,
) -> Result<SessionLoopJob, AppError> {
    if !session_accepts_worker_execution(state, id).await? {
        return Err(AppError::bad_request(
            "session is terminal and cannot enqueue session loop work",
        ));
    }
    let job = match pending_event_seq_start_floor {
        Some(seq_start) => {
            state
                .enqueue_session_loop_job_from(id, trigger_event_id, reason, Some(seq_start))
                .await?
        }
        None => {
            state
                .enqueue_session_loop_job(id, trigger_event_id, reason)
                .await?
        }
    };
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
    project_session_event_to_loop_from(state, event, None).await
}

pub(crate) async fn project_session_event_to_loop_from(
    state: &AppState,
    event: &SessionEvent,
    pending_event_seq_start_floor: Option<i64>,
) -> Result<Option<SessionLoopJob>, AppError> {
    let Some(reason) = session_loop_reason_for_event(&event.event_type) else {
        return Ok(None);
    };
    if event.event_type == "execution.completed"
        && !trusted_execution_completion_event(state, event).await?
    {
        return Ok(None);
    }
    if event.event_type == "execution.failed"
        && !trusted_execution_failure_event(state, event).await?
    {
        return Ok(None);
    }
    if matches!(
        event.event_type.as_str(),
        "approval.approved" | "approval.rejected" | "execution.completed" | "execution.failed"
    ) {
        if !session_accepts_worker_execution(state, event.session_id).await? {
            return Ok(None);
        }
        set_managed_session_status(
            state,
            event.session_id,
            SessionStatus::Idle,
            "durable event projected to session loop",
        )
        .await?;
    }
    if state
        .has_unresolved_execution_result_at_or_before(event.session_id, event.seq)
        .await?
    {
        return Ok(None);
    }
    let pending_event_seq_start_floor = match (
        pending_event_seq_start_floor,
        crate::execution::earliest_uncovered_pending_tool_result_seq(state, event.session_id)
            .await?,
    ) {
        (Some(requested), Some(pending)) => Some(requested.min(pending)),
        (Some(seq), None) | (None, Some(seq)) => Some(seq),
        (None, None) => None,
    };
    let projection_covers_event = match pending_event_seq_start_floor {
        Some(seq_start) => {
            state
                .session_loop_projection_covers_range(event.session_id, seq_start, event.seq)
                .await?
        }
        None => {
            state
                .session_loop_projection_covers(event.session_id, event.seq)
                .await?
        }
    };
    if projection_covers_event {
        return Ok(None);
    }
    enqueue_session_loop_from(
        state,
        event.session_id,
        Some(event.id),
        reason,
        pending_event_seq_start_floor,
    )
    .await
    .map(Some)
}

async fn trusted_execution_completion_event(
    state: &AppState,
    event: &SessionEvent,
) -> Result<bool, AppError> {
    let Some(job_id) = event.actor_id else {
        return Ok(false);
    };
    Ok(state
        .execution_queue
        .list()
        .await?
        .into_iter()
        .any(|job| job.id == job_id && execution_completion_event_matches_job(event, &job)))
}

async fn trusted_execution_failure_event(
    state: &AppState,
    event: &SessionEvent,
) -> Result<bool, AppError> {
    let Some(job_id) = event.actor_id else {
        return Ok(false);
    };
    Ok(state
        .execution_queue
        .list()
        .await?
        .into_iter()
        .any(|job| job.id == job_id && execution_failure_event_matches_job(event, &job)))
}

pub(crate) fn execution_completion_event_matches_job(
    event: &SessionEvent,
    job: &crate::execution_queue::ExecutionJob,
) -> bool {
    event.actor_type == "worker"
        && event.actor_id == Some(job.id)
        && event.session_id == job.session_id
        && event.event_type == "execution.completed"
        && event.payload["status"] == "completed"
        && job.status == ExecutionJobStatus::Completed
        && event.payload["execution_job_id"] == json!(job.id)
        && event.payload["tool_call_id"] == json!(job.tool_call_id)
        && event.payload["attempt_count"] == json!(job.attempt_count)
        && event.payload["claim_generation"] == json!(job.claim_generation)
}

pub(crate) fn execution_failure_event_matches_job(
    event: &SessionEvent,
    job: &crate::execution_queue::ExecutionJob,
) -> bool {
    event.actor_type == "worker"
        && event.actor_id == Some(job.id)
        && event.session_id == job.session_id
        && event.event_type == "execution.failed"
        && event.payload["status"] == "failed"
        && job.status == ExecutionJobStatus::Failed
        && event.payload["execution_job_id"] == json!(job.id)
        && event.payload["tool_call_id"] == json!(job.tool_call_id)
        && event.payload["attempt_count"] == json!(job.attempt_count)
        && event.payload["claim_generation"] == json!(job.claim_generation)
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
    state.ensure_session_runnable(id).await?;
    let active_task_grant = if crate::store_entities::agent_release_enforcement_required() {
        Some(require_active_task_grant_for_session(state, id).await?)
    } else {
        active_task_grant_for_session(state, id).await?
    };
    let active_task_grant = match active_task_grant {
        Some((run, grant)) => {
            let reserved = match state.reserve_task_grant_turn(grant.id).await {
                Ok(grant) => grant,
                Err(error) => {
                    record_task_grant_denied(
                        state,
                        id,
                        Some(&grant),
                        Some(run.id),
                        "session_loop.turn",
                        &error.message,
                    )
                    .await?;
                    return Err(error);
                }
            };
            Some((run, reserved))
        }
        None => None,
    };
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
    let cost_metering_provider = if let Some((run, grant)) = active_task_grant.as_ref()
        && grant.max_cost_usd_micros.is_some()
    {
        match task_grant_cost_metering_provider(state, &provider_label).await {
            Ok(provider) => Some(provider),
            Err(error) => {
                record_task_grant_denied(
                    state,
                    id,
                    Some(grant),
                    Some(run.id),
                    "session_loop.provider",
                    &error.message,
                )
                .await?;
                return Err(error);
            }
        }
    } else {
        None
    };
    let provider_response = run_provider_harness(
        state,
        id,
        provider.as_ref(),
        &provider_label,
        job.pending_event_seq_start,
        job.pending_event_seq_end,
    )
    .await?;

    if let Some((run, grant)) = active_task_grant.as_ref() {
        if let Some(provider) = cost_metering_provider.as_ref()
            && provider_uses_token_pricing(provider)
            && provider_response.usage.is_none()
        {
            let reason = "task grant cost budget requires provider token usage";
            record_task_grant_denied(
                state,
                id,
                Some(grant),
                Some(run.id),
                "session_loop.provider",
                reason,
            )
            .await?;
            return Err(AppError::forbidden(reason));
        }
        let cost_usd_micros = cost_metering_provider
            .as_ref()
            .map(|provider| {
                provider_response_cost_usd_micros(provider, provider_response.usage.as_ref())
            })
            .transpose()?
            .unwrap_or(0);
        let updated = state.add_task_grant_cost(grant.id, cost_usd_micros).await?;
        if updated
            .max_cost_usd_micros
            .is_some_and(|limit| updated.cost_usd_micros_used > limit)
        {
            let reason = "task grant cost budget exceeded by provider response";
            record_task_grant_denied(
                state,
                id,
                Some(&updated),
                Some(run.id),
                "session_loop.provider",
                reason,
            )
            .await?;
            return Err(AppError::forbidden(reason));
        }
    }

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

    let completion = provider_completion_request(&provider_response.tool_calls)?;
    let session_task_grant_id = active_task_grant.as_ref().map(|(_, grant)| grant.id);
    let mut waiting_for_approval = false;
    for tool_call in &provider_response.tool_calls {
        if tool_call.tool_name == "complete_task" {
            continue;
        }
        let result = execute_tool_invocation(
            state,
            &tool_call.tool_name,
            ExecuteTool {
                session_id: id,
                task_grant_id: session_task_grant_id,
                args: tool_call.args.clone(),
            },
            ToolInvocationOrigin::SessionLoop,
        )
        .await?;
        if result.get("status").and_then(Value::as_str) == Some("approval_required") {
            waiting_for_approval = true;
            break;
        }
    }

    let tool_calls = provider_response.tool_calls.clone();
    let final_report = provider_response
        .final_message
        .as_ref()
        .map(|final_message| json!({"summary": final_message}));
    state
        .append_event(
            "agent",
            None,
            id,
            "session_loop.turn_summary",
            serde_json::json!({
                "provider": provider_label,
                "client": provider.name(),
                "plan": provider_response.plan,
                "tool_calls": tool_calls,
                "final_message": provider_response.final_message,
                "final_report": final_report,
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

    if let Some((status, summary)) = completion {
        return apply_provider_completion(state, id, &status, &summary).await;
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
