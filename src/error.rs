use thiserror::Error;

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("No credential set. Expected one of: ANTHROPIC_API_KEY. {hint}")]
    NoCredential {
        hint: String,
    },
    #[error("API key in wrong variable. Move the key to ANTHROPIC_API_KEY")]
    WrongVariable,
    #[error("Expired OAuth token. Refresh attempt failed. Please re-authenticate.")]
    ExpiredOAuthToken,
    #[error("Provider error: HTTP {0}")]
    InvalidKey(u16),
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Invalid MCP server config: {reason}")]
    InvalidMcpServer {
        error_field: String,
        reason: String,
    },
    #[error("Unknown hook event: {event_name}")]
    InvalidHook {
        event_name: String,
    },
    #[error("Invalid output format: {value}. Expected: [\"text\", \"json\"]")]
    InvalidOutputFormat {
        value: String,
    },
    #[error("Invalid tool name: {name}. Available: {available:?}")]
    InvalidToolName {
        name: String,
        available: Vec<String>,
        aliases: Vec<String>,
    },
    #[error("Missing argument: {argument}")]
    MissingArgument {
        argument: String,
    },
    #[error("Invalid working directory path")]
    InvalidCwd,
}
