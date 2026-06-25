import re
from typing import List, Dict, Any
from inventory import InventoryLoader, CommandRecord, ToolRecord
from dataclasses import dataclass

@dataclass
class MatchResult:
    name: str
    kind: str
    score: int
    source: str

class RoutingEngine:
    def __init__(self, inventory: InventoryLoader):
        self.inventory = inventory

    def tokenize(self, text: str) -> List[str]:
        tokens = [t.lower() for t in re.split(r'[\s/-]+', text) if t]
        return tokens

    def score_module(self, tokens: List[str], name: str, source: str, responsibility: str) -> int:
        score = 0
        name_tokens = self.tokenize(name)
        source_tokens = self.tokenize(source)
        resp_tokens = self.tokenize(responsibility)

        for t in tokens:
            if t in name_tokens:
                score += 1
            if t in source_tokens:
                score += 1
            if t in resp_tokens:
                score += 1
        return score

    def route(self, prompt: str, limit: int = 5) -> List[MatchResult]:
        tokens = self.tokenize(prompt)
        if not tokens:
            return []

        first_token = tokens[0]
        explicit_match = None
        if first_token in self.inventory.commands:
            explicit_match = MatchResult(
                name=first_token,
                kind="command",
                score=100,
                source=self.inventory.commands[first_token].source
            )

        results = []
        for cmd_name, cmd in self.inventory.commands.items():
            if explicit_match and cmd_name == explicit_match.name:
                continue
            score = self.score_module(tokens, cmd.name, cmd.source, cmd.responsibility)
            if score > 0:
                results.append(MatchResult(cmd.name, "command", score, cmd.source))

        for tool_name, tool in self.inventory.tools.items():
            score = self.score_module(tokens, tool.name, tool.source, tool.responsibility)
            if score > 0:
                results.append(MatchResult(tool.name, "tool", score, tool.source))

        results.sort(key=lambda x: (-x.score, x.kind, x.name))

        final_list = []
        if explicit_match:
            final_list.append(explicit_match)
        
        highest_cmd = next((r for r in results if r.kind == "command"), None)
        if highest_cmd:
            final_list.append(highest_cmd)
            results.remove(highest_cmd)

        highest_tool = next((r for r in results if r.kind == "tool"), None)
        if highest_tool:
            final_list.append(highest_tool)
            results.remove(highest_tool)

        for r in results:
            if len(final_list) >= limit:
                break
            final_list.append(r)

        return final_list[:limit]
