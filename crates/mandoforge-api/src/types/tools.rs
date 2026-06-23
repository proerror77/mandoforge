use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub(crate) struct ToolDescriptor {
    pub(crate) name: &'static str,
    pub(crate) risk: &'static str,
    pub(crate) description: &'static str,
}

pub(crate) struct FileReadTool;
pub(crate) struct SqlSchemaTool;
pub(crate) struct SqlQueryTool;
pub(crate) struct ShellExecTool;
pub(crate) struct ArtifactCreateTool;
pub(crate) struct ApprovalRequestTool;
pub(crate) struct McpCallTool;
pub(crate) struct SemanticObjectFetchTool;
pub(crate) struct SemanticObjectSearchTool;
pub(crate) struct SemanticLinkExpandTool;
pub(crate) struct OntologyTypeLookupTool;

#[derive(Debug, Deserialize)]
pub(crate) struct ExecuteTool {
    pub(crate) session_id: Uuid,
    #[serde(default)]
    pub(crate) task_grant_id: Option<Uuid>,
    pub(crate) args: Value,
}
