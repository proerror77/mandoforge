use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::agent_release_from_row;
use crate::{AgentRelease, AppError, AppState, CreateAgentRelease};

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
                let rows = sqlx::query(
                    "SELECT id, agent_id, agent_version_id, environment, status, eval_run_id, eval_score, min_score, promoted_by, promoted_at, created_at
                     FROM agent_releases
                     WHERE tenant_id = $1 AND agent_id = $2
                     ORDER BY created_at DESC",
                )
                .bind(self.tenant_id)
                .bind(agent_id)
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
        self.get_agent(agent_id).await?;
        let agent_version = if let Some(agent_version_id) = input.agent_version_id {
            let version = self
                .list_agent_versions(agent_id)
                .await?
                .into_iter()
                .find(|version| version.id == agent_version_id)
                .ok_or_else(|| AppError::not_found("agent version not found"))?;
            version
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
        let now = Utc::now();
        let release = AgentRelease {
            id: Uuid::new_v4(),
            agent_id,
            agent_version_id: agent_version.id,
            environment: input.environment,
            status: "promoted".to_string(),
            eval_run_id: Some(eval_run.id),
            eval_score: Some(score),
            min_score,
            promoted_by: Some(promoted_by),
            promoted_at: Some(now),
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
                    "INSERT INTO agent_releases (id, tenant_id, agent_id, agent_version_id, environment, status, eval_run_id, eval_score, min_score, promoted_by, promoted_at, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
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
                .bind(&release.promoted_by)
                .bind(release.promoted_at)
                .bind(release.created_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(release)
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
                let existing = sqlx::query(
                    "SELECT id, agent_id, agent_version_id, environment, status, eval_run_id, eval_score, min_score, promoted_by, promoted_at, created_at
                     FROM agent_releases
                     WHERE tenant_id = $1 AND agent_id = $2 AND id = $3",
                )
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
                     RETURNING id, agent_id, agent_version_id, environment, status, eval_run_id, eval_score, min_score, promoted_by, promoted_at, created_at",
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
}
