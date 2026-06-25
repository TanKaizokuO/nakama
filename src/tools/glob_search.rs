use super::{Tool, ToolError, ToolResult, PermissionMode};
use async_trait::async_trait;

pub struct GlobSearch;

#[async_trait]
impl Tool for GlobSearch {
    fn name(&self) -> &'static str { "GlobSearch" }
    fn description(&self) -> &'static str { "Searches using glob." }
    fn required_permission(&self) -> PermissionMode { PermissionMode::ReadOnly }
    fn input_schema(&self) -> serde_json::Value { serde_json::json!({}) }
    fn output_schema(&self) -> serde_json::Value { serde_json::json!({}) }
    async fn execute(&self, _input: serde_json::Value, _context: super::ToolContext) -> Result<ToolResult, ToolError> {
        unimplemented!()
    }
}
