use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

use crate::*;

pub(crate) fn semantic_object_matched_fields(
    object: &SemanticObject,
    query_text: &str,
) -> Vec<String> {
    if query_text.is_empty() {
        return vec!["all".to_string()];
    }
    let needle = query_text.to_ascii_lowercase();
    let mut fields = Vec::new();
    for (field, value) in [
        ("title", object.title.as_str()),
        ("summary", object.summary.as_str()),
        ("object_key", object.object_key.as_str()),
    ] {
        if value.to_ascii_lowercase().contains(&needle) {
            fields.push(field.to_string());
        }
    }
    if object
        .content
        .to_string()
        .to_ascii_lowercase()
        .contains(&needle)
    {
        fields.push("content".to_string());
    }
    fields
}

pub(crate) fn build_semantic_graph_snapshot(
    objects: Vec<SemanticObject>,
    links: Vec<SemanticLink>,
    generated_at: DateTime<Utc>,
) -> SemanticGraphSnapshot {
    let object_ids = objects
        .iter()
        .map(|object| object.id.to_string())
        .collect::<HashSet<_>>();
    let nodes = objects
        .iter()
        .map(semantic_graph_node_from_object)
        .collect::<Vec<_>>();
    let edges = links
        .iter()
        .filter(|link| link.status == "active")
        .filter(|link| {
            object_ids.contains(&link.from_entity_id) && object_ids.contains(&link.to_entity_id)
        })
        .map(|link| SemanticGraphEdge {
            id: link.id,
            from: link.from_entity_id.clone(),
            to: link.to_entity_id.clone(),
            relation_type: link.relation_type.clone(),
            confidence: link.confidence,
            status: link.status.clone(),
        })
        .collect::<Vec<_>>();
    let conflicts = semantic_graph_conflicts(&objects, &links);
    let partitions = semantic_graph_partitions(&nodes, &conflicts);
    let stale_nodes = nodes
        .iter()
        .filter(|node| node.freshness != "current")
        .cloned()
        .collect::<Vec<_>>();
    SemanticGraphSnapshot {
        generated_at,
        node_count: nodes.len(),
        edge_count: edges.len(),
        partition_count: partitions.len(),
        nodes,
        edges,
        partitions,
        conflicts,
        stale_nodes,
    }
}

pub(crate) fn semantic_graph_node_from_object(object: &SemanticObject) -> SemanticGraphNode {
    SemanticGraphNode {
        id: object.id,
        object_type: object.object_type.clone(),
        object_key: object.object_key.clone(),
        title: object.title.clone(),
        summary: object.summary.clone(),
        trust_level: object.trust_level.clone(),
        freshness: object.freshness.clone(),
        status: object.status.clone(),
        partition_key: memory_governance_object_partition_key(object),
        semantic_scopes: object.semantic_scopes.clone(),
        source_uri: object.source_uri.clone(),
        updated_at: object.updated_at,
    }
}

pub(crate) fn semantic_graph_conflicts(
    objects: &[SemanticObject],
    links: &[SemanticLink],
) -> Vec<SemanticGraphConflict> {
    let mut conflicts = Vec::new();
    let active_ids = objects
        .iter()
        .filter(|object| object.status == "active")
        .map(|object| (object.id.to_string(), object))
        .collect::<HashMap<_, _>>();
    let mut objects_by_key: BTreeMap<String, Vec<&SemanticObject>> = BTreeMap::new();
    for object in objects.iter().filter(|object| object.status == "active") {
        objects_by_key
            .entry(object.object_key.clone())
            .or_default()
            .push(object);
    }
    for (object_key, grouped) in objects_by_key {
        if grouped.len() > 1 {
            let partition_key = memory_governance_object_partition_key(grouped[0]);
            conflicts.push(SemanticGraphConflict {
                kind: "duplicate_object_key".to_string(),
                object_key: Some(object_key.clone()),
                relation_id: None,
                object_ids: grouped.iter().map(|object| object.id).collect(),
                partition_key,
                message: format!(
                    "{} active semantic object(s) share object_key {object_key}",
                    grouped.len()
                ),
            });
        }
    }
    for link in links
        .iter()
        .filter(|link| link.status == "active" && link.relation_type == "contradicts")
    {
        let Some(from) = active_ids.get(&link.from_entity_id) else {
            continue;
        };
        let Some(to) = active_ids.get(&link.to_entity_id) else {
            continue;
        };
        conflicts.push(SemanticGraphConflict {
            kind: "contradiction_link".to_string(),
            object_key: None,
            relation_id: Some(link.id),
            object_ids: vec![from.id, to.id],
            partition_key: memory_governance_object_partition_key(from),
            message: format!("{} contradicts {}", from.object_key, to.object_key),
        });
    }
    conflicts
}

pub(crate) fn semantic_graph_partitions(
    nodes: &[SemanticGraphNode],
    conflicts: &[SemanticGraphConflict],
) -> Vec<SemanticGraphPartition> {
    let mut partitions = BTreeMap::<String, SemanticGraphPartition>::new();
    for node in nodes {
        let domain_scope = memory_governance_scope_value(&node.semantic_scopes, "domain_scope");
        let workflow_scope = memory_governance_scope_value(&node.semantic_scopes, "workflow_scope");
        let memory_scope = memory_governance_scope_value(&node.semantic_scopes, "memory_scope");
        let partition =
            partitions
                .entry(node.partition_key.clone())
                .or_insert(SemanticGraphPartition {
                    partition_key: node.partition_key.clone(),
                    domain_scope,
                    workflow_scope,
                    memory_scope,
                    node_count: 0,
                    stale_count: 0,
                    unverified_count: 0,
                    conflict_count: 0,
                });
        partition.node_count += 1;
        if node.freshness != "current" {
            partition.stale_count += 1;
        }
        if node.trust_level == "unverified" {
            partition.unverified_count += 1;
        }
    }
    for conflict in conflicts {
        if let Some(partition) = partitions.get_mut(&conflict.partition_key) {
            partition.conflict_count += 1;
        }
    }
    partitions.into_values().collect()
}

pub(crate) fn normalize_semantic_conflict_strategy(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "flag" | "manual_review" => Ok(normalized),
        _ => Err(AppError::bad_request(
            "conflict_strategy must be flag or manual_review",
        )),
    }
}

pub(crate) async fn memory_governance_inputs_for_principal(
    state: &AppState,
    principal: &Principal,
) -> Result<(Vec<SemanticObject>, Vec<MemoryWritebackCandidate>), AppError> {
    let objects = state.list_semantic_objects().await?;
    let candidates = state.list_memory_writeback_candidates(None).await?;
    if principal.roles.contains(&Role::Admin) {
        return Ok((objects, candidates));
    }

    let visible_session_ids = visible_session_ids_for_principal(state, principal).await?;
    let scoped_objects = objects
        .into_iter()
        .filter(|object| semantic_object_visible_to_sessions(object, &visible_session_ids))
        .collect::<Vec<_>>();
    let scoped_candidates = candidates
        .into_iter()
        .filter(|candidate| visible_session_ids.contains(&candidate.session_id))
        .collect::<Vec<_>>();
    Ok((scoped_objects, scoped_candidates))
}

pub(crate) fn semantic_object_visible_to_sessions(
    object: &SemanticObject,
    visible_session_ids: &HashSet<Uuid>,
) -> bool {
    if visible_session_ids.is_empty() {
        return false;
    }

    let mut object_session_ids = HashSet::new();
    if let Some(source_uri) = object.source_uri.as_deref() {
        collect_session_ids_from_uri(source_uri, &mut object_session_ids);
    }
    collect_session_ids_from_json(&object.content, &mut object_session_ids);
    collect_session_ids_from_json(&object.provenance, &mut object_session_ids);

    object_session_ids
        .iter()
        .any(|session_id| visible_session_ids.contains(session_id))
}

pub(crate) fn collect_session_ids_from_uri(uri: &str, session_ids: &mut HashSet<Uuid>) {
    if let Some(rest) = uri.strip_prefix("session://") {
        let candidate = rest.split('/').next().unwrap_or_default();
        if let Ok(session_id) = Uuid::parse_str(candidate) {
            session_ids.insert(session_id);
        }
    }
}

pub(crate) fn collect_session_ids_from_json(value: &Value, session_ids: &mut HashSet<Uuid>) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                if key == "session_id" || key.ends_with("_session_id") {
                    if let Some(candidate) = value.as_str() {
                        if let Ok(session_id) = Uuid::parse_str(candidate) {
                            session_ids.insert(session_id);
                        }
                    }
                }
                collect_session_ids_from_json(value, session_ids);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_session_ids_from_json(item, session_ids);
            }
        }
        _ => {}
    }
}

pub(crate) fn build_memory_governance_summary(
    objects: &[SemanticObject],
    candidates: &[MemoryWritebackCandidate],
    generated_at: DateTime<Utc>,
) -> MemoryGovernanceSummary {
    let mut trust_counts = BTreeMap::new();
    let mut freshness_counts = BTreeMap::new();
    let mut partition_accumulators: BTreeMap<String, MemoryGovernancePartition> = BTreeMap::new();
    let mut pending_writeback_partitions: BTreeMap<String, usize> = BTreeMap::new();
    let mut attention_items = Vec::new();

    for object in objects {
        *trust_counts.entry(object.trust_level.clone()).or_insert(0) += 1;
        *freshness_counts
            .entry(object.freshness.clone())
            .or_insert(0) += 1;
        if object.object_type != "memory" {
            continue;
        }
        let domain_scope = memory_governance_scope_value(&object.semantic_scopes, "domain_scope");
        let workflow_scope =
            memory_governance_scope_value(&object.semantic_scopes, "workflow_scope");
        let memory_scope = memory_governance_scope_value(&object.semantic_scopes, "memory_scope");
        let partition_key =
            memory_governance_partition_key(&domain_scope, &workflow_scope, &memory_scope);
        let shared = object
            .semantic_scopes
            .get("share_policy")
            .or_else(|| object.semantic_scopes.get("visibility"))
            .and_then(Value::as_str)
            .is_some_and(|value| matches!(value, "shared" | "tenant_shared" | "org_shared"));
        let partition = partition_accumulators
            .entry(partition_key.clone())
            .or_insert(MemoryGovernancePartition {
                partition_key: partition_key.clone(),
                domain_scope,
                workflow_scope,
                memory_scope,
                object_count: 0,
                memory_object_count: 0,
                human_verified_count: 0,
                unverified_count: 0,
                stale_count: 0,
                shared,
            });
        partition.object_count += 1;
        partition.memory_object_count += 1;
        partition.shared |= shared;
        if object.trust_level == "human_verified" {
            partition.human_verified_count += 1;
        }
        if object.trust_level == "unverified" {
            partition.unverified_count += 1;
        }
        if object.freshness != "current" {
            partition.stale_count += 1;
        }
    }

    for candidate in candidates
        .iter()
        .filter(|candidate| candidate.status == "pending")
    {
        let domain_scope =
            memory_governance_scope_value(&candidate.semantic_scopes, "domain_scope");
        let workflow_scope =
            memory_governance_scope_value(&candidate.semantic_scopes, "workflow_scope");
        let memory_scope =
            memory_governance_scope_value(&candidate.semantic_scopes, "memory_scope");
        let partition_key =
            memory_governance_partition_key(&domain_scope, &workflow_scope, &memory_scope);
        let shared = candidate
            .semantic_scopes
            .get("share_policy")
            .or_else(|| candidate.semantic_scopes.get("visibility"))
            .and_then(Value::as_str)
            .is_some_and(|value| matches!(value, "shared" | "tenant_shared" | "org_shared"));
        partition_accumulators
            .entry(partition_key.clone())
            .or_insert(MemoryGovernancePartition {
                partition_key: partition_key.clone(),
                domain_scope,
                workflow_scope,
                memory_scope,
                object_count: 0,
                memory_object_count: 0,
                human_verified_count: 0,
                unverified_count: 0,
                stale_count: 0,
                shared,
            })
            .shared |= shared;
        *pending_writeback_partitions
            .entry(partition_key)
            .or_insert(0) += 1;
    }

    for partition in partition_accumulators.values() {
        if partition.stale_count > 0 {
            attention_items.push(MemoryGovernanceAttentionItem {
                severity: "medium".to_string(),
                kind: "stale_memory_objects".to_string(),
                message: format!(
                    "{} stale memory object(s) require refresh or retirement",
                    partition.stale_count
                ),
                partition_key: Some(partition.partition_key.clone()),
            });
        }
        if partition.unverified_count > 0 {
            attention_items.push(MemoryGovernanceAttentionItem {
                severity: "high".to_string(),
                kind: "unverified_memory_objects".to_string(),
                message: format!(
                    "{} unverified memory object(s) must not feed high-risk execution",
                    partition.unverified_count
                ),
                partition_key: Some(partition.partition_key.clone()),
            });
        }
    }

    for (partition_key, pending_count) in &pending_writeback_partitions {
        attention_items.push(MemoryGovernanceAttentionItem {
            severity: "medium".to_string(),
            kind: "pending_memory_writeback_review".to_string(),
            message: format!(
                "{pending_count} memory writeback candidate(s) need review before becoming reusable memory"
            ),
            partition_key: Some(partition_key.clone()),
        });
    }

    let writeback = MemoryGovernanceWritebackSummary {
        pending_count: candidates
            .iter()
            .filter(|candidate| candidate.status == "pending")
            .count(),
        approved_count: candidates
            .iter()
            .filter(|candidate| candidate.status == "approved")
            .count(),
        rejected_count: candidates
            .iter()
            .filter(|candidate| candidate.status == "rejected")
            .count(),
    };
    let partitions = partition_accumulators.into_values().collect::<Vec<_>>();
    let status = if attention_items.iter().any(|item| item.severity == "high") {
        "attention_required"
    } else if attention_items.is_empty() {
        "ready"
    } else {
        "review_needed"
    }
    .to_string();

    MemoryGovernanceSummary {
        status,
        generated_at,
        isolation_policy: "domain_scope + workflow_scope + memory_scope".to_string(),
        semantic_object_count: objects.len(),
        memory_object_count: objects
            .iter()
            .filter(|object| object.object_type == "memory")
            .count(),
        partition_count: partitions.len(),
        partitions,
        trust_counts,
        freshness_counts,
        writeback,
        attention_items,
    }
}

pub(crate) fn memory_governance_scope_value(scopes: &Value, key: &str) -> String {
    scopes
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unspecified")
        .to_string()
}

pub(crate) fn memory_governance_partition_key(
    domain_scope: &str,
    workflow_scope: &str,
    memory_scope: &str,
) -> String {
    format!(
        "domain={}|workflow={}|memory={}",
        memory_governance_partition_key_component(domain_scope),
        memory_governance_partition_key_component(workflow_scope),
        memory_governance_partition_key_component(memory_scope)
    )
}

pub(crate) fn memory_governance_partition_key_component(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| match character {
            '%' => "%25".chars().collect::<Vec<_>>(),
            '|' => "%7C".chars().collect::<Vec<_>>(),
            '=' => "%3D".chars().collect::<Vec<_>>(),
            other => vec![other],
        })
        .collect()
}

pub(crate) fn memory_governance_object_partition_key(object: &SemanticObject) -> String {
    memory_governance_partition_key(
        &memory_governance_scope_value(&object.semantic_scopes, "domain_scope"),
        &memory_governance_scope_value(&object.semantic_scopes, "workflow_scope"),
        &memory_governance_scope_value(&object.semantic_scopes, "memory_scope"),
    )
}

pub(crate) fn memory_governance_candidate_partition_key(
    candidate: &MemoryWritebackCandidate,
) -> String {
    memory_governance_partition_key(
        &memory_governance_scope_value(&candidate.semantic_scopes, "domain_scope"),
        &memory_governance_scope_value(&candidate.semantic_scopes, "workflow_scope"),
        &memory_governance_scope_value(&candidate.semantic_scopes, "memory_scope"),
    )
}

pub(crate) fn memory_governance_object_ref(object: &SemanticObject) -> MemoryGovernanceObjectRef {
    MemoryGovernanceObjectRef {
        id: object.id,
        object_key: object.object_key.clone(),
        title: object.title.clone(),
        summary: object.summary.clone(),
        trust_level: object.trust_level.clone(),
        freshness: object.freshness.clone(),
        status: object.status.clone(),
        source_uri: object.source_uri.clone(),
        semantic_scopes: object.semantic_scopes.clone(),
        provenance: object.provenance.clone(),
        created_at: object.created_at,
        updated_at: object.updated_at,
    }
}

pub(crate) fn memory_governance_writeback_ref(
    candidate: &MemoryWritebackCandidate,
) -> MemoryGovernanceWritebackRef {
    MemoryGovernanceWritebackRef {
        id: candidate.id,
        session_id: candidate.session_id,
        candidate_type: candidate.candidate_type.clone(),
        proposed_object_type: candidate.proposed_object_type.clone(),
        proposed_object_key: candidate.proposed_object_key.clone(),
        title: candidate.title.clone(),
        summary: candidate.summary.clone(),
        trust_level: candidate.trust_level.clone(),
        freshness: candidate.freshness.clone(),
        status: candidate.status.clone(),
        partition_key: memory_governance_candidate_partition_key(candidate),
        semantic_object_id: candidate.semantic_object_id,
        created_at: candidate.created_at,
        updated_at: candidate.updated_at,
        decided_at: candidate.decided_at,
    }
}
