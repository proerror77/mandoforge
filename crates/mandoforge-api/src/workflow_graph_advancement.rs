use std::collections::HashSet;

use chrono::Utc;
use serde_json::{Value, json};

use crate::*;

pub(crate) async fn record_workflow_step_run_updated(
    state: &AppState,
    run: &WorkflowRun,
    step: &WorkflowStepRun,
    previous_status: &str,
) -> Result<(), AppError> {
    let status_changed = previous_status != step.status;
    let event_type = if status_changed {
        format!("workflow.step.{}", step.status)
    } else {
        "workflow.step.updated".to_string()
    };
    state
        .append_event(
            "system",
            Some(step.id),
            run.primary_session_id,
            &event_type,
            json!({
                "workflow_run_id": run.id,
                "workflow_step_run_id": step.id,
                "step_key": step.step_key,
                "previous_status": previous_status,
                "status": step.status,
                "scheduled_at": step.scheduled_at,
                "output_payload": step.output_payload,
                "artifact_ids": step.artifact_ids,
                "approval_ids": step.approval_ids,
                "tool_call_ids": step.tool_call_ids
            }),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(run.primary_session_id),
            "system",
            Some(step.id),
            event_type
                .replace("workflow.step", "workflow_step_run")
                .as_str(),
            "workflow_step_run",
            Some(step.id),
            json!({
                "workflow_run_id": run.id,
                "step_key": step.step_key,
                "previous_status": previous_status,
                "status": step.status,
                "scheduled_at": step.scheduled_at
            }),
        ))
        .await?;
    Ok(())
}

pub(crate) async fn advance_workflow_graph_after_step_update(
    state: &AppState,
    run: &WorkflowRun,
    step: &WorkflowStepRun,
) -> Result<(), AppError> {
    if step.status == "failed" || step.status == "canceled" {
        advance_workflow_graph_after_step_failure(state, run, step).await?;
        return Ok(());
    }
    if !workflow_step_status_successful(&step.status) {
        if step.status == "running" {
            update_workflow_run_status_and_record(state, run, "running").await?;
        } else if step.status == "requires_action" {
            update_workflow_run_status_and_record(state, run, "requires_action").await?;
        }
        return Ok(());
    }

    let definition = state
        .get_workflow_definition(run.workflow_definition_id)
        .await?;
    let session = state.get_session(run.primary_session_id).await?;
    let root_task_grant_id = run
        .root_task_grant_id
        .ok_or_else(|| AppError::forbidden("workflow run requires root task grant"))?;
    let root_grant = state.get_task_grant(root_task_grant_id).await?;
    loop {
        let existing_steps = state.list_workflow_step_runs(run.id).await?;
        let ready_steps = workflow_graph_ready_steps(&definition.step_graph, &existing_steps)?;
        if ready_steps.is_empty() {
            break;
        }
        let fan_out_max_parallel = workflow_graph_fan_out_max_parallel(&definition.step_graph)?;
        let mut active_parallel_count = workflow_graph_active_parallel_count(&existing_steps);
        let mut materialized_any = false;
        for ready_step in ready_steps {
            let graph_step = ready_step.graph_step;
            let fan_in_payload = workflow_graph_fan_in_payload(&ready_step.fan_in);
            let fan_out_payload =
                workflow_graph_fan_out_payload(fan_out_max_parallel, active_parallel_count);
            if let Some(max_parallel) = fan_out_max_parallel {
                if active_parallel_count >= max_parallel {
                    record_workflow_transition(
                        state,
                        run,
                        Some(step),
                        None,
                        "fan_out",
                        "deferred",
                        json!({
                            "source": "step_graph_fan_out",
                            "max_parallel": max_parallel,
                            "active_parallel_count": active_parallel_count,
                            "dependencies": ready_step.fan_in.dependencies,
                            "graph_step": graph_step
                        }),
                        json!({
                            "deferred_step_key": workflow_graph_step_key(graph_step)?,
                            "reason": "fan_out_max_parallel_reached"
                        }),
                    )
                    .await?;
                    continue;
                }
            }
            let branch_payload = if let Some(evaluation) =
                workflow_graph_step_condition_evaluation(graph_step, &existing_steps)?
            {
                if !evaluation.matched {
                    let condition = evaluation.condition.clone();
                    let path = evaluation.path.clone();
                    let actual = evaluation.actual.clone();
                    let expected = evaluation.expected.clone();
                    let skipped = materialize_workflow_graph_step_with_policy_context(
                        state,
                        &definition,
                        run,
                        &session,
                        &root_grant,
                        graph_step,
                        "skipped",
                        None,
                        json!({
                            "fan_in": fan_in_payload.clone(),
                            "fan_out": fan_out_payload.clone(),
                            "branch": {
                                "condition": condition.clone(),
                                "path": path.clone(),
                                "actual": actual.clone(),
                                "expected": expected.clone(),
                                "reason": "branch_condition_false"
                            }
                        }),
                        json!({
                            "skip_reason": "branch_condition_false",
                            "condition": condition,
                            "actual": actual,
                            "expected": expected
                        }),
                    )
                    .await?;
                    materialized_any = true;
                    record_workflow_transition(
                        state,
                        run,
                        evaluation.source_step.as_ref().or(Some(step)),
                        Some(&skipped),
                        "branch",
                        "skipped",
                        json!({
                            "source": "step_graph_branch",
                            "condition": skipped.output_payload["condition"].clone(),
                            "actual": skipped.output_payload["actual"].clone(),
                            "expected": skipped.output_payload["expected"].clone(),
                            "graph_step": graph_step
                        }),
                        json!({
                            "workflow_step_run_id": skipped.id,
                            "skip_reason": "branch_condition_false"
                        }),
                    )
                    .await?;
                    continue;
                }
                Some(json!({
                    "condition": evaluation.condition,
                    "path": evaluation.path,
                    "actual": evaluation.actual,
                    "expected": evaluation.expected,
                    "matched": true
                }))
            } else {
                None
            };
            let mut policy_context = serde_json::Map::new();
            policy_context.insert("fan_in".to_string(), fan_in_payload.clone());
            policy_context.insert("fan_out".to_string(), fan_out_payload.clone());
            if let Some(branch_payload) = &branch_payload {
                policy_context.insert("branch".to_string(), branch_payload.clone());
            }
            let materialized = materialize_workflow_graph_step_with_policy_context(
                state,
                &definition,
                run,
                &session,
                &root_grant,
                graph_step,
                "queued",
                None,
                Value::Object(policy_context),
                empty_json_object(),
            )
            .await?;
            active_parallel_count += 1;
            materialized_any = true;
            let transition_type = if ready_step.fan_in.mode == "all" {
                if fan_out_max_parallel.is_some() {
                    "fan_out"
                } else {
                    "dependency"
                }
            } else {
                "fan_in"
            };
            record_workflow_transition(
                state,
                run,
                Some(step),
                Some(&materialized),
                transition_type,
                "materialized",
                json!({
                    "source": if ready_step.fan_in.mode == "all" {
                        if fan_out_max_parallel.is_some() { "step_graph_fan_out" } else { "step_graph_dependency" }
                    } else { "step_graph_fan_in" },
                    "dependencies": workflow_graph_step_dependencies(graph_step)?,
                    "fan_out": workflow_graph_fan_out_payload(fan_out_max_parallel, active_parallel_count - 1),
                    "mode": ready_step.fan_in.mode,
                    "min_success": ready_step.fan_in.min_success,
                    "successful_dependencies": ready_step.fan_in.successful_dependencies,
                    "failed_dependencies": ready_step.fan_in.failed_dependencies,
                    "pending_dependencies": ready_step.fan_in.pending_dependencies,
                    "branch": branch_payload,
                    "graph_step": graph_step
                }),
                empty_json_object(),
            )
            .await?;
        }
        if !materialized_any {
            break;
        }
    }

    finalize_workflow_graph_after_transition_policy(state, &definition, run, step).await?;
    Ok(())
}

pub(crate) async fn advance_workflow_graph_after_step_failure(
    state: &AppState,
    run: &WorkflowRun,
    step: &WorkflowStepRun,
) -> Result<(), AppError> {
    let definition = state
        .get_workflow_definition(run.workflow_definition_id)
        .await?;
    let session = state.get_session(run.primary_session_id).await?;
    let root_task_grant_id = run
        .root_task_grant_id
        .ok_or_else(|| AppError::forbidden("workflow run requires root task grant"))?;
    let root_grant = state.get_task_grant(root_task_grant_id).await?;
    let existing_steps = state.list_workflow_step_runs(run.id).await?;
    let Some(graph_step) = workflow_graph_step_by_key(&definition.step_graph, &step.step_key)?
    else {
        update_workflow_run_status_and_record(state, run, &step.status).await?;
        return Ok(());
    };

    if step.status == "failed" {
        let attempts = workflow_graph_step_attempt_count(&existing_steps, &step.step_key);
        let retry_policy = workflow_graph_step_retry_policy(graph_step)?;
        if attempts < retry_policy.max_attempts {
            let next_attempt = attempts + 1;
            let scheduled_at = (retry_policy.delay_seconds > 0)
                .then(|| Utc::now() + chrono::Duration::seconds(retry_policy.delay_seconds));
            let retry_status = if scheduled_at.is_some() {
                "scheduled"
            } else {
                "queued"
            };
            let retry_step = materialize_workflow_graph_step_with_policy_context(
                state,
                &definition,
                run,
                &session,
                &root_grant,
                graph_step,
                retry_status,
                scheduled_at,
                json!({
                    "retry": {
                        "attempt": next_attempt,
                        "max_attempts": retry_policy.max_attempts,
                        "delay_seconds": retry_policy.delay_seconds,
                        "scheduled_at": scheduled_at,
                        "previous_step_run_id": step.id,
                        "previous_status": step.status,
                        "previous_output": step.output_payload
                    }
                }),
                empty_json_object(),
            )
            .await?;
            record_workflow_transition(
                state,
                run,
                Some(step),
                Some(&retry_step),
                "retry",
                retry_status,
                json!({
                    "source": "step_graph_retry",
                    "attempt": next_attempt,
                    "max_attempts": retry_policy.max_attempts,
                    "delay_seconds": retry_policy.delay_seconds,
                    "scheduled_at": scheduled_at,
                    "graph_step": graph_step
                }),
                json!({
                    "workflow_status": "running",
                    "retry_step_run_id": retry_step.id,
                    "retry_status": retry_status
                }),
            )
            .await?;
            update_workflow_run_status_and_record(state, run, "running").await?;
            return Ok(());
        }
    }

    materialize_workflow_graph_failure_policy_steps(
        state,
        &definition,
        run,
        &session,
        &root_grant,
        step,
    )
    .await?;
    finalize_workflow_graph_after_transition_policy(state, &definition, run, step).await?;
    Ok(())
}

pub(crate) async fn finalize_workflow_graph_after_transition_policy(
    state: &AppState,
    definition: &WorkflowDefinition,
    run: &WorkflowRun,
    step: &WorkflowStepRun,
) -> Result<(), AppError> {
    let current_run = state.get_workflow_run(run.id).await?;
    if workflow_graph_run_completed(state, definition, &current_run).await? {
        let updated =
            update_workflow_run_status_and_record(state, &current_run, "completed").await?;
        record_workflow_transition(
            state,
            &updated,
            Some(step),
            None,
            "complete",
            "completed",
            json!({
                "source": "step_graph_complete",
                "graph_keys": workflow_graph_step_keys(&definition.step_graph)?
                    .into_iter()
                    .collect::<Vec<_>>()
            }),
            json!({
                "workflow_status": updated.status,
                "completed_at": updated.completed_at
            }),
        )
        .await?;
    } else if let Some(failure_status) =
        workflow_graph_terminal_failure_status(state, &current_run).await?
    {
        let updated =
            update_workflow_run_status_and_record(state, &current_run, &failure_status).await?;
        let failed_step_keys = workflow_graph_failed_step_keys(state, &updated).await?;
        record_workflow_transition(
            state,
            &updated,
            Some(step),
            None,
            "fail",
            &failure_status,
            json!({
                "source": "step_graph_terminal_failure",
                "graph_keys": workflow_graph_step_keys(&definition.step_graph)?
                    .into_iter()
                    .collect::<Vec<_>>()
            }),
            json!({
                "workflow_status": updated.status,
                "completed_at": updated.completed_at,
                "failed_step_keys": failed_step_keys
            }),
        )
        .await?;
    } else {
        update_workflow_run_status_and_record(state, &current_run, "running").await?;
    }
    Ok(())
}

pub(crate) async fn materialize_workflow_graph_failure_policy_steps(
    state: &AppState,
    definition: &WorkflowDefinition,
    run: &WorkflowRun,
    session: &Session,
    root_grant: &TaskGrant,
    failed_step: &WorkflowStepRun,
) -> Result<(), AppError> {
    let Some(steps) = definition.step_graph.get("steps").and_then(Value::as_array) else {
        return Ok(());
    };
    loop {
        let existing_steps = state.list_workflow_step_runs(run.id).await?;
        let failed_keys = workflow_graph_blocking_failure_keys(&existing_steps);
        let mut materialized_any = false;
        for graph_step in steps {
            let key = workflow_graph_step_key(graph_step)?;
            if workflow_graph_latest_step(&existing_steps, &key).is_some() {
                continue;
            }
            let failure_sources = workflow_graph_step_failure_sources(graph_step)?;
            if failure_sources
                .iter()
                .any(|source| source == &failed_step.step_key)
            {
                let compensation_step = materialize_workflow_graph_step_with_policy_context(
                    state,
                    definition,
                    run,
                    session,
                    root_grant,
                    graph_step,
                    "queued",
                    None,
                    json!({
                        "failure_trigger": {
                            "failed_step_run_id": failed_step.id,
                            "failed_step_key": failed_step.step_key,
                            "failed_status": failed_step.status,
                            "failed_output": failed_step.output_payload
                        }
                    }),
                    empty_json_object(),
                )
                .await?;
                record_workflow_transition(
                    state,
                    run,
                    Some(failed_step),
                    Some(&compensation_step),
                    "compensation",
                    "materialized",
                    json!({
                        "source": "step_graph_failure_policy",
                        "failure_sources": failure_sources,
                        "graph_step": graph_step
                    }),
                    empty_json_object(),
                )
                .await?;
                materialized_any = true;
                continue;
            }

            if !failure_sources.is_empty() {
                continue;
            }
            let dependencies = workflow_graph_step_dependencies(graph_step)?;
            let failed_dependencies = dependencies
                .iter()
                .filter(|dependency| failed_keys.contains(*dependency))
                .cloned()
                .collect::<Vec<_>>();
            if failed_dependencies.is_empty() {
                continue;
            }
            let skipped_step = materialize_workflow_graph_step_with_policy_context(
                state,
                definition,
                run,
                session,
                root_grant,
                graph_step,
                "skipped",
                None,
                json!({
                    "skip": {
                        "reason": "dependency_failed",
                        "failed_dependencies": failed_dependencies,
                        "source_step_run_id": failed_step.id,
                        "source_step_key": failed_step.step_key
                    }
                }),
                json!({
                    "skip_reason": "dependency_failed",
                    "failed_dependencies": failed_dependencies
                }),
            )
            .await?;
            record_workflow_transition(
                state,
                run,
                Some(failed_step),
                Some(&skipped_step),
                "skip",
                "skipped",
                json!({
                    "source": "step_graph_failure_policy",
                    "dependencies": dependencies,
                    "failed_dependencies": skipped_step.output_payload["failed_dependencies"].clone(),
                    "graph_step": graph_step
                }),
                json!({
                    "workflow_step_run_id": skipped_step.id,
                    "skip_reason": "dependency_failed"
                }),
            )
            .await?;
            materialized_any = true;
        }
        if !materialized_any {
            break;
        }
    }
    Ok(())
}

pub(crate) fn workflow_graph_step_attempt_count(
    existing_steps: &[WorkflowStepRun],
    step_key: &str,
) -> usize {
    existing_steps
        .iter()
        .filter(|step| step.step_key == step_key)
        .count()
}

pub(crate) fn workflow_graph_latest_step<'a>(
    existing_steps: &'a [WorkflowStepRun],
    step_key: &str,
) -> Option<&'a WorkflowStepRun> {
    existing_steps
        .iter()
        .rev()
        .find(|step| step.step_key == step_key)
}

pub(crate) fn workflow_graph_blocking_failure_keys(
    existing_steps: &[WorkflowStepRun],
) -> HashSet<String> {
    let mut latest_by_key = HashMap::new();
    for step in existing_steps {
        latest_by_key.insert(step.step_key.as_str(), step);
    }
    latest_by_key
        .values()
        .filter(|step| {
            matches!(step.status.as_str(), "failed" | "canceled")
                || (step.status == "skipped"
                    && step
                        .output_payload
                        .get("skip_reason")
                        .and_then(Value::as_str)
                        == Some("dependency_failed"))
        })
        .map(|step| step.step_key.clone())
        .collect()
}

pub(crate) async fn workflow_graph_terminal_failure_status(
    state: &AppState,
    run: &WorkflowRun,
) -> Result<Option<String>, AppError> {
    let steps = state.list_workflow_step_runs(run.id).await?;
    if steps
        .iter()
        .any(|step| !workflow_step_status_terminal(&step.status))
    {
        return Ok(None);
    }
    if steps.iter().any(|step| step.status == "failed") {
        return Ok(Some("failed".to_string()));
    }
    if steps.iter().any(|step| step.status == "canceled") {
        return Ok(Some("canceled".to_string()));
    }
    Ok(None)
}

pub(crate) async fn workflow_graph_failed_step_keys(
    state: &AppState,
    run: &WorkflowRun,
) -> Result<Vec<String>, AppError> {
    let mut keys = state
        .list_workflow_step_runs(run.id)
        .await?
        .into_iter()
        .filter(|step| step.status == "failed")
        .map(|step| step.step_key)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    keys.sort();
    Ok(keys)
}

pub(crate) async fn workflow_graph_run_completed(
    state: &AppState,
    definition: &WorkflowDefinition,
    run: &WorkflowRun,
) -> Result<bool, AppError> {
    let graph_keys = workflow_graph_step_keys(&definition.step_graph)?;
    if graph_keys.is_empty() {
        return Ok(false);
    }
    let existing_by_key = state
        .list_workflow_step_runs(run.id)
        .await?
        .into_iter()
        .map(|step| (step.step_key, step.status))
        .collect::<HashMap<_, _>>();
    Ok(graph_keys.iter().all(|key| {
        existing_by_key
            .get(key)
            .is_some_and(|status| workflow_step_status_successful(status))
    }))
}

pub(crate) async fn update_workflow_run_status_and_record(
    state: &AppState,
    run: &WorkflowRun,
    status: &str,
) -> Result<WorkflowRun, AppError> {
    if run.status == status {
        return Ok(run.clone());
    }
    let now = Utc::now();
    let terminal = workflow_step_status_terminal(status);
    let started_at = run.started_at.or(Some(now));
    let completed_at = if terminal {
        run.completed_at.or(Some(now))
    } else {
        None
    };
    let updated = state
        .update_workflow_run_status(run.id, status.to_string(), started_at, completed_at)
        .await?;
    let event_type = format!("workflow.run.{status}");
    state
        .append_event(
            "system",
            Some(run.id),
            run.primary_session_id,
            &event_type,
            json!({
                "workflow_run_id": run.id,
                "previous_status": run.status,
                "status": updated.status,
                "started_at": updated.started_at,
                "completed_at": updated.completed_at
            }),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(run.primary_session_id),
            "system",
            Some(run.id),
            event_type.replace("workflow.run", "workflow_run").as_str(),
            "workflow_run",
            Some(run.id),
            json!({
                "previous_status": run.status,
                "status": updated.status,
                "started_at": updated.started_at,
                "completed_at": updated.completed_at
            }),
        ))
        .await?;
    Ok(updated)
}
