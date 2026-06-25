use super::{Tool, ToolError, ToolResult, PermissionMode};
use async_trait::async_trait;
use serde::Deserialize;
use std::path::Path;

pub struct FileRead;

#[derive(Deserialize)]
struct FileReadInput {
    file_path: String,
    start_line: Option<usize>,
    end_line: Option<usize>,
}

#[async_trait]
impl Tool for FileRead {
    fn name(&self) -> &'static str { "FileRead" }
    fn description(&self) -> &'static str { "Reads a file." }
    fn required_permission(&self) -> PermissionMode { PermissionMode::ReadOnly }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": { "type": "string" },
                "start_line": { "type": "integer" },
                "end_line": { "type": "integer" }
            },
            "required": ["file_path"]
        })
    }
    fn output_schema(&self) -> serde_json::Value { serde_json::json!({}) }
    async fn execute(&self, input: serde_json::Value, context: super::ToolContext) -> Result<ToolResult, ToolError> {
        let params: FileReadInput = serde_json::from_value(input).map_err(|e| ToolError {
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
        
        let content = match tokio::fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(e) => {
                // Return file_error
                return Ok(ToolResult {
                    handled: false,
                    message: format!("File error on path {}: {}", path.display(), e),
                    error_type: Some("file_error".to_string()),
                    data: None,
                });
            }
        };

        // Handle binary check: if content contains null byte (we read it as string, so read_to_string fails for non-utf8)
        // If it fails utf8, read_to_string returns error. Wait, we should probably read bytes first to check for binary?
        // Let's just rely on read_to_string for now. If it's not valid UTF-8, it returns InvalidData error.

        let lines: Vec<&str> = content.lines().collect();
        let mut start = params.start_line.unwrap_or(1).saturating_sub(1);
        let end = params.end_line.unwrap_or(lines.len());
        
        if start > lines.len() { start = lines.len(); }
        let end_idx = end.min(lines.len());
        
        let mut result = String::new();
        for i in start..end_idx {
            result.push_str(lines[i]);
            result.push('\n');
        }

        Ok(ToolResult {
            handled: true,
            message: result,
            error_type: None,
            data: None,
        })
    }
}
