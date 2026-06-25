# Phase 3 — Session & Conversation Runtime

## Overview

Phase 3 delivers the conversation runtime and session persistence layer — the central execution loop that ties together the provider API (Phase 1) and the tool system (Phase 2). It implements the full 10-step conversation turn loop with slash-command routing as a distinct step, session persistence via JSONL serialization/deserialization with resume and fork operations, the compaction engine for context-window management, provider error handling with structured output retry and simplified fallback payloads, and the session state schema freeze that defines the canonical JSONL output contract consumed by Phase 5's Python reader. This phase depends on Phase 1 (provider API, usage tracker, config) and Phase 2 (tool system, permission engine). It unlocks Phase 4 (CLI entry dispatches into this runtime, WorkerState reads session state) and Phase 5 (Python companion consumes the frozen JSONL schema).

## Depends on

- **Phase 1 — Core Infrastructure**: provider API client (for step 5: send to provider), SSE stream events (for streaming response processing), usage tracker (for step 8: record usage), configuration loading (for compaction thresholds, model parameters, system prompt assembly), workspace path scope validation (inherited through tool dispatch).
- **Phase 2 — Tool System & Permission Engine**: tool pool (for step 6: tool-call dispatch), permission engine (for tool invocation gating), hook system (for PreToolUse/PostToolUse lifecycle events during tool execution).

## Unlocks

- **Phase 4** — CLI, Bootstrap & Plugin System: the REPL and one-shot prompt modes enter this conversation runtime at the MainRuntime bootstrap phase; `WorkerState` subcommand reads the worker-state.json produced by this phase; `DumpManifests` and `BootstrapPlan` must output schemas compatible with the JSONL contract frozen here.
- **Phase 5** — Python Companion Workspace: the Python JSONL loader consumes the session state schema frozen in this phase's data contracts; the query engine's structured output retry uses the same fallback payload spec.

## Scope

### Conversation Runtime — 10-Step Turn Loop

**Spec references:** §3.2

The conversation loop executes the following steps for each turn:

1. **Receive user input** — Read from the REPL readline (interactive mode) or from the one-shot prompt argument / stdin pipe (non-interactive mode). Empty input is silently ignored (no API call is made).

2. **Route the prompt** — Check if the input begins with a `/` prefix (slash command).
   - If it matches a known slash command, dispatch to the corresponding command handler. The command handler executes synchronously and produces output directly (not through the model). After the command handler returns, skip steps 3–7 and proceed to step 10 (persist session).
   - If the `/` prefix does not match any known command, treat it as a model prompt (proceed to step 3).
   - If the input has no `/` prefix, treat it as a model prompt.

   > [RESOLVED: A6] Slash-command routing is specified as a distinct step (step 2) in the turn loop, separate from model-prompt routing. Slash commands bypass the model entirely and dispatch to command handlers.

3. **Assemble the API request** — Build a `MessageRequest` (Phase 1) containing:
   - `ModelIdentifier`: resolved via alias resolution and model selection precedence.
   - `MessageHistory`: the current conversation messages (including any compacted summary prefix).
   - `SystemInstruction`: the assembled system prompt (from instruction files §2.4.3 and runtime context).
   - `ToolDefinitions`: the registered tool pool (built-in tools from Phase 2 + MCP tools).
   - `ToolSelectionPolicy`: `Auto` by default.
   - `StreamingEnabled`: `true` for REPL mode, configurable for one-shot mode.
   - Model parameters: `Temperature`, `TopP`, etc. from configuration or defaults.

4. **Request preflight** — Estimate the request size in tokens using the arithmetic estimator: `⌊character_count / 4⌋ + 1` per content block, summed across all messages and the system prompt. Compare against the model's context window limit.
   - If the estimated size exceeds the context window budget → trigger automatic compaction (invoke the compaction engine, see below) and re-estimate.
   - If after compaction the estimate still exceeds the budget → surface a `context_window_exceeded` error to the user.

5. **Send to provider** — Submit the request via the provider API client (Phase 1).
   - Streaming mode: process the SSE event stream using the six event types (SessionStart through SessionEnd) and accumulate the `MessageResponse`.
   - Non-streaming mode: receive the complete `MessageResponse` as a single JSON payload.

6. **Process response** — Iterate over the `ContentBlocks` in the `MessageResponse`:
   - **TextContent** → Render the text to the user (ANSI-formatted Markdown in REPL mode; plain text in one-shot mode).
   - **ThinkingContent** → Optionally display (configurable). Store in the message history for conversation continuity.
   - **ToolInvocation** → For each tool call:
     a. Evaluate permissions via the permission engine (Phase 2, nine-step evaluation).
     b. If permitted, execute the tool via the tool system (Phase 2).
     c. If denied, create a tool result with the denial reason.
     d. Append the tool result to the message history as a `Tool`-role message.
   - **RedactedThinking** → Preserve opaquely in the message history. Do not attempt to decode or display.

7. **Evaluate stop reason** — Examine the `StopReason` from the `MessageResponse`:
   - `end_turn` → The model has finished its response. Proceed to step 8.
   - `tool_use` → The model wants to call tool(s). Append the assistant message (with tool invocations) and tool results to the history. Loop back to step 3 (re-assemble and re-send with tool results).
   - `max_tokens` → The model ran out of output budget. Signal to the user: "Response truncated due to output token limit." Proceed to step 8.

8. **Record usage** — Extract the `TokenUsage` from the `MessageResponse` and call `record()` on the cumulative usage tracker (Phase 1). This updates the latest turn usage, cumulative totals, and turn count.

9. **Check compaction threshold** — After recording usage, check if the session exceeds the configured token budget for compaction. If so, trigger compaction before the next turn (but do not re-send the current turn).

10. **Persist session** — Write the updated conversation state to the JSONL session file (see Session Persistence below).

### Session Persistence

**Spec references:** §5.1, §5.7

#### JSONL Serialization

Sessions are stored as JSONL files in `<workspace>/.claw/sessions/`, named by session identifier (UUID).

Each line in the JSONL file represents one conversation message serialized as a JSON object. The schema is defined in the Data Contracts section below.

- **Write**: after each turn (step 10), the entire message history is written to the session file (overwrite, not append). This ensures atomic session state — a crash between turns does not produce a partially-written session.
- **Read**: on session resume, the JSONL file is read line-by-line. Each line is parsed as a JSON message record. Malformed lines produce a deserialization error with the file path and line number.

#### Session Resume

1. Accept a session identifier or the keyword `latest`.
2. If `latest`, scan the sessions directory for the most recently modified `.jsonl` file.
3. Load the JSONL file and reconstruct the message history.
4. Reconstruct the usage tracker from the loaded messages (Phase 1: `reconstruct_from_messages()`).
5. Verify the session's liveness flag. If the session is marked as live (another process is using it), warn the user but allow resume (the heartbeat will be overwritten).
6. Set the liveness flag and update the heartbeat.

#### Session Fork

1. Copy the current message history to a new session with a freshly generated UUID.
2. The new session starts with the full conversation history but a separate identifier.
3. The forked session's usage tracker is reconstructed from its messages (not inherited from the parent tracker).
4. If the current session has no messages, reject the fork with error: `"cannot fork an empty session"`.

#### Heartbeat and Liveness

- **Heartbeat**: a timestamp field in the session metadata, updated after each turn (step 10). Stored as an ISO 8601 timestamp.
- **Liveness**: a boolean flag set to `true` when the REPL starts and cleared to `false` on REPL exit (including signal handlers for SIGINT/SIGTERM). Used to detect stale sessions.
- **Worker state**: after the first turn, write `worker-state.json` to `<workspace>/.claw/` containing:
  - `WorkerIdentifier` — a unique string (UUID) for this process instance.
  - `SessionReference` — the session identifier.
  - `Model` — the currently active model name.
  - `PermissionMode` — the active permission mode.

#### Session Error Handling

**Spec references:** §4.3

| Scenario | Error Output |
|----------|-------------|
| Resume requested but session file not found | `"no session file found at <path>"` |
| Session file is corrupted JSON | Deserialization error with file path and line number |
| Worker state file not found | Structured error with hint: `"Run the REPL or a one-shot prompt first to produce the worker state file"` |
| Session fork with no messages | `"cannot fork an empty session"` |

### Compaction Engine

**Spec references:** §3.7, §4.6

The compaction engine reduces the conversation transcript when it exceeds the configured token budget.

1. **Estimate tokens** — For each message, sum the estimated token footprint of each content block using: `⌊character_count / 4⌋ + 1`. Sum across all messages.

2. **Check threshold** — Compaction triggers when BOTH conditions are met:
   - The number of compactable messages (excluding any existing compacted summary prefix) exceeds the preservation count (configurable, default: 10).
   - The estimated total tokens exceed the maximum budget (configurable, default: model's context window × 0.8).

3. **Determine compaction boundary** — Preserve the most recent N messages. Then adjust:
   - If the first preserved message is a `Tool`-role message (tool result), walk the boundary backward to include the paired `Assistant`-role message that contains the corresponding `ToolInvocation`. This ensures tool-use / tool-result pairs are never split across the compaction boundary.

4. **Summarize removed messages** — Generate a structured summary containing:
   - **Scope**: message counts by role (User, Assistant, Tool, System).
   - **Unique tool names**: deduplicated set of all tool names mentioned in the removed messages.
   - **Recent user requests**: up to 3 of the most recent user messages, each truncated to 160 characters with ellipsis (`…`) if longer.
   - **Inferred pending work items**: messages whose text content contains any of: `"todo"`, `"next"`, `"pending"`, `"follow up"`, `"remaining"` (case-insensitive). Each item truncated to 160 characters.
   - **Key files referenced**: file paths extracted from path-like tokens with recognized extensions (`.rs`, `.py`, `.ts`, `.js`, `.json`, `.toml`, `.yaml`, `.yml`, `.md`, `.txt`, `.html`, `.css`). Deduplicated.
   - **Current work inference**: the last non-empty text content block from the removed messages.
   - **Key timeline**: chronological per-message summaries (role, first 80 characters of text content).

5. **Merge with prior summary** — If the first message in the session is already a compacted summary (from a previous compaction cycle):
   - Extract the prior summary's highlights.
   - Flatten them (do not nest summaries within summaries).
   - Append the new summary's highlights after the prior highlights.
   - This prevents summary inflation across multiple compaction cycles.

6. **Build continuation message** — Create a synthetic `System`-role message containing:
   - A preamble: `"This is a continuation of a previous conversation. Here is a summary of what was discussed:"`
   - The formatted summary (Markdown).
   - A note: `"The following <N> messages are preserved from the recent conversation."`
   - An instruction: `"Please continue from where we left off without recapping what was already discussed."` (if configured).

7. **Replace message history** — The compacted session's message list becomes: `[continuation_message, ...preserved_messages]`.

8. **Record compaction event** — Store in session metadata: `{ summary_text, removed_count, timestamp }`.

Compaction edge cases (§4.6):
- Session has fewer messages than preservation count → no compaction (session returned unchanged).
- First preserved message is a tool-result → boundary walked backward.
- Preservation count = 0 → maximum compaction (all messages summarized, none preserved).
- Existing compaction summary at session start → prior highlights flattened, not re-nested.
- Long content blocks in summary → truncated to 160 characters with `…`.

### Provider Error Handling

**Spec references:** §4.5

| Scenario | Behavior |
|----------|----------|
| Request body exceeds provider size limit | Preflight check (step 4) triggers compaction; retry after compaction |
| Context window exceeded | Preflight check (step 4) triggers compaction; if still exceeded after compaction, surface error |
| Rate limiting (429) | Surface error with retry guidance (provider's `Retry-After` header value if present) |
| Provider unreachable (network error) | Surface connection error with timeout details |
| SSE stream interrupted | Preserve partial `MessageResponse` (whatever was accumulated); append error to user output |
| Structured output serialization fails | Retry up to configured limit (default: 2). On final failure, use simplified fallback payload |

> [RESOLVED: A7] The structured output retry fallback payload is explicitly specified. The simplified fallback payload contains only the minimal mandatory communication fields: `session_id` (String), `prompt` (String), `output_text` (String), and `fallback_mode: true` (Boolean). All optional tracking arrays (matched commands, matched tools, denial records) are omitted.

Structured output retry sequence:
1. Attempt to serialize the full output as JSON.
2. If serialization fails (e.g., non-UTF-8 content, circular reference), retry up to the configured limit.
3. On each retry, log the serialization error.
4. If all retries fail, construct the fallback payload:
   ```json
   {
     "session_id": "<session_uuid>",
     "prompt": "<original_prompt>",
     "output_text": "<best_effort_text>",
     "fallback_mode": true
   }
   ```
5. If fallback payload construction also fails, raise a runtime error.

## Data contracts

This section defines the canonical field schemas for all outputs that cross a phase boundary. These schemas constitute the **session state schema freeze** — Phase 5's Python reader must consume these schemas as stable, read-only inputs.

> [RESOLVED: A12] The session state schema (§5.1–5.2) is explicitly specified as a data contract for Phase 5's Python loader. See the JSONL Message Record and Worker State schemas below.

### JSONL Message Record

Each line in the session JSONL file is a JSON object with the following schema:

| Field | Type | Required | Mutability | Description |
|-------|------|----------|------------|-------------|
| `role` | String enum: `"system"`, `"user"`, `"assistant"`, `"tool"` | Yes | Immutable after write | The message role |
| `content` | Array of ContentBlock objects | Yes | Immutable after write | Ordered content blocks |
| `usage` | UsageRecord object or `null` | No (present for `assistant` role) | Immutable after write | Token usage for this turn |
| `timestamp` | String (ISO 8601) | Yes | Immutable after write | When the message was created |
| `tool_call_id` | String or `null` | No (present for `tool` role) | Immutable after write | Links tool result to tool invocation |

**ContentBlock variants** (discriminated by `type` field):

| Type | Fields | Description |
|------|--------|-------------|
| `"text"` | `text: String` | Text content |
| `"tool_use"` | `id: String, name: String, input: JSON` | Tool invocation |
| `"tool_result"` | `tool_use_id: String, content: String, is_error: Boolean` | Tool execution result |
| `"thinking"` | `thinking: String, signature: String or null` | Model thinking content |
| `"redacted_thinking"` | `data: String` | Opaque redacted thinking blob |

**UsageRecord**:

| Field | Type | Description |
|-------|------|-------------|
| `input_tokens` | Integer (u32) | Input tokens consumed |
| `output_tokens` | Integer (u32) | Output tokens generated |
| `cache_creation_tokens` | Integer (u32) | Tokens used to create cache entries |
| `cache_read_tokens` | Integer (u32) | Tokens read from cache |

### Session Metadata (JSONL Header)

The first line of the JSONL file is a metadata record (distinguished by `type: "session_meta"`):

| Field | Type | Required | Mutability | Description |
|-------|------|----------|------------|-------------|
| `type` | String: `"session_meta"` | Yes | Immutable | Record type discriminator |
| `session_id` | String (UUID) | Yes | Immutable | Unique session identifier |
| `created_at` | String (ISO 8601) | Yes | Immutable | Session creation timestamp |
| `model` | String | Yes | Mutable (on model change) | Active model name |
| `permission_mode` | String enum | Yes | Immutable | Active permission mode |
| `heartbeat` | String (ISO 8601) | Yes | Mutable (each turn) | Last activity timestamp |
| `liveness` | Boolean | Yes | Mutable | Whether session is active |
| `compaction_history` | Array of CompactionRecord | Yes | Append-only | History of compaction events |

**CompactionRecord**:

| Field | Type | Description |
|-------|------|-------------|
| `summary_text` | String | The compaction summary |
| `removed_count` | Integer | Number of messages removed |
| `timestamp` | String (ISO 8601) | When compaction occurred |

### Worker State (worker-state.json)

| Field | Type | Required | Mutability | Description |
|-------|------|----------|------------|-------------|
| `worker_id` | String (UUID) | Yes | Immutable | Process instance identifier |
| `session_id` | String (UUID) | Yes | Immutable | Reference to the active session |
| `model` | String | Yes | Mutable | Currently active model name |
| `permission_mode` | String enum | Yes | Immutable | Active permission mode |

### Structured Output Fallback Payload

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `session_id` | String | Yes | Session identifier |
| `prompt` | String | Yes | Original user prompt |
| `output_text` | String | Yes | Best-effort output text |
| `fallback_mode` | Boolean (always `true`) | Yes | Indicates fallback was used |

## Acceptance criteria

1. The 10-step turn loop executes in the correct order for a model prompt: receive input → route (no slash command) → assemble request → preflight → send → process response → evaluate stop reason → record usage → check compaction → persist.
2. Slash commands beginning with `/` are dispatched to command handlers at step 2 without invoking the model (steps 3–7 are skipped). Unrecognized `/` prefixes are treated as model prompts.
3. The conversation loop correctly handles the `tool_use` stop reason by looping back to step 3 with tool results appended to the history.
4. Session JSONL serialization writes all message fields per the data contract schema. Deserialization reconstructs the message history with all content block variants.
5. Session resume reconstructs the usage tracker by replaying usage-bearing messages, sets the liveness flag, and updates the heartbeat.
6. Session fork creates a new session with a fresh UUID and full message history copy, and rejects forking an empty session.
7. The compaction engine correctly summarizes removed messages (scope, tool names, recent requests, pending items, key files, current work, timeline), preserves tool-use/tool-result pairs at the boundary, and flattens prior summaries without nesting.
8. The preflight check estimates token count using `⌊char_count / 4⌋ + 1` and triggers compaction when the estimate exceeds the context window budget.
9. Provider error handling preserves partial responses on SSE interruption and applies the structured output retry sequence with the correct fallback payload (session_id, prompt, output_text, fallback_mode).
10. The worker-state.json file is written after the first turn with the correct schema (worker_id, session_id, model, permission_mode) and is readable by Phase 5's Python loader.
11. ThinkingContent blocks are stored in the message history, and RedactedThinking blocks are preserved opaquely without decoding.

## Git commit plan

1. **`feat: implement 10-step conversation turn loop`**
   Build the conversation runtime with all 10 steps including slash-command routing as step 2. Resolves audit finding A6.
   *Satisfies acceptance criteria: 1, 2, 3*

2. **`feat: add session JSONL serialization`**
   Implement JSONL write (full overwrite per turn) and read (line-by-line parsing) with all message fields per the data contract schema.
   *Satisfies acceptance criteria: 4*

3. **`feat: implement session resume and fork`**
   Add session resume with usage tracker reconstruction, liveness flag, heartbeat update. Add session fork with UUID generation and empty-session rejection.
   *Satisfies acceptance criteria: 5, 6*

4. **`feat: add compaction engine`**
   Implement token estimation, threshold check, boundary adjustment for tool-use/result pairs, structured summary generation, prior summary merging, continuation message construction.
   *Satisfies acceptance criteria: 7, 8*

5. **`feat: add provider error handling and retry`**
   Implement preflight size check, compaction-triggered retry, rate limiting surface, SSE interruption partial preservation, structured output retry with fallback payload. Resolves audit finding A7.
   *Satisfies acceptance criteria: 8, 9*

6. **`feat: freeze session state schema (data contracts)`**
   Define canonical JSONL message record, session metadata, worker-state.json, and fallback payload schemas as the cross-phase data contract.
   *Satisfies acceptance criteria: 4, 10*

7. **`feat: add worker state and heartbeat management`**
   Implement worker-state.json write after first turn, heartbeat timestamp updates, and liveness flag lifecycle (set on REPL start, clear on exit/signal).
   *Satisfies acceptance criteria: 10*

8. **`feat: handle ThinkingContent and RedactedThinking`**
   Implement optional display of ThinkingContent blocks and opaque preservation of RedactedThinking blocks in the message history.
   *Satisfies acceptance criteria: 11*

9. **`test: add turn loop and compaction tests`**
   Test the 10-step loop with tool_use loopback, slash-command bypass, compaction boundary adjustment, and prior summary flattening.
   *Satisfies acceptance criteria: 1, 2, 3, 7*

10. **`test: add session persistence and error tests`**
    Test JSONL round-trip, session resume with usage reconstruction, fork rejection, corrupted JSONL handling, SSE interruption recovery, and fallback payload generation. Resolves audit finding A5 placement verification.
    *Satisfies acceptance criteria: 4, 5, 6, 9, 10*

## Open questions

1. **JSONL atomicity**: The spec implies full overwrite per turn. On crash during write, the session file could be corrupted. The implementer should consider writing to a temp file and atomically renaming it.
2. **Compaction preservation count default**: The spec does not specify a default preservation count. The implementer should use 10 as a reasonable default (preserving the last 10 messages).
3. **Slash command registry**: The spec references slash commands but does not enumerate them. Phase 4 defines the CLI subcommands; the implementer should treat slash commands as an extension point with a registry interface.
4. **System prompt assembly**: The spec references instruction files (§2.4.3) but the detailed loading logic is specified in Phase 4. The implementer should define an interface for system prompt assembly that Phase 4 can implement.
5. **Compaction summary language**: The spec does not specify whether the compaction summary should be in English or match the conversation language. The implementer should use English for summary templates and include original text snippets verbatim.
