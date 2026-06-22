use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
};
use chrono::Utc;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use uuid::Uuid;

use crate::{
    AppError, AppState, BuildSemanticOntologyRequest, CreateMemoryWritebackCandidates,
    CreateSemanticIngestionBatch, CreateSemanticLink, CreateSemanticObject, CreateSemanticSource,
    ExpandSemanticLinksRequest, ExpandSemanticLinksResponse,
    ExpandSemanticOntologyRequest, FetchSemanticObjectRequest, FetchSemanticObjectResponse,
    MemoryWritebackCandidate, Permission, ResolveSemanticConflictRequest,
    ReviewMemoryWritebackCandidate, ReviewOntologyProposalRequest, RunSemanticDreamingRequest, SemanticGraphSnapshot,
    SemanticLink, SemanticObject, SemanticGovernanceRunRequest, SemanticGovernanceRunResult, SemanticProductQuery,
    SemanticIngestionBatchResult, SemanticSearchResponse, SemanticSearchResult, SemanticSource, UpdateSemanticLink,
    UpdateSemanticObject, UpdateSemanticSource, authorize_collection_request, authorize_request,
    build_semantic_graph_snapshot, domain_ontology_object_type_suggestions, domain_ontology_relation_type_suggestions,
    expand_semantic_links_for_context, fetch_semantic_object_for_context,
    generate_memory_writeback_candidates, memory_governance_object_partition_key, memory_governance_scope_value, new_audit_log,
    normalize_ontology_builder_source_refs, normalize_ontology_builder_token,
    normalize_ontology_review_decision, normalize_optional_text, normalize_semantic_conflict_strategy,
    ontology_builder_candidate_types, ontology_builder_evidence_objects, principal_from_request,
    record_memory_writeback_candidate_review, record_semantic_link_audit, record_semantic_object_audit, record_semantic_source_audit,
    semantic_object_matched_fields, semantic_object_matches_product_query,
    semantic_ontology_builder_prompt_packet, validate_handoff_token,
    materialize_semantic_ingestion_batch, validate_semantic_ingestion_batch,
    validate_semantic_link_against_ontology, visible_session_ids_for_principal,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/semantic-sources",
            get(list_semantic_sources).post(create_semantic_source),
        )
        .route(
            "/api/semantic-sources/{id}",
            get(get_semantic_source)
                .patch(update_semantic_source)
                .delete(archive_semantic_source),
        )
        .route(
            "/api/semantic-sources/{id}/objects",
            get(list_semantic_source_objects),
        )
        .route(
            "/api/semantic-objects",
            get(list_semantic_objects).post(create_semantic_object),
        )
        .route(
            "/api/semantic-objects/{id}",
            get(get_semantic_object)
                .patch(update_semantic_object)
                .delete(archive_semantic_object),
        )
        .route(
            "/api/semantic-objects/{id}/fetch",
            post(fetch_semantic_object),
        )
        .route("/api/semantic-search", get(search_semantic_objects))
        .route("/api/semantic-graph", get(get_semantic_graph))
        .route("/api/semantic-workbench", get(get_semantic_workbench))
        .route(
            "/api/semantic-governance/run",
            post(run_semantic_governance),
        )
        .route(
            "/api/semantic-conflicts/resolve",
            post(resolve_semantic_conflict),
        )
        .route(
            "/api/semantic-reflection/dreaming/run",
            post(run_semantic_dreaming),
        )
        .route(
            "/api/semantic-reflection/queue",
            get(get_semantic_reflection_queue),
        )
        .route(
            "/api/sessions/{id}/memory-writeback-candidates",
            get(list_session_memory_writeback_candidates)
                .post(create_session_memory_writeback_candidates),
        )
        .route(
            "/api/memory-writeback-candidates",
            get(list_memory_writeback_candidates),
        )
        .route(
            "/api/memory-writeback-candidates/{id}",
            get(get_memory_writeback_candidate),
        )
        .route(
            "/api/memory-writeback-candidates/{id}/approve",
            post(approve_memory_writeback_candidate),
        )
        .route(
            "/api/memory-writeback-candidates/{id}/reject",
            post(reject_memory_writeback_candidate),
        )
        .route(
            "/api/semantic-ingestion/batches",
            post(create_semantic_ingestion_batch),
        )
        .route(
            "/api/semantic-ontology/expand",
            post(expand_semantic_ontology),
        )
        .route(
            "/api/semantic-ontology/builder",
            post(build_semantic_ontology),
        )
        .route(
            "/api/semantic-ontology/proposals/{id}/review",
            post(review_semantic_ontology_proposal),
        )
        .route(
            "/api/semantic-links",
            get(list_semantic_links).post(create_semantic_link),
        )
        .route("/api/semantic-links/expand", post(expand_semantic_links))
        .route(
            "/api/semantic-links/{id}",
            get(get_semantic_link)
                .patch(update_semantic_link)
                .delete(archive_semantic_link),
        )
}

async fn list_semantic_sources(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<SemanticSource>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "semantic_sources",
        None,
    )
    .await?;
    Ok(Json(state.list_semantic_sources().await?))
}

async fn create_semantic_source(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateSemanticSource>,
) -> Result<Json<SemanticSource>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsWrite,
        "semantic_sources",
        None,
    )
    .await?;
    let source = state.create_semantic_source(input).await?;
    record_semantic_source_audit(&state, &headers, &source, "semantic_source.created").await?;
    Ok(Json(source))
}

async fn get_semantic_source(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<SemanticSource>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "semantic_source",
        Some(id),
    )
    .await?;
    Ok(Json(state.get_semantic_source(id).await?))
}

async fn update_semantic_source(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateSemanticSource>,
) -> Result<Json<SemanticSource>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsWrite,
        "semantic_source",
        Some(id),
    )
    .await?;
    let source = state.update_semantic_source(id, input).await?;
    record_semantic_source_audit(&state, &headers, &source, "semantic_source.updated").await?;
    Ok(Json(source))
}

async fn archive_semantic_source(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<SemanticSource>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsWrite,
        "semantic_source",
        Some(id),
    )
    .await?;
    let source = state.archive_semantic_source(id).await?;
    record_semantic_source_audit(&state, &headers, &source, "semantic_source.archived").await?;
    Ok(Json(source))
}

async fn list_semantic_source_objects(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<SemanticObject>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "semantic_source",
        Some(id),
    )
    .await?;
    Ok(Json(state.list_semantic_objects_for_source(id).await?))
}

async fn list_semantic_objects(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<SemanticObject>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "semantic_objects",
        None,
    )
    .await?;
    Ok(Json(state.list_semantic_objects().await?))
}

async fn create_semantic_object(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateSemanticObject>,
) -> Result<Json<SemanticObject>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsWrite,
        "semantic_objects",
        None,
    )
    .await?;
    let object = state.create_semantic_object(input).await?;
    record_semantic_object_audit(&state, &headers, &object, "semantic_object.created").await?;
    Ok(Json(object))
}

async fn get_semantic_object(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<SemanticObject>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "semantic_object",
        Some(id),
    )
    .await?;
    Ok(Json(state.get_semantic_object(id).await?))
}

async fn fetch_semantic_object(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<FetchSemanticObjectRequest>,
) -> Result<Json<FetchSemanticObjectResponse>, AppError> {
    let packet = state.get_context_packet(input.context_packet_id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "context_packet",
        Some(packet.session_id),
    )
    .await?;
    let response = fetch_semantic_object_for_context(
        &state,
        &packet,
        id,
        input.include_content.unwrap_or(false),
    )
    .await?;
    Ok(Json(response))
}

async fn update_semantic_object(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateSemanticObject>,
) -> Result<Json<SemanticObject>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsWrite,
        "semantic_object",
        Some(id),
    )
    .await?;
    let object = state.update_semantic_object(id, input).await?;
    record_semantic_object_audit(&state, &headers, &object, "semantic_object.updated").await?;
    Ok(Json(object))
}

async fn archive_semantic_object(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<SemanticObject>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsWrite,
        "semantic_object",
        Some(id),
    )
    .await?;
    let object = state.archive_semantic_object(id).await?;
    record_semantic_object_audit(&state, &headers, &object, "semantic_object.archived").await?;
    Ok(Json(object))
}

async fn search_semantic_objects(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<SemanticProductQuery>,
) -> Result<Json<SemanticSearchResponse>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "semantic_search",
        None,
    )
    .await?;
    let query_text = query
        .q
        .as_ref()
        .and_then(|value| normalize_optional_text(value.clone()))
        .unwrap_or_default();
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let mut results = state
        .list_semantic_objects()
        .await?
        .into_iter()
        .filter(|object| semantic_object_matches_product_query(object, &query))
        .filter_map(|object| {
            let matched_fields = semantic_object_matched_fields(&object, &query_text);
            if !query_text.is_empty() && matched_fields.is_empty() {
                return None;
            }
            let mut score = matched_fields.len() as i32;
            if object.trust_level == "human_verified" {
                score += 5;
            } else if object.trust_level == "source_attested" {
                score += 2;
            }
            if object.freshness == "current" {
                score += 3;
            }
            if object.status == "active" {
                score += 1;
            }
            Some(SemanticSearchResult {
                partition_key: memory_governance_object_partition_key(&object),
                provenance: json!({
                    "source_uri": object.source_uri,
                    "source_id": object.source_id,
                    "trust_level": object.trust_level,
                    "freshness": object.freshness,
                    "updated_at": object.updated_at,
                }),
                object,
                score,
                matched_fields,
            })
        })
        .collect::<Vec<_>>();
    results.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| right.object.updated_at.cmp(&left.object.updated_at))
    });
    results.truncate(limit);
    Ok(Json(SemanticSearchResponse {
        query: query_text,
        generated_at: Utc::now(),
        result_count: results.len(),
        results,
    }))
}

async fn get_semantic_graph(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<SemanticProductQuery>,
) -> Result<Json<SemanticGraphSnapshot>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "semantic_graph",
        None,
    )
    .await?;
    let objects = state
        .list_semantic_objects()
        .await?
        .into_iter()
        .filter(|object| semantic_object_matches_product_query(object, &query))
        .collect::<Vec<_>>();
    let links = state.list_semantic_links().await?;
    Ok(Json(build_semantic_graph_snapshot(
        objects,
        links,
        Utc::now(),
    )))
}

async fn get_semantic_workbench(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<SemanticProductQuery>,
) -> Result<Json<Value>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "semantic_workbench",
        None,
    )
    .await?;
    let objects = state
        .list_semantic_objects()
        .await?
        .into_iter()
        .filter(|object| semantic_object_matches_product_query(object, &query))
        .collect::<Vec<_>>();
    let graph = build_semantic_graph_snapshot(
        objects.clone(),
        state.list_semantic_links().await?,
        Utc::now(),
    );
    let mut domains = BTreeMap::<String, Value>::new();
    for object in &objects {
        let domain_scope = memory_governance_scope_value(&object.semantic_scopes, "domain_scope");
        let entry = domains.entry(domain_scope.clone()).or_insert_with(|| {
            json!({
                "domain_scope": domain_scope,
                "object_count": 0,
                "memory_count": 0,
                "stale_count": 0,
                "conflict_count": 0,
                "suggested_object_types": [],
                "suggested_relation_types": ["supports", "contradicts", "supersedes"],
            })
        });
        let count = entry["object_count"].as_u64().unwrap_or(0);
        entry["object_count"] = json!(count + 1);
        if object.object_type == "memory" {
            let count = entry["memory_count"].as_u64().unwrap_or(0);
            entry["memory_count"] = json!(count + 1);
        }
        if object.freshness != "current" {
            let count = entry["stale_count"].as_u64().unwrap_or(0);
            entry["stale_count"] = json!(count + 1);
        }
    }
    for conflict in &graph.conflicts {
        if let Some(partition) = graph
            .partitions
            .iter()
            .find(|partition| partition.partition_key == conflict.partition_key)
        {
            if let Some(entry) = domains.get_mut(&partition.domain_scope) {
                let count = entry["conflict_count"].as_u64().unwrap_or(0);
                entry["conflict_count"] = json!(count + 1);
            }
        }
    }
    let domain_pilots = if domains.is_empty() {
        vec![json!({
            "domain_scope": query.domain_scope.clone().unwrap_or_else(|| "general".to_string()),
            "object_count": 0,
            "memory_count": 0,
            "stale_count": 0,
            "conflict_count": 0,
            "suggested_object_types": ["policy", "memory", "decision"],
            "suggested_relation_types": ["supports", "contradicts", "supersedes"],
        })]
    } else {
        domains.into_values().collect()
    };
    let aging_candidates = objects
        .iter()
        .filter(|object| object.freshness != "current")
        .map(|object| {
            json!({
                "object_id": object.id,
                "object_key": object.object_key,
                "title": object.title,
                "freshness": object.freshness,
                "trust_level": object.trust_level,
                "partition_key": memory_governance_object_partition_key(object),
                "recommended_action": "archive_or_refresh",
            })
        })
        .collect::<Vec<_>>();
    let ontology_expansion_suggestions = domain_pilots
        .iter()
        .map(|pilot| {
            let domain_scope = pilot
                .get("domain_scope")
                .and_then(Value::as_str)
                .unwrap_or("general");
            json!({
                "domain_scope": domain_scope,
                "object_types": domain_ontology_object_type_suggestions(domain_scope),
                "relation_types": domain_ontology_relation_type_suggestions(domain_scope),
                "reason": "observed semantic partition needs explicit domain ontology before production rollout",
            })
        })
        .collect::<Vec<_>>();
    Ok(Json(json!({
        "status": "ready",
        "generated_at": Utc::now(),
        "filters": {
            "domain_scope": query.domain_scope,
            "workflow_scope": query.workflow_scope,
            "memory_scope": query.memory_scope,
        },
        "domain_pilots": domain_pilots,
        "conflict_queue": graph.conflicts,
        "aging_candidates": aging_candidates,
        "ontology_expansion_suggestions": ontology_expansion_suggestions,
        "graph": graph,
    })))
}

async fn run_semantic_governance(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<SemanticGovernanceRunRequest>,
) -> Result<Json<SemanticGovernanceRunResult>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsWrite,
        "semantic_governance",
        None,
    )
    .await?;
    let conflict_strategy = normalize_semantic_conflict_strategy(&input.conflict_strategy)?;
    let query = SemanticProductQuery {
        q: None,
        object_type: None,
        domain_scope: input.domain_scope.clone(),
        workflow_scope: input.workflow_scope.clone(),
        memory_scope: input.memory_scope.clone(),
        status: Some("active".to_string()),
        trust_level: None,
        freshness: None,
        limit: None,
    };
    let objects = state
        .list_semantic_objects()
        .await?
        .into_iter()
        .filter(|object| semantic_object_matches_product_query(object, &query))
        .collect::<Vec<_>>();
    let links = state.list_semantic_links().await?;
    let graph = build_semantic_graph_snapshot(objects.clone(), links, Utc::now());
    let stale_objects = objects
        .iter()
        .filter(|object| object.freshness != "current")
        .cloned()
        .collect::<Vec<_>>();
    let mut archived_object_ids = Vec::new();
    if input.archive_stale && !input.dry_run {
        for object in stale_objects
            .iter()
            .filter(|object| object.status == "active")
        {
            let archived = state.archive_semantic_object(object.id).await?;
            record_semantic_object_audit(&state, &headers, &archived, "semantic_object.archived")
                .await?;
            archived_object_ids.push(archived.id);
        }
    }
    let result = SemanticGovernanceRunResult {
        status: if input.dry_run {
            "dry_run".to_string()
        } else {
            "applied".to_string()
        },
        generated_at: Utc::now(),
        dry_run: input.dry_run,
        archive_stale: input.archive_stale,
        conflict_strategy,
        archived_count: archived_object_ids.len(),
        conflict_count: graph.conflicts.len(),
        stale_count: stale_objects.len(),
        archived_object_ids,
        conflicts: graph.conflicts.clone(),
        graph,
    };
    let principal = principal_from_request(&state, &headers).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "semantic_governance.run",
            "semantic_governance",
            None,
            json!({
                "subject": principal.subject_id,
                "status": result.status,
                "dry_run": result.dry_run,
                "archive_stale": result.archive_stale,
                "conflict_strategy": result.conflict_strategy,
                "archived_count": result.archived_count,
                "conflict_count": result.conflict_count,
                "stale_count": result.stale_count,
                "archived_object_ids": result.archived_object_ids,
                "filters": {
                    "domain_scope": input.domain_scope,
                    "workflow_scope": input.workflow_scope,
                    "memory_scope": input.memory_scope,
                }
            }),
        ))
        .await?;
    Ok(Json(result))
}

async fn resolve_semantic_conflict(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ResolveSemanticConflictRequest>,
) -> Result<Json<Value>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsWrite,
        "semantic_conflicts",
        None,
    )
    .await?;
    let preferred = state.get_semantic_object(input.preferred_object_id).await?;
    let mut archived_object_ids = Vec::new();
    for object_id in input.archive_object_ids {
        let archived = state.archive_semantic_object(object_id).await?;
        let _ = state
            .create_semantic_link(CreateSemanticLink {
                from_entity_type: "semantic_object".to_string(),
                from_entity_id: preferred.id.to_string(),
                relation_type: "supersedes".to_string(),
                to_entity_type: "semantic_object".to_string(),
                to_entity_id: archived.id.to_string(),
                metadata: json!({
                    "reason": input.reason,
                    "resolution": "preferred_object_selected",
                }),
                provenance: json!({
                    "source": "semantic_conflicts.resolve",
                    "resolved_at": Utc::now(),
                }),
                confidence: 1.0,
                status: "active".to_string(),
            })
            .await?;
        archived_object_ids.push(archived.id);
    }
    let principal = principal_from_request(&state, &headers).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "semantic_conflict.resolved",
            "semantic_object",
            Some(preferred.id),
            json!({
                "subject": principal.subject_id,
                "preferred_object_id": preferred.id,
                "archived_object_ids": archived_object_ids,
                "reason": input.reason,
            }),
        ))
        .await?;
    Ok(Json(json!({
        "status": "resolved",
        "preferred_object_id": preferred.id,
        "archived_object_ids": archived_object_ids,
    })))
}

async fn run_semantic_dreaming(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RunSemanticDreamingRequest>,
) -> Result<Json<Value>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRun,
        "session",
        Some(input.session_id),
    )
    .await?;
    let session = state.get_session(input.session_id).await?;
    let now = Utc::now();
    let candidate = state
        .create_memory_writeback_candidate(MemoryWritebackCandidate {
            id: Uuid::new_v4(),
            session_id: session.id,
            candidate_type: "dreaming_synthesis".to_string(),
            source_event_id: None,
            source_artifact_id: None,
            source_approval_id: None,
            source_handoff_id: None,
            proposed_object_type: "memory".to_string(),
            proposed_object_key: format!(
                "memory:dreaming:{}:{}:{}",
                input.domain_scope,
                input.workflow_scope,
                Uuid::new_v4()
            ),
            title: format!("Dreaming synthesis: {}", input.goal),
            summary: "Queued reflection/dreaming synthesis for human review.".to_string(),
            content: json!({
                "goal": input.goal,
                "session_id": session.id,
                "domain_scope": input.domain_scope,
                "workflow_scope": input.workflow_scope,
                "memory_scope": input.memory_scope,
                "review_required": true,
            }),
            semantic_scopes: json!({
                "domain_scope": input.domain_scope,
                "workflow_scope": input.workflow_scope,
                "memory_scope": input.memory_scope,
                "share_policy": "review_required",
            }),
            source_refs: json!([{
                "source_type": "session",
                "source_id": session.id,
                "freshness": "current",
            }]),
            provenance: json!({
                "source": "semantic_reflection.dreaming.run",
                "session_id": session.id,
                "queued_at": now,
            }),
            trust_level: "source_attested".to_string(),
            freshness: "current".to_string(),
            status: "pending".to_string(),
            reviewer_subject: None,
            review_reason: None,
            semantic_object_id: None,
            audit_trace_id: None,
            created_at: now,
            updated_at: now,
            decided_at: None,
        })
        .await?;
    let principal = principal_from_request(&state, &headers).await?;
    state
        .append_audit_log(new_audit_log(
            Some(session.id),
            "user",
            None,
            "semantic_reflection.dreaming_queued",
            "memory_writeback_candidate",
            Some(candidate.id),
            json!({
                "subject": principal.subject_id,
                "candidate_id": candidate.id,
                "session_id": session.id,
            }),
        ))
        .await?;
    Ok(Json(json!({
        "status": "queued_for_review",
        "candidate": candidate,
    })))
}

async fn get_semantic_reflection_queue(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let principal = authorize_collection_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "semantic_reflection_queue",
    )
    .await?;
    let visible_session_ids = visible_session_ids_for_principal(&state, &principal).await?;
    let items = state
        .list_memory_writeback_candidates(None)
        .await?
        .into_iter()
        .filter(|candidate| visible_session_ids.contains(&candidate.session_id))
        .filter(|candidate| candidate.status == "pending")
        .filter(|candidate| {
            matches!(
                candidate.candidate_type.as_str(),
                "session_reflection" | "semantic_synthesis" | "dreaming_synthesis"
            )
        })
        .collect::<Vec<_>>();
    Ok(Json(json!({
        "status": "ready",
        "generated_at": Utc::now(),
        "item_count": items.len(),
        "items": items,
    })))
}

async fn list_session_memory_writeback_candidates(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<MemoryWritebackCandidate>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(id),
    )
    .await?;
    state.get_session(id).await?;
    Ok(Json(
        state.list_memory_writeback_candidates(Some(id)).await?,
    ))
}

async fn create_session_memory_writeback_candidates(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateMemoryWritebackCandidates>,
) -> Result<Json<Vec<MemoryWritebackCandidate>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRun,
        "session",
        Some(id),
    )
    .await?;
    let candidates = generate_memory_writeback_candidates(&state, id, input).await?;
    Ok(Json(candidates))
}

async fn list_memory_writeback_candidates(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<MemoryWritebackCandidate>>, AppError> {
    let principal = authorize_collection_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "memory_writeback_candidates",
    )
    .await?;
    let visible_session_ids = visible_session_ids_for_principal(&state, &principal).await?;
    Ok(Json(
        state
            .list_memory_writeback_candidates(None)
            .await?
            .into_iter()
            .filter(|candidate| visible_session_ids.contains(&candidate.session_id))
            .collect(),
    ))
}

async fn get_memory_writeback_candidate(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<MemoryWritebackCandidate>, AppError> {
    let candidate = state.get_memory_writeback_candidate(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "memory_writeback_candidate",
        Some(candidate.session_id),
    )
    .await?;
    Ok(Json(candidate))
}

async fn approve_memory_writeback_candidate(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ReviewMemoryWritebackCandidate>,
) -> Result<Json<MemoryWritebackCandidate>, AppError> {
    let candidate = state.get_memory_writeback_candidate(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::ApprovalsDecide,
        "memory_writeback_candidate",
        Some(candidate.session_id),
    )
    .await?;
    if candidate.status != "pending" {
        return Err(AppError::bad_request(
            "only pending memory writeback candidates can be reviewed",
        ));
    }
    let principal = principal_from_request(&state, &headers).await?;
    let semantic_object = state
        .create_semantic_object(CreateSemanticObject {
            source_id: None,
            object_type: candidate.proposed_object_type.clone(),
            object_key: candidate.proposed_object_key.clone(),
            title: candidate.title.clone(),
            summary: candidate.summary.clone(),
            content: candidate.content.clone(),
            semantic_scopes: candidate.semantic_scopes.clone(),
            source_uri: Some(format!(
                "session://{}/memory-writeback-candidates/{}",
                candidate.session_id, candidate.id
            )),
            provenance: candidate.provenance.clone(),
            trust_level: "human_verified".to_string(),
            freshness: candidate.freshness.clone(),
            status: "active".to_string(),
        })
        .await?;
    let updated = state
        .decide_memory_writeback_candidate(
            id,
            "approved",
            Some(principal.subject_id.clone()),
            input.reason.clone(),
            Some(semantic_object.id),
        )
        .await?;
    record_memory_writeback_candidate_review(&state, &updated, "memory_writeback.approved").await?;
    state
        .append_audit_log(new_audit_log(
            Some(updated.session_id),
            "user",
            Some(updated.id),
            "memory_writeback.semantic_object_created",
            "semantic_object",
            Some(semantic_object.id),
            json!({
                "candidate_id": updated.id,
                "session_id": updated.session_id,
                "semantic_object_id": semantic_object.id,
                "object_key": semantic_object.object_key,
                "trust_level": semantic_object.trust_level,
                "reviewer_subject": updated.reviewer_subject,
            }),
        ))
        .await?;
    Ok(Json(updated))
}

async fn reject_memory_writeback_candidate(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ReviewMemoryWritebackCandidate>,
) -> Result<Json<MemoryWritebackCandidate>, AppError> {
    let candidate = state.get_memory_writeback_candidate(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::ApprovalsDecide,
        "memory_writeback_candidate",
        Some(candidate.session_id),
    )
    .await?;
    if candidate.status != "pending" {
        return Err(AppError::bad_request(
            "only pending memory writeback candidates can be reviewed",
        ));
    }
    let principal = principal_from_request(&state, &headers).await?;
    let updated = state
        .decide_memory_writeback_candidate(
            id,
            "rejected",
            Some(principal.subject_id),
            input.reason,
            None,
        )
        .await?;
    record_memory_writeback_candidate_review(&state, &updated, "memory_writeback.rejected").await?;
    Ok(Json(updated))
}

async fn create_semantic_ingestion_batch(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateSemanticIngestionBatch>,
) -> Result<Json<SemanticIngestionBatchResult>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsWrite,
        "semantic_ingestion_batch",
        None,
    )
    .await?;
    validate_semantic_ingestion_batch(&input)?;
    let result = materialize_semantic_ingestion_batch(&state, &headers, input).await?;
    Ok(Json(result))
}

async fn expand_semantic_ontology(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ExpandSemanticOntologyRequest>,
) -> Result<Json<Value>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsWrite,
        "semantic_ontology",
        None,
    )
    .await?;
    let domain_scope = validate_handoff_token("domain_scope", &input.domain_scope)?;
    let object_types = input
        .object_types
        .iter()
        .map(|value| validate_handoff_token("object_type", value))
        .collect::<Result<Vec<_>, _>>()?;
    let relation_types = input
        .relation_types
        .iter()
        .map(|value| validate_handoff_token("relation_type", value))
        .collect::<Result<Vec<_>, _>>()?;
    let object = state
        .create_semantic_object(CreateSemanticObject {
            source_id: None,
            object_type: "ontology_expansion".to_string(),
            object_key: format!("ontology:{domain_scope}:proposal:{}", Uuid::new_v4()),
            title: format!("Ontology expansion proposal for {domain_scope}"),
            summary: input
                .reason
                .clone()
                .unwrap_or_else(|| format!("Proposed ontology expansion for {domain_scope}.")),
            content: json!({
                "domain_scope": domain_scope,
                "object_types": object_types,
                "relation_types": relation_types,
                "reason": input.reason,
                "status": "proposed",
            }),
            semantic_scopes: json!({
                "domain_scope": domain_scope,
                "workflow_scope": "ontology-expansion",
                "memory_scope": "ontology",
                "share_policy": "review_required",
            }),
            source_uri: Some(format!(
                "mandoforge://semantic-ontology/{domain_scope}/proposals"
            )),
            provenance: json!({
                "source": "semantic_ontology.expand",
                "proposed_at": Utc::now(),
            }),
            trust_level: "source_attested".to_string(),
            freshness: "current".to_string(),
            status: "active".to_string(),
        })
        .await?;
    let principal = principal_from_request(&state, &headers).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "semantic_ontology.expansion_proposed",
            "semantic_object",
            Some(object.id),
            json!({
                "subject": principal.subject_id,
                "domain_scope": domain_scope,
                "semantic_object_id": object.id,
            }),
        ))
        .await?;
    Ok(Json(json!({
        "status": "proposed",
        "object": object,
    })))
}

async fn build_semantic_ontology(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<BuildSemanticOntologyRequest>,
) -> Result<Json<Value>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsWrite,
        "semantic_ontology",
        None,
    )
    .await?;
    let domain_scope =
        normalize_ontology_builder_token("domain_scope", input.domain_scope.as_str())?;
    let workflow_scope = input
        .workflow_scope
        .as_deref()
        .map(|value| normalize_ontology_builder_token("workflow_scope", value))
        .transpose()?;
    let memory_scope = input
        .memory_scope
        .as_deref()
        .map(|value| normalize_ontology_builder_token("memory_scope", value))
        .transpose()?;
    let objective = input
        .objective
        .and_then(normalize_optional_text)
        .unwrap_or_else(|| format!("Build a governed ontology first draft for {domain_scope}."));
    let source_text = input.source_text.and_then(normalize_optional_text);
    let source_refs = normalize_ontology_builder_source_refs(input.source_refs)?;
    let object_types = ontology_builder_candidate_types(
        &domain_scope,
        source_text.as_deref(),
        input.agent_draft.as_ref(),
        "object_types",
        input.max_object_types.unwrap_or(12),
    )?;
    let relation_types = ontology_builder_candidate_types(
        &domain_scope,
        source_text.as_deref(),
        input.agent_draft.as_ref(),
        "relation_types",
        input.max_relation_types.unwrap_or(12),
    )?;
    if object_types.is_empty() && relation_types.is_empty() {
        return Err(AppError::bad_request(
            "ontology builder requires at least one candidate object_type or relation_type",
        ));
    }
    let evidence_objects = ontology_builder_evidence_objects(&state, &input.evidence_object_ids)
        .await?
        .into_iter()
        .map(|object| {
            json!({
                "semantic_object_id": object.id,
                "object_type": object.object_type,
                "object_key": object.object_key,
                "title": object.title,
                "trust_level": object.trust_level,
                "freshness": object.freshness,
            })
        })
        .collect::<Vec<_>>();
    let builder = json!({
        "mode": "ai_first_draft",
        "authority": "proposal_only",
        "draft_source": if input.agent_draft.is_some() { "agent_draft" } else { "deterministic_scaffold" },
        "requires_review": true,
        "object_candidate_count": object_types.len(),
        "relation_candidate_count": relation_types.len(),
    });
    let prompt_packet = semantic_ontology_builder_prompt_packet(
        &domain_scope,
        workflow_scope.as_deref(),
        memory_scope.as_deref(),
        &objective,
        source_text.as_deref(),
        &source_refs,
    );
    let content = json!({
        "domain_scope": domain_scope,
        "workflow_scope": workflow_scope,
        "memory_scope": memory_scope,
        "object_types": object_types,
        "relation_types": relation_types,
        "objective": objective,
        "source_text": source_text,
        "source_refs": source_refs,
        "evidence_object_ids": input.evidence_object_ids,
        "evidence_objects": evidence_objects,
        "agent_draft": input.agent_draft,
        "builder": builder.clone(),
        "prompt_packet": prompt_packet,
        "review_gates": [
            "human_review_required",
            "scope_isolation_check",
            "relation_policy_review",
            "source_provenance_check"
        ],
        "status": "proposed",
        "preview_only": input.preview_only,
    });
    if input.preview_only {
        return Ok(Json(json!({
            "status": "preview",
            "builder": builder,
            "proposal": content,
            "object": Value::Null,
        })));
    }
    let domain_scope = content
        .get("domain_scope")
        .and_then(Value::as_str)
        .unwrap_or("general")
        .to_string();
    let workflow_scope = content
        .get("workflow_scope")
        .and_then(Value::as_str)
        .unwrap_or("ontology-builder")
        .to_string();
    let memory_scope = content
        .get("memory_scope")
        .and_then(Value::as_str)
        .unwrap_or("ontology")
        .to_string();
    let object = state
        .create_semantic_object(CreateSemanticObject {
            source_id: None,
            object_type: "ontology_expansion".to_string(),
            object_key: format!("ontology:{domain_scope}:builder:{}", Uuid::new_v4()),
            title: format!("AI ontology first draft for {domain_scope}"),
            summary: format!(
                "AI-assisted ontology proposal for {domain_scope}/{workflow_scope}/{memory_scope}; review required before promotion."
            ),
            content: content.clone(),
            semantic_scopes: json!({
                "domain_scope": domain_scope,
                "workflow_scope": workflow_scope,
                "memory_scope": memory_scope,
                "share_policy": "review_required",
            }),
            source_uri: Some(format!("mandoforge://semantic-ontology/{domain_scope}/builder")),
            provenance: json!({
                "source": "semantic_ontology.builder",
                "proposed_at": Utc::now(),
                "authority": "proposal_only",
            }),
            trust_level: "source_attested".to_string(),
            freshness: "current".to_string(),
            status: "active".to_string(),
        })
        .await?;
    let principal = principal_from_request(&state, &headers).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "semantic_ontology.builder_proposed",
            "semantic_object",
            Some(object.id),
            json!({
                "subject": principal.subject_id,
                "domain_scope": domain_scope,
                "workflow_scope": workflow_scope,
                "memory_scope": memory_scope,
                "semantic_object_id": object.id,
                "builder": builder.clone(),
            }),
        ))
        .await?;
    Ok(Json(json!({
        "status": "proposed",
        "builder": builder,
        "object": object,
    })))
}

async fn review_semantic_ontology_proposal(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ReviewOntologyProposalRequest>,
) -> Result<Json<Value>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsWrite,
        "semantic_ontology",
        Some(id),
    )
    .await?;
    let proposal = state.get_semantic_object(id).await?;
    if proposal.object_type != "ontology_expansion" {
        return Err(AppError::bad_request(
            "semantic ontology proposal review requires an ontology_expansion object",
        ));
    }
    let decision = normalize_ontology_review_decision(&input.decision)?;
    let status = match decision.as_str() {
        "approve" => "approved",
        "reject" => "rejected",
        "request_changes" => "changes_requested",
        _ => "reviewed",
    };
    let principal = principal_from_request(&state, &headers).await?;
    let mut content = proposal.content.as_object().cloned().unwrap_or_default();
    content.insert("status".to_string(), json!(status));
    content.insert(
        "review".to_string(),
        json!({
            "decision": decision.clone(),
            "reason": input.reason.clone(),
            "reviewer": principal.subject_id.clone(),
            "reviewed_at": Utc::now(),
        }),
    );
    let object = state
        .update_semantic_object(
            proposal.id,
            UpdateSemanticObject {
                title: None,
                summary: None,
                content: Some(Value::Object(content)),
                semantic_scopes: None,
                source_uri: None,
                provenance: None,
                trust_level: None,
                freshness: None,
                status: None,
            },
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "semantic_ontology.proposal_reviewed",
            "semantic_object",
            Some(object.id),
            json!({
                "subject": principal.subject_id,
                "status": status,
                "decision": decision.clone(),
                "reason": input.reason,
                "semantic_object_id": object.id,
            }),
        ))
        .await?;
    Ok(Json(json!({
        "status": status,
        "object": object,
    })))
}

async fn list_semantic_links(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<SemanticLink>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "semantic_links",
        None,
    )
    .await?;
    Ok(Json(state.list_semantic_links().await?))
}

async fn create_semantic_link(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateSemanticLink>,
) -> Result<Json<SemanticLink>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsWrite,
        "semantic_links",
        None,
    )
    .await?;
    validate_semantic_link_against_ontology(&input)?;
    let link = state.create_semantic_link(input).await?;
    record_semantic_link_audit(&state, &headers, &link, "semantic_link.created").await?;
    Ok(Json(link))
}

async fn expand_semantic_links(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ExpandSemanticLinksRequest>,
) -> Result<Json<ExpandSemanticLinksResponse>, AppError> {
    let packet = state.get_context_packet(input.context_packet_id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "context_packet",
        Some(packet.session_id),
    )
    .await?;
    let response = expand_semantic_links_for_context(&state, &packet, input).await?;
    Ok(Json(response))
}

async fn get_semantic_link(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<SemanticLink>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "semantic_link",
        Some(id),
    )
    .await?;
    Ok(Json(state.get_semantic_link(id).await?))
}

async fn update_semantic_link(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateSemanticLink>,
) -> Result<Json<SemanticLink>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsWrite,
        "semantic_link",
        Some(id),
    )
    .await?;
    let link = state.update_semantic_link(id, input).await?;
    record_semantic_link_audit(&state, &headers, &link, "semantic_link.updated").await?;
    Ok(Json(link))
}

async fn archive_semantic_link(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<SemanticLink>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsWrite,
        "semantic_link",
        Some(id),
    )
    .await?;
    let link = state.archive_semantic_link(id).await?;
    record_semantic_link_audit(&state, &headers, &link, "semantic_link.archived").await?;
    Ok(Json(link))
}
