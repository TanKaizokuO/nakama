use crate::models::{MessageRequest, MessageResponse};
use async_trait::async_trait;
use futures::Stream;
use std::collections::HashSet;
use std::pin::Pin;
use thiserror::Error;

pub mod anthropic;
pub mod dashscope;
pub mod openai_compat;
pub mod xai;

#[derive(Error, Debug)]
pub enum ProviderError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Authentication error: {0}")]
    Authentication(String),
    #[error("Stream processing error: {0}")]
    Stream(String),
    #[error("General provider error: {0}")]
    General(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Anthropic,
    OpenAICompat,
    XAI,
    DashScope,
}

/// Helper function to filter out colliding keys from provider_extensions.
pub fn apply_extensions(
    base: &mut serde_json::Value,
    extensions: &Option<serde_json::Map<String, serde_json::Value>>,
) {
    if let Some(ext) = extensions {
        let protected_keys: HashSet<&str> = [
            "model",
            "messages",
            "stream",
            "max_tokens",
            "system",
            "temperature",
            "top_p",
            "presence_penalty",
            "frequency_penalty",
            "stop_sequences",
            "reasoning_effort",
            "tools",
            "tool_choice",
        ]
        .into_iter()
        .collect();

        if let serde_json::Value::Object(obj) = base {
            for (key, val) in ext {
                if !protected_keys.contains(key.as_str()) {
                    obj.insert(key.clone(), val.clone());
                } else {
                    eprintln!(
                        "Warning: Overriding protected core parameter '{}' via provider_extensions is prohibited and was ignored.",
                        key
                    );
                }
            }
        }
    }
}

/// Checks environment variables to see if any are non-empty.
fn is_env_set(var_name: &str) -> bool {
    std::env::var(var_name)
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
}

/// Performs the provider routing cascade based on resolved model name and environment.
pub fn route_model(model: &str) -> ProviderKind {
    let lower_model = model.to_lowercase();

    // Rule 1: Model name contains "claude" -> Anthropic
    if lower_model.contains("claude") {
        return ProviderKind::Anthropic;
    }

    // Rule 2: Model name contains "grok" -> xAI
    if lower_model.contains("grok") {
        return ProviderKind::XAI;
    }

    // Rule 3: Model name starts with "openai/", "local/", or "gpt-" -> OpenAI-compatible
    if lower_model.starts_with("openai/")
        || lower_model.starts_with("local/")
        || lower_model.starts_with("gpt-")
    {
        return ProviderKind::OpenAICompat;
    }

    // Rule 4: Model name starts with "qwen/", "qwen-", "kimi/", or "kimi-" -> DashScope
    if lower_model.starts_with("qwen/")
        || lower_model.starts_with("qwen-")
        || lower_model.starts_with("kimi/")
        || lower_model.starts_with("kimi-")
    {
        return ProviderKind::DashScope;
    }

    // Rule 5: Local-server base URL set and model name looks local.
    // "Looks local" means model name does not contain "/" (except "local/" prefix, which is already handled in Rule 3).
    let ollama_host_set = is_env_set("OLLAMA_HOST");
    if ollama_host_set && !lower_model.contains('/') {
        return ProviderKind::OpenAICompat;
    }

    // Rule 6: Fallback: check populated credentials (Anthropic -> OpenAI -> xAI).
    if is_env_set("ANTHROPIC_API_KEY") || is_env_set("ANTHROPIC_AUTH_TOKEN") {
        return ProviderKind::Anthropic;
    }
    if is_env_set("OPENAI_API_KEY") {
        return ProviderKind::OpenAICompat;
    }
    if is_env_set("XAI_API_KEY") {
        return ProviderKind::XAI;
    }

    // Rule 7: Final default -> Anthropic
    ProviderKind::Anthropic
}

#[derive(Debug, Clone)]
pub struct AuthHeader {
    pub name: String,
    pub value: String,
}

/// Resolves authentication credentials for a provider.
pub fn resolve_auth(kind: ProviderKind) -> Result<Option<AuthHeader>, ProviderError> {
    match kind {
        ProviderKind::Anthropic => {
            if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
                if !key.trim().is_empty() {
                    // Diagnostic hint checks
                    if key.starts_with("sk-ant-") {
                        return Ok(Some(AuthHeader {
                            name: "x-api-key".to_string(),
                            value: key,
                        }));
                    } else {
                        eprintln!("Warning: ANTHROPIC_API_KEY does not start with sk-ant-");
                        return Ok(Some(AuthHeader {
                            name: "x-api-key".to_string(),
                            value: key,
                        }));
                    }
                }
            }
            if let Ok(token) = std::env::var("ANTHROPIC_AUTH_TOKEN") {
                if !token.trim().is_empty() {
                    if token.starts_with("sk-ant-") {
                        eprintln!(
                            "Diagnostic hint: ANTHROPIC_AUTH_TOKEN contains an sk-ant-* API key. \
                             Please move this key to ANTHROPIC_API_KEY."
                        );
                    }
                    return Ok(Some(AuthHeader {
                        name: "Authorization".to_string(),
                        value: format!("Bearer {}", token),
                    }));
                }
            }
            Err(ProviderError::Authentication(
                "Neither ANTHROPIC_API_KEY nor ANTHROPIC_AUTH_TOKEN is configured.".to_string(),
            ))
        }
        ProviderKind::OpenAICompat => {
            if is_env_set("OLLAMA_HOST") {
                // Local server mode via OLLAMA_HOST - omit auth header
                return Ok(None);
            }
            if let Ok(key) = std::env::var("OPENAI_API_KEY") {
                if !key.trim().is_empty() {
                    return Ok(Some(AuthHeader {
                        name: "Authorization".to_string(),
                        value: format!("Bearer {}", key),
                    }));
                }
            }
            Err(ProviderError::Authentication(
                "OPENAI_API_KEY is not configured and OLLAMA_HOST is not set.".to_string(),
            ))
        }
        ProviderKind::XAI => {
            if let Ok(key) = std::env::var("XAI_API_KEY") {
                if !key.trim().is_empty() {
                    return Ok(Some(AuthHeader {
                        name: "Authorization".to_string(),
                        value: format!("Bearer {}", key),
                    }));
                }
            }
            Err(ProviderError::Authentication(
                "XAI_API_KEY is not configured.".to_string(),
            ))
        }
        ProviderKind::DashScope => {
            if let Ok(key) = std::env::var("DASHSCOPE_API_KEY") {
                if !key.trim().is_empty() {
                    return Ok(Some(AuthHeader {
                        name: "Authorization".to_string(),
                        value: format!("Bearer {}", key),
                    }));
                }
            }
            Err(ProviderError::Authentication(
                "DASHSCOPE_API_KEY is not configured.".to_string(),
            ))
        }
    }
}

#[async_trait]
pub trait ProviderClient {
    async fn send_message(&self, request: &MessageRequest) -> Result<MessageResponse, ProviderError>;
    async fn stream_message(
        &self,
        request: &MessageRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<crate::sse::SSEEvent, ProviderError>> + Send>>, ProviderError>;
}
