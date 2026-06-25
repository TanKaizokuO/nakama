use super::{Tool, ToolError, ToolResult, PermissionMode};
use async_trait::async_trait;

pub struct SkillInvoke;

#[async_trait]
impl Tool for SkillInvoke {
    fn name(&self) -> &'static str { "SkillInvoke" }
    fn description(&self) -> &'static str { "Invokes a skill." }
    fn required_permission(&self) -> PermissionMode { PermissionMode::ReadOnly }
    fn input_schema(&self) -> serde_json::Value { serde_json::json!({}) }
    fn output_schema(&self) -> serde_json::Value { serde_json::json!({}) }
    async fn execute(&self, _input: serde_json::Value) -> Result<ToolResult, ToolError> {
        unimplemented!()
    }
}
