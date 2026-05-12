use anyhow::Result;
use serde_json::{Value, json};
use sqlx::{Row, postgres::PgRow};

use crate::{
    Agent, AgentVersion, AppError, Approval, Artifact, AuditLog, EvalCase, EvalDataset, EvalRun,
    McpServerRecord, Membership, Organization, Project, ProviderAccess, ProviderRecord, Session,
    SessionEvent, Team, ToolCall,
};

pub(crate) fn agent_from_row(row: PgRow) -> Result<Agent, AppError> {
    let tools: Value = row.try_get("tools")?;
    Ok(Agent {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        kind: row.try_get("kind")?,
        team_id: row.try_get("team_id").unwrap_or(None),
        project_id: row.try_get("project_id").unwrap_or(None),
        provider: row.try_get("provider")?,
        model: row.try_get("model")?,
        system_prompt: row.try_get("system_prompt")?,
        tools: serde_json::from_value(tools).unwrap_or_default(),
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
        created_at: row.try_get("created_at")?,
        decided_at: row.try_get("decided_at")?,
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
        created_at: row.try_get("created_at")?,
    })
}

pub(crate) fn team_from_row(row: PgRow) -> Result<Team, AppError> {
    Ok(Team {
        id: row.try_get("id")?,
        organization_id: row.try_get("organization_id")?,
        name: row.try_get("name")?,
        slug: row.try_get("slug")?,
        created_at: row.try_get("created_at")?,
    })
}

pub(crate) fn project_from_row(row: PgRow) -> Result<Project, AppError> {
    Ok(Project {
        id: row.try_get("id")?,
        team_id: row.try_get("team_id")?,
        name: row.try_get("name")?,
        slug: row.try_get("slug")?,
        created_at: row.try_get("created_at")?,
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
