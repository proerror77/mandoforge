use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use uuid::Uuid;

use crate::{
    AppError, AppState, CreateSemanticObject, CreateSemanticSource, FetchSemanticObjectRequest,
    FetchSemanticObjectResponse, Permission, SemanticObject, SemanticSource, UpdateSemanticObject,
    UpdateSemanticSource, authorize_request, fetch_semantic_object_for_context,
    record_semantic_object_audit, record_semantic_source_audit,
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
