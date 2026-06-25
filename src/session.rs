use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;
use crate::data_contracts::{SessionMessageRecord, SessionMetadata, SessionMetadataRecord, UsageRecord};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SessionError {
    #[error("no session file found at {0}")]
    NotFound(String),
    #[error("deserialization error in {path} at line {line}: {source}")]
    Deserialization {
        path: String,
        line: usize,
        source: serde_json::Error,
    },
    #[error("cannot fork an empty session")]
    EmptyFork,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("missing metadata in session file")]
    MissingMetadata,
}

pub struct Session {
    pub session_id: String,
    pub metadata: SessionMetadata,
    pub messages: Vec<SessionMessageRecord>,
    pub base_dir: PathBuf,
}

impl Session {
    pub fn new(base_dir: PathBuf, model: String, permission_mode: String) -> Self {
        let session_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        
        Self {
            session_id: session_id.clone(),
            metadata: SessionMetadata {
                session_id,
                created_at: now.clone(),
                model,
                permission_mode,
                heartbeat: now,
                liveness: true,
                compaction_history: vec![],
            },
            messages: vec![],
            base_dir,
        }
    }

    fn file_path(&self) -> PathBuf {
        self.base_dir.join(format!("{}.jsonl", self.session_id))
    }

    pub fn save(&mut self) -> Result<(), SessionError> {
        self.metadata.heartbeat = chrono::Utc::now().to_rfc3339();
        
        let path = self.file_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        let temp_path = path.with_extension("tmp");
        {
            let mut file = File::create(&temp_path)?;
            let meta_record = SessionMetadataRecord::SessionMeta {
                session_id: self.metadata.session_id.clone(),
                created_at: self.metadata.created_at.clone(),
                model: self.metadata.model.clone(),
                permission_mode: self.metadata.permission_mode.clone(),
                heartbeat: self.metadata.heartbeat.clone(),
                liveness: self.metadata.liveness,
                compaction_history: self.metadata.compaction_history.clone(),
            };
            writeln!(file, "{}", serde_json::to_string(&meta_record).map_err(|e| SessionError::Deserialization { path: temp_path.to_string_lossy().to_string(), line: 1, source: e })?)?;
            
            for (i, msg) in self.messages.iter().enumerate() {
                writeln!(file, "{}", serde_json::to_string(msg).map_err(|e| SessionError::Deserialization { path: temp_path.to_string_lossy().to_string(), line: i + 2, source: e })?)?;
            }
            file.sync_all()?;
        }
        
        fs::rename(temp_path, path)?;
        Ok(())
    }

    pub fn resume(base_dir: PathBuf, session_id: &str) -> Result<Self, SessionError> {
        let path = base_dir.join(format!("{}.jsonl", session_id));
        if !path.exists() {
            return Err(SessionError::NotFound(path.to_string_lossy().to_string()));
        }

        let file = File::open(&path)?;
        let reader = BufReader::new(file);
        
        let mut metadata = None;
        let mut messages = vec![];

        for (i, line) in reader.lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            
            if i == 0 {
                let meta_record: SessionMetadataRecord = serde_json::from_str(&line).map_err(|e| SessionError::Deserialization {
                    path: path.to_string_lossy().to_string(),
                    line: i + 1,
                    source: e,
                })?;
                
                if let SessionMetadataRecord::SessionMeta { session_id, created_at, model, permission_mode, heartbeat, liveness, compaction_history } = meta_record {
                    metadata = Some(SessionMetadata {
                        session_id,
                        created_at,
                        model,
                        permission_mode,
                        heartbeat,
                        liveness,
                        compaction_history,
                    });
                }
            } else {
                let msg: SessionMessageRecord = serde_json::from_str(&line).map_err(|e| SessionError::Deserialization {
                    path: path.to_string_lossy().to_string(),
                    line: i + 1,
                    source: e,
                })?;
                messages.push(msg);
            }
        }

        let mut metadata = metadata.ok_or(SessionError::MissingMetadata)?;
        metadata.liveness = true;
        metadata.heartbeat = chrono::Utc::now().to_rfc3339();

        let mut session = Self {
            session_id: session_id.to_string(),
            metadata,
            messages,
            base_dir,
        };
        
        session.save()?; // Save immediately to update liveness and heartbeat
        Ok(session)
    }

    pub fn fork(&self) -> Result<Self, SessionError> {
        if self.messages.is_empty() {
            return Err(SessionError::EmptyFork);
        }

        let mut new_session = Self::new(
            self.base_dir.clone(),
            self.metadata.model.clone(),
            self.metadata.permission_mode.clone()
        );
        new_session.messages = self.messages.clone();
        new_session.save()?;
        Ok(new_session)
    }
    
    pub fn calculate_usage(&self) -> UsageRecord {
        let mut total = UsageRecord::default();
        for msg in &self.messages {
            if let Some(usage) = &msg.usage {
                total.input_tokens += usage.input_tokens;
                total.output_tokens += usage.output_tokens;
                total.cache_creation_tokens += usage.cache_creation_tokens;
                total.cache_read_tokens += usage.cache_read_tokens;
            }
        }
        total
    }
}
