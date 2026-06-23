use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::*;

pub(crate) async fn build_usage_summary(state: &AppState) -> Result<UsageSummary, AppError> {
    let sessions = state.list_sessions().await?;
    let tool_calls = state.list_tool_calls(None).await?;
    let approvals = state.list_approvals().await?;
    let providers = state.list_providers().await?;
    let provider_request_prices: HashMap<_, _> = providers
        .iter()
        .filter_map(|provider| {
            provider
                .config
                .get("pricing")
                .and_then(|pricing| pricing.get("per_request_cents"))
                .and_then(Value::as_f64)
                .map(|price| (provider.name.clone(), price))
        })
        .collect();
    let provider_prompt_token_prices: HashMap<_, _> = providers
        .iter()
        .filter_map(|provider| {
            provider
                .config
                .get("pricing")
                .and_then(|pricing| pricing.get("per_1k_prompt_tokens_cents"))
                .and_then(Value::as_f64)
                .map(|price| (provider.name.clone(), price))
        })
        .collect();
    let provider_completion_token_prices: HashMap<_, _> = providers
        .iter()
        .filter_map(|provider| {
            provider
                .config
                .get("pricing")
                .and_then(|pricing| pricing.get("per_1k_completion_tokens_cents"))
                .and_then(Value::as_f64)
                .map(|price| (provider.name.clone(), price))
        })
        .collect();

    let mut event_count = 0;
    let mut provider_request_count = 0;
    let mut provider_response_count = 0;
    let mut prompt_tokens = 0;
    let mut completion_tokens = 0;
    let mut total_tokens = 0;
    let mut by_provider = HashMap::<String, ProviderUsageSummary>::new();
    for session in &sessions {
        for event in state.list_events(session.id).await? {
            event_count += 1;
            if event.event_type == "llm.request" || event.event_type == "llm.response" {
                let provider = event
                    .payload
                    .get("provider")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string();
                let usage = by_provider.entry(provider.clone()).or_default();
                if event.event_type == "llm.request" {
                    provider_request_count += 1;
                    usage.request_count += 1;
                    usage.estimated_cost_cents += provider_request_prices
                        .get(&provider)
                        .copied()
                        .unwrap_or(0.0);
                } else {
                    provider_response_count += 1;
                    usage.response_count += 1;
                    let event_prompt_tokens = event
                        .payload
                        .get("usage")
                        .and_then(|usage| usage.get("prompt_tokens"))
                        .and_then(Value::as_i64)
                        .unwrap_or(0);
                    let event_completion_tokens = event
                        .payload
                        .get("usage")
                        .and_then(|usage| usage.get("completion_tokens"))
                        .and_then(Value::as_i64)
                        .unwrap_or(0);
                    let event_total_tokens = event
                        .payload
                        .get("usage")
                        .and_then(|usage| usage.get("total_tokens"))
                        .and_then(Value::as_i64)
                        .unwrap_or(event_prompt_tokens + event_completion_tokens);
                    usage.prompt_tokens += event_prompt_tokens;
                    usage.completion_tokens += event_completion_tokens;
                    usage.total_tokens += event_total_tokens;
                    prompt_tokens += event_prompt_tokens;
                    completion_tokens += event_completion_tokens;
                    total_tokens += event_total_tokens;
                    let token_cost = token_cost_cents(
                        event_prompt_tokens,
                        provider_prompt_token_prices.get(&provider).copied(),
                    ) + token_cost_cents(
                        event_completion_tokens,
                        provider_completion_token_prices.get(&provider).copied(),
                    );
                    usage.token_cost_cents += token_cost;
                    usage.estimated_cost_cents += token_cost;
                }
            }
        }
    }

    let mut by_tool = HashMap::<String, ToolUsageSummary>::new();
    let mut total_tool_duration_ms = 0;
    let mut tool_success_count = 0;
    let mut tool_failed_count = 0;
    for call in &tool_calls {
        let tool = by_tool.entry(call.tool_name.clone()).or_default();
        tool.call_count += 1;
        if call.status == "completed" {
            tool.success_count += 1;
            tool_success_count += 1;
        }
        if matches!(call.status.as_str(), "failed" | "denied") {
            tool.failed_count += 1;
            tool_failed_count += 1;
        }
        if let (Some(started_at), Some(completed_at)) = (call.started_at, call.completed_at) {
            let duration = completed_at
                .signed_duration_since(started_at)
                .num_milliseconds()
                .max(0);
            tool.total_duration_ms += duration;
            total_tool_duration_ms += duration;
        }
    }

    let mut estimated_provider_cost_cents: f64 = by_provider
        .values()
        .map(|usage| usage.estimated_cost_cents)
        .sum();
    if estimated_provider_cost_cents == 0.0 {
        estimated_provider_cost_cents = 0.0;
    }
    let provider_budgets = build_provider_budget_statuses(state, &providers).await?;
    Ok(UsageSummary {
        session_count: sessions.len(),
        event_count,
        provider_request_count,
        provider_response_count,
        tool_call_count: tool_calls.len(),
        tool_success_count,
        tool_failed_count,
        approval_count: approvals.len(),
        prompt_tokens,
        completion_tokens,
        total_tokens,
        total_tool_duration_ms,
        estimated_provider_cost_cents,
        by_provider,
        by_tool,
        provider_budgets,
    })
}

pub(crate) async fn build_usage_trend_summary(
    state: &AppState,
) -> Result<UsageTrendSummary, AppError> {
    let generated_at = Utc::now();
    let current = build_usage_summary(state).await?;
    let rollups = state.list_usage_rollups().await?;
    Ok(build_usage_trend_from_parts(
        current,
        &rollups,
        generated_at,
    ))
}

pub(crate) fn build_usage_trend_from_parts(
    current: UsageSummary,
    rollups: &[UsageRollup],
    generated_at: DateTime<Utc>,
) -> UsageTrendSummary {
    let current_period = UsageTrendPeriod {
        period_start: generated_at - chrono::Duration::hours(24),
        period_end: generated_at,
        cost_cents: current.estimated_provider_cost_cents,
        total_tokens: current.total_tokens,
        tool_calls: current.tool_call_count as i64,
    };
    let (comparison_basis, latest_period, previous_period) = match rollups {
        [latest, previous, ..] => (
            "latest_rollups".to_string(),
            Some(usage_rollup_trend_period(latest)),
            Some(usage_rollup_trend_period(previous)),
        ),
        [previous] => (
            "current_vs_latest_rollup".to_string(),
            Some(current_period.clone()),
            Some(usage_rollup_trend_period(previous)),
        ),
        [] => (
            "current_only".to_string(),
            Some(current_period.clone()),
            None,
        ),
    };
    let cost_delta_cents = latest_period
        .as_ref()
        .zip(previous_period.as_ref())
        .map(|(latest, previous)| latest.cost_cents - previous.cost_cents);
    let cost_delta_percent = latest_period
        .as_ref()
        .zip(previous_period.as_ref())
        .and_then(|(latest, previous)| percent_delta(latest.cost_cents, previous.cost_cents));
    let token_delta = latest_period
        .as_ref()
        .zip(previous_period.as_ref())
        .map(|(latest, previous)| latest.total_tokens - previous.total_tokens);
    let token_delta_percent = latest_period
        .as_ref()
        .zip(previous_period.as_ref())
        .and_then(|(latest, previous)| {
            percent_delta(latest.total_tokens as f64, previous.total_tokens as f64)
        });
    let tool_call_delta = latest_period
        .as_ref()
        .zip(previous_period.as_ref())
        .map(|(latest, previous)| latest.tool_calls - previous.tool_calls);
    let tool_call_delta_percent = latest_period
        .as_ref()
        .zip(previous_period.as_ref())
        .and_then(|(latest, previous)| {
            percent_delta(latest.tool_calls as f64, previous.tool_calls as f64)
        });
    let top_provider_by_cost = current
        .by_provider
        .iter()
        .max_by(|left, right| {
            left.1
                .estimated_cost_cents
                .total_cmp(&right.1.estimated_cost_cents)
        })
        .map(|(provider_name, usage)| UsageTrendProvider {
            provider_name: provider_name.clone(),
            estimated_cost_cents: usage.estimated_cost_cents,
            total_tokens: usage.total_tokens,
            request_count: usage.request_count,
        });
    let budget_pressure = build_usage_budget_pressure(&current.provider_budgets);
    let mut recommendations = Vec::new();
    if budget_pressure.critical_count > 0 {
        recommendations.push("critical_provider_budget_review".to_string());
    } else if budget_pressure.warning_count > 0 {
        recommendations.push("provider_budget_watch".to_string());
    }
    if cost_delta_percent.is_some_and(|percent| percent >= 25.0) {
        recommendations.push("cost_growth_investigation".to_string());
    }
    if rollups.is_empty() {
        recommendations.push("create_daily_usage_rollup".to_string());
    }
    let forecast = build_usage_forecast(&current, &current_period, generated_at);

    UsageTrendSummary {
        generated_at,
        rollup_count: rollups.len(),
        comparison_basis,
        current_cost_cents: current_period.cost_cents,
        current_total_tokens: current_period.total_tokens,
        current_tool_calls: current_period.tool_calls,
        latest_period,
        previous_period,
        cost_delta_cents,
        cost_delta_percent,
        token_delta,
        token_delta_percent,
        tool_call_delta,
        tool_call_delta_percent,
        top_provider_by_cost,
        budget_pressure,
        forecast,
        recommendations,
    }
}

pub(crate) fn build_usage_forecast(
    current: &UsageSummary,
    current_period: &UsageTrendPeriod,
    generated_at: DateTime<Utc>,
) -> UsageForecastSummary {
    let horizons = [7_i64, 30_i64]
        .into_iter()
        .map(|days| UsageForecastHorizon {
            days,
            projected_cost_cents: current_period.cost_cents * days as f64,
            projected_tokens: current_period.total_tokens.saturating_mul(days),
            projected_tool_calls: current_period.tool_calls.saturating_mul(days),
        })
        .collect();
    let mut provider_budget_exhaustion: Vec<_> = current
        .provider_budgets
        .iter()
        .filter_map(|budget| {
            let daily_cost_limit_cents = budget.daily_cost_limit_cents?;
            let current_daily_cost_cents = if budget.projected_daily_cost_cents > 0.0 {
                budget.projected_daily_cost_cents
            } else {
                budget.estimated_cost_cents
            };
            let projected_days_to_limit = if current_daily_cost_cents <= 0.0 {
                None
            } else {
                Some(
                    ((daily_cost_limit_cents - budget.estimated_cost_cents).max(0.0))
                        / current_daily_cost_cents,
                )
            };
            let projected_exhaustion_at = projected_days_to_limit.map(|days| {
                generated_at + chrono::Duration::seconds((days * 86_400.0).round() as i64)
            });
            Some(ProviderBudgetExhaustionForecast {
                provider_name: budget.provider_name.clone(),
                status: budget.status.clone(),
                current_daily_cost_cents,
                daily_cost_limit_cents,
                projected_days_to_limit,
                projected_exhaustion_at,
            })
        })
        .collect();
    provider_budget_exhaustion.sort_by(|left, right| {
        match (left.projected_days_to_limit, right.projected_days_to_limit) {
            (Some(left_days), Some(right_days)) => left_days.total_cmp(&right_days),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => left.provider_name.cmp(&right.provider_name),
        }
    });
    UsageForecastSummary {
        basis: "current_24h_run_rate".to_string(),
        horizons,
        provider_budget_exhaustion,
    }
}

pub(crate) fn usage_rollup_trend_period(rollup: &UsageRollup) -> UsageTrendPeriod {
    UsageTrendPeriod {
        period_start: rollup.period_start,
        period_end: rollup.period_end,
        cost_cents: json_f64(&rollup.summary, "estimated_provider_cost_cents"),
        total_tokens: json_i64(&rollup.summary, "total_tokens"),
        tool_calls: json_i64(&rollup.summary, "tool_call_count"),
    }
}

pub(crate) fn build_usage_budget_pressure(budgets: &[ProviderBudgetStatus]) -> UsageBudgetPressure {
    let critical_count = budgets
        .iter()
        .filter(|budget| budget.status == "critical")
        .count();
    let warning_count = budgets
        .iter()
        .filter(|budget| budget.status == "warning")
        .count();
    let highest_status = if critical_count > 0 {
        "critical"
    } else if warning_count > 0 {
        "warning"
    } else {
        "ok"
    }
    .to_string();
    let highest_used_percent = budgets
        .iter()
        .flat_map(|budget| {
            [
                budget.request_budget_used_percent,
                budget.cost_budget_used_percent,
            ]
        })
        .flatten()
        .max_by(f64::total_cmp);
    UsageBudgetPressure {
        total_budgeted_providers: budgets.len(),
        pressure_count: critical_count + warning_count,
        warning_count,
        critical_count,
        highest_status,
        highest_used_percent,
    }
}

pub(crate) fn json_f64(value: &Value, key: &str) -> f64 {
    value.get(key).and_then(Value::as_f64).unwrap_or_default()
}

pub(crate) fn json_i64(value: &Value, key: &str) -> i64 {
    value
        .get(key)
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_u64().map(|value| value as i64))
        })
        .unwrap_or_default()
}

pub(crate) fn percent_delta(current: f64, previous: f64) -> Option<f64> {
    if previous.abs() < f64::EPSILON {
        return None;
    }
    Some(((current - previous) / previous) * 100.0)
}

pub(crate) fn build_usage_finance_csv(summary: &UsageSummary, trend: &UsageTrendSummary) -> String {
    let mut csv = String::new();
    push_csv_row(
        &mut csv,
        vec![
            "section".to_string(),
            "name".to_string(),
            "status".to_string(),
            "requests".to_string(),
            "responses".to_string(),
            "tokens".to_string(),
            "tool_calls".to_string(),
            "cost_cents".to_string(),
            "percent".to_string(),
            "notes".to_string(),
        ],
    );
    push_csv_row(
        &mut csv,
        vec![
            "summary".to_string(),
            "current_24h".to_string(),
            "current".to_string(),
            summary.provider_request_count.to_string(),
            summary.provider_response_count.to_string(),
            summary.total_tokens.to_string(),
            summary.tool_call_count.to_string(),
            format_csv_float(summary.estimated_provider_cost_cents),
            optional_csv_float(trend.budget_pressure.highest_used_percent),
            format!(
                "sessions={};events={};approvals={}",
                summary.session_count, summary.event_count, summary.approval_count
            ),
        ],
    );
    if let Some(latest) = &trend.latest_period {
        push_usage_trend_period_csv_row(&mut csv, "trend", "latest", latest);
    }
    if let Some(previous) = &trend.previous_period {
        push_usage_trend_period_csv_row(&mut csv, "trend", "previous", previous);
    }
    push_csv_row(
        &mut csv,
        vec![
            "trend".to_string(),
            "delta".to_string(),
            trend.comparison_basis.clone(),
            String::new(),
            String::new(),
            trend
                .token_delta
                .map(|value| value.to_string())
                .unwrap_or_default(),
            trend
                .tool_call_delta
                .map(|value| value.to_string())
                .unwrap_or_default(),
            optional_csv_float(trend.cost_delta_cents),
            optional_csv_float(trend.cost_delta_percent),
            "percent_column_contains_cost_delta_percent_for_delta_row".to_string(),
        ],
    );

    let mut provider_entries: Vec<_> = summary.by_provider.iter().collect();
    provider_entries.sort_by(|left, right| {
        right
            .1
            .estimated_cost_cents
            .total_cmp(&left.1.estimated_cost_cents)
            .then_with(|| left.0.cmp(right.0))
    });
    for (provider_name, usage) in provider_entries {
        push_csv_row(
            &mut csv,
            vec![
                "provider".to_string(),
                provider_name.clone(),
                "usage".to_string(),
                usage.request_count.to_string(),
                usage.response_count.to_string(),
                usage.total_tokens.to_string(),
                String::new(),
                format_csv_float(usage.estimated_cost_cents),
                String::new(),
                format!(
                    "prompt_tokens={};completion_tokens={};token_cost_cents={}",
                    usage.prompt_tokens,
                    usage.completion_tokens,
                    format_csv_float(usage.token_cost_cents)
                ),
            ],
        );
    }
    for horizon in &trend.forecast.horizons {
        push_csv_row(
            &mut csv,
            vec![
                "forecast".to_string(),
                format!("{}d", horizon.days),
                trend.forecast.basis.clone(),
                String::new(),
                String::new(),
                horizon.projected_tokens.to_string(),
                horizon.projected_tool_calls.to_string(),
                format_csv_float(horizon.projected_cost_cents),
                String::new(),
                "projected_from_current_24h_run_rate".to_string(),
            ],
        );
    }
    for budget in &summary.provider_budgets {
        push_csv_row(
            &mut csv,
            vec![
                "budget".to_string(),
                budget.provider_name.clone(),
                budget.status.clone(),
                budget.request_count.to_string(),
                String::new(),
                String::new(),
                String::new(),
                format_csv_float(budget.estimated_cost_cents),
                optional_csv_float(budget_peak_percent(budget)),
                budget.messages.join(" | "),
            ],
        );
    }
    for forecast in &trend.forecast.provider_budget_exhaustion {
        push_csv_row(
            &mut csv,
            vec![
                "budget_forecast".to_string(),
                forecast.provider_name.clone(),
                forecast.status.clone(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                format_csv_float(forecast.current_daily_cost_cents),
                optional_csv_float(forecast.projected_days_to_limit),
                format!(
                    "daily_limit_cents={};projected_exhaustion_at={}",
                    format_csv_float(forecast.daily_cost_limit_cents),
                    forecast
                        .projected_exhaustion_at
                        .map(|value| value.to_rfc3339())
                        .unwrap_or_else(|| "unknown".to_string())
                ),
            ],
        );
    }
    for recommendation in &trend.recommendations {
        push_csv_row(
            &mut csv,
            vec![
                "recommendation".to_string(),
                recommendation.clone(),
                "open".to_string(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                "operator_action".to_string(),
            ],
        );
    }
    csv
}

pub(crate) fn push_usage_trend_period_csv_row(
    csv: &mut String,
    section: &str,
    name: &str,
    period: &UsageTrendPeriod,
) {
    push_csv_row(
        csv,
        vec![
            section.to_string(),
            name.to_string(),
            "window".to_string(),
            String::new(),
            String::new(),
            period.total_tokens.to_string(),
            period.tool_calls.to_string(),
            format_csv_float(period.cost_cents),
            String::new(),
            format!("{} to {}", period.period_start, period.period_end),
        ],
    );
}

pub(crate) fn budget_peak_percent(budget: &ProviderBudgetStatus) -> Option<f64> {
    [
        budget.request_budget_used_percent,
        budget.cost_budget_used_percent,
    ]
    .into_iter()
    .flatten()
    .max_by(f64::total_cmp)
}

pub(crate) fn format_csv_float(value: f64) -> String {
    format!("{value:.6}")
}

pub(crate) fn optional_csv_float(value: Option<f64>) -> String {
    value.map(format_csv_float).unwrap_or_default()
}

pub(crate) fn push_csv_row(csv: &mut String, cells: Vec<String>) {
    let row = cells
        .into_iter()
        .map(csv_escape_cell)
        .collect::<Vec<_>>()
        .join(",");
    csv.push_str(&row);
    csv.push('\n');
}

pub(crate) fn csv_escape_cell(cell: String) -> String {
    if cell.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", cell.replace('"', "\"\""))
    } else {
        cell
    }
}

pub(crate) async fn build_provider_budget_statuses(
    state: &AppState,
    providers: &[ProviderRecord],
) -> Result<Vec<ProviderBudgetStatus>, AppError> {
    let since = Utc::now() - chrono::Duration::hours(24);
    let mut statuses = Vec::new();
    for provider in providers {
        let daily_request_limit = provider_daily_request_limit(provider);
        let daily_cost_limit_cents = provider_daily_cost_limit_cents(provider);
        if daily_request_limit.is_none() && daily_cost_limit_cents.is_none() {
            continue;
        }
        let request_count = state
            .provider_request_count_since(&provider.name, since)
            .await?;
        let estimated_cost_cents =
            provider_estimated_cost_cents_since(state, provider, since).await?;
        let request_budget_used_percent =
            daily_request_limit.map(|limit| percent_used(request_count as f64, limit as f64));
        let cost_budget_used_percent =
            daily_cost_limit_cents.map(|limit| percent_used(estimated_cost_cents, limit));
        let max_used_percent = request_budget_used_percent
            .into_iter()
            .chain(cost_budget_used_percent)
            .fold(0.0, f64::max);
        let status = if max_used_percent >= 100.0 {
            "critical"
        } else if max_used_percent >= 80.0 {
            "warning"
        } else {
            "ok"
        }
        .to_string();
        let mut messages = Vec::new();
        if let (Some(limit), Some(percent)) = (daily_request_limit, request_budget_used_percent) {
            messages.push(format!(
                "{request_count} of {limit} daily requests used ({percent:.1}%)"
            ));
        }
        if let (Some(limit), Some(percent)) = (daily_cost_limit_cents, cost_budget_used_percent) {
            messages.push(format!(
                "{estimated_cost_cents:.2} of {limit:.2} daily cost cents used ({percent:.1}%)"
            ));
        }
        statuses.push(ProviderBudgetStatus {
            provider_name: provider.name.clone(),
            status,
            window_hours: 24,
            request_count,
            daily_request_limit,
            request_budget_used_percent,
            estimated_cost_cents,
            projected_daily_cost_cents: estimated_cost_cents,
            daily_cost_limit_cents,
            cost_budget_used_percent,
            messages,
        });
    }
    statuses.sort_by(|left, right| {
        budget_rank(&right.status)
            .cmp(&budget_rank(&left.status))
            .then_with(|| {
                right
                    .projected_daily_cost_cents
                    .total_cmp(&left.projected_daily_cost_cents)
            })
            .then_with(|| left.provider_name.cmp(&right.provider_name))
    });
    Ok(statuses)
}

pub(crate) fn percent_used(used: f64, limit: f64) -> f64 {
    if limit <= 0.0 {
        if used > 0.0 { 100.0 } else { 0.0 }
    } else {
        (used / limit) * 100.0
    }
}

pub(crate) fn build_cost_alerts(
    budgets: &[ProviderBudgetStatus],
    created_at: DateTime<Utc>,
) -> Vec<CostAlert> {
    budgets
        .iter()
        .filter(|budget| budget_rank(&budget.status) >= budget_rank("warning"))
        .map(|budget| CostAlert {
            provider_name: budget.provider_name.clone(),
            severity: budget.status.clone(),
            message: format!(
                "provider {} budget status is {}",
                budget.provider_name, budget.status
            ),
            messages: budget.messages.clone(),
            window_hours: budget.window_hours,
            request_budget_used_percent: budget.request_budget_used_percent,
            cost_budget_used_percent: budget.cost_budget_used_percent,
            estimated_cost_cents: budget.estimated_cost_cents,
            created_at,
        })
        .collect()
}

pub(crate) fn budget_rank(status: &str) -> i32 {
    match status {
        "critical" => 3,
        "warning" => 2,
        _ => 1,
    }
}

pub(crate) fn token_cost_cents(tokens: i64, price_per_1k_cents: Option<f64>) -> f64 {
    let Some(price) = price_per_1k_cents else {
        return 0.0;
    };
    (tokens.max(0) as f64 / 1000.0) * price
}
