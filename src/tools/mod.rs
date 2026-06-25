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

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn required_permission(&self) -> PermissionMode;
    fn input_schema(&self) -> serde_json::Value;
    fn output_schema(&self) -> serde_json::Value;
    
    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult, ToolError>;
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

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            tools: HashMap::new(),
        };
        // Tools will be registered here
        registry
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }
    
    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|b| b.as_ref())
    }
}
