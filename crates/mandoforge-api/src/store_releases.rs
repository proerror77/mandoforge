use anyhow::Result;
use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::agent_release_from_row;
use crate::{AgentRelease, AppError, AppState, CreateAgentRelease};

const AGENT_RELEASE_COLUMNS: &str = "id, agent_id, agent_version_id, environment, status, eval_run_id, eval_score, min_score, requested_by, requested_at, request_reason, approver_subject, decision_by, decided_at, decision_reason, promoted_by, promoted_at, automation_policy, created_at";

impl AppState {
    pub(crate) async fn list_agent_releases(
        &self,
        agent_id: Uuid,
    ) -> Result<Vec<AgentRelease>, AppError> {
        self.get_agent(agent_id).await?;
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut releases: Vec<_> = inner
                    .read()
                    .await
                    .agent_releases
                    .values()
                    .filter(|release| release.agent_id == agent_id)
                    .cloned()
                    .collect();
                releases.sort_by_key(|release| release.created_at);
                releases.reverse();
                Ok(releases)
            }
            StoreBackend::Postgres(pool) => {
                let sql = format!(
                    "SELECT {AGENT_RELEASE_COLUMNS}
                     FROM agent_releases
                     WHERE tenant_id = $1 AND agent_id = $2
                     ORDER BY created_at DESC"
                );
                let rows = sqlx::query(&sql)
                    .bind(self.tenant_id)
                    .bind(agent_id)
                    .fetch_all(pool)
                    .await?;
                rows.into_iter().map(agent_release_from_row).collect()
            }
        }
    }

    pub(crate) async fn list_all_agent_releases(&self) -> Result<Vec<AgentRelease>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut releases: Vec<_> = inner
                    .read()
                    .await
                    .agent_releases
                    .values()
                    .cloned()
                    .collect();
                releases.sort_by_key(|release| release.created_at);
                releases.reverse();
                Ok(releases)
            }
            StoreBackend::Postgres(pool) => {
                let sql = format!(
                    "SELECT {AGENT_RELEASE_COLUMNS}
                     FROM agent_releases
                     WHERE tenant_id = $1
                     ORDER BY created_at DESC"
                );
                let rows = sqlx::query(&sql)
                    .bind(self.tenant_id)
                    .fetch_all(pool)
                    .await?;
                rows.into_iter().map(agent_release_from_row).collect()
            }
        }
    }

    pub(crate) async fn list_pending_agent_releases(&self) -> Result<Vec<AgentRelease>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut releases: Vec<_> = inner
                    .read()
                    .await
                    .agent_releases
                    .values()
                    .filter(|release| release.status == "pending_approval")
                    .cloned()
                    .collect();
                releases.sort_by_key(|release| release.created_at);
                Ok(releases)
            }
            StoreBackend::Postgres(pool) => {
                let sql = format!(
                    "SELECT {AGENT_RELEASE_COLUMNS}
                     FROM agent_releases
                     WHERE tenant_id = $1 AND status = 'pending_approval'
                     ORDER BY created_at ASC"
                );
                let rows = sqlx::query(&sql)
                    .bind(self.tenant_id)
                    .fetch_all(pool)
                    .await?;
                rows.into_iter().map(agent_release_from_row).collect()
            }
        }
    }

    pub(crate) async fn create_agent_release(
        &self,
        agent_id: Uuid,
        input: CreateAgentRelease,
        promoted_by: String,
    ) -> Result<AgentRelease, AppError> {
        let (agent_version_id, environment, eval_run_id, score, min_score) =
            self.validate_agent_release_input(agent_id, &input).await?;
        let now = Utc::now();
        let release = AgentRelease {
            id: Uuid::new_v4(),
            agent_id,
            agent_version_id,
            environment,
            status: "promoted".to_string(),
            eval_run_id: Some(eval_run_id),
            eval_score: Some(score),
            min_score,
            requested_by: Some(promoted_by.clone()),
            requested_at: Some(now),
            request_reason: None,
            approver_subject: None,
            decision_by: Some(promoted_by.clone()),
            decided_at: Some(now),
            decision_reason: Some("direct promotion".to_string()),
            promoted_by: Some(promoted_by),
            promoted_at: Some(now),
            automation_policy: json!({}),
            created_at: now,
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .agent_releases
                    .insert(release.id, release.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO agent_releases (id, tenant_id, agent_id, agent_version_id, environment, status, eval_run_id, eval_score, min_score, requested_by, requested_at, request_reason, approver_subject, decision_by, decided_at, decision_reason, promoted_by, promoted_at, automation_policy, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)",
                )
                .bind(release.id)
                .bind(self.tenant_id)
                .bind(release.agent_id)
                .bind(release.agent_version_id)
                .bind(&release.environment)
                .bind(&release.status)
                .bind(release.eval_run_id)
                .bind(release.eval_score)
                .bind(release.min_score)
                .bind(&release.requested_by)
                .bind(release.requested_at)
                .bind(&release.request_reason)
                .bind(&release.approver_subject)
                .bind(&release.decision_by)
                .bind(release.decided_at)
                .bind(&release.decision_reason)
                .bind(&release.promoted_by)
                .bind(release.promoted_at)
                .bind(&release.automation_policy)
                .bind(release.created_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(release)
    }

    pub(crate) async fn request_agent_release_promotion(
        &self,
        agent_id: Uuid,
        input: CreateAgentRelease,
        requested_by: String,
        approver_subject: Option<String>,
        request_reason: Option<String>,
        automation_policy: Value,
    ) -> Result<AgentRelease, AppError> {
        let (agent_version_id, environment, eval_run_id, score, min_score) =
            self.validate_agent_release_input(agent_id, &input).await?;
        let now = Utc::now();
        let release = AgentRelease {
            id: Uuid::new_v4(),
            agent_id,
            agent_version_id,
            environment,
            status: "pending_approval".to_string(),
            eval_run_id: Some(eval_run_id),
            eval_score: Some(score),
            min_score,
            requested_by: Some(requested_by),
            requested_at: Some(now),
            request_reason,
            approver_subject,
            decision_by: None,
            decided_at: None,
            decision_reason: None,
            promoted_by: None,
            promoted_at: None,
            automation_policy,
            created_at: now,
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .agent_releases
                    .insert(release.id, release.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO agent_releases (id, tenant_id, agent_id, agent_version_id, environment, status, eval_run_id, eval_score, min_score, requested_by, requested_at, request_reason, approver_subject, decision_by, decided_at, decision_reason, promoted_by, promoted_at, automation_policy, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)",
                )
                .bind(release.id)
                .bind(self.tenant_id)
                .bind(release.agent_id)
                .bind(release.agent_version_id)
                .bind(&release.environment)
                .bind(&release.status)
                .bind(release.eval_run_id)
                .bind(release.eval_score)
                .bind(release.min_score)
                .bind(&release.requested_by)
                .bind(release.requested_at)
                .bind(&release.request_reason)
                .bind(&release.approver_subject)
                .bind(&release.decision_by)
                .bind(release.decided_at)
                .bind(&release.decision_reason)
                .bind(&release.promoted_by)
                .bind(release.promoted_at)
                .bind(&release.automation_policy)
                .bind(release.created_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(release)
    }

    pub(crate) async fn approve_agent_release_promotion(
        &self,
        agent_id: Uuid,
        release_id: Uuid,
        approved_by: String,
    ) -> Result<AgentRelease, AppError> {
        self.decide_agent_release_promotion(
            agent_id,
            release_id,
            "promoted",
            approved_by,
            Some("approved".to_string()),
        )
        .await
    }

    pub(crate) async fn reject_agent_release_promotion(
        &self,
        agent_id: Uuid,
        release_id: Uuid,
        rejected_by: String,
        reason: Option<String>,
    ) -> Result<AgentRelease, AppError> {
        self.decide_agent_release_promotion(agent_id, release_id, "rejected", rejected_by, reason)
            .await
    }

    pub(crate) async fn rollback_agent_release(
        &self,
        agent_id: Uuid,
        release_id: Uuid,
    ) -> Result<AgentRelease, AppError> {
        self.get_agent(agent_id).await?;
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let release = store
                    .agent_releases
                    .get_mut(&release_id)
                    .ok_or_else(|| AppError::not_found("agent release not found"))?;
                if release.agent_id != agent_id {
                    return Err(AppError::not_found("agent release not found"));
                }
                if release.status != "promoted" {
                    return Err(AppError::bad_request("agent release is not promoted"));
                }
                release.status = "rolled_back".to_string();
                Ok(release.clone())
            }
            StoreBackend::Postgres(pool) => {
                let sql = format!(
                    "SELECT {AGENT_RELEASE_COLUMNS}
                     FROM agent_releases
                     WHERE tenant_id = $1 AND agent_id = $2 AND id = $3"
                );
                let existing = sqlx::query(&sql)
                    .bind(self.tenant_id)
                    .bind(agent_id)
                    .bind(release_id)
                    .fetch_optional(pool)
                    .await?
                    .ok_or_else(|| AppError::not_found("agent release not found"))
                    .and_then(agent_release_from_row)?;
                if existing.status != "promoted" {
                    return Err(AppError::bad_request("agent release is not promoted"));
                }
                let row = sqlx::query(
                    "UPDATE agent_releases
                     SET status = 'rolled_back'
                     WHERE tenant_id = $1 AND agent_id = $2 AND id = $3
                     RETURNING id, agent_id, agent_version_id, environment, status, eval_run_id, eval_score, min_score, requested_by, requested_at, request_reason, approver_subject, decision_by, decided_at, decision_reason, promoted_by, promoted_at, automation_policy, created_at",
                )
                .bind(self.tenant_id)
                .bind(agent_id)
                .bind(release_id)
                .fetch_one(pool)
                .await?;
                agent_release_from_row(row)
            }
        }
    }

    async fn validate_agent_release_input(
        &self,
        agent_id: Uuid,
        input: &CreateAgentRelease,
    ) -> Result<(Uuid, String, Uuid, f64, f64), AppError> {
        self.get_agent(agent_id).await?;
        let agent_version = if let Some(agent_version_id) = input.agent_version_id {
            self.list_agent_versions(agent_id)
                .await?
                .into_iter()
                .find(|version| version.id == agent_version_id)
                .ok_or_else(|| AppError::not_found("agent version not found"))?
        } else {
            self.current_agent_version(agent_id).await?
        };
        let eval_run = self
            .list_eval_runs(None)
            .await?
            .into_iter()
            .find(|run| run.id == input.eval_run_id)
            .ok_or_else(|| AppError::not_found("eval run not found"))?;
        if eval_run.agent_id != agent_id || eval_run.agent_version_id != agent_version.id {
            return Err(AppError::bad_request(
                "eval run must target the released agent version",
            ));
        }
        let min_score = input.min_score.unwrap_or(1.0);
        let score = eval_run.score.unwrap_or(0.0);
        if eval_run.status != "completed" || score < min_score {
            return Err(AppError::bad_request(format!(
                "eval gate failed: status={}, score={score:.4}, min_score={min_score:.4}",
                eval_run.status
            )));
        }
        Ok((
            agent_version.id,
            input.environment.clone(),
            eval_run.id,
            score,
            min_score,
        ))
    }

    async fn decide_agent_release_promotion(
        &self,
        agent_id: Uuid,
        release_id: Uuid,
        next_status: &str,
        decided_by: String,
        reason: Option<String>,
    ) -> Result<AgentRelease, AppError> {
        self.get_agent(agent_id).await?;
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let release = store
                    .agent_releases
                    .get_mut(&release_id)
                    .ok_or_else(|| AppError::not_found("agent release not found"))?;
                validate_release_decision(release, &decided_by)?;
                let now = Utc::now();
                release.status = next_status.to_string();
                release.decision_by = Some(decided_by.clone());
                release.decided_at = Some(now);
                release.decision_reason = reason;
                if next_status == "promoted" {
                    release.promoted_by = Some(decided_by);
                    release.promoted_at = Some(now);
                }
                Ok(release.clone())
            }
            StoreBackend::Postgres(pool) => {
                let sql = format!(
                    "SELECT {AGENT_RELEASE_COLUMNS}
                     FROM agent_releases
                     WHERE tenant_id = $1 AND agent_id = $2 AND id = $3"
                );
                let existing = sqlx::query(&sql)
                    .bind(self.tenant_id)
                    .bind(agent_id)
                    .bind(release_id)
                    .fetch_optional(pool)
                    .await?
                    .ok_or_else(|| AppError::not_found("agent release not found"))
                    .and_then(agent_release_from_row)?;
                validate_release_decision(&existing, &decided_by)?;
                let now = Utc::now();
                let promoted_by = (next_status == "promoted").then_some(decided_by.clone());
                let promoted_at = (next_status == "promoted").then_some(now);
                let row = sqlx::query(
                    "UPDATE agent_releases
                     SET status = $4,
                         decision_by = $5,
                         decided_at = $6,
                         decision_reason = $7,
                         promoted_by = $8,
                         promoted_at = $9
                     WHERE tenant_id = $1 AND agent_id = $2 AND id = $3
                     RETURNING id, agent_id, agent_version_id, environment, status, eval_run_id, eval_score, min_score, requested_by, requested_at, request_reason, approver_subject, decision_by, decided_at, decision_reason, promoted_by, promoted_at, automation_policy, created_at",
                )
                .bind(self.tenant_id)
                .bind(agent_id)
                .bind(release_id)
                .bind(next_status)
                .bind(&decided_by)
                .bind(now)
                .bind(&reason)
                .bind(&promoted_by)
                .bind(promoted_at)
                .fetch_one(pool)
                .await?;
                agent_release_from_row(row)
            }
        }
    }

    pub(crate) async fn automate_agent_release_decision(
        &self,
        agent_id: Uuid,
        release_id: Uuid,
        next_status: &str,
        decided_by: String,
        reason: String,
    ) -> Result<AgentRelease, AppError> {
        self.get_agent(agent_id).await?;
        if !matches!(next_status, "promoted" | "rejected") {
            return Err(AppError::bad_request(
                "unsupported automated release decision",
            ));
        }
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let release = store
                    .agent_releases
                    .get_mut(&release_id)
                    .ok_or_else(|| AppError::not_found("agent release not found"))?;
                if release.agent_id != agent_id {
                    return Err(AppError::not_found("agent release not found"));
                }
                if release.status != "pending_approval" {
                    return Err(AppError::bad_request(
                        "agent release is not pending approval",
                    ));
                }
                let now = Utc::now();
                release.status = next_status.to_string();
                release.decision_by = Some(decided_by.clone());
                release.decided_at = Some(now);
                release.decision_reason = Some(reason);
                if next_status == "promoted" {
                    release.promoted_by = Some(decided_by);
                    release.promoted_at = Some(now);
                }
                Ok(release.clone())
            }
            StoreBackend::Postgres(pool) => {
                let sql = format!(
                    "SELECT {AGENT_RELEASE_COLUMNS}
                     FROM agent_releases
                     WHERE tenant_id = $1 AND agent_id = $2 AND id = $3"
                );
                let existing = sqlx::query(&sql)
                    .bind(self.tenant_id)
                    .bind(agent_id)
                    .bind(release_id)
                    .fetch_optional(pool)
                    .await?
                    .ok_or_else(|| AppError::not_found("agent release not found"))
                    .and_then(agent_release_from_row)?;
                if existing.status != "pending_approval" {
                    return Err(AppError::bad_request(
                        "agent release is not pending approval",
                    ));
                }
                let now = Utc::now();
                let promoted_by = (next_status == "promoted").then_some(decided_by.clone());
                let promoted_at = (next_status == "promoted").then_some(now);
                let row = sqlx::query(
                    "UPDATE agent_releases
                     SET status = $4,
                         decision_by = $5,
                         decided_at = $6,
                         decision_reason = $7,
                         promoted_by = $8,
                         promoted_at = $9
                     WHERE tenant_id = $1 AND agent_id = $2 AND id = $3
                     RETURNING id, agent_id, agent_version_id, environment, status, eval_run_id, eval_score, min_score, requested_by, requested_at, request_reason, approver_subject, decision_by, decided_at, decision_reason, promoted_by, promoted_at, automation_policy, created_at",
                )
                .bind(self.tenant_id)
                .bind(agent_id)
                .bind(release_id)
                .bind(next_status)
                .bind(&decided_by)
                .bind(now)
                .bind(&reason)
                .bind(&promoted_by)
                .bind(promoted_at)
                .fetch_one(pool)
                .await?;
                agent_release_from_row(row)
            }
        }
    }
}

fn validate_release_decision(release: &AgentRelease, decided_by: &str) -> Result<(), AppError> {
    if release.status != "pending_approval" {
        return Err(AppError::bad_request(
            "agent release is not pending approval",
        ));
    }
    if release
        .requested_by
        .as_deref()
        .is_some_and(|requested_by| requested_by == decided_by)
    {
        return Err(AppError::forbidden(
            "release requester cannot approve or reject the same release",
        ));
    }
    if release
        .approver_subject
        .as_deref()
        .is_some_and(|approver| approver != decided_by)
    {
        return Err(AppError::forbidden(
            "release decision requires the delegated approver subject",
        ));
    }
    Ok(())
}
