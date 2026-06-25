use super::{Tool, ToolError, ToolResult, PermissionMode};
use async_trait::async_trait;

pub struct ShellExecute;

#[async_trait]
impl Tool for ShellExecute {
    fn name(&self) -> &'static str { "ShellExecute" }
    fn description(&self) -> &'static str { "Executes a shell command." }
    fn required_permission(&self) -> PermissionMode { PermissionMode::DangerFullAccess }
    fn input_schema(&self) -> serde_json::Value { serde_json::json!({}) }
    fn output_schema(&self) -> serde_json::Value { serde_json::json!({}) }
    async fn execute(&self, _input: serde_json::Value) -> Result<ToolResult, ToolError> {
        unimplemented!()
    }
}
