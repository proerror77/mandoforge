use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::{
    AppError, AppState, RemoteComputerAgentSandboxLiveEvidenceReadiness,
    RemoteComputerAgentSandboxReadiness, RemoteComputerArtifactDiscoverySidecarConfigReadiness,
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
    let agent_sandbox = build_remote_computer_agent_sandbox_readiness();

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
    if !agent_sandbox.static_contract_ready {
        attention_items.push(remote_computer_attention(
            "agent_sandbox_static_contract_missing",
            "critical",
            "Agent Sandbox dedicated image, tracked build context, template, egress, smoke lifecycle, or static verifier contract is incomplete",
        ));
    } else if agent_sandbox.live_evidence.status == "missing" {
        attention_items.push(remote_computer_attention(
            "agent_sandbox_live_evidence_missing",
            "critical",
            "Agent Sandbox static contracts are ready, but no live Claim/Sandbox/Pod lifecycle evidence bundle is present",
        ));
    } else if agent_sandbox.live_evidence.status == "ready" && agent_sandbox.production_blocked {
        attention_items.push(remote_computer_attention(
            "agent_sandbox_target_evidence_missing",
            "critical",
            "Agent Sandbox local live evidence is valid, but production_target evidence is required before production promotion",
        ));
    } else if agent_sandbox.production_blocked {
        attention_items.push(remote_computer_attention(
            "agent_sandbox_live_evidence_blocked",
            "critical",
            "Agent Sandbox live evidence is invalid, incomplete, or failed; keep the runtime pilot-only",
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
    runbook_actions.push(
        "run docs/runbooks/agent-sandbox-runtime-drill.md and archive a complete summary.json before promoting Agent Sandbox beyond pilot_only"
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
        agent_sandbox,
        event_types,
        attention_items,
        runbook_actions,
    })
}

const AGENT_SANDBOX_RUNTIME_IMAGE: &str = "ghcr.io/proerror77/mandoforge/mandoforge-agent-sandbox-runtime@sha256:6a87a608da1a06b4bbfb3bda7ebf95d9cd608655a1f7b7d499d74fb40c6fbfb5";
const DEFAULT_AGENT_SANDBOX_CONTROLLER_VERSION: &str = "v0.5.1";
const DEFAULT_AGENT_SANDBOX_EVIDENCE_MAX_AGE_HOURS: i64 = 168;
const AGENT_SANDBOX_REQUIRED_LIVE_CHECKS: &[&str] = &[
    "controller_ready",
    "claim_bound",
    "pod_ready",
    "runtime_versions",
    "workspace_reuse",
    "cross_session_isolation",
    "cache_scope",
    "network_policy",
    "cancel_cleanup",
    "ttl_cleanup",
    "retry_idempotency",
    "approved_exec",
    "durable_event",
    "artifact",
    "audit_log",
];
const AGENT_SANDBOX_REQUIRED_PRODUCTION_CHECKS: &[&str] = &[
    "target_cluster",
    "rwx_cache",
    "distributed_state_sync",
    "network_enforcement",
    "load_validation",
    "rollback_validation",
];

fn build_remote_computer_agent_sandbox_readiness() -> RemoteComputerAgentSandboxReadiness {
    let runtime_dockerfile_present =
        project_file_contains(
            "Dockerfile.agent-sandbox",
            "/usr/local/bin/mandoforge-sandbox-runtime",
        ) && project_file_contains("Dockerfile.agent-sandbox", ".mandoforge-tracked-context");
    let tracked_context_builder_present = project_file_contains(
        "scripts/build-agent-sandbox-runtime-image.sh",
        "git checkout-index --all --force",
    ) && project_file_contains(
        "scripts/build-agent-sandbox-runtime-image.sh",
        "git write-tree",
    );
    let runtime_manifest_present = project_file_contains(
        "deploy/k8s/agent-sandbox-runtime.yaml",
        &format!("image: {AGENT_SANDBOX_RUNTIME_IMAGE}"),
    ) && project_file_contains(
        "deploy/k8s/agent-sandbox-runtime.yaml",
        "value: \"/workspace/sessions\"",
    ) && project_file_contains(
        "deploy/k8s/agent-sandbox-runtime.yaml",
        "readOnlyRootFilesystem: true",
    );
    let egress_policy_present = project_file_contains(
        "deploy/k8s/agent-sandbox-egress-networkpolicy.yaml",
        "name: mandoforge-agent-sandbox-egress",
    ) && project_file_contains(
        "deploy/k8s/agent-sandbox-egress-networkpolicy.yaml",
        "169.254.0.0/16",
    );
    let smoke_claim_present = project_file_contains(
        "deploy/agent-sandbox-smoke/sandbox-claim.yaml",
        "shutdownPolicy: Delete",
    ) && project_file_contains(
        "deploy/agent-sandbox-smoke/sandbox-claim.yaml",
        "ttlSecondsAfterFinished: 300",
    );
    let static_verifier_present = project_file_contains(
        "scripts/verify-remote-computer-k8s-manifests.sh",
        "extract_rendered_resource",
    ) && project_file_contains(
        "scripts/verify-remote-computer-k8s-manifests.sh",
        "agent_sandbox_egress_render",
    );
    let static_contract_ready = runtime_dockerfile_present
        && tracked_context_builder_present
        && runtime_manifest_present
        && egress_policy_present
        && smoke_claim_present
        && static_verifier_present;
    let live_evidence = build_agent_sandbox_live_evidence_readiness();
    let mut blocking_reasons = Vec::new();
    if !static_contract_ready {
        blocking_reasons.push("Agent Sandbox static runtime contract is incomplete".to_string());
    }
    blocking_reasons.extend(live_evidence.blocking_reasons.iter().cloned());
    let production_scope_validated = live_evidence.production_ready;
    if live_evidence.status == "ready" && !production_scope_validated {
        blocking_reasons.push(
            "Agent Sandbox production promotion requires validation_scope=production_target"
                .to_string(),
        );
    }
    let production_blocked = !static_contract_ready || !production_scope_validated;
    let status = if !static_contract_ready {
        "blocked"
    } else if production_blocked {
        "pilot_only"
    } else {
        "live_validated"
    }
    .to_string();

    RemoteComputerAgentSandboxReadiness {
        status,
        static_contract_ready,
        production_blocked,
        runtime_image: AGENT_SANDBOX_RUNTIME_IMAGE.to_string(),
        runtime_dockerfile_present,
        tracked_context_builder_present,
        runtime_manifest_present,
        egress_policy_present,
        smoke_claim_present,
        static_verifier_present,
        live_evidence,
        blocking_reasons,
    }
}

fn build_agent_sandbox_live_evidence_readiness() -> RemoteComputerAgentSandboxLiveEvidenceReadiness
{
    let path = agent_sandbox_evidence_path();
    let path_display = path.to_string_lossy().to_string();
    let expected_controller_version = expected_agent_sandbox_controller_version();
    let max_age_hours = agent_sandbox_evidence_max_age_hours();
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return RemoteComputerAgentSandboxLiveEvidenceReadiness {
                path: path_display,
                present: false,
                valid: false,
                status: "missing".to_string(),
                captured_at: None,
                cluster_context: None,
                controller_version: None,
                validation_scope: None,
                production_checks: Value::Null,
                production_ready: false,
                expected_controller_version,
                max_age_hours,
                age_hours: None,
                fresh: false,
                checks: Value::Null,
                blocking_reasons: vec![
                    "Agent Sandbox live evidence summary is missing".to_string(),
                ],
            };
        }
        Err(error) => {
            return RemoteComputerAgentSandboxLiveEvidenceReadiness {
                path: path_display,
                present: true,
                valid: false,
                status: "invalid".to_string(),
                captured_at: None,
                cluster_context: None,
                controller_version: None,
                validation_scope: None,
                production_checks: Value::Null,
                production_ready: false,
                expected_controller_version,
                max_age_hours,
                age_hours: None,
                fresh: false,
                checks: Value::Null,
                blocking_reasons: vec![format!(
                    "failed to read Agent Sandbox live evidence: {error}"
                )],
            };
        }
    };
    let evidence: Value = match serde_json::from_str(&content) {
        Ok(evidence) => evidence,
        Err(error) => {
            return RemoteComputerAgentSandboxLiveEvidenceReadiness {
                path: path_display,
                present: true,
                valid: false,
                status: "invalid".to_string(),
                captured_at: None,
                cluster_context: None,
                controller_version: None,
                validation_scope: None,
                production_checks: Value::Null,
                production_ready: false,
                expected_controller_version,
                max_age_hours,
                age_hours: None,
                fresh: false,
                checks: Value::Null,
                blocking_reasons: vec![format!(
                    "failed to parse Agent Sandbox live evidence: {error}"
                )],
            };
        }
    };

    let mut structural_errors = Vec::new();
    if evidence.get("schema_version").and_then(Value::as_u64) != Some(1) {
        structural_errors.push("schema_version must equal 1".to_string());
    }
    let declared_status = evidence.get("status").and_then(Value::as_str);
    if declared_status.is_none() {
        structural_errors.push("status must be a string".to_string());
    }
    let captured_at = evidence
        .get("captured_at")
        .and_then(Value::as_str)
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc));
    if captured_at.is_none() {
        structural_errors.push("captured_at must be an RFC3339 timestamp".to_string());
    }
    let age_hours = captured_at.map(|value| Utc::now().signed_duration_since(value).num_hours());
    let fresh = age_hours.is_some_and(|age| (-1..=max_age_hours).contains(&age));
    if let Some(age) = age_hours {
        if age < -1 {
            structural_errors
                .push("live evidence captured_at is too far in the future".to_string());
        } else if age > max_age_hours {
            structural_errors.push(format!(
                "live evidence is stale: age {age}h exceeds {max_age_hours}h"
            ));
        }
    }
    let cluster_context = non_empty_evidence_string(&evidence, "cluster_context");
    if cluster_context.is_none() {
        structural_errors.push("cluster_context must be a non-empty string".to_string());
    }
    let controller_version = non_empty_evidence_string(&evidence, "controller_version");
    if controller_version.is_none() {
        structural_errors.push("controller_version must be a non-empty string".to_string());
    } else if controller_version.as_deref() != Some(expected_controller_version.as_str()) {
        structural_errors.push(format!(
            "controller version must equal {expected_controller_version}"
        ));
    }
    let validation_scope = non_empty_evidence_string(&evidence, "validation_scope");
    if !matches!(
        validation_scope.as_deref(),
        Some("local_pilot" | "production_target")
    ) {
        structural_errors
            .push("validation_scope must equal local_pilot or production_target".to_string());
    }
    let production_checks = evidence
        .get("production_checks")
        .cloned()
        .unwrap_or(Value::Null);
    if validation_scope.as_deref() == Some("production_target") {
        if !production_checks.is_object() {
            structural_errors
                .push("production_checks must be an object for production_target".to_string());
        }
        for check in AGENT_SANDBOX_REQUIRED_PRODUCTION_CHECKS {
            match production_checks.get(*check).and_then(Value::as_bool) {
                Some(true) => {}
                Some(false) => structural_errors.push(format!(
                    "production check must pass for production_target: {check}"
                )),
                None => structural_errors.push(format!(
                    "production check is missing or not boolean: {check}"
                )),
            }
        }
    }
    let checks = evidence.get("checks").cloned().unwrap_or(Value::Null);
    if !checks.is_object() {
        structural_errors.push("checks must be an object".to_string());
    }
    let mut failed_checks = Vec::new();
    for check in AGENT_SANDBOX_REQUIRED_LIVE_CHECKS {
        match checks.get(*check).and_then(Value::as_bool) {
            Some(true) => {}
            Some(false) => failed_checks.push(format!("live check failed: {check}")),
            None => {
                structural_errors.push(format!("live check is missing or not boolean: {check}"))
            }
        }
    }
    let valid = structural_errors.is_empty();
    let ready = valid && declared_status == Some("passed") && failed_checks.is_empty();
    let production_ready = ready
        && validation_scope.as_deref() == Some("production_target")
        && AGENT_SANDBOX_REQUIRED_PRODUCTION_CHECKS
            .iter()
            .all(|check| production_checks.get(*check).and_then(Value::as_bool) == Some(true));
    let mut blocking_reasons = structural_errors;
    blocking_reasons.extend(failed_checks);
    if valid && declared_status != Some("passed") {
        blocking_reasons.push("live evidence status is not passed".to_string());
    }

    RemoteComputerAgentSandboxLiveEvidenceReadiness {
        path: path_display,
        present: true,
        valid,
        status: if ready { "ready" } else { "blocked" }.to_string(),
        captured_at,
        cluster_context,
        controller_version,
        validation_scope,
        production_checks,
        production_ready,
        expected_controller_version,
        max_age_hours,
        age_hours,
        fresh,
        checks,
        blocking_reasons,
    }
}

fn expected_agent_sandbox_controller_version() -> String {
    std::env::var("MANDOFORGE_AGENT_SANDBOX_CONTROLLER_VERSION")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_AGENT_SANDBOX_CONTROLLER_VERSION.to_string())
}

fn agent_sandbox_evidence_max_age_hours() -> i64 {
    std::env::var("MANDOFORGE_AGENT_SANDBOX_EVIDENCE_MAX_AGE_HOURS")
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_AGENT_SANDBOX_EVIDENCE_MAX_AGE_HOURS)
}

fn agent_sandbox_evidence_path() -> PathBuf {
    std::env::var("MANDOFORGE_AGENT_SANDBOX_EVIDENCE_FILE")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".mandoforge/agent-sandbox-runtime-evidence/summary.json"))
}

fn non_empty_evidence_string(evidence: &Value, key: &str) -> Option<String> {
    evidence
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
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
