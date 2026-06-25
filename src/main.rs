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

fn main() {
    println!("Nakama Core Infrastructure initialized.");
}
