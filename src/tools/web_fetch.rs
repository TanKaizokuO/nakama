use super::{Tool, ToolError, ToolResult, PermissionMode};
use async_trait::async_trait;
use futures::StreamExt;

pub struct WebFetch;

#[async_trait]
impl Tool for WebFetch {
    fn name(&self) -> &'static str { "web_fetch" }
    fn description(&self) -> &'static str { "Fetches a URL." }
    fn required_permission(&self) -> PermissionMode { PermissionMode::DangerFullAccess }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "The URL to fetch" }
            },
            "required": ["url"]
        })
    }
    fn output_schema(&self) -> serde_json::Value { serde_json::json!({}) }
    
    async fn execute(&self, input: serde_json::Value, _context: super::ToolContext) -> Result<ToolResult, ToolError> {
        let url_str = match input.get("url").and_then(|v| v.as_str()) {
            Some(u) => u,
            None => return Err(ToolError {
                error_type: "InvalidInput".to_string(),
                message: "Missing 'url' parameter".to_string(),
            }),
        };

        let parsed_url = match reqwest::Url::parse(url_str) {
            Ok(u) => u,
            Err(e) => return Err(ToolError {
                error_type: "InvalidUrl".to_string(),
                message: format!("Invalid URL: {}", e),
            }),
        };

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| ToolError {
                error_type: "ClientError".to_string(),
                message: format!("Failed to build HTTP client: {}", e),
            })?;

        let response = match client.get(parsed_url).send().await {
            Ok(r) => r,
            Err(e) => return Err(ToolError {
                error_type: "NetworkError".to_string(),
                message: format!("WebFetch network error: {}", e),
            }),
        };

        let final_url = response.url().to_string();
        
        let status = response.status();
        if !status.is_success() {
            return Err(ToolError {
                error_type: "HttpError".to_string(),
                message: format!("WebFetch error: {} {}", status.as_u16(), status.canonical_reason().unwrap_or("Unknown")),
            });
        }

        let content_type = response.headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("text/plain")
            .to_lowercase();

        if !content_type.starts_with("text/") && !content_type.starts_with("application/json") {
            return Err(ToolError {
                error_type: "UnsupportedType".to_string(),
                message: format!("WebFetch: binary content type not supported ({})", content_type),
            });
        }

        let mut content_bytes = Vec::new();
        let mut stream = response.bytes_stream();
        let mut truncated = false;
        
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(|e| ToolError {
                error_type: "ReadError".to_string(),
                message: format!("Failed to read response body: {}", e),
            })?;
            
            if content_bytes.len() + chunk.len() > 2 * 1024 * 1024 {
                let remaining = 2 * 1024 * 1024 - content_bytes.len();
                content_bytes.extend_from_slice(&chunk[..remaining]);
                truncated = true;
                break;
            } else {
                content_bytes.extend_from_slice(&chunk);
            }
        }

        let mut content = String::from_utf8_lossy(&content_bytes).to_string();

        if content_type.starts_with("text/html") {
            content = strip_html(&content);
        }

        if truncated {
            content.push_str("\n[Content truncated at 2MB limit]");
        }

        Ok(ToolResult {
            handled: true,
            message: format!("URL: {}\n\n{}", final_url, content),
            error_type: None,
            data: None,
        })
    }
}

fn strip_html(html: &str) -> String {
    // Basic regex-free tag stripping to avoid new dependencies, or use regex if it's there
    let re = regex::Regex::new(r"(?s)<[^>]*>").unwrap();
    let stripped = re.replace_all(html, " ");
    
    // basic entity decoding
    let decoded = stripped
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");
        
    // collapse whitespace
    let ws_re = regex::Regex::new(r"\s+").unwrap();
    ws_re.replace_all(&decoded, " ").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_web_fetch() {
        let fetch = WebFetch;
        let ctx = crate::tools::ToolContext {
            workspace_roots: vec![PathBuf::from("/tmp")],
            cwd: PathBuf::from("/tmp"),
        };
        
        let res = fetch.execute(serde_json::json!({"url": "https://example.com"}), ctx).await.unwrap();
        assert!(res.message.contains("URL: https://example.com"));
        assert!(!res.message.contains("<html>"));
    }
}
