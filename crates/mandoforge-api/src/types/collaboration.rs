use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AgentTeammate {
    pub(crate) id: Uuid,
    pub(crate) agent_id: Option<Uuid>,
    pub(crate) display_name: String,
    pub(crate) handle: Option<String>,
    pub(crate) role: String,
    pub(crate) status: String,
    pub(crate) metadata: Value,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateAgentTeammate {
    #[serde(default)]
    pub(crate) agent_id: Option<Uuid>,
    pub(crate) display_name: String,
    #[serde(default)]
    pub(crate) handle: Option<String>,
    #[serde(default = "default_agent_teammate_role")]
    pub(crate) role: String,
    #[serde(default = "default_collaboration_record_status")]
    pub(crate) status: String,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Squad {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) purpose: Option<String>,
    pub(crate) status: String,
    pub(crate) metadata: Value,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateSquad {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) purpose: Option<String>,
    #[serde(default = "default_collaboration_record_status")]
    pub(crate) status: String,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SquadMember {
    pub(crate) id: Uuid,
    pub(crate) squad_id: Uuid,
    pub(crate) teammate_id: Uuid,
    pub(crate) role: String,
    pub(crate) status: String,
    pub(crate) metadata: Value,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateSquadMember {
    pub(crate) teammate_id: Uuid,
    #[serde(default = "default_squad_member_role")]
    pub(crate) role: String,
    #[serde(default = "default_collaboration_record_status")]
    pub(crate) status: String,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkItem {
    pub(crate) id: Uuid,
    pub(crate) organization_id: Option<Uuid>,
    pub(crate) team_id: Option<Uuid>,
    pub(crate) project_id: Option<Uuid>,
    pub(crate) title: String,
    pub(crate) description: Option<String>,
    pub(crate) source: String,
    pub(crate) source_url: Option<String>,
    pub(crate) status: String,
    pub(crate) priority: String,
    pub(crate) assignee: Option<String>,
    pub(crate) metadata: Value,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateWorkItem {
    #[serde(default)]
    pub(crate) organization_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) team_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) project_id: Option<Uuid>,
    pub(crate) title: String,
    #[serde(default)]
    pub(crate) description: Option<String>,
    #[serde(default = "default_work_item_source")]
    pub(crate) source: String,
    #[serde(default)]
    pub(crate) source_url: Option<String>,
    #[serde(default = "default_work_item_status")]
    pub(crate) status: String,
    #[serde(default = "default_work_item_priority")]
    pub(crate) priority: String,
    #[serde(default)]
    pub(crate) assignee: Option<String>,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkItemAssignment {
    pub(crate) id: Uuid,
    pub(crate) work_item_id: Uuid,
    pub(crate) assignee_kind: String,
    pub(crate) assignee_id: String,
    pub(crate) role: String,
    pub(crate) status: String,
    pub(crate) assigned_by: Option<String>,
    pub(crate) metadata: Value,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateWorkItemAssignment {
    pub(crate) assignee_kind: String,
    pub(crate) assignee_id: String,
    #[serde(default = "default_work_item_assignment_role")]
    pub(crate) role: String,
    #[serde(default = "default_work_item_assignment_status")]
    pub(crate) status: String,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkItemReview {
    pub(crate) id: Uuid,
    pub(crate) work_item_id: Uuid,
    pub(crate) reviewer_kind: String,
    pub(crate) reviewer_id: String,
    pub(crate) status: String,
    pub(crate) decision: Option<String>,
    pub(crate) summary: Option<String>,
    pub(crate) metadata: Value,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateWorkItemReview {
    pub(crate) reviewer_kind: String,
    pub(crate) reviewer_id: String,
    #[serde(default = "default_work_item_review_status")]
    pub(crate) status: String,
    #[serde(default)]
    pub(crate) decision: Option<String>,
    #[serde(default)]
    pub(crate) summary: Option<String>,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkItemActivityEntry {
    pub(crate) id: Uuid,
    pub(crate) work_item_id: Uuid,
    pub(crate) event_type: String,
    pub(crate) actor_subject: Option<String>,
    pub(crate) subject_type: Option<String>,
    pub(crate) subject_id: Option<Uuid>,
    pub(crate) summary: String,
    pub(crate) metadata: Value,
    pub(crate) created_at: DateTime<Utc>,
}

fn default_work_item_source() -> String {
    "manual".to_string()
}

fn default_work_item_status() -> String {
    "open".to_string()
}

fn default_work_item_priority() -> String {
    "normal".to_string()
}

fn default_agent_teammate_role() -> String {
    "teammate".to_string()
}

fn default_squad_member_role() -> String {
    "member".to_string()
}

fn default_collaboration_record_status() -> String {
    "active".to_string()
}

fn default_work_item_assignment_role() -> String {
    "owner".to_string()
}

fn default_work_item_assignment_status() -> String {
    "assigned".to_string()
}

fn default_work_item_review_status() -> String {
    "requested".to_string()
}
