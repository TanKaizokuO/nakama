use super::{Tool, ToolError, ToolResult, PermissionMode};
use async_trait::async_trait;

pub struct TodoWrite;

#[async_trait]
impl Tool for TodoWrite {
    fn name(&self) -> &'static str { "todo_write" }
    fn description(&self) -> &'static str { "Writes a todo item." }
    fn required_permission(&self) -> PermissionMode { PermissionMode::WorkspaceWrite }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
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
        })
    }
    fn output_schema(&self) -> serde_json::Value { serde_json::json!({}) }
    
    async fn execute(&self, input: serde_json::Value, context: super::ToolContext) -> Result<ToolResult, ToolError> {
        let todos_input = input.get("todos").and_then(|v| v.as_array()).ok_or_else(|| ToolError {
            error_type: "InvalidInput".to_string(),
            message: "Missing 'todos' parameter".to_string(),
        })?;

        let workspace_root = context.workspace_roots.first().ok_or_else(|| ToolError {
            error_type: "WorkspaceError".to_string(),
            message: "No workspace root available".to_string(),
        })?;
        
        let claw_dir = workspace_root.join(".claw");
        let todos_file = claw_dir.join("todos.json");

        if !claw_dir.exists() {
            tokio::fs::create_dir_all(&claw_dir).await.map_err(|e| ToolError {
                error_type: "FsError".to_string(),
                message: format!("Failed to create .claw dir: {}", e),
            })?;
        }

        let mut existing_todos: Vec<serde_json::Value> = if todos_file.exists() {
            let content = tokio::fs::read_to_string(&todos_file).await.map_err(|e| ToolError {
                error_type: "FsError".to_string(),
                message: format!("Failed to read todos.json: {}", e),
            })?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Vec::new()
        };

        let now = chrono::Utc::now().to_rfc3339();

        for new_item in todos_input {
            let new_obj = match new_item.as_object() {
                Some(o) => o,
                None => continue,
            };

            let id_opt = new_obj.get("id").and_then(|v| v.as_str());
            let mut updated_existing = false;
            
            if let Some(id) = id_opt {
                if let Some(existing) = existing_todos.iter_mut().find(|t| t.get("id").and_then(|v| v.as_str()) == Some(id)) {
                    // Update existing
                    if let Some(existing_obj) = existing.as_object_mut() {
                        if let Some(content) = new_obj.get("content") {
                            existing_obj.insert("content".to_string(), content.clone());
                        }
                        if let Some(status) = new_obj.get("status") {
                            existing_obj.insert("status".to_string(), status.clone());
                        }
                        if let Some(priority) = new_obj.get("priority") {
                            existing_obj.insert("priority".to_string(), priority.clone());
                        }
                        existing_obj.insert("updated_at".to_string(), serde_json::json!(now));
                        updated_existing = true;
                    }
                }
            }

            if !updated_existing {
                // Create new
                let id = id_opt.map(|s| s.to_string()).unwrap_or_else(|| uuid::Uuid::new_v4().to_string()[..8].to_string());
                let content = new_obj.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let status = new_obj.get("status").and_then(|v| v.as_str()).unwrap_or("pending").to_string();
                let priority = new_obj.get("priority").and_then(|v| v.as_str()).unwrap_or("medium").to_string();
                
                let mut item = serde_json::Map::new();
                item.insert("id".to_string(), serde_json::json!(id));
                item.insert("content".to_string(), serde_json::json!(content));
                item.insert("status".to_string(), serde_json::json!(status));
                item.insert("priority".to_string(), serde_json::json!(priority));
                item.insert("created_at".to_string(), serde_json::json!(now.clone()));
                item.insert("updated_at".to_string(), serde_json::json!(now.clone()));
                
                existing_todos.push(serde_json::Value::Object(item));
            }
        }

        let out_json = serde_json::to_string_pretty(&existing_todos).map_err(|e| ToolError {
            error_type: "SerializeError".to_string(),
            message: format!("Failed to serialize todos: {}", e),
        })?;

        tokio::fs::write(&todos_file, out_json).await.map_err(|e| ToolError {
            error_type: "FsError".to_string(),
            message: format!("Failed to write todos.json: {}", e),
        })?;

        let total = existing_todos.len();
        let mut pending = 0;
        let mut in_progress = 0;
        let mut completed = 0;
        
        for t in &existing_todos {
            if let Some(s) = t.get("status").and_then(|v| v.as_str()) {
                match s {
                    "pending" => pending += 1,
                    "in_progress" => in_progress += 1,
                    "completed" => completed += 1,
                    _ => {}
                }
            }
        }

        Ok(ToolResult {
            handled: true,
            message: format!("Todos updated. {} items total ({} pending, {} in_progress, {} completed).", total, pending, in_progress, completed),
            error_type: None,
            data: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_todo_write() {
        let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scratch");
        std::fs::create_dir_all(&workspace).unwrap();
        
        let ctx = crate::tools::ToolContext {
            workspace_roots: vec![workspace.clone()],
            cwd: workspace.clone(),
        };
        let todo = TodoWrite;

        let todos_file = workspace.join(".claw/todos.json");
        if todos_file.exists() {
            std::fs::remove_file(&todos_file).unwrap();
        }

        let res1 = todo.execute(serde_json::json!({"todos": [{"content": "Write tests", "priority": "high"}]}), ctx.clone()).await.unwrap();
        assert!(res1.message.contains("1 items total (1 pending, 0 in_progress, 0 completed)"));

        let content = std::fs::read_to_string(&todos_file).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        let id = v[0]["id"].as_str().unwrap().to_string();

        let res2 = todo.execute(serde_json::json!({"todos": [{"id": id.clone(), "status": "completed"}]}), ctx.clone()).await.unwrap();
        assert!(res2.message.contains("1 items total (0 pending, 0 in_progress, 1 completed)"));

        let content2 = std::fs::read_to_string(&todos_file).unwrap();
        let v2: serde_json::Value = serde_json::from_str(&content2).unwrap();
        assert_eq!(v2.as_array().unwrap().len(), 1);
        assert_eq!(v2[0]["status"], "completed");
    }
}
