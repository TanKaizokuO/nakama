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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mcp_lifecycle() {
        let mut manager = McpManager::new();
        
        let mut mcp_servers = HashMap::new();
        
        // Invalid stdio server (missing command)
        mcp_servers.insert("invalid_stdio".to_string(), serde_json::json!({
            "transport": "stdio"
        }));
        
        // Valid stdio server
        mcp_servers.insert("valid_stdio".to_string(), serde_json::json!({
            "transport": "stdio",
            "command": "python3",
            "args": ["-m", "http.server"]
        }));

        manager.discover_from_config(&Some(mcp_servers));

        // Invalid isolation check
        assert_eq!(manager.invalid_servers.len(), 1);
        assert_eq!(manager.invalid_servers[0].server_name, "invalid_stdio");
        assert_eq!(manager.invalid_servers[0].error_field, "command");

        // Valid server validation
        assert_eq!(manager.servers.len(), 1);
        let valid_server = manager.servers.get("valid_stdio").unwrap();
        assert_eq!(valid_server.state, McpServerState::Validated);
        assert_eq!(valid_server.command, Some("python3".to_string()));

        // Run spawn_and_initialize (it simulates the lifecycle)
        manager.spawn_and_initialize().await;
        
        let valid_server_after = manager.servers.get("valid_stdio").unwrap();
        assert_eq!(valid_server_after.state, McpServerState::Ready);
    }
}
