use chrono::Utc;

use crate::{Stage2CompletionReadiness, Stage2EvidenceRequirement, project_file_path};

pub(crate) fn build_stage2_completion_readiness() -> Stage2CompletionReadiness {
    let audit_path = "docs/stage2-completion-audit.md";
    let audit_content =
        project_file_path(audit_path).and_then(|path| std::fs::read_to_string(path).ok());
    let audit_present = audit_content.is_some();
    let open_gaps = audit_content
        .as_deref()
        .map(parse_stage2_open_gaps)
        .unwrap_or_else(|| {
            vec!["Stage 2 completion audit file is missing; completion is blocked".to_string()]
        });
    let completion_blocked = !open_gaps.is_empty();
    let status = if completion_blocked {
        "blocked"
    } else {
        "ready"
    }
    .to_string();
    let message = if completion_blocked {
        format!(
            "Stage 2 completion is blocked by {} open audit gap(s)",
            open_gaps.len()
        )
    } else {
        "Stage 2 completion audit reports no open gaps".to_string()
    };
    Stage2CompletionReadiness {
        generated_at: Utc::now(),
        status,
        objective: "Complete Stage 2 Governed Runtime Pilot".to_string(),
        audit_path: audit_path.to_string(),
        audit_present,
        open_gap_count: open_gaps.len(),
        evidence_requirements: build_stage2_evidence_requirements(&open_gaps),
        open_gaps,
        completion_blocked,
        message,
    }
}

fn build_stage2_evidence_requirements(open_gaps: &[String]) -> Vec<Stage2EvidenceRequirement> {
    struct Stage2EvidenceRequirementSpec<'a> {
        id: &'a str,
        title: &'a str,
        category: &'a str,
        required_for_stage2_production: bool,
        production_target: &'a str,
        evidence_scripts: Vec<&'a str>,
        evidence_job_manifests: Vec<&'a str>,
        readiness_endpoints: Vec<&'a str>,
        validation_endpoints: Vec<&'a str>,
        required_flags: Vec<&'a str>,
        required_artifacts: Vec<&'a str>,
        required_evidence: Vec<&'a str>,
    }

    let specs = [
        Stage2EvidenceRequirementSpec {
            id: "tenant-routing",
            title: "Cross-tenant runtime isolation",
            category: "enterprise_optional",
            required_for_stage2_production: false,
            production_target: "Real multi-tenant routing target with RLS enabled, forced, and tenant context configured",
            evidence_scripts: vec!["./scripts/tenant-isolation-evidence-gate.sh"],
            evidence_job_manifests: vec![
                "deploy/stage2-evidence/tenant-isolation-evidence-job.example.yaml",
            ],
            readiness_endpoints: vec!["/api/tenant-isolation/readiness"],
            validation_endpoints: vec!["/api/tenant-isolation/routing/validate"],
            required_flags: vec![
                "RUN_STAGE2_PRODUCTION_VALIDATIONS=1",
                "MANDOFORGE_TENANT_ROUTING_CONTROLLER_REQUIRED=true",
            ],
            required_artifacts: vec![
                "production-evidence-run.json",
                "api-tenant-isolation-routing-validate.json",
                "tenant-routing-validation-evidence.json",
            ],
            required_evidence: vec![
                "runtime tenant routing is not single-tenant",
                "tracked tenant tables have enabled and forced RLS",
                "external routing controller validates cross-tenant isolation when required",
            ],
        },
        Stage2EvidenceRequirementSpec {
            id: "policy-rollout",
            title: "Policy rollout orchestration",
            category: "enterprise_optional",
            required_for_stage2_production: false,
            production_target: "Real production policy rollout controller target",
            evidence_scripts: vec!["./scripts/policy-rollout-evidence-gate.sh"],
            evidence_job_manifests: vec![
                "deploy/stage2-evidence/policy-rollout-evidence-job.example.yaml",
            ],
            readiness_endpoints: vec!["/api/policy/rollout/orchestration/readiness"],
            validation_endpoints: vec!["/api/policy/rollout/orchestration/validate"],
            required_flags: vec![
                "RUN_STAGE2_PRODUCTION_VALIDATIONS=1",
                "MANDOFORGE_POLICY_ROLLOUT_ORCHESTRATION_CONTROLLER_REQUIRED=true",
                "RUN_STAGE2_POLICY_DUE_RUN=1",
            ],
            required_artifacts: vec![
                "production-evidence-run.json",
                "policy-rollout-orchestration-validation-evidence.json",
                "policy-rollout-due-run-evidence.json",
            ],
            required_evidence: vec![
                "fresh due-run or rollout supervision evidence",
                "external orchestration controller confirms production target state when required",
                "rollback path remains available and audited",
            ],
        },
        Stage2EvidenceRequirementSpec {
            id: "provider-rollout",
            title: "Provider policy gate workflow",
            category: "stage2_production",
            required_for_stage2_production: true,
            production_target: "Real provider deployment, rollout, and rollback target",
            evidence_scripts: vec!["./scripts/provider-governance-evidence-gate.sh"],
            evidence_job_manifests: vec![
                "deploy/stage2-evidence/provider-governance-evidence-job.example.yaml",
            ],
            readiness_endpoints: vec![
                "/api/providers/summary",
                "/api/providers/policy-gate",
                "/api/providers/policy-gate/runs",
            ],
            validation_endpoints: vec![
                "/api/providers/policy-gate/run",
                "/api/providers/deployment/validate",
                "/api/providers/production-rollout/run",
                "/api/providers/production-rollout/rollback",
            ],
            required_flags: vec![
                "RUN_STAGE2_PRODUCTION_VALIDATIONS=1",
                "MANDOFORGE_PROVIDER_DEPLOYMENT_CONTROLLER_REQUIRED=true",
                "RUN_STAGE2_PROVIDER_ROLLOUT=1",
            ],
            required_artifacts: vec![
                "api-providers-policy-gate-run.json",
                "api-providers-deployment-validate.json",
                "provider-production-rollout-evidence.json",
                "provider-production-rollback-evidence.json",
            ],
            required_evidence: vec![
                "fresh provider gate covers the current provider set",
                "deployment controller validates real provider target",
                "rollout and rollback controller evidence is audited",
            ],
        },
        Stage2EvidenceRequirementSpec {
            id: "vault-kms",
            title: "Secret lifecycle and KMS/HSM recovery",
            category: "enterprise_optional",
            required_for_stage2_production: false,
            production_target: "Real Vault plus external KMS/HSM lifecycle target",
            evidence_scripts: vec!["./scripts/vault-evidence-gate.sh"],
            evidence_job_manifests: vec!["deploy/stage2-evidence/vault-evidence-job.example.yaml"],
            readiness_endpoints: vec!["/api/vault/readiness", "/api/vault/health"],
            validation_endpoints: vec![
                "/api/vault/kms/rotation/run",
                "/api/vault/kms/recovery/validate",
            ],
            required_flags: vec![
                "RUN_STAGE2_PRODUCTION_VALIDATIONS=1",
                "MANDOFORGE_KMS_RECOVERY_CONTROLLER_REQUIRED=true",
                "RUN_STAGE2_SECRET_LIFECYCLE=1",
            ],
            required_artifacts: vec![
                "production-evidence-run.json",
                "vault-kms-recovery-evidence.json",
                "vault-kms-rotation-evidence.json",
            ],
            required_evidence: vec![
                "Vault provider is healthy and selected",
                "KMS rotation evidence exists without exposing secret values",
                "recovery drill validates a real KMS/HSM target when required",
            ],
        },
        Stage2EvidenceRequirementSpec {
            id: "worker-remote-computer",
            title: "Worker autoscaling and Remote Computer real-cluster validation",
            category: "enterprise_optional",
            required_for_stage2_production: false,
            production_target: "Durable worker queue, isolated worker pool, and real Remote Computer state filesystem",
            evidence_scripts: vec![
                "./scripts/worker-evidence-gate.sh",
                "./scripts/remote-computer-evidence-gate.sh",
                "./scripts/worker-remote-computer-evidence-gate.sh",
            ],
            evidence_job_manifests: vec![
                "deploy/stage2-evidence/worker-evidence-job.example.yaml",
                "deploy/stage2-evidence/remote-computer-evidence-job.example.yaml",
                "deploy/stage2-evidence/worker-remote-computer-evidence-job.example.yaml",
            ],
            readiness_endpoints: vec![
                "/api/execution-jobs/worker-readiness",
                "/api/remote-computers/readiness",
                "/api/remote-computers/runner/readiness",
            ],
            validation_endpoints: vec![
                "/api/execution-jobs/worker-load-validation/run",
                "/api/remote-computers/state-sync/validate",
                "/api/remote-computers/sidecars/recovery/run",
            ],
            required_flags: vec![
                "RUN_STAGE2_PRODUCTION_VALIDATIONS=1",
                "MANDOFORGE_WORKER_LOAD_VALIDATION_CONTROLLER_REQUIRED=true",
                "MANDOFORGE_REMOTE_COMPUTER_STATE_SYNC_CONTROLLER_REQUIRED=true",
                "MANDOFORGE_REMOTE_COMPUTER_SIDECAR_VALIDATION_REQUIRED=true",
                "RUN_STAGE2_REMOTE_SIDECAR_RECOVERY=1",
            ],
            required_artifacts: vec![
                "production-evidence-run.json",
                "worker-load-validation-evidence.json",
                "remote-computer-state-sync-evidence.json",
                "remote-computer-sidecar-recovery-evidence.json",
                "worker-remote-computer/summary.json",
            ],
            required_evidence: vec![
                "durable queue-backed worker mode is enabled",
                "production-like load validation proves autoscaling and worker-pool isolation",
                "Remote Computer distributed state sync and sidecar replacement are validated against a real cluster",
            ],
        },
        Stage2EvidenceRequirementSpec {
            id: "approval-notifications",
            title: "Approval notification operations",
            category: "stage2_production",
            required_for_stage2_production: true,
            production_target: "Real webhook, Slack, or email notification provider targets",
            evidence_scripts: vec!["./scripts/approval-notification-evidence-gate.sh"],
            evidence_job_manifests: vec![
                "deploy/stage2-evidence/approval-notification-evidence-job.example.yaml",
            ],
            readiness_endpoints: vec![
                "/api/approvals/notification-routing/summary",
                "/api/approvals/notifications/runs",
            ],
            validation_endpoints: vec![
                "/api/approvals/notifications/deployment/validate",
                "/api/approvals/notifications/ops/validate",
                "/api/approvals/notifications/run",
            ],
            required_flags: vec![
                "RUN_STAGE2_PRODUCTION_VALIDATIONS=1",
                "MANDOFORGE_APPROVAL_NOTIFICATION_DEPLOYMENT_CONTROLLER_REQUIRED=true",
                "MANDOFORGE_APPROVAL_NOTIFICATION_OPS_CONTROLLER_REQUIRED=true",
                "RUN_STAGE2_APPROVAL_DELIVERY=1",
            ],
            required_artifacts: vec![
                "api-approvals-notifications-deployment-validate.json",
                "api-approvals-notifications-ops-validate.json",
                "approval-notification-delivery-evidence.json",
            ],
            required_evidence: vec![
                "persisted channel policies route pending approvals",
                "deployment and ops controllers validate real delivery providers",
                "bounded delivery attempts are audited",
            ],
        },
        Stage2EvidenceRequirementSpec {
            id: "mcp-rollout",
            title: "MCP connector rollout orchestration",
            category: "stage2_production",
            required_for_stage2_production: true,
            production_target: "Team-scoped MCP deployment, rollout, and rollback target",
            evidence_scripts: vec!["./scripts/mcp-gateway-evidence-gate.sh"],
            evidence_job_manifests: vec![
                "deploy/stage2-evidence/mcp-gateway-evidence-job.example.yaml",
            ],
            readiness_endpoints: vec![
                "/api/teams/{team_id}/mcp-servers/rollouts/summary",
                "/api/teams/{team_id}/mcp-servers/rollouts/runs",
            ],
            validation_endpoints: vec![
                "/api/teams/{team_id}/mcp-servers/deployment/validate",
                "/api/teams/{team_id}/mcp-servers/rollouts/run-due",
            ],
            required_flags: vec![
                "RUN_STAGE2_PRODUCTION_VALIDATIONS=1",
                "MANDOFORGE_MCP_DEPLOYMENT_CONTROLLER_REQUIRED=true",
                "MANDOFORGE_MCP_ROLLOUT_CONTROLLER_REQUIRED=true",
                "MANDOFORGE_MCP_ROLLOUT_ROLLBACK_CONTROLLER_REQUIRED=true",
                "RUN_STAGE2_MCP_DUE_RUN=1",
                "RUN_STAGE2_MCP_ROLLBACK=1",
            ],
            required_artifacts: vec![
                "team-discovery.json",
                "mcp-deployment-validation-evidence.json",
                "mcp-rollout-due-run-evidence.json",
                "mcp-rollback-evidence.json",
            ],
            required_evidence: vec![
                "team MCP connector health and rollout state are fresh",
                "deployment controller validates real connector supervision",
                "rollout and rollback controller evidence is present when required",
            ],
        },
        Stage2EvidenceRequirementSpec {
            id: "codex-app-server",
            title: "Codex App Server control-plane operations",
            category: "core_runtime",
            required_for_stage2_production: true,
            production_target: "Real Codex App Server deployment and ops target",
            evidence_scripts: vec!["./scripts/codex-app-server-evidence-gate.sh"],
            evidence_job_manifests: vec![
                "deploy/stage2-evidence/codex-app-server-evidence-job.example.yaml",
            ],
            readiness_endpoints: vec!["/api/codex-app-server/control-plane/summary"],
            validation_endpoints: vec![
                "/api/codex-app-server/deployment/validate",
                "/api/codex-app-server/ops/validate",
                "/api/codex-app-server/runs/poll-stale",
            ],
            required_flags: vec![
                "RUN_STAGE2_PRODUCTION_VALIDATIONS=1",
                "MANDOFORGE_CODEX_APP_SERVER_DEPLOYMENT_CONTROLLER_REQUIRED=true",
                "MANDOFORGE_CODEX_APP_SERVER_OPS_CONTROLLER_REQUIRED=true",
                "RUN_STAGE2_CODEX_STALE_POLL=1",
            ],
            required_artifacts: vec![
                "api-codex-app-server-deployment-validate.json",
                "api-codex-app-server-ops-validate.json",
                "codex-app-server-stale-poll-evidence.json",
            ],
            required_evidence: vec![
                "deployment validation is fresh and healthy",
                "ops validation proves stale turn supervision",
                "controller evidence is present when required",
            ],
        },
        Stage2EvidenceRequirementSpec {
            id: "managed-session-restart-resume",
            title: "Managed-session restart and resume proof",
            category: "core_runtime",
            required_for_stage2_production: true,
            production_target: "Real API and worker restart drill proving session-loop recovery, thread lineage, and lease fencing",
            evidence_scripts: vec!["./scripts/managed-session-runtime-evidence-gate.sh"],
            evidence_job_manifests: vec![
                "deploy/stage2-evidence/managed-session-runtime-evidence-job.example.yaml",
            ],
            readiness_endpoints: vec!["/api/stage2/readiness"],
            validation_endpoints: vec!["./scripts/managed-session-runtime-evidence-gate.sh"],
            required_flags: vec![
                "RUN_STAGE2_MANAGED_SESSION_RESTART_RESUME=1",
                "MANDOFORGE_STAGE2_MANAGED_SESSION_RUNTIME_TARGET_ID=managed-session-runtime-prod",
                "MANAGED_SESSION_RESTART_RESUME_CONTROLLER_URL=https://controller.example.com/mandoforge/managed-sessions/restart-resume/validate",
            ],
            required_artifacts: vec![
                "production-evidence-run.json",
                "managed-session-restart-resume-evidence.json",
            ],
            required_evidence: vec![
                "session event enqueue and worker drain are observed before restart",
                "API and worker are restarted during the drill",
                "resumed session state preserves processed event cursor, thread lineage, final runtime turn, and final message",
                "lease fencing rejects stale workers and finalizes through the active lease only",
            ],
        },
        Stage2EvidenceRequirementSpec {
            id: "managed-workflow-runtime-proof",
            title: "Managed workflow runtime proof",
            category: "stage2_production",
            required_for_stage2_production: true,
            production_target: "End-to-end managed workflow proof covering graph transitions, scheduler due activation, result artifact, and memory governance drilldown",
            evidence_scripts: vec!["./scripts/managed-workflow-runtime-evidence-gate.sh"],
            evidence_job_manifests: vec![
                "deploy/stage2-evidence/stage2-production-evidence-gate-job.example.yaml",
            ],
            readiness_endpoints: vec![
                "/api/workflow-runs",
                "/api/scheduler/due-plan",
                "/api/memory-governance/summary",
            ],
            validation_endpoints: vec!["./scripts/managed-workflow-runtime-evidence-gate.sh"],
            required_flags: vec!["RUN_STAGE2_MANAGED_WORKFLOW_RUNTIME=1"],
            required_artifacts: vec![
                "local-script-scripts-managed-workflow-runtime-evidence-gate.sh.json",
                "api-workflow-runtime-proof-graph.json",
                "api-workflow-runtime-proof-transitions.json",
                "api-workflow-runtime-proof-memory-governance-partition.json",
            ],
            required_evidence: vec![
                "workflow graph reaches completed through durable transition records",
                "scheduler due-run activates scheduled retry steps rather than a manual workflow-only endpoint",
                "result artifact is linked to the workflow primary session",
                "memory governance exposes partition drilldown and writeback queue evidence",
            ],
        },
        Stage2EvidenceRequirementSpec {
            id: "eval-release",
            title: "Eval and release rollout orchestration",
            category: "stage2_production",
            required_for_stage2_production: true,
            production_target: "Real production agent release target",
            evidence_scripts: vec!["./scripts/eval-release-evidence-gate.sh"],
            evidence_job_manifests: vec![
                "deploy/stage2-evidence/eval-release-evidence-job.example.yaml",
            ],
            readiness_endpoints: vec!["/api/agents/releases/automation-runs"],
            validation_endpoints: vec![
                "/api/eval/suites/stage2-regression",
                "/api/agents/releases/deployment/validate",
                "/api/agents/releases/orchestration/validate",
                "/api/agents/releases/run-due",
            ],
            required_flags: vec![
                "RUN_STAGE2_PRODUCTION_VALIDATIONS=1",
                "MANDOFORGE_AGENT_RELEASE_CONTROLLER_REQUIRED=true",
                "MANDOFORGE_AGENT_RELEASE_DEPLOYMENT_CONTROLLER_REQUIRED=true",
                "MANDOFORGE_AGENT_RELEASE_ORCHESTRATION_CONTROLLER_REQUIRED=true",
                "MANDOFORGE_AGENT_RELEASE_ROLLBACK_CONTROLLER_REQUIRED=true",
                "RUN_STAGE2_EVAL_RELEASE_AUTOMATION=1",
                "RUN_STAGE2_EVAL_RELEASE_ROLLBACK=1",
            ],
            required_artifacts: vec![
                "eval-release-stage2-regression-evidence.json",
                "eval-release-deployment-validation-evidence.json",
                "eval-release-orchestration-validation-evidence.json",
                "eval-release-due-run-evidence.json",
                "eval-release-rollback-evidence.json",
            ],
            required_evidence: vec![
                "Stage 2 regression suite has passing gate evidence",
                "release deployment and orchestration controllers validate real target state",
                "rollback evidence exists for promoted releases",
            ],
        },
        Stage2EvidenceRequirementSpec {
            id: "observability-collector",
            title: "Real-cluster collector rollout",
            category: "stage2_production",
            required_for_stage2_production: true,
            production_target: "Real OTLP collector deployment and cluster rollout target",
            evidence_scripts: vec!["./scripts/observability-collector-evidence-gate.sh"],
            evidence_job_manifests: vec![
                "deploy/stage2-evidence/observability-collector-evidence-job.example.yaml",
            ],
            readiness_endpoints: vec![
                "/api/observability",
                "/api/observability/collector-readiness",
            ],
            validation_endpoints: vec![
                "/api/observability/collector/deployment/validate",
                "/api/observability/collector/cluster/validate",
                "/api/observability/remediation/run",
            ],
            required_flags: vec![
                "RUN_STAGE2_PRODUCTION_VALIDATIONS=1",
                "MANDOFORGE_OBSERVABILITY_COLLECTOR_DEPLOYMENT_CONTROLLER_REQUIRED=true",
                "MANDOFORGE_OBSERVABILITY_COLLECTOR_CLUSTER_CONTROLLER_REQUIRED=true",
                "MANDOFORGE_OBSERVABILITY_REMEDIATION_CONTROLLER_REQUIRED=true",
                "RUN_STAGE2_OBSERVABILITY_REMEDIATION=1",
            ],
            required_artifacts: vec![
                "observability-collector-deployment-evidence.json",
                "observability-collector-cluster-rollout-evidence.json",
                "observability-collector-remediation-evidence.json",
            ],
            required_evidence: vec![
                "OTLP export is enabled and collector health is fresh",
                "deployment and cluster rollout controllers validate real collector paths",
                "logs, traces, and metrics endpoints are configured",
            ],
        },
        Stage2EvidenceRequirementSpec {
            id: "finance-close",
            title: "Finance close and accounting reconciliation",
            category: "enterprise_optional",
            required_for_stage2_production: false,
            production_target: "Real accounting-system reconciliation target",
            evidence_scripts: vec!["./scripts/finance-evidence-gate.sh"],
            evidence_job_manifests: vec![
                "deploy/stage2-evidence/finance-evidence-job.example.yaml",
            ],
            readiness_endpoints: vec!["/api/usage/finance-operations/summary"],
            validation_endpoints: vec![
                "/api/usage/finance-operations/run",
                "/api/usage/finance-operations/reconcile",
            ],
            required_flags: vec![
                "RUN_STAGE2_PRODUCTION_VALIDATIONS=1",
                "MANDOFORGE_FINANCE_CLOSE_CONTROLLER_REQUIRED=true",
                "MANDOFORGE_FINANCE_RECONCILIATION_CONTROLLER_REQUIRED=true",
                "FINANCE_EXPORT_DELIVERY_OBSERVER_URL=https://controller.example.com/mandoforge/finance/export/observer",
                "RUN_STAGE2_FINANCE_CONTROLLERS=1",
                "RUN_STAGE2_FINANCE_EXPORT=1",
            ],
            required_artifacts: vec![
                "production-evidence-run.json",
                "finance-close-evidence.json",
                "finance-reconciliation-evidence.json",
                "usage-export-csv-evidence.json",
                "finance-export-delivery-evidence.json",
                "finance-export-delivery-observer.json",
            ],
            required_evidence: vec![
                "usage rollup and export evidence is fresh",
                "finance close controller confirms production close",
                "accounting reconciliation controller confirms real target reconciliation when required",
                "export delivery observer confirms a true accounting/ERP target rather than Feishu Drive artifact delivery",
            ],
        },
        Stage2EvidenceRequirementSpec {
            id: "ui-production-polish",
            title: "Production UI CRUD and dashboard polish",
            category: "stage2_production",
            required_for_stage2_production: true,
            production_target: "Operator UI flows for production governance tasks",
            evidence_scripts: vec![
                "./scripts/verify-static-ui-actionbook.sh",
                "./scripts/verify-static-ui-assets.sh",
            ],
            evidence_job_manifests: vec![
                "deploy/stage2-evidence/stage2-production-evidence-gate-job.example.yaml",
            ],
            readiness_endpoints: vec!["/api/stage2/readiness"],
            validation_endpoints: vec![
                "./scripts/verify-static-ui-actionbook.sh",
                "./scripts/verify-static-ui-assets.sh",
            ],
            required_flags: vec![
                "RUN_STAGE2_UI_ACTIONBOOK=1",
                "RUN_STAGE2_UI_STATIC_ASSETS=1",
            ],
            required_artifacts: vec![
                "local-script-scripts-verify-static-ui-actionbook.sh.json",
                "local-script-scripts-verify-static-ui-assets.sh.json",
            ],
            required_evidence: vec![
                "admin CRUD flows expose create/update/archive/delete where applicable",
                "dashboard surfaces production gate evidence without relying on green proxy checks",
                "static UI smoke covers Stage 2 readiness and key operator panels",
                "browserless static asset smoke covers key labels, routes, and form-based controls",
            ],
        },
    ];

    specs
        .iter()
        .enumerate()
        .map(|(index, spec)| {
            let required_for_core = spec.category == "core_runtime";
            let enterprise_optional = spec.category == "enterprise_optional";

            Stage2EvidenceRequirement {
                id: spec.id.to_string(),
                title: spec.title.to_string(),
                category: spec.category.to_string(),
                required_for_core,
                required_for_stage2_production: spec.required_for_stage2_production,
                enterprise_optional,
                gap: open_gaps.get(index).cloned().unwrap_or_default(),
                production_target: spec.production_target.to_string(),
                evidence_scripts: spec
                    .evidence_scripts
                    .iter()
                    .map(|script| script.to_string())
                    .collect(),
                evidence_job_manifests: spec
                    .evidence_job_manifests
                    .iter()
                    .map(|manifest| manifest.to_string())
                    .collect(),
                readiness_endpoints: spec
                    .readiness_endpoints
                    .iter()
                    .map(|endpoint| endpoint.to_string())
                    .collect(),
                validation_endpoints: spec
                    .validation_endpoints
                    .iter()
                    .map(|endpoint| endpoint.to_string())
                    .collect(),
                required_flags: spec
                    .required_flags
                    .iter()
                    .map(|flag| flag.to_string())
                    .collect(),
                required_artifacts: spec
                    .required_artifacts
                    .iter()
                    .map(|artifact| artifact.to_string())
                    .collect(),
                required_evidence: spec
                    .required_evidence
                    .iter()
                    .map(|evidence| evidence.to_string())
                    .collect(),
            }
        })
        .collect()
}

pub(crate) fn parse_stage2_open_gaps(audit_content: &str) -> Vec<String> {
    let mut in_gaps = false;
    let mut gaps = Vec::new();
    for line in audit_content.lines() {
        let trimmed = line.trim();
        if trimmed == "## Open Completion Gaps" {
            in_gaps = true;
            continue;
        }
        if in_gaps && trimmed.starts_with("## ") {
            break;
        }
        if !in_gaps {
            continue;
        }
        if let Some((number, text)) = trimmed.split_once(". ") {
            if number.chars().all(|char| char.is_ascii_digit()) && !text.trim().is_empty() {
                gaps.push(text.trim().to_string());
            }
        }
    }
    gaps
}
