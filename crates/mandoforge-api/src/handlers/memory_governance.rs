use axum::{
    Json, Router,
    extract::{Query, State},
    http::HeaderMap,
    routing::get,
};
use chrono::Utc;

use crate::{
    AppError, AppState, MemoryGovernancePartitionDetail, MemoryGovernancePartitionQuery,
    MemoryGovernanceSummary, MemoryGovernanceWritebackQuery, MemoryGovernanceWritebackQueue,
    Permission, authorize_collection_request, build_memory_governance_summary,
    memory_governance_candidate_partition_key, memory_governance_inputs_for_principal,
    memory_governance_object_partition_key, memory_governance_object_ref,
    memory_governance_writeback_ref, normalize_optional_text, require_non_empty,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/memory-governance/summary",
            get(get_memory_governance_summary),
        )
        .route(
            "/api/memory-governance/partitions",
            get(get_memory_governance_partition_detail),
        )
        .route(
            "/api/memory-governance/writebacks",
            get(get_memory_governance_writebacks),
        )
}

async fn get_memory_governance_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<MemoryGovernanceSummary>, AppError> {
    let principal = authorize_collection_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "memory_governance",
    )
    .await?;
    let (objects, candidates) = memory_governance_inputs_for_principal(&state, &principal).await?;
    Ok(Json(build_memory_governance_summary(
        &objects,
        &candidates,
        Utc::now(),
    )))
}

async fn get_memory_governance_partition_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<MemoryGovernancePartitionQuery>,
) -> Result<Json<MemoryGovernancePartitionDetail>, AppError> {
    let principal = authorize_collection_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "memory_governance",
    )
    .await?;
    let partition_key = require_non_empty(query.partition_key, "memory partition key")?;
    let limit = query.limit.unwrap_or(25).clamp(1, 100);
    let generated_at = Utc::now();
    let (objects, candidates) = memory_governance_inputs_for_principal(&state, &principal).await?;
    let summary = build_memory_governance_summary(&objects, &candidates, generated_at);
    let Some(partition) = summary
        .partitions
        .iter()
        .find(|partition| partition.partition_key == partition_key)
        .cloned()
    else {
        return Err(AppError::not_found("memory governance partition not found"));
    };
    let mut object_refs = objects
        .iter()
        .filter(|object| object.object_type == "memory")
        .filter(|object| memory_governance_object_partition_key(object) == partition_key)
        .map(memory_governance_object_ref)
        .collect::<Vec<_>>();
    let object_count = object_refs.len();
    object_refs.sort_by_key(|object_ref| std::cmp::Reverse(object_ref.updated_at));
    object_refs.truncate(limit);

    let mut writeback_candidates = candidates
        .iter()
        .filter(|candidate| memory_governance_candidate_partition_key(candidate) == partition_key)
        .map(memory_governance_writeback_ref)
        .collect::<Vec<_>>();
    writeback_candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.updated_at));
    writeback_candidates.truncate(limit);

    let risk_items = summary
        .attention_items
        .into_iter()
        .filter(|item| item.partition_key.as_deref() == Some(partition_key.as_str()))
        .collect::<Vec<_>>();
    let pending_writeback_count = candidates
        .iter()
        .filter(|candidate| candidate.status == "pending")
        .filter(|candidate| memory_governance_candidate_partition_key(candidate) == partition_key)
        .count();
    let access_policy = if partition.shared {
        "shared_within_declared_scope"
    } else {
        "isolated_partition"
    }
    .to_string();

    Ok(Json(MemoryGovernancePartitionDetail {
        generated_at,
        partition,
        object_count,
        pending_writeback_count,
        access_policy,
        risk_items,
        objects: object_refs,
        writeback_candidates,
    }))
}

async fn get_memory_governance_writebacks(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<MemoryGovernanceWritebackQuery>,
) -> Result<Json<MemoryGovernanceWritebackQueue>, AppError> {
    let principal = authorize_collection_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "memory_governance",
    )
    .await?;
    let status_filter = query
        .status
        .as_ref()
        .and_then(|value| normalize_optional_text(value.clone()));
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let generated_at = Utc::now();
    let (_, candidates) = memory_governance_inputs_for_principal(&state, &principal).await?;
    let pending_count = candidates
        .iter()
        .filter(|candidate| candidate.status == "pending")
        .count();
    let mut refs = candidates
        .iter()
        .filter(|candidate| {
            status_filter
                .as_deref()
                .map(|status| candidate.status == status)
                .unwrap_or(true)
        })
        .map(memory_governance_writeback_ref)
        .collect::<Vec<_>>();
    refs.sort_by_key(|object_ref| std::cmp::Reverse(object_ref.updated_at));
    refs.truncate(limit);
    Ok(Json(MemoryGovernanceWritebackQueue {
        generated_at,
        status_filter,
        candidate_count: refs.len(),
        pending_count,
        candidates: refs,
    }))
}
