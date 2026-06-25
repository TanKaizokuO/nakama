use crate::session::Session;
use crate::worker_state::WorkerStateManager;
use crate::data_contracts::WorkerState;
use crate::compaction::{CompactionEngine, CompactionConfig};
use crate::slash_commands::SlashCommandRegistry;
use crate::data_contracts::{MessageRole, SessionMessageRecord, ContentBlock, UsageRecord};
use crate::nim_accumulator::NimAccumulator;
use std::io::Write;
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

    /// Real streaming provider call to NVIDIA NIM (OpenAI-compatible).
    ///
    /// Sends user input to the NIM endpoint, streams the response to stdout
    /// chunk by chunk, then persists both the user and assistant turns to JSONL.
    pub async fn execute_turn_async(
        &mut self,
        user_input: &str,
        api_key: &str,
        base_url: &str,
    ) {
        // Step 1: Skip empty input
        if user_input.trim().is_empty() {
            return;
        }

        // Step 2: Route slash commands (existing logic)
        if let Some(cmd_output) = self.slash_commands.dispatch(user_input) {
            println!("{}", cmd_output);
            self.persist_session();
            return;
        }

        // Build the OpenAI-compatible messages array from session history + new input.
        // G2: Explicit conversion from Vec<ContentBlock> to OpenAI message format.
        // Stage 1 only handles text blocks; other variants are stringified as placeholders.
        let content_blocks_to_string = |blocks: &[ContentBlock]| -> String {
            blocks
                .iter()
                .map(|b| match b {
                    ContentBlock::Text { text } => text.clone(),
                    _ => "[unsupported block]".to_string(),
                })
                .collect::<Vec<_>>()
                .join("")
        };

        let mut messages: Vec<serde_json::Value> = self
            .session
            .messages
            .iter()
            .map(|m| {
                let role = match m.role {
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::System => "system",
                    MessageRole::Tool => "tool",
                };
                serde_json::json!({
                    "role": role,
                    "content": content_blocks_to_string(&m.content)
                })
            })
            .collect();

        // Append the new user message
        messages.push(serde_json::json!({
            "role": "user",
            "content": user_input
        }));

        // Build request body
        let request_body = serde_json::json!({
            "model": "moonshotai/kimi-k2.6",
            "max_tokens": 4096,
            "stream": true,
            "stream_options": { "include_usage": true },
            "messages": messages
        });

        // Send POST request
        let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
        let client = reqwest::Client::new();

        let response = match client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .json(&request_body)
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                eprintln!("error: HTTP request failed: {}", e);
                return;
            }
        };

        // G3: Check HTTP status before entering the streaming loop.
        // Non-2xx responses contain a JSON error body, not SSE events.
        if !response.status().is_success() {
            let status = response.status();
            let err_body = response.text().await.unwrap_or_default();
            eprintln!("error: NIM API returned HTTP {}: {}", status, err_body);
            return;
        }

        // Stream SSE chunks
        let mut accumulator = NimAccumulator::new();
        let mut byte_stream = response.bytes_stream();

        use futures::StreamExt;
        let mut line_buffer = String::new();

        while let Some(chunk_result) = byte_stream.next().await {
            let chunk_bytes = match chunk_result {
                Ok(bytes) => bytes,
                Err(e) => {
                    eprintln!("error: stream read failed: {}", e);
                    break;
                }
            };

            // Append raw bytes to line buffer and process complete lines
            let chunk_str = String::from_utf8_lossy(&chunk_bytes);
            line_buffer.push_str(&chunk_str);

            // Process all complete lines in the buffer
            while let Some(newline_pos) = line_buffer.find('\n') {
                let line = line_buffer[..newline_pos].trim().to_string();
                line_buffer = line_buffer[newline_pos + 1..].to_string();

                if line.is_empty() {
                    continue;
                }

                // SSE lines are prefixed with "data: "
                if let Some(data) = line.strip_prefix("data: ") {
                    if let Some(text) = accumulator.process_line(data) {
                        // Task 1.3: Stream output to terminal as chunks arrive
                        print!("{}", text);
                        std::io::stdout().flush().unwrap();
                    }
                }
                // Ignore non-data SSE lines (e.g., "event:", "id:", comments)
            }

            if accumulator.is_done() {
                break;
            }
        }

        // Print newline after [DONE]
        println!();

        // Consume the accumulator
        let (full_text, usage, _stop_reason) = accumulator.into_result();

        // Task 1.4: Persist the turn to JSONL
        // User turn
        let user_record = SessionMessageRecord {
            role: MessageRole::User,
            content: vec![ContentBlock::Text { text: user_input.to_string() }],
            usage: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            tool_call_id: None,
        };
        self.session.messages.push(user_record);

        // Assistant turn
        let assistant_record = SessionMessageRecord {
            role: MessageRole::Assistant,
            content: vec![ContentBlock::Text { text: full_text }],
            usage: Some(usage),
            timestamp: chrono::Utc::now().to_rfc3339(),
            tool_call_id: None,
        };
        self.session.messages.push(assistant_record);

        // Persist
        self.persist_session();
    }

    // TODO: Remove in Stage 2 — replaced by execute_turn_async
    #[allow(dead_code)]
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
                usage: Some(UsageRecord { input_tokens: 10, output_tokens: 20, cache_creation_tokens: 0, cache_read_tokens: 0 }),
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
