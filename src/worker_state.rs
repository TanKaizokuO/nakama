use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use crate::data_contracts::WorkerState;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WorkerStateError {
    #[error("Run the REPL or a one-shot prompt first to produce the worker state file")]
    NotFound,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub struct WorkerStateManager {
    base_dir: PathBuf,
}

impl WorkerStateManager {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    fn file_path(&self) -> PathBuf {
        self.base_dir.join("worker-state.json")
    }

    pub fn write_state(&self, state: &WorkerState) -> Result<(), WorkerStateError> {
        let path = self.file_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        let temp_path = path.with_extension("tmp");
        {
            let mut file = File::create(&temp_path)?;
            let content = serde_json::to_string_pretty(state)?;
            file.write_all(content.as_bytes())?;
            file.sync_all()?;
        }
        
        fs::rename(temp_path, path)?;
        Ok(())
    }

    pub fn read_state(&self) -> Result<WorkerState, WorkerStateError> {
        let path = self.file_path();
        if !path.exists() {
            return Err(WorkerStateError::NotFound);
        }
        
        let content = fs::read_to_string(&path)?;
        let state: WorkerState = serde_json::from_str(&content)?;
        Ok(state)
    }
}
