use super::{Tool, ToolError, ToolResult, PermissionMode};
use async_trait::async_trait;

pub struct WebFetch;

#[async_trait]
impl Tool for WebFetch {
    fn name(&self) -> &'static str { "WebFetch" }
    fn description(&self) -> &'static str { "Fetches a URL." }
    fn required_permission(&self) -> PermissionMode { PermissionMode::DangerFullAccess }
    fn input_schema(&self) -> serde_json::Value { serde_json::json!({}) }
    fn output_schema(&self) -> serde_json::Value { serde_json::json!({}) }
    async fn execute(&self, _input: serde_json::Value, _context: super::ToolContext) -> Result<ToolResult, ToolError> {
        unimplemented!()
    }
}
