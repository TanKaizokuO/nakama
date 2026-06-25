use crate::config::Config;

pub fn resolve_alias(model: &str, config: &Config) -> String {
    // 1. Check user-defined aliases in config (exact case match)
    if let Some(aliases) = &config.model_aliases {
        if let Some(resolved) = aliases.get(model) {
            return resolved.clone();
        }
    }

    // 2. Check built-in aliases (case-insensitive check)
    match model.to_lowercase().as_str() {
        "opus" => "claude-opus-4-7".to_string(),
        "sonnet" => "claude-sonnet-4-6".to_string(),
        "haiku" => "claude-haiku-4-5-20251213".to_string(),
        "grok" | "grok-3" => "grok-3".to_string(),
        "grok-mini" | "grok-3-mini" => "grok-3-mini".to_string(),
        "kimi" => "kimi-k2.5".to_string(),
        "qwen-max" => "qwen-max".to_string(),
        "qwen-plus" => "qwen-plus".to_string(),
        // 3. Fallback: pass-through verbatim
        _ => model.to_string(),
    }
}
