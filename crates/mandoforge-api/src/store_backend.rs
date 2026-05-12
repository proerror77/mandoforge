use std::{collections::HashMap, sync::Arc};

use sqlx::PgPool;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    Agent, AgentVersion, Approval, Artifact, AuditLog, Membership, Organization, Project, Session,
    SessionEvent, Team, ToolCall,
};

#[derive(Default)]
pub(crate) struct MemoryStore {
    pub(crate) agents: HashMap<Uuid, Agent>,
    pub(crate) agent_versions: HashMap<Uuid, Vec<AgentVersion>>,
    pub(crate) sessions: HashMap<Uuid, Session>,
    pub(crate) events: HashMap<Uuid, Vec<SessionEvent>>,
    pub(crate) approvals: HashMap<Uuid, Approval>,
    pub(crate) artifacts: HashMap<Uuid, Artifact>,
    pub(crate) tool_calls: HashMap<Uuid, ToolCall>,
    pub(crate) audit_logs: HashMap<Uuid, AuditLog>,
    pub(crate) organizations: HashMap<Uuid, Organization>,
    pub(crate) teams: HashMap<Uuid, Team>,
    pub(crate) projects: HashMap<Uuid, Project>,
    pub(crate) memberships: HashMap<Uuid, Membership>,
}

#[derive(Clone)]
pub(crate) enum StoreBackend {
    Memory(Arc<RwLock<MemoryStore>>),
    Postgres(PgPool),
}
