use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    routing::get,
};

use crate::{
    AppError, AppState, OntologyEngineReadiness, OntologyRegistry, Permission, authorize_request,
    build_ontology_engine_readiness, ontology_registry,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/ontology/registry", get(get_ontology_registry))
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
