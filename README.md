# Nakama

Nakama is a dual-architecture AI coding assistant and agent infrastructure. It consists of a high-performance primary engine written in Rust, and a companion workspace written in Python that mirrors its state, routes prompts, and audits for parity.

## Architecture

The project is structured into 5 cohesive phases:

### Phase 1: Core Infrastructure (Rust)
- **Data Contracts**: Strict structs mapping AI token usages, pricing configurations, and conversation schemas.
- **Provider Interfaces**: Modular AI model integrations (e.g., Anthropic, OpenAI) supporting raw REST and generic traits.
- **SSE Streams**: Asynchronous Server-Sent Events infrastructure using `reqwest` and `tokio`.

### Phase 2: Tool System & Permissions (Rust)
- **Tool Pool**: A canonical inventory mapping operations to permissions.
- **Permission Engine**: Enforces rigid boundaries (`ReadOnly`, `WriteApprove`, etc.) over tool execution and filesystem scope.
- **Sandbox Detection**: Safely identifies execution environments (Docker, Kubernetes, WSL).

### Phase 3: Session Runtime (Rust)
- **JSONL Store**: State persistence leveraging a strict message layout (`role`, `content`, `usage`, `tool_call_id`).
- **Worker State**: Secure session markers tracking the active process parameters.
- **Transcript Compaction**: Automated message token summarization to stay under limits.

### Phase 4: CLI & Plugin Lifecycle (Rust)
- **Subcommands**: 12 dedicated `clap`-based CLI hooks returning strict JSON formats (e.g., `DumpManifests`, `SetupReport`).
- **Bootstrap Pipeline**: A 12-stage startup sequence guaranteeing safe deduplicated environment initializations.
- **Plugin State Machine**: Dynamic state toggling (`Discovered`, `Installed`, `Enabled`, `Disabled`, `Uninstalled`) with degraded capability tracking.
- **Instruction Loader**: Dynamic framework loading (`.claw/rules/`, `CLAUDE.md`) respecting configuration priorities.

### Phase 5: Companion Workspace & Audit (Python)
Located in `/python_companion/`, this subsystem consumes the artifacts produced by the Rust engine:
- **Routing Engine**: Maps unstructured prompt inputs to known commands and tools via token-overlap scoring.
- **Query Engine**: An iterative simulation tracking token budgets, enforcing turn limits, and emitting precise structured fallback payloads.
- **Transcript Store**: An in-memory cache regulating flush and replay conditions via `.jsonl` disk persistence.
- **Parity Auditor**: Validates file coverage, command entries, and tools against an archived reference snapshot of the Rust application to prevent drift.

## Getting Started

### Rust Core
To build and run the primary Rust CLI:
```bash
cargo build --release
cargo run -- --help
```

### Python Companion
To utilize the Python querying engine and auditing suites:
```bash
cd python_companion
python main.py parityaudit
python main.py setupreport
python main.py routeprompt "explain how to build the rust project"
```

## Requirements
- Rust Edition 2024 / Cargo
- Python 3.10+
