use crate::models::{MessageResponse, OutputContentBlock, TokenUsage};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum DeltaPayload {
    #[serde(rename = "text_delta")]
    Text { text: String },
    #[serde(rename = "input_json_delta")]
    Json { partial_json: String },
    #[serde(rename = "thinking_delta")]
    Thinking { thinking: String },
    #[serde(rename = "signature_delta")]
    Signature { signature: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageDeltaPayload {
    pub stop_reason: Option<String>,
    pub token_usage: Option<TokenUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum SSEEvent {
    SessionStart { response: MessageResponse },
    ContentBlockBegin { index: usize, block: OutputContentBlock },
    ContentBlockDelta { index: usize, delta: DeltaPayload },
    ContentBlockEnd { index: usize },
    MessageDelta { delta: MessageDeltaPayload },
    SessionEnd,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AccumulatorState {
    Initial,
    Streaming {
        response: MessageResponse,
        open_block_indices: HashSet<usize>,
        tool_inputs: HashMap<usize, String>,
    },
    Finalizing {
        response: MessageResponse,
    },
    Complete(MessageResponse),
    Error {
        message: String,
        partial_response: Option<MessageResponse>,
    },
}

impl AccumulatorState {
    pub fn new() -> Self {
        AccumulatorState::Initial
    }

    pub fn get_partial_response(&self) -> Option<MessageResponse> {
        match self {
            AccumulatorState::Initial => None,
            AccumulatorState::Streaming { response, .. } => Some(response.clone()),
            AccumulatorState::Finalizing { response } => Some(response.clone()),
            AccumulatorState::Complete(response) => Some(response.clone()),
            AccumulatorState::Error { partial_response, .. } => partial_response.clone(),
        }
    }

    pub fn transition(&mut self, event: SSEEvent) {
        match std::mem::replace(self, AccumulatorState::Initial) {
            AccumulatorState::Initial => {
                if let SSEEvent::SessionStart { response } = event {
                    *self = AccumulatorState::Streaming {
                        response,
                        open_block_indices: HashSet::new(),
                        tool_inputs: HashMap::new(),
                    };
                } else {
                    *self = AccumulatorState::Error {
                        message: "Received event before SessionStart".to_string(),
                        partial_response: None,
                    };
                }
            }
            AccumulatorState::Streaming {
                mut response,
                mut open_block_indices,
                mut tool_inputs,
            } => {
                match event {
                    SSEEvent::SessionStart { .. } => {
                        *self = AccumulatorState::Error {
                            message: "Duplicate SessionStart event received".to_string(),
                            partial_response: Some(response),
                        };
                    }
                    SSEEvent::ContentBlockBegin { index, block } => {
                        if open_block_indices.contains(&index) {
                            eprintln!("Warning: Duplicate ContentBlockBegin for index {}, resetting block", index);
                        }
                        open_block_indices.insert(index);
                        
                        // Pad content_blocks if necessary
                        while response.content_blocks.len() <= index {
                            // Pad with empty text as fallback
                            response.content_blocks.push(OutputContentBlock::TextContent {
                                text: String::new(),
                            });
                        }
                        response.content_blocks[index] = block;
                        
                        // Clear any old accumulated tool inputs
                        tool_inputs.remove(&index);

                        *self = AccumulatorState::Streaming {
                            response,
                            open_block_indices,
                            tool_inputs,
                        };
                    }
                    SSEEvent::ContentBlockDelta { index, delta } => {
                        if !open_block_indices.contains(&index) {
                            eprintln!("Warning: ContentBlockDelta received for unopened/closed index {}", index);
                            // Keep streaming but skip delta
                            *self = AccumulatorState::Streaming {
                                response,
                                open_block_indices,
                                tool_inputs,
                            };
                            return;
                        }

                        if index >= response.content_blocks.len() {
                            eprintln!("Warning: ContentBlockDelta index {} out of range", index);
                            *self = AccumulatorState::Streaming {
                                response,
                                open_block_indices,
                                tool_inputs,
                            };
                            return;
                        }

                        match (&mut response.content_blocks[index], delta) {
                            (OutputContentBlock::TextContent { text }, DeltaPayload::Text { text: fragment }) => {
                                text.push_str(&fragment);
                            }
                            (OutputContentBlock::ToolInvocation { .. }, DeltaPayload::Json { partial_json: fragment }) => {
                                tool_inputs.entry(index).or_default().push_str(&fragment);
                            }
                            (OutputContentBlock::ThinkingContent { thinking, .. }, DeltaPayload::Thinking { thinking: fragment }) => {
                                thinking.push_str(&fragment);
                            }
                            (OutputContentBlock::ThinkingContent { signature, .. }, DeltaPayload::Signature { signature: fragment }) => {
                                *signature = Some(fragment);
                            }
                            (block, delta) => {
                                eprintln!("Warning: Mismatched block type ({:?}) and delta payload ({:?})", block, delta);
                            }
                        }

                        *self = AccumulatorState::Streaming {
                            response,
                            open_block_indices,
                            tool_inputs,
                        };
                    }
                    SSEEvent::ContentBlockEnd { index } => {
                        if !open_block_indices.remove(&index) {
                            eprintln!("Warning: ContentBlockEnd received for unopened/closed index {}", index);
                        } else if index < response.content_blocks.len() {
                            if let OutputContentBlock::ToolInvocation { input, .. } = &mut response.content_blocks[index] {
                                if let Some(raw_json) = tool_inputs.remove(&index) {
                                    match serde_json::from_str::<serde_json::Value>(&raw_json) {
                                        Ok(parsed) => {
                                            *input = parsed;
                                        }
                                        Err(e) => {
                                            eprintln!("Warning: Failed to parse tool input JSON: {}", e);
                                            *input = serde_json::Value::Null;
                                        }
                                    }
                                }
                            }
                        }

                        *self = AccumulatorState::Streaming {
                            response,
                            open_block_indices,
                            tool_inputs,
                        };
                    }
                    SSEEvent::MessageDelta { delta } => {
                        response.stop_reason = delta.stop_reason;
                        if let Some(usage) = delta.token_usage {
                            response.token_usage = usage;
                        }
                        *self = AccumulatorState::Finalizing { response };
                    }
                    SSEEvent::SessionEnd => {
                        *self = AccumulatorState::Complete(response);
                    }
                }
            }
            AccumulatorState::Finalizing { response } => {
                if let SSEEvent::SessionEnd = event {
                    *self = AccumulatorState::Complete(response);
                } else {
                    eprintln!("Warning: Received event {:?} in Finalizing state", event);
                    *self = AccumulatorState::Finalizing { response };
                }
            }
            AccumulatorState::Complete(response) => {
                *self = AccumulatorState::Complete(response);
            }
            AccumulatorState::Error { message, partial_response } => {
                *self = AccumulatorState::Error { message, partial_response };
            }
        }
    }

    pub fn force_error(&mut self, err_msg: &str) {
        let partial = self.get_partial_response();
        *self = AccumulatorState::Error {
            message: err_msg.to_string(),
            partial_response: partial,
        };
    }

    pub fn into_provider_turn_result(self) -> crate::data_contracts::ProviderTurnResult {
        match self {
            AccumulatorState::Complete(response) => {
                let mut text = String::new();
                let mut tool_calls = Vec::new();
                
                for block in response.content_blocks {
                    match block {
                        OutputContentBlock::TextContent { text: t } => {
                            text.push_str(&t);
                        }
                        OutputContentBlock::ToolInvocation { id, name, input } => {
                            tool_calls.push(crate::data_contracts::AccumulatedToolCall {
                                id,
                                name,
                                input,
                            });
                        }
                        _ => {}
                    }
                }
                
                crate::data_contracts::ProviderTurnResult {
                    text,
                    tool_calls,
                    stop_reason: response.stop_reason,
                    usage: Some(crate::data_contracts::UsageRecord {
                        input_tokens: response.token_usage.input_tokens,
                        output_tokens: response.token_usage.output_tokens,
                        cache_creation_tokens: response.token_usage.cache_creation_tokens,
                        cache_read_tokens: response.token_usage.cache_read_tokens,
                    }),
                }
            }
            _ => {
                crate::data_contracts::ProviderTurnResult {
                    text: String::new(),
                    tool_calls: vec![],
                    stop_reason: None,
                    usage: None,
                }
            }
        }
    }
}

pub fn parse_sse_event(event_name: &str, data: &str) -> Result<SSEEvent, serde_json::Error> {
    let data_val = serde_json::from_str::<serde_json::Value>(data)?;

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
                    DeltaPayload::Json { partial_json: json }
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
            let output_tokens = data_val["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32;
            
            Ok(SSEEvent::MessageDelta {
                delta: MessageDeltaPayload {
                    stop_reason,
                    token_usage: Some(TokenUsage {
                        input_tokens: 0,
                        output_tokens,
                        cache_creation_tokens: 0,
                        cache_read_tokens: 0,
                    }),
                },
            })
        }
        "message_stop" => Ok(SSEEvent::SessionEnd),
        _ => Err(serde::de::Error::custom("Unknown event type")),
    }
}
