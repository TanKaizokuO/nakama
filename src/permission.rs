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
        for denied_tool in &self.rules.denied_tools {
            if denied_tool.to_lowercase() == tool_name_lower {
                return PermissionDecision::Deny("denied by denied_tools configuration".to_string());
            }
        }

        // 2. Check deny rules
        for rule in &self.rules.deny {
            if Self::match_rule(rule, &tool_name_lower, subject) {
                return PermissionDecision::Deny(format!("denied by deny rule: {}", rule));
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
        for rule in &self.rules.ask {
            if Self::match_rule(rule, &tool_name_lower, subject) {
                return PermissionDecision::Prompt(format!("matched ask rule: {}", rule));
            }
        }

        // 6. Check allow rules
        for rule in &self.rules.allow {
            if Self::match_rule(rule, &tool_name_lower, subject) {
                return PermissionDecision::Permit;
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

        if pattern.ends_with('*') {
            let prefix = &pattern[0..pattern.len() - 1];
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_matching() {
        assert!(PermissionEngine::match_rule("FileRead(*)", "fileread", "any subject"));
        assert!(PermissionEngine::match_rule("FileRead", "fileread", "any subject"));
        assert!(PermissionEngine::match_rule("FileRead(exact)", "fileread", "exact"));
        assert!(!PermissionEngine::match_rule("FileRead(exact)", "fileread", "not exact"));
        assert!(PermissionEngine::match_rule("FileRead(prefix*)", "fileread", "prefixsomething"));
        assert!(!PermissionEngine::match_rule("FileRead(prefix*)", "fileread", "notprefix"));
        assert!(PermissionEngine::match_rule("FileRead(\\(escaped\\))", "fileread", "(escaped)"));
    }

    #[test]
    fn test_extract_subject() {
        let json = serde_json::json!({ "file_path": "test.txt", "other": "val" });
        assert_eq!(PermissionEngine::extract_subject(&json), "test.txt");

        let json2 = serde_json::json!({ "command": "echo hi" });
        assert_eq!(PermissionEngine::extract_subject(&json2), "echo hi");

        let json3 = serde_json::json!("raw string");
        assert_eq!(PermissionEngine::extract_subject(&json3), "raw string");
    }

    #[test]
    fn test_evaluation_order() {
        let mut rules = PermissionRules::default();
        rules.denied_tools = vec!["BadTool".to_string()];
        rules.deny = vec!["FileWrite(/root/*)".to_string()];
        rules.allow = vec!["FileWrite(/tmp/*)".to_string()];

        let engine = PermissionEngine::new(PermissionMode::ReadOnly, rules);

        // 1. Check denied-tools list
        assert!(matches!(
            engine.evaluate("BadTool", PermissionMode::ReadOnly, "any", None),
            PermissionDecision::Deny(_)
        ));

        // 2. Check deny rules
        assert!(matches!(
            engine.evaluate("FileWrite", PermissionMode::WorkspaceWrite, "/root/test", None),
            PermissionDecision::Deny(_)
        ));

        // 4. Hook override Allow
        assert_eq!(
            engine.evaluate("FileWrite", PermissionMode::WorkspaceWrite, "/var/test", Some(HookOverride::Allow)),
            PermissionDecision::Permit
        );

        // 6. Check allow rules (Active mode is ReadOnly, so normally it would deny WorkspaceWrite, but allow rule permits it)
        assert_eq!(
            engine.evaluate("FileWrite", PermissionMode::WorkspaceWrite, "/tmp/test", None),
            PermissionDecision::Permit
        );

        // 9. Default deny (Active mode ReadOnly does not satisfy WorkspaceWrite)
        assert!(matches!(
            engine.evaluate("FileWrite", PermissionMode::WorkspaceWrite, "/other/test", None),
            PermissionDecision::Deny(_)
        ));
    }
}
