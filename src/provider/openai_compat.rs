use crate::models::{
    InputContent, InputContentBlock, InputMessage, MessageRequest, MessageResponse,
    MessageRole, OutputContentBlock, TokenUsage, ToolResultContent,
};
use crate::provider::{resolve_auth, apply_extensions, ProviderClient, ProviderError, ProviderKind};
use crate::sse::{DeltaPayload, MessageDeltaPayload, SSEEvent};
use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::{Stream, StreamExt};
use serde_json::json;
use std::pin::Pin;

pub struct OpenAICompatClient {
    client: reqwest::Client,
    pub base_url: String,
    pub provider_kind: ProviderKind,
}

impl OpenAICompatClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: "https://api.openai.com/v1/chat/completions".to_string(),
            provider_kind: ProviderKind::OpenAICompat,
        }
    }

    pub fn with_custom(base_url: String, provider_kind: ProviderKind) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url,
            provider_kind,
        }
    }
}

#[async_trait]
impl ProviderClient for OpenAICompatClient {
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
                "OpenAI compatible API returned error {}: {}",
                status,
                err_text
            )));
        }

        let oai_resp = resp.json::<serde_json::Value>().await?;
        self.parse_response(&oai_resp)
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
                "OpenAI compatible API returned error {}: {}",
                status,
                err_text
            )));
        }

        let event_stream = resp.bytes_stream().eventsource();
        
        let mapped_stream = event_stream.map(|event_result| match event_result {
            Ok(event) => {
                if event.data == "[DONE]" {
                    Ok(SSEEvent::SessionEnd)
                } else {
                    map_openai_event(&event.data)
                }
            }
            Err(e) => Err(ProviderError::Stream(e.to_string())),
        });

        Ok(Box::pin(mapped_stream))
    }
}

impl OpenAICompatClient {
    fn prepare_request(
        &self,
        request: &MessageRequest,
    ) -> Result<(String, Vec<crate::provider::AuthHeader>, serde_json::Map<String, serde_json::Value>), ProviderError> {
        let mut url = self.base_url.clone();
        
        // Ollama / local server check - only applicable for core OpenAICompat
        if self.provider_kind == ProviderKind::OpenAICompat {
            if let Ok(ollama_host) = std::env::var("OLLAMA_HOST") {
                if !ollama_host.trim().is_empty() {
                    url = ollama_host.trim().to_string();
                    if !url.contains("/v1/chat/completions") {
                        if url.ends_with('/') {
                            url.push_str("v1/chat/completions");
                        } else {
                            url.push_str("/v1/chat/completions");
                        }
                    }
                }
            }
        }

        let mut headers = Vec::new();
        if let Some(auth) = resolve_auth(self.provider_kind)? {
            headers.push(auth);
        }
        headers.push(crate::provider::AuthHeader {
            name: "content-type".to_string(),
            value: "application/json".to_string(),
        });

        let mut body = serde_json::Map::new();
        body.insert("model".to_string(), json!(request.model_identifier));
        
        // OpenAI compatibility for O1/O3 reasoning models might use max_completion_tokens instead of max_tokens
        let max_tokens_field = if request.model_identifier.starts_with("o1") || request.model_identifier.starts_with("o3") {
            "max_completion_tokens"
        } else {
            "max_tokens"
        };
        body.insert(max_tokens_field.to_string(), json!(request.max_output_tokens));

        let mut messages = Vec::new();
        for msg in &request.message_history {
            let role_str = match msg.role {
                MessageRole::System => "system",
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::Tool => "tool",
            };

            match &msg.content {
                InputContent::SingleText(text) => {
                    messages.push(json!({
                        "role": role_str,
                        "content": text
                    }));
                }
                InputContent::Blocks(blocks) => {
                    let mut text_accum = String::new();
                    let mut tool_calls = Vec::new();
                    let mut tool_results = Vec::new();

                    for b in blocks {
                        match b {
                            InputContentBlock::Text { text } => {
                                text_accum.push_str(text);
                            }
                            InputContentBlock::Image { .. } => {
                                // Fallback/Skip or format as image URL if needed
                            }
                            InputContentBlock::ToolUse { id, name, input } => {
                                tool_calls.push(json!({
                                    "id": id,
                                    "type": "function",
                                    "function": {
                                        "name": name,
                                        "arguments": input.to_string()
                                    }
                                }));
                            }
                            InputContentBlock::ToolResult { tool_use_id, content, .. } => {
                                let content_str = match content {
                                    ToolResultContent::Text(t) => t.clone(),
                                    ToolResultContent::Blocks(blks) => {
                                        // Flatten block list to JSON string or similar
                                        serde_json::to_string(blks).unwrap_or_default()
                                    }
                                };
                                tool_results.push((tool_use_id.clone(), content_str));
                            }
                            InputContentBlock::Thinking { thinking, .. } => {
                                text_accum.push_str(thinking);
                            }
                        }
                    }

                    if msg.role == MessageRole::Tool {
                        for (id, res) in tool_results {
                            messages.push(json!({
                                "role": "tool",
                                "tool_call_id": id,
                                "content": res
                            }));
                        }
                    } else if msg.role == MessageRole::Assistant && !tool_calls.is_empty() {
                        let mut msg_obj = serde_json::Map::new();
                        msg_obj.insert("role".to_string(), json!("assistant"));
                        if !text_accum.is_empty() {
                            msg_obj.insert("content".to_string(), json!(text_accum));
                        }
                        msg_obj.insert("tool_calls".to_string(), json!(tool_calls));
                        messages.push(serde_json::Value::Object(msg_obj));
                    } else {
                        messages.push(json!({
                            "role": role_str,
                            "content": text_accum
                        }));
                    }
                }
            }
        }

        // Prepend system instruction if present
        if let Some(sys) = &request.system_instruction {
            messages.insert(0, json!({
                "role": "system",
                "content": sys
            }));
        }

        body.insert("messages".to_string(), json!(messages));

        // Tools
        if let Some(tools) = &request.tool_definitions {
            let mapped_tools: Vec<serde_json::Value> = tools
                .iter()
                .map(|t| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.input_schema
                        }
                    })
                })
                .collect();
            body.insert("tools".to_string(), json!(mapped_tools));
        }

        // Tool choice
        if let Some(policy) = &request.tool_selection_policy {
            let policy_val = match policy {
                crate::models::ToolSelectionPolicy::Auto => json!("auto"),
                crate::models::ToolSelectionPolicy::Any => json!("required"),
                crate::models::ToolSelectionPolicy::SpecificTool(name) => {
                    json!({
                        "type": "function",
                        "function": { "name": name }
                    })
                }
            };
            body.insert("tool_choice".to_string(), policy_val);
        }

        // Parameters
        if request.temperature.is_some() {
            body.insert("temperature".to_string(), json!(request.temperature));
        }
        if request.top_p.is_some() {
            body.insert("top_p".to_string(), json!(request.top_p));
        }
        if request.presence_penalty.is_some() {
            body.insert("presence_penalty".to_string(), json!(request.presence_penalty));
        }
        if request.frequency_penalty.is_some() {
            body.insert("frequency_penalty".to_string(), json!(request.frequency_penalty));
        }
        if let Some(stop) = &request.stop_sequences {
            body.insert("stop".to_string(), json!(stop));
        }

        // Reasoning models reasoning_effort
        if let Some(effort) = request.reasoning_effort {
            let effort_str = match effort {
                crate::models::ReasoningEffort::Low => "low",
                crate::models::ReasoningEffort::Medium => "medium",
                crate::models::ReasoningEffort::High => "high",
            };
            body.insert("reasoning_effort".to_string(), json!(effort_str));
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
        let choice = &response["choices"][0];
        let role = choice["message"]["role"].as_str().unwrap_or("assistant").to_string();
        let model_used = response["model"].as_str().unwrap_or_default().to_string();
        let stop_reason = choice["finish_reason"].as_str().map(|s| s.to_string());

        let mut content_blocks = Vec::new();
        if let Some(text) = choice["message"]["content"].as_str() {
            if !text.is_empty() {
                content_blocks.push(OutputContentBlock::TextContent { text: text.to_string() });
            }
        }

        if let Some(tool_calls) = choice["message"]["tool_calls"].as_array() {
            for call in tool_calls {
                let id = call["id"].as_str().unwrap_or_default().to_string();
                let name = call["function"]["name"].as_str().unwrap_or_default().to_string();
                let arg_str = call["function"]["arguments"].as_str().unwrap_or("{}");
                let input = serde_json::from_str(arg_str).unwrap_or(serde_json::Value::Null);
                content_blocks.push(OutputContentBlock::ToolInvocation { id, name, input });
            }
        }

        let input_tokens = response["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32;
        let output_tokens = response["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32;

        Ok(MessageResponse {
            response_id,
            role,
            content_blocks,
            model_used,
            stop_reason,
            token_usage: TokenUsage {
                input_tokens,
                output_tokens,
                cache_creation_tokens: 0,
                cache_read_tokens: 0,
            },
        })
    }
}

fn map_openai_event(data: &str) -> Result<SSEEvent, ProviderError> {
    let data_val = serde_json::from_str::<serde_json::Value>(data)
        .map_err(|e| ProviderError::Stream(e.to_string()))?;

    let response_id = data_val["id"].as_str().unwrap_or_default().to_string();
    let model_used = data_val["model"].as_str().unwrap_or_default().to_string();
    let choice = &data_val["choices"][0];

    // If it is the first chunk, we can send SessionStart
    // But since OpenAI sends chunk-by-chunk, we can synthesize a SessionStart event if we see content deltas.
    // Wait, to be compliant with our 6 SSE events, the first event MUST be SessionStart.
    // So the caller stream mapper or client stream wrapper should send SessionStart first, then the deltas.
    // Let's check if the choices or delta exists.
    let delta_val = &choice["delta"];
    
    // Check if this chunk contains content block end (finish_reason is not null)
    let finish_reason = choice["finish_reason"].as_str();

    if delta_val.is_null() && finish_reason.is_some() {
        return Ok(SSEEvent::MessageDelta {
            delta: MessageDeltaPayload {
                stop_reason: finish_reason.map(|s| s.to_string()),
                token_usage: None,
            },
        });
    }

    if let Some(text) = delta_val["content"].as_str() {
        // OpenAI sends bare text. We map it to ContentBlockDelta.
        // But wait! We need ContentBlockBegin first!
        // To handle this cleanly, the client can return a stream that synthesizes SessionStart and ContentBlockBegin when it starts.
        // Let's do that! Or we can return the raw mapped event.
        // Let's return the text delta here.
        return Ok(SSEEvent::ContentBlockDelta {
            index: 0,
            delta: DeltaPayload::Text { text: text.to_string() },
        });
    }

    if let Some(tool_calls) = delta_val["tool_calls"].as_array() {
        let call = &tool_calls[0];
        let id = call["id"].as_str().unwrap_or_default().to_string();
        let name = call["function"]["name"].as_str().unwrap_or_default().to_string();
        let arguments = call["function"]["arguments"].as_str().unwrap_or_default().to_string();
        
        if !id.is_empty() || !name.is_empty() {
            // This is a ContentBlockBegin for tool_use!
            return Ok(SSEEvent::ContentBlockBegin {
                index: 0,
                block: OutputContentBlock::ToolInvocation {
                    id,
                    name,
                    input: serde_json::Value::Null,
                },
            });
        } else {
            // Delta JSON string argument segment
            return Ok(SSEEvent::ContentBlockDelta {
                index: 0,
                delta: DeltaPayload::Json { json: arguments },
            });
        }
    }

    // Default to a SessionStart skeleton if it's the very start of the stream
    let response = MessageResponse {
        response_id,
        role: "assistant".to_string(),
        content_blocks: Vec::new(),
        model_used,
        stop_reason: None,
        token_usage: TokenUsage::default(),
    };
    Ok(SSEEvent::SessionStart { response })
}
