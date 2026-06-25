use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct HealthCheckResult {
    pub name: String,
    pub status: String,
    pub detail: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VersionInfoResult {
    pub git_sha: String,
    pub git_sha_short: String,
    pub is_dirty: bool,
    pub branch: String,
    pub commit_date: String,
    pub rustc_version: String,
    pub executable_path: String,
    pub binary_provenance: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StatusReportResult {
    pub workspace: String,
    pub model: String,
    pub permissions: String,
    pub memory_files: Vec<String>,
    pub mcp_validation: String,
    pub hook_validation: String,
    pub allowed_tools: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum ContainerEnvironment {
    Docker,
    Kubernetes,
    Codespaces,
    Gitpod,
    Wsl,
    None,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SandboxInfoResult {
    pub container_environment: ContainerEnvironment,
    pub filesystem_isolation_mode: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InitWorkspaceResult {
    pub created: Vec<String>,
    pub updated: Vec<String>,
    pub partial: Vec<String>,
    pub deferred: Vec<String>,
    pub skipped: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ToolDefinition {
    pub name: String,
    pub permission_level: String, // Matches Phase 3 permission_mode enum values
    pub source: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DumpManifestsResult {
    pub commands: Vec<CommandDef>,
    pub tools: Vec<ToolDefinition>,
    pub agents: Vec<String>,
    pub skills: Vec<String>,
    pub bootstrap_phases: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CommandDef {
    pub name: String,
    pub source: String,
    pub kind: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SystemPromptResult {
    pub system_prompt: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AgentMetadata {
    pub name: String,
    pub description: String,
    pub model: String,
    pub tools: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct McpInspectResult {
    pub servers: Vec<ServerInfo>,
    pub invalid_servers: Vec<InvalidServerInfo>,
    pub total_configured: u32,
    pub valid_count: u32,
    pub invalid_count: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ServerInfo {
    pub name: String,
    pub transport_type: String,
    pub tool_count: u32,
    pub lifecycle_phase: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InvalidServerInfo {
    pub name: String,
    pub error_field: String,
    pub reason: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SkillsInspectResult {
    pub name: String,
    pub source: String,
    pub installed: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BootstrapPlanResult {
    pub phases: Vec<String>, // Matches Phase 5 BootstrapGraph format
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WorkerStateResult {
    pub worker_id: String,
    pub session_id: String,
    pub model: String,
    pub permission_mode: String,
}
