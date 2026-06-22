use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::get,
};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    AgentTeammate, AppError, AppState, CreateAgentTeammate, CreateSquad, CreateSquadMember,
    CreateWorkItem, CreateWorkItemAssignment, CreateWorkItemReview, Squad, SquadMember, WorkItem,
    WorkItemActivityEntry, WorkItemAssignment, WorkItemReview,
    add_squad_member as add_squad_member_impl,
    create_agent_teammate as create_agent_teammate_impl, create_squad as create_squad_impl,
    create_work_item as create_work_item_impl,
    create_work_item_assignment as create_work_item_assignment_impl,
    create_work_item_review as create_work_item_review_impl,
    get_capability_discovery as get_capability_discovery_impl,
    list_agent_teammates as list_agent_teammates_impl,
    list_squad_members as list_squad_members_impl, list_squads as list_squads_impl,
    list_work_item_activity as list_work_item_activity_impl,
    list_work_item_assignments as list_work_item_assignments_impl,
    list_work_item_reviews as list_work_item_reviews_impl, list_work_items as list_work_items_impl,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/work-items",
            get(list_work_items).post(create_work_item),
        )
        .route(
            "/api/agent-teammates",
            get(list_agent_teammates).post(create_agent_teammate),
        )
        .route("/api/squads", get(list_squads).post(create_squad))
        .route(
            "/api/squads/{id}/members",
            get(list_squad_members).post(add_squad_member),
        )
        .route(
            "/api/work-items/{id}/assignments",
            get(list_work_item_assignments).post(create_work_item_assignment),
        )
        .route(
            "/api/work-items/{id}/reviews",
            get(list_work_item_reviews).post(create_work_item_review),
        )
        .route(
            "/api/work-items/{id}/activity",
            get(list_work_item_activity),
        )
        .route("/api/capability-discovery", get(get_capability_discovery))
}

async fn list_work_items(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<WorkItem>>, AppError> {
    list_work_items_impl(state, headers).await
}

async fn create_work_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateWorkItem>,
) -> Result<Json<WorkItem>, AppError> {
    create_work_item_impl(state, headers, input).await
}

async fn list_agent_teammates(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AgentTeammate>>, AppError> {
    list_agent_teammates_impl(state, headers).await
}

async fn create_agent_teammate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateAgentTeammate>,
) -> Result<Json<AgentTeammate>, AppError> {
    create_agent_teammate_impl(state, headers, input).await
}

async fn list_squads(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Squad>>, AppError> {
    list_squads_impl(state, headers).await
}

async fn create_squad(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateSquad>,
) -> Result<Json<Squad>, AppError> {
    create_squad_impl(state, headers, input).await
}

async fn list_squad_members(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<SquadMember>>, AppError> {
    list_squad_members_impl(state, id, headers).await
}

async fn add_squad_member(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateSquadMember>,
) -> Result<Json<SquadMember>, AppError> {
    add_squad_member_impl(state, id, headers, input).await
}

async fn list_work_item_assignments(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<WorkItemAssignment>>, AppError> {
    list_work_item_assignments_impl(state, id, headers).await
}

async fn create_work_item_assignment(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateWorkItemAssignment>,
) -> Result<Json<WorkItemAssignment>, AppError> {
    create_work_item_assignment_impl(state, id, headers, input).await
}

async fn list_work_item_reviews(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<WorkItemReview>>, AppError> {
    list_work_item_reviews_impl(state, id, headers).await
}

async fn create_work_item_review(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateWorkItemReview>,
) -> Result<Json<WorkItemReview>, AppError> {
    create_work_item_review_impl(state, id, headers, input).await
}

async fn list_work_item_activity(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<WorkItemActivityEntry>>, AppError> {
    list_work_item_activity_impl(state, id, headers).await
}

async fn get_capability_discovery(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    get_capability_discovery_impl(state, headers).await
}
