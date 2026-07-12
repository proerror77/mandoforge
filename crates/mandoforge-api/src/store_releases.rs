use anyhow::Result;
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::agent_release_from_row;
use crate::{AgentRelease, AgentVersion, AppError, AppState, CreateAgentRelease};

pub(crate) const AGENT_RELEASE_COLUMNS: &str = "id, agent_id, agent_version_id, environment, status, eval_run_id, eval_score, min_score, requested_by, requested_at, request_reason, approver_subject, decision_by, decided_at, decision_reason, promoted_by, promoted_at, automation_policy, created_at";

pub(crate) fn new_workflow_pack_agent_release(
    installation_id: Uuid,
    agent_id: Uuid,
    agent_version_id: Uuid,
    environment: &str,
    promoted_by: &str,
    gate_evidence: &Value,
    promoted_at: DateTime<Utc>,
) -> AgentRelease {
    let actor = promoted_by.to_string();
    AgentRelease {
        id: Uuid::new_v4(),
        agent_id,
        agent_version_id,
        environment: environment.to_string(),
        status: "promoted".to_string(),
        eval_run_id: None,
        eval_score: None,
        min_score: 1.0,
        requested_by: Some(actor.clone()),
        requested_at: Some(promoted_at),
        request_reason: Some("workflow pack release".to_string()),
        approver_subject: Some(actor.clone()),
        decision_by: Some(actor.clone()),
        decided_at: Some(promoted_at),
        decision_reason: Some("workflow pack eval and release gates passed".to_string()),
        promoted_by: Some(actor),
        promoted_at: Some(promoted_at),
        automation_policy: json!({
            "source": "workflow_pack_release",
            "workflow_pack_installation_id": installation_id,
            "gate_evidence": gate_evidence,
        }),
        created_at: promoted_at,
    }
}

pub(crate) async fn require_workflow_pack_agent_version_tx(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    agent_id: Uuid,
    agent_version_id: Uuid,
) -> Result<(), AppError> {
    let version_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1
            FROM agent_versions AS versions
            INNER JOIN agents ON agents.id = versions.agent_id
            WHERE agents.tenant_id = $1
              AND versions.agent_id = $2
              AND versions.id = $3
              AND agents.archived_at IS NULL
        )",
    )
    .bind(tenant_id)
    .bind(agent_id)
    .bind(agent_version_id)
    .fetch_one(&mut **tx)
    .await?;
    if !version_exists {
        return Err(AppError::bad_request(
            "workflow pack agent binding targets an unknown agent version",
        ));
    }
    Ok(())
}

pub(crate) async fn insert_or_get_promoted_agent_release_tx(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    release: &AgentRelease,
) -> Result<AgentRelease, AppError> {
    let insert_sql = format!(
        "INSERT INTO agent_releases
            (id, tenant_id, agent_id, agent_version_id, environment, status, eval_run_id, eval_score, min_score, requested_by, requested_at, request_reason, approver_subject, decision_by, decided_at, decision_reason, promoted_by, promoted_at, automation_policy, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)
         ON CONFLICT (tenant_id, agent_id, agent_version_id, lower(environment))
             WHERE status = 'promoted'
         DO NOTHING
         RETURNING {AGENT_RELEASE_COLUMNS}"
    );
    let inserted = sqlx::query(&insert_sql)
        .bind(release.id)
        .bind(tenant_id)
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
        .fetch_optional(&mut **tx)
        .await?;
    if let Some(row) = inserted {
        return agent_release_from_row(row);
    }

    let select_sql = format!(
        "SELECT {AGENT_RELEASE_COLUMNS}
         FROM agent_releases
         WHERE tenant_id = $1
           AND agent_id = $2
           AND agent_version_id = $3
           AND lower(environment) = lower($4)
           AND status = 'promoted'
         LIMIT 1"
    );
    let row = sqlx::query(&select_sql)
        .bind(tenant_id)
        .bind(release.agent_id)
        .bind(release.agent_version_id)
        .bind(&release.environment)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| {
            AppError::conflict(
                "promoted agent release disappeared during concurrent workflow pack release",
            )
        })?;
    agent_release_from_row(row)
}

async fn lock_agent_release_target_tx(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    agent_id: Uuid,
    agent_version_id: Uuid,
    environment: &str,
) -> Result<(), AppError> {
    let promotion_lock_key = format!(
        "{}:{}:{}:{}",
        tenant_id,
        agent_id,
        agent_version_id,
        environment.to_ascii_lowercase()
    );
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(promotion_lock_key)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

impl AppState {
    pub(crate) async fn agent_version_has_promoted_release(
        &self,
        agent_id: Uuid,
        agent_version_id: Uuid,
        environment: &str,
    ) -> Result<bool, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                Ok(inner.read().await.agent_releases.values().any(|release| {
                    release.agent_id == agent_id
                        && release.agent_version_id == agent_version_id
                        && release.status == "promoted"
                        && release.environment.eq_ignore_ascii_case(environment)
                }))
            }
            StoreBackend::Postgres(pool) => {
                let exists: bool = sqlx::query_scalar(
                    "SELECT EXISTS(
                        SELECT 1
                        FROM agent_releases
                        WHERE tenant_id = $1
                          AND agent_id = $2
                          AND agent_version_id = $3
                          AND lower(environment) = lower($4)
                          AND status = 'promoted'
                    )",
                )
                .bind(self.current_tenant_id())
                .bind(agent_id)
                .bind(agent_version_id)
                .bind(environment)
                .fetch_one(pool)
                .await?;
                Ok(exists)
            }
        }
    }

    pub(crate) async fn promoted_agent_version(
        &self,
        agent_id: Uuid,
        environment: &str,
    ) -> Result<AgentVersion, AppError> {
        let release = self
            .list_agent_releases(agent_id)
            .await?
            .into_iter()
            .filter(|release| {
                release.status == "promoted"
                    && release.environment.eq_ignore_ascii_case(environment)
            })
            .max_by_key(|release| {
                (
                    release.promoted_at.unwrap_or(release.created_at),
                    release.created_at,
                    release.id,
                )
            })
            .ok_or_else(|| {
                AppError::forbidden(format!(
                    "agent has no promoted release for environment {environment}"
                ))
            })?;
        self.list_agent_versions(agent_id)
            .await?
            .into_iter()
            .find(|version| version.id == release.agent_version_id)
            .ok_or_else(|| AppError::not_found("promoted agent version not found"))
    }

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
                    .bind(self.current_tenant_id())
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
                    .bind(self.current_tenant_id())
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
                    .filter(|release| {
                        matches!(
                            release.status.as_str(),
                            "pending_approval" | "promotion_in_progress" | "promotion_failed"
                        )
                    })
                    .cloned()
                    .collect();
                releases.sort_by_key(|release| release.created_at);
                Ok(releases)
            }
            StoreBackend::Postgres(pool) => {
                let sql = format!(
                    "SELECT {AGENT_RELEASE_COLUMNS}
                     FROM agent_releases
                     WHERE tenant_id = $1
                       AND status IN ('pending_approval', 'promotion_in_progress', 'promotion_failed')
                     ORDER BY created_at ASC"
                );
                let rows = sqlx::query(&sql)
                    .bind(self.current_tenant_id())
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
                let mut store = inner.write().await;
                let existing = store.agent_releases.values().find(|existing| {
                    existing.agent_id == release.agent_id
                        && existing.agent_version_id == release.agent_version_id
                        && existing
                            .environment
                            .eq_ignore_ascii_case(&release.environment)
                        && existing.status == "promoted"
                });
                if let Some(existing) = existing {
                    if existing.automation_policy["source"] != "workflow_pack_release" {
                        return Ok(existing.clone());
                    }
                    let existing_id = existing.id;
                    store
                        .agent_releases
                        .get_mut(&existing_id)
                        .expect("promoted agent release selected from store")
                        .status = "superseded".to_string();
                }
                store.agent_releases.insert(release.id, release.clone());
                Ok(release)
            }
            StoreBackend::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                lock_agent_release_target_tx(
                    &mut tx,
                    self.current_tenant_id(),
                    release.agent_id,
                    release.agent_version_id,
                    &release.environment,
                )
                .await?;
                let existing_sql = format!(
                    "SELECT {AGENT_RELEASE_COLUMNS}
                     FROM agent_releases
                     WHERE tenant_id = $1
                       AND agent_id = $2
                       AND agent_version_id = $3
                       AND lower(environment) = lower($4)
                       AND status = 'promoted'
                     LIMIT 1
                     FOR UPDATE"
                );
                let existing = sqlx::query(&existing_sql)
                    .bind(self.current_tenant_id())
                    .bind(release.agent_id)
                    .bind(release.agent_version_id)
                    .bind(&release.environment)
                    .fetch_optional(&mut *tx)
                    .await?
                    .map(agent_release_from_row)
                    .transpose()?;
                if let Some(existing) = existing {
                    if existing.automation_policy["source"] != "workflow_pack_release" {
                        tx.commit().await?;
                        return Ok(existing);
                    }
                    sqlx::query(
                        "UPDATE agent_releases
                         SET status = 'superseded'
                         WHERE tenant_id = $1 AND id = $2 AND status = 'promoted'",
                    )
                    .bind(self.current_tenant_id())
                    .bind(existing.id)
                    .execute(&mut *tx)
                    .await?;
                }
                let stored = insert_or_get_promoted_agent_release_tx(
                    &mut tx,
                    self.current_tenant_id(),
                    &release,
                )
                .await?;
                tx.commit().await?;
                Ok(stored)
            }
        }
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
                .bind(self.current_tenant_id())
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

    pub(crate) async fn begin_agent_release_promotion(
        &self,
        agent_id: Uuid,
        release_id: Uuid,
        decided_by: String,
        reason: String,
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
                if release.status == "promotion_in_progress" {
                    validate_agent_release_decider(release, &decided_by)?;
                    if release.decision_by.as_deref() != Some(decided_by.as_str()) {
                        return Err(AppError::forbidden(
                            "agent release promotion is owned by another approver",
                        ));
                    }
                    return Ok(release.clone());
                }
                validate_agent_release_decision(release, &decided_by)?;
                let now = Utc::now();
                release.status = "promotion_in_progress".to_string();
                release.decision_by = Some(decided_by);
                release.decided_at = Some(now);
                release.decision_reason = Some(reason);
                release.promoted_by = None;
                release.promoted_at = None;
                Ok(release.clone())
            }
            StoreBackend::Postgres(pool) => {
                let sql = format!(
                    "SELECT {AGENT_RELEASE_COLUMNS}
                     FROM agent_releases
                     WHERE tenant_id = $1 AND agent_id = $2 AND id = $3"
                );
                let existing = sqlx::query(&sql)
                    .bind(self.current_tenant_id())
                    .bind(agent_id)
                    .bind(release_id)
                    .fetch_optional(pool)
                    .await?
                    .ok_or_else(|| AppError::not_found("agent release not found"))
                    .and_then(agent_release_from_row)?;
                if existing.status == "promotion_in_progress" {
                    validate_agent_release_decider(&existing, &decided_by)?;
                    if existing.decision_by.as_deref() != Some(decided_by.as_str()) {
                        return Err(AppError::forbidden(
                            "agent release promotion is owned by another approver",
                        ));
                    }
                    return Ok(existing);
                }
                validate_agent_release_decision(&existing, &decided_by)?;
                let now = Utc::now();
                let row = sqlx::query(
                    "UPDATE agent_releases
                     SET status = 'promotion_in_progress',
                         decision_by = $4,
                         decided_at = $5,
                         decision_reason = $6,
                         promoted_by = NULL,
                         promoted_at = NULL
                     WHERE tenant_id = $1
                       AND agent_id = $2
                       AND id = $3
                       AND status IN ('pending_approval', 'promotion_failed')
                     RETURNING id, agent_id, agent_version_id, environment, status, eval_run_id, eval_score, min_score, requested_by, requested_at, request_reason, approver_subject, decision_by, decided_at, decision_reason, promoted_by, promoted_at, automation_policy, created_at",
                )
                .bind(self.current_tenant_id())
                .bind(agent_id)
                .bind(release_id)
                .bind(&decided_by)
                .bind(now)
                .bind(&reason)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| {
                    AppError::bad_request("agent release promotion state changed concurrently")
                })?;
                agent_release_from_row(row)
            }
        }
    }

    pub(crate) async fn complete_agent_release_promotion(
        &self,
        agent_id: Uuid,
        release_id: Uuid,
        promoted_by: String,
    ) -> Result<AgentRelease, AppError> {
        self.get_agent(agent_id).await?;
        let now = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let current = store
                    .agent_releases
                    .get(&release_id)
                    .cloned()
                    .ok_or_else(|| AppError::not_found("agent release not found"))?;
                if current.agent_id != agent_id {
                    return Err(AppError::not_found("agent release not found"));
                }
                if current.status != "promotion_in_progress"
                    || current.decision_by.as_deref() != Some(promoted_by.as_str())
                {
                    return Err(AppError::bad_request(
                        "agent release is not in promotion by this approver",
                    ));
                }
                for sibling in store.agent_releases.values_mut() {
                    if sibling.id != current.id
                        && sibling.agent_id == current.agent_id
                        && sibling.agent_version_id == current.agent_version_id
                        && sibling
                            .environment
                            .eq_ignore_ascii_case(&current.environment)
                        && sibling.status == "promoted"
                    {
                        sibling.status = "superseded".to_string();
                    }
                }
                let release = store
                    .agent_releases
                    .get_mut(&release_id)
                    .expect("agent release checked before promotion completion");
                release.status = "promoted".to_string();
                release.promoted_by = Some(promoted_by);
                release.promoted_at = Some(now);
                Ok(release.clone())
            }
            StoreBackend::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                let select_sql = format!(
                    "SELECT {AGENT_RELEASE_COLUMNS}
                     FROM agent_releases
                     WHERE tenant_id = $1 AND agent_id = $2 AND id = $3
                     FOR UPDATE"
                );
                let current = sqlx::query(&select_sql)
                    .bind(self.current_tenant_id())
                    .bind(agent_id)
                    .bind(release_id)
                    .fetch_optional(&mut *tx)
                    .await?
                    .ok_or_else(|| AppError::not_found("agent release not found"))
                    .and_then(agent_release_from_row)?;
                if current.status != "promotion_in_progress"
                    || current.decision_by.as_deref() != Some(promoted_by.as_str())
                {
                    return Err(AppError::bad_request(
                        "agent release is not in promotion by this approver",
                    ));
                }
                lock_agent_release_target_tx(
                    &mut tx,
                    self.current_tenant_id(),
                    current.agent_id,
                    current.agent_version_id,
                    &current.environment,
                )
                .await?;
                sqlx::query(
                    "UPDATE agent_releases
                     SET status = 'superseded'
                     WHERE tenant_id = $1
                       AND id <> $2
                       AND agent_id = $3
                       AND agent_version_id = $4
                       AND lower(environment) = lower($5)
                       AND status = 'promoted'",
                )
                .bind(self.current_tenant_id())
                .bind(release_id)
                .bind(current.agent_id)
                .bind(current.agent_version_id)
                .bind(&current.environment)
                .execute(&mut *tx)
                .await?;
                let row = sqlx::query(
                    "UPDATE agent_releases
                     SET status = 'promoted', promoted_by = $4, promoted_at = $5
                     WHERE tenant_id = $1
                       AND agent_id = $2
                       AND id = $3
                       AND status = 'promotion_in_progress'
                       AND decision_by = $4
                     RETURNING id, agent_id, agent_version_id, environment, status, eval_run_id, eval_score, min_score, requested_by, requested_at, request_reason, approver_subject, decision_by, decided_at, decision_reason, promoted_by, promoted_at, automation_policy, created_at",
                )
                .bind(self.current_tenant_id())
                .bind(agent_id)
                .bind(release_id)
                .bind(&promoted_by)
                .bind(now)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| {
                    AppError::bad_request("agent release is not in promotion by this approver")
                })?;
                let release = agent_release_from_row(row)?;
                tx.commit().await?;
                Ok(release)
            }
        }
    }

    pub(crate) async fn fail_agent_release_promotion(
        &self,
        agent_id: Uuid,
        release_id: Uuid,
        decided_by: String,
        reason: String,
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
                if release.status != "promotion_in_progress"
                    || release.decision_by.as_deref() != Some(decided_by.as_str())
                {
                    return Err(AppError::bad_request(
                        "agent release is not in promotion by this approver",
                    ));
                }
                release.status = "promotion_failed".to_string();
                release.decision_reason = Some(reason);
                release.promoted_by = None;
                release.promoted_at = None;
                Ok(release.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE agent_releases
                     SET status = 'promotion_failed',
                         decision_reason = $5,
                         promoted_by = NULL,
                         promoted_at = NULL
                     WHERE tenant_id = $1
                       AND agent_id = $2
                       AND id = $3
                       AND status = 'promotion_in_progress'
                       AND decision_by = $4
                     RETURNING id, agent_id, agent_version_id, environment, status, eval_run_id, eval_score, min_score, requested_by, requested_at, request_reason, approver_subject, decision_by, decided_at, decision_reason, promoted_by, promoted_at, automation_policy, created_at",
                )
                .bind(self.current_tenant_id())
                .bind(agent_id)
                .bind(release_id)
                .bind(&decided_by)
                .bind(&reason)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| {
                    AppError::bad_request("agent release is not in promotion by this approver")
                })?;
                agent_release_from_row(row)
            }
        }
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
        let release = match &self.store {
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
                    .bind(self.current_tenant_id())
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
                     WHERE tenant_id = $1 AND agent_id = $2 AND id = $3 AND status = 'promoted'
                     RETURNING id, agent_id, agent_version_id, environment, status, eval_run_id, eval_score, min_score, requested_by, requested_at, request_reason, approver_subject, decision_by, decided_at, decision_reason, promoted_by, promoted_at, automation_policy, created_at",
                )
                .bind(self.current_tenant_id())
                .bind(agent_id)
                .bind(release_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::bad_request("agent release is not promoted"))?;
                agent_release_from_row(row)
            }
        }?;
        Ok(release)
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
        let release = match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let release = store
                    .agent_releases
                    .get_mut(&release_id)
                    .ok_or_else(|| AppError::not_found("agent release not found"))?;
                validate_agent_release_decision(release, &decided_by)?;
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
                    .bind(self.current_tenant_id())
                    .bind(agent_id)
                    .bind(release_id)
                    .fetch_optional(pool)
                    .await?
                    .ok_or_else(|| AppError::not_found("agent release not found"))
                    .and_then(agent_release_from_row)?;
                validate_agent_release_decision(&existing, &decided_by)?;
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
                     WHERE tenant_id = $1
                       AND agent_id = $2
                       AND id = $3
                       AND status IN ('pending_approval', 'promotion_failed')
                     RETURNING id, agent_id, agent_version_id, environment, status, eval_run_id, eval_score, min_score, requested_by, requested_at, request_reason, approver_subject, decision_by, decided_at, decision_reason, promoted_by, promoted_at, automation_policy, created_at",
                )
                .bind(self.current_tenant_id())
                .bind(agent_id)
                .bind(release_id)
                .bind(next_status)
                .bind(&decided_by)
                .bind(now)
                .bind(&reason)
                .bind(&promoted_by)
                .bind(promoted_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::bad_request("agent release is not pending approval"))?;
                agent_release_from_row(row)
            }
        }?;
        Ok(release)
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
        if next_status == "promoted" {
            self.begin_agent_release_promotion(agent_id, release_id, decided_by.clone(), reason)
                .await?;
            return self
                .complete_agent_release_promotion(agent_id, release_id, decided_by)
                .await;
        }
        let release = match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let release = store
                    .agent_releases
                    .get_mut(&release_id)
                    .ok_or_else(|| AppError::not_found("agent release not found"))?;
                if release.agent_id != agent_id {
                    return Err(AppError::not_found("agent release not found"));
                }
                if !matches!(
                    release.status.as_str(),
                    "pending_approval" | "promotion_failed"
                ) {
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
                    .bind(self.current_tenant_id())
                    .bind(agent_id)
                    .bind(release_id)
                    .fetch_optional(pool)
                    .await?
                    .ok_or_else(|| AppError::not_found("agent release not found"))
                    .and_then(agent_release_from_row)?;
                if !matches!(
                    existing.status.as_str(),
                    "pending_approval" | "promotion_failed"
                ) {
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
                     WHERE tenant_id = $1
                       AND agent_id = $2
                       AND id = $3
                       AND status IN ('pending_approval', 'promotion_failed')
                     RETURNING id, agent_id, agent_version_id, environment, status, eval_run_id, eval_score, min_score, requested_by, requested_at, request_reason, approver_subject, decision_by, decided_at, decision_reason, promoted_by, promoted_at, automation_policy, created_at",
                )
                .bind(self.current_tenant_id())
                .bind(agent_id)
                .bind(release_id)
                .bind(next_status)
                .bind(&decided_by)
                .bind(now)
                .bind(&reason)
                .bind(&promoted_by)
                .bind(promoted_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::bad_request("agent release is not pending approval"))?;
                agent_release_from_row(row)
            }
        }?;
        Ok(release)
    }
}

pub(crate) fn validate_agent_release_decision(
    release: &AgentRelease,
    decided_by: &str,
) -> Result<(), AppError> {
    if !matches!(
        release.status.as_str(),
        "pending_approval" | "promotion_failed"
    ) {
        return Err(AppError::bad_request(
            "agent release is not pending approval",
        ));
    }
    validate_agent_release_decider(release, decided_by)
}

fn validate_agent_release_decider(
    release: &AgentRelease,
    decided_by: &str,
) -> Result<(), AppError> {
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
