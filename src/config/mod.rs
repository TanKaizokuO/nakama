use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PermissionRules {
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
    #[serde(default)]
    pub ask: Vec<String>,
    #[serde(default)]
    pub denied_tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RulesImportMode {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "none")]
    None,
    #[serde(untagged)]
    Explicit(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub model_aliases: HashMap<String, String>,
    
    #[serde(default)]
    pub mcp_servers: HashMap<String, serde_json::Value>,
    
    #[serde(default)]
    pub hooks: HashMap<String, serde_json::Value>,
    
    #[serde(default)]
    pub permission_rules: Option<PermissionRules>,
    
    #[serde(default)]
    pub provider_settings: Option<serde_json::Value>,
    
    #[serde(default)]
    pub feature_flags: Option<serde_json::Value>,
    
    #[serde(default)]
    pub rules_import: Option<RulesImportMode>,

    #[serde(skip)]
    pub instruction_content: Option<String>,
}

impl AppConfig {
    pub fn merge(&mut self, other: AppConfig) {
        self.model_aliases.extend(other.model_aliases);
        self.mcp_servers.extend(other.mcp_servers);
        self.hooks.extend(other.hooks);

        if let Some(other_rules) = other.permission_rules {
            if let Some(mut my_rules) = self.permission_rules.take() {
                my_rules.allow.extend(other_rules.allow);
                my_rules.deny.extend(other_rules.deny);
                my_rules.ask.extend(other_rules.ask);
                my_rules.denied_tools.extend(other_rules.denied_tools);
                self.permission_rules = Some(my_rules);
            } else {
                self.permission_rules = Some(other_rules);
            }
        }

        if other.provider_settings.is_some() {
            self.provider_settings = other.provider_settings;
        }

        if other.feature_flags.is_some() {
            self.feature_flags = other.feature_flags;
        }

        if other.rules_import.is_some() {
            self.rules_import = other.rules_import;
        }
    }
}

pub fn load_merged_config(workspace_root: &Path) -> AppConfig {
    let mut config = AppConfig::default();

    let mut paths = Vec::new();
    
    // 1. ~/.claw.json
    // 2. ~/.config/claw/settings.json
    if let Some(home) = home::home_dir() {
        paths.push(home.join(".claw.json"));
        paths.push(home.join(".config/claw/settings.json"));
    }

    // 3. <repo>/.claw.json
    // 4. <repo>/.claw/settings.json
    // 5. <repo>/.claw/settings.local.json
    paths.push(workspace_root.join(".claw.json"));
    paths.push(workspace_root.join(".claw/settings.json"));
    paths.push(workspace_root.join(".claw/settings.local.json"));

    for path in paths {
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(parsed) = serde_json::from_str::<AppConfig>(&content) {
                    config.merge(parsed);
                }
            }
        }
    }

    // Discover instructions
    config.instruction_content = discover_instructions(workspace_root);

    config
}

fn discover_instructions(workspace_root: &Path) -> Option<String> {
    let mut instructions = Vec::new();

    // The order determines priority
    let explicit_paths = vec![
        "CLAUDE.md",
        "CLAW.md",
        "AGENTS.md",
        ".claw/CLAUDE.md",
        ".claude/CLAUDE.md",
        ".claw/instructions.md",
    ];

    for path_str in explicit_paths {
        let path = workspace_root.join(path_str);
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                instructions.push(content);
            }
        }
    }

    // 5. Sorted files from .claw/rules/
    let mut rules_files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(workspace_root.join(".claw/rules")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if ext == "md" || ext == "txt" || ext == "mdc" {
                        rules_files.push(path);
                    }
                }
            }
        }
    }
    rules_files.sort();
    for path in rules_files {
        if let Ok(content) = std::fs::read_to_string(&path) {
            instructions.push(content);
        }
    }

    // 6. Sorted files from .claw/rules.local/
    let mut local_rules_files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(workspace_root.join(".claw/rules.local")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if ext == "md" || ext == "txt" || ext == "mdc" {
                        local_rules_files.push(path);
                    }
                }
            }
        }
    }
    local_rules_files.sort();
    for path in local_rules_files {
        if let Ok(content) = std::fs::read_to_string(&path) {
            instructions.push(content);
        }
    }

    if instructions.is_empty() {
        None
    } else {
        Some(instructions.join("\n---\n"))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    #[test]
    fn test_merge() {
        let config = load_merged_config(&PathBuf::from("."));
        println!("{:?}", config.permission_rules.unwrap().denied_tools);
    }
}
