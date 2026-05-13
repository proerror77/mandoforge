use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::approval_notification_channel_policy_from_row;
use crate::{
    AppError, AppState, ApprovalNotificationChannelPolicy, CreateApprovalNotificationChannelPolicy,
};

impl AppState {
    pub(crate) async fn list_approval_notification_channel_policies(
        &self,
    ) -> Result<Vec<ApprovalNotificationChannelPolicy>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut policies: Vec<_> = inner
                    .read()
                    .await
                    .approval_notification_channel_policies
                    .values()
                    .cloned()
                    .collect();
                policies.sort_by_key(|policy| policy.created_at);
                policies.reverse();
                Ok(policies)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, name, channel, target_env, risk_filter, max_attempts, backoff_seconds, status, created_at
                     FROM approval_notification_channel_policies
                     WHERE tenant_id = $1
                     ORDER BY created_at DESC",
                )
                .bind(self.tenant_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter()
                    .map(approval_notification_channel_policy_from_row)
                    .collect()
            }
        }
    }

    pub(crate) async fn create_approval_notification_channel_policy(
        &self,
        input: CreateApprovalNotificationChannelPolicy,
    ) -> Result<ApprovalNotificationChannelPolicy, AppError> {
        let policy = ApprovalNotificationChannelPolicy {
            id: Uuid::new_v4(),
            name: input.name,
            channel: input.channel,
            target_env: input.target_env,
            risk_filter: input.risk_filter,
            max_attempts: input.max_attempts,
            backoff_seconds: input.backoff_seconds,
            status: "active".to_string(),
            created_at: Utc::now(),
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                if store
                    .approval_notification_channel_policies
                    .values()
                    .any(|existing| existing.name == policy.name)
                {
                    return Err(AppError::bad_request(
                        "approval notification channel policy name already exists",
                    ));
                }
                store
                    .approval_notification_channel_policies
                    .insert(policy.id, policy.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO approval_notification_channel_policies
                     (id, tenant_id, name, channel, target_env, risk_filter, max_attempts, backoff_seconds, status, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
                )
                .bind(policy.id)
                .bind(self.tenant_id)
                .bind(&policy.name)
                .bind(&policy.channel)
                .bind(&policy.target_env)
                .bind(&policy.risk_filter)
                .bind(policy.max_attempts)
                .bind(policy.backoff_seconds)
                .bind(&policy.status)
                .bind(policy.created_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(policy)
    }

    pub(crate) async fn archive_approval_notification_channel_policy(
        &self,
        id: Uuid,
    ) -> Result<ApprovalNotificationChannelPolicy, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let Some(policy) = store.approval_notification_channel_policies.get_mut(&id) else {
                    return Err(AppError::not_found("approval notification channel policy"));
                };
                policy.status = "archived".to_string();
                Ok(policy.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE approval_notification_channel_policies
                     SET status = 'archived'
                     WHERE tenant_id = $1 AND id = $2
                     RETURNING id, name, channel, target_env, risk_filter, max_attempts, backoff_seconds, status, created_at",
                )
                .bind(self.tenant_id)
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("approval notification channel policy"))?;
                approval_notification_channel_policy_from_row(row)
            }
        }
    }
}
