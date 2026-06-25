import json
from typing import List, Dict, Any, Generator
from dataclasses import dataclass, asdict
from session import UsageRecord
from routing import MatchResult

@dataclass
class DenialRecord:
    tool: str
    reason: str

@dataclass
class TurnResult:
    prompt: str
    output: str
    matched_commands: List[MatchResult]
    matched_tools: List[MatchResult]
    denials: List[DenialRecord]
    usage: UsageRecord
    stop_reason: str

class QueryEngine:
    def __init__(self, max_turns: int = 10, max_budget: int = 100000, compaction_threshold: int = 20, compaction_keep: int = 10):
        self.max_turns = max_turns
        self.max_budget = max_budget
        self.compaction_threshold = compaction_threshold
        self.compaction_keep = compaction_keep
        self.messages = []
        self.cumulative_usage = UsageRecord(0, 0, 0, 0)
        self.turn_count = 0

    def execute_turn(self, session_id: str, prompt: str, matched_commands: List[MatchResult], matched_tools: List[MatchResult], structured_output: bool = True) -> TurnResult:
        if self.turn_count >= self.max_turns:
            return TurnResult(prompt, "", [], [], [], self.cumulative_usage, "max_turns_reached")

        output_text = f"Processed: {prompt}"
        denials = []

        usage = UsageRecord(
            input_tokens=len(prompt.split()),
            output_tokens=len(output_text.split()),
            cache_creation_tokens=0,
            cache_read_tokens=0
        )
        self.cumulative_usage.input_tokens += usage.input_tokens
        self.cumulative_usage.output_tokens += usage.output_tokens

        stop_reason = None
        if self.cumulative_usage.input_tokens + self.cumulative_usage.output_tokens >= self.max_budget:
            stop_reason = "max_budget_reached"

        self.messages.append(prompt)
        
        if len(self.messages) > self.compaction_threshold:
            self.messages = self.messages[-self.compaction_keep:]

        if structured_output:
            payload = {
                "session_id": session_id,
                "prompt": prompt,
                "output_text": output_text,
                "matched_commands": [asdict(m) for m in matched_commands],
                "matched_tools": [asdict(m) for m in matched_tools],
                "denials": [asdict(d) for d in denials],
                "usage": asdict(usage),
                "stop_reason": stop_reason
            }
            try:
                output = json.dumps(payload)
            except Exception:
                fallback = {
                    "session_id": session_id,
                    "prompt": prompt,
                    "output_text": output_text,
                    "fallback_mode": True
                }
                output = json.dumps(fallback)
        else:
            output = f"Prompt: {prompt}\nOutput: {output_text}"

        self.turn_count += 1
        return TurnResult(prompt, output, matched_commands, matched_tools, denials, usage, stop_reason)

    def execute_turn_stream(self, session_id: str, prompt: str, matched_commands: List[MatchResult], matched_tools: List[MatchResult]) -> Generator[Dict[str, Any], None, None]:
        yield {"event": "session_start", "payload": {"session_id": session_id, "prompt": prompt}}
        
        if matched_commands:
            yield {"event": "command_match", "payload": {"commands": [m.name for m in matched_commands]}}
        
        if matched_tools:
            yield {"event": "tool_match", "payload": {"tools": [m.name for m in matched_tools]}}

        denials = []
        if denials:
            yield {"event": "permission_denial", "payload": {"denials": [d.tool for d in denials]}}

        output_text = f"Processed: {prompt}"
        yield {"event": "message_delta", "payload": {"text": output_text}}

        usage = UsageRecord(len(prompt.split()), len(output_text.split()), 0, 0)
        yield {"event": "session_end", "payload": {"usage": asdict(usage), "stop_reason": "end_turn", "transcript_size": len(self.messages)}}
