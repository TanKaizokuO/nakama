# Current Project State

Last Updated: 2026-06-27

## Overall Progress
- Estimated completion: 85%
- Total features: 12
- Completed: 11
- Partially Implemented: 1
- Not Started: 0

## Implemented Features

### CLI Entry Points & Commands
- Status: ✅ Complete
- Description: Application command line arguments, flags, interactive REPL, one-shot prompt, and various status subcommands.
- Files involved: `src/cli.rs`, `src/subcommands.rs`, `src/slash_commands.rs`, `src/main.rs`.
- Notes: Supports all CLI flags defined in the specification (OutputFormat, PermissionMode, AllowedTools, etc.).

### Provider API Interface & Streaming
- Status: ✅ Complete
- Description: Multi-provider abstraction handling Auth, SSE stream mapping, formatting, and routing. 
- Files involved: `src/provider/mod.rs`, `src/provider/anthropic.rs`, `src/provider/openai_compat.rs`, `src/provider/dashscope.rs`, `src/provider/xai.rs`, `src/sse.rs`.
- Notes: Native Anthropic, DashScope, xAI, NVIDIA NIM, and OpenAI all actively supported with streaming and preflight.

### Conversation Runtime & Compaction Engine
- Status: ✅ Complete
- Description: Core multi-turn conversation loop, token budget preflight checks, tool dispatch, and algorithmic context compaction threshold management.
- Files involved: `src/runtime.rs`, `src/compaction.rs`, `src/nim_accumulator.rs`.
- Notes: The compaction engine estimates tokens and effectively splits tools safely on thresholds, generating the required structured metadata summary payload.

### Permission Engine
- Status: ✅ Complete
- Description: Multi-staged permission level hierarchies (ReadOnly, WorkspaceWrite, DangerFullAccess), explicit allow/deny/ask configurations, and runtime interactivity (Prompt mode).
- Files involved: `src/permission.rs`, `src/prompter.rs`.
- Notes: Rule matching handles globs and precise JSON key extracting for targets exactly as specified in the document.

### Path Scope Validation & Security
- Status: ✅ Complete
- Description: Resolves, canonicalizes, and bounds candidate paths against workspace roots using POSIX tokenization and glob expansion bounds checking.
- Files involved: `src/path_scope.rs`.
- Notes: Utilizes POSIX `shlex::split` safely as required.

### External MCP Bridge System
- Status: ✅ Complete
- Description: MCP (Model Context Protocol) bridging for external runtime servers and tools dynamically.
- Files involved: `src/mcp.rs`.

### Session Persistence & Worker State
- Status: ✅ Complete
- Description: Persists sessions in JSONL format, manages forks and heartbeats, tracks `worker-state.json`.
- Files involved: `src/session.rs`, `src/worker_state.rs`.

### Configuration Hierarchy
- Status: ✅ Complete
- Description: Layered configuration overrides processing `~/.claw.json` all the way down to `.claw/settings.local.json`.
- Files involved: `src/config/mod.rs`, `src/config/precedence.rs`, `src/config/aliases.rs`.
- Notes: Parses and implements instructions discovering from `CLAUDE.md`, `.claw/rules/`, etc.

### Usage & Telemetry
- Status: ✅ Complete
- Description: Token caching reads and writes usage aggregation, calculating runtime usage costs natively.
- Files involved: `src/usage.rs`.

### Bootstrap Sequence & Hook System
- Status: ✅ Complete
- Description: Multi-phase structured startup flow and PreToolUse / PostToolUse hook triggers.
- Files involved: `src/bootstrap.rs`, `src/hook.rs`.

### Python Companion Parity Toolkit
- Status: ✅ Complete
- Description: Query engine, turn routing simulator, and parity coverage audit.
- Files involved: `python_companion/*.py`.

## Partially Implemented

### Built-In Tool System
- Status: 🟡 Partial
- What is implemented: Core filesystem and execution tasks (`ShellExecute`, `FileRead`, `FileWrite`, `FileEdit`, `GlobSearch`, `GrepSearch`).
- What is missing: Domain specific integration tools (`AgentLaunch`, `NotebookEdit`, `SkillInvoke`, `TodoWrite`, `ToolSearch`, `WebFetch`, `WebSearch`).
- Files involved: `src/tools/shell.rs`, `src/tools/dispatch.rs`, and remaining stubbed tool files.

## Not Yet Implemented

(All core subsystems are at least partially implemented. Domain-specific built-in tools are technically missing but grouped above).

## Technical Debt
- Missing implementations for `unimplemented!()` domain-specific built-in tools.
- Refactoring opportunities: Provider clients could share more common stream parsing error handling code.
- Known bugs: None presently tracked, but `unimplemented!()` stubs will crash if specifically invoked.

## File/Module Status

| Module | Status | Notes |
|---------|--------|------|
| `src/cli.rs` / `src/subcommands.rs` | Complete | Active and parsing properly |
| `src/provider/` | Complete | All API providers are functional |
| `src/runtime.rs` / `src/compaction.rs` | Complete | Conversational multi-turn loop and context sliding window |
| `src/tools/shell.rs`, `file_*.rs` | Complete | Baseline tool execution |
| `src/tools/web_*.rs`, `agent_*.rs` | Partial | These are just unimplemented stubs |
| `src/path_scope.rs` / `src/permission.rs`| Complete | Access boundaries active |
| `src/config/` | Complete | Hierarchical settings implemented |

## API Status (if applicable)

| Endpoint | Status | Notes |
|---------|--------|------|
| Anthropic API Client | Complete | Live streaming parsing mapped correctly to internal state |
| OpenAI/NIM/xAI Client | Complete | Compatibility layers running |

## Database Status (if applicable)

- Existing tables/models: N/A (File-based JSONL)

## UI Status (if applicable)

- Completed screens: N/A (CLI Interface)

## Remaining Work

### High Priority
- [ ] Implement `WebFetch` and `WebSearch` built-in tools to grant external network connectivity outside MCP.
- [ ] Implement `TodoWrite` for session persistence tracking integration.
- [ ] Add integration testing coverage on Anthropic stream extraction.

### Medium Priority
- [ ] Implement `SkillInvoke` and `AgentLaunch` for recursive workflows.

### Low Priority
- [ ] Implement `NotebookEdit`.

## Next Recommended Steps

1. Fill in the stubbed Built-In Tools starting with `WebFetch` and `WebSearch`.
2. Consolidate unit testing around the Anthropic JSON mapper to ensure token boundary robustness.
3. Validate MCP lifecycle on non-stdio transports (e.g. WebSockets) if not fully tested.
