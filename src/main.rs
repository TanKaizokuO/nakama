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
pub mod slash_commands;
pub mod runtime;

fn main() {
    println!("Nakama Core Infrastructure initialized.");
}
