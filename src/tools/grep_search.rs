use super::{Tool, ToolError, ToolResult, PermissionMode};
use async_trait::async_trait;
use serde::Deserialize;
use std::path::PathBuf;
use regex::{Regex, RegexBuilder};

pub struct GrepSearch;

#[derive(Deserialize)]
struct GrepSearchInput {
    pattern: String,
    search_path: String,
    #[serde(default)]
    regex_flag: bool,
    #[serde(default)]
    case_insensitive: bool,
    #[serde(default)]
    per_line: bool,
}

#[async_trait]
impl Tool for GrepSearch {
    fn name(&self) -> &'static str { "GrepSearch" }
    fn description(&self) -> &'static str { "Searches using grep/regex." }
    fn required_permission(&self) -> PermissionMode { PermissionMode::ReadOnly }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string" },
                "search_path": { "type": "string" },
                "regex_flag": { "type": "boolean" },
                "case_insensitive": { "type": "boolean" },
                "per_line": { "type": "boolean" }
            },
            "required": ["pattern", "search_path"]
        })
    }
    fn output_schema(&self) -> serde_json::Value { serde_json::json!({}) }
    async fn execute(&self, input: serde_json::Value, context: super::ToolContext) -> Result<ToolResult, ToolError> {
        let params: GrepSearchInput = serde_json::from_value(input).map_err(|e| ToolError {
            error_type: "invalid_input".to_string(),
            message: e.to_string(),
        })?;

        let val_res = crate::path_scope::validate_path(&params.search_path, &context.workspace_roots, &context.cwd);
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

        let resolved = crate::path_scope::expand_home(&crate::path_scope::expand_env_vars(&params.search_path));
        let base_path = if resolved.is_absolute() { resolved } else { context.cwd.join(resolved) };

        let pattern_str = if params.regex_flag {
            params.pattern.clone()
        } else {
            regex::escape(&params.pattern)
        };

        let re = match RegexBuilder::new(&pattern_str)
            .case_insensitive(params.case_insensitive)
            .build() {
            Ok(r) => r,
            Err(e) => {
                return Ok(ToolResult {
                    handled: false,
                    message: format!("Invalid regex pattern: {}", e),
                    error_type: Some("invalid_input".to_string()),
                    data: None,
                });
            }
        };

        let mut matches = Vec::new();
        // A simple recursive directory traversal
        let mut dirs = vec![base_path.clone()];
        while let Some(dir) = dirs.pop() {
            if dir.is_file() {
                // To avoid reading binary files entirely into memory or failing,
                // we'll just attempt to read_to_string.
                if let Ok(content) = std::fs::read_to_string(&dir) {
                    if params.per_line {
                        for (i, line) in content.lines().enumerate() {
                            if re.is_match(line) {
                                matches.push(serde_json::json!({
                                    "file_path": dir.to_string_lossy().to_string(),
                                    "line_number": i + 1,
                                    "line_content": line
                                }));
                            }
                        }
                    } else if re.is_match(&content) {
                        matches.push(serde_json::json!({
                            "file_path": dir.to_string_lossy().to_string()
                        }));
                    }
                }
            } else if dir.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        // Validate each child path to be safe, though parent is validated.
                        if let crate::path_scope::ValidationResult::Allowed { .. } = crate::path_scope::validate_path(&path.to_string_lossy(), &context.workspace_roots, &context.cwd) {
                            dirs.push(path);
                        }
                    }
                }
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
