use anyhow::Result;
use chrono::Utc;
use serde_json::Value;
use uuid::Uuid;

use crate::store_audit::validate_idempotent_audit_identity;
use crate::store_backend::StoreBackend;
use crate::store_events::{
    POSTGRES_SESSION_EVENT_CHANNEL, session_event_notify_payload,
    validate_idempotent_event_identity,
};
use crate::store_rows::{
    approval_commit_token_from_row, approval_from_row, audit_log_from_row, event_from_row,
    task_grant_from_row, tool_call_from_row,
};
use crate::store_workflows::TASK_GRANT_COLUMNS;
use crate::{
    AppError, AppState, Approval, ApprovalCommitToken, AuditLog, SessionEvent, ToolCall,
    approval_commit_binding_for_args, deterministic_record_id, new_audit_log,
    ontology_action_name_from_args, task_grant_requires_approval_commit_token,
};

fn validate_modified_tool_args(
    tool_call: &ToolCall,
    args: &Value,
    commit_binding_required: bool,
) -> Result<(Option<String>, Value), AppError> {
    if tool_call.status != "waiting_approval" {
        return Err(AppError::bad_request(
            "only waiting approval tool calls can be modified",
        ));
    }
    if tool_call.tool_name == "ontology.action.execute"
        && ontology_action_name_from_args(&tool_call.args)? != ontology_action_name_from_args(args)?
    {
        return Err(AppError::forbidden(
            "ontology action identity cannot be changed after approval is requested",
        ));
    }
    if commit_binding_required {
        let binding = approval_commit_binding_for_args(&tool_call.tool_name, args)?;
        Ok((Some(binding.normalized_args_hash), binding.target_binding))
    } else {
        Ok((None, tool_call.target_binding.clone()))
    }
}

fn declined_approval_event(
    id: Uuid,
    approval: &Approval,
    actor_type: &str,
    actor_id: Option<Uuid>,
    event_type: &str,
    payload: Value,
) -> SessionEvent {
    SessionEvent {
        id,
        session_id: approval.session_id,
        seq: 0,
        parent_event_id: None,
        actor_type: actor_type.to_string(),
        actor_id,
        event_type: event_type.to_string(),
        payload,
        created_at: Utc::now(),
    }
}

fn declined_approval_records(
    approval: &Approval,
    decision: &str,
    actor_type: &str,
    tool_call: Option<&ToolCall>,
) -> (Value, Vec<SessionEvent>, Vec<AuditLog>) {
    let result = serde_json::json!({
        "status": "denied",
        "approval": decision,
        "approval_id": approval.id,
        "reason": approval.reason,
    });
    let mut events = Vec::with_capacity(if tool_call.is_some() { 3 } else { 1 });
    let mut audits = Vec::with_capacity(if tool_call.is_some() { 2 } else { 1 });
    if let Some(tool_call) = tool_call.filter(|tool_call| {
        tool_call.status == "waiting_approval"
            || (tool_call.status == "denied" && tool_call.result.as_ref() == Some(&result))
    }) {
        events.push(declined_approval_event(
            deterministic_record_id(
                approval.id,
                "approval-tool-result-event",
                &[decision, "tool.result"],
            ),
            approval,
            "tool",
            Some(tool_call.id),
            "tool.result",
            serde_json::json!({
                "tool_call_id": tool_call.id,
                "tool": tool_call.tool_name,
                "content": result,
            }),
        ));
        events.push(declined_approval_event(
            deterministic_record_id(
                approval.id,
                "approval-tool-result-event",
                &[decision, "agent.tool_result"],
            ),
            approval,
            "agent",
            Some(tool_call.id),
            "agent.tool_result",
            serde_json::json!({
                "tool_call_id": tool_call.id,
                "tool": tool_call.tool_name,
                "status": "denied",
                "content": result,
            }),
        ));
        let mut audit = new_audit_log(
            Some(approval.session_id),
            "tool",
            Some(tool_call.id),
            "tool.denied",
            "tool_call",
            Some(tool_call.id),
            serde_json::json!({
                "tool": tool_call.tool_name,
                "approval_id": approval.id,
                "decision": decision,
                "status": "denied",
            }),
        );
        audit.id = deterministic_record_id(
            approval.id,
            "approval-tool-result-audit",
            &[decision, "tool.denied"],
        );
        audits.push(audit);
    }
    let event_type = format!("approval.{decision}");
    let event_id = deterministic_record_id(approval.id, "approval-event", &[decision]);
    events.push(declined_approval_event(
        event_id,
        approval,
        actor_type,
        Some(approval.id),
        &event_type,
        serde_json::json!({
            "approval_id": approval.id,
            "decision": decision,
            "expires_at": approval.expires_at,
        }),
    ));
    let mut audit = new_audit_log(
        Some(approval.session_id),
        actor_type,
        Some(approval.id),
        &event_type,
        "approval",
        Some(approval.id),
        serde_json::json!({
            "tool_call_id": approval.tool_call_id,
            "decision": decision,
            "expires_at": approval.expires_at,
        }),
    );
    audit.id = deterministic_record_id(event_id, "audit", &[event_type.as_str()]);
    audits.push(audit);
    (result, events, audits)
}

impl AppState {
    pub(crate) async fn insert_approval(&self, approval: Approval) -> Result<Approval, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .approvals
                    .insert(approval.id, approval.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO approvals (id, tenant_id, session_id, tool_call_id, action, risk_level, reason, evidence, decision_payload, status, expires_at, created_at, decided_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
                )
                .bind(approval.id)
                .bind(self.current_tenant_id())
                .bind(approval.session_id)
                .bind(approval.tool_call_id)
                .bind(&approval.action)
                .bind(&approval.risk_level)
                .bind(&approval.reason)
                .bind(&approval.evidence)
                .bind(&approval.decision_payload)
                .bind(&approval.status)
                .bind(approval.expires_at)
                .bind(approval.created_at)
                .bind(approval.decided_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(approval)
    }

    pub(crate) async fn create_approval_commit_token(
        &self,
        token: ApprovalCommitToken,
    ) -> Result<ApprovalCommitToken, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .approval_commit_tokens
                    .insert(token.id, token.clone());
                Ok(token)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "INSERT INTO approval_commit_tokens
                        (id, tenant_id, approval_id, tool_call_id, task_grant_id, session_id, tool_name, normalized_args_hash, target_binding, approver_subject, status, expires_at, consumed_at, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                     RETURNING id, approval_id, tool_call_id, task_grant_id, session_id, tool_name, normalized_args_hash, target_binding, approver_subject, status, expires_at, consumed_at, created_at",
                )
                .bind(token.id)
                .bind(self.current_tenant_id())
                .bind(token.approval_id)
                .bind(token.tool_call_id)
                .bind(token.task_grant_id)
                .bind(token.session_id)
                .bind(&token.tool_name)
                .bind(&token.normalized_args_hash)
                .bind(&token.target_binding)
                .bind(&token.approver_subject)
                .bind(&token.status)
                .bind(token.expires_at)
                .bind(token.consumed_at)
                .bind(token.created_at)
                .fetch_one(pool)
                .await?;
                approval_commit_token_from_row(row)
            }
        }
    }

    pub(crate) async fn approval_commit_token_for_approval(
        &self,
        approval_id: Uuid,
    ) -> Result<Option<ApprovalCommitToken>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => Ok(inner
                .read()
                .await
                .approval_commit_tokens
                .values()
                .find(|token| token.approval_id == approval_id)
                .cloned()),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, approval_id, tool_call_id, task_grant_id, session_id, tool_name, normalized_args_hash, target_binding, approver_subject, status, expires_at, consumed_at, created_at
                     FROM approval_commit_tokens
                     WHERE tenant_id = $1 AND approval_id = $2",
                )
                .bind(self.current_tenant_id())
                .bind(approval_id)
                .fetch_optional(pool)
                .await?;
                row.map(approval_commit_token_from_row).transpose()
            }
        }
    }

    pub(crate) async fn consume_approval_commit_token(
        &self,
        token_id: Uuid,
    ) -> Result<ApprovalCommitToken, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let token = store
                    .approval_commit_tokens
                    .get_mut(&token_id)
                    .ok_or_else(|| AppError::not_found("approval commit token not found"))?;
                token.status = "consumed".to_string();
                token.consumed_at = Some(Utc::now());
                Ok(token.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE approval_commit_tokens
                     SET status = 'consumed', consumed_at = now()
                     WHERE tenant_id = $1 AND id = $2
                     RETURNING id, approval_id, tool_call_id, task_grant_id, session_id, tool_name, normalized_args_hash, target_binding, approver_subject, status, expires_at, consumed_at, created_at",
                )
                .bind(self.current_tenant_id())
                .bind(token_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("approval commit token not found"))?;
                approval_commit_token_from_row(row)
            }
        }
    }

    pub(crate) async fn list_approvals(&self) -> Result<Vec<Approval>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                Ok(inner.read().await.approvals.values().cloned().collect())
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, session_id, tool_call_id, action, risk_level, reason, evidence, decision_payload, status, expires_at, created_at, decided_at
                     FROM approvals
                     WHERE tenant_id = $1
                     ORDER BY created_at DESC",
                )
                .bind(self.current_tenant_id())
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(approval_from_row).collect()
            }
        }
    }

    pub(crate) async fn list_approvals_for_due_run(&self) -> Result<Vec<Approval>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let store = inner.read().await;
                Ok(store
                    .approvals
                    .values()
                    .filter(|approval| {
                        approval.status == "pending"
                            || (approval.status == "expired"
                                && approval.tool_call_id.is_some_and(|tool_call_id| {
                                    store
                                        .tool_calls
                                        .get(&tool_call_id)
                                        .is_some_and(|tool_call| {
                                            tool_call.status == "waiting_approval"
                                        })
                                }))
                    })
                    .cloned()
                    .collect())
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT a.id, a.session_id, a.tool_call_id, a.action, a.risk_level, a.reason, a.evidence, a.decision_payload, a.status, a.expires_at, a.created_at, a.decided_at
                     FROM approvals a
                     LEFT JOIN tool_calls t
                       ON t.tenant_id = a.tenant_id AND t.id = a.tool_call_id
                     WHERE a.tenant_id = $1
                       AND (a.status = 'pending'
                            OR (a.status = 'expired' AND t.status = 'waiting_approval'))
                     ORDER BY a.created_at DESC",
                )
                .bind(self.current_tenant_id())
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(approval_from_row).collect()
            }
        }
    }

    pub(crate) async fn get_approval(&self, approval_id: Uuid) -> Result<Approval, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .approvals
                .get(&approval_id)
                .cloned()
                .ok_or_else(|| AppError::not_found("approval not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, session_id, tool_call_id, action, risk_level, reason, evidence, decision_payload, status, expires_at, created_at, decided_at
                     FROM approvals
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.current_tenant_id())
                .bind(approval_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("approval not found"))?;
                approval_from_row(row)
            }
        }
    }

    pub(crate) async fn decide_approval(
        &self,
        approval_id: Uuid,
        status: &str,
    ) -> Result<Approval, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let approval = store
                    .approvals
                    .get_mut(&approval_id)
                    .ok_or_else(|| AppError::not_found("approval not found"))?;
                if approval.status != "pending" {
                    return Err(AppError::bad_request(
                        "only pending approvals can be decided",
                    ));
                }
                approval.status = status.to_string();
                approval.decided_at = Some(Utc::now());
                Ok(approval.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE approvals
                     SET status = $1, decided_at = now()
                     WHERE tenant_id = $2 AND id = $3 AND status = 'pending'
                     RETURNING id, session_id, tool_call_id, action, risk_level, reason, evidence, decision_payload, status, expires_at, created_at, decided_at",
                )
                .bind(status)
                .bind(self.current_tenant_id())
                .bind(approval_id)
                .fetch_optional(pool)
                .await?;
                match row {
                    Some(row) => approval_from_row(row),
                    None => {
                        let exists = sqlx::query_scalar::<_, bool>(
                            "SELECT EXISTS(
                                 SELECT 1 FROM approvals WHERE tenant_id = $1 AND id = $2
                             )",
                        )
                        .bind(self.current_tenant_id())
                        .bind(approval_id)
                        .fetch_one(pool)
                        .await?;
                        if exists {
                            Err(AppError::bad_request(
                                "only pending approvals can be decided",
                            ))
                        } else {
                            Err(AppError::not_found("approval not found"))
                        }
                    }
                }
            }
        }
    }

    pub(crate) async fn decline_approval_and_tool_call(
        &self,
        approval_id: Uuid,
        decision: &str,
        actor_type: &str,
    ) -> Result<(Approval, Option<ToolCall>, Vec<SessionEvent>), AppError> {
        if !matches!(decision, "rejected" | "expired") {
            return Err(AppError::bad_request(
                "declined approval decision must be rejected or expired",
            ));
        }
        let committed = match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let current = store
                    .approvals
                    .get(&approval_id)
                    .cloned()
                    .ok_or_else(|| AppError::not_found("approval not found"))?;
                if current.status != "pending" && current.status != decision {
                    if decision == "expired" {
                        return Ok((current, None, Vec::new()));
                    }
                    return Err(AppError::bad_request(
                        "only pending approvals can be declined",
                    ));
                }
                let repairing_legacy_expiry = current.status == "expired" && decision == "expired";
                let existing_tool_call = current
                    .tool_call_id
                    .map(|tool_call_id| {
                        store
                            .tool_calls
                            .get(&tool_call_id)
                            .cloned()
                            .ok_or_else(|| AppError::not_found("tool call not found"))
                    })
                    .transpose()?;
                let (result, events, audits) = declined_approval_records(
                    &current,
                    decision,
                    actor_type,
                    existing_tool_call.as_ref(),
                );
                let existing_events = events
                    .iter()
                    .map(|requested| {
                        let existing = store
                            .events
                            .values()
                            .flatten()
                            .find(|event| event.id == requested.id)
                            .cloned()
                            .or_else(|| {
                                (repairing_legacy_expiry
                                    && requested.event_type == "approval.expired")
                                    .then(|| {
                                        store
                                            .events
                                            .get(&requested.session_id)
                                            .and_then(|events| {
                                                events.iter().find(|event| {
                                                    validate_idempotent_event_identity(
                                                        event,
                                                        &requested.actor_type,
                                                        requested.actor_id,
                                                        requested.session_id,
                                                        &requested.event_type,
                                                        &requested.payload,
                                                    )
                                                    .is_ok()
                                                })
                                            })
                                            .cloned()
                                    })
                                    .flatten()
                            });
                        if let Some(existing) = existing.as_ref() {
                            validate_idempotent_event_identity(
                                existing,
                                &requested.actor_type,
                                requested.actor_id,
                                requested.session_id,
                                &requested.event_type,
                                &requested.payload,
                            )?;
                        }
                        Ok(existing)
                    })
                    .collect::<Result<Vec<_>, AppError>>()?;
                let existing_audits = audits
                    .iter()
                    .map(|requested| {
                        let existing = store.audit_logs.get(&requested.id).cloned().or_else(|| {
                            (repairing_legacy_expiry && requested.action == "approval.expired")
                                .then(|| {
                                    store
                                        .audit_logs
                                        .values()
                                        .find(|existing| {
                                            validate_idempotent_audit_identity(existing, requested)
                                                .is_ok()
                                        })
                                        .cloned()
                                })
                                .flatten()
                        });
                        if let Some(existing) = existing.as_ref() {
                            validate_idempotent_audit_identity(existing, requested)?;
                        }
                        Ok(existing)
                    })
                    .collect::<Result<Vec<_>, AppError>>()?;
                if current.status == "pending" {
                    let approval = store
                        .approvals
                        .get_mut(&approval_id)
                        .expect("validated approval");
                    approval.status = decision.to_string();
                    approval.decided_at = Some(Utc::now());
                }
                let approval = store
                    .approvals
                    .get(&approval_id)
                    .expect("validated approval")
                    .clone();
                let tool_call = approval.tool_call_id.and_then(|tool_call_id| {
                    let tool_call = store
                        .tool_calls
                        .get_mut(&tool_call_id)
                        .expect("validated tool call");
                    if tool_call.status == "waiting_approval" {
                        tool_call.status = "denied".to_string();
                        tool_call.completed_at = Some(Utc::now());
                        tool_call.result = Some(result.clone());
                        tool_call.error = None;
                        Some(tool_call.clone())
                    } else if tool_call.status == "denied"
                        && tool_call.result.as_ref() == Some(&result)
                    {
                        Some(tool_call.clone())
                    } else {
                        None
                    }
                });
                let persisted_events = store.events.entry(approval.session_id).or_default();
                let mut committed_events = Vec::with_capacity(events.len());
                let mut new_events = Vec::with_capacity(events.len());
                for (mut event, existing) in events.into_iter().zip(existing_events) {
                    if let Some(existing) = existing {
                        committed_events.push(existing);
                    } else {
                        event.seq = persisted_events.len() as i64 + 1;
                        persisted_events.push(event.clone());
                        committed_events.push(event.clone());
                        new_events.push(event);
                    }
                }
                for (audit, existing) in audits.into_iter().zip(existing_audits) {
                    if existing.is_none() {
                        store.audit_logs.insert(audit.id, audit);
                    }
                }
                (approval, tool_call, committed_events, new_events)
            }
            StoreBackend::Postgres(pool) => {
                let tenant_id = self.current_tenant_id();
                let mut tx = pool.begin().await?;
                let approval_row = sqlx::query(
                    "SELECT id, session_id, tool_call_id, action, risk_level, reason, evidence, decision_payload, status, expires_at, created_at, decided_at
                     FROM approvals
                     WHERE tenant_id = $1 AND id = $2
                     FOR UPDATE",
                )
                .bind(tenant_id)
                .bind(approval_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::not_found("approval not found"))?;
                let mut approval = approval_from_row(approval_row)?;
                if approval.status != "pending" && approval.status != decision {
                    if decision == "expired" {
                        return Ok((approval, None, Vec::new()));
                    }
                    return Err(AppError::bad_request(
                        "only pending approvals can be declined",
                    ));
                }
                let repairing_legacy_expiry = approval.status == "expired" && decision == "expired";
                let existing_tool_call = if let Some(tool_call_id) = approval.tool_call_id {
                    let row = sqlx::query(
                        "SELECT id, session_id, event_id, tool_name, args, task_grant_id, normalized_args_hash, target_binding, status, risk_level, policy_decision, result, error, started_at, completed_at, created_at
                         FROM tool_calls
                         WHERE tenant_id = $1 AND id = $2
                         FOR UPDATE",
                    )
                    .bind(tenant_id)
                    .bind(tool_call_id)
                    .fetch_optional(&mut *tx)
                    .await?
                    .ok_or_else(|| AppError::not_found("tool call not found"))?;
                    Some(tool_call_from_row(row)?)
                } else {
                    None
                };
                let (result, events, audits) = declined_approval_records(
                    &approval,
                    decision,
                    actor_type,
                    existing_tool_call.as_ref(),
                );
                if approval.status == "pending" {
                    let row = sqlx::query(
                        "UPDATE approvals
                         SET status = $1, decided_at = now()
                         WHERE tenant_id = $2 AND id = $3 AND status = 'pending'
                         RETURNING id, session_id, tool_call_id, action, risk_level, reason, evidence, decision_payload, status, expires_at, created_at, decided_at",
                    )
                    .bind(decision)
                    .bind(tenant_id)
                    .bind(approval_id)
                    .fetch_one(&mut *tx)
                    .await?;
                    approval = approval_from_row(row)?;
                }
                let tool_call = match existing_tool_call {
                    Some(tool_call) if tool_call.status == "waiting_approval" => {
                        let row = sqlx::query(
                            "UPDATE tool_calls
                             SET status = 'denied', result = $1, error = NULL, completed_at = now()
                             WHERE tenant_id = $2 AND id = $3 AND status = 'waiting_approval'
                             RETURNING id, session_id, event_id, tool_name, args, task_grant_id, normalized_args_hash, target_binding, status, risk_level, policy_decision, result, error, started_at, completed_at, created_at",
                        )
                        .bind(&result)
                        .bind(tenant_id)
                        .bind(tool_call.id)
                        .fetch_one(&mut *tx)
                        .await?;
                        Some(tool_call_from_row(row)?)
                    }
                    Some(tool_call)
                        if tool_call.status == "denied"
                            && tool_call.result.as_ref() == Some(&result) =>
                    {
                        Some(tool_call)
                    }
                    _ => None,
                };
                sqlx::query(
                    "SELECT pg_advisory_xact_lock(hashtextextended($1::uuid::text || ':' || $2::uuid::text, 0))",
                )
                .bind(tenant_id)
                .bind(approval.session_id)
                .execute(&mut *tx)
                .await?;
                let mut next_seq = sqlx::query_scalar::<_, i64>(
                    "SELECT COALESCE(MAX(seq), 0) + 1
                     FROM session_events
                     WHERE tenant_id = $1 AND session_id = $2",
                )
                .bind(tenant_id)
                .bind(approval.session_id)
                .fetch_one(&mut *tx)
                .await?;
                let mut committed_events = Vec::with_capacity(events.len());
                let mut new_events = Vec::with_capacity(events.len());
                for mut event in events {
                    let mut existing = sqlx::query(
                        "SELECT id, session_id, seq, parent_event_id, actor_type, actor_id, event_type, payload, created_at
                         FROM session_events
                         WHERE tenant_id = $1 AND id = $2",
                    )
                    .bind(tenant_id)
                    .bind(event.id)
                    .fetch_optional(&mut *tx)
                    .await?;
                    if existing.is_none()
                        && repairing_legacy_expiry
                        && event.event_type == "approval.expired"
                    {
                        existing = sqlx::query(
                            "SELECT id, session_id, seq, parent_event_id, actor_type, actor_id, event_type, payload, created_at
                             FROM session_events
                             WHERE tenant_id = $1 AND session_id = $2
                               AND actor_type = $3 AND actor_id IS NOT DISTINCT FROM $4
                               AND event_type = $5 AND payload = $6
                             ORDER BY seq ASC
                             LIMIT 1",
                        )
                        .bind(tenant_id)
                        .bind(event.session_id)
                        .bind(&event.actor_type)
                        .bind(event.actor_id)
                        .bind(&event.event_type)
                        .bind(&event.payload)
                        .fetch_optional(&mut *tx)
                        .await?;
                    }
                    if let Some(row) = existing {
                        let existing = event_from_row(row)?;
                        validate_idempotent_event_identity(
                            &existing,
                            &event.actor_type,
                            event.actor_id,
                            event.session_id,
                            &event.event_type,
                            &event.payload,
                        )?;
                        committed_events.push(existing);
                        continue;
                    }
                    event.seq = next_seq;
                    next_seq += 1;
                    sqlx::query(
                        "INSERT INTO session_events
                            (id, tenant_id, session_id, seq, parent_event_id, actor_type, actor_id, event_type, payload, created_at)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
                    )
                    .bind(event.id)
                    .bind(tenant_id)
                    .bind(event.session_id)
                    .bind(event.seq)
                    .bind(event.parent_event_id)
                    .bind(&event.actor_type)
                    .bind(event.actor_id)
                    .bind(&event.event_type)
                    .bind(&event.payload)
                    .bind(event.created_at)
                    .execute(&mut *tx)
                    .await?;
                    sqlx::query("SELECT pg_notify($1, $2)")
                        .bind(POSTGRES_SESSION_EVENT_CHANNEL)
                        .bind(session_event_notify_payload(tenant_id, &event))
                        .execute(&mut *tx)
                        .await?;
                    committed_events.push(event.clone());
                    new_events.push(event);
                }
                for audit in audits {
                    let mut existing = sqlx::query(
                        "SELECT id, session_id, actor_type, actor_id, action, resource_type, resource_id, details, created_at
                         FROM audit_logs
                         WHERE tenant_id = $1 AND id = $2",
                    )
                    .bind(tenant_id)
                    .bind(audit.id)
                    .fetch_optional(&mut *tx)
                    .await?;
                    if existing.is_none()
                        && repairing_legacy_expiry
                        && audit.action == "approval.expired"
                    {
                        existing = sqlx::query(
                            "SELECT id, session_id, actor_type, actor_id, action, resource_type, resource_id, details, created_at
                             FROM audit_logs
                             WHERE tenant_id = $1
                               AND session_id IS NOT DISTINCT FROM $2
                               AND actor_type = $3 AND actor_id IS NOT DISTINCT FROM $4
                               AND action = $5 AND resource_type = $6
                               AND resource_id IS NOT DISTINCT FROM $7 AND details = $8
                             ORDER BY created_at ASC
                             LIMIT 1",
                        )
                        .bind(tenant_id)
                        .bind(audit.session_id)
                        .bind(&audit.actor_type)
                        .bind(audit.actor_id)
                        .bind(&audit.action)
                        .bind(&audit.resource_type)
                        .bind(audit.resource_id)
                        .bind(&audit.details)
                        .fetch_optional(&mut *tx)
                        .await?;
                    }
                    if let Some(row) = existing {
                        let existing = audit_log_from_row(row)?;
                        validate_idempotent_audit_identity(&existing, &audit)?;
                    } else {
                        sqlx::query(
                            "INSERT INTO audit_logs
                                (id, tenant_id, session_id, actor_type, actor_id, action, resource_type, resource_id, details, created_at)
                             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
                        )
                        .bind(audit.id)
                        .bind(tenant_id)
                        .bind(audit.session_id)
                        .bind(&audit.actor_type)
                        .bind(audit.actor_id)
                        .bind(&audit.action)
                        .bind(&audit.resource_type)
                        .bind(audit.resource_id)
                        .bind(&audit.details)
                        .bind(audit.created_at)
                        .execute(&mut *tx)
                        .await?;
                    }
                }
                tx.commit().await?;
                (approval, tool_call, committed_events, new_events)
            }
        };
        self.emit_committed_session_events(&committed.3).await;
        Ok((committed.0, committed.1, committed.2))
    }

    pub(crate) async fn modify_approval(
        &self,
        approval_id: Uuid,
        modified_args: Value,
        comment: Option<String>,
    ) -> Result<Approval, AppError> {
        let decision_payload = serde_json::json!({
            "modified_args": modified_args,
            "comment": comment,
        });
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let current = store
                    .approvals
                    .get(&approval_id)
                    .cloned()
                    .ok_or_else(|| AppError::not_found("approval not found"))?;
                if current.status != "pending" {
                    return Err(AppError::bad_request(
                        "only pending approvals can be modified",
                    ));
                }
                if current
                    .expires_at
                    .is_some_and(|expires_at| expires_at <= Utc::now())
                {
                    return Err(AppError::bad_request("approval expired"));
                }
                if let Some(tool_call_id) = current.tool_call_id {
                    let current_tool_call = store
                        .tool_calls
                        .get(&tool_call_id)
                        .cloned()
                        .ok_or_else(|| AppError::not_found("tool call not found"))?;
                    let commit_binding_required = match current_tool_call.task_grant_id {
                        Some(task_grant_id) => {
                            let grant = store
                                .task_grants
                                .get(&task_grant_id)
                                .ok_or_else(|| AppError::not_found("task grant not found"))?;
                            task_grant_requires_approval_commit_token(
                                grant,
                                &current_tool_call.tool_name,
                            )
                        }
                        None => current_tool_call.normalized_args_hash.is_some(),
                    };
                    let (normalized_args_hash, target_binding) = validate_modified_tool_args(
                        &current_tool_call,
                        &modified_args,
                        commit_binding_required,
                    )?;
                    let tool_call = store
                        .tool_calls
                        .get_mut(&tool_call_id)
                        .expect("validated tool call remains present");
                    tool_call.args = modified_args.clone();
                    tool_call.normalized_args_hash = normalized_args_hash;
                    tool_call.target_binding = target_binding;
                }
                let approval = store
                    .approvals
                    .get_mut(&approval_id)
                    .expect("validated approval remains present");
                approval.decision_payload = decision_payload;
                Ok(approval.clone())
            }
            StoreBackend::Postgres(pool) => {
                let tenant_id = self.current_tenant_id();
                let mut tx = pool.begin().await?;
                let approval_row = sqlx::query(
                    "SELECT id, session_id, tool_call_id, action, risk_level, reason, evidence, decision_payload, status, expires_at, created_at, decided_at
                     FROM approvals
                     WHERE tenant_id = $1 AND id = $2
                     FOR UPDATE",
                )
                .bind(tenant_id)
                .bind(approval_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::not_found("approval not found"))?;
                let current = approval_from_row(approval_row)?;
                if current.status != "pending" {
                    return Err(AppError::bad_request(
                        "only pending approvals can be modified",
                    ));
                }
                if current
                    .expires_at
                    .is_some_and(|expires_at| expires_at <= Utc::now())
                {
                    return Err(AppError::bad_request("approval expired"));
                }
                if let Some(tool_call_id) = current.tool_call_id {
                    let tool_call_row = sqlx::query(
                        "SELECT id, session_id, event_id, tool_name, args, task_grant_id, normalized_args_hash, target_binding, status, risk_level, policy_decision, result, error, started_at, completed_at, created_at
                         FROM tool_calls
                         WHERE tenant_id = $1 AND id = $2
                         FOR UPDATE",
                    )
                    .bind(tenant_id)
                    .bind(tool_call_id)
                    .fetch_optional(&mut *tx)
                    .await?
                    .ok_or_else(|| AppError::not_found("tool call not found"))?;
                    let tool_call = tool_call_from_row(tool_call_row)?;
                    let commit_binding_required = match tool_call.task_grant_id {
                        Some(task_grant_id) => {
                            let select_sql = format!(
                                "SELECT {TASK_GRANT_COLUMNS}
                                 FROM task_grants
                                 WHERE tenant_id = $1 AND id = $2"
                            );
                            let row = sqlx::query(&select_sql)
                                .bind(tenant_id)
                                .bind(task_grant_id)
                                .fetch_optional(&mut *tx)
                                .await?
                                .ok_or_else(|| AppError::not_found("task grant not found"))?;
                            let grant = task_grant_from_row(row)?;
                            task_grant_requires_approval_commit_token(&grant, &tool_call.tool_name)
                        }
                        None => tool_call.normalized_args_hash.is_some(),
                    };
                    let (normalized_args_hash, target_binding) = validate_modified_tool_args(
                        &tool_call,
                        &modified_args,
                        commit_binding_required,
                    )?;
                    sqlx::query(
                        "UPDATE tool_calls
                         SET args = $1, normalized_args_hash = $2, target_binding = $3
                         WHERE tenant_id = $4 AND id = $5",
                    )
                    .bind(&modified_args)
                    .bind(&normalized_args_hash)
                    .bind(&target_binding)
                    .bind(tenant_id)
                    .bind(tool_call_id)
                    .execute(&mut *tx)
                    .await?;
                }
                let row = sqlx::query(
                    "UPDATE approvals
                     SET decision_payload = $1
                     WHERE tenant_id = $2 AND id = $3
                     RETURNING id, session_id, tool_call_id, action, risk_level, reason, evidence, decision_payload, status, expires_at, created_at, decided_at",
                )
                .bind(&decision_payload)
                .bind(tenant_id)
                .bind(approval_id)
                .fetch_one(&mut *tx)
                .await?;
                let approval = approval_from_row(row)?;
                tx.commit().await?;
                Ok(approval)
            }
        }
    }

    pub(crate) async fn update_approval_evidence(
        &self,
        approval_id: Uuid,
        evidence: Value,
    ) -> Result<Approval, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let approval = store
                    .approvals
                    .get_mut(&approval_id)
                    .ok_or_else(|| AppError::not_found("approval not found"))?;
                approval.evidence = evidence;
                Ok(approval.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE approvals
                     SET evidence = $1
                     WHERE tenant_id = $2 AND id = $3
                     RETURNING id, session_id, tool_call_id, action, risk_level, reason, evidence, decision_payload, status, expires_at, created_at, decided_at",
                )
                .bind(&evidence)
                .bind(self.current_tenant_id())
                .bind(approval_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("approval not found"))?;
                approval_from_row(row)
            }
        }
    }
}
