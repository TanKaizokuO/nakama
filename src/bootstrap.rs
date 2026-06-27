#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BootstrapPhase {
    CLIEntry,
    FastPathVersion,
    StartupProfiler,
    SystemPromptFastPath,
    ChromeMcpFastPath,
    DaemonWorkerFastPath,
    BridgeFastPath,
    DaemonFastPath,
    BackgroundSessionFastPath,
    TemplateFastPath,
    EnvironmentRunnerFastPath,
    MainRuntime,
}

pub struct BootstrapPipeline {
    pub phases: Vec<BootstrapPhase>,
}

impl Default for BootstrapPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl BootstrapPipeline {
    pub fn new() -> Self {
        Self {
            phases: vec![
                BootstrapPhase::CLIEntry,
                BootstrapPhase::FastPathVersion,
                BootstrapPhase::StartupProfiler,
                BootstrapPhase::SystemPromptFastPath,
                BootstrapPhase::ChromeMcpFastPath,
                BootstrapPhase::DaemonWorkerFastPath,
                BootstrapPhase::BridgeFastPath,
                BootstrapPhase::DaemonFastPath,
                BootstrapPhase::BackgroundSessionFastPath,
                BootstrapPhase::TemplateFastPath,
                BootstrapPhase::EnvironmentRunnerFastPath,
                BootstrapPhase::MainRuntime,
            ],
        }
    }

    pub fn deduplicate(&mut self) {
        let mut seen = std::collections::HashSet::new();
        self.phases.retain(|e| seen.insert(*e));
    }

    pub fn run(&self) {
        for phase in &self.phases {
            println!("Running phase: {:?}", phase);
        }
    }
}

pub struct Bootstrap;

impl Bootstrap {
    pub fn load_config(workspace_root: &std::path::Path) -> crate::config::AppConfig {
        crate::config::load_merged_config(workspace_root)
    }
}

