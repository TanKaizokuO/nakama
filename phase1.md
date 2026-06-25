# Phase 1 — Core Infrastructure

## Overview

Phase 1 delivers the foundational primitives upon which every subsequent phase depends: the multi-provider API client (supporting Anthropic, OpenAI-compatible, xAI, and DashScope endpoints with SSE streaming), the canonical request/response data structures, all six SSE stream event types, hierarchical configuration loading with five-level file precedence and model alias resolution, workspace path scope validation with full POSIX shell tokenization, and per-turn/cumulative usage tracking with prompt-cache-aware cost estimation. This phase has no upstream dependencies. It unlocks Phase 2 (tool system and permission engine, which needs the provider client and config), Phase 3 (session and conversation runtime, which needs usage tracking and the API layer), and Phase 4 (CLI bootstrap, which needs config loading and provider routing).

## Depends on

- None. This is the foundation phase.

## Unlocks

- **Phase 2** — Tool System & Permission Engine: requires the provider API client for tool-call round-trips, configuration loading for permission rules, hook definitions, and MCP server declarations.
- **Phase 3** — Session & Conversation Runtime: requires the message data structures, SSE streaming, usage tracker, and configuration for compaction thresholds and model parameters.
- **Phase 4** — CLI, Bootstrap & Plugin System: requires configuration loading, model alias resolution, and provider routing for subcommand dispatch and output formatting.
- **Phase 5** — Python Companion Workspace: requires the usage tracker schema and cost estimation logic as reference implementations.

## Scope

### Provider API Client

**Spec references:** §2.2, §2.2.4, §2.2.5

The provider API client is an HTTP abstraction layer that dispatches requests to one of four provider backends based on model name analysis.

- Implement an HTTP client capable of issuing POST requests with JSON payloads and receiving both complete JSON responses and SSE event streams.
- Support four provider backends:
  - **Anthropic-native** — Endpoint: `https://api.anthropic.com/v1/messages`. Authentication via `x-api-key` header using `ANTHROPIC_API_KEY`, or `Authorization: Bearer` using `ANTHROPIC_AUTH_TOKEN`.
  - **OpenAI-compatible** — Endpoint: configurable base URL (default: `https://api.openai.com/v1/chat/completions`). Authentication via `Authorization: Bearer` using `OPENAI_API_KEY`. Also used for local servers (Ollama) when `OLLAMA_HOST` is set (no auth header).
  - **xAI** — Endpoint: `https://api.x.ai/v1/chat/completions`. Authentication via `Authorization: Bearer` using `XAI_API_KEY`.
  - **DashScope** — Endpoint: `https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions`. Authentication via `Authorization: Bearer` using `DASHSCOPE_API_KEY`.
- Implement the provider routing cascade (§2.2.4):
  1. Model name contains `claude` → Anthropic.
  2. Model name contains `grok` → xAI.
  3. Model name starts with `openai/`, `local/`, or `gpt-` → OpenAI-compatible.
  4. Model name starts with `qwen/`, `qwen-`, `kimi/`, or `kimi-` → DashScope.
  5. Local-server base URL set and model name looks local → OpenAI-compatible.
  6. Fallback: check which credential env var is populated (Anthropic → OpenAI → xAI).
  7. Final default → Anthropic.
- Implement authentication credential resolution for each provider per §2.2.5.
- Handle the `sk-ant-*` key format detection for Anthropic API key vs. OAuth token routing.
- Support request-level configuration: timeout overrides from `ProviderSettings` in config.

### MessageRequest / MessageResponse / OutputContentBlock Data Structures

**Spec references:** §2.2.1, §2.2.2

- Define the `MessageRequest` structure with all fields from §2.2.1:
  - `ModelIdentifier` (String, required, non-empty)
  - `MaxOutputTokens` (u32, must be > 0)
  - `MessageHistory` (ordered list of `InputMessage` records, at least one)
  - `SystemInstruction` (optional String)
  - `ToolDefinitions` (optional list of `ToolDefinition` records with name, optional description, JSON-Schema input spec)
  - `ToolSelectionPolicy` (optional enum: `Auto`, `Any`, `SpecificTool(name)`)
  - `StreamingEnabled` (bool)
  - `Temperature` (optional float, range [0.0, 2.0], omitted for reasoning models)
  - `TopP` (optional float, range [0.0, 1.0], omitted for reasoning models)
  - `FrequencyPenalty` (optional float, provider-specific range)
  - `PresencePenalty` (optional float, provider-specific range)
  - `StopSequences` (optional list of Strings)
  - `ReasoningEffort` (optional enum: `low`, `medium`, `high` — reasoning models only)
  - `ProviderExtensions` (key-value map of arbitrary JSON; core protocol keys are protected from override)
- Define the `MessageResponse` structure with all fields from §2.2.2:
  - `ResponseId` (String)
  - `Role` (constant: `assistant`)
  - `ContentBlocks` (ordered list of `OutputContentBlock` variants)
  - `ModelUsed` (String)
  - `StopReason` (optional String: `end_turn`, `max_tokens`, `tool_use`, etc.)
  - `TokenUsage` (UsageRecord: `input_tokens`, `output_tokens`, `cache_creation_tokens`, `cache_read_tokens`)
- Define four `OutputContentBlock` variants:
  - `TextContent` — `text: String`
  - `ToolInvocation` — `id: String`, `name: String`, `input: JSON`
  - `ThinkingContent` — `thinking: String`, `signature: Option<String>`
  - `RedactedThinking` — opaque data blob

### SSE Stream Events

**Spec references:** §2.2.3

All six typed SSE stream events must be individually defined and handled:

1. **SessionStart** — Contains the initial `MessageResponse` skeleton (response ID, model, empty content blocks). The consumer uses this to allocate the response accumulator.
2. **ContentBlockBegin** — Carries the content block index and the initial block variant (e.g., empty `TextContent`, `ToolInvocation` with name and empty input). The consumer appends a new block to the accumulator.
3. **ContentBlockDelta** — Carries incremental fragments: text deltas for `TextContent`, JSON deltas for `ToolInvocation` input, thinking deltas for `ThinkingContent`, signature deltas for `ThinkingContent`. The consumer appends the fragment to the corresponding block.
4. **ContentBlockEnd** — Signals that the content block at the given index is complete. No further deltas will arrive for this index.
5. **MessageDelta** — Carries the final `stop_reason` and the final `TokenUsage` record. The consumer sets the stop reason and usage on the response.
6. **SessionEnd** — Signals that the full response is complete. No further events will arrive.

> [RESOLVED: A2] All six SSE stream events (SessionStart, ContentBlockBegin, ContentBlockDelta, ContentBlockEnd, MessageDelta, SessionEnd) are individually named and specified with their payloads and consumer responsibilities.

- Implement an SSE parser that reads `event:` and `data:` lines from the HTTP response stream.
- Map each SSE event type string to the corresponding typed enum variant.
- Accumulate a complete `MessageResponse` from the stream by applying each event in sequence.
- Handle malformed SSE lines gracefully (skip unknown event types, log warnings).
- Handle SSE stream interruption (connection drop mid-stream): preserve partial response and surface an error.

### Configuration Loading

**Spec references:** §2.4, §3.9

- Implement the five-level configuration file hierarchy (§2.4.1), loading files in precedence order (low → high):
  1. `~/.nakama.json` (user-level legacy)
  2. `~/.config/claw/settings.json` (user-level settings)
  3. `<repo>/.claw.json` (project-level legacy)
  4. `<repo>/.claw/settings.json` (project-level settings)
  5. `<repo>/.claw/settings.local.json` (project-level local, gitignored)
- Implement key-level merge logic: later files override earlier files for overlapping keys. Non-overlapping keys from all levels are preserved.
- Parse and validate all configuration fields (§2.4.2):
  - `ModelAliases` (Map: alias → model_name)
  - `McpServers` (Map: server_name → server_config)
  - `Hooks` (Map: event_name → hook_list)
  - `PermissionRules` (Object: allow[], deny[], ask[], denied_tools[])
  - `ProviderSettings` (Object: timeout overrides, fallback config)
  - `PluginConfig` (List of plugin objects)
  - `FeatureFlags` (Object: runtime toggles)
  - `RulesImport` (Enum: `auto` | `none` | list of framework names)
- Track precedence metadata for machine-readable output:
  - `precedence_rank` for each config file
  - `wins_for_keys` — which keys each file controls
  - `shadowed_keys` — which keys are overridden by higher-precedence files
- Implement model alias resolution (§3.9):
  - Built-in alias table: `opus` → `claude-opus-4-7`, `sonnet` → `claude-sonnet-4-6`, `haiku` → `claude-haiku-4-5-20251213`, `grok`/`grok-3` → `grok-3`, `grok-mini`/`grok-3-mini` → `grok-3-mini`, `kimi` → `kimi-k2.5`, `qwen-max` → `qwen-max`, `qwen-plus` → `qwen-plus`.
  - User-defined aliases from config files override built-in aliases.
  - Alias resolution is applied before provider routing.
- Implement model selection precedence: CLI flag → env var (`CLAW_MODEL` → `ANTHROPIC_MODEL` → `ANTHROPIC_DEFAULT_MODEL`) → config file → hardcoded default.

### Workspace Path Scope Validation

**Spec references:** §3.6, §4.7

- Implement path extraction from tool payloads using POSIX shell tokenization (shlex parsing).
  - If parsing fails due to unmatched quotes or malformed shell syntax, fall back to whitespace splitting with quote-stripping sanitization.
- Filter out tokens starting with `-` (flags) and environment variable assignments (`KEY=VALUE`).
- Strip shell redirection operators (`>`, `>>`, `<`, `<>`) and extract their target paths.
- Identify path-like tokens using the following heuristics:
  - Contains `/` or `\`
  - Starts with `./`, `../`, `/`, `~/`
  - Is `.` or `..`
  - Contains glob metacharacters (`*`, `?`, `[`)
  - Matches a Windows drive letter pattern (e.g., `C:\`)
- Expand environment variables and home directory shorthand (`~`).
- Resolve relative paths against the current working directory or the first workspace root.
- Expand glob patterns and validate each matched path individually.
  - For unmatched globs, validate the stable (non-glob) prefix.
- Check containment: the resolved path must be a descendant of at least one configured workspace root, using symlink-resolved canonical paths.
- Handle Windows paths: `C:\...` and `\\server\share` patterns validated against Windows-style workspace roots using platform path comparison.
- Return `allowed` with reason, or `denied` with candidate path and resolved path.

Edge cases from §4.7:
- Symlink resolving outside workspace → denied.
- Glob expanding to out-of-scope paths → each expansion checked individually.
- Unmatched glob → stable prefix validated.
- Windows drive path on POSIX root → denied unless matching Windows-style root configured.
- UNC path → treated as Windows absolute, validated against Windows-style roots.
- Environment variables in path → expanded before validation.
- Home directory shorthand → expanded before validation.
- Shell redirection in payload → target path extracted and validated.
- Malformed shell syntax → fallback to whitespace splitting.

### Usage Tracking & Cost Estimation

**Spec references:** §3.8, §3.8.2, §5.4

- Define the `TokenUsage` record with all four fields (§3.8.1):
  - `InputTokens` (u32)
  - `OutputTokens` (u32)
  - `CacheCreationTokens` (u32)
  - `CacheReadTokens` (u32)

> [RESOLVED: A1] All four token fields — InputTokens, OutputTokens, CacheCreationTokens, and CacheReadTokens — are included in the usage record and the pricing table. Cache write and cache read rates are specified per model family.

- Compute total tokens: `InputTokens + OutputTokens + CacheCreationTokens + CacheReadTokens`.
- Implement the model-specific pricing table (§3.8.2):
  - Haiku-class: Input $1.00/M, Output $5.00/M, Cache Write $1.25/M, Cache Read $0.10/M
  - Sonnet-class: Input $15.00/M, Output $75.00/M, Cache Write $18.75/M, Cache Read $1.50/M
  - Opus-class: Input $15.00/M, Output $75.00/M, Cache Write $18.75/M, Cache Read $1.50/M
  - Unknown (default): Input $15.00/M, Output $75.00/M, Cache Write $18.75/M, Cache Read $1.50/M
- Implement cost formula: `cost = (tokens / 1,000,000) × rate_per_million`.
- Format dollar amounts as `$<amount>` with 4 decimal places (e.g., `$0.0150`).
- Implement the cumulative usage tracker (§5.4):
  - `LatestTurnUsage` — replaced on each `record()` call.
  - `CumulativeUsage` — additively accumulated across all turns.
  - `TurnCount` — incremented on each `record()` call.

### Usage Tracker Reconstruction from JSONL Replay

**Spec references:** §3.8.3, §5.1

> [RESOLVED: A5] Usage tracker reconstruction from JSONL replay is placed in Phase 1 as a foundation primitive. The tracker can be rebuilt by iterating over all persisted messages that carry usage metadata, additively accumulating token counts and incrementing the turn counter for each usage-bearing message.

- Implement a `reconstruct_from_messages(messages)` function on the usage tracker.
- Iterate over all messages in the persisted session.
- For each message that carries a `TokenUsage` record (i.e., assistant messages with usage metadata):
  - Add each of the four token fields to the cumulative totals.
  - Increment the turn count.
  - Set the latest turn usage to the current message's usage.
- After iteration, the tracker state must be identical to the state that would have resulted from calling `record()` for each turn during the original session.
- Handle edge cases:
  - Empty message list → tracker remains at zero.
  - Messages without usage metadata → skipped (no contribution to totals).
  - Multiple messages with usage in a single turn → each contributes independently (the tracker does not deduplicate).

## Acceptance criteria

1. The provider API client can construct and send a valid HTTP request to each of the four providers (Anthropic, OpenAI-compatible, xAI, DashScope) with correct authentication headers and endpoint URLs.
2. The provider routing cascade correctly selects the provider for at least 10 model names covering all routing rules (e.g., `claude-sonnet-4-6`, `grok-3`, `openai/gpt-4o`, `qwen-max`, `local/llama3`, an unknown model defaulting to Anthropic).
3. The `MessageRequest` structure can represent all fields defined in §2.2.1, and the `MessageResponse` structure can deserialize a valid provider response including all four `OutputContentBlock` variants.
4. The SSE stream parser correctly processes a synthetic event stream containing all six event types (SessionStart, ContentBlockBegin, ContentBlockDelta, ContentBlockEnd, MessageDelta, SessionEnd) and produces a complete `MessageResponse`.
5. Configuration loading discovers, reads, and merges all five config file levels, with later files correctly overriding earlier files for overlapping keys while preserving non-overlapping keys.
6. Model alias resolution maps all eight built-in aliases to their correct model identifiers and correctly applies user-defined overrides from config files.
7. Workspace path scope validation correctly allows paths within the workspace root, denies paths outside the workspace root (including symlink escapes), handles glob expansion, environment variable expansion, home directory shorthand, shell redirections, and Windows paths per §3.6 and §4.7.
8. The usage tracker correctly accumulates token counts across multiple `record()` calls, computes cost using the model-specific pricing table (including cache write and cache read rates), and formats costs as `$<amount>` with 4 decimal places.
9. The usage tracker reconstruction function produces a tracker state identical to the original tracker state when given the same sequence of usage-bearing messages.
10. The POSIX shell tokenizer correctly parses quoted strings, handles escaped characters, and falls back to whitespace splitting when encountering malformed shell syntax.

## Git commit plan

1. **`feat: add MessageRequest/Response data structures`**
   Define all request/response types from §2.2.1–2.2.2 including the four OutputContentBlock variants.
   *Satisfies acceptance criteria: 3*

2. **`feat: implement SSE stream event types and parser`**
   Define all six typed SSE events (SessionStart through SessionEnd) and implement the stream-to-response accumulator. Resolves audit finding A2.
   *Satisfies acceptance criteria: 4*

3. **`feat: add provider routing cascade`**
   Implement the seven-step provider selection logic from §2.2.4 with model name pattern matching.
   *Satisfies acceptance criteria: 2*

4. **`feat: implement provider API client with auth`**
   Build the HTTP client abstraction for all four providers with credential resolution from environment variables per §2.2.5.
   *Satisfies acceptance criteria: 1*

5. **`feat: implement config file loading and merging`**
   Load, parse, and merge the five-level config hierarchy with key-level precedence tracking.
   *Satisfies acceptance criteria: 5*

6. **`feat: add model alias resolution`**
   Implement built-in alias table, user-defined overrides, and model selection precedence chain (CLI → env → config → default).
   *Satisfies acceptance criteria: 6*

7. **`feat: implement workspace path scope validation`**
   Build POSIX shell tokenizer, path extraction, glob expansion, symlink resolution, containment check, and all §4.7 edge cases.
   *Satisfies acceptance criteria: 7, 10*

8. **`feat: add usage tracking and cost estimation`**
   Implement TokenUsage record with all four fields, pricing table with cache rates, cost formula, dollar formatting, and cumulative tracker. Resolves audit findings A1 and A5.
   *Satisfies acceptance criteria: 8, 9*

9. **`test: add unit tests for provider routing and SSE`**
   Cover provider cascade selection, SSE event parsing, and stream accumulation with edge cases.
   *Satisfies acceptance criteria: 2, 4*

10. **`test: add tests for config merge, path scope, usage`**
    Cover config file precedence, alias resolution, path validation edge cases, and usage tracker reconstruction.
    *Satisfies acceptance criteria: 5, 6, 7, 8, 9, 10*

## Open questions

1. **Provider request body format divergence:** The spec defines a single `MessageRequest` structure, but Anthropic uses a different JSON schema (e.g., `messages` vs. `content`) than OpenAI-compatible providers (e.g., `messages` with `role`/`content` objects). The implementer must decide whether to define per-provider serializers or a single canonical format with per-provider adapters.
2. **SSE reconnection policy:** §4.5 states that SSE stream interruption should preserve partial responses, but does not specify whether automatic reconnection should be attempted or how many bytes of partial response constitute a usable result.
3. **Config file encoding:** The spec does not specify whether config files must be UTF-8 or whether other encodings are tolerated. The implementer should assume UTF-8 and surface an error for non-UTF-8 files.
4. **Glob expansion limits:** The spec does not cap the number of glob expansion results. A pathological glob (e.g., `/**/*`) could produce millions of results. The implementer should consider a configurable expansion limit.
5. **Windows path validation on POSIX hosts:** The spec requires Windows drive/UNC path handling (§3.6, §4.7), but it is unclear whether a POSIX-only host should ever accept Windows-style workspace roots. The implementer should treat Windows path validation as a cross-platform portability concern.
