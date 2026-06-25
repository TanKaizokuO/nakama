use crate::permission::HookOverride;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::process::Command;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookEntry {
    #[serde(rename = "type")]
    pub event_type: String,
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectHook {
    pub matcher: Option<String>,
    pub hooks: Vec<HookEntry>,
}

#[derive(Debug, Clone)]
pub struct HookConfig {
    pub name: String,
    pub matcher: Option<String>,
    pub event_type: String,
    pub command: String,
}

pub struct HookManager {
    hooks: Vec<HookConfig>,
    timeout: Duration,
}

#[derive(Deserialize)]
struct HookOutputParse {
    override_val: Option<String>, // aliased to "override" in json
    reason: Option<String>,
}

impl HookManager {
    pub fn new(hooks_map: Option<HashMap<String, serde_json::Value>>, timeout_secs: u64) -> Self {
        let mut hooks = Vec::new();

        if let Some(map) = hooks_map {
            for (key, val) in map {
                if let Some(cmd_str) = val.as_str() {
                    // Legacy string format: key is the event type
                    hooks.push(HookConfig {
                        name: key.clone(),
                        matcher: None,
                        event_type: key,
                        command: cmd_str.to_string(),
                    });
                } else if let Ok(obj) = serde_json::from_value::<ObjectHook>(val) {
                    for entry in obj.hooks {
                        hooks.push(HookConfig {
                            name: key.clone(),
                            matcher: obj.matcher.clone(),
                            event_type: entry.event_type,
                            command: entry.command,
                        });
                    }
                }
            }
        }

        Self {
            hooks,
            timeout: Duration::from_secs(timeout_secs),
        }
    }

    fn matches(tool_name: &str, matcher: Option<&String>) -> bool {
        match matcher {
            None => true, // Omitted means matches all
            Some(pattern) => {
                let tool_lower = tool_name.to_lowercase();
                // Matcher supports case-insensitive matching with * wildcards and comma/pipe-separated alternatives.
                let alternatives: Vec<&str> = pattern.split(|c| c == ',' || c == '|').collect();
                for alt in alternatives {
                    let alt = alt.trim().to_lowercase();
                    if alt.contains('*') {
                        let prefix = alt.replace("*", ""); // simplified prefix/suffix logic
                        // In a real implementation, we would use regex or proper glob.
                        // For basic "*", if it starts with or ends with...
                        if alt.starts_with('*') && alt.ends_with('*') {
                            if tool_lower.contains(&prefix) { return true; }
                        } else if alt.ends_with('*') {
                            let prefix = &alt[..alt.len() - 1];
                            if tool_lower.starts_with(prefix) { return true; }
                        } else if alt.starts_with('*') {
                            let suffix = &alt[1..];
                            if tool_lower.ends_with(suffix) { return true; }
                        } else {
                            // wildcard in middle
                            let parts: Vec<&str> = alt.split('*').collect();
                            if parts.len() == 2 && tool_lower.starts_with(parts[0]) && tool_lower.ends_with(parts[1]) {
                                return true;
                            }
                        }
                    } else if alt == tool_lower {
                        return true;
                    }
                }
                false
            }
        }
    }

    pub async fn run_pre_tool_use(&self, tool_name: &str, tool_input: &serde_json::Value) -> Option<HookOverride> {
        let input_str = serde_json::to_string(tool_input).unwrap_or_default();
        let mut final_override = None;

        for hook in &self.hooks {
            if hook.event_type == "PreToolUse" && Self::matches(tool_name, hook.matcher.as_ref()) {
                let mut cmd = Command::new("sh");
                cmd.arg("-c").arg(&hook.command)
                   .env("TOOL_NAME", tool_name)
                   .env("TOOL_INPUT", &input_str);

                if let Ok(Ok(output)) = tokio::time::timeout(self.timeout, cmd.output()).await {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    // Try parsing as JSON
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&stdout) {
                        if let Some(obj) = parsed.as_object() {
                            if let Some(ov) = obj.get("override").and_then(|v| v.as_str()) {
                                let reason = obj.get("reason").and_then(|v| v.as_str()).map(|s| s.to_string());
                                match ov.to_lowercase().as_str() {
                                    "allow" => final_override = Some(HookOverride::Allow),
                                    "deny" => final_override = Some(HookOverride::Deny { reason }),
                                    "ask" => final_override = Some(HookOverride::Ask),
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }
        final_override
    }

    pub async fn run_post_tool_use(&self, tool_name: &str, tool_input: &serde_json::Value, result: &crate::tools::ToolResult) {
        let input_str = serde_json::to_string(tool_input).unwrap_or_default();
        let result_str = serde_json::to_string(result.data.as_ref().unwrap_or(&serde_json::json!({}))).unwrap_or_default();
        
        for hook in &self.hooks {
            if hook.event_type == "PostToolUse" && Self::matches(tool_name, hook.matcher.as_ref()) {
                let mut cmd = Command::new("sh");
                cmd.arg("-c").arg(&hook.command)
                   .env("TOOL_NAME", tool_name)
                   .env("TOOL_INPUT", &input_str)
                   .env("TOOL_RESULT", &result_str);

                let _ = tokio::time::timeout(self.timeout, cmd.output()).await;
            }
        }
    }

    pub async fn run_post_tool_use_failure(&self, tool_name: &str, tool_input: &serde_json::Value, error: &crate::tools::ToolError) {
        let input_str = serde_json::to_string(tool_input).unwrap_or_default();
        let error_str = error.message.clone();

        for hook in &self.hooks {
            if hook.event_type == "PostToolUseFailure" && Self::matches(tool_name, hook.matcher.as_ref()) {
                let mut cmd = Command::new("sh");
                cmd.arg("-c").arg(&hook.command)
                   .env("TOOL_NAME", tool_name)
                   .env("TOOL_INPUT", &input_str)
                   .env("TOOL_ERROR", &error_str);

                let _ = tokio::time::timeout(self.timeout, cmd.output()).await;
            }
        }
    }
}
