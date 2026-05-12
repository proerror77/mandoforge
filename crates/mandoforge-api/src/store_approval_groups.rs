use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::{approval_escalation_rule_from_row, approval_group_from_row};
use crate::{
    AppError, AppState, ApprovalEscalationRule, ApprovalGroup, CreateApprovalEscalationRule,
    CreateApprovalGroup,
};

impl AppState {
    pub(crate) async fn list_approval_groups(&self) -> Result<Vec<ApprovalGroup>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut groups: Vec<_> = inner
                    .read()
                    .await
                    .approval_groups
                    .values()
                    .cloned()
                    .collect();
                groups.sort_by_key(|group| group.created_at);
                groups.reverse();
                Ok(groups)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, name, subjects, status, created_at
                     FROM approval_groups
                     WHERE tenant_id = $1
                     ORDER BY created_at DESC",
                )
                .bind(self.tenant_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(approval_group_from_row).collect()
            }
        }
    }

    pub(crate) async fn get_approval_group(&self, id: Uuid) -> Result<ApprovalGroup, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .approval_groups
                .get(&id)
                .cloned()
                .ok_or_else(|| AppError::not_found("approval group not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, name, subjects, status, created_at
                     FROM approval_groups
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.tenant_id)
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("approval group not found"))?;
                approval_group_from_row(row)
            }
        }
    }

    pub(crate) async fn create_approval_group(
        &self,
        input: CreateApprovalGroup,
    ) -> Result<ApprovalGroup, AppError> {
        let group = ApprovalGroup {
            id: Uuid::new_v4(),
            name: input.name,
            subjects: input.subjects,
            status: "active".to_string(),
            created_at: Utc::now(),
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                if store
                    .approval_groups
                    .values()
                    .any(|existing| existing.name == group.name)
                {
                    return Err(AppError::bad_request("approval group name already exists"));
                }
                store.approval_groups.insert(group.id, group.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO approval_groups (id, tenant_id, name, subjects, status, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6)",
                )
                .bind(group.id)
                .bind(self.tenant_id)
                .bind(&group.name)
                .bind(serde_json::to_value(&group.subjects)?)
                .bind(&group.status)
                .bind(group.created_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(group)
    }

    pub(crate) async fn list_approval_escalation_rules(
        &self,
    ) -> Result<Vec<ApprovalEscalationRule>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut rules: Vec<_> = inner
                    .read()
                    .await
                    .approval_escalation_rules
                    .values()
                    .cloned()
                    .collect();
                rules.sort_by_key(|rule| (rule.order_index, rule.created_at));
                Ok(rules)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, name, risk_level, group_id, order_index, after_seconds, status, created_at
                     FROM approval_escalation_rules
                     WHERE tenant_id = $1
                     ORDER BY order_index ASC, created_at ASC",
                )
                .bind(self.tenant_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter()
                    .map(approval_escalation_rule_from_row)
                    .collect()
            }
        }
    }

    pub(crate) async fn create_approval_escalation_rule(
        &self,
        input: CreateApprovalEscalationRule,
    ) -> Result<ApprovalEscalationRule, AppError> {
        self.get_approval_group(input.group_id).await?;
        let rule = ApprovalEscalationRule {
            id: Uuid::new_v4(),
            name: input.name,
            risk_level: input.risk_level,
            group_id: input.group_id,
            order_index: input.order_index,
            after_seconds: input.after_seconds,
            status: "active".to_string(),
            created_at: Utc::now(),
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                if store
                    .approval_escalation_rules
                    .values()
                    .any(|existing| existing.name == rule.name)
                {
                    return Err(AppError::bad_request(
                        "approval escalation rule name already exists",
                    ));
                }
                store
                    .approval_escalation_rules
                    .insert(rule.id, rule.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO approval_escalation_rules (id, tenant_id, name, risk_level, group_id, order_index, after_seconds, status, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
                )
                .bind(rule.id)
                .bind(self.tenant_id)
                .bind(&rule.name)
                .bind(&rule.risk_level)
                .bind(rule.group_id)
                .bind(rule.order_index)
                .bind(rule.after_seconds)
                .bind(&rule.status)
                .bind(rule.created_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(rule)
    }

    pub(crate) async fn first_active_escalation_rule_for_risk(
        &self,
        risk_level: &str,
    ) -> Result<Option<ApprovalEscalationRule>, AppError> {
        Ok(self
            .list_approval_escalation_rules()
            .await?
            .into_iter()
            .find(|rule| rule.status == "active" && rule.risk_level == risk_level))
    }
}
