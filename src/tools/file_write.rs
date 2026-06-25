use super::{Tool, ToolError, ToolResult, PermissionMode};
use async_trait::async_trait;
use serde::Deserialize;

pub struct FileWrite;

#[derive(Deserialize)]
struct FileWriteInput {
    file_path: String,
    content: String,
}

#[async_trait]
impl Tool for FileWrite {
    fn name(&self) -> &'static str { "FileWrite" }
    fn description(&self) -> &'static str { "Writes to a file." }
    fn required_permission(&self) -> PermissionMode { PermissionMode::WorkspaceWrite }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": { "type": "string" },
                "content": { "type": "string" }
            },
            "required": ["file_path", "content"]
        })
    }
    fn output_schema(&self) -> serde_json::Value { serde_json::json!({}) }
    async fn execute(&self, input: serde_json::Value, context: super::ToolContext) -> Result<ToolResult, ToolError> {
        let params: FileWriteInput = serde_json::from_value(input).map_err(|e| ToolError {
            error_type: "invalid_input".to_string(),
            message: e.to_string(),
        })?;

        let val_res = crate::path_scope::validate_path(&params.file_path, &context.workspace_roots, &context.cwd);
        match val_res {
            crate::path_scope::ValidationResult::Denied { reason, candidate, resolved } => {
                return Ok(ToolResult {
                    handled: false,
                    message: format!("Scope violation: {}. Candidate: {}, Resolved: {}", reason, candidate, resolved),
                    error_type: Some("scope_violation".to_string()),
                    data: None,
                });
            }
            crate::path_scope::ValidationResult::Allowed { .. } => {}
        }

        let resolved = crate::path_scope::expand_home(&crate::path_scope::expand_env_vars(&params.file_path));
        let path = if resolved.is_absolute() { resolved } else { context.cwd.join(resolved) };
        
        if let Some(parent) = path.parent() {
            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                return Ok(ToolResult {
                    handled: false,
                    message: format!("Failed to create parent directories for {}: {}", path.display(), e),
                    error_type: Some("file_error".to_string()),
                    data: None,
                });
            }
        }

        match tokio::fs::write(&path, params.content).await {
            Ok(_) => {
                Ok(ToolResult {
                    handled: true,
                    message: format!("Successfully wrote to {}", params.file_path),
                    error_type: None,
                    data: None,
                })
            },
            Err(e) => {
                Ok(ToolResult {
                    handled: false,
                    message: format!("File error on path {}: {}", path.display(), e),
                    error_type: Some("file_error".to_string()),
                    data: None,
                })
            }
        }
    }
}
