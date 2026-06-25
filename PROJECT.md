# Nakama
## Overview
**Vision/Goal:** Nakama is a conversational AI agent runtime that combines a high-performance Rust core for handling state, context compaction, and SSE streaming, alongside a Python companion workspace for prompt routing, auditing, and subsystem management.

**Current Status:** Active Development

## Tech Stack
**Language/Runtime:** Rust (Edition 2024), Python 3.12

**Frameworks/Libraries:** 
- **Rust:** Tokio (Async Runtime), Reqwest (HTTP), Serde (Serialization), eventsource-stream, shlex.
- **Python:** Standard Library (`argparse`, `dataclasses`, `json`).

**Key Dependencies:** Tokio, Reqwest, Serde (Rust) for managing concurrent asynchronous streaming of provider responses.

## Directory Structure
```text
.
├── Cargo.toml
├── src/
│   ├── main.rs              (Core initialization)
│   ├── runtime.rs           (ConversationRuntime loop & mock API flow)
│   ├── sse.rs               (Server-Sent Events payload accumulator)
│   ├── session.rs           (State persistence & message history)
│   ├── compaction.rs        (Context window management)
│   ├── slash_commands.rs    (Command dispatch logic)
│   ├── worker_state.rs      (Process tracking)
│   └── (cli.rs, repl.rs, subcommands.rs, etc.)
└── python_companion/
    ├── main.py              (CLI entrypoint & IPC router)
    ├── routing.py           (Prompt routing logic)
    ├── audit.py             (Parity & state auditor)
    ├── session.py           (Python-side session loader)
    ├── query.py             (Auxiliary querying)
    └── inventory.py         (Workspace inventory loader)
```

## Core Logic & Data Flow
1. **Conversation Runtime Loop (`runtime.rs`):** The primary loop receives user input, checks for slash commands, appends to the session message history, and performs context window checks via `CompactionEngine`. If the context exceeds limits, messages are compacted. Currently, the provider request mechanism is mocked.
2. **Server-Sent Events (SSE) Streaming (`sse.rs`):** Manages the streaming response from language model providers. It maintains an `AccumulatorState` state machine, merging fragmented `DeltaPayload` messages (Text, Json, Thinking, Signature) and yielding consolidated output blocks once complete.
3. **Python Companion Inter-process Communication:** The Rust binary acts as the central orchestrator and relies on `python_companion/main.py` to handle specialized workspace tasks like parsing project inventory (`inventory.py`), executing audits (`parityaudit`), routing prompts (`routeprompt`), and verifying configurations via JSON serialization.

## Environment & Setup
**Prerequisites:**
- Rust toolchain (Edition 2024 support)
- Python 3.12+

**Setup / Run Commands:**
- Build Rust core: `cargo build`
- Run Rust CLI: `cargo run`
- Execute Python utility: `python3 python_companion/main.py <command>`

## Development Conventions
- **Rust Patterns:** Uses `tokio` for async operations and `serde` heavily for strong type contracts between the Rust models and JSON payloads.
- **State Management:** Session history and worker states are persisted locally. The `sse.rs` module uses a robust state machine (`AccumulatorState`) to parse arbitrary chunks into structured API responses safely.
- **Python IPC:** The Python CLI utilizes `argparse` extensively, returning purely `json.dumps()` output to standard out so the Rust binary can reliably parse execution results.

## Known Issues / Debt
- **Mocked Provider:** The API interaction in `runtime.rs` is currently mocked, short-circuiting real network calls to inference endpoints.
- **Unimplemented Remote Modes:** Network connection modes in the Python companion (`remotemode`, `sshmode`, `teleportmode`, etc.) return mock "not implemented" statuses.
- **Context Compaction:** The context token estimations and constraints are relatively rigid and might require optimization for specific model tokenizers in production.
