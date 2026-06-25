import json
import os
from typing import List, Tuple

class TranscriptStore:
    def __init__(self, workspace_path: str, compaction_threshold: int = 100, compaction_keep: int = 50):
        self.workspace_path = workspace_path
        self.entries: List[str] = []
        self.flushed_flag: bool = True
        self.compaction_threshold = compaction_threshold
        self.compaction_keep = compaction_keep

    def append(self, prompt: str):
        self.entries.append(prompt)
        self.flushed_flag = False

    def flush(self) -> str:
        os.makedirs(os.path.join(self.workspace_path, ".claw", "sessions"), exist_ok=True)
        file_path = os.path.join(self.workspace_path, ".claw", "sessions", "transcript.jsonl")
        
        with open(file_path, "a", encoding="utf-8") as f:
            for entry in self.entries:
                f.write(json.dumps({"prompt": entry}) + "\n")
        
        self.flushed_flag = True
        return file_path

    def replay(self) -> Tuple[str, ...]:
        return tuple(self.entries)

    def compact(self):
        if len(self.entries) > self.compaction_threshold:
            self.entries = self.entries[-self.compaction_keep:]
            self.flushed_flag = False
