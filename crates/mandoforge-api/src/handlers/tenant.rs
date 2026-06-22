use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, patch, post},
};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    AppError, AppState, BootstrapTenantProvisioning, CreateOrganization, Organization,
    TenantIsolationReadinessReport, TenantProvisioningResult, TransferOrganizationOwnership,
    archive_organization as archive_organization_impl,
    bootstrap_tenant_provisioning as bootstrap_tenant_provisioning_impl,
    create_organization as create_organization_impl, delete_organization as delete_organization_impl,
    get_tenant_isolation_readiness as get_tenant_isolation_readiness_impl,
    list_organizations as list_organizations_impl,
    transfer_organization_ownership as transfer_organization_ownership_impl,
    update_organization as update_organization_impl,
    validate_tenant_production_routing as validate_tenant_production_routing_impl,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/organizations",
            get(list_organizations).post(create_organization),
        )
        .route(
            "/api/tenant-provisioning/bootstrap",
            post(bootstrap_tenant_provisioning),
        )
        .route(
            "/api/tenant-isolation/readiness",
            get(get_tenant_isolation_readiness),
        )
        .route(
            "/api/tenant-isolation/routing/validate",
            post(validate_tenant_production_routing),
        )
        .route(
            "/api/organizations/{id}",
            patch(update_organization).delete(delete_organization),
        )
        .route(
            "/api/organizations/{id}/archive",
            post(archive_organization),
        )
        .route(
            "/api/organizations/{id}/transfer-ownership",
            post(transfer_organization_ownership),
        )
}

async fn list_organizations(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Organization>>, AppError> {
    list_organizations_impl(state, headers).await
}

async fn create_organization(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateOrganization>,
) -> Result<Json<Organization>, AppError> {
    create_organization_impl(state, headers, input).await
}

async fn bootstrap_tenant_provisioning(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<BootstrapTenantProvisioning>,
) -> Result<Json<TenantProvisioningResult>, AppError> {
    bootstrap_tenant_provisioning_impl(state, headers, input).await
}

async fn get_tenant_isolation_readiness(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<TenantIsolationReadinessReport>, AppError> {
    get_tenant_isolation_readiness_impl(state, headers).await
}

async fn validate_tenant_production_routing(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    validate_tenant_production_routing_impl(state, headers).await
}

async fn update_organization(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateOrganization>,
) -> Result<Json<Organization>, AppError> {
    update_organization_impl(state, id, headers, input).await
}

async fn archive_organization(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Organization>, AppError> {
    archive_organization_impl(state, id, headers).await
}

async fn delete_organization(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Organization>, AppError> {
    delete_organization_impl(state, id, headers).await
}

async fn transfer_organization_ownership(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<TransferOrganizationOwnership>,
) -> Result<Json<Organization>, AppError> {
    transfer_organization_ownership_impl(state, id, headers, input).await
}
