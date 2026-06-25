use crate::models::{MessageResponse, OutputContentBlock, TokenUsage};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DeltaPayload {
    Text { text: String },
    Json { json: String },
    Thinking { thinking: String },
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
                            (OutputContentBlock::ToolInvocation { .. }, DeltaPayload::Json { json: fragment }) => {
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
            AccumulatorState::Finalizing { mut response } => {
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
}

pub fn parse_sse_event(event_name: &str, data: &str) -> Result<SSEEvent, serde_json::Error> {
    match event_name {
        "session_start" => {
            let response = serde_json::from_str::<MessageResponse>(data)?;
            Ok(SSEEvent::SessionStart { response })
        }
        "content_block_begin" => {
            #[derive(Deserialize)]
            struct BeginPayload {
                index: usize,
                block: OutputContentBlock,
            }
            let payload = serde_json::from_str::<BeginPayload>(data)?;
            Ok(SSEEvent::ContentBlockBegin {
                index: payload.index,
                block: payload.block,
            })
        }
        "content_block_delta" => {
            #[derive(Deserialize)]
            struct DeltaWrap {
                index: usize,
                delta: DeltaPayload,
            }
            let payload = serde_json::from_str::<DeltaWrap>(data)?;
            Ok(SSEEvent::ContentBlockDelta {
                index: payload.index,
                delta: payload.delta,
            })
        }
        "content_block_end" => {
            #[derive(Deserialize)]
            struct EndPayload {
                index: usize,
            }
            let payload = serde_json::from_str::<EndPayload>(data)?;
            Ok(SSEEvent::ContentBlockEnd { index: payload.index })
        }
        "message_delta" => {
            let delta = serde_json::from_str::<MessageDeltaPayload>(data)?;
            Ok(SSEEvent::MessageDelta { delta })
        }
        "session_end" => Ok(SSEEvent::SessionEnd),
        _ => {
            // Unrecognized event type, deserialize as SessionEnd or return error,
            // but the spec says to skip unknown events gracefully.
            // We can return a specific error that the caller ignores.
            Err(serde::de::Error::custom(format!("Unknown event type: {}", event_name)))
        }
    }
}
