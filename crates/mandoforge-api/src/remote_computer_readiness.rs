use chrono::Utc;

use crate::{
    AppError, AppState, RemoteComputerArtifactDiscoverySidecarConfigReadiness,
    RemoteComputerAttentionItem, RemoteComputerAutoscalingReadiness,
    RemoteComputerManifestReadiness, RemoteComputerReadinessReport,
    RemoteComputerStateFilesystemReadiness, RemoteComputerWarmPoolReadiness,
    build_remote_computer_execution_transport_readiness,
    build_remote_computer_production_state_sync_readiness, build_remote_computer_runner_readiness,
    build_remote_computer_sidecar_recovery_readiness, build_remote_computer_sidecar_supervision,
    env_bool, project_file_contains, project_file_path, remote_computer_sidecar_recovery_targets,
    remote_computer_state_sync_controller_configured,
    remote_computer_state_sync_controller_required,
};

pub(crate) async fn build_remote_computer_readiness(
    state: &AppState,
) -> Result<RemoteComputerReadinessReport, AppError> {
    let pod_template =
        remote_computer_manifest_readiness("deploy/k8s/agent-remote-computer.yaml", "pod_template");
    let service_account = remote_computer_manifest_readiness(
        "deploy/k8s/remote-computer-serviceaccount.yaml",
        "service_account",
    );
    let network_policy = remote_computer_manifest_readiness(
        "deploy/k8s/remote-computer-networkpolicy.yaml",
        "network_policy",
    );
    let pvc_path = "deploy/k8s/remote-computer-state-pvc.yaml";
    let state_contract_path = "deploy/k8s/remote-computer-state-contract.yaml";
    let provider_manifest_path = "deploy/k8s/remote-computer-state-juicefs-example.yaml";
    let production_profile_path = "deploy/k8s/remote-computer-state-juicefs-profile.yaml";
    let pvc_present = project_file_path(pvc_path).is_some();
    let state_contract_present = project_file_path(state_contract_path).is_some();
    let state_provider = std::env::var("MANDOFORGE_REMOTE_COMPUTER_STATE_PROVIDER")
        .ok()
        .map(|provider| provider.trim().to_string())
        .filter(|provider| !provider.is_empty())
        .unwrap_or_else(|| "pvc-placeholder".to_string());
    let provider_configured_by_env = state_provider != "pvc-placeholder";
    let provider_manifest_present = project_file_path(provider_manifest_path).is_some();
    let production_profile_present = project_file_path(production_profile_path).is_some();
    let distributed_filesystem_configured = provider_configured_by_env;
    let conflict_policy = std::env::var("MANDOFORGE_REMOTE_COMPUTER_STATE_CONFLICT_POLICY")
        .ok()
        .map(|policy| policy.trim().to_string())
        .filter(|policy| !policy.is_empty())
        .unwrap_or_else(|| "one-active-writer-per-session".to_string());
    let lock_manager_configured = env_bool("MANDOFORGE_REMOTE_COMPUTER_STATE_LOCK_MANAGER");
    let sync_contract_status =
        if distributed_filesystem_configured && state_contract_present && lock_manager_configured {
            "provider_and_lock_manager_configured"
        } else if distributed_filesystem_configured && state_contract_present {
            "provider_configured_without_lock_manager"
        } else if state_contract_present {
            "contract_documented"
        } else {
            "missing"
        }
        .to_string();
    let state_filesystem = RemoteComputerStateFilesystemReadiness {
        pvc_present,
        pvc_path: pvc_path.to_string(),
        access_mode: "ReadWriteMany".to_string(),
        mount_path: "/agent-state".to_string(),
        state_contract_present,
        state_contract_path: state_contract_path.to_string(),
        state_layout_paths: vec![
            "/agent-state/memory".to_string(),
            "/agent-state/notes".to_string(),
            "/agent-state/skills".to_string(),
            "/agent-state/artifacts".to_string(),
            "/agent-state/.locks".to_string(),
            "/agent-state/.mandoforge".to_string(),
        ],
        conflict_policy,
        lock_manager_configured,
        sync_contract_status,
        distributed_filesystem_configured,
        provider: state_provider,
        provider_configured_by_env,
        provider_manifest_present,
        provider_manifest_path: provider_manifest_path.to_string(),
        production_profile_present,
        production_profile_path: production_profile_path.to_string(),
        production_claim_name: "mandoforge-remote-computer-state".to_string(),
        supported_providers: vec![
            "juicefs".to_string(),
            "cephfs".to_string(),
            "longhorn-rwx".to_string(),
            "cloud-file-storage".to_string(),
            "object-sync".to_string(),
        ],
        status: if pvc_present && distributed_filesystem_configured && production_profile_present {
            "configured"
        } else if pvc_present && distributed_filesystem_configured {
            "provider_configured_profile_missing"
        } else if pvc_present && production_profile_present {
            "production_profile_present"
        } else if pvc_present && provider_manifest_present {
            "example_present"
        } else if pvc_present {
            "skeleton"
        } else {
            "missing"
        }
        .to_string(),
    };
    let audit_logs = state.list_audit_logs(None).await.unwrap_or_default();
    let generated_at = Utc::now();
    let production_state_sync = build_remote_computer_production_state_sync_readiness(
        &state_filesystem,
        &audit_logs,
        generated_at,
        remote_computer_state_sync_controller_required(&|key| std::env::var(key).ok()),
        remote_computer_state_sync_controller_configured(&|key| std::env::var(key).ok()),
    );
    let remote_pool_scaled_object_path = "deploy/k8s/remote-computer-keda.yaml";
    let remote_pool_scaled_object_present =
        project_file_path(remote_pool_scaled_object_path).is_some();
    let worker_keda_present = project_file_path("deploy/k8s/worker-keda.yaml").is_some();
    let autoscaling = RemoteComputerAutoscalingReadiness {
        worker_hpa_present: project_file_path("deploy/k8s/worker-hpa.yaml").is_some(),
        keda_manifest_present: project_file_path("deploy/k8s/keda.yaml").is_some()
            || worker_keda_present
            || remote_pool_scaled_object_present,
        remote_pool_scaled_object_present,
        remote_pool_scaled_object_path: remote_pool_scaled_object_path.to_string(),
        queue_depth_scaling_present: remote_pool_scaled_object_present,
        status: if remote_pool_scaled_object_present {
            "remote_pool_scaler_example_present"
        } else if worker_keda_present {
            "worker_queue_scaling_present"
        } else if project_file_path("deploy/k8s/worker-hpa.yaml").is_some() {
            "hpa_skeleton"
        } else {
            "missing"
        }
        .to_string(),
    };
    let warm_pool_path = "deploy/k8s/remote-computer-warm-pool.yaml";
    let warm_pool_manifest_present = project_file_path(warm_pool_path).is_some();
    let warm_pool_configured = std::env::var("MANDOFORGE_REMOTE_COMPUTER_WARM_POOL")
        .ok()
        .is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true"));
    let warm_pool = RemoteComputerWarmPoolReadiness {
        configured: warm_pool_configured,
        manifest_present: warm_pool_manifest_present,
        manifest_path: warm_pool_path.to_string(),
        status: if warm_pool_manifest_present && warm_pool_configured {
            "configured"
        } else if warm_pool_manifest_present {
            "skeleton"
        } else {
            "missing"
        }
        .to_string(),
    };
    let artifact_discovery_sidecar = remote_computer_manifest_readiness(
        "deploy/k8s/remote-computer-artifact-discovery-sidecar.yaml",
        "artifact_discovery_sidecar",
    );
    let artifact_discovery_sidecar_config =
        remote_computer_artifact_discovery_sidecar_config_readiness();
    let sidecar_supervision =
        build_remote_computer_sidecar_supervision(state, artifact_discovery_sidecar.present)
            .await?;
    let runner = build_remote_computer_runner_readiness();
    let sidecar_recovery_targets = remote_computer_sidecar_recovery_targets(state).await?;
    let sidecar_recovery =
        build_remote_computer_sidecar_recovery_readiness(&sidecar_recovery_targets, &runner);
    let execution_transport = build_remote_computer_execution_transport_readiness(state).await?;

    let event_types = vec![
        "remote_computer.requested".to_string(),
        "remote_computer.leased".to_string(),
        "remote_computer.started".to_string(),
        "remote_computer.heartbeat".to_string(),
        "remote_computer.attached".to_string(),
        "remote_computer.warm_pool_claimed".to_string(),
        "remote_computer.execution_handoff_planned".to_string(),
        "remote_computer.execution_handoff_assigned".to_string(),
        "remote_computer.execution_handoff_acknowledged".to_string(),
        "remote_computer.execution_handoff_completed".to_string(),
        "remote_computer.execution_handoff_released".to_string(),
        "remote_computer.execution_handoff_failed".to_string(),
        "remote_computer.execution_handoff_canceled".to_string(),
        "remote_computer.state_lock_acquired".to_string(),
        "remote_computer.state_lock_released".to_string(),
        "remote_computer.sidecar_heartbeat".to_string(),
        "remote_computer.execution_transport_planned".to_string(),
        "remote_computer.execution_transport_completed".to_string(),
        "remote_computer.runner_dry_run".to_string(),
        "remote_computer.artifact_discovered".to_string(),
        "remote_computer.detached".to_string(),
        "remote_computer.attachment_reclaimed".to_string(),
        "remote_computer.lease_reclaimed".to_string(),
        "remote_computer.released".to_string(),
        "remote_computer.failed".to_string(),
    ];
    let mut attention_items = Vec::new();
    if !pod_template.present {
        attention_items.push(remote_computer_attention(
            "pod_template_missing",
            "critical",
            "deploy/k8s/agent-remote-computer.yaml is required before Pod-based session leases can be piloted",
        ));
    }
    if !service_account.present {
        attention_items.push(remote_computer_attention(
            "service_account_missing",
            "critical",
            "remote computer Pods need a restricted service account before execution moves into Pods",
        ));
    }
    if state_filesystem.status == "missing" {
        attention_items.push(remote_computer_attention(
            "state_pvc_missing",
            "critical",
            "remote computer state PVC placeholder is missing",
        ));
    } else if !state_filesystem.state_contract_present {
        attention_items.push(remote_computer_attention(
            "state_contract_missing",
            "critical",
            "remote computer Memory/Notes/Skills state contract ConfigMap is missing",
        ));
    } else if state_filesystem.provider_configured_by_env
        && !state_filesystem.production_profile_present
    {
        attention_items.push(remote_computer_attention(
            "distributed_state_profile_missing",
            "critical",
            "remote computer state provider is configured, but the production state filesystem profile manifest is missing",
        ));
    } else if !state_filesystem.distributed_filesystem_configured {
        attention_items.push(remote_computer_attention(
            "distributed_state_filesystem_missing",
            "critical",
            "state mount has only a PVC/RWX placeholder and optional production profile; set a real JuiceFS/CephFS/Longhorn or equivalent provider before multi-Pod state sync",
        ));
    }
    if !state_filesystem.lock_manager_configured {
        attention_items.push(remote_computer_attention(
            "state_lock_manager_missing",
            "critical",
            "shared Memory/Notes/Skills writes require a lock-aware state sync manager before Remote Computer can claim distributed production state",
        ));
    }
    if production_state_sync.production_blocked {
        attention_items.push(remote_computer_attention(
            "production_state_sync_blocked",
            "critical",
            production_state_sync.message.as_str(),
        ));
    }
    if !network_policy.present {
        attention_items.push(remote_computer_attention(
            "network_policy_missing",
            "warning",
            "remote computer Pods need a NetworkPolicy before shared-cluster sandbox execution",
        ));
    }
    if !autoscaling.queue_depth_scaling_present {
        attention_items.push(remote_computer_attention(
            "queue_depth_scaling_missing",
            "warning",
            "worker HPA is present as a skeleton, but KEDA/queue-depth scaling for remote computer pools is not configured",
        ));
    }
    if !warm_pool.manifest_present {
        attention_items.push(remote_computer_attention(
            "warm_pool_missing",
            "warning",
            "no remote computer warm-pool manifest is present; first Pod lease will still cold start",
        ));
    }
    if !artifact_discovery_sidecar.present {
        attention_items.push(remote_computer_attention(
            "artifact_discovery_sidecar_missing",
            "warning",
            "remote computer Pods need an artifact discovery sidecar before continuous artifact sync can be piloted",
        ));
    } else if artifact_discovery_sidecar_config.status != "configured" {
        attention_items.push(remote_computer_attention(
            "artifact_discovery_sidecar_api_url_mismatch",
            "warning",
            "remote computer artifact discovery sidecar must target the in-cluster mandoforge-api service on port 8787",
        ));
    } else if sidecar_supervision.missing_heartbeat_count > 0 {
        attention_items.push(remote_computer_attention(
            "artifact_discovery_sidecar_heartbeat_missing",
            "warning",
            "one or more active remote computers have no artifact-discovery sidecar heartbeat",
        ));
    }
    if sidecar_supervision.stale_heartbeat_count > 0 {
        attention_items.push(remote_computer_attention(
            "artifact_discovery_sidecar_heartbeat_stale",
            "warning",
            "one or more artifact-discovery sidecar heartbeats are older than the configured stale threshold",
        ));
    }
    if sidecar_recovery.status == "blocked" {
        attention_items.push(remote_computer_attention(
            "sidecar_replacement_blocked",
            "warning",
            "Remote Computer sidecar recovery found unhealthy sidecars, but replacement automation is blocked",
        ));
    }
    if !runner.configured {
        attention_items.push(remote_computer_attention(
            "runner_reserved",
            "warning",
            "Remote Computer runner is fail-closed; Kubernetes Pod create/delete is dry-run only",
        ));
    }
    if !execution_transport.execution_enabled {
        attention_items.push(remote_computer_attention(
            "pod_execution_transport_disabled",
            "warning",
            "Remote Computer job handoff can be planned and audited, but approved tools still execute on the existing worker path",
        ));
    }

    let mut runbook_actions = Vec::new();
    if !state_filesystem.distributed_filesystem_configured {
        runbook_actions.push(
            "set MANDOFORGE_REMOTE_COMPUTER_STATE_PROVIDER to juicefs, cephfs, longhorn-rwx, or an equivalent provider before running multi-Pod Memory/Notes/Skills sync"
                .to_string(),
        );
    }
    if state_filesystem.production_profile_present {
        runbook_actions.push(
            "apply deploy/k8s/remote-computer-state-juicefs-profile.yaml only after replacing placeholder JuiceFS metadata, object storage, and access credentials"
                .to_string(),
        );
    }
    if !state_filesystem.lock_manager_configured {
        runbook_actions.push(
            "keep shared Memory/Notes/Skills read-mostly and route writes through runtime APIs until a lock-aware sync manager is configured"
                .to_string(),
        );
    }
    if !autoscaling.queue_depth_scaling_present {
        runbook_actions.push(
            "add or enable deploy/k8s/remote-computer-keda.yaml before claiming remote computer pool autoscaling"
                .to_string(),
        );
    }
    if !warm_pool.manifest_present {
        runbook_actions.push(
            "add a warm-pool controller only after the Pod lease lifecycle and state sync are observable"
                .to_string(),
        );
    }
    if artifact_discovery_sidecar.present {
        runbook_actions.push(
            "monitor /api/remote-computers/sidecars/heartbeats and treat stale or missing artifact-discovery heartbeats as Remote Computer pilot blockers"
                .to_string(),
        );
        runbook_actions.push(
            "run /api/remote-computers/sidecars/recovery/run to produce an audited Pod replacement plan; enable MANDOFORGE_REMOTE_COMPUTER_SIDECAR_REPLACEMENT_ENABLED only after Kubernetes runner live mutation gates are validated"
                .to_string(),
        );
    }
    runbook_actions.push(
        "keep unassigned jobs on the existing approved worker path until Remote Computer lease assignment is production-hardened"
            .to_string(),
    );
    runbook_actions.push(
        "use /api/remote-computers/runner/dry-run to inspect Pod runner intent without mutating Kubernetes"
            .to_string(),
    );
    runbook_actions.push(
        "use /api/remote-computers/artifacts/discover for shared-workspace artifact scans, and keep sidecar-driven continuous discovery as a production hardening task"
            .to_string(),
    );
    runbook_actions.push(
        "enable MANDOFORGE_REMOTE_COMPUTER_EXECUTION_ENABLED only for explicit Kubernetes Pod exec pilots with approved shell.exec/codex.exec jobs"
            .to_string(),
    );
    runbook_actions.push(
        "run /api/remote-computers/reclaim-stale to clear stale attachment and expired lease records without executing tools"
            .to_string(),
    );

    let critical_count = attention_items
        .iter()
        .filter(|item| item.severity == "critical")
        .count() as i64;
    let warning_count = attention_items
        .iter()
        .filter(|item| item.severity == "warning")
        .count() as i64;
    let readiness_score = (100_i64 - critical_count * 25 - warning_count * 10).clamp(0, 100);
    let status = if critical_count > 0 {
        "critical"
    } else if warning_count > 0 {
        "attention"
    } else {
        "ready"
    }
    .to_string();

    Ok(RemoteComputerReadinessReport {
        generated_at,
        status,
        readiness_score,
        pod_template,
        service_account,
        state_filesystem,
        production_state_sync,
        network_policy,
        autoscaling,
        warm_pool,
        artifact_discovery_sidecar,
        artifact_discovery_sidecar_config,
        sidecar_supervision,
        sidecar_recovery,
        runner,
        execution_transport,
        event_types,
        attention_items,
        runbook_actions,
    })
}

fn remote_computer_manifest_readiness(
    path: &str,
    configured_status: &str,
) -> RemoteComputerManifestReadiness {
    let present = project_file_path(path).is_some();
    RemoteComputerManifestReadiness {
        present,
        path: path.to_string(),
        status: if present {
            configured_status
        } else {
            "missing"
        }
        .to_string(),
    }
}

fn remote_computer_artifact_discovery_sidecar_config_readiness()
-> RemoteComputerArtifactDiscoverySidecarConfigReadiness {
    let expected_api_url = "http://mandoforge-api:8787".to_string();
    let pod_template_path = "deploy/k8s/agent-remote-computer.yaml";
    let warm_pool_path = "deploy/k8s/remote-computer-warm-pool.yaml";
    let sidecar_config_path = "deploy/k8s/remote-computer-artifact-discovery-sidecar.yaml";
    let pod_template_api_url_configured =
        project_file_contains(pod_template_path, expected_api_url.as_str());
    let warm_pool_api_url_configured =
        project_file_contains(warm_pool_path, expected_api_url.as_str());
    let configmap_default_api_url_configured =
        project_file_contains(sidecar_config_path, expected_api_url.as_str());
    let mut blocking_reasons = Vec::new();
    if !pod_template_api_url_configured {
        blocking_reasons.push(
            "Remote Computer Pod template artifact sidecar does not target mandoforge-api:8787"
                .to_string(),
        );
    }
    if !warm_pool_api_url_configured {
        blocking_reasons.push(
            "Remote Computer warm-pool artifact sidecar does not target mandoforge-api:8787"
                .to_string(),
        );
    }
    if !configmap_default_api_url_configured {
        blocking_reasons.push(
            "artifact discovery sidecar ConfigMap default does not target mandoforge-api:8787"
                .to_string(),
        );
    }
    let status = if blocking_reasons.is_empty() {
        "configured"
    } else {
        "attention"
    }
    .to_string();
    RemoteComputerArtifactDiscoverySidecarConfigReadiness {
        status,
        expected_api_url,
        pod_template_api_url_configured,
        warm_pool_api_url_configured,
        configmap_default_api_url_configured,
        blocking_reasons,
    }
}

fn remote_computer_attention(
    kind: &str,
    severity: &str,
    message: &str,
) -> RemoteComputerAttentionItem {
    RemoteComputerAttentionItem {
        kind: kind.to_string(),
        severity: severity.to_string(),
        message: message.to_string(),
    }
}
