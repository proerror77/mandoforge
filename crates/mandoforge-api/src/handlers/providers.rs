use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, patch, post},
};
use uuid::Uuid;

use crate::{
    AppError, AppState, CreateProviderAccess, CreateProviderRecord, ProviderAccess,
    ProviderGovernanceSummary, ProviderRecord, UpdateProviderAccess,
    archive_provider_access as archive_provider_access_impl,
    create_provider as create_provider_impl,
    create_provider_access as create_provider_access_impl,
    get_provider_summary as get_provider_summary_impl,
    list_providers as list_providers_impl,
    list_provider_access as list_provider_access_impl,
    update_provider as update_provider_impl,
    update_provider_access as update_provider_access_impl,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/providers", get(list_providers).post(create_provider))
        .route("/api/providers/{id}", patch(update_provider))
        .route("/api/providers/summary", get(get_provider_summary))
        .route(
            "/api/teams/{id}/provider-access",
            get(list_provider_access).post(create_provider_access),
        )
        .route("/api/provider-access/{id}", patch(update_provider_access))
        .route(
            "/api/provider-access/{id}/archive",
            post(archive_provider_access),
        )
}

async fn list_providers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ProviderRecord>>, AppError> {
    list_providers_impl(state, headers).await
}

async fn create_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateProviderRecord>,
) -> Result<Json<ProviderRecord>, AppError> {
    create_provider_impl(state, headers, input).await
}

async fn update_provider(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateProviderRecord>,
) -> Result<Json<ProviderRecord>, AppError> {
    update_provider_impl(state, id, headers, input).await
}

async fn get_provider_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ProviderGovernanceSummary>, AppError> {
    get_provider_summary_impl(state, headers).await
}

async fn list_provider_access(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<ProviderAccess>>, AppError> {
    list_provider_access_impl(state, id, headers).await
}

async fn create_provider_access(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateProviderAccess>,
) -> Result<Json<ProviderAccess>, AppError> {
    create_provider_access_impl(state, id, headers, input).await
}

async fn update_provider_access(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateProviderAccess>,
) -> Result<Json<ProviderAccess>, AppError> {
    update_provider_access_impl(state, id, headers, input).await
}

async fn archive_provider_access(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<ProviderAccess>, AppError> {
    archive_provider_access_impl(state, id, headers).await
}
