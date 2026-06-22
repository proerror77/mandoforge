use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputer {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) profile: String,
    pub(crate) status: String,
    pub(crate) namespace: String,
    pub(crate) pod_name: Option<String>,
    pub(crate) workspace_path: String,
    pub(crate) state_mount_path: String,
    pub(crate) metadata: Value,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerLease {
    pub(crate) id: Uuid,
    pub(crate) remote_computer_id: Uuid,
    pub(crate) session_id: Option<Uuid>,
    pub(crate) status: String,
    pub(crate) worker_id: Option<String>,
    pub(crate) lease_expires_at: Option<DateTime<Utc>>,
    pub(crate) heartbeat_at: Option<DateTime<Utc>>,
    pub(crate) metadata: Value,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerAttachment {
    pub(crate) id: Uuid,
    pub(crate) remote_computer_id: Uuid,
    pub(crate) lease_id: Uuid,
    pub(crate) session_id: Uuid,
    pub(crate) status: String,
    pub(crate) attached_by: Option<String>,
    pub(crate) stale_after: Option<DateTime<Utc>>,
    pub(crate) released_at: Option<DateTime<Utc>>,
    pub(crate) metadata: Value,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerJobAssignment {
    pub(crate) id: Uuid,
    pub(crate) execution_job_id: Uuid,
    pub(crate) remote_computer_id: Uuid,
    pub(crate) lease_id: Uuid,
    pub(crate) session_id: Uuid,
    pub(crate) status: String,
    pub(crate) assigned_by: Option<String>,
    pub(crate) metadata: Value,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerStateLock {
    pub(crate) id: Uuid,
    pub(crate) lock_key: String,
    pub(crate) status: String,
    pub(crate) remote_computer_id: Option<Uuid>,
    pub(crate) lease_id: Option<Uuid>,
    pub(crate) session_id: Option<Uuid>,
    pub(crate) owner: Option<String>,
    pub(crate) expires_at: Option<DateTime<Utc>>,
    pub(crate) released_at: Option<DateTime<Utc>>,
    pub(crate) metadata: Value,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerSidecarHeartbeat {
    pub(crate) id: Uuid,
    pub(crate) remote_computer_id: Uuid,
    pub(crate) session_id: Option<Uuid>,
    pub(crate) assignment_id: Option<Uuid>,
    pub(crate) sidecar_name: String,
    pub(crate) status: String,
    pub(crate) observed_at: DateTime<Utc>,
    pub(crate) metadata: Value,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CreateRemoteComputer {
    pub(crate) name: String,
    pub(crate) profile: Option<String>,
    pub(crate) namespace: Option<String>,
    pub(crate) pod_name: Option<String>,
    pub(crate) workspace_path: Option<String>,
    pub(crate) state_mount_path: Option<String>,
    pub(crate) metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CreateRemoteComputerLease {
    pub(crate) session_id: Option<Uuid>,
    pub(crate) worker_id: Option<String>,
    pub(crate) lease_seconds: Option<i64>,
    pub(crate) metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct UpdateRemoteComputerLease {
    pub(crate) reason: Option<String>,
    pub(crate) metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CreateRemoteComputerAttachment {
    pub(crate) session_id: Uuid,
    pub(crate) attached_by: Option<String>,
    pub(crate) stale_after_seconds: Option<i64>,
    pub(crate) metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CreateRemoteComputerJobAssignment {
    pub(crate) lease_id: Uuid,
    pub(crate) assigned_by: Option<String>,
    pub(crate) metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CreateRemoteComputerStateLock {
    pub(crate) lock_key: String,
    pub(crate) remote_computer_id: Option<Uuid>,
    pub(crate) lease_id: Option<Uuid>,
    pub(crate) session_id: Option<Uuid>,
    pub(crate) owner: Option<String>,
    pub(crate) lease_seconds: Option<i64>,
    pub(crate) metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ReleaseRemoteComputerStateLock {
    pub(crate) reason: Option<String>,
    pub(crate) metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CreateRemoteComputerSidecarHeartbeat {
    pub(crate) remote_computer_id: Uuid,
    pub(crate) session_id: Option<Uuid>,
    pub(crate) assignment_id: Option<Uuid>,
    pub(crate) sidecar_name: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct UpdateRemoteComputerAttachment {
    pub(crate) reason: Option<String>,
    pub(crate) metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerReclaimRun {
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) status: String,
    pub(crate) stale_attachment_count: usize,
    pub(crate) reclaimed_attachment_count: usize,
    pub(crate) expired_lease_count: usize,
    pub(crate) reclaimed_lease_count: usize,
    pub(crate) attachments: Vec<RemoteComputerAttachment>,
    pub(crate) leases: Vec<RemoteComputerLease>,
    pub(crate) execution_enabled: bool,
}
