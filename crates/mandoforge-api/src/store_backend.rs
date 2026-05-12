use std::{collections::HashMap, sync::Arc};

use sqlx::PgPool;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    Agent, AgentVersion, Approval, Artifact, AuditLog, EvalCase, EvalDataset, EvalRun, Membership,
    Organization, Project, ProviderAccess, ProviderRecord, Session, SessionEvent, Team, ToolCall,
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
    pub(crate) provider_access: HashMap<Uuid, ProviderAccess>,
    pub(crate) providers: HashMap<Uuid, ProviderRecord>,
    pub(crate) eval_datasets: HashMap<Uuid, EvalDataset>,
    pub(crate) eval_cases: HashMap<Uuid, EvalCase>,
    pub(crate) eval_runs: HashMap<Uuid, EvalRun>,
}

#[derive(Clone)]
pub(crate) enum StoreBackend {
    Memory(Arc<RwLock<MemoryStore>>),
    Postgres(PgPool),
}
