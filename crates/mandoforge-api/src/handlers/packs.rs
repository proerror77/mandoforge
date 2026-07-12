use std::{collections::BTreeSet, path::Path as FsPath};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    AppError, AppState, InstallWorkflowPack, Permission, ValidateWorkflowPack,
    WorkflowPackArchiveRequest, WorkflowPackBinding, WorkflowPackConfigWizardPlanRequest,
    WorkflowPackConnectorQualityAssessment, WorkflowPackConnectorQualityAssessmentRequest,
    WorkflowPackInstallation, WorkflowPackOnboardingAssessment,
    WorkflowPackOnboardingAssessmentRequest, WorkflowPackProfileAsset,
    WorkflowPackProfileAssetSaveRequest, WorkflowPackReleaseRequest, WorkflowPackRollbackRequest,
    WorkflowPackRuntimeObject, WorkflowPackStageRequest, WorkflowPackUpdateRequest,
    assess_workflow_pack_connector_quality, assess_workflow_pack_onboarding, authorize_request,
    load_and_validate_workflow_pack, new_audit_log, principal_from_request,
    project_workflow_pack_semantic_layer, record_workflow_pack_installation_audit,
    record_workflow_pack_profile_asset_bootstrap_audit, resolve_workflow_pack_manifest_path,
    validate_workflow_pack_profile_assets_input, workflow_pack,
    workflow_pack_default_profile_assets, workflow_pack_kind_label, workflow_pack_manifest_summary,
    workflow_pack_materialized_bindings_with_runtime_targets,
    workflow_pack_runtime_objects_from_bindings,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/workflow-packs/validate",
            post(validate_workflow_pack_route),
        )
        .route(
            "/api/workflow-packs/marketplace",
            get(get_workflow_pack_marketplace),
        )
        .route(
            "/api/workflow-packs/config-wizard/plan",
            post(plan_workflow_pack_config_wizard),
        )
        .route(
            "/api/workflow-packs/install",
            post(install_workflow_pack_route),
        )
        .route(
            "/api/workflow-packs/installations",
            get(list_workflow_pack_installations_route),
        )
        .route(
            "/api/workflow-packs/installations/{id}",
            get(get_workflow_pack_installation_route),
        )
        .route(
            "/api/workflow-packs/installations/{id}/bindings",
            get(list_workflow_pack_bindings_route),
        )
        .route(
            "/api/workflow-packs/installations/{id}/runtime-objects",
            get(list_workflow_pack_runtime_objects_route),
        )
        .route(
            "/api/workflow-packs/installations/{id}/stage",
            post(stage_workflow_pack_installation_route),
        )
        .route(
            "/api/workflow-packs/installations/{id}/onboarding/assess",
            post(assess_workflow_pack_onboarding_route),
        )
        .route(
            "/api/workflow-packs/installations/{id}/onboarding/profiles",
            get(list_workflow_pack_profile_assets_route)
                .post(save_workflow_pack_profile_assets_route),
        )
        .route(
            "/api/workflow-packs/installations/{id}/connectors/quality/assess",
            post(assess_workflow_pack_connector_quality_route),
        )
        .route(
            "/api/workflow-packs/installations/{id}/update",
            post(update_workflow_pack_installation_route),
        )
        .route(
            "/api/workflow-packs/installations/{id}/release",
            post(release_workflow_pack_installation_route),
        )
        .route(
            "/api/workflow-packs/installations/{id}/rollback",
            post(rollback_workflow_pack_installation_route),
        )
        .route(
            "/api/workflow-packs/installations/{id}/archive",
            post(archive_workflow_pack_installation_route),
        )
}

async fn validate_workflow_pack_route(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ValidateWorkflowPack>,
) -> Result<Json<workflow_pack::WorkflowPackValidationReport>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "workflow_pack", None).await?;
    let manifest_path = resolve_workflow_pack_manifest_path(&input.manifest_path)?;
    let report = workflow_pack::validate_workflow_pack_manifest_path(&manifest_path)
        .map_err(|error| AppError::bad_request(error.to_string()))?;
    Ok(Json(report))
}

async fn get_workflow_pack_marketplace(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "workflow_pack", None).await?;
    let generated_at = Utc::now();
    let mut packs = Vec::new();
    for manifest_path in workflow_pack_marketplace_manifest_paths() {
        match load_and_validate_workflow_pack(&manifest_path) {
            Ok((_resolved_path, manifest, report)) => {
                packs.push(json!({
                    "id": manifest.id,
                    "name": manifest.name,
                    "version": manifest.version,
                    "kind": workflow_pack_kind_label(&manifest.kind),
                    "description": manifest.description,
                    "manifest_path": manifest_path,
                    "status": "ready",
                    "validation": report,
                    "manifest_summary": workflow_pack_manifest_summary(&manifest, &report),
                    "actions": ["install", "configure", "stage", "release"],
                }));
            }
            Err(error) => {
                let fallback_id = workflow_pack_marketplace_id_from_path(&manifest_path);
                packs.push(json!({
                    "id": fallback_id,
                    "name": fallback_id,
                    "manifest_path": manifest_path,
                    "status": "blocked",
                    "error": error.message,
                }));
            }
        }
    }
    Ok(Json(json!({
        "status": "ready",
        "generated_at": generated_at,
        "packs": packs,
    })))
}

async fn plan_workflow_pack_config_wizard(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<WorkflowPackConfigWizardPlanRequest>,
) -> Result<Json<Value>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "workflow_pack", None).await?;
    let manifest_path = resolve_workflow_pack_manifest_path(&input.manifest_path)?;
    let manifest_input = std::fs::read_to_string(&manifest_path)?;
    let manifest = workflow_pack::WorkflowPackManifest::from_yaml_str(&manifest_input)
        .map_err(|error| AppError::bad_request(error.to_string()))?;
    let package_dir = manifest_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let validation = match manifest.validate_package_dir(package_dir) {
        Ok(report) => json!(report),
        Err(error) => json!({
            "status": "blocked",
            "error": error.to_string(),
        }),
    };
    let steps = vec![
        json!({
            "key": "validate_manifest",
            "title": "Validate manifest",
            "status": validation
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("ready"),
            "artifact": input.manifest_path.clone(),
        }),
        json!({
            "key": "install_pack",
            "title": "Install pack",
            "status": "ready",
            "endpoint": "/api/workflow-packs/install",
        }),
        json!({
            "key": "configure_profiles",
            "title": "Configure profiles",
            "status": "operator_input_required",
            "profile_count": manifest.profiles.len(),
        }),
        json!({
            "key": "assess_onboarding",
            "title": "Assess onboarding",
            "status": "ready",
        }),
        json!({
            "key": "assess_connectors",
            "title": "Assess connectors",
            "status": "ready",
            "connector_count": manifest.connectors.len(),
        }),
        json!({
            "key": "stage_pack",
            "title": "Stage pack",
            "status": "ready",
        }),
        json!({
            "key": "release_pack",
            "title": "Release pack",
            "status": "approval_required",
        }),
    ];
    let principal = principal_from_request(&state, &headers).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "workflow_pack.config_wizard_planned",
            "workflow_pack",
            None,
            json!({
                "subject": principal.subject_id,
                "manifest_path": input.manifest_path.clone(),
                "pack_id": manifest.id.clone(),
                "version": manifest.version.clone(),
                "domain_scope": input.domain_scope.clone(),
                "target_environment": input.target_environment.clone(),
            }),
        ))
        .await?;
    Ok(Json(json!({
        "status": "ready",
        "generated_at": Utc::now(),
        "manifest_path": input.manifest_path,
        "pack": {
            "id": manifest.id,
            "name": manifest.name,
            "version": manifest.version,
            "description": manifest.description,
        },
        "validation": validation,
        "operator_inputs": {
            "domain_scope": input.domain_scope,
            "target_environment": input.target_environment,
        },
        "steps": steps,
    })))
}

fn workflow_pack_marketplace_manifest_paths() -> Vec<String> {
    vec![
        "packs/ai-governance/package.yaml".to_string(),
        "packs/ecommerce-amazon/package.yaml".to_string(),
        "packs/ecommerce-core/package.yaml".to_string(),
        "packs/ecommerce-taobao/package.yaml".to_string(),
        "packs/ecommerce-tiktok-shop/package.yaml".to_string(),
        "packs/ecommerce-tmall/package.yaml".to_string(),
        "packs/ecommerce-xianyu/package.yaml".to_string(),
        "packs/ecommerce-xiaohongshu/package.yaml".to_string(),
        "packs/legal/package.yaml".to_string(),
    ]
}

fn workflow_pack_marketplace_id_from_path(manifest_path: &str) -> String {
    FsPath::new(manifest_path)
        .parent()
        .and_then(FsPath::file_name)
        .and_then(|value| value.to_str())
        .unwrap_or(manifest_path)
        .to_string()
}

async fn list_workflow_pack_installations_route(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<WorkflowPackInstallation>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "workflow_pack_installations",
        None,
    )
    .await?;
    Ok(Json(state.list_workflow_pack_installations().await?))
}

async fn get_workflow_pack_installation_route(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<WorkflowPackInstallation>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "workflow_pack_installation",
        Some(id),
    )
    .await?;
    Ok(Json(state.get_workflow_pack_installation(id).await?))
}

async fn list_workflow_pack_bindings_route(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<WorkflowPackBinding>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "workflow_pack_installation",
        Some(id),
    )
    .await?;
    state.get_workflow_pack_installation(id).await?;
    Ok(Json(state.list_workflow_pack_bindings(id).await?))
}

async fn list_workflow_pack_runtime_objects_route(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<WorkflowPackRuntimeObject>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "workflow_pack_installation",
        Some(id),
    )
    .await?;
    state.get_workflow_pack_installation(id).await?;
    Ok(Json(state.list_workflow_pack_runtime_objects(id).await?))
}

async fn install_workflow_pack_route(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<InstallWorkflowPack>,
) -> Result<Json<WorkflowPackInstallation>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "workflow_pack", None).await?;
    let (manifest_path, manifest, report) = load_and_validate_workflow_pack(&input.manifest_path)?;
    let default_profile_assets =
        workflow_pack_default_profile_assets(&manifest, manifest_path.as_path())?;
    let now = Utc::now();
    let (installation, bootstrapped_profile_assets) = state
        .create_workflow_pack_installation_with_profile_assets(
            WorkflowPackInstallation {
                id: Uuid::new_v4(),
                pack_id: manifest.id.clone(),
                kind: workflow_pack_kind_label(&manifest.kind).to_string(),
                version: manifest.version.clone(),
                manifest_path: manifest_path.display().to_string(),
                manifest: serde_json::to_value(&manifest)?,
                validation_report: serde_json::to_value(&report)?,
                status: "installed".to_string(),
                eval_gate_status: "pending".to_string(),
                release_gate_status: "pending".to_string(),
                gate_evidence: json!({}),
                staged_at: None,
                released_at: None,
                archived_at: None,
                created_at: now,
                updated_at: now,
            },
            &default_profile_assets,
        )
        .await?;
    record_workflow_pack_installation_audit(
        &state,
        &installation,
        "workflow_pack.installed",
        json!({"validation_report": installation.validation_report}),
    )
    .await?;
    record_workflow_pack_profile_asset_bootstrap_audit(
        &state,
        &installation,
        &bootstrapped_profile_assets,
    )
    .await?;
    Ok(Json(installation))
}

async fn stage_workflow_pack_installation_route(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<WorkflowPackStageRequest>,
) -> Result<Json<WorkflowPackInstallation>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "workflow_pack_installation",
        Some(id),
    )
    .await?;
    let current = state.get_workflow_pack_installation(id).await?;
    if current.status != "installed" {
        return Err(AppError::bad_request(
            "only installed workflow packs can be staged",
        ));
    }
    let profile_assets = state.list_workflow_pack_profile_assets(id).await?;
    let materialization =
        workflow_pack_materialized_bindings_with_runtime_targets(&state, &current, &profile_assets)
            .await?;
    let staged_at = Utc::now();
    let (installation, bindings) = state
        .stage_workflow_pack_runtime_materialization(
            id,
            &current.eval_gate_status,
            &current.release_gate_status,
            current.gate_evidence.clone(),
            staged_at,
            current.released_at,
            materialization.agents,
            materialization.workflow_definitions,
            materialization.bindings,
        )
        .await?;
    let runtime_objects =
        workflow_pack_runtime_objects_from_bindings(&installation, &bindings, "staged")?;
    let runtime_objects = state
        .create_workflow_pack_runtime_objects(runtime_objects)
        .await?;
    let semantic_projection =
        project_workflow_pack_semantic_layer(&state, &installation, &bindings, &runtime_objects)
            .await?;
    record_workflow_pack_installation_audit(
        &state,
        &installation,
        "workflow_pack.staged",
        json!({"reason": input.reason}),
    )
    .await?;
    record_workflow_pack_installation_audit(
        &state,
        &installation,
        "workflow_pack.bindings_materialized",
        json!({
            "binding_count": bindings.len(),
            "binding_types": bindings
                .iter()
                .map(|binding| binding.binding_type.clone())
                .collect::<BTreeSet<_>>(),
            "runtime_object_count": runtime_objects.len(),
            "runtime_object_types": runtime_objects
                .iter()
                .map(|object| object.object_type.clone())
                .collect::<BTreeSet<_>>(),
            "semantic_projection": semantic_projection,
        }),
    )
    .await?;
    Ok(Json(installation))
}

async fn assess_workflow_pack_onboarding_route(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<WorkflowPackOnboardingAssessmentRequest>,
) -> Result<Json<WorkflowPackOnboardingAssessment>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "workflow_pack_installation",
        Some(id),
    )
    .await?;
    let installation = state.get_workflow_pack_installation(id).await?;
    let persisted_profiles = state.list_workflow_pack_profile_assets(id).await?;
    let assessment =
        assess_workflow_pack_onboarding(&installation, &persisted_profiles, input.clone())?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "workflow_pack.onboarding_assessed",
            "workflow_pack_installation",
            Some(installation.id),
            json!({
                "pack_id": installation.pack_id,
                "version": installation.version,
                "status": assessment.status,
                "reason": input.reason,
                "required_profile_count": assessment.required_profile_count,
                "inline_profile_count": assessment.inline_profile_count,
                "persisted_profile_count": assessment.persisted_profile_count,
                "provided_profile_count": assessment.provided_profile_count,
                "placeholder_profile_count": assessment.placeholder_profile_count,
                "connector_requirement_count": assessment.connector_requirement_count,
                "ready_connector_count": assessment.ready_connector_count,
                "missing_profiles": assessment.missing_profiles,
                "placeholder_profiles": assessment.placeholder_profiles,
                "connector_blockers": assessment.connector_blockers,
                "blockers": assessment.blockers,
            }),
        ))
        .await?;
    Ok(Json(assessment))
}

async fn list_workflow_pack_profile_assets_route(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<WorkflowPackProfileAsset>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "workflow_pack_installation",
        Some(id),
    )
    .await?;
    state.get_workflow_pack_installation(id).await?;
    Ok(Json(state.list_workflow_pack_profile_assets(id).await?))
}

async fn save_workflow_pack_profile_assets_route(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<WorkflowPackProfileAssetSaveRequest>,
) -> Result<Json<Vec<WorkflowPackProfileAsset>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "workflow_pack_installation",
        Some(id),
    )
    .await?;
    let installation = state.get_workflow_pack_installation(id).await?;
    let validated_profiles =
        validate_workflow_pack_profile_assets_input(&installation, input.profiles.clone())?;
    let mut saved = Vec::with_capacity(validated_profiles.len());
    for profile in &validated_profiles {
        saved.push(
            state
                .save_workflow_pack_profile_asset(id, &profile.id, &profile.content)
                .await?,
        );
    }
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "workflow_pack.onboarding_profiles_saved",
            "workflow_pack_installation",
            Some(installation.id),
            json!({
                "pack_id": installation.pack_id,
                "version": installation.version,
                "reason": input.reason,
                "profile_ids": saved.iter().map(|profile| profile.profile_id.clone()).collect::<Vec<_>>(),
                "versions": saved.iter().map(|profile| json!({
                    "profile_id": profile.profile_id,
                    "version": profile.version,
                })).collect::<Vec<_>>(),
            }),
        ))
        .await?;
    Ok(Json(saved))
}

async fn assess_workflow_pack_connector_quality_route(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<WorkflowPackConnectorQualityAssessmentRequest>,
) -> Result<Json<WorkflowPackConnectorQualityAssessment>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "workflow_pack_installation",
        Some(id),
    )
    .await?;
    let installation = state.get_workflow_pack_installation(id).await?;
    let assessment =
        assess_workflow_pack_connector_quality(&state, &installation, input.clone()).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "workflow_pack.connector_quality_assessed",
            "workflow_pack_installation",
            Some(installation.id),
            json!({
                "pack_id": installation.pack_id,
                "version": installation.version,
                "status": assessment.status,
                "reason": input.reason,
                "connector_requirement_count": assessment.connector_requirement_count,
                "ready_connector_count": assessment.ready_connector_count,
                "connector_results": assessment.connector_results,
                "blockers": assessment.blockers,
            }),
        ))
        .await?;
    Ok(Json(assessment))
}

async fn update_workflow_pack_installation_route(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<WorkflowPackUpdateRequest>,
) -> Result<Json<WorkflowPackInstallation>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "workflow_pack_installation",
        Some(id),
    )
    .await?;
    let current = state.get_workflow_pack_installation(id).await?;
    if !matches!(current.status.as_str(), "released" | "rolled_back") {
        return Err(AppError::bad_request(
            "only released or rolled back workflow packs can create a new version",
        ));
    }
    let (manifest_path, manifest, report) = load_and_validate_workflow_pack(&input.manifest_path)?;
    let kind = workflow_pack_kind_label(&manifest.kind);
    if manifest.id != current.pack_id || kind != current.kind {
        return Err(AppError::bad_request(
            "workflow pack update manifest must match the source pack id and kind",
        ));
    }
    if manifest.version == current.version {
        return Err(AppError::bad_request(
            "workflow pack update manifest must declare a new version",
        ));
    }
    let default_profile_assets =
        workflow_pack_default_profile_assets(&manifest, manifest_path.as_path())?;

    let now = Utc::now();
    let (installation, bootstrapped_profile_assets) = state
        .create_workflow_pack_installation_with_profile_assets(
            WorkflowPackInstallation {
                id: Uuid::new_v4(),
                pack_id: manifest.id.clone(),
                kind: kind.to_string(),
                version: manifest.version.clone(),
                manifest_path: manifest_path.display().to_string(),
                manifest: serde_json::to_value(&manifest)?,
                validation_report: serde_json::to_value(&report)?,
                status: "installed".to_string(),
                eval_gate_status: "pending".to_string(),
                release_gate_status: "pending".to_string(),
                gate_evidence: json!({
                    "version_update": {
                        "source_installation_id": current.id,
                        "source_status": current.status,
                        "source_version": current.version,
                        "reason": input.reason,
                        "created_at": now,
                    },
                }),
                staged_at: None,
                released_at: None,
                archived_at: None,
                created_at: now,
                updated_at: now,
            },
            &default_profile_assets,
        )
        .await?;
    record_workflow_pack_installation_audit(
        &state,
        &installation,
        "workflow_pack.version_created",
        json!({
            "source_installation_id": current.id,
            "source_status": current.status,
            "source_version": current.version,
            "new_version": installation.version,
            "validation_report": installation.validation_report,
        }),
    )
    .await?;
    record_workflow_pack_profile_asset_bootstrap_audit(
        &state,
        &installation,
        &bootstrapped_profile_assets,
    )
    .await?;
    Ok(Json(installation))
}

async fn release_workflow_pack_installation_route(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<WorkflowPackReleaseRequest>,
) -> Result<Json<WorkflowPackInstallation>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "workflow_pack_installation",
        Some(id),
    )
    .await?;
    let current = state.get_workflow_pack_installation(id).await?;
    if current.status != "staged" {
        return Err(AppError::bad_request(
            "only staged workflow packs can be released",
        ));
    }
    if input.eval_gate_status != "passed" || input.release_gate_status != "passed" {
        return Err(AppError::bad_request(
            "workflow pack release requires passed eval and release gates",
        ));
    }
    if !input.gate_evidence.is_object() {
        return Err(AppError::bad_request(
            "workflow pack release gate_evidence must be a JSON object",
        ));
    }
    let released_at = Utc::now();
    let gate_evidence = json!({
        "reason": input.reason,
        "evidence": input.gate_evidence,
        "released_at": released_at,
    });
    let installation = state
        .update_workflow_pack_installation_state(
            id,
            "released",
            &input.eval_gate_status,
            &input.release_gate_status,
            gate_evidence,
            current.staged_at,
            Some(released_at),
            Some("staged"),
        )
        .await?;
    let released_definitions = state
        .update_workflow_definition_release_states_for_pack_installation(id, "released")
        .await?;
    let released_bindings = state
        .update_workflow_pack_binding_statuses(id, "released")
        .await?;
    let released_runtime_objects = state
        .update_workflow_pack_runtime_object_statuses(id, "released")
        .await?;
    record_workflow_pack_installation_audit(
        &state,
        &installation,
        "workflow_pack.released",
        json!({
            "eval_gate_status": installation.eval_gate_status,
            "release_gate_status": installation.release_gate_status,
            "gate_evidence": installation.gate_evidence,
            "workflow_definition_count": released_definitions.len(),
            "binding_count": released_bindings.len(),
            "runtime_object_count": released_runtime_objects.len(),
        }),
    )
    .await?;
    Ok(Json(installation))
}

async fn rollback_workflow_pack_installation_route(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<WorkflowPackRollbackRequest>,
) -> Result<Json<WorkflowPackInstallation>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "workflow_pack_installation",
        Some(id),
    )
    .await?;
    let current = state.get_workflow_pack_installation(id).await?;
    if current.status != "released" {
        return Err(AppError::bad_request(
            "only released workflow packs can be rolled back",
        ));
    }
    if !input.gate_evidence.is_object() {
        return Err(AppError::bad_request(
            "workflow pack rollback gate_evidence must be a JSON object",
        ));
    }
    let rolled_back_at = Utc::now();
    let reason = input.reason.clone();
    let gate_evidence = json!({
        "release": current.gate_evidence,
        "rollback": {
            "reason": reason,
            "evidence": input.gate_evidence,
            "rolled_back_at": rolled_back_at,
        },
    });
    let installation = state
        .update_workflow_pack_installation_state(
            id,
            "rolled_back",
            &current.eval_gate_status,
            &current.release_gate_status,
            gate_evidence,
            current.staged_at,
            current.released_at,
            Some("released"),
        )
        .await?;
    let rolled_back_definitions = state
        .update_workflow_definition_release_states_for_pack_installation(id, "rolled_back")
        .await?;
    let rolled_back_bindings = state
        .update_workflow_pack_binding_statuses(id, "rolled_back")
        .await?;
    let rolled_back_runtime_objects = state
        .update_workflow_pack_runtime_object_statuses(id, "rolled_back")
        .await?;
    record_workflow_pack_installation_audit(
        &state,
        &installation,
        "workflow_pack.rolled_back",
        json!({
            "reason": input.reason,
            "rolled_back_at": rolled_back_at,
            "gate_evidence": installation.gate_evidence,
            "workflow_definition_count": rolled_back_definitions.len(),
            "binding_count": rolled_back_bindings.len(),
            "runtime_object_count": rolled_back_runtime_objects.len(),
        }),
    )
    .await?;
    Ok(Json(installation))
}

async fn archive_workflow_pack_installation_route(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<WorkflowPackArchiveRequest>,
) -> Result<Json<WorkflowPackInstallation>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "workflow_pack_installation",
        Some(id),
    )
    .await?;
    let current = state.get_workflow_pack_installation(id).await?;
    if !matches!(
        current.status.as_str(),
        "installed" | "staged" | "released" | "rolled_back"
    ) {
        return Err(AppError::bad_request(
            "only installed, staged, released, or rolled back workflow packs can be archived",
        ));
    }
    let installation = state.archive_workflow_pack_installation(id).await?;
    let archived_definitions = state
        .update_workflow_definition_release_states_for_pack_installation(id, "archived")
        .await?;
    let archived_bindings = state
        .update_workflow_pack_binding_statuses(id, "archived")
        .await?;
    let archived_runtime_objects = state
        .update_workflow_pack_runtime_object_statuses(id, "archived")
        .await?;
    record_workflow_pack_installation_audit(
        &state,
        &installation,
        "workflow_pack.archived",
        json!({
            "reason": input.reason,
            "archived_at": installation.archived_at,
            "previous_status": current.status,
            "workflow_definition_count": archived_definitions.len(),
            "binding_count": archived_bindings.len(),
            "runtime_object_count": archived_runtime_objects.len(),
        }),
    )
    .await?;
    Ok(Json(installation))
}
