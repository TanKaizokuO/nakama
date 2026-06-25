use crate::config::Config;

pub fn resolve_model(cli_model: Option<&str>, config: &Config) -> String {
    // 1. CLI flag
    if let Some(m) = cli_model {
        if !m.trim().is_empty() {
            return m.trim().to_string();
        }
    }

    // 2. NAKAMA_MODEL env var
    if let Ok(m) = std::env::var("NAKAMA_MODEL") {
        if !m.trim().is_empty() {
            return m.trim().to_string();
        }
    }

    // 3. ANTHROPIC_MODEL env var
    if let Ok(m) = std::env::var("ANTHROPIC_MODEL") {
        if !m.trim().is_empty() {
            return m.trim().to_string();
        }
    }

    // 4. ANTHROPIC_DEFAULT_MODEL env var
    if let Ok(m) = std::env::var("ANTHROPIC_DEFAULT_MODEL") {
        if !m.trim().is_empty() {
            return m.trim().to_string();
        }
    }

    // 5. Config file default model
    if let Some(m) = &config.model {
        if !m.trim().is_empty() {
            return m.trim().to_string();
        }
    }

    // 6. Hardcoded default
    "claude-sonnet-4-6".to_string()
}
