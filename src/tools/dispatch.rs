use std::path::{Path, PathBuf};
use serde_json::Value;

pub fn build_tool_definitions() -> Value {
    serde_json::json!([
        {
            "type": "function",
            "function": {
                "name": "shell",
                "description": "Execute a shell command and return stdout and stderr.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "command": { "type": "string", "description": "The shell command to execute." },
                        "timeout_ms": { "type": "integer", "description": "Timeout in milliseconds. Default 10000." }
                    },
                    "required": ["command"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "file_read",
                "description": "Read the contents of a file at the given path.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Absolute or workspace-relative file path." }
                    },
                    "required": ["path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "file_write",
                "description": "Write content to a file, creating parent directories if needed.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "File path to write to." },
                        "content": { "type": "string", "description": "Content to write." }
                    },
                    "required": ["path", "content"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "grep_search",
                "description": "Search for a regex pattern across files in a directory.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "pattern": { "type": "string", "description": "Regex pattern to search for." },
                        "path": { "type": "string", "description": "Directory or file to search within." },
                        "case_sensitive": { "type": "boolean", "description": "Default true." }
                    },
                    "required": ["pattern", "path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "list_files",
                "description": "List files and directories at a given path.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Directory path to list." },
                        "depth": { "type": "integer", "description": "Max recursion depth. Default 1." }
                    },
                    "required": ["path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "web_fetch",
                "description": "Fetches a URL.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "url": { "type": "string", "description": "The URL to fetch" }
                    },
                    "required": ["url"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "web_search",
                "description": "Searches the web.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "The search query" }
                    },
                    "required": ["query"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "todo_write",
                "description": "Writes a todo item.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "todos": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "id": { "type": "string" },
                                    "content": { "type": "string" },
                                    "status": { "type": "string", "enum": ["pending", "in_progress", "completed"] },
                                    "priority": { "type": "string", "enum": ["high", "medium", "low"] }
                                },
                                "required": ["content"]
                            }
                        }
                    },
                    "required": ["todos"]
                }
            }
        }
    ])
}

pub async fn dispatch_tool(name: &str, arguments_str: &str, workspace_root: &Path) -> String {
    let args: Value = match serde_json::from_str(arguments_str) {
        Ok(v) => v,
        Err(e) => return format!("error: failed to parse arguments JSON: {}", e),
    };

    let result = match name {
        "shell" => tool_shell(&args, workspace_root).await,
        "file_read" => tool_file_read(&args, workspace_root).await,
        "file_write" => tool_file_write(&args, workspace_root).await,
        "grep_search" => tool_grep_search(&args, workspace_root).await,
        "list_files" => tool_list_files(&args, workspace_root).await,
        "web_fetch" => {
            let tool = crate::tools::web_fetch::WebFetch;
            let ctx = crate::tools::ToolContext {
                workspace_roots: vec![workspace_root.to_path_buf()],
                cwd: workspace_root.to_path_buf(),
            };
            use crate::tools::Tool;
            match tool.execute(args.clone(), ctx).await {
                Ok(res) => Ok(res.message),
                Err(err) => Err(err.message),
            }
        },
        "web_search" => {
            let tool = crate::tools::web_search::WebSearch;
            let ctx = crate::tools::ToolContext {
                workspace_roots: vec![workspace_root.to_path_buf()],
                cwd: workspace_root.to_path_buf(),
            };
            use crate::tools::Tool;
            match tool.execute(args.clone(), ctx).await {
                Ok(res) => Ok(res.message),
                Err(err) => Err(err.message),
            }
        },
        "todo_write" => {
            let tool = crate::tools::todo_write::TodoWrite;
            let ctx = crate::tools::ToolContext {
                workspace_roots: vec![workspace_root.to_path_buf()],
                cwd: workspace_root.to_path_buf(),
            };
            use crate::tools::Tool;
            match tool.execute(args.clone(), ctx).await {
                Ok(res) => Ok(res.message),
                Err(err) => Err(err.message),
            }
        },
        _ => Err(format!("error: unknown tool '{}'", name)),
    };

    match result {
        Ok(res) => res,
        Err(err) => err,
    }
}

async fn tool_shell(args: &Value, workspace_root: &Path) -> Result<String, String> {
    let command = args.get("command").and_then(|v| v.as_str()).ok_or("error: missing 'command' parameter")?;
    let timeout_ms = args.get("timeout_ms").and_then(|v| v.as_u64()).unwrap_or(10000);

    let tokens = crate::path_scope::tokenize_payload(command);
    let paths = crate::path_scope::extract_paths(&tokens);
    for path in paths {
        let validation = crate::path_scope::validate_path(&path, &[workspace_root.to_path_buf()], workspace_root);
        if let crate::path_scope::ValidationResult::Denied { reason, candidate, resolved } = validation {
            return Err(format!("error: path scope denial for '{}': {} (resolved to {})", candidate, reason, resolved));
        }
    }

    let child = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("error: failed to spawn shell: {}", e))?;

    let timeout_duration = std::time::Duration::from_millis(timeout_ms);
    match tokio::time::timeout(timeout_duration, child.wait_with_output()).await {
        Ok(Ok(output)) => {
            let exit_code = output.status.code().unwrap_or(-1);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            Ok(format!("exit_code: {}\nstdout: {}\nstderr: {}", exit_code, stdout, stderr))
        }
        Ok(Err(e)) => Err(format!("error: command failed: {}", e)),
        Err(_) => Err(format!("error: timeout after {}ms", timeout_ms)),
    }
}

async fn tool_file_read(args: &Value, workspace_root: &Path) -> Result<String, String> {
    let path_str = args.get("path").and_then(|v| v.as_str()).ok_or("error: missing 'path' parameter")?;
    
    let path = resolve_path(path_str, workspace_root);
    if let crate::path_scope::ValidationResult::Denied { .. } = crate::path_scope::validate_path(&path.to_string_lossy(), &[workspace_root.to_path_buf()], workspace_root) {
        return Err("error: path is outside workspace".to_string());
    }

    tokio::fs::read_to_string(&path).await
        .map_err(|e| format!("could not read {}: {}", path.display(), e))
}

async fn tool_file_write(args: &Value, workspace_root: &Path) -> Result<String, String> {
    let path_str = args.get("path").and_then(|v| v.as_str()).ok_or("error: missing 'path' parameter")?;
    let content = args.get("content").and_then(|v| v.as_str()).ok_or("error: missing 'content' parameter")?;
    
    let path = resolve_path(path_str, workspace_root);
    if let crate::path_scope::ValidationResult::Denied { .. } = crate::path_scope::validate_path(&path.to_string_lossy(), &[workspace_root.to_path_buf()], workspace_root) {
        return Err("error: path is outside workspace".to_string());
    }

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await
            .map_err(|e| format!("error creating parent directories: {}", e))?;
    }

    tokio::fs::write(&path, content).await
        .map_err(|e| format!("could not write {}: {}", path.display(), e))?;
        
    Ok(format!("wrote {} bytes to {}", content.len(), path.display()))
}

async fn tool_grep_search(args: &Value, workspace_root: &Path) -> Result<String, String> {
    let pattern = args.get("pattern").and_then(|v| v.as_str()).ok_or("error: missing 'pattern' parameter")?;
    let path_str = args.get("path").and_then(|v| v.as_str()).ok_or("error: missing 'path' parameter")?;
    let _case_sensitive = args.get("case_sensitive").and_then(|v| v.as_bool()).unwrap_or(true);
    
    let base_path = resolve_path(path_str, workspace_root);
    if let crate::path_scope::ValidationResult::Denied { .. } = crate::path_scope::validate_path(&base_path.to_string_lossy(), &[workspace_root.to_path_buf()], workspace_root) {
        return Err("error: path is outside workspace".to_string());
    }

    // Try ripgrep first
    let mut cmd = tokio::process::Command::new("rg");
    cmd.arg("--json");
    if !_case_sensitive {
        cmd.arg("-i");
    }
    cmd.arg(pattern);
    cmd.arg(&base_path);
    
    if let Ok(output) = cmd.output().await {
        if output.status.success() || output.status.code() == Some(1) {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut matches = Vec::new();
            
            for line in stdout.lines() {
                if let Ok(json) = serde_json::from_str::<Value>(line) {
                    if json.get("type").and_then(|v| v.as_str()) == Some("match") {
                        if let Some(data) = json.get("data") {
                            let file = data.get("path").and_then(|p| p.get("text")).and_then(|t| t.as_str()).unwrap_or("?");
                            let line_num = data.get("line_number").and_then(|l| l.as_u64()).unwrap_or(0);
                            let text = data.get("lines").and_then(|l| l.get("text")).and_then(|t| t.as_str()).unwrap_or("");
                            matches.push(format!("{}:{}:{}", file, line_num, text.trim_end()));
                        }
                    }
                }
            }
            
            if matches.len() > 50 {
                let mut truncated = matches.into_iter().take(50).collect::<Vec<_>>();
                truncated.push("... truncated".to_string());
                return Ok(truncated.join("\n"));
            }
            if matches.is_empty() {
                return Ok("No matches found.".to_string());
            }
            return Ok(matches.join("\n"));
        }
    }
    
    // Fallback to regex walker
    let re = regex::RegexBuilder::new(pattern).case_insensitive(!_case_sensitive).build()
        .map_err(|e| format!("error: invalid regex: {}", e))?;
        
    let mut matches = Vec::new();
    let mut dirs = vec![base_path];
    
    while let Some(dir) = dirs.pop() {
        if dir.is_file() {
            if let Ok(content) = std::fs::read_to_string(&dir) {
                for (i, line) in content.lines().enumerate() {
                    if re.is_match(line) {
                        matches.push(format!("{}:{}:{}", dir.display(), i + 1, line));
                        if matches.len() > 50 {
                            matches.push("... truncated".to_string());
                            return Ok(matches.join("\n"));
                        }
                    }
                }
            }
        } else if dir.is_dir() {
            if let Ok(mut entries) = tokio::fs::read_dir(&dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    if let crate::path_scope::ValidationResult::Allowed { .. } = crate::path_scope::validate_path(&path.to_string_lossy(), &[workspace_root.to_path_buf()], workspace_root) {
                        dirs.push(path);
                    }
                }
            }
        }
    }
    
    if matches.is_empty() {
        Ok("No matches found.".to_string())
    } else {
        Ok(matches.join("\n"))
    }
}

async fn tool_list_files(args: &Value, workspace_root: &Path) -> Result<String, String> {
    let path_str = args.get("path").and_then(|v| v.as_str()).ok_or("error: missing 'path' parameter")?;
    let depth = args.get("depth").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
    
    let base_path = resolve_path(path_str, workspace_root);
    if let crate::path_scope::ValidationResult::Denied { .. } = crate::path_scope::validate_path(&base_path.to_string_lossy(), &[workspace_root.to_path_buf()], workspace_root) {
        return Err("error: path is outside workspace".to_string());
    }

    if !base_path.is_dir() {
        return Err(format!("error: path is not a directory: {}", base_path.display()));
    }

    let mut results = Vec::new();
    let mut queue = vec![(base_path.clone(), 0)];
    let mut count = 0;

    while let Some((current_dir, current_depth)) = queue.pop() {
        if current_depth > depth || count >= 200 {
            if count >= 200 {
                results.push("... truncated".to_string());
            }
            break;
        }

        if let Ok(mut entries) = tokio::fs::read_dir(&current_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                count += 1;
                if count >= 200 {
                    results.push("... truncated".to_string());
                    break;
                }
                
                let path = entry.path();
                let rel_path = path.strip_prefix(workspace_root).unwrap_or(&path).to_string_lossy().to_string();
                
                if path.is_dir() {
                    results.push(format!("/{}", rel_path));
                    queue.push((path, current_depth + 1));
                } else {
                    results.push(rel_path);
                }
            }
        }
    }
    
    if results.is_empty() {
        Ok("Directory is empty.".to_string())
    } else {
        Ok(results.join("\n"))
    }
}

fn resolve_path(path_str: &str, workspace_root: &Path) -> PathBuf {
    let expanded = crate::path_scope::expand_home(&crate::path_scope::expand_env_vars(path_str));
    if expanded.is_absolute() {
        expanded
    } else {
        workspace_root.join(expanded)
    }
}
