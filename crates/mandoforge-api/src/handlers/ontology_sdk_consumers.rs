use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header::CONTENT_TYPE},
    response::IntoResponse,
    routing::{get, post},
};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    AppError, AppState, OntologySdkConsumerActionRequest, OntologySdkConsumerReadQuery,
    authorize_consumer_read, consumer_object_by_id, consumer_objects, consumer_relations,
    generate_typescript_sdk, propose_consumer_action, resolved_catalog_for_subset,
    task_grant_for_consumer_read,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/ontology-sdk/applications/{application_id}/objects/{api_name}",
            get(list_objects),
        )
        .route(
            "/api/ontology-sdk/applications/{application_id}/objects/{api_name}/{object_id}",
            get(get_object),
        )
        .route(
            "/api/ontology-sdk/applications/{application_id}/relations",
            get(list_relations),
        )
        .route(
            "/api/ontology-sdk/applications/{application_id}/typescript",
            get(get_typescript),
        )
        .route(
            "/api/ontology-sdk/applications/{application_id}/actions/{api_name}",
            post(propose_action),
        )
        .route(
            "/api/ontology-sdk/applications/{application_id}/actions/{api_name}/proposal",
            post(propose_action),
        )
}

async fn list_objects(
    State(state): State<AppState>,
    Path((application_id, api_name)): Path<(Uuid, String)>,
    Query(query): Query<OntologySdkConsumerReadQuery>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let (principal, application, _release, catalog) =
        authorize_consumer_read(&state, &headers, application_id).await?;
    let grant = task_grant_for_consumer_read(&state, query.task_grant_id).await?;
    Ok(Json(serde_json::to_value(
        consumer_objects(
            &state,
            &principal,
            &application,
            &catalog,
            &api_name,
            grant.as_ref(),
        )
        .await?,
    )?))
}

async fn get_object(
    State(state): State<AppState>,
    Path((application_id, api_name, object_id)): Path<(Uuid, String, String)>,
    Query(query): Query<OntologySdkConsumerReadQuery>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let object_id = Uuid::parse_str(&object_id)
        .map_err(|_| AppError::bad_request("ontology SDK object id must be a UUID"))?;
    let (principal, application, _release, catalog) =
        authorize_consumer_read(&state, &headers, application_id).await?;
    let grant = task_grant_for_consumer_read(&state, query.task_grant_id).await?;
    Ok(Json(serde_json::to_value(
        consumer_object_by_id(
            &state,
            &principal,
            &application,
            &catalog,
            &api_name,
            object_id,
            grant.as_ref(),
        )
        .await?,
    )?))
}

async fn list_relations(
    State(state): State<AppState>,
    Path(application_id): Path<Uuid>,
    Query(query): Query<OntologySdkConsumerReadQuery>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let (principal, application, _release, catalog) =
        authorize_consumer_read(&state, &headers, application_id).await?;
    let grant = task_grant_for_consumer_read(&state, query.task_grant_id).await?;
    Ok(Json(serde_json::to_value(
        consumer_relations(
            &state,
            &principal,
            &application,
            &catalog,
            grant.as_ref(),
            &query,
        )
        .await?,
    )?))
}

async fn get_typescript(
    State(state): State<AppState>,
    Path(application_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let (_principal, application, _release, catalog) =
        authorize_consumer_read(&state, &headers, application_id).await?;
    let resolved_catalog = resolved_catalog_for_subset(&catalog, &application.subset_manifest)?;
    let source = generate_typescript_sdk(application.id, &resolved_catalog)?;
    Ok((
        StatusCode::OK,
        [(CONTENT_TYPE, "text/typescript; charset=utf-8")],
        source,
    ))
}

async fn propose_action(
    State(state): State<AppState>,
    Path((application_id, api_name)): Path<(Uuid, String)>,
    headers: HeaderMap,
    Json(input): Json<OntologySdkConsumerActionRequest>,
) -> Result<Json<Value>, AppError> {
    let (principal, application, release, catalog) =
        authorize_consumer_read(&state, &headers, application_id).await?;
    Ok(Json(
        propose_consumer_action(
            &state,
            &principal,
            &application,
            &release,
            &catalog,
            &api_name,
            input,
        )
        .await?,
    ))
}
