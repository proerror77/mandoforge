use chrono::Utc;
use serde::Deserialize;
use serde_json::Value;
use std::path::{Component, Path, PathBuf};

use crate::{
    EnterpriseEvidenceArchiveMetadata, EnterpriseProductCompletionLane,
    EnterpriseProductCompletionReadiness, project_file_path,
};

pub(crate) fn build_enterprise_product_completion_readiness() -> EnterpriseProductCompletionReadiness
{
    let contract_path = "docs/enterprise-product-completion-contract.md";
    let contract_present = project_file_path(contract_path)
        .map(|path| path.is_file())
        .unwrap_or(false);
    let mut lanes = build_enterprise_product_completion_lanes(contract_present);
    apply_customer_grade_evidence(&mut lanes);
    let evidence_archive =
        enterprise_product_completion_checklist(&enterprise_product_evidence_root())
            .and_then(|checklist| enterprise_completion_archive_metadata(&checklist));
    let lane_count = lanes.len();
    let ready_lane_count = lanes
        .iter()
        .filter(|lane| lane.status == "ready" && lane.current_evidence_class == "customer_grade")
        .count();
    let pilot_ready_lane_count = lanes
        .iter()
        .filter(|lane| lane.status == "pilot_ready")
        .count();
    let blocked_lane_count = lanes.iter().filter(|lane| lane.status == "blocked").count();
    let completion_blocked = !contract_present || ready_lane_count != lane_count;
    let status = if completion_blocked {
        "blocked"
    } else {
        "enterprise_product_complete"
    }
    .to_string();
    let mut next_actions = if completion_blocked {
        vec![
            "close Remote Computer multi-node distributed state evidence".to_string(),
            "promote live connectors from generic approval-gated calls to platform-specific production semantics".to_string(),
            "promote Context OS primitives into a versioned Ontology Engine release workflow".to_string(),
            "add enterprise identity, audit export, data-retention, and operations evidence".to_string(),
        ]
    } else {
        vec![
            "keep the customer-grade enterprise completion evidence archive fresh and replayable"
                .to_string(),
        ]
    };
    if !contract_present {
        next_actions.insert(
            0,
            "restore docs/enterprise-product-completion-contract.md".to_string(),
        );
    }
    let message = if completion_blocked {
        format!(
            "Enterprise product completion is blocked: {ready_lane_count}/{lane_count} lanes are customer-grade ready, {pilot_ready_lane_count} are pilot-ready, and {blocked_lane_count} remain blocked"
        )
    } else {
        "Enterprise product completion has customer-grade evidence for every required lane"
            .to_string()
    };

    EnterpriseProductCompletionReadiness {
        generated_at: Utc::now(),
        status,
        objective: "Full Enterprise Product Completion".to_string(),
        contract_path: contract_path.to_string(),
        contract_present,
        required_evidence_class: "customer_grade".to_string(),
        lane_count,
        ready_lane_count,
        pilot_ready_lane_count,
        blocked_lane_count,
        completion_blocked,
        evidence_archive,
        lanes,
        next_actions,
        message,
    }
}

fn build_enterprise_product_completion_lanes(
    contract_present: bool,
) -> Vec<EnterpriseProductCompletionLane> {
    let production_deployment_safety_static_ready = production_deployment_safety_static_ready();
    let production_deployment_safety_blockers = if production_deployment_safety_static_ready {
        vec![
            "customer-grade external secret delivery evidence is missing; static deployment safety wiring alone is not production-ready",
        ]
    } else {
        vec![
            "production launch preflight, default Secret exclusion, secret delivery contract, production runtime config, API workspace PVC, or deployment safety verifier evidence is missing",
        ]
    };
    let production_deployment_safety_next_actions = if production_deployment_safety_static_ready {
        vec![
            "run ./scripts/production-launch-preflight.sh against the production API before launch",
            "archive customer-grade secret-manager, ExternalSecret, SealedSecret, or equivalent delivery evidence before marking this lane ready",
        ]
    } else {
        vec![
            "remove example Secrets from default deployment paths",
            "restore deploy/k8s/secret-delivery-contract.yaml",
            "replace API workspace emptyDir with a PVC or object-storage-backed workspace",
            "restore ./scripts/production-launch-preflight.sh",
        ]
    };

    struct EnterpriseLaneSpec<'a> {
        id: &'a str,
        title: &'a str,
        status: &'a str,
        current_evidence_class: &'a str,
        current_boundary: &'a str,
        production_target: &'a str,
        readiness_endpoints: Vec<&'a str>,
        evidence_scripts: Vec<&'a str>,
        required_evidence: Vec<&'a str>,
        blockers: Vec<&'a str>,
        next_actions: Vec<&'a str>,
    }

    let mut specs = vec![
        EnterpriseLaneSpec {
            id: "production-deployment-safety",
            title: "Production deployment safety",
            status: "blocked",
            current_evidence_class: if production_deployment_safety_static_ready {
                "repo_controlled"
            } else {
                "incomplete"
            },
            current_boundary: if production_deployment_safety_static_ready {
                "Default K8s deployment excludes example Secrets, includes a no-secret delivery contract, avoids API workspace emptyDir, and has a launch preflight gate; customer-grade secret delivery evidence is still external"
            } else {
                "Default deployment safety evidence is incomplete"
            },
            production_target: "Fail-closed production deployment path with out-of-band secrets, durable workspace state, and a launch preflight gate",
            readiness_endpoints: vec!["/api/enterprise-product/readiness"],
            evidence_scripts: vec![
                "./scripts/production-launch-preflight.sh",
                "./scripts/production-deployment-safety-gate.sh",
            ],
            required_evidence: vec![
                "deploy/k8s/kustomization.yaml does not apply secret.example.yaml",
                "deploy/k8s/secret.example.yaml does not contain default database credentials",
                "deploy/k8s/secret-delivery-contract.yaml declares mandoforge-secrets as externally delivered production state",
                "deploy/k8s/configmap.yaml forces provider runtime production mode and Kubernetes Remote Computer transport",
                "deploy/k8s/configmap.yaml does not enable insecure dev auth, trusted caller headers, trusted tenant spoofing, host shell, or inline shell execution",
                "API workspace storage is backed by mandoforge-workspaces PVC instead of emptyDir",
                "production launch preflight, deployment safety gate, enterprise completion contract gate, and K8s evidence manifest verifier are executable",
                "customer-grade launch still requires external secret-manager, ExternalSecret, SealedSecret, or equivalent delivery evidence",
            ],
            blockers: production_deployment_safety_blockers,
            next_actions: production_deployment_safety_next_actions,
        },
        EnterpriseLaneSpec {
            id: "runtime-production",
            title: "Runtime production hardening",
            status: "pilot_ready",
            current_evidence_class: "production_like_pilot",
            current_boundary: "Agent OS core and Stage 2 runtime evidence are strong, but customer-grade restart, backup, restore, idempotency, and dead-letter drills are still required",
            production_target: "Customer-grade managed runtime with durable store, queue, replay, backup/restore, restart/resume, and support ownership",
            readiness_endpoints: vec![
                "/api/stage2/readiness",
                "/api/execution-jobs/worker-readiness",
                "/api/session-loop-jobs",
            ],
            evidence_scripts: vec![
                "./scripts/runtime-production-readiness-gate.sh",
                "./scripts/agent-os-core-evidence-gate.sh",
                "./scripts/managed-session-runtime-evidence-gate.sh",
                "./scripts/worker-evidence-gate.sh",
                "./scripts/provider-governance-evidence-gate.sh",
            ],
            required_evidence: vec![
                "production provider runtime forbids mock providers and env/mock fallback in session loop execution",
                "backup and restore preserve runtime action records",
                "session-loop and execution jobs recover after API, worker, and queue restart",
                "external side effects have idempotency, dead-letter, and manual replay evidence",
                "runtime-production-recovery-evidence.json binds backup/restore, dead-letter replay, and idempotency drills to a production runtime target",
            ],
            blockers: vec![
                "customer-grade runtime recovery gate exists, but backup/restore, idempotency, dead-letter, and manual replay evidence has not yet been archived as ready",
            ],
            next_actions: vec![
                "run ./scripts/runtime-production-readiness-gate.sh with production runtime recovery evidence",
                "archive restart/resume, backup/restore, idempotency, and dead-letter repair evidence",
            ],
        },
        EnterpriseLaneSpec {
            id: "remote-computer-multinode",
            title: "Remote Computer multi-node production state",
            status: "blocked",
            current_evidence_class: "production_like_pilot",
            current_boundary: "Whiskey single-node k3s validates the pilot path but not multi-node distributed state",
            production_target: "At least two schedulable nodes with distributed RWX state, lock-aware sync, sidecar recovery, NetworkPolicy, and tenant isolation",
            readiness_endpoints: vec![
                "/api/remote-computers/readiness",
                "/api/remote-computers/runner/readiness",
                "/api/remote-computers/production-path",
            ],
            evidence_scripts: vec![
                "./scripts/remote-computer-production-state-gate.sh",
                "./scripts/remote-computer-evidence-gate.sh",
                "./scripts/worker-remote-computer-evidence-gate.sh",
                "./scripts/whiskey-remote-computer-k3s-verify.sh",
            ],
            required_evidence: vec![
                "distributed RWX storage such as JuiceFS, CephFS, Longhorn RWX, or equivalent",
                "workspace, notes, memory, skills, and artifacts use lock-aware sync",
                "sidecar heartbeat and replacement recovery work across nodes",
                "Remote Computer standalone evidence and worker/Remote Computer combined evidence bind to the same production cluster, state claim, and distributed backend",
                "session Pod lifecycle evidence covers create, Running, approved exec, heartbeat, lease release, Pod deletion, and orphan sweep",
            ],
            blockers: vec![
                "Remote Computer production state gate exists, but multi-node RWX state, session Pod lifecycle, sidecar recovery, and combined worker binding evidence has not yet been archived as ready",
            ],
            next_actions: vec![
                "run ./scripts/remote-computer-production-state-gate.sh with production Remote Computer evidence",
                "archive multi-node Remote Computer state sync, session Pod lifecycle, sidecar recovery, and worker/Remote Computer combined evidence",
            ],
        },
        EnterpriseLaneSpec {
            id: "live-connector-production",
            title: "Live connector production semantics",
            status: "blocked",
            current_evidence_class: "repo_controlled",
            current_boundary: "Ecommerce, GitHub/SWE, Lark, and Feishu connector inventory exists, but platform-specific production semantics remain per-connector work",
            production_target: "Platform-specific live connectors with OAuth/token lifecycle, rate limits, idempotency, reconciliation, webhook ingestion, and compensation policy",
            readiness_endpoints: vec![
                "/api/native-connectors/production-readiness",
                "/api/workflow-packs/installations",
                "/api/stage2/readiness",
            ],
            evidence_scripts: vec![
                "./scripts/native-connector-production-readiness-gate.sh",
                "./scripts/live-connector-production-semantics-gate.sh",
                "./scripts/verify-ecommerce-tmall-context-os.sh",
                "./scripts/workflow-pack-evidence-gate.sh",
            ],
            required_evidence: vec![
                "sandbox/live separation and token lifecycle for each connector",
                "idempotent external writes plus reconciliation against external platform state",
                "platform-specific error taxonomy, rate-limit handling, and compensation policy",
                "immutable deployment evidence archive for every promoted live connector",
                "per-connector live production semantics evidence is archived under live-connector-production-semantics/<connector-id>/summary.json",
                "GitHub/SWE connector production semantics evidence is required alongside ecommerce and Lark/Feishu evidence",
            ],
            blockers: vec![
                "live connector production semantics gate exists, but per-connector customer-grade semantics evidence has not yet been archived as ready",
                "Tmall/Taobao, Xiaohongshu, TikTok Shop, Amazon SP-API, GitHub/SWE, and Lark/Feishu each need promoted production contracts",
            ],
            next_actions: vec![
                "run ./scripts/live-connector-production-semantics-gate.sh with promoted connector evidence",
                "archive token lifecycle, reconciliation, webhook, compensation, secret-redaction, and deployment evidence for each promoted connector",
                "extend the same evidence contract to GitHub/SWE and Lark/Feishu enterprise connectors",
            ],
        },
        EnterpriseLaneSpec {
            id: "ontology-engine",
            title: "Versioned Ontology Engine",
            status: "blocked",
            current_evidence_class: "repo_controlled",
            current_boundary: "Context OS primitives and Ontology Builder proposal flow exist; ontology-ready is not a full ontology engine",
            production_target: "Versioned core and domain ontology registry with migrations, relation constraints, conflict handling, approvals, and runtime enforcement",
            readiness_endpoints: vec![
                "/api/ontology/engine-readiness",
                "/api/ontology/registry",
                "/api/semantic-graph",
                "/api/semantic-workbench",
            ],
            evidence_scripts: vec![
                "./scripts/ontology-engine-readiness-gate.sh",
                "./scripts/ontology-engine-production-gate.sh",
                "./scripts/ontology-release-workflow-trigger-gate.sh",
                "./scripts/verify-ecommerce-tmall-context-os.sh",
            ],
            required_evidence: vec![
                "core and domain ontology versions can be promoted, rolled back, and migrated",
                "promoted ontology releases trigger matching workflow runs with durable audit evidence",
                "relation constraints are enforced before policy decisions rely on semantic links",
                "approved ontology changes create durable audit and context-packet evidence",
            ],
            blockers: vec![
                "ontology release workflow trigger gate exists, but customer-grade trigger evidence has not yet been archived as ready",
                "ontology engine production gate exists, but customer-grade migration, relation-constraint, conflict, trust, and context-packet evidence has not yet been archived as ready",
            ],
            next_actions: vec![
                "run ./scripts/ontology-engine-production-gate.sh with production ontology engine evidence",
                "run ./scripts/ontology-release-workflow-trigger-gate.sh against a production target and archive the trigger evidence",
            ],
        },
        EnterpriseLaneSpec {
            id: "workflowpack-enterprise-lifecycle",
            title: "WorkflowPack enterprise lifecycle",
            status: "pilot_ready",
            current_evidence_class: "production_like_pilot",
            current_boundary: "Install, stage, release, rollback, archive, onboarding, and connector quality are implemented, but customer-grade pack operations need canary, compatibility, and tenant-specific promotion evidence",
            production_target: "Operational pack lifecycle with tenant onboarding, real connector quality, eval regression, canary, rollback, version compatibility, and customer overrides",
            readiness_endpoints: vec![
                "/api/workflow-packs/installations",
                "/api/workflow-runs",
                "/api/memory-governance/summary",
            ],
            evidence_scripts: vec![
                "./scripts/verify-workflow-pack-manifest.sh",
                "./scripts/workflow-pack-evidence-gate.sh",
                "./scripts/managed-workflow-runtime-evidence-gate.sh",
                "./scripts/workflowpack-enterprise-lifecycle-gate.sh",
            ],
            required_evidence: vec![
                "tenant onboarding profiles are versioned and completeness-gated",
                "real connector account quality checks gate pack release",
                "pack updates preserve compatibility and rollback evidence",
                "managed workflow runtime proves scheduler retry, fan-in completion, and expired step lease reclaim",
            ],
            blockers: vec![
                "WorkflowPack enterprise lifecycle gate exists, but customer-grade canary, compatibility matrix, tenant override policy, and workflow recovery evidence has not yet been archived as ready",
            ],
            next_actions: vec![
                "run ./scripts/workflowpack-enterprise-lifecycle-gate.sh with production WorkflowPack lifecycle evidence",
                "run ./scripts/managed-workflow-runtime-evidence-gate.sh against a production target",
                "gate customer-specific overrides without weakening manifest contract",
            ],
        },
        EnterpriseLaneSpec {
            id: "enterprise-security-admin",
            title: "Enterprise security and administration",
            status: "blocked",
            current_evidence_class: "production_like_pilot",
            current_boundary: "RBAC, tenant routing/RLS evidence, approval notifications, and Vault/KMS pilot surfaces exist; SSO/SCIM/SIEM/data-governance completion is not done",
            production_target: "SSO/OIDC/SAML, SCIM, RBAC/ABAC, tenant RLS, production KMS, SIEM export, retention, legal hold, deletion/export, DLP, and break-glass audit",
            readiness_endpoints: vec![
                "/api/enterprise-security/admin-readiness",
                "/api/tenant-isolation/readiness",
                "/api/vault/readiness",
                "/api/approvals/notification-routing/summary",
            ],
            evidence_scripts: vec![
                "./scripts/enterprise-security-admin-readiness-gate.sh",
                "./scripts/enterprise-security-production-controls-gate.sh",
                "./scripts/tenant-isolation-evidence-gate.sh",
                "./scripts/vault-evidence-gate.sh",
                "./scripts/approval-notification-evidence-gate.sh",
            ],
            required_evidence: vec![
                "SSO and directory provisioning are production configured",
                "audit export supports SIEM ingestion",
                "data retention, legal hold, deletion/export, PII redaction, and DLP policies are tested",
                "security production controls evidence proves SSO/SCIM, tenant RLS/ABAC, KMS, break-glass, SIEM, data governance, and incident operations",
            ],
            blockers: vec![
                "enterprise security production controls gate exists, but customer-grade security controls evidence has not yet been archived as ready",
                "SSO/SCIM/SIEM/data-governance lanes are not complete",
            ],
            next_actions: vec![
                "run ./scripts/enterprise-security-production-controls-gate.sh with production security controls evidence",
                "archive identity provisioning, SIEM delivery, data governance, KMS, tenant isolation, break-glass, and incident-ops evidence",
            ],
        },
        EnterpriseLaneSpec {
            id: "observability-ops",
            title: "Observability and operations",
            status: "blocked",
            current_evidence_class: "production_like_pilot",
            current_boundary: "OTel, usage, cost, finance, and controller evidence exist for pilot targets; enterprise support SLOs and repair runbooks are not complete",
            production_target: "Customer operations surface with metrics, traces, logs, audit correlation, alerts, SLOs, incident timeline, and manual repair workflows",
            readiness_endpoints: vec![
                "/api/observability",
                "/api/observability/collector-readiness",
                "/api/usage/finance-operations/summary",
            ],
            evidence_scripts: vec![
                "./scripts/observability-collector-evidence-gate.sh",
                "./scripts/observability-ops-production-gate.sh",
                "./scripts/finance-evidence-gate.sh",
            ],
            required_evidence: vec![
                "alerts exist for failed jobs, stale leases, delivery failures, connector degradation, provider degradation, budget breach, and queue backlog",
                "deployment, migration, pack, ontology, and connector versions are visible",
                "incident timeline and manual repair actions are audited",
                "observability ops production evidence proves SLOs, runbooks, alert delivery, audit correlation, manual repair, and immutable incident evidence archives",
            ],
            blockers: vec![
                "observability ops production gate exists, but customer-grade SLO, alert, incident, repair, and runbook evidence has not yet been archived as ready",
            ],
            next_actions: vec![
                "run ./scripts/observability-ops-production-gate.sh with production operations evidence",
                "archive SLO, alert coverage, incident timeline, manual repair, version visibility, and runbook rehearsal evidence",
            ],
        },
        EnterpriseLaneSpec {
            id: "product-surfaces",
            title: "Enterprise product surfaces",
            status: "blocked",
            current_evidence_class: "repo_controlled",
            current_boundary: "The UI reads many live APIs, but full Admin, Operator, Builder, and Ops consoles are not enterprise-complete",
            production_target: "Live API-backed Admin, Operator, Builder, and Ops consoles with no fake completion state",
            readiness_endpoints: vec![
                "/api/enterprise-product/readiness",
                "/api/stage2/readiness",
                "/api/semantic-workbench",
            ],
            evidence_scripts: vec![
                "./scripts/verify-static-ui-assets.sh",
                "./scripts/verify-ui-api-truth-gate.mjs",
                "./scripts/verify-static-ui-actionbook.sh",
                "./scripts/product-surfaces-production-gate.sh",
            ],
            required_evidence: vec![
                "Admin Console covers tenants, teams, agents, runtime profiles, providers, policies, approvals, connectors, budgets, and release state",
                "Operator Console covers blocked work, approvals, replay, artifacts, jobs, and manual repair",
                "Builder Console covers Workflow Packs, Ontology Builder, connector mapping, eval gates, and release gates",
                "Ops Console covers health, workers, queues, costs, alerts, deployments, and incident evidence",
                "product-surfaces/summary.json proves live API readback, authorization boundaries, no fake completion state, and immutable evidence archives for Admin, Operator, Builder, and Ops consoles",
            ],
            blockers: vec![
                "product surfaces production gate exists, but customer-grade Admin, Operator, Builder, and Ops console evidence has not yet been archived as ready",
            ],
            next_actions: vec![
                "run ./scripts/product-surfaces-production-gate.sh with production product surface evidence",
                "archive Admin, Operator, Builder, and Ops console live API readback, authorization, no-fake-completion, and immutable evidence",
            ],
        },
    ];

    if !contract_present {
        specs.insert(
            0,
            EnterpriseLaneSpec {
                id: "enterprise-contract",
                title: "Enterprise completion contract",
                status: "blocked",
                current_evidence_class: "none",
                current_boundary: "The enterprise product completion contract document is missing",
                production_target: "Versioned enterprise completion contract in repo documentation",
                readiness_endpoints: vec!["/api/enterprise-product/readiness"],
                evidence_scripts: vec!["./scripts/enterprise-product-completion-contract-gate.sh"],
                required_evidence: vec![
                    "docs/enterprise-product-completion-contract.md exists",
                    "all required enterprise lanes are defined",
                ],
                blockers: vec!["enterprise product completion contract is missing"],
                next_actions: vec!["restore docs/enterprise-product-completion-contract.md"],
            },
        );
    }

    specs
        .into_iter()
        .map(|spec| EnterpriseProductCompletionLane {
            id: spec.id.to_string(),
            title: spec.title.to_string(),
            status: spec.status.to_string(),
            current_evidence_class: spec.current_evidence_class.to_string(),
            required_evidence_class: "customer_grade".to_string(),
            current_boundary: spec.current_boundary.to_string(),
            production_target: spec.production_target.to_string(),
            readiness_endpoints: spec
                .readiness_endpoints
                .into_iter()
                .map(str::to_string)
                .collect(),
            evidence_scripts: spec
                .evidence_scripts
                .into_iter()
                .map(str::to_string)
                .collect(),
            required_evidence: spec
                .required_evidence
                .into_iter()
                .map(str::to_string)
                .collect(),
            blockers: spec.blockers.into_iter().map(str::to_string).collect(),
            next_actions: spec.next_actions.into_iter().map(str::to_string).collect(),
        })
        .collect()
}

fn apply_customer_grade_evidence(lanes: &mut [EnterpriseProductCompletionLane]) {
    let evidence_root = enterprise_product_evidence_root();
    let Some(checklist) = enterprise_product_completion_checklist(&evidence_root) else {
        return;
    };
    for lane in lanes {
        if !enterprise_lane_has_customer_grade_evidence(&evidence_root, &checklist, &lane.id) {
            continue;
        }
        lane.status = "ready".to_string();
        lane.current_evidence_class = "customer_grade".to_string();
        lane.current_boundary = format!(
            "Customer-grade evidence summaries under {} prove this lane is ready",
            evidence_root.display()
        );
        lane.blockers.clear();
        lane.next_actions =
            vec!["keep the customer-grade evidence fresh, immutable, and replayable".to_string()];
    }
}

fn enterprise_product_evidence_root() -> PathBuf {
    std::env::var("MANDOFORGE_ENTERPRISE_PRODUCT_EVIDENCE_DIR")
        .or_else(|_| std::env::var("STAGE2_EVIDENCE_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| project_root_path().join(".mandoforge/stage2-production-evidence"))
}

fn project_root_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn enterprise_lane_has_customer_grade_evidence(
    root: &Path,
    checklist: &Value,
    lane_id: &str,
) -> bool {
    match lane_id {
        "production-deployment-safety" => {
            checklist_evidence_summary_ready(
                root,
                checklist,
                "production-deployment-safety",
                "production-deployment-safety-gate",
            ) && stage2_production_preflight_summary_ready(root)
        }
        "runtime-production" => checklist_evidence_summary_ready(
            root,
            checklist,
            "runtime-production",
            "runtime-production-readiness-gate",
        ),
        "remote-computer-multinode" => checklist_evidence_summary_ready(
            root,
            checklist,
            "remote-computer-multinode",
            "remote-computer-production-state-gate",
        ),
        "live-connector-production" => checklist_evidence_summary_ready(
            root,
            checklist,
            "live-connector-production",
            "live-connector-production-semantics-gate",
        ),
        "ontology-engine" => {
            checklist_evidence_summary_ready(
                root,
                checklist,
                "ontology-engine",
                "ontology-engine-production-gate",
            ) && checklist_evidence_summary_ready(
                root,
                checklist,
                "ontology-release-workflow-trigger",
                "ontology-release-workflow-trigger-gate",
            )
        }
        "workflowpack-enterprise-lifecycle" => checklist_evidence_summary_ready(
            root,
            checklist,
            "workflowpack-enterprise-lifecycle",
            "workflowpack-enterprise-lifecycle-gate",
        ),
        "enterprise-security-admin" => checklist_evidence_summary_ready(
            root,
            checklist,
            "enterprise-security-admin",
            "enterprise-security-production-controls-gate",
        ),
        "observability-ops" => checklist_evidence_summary_ready(
            root,
            checklist,
            "observability-ops",
            "observability-ops-production-gate",
        ),
        "product-surfaces" => checklist_evidence_summary_ready(
            root,
            checklist,
            "product-surfaces",
            "product-surfaces-production-gate",
        ),
        _ => false,
    }
}

fn enterprise_product_completion_checklist(root: &Path) -> Option<Value> {
    let path = root.join("enterprise-product-completion-contract-gate/checklist.json");
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return None,
    };
    let value = match serde_json::from_str::<Value>(&content) {
        Ok(value) => value,
        Err(_) => return None,
    };
    if value.get("source").and_then(Value::as_str)
        != Some("enterprise-product-completion-contract-gate")
        || value.get("required_evidence_class").and_then(Value::as_str) != Some("customer_grade")
        || value
            .get("enterprise_product_status")
            .and_then(Value::as_str)
            != Some("enterprise_product_complete")
        || value.get("completion_blocked").and_then(Value::as_bool) != Some(false)
        || enterprise_completion_archive_metadata(&value).is_none()
    {
        return None;
    }

    let expected_required_lanes = [
        "production-deployment-safety",
        "runtime-production",
        "remote-computer-multinode",
        "live-connector-production",
        "ontology-engine",
        "workflowpack-enterprise-lifecycle",
        "enterprise-security-admin",
        "observability-ops",
        "product-surfaces",
    ];
    if json_array_len(&value, "required_lanes") != Some(expected_required_lanes.len())
        || json_array_len(&value, "ready_lanes") != Some(expected_required_lanes.len())
        || !json_string_array_contains_all(&value, "required_lanes", &expected_required_lanes)
        || !json_string_array_contains_all(&value, "ready_lanes", &expected_required_lanes)
        || !json_array_empty(&value, "blocked_lanes")
    {
        return None;
    }

    let expected_lane_results = [
        (
            "production-deployment-safety",
            "production-deployment-safety-gate",
        ),
        ("runtime-production", "runtime-production-readiness-gate"),
        (
            "remote-computer-multinode",
            "remote-computer-production-state-gate",
        ),
        (
            "live-connector-production",
            "live-connector-production-semantics-gate",
        ),
        ("ontology-engine", "ontology-engine-production-gate"),
        (
            "ontology-release-workflow-trigger",
            "ontology-release-workflow-trigger-gate",
        ),
        (
            "workflowpack-enterprise-lifecycle",
            "workflowpack-enterprise-lifecycle-gate",
        ),
        (
            "enterprise-security-admin",
            "enterprise-security-production-controls-gate",
        ),
        ("observability-ops", "observability-ops-production-gate"),
        ("product-surfaces", "product-surfaces-production-gate"),
    ];
    if json_array_len(&value, "lane_results") != Some(expected_lane_results.len()) {
        return None;
    }
    if expected_lane_results
        .iter()
        .all(|(lane, source)| checklist_lane_result_ready(&value, lane, source))
    {
        Some(value)
    } else {
        None
    }
}

fn enterprise_completion_archive_metadata(
    value: &Value,
) -> Option<EnterpriseEvidenceArchiveMetadata> {
    let support_owner = value.get("support_owner").and_then(Value::as_str)?;
    let uri = value
        .pointer("/evidence_archive/uri")
        .and_then(Value::as_str)?;
    let digest = value
        .pointer("/evidence_archive/digest")
        .and_then(Value::as_str)?;
    let retention_policy = value
        .pointer("/evidence_archive/retention_policy")
        .and_then(Value::as_str)?;
    let immutable = value
        .pointer("/evidence_archive/immutable")
        .and_then(Value::as_bool)?;
    if !immutable
        || value.get("archive_metadata_ready").and_then(Value::as_bool) != Some(true)
        || !looks_production_identity(support_owner)
        || !looks_production_archive_uri(uri)
        || !looks_evidence_digest(digest)
        || retention_policy.trim().is_empty()
    {
        return None;
    }

    Some(EnterpriseEvidenceArchiveMetadata {
        support_owner: support_owner.to_string(),
        uri: uri.to_string(),
        digest: digest.to_string(),
        retention_policy: retention_policy.to_string(),
        immutable,
    })
}

fn looks_production_identity(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    !value.trim().is_empty()
        && !contains_reserved_target_token(&value)
        && !value.contains("127.0.0.1")
        && !value.contains("[::1]")
}

fn looks_production_archive_uri(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    (value.starts_with("s3://")
        || value.starts_with("gs://")
        || value.starts_with("az://")
        || value.starts_with("https://"))
        && !value.contains("example.com")
        && !contains_reserved_target_token(&value)
        && !value.contains("127.0.0.1")
        && !value.contains("[::1]")
}

fn looks_evidence_digest(value: &str) -> bool {
    let digest = value.strip_prefix("sha256:").unwrap_or(value);
    digest.len() == 64 && digest.chars().all(|c| c.is_ascii_hexdigit())
}

fn contains_reserved_target_token(value: &str) -> bool {
    value.split(['.', '/', ':', '_', '-']).any(|part| {
        matches!(
            part,
            "whiskey"
                | "pilot"
                | "mock"
                | "example"
                | "sample"
                | "demo"
                | "local"
                | "localhost"
                | "sandbox"
                | "sandbox-only"
        )
    })
}

fn checklist_lane_result_ready(value: &Value, lane: &str, expected_source: &str) -> bool {
    value
        .get("lane_results")
        .and_then(Value::as_array)
        .is_some_and(|results| {
            results.iter().any(|result| {
                result.get("lane").and_then(Value::as_str) == Some(lane)
                    && result.get("expected_source").and_then(Value::as_str)
                        == Some(expected_source)
                    && result.get("status").and_then(Value::as_str) == Some("ready")
                    && result
                        .get("summary_path")
                        .and_then(Value::as_str)
                        .is_some_and(|path| !path.is_empty())
                    && result.get("issue").is_none_or(Value::is_null)
            })
        })
}

fn checklist_evidence_summary_ready(
    root: &Path,
    checklist: &Value,
    lane: &str,
    expected_source: &str,
) -> bool {
    checklist
        .get("lane_results")
        .and_then(Value::as_array)
        .and_then(|results| {
            results.iter().find_map(|result| {
                if result.get("lane").and_then(Value::as_str) == Some(lane)
                    && result.get("expected_source").and_then(Value::as_str)
                        == Some(expected_source)
                    && result.get("status").and_then(Value::as_str) == Some("ready")
                    && result.get("issue").is_none_or(Value::is_null)
                {
                    result.get("summary_path").and_then(Value::as_str)
                } else {
                    None
                }
            })
        })
        .and_then(checked_relative_path)
        .is_some_and(|relative_path| evidence_summary_ready(root, &relative_path, expected_source))
}

fn checked_relative_path(path: &str) -> Option<String> {
    let parsed = Path::new(path);
    if parsed.is_absolute() {
        return None;
    }
    let mut clean = PathBuf::new();
    for component in parsed.components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if clean.as_os_str().is_empty() {
        None
    } else {
        Some(clean.to_string_lossy().into_owned())
    }
}

fn json_string_array_contains_all(value: &Value, key: &str, expected_items: &[&str]) -> bool {
    value
        .get(key)
        .and_then(Value::as_array)
        .is_some_and(|items| {
            expected_items
                .iter()
                .all(|expected| items.iter().any(|item| item.as_str() == Some(*expected)))
        })
}

fn json_array_empty(value: &Value, key: &str) -> bool {
    value
        .get(key)
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty)
}

fn json_array_len(value: &Value, key: &str) -> Option<usize> {
    value.get(key).and_then(Value::as_array).map(Vec::len)
}

fn evidence_summary_ready(root: &Path, relative_path: &str, expected_source: &str) -> bool {
    let path = root.join(relative_path);
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return false,
    };
    let value = match serde_json::from_str::<Value>(&content) {
        Ok(value) => value,
        Err(_) => return false,
    };
    let status_ready = value
        .get("status")
        .and_then(Value::as_str)
        .is_some_and(|status| matches!(status, "ready" | "validated" | "completed" | "passed"));
    let source_matches = value
        .get("source")
        .and_then(Value::as_str)
        .is_some_and(|source| source == expected_source);
    let evidence_class_matches = value
        .get("required_evidence_class")
        .and_then(Value::as_str)
        .is_some_and(|evidence_class| evidence_class == "customer_grade");
    let blocked_count = value
        .get("blocked_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    source_matches
        && evidence_class_matches
        && status_ready
        && blocked_count == 0
        && gate_summary_has_required_shape(&value, expected_source)
}

fn stage2_production_preflight_summary_ready(root: &Path) -> bool {
    let path = root.join("stage2-production-evidence-preflight.json");
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return false,
    };
    let value = match serde_json::from_str::<Value>(&content) {
        Ok(value) => value,
        Err(_) => return false,
    };
    value.get("source").and_then(Value::as_str) == Some("stage2-production-evidence-preflight")
        && value.get("status").and_then(Value::as_str) == Some("passed")
        && value.get("fail_count").and_then(Value::as_u64) == Some(0)
        && value
            .get("pass_count")
            .and_then(Value::as_u64)
            .is_some_and(|count| count > 0)
        && value
            .get("checks")
            .and_then(Value::as_array)
            .is_some_and(|checks| {
                let pass_count = value
                    .get("pass_count")
                    .and_then(Value::as_u64)
                    .unwrap_or_default() as usize;
                checks.len() == pass_count
                    && checks.iter().all(|check| {
                        check.get("status").and_then(Value::as_str) == Some("passed")
                            && check
                                .get("scope")
                                .and_then(Value::as_str)
                                .is_some_and(|scope| !scope.is_empty())
                            && check
                                .get("detail")
                                .and_then(Value::as_str)
                                .is_some_and(|detail| !detail.is_empty())
                    })
            })
}

fn gate_summary_has_required_shape(value: &Value, expected_source: &str) -> bool {
    match expected_source {
        "production-deployment-safety-gate" => {
            json_string_present(value, "production_deployment_safety_evidence_file")
        }
        "runtime-production-readiness-gate" => {
            json_string_present(value, "runtime_recovery_evidence_file")
                && value
                    .get("runtime_recovery_status")
                    .and_then(Value::as_str)
                    .is_some_and(|status| matches!(status, "ready" | "validated" | "completed"))
        }
        "remote-computer-production-state-gate" => {
            json_string_present(value, "remote_evidence_dir")
                && json_string_present(value, "combined_evidence_dir")
                && json_string_present(value, "lifecycle_evidence_file")
        }
        "live-connector-production-semantics-gate" => {
            json_string_present(value, "source_evidence_dir")
                && value
                    .get("connector_count")
                    .and_then(Value::as_u64)
                    .is_some_and(|count| count > 0)
        }
        "ontology-engine-production-gate" => {
            json_string_present(value, "ontology_engine_evidence_file")
        }
        "ontology-release-workflow-trigger-gate" => {
            json_string_present(value, "domain_scope")
                && json_string_present(value, "workflow_definition_id")
                && json_string_present(value, "workflow_run_id")
                && json_string_present(value, "ontology_release_id")
                && json_string_present(value, "support_owner")
                && value.pointer("/target/environment").and_then(Value::as_str)
                    == Some("production")
                && value
                    .pointer("/evidence_archive/immutable")
                    .and_then(Value::as_bool)
                    == Some(true)
                && value
                    .pointer("/checks/workflow_run_queued")
                    .and_then(Value::as_bool)
                    == Some(true)
        }
        "workflowpack-enterprise-lifecycle-gate" => {
            json_string_present(value, "workflowpack_enterprise_lifecycle_evidence_file")
        }
        "enterprise-security-production-controls-gate" => {
            json_string_present(value, "controls_evidence_file")
        }
        "observability-ops-production-gate" => json_string_present(value, "ops_evidence_file"),
        "product-surfaces-production-gate" => {
            json_string_present(value, "product_surfaces_evidence_file")
        }
        _ => false,
    }
}

fn json_string_present(value: &Value, key: &str) -> bool {
    value
        .get(key)
        .and_then(Value::as_str)
        .is_some_and(|text| !text.is_empty())
}

fn production_deployment_safety_static_ready() -> bool {
    let kustomization = project_file_content("deploy/k8s/kustomization.yaml");
    let secret_example = project_file_content("deploy/k8s/secret.example.yaml");
    let secret_delivery_contract = project_file_content("deploy/k8s/secret-delivery-contract.yaml");
    let agent_sandbox_controller_contract =
        project_file_content("deploy/k8s/agent-sandbox-controller-contract.yaml");
    let agent_sandbox_runtime = project_file_content("deploy/k8s/agent-sandbox-runtime.yaml");
    let agent_sandbox_network_policy =
        project_file_content("deploy/k8s/agent-sandbox-egress-networkpolicy.yaml");
    let api_agent_sandbox_rbac = project_file_content("deploy/k8s/api-agent-sandbox-rbac.yaml");
    let configmap = project_file_content("deploy/k8s/configmap.yaml");
    let api_manifest = project_file_content("deploy/k8s/api.yaml");
    let worker_manifest = project_file_content("deploy/k8s/worker.yaml");
    let isolated_worker_manifest = project_file_content("deploy/k8s/worker-isolated-pool.yaml");
    let isolated_worker_network_policy =
        project_file_content("deploy/k8s/worker-isolated-pool-networkpolicy.yaml");
    let preflight_present = project_file_is_executable("scripts/production-launch-preflight.sh");
    let deployment_safety_gate_present =
        project_file_is_executable("scripts/production-deployment-safety-gate.sh");
    let contract_gate_present =
        project_file_is_executable("scripts/enterprise-product-completion-contract-gate.sh");
    let k8s_manifest_verifier_present =
        project_file_is_executable("scripts/verify-stage2-evidence-k8s-manifests.sh");

    kustomization
        .as_deref()
        .is_some_and(|content| !content.contains("secret.example.yaml"))
        && secret_example.as_deref().is_some_and(|content| {
            !content.contains("POSTGRES_PASSWORD: \"mandoforge\"")
                && !content.contains("postgres://mandoforge:mandoforge@")
        })
        && secret_delivery_contract.as_deref().is_some_and(|content| {
            content.contains("MANDOFORGE_SECRET_DELIVERY_REQUIRED: \"true\"")
                && content.contains("MANDOFORGE_SECRET_NAME: \"mandoforge-secrets\"")
                && content.contains("MANDOFORGE_SECRET_MUST_NOT_BE_EXAMPLE: \"true\"")
        })
        && kustomization.as_deref().is_some_and(|content| {
            content.contains("agent-sandbox-controller-contract.yaml")
                && content.contains("api-serviceaccount.yaml")
                && content.contains("api-agent-sandbox-rbac.yaml")
                && content.contains("agent-sandbox-runtime.yaml")
                && content.contains("agent-sandbox-egress-networkpolicy.yaml")
                && !content.contains("agent-remote-computer.yaml")
        })
        && agent_sandbox_controller_contract
            .as_deref()
            .is_some_and(|content| {
                content.contains("MANDOFORGE_AGENT_SANDBOX_CONTROLLER_VERSION: \"v0.5.1\"")
                    && content.contains(
                        "MANDOFORGE_AGENT_SANDBOX_EXTENSIONS_API: \"extensions.agents.x-k8s.io/v1beta1\"",
                    )
                    && content.contains(
                        "MANDOFORGE_AGENT_SANDBOX_CORE_INSTALL_ASSET: \"manifest.yaml\"",
                    )
                    && content.contains(
                        "MANDOFORGE_AGENT_SANDBOX_CORE_INSTALL_SHA256: \"8cfdf0a878f66b91d2e7103e77859d1412d850ce3f5fe5c3fa134c36bd55504a\"",
                    )
                    && content.contains(
                        "MANDOFORGE_AGENT_SANDBOX_EXTENSIONS_INSTALL_ASSET: \"extensions.yaml\"",
                    )
                    && content.contains(
                        "MANDOFORGE_AGENT_SANDBOX_EXTENSIONS_INSTALL_SHA256: \"7c22b450e24ede3fddbcd5ae0ee7c78ea102d6c30635ff860cc486578a55932e\"",
                    )
                    && content.contains("MANDOFORGE_AGENT_SANDBOX_CONTROLLER_REQUIRED: \"true\"")
            })
        && agent_sandbox_runtime.as_deref().is_some_and(|content| {
            content.contains("kind: SandboxTemplate")
                && content.contains("kind: SandboxWarmPool")
                && content.contains("automountServiceAccountToken: false")
        })
        && agent_sandbox_network_policy
            .as_deref()
            .is_some_and(agent_sandbox_network_policy_is_restricted)
        && api_agent_sandbox_rbac
            .as_deref()
            .is_some_and(api_agent_sandbox_rbac_is_minimal)
        && configmap.as_deref().is_some_and(|content| {
            content.contains("MANDOFORGE_PROVIDER_RUNTIME_ENV: \"production\"")
                && content.contains("MANDOFORGE_REMOTE_COMPUTER_RUNNER: \"agent-sandbox\"")
                && content
                    .contains("MANDOFORGE_REMOTE_COMPUTER_EXECUTION_TRANSPORT: \"kubernetes\"")
                && content.contains(
                    "MANDOFORGE_REMOTE_COMPUTER_TEMPLATE_PATH: \"deploy/k8s/agent-sandbox-runtime.yaml\"",
                )
                && content.contains("MANDOFORGE_REMOTE_COMPUTER_EXECUTION_ENABLED: \"false\"")
                && content.contains("MANDOFORGE_REMOTE_COMPUTER_MUTATION_ENABLED: \"false\"")
                && content
                    .contains("MANDOFORGE_REMOTE_COMPUTER_LIVE_MUTATION_ENABLED: \"false\"")
                && !content.contains("MANDOFORGE_INSECURE_DEV_AUTH: \"true\"")
                && !content.contains("MANDOFORGE_INSECURE_DEV_AUTH: \"1\"")
                && !content.contains("MANDOFORGE_TRUST_X_MANDOFORGE_SUBJECT: \"true\"")
                && !content.contains("MANDOFORGE_TRUST_X_MANDOFORGE_SUBJECT: \"1\"")
                && !content.contains("TRUSTED_TENANT_ID:")
                && !content.contains("MANDOFORGE_ALLOW_HOST_SHELL_EXEC: \"true\"")
                && !content.contains("MANDOFORGE_ALLOW_HOST_SHELL_EXEC: \"1\"")
                && !content.contains("MANDOFORGE_ALLOW_INLINE_SHELL_EXEC: \"true\"")
                && !content.contains("MANDOFORGE_ALLOW_INLINE_SHELL_EXEC: \"1\"")
        })
        && api_manifest.as_deref().is_some_and(|content| {
            content.contains("claimName: mandoforge-workspaces")
                && content.contains("serviceAccountName: mandoforge-api")
                && content.contains("name: mandoforge-agent-sandbox-controller-contract")
                && content.contains("mountPath: /var/run/secrets/kubernetes.io/serviceaccount")
                && !content.contains("emptyDir: {}")
        })
        && worker_manifest.as_deref().is_some_and(|content| {
            workload_disables_service_account_token(content)
                && !content.contains("mountPath: /var/run/secrets/kubernetes.io/serviceaccount")
        })
        && isolated_worker_manifest.as_deref().is_some_and(|content| {
            workload_disables_service_account_token(content)
                && !content.contains("mountPath: /var/run/secrets/kubernetes.io/serviceaccount")
        })
        && isolated_worker_network_policy
            .as_deref()
            .is_some_and(|content| {
                !content.contains("app: agent-remote-computer")
                    && !content.contains("port: 8080")
            })
        && preflight_present
        && deployment_safety_gate_present
        && contract_gate_present
        && k8s_manifest_verifier_present
}

fn yaml_documents(content: &str) -> Option<Vec<Value>> {
    serde_yaml::Deserializer::from_str(content)
        .map(Value::deserialize)
        .collect::<Result<Vec<_>, _>>()
        .ok()
}

fn k8s_resource_named<'a>(documents: &'a [Value], kind: &str, name: &str) -> Option<&'a Value> {
    documents.iter().find(|document| {
        document.get("kind").and_then(Value::as_str) == Some(kind)
            && document
                .get("metadata")
                .and_then(|metadata| metadata.get("name"))
                .and_then(Value::as_str)
                == Some(name)
    })
}

fn string_array_equals(value: Option<&Value>, expected: &[&str]) -> bool {
    value.and_then(Value::as_array).is_some_and(|items| {
        items.len() == expected.len()
            && expected
                .iter()
                .all(|expected| items.iter().any(|item| item.as_str() == Some(expected)))
    })
}

fn network_policy_ports_equal(rule: &Value, expected: &[(&str, u64)]) -> bool {
    rule.get("ports")
        .and_then(Value::as_array)
        .is_some_and(|ports| {
            ports.len() == expected.len()
                && expected.iter().all(|(protocol, port)| {
                    ports.iter().any(|candidate| {
                        candidate.get("protocol").and_then(Value::as_str) == Some(*protocol)
                            && candidate.get("port").and_then(Value::as_u64) == Some(*port)
                    })
                })
        })
}

fn network_policy_dns_rule(rule: &Value) -> bool {
    let Some(destinations) = rule.get("to").and_then(Value::as_array) else {
        return false;
    };
    destinations.len() == 1
        && destinations[0]
            .get("namespaceSelector")
            .and_then(|selector| selector.get("matchLabels"))
            .and_then(|labels| labels.get("kubernetes.io/metadata.name"))
            .and_then(Value::as_str)
            == Some("kube-system")
        && destinations[0]
            .get("podSelector")
            .and_then(|selector| selector.get("matchLabels"))
            .and_then(|labels| labels.get("k8s-app"))
            .and_then(Value::as_str)
            == Some("kube-dns")
        && network_policy_ports_equal(rule, &[("UDP", 53), ("TCP", 53)])
}

fn network_policy_api_rule(rule: &Value) -> bool {
    let Some(destinations) = rule.get("to").and_then(Value::as_array) else {
        return false;
    };
    destinations.len() == 1
        && destinations[0]
            .get("podSelector")
            .and_then(|selector| selector.get("matchLabels"))
            .and_then(|labels| labels.get("app"))
            .and_then(Value::as_str)
            == Some("mandoforge-api")
        && network_policy_ports_equal(rule, &[("TCP", 8787)])
}

fn network_policy_external_https_rule(rule: &Value) -> bool {
    let Some(destinations) = rule.get("to").and_then(Value::as_array) else {
        return false;
    };
    let ipv4 = destinations.iter().find_map(|destination| {
        destination
            .get("ipBlock")
            .filter(|block| block.get("cidr").and_then(Value::as_str) == Some("0.0.0.0/0"))
    });
    let ipv6 = destinations.iter().find_map(|destination| {
        destination
            .get("ipBlock")
            .filter(|block| block.get("cidr").and_then(Value::as_str) == Some("::/0"))
    });
    destinations.len() == 2
        && ipv4.is_some_and(|block| {
            [
                "10.0.0.0/8",
                "127.0.0.0/8",
                "169.254.0.0/16",
                "172.16.0.0/12",
                "192.168.0.0/16",
            ]
            .iter()
            .all(|cidr| {
                block
                    .get("except")
                    .and_then(Value::as_array)
                    .is_some_and(|items| items.iter().any(|item| item.as_str() == Some(cidr)))
            })
        })
        && ipv6.is_some_and(|block| {
            ["::1/128", "fc00::/7", "fe80::/10"].iter().all(|cidr| {
                block
                    .get("except")
                    .and_then(Value::as_array)
                    .is_some_and(|items| items.iter().any(|item| item.as_str() == Some(cidr)))
            })
        })
        && network_policy_ports_equal(rule, &[("TCP", 443)])
}

fn agent_sandbox_network_policy_is_restricted(content: &str) -> bool {
    let Some(documents) = yaml_documents(content) else {
        return false;
    };
    let Some(policy) = k8s_resource_named(
        &documents,
        "NetworkPolicy",
        "mandoforge-agent-sandbox-egress",
    ) else {
        return false;
    };
    let Some(spec) = policy.get("spec") else {
        return false;
    };
    let labels = spec
        .get("podSelector")
        .and_then(|selector| selector.get("matchLabels"))
        .and_then(Value::as_object);
    let selector_matches = labels.is_some_and(|labels| {
        labels.len() == 2
            && labels.get("app").and_then(Value::as_str) == Some("mandoforge-agent-remote-computer")
            && labels
                .get("mandoforge.io/runtime-substrate")
                .and_then(Value::as_str)
                == Some("agent-sandbox")
    });
    let Some(egress) = spec.get("egress").and_then(Value::as_array) else {
        return false;
    };
    selector_matches
        && string_array_equals(spec.get("policyTypes"), &["Ingress", "Egress"])
        && spec
            .get("ingress")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
        && egress.len() == 3
        && egress.iter().any(network_policy_dns_rule)
        && egress.iter().any(network_policy_api_rule)
        && egress.iter().any(network_policy_external_https_rule)
}

fn rbac_rule_is_exact(rule: &Value, api_group: &str, resource: &str, verbs: &[&str]) -> bool {
    string_array_equals(rule.get("apiGroups"), &[api_group])
        && string_array_equals(rule.get("resources"), &[resource])
        && string_array_equals(rule.get("verbs"), verbs)
}

fn api_agent_sandbox_rbac_is_minimal(content: &str) -> bool {
    let Some(documents) = yaml_documents(content) else {
        return false;
    };
    let Some(role) = k8s_resource_named(&documents, "Role", "mandoforge-api-agent-sandbox") else {
        return false;
    };
    let Some(rules) = role.get("rules").and_then(Value::as_array) else {
        return false;
    };
    rules.len() == 4
        && rules
            .iter()
            .any(|rule| rbac_rule_is_exact(rule, "", "pods", &["get"]))
        && rules
            .iter()
            .any(|rule| rbac_rule_is_exact(rule, "", "pods/exec", &["create", "get"]))
        && rules.iter().any(|rule| {
            rbac_rule_is_exact(
                rule,
                "extensions.agents.x-k8s.io",
                "sandboxclaims",
                &["get", "create", "delete"],
            )
        })
        && rules
            .iter()
            .any(|rule| rbac_rule_is_exact(rule, "agents.x-k8s.io", "sandboxes", &["get"]))
}

fn workload_disables_service_account_token(content: &str) -> bool {
    let Some(documents) = yaml_documents(content) else {
        return false;
    };
    documents.iter().any(|document| {
        document.get("kind").and_then(Value::as_str) == Some("Deployment")
            && document
                .get("spec")
                .and_then(|spec| spec.get("template"))
                .and_then(|template| template.get("spec"))
                .and_then(|spec| spec.get("automountServiceAccountToken"))
                .and_then(Value::as_bool)
                == Some(false)
    })
}

fn project_file_content(path: &str) -> Option<String> {
    project_file_path(path).and_then(|path| std::fs::read_to_string(path).ok())
}

#[cfg(unix)]
fn project_file_is_executable(path: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;

    project_file_path(path)
        .and_then(|path| {
            path.metadata()
                .ok()
                .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        })
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn project_file_is_executable(path: &str) -> bool {
    project_file_path(path)
        .map(|path| path.is_file())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_sandbox_network_policy_requires_restricted_rule_semantics() {
        let policy = include_str!("../../../deploy/k8s/agent-sandbox-egress-networkpolicy.yaml");
        assert!(agent_sandbox_network_policy_is_restricted(policy));

        let permissive = policy.replace("  ingress: []", "  ingress:\n    - {}");
        assert_ne!(permissive, policy, "fixture mutation must change ingress");
        assert!(!agent_sandbox_network_policy_is_restricted(&permissive));
    }

    #[test]
    fn agent_sandbox_rbac_requires_exact_minimum_permissions() {
        let rbac = include_str!("../../../deploy/k8s/api-agent-sandbox-rbac.yaml");
        assert!(api_agent_sandbox_rbac_is_minimal(rbac));

        let expanded = rbac.replace("verbs: [\"get\"]", "verbs: [\"get\", \"list\"]");
        assert_ne!(expanded, rbac, "fixture mutation must expand RBAC verbs");
        assert!(!api_agent_sandbox_rbac_is_minimal(&expanded));
    }

    #[test]
    fn worker_manifest_must_explicitly_disable_service_account_tokens() {
        let worker = include_str!("../../../deploy/k8s/worker.yaml");
        assert!(workload_disables_service_account_token(worker));
        let token_enabled = worker.replace(
            "automountServiceAccountToken: false",
            "automountServiceAccountToken: true",
        );
        assert_ne!(token_enabled, worker, "fixture mutation must enable token");
        assert!(!workload_disables_service_account_token(&token_enabled));
    }
}
