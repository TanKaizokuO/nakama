import os
import glob
from dataclasses import dataclass
from typing import List, Dict, Any
from .inventory import InventoryLoader

@dataclass
class Ratio:
    matched: int
    expected: int

@dataclass
class RatioExt:
    python: int
    archived: int

@dataclass
class RatioInv:
    snapshot: int
    reference: int

@dataclass
class ParityAuditResult:
    archive_present: bool
    root_file_coverage: Ratio
    directory_coverage: Ratio
    total_file_ratio: RatioExt
    command_entry_ratio: RatioInv
    tool_entry_ratio: RatioInv
    missing_root_targets: List[str]
    missing_directory_targets: List[str]

class ParityAuditor:
    def __init__(self, workspace_path: str, inventory: InventoryLoader):
        self.workspace_path = workspace_path
        self.python_dir = os.path.join(workspace_path, "python_companion")
        self.archive_dir = os.path.join(workspace_path, ".claw", "archive", "rust_reference")
        self.inventory = inventory

    def audit(self) -> ParityAuditResult:
        archive_present = os.path.isdir(self.archive_dir)
        
        expected_roots = ['main.py', 'session.py', 'inventory.py', 'routing.py', 'query.py', 'audit.py', 'transcript.py']
        expected_dirs = []

        missing_roots = []
        matched_roots = 0
        for f in expected_roots:
            if os.path.isfile(os.path.join(self.python_dir, f)):
                matched_roots += 1
            else:
                missing_roots.append(f)

        missing_dirs = []
        matched_dirs = 0
        for d in expected_dirs:
            if os.path.isdir(os.path.join(self.python_dir, d)):
                matched_dirs += 1
            else:
                missing_dirs.append(d)

        python_files = len(glob.glob(os.path.join(self.python_dir, "**/*.py"), recursive=True))
        archived_files = len(glob.glob(os.path.join(self.archive_dir, "**/*.rs"), recursive=True)) if archive_present else 0

        ref_cmds = 12 if archive_present else 0
        ref_tools = 0 if archive_present else 0

        if not archive_present:
            return ParityAuditResult(
                archive_present=False,
                root_file_coverage=Ratio(0, 0),
                directory_coverage=Ratio(0, 0),
                total_file_ratio=RatioExt(0, 0),
                command_entry_ratio=RatioInv(0, 0),
                tool_entry_ratio=RatioInv(0, 0),
                missing_root_targets=missing_roots,
                missing_directory_targets=missing_dirs
            )

        return ParityAuditResult(
            archive_present=True,
            root_file_coverage=Ratio(matched_roots, len(expected_roots)),
            directory_coverage=Ratio(matched_dirs, len(expected_dirs)),
            total_file_ratio=RatioExt(python_files, archived_files),
            command_entry_ratio=RatioInv(len(self.inventory.commands), ref_cmds),
            tool_entry_ratio=RatioInv(len(self.inventory.tools), ref_tools),
            missing_root_targets=missing_roots,
            missing_directory_targets=missing_dirs
        )
