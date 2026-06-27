import sys

with open("src/runtime.rs", "r") as f:
    content = f.read()

prefix_str = "        self.persist_session();\n\n        loop {"
if prefix_str not in content:
    print("Cannot find anchor")
    sys.exit(1)

prefix_idx = content.find(prefix_str)
suffix_str = "    pub fn persist_session(&mut self) {"
suffix_idx = content.find(suffix_str)

if suffix_idx == -1:
    print("Cannot find suffix")
    sys.exit(1)

new_loop = """        loop {
            let is_anthropic = matches!(self.provider_config.auth_header, crate::data_contracts::AuthHeader::XApiKey);
            
            let mut request_body = serde_json::Map::new();
            request_body.insert("model".to_string(), serde_json::json!(self.provider_config.model));
            request_body.insert("max_tokens".to_string(), serde_json::json!(crate::runtime::DEFAULT_MAX_TOKENS));
            request_body.insert("stream".to_string(), serde_json::json!(true));

            if !is_anthropic {
                request_body.insert("stream_options".to_string(), serde_json::json!({ "include_usage": true }));
            }

            if let Some(ref instructions) = self.app_config.instruction_content {
                if std::env::var("TEST_CONFIG").is_ok() {
                    println!("INSTRUCTIONS_LOADED:\\n{}", instructions);
                    std::process::exit(0);
                }
                if is_anthropic {
                    request_body.insert("system".to_string(), serde_json::json!(instructions));
                }
            }

            let mut messages: Vec<serde_json::Value> = Vec::new();
            
            if !is_anthropic {
                if let Some(ref instructions) = self.app_config.instruction_content {
                    messages.push(serde_json::json!({
                        "role": "system",
                        "content": instructions
                    }));
                }
            }

            for m in &self.session.messages {
                let role = match m.role {
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::System => "system",
                    MessageRole::Tool => "tool",
                };

                if is_anthropic {
                    if m.role == MessageRole::System {
                        continue;
                    }
                    if m.role == MessageRole::Tool {
                        let content = match m.content.first() {
                            Some(ContentBlock::ToolResult { content, is_error, .. }) => {
                                let mut res = serde_json::Map::new();
                                res.insert("type".to_string(), serde_json::json!("tool_result"));
                                res.insert("tool_use_id".to_string(), serde_json::json!(m.tool_call_id.clone().unwrap_or_default()));
                                res.insert("content".to_string(), serde_json::json!(content));
                                if *is_error {
                                    res.insert("is_error".to_string(), serde_json::json!(true));
                                }
                                res
                            },
                            _ => {
                                let mut res = serde_json::Map::new();
                                res.insert("type".to_string(), serde_json::json!("tool_result"));
                                res.insert("tool_use_id".to_string(), serde_json::json!(m.tool_call_id.clone().unwrap_or_default()));
                                res.insert("content".to_string(), serde_json::json!("[missing result]"));
                                res
                            }
                        };
                        messages.push(serde_json::json!({
                            "role": "user",
                            "content": [content]
                        }));
                    } else if m.role == MessageRole::Assistant && m.content.iter().any(|b| matches!(b, ContentBlock::ToolUse { .. })) {
                        let mut content_blocks = Vec::new();
                        for block in &m.content {
                            match block {
                                ContentBlock::Text { text } => {
                                    if !text.is_empty() {
                                        content_blocks.push(serde_json::json!({
                                            "type": "text",
                                            "text": text
                                        }));
                                    }
                                }
                                ContentBlock::ToolUse { id, name, input } => {
                                    content_blocks.push(serde_json::json!({
                                        "type": "tool_use",
                                        "id": id,
                                        "name": name,
                                        "input": input
                                    }));
                                }
                                _ => {}
                            }
                        }
                        messages.push(serde_json::json!({
                            "role": role,
                            "content": content_blocks
                        }));
                    } else {
                        let text_content = m.content.iter().map(|b| match b {
                            ContentBlock::Text { text } => text.clone(),
                            _ => "[unsupported block]".to_string(),
                        }).collect::<Vec<_>>().join("");
                        
                        messages.push(serde_json::json!({
                            "role": role,
                            "content": text_content
                        }));
                    }
                } else {
                    if m.role == MessageRole::Tool {
                        let content = match m.content.first() {
                            Some(ContentBlock::ToolResult { content, .. }) => content.clone(),
                            _ => "[missing result]".to_string(),
                        };
                        messages.push(serde_json::json!({
                            "role": role,
                            "tool_call_id": m.tool_call_id,
                            "content": content
                        }));
                    } else if m.role == MessageRole::Assistant && m.content.iter().any(|b| matches!(b, ContentBlock::ToolUse { .. })) {
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
                        
                        messages.push(serde_json::json!({
                            "role": role,
                            "content": if text.is_empty() { serde_json::Value::Null } else { serde_json::json!(text) },
                            "tool_calls": tool_calls
                        }));
                    } else {
                        let content = m.content.iter().map(|b| match b {
                            ContentBlock::Text { text } => text.clone(),
                            _ => "[unsupported block]".to_string(),
                        }).collect::<Vec<_>>().join("");
                        
                        messages.push(serde_json::json!({
                            "role": role,
                            "content": content
                        }));
                    }
                }
            }

            request_body.insert("messages".to_string(), serde_json::json!(messages));

            let mut tools = crate::tools::dispatch::build_tool_definitions();
            if is_anthropic {
                if let Some(arr) = tools.as_array_mut() {
                    for t in arr.iter_mut() {
                        let obj = t.as_object_mut().unwrap();
                        if let Some(function) = obj.remove("function") {
                            let mut func_obj = function.as_object().unwrap().clone();
                            if let Some(params) = func_obj.remove("parameters") {
                                func_obj.insert("input_schema".to_string(), params);
                            }
                            obj.insert("name".to_string(), func_obj.remove("name").unwrap());
                            obj.insert("description".to_string(), func_obj.remove("description").unwrap_or(serde_json::json!("")));
                            obj.insert("input_schema".to_string(), func_obj.remove("input_schema").unwrap_or(serde_json::json!({})));
                        }
                        obj.remove("type");
                    }
                }
            }
            request_body.insert("tools".to_string(), serde_json::json!(tools));

            let url = if is_anthropic {
                format!("{}/messages", self.provider_config.base_url.trim_end_matches('/'))
            } else {
                format!("{}/chat/completions", self.provider_config.base_url.trim_end_matches('/'))
            };
            
            let client = reqwest::Client::new();
            
            let auth_header_value = match self.provider_config.auth_header {
                crate::data_contracts::AuthHeader::Bearer => format!("Bearer {}", self.provider_config.api_key),
                crate::data_contracts::AuthHeader::XApiKey => self.provider_config.api_key.clone(),
            };
            
            let auth_header_key = match self.provider_config.auth_header {
                crate::data_contracts::AuthHeader::Bearer => "Authorization",
                crate::data_contracts::AuthHeader::XApiKey => "x-api-key",
            };

            let mut req = client
                .post(&url)
                .header(auth_header_key, auth_header_value)
                .header("Content-Type", "application/json")
                .header("Accept", "text/event-stream")
                .json(&request_body);
                
            if is_anthropic {
                req = req.header("anthropic-version", "2023-06-01");
            }

            let response = match req.send().await {
                Ok(resp) => resp,
                Err(e) => {
                    eprintln!("error: HTTP request failed: {}", e);
                    return;
                }
            };

            if !response.status().is_success() {
                let status = response.status();
                let err_body = response.text().await.unwrap_or_default();
                eprintln!("error: API returned HTTP {}: {}", status, err_body);
                return;
            }

            let mut nim_accumulator = crate::nim_accumulator::NimAccumulator::new();
            let mut anthropic_accumulator = crate::sse::AccumulatorState::new();
            
            let mut byte_stream = response.bytes_stream();
            use futures::StreamExt;
            let mut line_buffer = String::new();
            let mut current_event_name = String::new();

            while let Some(chunk_result) = byte_stream.next().await {
                let chunk_bytes = match chunk_result {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        eprintln!("error: stream read failed: {}", e);
                        break;
                    }
                };

                let chunk_str = String::from_utf8_lossy(&chunk_bytes);
                line_buffer.push_str(&chunk_str);

                while let Some(newline_pos) = line_buffer.find('\\n') {
                    let line = line_buffer[..newline_pos].trim().to_string();
                    line_buffer = line_buffer[newline_pos + 1..].to_string();

                    if line.is_empty() {
                        continue;
                    }

                    if let Some(event) = line.strip_prefix("event: ") {
                        current_event_name = event.to_string();
                    } else if let Some(data) = line.strip_prefix("data: ") {
                        if is_anthropic {
                            match crate::sse::parse_sse_event(&current_event_name, data) {
                                Ok(event) => {
                                    match &event {
                                        crate::sse::SSEEvent::ContentBlockDelta { delta, .. } => {
                                            if let crate::sse::DeltaPayload::Text { text } = delta {
                                                print!("{}", text);
                                                std::io::stdout().flush().unwrap();
                                            }
                                        }
                                        _ => {}
                                    }
                                    anthropic_accumulator.transition(event);
                                }
                                Err(e) => {
                                    // Silently ignore unrecognized events
                                }
                            }
                        } else {
                            if let Some(text) = nim_accumulator.process_line(data) {
                                print!("{}", text);
                                std::io::stdout().flush().unwrap();
                            }
                        }
                    }
                }
                
                if is_anthropic {
                    if matches!(anthropic_accumulator, crate::sse::AccumulatorState::Complete(_)) || matches!(anthropic_accumulator, crate::sse::AccumulatorState::Error { .. }) {
                        break;
                    }
                } else {
                    if nim_accumulator.is_done() {
                        break;
                    }
                }
            }

            let provider_result = if is_anthropic {
                anthropic_accumulator.into_provider_turn_result()
            } else {
                nim_accumulator.into_provider_turn_result()
            };

            let is_tool_call = provider_result.stop_reason.as_deref() == Some("tool_calls") || provider_result.stop_reason.as_deref() == Some("tool_use") || provider_result.stop_reason.as_deref() == Some("function_call") || !provider_result.tool_calls.is_empty();

            if is_tool_call {
                if let Some(tc) = provider_result.tool_calls.first() {
                    let args_str = serde_json::to_string(&tc.input).unwrap_or_default();
                    println!("\\n[tool: {}({})]", tc.name, args_str);
                    
                    let mut is_denied = false;
                    
                    if self.stage_permission_mode == StagePermissionMode::Prompt {
                        print!("Allow tool call: {}({})? [y/N] ", tc.name, args_str);
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
                        crate::tools::dispatch::dispatch_tool(&tc.name, &args_str, &self.workspace_root).await
                    };

                    let mut content_blocks = Vec::new();
                    if !provider_result.text.is_empty() {
                        content_blocks.push(ContentBlock::Text { text: provider_result.text.clone() });
                    }
                    content_blocks.push(ContentBlock::ToolUse {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        input: serde_json::from_str(&args_str).unwrap_or(serde_json::json!(args_str)),
                    });

                    let assistant_record = SessionMessageRecord {
                        role: MessageRole::Assistant,
                        content: content_blocks,
                        usage: provider_result.usage.clone(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        tool_call_id: None,
                    };
                    self.session.messages.push(assistant_record);

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
                } else {
                    eprintln!("Warning: tool_calls stop reason but no tool call extracted");
                    break;
                }
            } else {
                println!();
                let assistant_record = SessionMessageRecord {
                    role: MessageRole::Assistant,
                    content: vec![ContentBlock::Text { text: provider_result.text }],
                    usage: provider_result.usage,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    tool_call_id: None,
                };
                self.session.messages.push(assistant_record);
                self.persist_session();
                break;
            }
        }
    }
"""

new_content = content[:prefix_idx + len("        self.persist_session();\n\n")] + new_loop + "\n" + content[suffix_idx:]
new_content = "pub const DEFAULT_MAX_TOKENS: u32 = 4096;\n\n" + new_content

with open("src/runtime.rs", "w") as f:
    f.write(new_content)
