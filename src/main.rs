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
mod tests;

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
use crate::data_contracts::StagePermissionMode;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    // Step 1: Load .env
    dotenvy::dotenv().ok();

    // Step 2: Read NVIDIA_API_KEY — exit with error if missing
    let api_key = match std::env::var("NVIDIA_API_KEY") {
        Ok(key) if !key.trim().is_empty() => key,
        _ => {
            eprintln!("error: NVIDIA_API_KEY not set. Add it to .env or export it in your shell.");
            std::process::exit(1);
        }
    };

    // Step 3: Read URL — default to https://integrate.api.nvidia.com/v1 if not set
    let base_url = std::env::var("URL")
        .unwrap_or_else(|_| "https://integrate.api.nvidia.com/v1".to_string());

    // Step 4: Print ready message
    println!("Nakama ready. Model: moonshotai/kimi-k2.6");

    let perm_mode_str = std::env::var("NAKAMA_PERMISSION_MODE")
        .unwrap_or_else(|_| "prompt".to_string())
        .to_lowercase();
        
    let stage_permission_mode = match perm_mode_str.as_str() {
        "auto" => StagePermissionMode::Auto,
        _ => StagePermissionMode::Prompt,
    };

    let workspace_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // Initialize the conversation runtime
    let session_dir = PathBuf::from(".claw/sessions");
    let config = RuntimeConfig {
        base_dir: session_dir,
        active_model: "moonshotai/kimi-k2.6".to_string(),
        permission_mode: "default".to_string(),
        workspace_root,
        stage_permission_mode,
    };
    let mut runtime = ConversationRuntime::new(config, None);

    // Step 5: REPL loop
    loop {
        // Print prompt
        print!("> ");
        io::stdout().flush().unwrap();

        // Read a line (without holding a persistent lock, so tool prompts can read stdin too)
        let mut line = String::new();
        match io::stdin().read_line(&mut line) {
            Ok(0) => {
                // EOF (Ctrl-D)
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

        // Handle /quit
        if input == "/quit" {
            break;
        }

        // Skip empty lines
        if input.is_empty() {
            continue;
        }

        // Execute the turn with real streaming
        runtime.execute_turn_async(input, &api_key, &base_url).await;
    }
}
