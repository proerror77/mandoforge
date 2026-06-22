use serde_json::json;

use crate::{
    AppError, AppState, RemoteComputerAttachment, RemoteComputerJobAssignment,
    RemoteComputerLease, RemoteComputerSidecarHeartbeat, RemoteComputerStateLock, execution_queue,
    new_audit_log,
};

pub(crate) async fn record_remote_computer_lease_event(
    state: &AppState,
    lease: &RemoteComputerLease,
    event_type: &str,
) -> Result<(), AppError> {
    let details = json!({
        "lease_id": lease.id,
        "remote_computer_id": lease.remote_computer_id,
        "session_id": lease.session_id,
        "status": lease.status,
        "worker_id": lease.worker_id,
        "lease_expires_at": lease.lease_expires_at,
        "heartbeat_at": lease.heartbeat_at,
        "metadata": lease.metadata,
        "execution_enabled": false
    });
    if let Some(session_id) = lease.session_id {
        state
            .append_event("system", None, session_id, event_type, details.clone())
            .await?;
    }
    state
        .append_audit_log(new_audit_log(
            lease.session_id,
            "system",
            None,
            event_type,
            "remote_computer_lease",
            Some(lease.id),
            details,
        ))
        .await?;
    Ok(())
}

pub(crate) async fn record_remote_computer_attachment_event(
    state: &AppState,
    attachment: &RemoteComputerAttachment,
    event_type: &str,
) -> Result<(), AppError> {
    let details = json!({
        "attachment_id": attachment.id,
        "remote_computer_id": attachment.remote_computer_id,
        "lease_id": attachment.lease_id,
        "session_id": attachment.session_id,
        "status": attachment.status,
        "attached_by": attachment.attached_by,
        "stale_after": attachment.stale_after,
        "released_at": attachment.released_at,
        "metadata": attachment.metadata,
        "execution_enabled": false
    });
    state
        .append_event(
            "system",
            None,
            attachment.session_id,
            event_type,
            details.clone(),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(attachment.session_id),
            "system",
            None,
            event_type,
            "remote_computer_attachment",
            Some(attachment.id),
            details,
        ))
        .await?;
    Ok(())
}

pub(crate) async fn record_remote_computer_job_assignment_event(
    state: &AppState,
    assignment: &RemoteComputerJobAssignment,
    job: &execution_queue::ExecutionJob,
    event_type: &str,
) -> Result<(), AppError> {
    let details = json!({
        "assignment_id": assignment.id,
        "execution_job_id": assignment.execution_job_id,
        "approval_id": job.approval_id,
        "tool_call_id": job.tool_call_id,
        "tool_name": job.tool_name,
        "remote_computer_id": assignment.remote_computer_id,
        "lease_id": assignment.lease_id,
        "session_id": assignment.session_id,
        "status": assignment.status,
        "assigned_by": assignment.assigned_by,
        "metadata": assignment.metadata,
        "execution_enabled": false
    });
    state
        .append_event(
            "system",
            None,
            assignment.session_id,
            event_type,
            details.clone(),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(assignment.session_id),
            "system",
            None,
            event_type,
            "remote_computer_job_assignment",
            Some(assignment.id),
            details,
        ))
        .await?;
    Ok(())
}

pub(crate) async fn record_remote_computer_state_lock_event(
    state: &AppState,
    lock: &RemoteComputerStateLock,
    event_type: &str,
) -> Result<(), AppError> {
    let details = json!({
        "lock_id": lock.id,
        "lock_key": lock.lock_key,
        "status": lock.status,
        "remote_computer_id": lock.remote_computer_id,
        "lease_id": lock.lease_id,
        "session_id": lock.session_id,
        "owner": lock.owner,
        "expires_at": lock.expires_at,
        "released_at": lock.released_at,
        "metadata": lock.metadata,
    });
    if let Some(session_id) = lock.session_id {
        state
            .append_event("system", None, session_id, event_type, details.clone())
            .await?;
    }
    state
        .append_audit_log(new_audit_log(
            lock.session_id,
            "system",
            None,
            event_type,
            "remote_computer_state_lock",
            Some(lock.id),
            details,
        ))
        .await?;
    Ok(())
}

pub(crate) async fn record_remote_computer_sidecar_heartbeat_event(
    state: &AppState,
    heartbeat: &RemoteComputerSidecarHeartbeat,
) -> Result<(), AppError> {
    let details = json!({
        "heartbeat_id": heartbeat.id,
        "remote_computer_id": heartbeat.remote_computer_id,
        "session_id": heartbeat.session_id,
        "assignment_id": heartbeat.assignment_id,
        "sidecar_name": heartbeat.sidecar_name,
        "status": heartbeat.status,
        "observed_at": heartbeat.observed_at,
        "metadata": heartbeat.metadata,
    });
    if let Some(session_id) = heartbeat.session_id {
        state
            .append_event(
                "worker",
                Some(heartbeat.id),
                session_id,
                "remote_computer.sidecar_heartbeat",
                details.clone(),
            )
            .await?;
    }
    state
        .append_audit_log(new_audit_log(
            heartbeat.session_id,
            "worker",
            Some(heartbeat.id),
            "remote_computer.sidecar_heartbeat",
            "remote_computer_sidecar_heartbeat",
            Some(heartbeat.id),
            details,
        ))
        .await?;
    Ok(())
}
