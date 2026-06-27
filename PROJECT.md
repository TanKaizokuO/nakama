# Nakama

## Overview
**Vision/Goal:** Nakama is a high-performance, dual-architecture AI coding assistant and agent infrastructure that orchestrates autonomous AI agents by using a blistering-fast Rust primary engine for real-time API streaming and sandboxed tool execution, alongside a Python companion for state mirroring, dynamic prompt routing, and parity auditing.

**Current Status:** Active Development (Version 0.1.0)

## Tech Stack
**Language/Runtime:** Rust (Edition 2024), Python 3.10+

**Frameworks/Libraries:** tokio (async), reqwest, serde, clap, rustyline (Rust); Standard library (Python)

**Key Dependencies:** OpenAI-Compatible APIs, NVIDIA NIM APIs, Anthropic APIs, Model Context Protocol (MCP)

## Directory Structure
```text
.
├── Cargo.toml              # Rust package and dependency manifest
├── python_companion/       # Python workspace for routing and parity auditing
│   ├── audit.py            # Parity auditing & coverage checks
│   ├── inventory.py        # Command & tool inventories
│   ├── main.py             # CLI entry point for companion
│   ├── query.py            # Query simulation engine
│   ├── routing.py          # Token-based prompt routing
│   ├── session.py          # Session persistence logic
│   └── transcript.py       # Mutable transcript store
└── src/                    # Core Rust runtime
    ├── bootstrap.rs        # Multi-phase startup pipeline
    ├── cli.rs              # Argument parsing & CLI dispatch
    ├── compaction.rs       # Context window summarization engine
    ├── config/             # Hierarchical configuration logic
    ├── data_contracts.rs   # Core structures and type boundaries
    ├── mcp.rs              # Model Context Protocol bridge
    ├── path_scope.rs       # Directory traversal prevention
    ├── permission.rs       # Tool execution rules and interactive gates
    ├── provider/           # HTTP client abstractions for LLM providers
    ├── repl.rs             # Interactive Read-Eval-Print Loop logic
    ├── runtime.rs          # Multi-turn conversation loop
    ├── session.rs          # State serialization (.jsonl)
    ├── sse.rs              # Server-Sent Events stream parser
    ├── tools/              # Built-in tool implementations
    └── worker_state.rs     # Worker state persistence
```

## Core Logic & Data Flow
1. **Interactive Loop & Streaming (Rust Core):** Data enters through the CLI or interactive REPL (`repl.rs`, `cli.rs`). The `runtime.rs` orchestrates the multi-turn conversational loop, routing requests to the appropriate LLM provider and streaming back responses in real-time via `sse.rs` (Server-Sent Events).
2. **Tool Dispatch & Sandboxing:** When the LLM initiates a tool call (e.g., shell commands, filesystem I/O), it is intercepted by `mcp.rs` and built-in dispatchers (`tools/`). Before any code operation occurs, actions are validated against strict path boundaries (`path_scope.rs`) and permission levels (`permission.rs` - e.g., 'prompt' vs 'auto' modes) to prevent directory traversal and unauthorized escapes.
3. **Session Persistence & Auditing (Dual-Architecture):** The Rust core continuously serializes conversational state into disk artifacts (`session.rs`, `worker_state.rs`). The Python companion workspace (`python_companion/`) acts as a secondary layer that reads this state for parity auditing (`audit.py`), prompt routing (`routing.py`), and query simulation without blocking the fast primary engine.

## Environment & Setup
**Prerequisites:** 
- Rust & Cargo (Edition 2024)
- Python 3.10+
- API Keys for preferred AI providers (e.g., `NVIDIA_API_KEY`, `ANTHROPIC_API_KEY`)

**Environment Variables:**
Create a `.env` file in the root directory:
```env
NVIDIA_API_KEY=your_api_key_here
URL=https://integrate.api.nvidia.com/v1
NAKAMA_PERMISSION_MODE=prompt
```

**Essential Start Commands:**
- Build the Rust Core: `cargo build --release`
- Run the Application (REPL): `cargo run`
- Setup Python Companion: `cd python_companion && python main.py setupreport`

## Development Conventions
- **Dual-Architecture Separation:** High-performance, strict, and state-heavy logic resides in Rust (async via `tokio`); flexible prompt routing, simulation, and auditing are handled in Python.
- **Strict Boundary Control:** AI actions never execute implicitly. All tool calls and filesystem operations must pass through explicit `path_scope` validations and interactive permission gates (Prompt mode).
- **Data Contracts:** Communication between the Rust core and Python companion relies on frozen, serialized JSON artifacts (e.g., `.jsonl` session transcripts) rather than direct in-memory FFI, ensuring decoupled stability.
- **Error Handling:** Centralized through `thiserror` (Rust) with strongly typed error variants across the system (`error.rs`, `error_handling.rs`).

## Known Issues / Debt
- **Token Limits vs. Compaction Fidelity:** The intelligent transcript compaction system (`compaction.rs`) automatically summarizes lengthy conversations to stay within model context limits, which may result in a loss of granular detail in extremely long, multi-turn sessions.
- **Dual-State Synchronization:** Because the architecture splits responsibilities between Rust and Python through disk-based JSON artifacts, subtle race conditions or parsing desyncs can occur if the Python companion reads state while the Rust engine is mid-write.
- **API Interoperability Churn:** Standardizing OpenAI-compatible chat completions and tool-calling payloads across disparate providers requires continuous adaptation of the `sse.rs` and `provider/` modules when upstream APIs change or diverge from standard SSE formats.
