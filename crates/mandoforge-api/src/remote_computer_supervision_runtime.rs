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

pub(crate) async fn execute_remote_computer_stale_reclaim(
    state: &AppState,
) -> Result<RemoteComputerReclaimRun, AppError> {
    let stale_attachments = state.list_stale_remote_computer_attachments().await?;
    let expired_leases: Vec<_> = state
        .list_remote_computer_leases()
        .await?
        .into_iter()
        .filter(|lease| {
            lease.status == "leased"
                && lease
                    .lease_expires_at
                    .is_some_and(|lease_expires_at| lease_expires_at <= Utc::now())
        })
        .collect();

    let mut reclaimed_attachments = Vec::new();
    for attachment in &stale_attachments {
        let reclaimed = state
            .release_remote_computer_attachment(
                attachment.id,
                UpdateRemoteComputerAttachment {
                    reason: Some("stale attachment reclaimed".to_string()),
                    metadata: Some(json!({
                        "reclaim_reason": "stale_attachment",
                        "execution_enabled": false
                    })),
                },
            )
            .await?;
        record_remote_computer_attachment_event(
            state,
            &reclaimed,
            "remote_computer.attachment_reclaimed",
        )
        .await?;
        reclaimed_attachments.push(reclaimed);
    }

    let mut reclaimed_leases = Vec::new();
    for lease in &expired_leases {
        let reclaimed = state
            .update_remote_computer_lease_status(
                lease.id,
                "failed",
                UpdateRemoteComputerLease {
                    reason: Some("expired lease reclaimed".to_string()),
                    metadata: Some(json!({
                        "reclaim_reason": "expired_lease",
                        "execution_enabled": false
                    })),
                },
            )
            .await?;
        // If this was an on-demand Pod, delete it from Kubernetes now that the lease expired.
        let is_on_demand = lease
            .metadata
            .get("on_demand")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if is_on_demand
            && let Some(pod_name) = state
                .list_remote_computers()
                .await
                .unwrap_or_default()
                .into_iter()
                .find(|c| c.id == lease.remote_computer_id)
                .and_then(|c| c.pod_name)
        {
            let config = RemoteComputerRunnerConfig::from_env();
            let runner = remote_computer_runner_for_config(&config);
            runner
                .mutate(
                    &config,
                    RemoteComputerRunnerDryRunRequest {
                        operation: Some("live_delete".to_string()),
                        remote_computer_id: Some(lease.remote_computer_id),
                        session_id: lease.session_id,
                        pod_name: Some(pod_name),
                        metadata: Some(json!({
                            "reclaim_reason": "expired_on_demand_lease",
                            "lease_id": lease.id
                        })),
                    },
                )
                .await;
            // Deletion errors are fire-and-forget: the Pod will be garbage-collected
            // by Kubernetes TTL or a future sweep. We do not propagate the error here
            // so that the lease reclaim itself always succeeds.
        }
        record_remote_computer_lease_event(state, &reclaimed, "remote_computer.lease_reclaimed")
            .await?;
        reclaimed_leases.push(reclaimed);
    }

    let run = RemoteComputerReclaimRun {
        generated_at: Utc::now(),
        status: if reclaimed_attachments.is_empty() && reclaimed_leases.is_empty() {
            "noop"
        } else {
            "completed"
        }
        .to_string(),
        stale_attachment_count: stale_attachments.len(),
        reclaimed_attachment_count: reclaimed_attachments.len(),
        expired_lease_count: expired_leases.len(),
        reclaimed_lease_count: reclaimed_leases.len(),
        attachments: reclaimed_attachments,
        leases: reclaimed_leases,
        execution_enabled: false,
    };
    state
        .append_audit_log(new_audit_log(
            None,
            "system",
            None,
            "remote_computer.reclaim_stale_run",
            "remote_computers",
            None,
            json!(&run),
        ))
        .await?;
    Ok(run)
}

pub(crate) async fn ensure_remote_computer_lock_refs_match_session(
    state: &AppState,
    input: &CreateRemoteComputerStateLock,
    session_id: Uuid,
) -> Result<(), AppError> {
    if let Some(lease_id) = input.lease_id {
        let lease = state
            .list_remote_computer_leases()
            .await?
            .into_iter()
            .find(|lease| lease.id == lease_id)
            .ok_or_else(|| AppError::not_found("Remote Computer lease not found"))?;
        if lease.session_id != Some(session_id) {
            return Err(AppError::forbidden(
                "Remote Computer state lock lease does not belong to the session",
            ));
        }
        if let Some(remote_computer_id) = input.remote_computer_id
            && lease.remote_computer_id != remote_computer_id
        {
            return Err(AppError::forbidden(
                "Remote Computer state lock lease does not belong to the remote computer",
            ));
        }
    }
    Ok(())
}

pub(crate) async fn authorize_remote_computer_state_lock_release(
    state: &AppState,
    headers: &HeaderMap,
    lock: &RemoteComputerStateLock,
) -> Result<(), AppError> {
    let principal = principal_from_request(state, headers).await?;
    let session_id = lock.session_id.ok_or_else(|| {
        AppError::forbidden("Remote Computer state lock is not bound to a session")
    })?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::SessionsRun,
        resource_type: "session".to_string(),
        resource_id: Some(session_id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(state, &principal, &request).await?;
    if !principal.roles.contains(&Role::Admin)
        && let Some(owner) = lock.owner.as_deref()
    {
        let worker_id = header_value(headers, "x-mandoforge-worker-id")
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if worker_id != Some(owner) {
            return Err(AppError::forbidden(
                "Remote Computer state lock owner does not match worker identity",
            ));
        }
    }
    Ok(())
}

pub(crate) async fn ensure_remote_computer_heartbeat_refs_match_session(
    state: &AppState,
    input: &CreateRemoteComputerSidecarHeartbeat,
    session_id: Uuid,
) -> Result<(), AppError> {
    if let Some(assignment_id) = input.assignment_id {
        let assignment = state
            .list_remote_computer_job_assignments()
            .await?
            .into_iter()
            .find(|assignment| assignment.id == assignment_id)
            .ok_or_else(|| AppError::not_found("Remote Computer job assignment not found"))?;
        if assignment.session_id != session_id {
            return Err(AppError::forbidden(
                "Remote Computer sidecar heartbeat assignment does not belong to the session",
            ));
        }
        if assignment.remote_computer_id != input.remote_computer_id {
            return Err(AppError::forbidden(
                "Remote Computer sidecar heartbeat assignment does not belong to the remote computer",
            ));
        }
    }
    Ok(())
}
