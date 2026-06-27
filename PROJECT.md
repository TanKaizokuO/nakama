# Nakama

## Overview
**Vision/Goal:** Nakama is a high-performance, dual-architecture AI coding assistant and agent infrastructure designed to bridge the gap between fast systems programming and flexible scripting, providing a secure, strict-sandboxed foundation for agentic workflows.

**Current Status:** Active Development (Version 0.1.0)

## Tech Stack
**Language/Runtime:** Rust (Edition 2024), Python 3.10+

**Frameworks/Libraries:** 
- **Rust:** `tokio` (async runtime), `reqwest` (HTTP/SSE streams), `clap` (CLI), `serde`/`serde_json` (serialization), `rustyline` (REPL)
- **Python:** Standard library (`python_companion`)

**Key Dependencies:** 
- Model Context Protocol (MCP) for tool bridging.
- Standardized OpenAI-compatible, Anthropic, DashScope, and xAI streaming API endpoints.

## Directory Structure
```text
nakama/
├── Cargo.toml
├── README.md
├── python_companion/          # Porting scaffold & audit workspace
│   ├── audit.py               # Parity auditing & coverage checks
│   ├── inventory.py           # Command & tool inventories
│   ├── main.py                # Python companion CLI entry
│   ├── query.py               # Query simulation engine
│   ├── routing.py             # Token-based prompt routing
│   ├── session.py             # Session persistence logic
│   └── transcript.py          # Mutable transcript store
└── src/                       # Primary Rust Runtime
    ├── bootstrap.rs           # Multi-phase startup pipeline
    ├── cli.rs / subcommands.rs# Argument parsing & CLI dispatch
    ├── compaction.rs          # Context window summarization engine
    ├── config/                # Hierarchical configuration loading
    ├── hook.rs                # Lifecycle events (Pre/Post tool use)
    ├── mcp.rs                 # External tool bridging
    ├── models.rs              # Model alias resolution
    ├── path_scope.rs          # Security & directory traversal prevention
    ├── permission.rs          # Hierarchical tool execution rules
    ├── provider/              # HTTP client abstractions for LLM providers
    ├── runtime.rs             # Multi-turn conversation loop
    ├── session.rs             # JSONL state serialization
    └── tools/                 # Built-in tool implementations
```

## Core Logic & Data Flow
1. **Conversation Runtime & API Streaming:** 
   User input is captured via a REPL or CLI argument. The Rust core routes the prompt, assembles a system instruction, performs context-window preflight checks (triggering compaction if necessary), and sends the request to the LLM provider. Responses are parsed via Server-Sent Events (SSE), extracting text blocks, tool invocations, and usage metrics.
2. **Strict Permission & Tool Dispatch Engine:** 
   When an LLM attempts to call a tool, the payload is intercepted by the Permission Engine. It extracts subject paths, expands globs/variables, and resolves them against canonical workspace roots to prevent directory escapes. The engine evaluates hierarchical rules (`DangerFullAccess`, `WorkspaceWrite`, `ReadOnly`) against the tool's required mode, falling back to an interactive prompter (`Prompt` mode) if escalation is required.
3. **Companion Parity & Auditing (Python):** 
   The Python companion acts as a decoupled porting scaffold. It reads the `.claw/sessions/` JSONL files and `worker-state.json` emitted by the Rust core, scores/routes prompts, simulates turn loops with budget constraints, and performs cross-language parity audits to verify feature implementation alignment against an archived reference workspace.

## Environment & Setup
**Prerequisites:** 
- Rust (Edition 2024)
- Python 3.10+
- Applicable LLM API keys.

**Environment Variables (`.env`):**
- `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `NVIDIA_API_KEY`, `XAI_API_KEY`, or `DASHSCOPE_API_KEY` (depending on selected provider)
- `URL` (Custom base URL, e.g., for NVIDIA NIM or local Ollama)
- `NAKAMA_PERMISSION_MODE` (e.g., `prompt`, `allow`, `readonly`)
- `CLAW_MODEL` (Overrides default model selection)

**Start Commands:**
```bash
cargo build --release
cargo run
```

## Development Conventions
- **Dual-Layer Architecture:** The Rust layer exclusively handles performance-critical I/O, strict state management, and permissions. The Python layer handles isolated prompt routing, auditing, and fallback query logic.
- **Hierarchical Configuration:** Settings load sequentially from `~/.claw.json` → `~/.config/claw/settings.json` → `<repo>/.claw.json` → `<repo>/.claw/settings.local.json`.
- **Stateless Tool Invocation:** Built-in tools and MCP servers are isolated. The system relies heavily on explicit lifecycle hooks (`PreToolUse`, `PostToolUse`) rather than shared memory.

## Known Issues / Debt
- **JSON Key Brittleness in Permissions:** The permission rule engine extracts subjects by looking for hardcoded JSON keys (e.g., `command`, `file_path`, `url`). If an LLM hallucinates an unmapped parameter name, the rule may fail to evaluate correctly, requiring a fallback to generic string-matching.
- **State Decoupling Latency:** Because the Python companion relies on parsing serialized JSONL session files rather than IPC or shared memory, rapid state mutations could theoretically cause race conditions if the companion reads before the Rust core fully flushes a write.
- **Compaction Summarization Inflation:** The compaction engine summarizes discarded messages. If repeatedly triggered, older summaries might become overly dense or lose nuance, despite logic designed to flatten previous highlights.
