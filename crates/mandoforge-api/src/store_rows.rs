use anyhow::Result;
use serde_json::{Value, json};
use sqlx::{Row, postgres::PgRow};

use crate::{
    Agent, AgentHandoffAssignment, AgentHandoffEvent, AgentRelease, AgentRuntimeProfile,
    AgentVersion, AppError, Approval, ApprovalEscalationRule, ApprovalGroup,
    ApprovalNotificationChannelPolicy, Artifact, AuditLog, CostAlertRoute, EvalCase, EvalDataset,
    EvalRun, ManagerAgentPlan, McpServerRecord, Membership, Organization, PolicyRevision, Project,
    ProviderAccess, ProviderRecord, SecretRecord, Session, SessionEvent, Team, TenantInvitation,
    ToolCall, UsageRollup, WorkflowPackInstallation, WorkflowPackProfileAsset,
};

pub(crate) fn agent_from_row(row: PgRow) -> Result<Agent, AppError> {
    let tools: Value = row.try_get("tools")?;
    let mcp_server_ids: Value = row.try_get("mcp_server_ids").unwrap_or_else(|_| json!([]));
    let skill_ids: Value = row.try_get("skill_ids").unwrap_or_else(|_| json!([]));
    let workflow_pack_ids: Value = row
        .try_get("workflow_pack_ids")
        .unwrap_or_else(|_| json!([]));
    Ok(Agent {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        kind: row.try_get("kind")?,
        team_id: row.try_get("team_id").unwrap_or(None),
        project_id: row.try_get("project_id").unwrap_or(None),
        runtime_profile_id: row.try_get("runtime_profile_id").unwrap_or(None),
        agent_role: row
            .try_get("agent_role")
            .unwrap_or_else(|_| "specialist".to_string()),
        provider: row.try_get("provider")?,
        model: row.try_get("model")?,
        system_prompt: row.try_get("system_prompt")?,
        tools: serde_json::from_value(tools).unwrap_or_default(),
        tool_policy: row.try_get("tool_policy").unwrap_or_else(|_| json!({})),
        mcp_server_ids: serde_json::from_value(mcp_server_ids).unwrap_or_default(),
        skill_ids: serde_json::from_value(skill_ids).unwrap_or_default(),
        workflow_pack_ids: serde_json::from_value(workflow_pack_ids).unwrap_or_default(),
        remote_computer_profile: row
            .try_get("remote_computer_profile")
            .unwrap_or_else(|_| json!({})),
        semantic_scopes: row.try_get("semantic_scopes").unwrap_or_else(|_| json!({})),
        release_state: row
            .try_get("release_state")
            .unwrap_or_else(|_| "draft".to_string()),
        created_at: row.try_get("created_at")?,
    })
}

pub(crate) fn session_from_row(row: PgRow) -> Result<Session, AppError> {
    let status: String = row.try_get("status")?;
    Ok(Session {
        id: row.try_get("id")?,
        agent_id: row.try_get("agent_id")?,
        agent_version_id: row.try_get("agent_version_id")?,
        title: row.try_get("title")?,
        status: status.into(),
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

pub(crate) fn agent_version_from_row(row: PgRow) -> Result<AgentVersion, AppError> {
    let tools: Value = row.try_get("tools")?;
    let tool_names: Value = row.try_get("tool_names")?;
    Ok(AgentVersion {
        id: row.try_get("id")?,
        agent_id: row.try_get("agent_id")?,
        version: row.try_get("version")?,
        model: row.try_get("model")?,
        system_prompt: row.try_get("system_prompt")?,
        tools: serde_json::from_value(tools).unwrap_or_default(),
        tool_names: serde_json::from_value(tool_names).unwrap_or_default(),
        runtime_config: row.try_get("runtime_config")?,
        approval_policy: row.try_get("approval_policy")?,
        created_at: row.try_get("created_at")?,
    })
}

pub(crate) fn agent_runtime_profile_from_row(row: PgRow) -> Result<AgentRuntimeProfile, AppError> {
    let default_args: Value = row.try_get("default_args")?;
    Ok(AgentRuntimeProfile {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        runtime_type: row.try_get("runtime_type")?,
        command: row.try_get("command")?,
        default_args: serde_json::from_value(default_args).unwrap_or_default(),
        env: row.try_get("env")?,
        timeout_seconds: row.try_get("timeout_seconds")?,
        remote_computer_required: row.try_get("remote_computer_required")?,
        status: row.try_get("status")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        archived_at: row.try_get("archived_at")?,
    })
}

pub(crate) fn agent_release_from_row(row: PgRow) -> Result<AgentRelease, AppError> {
    Ok(AgentRelease {
        id: row.try_get("id")?,
        agent_id: row.try_get("agent_id")?,
        agent_version_id: row.try_get("agent_version_id")?,
        environment: row.try_get("environment")?,
        status: row.try_get("status")?,
        eval_run_id: row.try_get("eval_run_id")?,
        eval_score: row.try_get("eval_score")?,
        min_score: row.try_get("min_score")?,
        requested_by: row.try_get("requested_by")?,
        requested_at: row.try_get("requested_at")?,
        request_reason: row.try_get("request_reason")?,
        approver_subject: row.try_get("approver_subject")?,
        decision_by: row.try_get("decision_by")?,
        decided_at: row.try_get("decided_at")?,
        decision_reason: row.try_get("decision_reason")?,
        promoted_by: row.try_get("promoted_by")?,
        promoted_at: row.try_get("promoted_at")?,
        automation_policy: row
            .try_get("automation_policy")
            .unwrap_or_else(|_| json!({})),
        created_at: row.try_get("created_at")?,
    })
}

pub(crate) fn policy_revision_from_row(row: PgRow) -> Result<PolicyRevision, AppError> {
    Ok(PolicyRevision {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        body: row.try_get("body")?,
        status: row.try_get("status")?,
        created_by: row.try_get("created_by")?,
        created_at: row.try_get("created_at")?,
        activated_at: row.try_get("activated_at")?,
        gate_status: row.try_get("gate_status")?,
        gate_result: row.try_get("gate_result")?,
        gated_at: row.try_get("gated_at")?,
    })
}

pub(crate) fn secret_record_from_row(row: PgRow) -> Result<SecretRecord, AppError> {
    Ok(SecretRecord {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        path: row.try_get("path")?,
        key: row.try_get("key")?,
        scope_type: row.try_get("scope_type")?,
        scope_id: row.try_get("scope_id")?,
        status: row.try_get("status")?,
        version: row.try_get("version")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

pub(crate) fn event_from_row(row: PgRow) -> Result<SessionEvent, AppError> {
    Ok(SessionEvent {
        id: row.try_get("id")?,
        session_id: row.try_get("session_id")?,
        seq: row.try_get("seq")?,
        parent_event_id: row.try_get("parent_event_id")?,
        actor_type: row
            .try_get::<Option<String>, _>("actor_type")?
            .unwrap_or_else(|| "system".to_string()),
        actor_id: row.try_get("actor_id")?,
        event_type: row.try_get("event_type")?,
        payload: row.try_get("payload")?,
        created_at: row.try_get("created_at")?,
    })
}

pub(crate) fn agent_handoff_event_from_row(row: PgRow) -> Result<AgentHandoffEvent, AppError> {
    Ok(AgentHandoffEvent {
        id: row.try_get("id")?,
        source_session_id: row.try_get("source_session_id")?,
        source_agent_id: row.try_get("source_agent_id")?,
        target_agent_id: row.try_get("target_agent_id")?,
        manager_plan_id: row.try_get("manager_plan_id")?,
        intent: row.try_get("intent")?,
        payload: row.try_get("payload")?,
        schema_version: row.try_get("schema_version")?,
        risk_level: row.try_get("risk_level")?,
        approval_required: row.try_get("approval_required")?,
        semantic_scopes: row.try_get("semantic_scopes")?,
        runtime_profile_id: row.try_get("runtime_profile_id")?,
        remote_computer_required: row.try_get("remote_computer_required")?,
        review_status: row.try_get("review_status")?,
        human_escalation_status: row.try_get("human_escalation_status")?,
        status: row.try_get("status")?,
        audit_trace_id: row.try_get("audit_trace_id")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

pub(crate) fn agent_handoff_assignment_from_row(
    row: PgRow,
) -> Result<AgentHandoffAssignment, AppError> {
    Ok(AgentHandoffAssignment {
        id: row.try_get("id")?,
        agent_handoff_event_id: row.try_get("agent_handoff_event_id")?,
        manager_plan_id: row.try_get("manager_plan_id")?,
        source_session_id: row.try_get("source_session_id")?,
        specialist_session_id: row.try_get("specialist_session_id")?,
        source_agent_id: row.try_get("source_agent_id")?,
        target_agent_id: row.try_get("target_agent_id")?,
        semantic_scopes: row.try_get("semantic_scopes")?,
        runtime_profile_id: row.try_get("runtime_profile_id")?,
        remote_computer_required: row.try_get("remote_computer_required")?,
        remote_computer_job_assignment_id: row.try_get("remote_computer_job_assignment_id")?,
        status: row.try_get("status")?,
        assigned_by: row.try_get("assigned_by")?,
        metadata: row.try_get("metadata")?,
        audit_trace_id: row.try_get("audit_trace_id")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

pub(crate) fn manager_agent_plan_from_row(row: PgRow) -> Result<ManagerAgentPlan, AppError> {
    Ok(ManagerAgentPlan {
        id: row.try_get("id")?,
        session_id: row.try_get("session_id")?,
        manager_agent_id: row.try_get("manager_agent_id")?,
        specialist_agent_id: row.try_get("specialist_agent_id")?,
        task_intake: row.try_get("task_intake")?,
        decomposition: row.try_get("decomposition")?,
        specialist_selection: row.try_get("specialist_selection")?,
        risk_classification: row.try_get("risk_classification")?,
        review: row.try_get("review")?,
        status: row.try_get("status")?,
        audit_trace_id: row.try_get("audit_trace_id")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

pub(crate) fn workflow_pack_installation_from_row(
    row: PgRow,
) -> Result<WorkflowPackInstallation, AppError> {
    Ok(WorkflowPackInstallation {
        id: row.try_get("id")?,
        pack_id: row.try_get("pack_id")?,
        kind: row.try_get("kind")?,
        version: row.try_get("version")?,
        manifest_path: row.try_get("manifest_path")?,
        manifest: row.try_get("manifest")?,
        validation_report: row.try_get("validation_report")?,
        status: row.try_get("status")?,
        eval_gate_status: row.try_get("eval_gate_status")?,
        release_gate_status: row.try_get("release_gate_status")?,
        gate_evidence: row.try_get("gate_evidence")?,
        staged_at: row.try_get("staged_at")?,
        released_at: row.try_get("released_at")?,
        archived_at: row.try_get("archived_at")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

pub(crate) fn workflow_pack_profile_asset_from_row(
    row: PgRow,
) -> Result<WorkflowPackProfileAsset, AppError> {
    Ok(WorkflowPackProfileAsset {
        id: row.try_get("id")?,
        installation_id: row.try_get("installation_id")?,
        profile_id: row.try_get("profile_id")?,
        content: row.try_get("content")?,
        version: row.try_get("version")?,
        status: row.try_get("status")?,
        created_at: row.try_get("created_at")?,
        archived_at: row.try_get("archived_at")?,
    })
}

pub(crate) fn artifact_from_row(row: PgRow) -> Result<Artifact, AppError> {
    Ok(Artifact {
        id: row.try_get("id")?,
        session_id: row.try_get("session_id")?,
        artifact_type: row.try_get("artifact_type")?,
        name: row.try_get("name")?,
        path: row.try_get("path")?,
        content: row
            .try_get::<Option<Value>, _>("content")?
            .unwrap_or(json!({})),
        created_at: row.try_get("created_at")?,
    })
}

pub(crate) fn approval_from_row(row: PgRow) -> Result<Approval, AppError> {
    Ok(Approval {
        id: row.try_get("id")?,
        session_id: row.try_get("session_id")?,
        tool_call_id: row.try_get("tool_call_id")?,
        action: row.try_get("action")?,
        risk_level: row.try_get("risk_level")?,
        reason: row.try_get("reason")?,
        evidence: row.try_get("evidence")?,
        decision_payload: row
            .try_get::<Option<Value>, _>("decision_payload")?
            .unwrap_or(json!({})),
        status: row.try_get("status")?,
        expires_at: row.try_get("expires_at")?,
        created_at: row.try_get("created_at")?,
        decided_at: row.try_get("decided_at")?,
    })
}

pub(crate) fn approval_group_from_row(row: PgRow) -> Result<ApprovalGroup, AppError> {
    let subjects: Value = row.try_get("subjects")?;
    Ok(ApprovalGroup {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        subjects: serde_json::from_value(subjects).unwrap_or_default(),
        status: row.try_get("status")?,
        created_at: row.try_get("created_at")?,
    })
}

pub(crate) fn approval_escalation_rule_from_row(
    row: PgRow,
) -> Result<ApprovalEscalationRule, AppError> {
    Ok(ApprovalEscalationRule {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        risk_level: row.try_get("risk_level")?,
        group_id: row.try_get("group_id")?,
        order_index: row.try_get("order_index")?,
        after_seconds: row.try_get("after_seconds")?,
        status: row.try_get("status")?,
        created_at: row.try_get("created_at")?,
    })
}

pub(crate) fn approval_notification_channel_policy_from_row(
    row: PgRow,
) -> Result<ApprovalNotificationChannelPolicy, AppError> {
    Ok(ApprovalNotificationChannelPolicy {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        channel: row.try_get("channel")?,
        target_env: row.try_get("target_env")?,
        risk_filter: row.try_get("risk_filter")?,
        max_attempts: row.try_get("max_attempts")?,
        backoff_seconds: row.try_get("backoff_seconds")?,
        status: row.try_get("status")?,
        created_at: row.try_get("created_at")?,
    })
}

pub(crate) fn tool_call_from_row(row: PgRow) -> Result<ToolCall, AppError> {
    Ok(ToolCall {
        id: row.try_get("id")?,
        session_id: row.try_get("session_id")?,
        event_id: row.try_get("event_id")?,
        tool_name: row.try_get("tool_name")?,
        args: row.try_get("args")?,
        status: row.try_get("status")?,
        risk_level: row.try_get("risk_level")?,
        policy_decision: row.try_get("policy_decision")?,
        result: row.try_get("result")?,
        error: row.try_get("error")?,
        started_at: row.try_get("started_at")?,
        completed_at: row.try_get("completed_at")?,
        created_at: row.try_get("created_at")?,
    })
}

pub(crate) fn audit_log_from_row(row: PgRow) -> Result<AuditLog, AppError> {
    Ok(AuditLog {
        id: row.try_get("id")?,
        session_id: row.try_get("session_id")?,
        actor_type: row.try_get("actor_type")?,
        actor_id: row.try_get("actor_id")?,
        action: row.try_get("action")?,
        resource_type: row.try_get("resource_type")?,
        resource_id: row.try_get("resource_id")?,
        details: row.try_get("details")?,
        created_at: row.try_get("created_at")?,
    })
}

pub(crate) fn organization_from_row(row: PgRow) -> Result<Organization, AppError> {
    Ok(Organization {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        slug: row.try_get("slug")?,
        owner_subject: row.try_get("owner_subject")?,
        created_at: row.try_get("created_at")?,
        archived_at: row.try_get("archived_at")?,
    })
}

pub(crate) fn team_from_row(row: PgRow) -> Result<Team, AppError> {
    Ok(Team {
        id: row.try_get("id")?,
        organization_id: row.try_get("organization_id")?,
        name: row.try_get("name")?,
        slug: row.try_get("slug")?,
        created_at: row.try_get("created_at")?,
        archived_at: row.try_get("archived_at")?,
    })
}

pub(crate) fn project_from_row(row: PgRow) -> Result<Project, AppError> {
    Ok(Project {
        id: row.try_get("id")?,
        team_id: row.try_get("team_id")?,
        name: row.try_get("name")?,
        slug: row.try_get("slug")?,
        created_at: row.try_get("created_at")?,
        archived_at: row.try_get("archived_at")?,
    })
}

pub(crate) fn membership_from_row(row: PgRow) -> Result<Membership, AppError> {
    Ok(Membership {
        id: row.try_get("id")?,
        user_id: row.try_get("user_id")?,
        organization_id: row.try_get("organization_id")?,
        team_id: row.try_get("team_id")?,
        project_id: row.try_get("project_id").unwrap_or(None),
        role: row.try_get("role")?,
        created_at: row.try_get("created_at")?,
    })
}

pub(crate) fn tenant_invitation_from_row(row: PgRow) -> Result<TenantInvitation, AppError> {
    Ok(TenantInvitation {
        id: row.try_get("id")?,
        organization_id: row.try_get("organization_id")?,
        team_id: row.try_get("team_id")?,
        project_id: row.try_get("project_id")?,
        email: row.try_get("email")?,
        role: row.try_get("role")?,
        status: row.try_get("status")?,
        token: row.try_get("token")?,
        invited_by: row.try_get("invited_by")?,
        accepted_by: row.try_get("accepted_by")?,
        expires_at: row.try_get("expires_at")?,
        created_at: row.try_get("created_at")?,
        decided_at: row.try_get("decided_at")?,
    })
}

pub(crate) fn provider_access_from_row(row: PgRow) -> Result<ProviderAccess, AppError> {
    let model_allowlist: Value = row.try_get("model_allowlist")?;
    Ok(ProviderAccess {
        id: row.try_get("id")?,
        team_id: row.try_get("team_id")?,
        provider_name: row.try_get("provider_name")?,
        model_allowlist: serde_json::from_value(model_allowlist).unwrap_or_default(),
        status: row.try_get("status")?,
        created_at: row.try_get("created_at")?,
    })
}

pub(crate) fn provider_record_from_row(row: PgRow) -> Result<ProviderRecord, AppError> {
    Ok(ProviderRecord {
        id: row.try_get("id")?,
        provider_type: row.try_get("provider_type")?,
        name: row.try_get("name")?,
        base_url: row.try_get("base_url")?,
        default_model: row.try_get("default_model")?,
        config: row.try_get("config")?,
        status: row.try_get("status")?,
        created_at: row.try_get("created_at")?,
    })
}

pub(crate) fn mcp_server_from_row(row: PgRow) -> Result<McpServerRecord, AppError> {
    let tool_allowlist: Value = row.try_get("tool_allowlist")?;
    Ok(McpServerRecord {
        id: row.try_get("id")?,
        team_id: row.try_get("team_id")?,
        name: row.try_get("name")?,
        transport: row.try_get("transport")?,
        config: row.try_get("config")?,
        tool_allowlist: serde_json::from_value(tool_allowlist).unwrap_or_default(),
        status: row.try_get("status")?,
        created_at: row.try_get("created_at")?,
    })
}

pub(crate) fn eval_dataset_from_row(row: PgRow) -> Result<EvalDataset, AppError> {
    Ok(EvalDataset {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        description: row.try_get("description")?,
        created_at: row.try_get("created_at")?,
    })
}

pub(crate) fn eval_case_from_row(row: PgRow) -> Result<EvalCase, AppError> {
    Ok(EvalCase {
        id: row.try_get("id")?,
        dataset_id: row.try_get("dataset_id")?,
        input: row.try_get("input")?,
        expected: row.try_get("expected")?,
        grading_policy: row.try_get("grading_policy")?,
        created_at: row.try_get("created_at")?,
    })
}

pub(crate) fn eval_run_from_row(row: PgRow) -> Result<EvalRun, AppError> {
    Ok(EvalRun {
        id: row.try_get("id")?,
        dataset_id: row.try_get("dataset_id")?,
        agent_id: row.try_get("agent_id")?,
        agent_version_id: row.try_get("agent_version_id")?,
        status: row.try_get("status")?,
        score: row.try_get("score")?,
        details: row.try_get("details")?,
        created_at: row.try_get("created_at")?,
    })
}

pub(crate) fn usage_rollup_from_row(row: PgRow) -> Result<UsageRollup, AppError> {
    Ok(UsageRollup {
        id: row.try_get("id")?,
        period_start: row.try_get("period_start")?,
        period_end: row.try_get("period_end")?,
        summary: row.try_get("summary")?,
        created_at: row.try_get("created_at")?,
    })
}

pub(crate) fn cost_alert_route_from_row(row: PgRow) -> Result<CostAlertRoute, AppError> {
    Ok(CostAlertRoute {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        channel: row.try_get("channel")?,
        target: row.try_get("target")?,
        severity_filter: row.try_get("severity_filter")?,
        status: row.try_get("status")?,
        created_at: row.try_get("created_at")?,
    })
}
