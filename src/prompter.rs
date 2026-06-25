use std::io::{self, Write};
use tokio::task;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptResponse {
    Allow,
    AllowAlways,
    Deny,
}

pub struct InteractivePrompter;

impl InteractivePrompter {
    pub async fn prompt(
        tool_name: &str,
        tool_description: &str,
        tool_input: &serde_json::Value,
        reason: &str,
        is_interactive: bool,
    ) -> Result<PromptResponse, String> {
        if !is_interactive {
            return Err("interactive prompt required but session is non-interactive".to_string());
        }

        let tool_name = tool_name.to_string();
        let tool_description = tool_description.to_string();
        let tool_input_str = serde_json::to_string_pretty(tool_input).unwrap_or_default();
        let reason = reason.to_string();

        task::spawn_blocking(move || {
            let mut stdout = io::stdout();
            writeln!(stdout, "--- Tool Execution Approval Required ---").unwrap();
            writeln!(stdout, "Tool: {} - {}", tool_name, tool_description).unwrap();
            writeln!(stdout, "Reason: {}", reason).unwrap();
            writeln!(stdout, "Input Payload:\n{}", tool_input_str).unwrap();
            writeln!(stdout, "Do you want to allow this execution?").unwrap();
            writeln!(stdout, "  1) Allow once").unwrap();
            writeln!(stdout, "  2) Allow always (this session)").unwrap();
            writeln!(stdout, "  3) Deny").unwrap();
            write!(stdout, "Enter choice (1/2/3): ").unwrap();
            stdout.flush().unwrap();

            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_ok() {
                let choice = input.trim();
                match choice {
                    "1" | "allow" | "y" | "yes" => Ok(PromptResponse::Allow),
                    "2" | "always" | "allow always" => Ok(PromptResponse::AllowAlways),
                    _ => Ok(PromptResponse::Deny), // Default to Deny for safety
                }
            } else {
                Err("Failed to read from stdin".to_string())
            }
        })
        .await
        .unwrap_or_else(|e| Err(e.to_string()))
    }
}
