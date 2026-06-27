use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationResult {
    Allowed { reason: String },
    Denied { candidate: String, resolved: String, reason: String },
}

/// Tokenizes a command/payload using POSIX shell syntax, falling back to whitespace splitting on unmatched quotes.
pub fn tokenize_payload(payload: &str) -> Vec<String> {
    if let Some(tokens) = shlex::split(payload) {
        tokens
    } else {
        // Fallback: split on whitespace, strip matching external quotes
        payload
            .split_whitespace()
            .map(|token| {
                let mut s = token.to_string();
                if (s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2)
                    || (s.starts_with('"') && s.ends_with('"') && s.len() >= 2)
                {
                    s.remove(0);
                    s.pop();
                }
                s
            })
            .collect()
    }
}

/// Helper to check if a token is path-like.
pub fn is_path_like(token: &str) -> bool {
    token.contains('/')
        || token.contains('\\')
        || token.starts_with("./")
        || token.starts_with("../")
        || token.starts_with("~/")
        || token == "."
        || token == ".."
        || token.contains('*')
        || token.contains('?')
        || token.contains('[')
        || (token.len() >= 3 && token.chars().next().unwrap().is_alphabetic() && &token[1..3] == ":\\")
        || (token.len() >= 3 && token.chars().next().unwrap().is_alphabetic() && &token[1..3] == ":/")
        || token.starts_with("\\\\")
}

/// Helper to expand environment variables like $VAR or ${VAR} in a string.
pub fn expand_env_vars(token: &str) -> String {
    let mut result = String::new();
    let mut chars = token.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '$' {
            if let Some(&'{') = chars.peek() {
                chars.next(); // consume '{'
                let mut var_name = String::new();
                while let Some(vc) = chars.next() {
                    if vc == '}' {
                        break;
                    }
                    var_name.push(vc);
                }
                if let Ok(val) = std::env::var(&var_name) {
                    result.push_str(&val);
                }
            } else {
                let mut var_name = String::new();
                while let Some(&vc) = chars.peek() {
                    if vc.is_alphanumeric() || vc == '_' {
                        chars.next();
                        var_name.push(vc);
                    } else {
                        break;
                    }
                }
                if !var_name.is_empty() {
                    if let Ok(val) = std::env::var(&var_name) {
                        result.push_str(&val);
                    }
                } else {
                    result.push('$');
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Helper to expand home directory shorthand `~`.
pub fn expand_home(token: &str) -> PathBuf {
    if token == "~" {
        home::home_dir().unwrap_or_else(|| PathBuf::from("~"))
    } else if token.starts_with("~/") {
        if let Some(home) = home::home_dir() {
            home.join(&token[2..])
        } else {
            PathBuf::from(token)
        }
    } else {
        PathBuf::from(token)
    }
}

/// Extract redirection targets and path-like tokens from tokens.
pub fn extract_paths(tokens: &[String]) -> Vec<String> {
    let mut paths = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let token = &tokens[i];

        // Filter out flags (starting with -) and env variables (KEY=VALUE)
        if token.starts_with('-') || (token.contains('=') && !token.contains('/') && !token.contains('\\')) {
            i += 1;
            continue;
        }

        // Redirection operators: >, >>, <, <>
        if token == ">" || token == ">>" || token == "<" || token == "<>" {
            if i + 1 < tokens.len() {
                paths.push(tokens[i + 1].clone());
                i += 2;
                continue;
            }
        }

        // Redirection operator prefix (e.g. >file or >>file)
        if token.starts_with(">>") {
            paths.push(token[2..].to_string());
            i += 1;
            continue;
        } else if token.starts_with('>') || token.starts_with('<') {
            paths.push(token[1..].to_string());
            i += 1;
            continue;
        }

        if is_path_like(token) {
            paths.push(token.clone());
        }

        i += 1;
    }
    paths
}

/// Helper to normalize path components to resolve '..' and '.' without filesystem dependency.
pub fn normalize_path(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                normalized.pop();
            }
            Component::CurDir => {}
            Component::Normal(c) => {
                normalized.push(c);
            }
            c => {
                normalized.push(c.as_os_str());
            }
        }
    }
    normalized
}

/// Robust canonicalization that handles non-existent paths by climbing to the first existing parent.
pub fn canonicalize_path(path: &Path) -> PathBuf {
    let mut existing = path.to_path_buf();
    let mut components = Vec::new();

    while !existing.exists() && existing.parent().is_some() {
        if let Some(name) = existing.file_name() {
            components.push(name.to_os_string());
        } else if let Some(last_comp) = existing.components().last() {
            components.push(last_comp.as_os_str().to_os_string());
        }
        existing.pop();
    }

    let mut resolved = std::fs::canonicalize(&existing).unwrap_or(existing);
    for comp in components.into_iter().rev() {
        resolved.push(comp);
    }
    normalize_path(&resolved)
}

/// Returns the stable non-glob prefix of a path containing glob characters.
pub fn get_stable_glob_prefix(path_str: &str) -> String {
    let mut prefix = String::new();
    for part in path_str.split('/') {
        if part.contains('*') || part.contains('?') || part.contains('[') {
            break;
        }
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(part);
    }
    if path_str.starts_with('/') && !prefix.starts_with('/') {
        prefix.insert(0, '/');
    }
    prefix
}

/// Validates whether a candidate path token is within the workspace roots.
pub fn validate_path(
    candidate_token: &str,
    workspace_roots: &[PathBuf],
    cwd: &Path,
) -> ValidationResult {
    let expanded_env = expand_env_vars(candidate_token);
    let expanded_path = expand_home(&expanded_env);

    // Resolve relative path against CWD
    let resolved_absolute = if expanded_path.is_absolute() {
        expanded_path
    } else {
        cwd.join(expanded_path)
    };

    let path_str = resolved_absolute.to_string_lossy().to_string();

    // Check if it contains glob metacharacters
    if path_str.contains('*') || path_str.contains('?') || path_str.contains('[') {
        match glob::glob(&path_str) {
            Ok(entries) => {
                let mut matched_any = false;
                let mut count = 0;
                for entry in entries {
                    matched_any = true;
                    count += 1;
                    if count > 1000 {
                        // Enforce safety limit of 1000 expansions
                        return ValidationResult::Denied {
                            candidate: candidate_token.to_string(),
                            resolved: path_str,
                            reason: "Glob expansion exceeded limit of 1,000 matches".to_string(),
                        };
                    }
                    if let Ok(matched_path) = entry {
                        let res = validate_single_path(&matched_path, workspace_roots);
                        if let ValidationResult::Denied { .. } = res {
                            return res;
                        }
                    }
                }

                if !matched_any {
                    // Fall back to validating stable prefix
                    let stable_prefix = get_stable_glob_prefix(&path_str);
                    if stable_prefix.is_empty() {
                        return ValidationResult::Denied {
                            candidate: candidate_token.to_string(),
                            resolved: path_str,
                            reason: "Stable glob prefix is empty".to_string(),
                        };
                    }
                    return validate_single_path(Path::new(&stable_prefix), workspace_roots);
                }

                ValidationResult::Allowed {
                    reason: "All glob matched files are in scope".to_string(),
                }
            }
            Err(e) => ValidationResult::Denied {
                candidate: candidate_token.to_string(),
                resolved: path_str,
                reason: format!("Invalid glob pattern: {}", e),
            },
        }
    } else {
        validate_single_path(&resolved_absolute, workspace_roots)
    }
}

fn validate_single_path(path: &Path, workspace_roots: &[PathBuf]) -> ValidationResult {
    let path_str = path.to_string_lossy();
    if cfg!(unix) {
        if path_str.starts_with("\\\\") 
            || (path_str.len() >= 3 && path_str.chars().next().unwrap().is_alphabetic() && &path_str[1..3] == ":\\")
            || (path_str.len() >= 3 && path_str.chars().next().unwrap().is_alphabetic() && &path_str[1..3] == ":/") 
        {
            return ValidationResult::Denied {
                candidate: path_str.to_string(),
                resolved: path_str.to_string(),
                reason: "Windows or UNC paths are not allowed on POSIX workspace roots".to_string(),
            };
        }
    }

    let canonical = canonicalize_path(path);

    for root in workspace_roots {
        let canonical_root = canonicalize_path(root);
        
        // Use Rust's Path prefix check which handles component boundaries properly.
        if canonical.strip_prefix(&canonical_root).is_ok() {
            return ValidationResult::Allowed {
                reason: format!("Path is contained in workspace root: {}", root.display()),
            };
        }
    }

    ValidationResult::Denied {
        candidate: path.to_string_lossy().to_string(),
        resolved: canonical.to_string_lossy().to_string(),
        reason: "path resolves outside workspace scope".to_string(),
    }
}


#[cfg(test)]
mod hardening_tests {
    use super::*;
    use std::fs;
    
    #[test]
    fn test_symlink_escape() {
        let out_dir = PathBuf::from("/tmp/nakama_test_out");
        let _ = fs::create_dir_all(&out_dir);
        let file_out = out_dir.join("secret.txt");
        let _ = fs::write(&file_out, "secret");

        let workspace_root = std::env::current_dir().unwrap();
        let symlink_path = workspace_root.join("symlink_to_secret");
        let _ = std::os::unix::fs::symlink(&file_out, &symlink_path);

        let res = validate_path(&symlink_path.to_string_lossy(), &[workspace_root.clone()], &workspace_root);
        let _ = fs::remove_file(&symlink_path);
        
        match res {
            ValidationResult::Denied { reason, .. } => {
                assert!(reason.contains("resolves outside workspace scope"));
            }
            _ => panic!("Symlink escape should be denied"),
        }
    }
    
    #[test]
    fn test_windows_unc_path() {
        let workspace_root = std::env::current_dir().unwrap();
        
        let path = "\\\\server\\share\\file.txt";
        let res = validate_path(path, &[workspace_root.clone()], &workspace_root);
        if cfg!(unix) {
            match res {
                ValidationResult::Denied { reason, .. } => {
                    assert!(reason.contains("Windows or UNC paths"));
                }
                _ => panic!("UNC path should be denied on POSIX"),
            }
        }
        
        let path = "C:\\Windows\\System32";
        let res = validate_path(path, &[workspace_root.clone()], &workspace_root);
        if cfg!(unix) {
            match res {
                ValidationResult::Denied { reason, .. } => {
                    assert!(reason.contains("Windows or UNC paths"));
                }
                _ => panic!("Windows path should be denied on POSIX"),
            }
        }
    }
    
    #[test]
    fn test_traversal_escape() {
        let workspace_root = std::env::current_dir().unwrap();
        let path = "../../etc/passwd";
        let res = validate_path(path, &[workspace_root.clone()], &workspace_root);
        match res {
            ValidationResult::Denied { reason, .. } => {
                assert!(reason.contains("resolves outside workspace scope"));
            }
            _ => panic!("Traversal escape should be denied"),
        }
    }
    
    #[test]
    fn test_glob_escape() {
        let workspace_root = std::env::current_dir().unwrap();
        let path = "/*";
        let res = validate_path(path, &[workspace_root.clone()], &workspace_root);
        match res {
            ValidationResult::Denied { reason, .. } => {
                assert!(reason.contains("resolves outside workspace scope"));
            }
            _ => panic!("Glob escape should be denied"),
        }
    }
}
