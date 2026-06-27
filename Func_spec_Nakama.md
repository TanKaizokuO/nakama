# Comprehensive Functional Specification — CLI Agent Harness System

> **Document Type:** Language-Agnostic Clean Room Functional Specification
>
> **Subject System:** A multi-layer CLI agent harness that orchestrates conversational AI interactions, tool execution, permission enforcement, session management, and provider-agnostic API communication.
>
> **Scope:** This specification covers the canonical Rust workspace (primary runtime surface), the companion Python porting/audit workspace, and all shared subsystems.

---

## Table of Contents

1. [System Overview](#1-system-overview)
2. [Public API / Interface Contracts](#2-public-api--interface-contracts)
3. [Core Logic & Behavior](#3-core-logic--behavior)
4. [Edge Cases & Error Handling](#4-edge-cases--error-handling)
5. [State Management](#5-state-management)

---

## 1. System Overview

### 1.1 Purpose

The system is a **CLI-based conversational agent harness** that mediates between a human operator and one or more large language model (LLM) providers. It exposes both an interactive read-eval-print loop (REPL) and a one-shot prompt interface, manages multi-turn conversation state, enforces a layered permission model on tool invocations, and provides session persistence, cost tracking, plugin/skill extensibility, and workspace-scoped security guardrails.

### 1.2 High-Level Architecture

The system is decomposed into the following functional layers:

| Layer | Responsibility |
|---|---|
| **CLI Entry** | Argument parsing, subcommand dispatch, output formatting (human-readable or machine-readable), shorthand prompt mode |
| **Provider API** | Multi-provider HTTP client abstraction (Anthropic-native, OpenAI-compatible, xAI, DashScope), SSE streaming, bearer/API-key authentication, request preflight, prompt caching |
| **Conversation Runtime** | Multi-turn conversation loop, automatic compaction, system prompt assembly, tool-call dispatch, stop-reason evaluation |
| **Permission Engine** | Hierarchical permission modes, rule-based allow/deny/ask policies, hook-driven overrides, interactive prompter interface, tool-name deny-lists |
| **Tool System** | Built-in tools (shell execution, file read/write/edit, glob/grep search, web fetch/search, notebook edit, agent launch, todo tracking), skill resolution, tool discovery, MCP bridge |
| **Session Persistence** | Conversation serialization/deserialization (JSONL), session resume, fork, heartbeat, liveness detection |
| **Configuration** | Hierarchical config file loading (user → project → local), model alias resolution, provider routing, MCP server declarations, hook definitions, permission rules |
| **Plugin & Skill System** | Plugin install/enable/disable/update/uninstall lifecycle, skill directory scanning, MCP server lifecycle, tool and command extension surfaces |
| **Usage & Telemetry** | Per-turn and cumulative token accounting, model-specific cost estimation, session tracing events |
| **Bootstrap & Setup** | Multi-phase startup pipeline, prefetch side-effects, deferred trust-gated initialization, health-check diagnostics |
| **Workspace Security** | Path-scope validation, symlink resolution, glob expansion confinement, Windows drive/UNC path handling, workspace-root containment |
| **Compaction Engine** | Context-window management via summarization of older messages, tool-use/tool-result pair preservation at compaction boundaries |
| **Parity Audit** | Cross-language coverage comparison between the Rust implementation and the companion Python/archive workspace |

### 1.3 Companion Python Workspace

A secondary workspace provides:

- A mirrored command and tool inventory loaded from reference snapshot files.
- A query engine that simulates the conversation loop with token budgeting and structured output.
- A prompt routing engine that matches user input to commands and tools using token-based scoring.
- A parity audit that compares file coverage, command entries, and tool entries against an archived reference.
- Session persistence, transcript management, and setup/bootstrap report generation.

This workspace is **not** the primary runtime. It serves as a porting scaffold and audit surface.

---

## 2. Public API / Interface Contracts

### 2.1 CLI Entry Points

#### 2.1.1 Interactive REPL

| Property | Specification |
|---|---|
| **Invocation** | Launch the binary with no subcommand argument |
| **Input** | User-typed prompts or slash commands, tab-completed |
| **Output** | Streamed assistant responses rendered as ANSI-formatted Markdown; tool calls rendered inline |
| **Session** | Conversation state persisted to a workspace-local sessions directory |

#### 2.1.2 One-Shot Prompt

| Property | Specification |
|---|---|
| **Invocation** | Binary followed by `prompt` subcommand and a text argument |
| **Input** | A single prompt string (may also be piped via stdin) |
| **Output** | Complete response text (or JSON envelope when machine-readable output is selected) |
| **Return Code** | 0 on success; non-zero on provider or permission error |

#### 2.1.3 Shorthand Prompt

| Property | Specification |
|---|---|
| **Invocation** | Binary followed by a bare string argument that does not match any subcommand |
| **Behavior** | Treated as equivalent to the one-shot prompt subcommand |
| **POSIX Separator** | A `--` token before the string prevents dash-prefixed text from being parsed as flags |

#### 2.1.4 Direct CLI Subcommands

The system exposes the following non-interactive subcommands. Each accepts an optional output-format flag accepting `text` or `json` (case-insensitive).

| Subcommand | Purpose | Key Output Fields (machine-readable mode) |
|---|---|---|
| **HealthCheck** | Validates API keys, model access, tool config, MCP, memory files, hooks | Array of check objects with name, status, detail |
| **StatusReport** | Reports workspace context, loaded memory files, permission mode, model, MCP validation, hook validation, allowed tools | Structured workspace, model, permissions, memory_files, mcp_validation, hook_validation objects |
| **SandboxInfo** | Reports container/sandbox detection and isolation mode | Container environment enum, filesystem isolation mode |
| **VersionInfo** | Reports build provenance including commit SHA, branch, build timestamp, compiler version, executable path | git_sha, git_sha_short, is_dirty, branch, commit_date, rustc_version, executable_path, binary_provenance |
| **InitWorkspace** | Scaffolds project configuration files, settings, and guidance file | Arrays of created, updated, partial, deferred, skipped artifact paths |
| **DumpManifests** | Emits the resolver inventory (commands, tools, agents, skills, bootstrap phases) | JSON arrays of registry entries |
| **SystemPrompt** | Renders the assembled system prompt for the current workspace | Full prompt text |
| **AgentList** | Lists, shows, or scaffolds agent definitions | Agent metadata objects |
| **McpInspect** | Lists configured MCP servers with validation status | servers[], invalid_servers[], total_configured, valid_count, invalid_count |
| **SkillsInspect** | Lists, shows, installs, or uninstalls skills | Skill metadata objects |
| **BootstrapPlan** | Displays the ordered startup phase plan | Ordered list of phase identifiers |
| **WorkerState** | Reads the persisted worker state from the sessions directory | Worker ID, session reference, model, permission mode |

#### 2.1.5 Global CLI Flags

| Flag | Data Type | Behavior |
|---|---|---|
| **ModelSelection** | String | Selects the LLM model; supports built-in aliases and pass-through for arbitrary names |
| **OutputFormat** | Enum: `text` ∣ `json` | Selects human-readable or machine-readable output; environment variable provides the default, flag overrides |
| **PermissionMode** | Enum (see §3.5) | Sets the active permission level for the session |
| **WorkingDirectory** | Filesystem path | Overrides the process working directory for workspace resolution |
| **SkipPermissions** | Boolean | Escalates to the maximum permission level (danger mode) |
| **AllowedTools** | Comma-separated list | Restricts the tool pool to named tools; accepts canonical names and aliases |
| **ResumeSession** | String (session-id ∣ `latest`) | Resumes a previously persisted session |

---

### 2.2 Provider API Interface

#### 2.2.1 MessageRequest

An outbound request to any LLM provider is structured as follows:

| Field | Data Type | Constraints |
|---|---|---|
| ModelIdentifier | String (required) | Non-empty; resolved from alias table or passed through verbatim |
| MaxOutputTokens | Unsigned 32-bit integer | Must be > 0; model-specific defaults apply |
| MessageHistory | Ordered list of InputMessage records | Must contain at least one message |
| SystemInstruction | Optional string | The assembled system prompt |
| ToolDefinitions | Optional list of ToolDefinition records | Each has name, optional description, and JSON-Schema input specification |
| ToolSelectionPolicy | Optional enum: `Auto` ∣ `Any` ∣ `SpecificTool(name)` | Controls model tool-calling behavior |
| StreamingEnabled | Boolean | When true, response is delivered as an SSE event stream |
| Temperature | Optional float | Range [0.0, 2.0]; omitted for reasoning models |
| TopP | Optional float | Range [0.0, 1.0]; omitted for reasoning models |
| FrequencyPenalty | Optional float | Provider-specific range |
| PresencePenalty | Optional float | Provider-specific range |
| StopSequences | Optional list of strings | Provider stops generation at any matching sequence |
| ReasoningEffort | Optional enum: `low` ∣ `medium` ∣ `high` | For reasoning-capable models only |
| ProviderExtensions | Key-value map of arbitrary JSON | Passed to the provider after core fields; core protocol keys are protected from override |

#### 2.2.2 MessageResponse

| Field | Data Type |
|---|---|
| ResponseId | String |
| Role | Constant: `assistant` |
| ContentBlocks | Ordered list of OutputContentBlock variants (see below) |
| ModelUsed | String |
| StopReason | Optional string (`end_turn`, `max_tokens`, `tool_use`, etc.) |
| TokenUsage | UsageRecord (input_tokens, output_tokens, cache_creation_tokens, cache_read_tokens) |

**OutputContentBlock variants:**

| Variant | Fields |
|---|---|
| TextContent | text: String |
| ToolInvocation | id: String, name: String, input: JSON |
| ThinkingContent | thinking: String, signature: Optional String |
| RedactedThinking | opaque data blob |

#### 2.2.3 SSE Stream Events

When streaming is enabled, the response is delivered as a sequence of typed events:

1. **SessionStart** — contains the initial MessageResponse skeleton
2. **ContentBlockBegin** — index and initial content block
3. **ContentBlockDelta** — incremental text, JSON, thinking, or signature fragments
4. **ContentBlockEnd** — signals completion of a content block
5. **MessageDelta** — stop_reason and final usage
6. **SessionEnd** — signals completion of the full response

#### 2.2.4 Provider Routing

Provider selection follows a deterministic cascade:

1. If the resolved model name contains `claude` → Anthropic-native provider.
2. If it contains `grok` → xAI provider.
3. If it starts with `openai/`, `local/`, or `gpt-` → OpenAI-compatible provider.
4. If it starts with `qwen/`, `qwen-`, `kimi/`, or `kimi-` → DashScope (Alibaba) provider.
5. If a local-server base URL is set and the model name looks local → OpenAI-compatible.
6. Fall back by checking which credential environment variable is populated (Anthropic → OpenAI → xAI).
7. If nothing matches → default to Anthropic.

#### 2.2.5 Authentication

| Credential Shape | Environment Variable | HTTP Header |
|---|---|---|
| API key (`sk-ant-*`) | `ANTHROPIC_API_KEY` | `x-api-key: <value>` |
| OAuth / bearer token | `ANTHROPIC_AUTH_TOKEN` | `Authorization: Bearer <value>` |
| OpenAI-compatible key | `OPENAI_API_KEY` | `Authorization: Bearer <value>` |
| xAI key | `XAI_API_KEY` | `Authorization: Bearer <value>` |
| DashScope key | `DASHSCOPE_API_KEY` | `Authorization: Bearer <value>` |
| Local (Ollama) | `OLLAMA_HOST` (URL only) | No auth header |

---

### 2.3 Tool System Interface

#### 2.3.1 Built-In Tools

| Tool Name | Required Permission Level | Inputs | Output |
|---|---|---|---|
| **ShellExecute** | DangerFullAccess | Command string, optional timeout, optional working directory | stdout, stderr, exit code |
| **FileRead** | ReadOnly | File path, optional line range | File contents (text) or error |
| **FileWrite** | WorkspaceWrite | File path, content string | Success confirmation or error |
| **FileEdit** | WorkspaceWrite | File path, list of patch hunks (old text → new text) | Diff summary or error |
| **GlobSearch** | ReadOnly | Glob pattern, optional root directory | List of matching file paths |
| **GrepSearch** | ReadOnly | Search pattern, search path, regex flag, case-insensitive flag, per-line flag | List of matches (file, line number, content) |
| **WebSearch** | DangerFullAccess | Query string | Search result summaries with URLs |
| **WebFetch** | DangerFullAccess | URL | Page content (HTML converted to text) |
| **AgentLaunch** | DangerFullAccess | Agent configuration, prompt | Agent execution result |
| **TodoWrite** | WorkspaceWrite | Todo items list | Confirmation |
| **NotebookEdit** | WorkspaceWrite | Notebook path, cell edits | Updated notebook |
| **SkillInvoke** | ReadOnly | Skill name, parameters | Skill output |
| **ToolSearch** | ReadOnly | Query string | List of matching tool names and descriptions |

#### 2.3.2 MCP Tool Bridge

External tools are surfaced through the Model Context Protocol (MCP). The system manages MCP server lifecycles including:

- **StdioTransport** — launches a subprocess and communicates over stdin/stdout JSON-RPC
- **WebSocketTransport** — connects to a WebSocket endpoint
- **RemoteTransport** — communicates with a remote HTTP endpoint
- **SdkTransport** — uses SDK-level integration
- **ManagedProxyTransport** — connects through an OAuth-authenticated proxy

MCP tool names are prefixed with the server name to avoid collisions. The discovery phase lists available tools from each server and registers them with the runtime tool pool.

---

### 2.4 Configuration Interface

#### 2.4.1 Config File Hierarchy (Precedence: Low → High)

1. User-level legacy config file (`~/.claw.json`)
2. User-level settings (`~/.config/claw/settings.json`)
3. Project-level legacy config file (`<repo>/.claw.json`)
4. Project-level settings (`<repo>/.claw/settings.json`)
5. Project-level local settings (`<repo>/.claw/settings.local.json`)

Later files override earlier files for overlapping keys.

#### 2.4.2 Configuration Fields

| Field | Type | Description |
|---|---|---|
| ModelAliases | Map: alias → model_name | User-defined short names for model identifiers |
| McpServers | Map: server_name → server_config | MCP server declarations (command, args, environment, transport type) |
| Hooks | Map: event_name → hook_list | Lifecycle hooks for PreToolUse, PostToolUse, PostToolUseFailure |
| PermissionRules | Object: allow[], deny[], ask[], denied_tools[] | Fine-grained tool permission rules |
| ProviderSettings | Object | API timeout overrides, provider fallback config |
| PluginConfig | List of plugin objects | Plugin enable/disable state |
| FeatureFlags | Object | Runtime feature toggles |
| RulesImport | Enum: `auto` ∣ `none` ∣ list of framework names | Controls import of instruction files from other AI coding tools |

#### 2.4.3 Project Instruction Files

The system loads instruction/guidance content from the following paths (priority order):

1. `CLAUDE.md` (highest priority)
2. `CLAW.md`
3. `AGENTS.md`
4. `.claw/CLAUDE.md`, `.claude/CLAUDE.md`, `.claw/instructions.md`
5. Sorted files from `.claw/rules/` (`.md`, `.txt`, `.mdc`)
6. Sorted files from `.claw/rules.local/` (gitignored, personal)

Discovery is bounded to the current Git root when present; otherwise to the working directory only.

---

### 2.5 Python Companion Workspace Interface

The Python companion workspace operates by parsing the serialized JSONL session files and the `worker-state.json` file generated within the `<workspace>/.claw/` directory by the primary Rust runtime, ensuring state evaluations are structurally decoupled from memory shared across execution lifecycles.

#### 2.5.1 CLI Subcommands

| Subcommand | Inputs | Output |
|---|---|---|
| **RenderSummary** | None | Markdown summary of the porting workspace state |
| **ShowManifest** | None | Module inventory with file counts and notes |
| **ParityAudit** | None | Coverage ratios and missing targets |
| **SetupReport** | None | Platform info, prefetch results, deferred init status |
| **CommandGraph** | None | Command categorization (builtins, plugin-like, skill-like) |
| **ToolPool** | Optional: simple_mode, include_mcp, permission_context | Filtered tool inventory |
| **BootstrapGraph** | None | Ordered list of bootstrap stages |
| **ListSubsystems** | Optional: limit (integer, default 32) | Top-level modules with file counts |
| **ListCommands** | Optional: limit, query, exclude_plugins, exclude_skills | Filtered command entries |
| **ListTools** | Optional: limit, query, simple_mode, no_mcp, deny_tool[], deny_prefix[] | Filtered tool entries |
| **RoutePrompt** | Prompt string, optional limit | Ranked list of matching commands/tools with scores |
| **BootstrapSession** | Prompt string, optional limit | Full runtime session report |
| **TurnLoop** | Prompt string, optional limit, max_turns, structured_output flag | List of turn results |
| **FlushTranscript** | Prompt string | Persisted path and flush status |
| **LoadSession** | Session ID string | Session metadata (messages, tokens) |
| **RemoteMode / SshMode / TeleportMode** | Target string | Mode report (mode, connected, detail) |
| **DirectConnectMode / DeepLinkMode** | Target string | Mode report (mode, target, active) |
| **ShowCommand / ShowTool** | Name string | Module details (name, source, responsibility) |
| **ExecuteCommand** | Name string, prompt string | Execution result (handled flag, message) |
| **ExecuteTool** | Name string, payload string | Execution result (handled flag, message) |

---

## 3. Core Logic & Behavior

### 3.1 Bootstrap Sequence

The system starts through a multi-phase pipeline. Each phase is executed in order; duplicate phases are deduplicated while preserving insertion order.

1. **CLIEntry** — Parse command-line arguments; exit early for fast-path subcommands (version, help).
2. **FastPathVersion** — Handle version queries without loading the full runtime.
3. **StartupProfiler** — Initialize timing infrastructure.
4. **SystemPromptFastPath** — Pre-assemble the system prompt if the subcommand requires it.
5. **ChromeMcpFastPath** — Pre-connect to Chrome-related MCP servers if configured.
6. **DaemonWorkerFastPath** — Check for a running daemon worker and resume if possible.
7. **BridgeFastPath** — Initialize bridge connections to external processes.
8. **DaemonFastPath** — Detect or launch daemon processes.
9. **BackgroundSessionFastPath** — Resume background sessions if applicable.
10. **TemplateFastPath** — Load template configurations.
11. **EnvironmentRunnerFastPath** — Configure environment-specific runners.
12. **MainRuntime** — Enter the full conversation runtime (REPL or one-shot).

### 3.2 Conversation Runtime

The conversation loop follows this sequence for each turn:

1. **Receive user input** — From the REPL readline or the one-shot prompt argument.
2. **Route the prompt** — Check for slash-command prefixes; if found, dispatch to the command handler. Otherwise, treat as a model prompt.
3. **Assemble the API request** — Build the message history, system prompt, tool definitions, and model parameters.
4. **Request preflight** — Estimate request size; compare against the model's context window. If the request exceeds the budget, trigger automatic compaction.
5. **Send to provider** — Submit via the appropriate provider client (streaming or non-streaming).
6. **Process response** — Iterate over response content blocks:
   - **TextContent** → Render to the user.
   - **ThinkingContent** → Optionally display or suppress.
   - **ToolInvocation** → Evaluate permissions, execute the tool, append the result to the history.
   - **RedactedThinking** → Preserve opaquely for conversation continuity.
7. **Evaluate stop reason**:
   - `end_turn` → The model has finished; prompt for next user input.
   - `tool_use` → The model wants to call tool(s); loop back to step 5 with tool results appended.
   - `max_tokens` → The model ran out of output budget; signal to the user.
8. **Record usage** — Update the cumulative token tracker.
9. **Check compaction threshold** — If the session exceeds the configured token budget, trigger compaction before the next turn.
10. **Persist session** — Write the updated conversation state to disk.

### 3.3 Prompt Routing (Python Companion)

When a user submits a prompt, the routing engine:

1. **Extract tokens** — Split the prompt into lowercase tokens, treating `/` and `-` as separators.
2. **Check for explicit command match** — If the first token (with optional `/` prefix removed) matches a known command name, create a high-priority match (score = 100).
3. **Score all commands and tools** — For each registered command/tool module, count how many prompt tokens appear in the module's name, source hint, or responsibility description.
4. **Rank and select** — Prioritize the explicit match first, then the highest-scoring command, then the highest-scoring tool, then fill remaining slots from a merged and sorted leftover pool (sorted by descending score, then alphabetically by kind and name).
5. **Apply limit** — Return at most `limit` matches.

### 3.4 Query Engine (Python Companion)

The query engine manages a simulated conversation:

1. **Check turn limit** — If the accumulated message count meets or exceeds the configured maximum turns, return immediately with a `max_turns_reached` stop reason.
2. **Build output** — Assemble a summary from the prompt, matched commands, matched tools, and permission denials.
3. **Format output** — If structured output mode is enabled, serialize as a JSON object (with retry on serialization failure, up to the configured retry limit). The simplified fallback payload must omit optional tracking arrays and contain only the minimal mandatory communication fields: `session_id`, `prompt`, `output_text`, and a boolean flag `fallback_mode: true`. Otherwise, join as newline-separated text.
4. **Compute usage** — Estimate input tokens as the word count of the prompt; estimate output tokens as the word count of the output. Add to the cumulative usage.
5. **Check budget** — If cumulative total tokens exceed the configured maximum budget, set stop reason to `max_budget_reached`.
6. **Append to transcript** — Store the prompt in the mutable message list and the transcript store.
7. **Compact if needed** — If the message count exceeds the compaction threshold, trim to keep only the most recent N messages.
8. **Return result** — Yield a turn result containing the prompt, output, matched items, denials, usage, and stop reason.

#### 3.4.1 Streaming Variant

The streaming variant yields a sequence of events in order:

1. `session_start` — session ID and prompt
2. `command_match` — list of matched command names (only if non-empty)
3. `tool_match` — list of matched tool names (only if non-empty)
4. `permission_denial` — list of denial records (only if non-empty)
5. `message_delta` — the full output text
6. `session_end` — usage counters, stop reason, transcript size

### 3.5 Permission Evaluation

Permission evaluation follows a multi-stage decision pipeline:

#### 3.5.1 Permission Modes (Ordered from Least to Most Permissive)

| Mode | Allows |
|---|---|
| **ReadOnly** | File reads, glob/grep searches, skill invocations, status queries |
| **WorkspaceWrite** | All ReadOnly tools plus file write/edit/notebook within the workspace |
| **DangerFullAccess** | All tools including shell execution, web access, agent launches |
| **Prompt** | Requires interactive approval for every tool invocation |
| **Allow** | Permits everything unconditionally |

#### 3.5.2 Evaluation Order

For each tool invocation:

1. **Check denied-tools list** — If the tool name appears in the unconditional deny list (case-insensitive), deny immediately with reason `"denied by denied_tools configuration"`.
2. **Check deny rules** — If a deny rule matches the tool name and input subject, deny immediately.
3. **Determine required mode** — Look up the tool's required permission mode (defaults to DangerFullAccess if unregistered).
4. **Apply hook overrides** — If a permission context carries an override:
   - `Deny` → deny immediately with the override reason.
   - `Ask` → require interactive approval regardless of mode.
   - `Allow` → proceed but still honor ask rules.
5. **Check ask rules** — If an ask rule matches, require interactive approval.
6. **Check allow rules** — If an allow rule matches, permit.
7. **Compare modes** — If the active mode is `Allow`, or the active mode ≥ the required mode, permit.
8. **Prompt for escalation** — If the active mode is `Prompt`, or if active mode is `WorkspaceWrite` and the tool requires `DangerFullAccess`, invoke the interactive prompter.
9. **Default deny** — If no other path permits, deny with a descriptive reason.

#### 3.5.3 Rule Matching

Permission rules use the syntax: `ToolName(subject_pattern)`

- `ToolName` alone → matches any invocation of that tool.
- `ToolName(*)` → same as above.
- `ToolName(exact_value)` → matches when the extracted subject equals the value.
- `ToolName(prefix:*)` → matches when the extracted subject starts with the prefix.
- Parentheses in the value can be escaped with backslash.
- Tool names are normalized to lowercase for matching.

The subject is extracted from the tool input by searching for well-known JSON keys in order: `command`, `path`, `file_path`, `filePath`, `notebook_path`, `notebookPath`, `url`, `pattern`, `code`, `message`. If no JSON is present, the raw input string is used.

### 3.6 Workspace Path Scope Validation

For security-sensitive tools (shell execution, file read/write/edit), the system validates that all path operands resolve within configured workspace roots:

1. **Extract path candidates** from the tool payload using shell tokenization. The system must tokenise payload strings using standard POSIX shell parsing conventions. If parsing fails due to unmatched quotes or malformed shell syntax, it must fallback to an absolute whitespace split, sanitising the resulting tokens by stripping matching external quotation marks before applying variable expansion.
2. **Filter candidates** — Ignore tokens starting with `-` (flags) and environment variable assignments.
3. **Strip redirection operators** — Extract the target path from shell redirections (`>`, `>>`, `<`, `<>`).
4. **Identify path-like tokens** — A token is path-like if it contains `/`, `\`, starts with `./`, `../`, `/`, `~/`, is `.` or `..`, contains glob metacharacters (`*`, `?`, `[`), or matches a Windows drive letter pattern.
5. **Expand variables** — Apply environment variable and home directory expansion.
6. **Resolve relative paths** — Resolve against the current working directory or the first workspace root.
7. **Expand globs** — If the path contains glob metacharacters, expand and validate each match. For unmatched globs, validate the stable (non-glob) prefix.
8. **Check containment** — The resolved path must be a descendant of at least one configured workspace root (using symlink-resolved, canonical paths).
9. **Windows paths** — Windows-style absolute paths (`C:\...`, `\\server\share`) are validated against any Windows-style workspace root using platform path comparison.

Decision: `allowed` with reason, or `denied` with candidate path and resolved path.

### 3.7 Session Compaction

When the conversation transcript exceeds the configured token budget:

1. **Estimate tokens** — Sum the estimated token footprint of each message. Token footprints are estimated arithmetically using integer floor division: $\lfloor \text{character\_count} / 4 \rfloor + 1$ per content block.
2. **Check threshold** — Compaction triggers when the number of compactable messages (excluding any existing compacted summary prefix) exceeds the preservation count AND the estimated tokens exceed the maximum budget.
3. **Determine compaction boundary** — Preserve the most recent N messages. Adjust the boundary to avoid splitting tool-use / tool-result pairs: if the first preserved message starts with a tool result, walk the boundary backward until the paired tool-use assistant message is also preserved.
4. **Summarize removed messages** — Generate a structured summary containing:
   - Scope (message counts by role)
   - Unique tool names mentioned
   - Recent user requests (up to 3, truncated to 160 characters)
   - Inferred pending work items (messages containing "todo", "next", "pending", "follow up", "remaining")
   - Key files referenced (extracted from path-like tokens with recognized extensions)
   - Current work inference (last non-empty text)
   - Key timeline (chronological per-message summaries)
5. **Merge with prior summary** — If a prior compaction summary exists, flatten its highlights (without nesting) and append the new highlights to prevent summary inflation across multiple compaction cycles.
6. **Build continuation message** — Create a synthetic system message containing:
   - A preamble stating the session is continued from a prior conversation.
   - The formatted summary.
   - A note about preserved recent messages (if any).
   - An instruction to resume directly without recap (if configured).
7. **Replace message history** — The compacted session starts with the synthetic system message followed by the preserved recent messages.
8. **Record compaction event** — Store the summary text and removed message count in the session metadata.

### 3.8 Usage Tracking & Cost Estimation

#### 3.8.1 Token Usage Record

| Field | Type |
|---|---|
| InputTokens | Unsigned 32-bit integer |
| OutputTokens | Unsigned 32-bit integer |
| CacheCreationTokens | Unsigned 32-bit integer |
| CacheReadTokens | Unsigned 32-bit integer |

**Total tokens** = InputTokens + OutputTokens + CacheCreationTokens + CacheReadTokens

#### 3.8.2 Model-Specific Pricing

| Model Family | Input ($/M tokens) | Output ($/M tokens) | Cache Write ($/M tokens) | Cache Read ($/M tokens) |
|---|---|---|---|---|
| Haiku-class | 1.00 | 5.00 | 1.25 | 0.10 |
| Sonnet-class | 15.00 | 75.00 | 18.75 | 1.50 |
| Opus-class | 15.00 | 75.00 | 18.75 | 1.50 |
| Unknown | 15.00 (default) | 75.00 (default) | 18.75 (default) | 1.50 (default) |

**Cost formula:** `cost = (tokens / 1,000,000) × rate_per_million`

**Dollar formatting:** `$<amount>` with 4 decimal places.

#### 3.8.3 Cumulative Tracker

The usage tracker records:
- The latest turn's token usage.
- The cumulative token usage across all turns (additive).
- The total number of turns.

It can be reconstructed from a persisted session by iterating over all messages that carry usage metadata.

### 3.9 Model Alias Resolution

Built-in aliases map short names to full model identifiers:

| Alias | Resolved Name |
|---|---|
| `opus` | `claude-opus-4-7` |
| `sonnet` | `claude-sonnet-4-6` |
| `haiku` | `claude-haiku-4-5-20251213` |
| `grok` / `grok-3` | `grok-3` |
| `grok-mini` / `grok-3-mini` | `grok-3-mini` |
| `kimi` | `kimi-k2.5` |
| `qwen-max` | `qwen-max` |
| `qwen-plus` | `qwen-plus` |

User-defined aliases in configuration files override built-in aliases. Alias resolution is applied before provider routing.

Model selection precedence: CLI flag → environment variable (`CLAW_MODEL` → `ANTHROPIC_MODEL` → `ANTHROPIC_DEFAULT_MODEL`) → configuration file → hardcoded default.

### 3.10 Hook System

Hooks fire at lifecycle events during tool execution:

| Event | Timing |
|---|---|
| **PreToolUse** | Before a tool is invoked; can abort or modify the invocation |
| **PostToolUse** | After successful tool execution |
| **PostToolUseFailure** | After a tool execution fails |

Hook configuration supports two formats:
- **Legacy string** — A bare shell command string (deprecated).
- **Object-style** — Contains a `matcher` (optional, matches tool names case-insensitively with `*` wildcards and comma/pipe alternatives) and a nested list of `hooks` (each with `type` and `command`).

Hooks execute in configuration order. A PreToolUse hook can return an override decision (Allow, Deny, or Ask) that feeds into the permission evaluation pipeline.

### 3.11 Plugin Lifecycle

Plugins follow a state machine:

```
Discovered → Installed → Enabled ⇆ Disabled → Uninstalled
```

Each state transition fires a lifecycle event. The plugin system supports:
- **Healthcheck** — Validates that the plugin's resources (MCP servers, tools) are reachable.
- **Degraded mode** — Plugins can continue in a limited capacity when dependencies are partially available.
- **Tool registration** — Plugins contribute tools to the runtime tool pool.

### 3.12 MCP Server Lifecycle

MCP servers follow a hardened lifecycle with phases:

1. **Discovery** — Read server configuration from config files.
2. **Validation** — Validate required fields (e.g., command, args). Invalid servers are recorded separately without preventing valid servers from loading.
3. **Spawn/Connect** — Start the subprocess or establish the network connection.
4. **Initialize** — Send the MCP `initialize` JSON-RPC request with client info and protocol version.
5. **Tool Discovery** — Request the list of available tools and register them with the runtime.
6. **Ready** — The server is available for tool calls.
7. **Error/Degraded** — Failures at any phase are captured with error surfaces and degraded reports.

---

## 4. Edge Cases & Error Handling

### 4.1 Authentication Errors

| Scenario | Behavior |
|---|---|
| No credential environment variable is set | Error message listing expected environment variables; if a non-Anthropic credential is detected, the message hints at the correct model prefix for provider routing |
| API key placed in wrong variable (`sk-ant-*` in `ANTHROPIC_AUTH_TOKEN`) | 401 error with a specific hint: "Move the key to `ANTHROPIC_API_KEY`" |
| Expired OAuth token | Refresh attempt; if refresh fails, error with re-authentication instructions |
| Invalid or revoked key | Provider returns 401/403; error surfaced to user with the HTTP status |

### 4.2 Tool Execution Errors

| Scenario | Behavior |
|---|---|
| Tool name not found in registry | Return a `not handled` result with message: "Unknown mirrored tool: `<name>`" |
| Tool blocked by permission deny-list | Return a `not handled` result with message: "Permission denied for tool `<name>`" |
| Tool payload references path outside workspace scope | Return a `not handled` result with reason, candidate path, and resolved path |
| Shell command times out | Return the partial output with an error indicator |
| File read on non-existent path | Return an error result with the filesystem error |
| File write in read-only mode | Permission denied via the permission engine |

### 4.3 Session Errors

| Scenario | Behavior |
|---|---|
| Resume requested but session file not found | Error: "no session file found at `<path>`" |
| Session file is corrupted JSON | Deserialization error with file path |
| Worker state file not found | Structured error with hint: "Run the REPL or a one-shot prompt first to produce the worker state file" |
| Session fork fails (no messages) | Error: cannot fork an empty session |

### 4.4 Configuration Errors

| Scenario | Behavior |
|---|---|
| Malformed MCP server config (missing required field) | Server recorded in `invalid_servers[]` with error_field, reason; valid sibling servers still load |
| Unknown hook event name | Recorded as invalid hook with `kind: unknown_hook_event`; valid hooks still load |
| Invalid output format value | Typed error `invalid_output_format` with value and expected array `["text", "json"]` |
| Invalid tool name in `--allowedTools` | Typed error `invalid_tool_name` with the name, available tools list, and aliases |
| Missing argument for `--allowedTools` before a subcommand | Typed error `missing_argument` with `argument: "--allowedTools"` |
| Repeated `--output-format` flags | Warning on stderr; last value wins; JSON status exposes `format_overridden: true` |
| Invalid working directory path | Typed error `invalid_cwd` in JSON mode |

### 4.5 Provider Errors

| Scenario | Behavior |
|---|---|
| Request body exceeds provider size limit | Preflight check fails; automatic compaction triggered before retry |
| Context window exceeded | Preflight check fails; automatic compaction triggered |
| Rate limiting (429) | Error surfaced with retry guidance |
| Provider unreachable (network error) | Connection error with timeout details |
| SSE stream interrupted | Partial response preserved; error appended |
| Structured output serialization fails | Retry up to the configured limit (default: 2) with a simplified fallback payload; if all retries fail, raise a runtime error. The simplified fallback payload must omit optional tracking arrays and contain only the minimal mandatory communication fields: `session_id`, `prompt`, `output_text`, and a boolean flag `fallback_mode: true`. |

### 4.6 Compaction Edge Cases

| Scenario | Behavior |
|---|---|
| Session has fewer messages than the preservation count | No compaction occurs; session returned unchanged |
| First preserved message is a tool-result without a preceding tool-use | Boundary walked backward to include the paired tool-use assistant message |
| Preserve-recent-messages set to 0 | Maximum compaction: all messages are summarized, none are preserved verbatim |
| Existing compaction summary at the start of the session | Prior highlights are flattened (not re-nested) to avoid summary inflation |
| Long content blocks in summary | Truncated to 160 characters with an ellipsis appended |

### 4.7 Path Scope Edge Cases

| Scenario | Behavior |
|---|---|
| Symlink resolves outside workspace | Denied: "path resolves outside workspace scope" |
| Glob expands to paths outside workspace | Each expansion checked individually; any out-of-scope match causes denial |
| Unmatched glob pattern | The stable non-glob prefix is validated instead |
| Windows drive path on a POSIX workspace root | Denied unless a matching Windows-style root is configured |
| UNC path (`\\server\share\...`) | Treated as Windows absolute path; validated against Windows-style roots |
| Environment variable in path | Expanded before validation |
| Home directory shorthand (`~/...`) | Expanded before validation |
| Shell redirection in payload (`>file`, `>>file`, `<file`) | Target path extracted and validated |
| Malformed shell syntax | Fallback to whitespace splitting instead of POSIX shlex parsing |

---

## 5. State Management

### 5.1 Session State

A session is the primary persistent state object. It contains:

| Field | Type | Persistence |
|---|---|---|
| SessionIdentifier | Unique string (UUID-based) | Persisted to disk as filename |
| MessageHistory | Ordered list of conversation messages | Persisted as JSONL |
| CompactionHistory | List of compaction records (summary, removed count) | Persisted within session |
| Heartbeat | Timestamp of last activity | Updated on each turn |
| Liveness | Boolean indicating whether the session is actively in use | Set on REPL start; cleared on exit |

Sessions are stored in `<workspace>/.claw/sessions/` as JSONL files named by session identifier.

**Session resume** loads a previously persisted session and reconstructs the conversation runtime state, including the usage tracker (by replaying all messages with usage metadata).

**Session fork** creates a new session from the current state, assigning a new identifier and preserving the full message history.

### 5.2 Conversation Messages

Each message in the history contains:

| Field | Type |
|---|---|
| Role | Enum: `System` ∣ `User` ∣ `Assistant` ∣ `Tool` |
| ContentBlocks | Ordered list of content blocks (Text, ToolUse, ToolResult, Thinking) |
| Usage | Optional token usage record (populated for assistant messages) |

### 5.3 Transcript Store (Python Companion)

The transcript store maintains:

| Field | Type | Behavior |
|---|---|---|
| Entries | Ordered list of prompt strings | Grows with each submitted message |
| FlushedFlag | Boolean | Set to `true` after `flush()`; reset to `false` on `append()` |

**Compaction:** When the entry count exceeds a threshold, the oldest entries are discarded, keeping only the most recent N entries.

**Replay:** Returns a snapshot of all current entries as an immutable tuple.

### 5.4 Usage Tracker State

| Field | Type | Mutability |
|---|---|---|
| LatestTurnUsage | TokenUsage record | Replaced on each `record()` call |
| CumulativeUsage | TokenUsage record | Additively accumulated |
| TurnCount | Unsigned integer | Incremented on each `record()` call |

### 5.5 Configuration State

Configuration is loaded once at startup and is immutable for the duration of the process. The effective configuration is the result of merging all discovered config files in precedence order.

Machine-readable output exposes:
- `precedence_rank` for each config file
- `wins_for_keys` — which keys this file controls
- `shadowed_keys` — which keys are overridden by higher-precedence files

### 5.6 Permission State

| Field | Mutability |
|---|---|
| Active permission mode | Set at startup; immutable during session |
| Tool requirement map | Set at startup from tool definitions |
| Allow/Deny/Ask rule lists | Set at startup from configuration |
| Denied-tools list | Set at startup from configuration |
| Hook override context | Created fresh per tool invocation |

### 5.7 Worker State

Worker state is written to `<workspace>/.claw/worker-state.json` by the REPL or one-shot prompt after the first turn executes. It contains:

| Field | Type |
|---|---|
| WorkerIdentifier | Unique string |
| SessionReference | Session identifier |
| Model | Currently active model name |
| PermissionMode | Active permission mode |

### 5.8 MCP Server State

Each configured MCP server transitions through lifecycle phases:

| Phase | State |
|---|---|
| Discovery | Configuration loaded, not yet validated |
| Validated | Configuration is structurally correct |
| Spawned | Subprocess started or connection established |
| Initialized | JSON-RPC handshake completed |
| ToolsDiscovered | Available tools listed and registered |
| Ready | Server is available for tool invocations |
| Failed | Error recorded with failure class and phase |
| Degraded | Partially functional (some tools available) |

Invalid servers are tracked separately with their error details, ensuring that valid servers remain operational.

### 5.9 Plugin State

Each plugin maintains:

| Field | Type |
|---|---|
| PluginIdentifier | Unique string |
| State | Enum: `Discovered` ∣ `Installed` ∣ `Enabled` ∣ `Disabled` ∣ `Uninstalled` |
| HealthStatus | Object with server health, tool info, resource info |
| DegradedMode | Optional degradation details |

### 5.10 Parity Audit State (Python Companion)

The parity audit produces an immutable result containing:

| Field | Type | Description |
|---|---|---|
| ArchivePresent | Boolean | Whether the reference archive directory exists |
| RootFileCoverage | Ratio (matched / expected) | How many expected root files exist in the Python workspace |
| DirectoryCoverage | Ratio (matched / expected) | How many expected directories exist |
| TotalFileRatio | Ratio (Python files / archived files) | Overall file count comparison |
| CommandEntryRatio | Ratio (snapshot count / reference count) | Command inventory coverage |
| ToolEntryRatio | Ratio (snapshot count / reference count) | Tool inventory coverage |
| MissingRootTargets | List of strings | Expected root files not found |
| MissingDirectoryTargets | List of strings | Expected directories not found |

---

*End of Specification*
