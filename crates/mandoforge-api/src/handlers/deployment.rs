use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    routing::{get, post},
};
use serde_json::{Value, json};

use crate::{
    AppError, AppState, DeploymentVersion, EnterpriseProductCompletionReadiness,
    EnterpriseSecurityAdminReadiness, Permission, ProductionAutoDeployRequest,
    ProductionDeploymentVerifyRequest, Stage2CompletionReadiness, authorize_request,
    build_enterprise_product_completion_readiness, build_enterprise_security_admin_readiness,
    build_stage2_completion_readiness, deployment_expected_value_matches,
    deployment_version_from_env, native_connectors, new_audit_log, principal_from_request,
    validate_handoff_token,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/api/deployment/version", get(get_deployment_version))
        .route(
            "/api/deployment/production/verify",
            post(verify_production_deployment),
        )
        .route("/api/deployment/auto-deploy", post(plan_auto_deploy))
        .route("/api/stage2/readiness", get(get_stage2_readiness))
        .route(
            "/api/enterprise-product/readiness",
            get(get_enterprise_product_readiness),
        )
        .route(
            "/api/enterprise-security/admin-readiness",
            get(get_enterprise_security_admin_readiness),
        )
        .route(
            "/api/native-connectors/production-readiness",
            get(get_native_connector_production_readiness),
        )
}

async fn healthz() -> Json<Value> {
    let mut payload = json!({"status": "ok"});
    if let Some(nonce) = std::env::var("MANDOFORGE_DESKTOP_HEALTH_NONCE")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        payload["desktop_health_nonce"] = json!(nonce);
    }
    Json(payload)
}

async fn get_deployment_version() -> Json<DeploymentVersion> {
    Json(deployment_version_from_env())
}

async fn verify_production_deployment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ProductionDeploymentVerifyRequest>,
) -> Result<Json<Value>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "deployment", None).await?;
    let version = deployment_version_from_env();
    let git_sha_match = deployment_expected_value_matches(
        input.expected_git_sha.as_deref(),
        version.git_sha.as_deref(),
    );
    let image_tag_match = deployment_expected_value_matches(
        input.expected_image_tag.as_deref(),
        version.image_tag.as_deref(),
    );
    let version_match = git_sha_match && image_tag_match;
    let status = if input.require_match && !version_match {
        "blocked"
    } else {
        "ready"
    };
    let target = input.target.unwrap_or_else(|| "default".to_string());
    let response = json!({
        "status": status,
        "target": target.clone(),
        "version_match": version_match,
        "running_version": version,
        "checks": {
            "git_sha_match": git_sha_match,
            "image_tag_match": image_tag_match,
            "require_match": input.require_match,
        },
        "next_actions": if status == "ready" {
            json!(["run_auto_deploy_dry_run", "run_post_deploy_verify"])
        } else {
            json!(["deploy_expected_version", "rerun_production_verify"])
        }
    });
    let principal = principal_from_request(&state, &headers).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "deployment.production_verify",
            "deployment",
            None,
            json!({
                "subject": principal.subject_id,
                "status": status,
                "target": target,
                "version_match": version_match,
                "expected_git_sha": input.expected_git_sha,
                "expected_image_tag": input.expected_image_tag,
            }),
        ))
        .await?;
    Ok(Json(response))
}

async fn plan_auto_deploy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ProductionAutoDeployRequest>,
) -> Result<Json<Value>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "deployment", None).await?;
    let version = deployment_version_from_env();
    let target = validate_handoff_token("deployment target", &input.target)?;
    let steps = vec![
        json!({
            "key": "verify_running_version",
            "title": "Verify running version",
            "status": "planned",
            "endpoint": "/api/deployment/production/verify"
        }),
        json!({
            "key": "build_or_select_image",
            "title": "Build or select image",
            "status": "planned",
            "git_sha": input.git_sha.clone(),
            "image_tag": input.image_tag.clone()
        }),
        json!({
            "key": "push_image",
            "title": "Push image",
            "status": if input.dry_run { "dry_run" } else { "blocked" }
        }),
        json!({
            "key": "deploy_target",
            "title": "Deploy target",
            "status": if input.dry_run { "dry_run" } else { "blocked" },
            "target": target.clone()
        }),
        json!({
            "key": "post_deploy_verify",
            "title": "Post deploy verify",
            "status": "planned",
            "endpoint": "/healthz"
        }),
    ];
    let status = if input.dry_run { "planned" } else { "blocked" };
    let response = json!({
        "status": status,
        "target": target.clone(),
        "dry_run": input.dry_run,
        "running_version": version,
        "requested": {
            "git_sha": input.git_sha.clone(),
            "image_tag": input.image_tag.clone()
        },
        "steps": steps,
        "blocked_reason": if input.dry_run {
            Value::Null
        } else {
            json!("live auto-deploy requires an explicit deployment controller; use dry_run until the controller is configured")
        }
    });
    let principal = principal_from_request(&state, &headers).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "deployment.auto_deploy_planned",
            "deployment",
            None,
            json!({
                "subject": principal.subject_id,
                "status": status,
                "target": target,
                "dry_run": input.dry_run,
                "git_sha": input.git_sha,
                "image_tag": input.image_tag,
            }),
        ))
        .await?;
    Ok(Json(response))
}

async fn get_stage2_readiness(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Stage2CompletionReadiness>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "stage2_readiness",
        None,
    )
    .await?;
    Ok(Json(build_stage2_completion_readiness()))
}

async fn get_enterprise_product_readiness(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<EnterpriseProductCompletionReadiness>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "enterprise_product_readiness",
        None,
    )
    .await?;
    Ok(Json(build_enterprise_product_completion_readiness()))
}

async fn get_enterprise_security_admin_readiness(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<EnterpriseSecurityAdminReadiness>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "enterprise_security_admin_readiness",
        None,
    )
    .await?;
    Ok(Json(
        build_enterprise_security_admin_readiness(&state).await?,
    ))
}

async fn get_native_connector_production_readiness(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<native_connectors::NativeConnectorProductionReadiness>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "native_connector_production_readiness",
        None,
    )
    .await?;
    Ok(Json(
        native_connectors::build_native_connector_production_readiness(),
    ))
}
