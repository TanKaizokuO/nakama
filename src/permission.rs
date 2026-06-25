use crate::tools::PermissionMode;
use crate::config::PermissionRules;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookOverride {
    Allow,
    Deny { reason: Option<String> },
    Ask,
}

#[derive(Debug, Clone)]
pub struct PermissionEngine {
    mode: PermissionMode,
    rules: PermissionRules,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionDecision {
    Permit,
    Deny(String),
    Prompt(String), // We return Prompt so the caller can trigger the interactive prompter
}

impl PermissionEngine {
    pub fn new(mode: PermissionMode, rules: PermissionRules) -> Self {
        Self { mode, rules }
    }

    pub fn mode(&self) -> PermissionMode {
        self.mode
    }

    pub fn evaluate(
        &self,
        tool_name: &str,
        required_mode: PermissionMode,
        subject: &str,
        hook_override: Option<HookOverride>,
    ) -> PermissionDecision {
        let tool_name_lower = tool_name.to_lowercase();

        // 1. Check denied-tools list
        if let Some(denied) = &self.rules.denied_tools {
            for denied_tool in denied {
                if denied_tool.to_lowercase() == tool_name_lower {
                    return PermissionDecision::Deny("denied by denied_tools configuration".to_string());
                }
            }
        }

        // 2. Check deny rules
        if let Some(deny_rules) = &self.rules.deny {
            for rule in deny_rules {
                if Self::match_rule(rule, &tool_name_lower, subject) {
                    return PermissionDecision::Deny(format!("denied by deny rule: {}", rule));
                }
            }
        }

        // 3. Determine required mode (Passed as argument, no-op here)
        
        // 4. Apply hook overrides
        let has_allow_override = matches!(hook_override, Some(HookOverride::Allow));
        if let Some(hook_res) = hook_override {
            match hook_res {
                HookOverride::Deny { reason } => {
                    return PermissionDecision::Deny(reason.unwrap_or_else(|| "denied by PreToolUse hook".to_string()));
                }
                HookOverride::Ask => {
                    return PermissionDecision::Prompt("PreToolUse hook requires interactive approval".to_string());
                }
                HookOverride::Allow => {
                    // Proceed to step 5 but we know it's allowed unless Ask rules trigger or we override later.
                    // Actually, the spec says: "Allow -> proceed to the next step (but still honor ask rules in step 5)."
                    // If no Ask rules trigger, does it bypass the mode check (step 7) and permit?
                    // Let's assume an explicit Allow override means we permit after step 5 and 6, bypassing 7 and 9.
                    // We'll track it using a boolean flag.
                }
            }
        }

        // 5. Check ask rules
        if let Some(ask_rules) = &self.rules.ask {
            for rule in ask_rules {
                if Self::match_rule(rule, &tool_name_lower, subject) {
                    return PermissionDecision::Prompt(format!("matched ask rule: {}", rule));
                }
            }
        }

        // 6. Check allow rules
        if let Some(allow_rules) = &self.rules.allow {
            for rule in allow_rules {
                if Self::match_rule(rule, &tool_name_lower, subject) {
                    return PermissionDecision::Permit;
                }
            }
        }

        if has_allow_override {
            return PermissionDecision::Permit;
        }

        // 7. Compare modes
        if self.mode == PermissionMode::Allow {
            return PermissionDecision::Permit;
        }

        if self.mode >= required_mode {
            return PermissionDecision::Permit;
        }

        // 8. Prompt for escalation
        if self.mode == PermissionMode::Prompt {
            return PermissionDecision::Prompt("Prompt mode requires approval".to_string());
        }

        if self.mode == PermissionMode::WorkspaceWrite && required_mode == PermissionMode::DangerFullAccess {
            return PermissionDecision::Prompt("Tool requires DangerFullAccess but session is WorkspaceWrite".to_string());
        }

        // 9. Default deny
        PermissionDecision::Deny(format!("insufficient permission: active mode '{:?}' does not satisfy required mode '{:?}'", self.mode, required_mode))
    }

    /// Implements rule matching syntax `ToolName(subject_pattern)`
    pub fn match_rule(rule: &str, tool_name_lower: &str, subject: &str) -> bool {
        // Parse ToolName(pattern)
        let parts: Vec<&str> = rule.splitn(2, '(').collect();
        let rule_tool = parts[0].to_lowercase();
        
        if rule_tool != tool_name_lower {
            return false;
        }

        if parts.len() == 1 {
            // ToolName alone matches any invocation
            return true;
        }

        let mut pattern = parts[1].to_string();
        if pattern.ends_with(')') {
            pattern.pop();
        }

        // Parse subject pattern:
        // `*` -> wildcard
        // `exact_value` -> exact match
        // `prefix:*` -> prefix match
        // Escaped parens `\(with\)` -> `(with)`

        let pattern = pattern.replace("\\(", "(").replace("\\)", ")");

        if pattern == "*" {
            return true;
        }

        if pattern.ends_with(":*") {
            let prefix = &pattern[0..pattern.len() - 2];
            return subject.starts_with(prefix);
        }

        subject == pattern
    }

    /// Extracts subject from well-known JSON keys
    pub fn extract_subject(input: &serde_json::Value) -> String {
        if let serde_json::Value::Object(map) = input {
            let keys = [
                "command", "path", "file_path", "filePath", 
                "notebook_path", "notebookPath", "url", 
                "pattern", "code", "message"
            ];

            for key in keys {
                if let Some(val) = map.get(key) {
                    if let serde_json::Value::String(s) = val {
                        return s.clone();
                    } else {
                        return val.to_string();
                    }
                }
            }
            // No key found, use serialized JSON string
            return serde_json::to_string(input).unwrap_or_default();
        }

        // Not valid JSON object, use raw string if possible or serialize
        if let serde_json::Value::String(s) = input {
            return s.clone();
        }

        serde_json::to_string(input).unwrap_or_default()
    }
}
