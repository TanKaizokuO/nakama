use nakama::path_scope::{validate_path, ValidationResult};
use std::path::{Path, PathBuf};
use std::fs;

fn main() {
    let out_dir = PathBuf::from("/tmp/nakama_test_out");
    fs::create_dir_all(&out_dir).unwrap();
    let file_out = out_dir.join("secret.txt");
    fs::write(&file_out, "secret").unwrap();

    let workspace_root = std::env::current_dir().unwrap();
    let symlink_path = workspace_root.join("symlink_to_secret");
    let _ = std::os::unix::fs::symlink(&file_out, &symlink_path);

    let res = validate_path(&symlink_path, &[workspace_root.clone()], &workspace_root);
    match res {
        ValidationResult::Denied { reason, .. } => println!("DENIED: {}", reason),
        _ => println!("ALLOWED!"),
    }
    let _ = fs::remove_file(&symlink_path);
}
