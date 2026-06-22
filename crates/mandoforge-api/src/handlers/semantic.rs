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
    AppError, AppState, CreateSemanticLink, CreateSemanticObject, CreateSemanticSource,
    ExpandSemanticLinksRequest, ExpandSemanticLinksResponse, ExpandSemanticOntologyRequest,
    FetchSemanticObjectRequest, FetchSemanticObjectResponse, Permission, ReviewOntologyProposalRequest,
    SemanticGraphSnapshot, SemanticLink, SemanticObject, SemanticGovernanceRunRequest,
    SemanticGovernanceRunResult, SemanticProductQuery, SemanticSearchResponse, SemanticSearchResult, SemanticSource,
    UpdateSemanticLink, UpdateSemanticObject, UpdateSemanticSource, authorize_request,
    build_semantic_graph_snapshot, domain_ontology_object_type_suggestions,
    domain_ontology_relation_type_suggestions, expand_semantic_links_for_context,
    fetch_semantic_object_for_context,
    memory_governance_object_partition_key, memory_governance_scope_value, new_audit_log,
    normalize_ontology_review_decision, normalize_optional_text, normalize_semantic_conflict_strategy,
    principal_from_request, record_semantic_link_audit, record_semantic_object_audit,
    record_semantic_source_audit, semantic_object_matched_fields, semantic_object_matches_product_query,
    validate_handoff_token, validate_semantic_link_against_ontology,
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
            "/api/semantic-ontology/expand",
            post(expand_semantic_ontology),
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
