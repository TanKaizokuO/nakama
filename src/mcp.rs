use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum McpServerState {
    Discovery,
    Validated,
    Spawned,
    Initialized,
    ToolsDiscovered,
    Ready,
    Failed,
    Degraded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransportType {
    Stdio,
    Websocket,
    Remote,
    Sdk,
    ManagedProxy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidServerRecord {
    pub server_name: String,
    pub error_field: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct McpServer {
    pub name: String,
    pub state: McpServerState,
    pub transport: TransportType,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub url: Option<String>,
}

pub struct McpManager {
    pub servers: HashMap<String, McpServer>,
    pub invalid_servers: Vec<InvalidServerRecord>,
}

impl McpManager {
    pub fn new() -> Self {
        Self {
            servers: HashMap::new(),
            invalid_servers: Vec::new(),
        }
    }

    pub fn discover_from_config(&mut self, mcp_servers: &Option<HashMap<String, serde_json::Value>>) {
        if let Some(map) = mcp_servers {
            for (name, val) in map {
                let transport_str = val.get("transport").and_then(|v| v.as_str()).unwrap_or("stdio");
                
                let transport = match transport_str.to_lowercase().as_str() {
                    "stdio" => TransportType::Stdio,
                    "websocket" => TransportType::Websocket,
                    "remote" => TransportType::Remote,
                    "sdk" => TransportType::Sdk,
                    "managed_proxy" => TransportType::ManagedProxy,
                    _ => {
                        self.invalid_servers.push(InvalidServerRecord {
                            server_name: name.clone(),
                            error_field: "transport".to_string(),
                            reason: format!("Unknown transport type: {}", transport_str),
                        });
                        continue;
                    }
                };

                let command = val.get("command").and_then(|v| v.as_str()).map(|s| s.to_string());
                let url = val.get("url").and_then(|v| v.as_str()).map(|s| s.to_string());
                
                if transport == TransportType::Stdio && command.is_none() {
                    self.invalid_servers.push(InvalidServerRecord {
                        server_name: name.clone(),
                        error_field: "command".to_string(),
                        reason: "command is required for stdio transport".to_string(),
                    });
                    continue;
                }

                if matches!(transport, TransportType::Websocket | TransportType::Remote | TransportType::ManagedProxy) && url.is_none() {
                    self.invalid_servers.push(InvalidServerRecord {
                        server_name: name.clone(),
                        error_field: "url".to_string(),
                        reason: "url is required for network transports".to_string(),
                    });
                    continue;
                }

                let mut args = Vec::new();
                if let Some(arr) = val.get("args").and_then(|v| v.as_array()) {
                    for a in arr {
                        if let Some(s) = a.as_str() {
                            args.push(s.to_string());
                        }
                    }
                }

                let mut env = HashMap::new();
                if let Some(e) = val.get("env").and_then(|v| v.as_object()) {
                    for (k, v) in e {
                        if let Some(s) = v.as_str() {
                            env.insert(k.clone(), s.to_string());
                        }
                    }
                }

                let server = McpServer {
                    name: name.clone(),
                    state: McpServerState::Validated, // Transition from Discovery -> Validated
                    transport,
                    command,
                    args,
                    env,
                    url,
                };
                
                self.servers.insert(name.clone(), server);
            }
        }
    }

    pub async fn spawn_and_initialize(&mut self) {
        for server in self.servers.values_mut() {
            if server.state == McpServerState::Validated {
                // Here we would spawn process or connect socket
                server.state = McpServerState::Spawned;
                // Then send initialize request
                server.state = McpServerState::Initialized;
                // Then tools/list
                server.state = McpServerState::ToolsDiscovered;
                // Then ready
                server.state = McpServerState::Ready;
                // Or if fails, set to Failed or Degraded
            }
        }
    }
}
