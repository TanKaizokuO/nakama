use crate::data_contracts::{StructuredOutputFallback, ContentBlock};
use thiserror::Error;
use std::time::Duration;

#[derive(Error, Debug)]
pub enum ProviderError {
    #[error("Request body exceeds provider size limit")]
    SizeLimitExceeded,
    #[error("Context window exceeded after compaction")]
    ContextWindowExceeded,
    #[error("Rate limited by provider. Retry after {0:?}")]
    RateLimited(Option<Duration>),
    #[error("Provider connection error: {0}")]
    ConnectionError(String),
    #[error("SSE stream interrupted: {error_msg}")]
    SseInterrupted {
        error_msg: String,
        partial_response: Vec<ContentBlock>,
    },
    #[error("Structured output serialization failed completely after {retries} retries. Fallback payload generated.")]
    StructuredOutputFailure {
        retries: usize,
        fallback: StructuredOutputFallback,
    },
}

pub struct StructuredOutputRetrier {
    max_retries: usize,
}

impl Default for StructuredOutputRetrier {
    fn default() -> Self {
        Self { max_retries: 2 }
    }
}

impl StructuredOutputRetrier {
    pub fn new(max_retries: usize) -> Self {
        Self { max_retries }
    }

    pub fn try_serialize<T, F>(&self, mut attempt_fn: F, session_id: &str, prompt: &str, best_effort_text: &str) -> Result<T, ProviderError>
    where
        F: FnMut() -> Result<T, serde_json::Error>,
    {
        let mut retries = 0;
        loop {
            match attempt_fn() {
                Ok(val) => return Ok(val),
                Err(e) => {
                    if retries >= self.max_retries {
                        return Err(ProviderError::StructuredOutputFailure {
                            retries,
                            fallback: StructuredOutputFallback {
                                session_id: session_id.to_string(),
                                prompt: prompt.to_string(),
                                output_text: best_effort_text.to_string(),
                                fallback_mode: true,
                            },
                        });
                    }
                    eprintln!("Structured output serialization failed: {}. Retrying...", e);
                    retries += 1;
                }
            }
        }
    }
}

pub fn handle_rate_limit(response_headers: &reqwest::header::HeaderMap) -> ProviderError {
    let mut retry_after = None;
    if let Some(val) = response_headers.get(reqwest::header::RETRY_AFTER) {
        if let Ok(val_str) = val.to_str() {
            if let Ok(secs) = val_str.parse::<u64>() {
                retry_after = Some(Duration::from_secs(secs));
            }
        }
    }
    ProviderError::RateLimited(retry_after)
}
