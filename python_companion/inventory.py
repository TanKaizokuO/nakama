import json
import os
from typing import List, Dict

class CommandRecord:
    def __init__(self, name: str, source: str, kind: str, responsibility: str):
        self.name = name
        self.source = source
        self.kind = kind
        self.responsibility = responsibility

class ToolRecord:
    def __init__(self, name: str, permission: str, source: str, responsibility: str, is_mcp: bool):
        self.name = name
        self.permission = permission
        self.source = source
        self.responsibility = responsibility
        self.is_mcp = is_mcp

class InventoryLoader:
    def __init__(self, workspace_path: str):
        self.workspace_path = workspace_path
        self.commands: Dict[str, CommandRecord] = {}
        self.tools: Dict[str, ToolRecord] = {}
        self.load_inventory()

    def load_inventory(self):
        cmd_path = os.path.join(self.workspace_path, ".claw", "snapshots", "commands.json")
        tool_path = os.path.join(self.workspace_path, ".claw", "snapshots", "tools.json")

        if os.path.exists(cmd_path):
            with open(cmd_path, "r", encoding="utf-8") as f:
                try:
                    data = json.load(f)
                    for item in data:
                        self.commands[item["name"]] = CommandRecord(**item)
                except json.JSONDecodeError:
                    pass

        if os.path.exists(tool_path):
            with open(tool_path, "r", encoding="utf-8") as f:
                try:
                    data = json.load(f)
                    for item in data:
                        self.tools[item["name"]] = ToolRecord(**item)
                except json.JSONDecodeError:
                    pass

    def get_command(self, name: str) -> CommandRecord:
        return self.commands.get(name)

    def get_tool(self, name: str) -> ToolRecord:
        return self.tools.get(name)
