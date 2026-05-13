use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use sqlx::{Row, postgres::PgRow};
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::{
    AppError, AppState, CreateRemoteComputer, CreateRemoteComputerLease, RemoteComputer,
    RemoteComputerLease, UpdateRemoteComputerLease,
};

impl AppState {
    pub(crate) async fn list_remote_computers(&self) -> Result<Vec<RemoteComputer>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut records: Vec<_> = inner
                    .read()
                    .await
                    .remote_computers
                    .values()
                    .cloned()
                    .collect();
                records.sort_by_key(|record| record.created_at);
                records.reverse();
                Ok(records)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, name, profile, status, namespace, pod_name, workspace_path, state_mount_path, metadata, created_at, updated_at
                     FROM remote_computers
                     WHERE tenant_id = $1
                     ORDER BY created_at DESC",
                )
                .bind(self.tenant_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(remote_computer_from_row).collect()
            }
        }
    }

    pub(crate) async fn create_remote_computer(
        &self,
        input: CreateRemoteComputer,
    ) -> Result<RemoteComputer, AppError> {
        let now = Utc::now();
        let record = RemoteComputer {
            id: Uuid::new_v4(),
            name: input.name.trim().to_string(),
            profile: input
                .profile
                .unwrap_or_else(|| "workspace-write".to_string())
                .trim()
                .to_string(),
            status: "available".to_string(),
            namespace: input
                .namespace
                .unwrap_or_else(|| "agent-os".to_string())
                .trim()
                .to_string(),
            pod_name: input
                .pod_name
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            workspace_path: input
                .workspace_path
                .unwrap_or_else(|| "/workspace".to_string())
                .trim()
                .to_string(),
            state_mount_path: input
                .state_mount_path
                .unwrap_or_else(|| "/agent-state".to_string())
                .trim()
                .to_string(),
            metadata: input.metadata.unwrap_or_else(|| json!({})),
            created_at: now,
            updated_at: now,
        };
        if record.name.is_empty() {
            return Err(AppError::bad_request("remote computer name is required"));
        }
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .remote_computers
                    .insert(record.id, record.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO remote_computers
                        (id, tenant_id, name, profile, status, namespace, pod_name, workspace_path, state_mount_path, metadata, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
                )
                .bind(record.id)
                .bind(self.tenant_id)
                .bind(&record.name)
                .bind(&record.profile)
                .bind(&record.status)
                .bind(&record.namespace)
                .bind(&record.pod_name)
                .bind(&record.workspace_path)
                .bind(&record.state_mount_path)
                .bind(&record.metadata)
                .bind(record.created_at)
                .bind(record.updated_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(record)
    }

    pub(crate) async fn list_remote_computer_leases(
        &self,
    ) -> Result<Vec<RemoteComputerLease>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut leases: Vec<_> = inner
                    .read()
                    .await
                    .remote_computer_leases
                    .values()
                    .cloned()
                    .collect();
                leases.sort_by_key(|lease| lease.created_at);
                leases.reverse();
                Ok(leases)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, remote_computer_id, session_id, status, worker_id, lease_expires_at, heartbeat_at, metadata, created_at, updated_at
                     FROM remote_computer_leases
                     WHERE tenant_id = $1
                     ORDER BY created_at DESC",
                )
                .bind(self.tenant_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter()
                    .map(remote_computer_lease_from_row)
                    .collect()
            }
        }
    }

    pub(crate) async fn create_remote_computer_lease(
        &self,
        remote_computer_id: Uuid,
        input: CreateRemoteComputerLease,
    ) -> Result<RemoteComputerLease, AppError> {
        let now = Utc::now();
        let lease_seconds = input.lease_seconds.unwrap_or(900).clamp(60, 86_400);
        let lease = RemoteComputerLease {
            id: Uuid::new_v4(),
            remote_computer_id,
            session_id: input.session_id,
            status: "leased".to_string(),
            worker_id: input.worker_id,
            lease_expires_at: Some(now + chrono::Duration::seconds(lease_seconds)),
            heartbeat_at: Some(now),
            metadata: input.metadata.unwrap_or_else(|| json!({})),
            created_at: now,
            updated_at: now,
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let computer = store
                    .remote_computers
                    .get_mut(&remote_computer_id)
                    .ok_or_else(|| AppError::not_found("Remote computer not found"))?;
                computer.status = "leased".to_string();
                computer.updated_at = now;
                store.remote_computer_leases.insert(lease.id, lease.clone());
            }
            StoreBackend::Postgres(pool) => {
                let updated = sqlx::query(
                    "UPDATE remote_computers
                     SET status = 'leased', updated_at = $1
                     WHERE tenant_id = $2 AND id = $3",
                )
                .bind(now)
                .bind(self.tenant_id)
                .bind(remote_computer_id)
                .execute(pool)
                .await?;
                if updated.rows_affected() == 0 {
                    return Err(AppError::not_found("Remote computer not found"));
                }
                sqlx::query(
                    "INSERT INTO remote_computer_leases
                        (id, tenant_id, remote_computer_id, session_id, status, worker_id, lease_expires_at, heartbeat_at, metadata, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
                )
                .bind(lease.id)
                .bind(self.tenant_id)
                .bind(lease.remote_computer_id)
                .bind(lease.session_id)
                .bind(&lease.status)
                .bind(&lease.worker_id)
                .bind(lease.lease_expires_at)
                .bind(lease.heartbeat_at)
                .bind(&lease.metadata)
                .bind(lease.created_at)
                .bind(lease.updated_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(lease)
    }

    pub(crate) async fn update_remote_computer_lease_status(
        &self,
        lease_id: Uuid,
        status: &str,
        input: UpdateRemoteComputerLease,
    ) -> Result<RemoteComputerLease, AppError> {
        let now = Utc::now();
        let heartbeat_at = if status == "leased" { Some(now) } else { None };
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let lease = store
                    .remote_computer_leases
                    .get_mut(&lease_id)
                    .ok_or_else(|| AppError::not_found("Remote computer lease not found"))?;
                lease.status = status.to_string();
                if let Some(heartbeat_at) = heartbeat_at {
                    lease.heartbeat_at = Some(heartbeat_at);
                }
                if let Some(metadata) = input.metadata {
                    lease.metadata = metadata;
                }
                if let Some(reason) = input.reason {
                    lease.metadata["reason"] = json!(reason);
                }
                lease.updated_at = now;
                let lease = lease.clone();
                if status == "released" || status == "failed" {
                    if let Some(computer) =
                        store.remote_computers.get_mut(&lease.remote_computer_id)
                    {
                        computer.status = if status == "released" {
                            "available"
                        } else {
                            "attention"
                        }
                        .to_string();
                        computer.updated_at = now;
                    }
                }
                Ok(lease)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE remote_computer_leases
                     SET status = $1,
                         heartbeat_at = COALESCE($2, heartbeat_at),
                         metadata = CASE
                           WHEN $3::jsonb IS NULL AND $4::text IS NULL THEN metadata
                           ELSE COALESCE($3::jsonb, metadata) || CASE WHEN $4::text IS NULL THEN '{}'::jsonb ELSE jsonb_build_object('reason', $4::text) END
                         END,
                         updated_at = $5
                     WHERE tenant_id = $6 AND id = $7
                     RETURNING id, remote_computer_id, session_id, status, worker_id, lease_expires_at, heartbeat_at, metadata, created_at, updated_at",
                )
                .bind(status)
                .bind(heartbeat_at)
                .bind(input.metadata)
                .bind(input.reason)
                .bind(now)
                .bind(self.tenant_id)
                .bind(lease_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("Remote computer lease not found"))?;
                let lease = remote_computer_lease_from_row(row)?;
                if status == "released" || status == "failed" {
                    let computer_status = if status == "released" {
                        "available"
                    } else {
                        "attention"
                    };
                    sqlx::query(
                        "UPDATE remote_computers
                         SET status = $1, updated_at = $2
                         WHERE tenant_id = $3 AND id = $4",
                    )
                    .bind(computer_status)
                    .bind(now)
                    .bind(self.tenant_id)
                    .bind(lease.remote_computer_id)
                    .execute(pool)
                    .await?;
                }
                Ok(lease)
            }
        }
    }
}

fn remote_computer_from_row(row: PgRow) -> Result<RemoteComputer, AppError> {
    Ok(RemoteComputer {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        profile: row.try_get("profile")?,
        status: row.try_get("status")?,
        namespace: row.try_get("namespace")?,
        pod_name: row.try_get("pod_name")?,
        workspace_path: row.try_get("workspace_path")?,
        state_mount_path: row.try_get("state_mount_path")?,
        metadata: row.try_get("metadata")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn remote_computer_lease_from_row(row: PgRow) -> Result<RemoteComputerLease, AppError> {
    Ok(RemoteComputerLease {
        id: row.try_get("id")?,
        remote_computer_id: row.try_get("remote_computer_id")?,
        session_id: row.try_get("session_id")?,
        status: row.try_get("status")?,
        worker_id: row.try_get("worker_id")?,
        lease_expires_at: row.try_get("lease_expires_at")?,
        heartbeat_at: row.try_get("heartbeat_at")?,
        metadata: row.try_get("metadata")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}
