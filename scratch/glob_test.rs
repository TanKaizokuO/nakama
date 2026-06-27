fn main() {
    let tokens = nakama::path_scope::tokenize_payload("ls /*");
    let paths = nakama::path_scope::extract_paths(&tokens);
    println!("Extracted: {:?}", paths);
    
    let res = nakama::path_scope::validate_path("/*", &[std::env::current_dir().unwrap()], &std::env::current_dir().unwrap());
    println!("Validate /*: {:?}", res);
    
    let res = nakama::path_scope::validate_path("C:\\Windows", &[std::env::current_dir().unwrap()], &std::env::current_dir().unwrap());
    println!("Validate C:\\Windows: {:?}", res);
}
