use super::{Tool, ToolError, ToolResult, PermissionMode};
use async_trait::async_trait;

pub struct WebSearch;

#[async_trait]
impl Tool for WebSearch {
    fn name(&self) -> &'static str { "web_search" }
    fn description(&self) -> &'static str { "Searches the web." }
    fn required_permission(&self) -> PermissionMode { PermissionMode::DangerFullAccess }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "The search query" }
            },
            "required": ["query"]
        })
    }
    fn output_schema(&self) -> serde_json::Value { serde_json::json!({}) }
    
    async fn execute(&self, input: serde_json::Value, context: super::ToolContext) -> Result<ToolResult, ToolError> {
        let query = match input.get("query").and_then(|v| v.as_str()) {
            Some(q) => q,
            None => return Err(ToolError {
                error_type: "InvalidInput".to_string(),
                message: "Missing 'query' parameter".to_string(),
            }),
        };

        let search_url = match reqwest::Url::parse_with_params("https://html.duckduckgo.com/html/", &[("q", query)]) {
            Ok(u) => u,
            Err(e) => return Err(ToolError {
                error_type: "UrlError".to_string(),
                message: format!("Failed to construct search URL: {}", e),
            }),
        };

        let fetcher = crate::tools::web_fetch::WebFetch;
        let fetch_input = serde_json::json!({ "url": search_url.as_str() });
        
        let mut result = fetcher.execute(fetch_input, context).await?;
        result.message = format!("Search results for: {}\n\n{}", query, result.message);
        
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_web_search() {
        let search = WebSearch;
        let ctx = crate::tools::ToolContext {
            workspace_roots: vec![PathBuf::from("/tmp")],
            cwd: PathBuf::from("/tmp"),
        };
        
        let res = search.execute(serde_json::json!({"query": "rust async programming"}), ctx).await.unwrap();
        assert!(res.message.starts_with("Search results for: rust async programming"));
    }
}
