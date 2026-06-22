use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use uuid::Uuid;

use crate::{
    AppError, AppState, ConfidenceCalibrationResponse, EntityResolutionRequest,
    EntityResolutionResponse, Permission, SchemaUnderstandingRequest, SchemaUnderstandingResponse,
    SubgraphProposalRequest, SubgraphProposalResponse, authorize_request,
    ontology_confidence_calibration_for_run, ontology_entity_resolution_for_request,
    ontology_schema_understanding_for_request, ontology_subgraph_proposals_for_request,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/ontology/intelligence/schema-understanding",
            post(run_ontology_schema_understanding),
        )
        .route(
            "/api/ontology/intelligence/subgraph-proposals",
            post(run_ontology_subgraph_proposals),
        )
        .route(
            "/api/ontology/intelligence/entity-resolution",
            post(run_ontology_entity_resolution),
        )
        .route(
            "/api/ontology/intelligence/runs/{id}/calibration",
            get(get_ontology_confidence_calibration),
        )
}

async fn run_ontology_schema_understanding(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<SchemaUnderstandingRequest>,
) -> Result<Json<SchemaUnderstandingResponse>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "ontology_intelligence",
        input.run_id,
    )
    .await?;
    ontology_schema_understanding_for_request(&state, &input)
        .await
        .map(Json)
}

async fn run_ontology_subgraph_proposals(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<SubgraphProposalRequest>,
) -> Result<Json<SubgraphProposalResponse>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "ontology_intelligence",
        input.run_id,
    )
    .await?;
    ontology_subgraph_proposals_for_request(&state, &input)
        .await
        .map(Json)
}

async fn run_ontology_entity_resolution(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<EntityResolutionRequest>,
) -> Result<Json<EntityResolutionResponse>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "ontology_intelligence",
        input.run_id,
    )
    .await?;
    ontology_entity_resolution_for_request(&state, &input)
        .await
        .map(Json)
}

async fn get_ontology_confidence_calibration(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<ConfidenceCalibrationResponse>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "ontology_intelligence",
        Some(id),
    )
    .await?;
    ontology_confidence_calibration_for_run(&state, id)
        .await
        .map(Json)
}
