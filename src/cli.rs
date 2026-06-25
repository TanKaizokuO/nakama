use clap::{Parser, Subcommand};
use crate::error::ConfigError;
use std::collections::HashMap;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[arg(long)]
    pub model: Option<String>,

    #[arg(long)]
    pub output_format: Vec<String>,

    #[arg(long)]
    pub permission: Option<String>,

    #[arg(long)]
    pub cwd: Option<String>,

    #[arg(long)]
    pub dangerously_skip_permissions: bool,

    #[arg(long, value_delimiter = ',')]
    pub allowed_tools: Option<Vec<String>>,

    #[arg(long)]
    pub resume: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Prompt {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        text: Vec<String>,
    },
    HealthCheck,
    StatusReport,
    SandboxInfo,
    VersionInfo,
    InitWorkspace,
    DumpManifests,
    SystemPrompt,
    AgentList,
    McpInspect,
    SkillsInspect,
    BootstrapPlan,
    WorkerState,
}

pub struct ParsedArgs {
    pub cli: Cli,
    pub format: String,
    pub format_overridden: bool,
    pub normalized_allowed_tools: Option<Vec<String>>,
}

pub fn parse_cli() -> Result<ParsedArgs, ConfigError> {
    let cli = Cli::parse();

    let mut format = "text".to_string();
    let mut format_overridden = false;
    
    if let Ok(env_val) = std::env::var("CLAW_OUTPUT_FORMAT") {
        format = env_val;
    }

    if !cli.output_format.is_empty() {
        if cli.output_format.len() > 1 {
            eprintln!("Warning: Multiple --output-format flags provided. Using the last one.");
            format_overridden = true;
        }
        format = cli.output_format.last().unwrap().clone();
    }

    if format != "text" && format != "json" {
        return Err(ConfigError::InvalidOutputFormat { value: format });
    }

    let mut normalized_tools = None;
    if let Some(tools) = &cli.allowed_tools {
        if tools.is_empty() {
            return Err(ConfigError::MissingArgument { argument: "--allowed-tools".to_string() });
        }
        let alias_table = get_alias_table();
        let mut normalized = Vec::new();
        for t in tools {
            if let Some(actual) = alias_table.get(t) {
                normalized.push(actual.clone());
            } else {
                normalized.push(t.clone());
            }
        }
        normalized_tools = Some(normalized);
    }

    Ok(ParsedArgs {
        cli,
        format,
        format_overridden,
        normalized_allowed_tools: normalized_tools,
    })
}

fn get_alias_table() -> HashMap<String, String> {
    let mut map = HashMap::new();
    map.insert("sh".to_string(), "bash".to_string());
    map
}
