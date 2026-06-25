import json
import os
from dataclasses import dataclass
from typing import List, Optional, Dict, Any

@dataclass
class UsageRecord:
    input_tokens: int
    output_tokens: int
    cache_creation_tokens: int
    cache_read_tokens: int

@dataclass
class ContentBlock:
    type: str
    text: Optional[str] = None
    id: Optional[str] = None
    name: Optional[str] = None
    input: Optional[Any] = None
    tool_use_id: Optional[str] = None
    content: Optional[str] = None
    is_error: Optional[bool] = None
    thinking: Optional[str] = None
    signature: Optional[str] = None
    data: Optional[str] = None

@dataclass
class SessionMessageRecord:
    role: str
    content: List[ContentBlock]
    timestamp: str
    usage: Optional[UsageRecord] = None
    tool_call_id: Optional[str] = None

@dataclass
class SessionMetadata:
    session_id: str
    created_at: str
    model: str
    permission_mode: str
    heartbeat: str
    liveness: bool
    compaction_history: List[Any]

class SessionLoader:
    def __init__(self, workspace_path: str):
        self.workspace_path = workspace_path

    def load_session(self, session_id: str):
        file_path = os.path.join(self.workspace_path, ".claw", "sessions", f"{session_id}.jsonl")
        if not os.path.exists(file_path):
            raise FileNotFoundError(f"Session file not found: {file_path}")

        with open(file_path, "r", encoding="utf-8") as f:
            lines = f.readlines()

        if not lines:
            raise ValueError("Empty session file")

        try:
            meta_dict = json.loads(lines[0])
        except json.JSONDecodeError as e:
            raise ValueError(f"Malformed JSON on line 1: {file_path}") from e

        if meta_dict.get("type") != "session_meta":
            raise ValueError("First line must be session_meta")

        # Handle nested session_meta
        if "session_meta" in meta_dict:
            meta_dict = meta_dict["session_meta"]

        meta = SessionMetadata(
            session_id=meta_dict.get("session_id", ""),
            created_at=meta_dict.get("created_at", ""),
            model=meta_dict.get("model", ""),
            permission_mode=meta_dict.get("permission_mode", ""),
            heartbeat=meta_dict.get("heartbeat", ""),
            liveness=meta_dict.get("liveness", False),
            compaction_history=meta_dict.get("compaction_history", [])
        )
        
        messages = []
        cumulative_usage = UsageRecord(0, 0, 0, 0)

        for line_num, line in enumerate(lines[1:], start=2):
            try:
                record = json.loads(line)
            except json.JSONDecodeError as e:
                raise ValueError(f"Malformed JSON on line {line_num}: {file_path}") from e

            role = record.get("role")
            if not role:
                raise ValueError(f"Missing 'role' on line {line_num}")

            tool_call_id = record.get("tool_call_id")
            if role == "tool" and tool_call_id is None:
                raise ValueError(f"tool_call_id must be non-null for tool role on line {line_num}")
            elif role != "tool" and tool_call_id is not None:
                raise ValueError(f"tool_call_id must be null for non-tool role on line {line_num}")

            content_blocks = []
            for cb in record.get("content", []):
                content_blocks.append(ContentBlock(**cb))

            usage = None
            u_dict = record.get("usage")
            if u_dict:
                usage = UsageRecord(
                    input_tokens=u_dict.get("input_tokens", 0),
                    output_tokens=u_dict.get("output_tokens", 0),
                    cache_creation_tokens=u_dict.get("cache_creation_tokens", 0),
                    cache_read_tokens=u_dict.get("cache_read_tokens", 0)
                )
                cumulative_usage.input_tokens += usage.input_tokens
                cumulative_usage.output_tokens += usage.output_tokens
                cumulative_usage.cache_creation_tokens += usage.cache_creation_tokens
                cumulative_usage.cache_read_tokens += usage.cache_read_tokens

            msg = SessionMessageRecord(
                role=role,
                content=content_blocks,
                timestamp=record.get("timestamp", ""),
                usage=usage,
                tool_call_id=tool_call_id
            )
            messages.append(msg)

        return meta, messages, cumulative_usage

    def load_worker_state(self):
        file_path = os.path.join(self.workspace_path, ".claw", "worker-state.json")
        if not os.path.exists(file_path):
            raise Exception("Run the REPL or a one-shot prompt first to produce the worker state file")

        with open(file_path, "r", encoding="utf-8") as f:
            try:
                state = json.load(f)
            except json.JSONDecodeError as e:
                raise Exception(f"Malformed JSON in worker-state.json: {file_path}") from e

        return {
            "worker_id": state.get("worker_id"),
            "session_id": state.get("session_id"),
            "model": state.get("model"),
            "permission_mode": state.get("permission_mode"),
        }
