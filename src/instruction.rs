use std::path::{Path, PathBuf};

pub struct InstructionLoader {
    workspace_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RulesImport {
    Auto,
    None,
    List(Vec<String>),
}

impl InstructionLoader {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }

    pub fn load_instructions(&self, rules_import: &RulesImport) -> String {
        let mut assembled = String::new();
        
        let priority_paths = vec![
            "CLAUDE.md",
            "CLAW.md",
            "AGENTS.md",
        ];

        for p in priority_paths {
            let path = self.workspace_root.join(p);
            if path.exists() {
                if let Ok(content) = self.read_text_file(&path) {
                    assembled.push_str(&format!("\n--- {} ---\n{}\n", p, content));
                }
            }
        }

        if rules_import != &RulesImport::None {
            let framework_paths = vec![
                ".claw/CLAUDE.md",
                ".claude/CLAUDE.md",
                ".claw/instructions.md",
            ];
            for p in framework_paths {
                let path = self.workspace_root.join(p);
                if path.exists() {
                    if let Ok(content) = self.read_text_file(&path) {
                        assembled.push_str(&format!("\n--- {} ---\n{}\n", p, content));
                    }
                }
            }
        }

        let rules_dir = self.workspace_root.join(".claw/rules/");
        if rules_dir.exists() && rules_dir.is_dir() {
            // Mock sort and load
            assembled.push_str("\n--- .claw/rules/ ---\n");
        }

        let local_rules_dir = self.workspace_root.join(".claw/rules.local/");
        if local_rules_dir.exists() && local_rules_dir.is_dir() {
            // Mock sort and load
            assembled.push_str("\n--- .claw/rules.local/ ---\n");
        }

        assembled
    }

    fn read_text_file(&self, path: &Path) -> Result<String, std::io::Error> {
        let bytes = std::fs::read(path)?;
        if bytes.contains(&0) {
            eprintln!("Warning: Skipping binary file {:?}", path);
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Binary file"));
        }
        String::from_utf8(bytes).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }
}
