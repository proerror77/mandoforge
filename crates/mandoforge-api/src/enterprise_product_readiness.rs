use chrono::Utc;

use crate::{
    EnterpriseProductCompletionLane, EnterpriseProductCompletionReadiness, project_file_path,
};

pub(crate) fn build_enterprise_product_completion_readiness() -> EnterpriseProductCompletionReadiness
{
    let contract_path = "docs/enterprise-product-completion-contract.md";
    let contract_present = project_file_path(contract_path)
        .map(|path| path.is_file())
        .unwrap_or(false);
    let lanes = build_enterprise_product_completion_lanes(contract_present);
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
    let mut next_actions = vec![
        "close Remote Computer multi-node distributed state evidence".to_string(),
        "promote live connectors from generic approval-gated calls to platform-specific production semantics".to_string(),
        "promote Context OS primitives into a versioned Ontology Engine release workflow".to_string(),
        "add enterprise identity, audit export, data-retention, and operations evidence".to_string(),
    ];
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
            "production launch preflight, default Secret exclusion, secret delivery contract, or API workspace PVC evidence is missing",
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
            evidence_scripts: vec!["./scripts/production-launch-preflight.sh"],
            required_evidence: vec![
                "deploy/k8s/kustomization.yaml does not apply secret.example.yaml",
                "deploy/k8s/secret.example.yaml does not contain default database credentials",
                "deploy/k8s/secret-delivery-contract.yaml declares mandoforge-secrets as externally delivered production state",
                "API workspace storage is backed by mandoforge-workspaces PVC instead of emptyDir",
                "production launch preflight verifies static deployment safety and live readiness gates",
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
            current_boundary: "Ecommerce live adapter and approval commit path exist, but platform-specific production semantics remain per-connector work",
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
            ],
            blockers: vec![
                "live connector production semantics gate exists, but per-connector customer-grade semantics evidence has not yet been archived as ready",
                "Tmall/Taobao, Xiaohongshu, TikTok Shop, Amazon SP-API, and Lark/Feishu each need promoted production contracts",
            ],
            next_actions: vec![
                "run ./scripts/live-connector-production-semantics-gate.sh with promoted connector evidence",
                "archive token lifecycle, reconciliation, webhook, compensation, secret-redaction, and deployment evidence for each promoted connector",
                "extend the same evidence contract to Lark/Feishu enterprise connectors",
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
                "domain ontology migration and conflict-resolution policy are not customer-grade",
            ],
            next_actions: vec![
                "run ./scripts/ontology-release-workflow-trigger-gate.sh against a production target and archive the trigger evidence",
                "add domain ontology migration and relation-constraint gates",
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
            ],
            required_evidence: vec![
                "tenant onboarding profiles are versioned and completeness-gated",
                "real connector account quality checks gate pack release",
                "pack updates preserve compatibility and rollback evidence",
                "managed workflow runtime proves scheduler retry, fan-in completion, and expired step lease reclaim",
            ],
            blockers: vec![
                "customer-grade canary, compatibility matrix, tenant override policy, and workflow recovery evidence are not yet promoted to the enterprise completion gate",
            ],
            next_actions: vec![
                "add pack version compatibility and canary evidence",
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

fn production_deployment_safety_static_ready() -> bool {
    let kustomization = project_file_content("deploy/k8s/kustomization.yaml");
    let secret_example = project_file_content("deploy/k8s/secret.example.yaml");
    let secret_delivery_contract = project_file_content("deploy/k8s/secret-delivery-contract.yaml");
    let api_manifest = project_file_content("deploy/k8s/api.yaml");
    let preflight_present = project_file_path("scripts/production-launch-preflight.sh")
        .map(|path| path.is_file())
        .unwrap_or(false);

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
        && api_manifest.as_deref().is_some_and(|content| {
            content.contains("claimName: mandoforge-workspaces")
                && !content.contains("emptyDir: {}")
        })
        && preflight_present
}

fn project_file_content(path: &str) -> Option<String> {
    project_file_path(path).and_then(|path| std::fs::read_to_string(path).ok())
}
