use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
};
use uuid::Uuid;

use crate::{
    AppError, AppState, CreateOntologyReleaseCandidateRequest, OntologyRelease,
    OntologyReleaseListQuery, Permission, archive_ontology_release_with_actor, authorize_request,
    create_ontology_release_candidate_with_actor, gate_ontology_release_with_actor,
    principal_from_request, promote_ontology_release_with_actor,
    rollback_ontology_release_with_actor,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/ontology/releases", get(list_ontology_releases))
        .route("/api/ontology/releases/{id}", get(get_ontology_release))
        .route(
            "/api/ontology/onboarding/runs/{id}/release-candidate",
            post(create_ontology_release_candidate),
        )
        .route(
            "/api/ontology/releases/{id}/gate",
            post(gate_ontology_release),
        )
        .route(
            "/api/ontology/releases/{id}/promote",
            post(promote_ontology_release),
        )
        .route(
            "/api/ontology/releases/{id}/rollback",
            post(rollback_ontology_release),
        )
        .route(
            "/api/ontology/releases/{id}/archive",
            post(archive_ontology_release),
        )
}

async fn list_ontology_releases(
    State(state): State<AppState>,
    Query(query): Query<OntologyReleaseListQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<OntologyRelease>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "ontology_release",
        None,
    )
    .await?;
    Ok(Json(
        state
            .list_ontology_releases_for_domain(query.domain_scope.as_deref())
            .await?,
    ))
}

async fn get_ontology_release(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<OntologyRelease>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "ontology_release",
        Some(id),
    )
    .await?;
    state.get_ontology_release(id).await.map(Json)
}

async fn create_ontology_release_candidate(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateOntologyReleaseCandidateRequest>,
) -> Result<Json<OntologyRelease>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "ontology_release",
        Some(id),
    )
    .await?;
    let principal = principal_from_request(&state, &headers).await?;
    create_ontology_release_candidate_with_actor(&state, id, input, &principal.subject_id)
        .await
        .map(Json)
}

async fn gate_ontology_release(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<OntologyRelease>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "ontology_release",
        Some(id),
    )
    .await?;
    let principal = principal_from_request(&state, &headers).await?;
    gate_ontology_release_with_actor(&state, id, &principal.subject_id)
        .await
        .map(Json)
}

async fn promote_ontology_release(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<OntologyRelease>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "ontology_release",
        Some(id),
    )
    .await?;
    let principal = principal_from_request(&state, &headers).await?;
    promote_ontology_release_with_actor(&state, id, &principal.subject_id)
        .await
        .map(Json)
}

async fn rollback_ontology_release(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<OntologyRelease>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "ontology_release",
        Some(id),
    )
    .await?;
    let principal = principal_from_request(&state, &headers).await?;
    rollback_ontology_release_with_actor(&state, id, &principal.subject_id)
        .await
        .map(Json)
}

async fn archive_ontology_release(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<OntologyRelease>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "ontology_release",
        Some(id),
    )
    .await?;
    let principal = principal_from_request(&state, &headers).await?;
    archive_ontology_release_with_actor(&state, id, &principal.subject_id)
        .await
        .map(Json)
}
