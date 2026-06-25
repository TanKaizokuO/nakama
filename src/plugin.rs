#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginState {
    Discovered,
    Installed,
    Enabled,
    Disabled,
    Uninstalled,
}

#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub server_health: String,
    pub tool_info: String,
    pub resource_info: String,
}

#[derive(Debug, Clone)]
pub struct DegradedMode {
    pub reduced_capabilities: Vec<String>,
}

pub struct Plugin {
    pub identifier: String,
    pub state: PluginState,
    pub health_status: Option<HealthStatus>,
    pub degraded_mode: Option<DegradedMode>,
}

impl Plugin {
    pub fn new(identifier: String) -> Self {
        Self {
            identifier,
            state: PluginState::Discovered,
            health_status: None,
            degraded_mode: None,
        }
    }

    pub fn transition(&mut self, new_state: PluginState) {
        println!("Plugin {} transition: {:?} -> {:?}", self.identifier, self.state, new_state);
        self.state = new_state;
    }

    pub fn check_health(&mut self) {
        self.health_status = Some(HealthStatus {
            server_health: "ok".to_string(),
            tool_info: "3 tools active".to_string(),
            resource_info: "memory: 10MB".to_string(),
        });
    }

    pub fn enter_degraded_mode(&mut self, capabilities: Vec<String>) {
        self.degraded_mode = Some(DegradedMode {
            reduced_capabilities: capabilities,
        });
        println!("Plugin {} entered degraded mode", self.identifier);
    }
}
