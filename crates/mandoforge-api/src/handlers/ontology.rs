use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::get,
};
use uuid::Uuid;

use crate::{
    AppError, AppState, OntologyActionContractGovernanceBoundary, OntologyActionContractListQuery,
    OntologyActionContractListResponse, OntologyActionContractProductView, OntologyEngineReadiness,
    OntologyRegistry, Permission, SemanticObject, authorize_collection_request, authorize_request,
    build_ontology_engine_readiness, ontology_action_contract_model_evidence,
    ontology_action_contract_payload, ontology_registry, visible_semantic_objects_for_principal,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/ontology/registry", get(get_ontology_registry))
        .route(
            "/api/ontology/action-contracts",
            get(list_ontology_action_contracts),
        )
        .route(
            "/api/ontology/action-contracts/{id}",
            get(get_ontology_action_contract),
        )
        .route(
            "/api/ontology/engine-readiness",
            get(get_ontology_engine_readiness),
        )
}

async fn get_ontology_registry(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<OntologyRegistry>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "ontology_registry",
        None,
    )
    .await?;
    Ok(Json(ontology_registry()))
}

async fn list_ontology_action_contracts(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<OntologyActionContractListQuery>,
) -> Result<Json<OntologyActionContractListResponse>, AppError> {
    let principal = authorize_collection_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "ontology_action_contracts",
    )
    .await?;
    let mut contracts: Vec<_> = visible_semantic_objects_for_principal(&state, &principal)
        .await?
        .into_iter()
        .filter(|object| object.object_type == "ontology_action_contract")
        .filter(|object| ontology_action_contract_matches_query(object, &query))
        .map(ontology_action_contract_product_view)
        .collect();
    contracts.sort_by(|left, right| left.object_key.cmp(&right.object_key));
    if let Some(limit) = query.limit {
        contracts.truncate(limit);
    }
    Ok(Json(OntologyActionContractListResponse {
        count: contracts.len(),
        contracts,
    }))
}

async fn get_ontology_action_contract(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<OntologyActionContractProductView>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "semantic_object",
        Some(id),
    )
    .await?;
    let object = state.get_semantic_object(id).await?;
    if object.object_type != "ontology_action_contract" {
        return Err(AppError::not_found("ontology action contract not found"));
    }
    Ok(Json(ontology_action_contract_product_view(object)))
}

async fn get_ontology_engine_readiness(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<OntologyEngineReadiness>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "ontology_engine_readiness",
        None,
    )
    .await?;
    Ok(Json(build_ontology_engine_readiness(&state).await?))
}

fn ontology_action_contract_matches_query(
    object: &SemanticObject,
    query: &OntologyActionContractListQuery,
) -> bool {
    if let Some(status) = query.status.as_deref()
        && object.status != status.trim()
    {
        return false;
    }
    if let Some(domain_scope) = query.domain_scope.as_deref() {
        let expected = domain_scope.trim();
        if !expected.is_empty()
            && object.semantic_scopes["domain_scope"].as_str() != Some(expected)
            && object.content["action_contract"]["domain_scope"].as_str() != Some(expected)
            && object.content["domain_scope"].as_str() != Some(expected)
        {
            return false;
        }
    }
    if let Some(q) = query.q.as_deref() {
        let needle = q.trim().to_ascii_lowercase();
        if !needle.is_empty()
            && !object.object_key.to_ascii_lowercase().contains(&needle)
            && !object.title.to_ascii_lowercase().contains(&needle)
            && !object.summary.to_ascii_lowercase().contains(&needle)
        {
            return false;
        }
    }
    true
}

fn ontology_action_contract_product_view(
    object: SemanticObject,
) -> OntologyActionContractProductView {
    let contract = ontology_action_contract_payload(&object.content)
        .cloned()
        .unwrap_or_else(|| object.content.clone());
    OntologyActionContractProductView {
        id: object.id,
        object_key: object.object_key,
        title: object.title,
        summary: object.summary,
        status: object.status,
        trust_level: object.trust_level,
        freshness: object.freshness,
        semantic_scopes: object.semantic_scopes,
        source_uri: object.source_uri,
        contract_model: ontology_action_contract_model_evidence(
            &contract,
            "ontology_action_contract",
        ),
        contract,
        governance_boundary: OntologyActionContractGovernanceBoundary {
            validity_authority: "ontology_action_contract".to_string(),
            execution_authority: "task_grant_policy_approval_tool_router".to_string(),
            auto_execute: false,
            requires_task_grant: true,
            requires_policy_check: true,
            requires_connector_scope: true,
            high_risk_enters_requires_action: true,
        },
        created_at: object.created_at,
        updated_at: object.updated_at,
    }
}
