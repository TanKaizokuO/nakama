# Phase 5 — Python Companion Workspace & Parity Audit

## Overview

Phase 5 delivers the companion Python workspace that provides a mirrored command/tool inventory, prompt routing engine, query engine with structured output and retry/fallback, and a parity audit against the primary Rust implementation. It explicitly consumes the Phase 3 JSONL session schema and worker-state.json as stable read-only inputs, implements all Python companion CLI subcommands (including the previously missing SetupReport), and provides the transcript store with flush/replay/compaction. This phase depends on Phases 1–4 (the Python workspace mirrors and audits the Rust runtime's command inventory, tool pool, and session state). It is the final phase and unlocks no further phases.

## Depends on

- **Phase 1 — Core Infrastructure**: usage tracking schema (token fields, pricing table) as a reference for the Python query engine's token budget and cost estimation.
- **Phase 2 — Tool System & Permission Engine**: tool pool schema (canonical tool names, permission levels) as reference data for the mirrored tool inventory.
- **Phase 3 — Session & Conversation Runtime**: JSONL session schema (data contracts) as the primary input format for `LoadSession`; worker-state.json schema for `WorkerState` reading; structured output fallback payload spec for the query engine's retry/fallback logic.
- **Phase 4 — CLI, Bootstrap & Plugin System**: command inventory from `DumpManifests` as reference data for the mirrored command inventory; bootstrap phase list from `BootstrapPlan` for `BootstrapGraph`; subcommand definitions as the reference for parity audit.

## Unlocks

- None. This is the final phase. The parity audit validates completeness against the Rust implementation.

## Scope

### Input Contracts

**Spec references:** §5.1, §5.2, §5.7

> [RESOLVED: A12] The session state schema (§5.1–5.2) is explicitly specified as an input contract for the Python loader. The Python workspace consumes the Phase 3 JSONL session schema and worker-state.json as stable, read-only inputs.

The Python companion workspace reads two classes of input files produced by the Rust runtime:

#### JSONL Session Files

- Location: `<workspace>/.claw/sessions/<session_id>.jsonl`
- Format: as defined in Phase 3 Data Contracts (JSONL Message Record schema).
- The Python loader must:
  1. Open the JSONL file and read line-by-line.
  2. Parse the first line as a session metadata record (`type: "session_meta"`).
  3. Parse subsequent lines as message records with `role`, `content`, `usage`, `timestamp`, `tool_call_id`.
  4. Deserialize `content` arrays into typed ContentBlock objects (`text`, `tool_use`, `tool_result`, `thinking`, `redacted_thinking`).
  5. Reconstruct the usage tracker from messages with non-null `usage` fields (same algorithm as Phase 1's `reconstruct_from_messages()`).
- Error handling:
  - Missing file → `FileNotFoundError` with the attempted path.
  - Malformed JSON line → `JSONDecodeError` with file path and line number.
  - Missing required fields → `ValueError` with the field name and record index.

#### Worker State File

- Location: `<workspace>/.claw/worker-state.json`
- Format: as defined in Phase 3 Data Contracts (Worker State schema).
- Fields consumed: `worker_id`, `session_id`, `model`, `permission_mode`.
- Error handling:
  - Missing file → structured error with hint: `"Run the REPL or a one-shot prompt first to produce the worker state file"`.
  - Malformed JSON → `JSONDecodeError` with file path.

### Mirrored Command/Tool Inventory

**Spec references:** §1.3

The Python workspace maintains a mirrored inventory of commands and tools loaded from reference snapshot files:

- **Command inventory**: loaded from a snapshot JSON file containing an array of command records. Each record has:
  - `name` (String): canonical command name.
  - `source` (String): source module or plugin.
  - `kind` (String enum: `"builtin"`, `"plugin"`, `"skill"`).
  - `responsibility` (String): one-sentence description of what the command does.

- **Tool inventory**: loaded from a snapshot JSON file containing an array of tool records. Each record has:
  - `name` (String): canonical tool name.
  - `permission` (String): required permission level.
  - `source` (String): source module, plugin, or MCP server.
  - `responsibility` (String): one-sentence description.
  - `is_mcp` (Boolean): whether the tool is from an MCP server.

Inventory loading:
- Snapshot files are read at startup.
- If a snapshot file is missing, the inventory starts empty (not an error; the parity audit will report the gap).
- Inventory is immutable after loading.

### Prompt Routing Engine

**Spec references:** §3.3

The prompt routing engine matches user input to commands and tools using token-based scoring:

1. **Extract tokens** — Split the prompt into lowercase tokens. Treat `/` and `-` as separators (e.g., `"/file-read"` → `["file", "read"]`). Strip leading `/` from the first token.

2. **Check for explicit command match** — If the first token (after `/` removal) exactly matches a known command name, create a high-priority match with score = 100. This match takes absolute precedence.

3. **Score all commands and tools** — For each registered command/tool module:
   - Count how many prompt tokens appear in the module's `name` (after the same tokenization).
   - Count how many prompt tokens appear in the module's `source` hint.
   - Count how many prompt tokens appear in the module's `responsibility` description.
   - Score = sum of all token matches.

4. **Rank and select** — Build the result list:
   a. The explicit command match (if any) goes first.
   b. The highest-scoring command (if different from the explicit match) goes second.
   c. The highest-scoring tool goes third.
   d. Remaining slots filled from a merged pool of all commands and tools, sorted by:
      - Descending score.
      - Alphabetically by kind (`"command"` before `"tool"`).
      - Alphabetically by name (within the same kind and score).

5. **Apply limit** — Return at most `limit` matches (configurable, default: 5).

Each match result contains:
- `name` (String): the matched command or tool name.
- `kind` (String): `"command"` or `"tool"`.
- `score` (Integer): the match score.
- `source` (String): the source module.

### Query Engine

**Spec references:** §3.4, §3.4.1

The query engine manages a simulated conversation with token budgeting and structured output:

1. **Check turn limit** — If the accumulated message count meets or exceeds `max_turns` (configurable, default: 10), return immediately with `stop_reason: "max_turns_reached"`.

2. **Build output** — Assemble a summary from:
   - The prompt text.
   - Matched commands (from the routing engine).
   - Matched tools (from the routing engine).
   - Permission denials (tools blocked by the permission engine's mode check).

3. **Format output** — If `structured_output` mode is enabled:
   - Serialize the output as a JSON object containing: `session_id`, `prompt`, `output_text`, `matched_commands[]`, `matched_tools[]`, `denials[]`, `usage`, `stop_reason`.
   - On serialization failure, retry up to the configured retry limit (default: 2).
   - If all retries fail, use the simplified fallback payload:

   > [RESOLVED: A13] The fallback payload spec (session_id, prompt, output_text, fallback_mode) is included in Phase 5's scope. The Python query engine uses the same fallback payload format as Phase 3.

   ```json
   {
     "session_id": "<session_uuid>",
     "prompt": "<original_prompt>",
     "output_text": "<best_effort_text>",
     "fallback_mode": true
   }
   ```
   - If not in structured output mode, join the output components as newline-separated text.

4. **Compute usage** — Estimate tokens:
   - `input_tokens` = word count of the prompt (split on whitespace).
   - `output_tokens` = word count of the output text.
   - Add to the cumulative usage tracker.

5. **Check budget** — If cumulative total tokens (input + output across all turns) exceed `max_budget` (configurable), set `stop_reason: "max_budget_reached"`.

6. **Append to transcript** — Store the prompt in the mutable message list and the transcript store.

7. **Compact if needed** — If the message count exceeds the compaction threshold (configurable, default: 20), trim to keep only the most recent N messages (configurable, default: 10).

8. **Return result** — Yield a turn result containing:
   - `prompt` (String)
   - `output` (String)
   - `matched_commands` (List)
   - `matched_tools` (List)
   - `denials` (List)
   - `usage` (UsageRecord)
   - `stop_reason` (String or null)

#### Streaming Variant (§3.4.1)

The streaming variant yields events in order:

1. `session_start` — `{ "session_id": "...", "prompt": "..." }`
2. `command_match` — `{ "commands": ["..."] }` (only if non-empty)
3. `tool_match` — `{ "tools": ["..."] }` (only if non-empty)
4. `permission_denial` — `{ "denials": [...] }` (only if non-empty)
5. `message_delta` — `{ "text": "..." }` (the full output text)
6. `session_end` — `{ "usage": {...}, "stop_reason": "...", "transcript_size": N }`

### Python Companion CLI Subcommands

**Spec references:** §2.5.1

All subcommands return structured results. Each subcommand is implemented as a function that returns a typed result object.

#### RenderSummary
- Inputs: none.
- Output: Markdown summary of the porting workspace state (module count, tool count, command count, session count, last activity).

#### ShowManifest
- Inputs: none.
- Output: module inventory with file counts, notes, and categorization.

#### ParityAudit
- Inputs: none.
- Output: coverage ratios and missing targets (see Parity Audit subsection below).

#### SetupReport

> [RESOLVED: A14] The SetupReport subcommand is included in Phase 5's scope. It reports platform info, prefetch results, and deferred init status.

- Inputs: none.
- Output:
  - `platform`: OS name, version, architecture, Python version.
  - `prefetch_results`: status of any pre-downloaded resources (model files, skill packages). Each result has `name`, `status` (success/failure/skipped), `detail`.
  - `deferred_init`: list of initialization steps that were deferred (not executed at startup due to trust gates or optional dependencies). Each entry has `name`, `reason`, `required_for`.

#### CommandGraph
- Inputs: none.
- Output: command categorization into three groups:
  - `builtins[]`: core commands.
  - `plugin_like[]`: commands from plugins.
  - `skill_like[]`: commands from skills.

#### ToolPool
- Inputs: optional `simple_mode` (Boolean), `include_mcp` (Boolean), `permission_context` (String).
- Output: filtered tool inventory. If `simple_mode`, omit description and source fields. If `include_mcp` is false, exclude MCP tools. If `permission_context` is set, annotate each tool with whether it would be permitted under that mode.

#### BootstrapGraph
- Inputs: none.
- Output: ordered list of bootstrap stages matching Phase 4's BootstrapPlan output.

#### ListSubsystems
- Inputs: optional `limit` (Integer, default: 32).
- Output: top-level modules with file counts, sorted by file count descending.

#### ListCommands
- Inputs: optional `limit`, `query`, `exclude_plugins` (Boolean), `exclude_skills` (Boolean).
- Output: filtered command entries. If `query` is set, filter by token match. If `exclude_plugins`, omit plugin-like commands. If `exclude_skills`, omit skill-like commands.

#### ListTools
- Inputs: optional `limit`, `query`, `simple_mode`, `no_mcp`, `deny_tool[]`, `deny_prefix[]`.
- Output: filtered tool entries. `deny_tool` removes specific tool names. `deny_prefix` removes tools whose names start with any prefix.

#### RoutePrompt
- Inputs: `prompt` (String, required), optional `limit` (Integer).
- Output: ranked list of matching commands/tools with scores (from the prompt routing engine).

#### BootstrapSession
- Inputs: `prompt` (String, required), optional `limit`.
- Output: full runtime session report including: routed matches, tool pool summary, permission context, bootstrap phase list.

#### TurnLoop
- Inputs: `prompt` (String, required), optional `limit`, `max_turns`, `structured_output` (Boolean).
- Output: list of turn results from the query engine. Each turn result contains prompt, output, matches, denials, usage, stop_reason.

#### FlushTranscript
- Inputs: `prompt` (String, required).
- Output: `{ "path": "<persisted_file_path>", "flushed": true/false }`.

#### LoadSession
- Inputs: `session_id` (String, required).
- Output: session metadata — message count, total tokens, model, permission mode, created_at, last heartbeat.

#### Remote Modes (RemoteMode, SshMode, TeleportMode, DirectConnectMode, DeepLinkMode)
- Inputs: `target` (String, required).
- Output: mode report — `{ "mode": "...", "target": "...", "connected": true/false, "detail": "..." }`.

#### ShowCommand / ShowTool
- Inputs: `name` (String, required).
- Output: module details — `{ "name": "...", "source": "...", "responsibility": "...", "kind": "...", "permission": "..." }`.

#### ExecuteCommand / ExecuteTool
- Inputs: `name` (String, required), `prompt`/`payload` (String, required).
- Output: execution result — `{ "handled": true/false, "message": "..." }`.

### Parity Audit

**Spec references:** §5.10

The parity audit compares the Python companion workspace against an archived reference of the Rust implementation:

- **ArchivePresent** (Boolean): whether the reference archive directory exists at the expected path.
- **RootFileCoverage** (Ratio): `matched_root_files / expected_root_files`. Expected root files: a predefined list of files that should exist in the Python workspace (e.g., `main.py`, `config.py`, `session.py`).
- **DirectoryCoverage** (Ratio): `matched_directories / expected_directories`. Expected directories: a predefined list of module directories.
- **TotalFileRatio** (Ratio): `python_file_count / archived_file_count`. Overall file count comparison.
- **CommandEntryRatio** (Ratio): `snapshot_command_count / reference_command_count`. How many commands from the Rust reference are present in the Python snapshot.
- **ToolEntryRatio** (Ratio): `snapshot_tool_count / reference_tool_count`. How many tools from the Rust reference are present in the Python snapshot.
- **MissingRootTargets** (List of Strings): expected root files not found in the Python workspace.
- **MissingDirectoryTargets** (List of Strings): expected directories not found in the Python workspace.

Audit procedure:
1. Check if the archive directory exists. If not, set `ArchivePresent = false` and skip ratio calculations (report all ratios as 0/0).
2. Scan the Python workspace for root files and compare against the expected list.
3. Scan the Python workspace for directories and compare against the expected list.
4. Count total Python files (`.py`) and compare against total archived files.
5. Load the command snapshot and compare entry count against the reference command count.
6. Load the tool snapshot and compare entry count against the reference tool count.
7. Collect missing targets for root files and directories.

### Transcript Store

**Spec references:** §5.3

The transcript store maintains an in-memory ordered list of prompt strings:

- **Append**: add a new prompt string to the end of the list. Sets `FlushedFlag = false`.
- **Flush**: persist the current entries to disk (implementation-defined format, e.g., JSONL or plain text). Sets `FlushedFlag = true`. Returns the persisted file path.
- **Replay**: returns a snapshot of all current entries as an immutable tuple (or frozen list). Does not modify state.
- **Compaction**: when the entry count exceeds a configurable threshold (default: 100), discard the oldest entries, keeping only the most recent N entries (default: 50).

State:
- `Entries` (List of Strings): grows with each `append()` call.
- `FlushedFlag` (Boolean): `true` after `flush()`, `false` after `append()`.

## Data contracts

This section defines the schemas that Phase 5 consumes (read-only) from earlier phases, and the schemas that Phase 5 produces.

### Consumed Schemas (from Phase 3)

Phase 5 reads the following schemas exactly as defined in Phase 3's Data Contracts:

- **JSONL Message Record**: `role`, `content` (ContentBlock array), `usage` (UsageRecord or null), `timestamp`, `tool_call_id`.
- **Session Metadata**: `session_id`, `created_at`, `model`, `permission_mode`, `heartbeat`, `liveness`, `compaction_history`.
- **Worker State**: `worker_id`, `session_id`, `model`, `permission_mode`.
- **Structured Output Fallback Payload**: `session_id`, `prompt`, `output_text`, `fallback_mode`.

These schemas are treated as stable contracts. The Python loader must not assume additional fields and must tolerate missing optional fields.

### Produced Schemas (Phase 5 outputs)

#### Parity Audit Result

| Field | Type | Description |
|-------|------|-------------|
| `archive_present` | Boolean | Whether the archive directory exists |
| `root_file_coverage` | Object: `{ matched: int, expected: int }` | Root file coverage ratio |
| `directory_coverage` | Object: `{ matched: int, expected: int }` | Directory coverage ratio |
| `total_file_ratio` | Object: `{ python: int, archived: int }` | Overall file count comparison |
| `command_entry_ratio` | Object: `{ snapshot: int, reference: int }` | Command inventory coverage |
| `tool_entry_ratio` | Object: `{ snapshot: int, reference: int }` | Tool inventory coverage |
| `missing_root_targets` | Array of Strings | Missing root files |
| `missing_directory_targets` | Array of Strings | Missing directories |

#### Turn Result (Query Engine)

| Field | Type | Description |
|-------|------|-------------|
| `prompt` | String | The user's input prompt |
| `output` | String | The assembled output text |
| `matched_commands` | Array of MatchResult | Routed command matches |
| `matched_tools` | Array of MatchResult | Routed tool matches |
| `denials` | Array of DenialRecord | Permission denials |
| `usage` | UsageRecord | Token usage for this turn |
| `stop_reason` | String or null | Why the turn ended |

#### Setup Report

| Field | Type | Description |
|-------|------|-------------|
| `platform` | Object: `{ os, version, arch, python_version }` | Platform information |
| `prefetch_results` | Array of `{ name, status, detail }` | Pre-downloaded resource status |
| `deferred_init` | Array of `{ name, reason, required_for }` | Deferred initialization steps |

## Acceptance criteria

1. The Python JSONL loader correctly parses session files conforming to the Phase 3 JSONL Message Record schema, including all five ContentBlock variants and the UsageRecord.
2. The Python loader correctly reads worker-state.json and produces a structured error with hint when the file is missing.
3. The mirrored command and tool inventories load from snapshot files and provide lookup by name.
4. The prompt routing engine correctly extracts tokens, identifies explicit command matches (score 100), scores all commands/tools by token overlap, and returns ranked results respecting the limit.
5. The query engine correctly enforces the turn limit, token budget, compaction threshold, and produces turn results with all required fields.
6. The structured output retry/fallback produces the correct simplified fallback payload (session_id, prompt, output_text, fallback_mode: true) when serialization fails.
7. The streaming variant yields events in the correct order (session_start, command_match, tool_match, permission_denial, message_delta, session_end) with conditional omission of empty events.
8. The SetupReport subcommand returns platform info, prefetch results, and deferred init status.
9. The parity audit correctly computes all coverage ratios and identifies missing root targets and directory targets.
10. The transcript store supports append, flush, replay, and compaction, with the FlushedFlag correctly toggling between append and flush operations.
11. All Python companion CLI subcommands (RenderSummary through ExecuteTool) return well-typed result objects.
12. The LoadSession subcommand reconstructs session metadata (message count, total tokens, model, permission mode) from the JSONL file.

## Git commit plan

1. **`feat: add Python JSONL session loader`**
   Implement the JSONL parser that reads Phase 3 session files, deserializes message records with all ContentBlock variants, and reconstructs the usage tracker. Resolves audit finding A12 (session state schema as input contract).
   *Satisfies acceptance criteria: 1*

2. **`feat: add worker state reader`**
   Implement worker-state.json reader with structured error on missing file.
   *Satisfies acceptance criteria: 2*

3. **`feat: add mirrored command/tool inventory`**
   Load command and tool inventories from snapshot JSON files with name-based lookup.
   *Satisfies acceptance criteria: 3*

4. **`feat: implement prompt routing engine`**
   Build token extraction, explicit command matching (score 100), token-based scoring across name/source/responsibility, ranking, and limit application.
   *Satisfies acceptance criteria: 4*

5. **`feat: implement query engine with structured output`**
   Build the simulated conversation engine with turn limit, output assembly, structured JSON serialization with retry/fallback, token budget, transcript append, and compaction. Resolves audit finding A13 (fallback payload spec).
   *Satisfies acceptance criteria: 5, 6*

6. **`feat: add streaming variant for query engine`**
   Implement the event-based streaming output (session_start through session_end) with conditional event omission.
   *Satisfies acceptance criteria: 7*

7. **`feat: add all Python CLI subcommands`**
   Implement RenderSummary, ShowManifest, ParityAudit, SetupReport, CommandGraph, ToolPool, BootstrapGraph, ListSubsystems, ListCommands, ListTools, RoutePrompt, BootstrapSession, TurnLoop, FlushTranscript, LoadSession, remote modes, ShowCommand/ShowTool, ExecuteCommand/ExecuteTool. Resolves audit finding A14 (SetupReport).
   *Satisfies acceptance criteria: 8, 11, 12*

8. **`feat: add parity audit and transcript store`**
   Implement the parity audit with all coverage ratios and missing target detection. Add transcript store with append/flush/replay/compaction.
   *Satisfies acceptance criteria: 9, 10*

9. **`test: add routing and query engine tests`**
   Test prompt routing with explicit matches, scoring, and ranking. Test query engine turn limits, budget enforcement, compaction, and fallback payload generation.
   *Satisfies acceptance criteria: 4, 5, 6, 7*

10. **`test: add parity audit and integration tests`**
    Test parity audit ratios, missing targets, JSONL loader round-trip, worker state reader, and all subcommand return types.
    *Satisfies acceptance criteria: 1, 2, 9, 10, 11, 12*

## Open questions

1. **Snapshot file location**: The spec states inventories are loaded from "reference snapshot files" but does not specify where these files are stored. The implementer should use `<workspace>/.claw/snapshots/commands.json` and `<workspace>/.claw/snapshots/tools.json`.
2. **Parity audit expected lists**: The spec does not define the expected root files and directories for the parity audit. The implementer should derive these lists from the Rust workspace's actual file structure at a point-in-time snapshot.
3. **Python version requirements**: The spec does not specify a minimum Python version. The implementer should target Python 3.10+ for modern typing and match/case support.
4. **Transcript persistence format**: The spec states `flush()` persists entries to disk but does not specify the format. The implementer should use JSONL (one JSON string per line) for consistency with the session JSONL format.
5. **Remote mode implementation**: The spec lists RemoteMode, SshMode, TeleportMode, DirectConnectMode, and DeepLinkMode but provides minimal detail on their behavior. The implementer should implement them as stubs that return mode reports with `connected: false` and `detail: "not implemented"` until the Rust runtime provides the corresponding functionality.
