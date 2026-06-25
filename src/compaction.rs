use crate::data_contracts::{ContentBlock, MessageRole, SessionMessageRecord};
use std::collections::HashSet;

pub struct CompactionConfig {
    pub preservation_count: usize,
    pub max_budget: usize,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            preservation_count: 10,
            max_budget: 8000,
        }
    }
}

pub struct CompactionEngine {
    config: CompactionConfig,
}

impl CompactionEngine {
    pub fn new(config: CompactionConfig) -> Self {
        Self { config }
    }

    pub fn estimate_tokens(msg: &SessionMessageRecord) -> usize {
        let mut total = 0;
        for block in &msg.content {
            match block {
                ContentBlock::Text { text } => {
                    total += (text.len() / 4) + 1;
                }
                ContentBlock::ToolUse { id, name, input } => {
                    let input_str = serde_json::to_string(input).unwrap_or_default();
                    total += (id.len() / 4) + 1;
                    total += (name.len() / 4) + 1;
                    total += (input_str.len() / 4) + 1;
                }
                ContentBlock::ToolResult { tool_use_id, content, .. } => {
                    total += (tool_use_id.len() / 4) + 1;
                    total += (content.len() / 4) + 1;
                }
                ContentBlock::Thinking { thinking, signature } => {
                    total += (thinking.len() / 4) + 1;
                    if let Some(s) = signature {
                        total += (s.len() / 4) + 1;
                    }
                }
                ContentBlock::RedactedThinking { data } => {
                    total += (data.len() / 4) + 1;
                }
            }
        }
        total
    }

    pub fn maybe_compact(&self, messages: Vec<SessionMessageRecord>) -> (Vec<SessionMessageRecord>, Option<crate::data_contracts::CompactionRecord>) {
        if messages.is_empty() {
            return (messages, None);
        }

        let mut start_idx = 0;
        let mut prior_summary_text = String::new();
        
        if !messages.is_empty() && messages[0].role == MessageRole::System {
            if let Some(ContentBlock::Text { text }) = messages[0].content.first() {
                if text.starts_with("This is a continuation of a previous conversation") {
                    start_idx = 1;
                    prior_summary_text = text.clone();
                }
            }
        }

        let compactable_count = messages.len() - start_idx;
        if compactable_count <= self.config.preservation_count {
            return (messages, None);
        }

        let estimated_total: usize = messages[start_idx..].iter().map(Self::estimate_tokens).sum();
        if estimated_total <= self.config.max_budget {
            return (messages, None);
        }

        let mut preserve_start = messages.len().saturating_sub(self.config.preservation_count);
        
        while preserve_start > start_idx && preserve_start < messages.len() {
            if messages[preserve_start].role == MessageRole::Tool {
                preserve_start -= 1;
            } else if messages[preserve_start].role == MessageRole::Assistant && 
                      messages[preserve_start].content.iter().any(|b| matches!(b, ContentBlock::ToolUse { .. })) {
                break;
            } else {
                break;
            }
        }

        if preserve_start <= start_idx {
            preserve_start = start_idx + 1;
        }

        let removed = &messages[start_idx..preserve_start];
        if removed.is_empty() {
            return (messages, None);
        }

        let new_summary_text = self.summarize(removed);
        
        let combined_summary = if !prior_summary_text.is_empty() {
            self.flatten_summaries(&prior_summary_text, &new_summary_text)
        } else {
            new_summary_text.clone()
        };

        let continuation_msg = SessionMessageRecord {
            role: MessageRole::System,
            content: vec![ContentBlock::Text {
                text: format!(
                    "This is a continuation of a previous conversation. Here is a summary of what was discussed:\n\n{}\n\nThe following {} messages are preserved from the recent conversation.\nPlease continue from where we left off without recapping what was already discussed.",
                    combined_summary,
                    messages.len() - preserve_start
                )
            }],
            usage: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            tool_call_id: None,
        };

        let mut new_messages = vec![continuation_msg];
        new_messages.extend_from_slice(&messages[preserve_start..]);

        let record = crate::data_contracts::CompactionRecord {
            summary_text: combined_summary,
            removed_count: removed.len(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        (new_messages, Some(record))
    }

    fn summarize(&self, messages: &[SessionMessageRecord]) -> String {
        let mut user_count = 0;
        let mut asst_count = 0;
        let mut tool_count = 0;
        let mut sys_count = 0;

        let mut tools = HashSet::new();
        let mut recent_requests = Vec::new();
        let mut pending_items = Vec::new();
        let mut key_files = HashSet::new();
        let mut last_work = String::new();
        let mut timeline = Vec::new();

        let pending_keywords = ["todo", "next", "pending", "follow up", "remaining"];

        for msg in messages {
            match msg.role {
                MessageRole::User => user_count += 1,
                MessageRole::Assistant => asst_count += 1,
                MessageRole::Tool => tool_count += 1,
                MessageRole::System => sys_count += 1,
            }

            let mut msg_text = String::new();
            for block in &msg.content {
                match block {
                    ContentBlock::Text { text } => {
                        msg_text.push_str(text);
                        msg_text.push(' ');
                        last_work = text.clone();
                    }
                    ContentBlock::ToolUse { name, .. } => {
                        tools.insert(name.clone());
                    }
                    _ => {}
                }
            }

            if msg.role == MessageRole::User {
                recent_requests.push(msg_text.clone());
                if recent_requests.len() > 3 {
                    recent_requests.remove(0);
                }
            }

            let msg_text_lower = msg_text.to_lowercase();
            if pending_keywords.iter().any(|k| msg_text_lower.contains(k)) {
                pending_items.push(msg_text.clone());
            }

            for word in msg_text.split_whitespace() {
                if word.contains('.') && (word.ends_with(".rs") || word.ends_with(".py") || word.ends_with(".js") || word.ends_with(".ts") || word.ends_with(".md")) {
                    key_files.insert(word.to_string());
                }
            }

            let mut timeline_entry = msg_text.trim().to_string();
            if timeline_entry.len() > 80 {
                timeline_entry.truncate(80);
                timeline_entry.push_str("...");
            }
            timeline.push(format!("{:?}: {}", msg.role, timeline_entry));
        }

        let truncate_160 = |s: &String| {
            if s.len() > 160 {
                format!("{}...", &s[..160])
            } else {
                s.clone()
            }
        };

        let mut out = String::new();
        out.push_str(&format!("Scope: User: {}, Assistant: {}, Tool: {}, System: {}\n", user_count, asst_count, tool_count, sys_count));
        out.push_str(&format!("Unique tool names: {:?}\n", tools));
        
        out.push_str("Recent user requests:\n");
        for r in &recent_requests {
            out.push_str(&format!("- {}\n", truncate_160(r)));
        }

        out.push_str("Pending items:\n");
        for p in &pending_items {
            out.push_str(&format!("- {}\n", truncate_160(p)));
        }

        out.push_str(&format!("Key files referenced: {:?}\n", key_files));
        out.push_str(&format!("Current work inference: {}\n", truncate_160(&last_work)));
        
        out.push_str("Key timeline:\n");
        for t in &timeline {
            out.push_str(&format!("- {}\n", t));
        }

        out
    }

    fn flatten_summaries(&self, prior: &str, new: &str) -> String {
        format!("{}\n\n=== Recent ===\n{}", prior, new)
    }
}
