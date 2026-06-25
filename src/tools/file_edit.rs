use super::{Tool, ToolError, ToolResult, PermissionMode};
use async_trait::async_trait;
use serde::Deserialize;

pub struct FileEdit;

#[derive(Deserialize)]
struct FileEditHunk {
    old_text: String,
    new_text: String,
}

#[derive(Deserialize)]
struct FileEditInput {
    file_path: String,
    hunks: Vec<FileEditHunk>,
}

#[async_trait]
impl Tool for FileEdit {
    fn name(&self) -> &'static str { "FileEdit" }
    fn description(&self) -> &'static str { "Edits a file." }
    fn required_permission(&self) -> PermissionMode { PermissionMode::WorkspaceWrite }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": { "type": "string" },
                "hunks": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "old_text": { "type": "string" },
                            "new_text": { "type": "string" }
                        },
                        "required": ["old_text", "new_text"]
                    }
                }
            },
            "required": ["file_path", "hunks"]
        })
    }
    fn output_schema(&self) -> serde_json::Value { serde_json::json!({}) }
    async fn execute(&self, input: serde_json::Value, context: super::ToolContext) -> Result<ToolResult, ToolError> {
        let params: FileEditInput = serde_json::from_value(input).map_err(|e| ToolError {
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
        
        let mut content = match tokio::fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(e) => {
                return Ok(ToolResult {
                    handled: false,
                    message: format!("File error on path {}: {}", path.display(), e),
                    error_type: Some("file_error".to_string()),
                    data: None,
                });
            }
        };

        for hunk in params.hunks {
            if !content.contains(&hunk.old_text) {
                return Ok(ToolResult {
                    handled: false,
                    message: format!("Hunk failed: expected text not found in file. Expected: {:?}", hunk.old_text),
                    error_type: Some("file_error".to_string()),
                    data: None,
                });
            }
            content = content.replacen(&hunk.old_text, &hunk.new_text, 1);
        }

        match tokio::fs::write(&path, content).await {
            Ok(_) => {
                Ok(ToolResult {
                    handled: true,
                    message: format!("Successfully edited {}", params.file_path),
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
