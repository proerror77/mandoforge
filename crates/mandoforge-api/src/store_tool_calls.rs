use anyhow::Result;
use chrono::Utc;
use serde_json::Value;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::tool_call_from_row;
use crate::{AppError, AppState, ToolCall};

impl AppState {
    pub(crate) async fn insert_tool_call(&self, tool_call: ToolCall) -> Result<ToolCall, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .tool_calls
                    .insert(tool_call.id, tool_call.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO tool_calls
                        (id, tenant_id, session_id, event_id, tool_name, args, result, status, risk_level, policy_decision, started_at, completed_at, error, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)",
                )
                .bind(tool_call.id)
                .bind(self.tenant_id)
                .bind(tool_call.session_id)
                .bind(tool_call.event_id)
                .bind(&tool_call.tool_name)
                .bind(&tool_call.args)
                .bind(&tool_call.result)
                .bind(&tool_call.status)
                .bind(&tool_call.risk_level)
                .bind(&tool_call.policy_decision)
                .bind(tool_call.started_at)
                .bind(tool_call.completed_at)
                .bind(&tool_call.error)
                .bind(tool_call.created_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(tool_call)
    }

    pub(crate) async fn update_tool_call_status(
        &self,
        id: Uuid,
        status: &str,
        result: Option<Value>,
        error: Option<Value>,
    ) -> Result<ToolCall, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let tool_call = store
                    .tool_calls
                    .get_mut(&id)
                    .ok_or_else(|| AppError::not_found("tool call not found"))?;
                tool_call.status = status.to_string();
                tool_call.completed_at = Some(Utc::now());
                tool_call.result = result;
                tool_call.error = error;
                Ok(tool_call.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE tool_calls
                     SET status = $1, result = $2, error = $3, completed_at = now()
                     WHERE tenant_id = $4 AND id = $5
                     RETURNING id, session_id, event_id, tool_name, args, status, risk_level, policy_decision, result, error, started_at, completed_at, created_at",
                )
                .bind(status)
                .bind(result)
                .bind(error)
                .bind(self.tenant_id)
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("tool call not found"))?;
                tool_call_from_row(row)
            }
        }
    }

    pub(crate) async fn update_tool_call_args(
        &self,
        id: Uuid,
        args: Value,
    ) -> Result<ToolCall, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let tool_call = store
                    .tool_calls
                    .get_mut(&id)
                    .ok_or_else(|| AppError::not_found("tool call not found"))?;
                if tool_call.status != "waiting_approval" {
                    return Err(AppError::bad_request(
                        "only waiting approval tool calls can be modified",
                    ));
                }
                tool_call.args = args;
                Ok(tool_call.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE tool_calls
                     SET args = $1
                     WHERE tenant_id = $2 AND id = $3 AND status = 'waiting_approval'
                     RETURNING id, session_id, event_id, tool_name, args, status, risk_level, policy_decision, result, error, started_at, completed_at, created_at",
                )
                .bind(args)
                .bind(self.tenant_id)
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("waiting approval tool call not found"))?;
                tool_call_from_row(row)
            }
        }
    }

    pub(crate) async fn get_tool_call(&self, id: Uuid) -> Result<ToolCall, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .tool_calls
                .get(&id)
                .cloned()
                .ok_or_else(|| AppError::not_found("tool call not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, session_id, event_id, tool_name, args, status, risk_level, policy_decision, result, error, started_at, completed_at, created_at
                     FROM tool_calls
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.tenant_id)
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("tool call not found"))?;
                tool_call_from_row(row)
            }
        }
    }

    pub(crate) async fn list_tool_calls(
        &self,
        session_id: Option<Uuid>,
    ) -> Result<Vec<ToolCall>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut calls: Vec<_> = inner
                    .read()
                    .await
                    .tool_calls
                    .values()
                    .filter(|call| session_id.is_none_or(|id| call.session_id == id))
                    .cloned()
                    .collect();
                calls.sort_by_key(|call| call.created_at);
                calls.reverse();
                Ok(calls)
            }
            StoreBackend::Postgres(pool) => {
                let rows = match session_id {
                    Some(session_id) => {
                        sqlx::query(
                            "SELECT id, session_id, event_id, tool_name, args, status, risk_level, policy_decision, result, error, started_at, completed_at, created_at
                             FROM tool_calls
                             WHERE tenant_id = $1 AND session_id = $2
                             ORDER BY created_at DESC",
                        )
                        .bind(self.tenant_id)
                        .bind(session_id)
                        .fetch_all(pool)
                        .await?
                    }
                    None => {
                        sqlx::query(
                            "SELECT id, session_id, event_id, tool_name, args, status, risk_level, policy_decision, result, error, started_at, completed_at, created_at
                             FROM tool_calls
                             WHERE tenant_id = $1
                             ORDER BY created_at DESC",
                        )
                        .bind(self.tenant_id)
                        .fetch_all(pool)
                        .await?
                    }
                };
                rows.into_iter().map(tool_call_from_row).collect()
            }
        }
    }
}
