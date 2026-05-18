use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::{agent_handoff_assignment_from_row, agent_handoff_event_from_row};
use crate::{AgentHandoffAssignment, AgentHandoffEvent, AppError, AppState};

impl AppState {
    pub(crate) async fn list_agent_handoff_events(
        &self,
        source_session_id: Option<Uuid>,
    ) -> Result<Vec<AgentHandoffEvent>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut handoffs: Vec<_> = inner
                    .read()
                    .await
                    .agent_handoff_events
                    .values()
                    .filter(|event| {
                        source_session_id
                            .is_none_or(|session_id| event.source_session_id == session_id)
                    })
                    .cloned()
                    .collect();
                handoffs.sort_by_key(|event| event.created_at);
                Ok(handoffs)
            }
            StoreBackend::Postgres(pool) => {
                let rows = match source_session_id {
                    Some(session_id) => {
                        sqlx::query(
                            "SELECT id, source_session_id, source_agent_id, target_agent_id, manager_plan_id, intent, payload, schema_version, risk_level, approval_required, semantic_scopes, runtime_profile_id, remote_computer_required, review_status, human_escalation_status, status, audit_trace_id, created_at, updated_at
                             FROM agent_handoff_events
                             WHERE tenant_id = $1 AND source_session_id = $2
                             ORDER BY created_at ASC",
                        )
                        .bind(self.current_tenant_id())
                        .bind(session_id)
                        .fetch_all(pool)
                        .await?
                    }
                    None => {
                        sqlx::query(
                            "SELECT id, source_session_id, source_agent_id, target_agent_id, manager_plan_id, intent, payload, schema_version, risk_level, approval_required, semantic_scopes, runtime_profile_id, remote_computer_required, review_status, human_escalation_status, status, audit_trace_id, created_at, updated_at
                             FROM agent_handoff_events
                             WHERE tenant_id = $1
                             ORDER BY created_at ASC",
                        )
                        .bind(self.current_tenant_id())
                        .fetch_all(pool)
                        .await?
                    }
                };
                rows.into_iter().map(agent_handoff_event_from_row).collect()
            }
        }
    }

    pub(crate) async fn get_agent_handoff_event(
        &self,
        id: Uuid,
    ) -> Result<AgentHandoffEvent, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .agent_handoff_events
                .get(&id)
                .cloned()
                .ok_or_else(|| AppError::not_found("agent handoff event not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, source_session_id, source_agent_id, target_agent_id, manager_plan_id, intent, payload, schema_version, risk_level, approval_required, semantic_scopes, runtime_profile_id, remote_computer_required, review_status, human_escalation_status, status, audit_trace_id, created_at, updated_at
                     FROM agent_handoff_events
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("agent handoff event not found"))?;
                agent_handoff_event_from_row(row)
            }
        }
    }

    pub(crate) async fn create_agent_handoff_event(
        &self,
        event: AgentHandoffEvent,
    ) -> Result<AgentHandoffEvent, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .agent_handoff_events
                    .insert(event.id, event.clone());
                Ok(event)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "INSERT INTO agent_handoff_events
                        (id, tenant_id, source_session_id, source_agent_id, target_agent_id, manager_plan_id, intent, payload, schema_version, risk_level, approval_required, semantic_scopes, runtime_profile_id, remote_computer_required, review_status, human_escalation_status, status, audit_trace_id, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)
                     RETURNING id, source_session_id, source_agent_id, target_agent_id, manager_plan_id, intent, payload, schema_version, risk_level, approval_required, semantic_scopes, runtime_profile_id, remote_computer_required, review_status, human_escalation_status, status, audit_trace_id, created_at, updated_at",
                )
                .bind(event.id)
                .bind(self.current_tenant_id())
                .bind(event.source_session_id)
                .bind(event.source_agent_id)
                .bind(event.target_agent_id)
                .bind(event.manager_plan_id)
                .bind(&event.intent)
                .bind(&event.payload)
                .bind(&event.schema_version)
                .bind(&event.risk_level)
                .bind(event.approval_required)
                .bind(&event.semantic_scopes)
                .bind(event.runtime_profile_id)
                .bind(event.remote_computer_required)
                .bind(&event.review_status)
                .bind(&event.human_escalation_status)
                .bind(&event.status)
                .bind(event.audit_trace_id)
                .bind(event.created_at)
                .bind(event.updated_at)
                .fetch_one(pool)
                .await?;
                agent_handoff_event_from_row(row)
            }
        }
    }

    pub(crate) async fn update_agent_handoff_event_status(
        &self,
        id: Uuid,
        status: &str,
        audit_trace_id: Option<Uuid>,
    ) -> Result<AgentHandoffEvent, AppError> {
        let updated_at = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let event = store
                    .agent_handoff_events
                    .get_mut(&id)
                    .ok_or_else(|| AppError::not_found("agent handoff event not found"))?;
                event.status = status.to_string();
                event.audit_trace_id = audit_trace_id;
                if status == "accepted" {
                    event.review_status = "approved".to_string();
                }
                if status == "rejected" {
                    event.review_status = "rejected".to_string();
                }
                if status == "failed" && event.human_escalation_status == "required" {
                    event.human_escalation_status = "requested".to_string();
                }
                event.updated_at = updated_at;
                Ok(event.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE agent_handoff_events
                     SET status = $3,
                         audit_trace_id = $4,
                         review_status = CASE
                            WHEN $3 = 'accepted' THEN 'approved'
                            WHEN $3 = 'rejected' THEN 'rejected'
                            ELSE review_status
                         END,
                         human_escalation_status = CASE
                            WHEN $3 = 'failed' AND human_escalation_status = 'required' THEN 'requested'
                            ELSE human_escalation_status
                         END,
                         updated_at = $5
                     WHERE tenant_id = $1 AND id = $2
                     RETURNING id, source_session_id, source_agent_id, target_agent_id, manager_plan_id, intent, payload, schema_version, risk_level, approval_required, semantic_scopes, runtime_profile_id, remote_computer_required, review_status, human_escalation_status, status, audit_trace_id, created_at, updated_at",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .bind(status)
                .bind(audit_trace_id)
                .bind(updated_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("agent handoff event not found"))?;
                agent_handoff_event_from_row(row)
            }
        }
    }

    pub(crate) async fn list_agent_handoff_assignments(
        &self,
        session_id: Option<Uuid>,
    ) -> Result<Vec<AgentHandoffAssignment>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut assignments: Vec<_> = inner
                    .read()
                    .await
                    .agent_handoff_assignments
                    .values()
                    .filter(|assignment| {
                        session_id.is_none_or(|id| {
                            assignment.source_session_id == id
                                || assignment.specialist_session_id == id
                        })
                    })
                    .cloned()
                    .collect();
                assignments.sort_by_key(|assignment| assignment.created_at);
                Ok(assignments)
            }
            StoreBackend::Postgres(pool) => {
                let rows = match session_id {
                    Some(session_id) => {
                        sqlx::query(
                            "SELECT id, agent_handoff_event_id, manager_plan_id, source_session_id, specialist_session_id, source_agent_id, target_agent_id, semantic_scopes, runtime_profile_id, remote_computer_required, remote_computer_job_assignment_id, status, assigned_by, metadata, audit_trace_id, created_at, updated_at
                             FROM agent_handoff_assignments
                             WHERE tenant_id = $1
                               AND (source_session_id = $2 OR specialist_session_id = $2)
                             ORDER BY created_at ASC",
                        )
                        .bind(self.current_tenant_id())
                        .bind(session_id)
                        .fetch_all(pool)
                        .await?
                    }
                    None => {
                        sqlx::query(
                            "SELECT id, agent_handoff_event_id, manager_plan_id, source_session_id, specialist_session_id, source_agent_id, target_agent_id, semantic_scopes, runtime_profile_id, remote_computer_required, remote_computer_job_assignment_id, status, assigned_by, metadata, audit_trace_id, created_at, updated_at
                             FROM agent_handoff_assignments
                             WHERE tenant_id = $1
                             ORDER BY created_at ASC",
                        )
                        .bind(self.current_tenant_id())
                        .fetch_all(pool)
                        .await?
                    }
                };
                rows.into_iter()
                    .map(agent_handoff_assignment_from_row)
                    .collect()
            }
        }
    }

    pub(crate) async fn get_agent_handoff_assignment(
        &self,
        id: Uuid,
    ) -> Result<AgentHandoffAssignment, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .agent_handoff_assignments
                .get(&id)
                .cloned()
                .ok_or_else(|| AppError::not_found("agent handoff assignment not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, agent_handoff_event_id, manager_plan_id, source_session_id, specialist_session_id, source_agent_id, target_agent_id, semantic_scopes, runtime_profile_id, remote_computer_required, remote_computer_job_assignment_id, status, assigned_by, metadata, audit_trace_id, created_at, updated_at
                     FROM agent_handoff_assignments
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("agent handoff assignment not found"))?;
                agent_handoff_assignment_from_row(row)
            }
        }
    }

    pub(crate) async fn get_agent_handoff_assignment_for_handoff(
        &self,
        agent_handoff_event_id: Uuid,
    ) -> Result<Option<AgentHandoffAssignment>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => Ok(inner
                .read()
                .await
                .agent_handoff_assignments
                .values()
                .find(|assignment| assignment.agent_handoff_event_id == agent_handoff_event_id)
                .cloned()),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, agent_handoff_event_id, manager_plan_id, source_session_id, specialist_session_id, source_agent_id, target_agent_id, semantic_scopes, runtime_profile_id, remote_computer_required, remote_computer_job_assignment_id, status, assigned_by, metadata, audit_trace_id, created_at, updated_at
                     FROM agent_handoff_assignments
                     WHERE tenant_id = $1 AND agent_handoff_event_id = $2",
                )
                .bind(self.current_tenant_id())
                .bind(agent_handoff_event_id)
                .fetch_optional(pool)
                .await?;
                row.map(agent_handoff_assignment_from_row).transpose()
            }
        }
    }

    pub(crate) async fn create_agent_handoff_assignment(
        &self,
        assignment: AgentHandoffAssignment,
    ) -> Result<AgentHandoffAssignment, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                if store.agent_handoff_assignments.values().any(|existing| {
                    existing.agent_handoff_event_id == assignment.agent_handoff_event_id
                }) {
                    return Err(AppError::bad_request(
                        "agent handoff already has an assignment",
                    ));
                }
                store
                    .agent_handoff_assignments
                    .insert(assignment.id, assignment.clone());
                Ok(assignment)
            }
            StoreBackend::Postgres(pool) => {
                let duplicate = sqlx::query(
                    "SELECT id
                     FROM agent_handoff_assignments
                     WHERE tenant_id = $1 AND agent_handoff_event_id = $2",
                )
                .bind(self.current_tenant_id())
                .bind(assignment.agent_handoff_event_id)
                .fetch_optional(pool)
                .await?;
                if duplicate.is_some() {
                    return Err(AppError::bad_request(
                        "agent handoff already has an assignment",
                    ));
                }
                let row = sqlx::query(
                    "INSERT INTO agent_handoff_assignments
                        (id, tenant_id, agent_handoff_event_id, manager_plan_id, source_session_id, specialist_session_id, source_agent_id, target_agent_id, semantic_scopes, runtime_profile_id, remote_computer_required, remote_computer_job_assignment_id, status, assigned_by, metadata, audit_trace_id, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
                     RETURNING id, agent_handoff_event_id, manager_plan_id, source_session_id, specialist_session_id, source_agent_id, target_agent_id, semantic_scopes, runtime_profile_id, remote_computer_required, remote_computer_job_assignment_id, status, assigned_by, metadata, audit_trace_id, created_at, updated_at",
                )
                .bind(assignment.id)
                .bind(self.current_tenant_id())
                .bind(assignment.agent_handoff_event_id)
                .bind(assignment.manager_plan_id)
                .bind(assignment.source_session_id)
                .bind(assignment.specialist_session_id)
                .bind(assignment.source_agent_id)
                .bind(assignment.target_agent_id)
                .bind(&assignment.semantic_scopes)
                .bind(assignment.runtime_profile_id)
                .bind(assignment.remote_computer_required)
                .bind(assignment.remote_computer_job_assignment_id)
                .bind(&assignment.status)
                .bind(&assignment.assigned_by)
                .bind(&assignment.metadata)
                .bind(assignment.audit_trace_id)
                .bind(assignment.created_at)
                .bind(assignment.updated_at)
                .fetch_one(pool)
                .await?;
                agent_handoff_assignment_from_row(row)
            }
        }
    }

    pub(crate) async fn update_agent_handoff_assignment_audit_trace(
        &self,
        id: Uuid,
        audit_trace_id: Uuid,
    ) -> Result<AgentHandoffAssignment, AppError> {
        let updated_at = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let assignment = store
                    .agent_handoff_assignments
                    .get_mut(&id)
                    .ok_or_else(|| AppError::not_found("agent handoff assignment not found"))?;
                assignment.audit_trace_id = Some(audit_trace_id);
                assignment.updated_at = updated_at;
                Ok(assignment.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE agent_handoff_assignments
                     SET audit_trace_id = $3,
                         updated_at = $4
                     WHERE tenant_id = $1 AND id = $2
                     RETURNING id, agent_handoff_event_id, manager_plan_id, source_session_id, specialist_session_id, source_agent_id, target_agent_id, semantic_scopes, runtime_profile_id, remote_computer_required, remote_computer_job_assignment_id, status, assigned_by, metadata, audit_trace_id, created_at, updated_at",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .bind(audit_trace_id)
                .bind(updated_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("agent handoff assignment not found"))?;
                agent_handoff_assignment_from_row(row)
            }
        }
    }

    pub(crate) async fn attach_agent_handoff_assignment_remote_computer_job(
        &self,
        id: Uuid,
        remote_computer_job_assignment_id: Uuid,
        metadata: serde_json::Value,
    ) -> Result<AgentHandoffAssignment, AppError> {
        let updated_at = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let assignment = store
                    .agent_handoff_assignments
                    .get_mut(&id)
                    .ok_or_else(|| AppError::not_found("agent handoff assignment not found"))?;
                assignment.remote_computer_job_assignment_id =
                    Some(remote_computer_job_assignment_id);
                assignment.status = "assigned".to_string();
                assignment.metadata =
                    merge_agent_handoff_assignment_metadata(&assignment.metadata, metadata);
                assignment.updated_at = updated_at;
                Ok(assignment.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE agent_handoff_assignments
                     SET remote_computer_job_assignment_id = $3,
                         status = 'assigned',
                         metadata = metadata || $4::jsonb,
                         updated_at = $5
                     WHERE tenant_id = $1 AND id = $2
                     RETURNING id, agent_handoff_event_id, manager_plan_id, source_session_id, specialist_session_id, source_agent_id, target_agent_id, semantic_scopes, runtime_profile_id, remote_computer_required, remote_computer_job_assignment_id, status, assigned_by, metadata, audit_trace_id, created_at, updated_at",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .bind(remote_computer_job_assignment_id)
                .bind(metadata)
                .bind(updated_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("agent handoff assignment not found"))?;
                agent_handoff_assignment_from_row(row)
            }
        }
    }
}

fn merge_agent_handoff_assignment_metadata(
    existing: &serde_json::Value,
    patch: serde_json::Value,
) -> serde_json::Value {
    match (existing.as_object(), patch.as_object()) {
        (Some(existing), Some(patch)) => {
            let mut merged = existing.clone();
            for (key, value) in patch {
                merged.insert(key.clone(), value.clone());
            }
            serde_json::Value::Object(merged)
        }
        _ => patch,
    }
}
