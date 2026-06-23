use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::*;

pub(crate) async fn build_semantic_synthesis_schedule_due_counts(
    state: &AppState,
    checked_at: DateTime<Utc>,
) -> Result<(usize, usize, usize), AppError> {
    let objects = state
        .list_workflow_pack_runtime_objects_by_runtime_kind("semantic_synthesis_schedule")
        .await?;
    let audit_logs = state.list_audit_logs(None).await?;
    let mut scheduled_count = 0usize;
    let mut due_count = 0usize;
    let mut skipped_count = 0usize;
    for object in objects {
        scheduled_count += 1;
        if !semantic_synthesis_schedule_is_runnable(&object) {
            skipped_count += 1;
            continue;
        }
        match semantic_synthesis_schedule_due_session_ids(state, &object, &audit_logs, checked_at)
            .await
        {
            Ok(session_ids) if !session_ids.is_empty() => due_count += session_ids.len(),
            Ok(_) | Err(_) => skipped_count += 1,
        }
    }
    Ok((scheduled_count, due_count, skipped_count))
}

pub(crate) async fn build_scheduled_runtime_object_due_counts(
    state: &AppState,
    checked_at: DateTime<Utc>,
    runtime_kind: &str,
    success_action: &str,
) -> Result<(usize, usize, usize), AppError> {
    let objects = state
        .list_workflow_pack_runtime_objects_by_runtime_kind(runtime_kind)
        .await?;
    let audit_logs = state.list_audit_logs(None).await?;
    let mut scheduled_count = 0usize;
    let mut due_count = 0usize;
    let mut skipped_count = 0usize;
    for object in objects {
        scheduled_count += 1;
        if !scheduled_runtime_object_is_runnable(&object, runtime_kind) {
            skipped_count += 1;
            continue;
        }
        match scheduled_runtime_object_is_due(&object, &audit_logs, checked_at, success_action) {
            Ok(true) => due_count += 1,
            Ok(false) | Err(_) => skipped_count += 1,
        }
    }
    Ok((scheduled_count, due_count, skipped_count))
}

pub(crate) async fn execute_due_semantic_synthesis_schedules(
    state: &AppState,
    checked_at: DateTime<Utc>,
) -> Result<SemanticSynthesisScheduleSweep, AppError> {
    let objects = state
        .list_workflow_pack_runtime_objects_by_runtime_kind("semantic_synthesis_schedule")
        .await?;
    let audit_logs = state.list_audit_logs(None).await?;
    let mut scheduled_count = 0usize;
    let mut due_count = 0usize;
    let mut created_count = 0usize;
    let mut skipped_count = 0usize;
    let mut failed_count = 0usize;
    let mut runs = Vec::new();

    for object in objects {
        scheduled_count += 1;
        if !semantic_synthesis_schedule_is_runnable(&object) {
            skipped_count += 1;
            runs.push(semantic_synthesis_scheduled_run_skipped(
                &object,
                None,
                None,
                "schedule runtime object is not released or active",
            ));
            continue;
        }
        let due_session_ids = match semantic_synthesis_schedule_due_session_ids(
            state,
            &object,
            &audit_logs,
            checked_at,
        )
        .await
        {
            Ok(session_ids) => session_ids,
            Err(error) => {
                failed_count += 1;
                runs.push(semantic_synthesis_scheduled_run_failed(
                    &object,
                    None,
                    None,
                    &error.message,
                ));
                continue;
            }
        };
        if due_session_ids.is_empty() {
            skipped_count += 1;
            runs.push(semantic_synthesis_scheduled_run_skipped(
                &object,
                None,
                None,
                "schedule is not due or was already completed for all target sessions",
            ));
            continue;
        }
        due_count += due_session_ids.len();
        let (synthesis_type, input) =
            match semantic_synthesis_schedule_input_from_runtime_object(&object) {
                Ok(input) => input,
                Err(error) => {
                    failed_count += 1;
                    runs.push(semantic_synthesis_scheduled_run_failed(
                        &object,
                        None,
                        None,
                        &error.message,
                    ));
                    continue;
                }
            };
        for session_id in due_session_ids {
            let input = semantic_synthesis_schedule_input_for_session(&object, input.clone());
            match materialize_semantic_synthesis_run_for_actor(
                state,
                session_id,
                "system".to_string(),
                "system",
                input,
            )
            .await
            {
                Ok(result) => {
                    state
                        .append_audit_log(new_audit_log(
                            Some(session_id),
                            "system",
                            Some(object.id),
                            "semantic_synthesis.schedule_run_created",
                            "workflow_pack_runtime_object",
                            Some(object.id),
                            json!({
                                "runtime_object_id": object.id,
                                "object_key": object.object_key.clone(),
                                "session_id": session_id,
                                "synthesis_type": result.synthesis_type.clone(),
                                "artifact_id": result.artifact.id,
                                "candidate_count": result.candidates.len(),
                                "checked_at": checked_at,
                            }),
                        ))
                        .await?;
                    created_count += 1;
                    runs.push(SemanticSynthesisScheduledRun {
                        runtime_object_id: object.id,
                        object_key: object.object_key.clone(),
                        session_id: Some(session_id),
                        synthesis_type: Some(result.synthesis_type),
                        status: "created".to_string(),
                        artifact_id: Some(result.artifact.id),
                        candidate_count: result.candidates.len(),
                        reason: None,
                    });
                }
                Err(error) => {
                    failed_count += 1;
                    state
                        .append_audit_log(new_audit_log(
                            Some(session_id),
                            "system",
                            Some(object.id),
                            "semantic_synthesis.schedule_run_failed",
                            "workflow_pack_runtime_object",
                            Some(object.id),
                            json!({
                                "runtime_object_id": object.id,
                                "object_key": object.object_key.clone(),
                                "session_id": session_id,
                                "synthesis_type": synthesis_type.clone(),
                                "error": error.message.clone(),
                                "checked_at": checked_at,
                            }),
                        ))
                        .await?;
                    runs.push(semantic_synthesis_scheduled_run_failed(
                        &object,
                        Some(session_id),
                        Some(synthesis_type.as_str()),
                        &error.message,
                    ));
                }
            }
        }
    }

    let mut actions = Vec::new();
    if created_count > 0 {
        actions.push("run_due_semantic_synthesis_schedules".to_string());
    }
    let status = if failed_count > 0 && created_count > 0 {
        "partial".to_string()
    } else if failed_count > 0 {
        "failed".to_string()
    } else if created_count > 0 {
        "completed".to_string()
    } else if scheduled_count > 0 {
        "waiting".to_string()
    } else {
        "noop".to_string()
    };

    Ok(SemanticSynthesisScheduleSweep {
        status,
        checked_at,
        scheduled_count,
        due_count,
        created_count,
        skipped_count,
        failed_count,
        runs,
        actions,
    })
}

pub(crate) fn semantic_synthesis_schedule_is_runnable(object: &WorkflowPackRuntimeObject) -> bool {
    object.object_type == "schedule"
        && object.runtime_kind == "semantic_synthesis_schedule"
        && matches!(object.status.as_str(), "released" | "active")
        && object
            .spec
            .pointer("/schedule_policy/enabled")
            .and_then(Value::as_bool)
            .unwrap_or(true)
}

pub(crate) async fn semantic_synthesis_schedule_due_session_ids(
    state: &AppState,
    object: &WorkflowPackRuntimeObject,
    audit_logs: &[AuditLog],
    checked_at: DateTime<Utc>,
) -> Result<Vec<Uuid>, AppError> {
    let due_at = semantic_synthesis_schedule_due_at(object)?.unwrap_or(object.created_at);
    if due_at > checked_at {
        return Ok(Vec::new());
    }
    let mut target_session_ids = Vec::new();
    if let Some(session_id) = semantic_synthesis_schedule_session_id(object)? {
        target_session_ids.push(session_id);
    } else if let Some(workflow_definition_id) =
        semantic_synthesis_schedule_workflow_definition_id(object)?
    {
        let mut seen = HashSet::new();
        for run in state
            .list_workflow_runs()
            .await?
            .into_iter()
            .filter(|run| run.workflow_definition_id == workflow_definition_id)
            .filter(|run| run.status == "completed")
            .filter(|run| {
                run.completed_at
                    .is_none_or(|completed_at| completed_at <= checked_at)
            })
        {
            if seen.insert(run.primary_session_id) {
                target_session_ids.push(run.primary_session_id);
            }
        }
    } else {
        return Err(AppError::bad_request(
            "semantic synthesis schedule requires session_id or workflow_definition_id",
        ));
    }
    Ok(target_session_ids
        .into_iter()
        .filter(|session_id| {
            semantic_synthesis_schedule_session_is_due(object, audit_logs, *session_id, checked_at)
        })
        .collect())
}

pub(crate) fn semantic_synthesis_schedule_session_is_due(
    object: &WorkflowPackRuntimeObject,
    audit_logs: &[AuditLog],
    session_id: Uuid,
    checked_at: DateTime<Utc>,
) -> bool {
    let Some(last_run_at) =
        semantic_synthesis_schedule_last_success_at(audit_logs, object.id, Some(session_id))
    else {
        return true;
    };
    let interval_seconds = object
        .spec
        .pointer("/schedule_policy/interval_seconds")
        .and_then(Value::as_i64)
        .filter(|seconds| *seconds > 0);
    let Some(interval_seconds) = interval_seconds else {
        return false;
    };
    last_run_at + chrono::Duration::seconds(interval_seconds) <= checked_at
}

pub(crate) fn semantic_synthesis_schedule_due_at(
    object: &WorkflowPackRuntimeObject,
) -> Result<Option<DateTime<Utc>>, AppError> {
    let Some(value) = object
        .spec
        .pointer("/schedule_policy/due_at")
        .or_else(|| object.spec.get("due_at"))
    else {
        return Ok(None);
    };
    let Some(raw) = value.as_str() else {
        return Err(AppError::bad_request(
            "semantic synthesis schedule due_at must be an RFC3339 string",
        ));
    };
    DateTime::parse_from_rfc3339(raw)
        .map(|value| Some(value.with_timezone(&Utc)))
        .map_err(|_| {
            AppError::bad_request("semantic synthesis schedule due_at must be an RFC3339 string")
        })
}

pub(crate) fn semantic_synthesis_schedule_last_success_at(
    audit_logs: &[AuditLog],
    runtime_object_id: Uuid,
    session_id: Option<Uuid>,
) -> Option<DateTime<Utc>> {
    audit_logs
        .iter()
        .filter(|log| {
            log.action == "semantic_synthesis.schedule_run_created"
                && log
                    .details
                    .get("runtime_object_id")
                    .and_then(Value::as_str)
                    .and_then(|value| Uuid::parse_str(value).ok())
                    == Some(runtime_object_id)
                && session_id.is_none_or(|session_id| {
                    log.details
                        .get("session_id")
                        .and_then(Value::as_str)
                        .and_then(|value| Uuid::parse_str(value).ok())
                        == Some(session_id)
                })
        })
        .map(|log| log.created_at)
        .max()
}

pub(crate) fn semantic_synthesis_schedule_session_id(
    object: &WorkflowPackRuntimeObject,
) -> Result<Option<Uuid>, AppError> {
    object
        .spec
        .get("session_id")
        .and_then(Value::as_str)
        .or_else(|| {
            object
                .spec
                .pointer("/semantic_synthesis/session_id")
                .and_then(Value::as_str)
        })
        .map(|value| {
            Uuid::parse_str(value).map_err(|_| {
                AppError::bad_request(
                    "semantic synthesis schedule session_id must be a UUID string",
                )
            })
        })
        .transpose()
}

pub(crate) fn semantic_synthesis_schedule_workflow_definition_id(
    object: &WorkflowPackRuntimeObject,
) -> Result<Option<Uuid>, AppError> {
    object
        .spec
        .get("workflow_definition_id")
        .and_then(Value::as_str)
        .or_else(|| {
            object
                .spec
                .pointer("/semantic_synthesis/workflow_definition_id")
                .and_then(Value::as_str)
        })
        .map(|value| {
            Uuid::parse_str(value).map_err(|_| {
                AppError::bad_request(
                    "semantic synthesis schedule workflow_definition_id must be a UUID string",
                )
            })
        })
        .transpose()
}

pub(crate) fn semantic_synthesis_schedule_input_from_runtime_object(
    object: &WorkflowPackRuntimeObject,
) -> Result<(String, CreateSemanticSynthesisRun), AppError> {
    let synthesis_type = object
        .spec
        .get("synthesis_type")
        .and_then(Value::as_str)
        .or_else(|| {
            object
                .spec
                .pointer("/semantic_synthesis/synthesis_type")
                .and_then(Value::as_str)
        })
        .unwrap_or("post_run_reflection")
        .to_string();
    let goal_attempted = object
        .spec
        .get("goal_attempted")
        .and_then(Value::as_str)
        .or_else(|| {
            object
                .spec
                .pointer("/semantic_synthesis/goal_attempted")
                .and_then(Value::as_str)
        })
        .unwrap_or("Scheduled semantic synthesis")
        .to_string();
    let durable_memory_candidates =
        serde_json::from_value::<Vec<SemanticSynthesisMemoryCandidateInput>>(
            object
                .spec
                .get("durable_memory_candidates")
                .or_else(|| {
                    object
                        .spec
                        .pointer("/semantic_synthesis/durable_memory_candidates")
                })
                .cloned()
                .unwrap_or_else(|| json!([])),
        )
        .map_err(|error| {
            AppError::bad_request(format!(
                "semantic synthesis schedule durable_memory_candidates are invalid: {error}"
            ))
        })?;
    let metadata = object
        .spec
        .get("metadata")
        .or_else(|| object.spec.pointer("/semantic_synthesis/metadata"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    Ok((
        synthesis_type.clone(),
        CreateSemanticSynthesisRun {
            synthesis_type,
            goal_attempted,
            context_used: semantic_synthesis_schedule_string_array(&object.spec, "context_used"),
            worked: semantic_synthesis_schedule_string_array(&object.spec, "worked"),
            failed_or_corrected: semantic_synthesis_schedule_string_array(
                &object.spec,
                "failed_or_corrected",
            ),
            unsafe_assumptions: semantic_synthesis_schedule_string_array(
                &object.spec,
                "unsafe_assumptions",
            ),
            durable_memory_candidates,
            metadata,
        },
    ))
}

pub(crate) fn semantic_synthesis_schedule_input_for_session(
    object: &WorkflowPackRuntimeObject,
    mut input: CreateSemanticSynthesisRun,
) -> CreateSemanticSynthesisRun {
    let mut metadata = input.metadata.as_object().cloned().unwrap_or_default();
    metadata.insert("source".to_string(), json!("semantic_synthesis_schedule"));
    metadata.insert("schedule_runtime_object_id".to_string(), json!(object.id));
    metadata.insert("object_key".to_string(), json!(object.object_key.clone()));
    if let Some(workflow_definition_id) = object.spec.get("workflow_definition_id") {
        metadata.insert(
            "workflow_definition_id".to_string(),
            workflow_definition_id.clone(),
        );
    }
    if let Some(workflow_id) = object.spec.get("workflow_id") {
        metadata.insert("workflow_id".to_string(), workflow_id.clone());
    }
    input.metadata = Value::Object(metadata);
    input
}

pub(crate) fn semantic_synthesis_schedule_string_array(spec: &Value, key: &str) -> Vec<String> {
    spec.get(key)
        .or_else(|| spec.pointer(&format!("/semantic_synthesis/{key}")))
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn semantic_synthesis_scheduled_run_skipped(
    object: &WorkflowPackRuntimeObject,
    session_id: Option<Uuid>,
    synthesis_type: Option<&str>,
    reason: &str,
) -> SemanticSynthesisScheduledRun {
    SemanticSynthesisScheduledRun {
        runtime_object_id: object.id,
        object_key: object.object_key.clone(),
        session_id,
        synthesis_type: synthesis_type.map(str::to_string),
        status: "skipped".to_string(),
        artifact_id: None,
        candidate_count: 0,
        reason: Some(reason.to_string()),
    }
}

pub(crate) fn semantic_synthesis_scheduled_run_failed(
    object: &WorkflowPackRuntimeObject,
    session_id: Option<Uuid>,
    synthesis_type: Option<&str>,
    reason: &str,
) -> SemanticSynthesisScheduledRun {
    SemanticSynthesisScheduledRun {
        runtime_object_id: object.id,
        object_key: object.object_key.clone(),
        session_id,
        synthesis_type: synthesis_type.map(str::to_string),
        status: "failed".to_string(),
        artifact_id: None,
        candidate_count: 0,
        reason: Some(reason.to_string()),
    }
}

pub(crate) async fn execute_due_semantic_aging_policies(
    state: &AppState,
    checked_at: DateTime<Utc>,
) -> Result<SemanticAgingPolicySweep, AppError> {
    let objects = state
        .list_workflow_pack_runtime_objects_by_runtime_kind("semantic_aging_policy")
        .await?;
    let audit_logs = state.list_audit_logs(None).await?;
    let mut policy_count = 0usize;
    let mut due_count = 0usize;
    let mut archived_count = 0usize;
    let mut skipped_count = 0usize;
    let failed_count = 0usize;
    let mut archived_object_ids = Vec::new();
    let mut runs = Vec::new();
    for object in objects {
        policy_count += 1;
        if !scheduled_runtime_object_is_runnable(&object, "semantic_aging_policy") {
            skipped_count += 1;
            runs.push(json!({
                "runtime_object_id": object.id,
                "object_key": object.object_key,
                "status": "skipped",
                "reason": "policy runtime object is not released or active",
            }));
            continue;
        }
        if !scheduled_runtime_object_is_due(
            &object,
            &audit_logs,
            checked_at,
            "semantic_aging.policy_run",
        )? {
            skipped_count += 1;
            runs.push(json!({
                "runtime_object_id": object.id,
                "object_key": object.object_key,
                "status": "skipped",
                "reason": "policy is not due or one-shot policy already ran",
            }));
            continue;
        }
        due_count += 1;
        let archive_stale = object
            .spec
            .get("archive_stale")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let query = SemanticProductQuery {
            q: None,
            object_type: object
                .spec
                .get("object_type")
                .and_then(Value::as_str)
                .map(str::to_string),
            domain_scope: object
                .spec
                .get("domain_scope")
                .and_then(Value::as_str)
                .map(str::to_string),
            workflow_scope: object
                .spec
                .get("workflow_scope")
                .and_then(Value::as_str)
                .map(str::to_string),
            memory_scope: object
                .spec
                .get("memory_scope")
                .and_then(Value::as_str)
                .map(str::to_string),
            status: Some("active".to_string()),
            trust_level: None,
            freshness: None,
            limit: None,
        };
        let stale_objects = state
            .list_semantic_objects()
            .await?
            .into_iter()
            .filter(|semantic_object| {
                semantic_object_matches_product_query(semantic_object, &query)
            })
            .filter(|semantic_object| semantic_object.freshness != "current")
            .collect::<Vec<_>>();
        let mut run_archived_ids = Vec::new();
        if archive_stale {
            for stale_object in stale_objects {
                let archived = state.archive_semantic_object(stale_object.id).await?;
                run_archived_ids.push(archived.id);
                archived_object_ids.push(archived.id);
                archived_count += 1;
            }
        }
        state
            .append_audit_log(new_audit_log(
                None,
                "system",
                Some(object.id),
                "semantic_aging.policy_run",
                "workflow_pack_runtime_object",
                Some(object.id),
                json!({
                    "runtime_object_id": object.id,
                    "object_key": object.object_key,
                    "checked_at": checked_at,
                    "archive_stale": archive_stale,
                    "archived_object_ids": run_archived_ids,
                }),
            ))
            .await?;
        runs.push(json!({
            "runtime_object_id": object.id,
            "object_key": object.object_key,
            "status": "processed",
            "archive_stale": archive_stale,
            "archived_object_ids": run_archived_ids,
        }));
    }
    let mut actions = Vec::new();
    if archived_count > 0 {
        actions.push("run_due_semantic_aging_policies".to_string());
    }
    let status = if failed_count > 0 && archived_count > 0 {
        "partial"
    } else if failed_count > 0 {
        "failed"
    } else if archived_count > 0 {
        "completed"
    } else if policy_count > 0 {
        "waiting"
    } else {
        "noop"
    }
    .to_string();
    Ok(SemanticAgingPolicySweep {
        status,
        checked_at,
        policy_count,
        due_count,
        archived_count,
        skipped_count,
        failed_count,
        archived_object_ids,
        runs,
        actions,
    })
}

pub(crate) fn scheduled_runtime_object_is_runnable(
    object: &WorkflowPackRuntimeObject,
    runtime_kind: &str,
) -> bool {
    object.object_type == "schedule"
        && object.runtime_kind == runtime_kind
        && matches!(object.status.as_str(), "released" | "active")
        && object
            .spec
            .pointer("/schedule_policy/enabled")
            .and_then(Value::as_bool)
            .unwrap_or(true)
}

pub(crate) fn scheduled_runtime_object_is_due(
    object: &WorkflowPackRuntimeObject,
    audit_logs: &[AuditLog],
    checked_at: DateTime<Utc>,
    success_action: &str,
) -> Result<bool, AppError> {
    if scheduled_runtime_object_due_at(object)?.unwrap_or(object.created_at) > checked_at {
        return Ok(false);
    }
    let one_shot = object
        .spec
        .pointer("/schedule_policy/one_shot")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if one_shot
        && audit_logs.iter().any(|log| {
            log.action == success_action
                && log
                    .details
                    .get("runtime_object_id")
                    .and_then(Value::as_str)
                    .and_then(|value| Uuid::parse_str(value).ok())
                    == Some(object.id)
        })
    {
        return Ok(false);
    }
    Ok(true)
}

pub(crate) fn scheduled_runtime_object_due_at(
    object: &WorkflowPackRuntimeObject,
) -> Result<Option<DateTime<Utc>>, AppError> {
    let Some(value) = object
        .spec
        .pointer("/schedule_policy/due_at")
        .or_else(|| object.spec.get("due_at"))
    else {
        return Ok(None);
    };
    let Some(raw) = value.as_str() else {
        return Err(AppError::bad_request(
            "schedule due_at must be an RFC3339 string",
        ));
    };
    DateTime::parse_from_rfc3339(raw)
        .map(|value| Some(value.with_timezone(&Utc)))
        .map_err(|_| AppError::bad_request("schedule due_at must be an RFC3339 string"))
}
