use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The canonical permission levels from §3.5
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PermissionMode {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
    Prompt,
    Allow,
}

#[derive(Debug, Clone)]
pub struct ToolError {
    pub error_type: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct ToolResult {
    pub handled: bool,
    pub message: String,
    pub error_type: Option<String>,
    pub data: Option<serde_json::Value>,
}

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ToolContext {
    pub workspace_roots: Vec<PathBuf>,
    pub cwd: PathBuf,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn required_permission(&self) -> PermissionMode;
    fn input_schema(&self) -> serde_json::Value;
    fn output_schema(&self) -> serde_json::Value;
    
    async fn execute(&self, input: serde_json::Value, context: ToolContext) -> Result<ToolResult, ToolError>;
}

pub mod shell;
pub mod file_read;
pub mod file_write;
pub mod file_edit;
pub mod glob_search;
pub mod grep_search;
pub mod web_search;
pub mod web_fetch;
pub mod agent_launch;
pub mod todo_write;
pub mod notebook_edit;
pub mod skill_invoke;
pub mod tool_search;
pub mod dispatch;

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            tools: HashMap::new(),
        };
        
        registry.register(Box::new(shell::ShellExecute));
        registry.register(Box::new(file_read::FileRead));
        registry.register(Box::new(file_write::FileWrite));
        registry.register(Box::new(file_edit::FileEdit));
        registry.register(Box::new(glob_search::GlobSearch));
        registry.register(Box::new(grep_search::GrepSearch));
        registry.register(Box::new(web_search::WebSearch));
        registry.register(Box::new(web_fetch::WebFetch));
        registry.register(Box::new(agent_launch::AgentLaunch));
        registry.register(Box::new(todo_write::TodoWrite));
        registry.register(Box::new(notebook_edit::NotebookEdit));
        registry.register(Box::new(skill_invoke::SkillInvoke));
        registry.register(Box::new(tool_search::ToolSearch));
        
        registry
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }
    
    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|b| b.as_ref())
    }
}
