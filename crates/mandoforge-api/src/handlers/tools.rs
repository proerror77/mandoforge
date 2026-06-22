use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use serde_json::Value;

use crate::{
    AppError, AppState, ExecuteTool, Permission, ToolCall, ToolDescriptor,
    authorize_collection_request, authorize_request, execute_tool as execute_tool_impl,
    tool_descriptors, visible_session_ids_for_principal,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/tools", get(list_tools))
        .route("/api/tools/{name}/execute", post(execute_tool))
        .route("/api/tool-calls", get(list_tool_calls))
}

async fn list_tools(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ToolDescriptor>>, AppError> {
    authorize_request(&state, &headers, Permission::AgentsRead, "tools", None).await?;
    Ok(Json(tool_descriptors()))
}

async fn execute_tool(
    State(state): State<AppState>,
    Path(name): Path<String>,
    headers: HeaderMap,
    Json(input): Json<ExecuteTool>,
) -> Result<Json<Value>, AppError> {
    execute_tool_impl(state, name, headers, input).await
}

async fn list_tool_calls(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ToolCall>>, AppError> {
    let principal =
        authorize_collection_request(&state, &headers, Permission::SessionsRead, "tool_calls")
            .await?;
    let visible_session_ids = visible_session_ids_for_principal(&state, &principal).await?;
    Ok(Json(
        state
            .list_tool_calls(None)
            .await?
            .into_iter()
            .filter(|tool_call| visible_session_ids.contains(&tool_call.session_id))
            .collect(),
    ))
}
