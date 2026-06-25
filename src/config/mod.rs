use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_aliases: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hooks: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_rules: Option<PermissionRules>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_settings: Option<ProviderSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_config: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature_flags: Option<HashMap<String, bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules_import: Option<RulesImport>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub struct PermissionRules {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deny: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ask: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub denied_tools: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub struct ProviderSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_config: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum RulesImport {
    Simple(String),
    Frameworks(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PrecedenceMetadata {
    pub file_path: String,
    pub precedence_rank: usize,
    pub wins_for_keys: Vec<String>,
    pub shadowed_keys: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct MergedConfig {
    pub config: Config,
    pub metadata: Vec<PrecedenceMetadata>,
}

pub fn find_git_root(start_dir: &Path) -> Option<PathBuf> {
    let mut current = start_dir.to_path_buf();
    loop {
        if current.join(".git").exists() {
            return Some(current);
        }
        if !current.pop() {
            break;
        }
    }
    None
}

/// Discovers the paths of the 5 configuration files in hierarchical order.
pub fn get_config_paths(cwd: &Path) -> Vec<(usize, PathBuf)> {
    let mut paths = Vec::new();

    let home = home::home_dir();
    
    // Level 1: ~/.nakama.json
    if let Some(h) = &home {
        paths.push((1, h.join(".nakama.json")));
    }

    // Level 2: ~/.config/nakama/settings.json
    if let Some(h) = &home {
        paths.push((2, h.join(".config").join("nakama").join("settings.json")));
    }

    // Determine repo root or fallback to working directory
    let repo_root = find_git_root(cwd).unwrap_or_else(|| cwd.to_path_buf());

    // Level 3: <repo>/.nakama.json
    paths.push((3, repo_root.join(".nakama.json")));

    // Level 4: <repo>/.nakama/settings.json
    paths.push((4, repo_root.join(".nakama").join("settings.json")));

    // Level 5: <repo>/.nakama/settings.local.json
    paths.push((5, repo_root.join(".nakama").join("settings.local.json")));

    paths
}

/// Loads and merges the 5-level configuration files.
pub fn load_merged_config(cwd: &Path) -> Result<MergedConfig, String> {
    let config_paths = get_config_paths(cwd);
    let mut loaded_files = Vec::new();

    for (rank, path) in config_paths {
        if path.exists() {
            let content = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read config file {}: {}", path.display(), e))?;
            
            let val = serde_json::from_str::<serde_json::Value>(&content)
                .map_err(|e| format!("config_parse_error at {}: line {} col {}", path.display(), e.line(), e.column()))?;
            
            if let serde_json::Value::Object(obj) = val {
                loaded_files.push((rank, path.display().to_string(), obj));
            } else {
                return Err(format!("config_parse_error: Config at {} is not a JSON object", path.display()));
            }
        }
    }

    // Key-level merge with metadata tracking
    let mut merged_map = serde_json::Map::new();
    let mut key_owners: HashMap<String, (usize, String)> = HashMap::new(); // key -> (rank, file_path)
    let mut key_history: HashMap<String, Vec<(usize, String)>> = HashMap::new(); // key -> list of (rank, file_path)

    for (rank, path, obj) in &loaded_files {
        for key in obj.keys() {
            key_history.entry(key.clone()).or_default().push((*rank, path.clone()));
            
            let should_update = match key_owners.get(key) {
                Some(&(old_rank, _)) => *rank > old_rank,
                None => true,
            };

            if should_update {
                key_owners.insert(key.clone(), (*rank, path.clone()));
            }
        }
    }

    // Build the merged object
    for (key, &(rank, _)) in &key_owners {
        // Find the obj with this rank and get value
        for (r, _, obj) in &loaded_files {
            if *r == rank {
                if let Some(val) = obj.get(key) {
                    merged_map.insert(key.clone(), val.clone());
                }
            }
        }
    }

    // Build metadata records
    let mut metadata = Vec::new();
    for (rank, path, obj) in &loaded_files {
        let mut wins_for_keys = Vec::new();
        let mut shadowed_keys = Vec::new();

        for key in obj.keys() {
            if let Some(&(owner_rank, _)) = key_owners.get(key) {
                if owner_rank == *rank {
                    wins_for_keys.push(key.clone());
                } else if owner_rank > *rank {
                    shadowed_keys.push(key.clone());
                }
            }
        }

        metadata.push(PrecedenceMetadata {
            file_path: path.clone(),
            precedence_rank: *rank,
            wins_for_keys,
            shadowed_keys,
        });
    }

    let config_val = serde_json::Value::Object(merged_map);
    let config = serde_json::from_value::<Config>(config_val)
        .map_err(|e| format!("Failed to convert merged JSON to Config structure: {}", e))?;

    Ok(MergedConfig { config, metadata })
}

pub mod precedence;
pub mod aliases;
