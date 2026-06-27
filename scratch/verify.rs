use std::env;
use std::process::Command;
use std::path::PathBuf;

fn main() {
    println!("Testing rules_import deserialization...");
    let json = r#"{"rules_import": "auto"}"#;
    let config: nakama::config::AppConfig = serde_json::from_str(json).unwrap();
    println!("Auto parsed: {:?}", config.rules_import);

    let json = r#"{"rules_import": "none"}"#;
    let config: nakama::config::AppConfig = serde_json::from_str(json).unwrap();
    println!("None parsed: {:?}", config.rules_import);

    let json = r#"{"rules_import": ["file1.md", "file2.md"]}"#;
    let config: nakama::config::AppConfig = serde_json::from_str(json).unwrap();
    println!("Explicit parsed: {:?}", config.rules_import);

    let json = r#"{"rules_import": "unknown"}"#;
    let res: Result<nakama::config::AppConfig, _> = serde_json::from_str(json);
    println!("Unknown parsed: {:?}", res.is_ok());

    println!("\nTesting glob expansion...");
    let tokens = nakama::path_scope::tokenize_payload("ls /*");
    let paths = nakama::path_scope::extract_paths(&tokens);
    let workspace_root = env::current_dir().unwrap();
    for path in paths {
        let res = nakama::path_scope::validate_path(&path, &[workspace_root.clone()], &workspace_root);
        println!("Glob path validation result for {}: {:?}", path, res);
    }
}
