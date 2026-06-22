use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::get,
};
use uuid::Uuid;

use crate::{
    AppError, AppState, CreateSemanticSource, Permission, SemanticObject, SemanticSource,
    UpdateSemanticSource, authorize_request, record_semantic_source_audit,
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
