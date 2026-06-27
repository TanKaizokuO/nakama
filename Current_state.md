# Nakama: Current Implementation State

## 1. Func_spec_Nakama.md Alignment
- `[x]` **CLI Entry Points & Commands**: Implemented (e.g., via `cli.rs`, `subcommands.rs`, `slash_commands.rs`).
- `[x]` **Provider API Interface & Streaming**: Fully implemented (NVIDIA NIM, OpenAI, Anthropic, DashScope, xAI).
- `[~]` **Conversation Runtime (including compaction)**: Partially implemented. The conversational loop is established, but compaction relies on a hardcoded formatting placeholder rather than true generative summaries.
- `[~]` **Permission Engine & Path Scope Validation**: Partially implemented. Basic path resolution is present, but missing a strict POSIX tokenizer and fully realized hierarchical permissions.
- `[x]` **Tool System & MCP Bridge**: Implemented (via `tools/` directory and `mcp.rs`).
- `[x]` **Session Persistence & Worker State**: Implemented (via `session.rs` and `worker_state.rs` producing `.jsonl` and `worker-state.json`).
- `[~]` **Configuration Hierarchy & Instruction Discovery**: Partially implemented. Foundation exists in `config/`, but parsing for `.claw/settings.json` is missing.

### Active Stage
**Stage 5: Anthropic Provider & Routing (COMPLETED)**
- [x] Wire `AccumulatorState` into live streaming path for Anthropic in `runtime.rs`
- [x] Unify response structs so `runtime.rs` receives standard format regardless of provider
- [x] Implement deterministic provider routing based on model keywords (`claude-` vs `nim-`)
- [x] Verify routing, fallback, and tool-call payload formats for both providers

### Next Up
**Stage 6: Multi-Turn Execution**
- Implement tool-loop mechanics.
- Add session file truncation handling.
- Verify multi-turn loops.

## 2. Source File Inventory

### `src/` Directory
* `bootstrap.rs`: Multi-phase startup pipeline implementation.
* `cli.rs`: Argument parsing & CLI dispatch definitions.
* `compaction.rs`: Context window summarization engine.
* `config/`: Hierarchical configuration loading logic.
* `data_contracts.rs`: Core structures and type definitions (frozen boundaries).
* `error.rs` / `error_handling.rs`: System-wide error types and handling.
* `hook.rs`: Lifecycle events (Pre/Post tool use).
* `instruction.rs`: System prompt assembly logic.
* `main.rs`: Application entry point.
* `mcp.rs`: External tool bridging (Model Context Protocol).
* `models.rs`: Model alias resolution.
* `nim_accumulator.rs`: Stream accumulator specific to NVIDIA NIM.
* `path_scope.rs`: Security & directory traversal prevention.
* `permission.rs`: Tool execution rules and hierarchy.
* `plugin.rs`: Plugin lifecycle management.
* `prompter.rs`: Interactive permission prompter.
* `provider/`: HTTP client abstractions for LLM providers.
* `repl.rs`: Interactive Read-Eval-Print Loop logic.
* `runtime.rs`: Multi-turn conversation loop.
* `session.rs`: JSONL state serialization.
* `slash_commands.rs`: REPL slash commands.
* `sse.rs`: Server-Sent Events stream parser.
* `subcommands.rs`: CLI subcommands dispatch.
* `tests.rs`: Test suite.
* `tools/`: Built-in tool implementations.
* `usage.rs`: Token usage tracking.
* `worker_state.rs`: Worker state persistence logic.

*Note: Expected architecture files like a standalone `security.rs` are missing based on general architectural patterns, as security logic is currently split between `path_scope.rs` and `permission.rs`. A top-level `config.rs` is structurally replaced by the `config/` directory.*

### `python_companion/` Directory
* `audit.py`: Parity auditing & coverage checks.
* `inventory.py`: Command & tool inventories.
* `main.py`: Python companion CLI entry point.
* `query.py`: Query simulation engine.
* `routing.py`: Token-based prompt routing.
* `session.py`: Session persistence logic.
* `transcript.py`: Mutable transcript store.

## 3. Provider & Environment

### Active vs. Stubbed Providers
* **Actively Functional**: NVIDIA NIM, OpenAI-compatible, DashScope, xAI.
* **Stubbed / Non-functional**: Anthropic client implementation.

### Required Environment Variables
* `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `NVIDIA_API_KEY`, `XAI_API_KEY`, or `DASHSCOPE_API_KEY` (depending on selected provider).
* `URL` (Custom base URL, e.g., for NVIDIA NIM or local instances).
* `NAKAMA_PERMISSION_MODE` (e.g., `prompt`, `allow`, `readonly`).
* `CLAW_MODEL` (Overrides default model selection).

## 4. Data Contracts (Frozen)
Core stabilized data structures that act as boundaries:

### State Immutability
* `ProviderConfig`: Holds `base_url`, `api_key`, `model`, and `auth_header`.
* `ContentBlock`: Enum defining `Text`, `ToolUse`, `ToolResult`, `Thinking`, and `RedactedThinking`.

### Data Boundaries with Python Companion
* `WorkerState`: Written to `worker-state.json` (contains `worker_id`, `session_id`, `model`, `permission_mode`).
* `SessionMessageRecord`: Structure used for serializing messages into the `.jsonl` session files (contains `role`, `content`, `usage`, `timestamp`, `tool_call_id`).

## 5. Known Debt & Inaccuracies
Specific gaps in the current implementation:
* **Anthropic Client Implementation**: The native Anthropic provider is stubbed out and non-functional.
* **Generative Compaction Summaries**: The compaction engine (`compaction.rs`) currently uses hardcoded string placeholders and basic counting instead of LLM-generated semantic summaries.
* **Security Constraints**: Missing a fully compliant POSIX tokenizer and complete hierarchical permissions.
* **Configuration Discovery**: Missing parsing for `.claw/settings.json`, preventing full hierarchical configuration override behavior.

## 6. Build & Run Reference

### Build the Project
```bash
cargo build --release
```

### Run the System
Interactive REPL:
```bash
cargo run
```

One-shot prompt:
```bash
cargo run -- prompt "Hello Nakama"
```

Resume an existing session (with a session UUID):
```bash
cargo run -- --resume-session <SESSION_UUID>
```

### Tests & Companion Audits
Run Rust tests:
```bash
cargo test
```

Execute Python Companion Parity Audit:
```bash
python3 python_companion/main.py parity-audit
```
