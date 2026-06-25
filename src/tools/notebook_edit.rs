use super::{Tool, ToolError, ToolResult, PermissionMode};
use async_trait::async_trait;

pub struct NotebookEdit;

#[async_trait]
impl Tool for NotebookEdit {
    fn name(&self) -> &'static str { "NotebookEdit" }
    fn description(&self) -> &'static str { "Edits a notebook." }
    fn required_permission(&self) -> PermissionMode { PermissionMode::WorkspaceWrite }
    fn input_schema(&self) -> serde_json::Value { serde_json::json!({}) }
    fn output_schema(&self) -> serde_json::Value { serde_json::json!({}) }
    async fn execute(&self, _input: serde_json::Value) -> Result<ToolResult, ToolError> {
        unimplemented!()
    }
}
