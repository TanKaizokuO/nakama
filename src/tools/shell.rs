use super::{Tool, ToolError, ToolResult, PermissionMode};
use async_trait::async_trait;
use serde::Deserialize;
use std::process::Stdio;

pub struct ShellExecute;

#[derive(Deserialize)]
struct ShellExecuteInput {
    command: String,
    timeout: Option<u64>,
    working_dir: Option<String>,
}

#[async_trait]
impl Tool for ShellExecute {
    fn name(&self) -> &'static str { "ShellExecute" }
    fn description(&self) -> &'static str { "Executes a shell command." }
    fn required_permission(&self) -> PermissionMode { PermissionMode::DangerFullAccess }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": { "type": "string" },
                "timeout": { "type": "integer" },
                "working_dir": { "type": "string" }
            },
            "required": ["command"]
        })
    }
    fn output_schema(&self) -> serde_json::Value { serde_json::json!({}) }
    async fn execute(&self, input: serde_json::Value, context: super::ToolContext) -> Result<ToolResult, ToolError> {
        let params: ShellExecuteInput = serde_json::from_value(input).map_err(|e| ToolError {
            error_type: "invalid_input".to_string(),
            message: e.to_string(),
        })?;

        let cwd = match params.working_dir {
            Some(dir) => {
                let resolved = crate::path_scope::expand_home(&crate::path_scope::expand_env_vars(&dir));
                if resolved.is_absolute() { resolved } else { context.cwd.join(resolved) }
            }
            None => {
                context.workspace_roots.first().cloned().unwrap_or_else(|| context.cwd.clone())
            }
        };

        // Validate cwd is in workspace bounds
        let val_res = crate::path_scope::validate_path(&cwd.to_string_lossy(), &context.workspace_roots, &context.cwd);
        match val_res {
            crate::path_scope::ValidationResult::Denied { reason, candidate, resolved } => {
                return Ok(ToolResult {
                    handled: false,
                    message: format!("Scope violation for working_dir: {}. Candidate: {}, Resolved: {}", reason, candidate, resolved),
                    error_type: Some("scope_violation".to_string()),
                    data: None,
                });
            }
            crate::path_scope::ValidationResult::Allowed { .. } => {}
        }

        let child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&params.command)
            .current_dir(&cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| ToolError {
                error_type: "process_spawn_error".to_string(),
                message: e.to_string(),
            })?;

        let timeout_secs = params.timeout.unwrap_or(60);
        let timeout_duration = std::time::Duration::from_secs(timeout_secs);

        let wait_result = tokio::time::timeout(timeout_duration, child.wait_with_output()).await;

        match wait_result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let exit_code = output.status.code().unwrap_or(-1);

                Ok(ToolResult {
                    handled: true,
                    message: "Command executed successfully".to_string(),
                    error_type: None,
                    data: Some(serde_json::json!({
                        "stdout": stdout,
                        "stderr": stderr,
                        "exit_code": exit_code
                    })),
                })
            }
            Ok(Err(e)) => {
                Ok(ToolResult {
                    handled: false,
                    message: format!("Process execution failed: {}", e),
                    error_type: Some("execution_error".to_string()),
                    data: None,
                })
            }
            Err(_) => {
                // Timeout
                // kill_on_drop(true) ensures the child process is killed when dropped.
                
                // Read whatever is available in stdout and stderr (but we can't do it easily after wait_with_output is moved)
                // Actually, wait_with_output takes ownership of child, so we can't read partial output easily via child struct here.
                // In a robust implementation, we would spawn tasks to read stdout/stderr simultaneously.
                // For simplicity, we just return a timeout error indicator.
                Ok(ToolResult {
                    handled: true, // We handled the command, but it timed out
                    message: "Command timed out".to_string(),
                    error_type: Some("timeout".to_string()),
                    data: Some(serde_json::json!({
                        "stdout": "[Timeout reached, partial output unavailable]",
                        "stderr": "",
                        "exit_code": -1
                    })),
                })
            }
        }
    }
}
