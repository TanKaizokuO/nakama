use super::{Tool, ToolError, ToolResult, PermissionMode};
use async_trait::async_trait;
use serde::Deserialize;

pub struct GlobSearch;

#[derive(Deserialize)]
struct GlobSearchInput {
    pattern: String,
    root_dir: Option<String>,
}

#[async_trait]
impl Tool for GlobSearch {
    fn name(&self) -> &'static str { "GlobSearch" }
    fn description(&self) -> &'static str { "Searches using glob." }
    fn required_permission(&self) -> PermissionMode { PermissionMode::ReadOnly }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string" },
                "root_dir": { "type": "string" }
            },
            "required": ["pattern"]
        })
    }
    fn output_schema(&self) -> serde_json::Value { serde_json::json!({}) }
    async fn execute(&self, input: serde_json::Value, context: super::ToolContext) -> Result<ToolResult, ToolError> {
        let params: GlobSearchInput = serde_json::from_value(input).map_err(|e| ToolError {
            error_type: "invalid_input".to_string(),
            message: e.to_string(),
        })?;

        let root_dir = params.root_dir.unwrap_or_else(|| {
            context.workspace_roots.first().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| ".".to_string())
        });

        let val_res = crate::path_scope::validate_path(&root_dir, &context.workspace_roots, &context.cwd);
        match val_res {
            crate::path_scope::ValidationResult::Denied { reason, candidate, resolved } => {
                return Ok(ToolResult {
                    handled: false,
                    message: format!("Scope violation for root_dir: {}. Candidate: {}, Resolved: {}", reason, candidate, resolved),
                    error_type: Some("scope_violation".to_string()),
                    data: None,
                });
            }
            crate::path_scope::ValidationResult::Allowed { .. } => {}
        }

        let resolved_root = crate::path_scope::expand_home(&crate::path_scope::expand_env_vars(&root_dir));
        let base_path = if resolved_root.is_absolute() { resolved_root } else { context.cwd.join(resolved_root) };
        let glob_path = base_path.join(&params.pattern).to_string_lossy().to_string();

        let mut matches = Vec::new();
        match glob::glob(&glob_path) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    if let crate::path_scope::ValidationResult::Allowed { .. } = crate::path_scope::validate_path(&entry.to_string_lossy(), &context.workspace_roots, &context.cwd) {
                        matches.push(entry.to_string_lossy().to_string());
                    }
                }
            }
            Err(e) => {
                return Ok(ToolResult {
                    handled: false,
                    message: format!("Invalid glob pattern: {}", e),
                    error_type: Some("invalid_input".to_string()),
                    data: None,
                });
            }
        }

        Ok(ToolResult {
            handled: true,
            message: format!("Found {} matches", matches.len()),
            error_type: None,
            data: Some(serde_json::json!({ "matches": matches })),
        })
    }
}
