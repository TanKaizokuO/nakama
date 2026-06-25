// Option A: Separate NimAccumulator for OpenAI-compatible SSE chunks.
// The existing AccumulatorState (Anthropic format) is left untouched.
//
// OpenAI SSE format:
//   data: {"id":"...","object":"chat.completion.chunk","choices":[{"delta":{"content":"hello"},"finish_reason":null}]}
//   data: [DONE]
//
// When stream_options.include_usage is true, the final chunk before [DONE]
// carries a top-level "usage" object with prompt_tokens and completion_tokens.
// If the provider does not honour this flag, token counts will be zero and a
// warning is logged (see G1 in the implementation plan).

use crate::data_contracts::UsageRecord;

/// Accumulates OpenAI-compatible SSE streaming chunks into a single text
/// response with token usage and stop reason.
pub struct NimAccumulator {
    text_buffer: String,
    input_tokens: u32,
    output_tokens: u32,
    stop_reason: Option<String>,
    done: bool,
}

/// The text fragment extracted from a single SSE chunk, if any.
/// Returned by `process_line` so the caller can print it immediately.
pub type ChunkText = Option<String>;

impl NimAccumulator {
    pub fn new() -> Self {
        Self {
            text_buffer: String::new(),
            input_tokens: 0,
            output_tokens: 0,
            stop_reason: None,
            done: false,
        }
    }

    /// Process a single raw SSE line (the part after "data: ").
    ///
    /// Returns `Some(text)` if this chunk contained a content delta that
    /// should be printed to stdout immediately. Returns `None` if the chunk
    /// was a control signal, had no content, or was the `[DONE]` sentinel.
    pub fn process_line(&mut self, data: &str) -> ChunkText {
        let data = data.trim();

        // Stream termination sentinel
        if data == "[DONE]" {
            self.done = true;
            return None;
        }

        // Parse the JSON chunk
        let chunk: serde_json::Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Warning: failed to parse SSE chunk JSON: {}", e);
                return None;
            }
        };

        // Extract usage from the chunk if present (typically on the final data chunk
        // when stream_options.include_usage is true).
        if let Some(usage) = chunk.get("usage") {
            if let Some(pt) = usage.get("prompt_tokens").and_then(|v| v.as_u64()) {
                self.input_tokens = pt as u32;
            }
            if let Some(ct) = usage.get("completion_tokens").and_then(|v| v.as_u64()) {
                self.output_tokens = ct as u32;
            }
        }

        // Extract choices[0]
        let choice = match chunk.get("choices").and_then(|c| c.get(0)) {
            Some(c) => c,
            None => return None, // No choices array — could be a usage-only final chunk
        };

        // Check finish_reason
        if let Some(reason) = choice.get("finish_reason").and_then(|v| v.as_str()) {
            self.stop_reason = Some(reason.to_string());
        }

        // Extract delta.content — ignore if null or absent
        let content = choice
            .get("delta")
            .and_then(|d| d.get("content"))
            .and_then(|c| c.as_str());

        match content {
            Some(text) if !text.is_empty() => {
                self.text_buffer.push_str(text);
                Some(text.to_string())
            }
            _ => None, // null, absent, or empty — ignore per spec
        }
    }

    /// Returns true after `data: [DONE]` has been received.
    pub fn is_done(&self) -> bool {
        self.done
    }

    /// Consume the accumulator and return the final result.
    /// Takes `self` by value — no clone of text_buffer needed.
    ///
    /// Returns (full_text, usage_record, stop_reason).
    /// If usage tokens are both zero after a completed stream, logs a warning
    /// per G1 — the provider may not have honoured stream_options.include_usage.
    pub fn into_result(self) -> (String, UsageRecord, Option<String>) {
        if self.done && self.input_tokens == 0 && self.output_tokens == 0 {
            eprintln!(
                "Warning: stream completed but no token usage was reported. \
                 The provider may not support stream_options.include_usage."
            );
        }

        let usage = UsageRecord {
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
            cache_creation_tokens: 0, // NIM does not return cache metrics
            cache_read_tokens: 0,
        };

        (self.text_buffer, usage, self.stop_reason)
    }
}
