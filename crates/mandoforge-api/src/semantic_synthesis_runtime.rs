use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::*;

pub(crate) async fn materialize_semantic_synthesis_run(
    state: &AppState,
    session_id: Uuid,
    subject_id: String,
    input: CreateSemanticSynthesisRun,
) -> Result<SemanticSynthesisRunResult, AppError> {
    materialize_semantic_synthesis_run_for_actor(state, session_id, subject_id, "user", input).await
}

pub(crate) async fn materialize_semantic_synthesis_run_for_actor(
    state: &AppState,
    session_id: Uuid,
    subject_id: String,
    actor_type: &str,
    input: CreateSemanticSynthesisRun,
) -> Result<SemanticSynthesisRunResult, AppError> {
    let synthesis_type = normalize_semantic_synthesis_type(&input.synthesis_type)?;
    let goal_attempted =
        normalize_required_synthesis_text(&input.goal_attempted, "goal_attempted")?;
    if !input.metadata.is_object() {
        return Err(AppError::bad_request(
            "semantic synthesis metadata must be a JSON object",
        ));
    }
    ensure_memory_writeback_permitted_for_session(state, session_id, "semantic_synthesis").await?;
    let session = state.get_session(session_id).await?;
    let agent = state.get_agent(session.agent_id).await?;
    let events = state.list_events(session_id).await?;
    let checkpoint = semantic_synthesis_checkpoint_event(&events).ok_or_else(|| {
        AppError::bad_request(
            "semantic synthesis requires a completed session or idle managed-session checkpoint",
        )
    })?;
    let artifacts_before = state.list_artifacts(session_id).await?;
    let approvals = state
        .list_approvals()
        .await?
        .into_iter()
        .filter(|approval| approval.session_id == session_id)
        .collect::<Vec<_>>();
    let context_packets = state.list_context_packets(session_id).await?;
    let handoffs = state
        .list_agent_handoff_events(Some(session_id))
        .await?
        .into_iter()
        .filter(|handoff| handoff.status == "completed")
        .collect::<Vec<_>>();
    let now = Utc::now();
    let artifact_type = semantic_synthesis_artifact_type(&synthesis_type);
    let candidate_type = semantic_synthesis_candidate_type(&synthesis_type);
    let report = json!({
        "synthesis_type": synthesis_type,
        "session_id": session_id,
        "agent_id": agent.id,
        "goal_attempted": goal_attempted,
        "context_used": input.context_used,
        "worked": input.worked,
        "failed_or_corrected": input.failed_or_corrected,
        "unsafe_assumptions": input.unsafe_assumptions,
        "metadata": input.metadata,
        "checkpoint": {
            "event_id": checkpoint.id,
            "event_type": checkpoint.event_type,
            "created_at": checkpoint.created_at
        },
        "evidence_counts": {
            "event_count": events.len(),
            "artifact_count": artifacts_before.len(),
            "approval_count": approvals.len(),
            "context_packet_count": context_packets.len(),
            "completed_handoff_count": handoffs.len()
        },
        "candidate_count": input.durable_memory_candidates.len(),
        "created_by": subject_id,
        "created_at": now
    });
    let artifact = Artifact {
        id: Uuid::new_v4(),
        session_id,
        artifact_type: artifact_type.to_string(),
        name: semantic_synthesis_artifact_name(&synthesis_type).to_string(),
        path: Some(format!(
            "semantic-synthesis/{}/{}.json",
            synthesis_type, session_id
        )),
        content: report,
        created_at: now,
    };
    let artifact = state.insert_artifact(artifact).await?;
    state
        .append_event(
            "system",
            Some(artifact.id),
            session_id,
            "artifact.created",
            json!({
                "artifact_id": artifact.id,
                "artifact_type": artifact.artifact_type,
                "name": artifact.name,
                "path": artifact.path,
                "source": "semantic_synthesis"
            }),
        )
        .await?;

    let mut candidates = Vec::new();
    for (index, candidate_input) in input.durable_memory_candidates.into_iter().enumerate() {
        let candidate_event = state
            .append_event(
                "system",
                Some(artifact.id),
                session_id,
                "semantic_synthesis.candidate_proposed",
                json!({
                    "artifact_id": artifact.id,
                    "synthesis_type": synthesis_type,
                    "candidate_type": candidate_type,
                    "candidate_index": index,
                    "proposed_object_key": candidate_input.proposed_object_key,
                }),
            )
            .await?;
        let candidate = semantic_synthesis_memory_candidate(
            session_id,
            &agent,
            &synthesis_type,
            candidate_type,
            &artifact,
            &candidate_event,
            candidate_input,
            now,
        )?;
        let candidate = state.create_memory_writeback_candidate(candidate).await?;
        let audit = record_memory_writeback_candidate_created(state, &candidate).await?;
        let candidate = state
            .update_memory_writeback_candidate_audit_trace(candidate.id, audit.id)
            .await?;
        candidates.push(candidate);
    }

    state
        .append_event(
            "system",
            Some(artifact.id),
            session_id,
            "semantic_synthesis.run_created",
            json!({
                "synthesis_type": synthesis_type,
                "artifact_id": artifact.id,
                "candidate_count": candidates.len(),
                "candidate_ids": candidates.iter().map(|candidate| candidate.id).collect::<Vec<_>>(),
                "checkpoint_event_id": checkpoint.id,
            }),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(session_id),
            actor_type,
            None,
            "semantic_synthesis.run_created",
            "artifact",
            Some(artifact.id),
            json!({
                "synthesis_type": synthesis_type,
                "session_id": session_id,
                "artifact_id": artifact.id,
                "candidate_count": candidates.len(),
                "candidate_ids": candidates.iter().map(|candidate| candidate.id).collect::<Vec<_>>(),
                "checkpoint_event_id": checkpoint.id,
                "subject": subject_id,
            }),
        ))
        .await?;

    Ok(SemanticSynthesisRunResult {
        status: "created".to_string(),
        synthesis_type,
        session_id,
        checkpoint_event_id: checkpoint.id,
        artifact,
        candidates,
        created_at: now,
    })
}

pub(crate) async fn ensure_memory_writeback_permitted_for_session(
    state: &AppState,
    session_id: Uuid,
    tool: &str,
) -> Result<(), AppError> {
    if let Some((run, grant)) = governing_task_grant_for_memory_writeback(state, session_id).await?
    {
        if !task_grant_memory_scope_allows_writeback(&grant.memory_scope) {
            let reason = "task grant memory scope does not allow memory writeback";
            record_task_grant_denied(state, session_id, Some(&grant), Some(run.id), tool, reason)
                .await?;
            return Err(AppError::forbidden(reason));
        }
        record_task_grant_checked(state, &grant, session_id, tool).await?;
    }
    Ok(())
}

pub(crate) fn semantic_synthesis_memory_candidate(
    session_id: Uuid,
    agent: &Agent,
    synthesis_type: &str,
    candidate_type: &str,
    artifact: &Artifact,
    candidate_event: &SessionEvent,
    input: SemanticSynthesisMemoryCandidateInput,
    created_at: DateTime<Utc>,
) -> Result<MemoryWritebackCandidate, AppError> {
    let proposed_object_type =
        normalize_required_synthesis_text(&input.proposed_object_type, "proposed_object_type")?;
    let proposed_object_key =
        normalize_required_synthesis_text(&input.proposed_object_key, "proposed_object_key")?;
    let title = normalize_required_synthesis_text(&input.title, "title")?;
    let summary = normalize_required_synthesis_text(&input.summary, "summary")?;
    if !input.content.is_object() {
        return Err(AppError::bad_request(
            "semantic synthesis candidate content must be a JSON object",
        ));
    }
    if !input.semantic_scopes.is_object() {
        return Err(AppError::bad_request(
            "semantic synthesis candidate semantic_scopes must be a JSON object",
        ));
    }
    if !input.provenance.is_object() {
        return Err(AppError::bad_request(
            "semantic synthesis candidate provenance must be a JSON object",
        ));
    }
    let semantic_scopes = if input
        .semantic_scopes
        .as_object()
        .is_none_or(|object| object.is_empty())
    {
        agent.semantic_scopes.clone()
    } else {
        input.semantic_scopes
    };
    let source_refs =
        semantic_synthesis_source_refs(&input.source_refs, artifact, candidate_event)?;
    Ok(MemoryWritebackCandidate {
        id: Uuid::new_v4(),
        session_id,
        candidate_type: candidate_type.to_string(),
        source_event_id: Some(candidate_event.id),
        source_artifact_id: Some(artifact.id),
        source_approval_id: None,
        source_handoff_id: None,
        proposed_object_type,
        proposed_object_key,
        title,
        summary,
        content: input.content,
        semantic_scopes,
        source_refs,
        provenance: merge_semantic_synthesis_provenance(
            input.provenance,
            synthesis_type,
            artifact,
            candidate_event,
        ),
        trust_level: normalize_semantic_synthesis_trust_level(&input.trust_level)?,
        freshness: normalize_semantic_synthesis_freshness(&input.freshness)?,
        status: "pending".to_string(),
        reviewer_subject: None,
        review_reason: None,
        semantic_object_id: None,
        audit_trace_id: None,
        created_at,
        updated_at: created_at,
        decided_at: None,
    })
}

pub(crate) fn semantic_synthesis_source_refs(
    input_source_refs: &Value,
    artifact: &Artifact,
    candidate_event: &SessionEvent,
) -> Result<Value, AppError> {
    if !input_source_refs.is_object() && !input_source_refs.is_array() {
        return Err(AppError::bad_request(
            "semantic synthesis candidate source_refs must be a JSON object or array",
        ));
    }
    let mut refs = Vec::new();
    refs.push(json!({"source_type": "artifact", "source_id": artifact.id}));
    refs.push(json!({"source_type": "session_event", "source_id": candidate_event.id}));
    if let Some(array) = input_source_refs.as_array() {
        refs.extend(array.iter().cloned());
    } else if input_source_refs
        .as_object()
        .is_some_and(|object| !object.is_empty())
    {
        refs.push(input_source_refs.clone());
    }
    Ok(Value::Array(refs))
}

pub(crate) fn merge_semantic_synthesis_provenance(
    provenance: Value,
    synthesis_type: &str,
    artifact: &Artifact,
    candidate_event: &SessionEvent,
) -> Value {
    let mut object = provenance.as_object().cloned().unwrap_or_default();
    object.insert(
        "source".to_string(),
        Value::String("semantic_synthesis".to_string()),
    );
    object.insert(
        "synthesis_type".to_string(),
        Value::String(synthesis_type.to_string()),
    );
    object.insert("artifact_id".to_string(), json!(artifact.id));
    object.insert("candidate_event_id".to_string(), json!(candidate_event.id));
    Value::Object(object)
}

pub(crate) fn semantic_synthesis_checkpoint_event(
    events: &[SessionEvent],
) -> Option<&SessionEvent> {
    events.iter().rev().find(|event| {
        matches!(
            event.event_type.as_str(),
            "session.loop.idle"
                | "session.completed"
                | "session.goal.completed"
                | "workflow.run.completed"
                | "agent_handoff.completed"
        )
    })
}

pub(crate) fn normalize_semantic_synthesis_type(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "post_run_reflection" | "session_reflection" | "reflection" => {
            Ok("post_run_reflection".to_string())
        }
        "dreaming_synthesis" | "dreaming" | "dream" => Ok("dreaming_synthesis".to_string()),
        _ => Err(AppError::bad_request(
            "semantic synthesis type must be post_run_reflection or dreaming_synthesis",
        )),
    }
}

pub(crate) fn semantic_synthesis_artifact_type(synthesis_type: &str) -> &'static str {
    match synthesis_type {
        "dreaming_synthesis" => "semantic_dreaming_report",
        _ => "semantic_reflection_report",
    }
}

pub(crate) fn semantic_synthesis_artifact_name(synthesis_type: &str) -> &'static str {
    match synthesis_type {
        "dreaming_synthesis" => "semantic-dreaming-report.json",
        _ => "semantic-reflection-report.json",
    }
}

pub(crate) fn semantic_synthesis_candidate_type(synthesis_type: &str) -> &'static str {
    match synthesis_type {
        "dreaming_synthesis" => "dreaming_synthesis",
        _ => "session_reflection",
    }
}

pub(crate) fn normalize_required_synthesis_text(
    value: &str,
    label: &str,
) -> Result<String, AppError> {
    let text = value.trim();
    if text.is_empty() {
        return Err(AppError::bad_request(format!(
            "semantic synthesis {label} cannot be empty"
        )));
    }
    Ok(text.to_string())
}

pub(crate) fn normalize_semantic_synthesis_trust_level(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "unverified" | "source_attested" | "human_verified" | "system_verified" => Ok(normalized),
        _ => Err(AppError::bad_request(
            "semantic synthesis trust_level must be unverified, source_attested, human_verified, or system_verified",
        )),
    }
}

pub(crate) fn normalize_semantic_synthesis_freshness(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "unknown" | "current" | "stale" | "expired" => Ok(normalized),
        _ => Err(AppError::bad_request(
            "semantic synthesis freshness must be unknown, current, stale, or expired",
        )),
    }
}

pub(crate) async fn generate_and_persist_context_packet(
    state: &AppState,
    session_id: Uuid,
) -> Result<ContextPacket, AppError> {
    let packet = build_context_packet(state, session_id).await?;
    let packet = state.create_context_packet(packet).await?;
    let event_details = context_packet_replay_details(&packet);
    state
        .append_event(
            "system",
            Some(packet.id),
            session_id,
            "context_packet.generated",
            event_details.clone(),
        )
        .await?;
    let audit = state
        .append_audit_log(new_audit_log(
            Some(session_id),
            "system",
            Some(packet.id),
            "context_packet.generated",
            "context_packet",
            Some(packet.id),
            event_details,
        ))
        .await?;
    state
        .update_context_packet_audit_trace(packet.id, audit.id)
        .await
}

pub(crate) async fn generate_memory_writeback_candidates(
    state: &AppState,
    session_id: Uuid,
    input: CreateMemoryWritebackCandidates,
) -> Result<Vec<MemoryWritebackCandidate>, AppError> {
    let session = state.get_session(session_id).await?;
    ensure_memory_writeback_permitted_for_session(state, session_id, "memory_writeback").await?;
    let events = state.list_events(session_id).await?;
    let session_reached_checkpoint = events.iter().any(|event| {
        matches!(
            event.event_type.as_str(),
            "session.loop.idle" | "session.completed"
        )
    });
    if !session_reached_checkpoint {
        return Err(AppError::bad_request(
            "memory writeback candidates require a completed session or idle managed-session checkpoint",
        ));
    }
    let agent = state.get_agent(session.agent_id).await?;
    let artifacts = state.list_artifacts(session_id).await?;
    let approvals = state
        .list_approvals()
        .await?
        .into_iter()
        .filter(|approval| approval.session_id == session_id)
        .collect::<Vec<_>>();
    let handoffs = state
        .list_agent_handoff_events(Some(session_id))
        .await?
        .into_iter()
        .filter(|handoff| handoff.status == "completed")
        .collect::<Vec<_>>();
    let include_session_summary = input.include_session_summary.unwrap_or(true);
    let include_artifacts = input.include_artifacts.unwrap_or(true);
    let include_handoffs = input.include_handoffs.unwrap_or(true);
    let include_approvals = input.include_approvals.unwrap_or(true);

    let mut proposed = Vec::new();
    if include_session_summary
        && let Some(event) = events.iter().rev().find(|event| {
            matches!(
                event.event_type.as_str(),
                "session.loop.idle" | "session.completed"
            )
        })
    {
        proposed.push(memory_candidate_from_session(
            &session, &agent, event, &events, &artifacts, &approvals, &handoffs,
        ));
    }
    if include_artifacts {
        for artifact in &artifacts {
            proposed.push(memory_candidate_from_artifact(&session, &agent, artifact));
        }
    }
    if include_handoffs {
        for handoff in &handoffs {
            proposed.push(memory_candidate_from_handoff(&session, &agent, handoff));
        }
    }
    if include_approvals {
        for approval in approvals
            .iter()
            .filter(|approval| matches!(approval.status.as_str(), "approved" | "rejected"))
        {
            proposed.push(memory_candidate_from_approval(&session, &agent, approval));
        }
    }

    let mut created = Vec::new();
    for candidate in proposed {
        match state.create_memory_writeback_candidate(candidate).await {
            Ok(candidate) => {
                let audit = record_memory_writeback_candidate_created(state, &candidate).await?;
                let candidate = state
                    .update_memory_writeback_candidate_audit_trace(candidate.id, audit.id)
                    .await?;
                created.push(candidate);
            }
            Err(error) if error.message.contains("already exists") => {}
            Err(error) => return Err(error),
        }
    }
    state
        .append_event(
            "system",
            None,
            session_id,
            "memory_writeback.candidates_generated",
            json!({
                "session_id": session_id,
                "candidate_count": created.len(),
                "candidate_ids": created.iter().map(|candidate| candidate.id).collect::<Vec<_>>(),
            }),
        )
        .await?;
    Ok(created)
}

pub(crate) fn task_grant_memory_scope_allows_writeback(memory_scope: &Value) -> bool {
    memory_scope
        .get("writeback_allowed")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub(crate) fn memory_candidate_from_session(
    session: &Session,
    agent: &Agent,
    event: &SessionEvent,
    events: &[SessionEvent],
    artifacts: &[Artifact],
    approvals: &[Approval],
    handoffs: &[AgentHandoffEvent],
) -> MemoryWritebackCandidate {
    let created_at = Utc::now();
    MemoryWritebackCandidate {
        id: Uuid::new_v4(),
        session_id: session.id,
        candidate_type: "session_summary".to_string(),
        source_event_id: Some(event.id),
        source_artifact_id: None,
        source_approval_id: None,
        source_handoff_id: None,
        proposed_object_type: "memory".to_string(),
        proposed_object_key: format!("memory:session:{}:summary", session.id),
        title: format!("Session memory: {}", session.title),
        summary: format!(
            "Completed session produced {} events, {} artifacts, {} reviewed approvals, and {} completed handoffs.",
            events.len(),
            artifacts.len(),
            approvals
                .iter()
                .filter(|approval| approval.status != "pending")
                .count(),
            handoffs.len()
        ),
        content: json!({
            "session_id": session.id,
            "title": session.title,
            "status": session.status.as_str(),
            "event_count": events.len(),
            "artifact_names": artifacts.iter().map(|artifact| artifact.name.clone()).collect::<Vec<_>>(),
            "approval_decisions": approvals.iter().map(|approval| {
                json!({
                    "approval_id": approval.id,
                    "action": approval.action,
                    "status": approval.status,
                    "risk_level": approval.risk_level,
                })
            }).collect::<Vec<_>>(),
            "completed_handoff_ids": handoffs.iter().map(|handoff| handoff.id).collect::<Vec<_>>(),
        }),
        semantic_scopes: agent.semantic_scopes.clone(),
        source_refs: json!([{"source_type": "session_event", "source_id": event.id}]),
        provenance: json!({
            "source": "session.completed",
            "session_id": session.id,
            "agent_id": agent.id,
            "observed_at": event.created_at,
        }),
        trust_level: "source_attested".to_string(),
        freshness: "current".to_string(),
        status: "pending".to_string(),
        reviewer_subject: None,
        review_reason: None,
        semantic_object_id: None,
        audit_trace_id: None,
        created_at,
        updated_at: created_at,
        decided_at: None,
    }
}

pub(crate) fn memory_candidate_from_artifact(
    session: &Session,
    agent: &Agent,
    artifact: &Artifact,
) -> MemoryWritebackCandidate {
    let created_at = Utc::now();
    MemoryWritebackCandidate {
        id: Uuid::new_v4(),
        session_id: session.id,
        candidate_type: "artifact".to_string(),
        source_event_id: None,
        source_artifact_id: Some(artifact.id),
        source_approval_id: None,
        source_handoff_id: None,
        proposed_object_type: "memory".to_string(),
        proposed_object_key: format!("memory:artifact:{}", artifact.id),
        title: format!("Artifact memory: {}", artifact.name),
        summary: format!(
            "Artifact {} of type {} was created during completed session {}.",
            artifact.name, artifact.artifact_type, session.id
        ),
        content: json!({
            "artifact_id": artifact.id,
            "artifact_type": artifact.artifact_type,
            "name": artifact.name,
            "path": artifact.path,
            "content": artifact.content,
        }),
        semantic_scopes: agent.semantic_scopes.clone(),
        source_refs: json!([{"source_type": "artifact", "source_id": artifact.id}]),
        provenance: json!({
            "source": "artifact.created",
            "session_id": session.id,
            "agent_id": agent.id,
            "observed_at": artifact.created_at,
        }),
        trust_level: "source_attested".to_string(),
        freshness: "current".to_string(),
        status: "pending".to_string(),
        reviewer_subject: None,
        review_reason: None,
        semantic_object_id: None,
        audit_trace_id: None,
        created_at,
        updated_at: created_at,
        decided_at: None,
    }
}

pub(crate) fn memory_candidate_from_handoff(
    session: &Session,
    agent: &Agent,
    handoff: &AgentHandoffEvent,
) -> MemoryWritebackCandidate {
    let created_at = Utc::now();
    MemoryWritebackCandidate {
        id: Uuid::new_v4(),
        session_id: session.id,
        candidate_type: "handoff_review".to_string(),
        source_event_id: None,
        source_artifact_id: None,
        source_approval_id: None,
        source_handoff_id: Some(handoff.id),
        proposed_object_type: "memory".to_string(),
        proposed_object_key: format!("memory:handoff:{}", handoff.id),
        title: format!("Handoff memory: {}", handoff.intent),
        summary: format!(
            "Completed handoff {} delegated intent {} from manager agent {} to specialist agent {}.",
            handoff.id, handoff.intent, handoff.source_agent_id, handoff.target_agent_id
        ),
        content: json!({
            "agent_handoff_event_id": handoff.id,
            "manager_plan_id": handoff.manager_plan_id,
            "intent": handoff.intent,
            "payload": handoff.payload,
            "review_status": handoff.review_status,
            "human_escalation_status": handoff.human_escalation_status,
            "risk_level": handoff.risk_level,
        }),
        semantic_scopes: merge_semantic_scopes(&agent.semantic_scopes, &handoff.semantic_scopes),
        source_refs: json!([{"source_type": "agent_handoff", "source_id": handoff.id}]),
        provenance: json!({
            "source": "agent_handoff.completed",
            "session_id": session.id,
            "agent_id": agent.id,
            "observed_at": handoff.updated_at,
        }),
        trust_level: "source_attested".to_string(),
        freshness: "current".to_string(),
        status: "pending".to_string(),
        reviewer_subject: None,
        review_reason: None,
        semantic_object_id: None,
        audit_trace_id: None,
        created_at,
        updated_at: created_at,
        decided_at: None,
    }
}

pub(crate) fn memory_candidate_from_approval(
    session: &Session,
    agent: &Agent,
    approval: &Approval,
) -> MemoryWritebackCandidate {
    let created_at = Utc::now();
    MemoryWritebackCandidate {
        id: Uuid::new_v4(),
        session_id: session.id,
        candidate_type: "approval_decision".to_string(),
        source_event_id: None,
        source_artifact_id: None,
        source_approval_id: Some(approval.id),
        source_handoff_id: None,
        proposed_object_type: "memory".to_string(),
        proposed_object_key: format!("memory:approval:{}", approval.id),
        title: format!("Approval memory: {}", approval.action),
        summary: format!(
            "Approval {} for action {} was decided as {} with {} risk.",
            approval.id, approval.action, approval.status, approval.risk_level
        ),
        content: json!({
            "approval_id": approval.id,
            "tool_call_id": approval.tool_call_id,
            "action": approval.action,
            "risk_level": approval.risk_level,
            "reason": approval.reason,
            "evidence": approval.evidence,
            "decision_payload": approval.decision_payload,
            "status": approval.status,
            "decided_at": approval.decided_at,
        }),
        semantic_scopes: agent.semantic_scopes.clone(),
        source_refs: json!([{"source_type": "approval", "source_id": approval.id}]),
        provenance: json!({
            "source": format!("approval.{}", approval.status),
            "session_id": session.id,
            "agent_id": agent.id,
            "observed_at": approval.decided_at,
        }),
        trust_level: "source_attested".to_string(),
        freshness: "current".to_string(),
        status: "pending".to_string(),
        reviewer_subject: None,
        review_reason: None,
        semantic_object_id: None,
        audit_trace_id: None,
        created_at,
        updated_at: created_at,
        decided_at: None,
    }
}

pub(crate) async fn record_memory_writeback_candidate_created(
    state: &AppState,
    candidate: &MemoryWritebackCandidate,
) -> Result<AuditLog, AppError> {
    state
        .append_audit_log(new_audit_log(
            Some(candidate.session_id),
            "system",
            Some(candidate.id),
            "memory_writeback.candidate_created",
            "memory_writeback_candidate",
            Some(candidate.id),
            json!({
                "session_id": candidate.session_id,
                "candidate_type": candidate.candidate_type,
                "proposed_object_key": candidate.proposed_object_key,
                "source_event_id": candidate.source_event_id,
                "source_artifact_id": candidate.source_artifact_id,
                "source_approval_id": candidate.source_approval_id,
                "source_handoff_id": candidate.source_handoff_id,
                "status": candidate.status,
            }),
        ))
        .await
}

pub(crate) async fn record_memory_writeback_candidate_review(
    state: &AppState,
    candidate: &MemoryWritebackCandidate,
    action: &str,
) -> Result<(), AppError> {
    state
        .append_event(
            "user",
            Some(candidate.id),
            candidate.session_id,
            action,
            json!({
                "candidate_id": candidate.id,
                "status": candidate.status,
                "reviewer_subject": candidate.reviewer_subject,
                "review_reason": candidate.review_reason,
                "semantic_object_id": candidate.semantic_object_id,
            }),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(candidate.session_id),
            "user",
            Some(candidate.id),
            action,
            "memory_writeback_candidate",
            Some(candidate.id),
            json!({
                "candidate_id": candidate.id,
                "status": candidate.status,
                "reviewer_subject": candidate.reviewer_subject,
                "review_reason": candidate.review_reason,
                "semantic_object_id": candidate.semantic_object_id,
            }),
        ))
        .await?;
    Ok(())
}
