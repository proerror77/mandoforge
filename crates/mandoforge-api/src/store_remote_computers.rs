use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use sqlx::{Row, postgres::PgRow};
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::{
    AppError, AppState, CreateRemoteComputer, CreateRemoteComputerAttachment,
    CreateRemoteComputerJobAssignment, CreateRemoteComputerLease,
    CreateRemoteComputerSidecarHeartbeat, CreateRemoteComputerStateLock,
    ReleaseRemoteComputerStateLock, RemoteComputer, RemoteComputerAttachment,
    RemoteComputerJobAssignment, RemoteComputerLease, RemoteComputerSidecarHeartbeat,
    RemoteComputerStateLock, UpdateRemoteComputerAttachment, UpdateRemoteComputerLease,
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
                .bind(self.current_tenant_id())
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
                .bind(self.current_tenant_id())
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
                .bind(self.current_tenant_id())
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
        let lease_seconds = input.lease_seconds.unwrap_or(900);
        if lease_seconds <= 0 {
            return Err(AppError::bad_request(
                "remote computer lease_seconds must be positive",
            ));
        }
        let lease_seconds = lease_seconds.clamp(30, 86_400);
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
                    .get(&remote_computer_id)
                    .ok_or_else(|| AppError::not_found("Remote computer not found"))?;
                if computer.status != "available" {
                    return Err(AppError::bad_request(
                        "Remote computer is not available for lease",
                    ));
                }
                if store.remote_computer_leases.values().any(|existing| {
                    existing.remote_computer_id == remote_computer_id && existing.status == "leased"
                }) {
                    return Err(AppError::bad_request(
                        "Remote computer already has an active lease",
                    ));
                }
                let computer = store
                    .remote_computers
                    .get_mut(&remote_computer_id)
                    .ok_or_else(|| AppError::not_found("Remote computer not found"))?;
                computer.status = "leased".to_string();
                computer.updated_at = now;
                store.remote_computer_leases.insert(lease.id, lease.clone());
            }
            StoreBackend::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                let updated = sqlx::query(
                    "UPDATE remote_computers
                     SET status = 'leased', updated_at = $1
                     WHERE tenant_id = $2 AND id = $3 AND status = 'available'",
                )
                .bind(now)
                .bind(self.current_tenant_id())
                .bind(remote_computer_id)
                .execute(&mut *tx)
                .await?;
                if updated.rows_affected() == 0 {
                    let exists: Option<(Uuid,)> = sqlx::query_as(
                        "SELECT id
                         FROM remote_computers
                         WHERE tenant_id = $1 AND id = $2",
                    )
                    .bind(self.current_tenant_id())
                    .bind(remote_computer_id)
                    .fetch_optional(&mut *tx)
                    .await?;
                    if exists.is_some() {
                        return Err(AppError::bad_request(
                            "Remote computer is not available for lease",
                        ));
                    }
                    return Err(AppError::not_found("Remote computer not found"));
                }
                sqlx::query(
                    "INSERT INTO remote_computer_leases
                        (id, tenant_id, remote_computer_id, session_id, status, worker_id, lease_expires_at, heartbeat_at, metadata, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
                )
                .bind(lease.id)
                .bind(self.current_tenant_id())
                .bind(lease.remote_computer_id)
                .bind(lease.session_id)
                .bind(&lease.status)
                .bind(&lease.worker_id)
                .bind(lease.lease_expires_at)
                .bind(lease.heartbeat_at)
                .bind(&lease.metadata)
                .bind(lease.created_at)
                .bind(lease.updated_at)
                .execute(&mut *tx)
                .await?;
                tx.commit().await?;
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
                let existing = store
                    .remote_computer_leases
                    .get(&lease_id)
                    .cloned()
                    .ok_or_else(|| AppError::not_found("Remote computer lease not found"))?;
                if status == "leased" && existing.status != "leased" {
                    return Err(AppError::bad_request(
                        "Remote computer lease is not active for heartbeat",
                    ));
                }
                if (status == "released" || status == "failed") && existing.status == status {
                    return Ok(existing);
                }
                if (status == "released" || status == "failed") && existing.status != "leased" {
                    return Err(AppError::bad_request(
                        "Remote computer lease is not active for status transition",
                    ));
                }
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
                let has_other_active_lease =
                    store.remote_computer_leases.values().any(|existing| {
                        existing.id != lease.id
                            && existing.remote_computer_id == lease.remote_computer_id
                            && existing.status == "leased"
                    });
                if !has_other_active_lease
                    && (status == "released" || status == "failed")
                    && let Some(computer) =
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
                Ok(lease)
            }
            StoreBackend::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                let existing_row = sqlx::query(
                    "SELECT id, remote_computer_id, session_id, status, worker_id, lease_expires_at, heartbeat_at, metadata, created_at, updated_at
                     FROM remote_computer_leases
                     WHERE tenant_id = $1 AND id = $2
                     FOR UPDATE",
                )
                .bind(self.current_tenant_id())
                .bind(lease_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::not_found("Remote computer lease not found"))?;
                let existing = remote_computer_lease_from_row(existing_row)?;
                if status == "leased" && existing.status != "leased" {
                    return Err(AppError::bad_request(
                        "Remote computer lease is not active for heartbeat",
                    ));
                }
                if (status == "released" || status == "failed") && existing.status == status {
                    tx.commit().await?;
                    return Ok(existing);
                }
                if (status == "released" || status == "failed") && existing.status != "leased" {
                    return Err(AppError::bad_request(
                        "Remote computer lease is not active for status transition",
                    ));
                }
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
                .bind(self.current_tenant_id())
                .bind(lease_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::not_found("Remote computer lease not found"))?;
                let lease = remote_computer_lease_from_row(row)?;
                if status == "released" || status == "failed" {
                    let has_other_active_lease: Option<(Uuid,)> = sqlx::query_as(
                        "SELECT id
                         FROM remote_computer_leases
                         WHERE tenant_id = $1
                           AND remote_computer_id = $2
                           AND status = 'leased'
                           AND id <> $3
                         LIMIT 1",
                    )
                    .bind(self.current_tenant_id())
                    .bind(lease.remote_computer_id)
                    .bind(lease.id)
                    .fetch_optional(&mut *tx)
                    .await?;
                    let computer_status = if status == "released" {
                        "available"
                    } else {
                        "attention"
                    };
                    if has_other_active_lease.is_none() {
                        sqlx::query(
                            "UPDATE remote_computers
                             SET status = $1, updated_at = $2
                             WHERE tenant_id = $3 AND id = $4",
                        )
                        .bind(computer_status)
                        .bind(now)
                        .bind(self.current_tenant_id())
                        .bind(lease.remote_computer_id)
                        .execute(&mut *tx)
                        .await?;
                    }
                }
                tx.commit().await?;
                Ok(lease)
            }
        }
    }

    pub(crate) async fn list_remote_computer_attachments(
        &self,
    ) -> Result<Vec<RemoteComputerAttachment>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut attachments: Vec<_> = inner
                    .read()
                    .await
                    .remote_computer_attachments
                    .values()
                    .cloned()
                    .collect();
                attachments.sort_by_key(|attachment| attachment.created_at);
                attachments.reverse();
                Ok(attachments)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, remote_computer_id, lease_id, session_id, status, attached_by, stale_after, released_at, metadata, created_at, updated_at
                     FROM remote_computer_session_attachments
                     WHERE tenant_id = $1
                     ORDER BY created_at DESC",
                )
                .bind(self.current_tenant_id())
                .fetch_all(pool)
                .await?;
                rows.into_iter()
                    .map(remote_computer_attachment_from_row)
                    .collect()
            }
        }
    }

    pub(crate) async fn list_remote_computer_job_assignments(
        &self,
    ) -> Result<Vec<RemoteComputerJobAssignment>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut assignments: Vec<_> = inner
                    .read()
                    .await
                    .remote_computer_job_assignments
                    .values()
                    .cloned()
                    .collect();
                assignments.sort_by_key(|assignment| assignment.created_at);
                assignments.reverse();
                Ok(assignments)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, execution_job_id, remote_computer_id, lease_id, session_id, status, assigned_by, metadata, created_at, updated_at
                     FROM remote_computer_job_assignments
                     WHERE tenant_id = $1
                     ORDER BY created_at DESC",
                )
                .bind(self.current_tenant_id())
                .fetch_all(pool)
                .await?;
                rows.into_iter()
                    .map(remote_computer_job_assignment_from_row)
                    .collect()
            }
        }
    }

    pub(crate) async fn create_remote_computer_job_assignment(
        &self,
        execution_job_id: Uuid,
        session_id: Uuid,
        input: CreateRemoteComputerJobAssignment,
    ) -> Result<RemoteComputerJobAssignment, AppError> {
        let now = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let lease = store
                    .remote_computer_leases
                    .get(&input.lease_id)
                    .cloned()
                    .ok_or_else(|| AppError::not_found("Remote computer lease not found"))?;
                validate_remote_computer_job_assignment_lease(&lease, session_id)?;
                if store
                    .remote_computer_job_assignments
                    .values()
                    .any(|assignment| {
                        assignment.execution_job_id == execution_job_id
                            && assignment.status == "assigned"
                    })
                {
                    return Err(AppError::bad_request(
                        "execution job already has an active remote computer assignment",
                    ));
                }
                if store
                    .remote_computer_job_assignments
                    .values()
                    .any(|assignment| {
                        assignment.session_id == session_id && assignment.status == "assigned"
                    })
                {
                    return Err(AppError::bad_request(
                        "session already has an active remote computer assignment",
                    ));
                }
                let assignment = new_remote_computer_job_assignment(
                    execution_job_id,
                    session_id,
                    lease,
                    input,
                    now,
                );
                store
                    .remote_computer_job_assignments
                    .insert(assignment.id, assignment.clone());
                Ok(assignment)
            }
            StoreBackend::Postgres(pool) => {
                let lease_row = sqlx::query(
                    "SELECT id, remote_computer_id, session_id, status, worker_id, lease_expires_at, heartbeat_at, metadata, created_at, updated_at
                     FROM remote_computer_leases
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.current_tenant_id())
                .bind(input.lease_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("Remote computer lease not found"))?;
                let lease = remote_computer_lease_from_row(lease_row)?;
                validate_remote_computer_job_assignment_lease(&lease, session_id)?;
                let duplicate = sqlx::query(
                    "SELECT id
                     FROM remote_computer_job_assignments
                     WHERE tenant_id = $1 AND execution_job_id = $2 AND status = 'assigned'
                     LIMIT 1",
                )
                .bind(self.current_tenant_id())
                .bind(execution_job_id)
                .fetch_optional(pool)
                .await?;
                if duplicate.is_some() {
                    return Err(AppError::bad_request(
                        "execution job already has an active remote computer assignment",
                    ));
                }
                let duplicate_session = sqlx::query(
                    "SELECT id
                     FROM remote_computer_job_assignments
                     WHERE tenant_id = $1 AND session_id = $2 AND status = 'assigned'
                     LIMIT 1",
                )
                .bind(self.current_tenant_id())
                .bind(session_id)
                .fetch_optional(pool)
                .await?;
                if duplicate_session.is_some() {
                    return Err(AppError::bad_request(
                        "session already has an active remote computer assignment",
                    ));
                }
                let assignment = new_remote_computer_job_assignment(
                    execution_job_id,
                    session_id,
                    lease,
                    input,
                    now,
                );
                sqlx::query(
                    "INSERT INTO remote_computer_job_assignments
                        (id, tenant_id, execution_job_id, remote_computer_id, lease_id, session_id, status, assigned_by, metadata, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
                )
                .bind(assignment.id)
                .bind(self.current_tenant_id())
                .bind(assignment.execution_job_id)
                .bind(assignment.remote_computer_id)
                .bind(assignment.lease_id)
                .bind(assignment.session_id)
                .bind(&assignment.status)
                .bind(&assignment.assigned_by)
                .bind(&assignment.metadata)
                .bind(assignment.created_at)
                .bind(assignment.updated_at)
                .execute(pool)
                .await?;
                Ok(assignment)
            }
        }
    }

    pub(crate) async fn update_remote_computer_job_assignment_status(
        &self,
        assignment_id: Uuid,
        status: &str,
        metadata: serde_json::Value,
    ) -> Result<RemoteComputerJobAssignment, AppError> {
        let now = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let assignment = store
                    .remote_computer_job_assignments
                    .get_mut(&assignment_id)
                    .ok_or_else(|| {
                        AppError::not_found("Remote computer job assignment not found")
                    })?;
                assignment.status = status.to_string();
                assignment.metadata =
                    merge_remote_computer_assignment_metadata(&assignment.metadata, metadata);
                assignment.updated_at = now;
                Ok(assignment.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE remote_computer_job_assignments
                     SET status = $1,
                         metadata = metadata || $2::jsonb,
                         updated_at = $3
                     WHERE tenant_id = $4 AND id = $5
                     RETURNING id, execution_job_id, remote_computer_id, lease_id, session_id, status, assigned_by, metadata, created_at, updated_at",
                )
                .bind(status)
                .bind(metadata)
                .bind(now)
                .bind(self.current_tenant_id())
                .bind(assignment_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("Remote computer job assignment not found"))?;
                remote_computer_job_assignment_from_row(row)
            }
        }
    }

    pub(crate) async fn list_remote_computer_state_locks(
        &self,
    ) -> Result<Vec<RemoteComputerStateLock>, AppError> {
        let now = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut locks: Vec<_> = inner
                    .read()
                    .await
                    .remote_computer_state_locks
                    .values()
                    .cloned()
                    .collect();
                for lock in &mut locks {
                    if lock.status == "held"
                        && lock.expires_at.is_some_and(|expires| expires <= now)
                    {
                        lock.status = "expired".to_string();
                    }
                }
                locks.sort_by_key(|lock| lock.created_at);
                locks.reverse();
                Ok(locks)
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "UPDATE remote_computer_state_locks
                     SET status = 'expired', updated_at = $1
                     WHERE tenant_id = $2 AND status = 'held' AND expires_at IS NOT NULL AND expires_at <= $1",
                )
                .bind(now)
                .bind(self.current_tenant_id())
                .execute(pool)
                .await?;
                let rows = sqlx::query(
                    "SELECT id, lock_key, status, remote_computer_id, lease_id, session_id, owner, expires_at, released_at, metadata, created_at, updated_at
                     FROM remote_computer_state_locks
                     WHERE tenant_id = $1
                     ORDER BY created_at DESC",
                )
                .bind(self.current_tenant_id())
                .fetch_all(pool)
                .await?;
                rows.into_iter()
                    .map(remote_computer_state_lock_from_row)
                    .collect()
            }
        }
    }

    pub(crate) async fn acquire_remote_computer_state_lock(
        &self,
        input: CreateRemoteComputerStateLock,
    ) -> Result<RemoteComputerStateLock, AppError> {
        let now = Utc::now();
        let lock_key = normalize_remote_computer_lock_key(&input.lock_key)?;
        let lease_seconds = input.lease_seconds.unwrap_or(900).clamp(30, 86_400);
        let lock = RemoteComputerStateLock {
            id: Uuid::new_v4(),
            lock_key: lock_key.clone(),
            status: "held".to_string(),
            remote_computer_id: input.remote_computer_id,
            lease_id: input.lease_id,
            session_id: input.session_id,
            owner: input
                .owner
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            expires_at: Some(now + chrono::Duration::seconds(lease_seconds)),
            released_at: None,
            metadata: input.metadata.unwrap_or_else(|| json!({})),
            created_at: now,
            updated_at: now,
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let active_conflict = store.remote_computer_state_locks.values().any(|existing| {
                    existing.lock_key == lock_key
                        && existing.status == "held"
                        && existing.expires_at.is_none_or(|expires| expires > now)
                });
                if active_conflict {
                    return Err(AppError::bad_request(
                        "Remote Computer state lock is already held",
                    ));
                }
                store
                    .remote_computer_state_locks
                    .insert(lock.id, lock.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "UPDATE remote_computer_state_locks
                     SET status = 'expired', updated_at = $1
                     WHERE tenant_id = $2 AND status = 'held' AND expires_at IS NOT NULL AND expires_at <= $1",
                )
                .bind(now)
                .bind(self.current_tenant_id())
                .execute(pool)
                .await?;
                let existing: Option<(Uuid,)> = sqlx::query_as(
                    "SELECT id
                     FROM remote_computer_state_locks
                     WHERE tenant_id = $1 AND lock_key = $2 AND status = 'held'
                     LIMIT 1",
                )
                .bind(self.current_tenant_id())
                .bind(&lock.lock_key)
                .fetch_optional(pool)
                .await?;
                if existing.is_some() {
                    return Err(AppError::bad_request(
                        "Remote Computer state lock is already held",
                    ));
                }
                sqlx::query(
                    "INSERT INTO remote_computer_state_locks
                        (id, tenant_id, lock_key, status, remote_computer_id, lease_id, session_id, owner, expires_at, released_at, metadata, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
                )
                .bind(lock.id)
                .bind(self.current_tenant_id())
                .bind(&lock.lock_key)
                .bind(&lock.status)
                .bind(lock.remote_computer_id)
                .bind(lock.lease_id)
                .bind(lock.session_id)
                .bind(&lock.owner)
                .bind(lock.expires_at)
                .bind(lock.released_at)
                .bind(&lock.metadata)
                .bind(lock.created_at)
                .bind(lock.updated_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(lock)
    }

    pub(crate) async fn release_remote_computer_state_lock(
        &self,
        lock_id: Uuid,
        input: ReleaseRemoteComputerStateLock,
    ) -> Result<RemoteComputerStateLock, AppError> {
        let now = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let lock = store
                    .remote_computer_state_locks
                    .get_mut(&lock_id)
                    .ok_or_else(|| AppError::not_found("Remote Computer state lock not found"))?;
                lock.status = "released".to_string();
                lock.released_at = Some(now);
                lock.updated_at = now;
                if let Some(metadata) = input.metadata {
                    lock.metadata = metadata;
                }
                if let Some(reason) = input.reason {
                    lock.metadata["release_reason"] = json!(reason);
                }
                Ok(lock.clone())
            }
            StoreBackend::Postgres(pool) => {
                let metadata = input.metadata.unwrap_or_else(|| json!({}));
                let row = sqlx::query(
                    "UPDATE remote_computer_state_locks
                     SET status = 'released',
                         released_at = $1,
                         updated_at = $1,
                         metadata = CASE
                             WHEN $2::jsonb = '{}'::jsonb THEN metadata
                             ELSE $2::jsonb
                         END
                     WHERE tenant_id = $3 AND id = $4
                     RETURNING id, lock_key, status, remote_computer_id, lease_id, session_id, owner, expires_at, released_at, metadata, created_at, updated_at",
                )
                .bind(now)
                .bind(&metadata)
                .bind(self.current_tenant_id())
                .bind(lock_id)
                .fetch_optional(pool)
                .await?;
                row.map(remote_computer_state_lock_from_row)
                    .transpose()?
                    .ok_or_else(|| AppError::not_found("Remote Computer state lock not found"))
            }
        }
    }

    pub(crate) async fn list_remote_computer_sidecar_heartbeats(
        &self,
    ) -> Result<Vec<RemoteComputerSidecarHeartbeat>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut heartbeats: Vec<_> = inner
                    .read()
                    .await
                    .remote_computer_sidecar_heartbeats
                    .values()
                    .cloned()
                    .collect();
                heartbeats.sort_by_key(|heartbeat| heartbeat.observed_at);
                heartbeats.reverse();
                Ok(heartbeats)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, remote_computer_id, session_id, assignment_id, sidecar_name, status, observed_at, metadata, created_at
                     FROM remote_computer_sidecar_heartbeats
                     WHERE tenant_id = $1
                     ORDER BY observed_at DESC
                     LIMIT 250",
                )
                .bind(self.current_tenant_id())
                .fetch_all(pool)
                .await?;
                rows.into_iter()
                    .map(remote_computer_sidecar_heartbeat_from_row)
                    .collect()
            }
        }
    }

    pub(crate) async fn record_remote_computer_sidecar_heartbeat(
        &self,
        input: CreateRemoteComputerSidecarHeartbeat,
    ) -> Result<RemoteComputerSidecarHeartbeat, AppError> {
        let now = Utc::now();
        let heartbeat = RemoteComputerSidecarHeartbeat {
            id: Uuid::new_v4(),
            remote_computer_id: input.remote_computer_id,
            session_id: input.session_id,
            assignment_id: input.assignment_id,
            sidecar_name: input
                .sidecar_name
                .unwrap_or_else(|| "artifact-discovery".to_string())
                .trim()
                .to_string(),
            status: input
                .status
                .unwrap_or_else(|| "ok".to_string())
                .trim()
                .to_string(),
            observed_at: now,
            metadata: input.metadata.unwrap_or_else(|| json!({})),
            created_at: now,
        };
        if heartbeat.sidecar_name.is_empty() {
            return Err(AppError::bad_request("sidecar_name is required"));
        }
        if heartbeat.status.is_empty() {
            return Err(AppError::bad_request(
                "sidecar heartbeat status is required",
            ));
        }
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                if !store
                    .remote_computers
                    .contains_key(&heartbeat.remote_computer_id)
                {
                    return Err(AppError::not_found("Remote computer not found"));
                }
                store
                    .remote_computer_sidecar_heartbeats
                    .insert(heartbeat.id, heartbeat.clone());
            }
            StoreBackend::Postgres(pool) => {
                let exists: Option<(Uuid,)> = sqlx::query_as(
                    "SELECT id FROM remote_computers WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.current_tenant_id())
                .bind(heartbeat.remote_computer_id)
                .fetch_optional(pool)
                .await?;
                if exists.is_none() {
                    return Err(AppError::not_found("Remote computer not found"));
                }
                sqlx::query(
                    "INSERT INTO remote_computer_sidecar_heartbeats
                        (id, tenant_id, remote_computer_id, session_id, assignment_id, sidecar_name, status, observed_at, metadata, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
                )
                .bind(heartbeat.id)
                .bind(self.current_tenant_id())
                .bind(heartbeat.remote_computer_id)
                .bind(heartbeat.session_id)
                .bind(heartbeat.assignment_id)
                .bind(&heartbeat.sidecar_name)
                .bind(&heartbeat.status)
                .bind(heartbeat.observed_at)
                .bind(&heartbeat.metadata)
                .bind(heartbeat.created_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(heartbeat)
    }

    pub(crate) async fn list_stale_remote_computer_attachments(
        &self,
    ) -> Result<Vec<RemoteComputerAttachment>, AppError> {
        let now = Utc::now();
        let attachments = self.list_remote_computer_attachments().await?;
        Ok(attachments
            .into_iter()
            .filter(|attachment| {
                attachment.status == "attached"
                    && attachment
                        .stale_after
                        .is_some_and(|stale_after| stale_after <= now)
            })
            .collect())
    }

    pub(crate) async fn create_remote_computer_attachment(
        &self,
        lease_id: Uuid,
        input: CreateRemoteComputerAttachment,
    ) -> Result<RemoteComputerAttachment, AppError> {
        let now = Utc::now();
        let stale_after_seconds = input
            .stale_after_seconds
            .unwrap_or(900)
            .clamp(-86_400, 86_400);
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let lease = store
                    .remote_computer_leases
                    .get(&lease_id)
                    .cloned()
                    .ok_or_else(|| AppError::not_found("Remote computer lease not found"))?;
                if lease.status != "leased" {
                    return Err(AppError::bad_request(
                        "only active remote computer leases can be attached",
                    ));
                }
                if let Some(leased_session_id) = lease.session_id
                    && leased_session_id != input.session_id
                {
                    return Err(AppError::bad_request(
                        "attachment session must match the lease session",
                    ));
                }
                if store
                    .remote_computer_attachments
                    .values()
                    .any(|attachment| {
                        attachment.lease_id == lease_id && attachment.status == "attached"
                    })
                {
                    return Err(AppError::bad_request(
                        "remote computer lease already has an active attachment",
                    ));
                }
                let attachment = RemoteComputerAttachment {
                    id: Uuid::new_v4(),
                    remote_computer_id: lease.remote_computer_id,
                    lease_id,
                    session_id: input.session_id,
                    status: "attached".to_string(),
                    attached_by: input
                        .attached_by
                        .map(|value| value.trim().to_string())
                        .filter(|value| !value.is_empty()),
                    stale_after: Some(now + chrono::Duration::seconds(stale_after_seconds)),
                    released_at: None,
                    metadata: input.metadata.unwrap_or_else(|| json!({})),
                    created_at: now,
                    updated_at: now,
                };
                store
                    .remote_computer_attachments
                    .insert(attachment.id, attachment.clone());
                Ok(attachment)
            }
            StoreBackend::Postgres(pool) => {
                let lease_row = sqlx::query(
                    "SELECT id, remote_computer_id, session_id, status, worker_id, lease_expires_at, heartbeat_at, metadata, created_at, updated_at
                     FROM remote_computer_leases
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.current_tenant_id())
                .bind(lease_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("Remote computer lease not found"))?;
                let lease = remote_computer_lease_from_row(lease_row)?;
                if lease.status != "leased" {
                    return Err(AppError::bad_request(
                        "only active remote computer leases can be attached",
                    ));
                }
                if let Some(leased_session_id) = lease.session_id
                    && leased_session_id != input.session_id
                {
                    return Err(AppError::bad_request(
                        "attachment session must match the lease session",
                    ));
                }
                let duplicate = sqlx::query(
                    "SELECT id
                     FROM remote_computer_session_attachments
                     WHERE tenant_id = $1 AND lease_id = $2 AND status = 'attached'
                     LIMIT 1",
                )
                .bind(self.current_tenant_id())
                .bind(lease_id)
                .fetch_optional(pool)
                .await?;
                if duplicate.is_some() {
                    return Err(AppError::bad_request(
                        "remote computer lease already has an active attachment",
                    ));
                }
                let attachment = RemoteComputerAttachment {
                    id: Uuid::new_v4(),
                    remote_computer_id: lease.remote_computer_id,
                    lease_id,
                    session_id: input.session_id,
                    status: "attached".to_string(),
                    attached_by: input
                        .attached_by
                        .map(|value| value.trim().to_string())
                        .filter(|value| !value.is_empty()),
                    stale_after: Some(now + chrono::Duration::seconds(stale_after_seconds)),
                    released_at: None,
                    metadata: input.metadata.unwrap_or_else(|| json!({})),
                    created_at: now,
                    updated_at: now,
                };
                sqlx::query(
                    "INSERT INTO remote_computer_session_attachments
                        (id, tenant_id, remote_computer_id, lease_id, session_id, status, attached_by, stale_after, released_at, metadata, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
                )
                .bind(attachment.id)
                .bind(self.current_tenant_id())
                .bind(attachment.remote_computer_id)
                .bind(attachment.lease_id)
                .bind(attachment.session_id)
                .bind(&attachment.status)
                .bind(&attachment.attached_by)
                .bind(attachment.stale_after)
                .bind(attachment.released_at)
                .bind(&attachment.metadata)
                .bind(attachment.created_at)
                .bind(attachment.updated_at)
                .execute(pool)
                .await?;
                Ok(attachment)
            }
        }
    }

    pub(crate) async fn release_remote_computer_attachment(
        &self,
        attachment_id: Uuid,
        input: UpdateRemoteComputerAttachment,
    ) -> Result<RemoteComputerAttachment, AppError> {
        let now = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let attachment = store
                    .remote_computer_attachments
                    .get_mut(&attachment_id)
                    .ok_or_else(|| AppError::not_found("Remote computer attachment not found"))?;
                attachment.status = "released".to_string();
                attachment.released_at = Some(now);
                if let Some(metadata) = input.metadata {
                    attachment.metadata = metadata;
                }
                if let Some(reason) = input.reason {
                    attachment.metadata["reason"] = json!(reason);
                }
                attachment.updated_at = now;
                Ok(attachment.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE remote_computer_session_attachments
                     SET status = 'released',
                         released_at = $1,
                         metadata = CASE
                           WHEN $2::jsonb IS NULL AND $3::text IS NULL THEN metadata
                           ELSE COALESCE($2::jsonb, metadata) || CASE WHEN $3::text IS NULL THEN '{}'::jsonb ELSE jsonb_build_object('reason', $3::text) END
                         END,
                         updated_at = $4
                     WHERE tenant_id = $5 AND id = $6
                     RETURNING id, remote_computer_id, lease_id, session_id, status, attached_by, stale_after, released_at, metadata, created_at, updated_at",
                )
                .bind(now)
                .bind(input.metadata)
                .bind(input.reason)
                .bind(now)
                .bind(self.current_tenant_id())
                .bind(attachment_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("Remote computer attachment not found"))?;
                remote_computer_attachment_from_row(row)
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

fn remote_computer_attachment_from_row(row: PgRow) -> Result<RemoteComputerAttachment, AppError> {
    Ok(RemoteComputerAttachment {
        id: row.try_get("id")?,
        remote_computer_id: row.try_get("remote_computer_id")?,
        lease_id: row.try_get("lease_id")?,
        session_id: row.try_get("session_id")?,
        status: row.try_get("status")?,
        attached_by: row.try_get("attached_by")?,
        stale_after: row.try_get("stale_after")?,
        released_at: row.try_get("released_at")?,
        metadata: row.try_get("metadata")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn new_remote_computer_job_assignment(
    execution_job_id: Uuid,
    session_id: Uuid,
    lease: RemoteComputerLease,
    input: CreateRemoteComputerJobAssignment,
    now: chrono::DateTime<Utc>,
) -> RemoteComputerJobAssignment {
    RemoteComputerJobAssignment {
        id: Uuid::new_v4(),
        execution_job_id,
        remote_computer_id: lease.remote_computer_id,
        lease_id: lease.id,
        session_id,
        status: "assigned".to_string(),
        assigned_by: input
            .assigned_by
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        metadata: input.metadata.unwrap_or_else(|| json!({})),
        created_at: now,
        updated_at: now,
    }
}

fn validate_remote_computer_job_assignment_lease(
    lease: &RemoteComputerLease,
    session_id: Uuid,
) -> Result<(), AppError> {
    if lease.status != "leased" {
        return Err(AppError::bad_request(
            "only active remote computer leases can receive execution handoff",
        ));
    }
    if let Some(leased_session_id) = lease.session_id
        && leased_session_id != session_id
    {
        return Err(AppError::bad_request(
            "execution handoff session must match the lease session",
        ));
    }
    Ok(())
}

fn remote_computer_job_assignment_from_row(
    row: PgRow,
) -> Result<RemoteComputerJobAssignment, AppError> {
    Ok(RemoteComputerJobAssignment {
        id: row.try_get("id")?,
        execution_job_id: row.try_get("execution_job_id")?,
        remote_computer_id: row.try_get("remote_computer_id")?,
        lease_id: row.try_get("lease_id")?,
        session_id: row.try_get("session_id")?,
        status: row.try_get("status")?,
        assigned_by: row.try_get("assigned_by")?,
        metadata: row.try_get("metadata")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn remote_computer_state_lock_from_row(row: PgRow) -> Result<RemoteComputerStateLock, AppError> {
    Ok(RemoteComputerStateLock {
        id: row.try_get("id")?,
        lock_key: row.try_get("lock_key")?,
        status: row.try_get("status")?,
        remote_computer_id: row.try_get("remote_computer_id")?,
        lease_id: row.try_get("lease_id")?,
        session_id: row.try_get("session_id")?,
        owner: row.try_get("owner")?,
        expires_at: row.try_get("expires_at")?,
        released_at: row.try_get("released_at")?,
        metadata: row.try_get("metadata")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn remote_computer_sidecar_heartbeat_from_row(
    row: PgRow,
) -> Result<RemoteComputerSidecarHeartbeat, AppError> {
    Ok(RemoteComputerSidecarHeartbeat {
        id: row.try_get("id")?,
        remote_computer_id: row.try_get("remote_computer_id")?,
        session_id: row.try_get("session_id")?,
        assignment_id: row.try_get("assignment_id")?,
        sidecar_name: row.try_get("sidecar_name")?,
        status: row.try_get("status")?,
        observed_at: row.try_get("observed_at")?,
        metadata: row.try_get("metadata")?,
        created_at: row.try_get("created_at")?,
    })
}

fn normalize_remote_computer_lock_key(lock_key: &str) -> Result<String, AppError> {
    let lock_key = lock_key.trim();
    if lock_key.is_empty() {
        return Err(AppError::bad_request(
            "Remote Computer state lock key is required",
        ));
    }
    if lock_key.starts_with('/') || lock_key.split('/').any(|segment| segment == "..") {
        return Err(AppError::bad_request(
            "Remote Computer state lock key must be relative and stay inside /agent-state",
        ));
    }
    let allowed_prefix = ["memory/", "notes/", "skills/", "artifacts/", ".mandoforge/"];
    if !allowed_prefix
        .iter()
        .any(|prefix| lock_key.starts_with(prefix))
    {
        return Err(AppError::bad_request(
            "Remote Computer state lock key must target memory, notes, skills, artifacts, or .mandoforge state",
        ));
    }
    Ok(lock_key.to_string())
}

fn merge_remote_computer_assignment_metadata(
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
