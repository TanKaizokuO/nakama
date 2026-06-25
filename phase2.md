# Phase 2 — Tool System & Permission Engine

## Overview

Phase 2 delivers the tool execution layer and the permission enforcement engine that gates every tool invocation. It defines all 13 built-in tools with their required permission levels, implements the five-mode permission engine with nine-step evaluation order, builds the interactive prompter interface for `Prompt` mode and escalation flows, defines the hook system (PreToolUse / PostToolUse / PostToolUseFailure) with matcher patterns and override decisions, specifies tool execution error handling for all failure classes, and implements the full MCP server lifecycle across seven phases with five transport types. This phase depends on the provider API client and configuration loading from Phase 1. It unlocks Phase 3 (the conversation runtime dispatches tool calls through this engine) and Phase 4 (CLI subcommands surface tool and permission state).

## Depends on

- **Phase 1 — Core Infrastructure**: provider API client (for tool results that feed back into provider requests), configuration loading (for permission rules, hook definitions, MCP server declarations, denied-tools lists), workspace path scope validation (for file and shell tool path checks).

## Unlocks

- **Phase 3** — Session & Conversation Runtime: requires the tool system for step 6 (ToolInvocation processing) of the conversation loop, and the permission engine for tool-call gating.
- **Phase 4** — CLI, Bootstrap & Plugin System: requires tool registration for `StatusReport`, `McpInspect`, `SkillsInspect` subcommands; permission state for `StatusReport`; MCP lifecycle for `McpInspect`.
- **Phase 5** — Python Companion Workspace: requires the tool inventory schema as a reference for the mirrored tool pool.

## Scope

### Built-In Tools

**Spec references:** §2.3.1

All 13 built-in tools must be registered with the runtime tool pool. Each tool has a canonical name, a required permission level, a typed input schema, and a typed output schema.

- **ShellExecute** — Permission: `DangerFullAccess`. Inputs: command string (required), optional timeout (unsigned integer, seconds), optional working directory (filesystem path). Output: `stdout` (String), `stderr` (String), `exit_code` (integer). The command is executed in a child process. The working directory defaults to the first workspace root if not specified. Timeout enforcement kills the child process and returns partial output with an error indicator.
- **FileRead** — Permission: `ReadOnly`. Inputs: file path (required), optional start line (1-indexed), optional end line (1-indexed). Output: file contents as text, or an error. Path must pass workspace scope validation (Phase 1). Non-existent paths return a filesystem error result. Binary files are detected and rejected with a descriptive error.
- **FileWrite** — Permission: `WorkspaceWrite`. Inputs: file path (required), content string (required). Output: success confirmation or error. Path must pass workspace scope validation. Parent directories are created if they do not exist. Existing files are overwritten.
- **FileEdit** — Permission: `WorkspaceWrite`. Inputs: file path (required), list of patch hunks (each containing old text and new text). Output: diff summary or error. Each hunk is applied in order. If the old text is not found in the file, the hunk fails with a descriptive error (including the expected text and the actual surrounding content). Path must pass workspace scope validation.
- **GlobSearch** — Permission: `ReadOnly`. Inputs: glob pattern (required), optional root directory. Output: list of matching file paths. The root directory defaults to the first workspace root. Results are confined to the workspace scope (glob expansion applies workspace path validation per Phase 1).
- **GrepSearch** — Permission: `ReadOnly`. Inputs: search pattern (required), search path (required), regex flag (boolean, default false), case-insensitive flag (boolean, default false), per-line flag (boolean, default false). Output: list of match records. Each match record contains: file path, line number (if per-line), line content (if per-line). The search path must pass workspace scope validation.
- **WebSearch** — Permission: `DangerFullAccess`. Inputs: query string (required). Output: list of search result summaries, each with a title, URL, and snippet.
- **WebFetch** — Permission: `DangerFullAccess`. Inputs: URL (required). Output: page content (HTML converted to plain text). URL validation: must be a valid HTTP or HTTPS URL.
- **AgentLaunch** — Permission: `DangerFullAccess`. Inputs: agent configuration object, prompt string. Output: agent execution result. The agent configuration specifies the model, tools, and permission context for the sub-agent.
- **TodoWrite** — Permission: `WorkspaceWrite`. Inputs: list of todo item objects (each with text and optional status). Output: confirmation. Todo items are written to a workspace-local todo file.
- **NotebookEdit** — Permission: `WorkspaceWrite`. Inputs: notebook path (required), list of cell edit objects (each with cell index, optional new source, optional new cell type). Output: updated notebook summary. Path must pass workspace scope validation. The notebook must be a valid `.ipynb` JSON file.
- **SkillInvoke** — Permission: `ReadOnly`. Inputs: skill name (required), parameters (key-value map). Output: skill output (format depends on the skill).
- **ToolSearch** — Permission: `ReadOnly`. Inputs: query string (required). Output: list of matching tool records (name, description, permission level).

Tool registration:
- Each tool is registered with the runtime tool pool at startup.
- The tool pool supports lookup by canonical name and by alias.
- Unregistered tool names default to `DangerFullAccess` permission level.
- The tool pool exposes a listing interface for `StatusReport` and `ToolSearch`.

### Permission Engine

**Spec references:** §3.5, §3.5.1, §3.5.2, §3.5.3

#### Permission Modes

Five modes ordered from least to most permissive:

1. **ReadOnly** — Allows: FileRead, GlobSearch, GrepSearch, SkillInvoke, ToolSearch.
2. **WorkspaceWrite** — Allows: all ReadOnly tools plus FileWrite, FileEdit, NotebookEdit, TodoWrite.
3. **DangerFullAccess** — Allows: all tools including ShellExecute, WebSearch, WebFetch, AgentLaunch.
4. **Prompt** — Requires interactive approval for every tool invocation (via the interactive prompter).
5. **Allow** — Permits everything unconditionally (no checks).

The active permission mode is set at startup (from CLI flag, config, or default) and is immutable for the duration of the session.

#### Nine-Step Evaluation Order

For each tool invocation, execute these steps in order:

1. **Check denied-tools list** — If the tool name appears in the `denied_tools[]` configuration array (case-insensitive comparison), deny immediately with reason `"denied by denied_tools configuration"`. No further evaluation occurs.
2. **Check deny rules** — Iterate over `PermissionRules.deny[]`. If any deny rule matches the tool name and input subject, deny immediately with reason `"denied by deny rule: <rule>"`.
3. **Determine required mode** — Look up the tool's required permission mode from the tool registration table. If the tool is not registered (e.g., an MCP tool), default to `DangerFullAccess`.
4. **Apply hook overrides** — If the permission context carries an override from a PreToolUse hook:
   - `Deny` → deny immediately with the override reason.
   - `Ask` → require interactive approval regardless of the active mode.
   - `Allow` → proceed to the next step (but still honor ask rules in step 5).
5. **Check ask rules** — Iterate over `PermissionRules.ask[]`. If any ask rule matches, require interactive approval via the prompter.
6. **Check allow rules** — Iterate over `PermissionRules.allow[]`. If any allow rule matches, permit the invocation.
7. **Compare modes** — If the active mode is `Allow`, permit. If the active mode's ordinal is ≥ the required mode's ordinal (using the ordering: ReadOnly < WorkspaceWrite < DangerFullAccess), permit.
8. **Prompt for escalation** — If the active mode is `Prompt`, invoke the interactive prompter. If the active mode is `WorkspaceWrite` and the tool requires `DangerFullAccess`, invoke the interactive prompter for escalation.
9. **Default deny** — If no prior step produced a permit or deny decision, deny with reason `"insufficient permission: active mode '<mode>' does not satisfy required mode '<required>'"`.

> [RESOLVED: A3] The interactive prompter interface for Prompt mode and WorkspaceWrite→DangerFullAccess escalation is specified as step 8 of the evaluation order, with the prompter interface defined in the Interactive Prompter subsection below.

#### Rule Matching Syntax

**Spec references:** §3.5.3

Rules use the syntax `ToolName(subject_pattern)`:

- `ToolName` alone → matches any invocation of that tool (equivalent to `ToolName(*)`).
- `ToolName(*)` → wildcard, matches any subject.
- `ToolName(exact_value)` → matches when the extracted subject equals `exact_value` exactly.
- `ToolName(prefix:*)` → matches when the extracted subject starts with `prefix:` (the `*` is a suffix wildcard after the literal prefix).
- Parentheses within the subject value are escaped with backslash: `ToolName(value\(with\)parens)`.
- Tool names are normalized to lowercase before matching.

> [RESOLVED: A4] Subject extraction from well-known JSON keys is specified: the subject is extracted from the tool input by searching for keys in this priority order: `command`, `path`, `file_path`, `filePath`, `notebook_path`, `notebookPath`, `url`, `pattern`, `code`, `message`. The first key found provides the subject value. If the input is not valid JSON, the raw input string is used as the subject.

Subject extraction procedure:
1. Attempt to parse the tool input as JSON.
2. If JSON, search for keys in order: `command`, `path`, `file_path`, `filePath`, `notebook_path`, `notebookPath`, `url`, `pattern`, `code`, `message`.
3. The value of the first found key is the subject (converted to a string if not already).
4. If no key is found in the JSON, use the serialized JSON string as the subject.
5. If the input is not valid JSON, use the raw input string as the subject.

### Interactive Prompter

**Spec references:** §3.5 (step 8)

The interactive prompter is invoked when the permission engine reaches step 8 (Prompt mode or WorkspaceWrite→DangerFullAccess escalation) or step 5 (ask rule match).

- Display to the user:
  - Tool name and tool description.
  - Tool input payload (formatted for readability).
  - The reason the prompter was invoked (e.g., "Prompt mode requires approval" or "Tool requires DangerFullAccess but session is WorkspaceWrite").
- Accept one of three responses:
  - **Allow** — Permit this single invocation. Does not change the session's active mode.
  - **Allow Always** — Permit this invocation and add a runtime allow rule for this tool (persisted for the session duration only, not written to config files).
  - **Deny** — Deny this invocation. The denial reason is `"denied by user via interactive prompt"`.
- Input method: read from stdin (REPL mode). In one-shot/non-interactive mode, the prompter is unavailable; if reached, the tool is denied with reason `"interactive prompt required but session is non-interactive"`.
- Timeout: no timeout (the prompter blocks until the user responds).

### Hook System

**Spec references:** §3.10

Hooks fire at lifecycle events during tool execution:

- **PreToolUse** — Fires before a tool is invoked. The hook can return an override decision:
  - `Allow` — Feed into step 4 of the permission evaluation.
  - `Deny` — Feed into step 4 with a denial reason.
  - `Ask` — Feed into step 4 to force interactive approval.
  - No override — The hook ran but produced no decision; continue normal evaluation.
- **PostToolUse** — Fires after a tool executes successfully. No override decision. Used for logging, auditing, or side-effects.
- **PostToolUseFailure** — Fires after a tool execution fails. No override decision. Used for error reporting or cleanup.

Hook configuration formats:

- **Legacy string format** (deprecated): A bare shell command string assigned to the event name. Executed in a child process. Stdout is parsed for override decisions (PreToolUse only).
- **Object-style format**: Contains:
  - `matcher` (optional): A tool name pattern. Supports case-insensitive matching with `*` wildcards and comma/pipe-separated alternatives (e.g., `FileWrite,FileEdit` or `File*`). If omitted, the hook fires for all tools.
  - `hooks`: A list of hook entries, each with:
    - `type`: The hook event (`PreToolUse`, `PostToolUse`, `PostToolUseFailure`).
    - `command`: The shell command to execute.

Hook execution:
- Hooks execute in configuration order (first defined, first executed).
- Multiple hooks can fire for the same event; they execute sequentially.
- Hook command environment: the hook command receives the tool name and tool input as environment variables or command-line arguments (implementation-defined).
- Hook command timeout: hooks inherit the shell command timeout from `ProviderSettings` (default: 60 seconds). Timed-out hooks are treated as producing no override.
- PreToolUse hook output parsing: the hook's stdout is parsed for a JSON object containing an `override` field with value `"allow"`, `"deny"`, or `"ask"`, and an optional `reason` field. If parsing fails or stdout is empty, no override is produced.

### Tool Execution Error Handling

**Spec references:** §4.2

| Scenario | Error Type | Output |
|----------|-----------|--------|
| Tool name not found in the registry | `not_handled` | Message: `"Unknown mirrored tool: <name>"` |
| Tool blocked by permission deny-list | `not_handled` | Message: `"Permission denied for tool <name>"` |
| Tool payload references path outside workspace scope | `not_handled` | Includes: reason, candidate path, resolved path |
| Shell command times out | `timeout` | Partial stdout/stderr with error indicator |
| File read on non-existent path | `file_error` | Filesystem error message with path |
| File write in read-only mode | `permission_denied` | Routed through permission engine (step 7 or 9 denial) |

Error result structure:
- `handled: false` — the tool invocation was not executed.
- `message` — human-readable error description.
- `error_type` — machine-readable error classification.
- Additional fields vary by error type (e.g., `candidate_path` and `resolved_path` for scope violations).

### MCP Server Lifecycle

**Spec references:** §2.3.2, §3.12, §5.8

MCP servers follow a seven-phase lifecycle:

1. **Discovery** — Read server configuration from the merged config files (`McpServers` map). Each entry specifies a server name, command, args, environment variables, and transport type.
2. **Validation** — Validate required fields:
   - `command` (required for Stdio transport)
   - `url` (required for WebSocket, Remote, and ManagedProxy transports)
   - Transport type must be one of: `stdio`, `websocket`, `remote`, `sdk`, `managed_proxy`.
   - Invalid servers are recorded in `invalid_servers[]` with `error_field` and `reason`. Valid sibling servers are not affected.
3. **Spawn/Connect** — Start the subprocess (Stdio) or establish the network connection (WebSocket, Remote, ManagedProxy). SDK transport uses in-process integration.
   - **StdioTransport**: launch child process with specified command and args; communicate over stdin/stdout JSON-RPC.
   - **WebSocketTransport**: connect to the WebSocket URL; communicate over WebSocket frames.
   - **RemoteTransport**: communicate with a remote HTTP endpoint via JSON-RPC over HTTP.
   - **SdkTransport**: use SDK-level integration (in-process, no network).
   - **ManagedProxyTransport**: connect through an OAuth-authenticated proxy; requires credential exchange before JSON-RPC begins.
4. **Initialize** — Send the MCP `initialize` JSON-RPC request with:
   - Client info: name, version.
   - Protocol version string.
   - Await the server's `initialize` response with server capabilities.
5. **Tool Discovery** — Send `tools/list` JSON-RPC request. Register each discovered tool with the runtime tool pool, prefixing the tool name with the server name to avoid collisions (e.g., `server_name__tool_name`).
6. **Ready** — The server is available for tool invocations. Tool calls are dispatched via `tools/call` JSON-RPC requests.
7. **Error/Degraded** — Failures at any phase are captured:
   - `Failed` state: error recorded with failure class (connection, initialization, tool discovery) and phase.
   - `Degraded` state: the server is partially functional (e.g., initialization succeeded but some tools failed to register). The server remains in the pool with a degraded flag.

Invalid server isolation: a server that fails validation or initialization does not prevent other servers from proceeding through their lifecycle. Each server's lifecycle is independent.

Tool name prefixing: MCP tools are prefixed as `<server_name>__<tool_name>` (double underscore separator). The prefix is stripped when displaying to the model but preserved internally for routing.

## Acceptance criteria

1. All 13 built-in tools are registered in the tool pool with correct canonical names, required permission levels, and typed input/output schemas.
2. The permission engine correctly evaluates all nine steps in order: a denied-tools entry blocks the tool before any rule check; deny rules take precedence over allow rules; hook overrides are applied at step 4; ask rules trigger the prompter; mode comparison uses ordinal ordering.
3. Rule matching correctly handles all syntax variants: bare tool name, wildcard `(*)`, exact match, prefix wildcard `(prefix:*)`, escaped parentheses, and case-insensitive tool name normalization.
4. Subject extraction correctly searches the 10 well-known JSON keys in priority order and falls back to raw input when the payload is not valid JSON.
5. The interactive prompter displays tool name, input, and reason, accepts Allow/Allow Always/Deny responses, and denies in non-interactive mode with a descriptive reason.
6. Hook matchers correctly match tool names with case-insensitive comparison, `*` wildcards, and comma/pipe alternatives. PreToolUse hooks produce Allow/Deny/Ask overrides parsed from stdout JSON.
7. Tool execution errors produce the correct error type and message for each of the six failure scenarios (unknown tool, permission denial, scope violation, timeout, file error, read-only write).
8. MCP server lifecycle progresses through all seven phases for a valid Stdio-transport server, and invalid servers are isolated in `invalid_servers[]` without blocking valid siblings.
9. MCP tool names are correctly prefixed with the server name (double underscore separator) and registered in the runtime tool pool alongside built-in tools.
10. The permission engine denies a tool invocation when the active mode is `ReadOnly` and the tool requires `WorkspaceWrite`, and permits when the active mode is `DangerFullAccess` for the same tool.

## Git commit plan

1. **`feat: register all 13 built-in tools`**
   Define all 13 tool records with canonical names, permission levels, and input/output schemas. Register them in the tool pool.
   *Satisfies acceptance criteria: 1*

2. **`feat: implement permission mode ordering`**
   Define the five permission modes with ordinal ordering and mode comparison logic.
   *Satisfies acceptance criteria: 10*

3. **`feat: add permission rule matching syntax`**
   Implement rule parsing for `ToolName(subject_pattern)` syntax with wildcard, exact, prefix, and escaped parentheses variants. Resolves audit finding A4 (subject extraction from well-known JSON keys).
   *Satisfies acceptance criteria: 3, 4*

4. **`feat: implement nine-step permission evaluation`**
   Build the full evaluation pipeline: denied-tools check, deny rules, required mode lookup, hook override application, ask rules, allow rules, mode comparison, escalation prompt, default deny. Resolves audit finding A3 (interactive prompter interface).
   *Satisfies acceptance criteria: 2, 5*

5. **`feat: add hook system with matcher patterns`**
   Implement PreToolUse/PostToolUse/PostToolUseFailure hooks with legacy and object-style config, case-insensitive wildcard matchers, and stdout-parsed override decisions.
   *Satisfies acceptance criteria: 6*

6. **`feat: implement tool execution error handling`**
   Define error types and messages for all six failure scenarios: unknown tool, permission denial, scope violation, timeout, file error, read-only write.
   *Satisfies acceptance criteria: 7*

7. **`feat: add MCP server lifecycle management`**
   Implement all seven lifecycle phases (Discovery through Ready/Error) with five transport types, tool name prefixing, and invalid server isolation.
   *Satisfies acceptance criteria: 8, 9*

8. **`feat: add interactive prompter interface`**
   Build the interactive prompter for Prompt mode and escalation, with Allow/Allow Always/Deny responses and non-interactive denial.
   *Satisfies acceptance criteria: 5*

9. **`test: add permission engine evaluation tests`**
   Test the nine-step pipeline with vectors covering denied-tools, deny/allow/ask rules, mode comparison, hook overrides, and escalation prompts.
   *Satisfies acceptance criteria: 2, 3, 4, 10*

10. **`test: add MCP lifecycle and hook system tests`**
    Test MCP lifecycle phase transitions, invalid server isolation, tool name prefixing, hook matcher patterns, and PreToolUse override parsing.
    *Satisfies acceptance criteria: 6, 8, 9*

## Open questions

1. **Hook command interface**: The spec does not define whether hook commands receive tool input via environment variables, command-line arguments, or stdin. The implementer should use environment variables (`TOOL_NAME`, `TOOL_INPUT`) for consistency with shell conventions.
2. **MCP server restart policy**: The spec does not specify whether a failed MCP server should be automatically restarted. The implementer should treat failed servers as permanently failed for the session duration (manual restart requires restarting the CLI).
3. **Allow Always scope**: The "Allow Always" response from the interactive prompter adds a runtime allow rule. The spec does not define whether this rule applies to the exact tool name only or to the tool name with the same subject pattern. The implementer should scope it to the tool name only (equivalent to `ToolName(*)`).
4. **MCP tool schema discovery**: The spec does not define how the JSON-Schema input specification for MCP tools is obtained. The implementer should use the `inputSchema` field from the MCP `tools/list` response.
5. **Hook execution parallelism**: The spec states hooks execute "in configuration order" but does not specify whether hooks for the same event can run in parallel. The implementer should execute them sequentially to ensure deterministic override resolution.
