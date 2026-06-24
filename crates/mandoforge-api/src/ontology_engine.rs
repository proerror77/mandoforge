use std::collections::HashMap;

use axum::http::HeaderMap;
use chrono::Utc;
use serde_json::{Value, json};

use crate::*;

pub(crate) async fn build_ontology_engine_readiness(
    state: &AppState,
) -> Result<OntologyEngineReadiness, AppError> {
    let registry = ontology_registry();
    let releases = state.list_ontology_releases().await?;
    let lifecycle_releases = releases
        .iter()
        .filter(|release| {
            matches!(
                release.status.as_str(),
                "candidate"
                    | ONTOLOGY_RELEASE_STATUS_ACTIVE
                    | ONTOLOGY_RELEASE_STATUS_ACTIVE_TRIGGER_FAILED
                    | "superseded"
                    | "rolled_back"
            )
        })
        .collect::<Vec<_>>();
    let active_releases = releases
        .iter()
        .filter(|release| ontology_release_current_status(&release.status))
        .collect::<Vec<_>>();
    let lifecycle_release_evidence_class =
        ontology_active_release_evidence_class(&lifecycle_releases);
    let lifecycle_release_evidence = ontology_active_release_evidence(&lifecycle_releases);
    let has_lifecycle_release = !lifecycle_releases.is_empty();
    let active_release_evidence_class = ontology_active_release_evidence_class(&active_releases);
    let active_release_evidence = ontology_active_release_evidence(&active_releases);
    let active_release_materialized = active_releases.iter().any(|release| {
        release.gate_result.get("status").and_then(Value::as_str) == Some("passed")
            && (release
                .materialized_object_ids
                .as_array()
                .is_some_and(|ids| !ids.is_empty())
                || release
                    .materialized_link_ids
                    .as_array()
                    .is_some_and(|ids| !ids.is_empty()))
    });
    let active_release_migration_ready = active_releases.iter().any(|release| {
        release.gate_result.get("status").and_then(Value::as_str) == Some("passed")
            && ontology_release_migration_policy_ready(&release.migration_policy)
    });
    let checks = vec![
        ontology_engine_check(
            "core-registry",
            "Core ontology registry",
            "ready",
            "repo_controlled",
            vec![
                format!("registry_version={}", registry.version),
                format!("object_type_count={}", registry.object_types.len()),
                format!("relation_type_count={}", registry.relation_types.len()),
                "GET /api/ontology/registry".to_string(),
            ],
            Vec::new(),
            Vec::new(),
        ),
        ontology_engine_check(
            "relation-constraints",
            "Relation constraint enforcement",
            "ready",
            "repo_controlled",
            vec![
                "semantic link writes call validate_semantic_link_against_ontology".to_string(),
                "semantic ingestion batches reject disallowed relations before writing".to_string(),
                "semantic_links_reject_relations_not_declared_in_ontology test coverage".to_string(),
            ],
            Vec::new(),
            Vec::new(),
        ),
        ontology_engine_check(
            "builder-review-proposals",
            "Ontology Builder proposal review",
            "ready",
            "repo_controlled",
            vec![
                "POST /api/semantic-ontology/builder creates proposal_only first drafts".to_string(),
                "POST /api/semantic-ontology/proposals/{id}/review records review decisions and audit logs".to_string(),
                "builder preview mode does not mutate durable registry state".to_string(),
            ],
            Vec::new(),
            Vec::new(),
        ),
        ontology_engine_check(
            "context-packet-rendering",
            "Context packet ontology rendering",
            "ready",
            "repo_controlled",
            vec![
                "POST /api/context-packets/{id}/render includes ontology_scope".to_string(),
                "POST /api/semantic-objects/{id}/fetch is constrained to packet retrieved_objects".to_string(),
                "POST /api/semantic-links/expand expands relation context from packet-visible objects".to_string(),
            ],
            Vec::new(),
            Vec::new(),
        ),
        ontology_engine_check(
            "conflict-trust-runtime-gates",
            "Conflict, trust, and freshness runtime gates",
            "pilot_ready",
            "production_like_pilot",
            vec![
                "semantic workbench exposes conflicts and reflection queue".to_string(),
                "high-risk tools fail closed on stale or untrusted semantic context".to_string(),
            ],
            vec![
                "customer-grade policy must bind conflict/trust downgrade states to every high-risk workflow lane".to_string(),
            ],
            vec![
                "archive conflict resolution, trust downgrade, and high-risk block evidence per customer domain".to_string(),
            ],
        ),
        ontology_engine_check(
            "domain-ontology-lifecycle",
            "Domain ontology lifecycle",
            if has_lifecycle_release { "ready" } else { "blocked" },
            &lifecycle_release_evidence_class,
            if has_lifecycle_release {
                lifecycle_release_evidence
            } else {
                vec![
                "workflow packs can declare ontology seeds and ontology action types".to_string(),
                "ontology_expansion proposals are persisted as semantic objects".to_string(),
                ]
            },
            if has_lifecycle_release {
                Vec::new()
            } else {
                vec![
                "domain ontology versions cannot yet be promoted, rolled back, or migrated as first-class registry releases".to_string(),
                ]
            },
            if has_lifecycle_release {
                vec![
                    "bind promoted ontology releases to customer workflow rollout policy".to_string(),
                ]
            } else {
                vec![
                "add domain ontology release records with promote, rollback, archive, and migration evidence".to_string(),
                ]
            },
        ),
        ontology_engine_check(
            "approved-release-materialization",
            "Approved ontology release materialization",
            if active_release_materialized {
                "ready"
            } else {
                "blocked"
            },
            &active_release_evidence_class,
            if active_release_materialized {
                active_release_evidence.clone()
            } else {
                vec!["proposal review records durable audit evidence".to_string()]
            },
            if active_release_materialized {
                Vec::new()
            } else {
                vec![
                    "approved ontology proposals do not yet materialize into versioned registry releases".to_string(),
                ]
            },
            if active_release_materialized {
                vec!["archive materialized object/link evidence per promoted release".to_string()]
            } else {
                vec![
                    "add a release workflow that turns approved proposals into immutable ontology versions".to_string(),
                ]
            },
        ),
        ontology_engine_check(
            "migration-policy",
            "Ontology migration policy",
            if active_release_migration_ready {
                "ready"
            } else {
                "blocked"
            },
            &active_release_evidence_class,
            if active_release_migration_ready {
                active_release_evidence
            } else {
                vec![
                    "core semantic storage migrations exist for sources, objects, links, context packets, and writeback candidates".to_string(),
                ]
            },
            if active_release_migration_ready {
                Vec::new()
            } else {
                vec![
                    "domain ontology schema migration compatibility and rollback policy are not represented as customer-grade evidence".to_string(),
                ]
            },
            if active_release_migration_ready {
                vec![
                    "exercise rollback evidence for each customer domain before production enablement".to_string(),
                ]
            } else {
                vec![
                    "add compatibility checks for ontology version migrations across WorkflowPack releases".to_string(),
                ]
            },
        ),
    ];
    let check_count = checks.len();
    let ready_check_count = checks
        .iter()
        .filter(|check| check.status == "ready")
        .count();
    let pilot_ready_check_count = checks
        .iter()
        .filter(|check| check.status == "pilot_ready")
        .count();
    let blocked_check_count = checks
        .iter()
        .filter(|check| check.status == "blocked")
        .count();
    let completion_blocked = ready_check_count != check_count;
    let status = if completion_blocked {
        "blocked"
    } else {
        "ready"
    }
    .to_string();
    let next_actions = vec![
        "promote ontology proposals into immutable registry versions".to_string(),
        "add domain ontology migration and rollback evidence".to_string(),
        "bind conflict, trust, and freshness gates to customer-grade high-risk workflow policy"
            .to_string(),
    ];
    let message = if completion_blocked {
        format!(
            "Ontology Engine completion is blocked: {ready_check_count}/{check_count} checks are ready, {pilot_ready_check_count} are pilot-ready, and {blocked_check_count} remain blocked"
        )
    } else {
        "Ontology Engine has customer-grade evidence for every required check".to_string()
    };

    Ok(OntologyEngineReadiness {
        generated_at: Utc::now(),
        status,
        registry_version: registry.version,
        required_evidence_class: "customer_grade".to_string(),
        object_type_count: registry.object_types.len(),
        relation_type_count: registry.relation_types.len(),
        check_count,
        ready_check_count,
        pilot_ready_check_count,
        blocked_check_count,
        completion_blocked,
        checks,
        next_actions,
        message,
    })
}

pub(crate) fn ontology_active_release_evidence_class(
    active_releases: &[&OntologyRelease],
) -> String {
    if active_releases
        .iter()
        .any(|release| release.release_class == "customer_grade")
    {
        "customer_grade".to_string()
    } else if active_releases
        .iter()
        .any(|release| release.release_class == "production_like_pilot")
    {
        "production_like_pilot".to_string()
    } else {
        "repo_controlled".to_string()
    }
}

pub(crate) fn ontology_active_release_evidence(
    active_releases: &[&OntologyRelease],
) -> Vec<String> {
    let mut evidence = vec![format!("active_release_count={}", active_releases.len())];
    for release in active_releases {
        evidence.push(format!(
            "active_release id={} version={} domain_scope={} release_class={} object_count={} relation_count={} action_count={}",
            release.id,
            release.version,
            release.domain_scope,
            release.release_class,
            release.object_count,
            release.relation_count,
            release.action_count
        ));
        evidence.push(format!(
            "active_release_gate_status={}",
            release
                .gate_result
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        ));
        evidence.push(format!(
            "materialized_object_id_count={}",
            release
                .materialized_object_ids
                .as_array()
                .map(Vec::len)
                .unwrap_or_default()
        ));
        evidence.push(format!(
            "materialized_link_id_count={}",
            release
                .materialized_link_ids
                .as_array()
                .map(Vec::len)
                .unwrap_or_default()
        ));
        evidence.push(format!(
            "migration_policy_compatibility={}",
            release
                .migration_policy
                .get("compatibility")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        ));
        evidence.push(format!(
            "migration_policy_rollback={}",
            release
                .migration_policy
                .get("rollback")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        ));
    }
    evidence
}

pub(crate) fn ontology_engine_check(
    id: &str,
    title: &str,
    status: &str,
    current_evidence_class: &str,
    evidence: Vec<String>,
    blockers: Vec<String>,
    next_actions: Vec<String>,
) -> OntologyEngineReadinessCheck {
    OntologyEngineReadinessCheck {
        id: id.to_string(),
        title: title.to_string(),
        status: status.to_string(),
        current_evidence_class: current_evidence_class.to_string(),
        required_evidence_class: "customer_grade".to_string(),
        evidence,
        blockers,
        next_actions,
    }
}

pub(crate) fn ontology_registry() -> OntologyRegistry {
    OntologyRegistry {
        version: "core-v0.1".to_string(),
        object_types: vec![
            ontology_object_type(
                "action",
                "Workflow-pack action contract component that can materialize into an executable ontology ActionType.",
                Some("action"),
                None,
                "approval, connector, and audit policy boundary",
            ),
            ontology_object_type(
                "agent",
                "Configured worker persona that can receive governed workflow tasks.",
                Some("agent"),
                None,
                "agent capability and release policy",
            ),
            ontology_object_type(
                "artifact",
                "Versioned output or evidence object produced during a managed run.",
                Some("artifact"),
                None,
                "session, workflow, and retention policy",
            ),
            ontology_object_type(
                "approval_rule",
                "Human-in-the-loop rule for risky workflow decisions or side effects.",
                None,
                None,
                "approval policy boundary",
            ),
            ontology_object_type(
                "case",
                "Customer, legal, commerce, or operations case boundary.",
                None,
                None,
                "domain and customer isolation policy",
            ),
            ontology_object_type(
                "connector",
                "External platform or internal adapter configured for governed workflow access.",
                Some("connector"),
                None,
                "credential, API, and side-effect approval boundary",
            ),
            ontology_object_type(
                "decision",
                "Human or system decision that explains why a workflow path changed.",
                None,
                None,
                "approval and audit policy",
            ),
            ontology_object_type(
                "memory",
                "Governed memory object that can be shared or isolated by semantic scope.",
                Some("memory"),
                Some("L0-L4"),
                "tenant, domain, workflow, and trust partition",
            ),
            ontology_object_type(
                "metric",
                "Observed runtime, quality, cost, or business signal.",
                None,
                None,
                "observability and evaluation policy",
            ),
            ontology_object_type(
                "team",
                "Organization team that owns agents, packs, memory, or workflows.",
                None,
                None,
                "tenant and team isolation policy",
            ),
            ontology_object_type(
                "tenant",
                "Top-level organization memory, policy, and execution boundary.",
                None,
                None,
                "hard tenant isolation policy",
            ),
            ontology_object_type(
                "tool",
                "Capability endpoint or adapter available behind governed runtime policy.",
                None,
                None,
                "tool policy and approval boundary",
            ),
            ontology_object_type(
                "pack",
                "Installed workflow pack or skill bundle boundary.",
                Some("pack"),
                None,
                "pack installation and version policy",
            ),
            ontology_object_type(
                "policy",
                "Runtime rule that constrains tools, memory, approvals, and side effects.",
                Some("policy"),
                None,
                "policy rollout and approval policy",
            ),
            ontology_object_type(
                "project",
                "Project-level scope for work, memory, artifacts, and workflows.",
                Some("project"),
                None,
                "project and tenant isolation policy",
            ),
            ontology_object_type(
                "repo",
                "Source repository or codebase scope.",
                Some("repo"),
                None,
                "repository access policy",
            ),
            ontology_object_type(
                "runtime_profile",
                "Execution runtime profile available to an agent or workflow step.",
                Some("runtime_profile"),
                None,
                "runtime and remote-computer policy",
            ),
            ontology_object_type(
                "semantic_object",
                "Canonical semantic fact, rule, runbook, memory, or decision node.",
                Some("semantic_object"),
                None,
                "source provenance, trust, freshness, and semantic scope",
            ),
            ontology_object_type(
                "semantic_source",
                "Ingested source that semantic objects can cite.",
                Some("semantic_source"),
                None,
                "source ownership and freshness policy",
            ),
            ontology_object_type(
                "service",
                "Internal or external service involved in a workflow.",
                Some("service"),
                None,
                "service access policy",
            ),
            ontology_object_type(
                "session",
                "Managed agent session event-log boundary.",
                Some("session"),
                None,
                "session execution and retention policy",
            ),
            ontology_object_type(
                "workflow_pack",
                "Productized workflow package that materializes pack-scoped runtime objects.",
                Some("pack"),
                None,
                "pack installation and version policy",
            ),
            ontology_object_type(
                "workflow",
                "Managed workflow or workflow step graph.",
                Some("workflow"),
                None,
                "workflow graph and task-grant policy",
            ),
            ontology_object_type(
                "business_object",
                "Approved enterprise ontology object compiled from reviewed onboarding proposals.",
                Some("semantic_object"),
                Some("domain"),
                "reviewed business ontology boundary",
            ),
            ontology_object_type(
                "business_metric",
                "Approved semantic metric definition compiled from reviewed onboarding proposals.",
                Some("semantic_object"),
                Some("domain"),
                "reviewed metric definition boundary",
            ),
            ontology_object_type(
                "ontology_action_type",
                "Operational ontology action contract with parameters, validations, approval, effects, and audit requirements.",
                Some("ontology_action_type"),
                Some("domain"),
                "action approval and connector side-effect boundary",
            ),
            ontology_object_type(
                "ontology_expansion",
                "Proposed domain ontology addition that must be reviewed before promotion.",
                Some("semantic_object"),
                Some("domain"),
                "semantic ontology governance boundary",
            ),
            ontology_object_type(
                "ontology_onboarding_proposal",
                "Evidence-backed ontology onboarding proposal that requires human review before materialization.",
                Some("semantic_object"),
                Some("domain"),
                "ontology onboarding proposal boundary",
            ),
            ontology_object_type(
                "ontology_object_type",
                "Domain object type declared by a workflow pack ontology seed.",
                Some("ontology_object_type"),
                Some("domain"),
                "domain semantic model governance boundary",
            ),
            ontology_object_type(
                "ontology_relation_type",
                "Domain relation type declared by a workflow pack ontology seed.",
                Some("ontology_relation_type"),
                Some("domain"),
                "domain semantic relation governance boundary",
            ),
        ],
        relation_types: vec![
            ontology_relation_type(
                "applies_to",
                "policy",
                "runtime_profile",
                "Policy constrains a runtime profile.",
                "runtime policy binding",
            ),
            ontology_relation_type(
                "belongs_to",
                "artifact",
                "session",
                "Artifact is owned by a session.",
                "session evidence boundary",
            ),
            ontology_relation_type(
                "belongs_to",
                "repo",
                "project",
                "Repository is scoped under a project.",
                "project isolation boundary",
            ),
            ontology_relation_type(
                "belongs_to",
                "workflow",
                "pack",
                "Workflow is delivered by a workflow pack.",
                "pack installation boundary",
            ),
            ontology_relation_type(
                "contradicts",
                "semantic_object",
                "semantic_object",
                "One semantic object conflicts with another and requires review.",
                "memory conflict governance",
            ),
            ontology_relation_type(
                "acts_on_object_type",
                "ontology_action_type",
                "ontology_object_type",
                "ActionType is scoped to a declared domain object type.",
                "action target governance",
            ),
            ontology_relation_type(
                "declares_action_type",
                "pack",
                "ontology_action_type",
                "Workflow pack declares an operational ontology ActionType.",
                "pack action contract boundary",
            ),
            ontology_relation_type(
                "declares_ontology_object_type",
                "pack",
                "ontology_object_type",
                "Workflow pack declares a domain ontology object type.",
                "pack ontology contract boundary",
            ),
            ontology_relation_type(
                "declares_ontology_relation_type",
                "pack",
                "ontology_relation_type",
                "Workflow pack declares a domain ontology relation type.",
                "pack ontology contract boundary",
            ),
            ontology_relation_type(
                "defines_scope_for",
                "semantic_object",
                "workflow",
                "Semantic object defines the intended scope for a workflow.",
                "workflow context boundary",
            ),
            ontology_relation_type(
                "derived_from",
                "semantic_object",
                "artifact",
                "Semantic object was derived from a concrete artifact.",
                "provenance boundary",
            ),
            ontology_relation_type(
                "executes",
                "agent",
                "workflow",
                "Agent can execute or claim work for a workflow.",
                "agent task-grant boundary",
            ),
            ontology_relation_type(
                "materializes_action_type",
                "action",
                "ontology_action_type",
                "Pack action component materializes into an ontology ActionType.",
                "action contract projection boundary",
            ),
            ontology_relation_type(
                "relation_from_object_type",
                "ontology_relation_type",
                "ontology_object_type",
                "Ontology relation type can originate from this object type.",
                "domain relation governance",
            ),
            ontology_relation_type(
                "relation_to_object_type",
                "ontology_relation_type",
                "ontology_object_type",
                "Ontology relation type can target this object type.",
                "domain relation governance",
            ),
            ontology_relation_type(
                "requires",
                "workflow",
                "policy",
                "Workflow requires a policy to be active.",
                "workflow policy boundary",
            ),
            ontology_relation_type(
                "supports",
                "semantic_object",
                "semantic_object",
                "One semantic object supports another.",
                "memory evidence governance",
            ),
            ontology_relation_type(
                "supersedes",
                "semantic_object",
                "semantic_object",
                "One semantic object replaces an older object.",
                "memory freshness governance",
            ),
            ontology_relation_type(
                "uses_connector",
                "ontology_action_type",
                "connector",
                "ActionType uses a configured connector for governed platform effects.",
                "connector side-effect governance",
            ),
        ],
    }
}

pub(crate) fn ontology_object_type(
    name: &str,
    description: &str,
    entity_type: Option<&str>,
    memory_level: Option<&str>,
    governance_boundary: &str,
) -> OntologyObjectType {
    OntologyObjectType {
        name: name.to_string(),
        description: description.to_string(),
        entity_type: entity_type.map(ToString::to_string),
        memory_level: memory_level.map(ToString::to_string),
        governance_boundary: governance_boundary.to_string(),
    }
}

pub(crate) fn ontology_relation_type(
    name: &str,
    from_entity_type: &str,
    to_entity_type: &str,
    description: &str,
    governance_boundary: &str,
) -> OntologyRelationType {
    OntologyRelationType {
        name: name.to_string(),
        from_entity_type: from_entity_type.to_string(),
        to_entity_type: to_entity_type.to_string(),
        description: description.to_string(),
        governance_boundary: governance_boundary.to_string(),
    }
}

pub(crate) fn validate_semantic_link_against_ontology(
    input: &CreateSemanticLink,
) -> Result<(), AppError> {
    let relation = normalized_ontology_token(&input.relation_type);
    if relation.is_empty() {
        return Err(AppError::bad_request(
            "semantic relation_type cannot be empty",
        ));
    }
    let from_entity_type = normalized_ontology_token(&input.from_entity_type);
    let to_entity_type = normalized_ontology_token(&input.to_entity_type);
    let allowed = ontology_registry()
        .relation_types
        .into_iter()
        .any(|allowed| {
            allowed.name == relation
                && allowed.from_entity_type == from_entity_type
                && allowed.to_entity_type == to_entity_type
        });
    if !allowed {
        return Err(AppError::bad_request(
            "semantic relation_type is not allowed by ontology registry",
        ));
    }
    Ok(())
}

pub(crate) fn normalized_ontology_token(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace('-', "_")
}

pub(crate) fn validate_semantic_ingestion_batch(
    input: &CreateSemanticIngestionBatch,
) -> Result<(), AppError> {
    if input.objects.is_empty() {
        return Err(AppError::bad_request(
            "semantic ingestion batch requires at least one object",
        ));
    }
    let mut refs = HashSet::new();
    for object in &input.objects {
        let temp_ref = normalize_ingestion_temp_ref(&object.temp_ref)?;
        if !refs.insert(temp_ref.clone()) {
            return Err(AppError::bad_request(format!(
                "semantic ingestion object temp_ref is duplicated: {temp_ref}"
            )));
        }
    }
    for link in &input.links {
        let from_ref = normalize_ingestion_temp_ref(&link.from_ref)?;
        if !refs.contains(&from_ref) {
            return Err(AppError::bad_request(format!(
                "semantic ingestion link from_ref is unknown: {from_ref}"
            )));
        }
        let to_ref = normalize_ingestion_temp_ref(&link.to_ref)?;
        if !refs.contains(&to_ref) {
            return Err(AppError::bad_request(format!(
                "semantic ingestion link to_ref is unknown: {to_ref}"
            )));
        }
        validate_semantic_link_against_ontology(&CreateSemanticLink {
            from_entity_type: "semantic_object".to_string(),
            from_entity_id: from_ref,
            relation_type: link.relation_type.clone(),
            to_entity_type: "semantic_object".to_string(),
            to_entity_id: to_ref,
            metadata: empty_json_object(),
            provenance: empty_json_object(),
            confidence: link.confidence,
            status: link.status.clone(),
        })?;
    }
    Ok(())
}

pub(crate) async fn materialize_semantic_ingestion_batch(
    state: &AppState,
    headers: &HeaderMap,
    input: CreateSemanticIngestionBatch,
) -> Result<SemanticIngestionBatchResult, AppError> {
    let source = state.create_semantic_source(input.source).await?;
    record_semantic_source_audit(state, headers, &source, "semantic_source.created").await?;

    let mut objects = Vec::new();
    let mut object_refs = Vec::new();
    let mut ids_by_ref = HashMap::new();
    for object_input in input.objects {
        let temp_ref = normalize_ingestion_temp_ref(&object_input.temp_ref)?;
        let object = state
            .create_semantic_object(CreateSemanticObject {
                source_id: Some(source.id),
                object_type: object_input.object_type,
                object_key: object_input.object_key,
                title: object_input.title,
                summary: object_input.summary,
                content: object_input.content,
                semantic_scopes: object_input.semantic_scopes,
                source_uri: None,
                provenance: merge_json_objects(
                    object_input.provenance,
                    json!({
                        "ingestion_source_id": source.id,
                        "ingestion_source_uri": source.source_uri,
                        "ingestion_temp_ref": temp_ref,
                    }),
                )?,
                trust_level: object_input.trust_level,
                freshness: object_input.freshness,
                status: object_input.status,
            })
            .await?;
        record_semantic_object_audit(state, headers, &object, "semantic_object.created").await?;
        ids_by_ref.insert(temp_ref.clone(), object.id);
        object_refs.push(SemanticIngestionObjectRef {
            temp_ref,
            semantic_object_id: object.id,
            object_key: object.object_key.clone(),
            title: object.title.clone(),
        });
        objects.push(object);
    }

    let mut links = Vec::new();
    for link_input in input.links {
        let from_ref = normalize_ingestion_temp_ref(&link_input.from_ref)?;
        let to_ref = normalize_ingestion_temp_ref(&link_input.to_ref)?;
        let from_id = ids_by_ref
            .get(&from_ref)
            .ok_or_else(|| AppError::bad_request("semantic ingestion link from_ref is unknown"))?;
        let to_id = ids_by_ref
            .get(&to_ref)
            .ok_or_else(|| AppError::bad_request("semantic ingestion link to_ref is unknown"))?;
        let link = state
            .create_semantic_link(CreateSemanticLink {
                from_entity_type: "semantic_object".to_string(),
                from_entity_id: from_id.to_string(),
                relation_type: link_input.relation_type,
                to_entity_type: "semantic_object".to_string(),
                to_entity_id: to_id.to_string(),
                metadata: link_input.metadata,
                provenance: merge_json_objects(
                    link_input.provenance,
                    json!({
                        "ingestion_source_id": source.id,
                        "ingestion_source_uri": source.source_uri,
                        "from_temp_ref": from_ref,
                        "to_temp_ref": to_ref,
                    }),
                )?,
                confidence: link_input.confidence,
                status: link_input.status,
            })
            .await?;
        record_semantic_link_audit(state, headers, &link, "semantic_link.created").await?;
        links.push(link);
    }

    let ingested_at = Utc::now();
    let result = SemanticIngestionBatchResult {
        status: "completed".to_string(),
        source,
        objects,
        object_refs,
        links,
        ingested_at,
    };
    record_semantic_ingestion_batch_audit(state, headers, &result).await?;
    Ok(result)
}

pub(crate) fn normalize_ingestion_temp_ref(value: &str) -> Result<String, AppError> {
    let temp_ref = value.trim();
    if temp_ref.is_empty() {
        Err(AppError::bad_request(
            "semantic ingestion object temp_ref cannot be empty",
        ))
    } else {
        Ok(temp_ref.to_string())
    }
}

pub(crate) fn merge_json_objects(base: Value, extra: Value) -> Result<Value, AppError> {
    let mut merged = base.as_object().cloned().ok_or_else(|| {
        AppError::bad_request("semantic ingestion provenance must be a JSON object")
    })?;
    let extra = extra.as_object().ok_or_else(|| {
        AppError::bad_request("semantic ingestion provenance must be a JSON object")
    })?;
    for (key, value) in extra {
        merged.insert(key.clone(), value.clone());
    }
    Ok(Value::Object(merged))
}

pub(crate) async fn record_semantic_ingestion_batch_audit(
    state: &AppState,
    headers: &HeaderMap,
    result: &SemanticIngestionBatchResult,
) -> Result<(), AppError> {
    let principal = principal_from_request(state, headers).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "semantic_ingestion.batch_created",
            "semantic_source",
            Some(result.source.id),
            json!({
                "subject": principal.subject_id,
                "source_id": result.source.id,
                "source_uri": result.source.source_uri,
                "object_count": result.objects.len(),
                "link_count": result.links.len(),
                "object_refs": result.object_refs,
                "ingested_at": result.ingested_at,
            }),
        ))
        .await?;
    Ok(())
}

pub(crate) async fn record_semantic_source_audit(
    state: &AppState,
    headers: &HeaderMap,
    source: &SemanticSource,
    action: &str,
) -> Result<(), AppError> {
    let principal = principal_from_request(state, headers).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            action,
            "semantic_source",
            Some(source.id),
            json!({
                "subject": principal.subject_id,
                "source_type": source.source_type,
                "source_uri": source.source_uri,
                "display_name": source.display_name,
                "owner_type": source.owner_type,
                "owner_id": source.owner_id,
                "status": source.status,
                "last_ingested_at": source.last_ingested_at,
                "metadata": source.metadata,
                "provenance": source.provenance,
                "freshness": source.freshness,
            }),
        ))
        .await?;
    Ok(())
}

pub(crate) async fn record_semantic_object_audit(
    state: &AppState,
    headers: &HeaderMap,
    object: &SemanticObject,
    action: &str,
) -> Result<(), AppError> {
    let principal = principal_from_request(state, headers).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            action,
            "semantic_object",
            Some(object.id),
            json!({
                "subject": principal.subject_id,
                "source_id": object.source_id,
                "object_type": object.object_type,
                "object_key": object.object_key,
                "title": object.title,
                "source_uri": object.source_uri,
                "trust_level": object.trust_level,
                "freshness": object.freshness,
                "status": object.status,
                "semantic_scopes": object.semantic_scopes,
                "provenance": object.provenance,
            }),
        ))
        .await?;
    Ok(())
}

pub(crate) async fn record_semantic_link_audit(
    state: &AppState,
    headers: &HeaderMap,
    link: &SemanticLink,
    action: &str,
) -> Result<(), AppError> {
    let principal = principal_from_request(state, headers).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            action,
            "semantic_link",
            Some(link.id),
            json!({
                "subject": principal.subject_id,
                "from_entity_type": link.from_entity_type,
                "from_entity_id": link.from_entity_id,
                "relation_type": link.relation_type,
                "to_entity_type": link.to_entity_type,
                "to_entity_id": link.to_entity_id,
                "confidence": link.confidence,
                "status": link.status,
                "metadata": link.metadata,
                "provenance": link.provenance,
            }),
        ))
        .await?;
    Ok(())
}
