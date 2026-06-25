use crate::models::{
    InputContent, InputContentBlock, MessageRequest, MessageResponse,
    MessageRole, OutputContentBlock, TokenUsage, ToolResultContent,
};
use crate::provider::{resolve_auth, apply_extensions, ProviderClient, ProviderError, ProviderKind};
use crate::sse::{DeltaPayload, MessageDeltaPayload, SSEEvent};
use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::{Stream, StreamExt};
use serde_json::json;
use std::pin::Pin;

pub struct AnthropicClient {
    client: reqwest::Client,
}

impl AnthropicClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl ProviderClient for AnthropicClient {
    async fn send_message(&self, request: &MessageRequest) -> Result<MessageResponse, ProviderError> {
        let (url, headers, body) = self.prepare_request(request)?;

        let mut req = self.client.post(&url);
        for h in headers {
            req = req.header(&h.name, &h.value);
        }

        let resp = req.json(&body).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_text = resp.text().await.unwrap_or_default();
            return Err(ProviderError::General(format!(
                "Anthropic API returned error {}: {}",
                status,
                err_text
            )));
        }

        let anthropic_resp = resp.json::<serde_json::Value>().await?;
        self.parse_response(&anthropic_resp)
    }

    async fn stream_message(
        &self,
        request: &MessageRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<SSEEvent, ProviderError>> + Send>>, ProviderError> {
        let (url, headers, mut body) = self.prepare_request(request)?;
        body.insert("stream".to_string(), json!(true));

        let mut req = self.client.post(&url);
        for h in headers {
            req = req.header(&h.name, &h.value);
        }

        let resp = req.json(&body).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_text = resp.text().await.unwrap_or_default();
            return Err(ProviderError::General(format!(
                "Anthropic API returned error {}: {}",
                status,
                err_text
            )));
        }

        let event_stream = resp.bytes_stream().eventsource();
        
        let mapped_stream = event_stream.map(|event_result| match event_result {
            Ok(event) => {
                map_anthropic_event(&event.event, &event.data)
            }
            Err(e) => Err(ProviderError::Stream(e.to_string())),
        });

        Ok(Box::pin(mapped_stream))
    }
}

// Implement the helper methods on AnthropicClient
impl AnthropicClient {
    fn prepare_request(
        &self,
        request: &MessageRequest,
    ) -> Result<(String, Vec<crate::provider::AuthHeader>, serde_json::Map<String, serde_json::Value>), ProviderError> {
        let url = "https://api.anthropic.com/v1/messages".to_string();

        let mut headers = Vec::new();
        if let Some(auth) = resolve_auth(ProviderKind::Anthropic)? {
            headers.push(auth);
        }
        headers.push(crate::provider::AuthHeader {
            name: "anthropic-version".to_string(),
            value: "2023-06-01".to_string(),
        });
        headers.push(crate::provider::AuthHeader {
            name: "content-type".to_string(),
            value: "application/json".to_string(),
        });

        // Construct standard Anthropic body
        let mut body = serde_json::Map::new();
        body.insert("model".to_string(), json!(request.model_identifier));
        body.insert("max_tokens".to_string(), json!(request.max_output_tokens));

        // Separate system messages and normal messages
        let mut system_parts = Vec::new();
        if let Some(sys) = &request.system_instruction {
            system_parts.push(sys.clone());
        }

        let mut messages = Vec::new();
        for msg in &request.message_history {
            match msg.role {
                MessageRole::System => {
                    match &msg.content {
                        InputContent::SingleText(t) => system_parts.push(t.clone()),
                        InputContent::Blocks(blocks) => {
                            for b in blocks {
                                if let InputContentBlock::Text { text } = b {
                                    system_parts.push(text.clone());
                                }
                            }
                        }
                    }
                }
                MessageRole::User | MessageRole::Assistant | MessageRole::Tool => {
                    let anth_role = match msg.role {
                        MessageRole::Assistant => "assistant",
                        _ => "user", // both user and tool go to "user" role in Anthropic
                    };

                    let content_val = match &msg.content {
                        InputContent::SingleText(t) => json!(t),
                        InputContent::Blocks(blocks) => {
                            let mapped: Vec<serde_json::Value> = blocks
                                .iter()
                                .map(|b| match b {
                                    InputContentBlock::Text { text } => {
                                        json!({ "type": "text", "text": text })
                                    }
                                    InputContentBlock::Image { source } => {
                                        json!({ "type": "image", "source": source })
                                    }
                                    InputContentBlock::ToolUse { id, name, input } => {
                                        json!({ "type": "tool_use", "id": id, "name": name, "input": input })
                                    }
                                    InputContentBlock::ToolResult { tool_use_id, content, is_error } => {
                                        let mut result_obj = serde_json::Map::new();
                                        result_obj.insert("type".to_string(), json!("tool_result"));
                                        result_obj.insert("tool_use_id".to_string(), json!(tool_use_id));
                                        
                                        let content_json = match content {
                                            ToolResultContent::Text(t) => json!(t),
                                            ToolResultContent::Blocks(blks) => json!(blks),
                                        };
                                        result_obj.insert("content".to_string(), content_json);
                                        
                                        if let Some(err) = is_error {
                                            result_obj.insert("is_error".to_string(), json!(err));
                                        }
                                        serde_json::Value::Object(result_obj)
                                    }
                                    InputContentBlock::Thinking { thinking, signature } => {
                                        let mut thinking_obj = serde_json::Map::new();
                                        thinking_obj.insert("type".to_string(), json!("thinking"));
                                        thinking_obj.insert("thinking".to_string(), json!(thinking));
                                        if let Some(sig) = signature {
                                            thinking_obj.insert("signature".to_string(), json!(sig));
                                        }
                                        serde_json::Value::Object(thinking_obj)
                                    }
                                })
                                .collect();
                            json!(mapped)
                        }
                    };

                    messages.push(json!({
                        "role": anth_role,
                        "content": content_val
                    }));
                }
            }
        }

        body.insert("messages".to_string(), json!(messages));

        if !system_parts.is_empty() {
            body.insert("system".to_string(), json!(system_parts.join("\n\n")));
        }

        // Tools
        if let Some(tools) = &request.tool_definitions {
            let mapped_tools: Vec<serde_json::Value> = tools
                .iter()
                .map(|t| {
                    json!({
                        "name": t.name,
                        "description": t.description,
                        "input_schema": t.input_schema
                    })
                })
                .collect();
            body.insert("tools".to_string(), json!(mapped_tools));
        }

        // Tool choice
        if let Some(policy) = &request.tool_selection_policy {
            let policy_val = match policy {
                crate::models::ToolSelectionPolicy::Auto => json!({ "type": "auto" }),
                crate::models::ToolSelectionPolicy::Any => json!({ "type": "any" }),
                crate::models::ToolSelectionPolicy::SpecificTool(name) => {
                    json!({ "type": "tool", "name": name })
                }
            };
            body.insert("tool_choice".to_string(), policy_val);
        }

        // Parameters
        if let Some(temp) = request.temperature {
            body.insert("temperature".to_string(), json!(temp));
        }
        if let Some(top_p) = request.top_p {
            body.insert("top_p".to_string(), json!(top_p));
        }
        if let Some(stop) = &request.stop_sequences {
            body.insert("stop_sequences".to_string(), json!(stop));
        }

        // Apply reasoning effort if compatible model (or if requested)
        if let Some(effort) = request.reasoning_effort {
            // Note: Anthropic models might require specific fields like thinking budget.
            // We will map it in a compatible way.
            let effort_str = match effort {
                crate::models::ReasoningEffort::Low => "low",
                crate::models::ReasoningEffort::Medium => "medium",
                crate::models::ReasoningEffort::High => "high",
            };
            body.insert("thinking".to_string(), json!({ "type": "enabled", "budget_tokens": 1024, "effort": effort_str }));
        }

        // Merge provider extensions, filtering out collisions
        let mut body_val = serde_json::Value::Object(body);
        apply_extensions(&mut body_val, &request.provider_extensions);

        if let serde_json::Value::Object(final_map) = body_val {
            Ok((url, headers, final_map))
        } else {
            Err(ProviderError::General("Request body serialization failed".to_string()))
        }
    }

    fn parse_response(&self, response: &serde_json::Value) -> Result<MessageResponse, ProviderError> {
        let response_id = response["id"].as_str().unwrap_or_default().to_string();
        let role = response["role"].as_str().unwrap_or("assistant").to_string();
        let model_used = response["model"].as_str().unwrap_or_default().to_string();
        let stop_reason = response["stop_reason"].as_str().map(|s| s.to_string());

        let mut content_blocks = Vec::new();
        if let Some(content_array) = response["content"].as_array() {
            for block_val in content_array {
                let block_type = block_val["type"].as_str().unwrap_or("text");
                match block_type {
                    "text" => {
                        let text = block_val["text"].as_str().unwrap_or_default().to_string();
                        content_blocks.push(OutputContentBlock::TextContent { text });
                    }
                    "tool_use" => {
                        let id = block_val["id"].as_str().unwrap_or_default().to_string();
                        let name = block_val["name"].as_str().unwrap_or_default().to_string();
                        let input = block_val["input"].clone();
                        content_blocks.push(OutputContentBlock::ToolInvocation { id, name, input });
                    }
                    "thinking" => {
                        let thinking = block_val["thinking"].as_str().unwrap_or_default().to_string();
                        let signature = block_val["signature"].as_str().map(|s| s.to_string());
                        content_blocks.push(OutputContentBlock::ThinkingContent { thinking, signature });
                    }
                    "redacted_thinking" => {
                        let data = block_val["data"].as_str().unwrap_or_default().to_string();
                        content_blocks.push(OutputContentBlock::RedactedThinking { data });
                    }
                    _ => {}
                }
            }
        }

        let input_tokens = response["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32;
        let output_tokens = response["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32;
        let cache_creation_tokens = response["usage"]["cache_creation_input_tokens"].as_u64().unwrap_or(0) as u32;
        let cache_read_tokens = response["usage"]["cache_read_input_tokens"].as_u64().unwrap_or(0) as u32;

        let token_usage = TokenUsage {
            input_tokens,
            output_tokens,
            cache_creation_tokens,
            cache_read_tokens,
        };

        Ok(MessageResponse {
            response_id,
            role,
            content_blocks,
            model_used,
            stop_reason,
            token_usage,
        })
    }
}

fn map_anthropic_event(event_name: &str, data: &str) -> Result<SSEEvent, ProviderError> {
    let data_val = serde_json::from_str::<serde_json::Value>(data)
        .map_err(|e| ProviderError::Stream(e.to_string()))?;

    match event_name {
        "message_start" => {
            let msg_val = &data_val["message"];
            let response_id = msg_val["id"].as_str().unwrap_or_default().to_string();
            let role = msg_val["role"].as_str().unwrap_or("assistant").to_string();
            let model_used = msg_val["model"].as_str().unwrap_or_default().to_string();
            
            let input_tokens = msg_val["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32;
            let cache_creation_tokens = msg_val["usage"]["cache_creation_input_tokens"].as_u64().unwrap_or(0) as u32;
            let cache_read_tokens = msg_val["usage"]["cache_read_input_tokens"].as_u64().unwrap_or(0) as u32;

            let response = MessageResponse {
                response_id,
                role,
                content_blocks: Vec::new(),
                model_used,
                stop_reason: None,
                token_usage: TokenUsage {
                    input_tokens,
                    output_tokens: 0,
                    cache_creation_tokens,
                    cache_read_tokens,
                },
            };
            Ok(SSEEvent::SessionStart { response })
        }
        "content_block_start" => {
            let index = data_val["index"].as_u64().unwrap_or(0) as usize;
            let block_val = &data_val["content_block"];
            let block_type = block_val["type"].as_str().unwrap_or("text");
            
            let block = match block_type {
                "text" => {
                    let text = block_val["text"].as_str().unwrap_or_default().to_string();
                    OutputContentBlock::TextContent { text }
                }
                "tool_use" => {
                    let id = block_val["id"].as_str().unwrap_or_default().to_string();
                    let name = block_val["name"].as_str().unwrap_or_default().to_string();
                    let input = block_val["input"].clone();
                    OutputContentBlock::ToolInvocation { id, name, input }
                }
                "thinking" => {
                    let thinking = block_val["thinking"].as_str().unwrap_or_default().to_string();
                    let signature = block_val["signature"].as_str().map(|s| s.to_string());
                    OutputContentBlock::ThinkingContent { thinking, signature }
                }
                "redacted_thinking" => {
                    let data = block_val["data"].as_str().unwrap_or_default().to_string();
                    OutputContentBlock::RedactedThinking { data }
                }
                _ => OutputContentBlock::TextContent { text: String::new() },
            };

            Ok(SSEEvent::ContentBlockBegin { index, block })
        }
        "content_block_delta" => {
            let index = data_val["index"].as_u64().unwrap_or(0) as usize;
            let delta_val = &data_val["delta"];
            let delta_type = delta_val["type"].as_str().unwrap_or("text_delta");
            
            let delta = match delta_type {
                "text_delta" => {
                    let text = delta_val["text"].as_str().unwrap_or_default().to_string();
                    DeltaPayload::Text { text }
                }
                "input_json_delta" => {
                    let json = delta_val["partial_json"].as_str().unwrap_or_default().to_string();
                    DeltaPayload::Json { json }
                }
                "thinking_delta" => {
                    let thinking = delta_val["thinking"].as_str().unwrap_or_default().to_string();
                    DeltaPayload::Thinking { thinking }
                }
                "signature_delta" => {
                    let signature = delta_val["signature"].as_str().unwrap_or_default().to_string();
                    DeltaPayload::Signature { signature }
                }
                _ => DeltaPayload::Text { text: String::new() },
            };

            Ok(SSEEvent::ContentBlockDelta { index, delta })
        }
        "content_block_stop" => {
            let index = data_val["index"].as_u64().unwrap_or(0) as usize;
            Ok(SSEEvent::ContentBlockEnd { index })
        }
        "message_delta" => {
            let delta_val = &data_val["delta"];
            let stop_reason = delta_val["stop_reason"].as_str().map(|s| s.to_string());
            let output_tokens = delta_val["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32;
            
            Ok(SSEEvent::MessageDelta {
                delta: MessageDeltaPayload {
                    stop_reason,
                    token_usage: Some(TokenUsage {
                        input_tokens: 0, // usually not sent or set to 0 in delta
                        output_tokens,
                        cache_creation_tokens: 0,
                        cache_read_tokens: 0,
                    }),
                },
            })
        }
        "message_stop" => Ok(SSEEvent::SessionEnd),
        _ => Err(ProviderError::Stream(format!("Unknown Anthropic event: {}", event_name))),
    }
}
