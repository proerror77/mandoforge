use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use uuid::Uuid;

use crate::{
    AppError, AppState, CreateOntologyOnboardingRunRequest, CuratedDatasetDraft,
    OntologyBuilderDag, OntologyOnboardingMaterializationResult, OntologyOnboardingProposalDraft,
    OntologyOnboardingRun, OntologyOnboardingToolSpecResponse, OntologyPromptPacket,
    OntologyReviewGraph, OntologySeedPackSummary, Permission, ReviewOntologyCuratedDatasetRequest,
    ReviewOntologyOnboardingProposalRequest, authorize_request,
    create_ontology_onboarding_run_from_adapter, create_ontology_onboarding_run_with_actor,
    get_ontology_onboarding_run_for_state, list_ontology_onboarding_runs_for_state,
    materialize_ontology_onboarding_run_with_actor, ontology_available_seed_packs,
    ontology_builder_dag_for_mode, ontology_onboarding_tool_specs_for_run,
    ontology_prompt_packet_for_run, ontology_review_graph_for_run, ontology_source_adapters,
    principal_from_request, review_ontology_curated_dataset_with_actor,
    review_ontology_onboarding_proposal_with_actor,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/ontology/onboarding/demo-runs",
            post(create_demo_ontology_onboarding_run),
        )
        .route(
            "/api/ontology/onboarding/seed-packs",
            get(list_ontology_onboarding_seed_packs),
        )
        .route(
            "/api/ontology/onboarding/runs",
            get(list_ontology_onboarding_runs).post(create_ontology_onboarding_run),
        )
        .route(
            "/api/ontology/onboarding/runs/{id}",
            get(get_ontology_onboarding_run),
        )
        .route(
            "/api/ontology/onboarding/runs/{id}/dag",
            get(get_ontology_onboarding_dag),
        )
        .route(
            "/api/ontology/onboarding/runs/{id}/prompt-packet",
            get(get_ontology_onboarding_prompt_packet),
        )
        .route(
            "/api/ontology/onboarding/runs/{id}/review-graph",
            get(get_ontology_onboarding_review_graph),
        )
        .route(
            "/api/ontology/onboarding/proposals/{id}/review",
            post(review_ontology_onboarding_proposal),
        )
        .route(
            "/api/ontology/onboarding/runs/{id}/materialize",
            post(materialize_ontology_onboarding_run),
        )
        .route(
            "/api/ontology/onboarding/runs/{id}/tool-specs",
            get(list_ontology_onboarding_tool_specs),
        )
        .route(
            "/api/ontology/onboarding/curated-datasets/{id}/review",
            post(review_ontology_curated_dataset),
        )
}

async fn create_demo_ontology_onboarding_run(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<OntologyOnboardingRun>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsWrite,
        "ontology_onboarding",
        None,
    )
    .await?;
    let principal = principal_from_request(&state, &headers).await?;
    Ok(Json(
        create_ontology_onboarding_run_with_actor(
            &state,
            "ecommerce",
            "demo_ecommerce",
            &principal.subject_id,
        )
        .await?,
    ))
}

async fn list_ontology_onboarding_seed_packs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<OntologySeedPackSummary>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "ontology_onboarding",
        None,
    )
    .await?;
    Ok(Json(
        ontology_available_seed_packs()
            .into_iter()
            .map(|seed| OntologySeedPackSummary {
                industry: seed.industry,
                domain_scope: seed.domain_scope,
                source_mode: seed.source_mode,
                tool_namespace: seed.tool_namespace,
                object_count: seed.objects.len(),
                relation_count: seed.relations.len(),
                metric_count: seed.metrics.len(),
                action_count: seed.actions.len(),
            })
            .collect(),
    ))
}

async fn create_ontology_onboarding_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateOntologyOnboardingRunRequest>,
) -> Result<Json<OntologyOnboardingRun>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsWrite,
        "ontology_onboarding",
        None,
    )
    .await?;
    let principal = principal_from_request(&state, &headers).await?;
    if let Some(payload) = input.source_payload {
        let adapted = ontology_source_adapters::adapt_payload(payload, None)?;
        return Ok(Json(
            create_ontology_onboarding_run_from_adapter(&state, adapted, &principal.subject_id)
                .await?,
        ));
    }
    let industry = input.industry.as_deref().unwrap_or("ecommerce");
    let source_mode = input.source_mode.as_deref().unwrap_or("demo_ecommerce");
    Ok(Json(
        create_ontology_onboarding_run_with_actor(
            &state,
            industry,
            source_mode,
            &principal.subject_id,
        )
        .await?,
    ))
}

async fn list_ontology_onboarding_runs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<OntologyOnboardingRun>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "ontology_onboarding",
        None,
    )
    .await?;
    Ok(Json(list_ontology_onboarding_runs_for_state(&state).await?))
}

async fn get_ontology_onboarding_run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<OntologyOnboardingRun>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "ontology_onboarding",
        Some(id),
    )
    .await?;
    get_ontology_onboarding_run_for_state(&state, id)
        .await
        .map(Json)
}

async fn get_ontology_onboarding_dag(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<OntologyBuilderDag>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "ontology_onboarding",
        Some(id),
    )
    .await?;
    let run = get_ontology_onboarding_run_for_state(&state, id).await?;
    ontology_builder_dag_for_mode("pipeline_mapping_v2", Some(run.id), None).map(Json)
}

async fn get_ontology_onboarding_prompt_packet(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<OntologyPromptPacket>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "ontology_onboarding",
        Some(id),
    )
    .await?;
    let run = get_ontology_onboarding_run_for_state(&state, id).await?;
    ontology_prompt_packet_for_run(&run).map(Json)
}

async fn get_ontology_onboarding_review_graph(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<OntologyReviewGraph>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "ontology_onboarding",
        Some(id),
    )
    .await?;
    let run = get_ontology_onboarding_run_for_state(&state, id).await?;
    ontology_review_graph_for_run(&state, &run).await.map(Json)
}

async fn review_ontology_onboarding_proposal(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ReviewOntologyOnboardingProposalRequest>,
) -> Result<Json<OntologyOnboardingProposalDraft>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsWrite,
        "ontology_onboarding",
        Some(id),
    )
    .await?;
    let principal = principal_from_request(&state, &headers).await?;
    review_ontology_onboarding_proposal_with_actor(
        &state,
        id,
        &input.decision,
        input.reason.as_deref(),
        &principal.subject_id,
    )
    .await
    .map(Json)
}

async fn materialize_ontology_onboarding_run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<OntologyOnboardingMaterializationResult>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsWrite,
        "ontology_onboarding",
        Some(id),
    )
    .await?;
    let principal = principal_from_request(&state, &headers).await?;
    materialize_ontology_onboarding_run_with_actor(&state, id, &principal.subject_id)
        .await
        .map(Json)
}

async fn list_ontology_onboarding_tool_specs(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<OntologyOnboardingToolSpecResponse>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "ontology_onboarding",
        Some(id),
    )
    .await?;
    Ok(Json(OntologyOnboardingToolSpecResponse {
        run_id: id,
        tool_specs: ontology_onboarding_tool_specs_for_run(&state, id).await?,
    }))
}

async fn review_ontology_curated_dataset(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<ReviewOntologyCuratedDatasetRequest>,
) -> Result<Json<CuratedDatasetDraft>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsWrite,
        "ontology_onboarding",
        None,
    )
    .await?;
    let principal = principal_from_request(&state, &headers).await?;
    review_ontology_curated_dataset_with_actor(
        &state,
        &id,
        &input.decision,
        input.reason.as_deref(),
        &principal.subject_id,
    )
    .await
    .map(Json)
}
