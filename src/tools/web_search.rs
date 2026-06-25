use super::{Tool, ToolError, ToolResult, PermissionMode};
use async_trait::async_trait;

pub struct WebSearch;

#[async_trait]
impl Tool for WebSearch {
    fn name(&self) -> &'static str { "WebSearch" }
    fn description(&self) -> &'static str { "Searches the web." }
    fn required_permission(&self) -> PermissionMode { PermissionMode::DangerFullAccess }
    fn input_schema(&self) -> serde_json::Value { serde_json::json!({}) }
    fn output_schema(&self) -> serde_json::Value { serde_json::json!({}) }
    async fn execute(&self, _input: serde_json::Value) -> Result<ToolResult, ToolError> {
        unimplemented!()
    }
}
