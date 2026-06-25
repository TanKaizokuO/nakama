use crate::models::{MessageResponse, TokenUsage};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelRates {
    pub input_rate: f64,
    pub output_rate: f64,
    pub cache_write_rate: f64,
    pub cache_read_rate: f64,
}

// Rate constants (per 1,000,000 tokens)
pub const HAIKU_RATES: ModelRates = ModelRates {
    input_rate: 1.00,
    output_rate: 5.00,
    cache_write_rate: 1.25,
    cache_read_rate: 0.10,
};

pub const SONNET_RATES: ModelRates = ModelRates {
    input_rate: 15.00,
    output_rate: 75.00,
    cache_write_rate: 18.75,
    cache_read_rate: 1.50,
};

pub const OPUS_RATES: ModelRates = ModelRates {
    input_rate: 15.00,
    output_rate: 75.00,
    cache_write_rate: 18.75,
    cache_read_rate: 1.50,
};

pub const UNKNOWN_RATES: ModelRates = ModelRates {
    input_rate: 15.00,
    output_rate: 75.00,
    cache_write_rate: 18.75,
    cache_read_rate: 1.50,
};

/// Resolves pricing rates for a given model name.
pub fn get_model_rates(model: &str) -> ModelRates {
    let lower = model.to_lowercase();
    if lower.contains("haiku") {
        HAIKU_RATES
    } else if lower.contains("sonnet") {
        SONNET_RATES
    } else if lower.contains("opus") {
        OPUS_RATES
    } else {
        UNKNOWN_RATES
    }
}

/// Calculates f64 cost in USD for a given TokenUsage and ModelRates.
pub fn calculate_cost(usage: TokenUsage, rates: ModelRates) -> f64 {
    let input = usage.input_tokens as f64 * rates.input_rate;
    let output = usage.output_tokens as f64 * rates.output_rate;
    let cache_write = usage.cache_creation_tokens as f64 * rates.cache_write_rate;
    let cache_read = usage.cache_read_tokens as f64 * rates.cache_read_rate;

    (input + output + cache_write + cache_read) / 1_000_000.0
}

/// Formats the cost in USD with 4 decimal places (e.g. "$0.0150").
pub fn format_cost(cost: f64) -> String {
    format!("${:.4}", cost)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UsageTracker {
    pub latest_turn_usage: TokenUsage,
    pub cumulative_usage: TokenUsage,
    pub turn_count: u32,
}

impl UsageTracker {
    pub fn new() -> Self {
        Self {
            latest_turn_usage: TokenUsage::default(),
            cumulative_usage: TokenUsage::default(),
            turn_count: 0,
        }
    }

    /// Records the usage of a single turn.
    pub fn record(&mut self, usage: TokenUsage) {
        self.latest_turn_usage = usage;
        self.cumulative_usage.input_tokens += usage.input_tokens;
        self.cumulative_usage.output_tokens += usage.output_tokens;
        self.cumulative_usage.cache_creation_tokens += usage.cache_creation_tokens;
        self.cumulative_usage.cache_read_tokens += usage.cache_read_tokens;
        self.turn_count += 1;
    }

    /// Reconstructs the usage tracker from a slice of MessageResponse records.
    pub fn reconstruct_from_messages(&mut self, messages: &[MessageResponse]) {
        *self = Self::new();
        for msg in messages {
            let usage = msg.token_usage;
            let total = usage.input_tokens
                + usage.output_tokens
                + usage.cache_creation_tokens
                + usage.cache_read_tokens;

            // Only record if the message has non-zero tokens.
            if total > 0 {
                self.record(usage);
            }
        }
    }
}
