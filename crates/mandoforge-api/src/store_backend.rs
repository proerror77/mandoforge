use std::{collections::HashMap, sync::Arc};

use sqlx::PgPool;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    Agent, AgentHandoffAssignment, AgentHandoffEvent, AgentRelease, AgentRuntimeProfile,
    AgentVersion, Approval, ApprovalEscalationRule, ApprovalGroup,
    ApprovalNotificationChannelPolicy, Artifact, AuditLog, CodexAppServerRun, ContextPacket,
    CostAlertRoute, EvalCase, EvalDataset, EvalRun, ManagerAgentPlan, McpServerRecord, Membership,
    MemoryWritebackCandidate, Organization, PolicyRevision, Project, ProviderAccess,
    ProviderRecord, RemoteComputer, RemoteComputerAttachment, RemoteComputerJobAssignment,
    RemoteComputerLease, RemoteComputerSidecarHeartbeat, RemoteComputerStateLock, SecretRecord,
    SemanticLink, SemanticObject, SemanticSource, Session, SessionEvent, Team, TenantInvitation,
    ToolCall, UsageRollup, WorkflowPackInstallation, WorkflowPackProfileAsset,
};

#[derive(Default)]
pub(crate) struct MemoryStore {
    pub(crate) agents: HashMap<Uuid, Agent>,
    pub(crate) agent_runtime_profiles: HashMap<Uuid, AgentRuntimeProfile>,
    pub(crate) agent_releases: HashMap<Uuid, AgentRelease>,
    pub(crate) policy_revisions: HashMap<Uuid, PolicyRevision>,
    pub(crate) secret_records: HashMap<Uuid, SecretRecord>,
    pub(crate) agent_versions: HashMap<Uuid, Vec<AgentVersion>>,
    pub(crate) sessions: HashMap<Uuid, Session>,
    pub(crate) events: HashMap<Uuid, Vec<SessionEvent>>,
    pub(crate) approvals: HashMap<Uuid, Approval>,
    pub(crate) approval_groups: HashMap<Uuid, ApprovalGroup>,
    pub(crate) approval_escalation_rules: HashMap<Uuid, ApprovalEscalationRule>,
    pub(crate) approval_notification_channel_policies:
        HashMap<Uuid, ApprovalNotificationChannelPolicy>,
    pub(crate) artifacts: HashMap<Uuid, Artifact>,
    pub(crate) tool_calls: HashMap<Uuid, ToolCall>,
    pub(crate) audit_logs: HashMap<Uuid, AuditLog>,
    pub(crate) cost_alert_routes: HashMap<Uuid, CostAlertRoute>,
    pub(crate) organizations: HashMap<Uuid, Organization>,
    pub(crate) teams: HashMap<Uuid, Team>,
    pub(crate) projects: HashMap<Uuid, Project>,
    pub(crate) memberships: HashMap<Uuid, Membership>,
    pub(crate) tenant_invitations: HashMap<Uuid, TenantInvitation>,
    pub(crate) provider_access: HashMap<Uuid, ProviderAccess>,
    pub(crate) providers: HashMap<Uuid, ProviderRecord>,
    pub(crate) mcp_servers: HashMap<Uuid, McpServerRecord>,
    pub(crate) eval_datasets: HashMap<Uuid, EvalDataset>,
    pub(crate) eval_cases: HashMap<Uuid, EvalCase>,
    pub(crate) eval_runs: HashMap<Uuid, EvalRun>,
    pub(crate) usage_rollups: HashMap<Uuid, UsageRollup>,
    pub(crate) codex_app_server_runs: HashMap<Uuid, CodexAppServerRun>,
    pub(crate) remote_computers: HashMap<Uuid, RemoteComputer>,
    pub(crate) remote_computer_leases: HashMap<Uuid, RemoteComputerLease>,
    pub(crate) remote_computer_attachments: HashMap<Uuid, RemoteComputerAttachment>,
    pub(crate) remote_computer_job_assignments: HashMap<Uuid, RemoteComputerJobAssignment>,
    pub(crate) remote_computer_state_locks: HashMap<Uuid, RemoteComputerStateLock>,
    pub(crate) remote_computer_sidecar_heartbeats: HashMap<Uuid, RemoteComputerSidecarHeartbeat>,
    pub(crate) agent_handoff_assignments: HashMap<Uuid, AgentHandoffAssignment>,
    pub(crate) agent_handoff_events: HashMap<Uuid, AgentHandoffEvent>,
    pub(crate) manager_agent_plans: HashMap<Uuid, ManagerAgentPlan>,
    pub(crate) workflow_pack_installations: HashMap<Uuid, WorkflowPackInstallation>,
    pub(crate) workflow_pack_profile_assets: HashMap<Uuid, WorkflowPackProfileAsset>,
    pub(crate) semantic_sources: HashMap<Uuid, SemanticSource>,
    pub(crate) semantic_objects: HashMap<Uuid, SemanticObject>,
    pub(crate) semantic_links: HashMap<Uuid, SemanticLink>,
    pub(crate) context_packets: HashMap<Uuid, ContextPacket>,
    pub(crate) memory_writeback_candidates: HashMap<Uuid, MemoryWritebackCandidate>,
}

#[derive(Clone)]
pub(crate) enum StoreBackend {
    Memory(Arc<RwLock<MemoryStore>>),
    Postgres(PgPool),
}
