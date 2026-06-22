use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Session {
    pub(crate) id: Uuid,
    pub(crate) agent_id: Uuid,
    pub(crate) agent_version_id: Option<Uuid>,
    pub(crate) environment_id: Option<Uuid>,
    pub(crate) title: String,
    pub(crate) status: SessionStatus,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SessionStatus {
    Idle,
    Running,
    RequiresAction,
    Rescheduling,
    Terminated,
    Failed,
}

impl SessionStatus {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::RequiresAction => "requires_action",
            Self::Rescheduling => "rescheduling",
            Self::Terminated => "terminated",
            Self::Failed => "failed",
        }
    }
}

impl From<String> for SessionStatus {
    fn from(value: String) -> Self {
        match value.as_str() {
            "idle" | "created" => Self::Idle,
            "running" => Self::Running,
            "requires_action" | "waiting_approval" => Self::RequiresAction,
            "rescheduling" => Self::Rescheduling,
            "terminated" | "completed" => Self::Terminated,
            "failed" => Self::Failed,
            _ => Self::Idle,
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateSession {
    pub(crate) agent_id: Uuid,
    #[serde(default)]
    pub(crate) environment_id: Option<Uuid>,
    #[serde(default = "crate::default_session_title")]
    pub(crate) title: String,
    pub(crate) message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AddMessage {
    pub(crate) message: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SendSessionEvents {
    #[serde(default)]
    pub(crate) events: Vec<IncomingSessionEvent>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct StreamEventsQuery {
    pub(crate) after_seq: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct IncomingSessionEvent {
    #[serde(rename = "type")]
    pub(crate) event_type: String,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SessionEvent {
    pub(crate) id: Uuid,
    pub(crate) session_id: Uuid,
    pub(crate) seq: i64,
    pub(crate) parent_event_id: Option<Uuid>,
    pub(crate) actor_type: String,
    pub(crate) actor_id: Option<Uuid>,
    pub(crate) event_type: String,
    pub(crate) payload: Value,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SessionLoopJobStatus {
    Queued,
    Running,
    Completed,
    Failed,
}

impl SessionLoopJobStatus {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

impl From<String> for SessionLoopJobStatus {
    fn from(value: String) -> Self {
        match value.as_str() {
            "running" => Self::Running,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            _ => Self::Queued,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SessionLoopJob {
    pub(crate) id: Uuid,
    pub(crate) session_id: Uuid,
    pub(crate) environment_id: Option<Uuid>,
    pub(crate) status: SessionLoopJobStatus,
    pub(crate) trigger_event_id: Option<Uuid>,
    pub(crate) pending_event_seq_start: Option<i64>,
    pub(crate) pending_event_seq_end: Option<i64>,
    pub(crate) processed_event_seq: Option<i64>,
    pub(crate) reason: String,
    pub(crate) enqueued_at: DateTime<Utc>,
    pub(crate) started_at: Option<DateTime<Utc>>,
    pub(crate) completed_at: Option<DateTime<Utc>>,
    pub(crate) worker_id: Option<String>,
    pub(crate) lease_expires_at: Option<DateTime<Utc>>,
    pub(crate) attempt_count: i32,
    pub(crate) max_attempts: i32,
    pub(crate) last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SessionThread {
    pub(crate) id: Uuid,
    pub(crate) session_id: Uuid,
    pub(crate) parent_thread_id: Option<Uuid>,
    pub(crate) thread_kind: String,
    pub(crate) agent_id: Uuid,
    pub(crate) agent_version_id: Option<Uuid>,
    pub(crate) environment_id: Option<Uuid>,
    pub(crate) source_handoff_id: Option<Uuid>,
    pub(crate) specialist_session_id: Option<Uuid>,
    pub(crate) status: String,
    pub(crate) title: String,
    pub(crate) context: Value,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}
