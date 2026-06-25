use crate::session::Session;
use crate::worker_state::WorkerStateManager;
use crate::data_contracts::WorkerState;
use crate::compaction::{CompactionEngine, CompactionConfig};
use crate::slash_commands::SlashCommandRegistry;
use crate::data_contracts::{MessageRole, SessionMessageRecord, ContentBlock, UsageRecord, StagePermissionMode};
use crate::nim_accumulator::NimAccumulator;
use std::io::Write;
use std::path::PathBuf;

pub struct RuntimeConfig {
    pub base_dir: PathBuf,
    pub active_model: String,
    pub permission_mode: String,
    pub workspace_root: PathBuf,
    pub stage_permission_mode: StagePermissionMode,
}

pub struct ConversationRuntime {
    pub session: Session,
    pub worker_state_manager: WorkerStateManager,
    pub compaction_engine: CompactionEngine,
    pub slash_commands: SlashCommandRegistry,
    pub turn_count: usize,
    pub workspace_root: PathBuf,
    pub stage_permission_mode: StagePermissionMode,
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
            workspace_root: config.workspace_root,
            stage_permission_mode: config.stage_permission_mode,
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

        // Persist User Turn initially
        let user_record = SessionMessageRecord {
            role: MessageRole::User,
            content: vec![ContentBlock::Text { text: user_input.to_string() }],
            usage: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            tool_call_id: None,
        };
        self.session.messages.push(user_record);
        self.persist_session();

        loop {
            // Build the OpenAI-compatible messages array from session history.
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
                    
                    if m.role == MessageRole::Tool {
                        let content = match m.content.first() {
                            Some(ContentBlock::ToolResult { content, .. }) => content.clone(),
                            _ => "[missing result]".to_string(),
                        };
                        return serde_json::json!({
                            "role": role,
                            "tool_call_id": m.tool_call_id,
                            "content": content
                        });
                    }

                    if m.role == MessageRole::Assistant && m.content.iter().any(|b| matches!(b, ContentBlock::ToolUse { .. })) {
                        let mut tool_calls = Vec::new();
                        let mut text = String::new();
                        
                        for block in &m.content {
                            match block {
                                ContentBlock::Text { text: t } => text.push_str(t),
                                ContentBlock::ToolUse { id, name, input } => {
                                    tool_calls.push(serde_json::json!({
                                        "id": id,
                                        "type": "function",
                                        "function": {
                                            "name": name,
                                            "arguments": input.to_string()
                                        }
                                    }));
                                }
                                _ => {}
                            }
                        }
                        
                        let mut msg = serde_json::json!({
                            "role": role,
                            "content": if text.is_empty() { serde_json::Value::Null } else { serde_json::json!(text) },
                            "tool_calls": tool_calls
                        });
                        return msg;
                    }
                    
                    let content = m.content.iter().map(|b| match b {
                        ContentBlock::Text { text } => text.clone(),
                        _ => "[unsupported block]".to_string(),
                    }).collect::<Vec<_>>().join("");
                    
                    serde_json::json!({
                        "role": role,
                        "content": content
                    })
                })
                .collect();

            // Build request body with tools
            let request_body = serde_json::json!({
                "model": "moonshotai/kimi-k2.6",
                "max_tokens": 4096,
                "stream": true,
                "stream_options": { "include_usage": true },
                "messages": messages,
                "tools": crate::tools::dispatch::build_tool_definitions()
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

            // Check HTTP status before streaming loop
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
                            print!("{}", text);
                            std::io::stdout().flush().unwrap();
                        }
                    }
                }

                if accumulator.is_done() {
                    break;
                }
            }

            // Consume the accumulator
            let (tool_call_opt, full_text, usage, stop_reason) = accumulator.into_tool_call();
            let is_tool_call = stop_reason.as_deref() == Some("tool_calls") || stop_reason.as_deref() == Some("function_call") || tool_call_opt.is_some();

            if is_tool_call {
                if let Some(tc) = tool_call_opt {
                    println!("\n[tool: {}({})]", tc.name, tc.arguments);
                    
                    let mut is_denied = false;
                    
                    if self.stage_permission_mode == StagePermissionMode::Prompt {
                        print!("Allow tool call: {}({})? [y/N] ", tc.name, tc.arguments);
                        std::io::stdout().flush().unwrap();
                        let mut line = String::new();
                        if std::io::stdin().read_line(&mut line).is_ok() {
                            let resp = line.trim().to_lowercase();
                            if resp != "y" {
                                is_denied = true;
                            }
                        } else {
                            is_denied = true;
                        }
                    }

                    let tool_result_str = if is_denied {
                        "tool call denied by user".to_string()
                    } else {
                        crate::tools::dispatch::dispatch_tool(&tc.name, &tc.arguments, &self.workspace_root).await
                    };

                    let mut content_blocks = Vec::new();
                    if !full_text.is_empty() {
                        content_blocks.push(ContentBlock::Text { text: full_text.clone() });
                    }
                    content_blocks.push(ContentBlock::ToolUse {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        input: serde_json::from_str(&tc.arguments).unwrap_or(serde_json::json!(tc.arguments)),
                    });

                    // Assistant tool-call record
                    let assistant_record = SessionMessageRecord {
                        role: MessageRole::Assistant,
                        content: content_blocks,
                        usage: Some(usage),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        tool_call_id: None,
                    };
                    self.session.messages.push(assistant_record);

                    // Tool result record
                    let tool_record = SessionMessageRecord {
                        role: MessageRole::Tool,
                        content: vec![ContentBlock::ToolResult {
                            tool_use_id: tc.id.clone(),
                            content: tool_result_str,
                            is_error: is_denied,
                        }],
                        usage: None,
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        tool_call_id: Some(tc.id.clone()),
                    };
                    self.session.messages.push(tool_record);

                    self.persist_session();
                    // Loop will continue and send the new history
                } else {
                    eprintln!("Warning: tool_calls stop reason but no tool call extracted");
                    break;
                }
            } else {
                // Final text response
                println!();
                let assistant_record = SessionMessageRecord {
                    role: MessageRole::Assistant,
                    content: vec![ContentBlock::Text { text: full_text }],
                    usage: Some(usage),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    tool_call_id: None,
                };
                self.session.messages.push(assistant_record);
                self.persist_session();
                break;
            }
        }
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
