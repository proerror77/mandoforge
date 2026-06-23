use crate::*;

pub(crate) async fn execute_remote_computer_sidecar_supervision(
    state: &AppState,
) -> Result<RemoteComputerSidecarSupervisionRun, AppError> {
    let checked_at = Utc::now();
    let readiness = build_remote_computer_readiness(state).await?;
    let supervision = readiness.sidecar_supervision;
    let mut actions = Vec::new();
    if supervision.missing_heartbeat_count > 0 {
        actions.push("flag_missing_artifact_discovery_sidecar_heartbeats".to_string());
    }
    if supervision.stale_heartbeat_count > 0 {
        actions.push("flag_stale_artifact_discovery_sidecar_heartbeats".to_string());
    }
    if !actions.is_empty() {
        actions
            .push("keep_remote_computer_pods_out_of_production_until_sidecars_recover".to_string());
    }
    let status = if actions.is_empty() {
        "ok"
    } else {
        "attention"
    }
    .to_string();
    let run = RemoteComputerSidecarSupervisionRun {
        status,
        checked_at,
        active_remote_computer_count: supervision.active_remote_computer_count,
        heartbeat_count: supervision.heartbeat_count,
        missing_heartbeat_count: supervision.missing_heartbeat_count,
        stale_heartbeat_count: supervision.stale_heartbeat_count,
        stale_after_seconds: supervision.stale_after_seconds,
        actions,
    };
    state
        .append_audit_log(new_audit_log(
            None,
            "system",
            None,
            "remote_computer.sidecar_supervision_run",
            "remote_computer_sidecar",
            None,
            json!({
                "status": run.status,
                "actions": run.actions,
                "active_remote_computer_count": run.active_remote_computer_count,
                "heartbeat_count": run.heartbeat_count,
                "missing_heartbeat_count": run.missing_heartbeat_count,
                "stale_heartbeat_count": run.stale_heartbeat_count,
                "stale_after_seconds": run.stale_after_seconds,
                "checked_at": run.checked_at,
            }),
        ))
        .await?;
    Ok(run)
}
