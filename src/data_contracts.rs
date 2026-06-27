use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
    Thinking {
        thinking: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
    RedactedThinking {
        data: String,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct UsageRecord {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_creation_tokens: u32,
    pub cache_read_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionMessageRecord {
    pub role: MessageRole,
    pub content: Vec<ContentBlock>,
    pub usage: Option<UsageRecord>,
    pub timestamp: String,
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompactionRecord {
    pub summary_text: String,
    pub removed_count: usize,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionMetadata {
    pub session_id: String,
    pub created_at: String,
    pub model: String,
    pub permission_mode: String,
    pub heartbeat: String,
    pub liveness: bool,
    pub compaction_history: Vec<CompactionRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum SessionMetadataRecord {
    #[serde(rename = "session_meta")]
    SessionMeta {
        session_id: String,
        created_at: String,
        model: String,
        permission_mode: String,
        heartbeat: String,
        liveness: bool,
        compaction_history: Vec<CompactionRecord>,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkerState {
    pub worker_id: String,
    pub session_id: String,
    pub model: String,
    pub permission_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StructuredOutputFallback {
    pub session_id: String,
    pub prompt: String,
    pub output_text: String,
    pub fallback_mode: bool, // always true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StagePermissionMode {
    Auto,    // approve everything within workspace
    Prompt,  // ask user before each tool call
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthHeader {
    Bearer,
    XApiKey,
}

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub auth_header: AuthHeader,
}


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AccumulatedToolCall {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderTurnResult {
    pub text: String,
    pub tool_calls: Vec<AccumulatedToolCall>,
    pub stop_reason: Option<String>,
    pub usage: Option<UsageRecord>,
}
