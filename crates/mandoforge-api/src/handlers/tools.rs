use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use serde_json::Value;

use crate::{
    AppError, AppState, ExecuteTool, ToolCall, ToolDescriptor,
    execute_tool as execute_tool_impl, list_tool_calls as list_tool_calls_impl,
    list_tools as list_tools_impl,
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
    list_tools_impl(state, headers).await
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
    list_tool_calls_impl(state, headers).await
}
