use crate::session::Session;
use crate::worker_state::{WorkerStateManager, WorkerState};
use crate::compaction::{CompactionEngine, CompactionConfig};
use crate::slash_commands::SlashCommandRegistry;
use crate::data_contracts::{MessageRole, SessionMessageRecord, ContentBlock};
use std::path::PathBuf;

pub struct RuntimeConfig {
    pub base_dir: PathBuf,
    pub active_model: String,
    pub permission_mode: String,
}

pub struct ConversationRuntime {
    pub session: Session,
    pub worker_state_manager: WorkerStateManager,
    pub compaction_engine: CompactionEngine,
    pub slash_commands: SlashCommandRegistry,
    pub turn_count: usize,
}

impl ConversationRuntime {
    pub fn new(config: RuntimeConfig, session_id: Option<&str>) -> Self {
        let base_dir = config.base_dir.clone();
        
        let session = if let Some(sid) = session_id {
            Session::resume(base_dir.clone(), sid).unwrap_or_else(|_| Session::new(base_dir.clone(), config.active_model.clone(), config.permission_mode.clone()))
        } else {
            Session::new(base_dir.clone(), config.active_model.clone(), config.permission_mode.clone())
        };

        Self {
            session,
            worker_state_manager: WorkerStateManager::new(base_dir),
            compaction_engine: CompactionEngine::new(CompactionConfig::default()),
            slash_commands: SlashCommandRegistry::new(),
            turn_count: 0,
        }
    }

    pub fn execute_turn(&mut self, user_input: &str) {
        // Step 1: Receive user input
        if user_input.trim().is_empty() {
            return;
        }

        // Step 2: Route the prompt (slash commands)
        if let Some(cmd_output) = self.slash_commands.dispatch(user_input) {
            println!("{}", cmd_output);
            self.persist_session();
            return;
        }

        // Step 3-7 Loop
        let input_msg = SessionMessageRecord {
            role: MessageRole::User,
            content: vec![ContentBlock::Text { text: user_input.to_string() }],
            usage: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            tool_call_id: None,
        };
        self.session.messages.push(input_msg);

        loop {
            // Step 3: Assemble API request (mocked)
            // Step 4: Request preflight
            let (new_messages, compaction_record) = self.compaction_engine.maybe_compact(self.session.messages.clone());
            if let Some(record) = compaction_record {
                self.session.metadata.compaction_history.push(record);
            }
            self.session.messages = new_messages;
            
            let current_tokens: usize = self.session.messages.iter().map(|m| CompactionEngine::estimate_tokens(m)).sum();
            if current_tokens > 100000 {
                println!("Error: Context window exceeded after compaction.");
                break;
            }

            // Step 5: Send to provider (mocked)
            let mock_response = SessionMessageRecord {
                role: MessageRole::Assistant,
                content: vec![ContentBlock::Text { text: "Mock response".to_string() }],
                usage: Some(crate::data_contracts::UsageRecord { input_tokens: 10, output_tokens: 20, cache_creation_tokens: 0, cache_read_tokens: 0 }),
                timestamp: chrono::Utc::now().to_rfc3339(),
                tool_call_id: None,
            };
            
            // Step 6: Process response
            let mut stop_reason = "end_turn"; // mocked
            
            for block in &mock_response.content {
                match block {
                    ContentBlock::Text { text } => println!("{}", text),
                    ContentBlock::ToolUse { .. } => {
                        stop_reason = "tool_use";
                    }
                    _ => {}
                }
            }

            self.session.messages.push(mock_response.clone());

            // Step 7: Evaluate stop reason
            if stop_reason == "end_turn" {
                // Step 8: Record usage implicitly handled
                break;
            } else if stop_reason == "tool_use" {
                continue;
            } else if stop_reason == "max_tokens" {
                println!("Response truncated due to output token limit.");
                break;
            }
        }

        // Step 10: Persist session
        self.persist_session();
    }

    fn persist_session(&mut self) {
        if let Err(e) = self.session.save() {
            eprintln!("Failed to save session: {}", e);
        }

        self.turn_count += 1;
        if self.turn_count == 1 {
            let state = WorkerState {
                worker_id: uuid::Uuid::new_v4().to_string(),
                session_id: self.session.metadata.session_id.clone(),
                model: self.session.metadata.model.clone(),
                permission_mode: self.session.metadata.permission_mode.clone(),
            };
            if let Err(e) = self.worker_state_manager.write_state(&state) {
                eprintln!("Failed to write worker state: {}", e);
            }
        }
    }
}
