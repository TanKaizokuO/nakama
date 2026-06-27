use crate::runtime::ConversationRuntime;

pub enum SlashCommandResult {
    Handled,
    Exit,
    NotACommand,
}

pub struct SlashCommandRegistry;

impl SlashCommandRegistry {
    pub fn new() -> Self {
        Self
    }

    pub fn dispatch(&self, input: &str, runtime: &mut ConversationRuntime) -> SlashCommandResult {
        if !input.starts_with('/') {
            return SlashCommandResult::NotACommand;
        }

        let parts: Vec<&str> = input.trim().splitn(2, ' ').collect();
        let cmd_name = &parts[0][1..]; // trim the '/'

        match cmd_name {
            "help" => {
                println!("Commands:");
                println!("  /help      show this message");
                println!("  /compact   force context compaction now");
                println!("  /session   show current session info");
                println!("  /quit      exit");
                println!("\nTools available: shell, file_read, file_write, grep_search, list_files");
                SlashCommandResult::Handled
            }
            "quit" => {
                SlashCommandResult::Exit
            }
            "session" => {
                let session_id = &runtime.session.metadata.session_id;
                let turns = runtime.turn_count;
                println!("Session:  {}", session_id);
                println!("Turns:    {}", turns);
                println!("File:     .claw/sessions/{}.jsonl", session_id);
                SlashCommandResult::Handled
            }
            "compact" => {
                println!("Compacting context...");
                let old_count = runtime.session.messages.len();
                
                if let Ok(Some(_record)) = runtime.compaction_engine.compact(&mut runtime.session.messages, true) {
                    let new_count = runtime.session.messages.len();
                    
                    let system_msg = crate::data_contracts::SessionMessageRecord {
                        role: crate::data_contracts::MessageRole::User,
                        content: vec![crate::data_contracts::ContentBlock::Text {
                            text: "[Context compacted. Prior conversation summarised above.]".to_string(),
                        }],
                        usage: None,
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        tool_call_id: None,
                    };
                    runtime.session.messages.push(system_msg);
                    
                    println!("Compaction complete. Messages reduced from {} to {}.", old_count, new_count);
                } else {
                    println!("Compaction not needed or failed.");
                }
                SlashCommandResult::Handled
            }
            _ => {
                // Unrecognized slash command => treat as model prompt as per spec
                SlashCommandResult::NotACommand
            }
        }
    }
}
