use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::get,
};
use chrono::Utc;
use uuid::Uuid;

use crate::{
    AppError, AppState, CreateOntologySdkApplicationRequest,
    ONTOLOGY_SDK_APPLICATION_STATUS_ACTIVE, OntologySdkApplication, OntologySdkApplicationManifest,
    Permission, authorize_collection_request, authorize_request, new_audit_log,
    normalize_and_validate_subset, ontology_release_current_status, release_catalog_from_evidence,
    resolved_catalog_for_subset,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/ontology-sdk/applications",
            get(list_applications).post(create_application),
        )
        .route("/api/ontology-sdk/applications/{id}", get(get_application))
        .route(
            "/api/ontology-sdk/applications/{id}/manifest",
            get(get_application_manifest),
        )
}

async fn list_applications(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<OntologySdkApplication>>, AppError> {
    authorize_collection_request(
        &state,
        &headers,
        Permission::Admin,
        "ontology_sdk_application",
    )
    .await?;
    state.list_ontology_sdk_applications(None).await.map(Json)
}

async fn get_application(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<OntologySdkApplication>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "ontology_sdk_application",
        Some(id),
    )
    .await?;
    state.get_ontology_sdk_application(id).await.map(Json)
}

async fn get_application_manifest(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<OntologySdkApplicationManifest>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "ontology_sdk_application",
        Some(id),
    )
    .await?;
    let application = state.get_ontology_sdk_application(id).await?;
    let release = state
        .get_ontology_release(application.ontology_release_id)
        .await?;
    let (catalog, catalog_digest) = release_catalog_from_evidence(&release)?;
    if catalog_digest != application.catalog_digest
        || release.version != application.release_version
        || release.domain_scope != application.domain_scope
    {
        return Err(AppError::forbidden(
            "ontology SDK application release snapshot does not match the manifest",
        ));
    }
    let (subset, subset_digest) =
        normalize_and_validate_subset(&catalog, &application.subset_manifest)?;
    if subset != application.subset_manifest || subset_digest != application.subset_digest {
        return Err(AppError::forbidden(
            "ontology SDK application subset manifest does not match the manifest",
        ));
    }
    Ok(Json(OntologySdkApplicationManifest {
        application_id: application.id,
        ontology_release_id: application.ontology_release_id,
        release_version: application.release_version,
        domain_scope: application.domain_scope,
        catalog_digest: application.catalog_digest,
        subset_manifest: application.subset_manifest,
        subset_digest: application.subset_digest,
        status: application.status,
        resolved_catalog: resolved_catalog_for_subset(&catalog, &subset)?,
    }))
}

async fn create_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateOntologySdkApplicationRequest>,
) -> Result<Json<OntologySdkApplication>, AppError> {
    let principal = authorize_collection_request(
        &state,
        &headers,
        Permission::Admin,
        "ontology_sdk_application",
    )
    .await?;
    let release = state
        .get_ontology_release(input.ontology_release_id)
        .await?;
    if !ontology_release_current_status(&release.status)
        || release.promoted_at.is_none()
        || release
            .gate_result
            .get("status")
            .and_then(|value| value.as_str())
            != Some("passed")
    {
        return Err(AppError::bad_request(
            "ontology SDK applications require an active promoted ontology release",
        ));
    }
    let (catalog, catalog_digest) = release_catalog_from_evidence(&release)?;
    let (subset_manifest, subset_digest) = normalize_and_validate_subset(&catalog, &input.subset)?;
    let application = OntologySdkApplication {
        id: Uuid::new_v4(),
        tenant_id: state.current_tenant_id(),
        subject: principal.subject_id.clone(),
        ontology_release_id: release.id,
        release_version: release.version.clone(),
        domain_scope: release.domain_scope.clone(),
        catalog_digest,
        subset_manifest,
        subset_digest,
        status: ONTOLOGY_SDK_APPLICATION_STATUS_ACTIVE.to_string(),
        created_at: Utc::now(),
    };
    let application = state.create_ontology_sdk_application(application).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "ontology_sdk.application_created",
            "ontology_sdk_application",
            Some(application.id),
            serde_json::json!({
                "subject": principal.subject_id,
                "application_id": application.id,
                "ontology_release_id": application.ontology_release_id,
                "catalog_digest": application.catalog_digest,
                "subset_digest": application.subset_digest,
            }),
        ))
        .await?;
    Ok(Json(application))
}
