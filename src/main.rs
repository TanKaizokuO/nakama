pub mod models;
pub mod sse;
pub mod provider;
pub mod config;
pub mod path_scope;
pub mod usage;
pub mod tools;
pub mod permission;
pub mod prompter;
pub mod hook;
pub mod mcp;
// mod tests;

pub mod data_contracts;
pub mod session;
pub mod worker_state;
pub mod compaction;
pub mod error_handling;
pub mod error;
pub mod cli;
pub mod repl;
pub mod bootstrap;
pub mod subcommands;
pub mod plugin;
pub mod instruction;
pub mod slash_commands;
pub mod runtime;
pub mod nim_accumulator;

use runtime::{ConversationRuntime, RuntimeConfig};
use crate::data_contracts::{StagePermissionMode, ProviderConfig, AuthHeader};
use crate::slash_commands::{SlashCommandRegistry, SlashCommandResult};
use std::io::{self, Write};
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    // Step 1: Load .env
    dotenvy::dotenv().ok();

    // Parse --session <id>
    let mut args = std::env::args().skip(1);
    let mut session_id_arg = None;
    while let Some(arg) = args.next() {
        if arg == "--session" {
            session_id_arg = args.next();
        }
    }

    let session_dir = PathBuf::from(".claw/sessions");

    if let Some(ref id) = session_id_arg {
        let file_path = session_dir.join(format!("{}.jsonl", id));
        if !file_path.exists() {
            eprintln!("error: session {} not found at .claw/sessions/{}.jsonl", id, id);
            std::process::exit(1);
        }
    }

    let workspace_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // Step 2: Load Hierarchical Config
    let app_config = crate::bootstrap::Bootstrap::load_config(&workspace_root);

    // Provider parsing
    let mut model = std::env::var("NAKAMA_MODEL").unwrap_or_default();
    if let Some(resolved) = app_config.model_aliases.get(&model) {
        model = resolved.clone();
    }
    
    let explicit_provider = std::env::var("NAKAMA_PROVIDER").unwrap_or_default().to_lowercase();
    
    let provider_config = if explicit_provider == "nim" {
        // Legacy override
        let api_key = match std::env::var("NVIDIA_API_KEY").or_else(|_| std::env::var("OPENAI_API_KEY")) {
            Ok(key) if !key.trim().is_empty() => key,
            _ => {
                eprintln!("error: NVIDIA_API_KEY (or OPENAI_API_KEY) not set for nim provider.");
                std::process::exit(1);
            }
        };
        let base_url = std::env::var("URL").unwrap_or_else(|_| "https://integrate.api.nvidia.com/v1".to_string());
        if model.is_empty() {
            model = "moonshotai/kimi-k2-5".to_string();
        }
        ProviderConfig {
            base_url,
            api_key,
            model,
            auth_header: AuthHeader::Bearer,
        }
    } else if model.contains("claude") {
        let api_key = match std::env::var("ANTHROPIC_API_KEY") {
            Ok(key) if !key.trim().is_empty() => key,
            _ => {
                eprintln!("error: ANTHROPIC_API_KEY not set for Anthropic provider.");
                std::process::exit(1);
            }
        };
        ProviderConfig {
            base_url: std::env::var("URL").unwrap_or_else(|_| "https://api.anthropic.com/v1".to_string()),
            api_key,
            model,
            auth_header: AuthHeader::XApiKey,
        }
    } else if model.contains("grok") {
        eprintln!("error: xAI provider not yet implemented");
        std::process::exit(1);
    } else if model.starts_with("openai/") || model.starts_with("local/") || model.starts_with("gpt-") || model.starts_with("qwen/") || model.starts_with("qwen-") || model.starts_with("kimi/") || model.starts_with("kimi-") {
        let api_key = match std::env::var("OPENAI_API_KEY").or_else(|_| std::env::var("NVIDIA_API_KEY")) {
            Ok(key) if !key.trim().is_empty() => key,
            _ => {
                eprintln!("error: OPENAI_API_KEY (or NVIDIA_API_KEY) not set for OpenAI-compatible provider.");
                std::process::exit(1);
            }
        };
        let base_url = std::env::var("URL").unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
        ProviderConfig {
            base_url,
            api_key,
            model,
            auth_header: AuthHeader::Bearer,
        }
    } else {
        // Fallback checks
        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            if !key.trim().is_empty() {
                if model.is_empty() {
                    model = "claude-3-5-sonnet-20241022".to_string();
                }
                ProviderConfig {
                    base_url: std::env::var("URL").unwrap_or_else(|_| "https://api.anthropic.com/v1".to_string()),
                    api_key: key,
                    model,
                    auth_header: AuthHeader::XApiKey,
                }
            } else {
                eprintln!("error: No provider matched and ANTHROPIC_API_KEY is empty.");
                std::process::exit(1);
            }
        } else if let Ok(key) = std::env::var("OPENAI_API_KEY").or_else(|_| std::env::var("NVIDIA_API_KEY")) {
            if !key.trim().is_empty() {
                if model.is_empty() {
                    model = "gpt-4o".to_string();
                }
                ProviderConfig {
                    base_url: std::env::var("URL").unwrap_or_else(|_| "https://api.openai.com/v1".to_string()),
                    api_key: key,
                    model,
                    auth_header: AuthHeader::Bearer,
                }
            } else {
                eprintln!("error: No provider matched and OPENAI_API_KEY is empty.");
                std::process::exit(1);
            }
        } else {
            eprintln!("error: No API keys configured. Set ANTHROPIC_API_KEY or OPENAI_API_KEY.");
            std::process::exit(1);
        }
    };
    
    let provider_name = match provider_config.auth_header {
        AuthHeader::XApiKey => "anthropic",
        AuthHeader::Bearer => "nim",
    };

    let perm_mode_str = std::env::var("NAKAMA_PERMISSION_MODE")
        .unwrap_or_else(|_| "prompt".to_string())
        .to_lowercase();
        
    let stage_permission_mode = match perm_mode_str.as_str() {
        "auto" => StagePermissionMode::Auto,
        _ => StagePermissionMode::Prompt,
    };

    let compaction_threshold = std::env::var("NAKAMA_COMPACTION_THRESHOLD")
        .unwrap_or_else(|_| "32000".to_string())
        .parse::<usize>()
        .unwrap_or(32000);

    // Initialize the conversation runtime
    let config = RuntimeConfig {
        base_dir: session_dir,
        provider_config: provider_config.clone(),
        permission_mode: "default".to_string(),
        workspace_root,
        stage_permission_mode,
        compaction_threshold,
        app_config,
    };
    
    let mut runtime = ConversationRuntime::new(config, session_id_arg.as_deref());

    if session_id_arg.is_some() {
        println!("Resuming session: {}", runtime.session.metadata.session_id);
    } else {
        println!("New session: {}", runtime.session.metadata.session_id);
    }

    println!("╭─ Nakama ──────────────────────────────╮");
    println!("│  Session   {:<26} │", runtime.session.metadata.session_id);
    println!("│  Model     {:<26} │", provider_config.model);
    println!("│  Provider  {:<26} │", provider_name);
    println!("│  Mode      {:<26} │", perm_mode_str);
    println!("│  Compaction threshold: {:<14} │", format!("{} tokens", compaction_threshold));
    println!("╰───────────────────────────────────────╯");
    println!("Type /help for commands.");

    let slash_registry = SlashCommandRegistry::new();

    // Step 5: REPL loop
    loop {
        print!("> ");
        io::stdout().flush().unwrap();

        let mut line = String::new();
        match io::stdin().read_line(&mut line) {
            Ok(0) => {
                println!();
                break;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("error: failed to read input: {}", e);
                break;
            }
        }

        let input = line.trim();

        if input.is_empty() {
            continue;
        }

        if input.starts_with('/') {
            match slash_registry.dispatch(input, &mut runtime) {
                SlashCommandResult::Exit => break,
                SlashCommandResult::Handled => {
                    runtime.persist_session();
                    continue;
                }
                SlashCommandResult::NotACommand => {}
            }
        }

        runtime.execute_turn_async(input).await;
    }
}
