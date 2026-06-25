use super::{Tool, ToolError, ToolResult, PermissionMode};
use async_trait::async_trait;

pub struct FileRead;

#[async_trait]
impl Tool for FileRead {
    fn name(&self) -> &'static str { "FileRead" }
    fn description(&self) -> &'static str { "Reads a file." }
    fn required_permission(&self) -> PermissionMode { PermissionMode::ReadOnly }
    fn input_schema(&self) -> serde_json::Value { serde_json::json!({}) }
    fn output_schema(&self) -> serde_json::Value { serde_json::json!({}) }
    async fn execute(&self, _input: serde_json::Value) -> Result<ToolResult, ToolError> {
        unimplemented!()
    }
}
