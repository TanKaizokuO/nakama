use super::{Tool, ToolError, ToolResult, PermissionMode};
use async_trait::async_trait;

pub struct AgentLaunch;

#[async_trait]
impl Tool for AgentLaunch {
    fn name(&self) -> &'static str { "AgentLaunch" }
    fn description(&self) -> &'static str { "Launches a sub-agent." }
    fn required_permission(&self) -> PermissionMode { PermissionMode::DangerFullAccess }
    fn input_schema(&self) -> serde_json::Value { serde_json::json!({}) }
    fn output_schema(&self) -> serde_json::Value { serde_json::json!({}) }
    async fn execute(&self, _input: serde_json::Value) -> Result<ToolResult, ToolError> {
        Err(ToolError {
            error_type: "NotYetImplemented".to_string(),
            message: "AgentLaunch requires Phase 3 session runtime which is not yet implemented.".to_string(),
        })
    }
}
