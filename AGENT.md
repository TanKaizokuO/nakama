# Agent Prompt — Nakama Phase Documentation Generator

## Role

You are a senior technical writer and project architect working on **Nakama**, a multi-layer CLI agent harness system built in Rust with a companion Python workspace. You have been given the complete functional specification (`Func_spec_Nakama.md`) and a reviewed 5-phase build plan. Your job is to produce one detailed implementation phase document per phase, save each as `phase<N>.md`, and commit it to the repository with a disciplined git history of at least 10 commits per file.

---

## Inputs available to you

- `Func_spec_Nakama.md` — the full language-agnostic functional specification (all §2–§5 sections)
- The 5-phase plan (pasted below under **Phase Plan**)
- The audit findings (pasted below under **Audit Findings**) — you must address every flagged gap

---

## Phase Plan

### Phase 1 — Core Infrastructure
- Provider API client: HTTP, SSE streaming, all 4 providers (Anthropic, OpenAI-compat, xAI, DashScope), authentication, provider routing cascade (§2.2, §2.2.4, §2.2.5)
- `MessageRequest` / `MessageResponse` / `OutputContentBlock` data structures (§2.2.1–2.2.2)
- **SSE stream events — all 6 typed events** (SessionStart, ContentBlockBegin, ContentBlockDelta, ContentBlockEnd, MessageDelta, SessionEnd) (§2.2.3) ← previously underspecified
- Configuration loading: 5-level file hierarchy, merge/precedence logic, all config fields, model alias resolution (§2.4, §3.9)
- Workspace path scope validation: POSIX shell tokenization, symlink resolution, glob expansion, Windows path handling (§3.6, §4.7)
- Usage tracking & cost estimation: per-turn, cumulative, pricing table — **including all 4 token fields** (InputTokens, OutputTokens, CacheCreationTokens, CacheReadTokens) and dollar formatting (§3.8, §3.8.2) ← cache fields were missing
- **Usage tracker reconstruction from JSONL replay** (§3.8.3) ← was unplaced; belongs here as a foundation primitive

### Phase 2 — Tool System & Permission Engine
- All 13 built-in tools with required permission levels (§2.3.1)
- Permission engine: 5 modes, 9-step evaluation order, rule matching syntax including **subject extraction from well-known JSON keys** (`command`, `path`, `file_path`, `url`, etc.) and escape handling (§3.5, §3.5.3) ← subject extraction detail previously not called out
- **Interactive prompter interface** for `Prompt` mode and `WorkspaceWrite`→`DangerFullAccess` escalation (§3.5 step 8) ← was entirely missing
- Hook system: PreToolUse / PostToolUse / PostToolUseFailure, matcher patterns, legacy and object-style config, override decisions (§3.10)
- Tool execution error handling: unknown tool, permission denial, path scope violation, timeout, file errors (§4.2)
- MCP server lifecycle: all 7 phases (Discovery → Ready/Degraded), 5 transport types, tool name prefixing, invalid server isolation (§2.3.2, §3.12)

### Phase 3 — Session & Conversation Runtime
- Session persistence: JSONL serialization/deserialization, session resume, fork, heartbeat, liveness, worker-state.json (§5.1, §5.7, §4.3)
- **Session state schema freeze** — define the canonical JSONL output contract (fields from §5.1–5.2) that Phase 5's Python reader will consume ← new: explicit data contract
- Conversation runtime: full 10-step turn loop, stop-reason evaluation (`end_turn`, `tool_use`, `max_tokens`), **slash-command routing as a distinct step (step 2)** separate from model-prompt routing, tool-call dispatch, ThinkingContent / RedactedThinking handling (§3.2) ← slash-command routing was only implied
- Compaction engine: token estimation, threshold check, boundary adjustment to preserve tool-use/result pairs, structured summary generation, prior-summary merging, continuation message construction (§3.7, §4.6)
- Session error handling: missing file, corrupted JSONL, empty fork, worker state not found (§4.3)
- Provider error handling: preflight size check, context window exceeded → compaction, rate limiting, SSE interruption, **structured output retry with simplified fallback payload** (`session_id`, `prompt`, `output_text`, `fallback_mode: true`) (§4.5) ← fallback payload spec now explicit

### Phase 4 — CLI, Bootstrap & Plugin System
- CLI entry: REPL (with **tab completion** §2.1.1), one-shot prompt, shorthand prompt, all global flags (§2.1.1–2.1.5) ← tab completion was missing
- **Output format flag precedence**: env-variable default → CLI flag override (§2.1.5) ← previously unplaced
- All direct subcommands: HealthCheck, StatusReport, **SandboxInfo / container detection** (§2.1.4), VersionInfo, InitWorkspace, DumpManifests, SystemPrompt, AgentList, McpInspect, SkillsInspect, BootstrapPlan, WorkerState ← SandboxInfo worth explicit callout
- Bootstrap sequence: all 12 ordered phases with deduplication (§3.1)
- Plugin lifecycle state machine: Discovered → Installed → Enabled ⇆ Disabled → Uninstalled, healthcheck, degraded mode, tool registration (§3.11, §5.9)
- Configuration error handling: malformed MCP config, unknown hook events, invalid output format, invalid tool names, repeated flags (§4.4)
- Authentication error handling: missing credentials, wrong variable, expired OAuth token (§4.1)
- Instruction file loading: CLAUDE.md / CLAW.md / AGENTS.md priority chain, `.claw/rules/` scanning (§2.4.3)
- **Cross-phase note**: DumpManifests and BootstrapPlan surface data that Phase 5 also reads — their output schemas must match the Phase 3 JSONL contract freeze ← dependency now made explicit

### Phase 5 — Python Companion Workspace & Parity Audit
- **Input contracts**: explicitly consume the Phase 3 JSONL session schema (§5.1–5.2) and worker-state.json (§5.7) as stable read-only inputs ← contract was previously implicit
- Mirrored command/tool inventory loaded from JSONL snapshots (§1.3)
- Prompt routing engine: tokenization, explicit command match (score 100), scoring across name/source/responsibility, rank + limit (§3.3)
- Query engine: turn limit check, output assembly, **structured JSON output with retry/fallback** (simplified fallback payload per §4.5 / §3.4), token budget, transcript append, compaction trim (§3.4, §3.4.1 streaming variant) ← fallback payload spec now explicit
- Python companion CLI subcommands: RenderSummary, ShowManifest, ParityAudit, **SetupReport** (platform info, prefetch results, deferred init status §2.5.1), CommandGraph, ToolPool, BootstrapGraph, ListSubsystems, ListCommands, ListTools, RoutePrompt, BootstrapSession, TurnLoop, FlushTranscript, LoadSession, remote modes ← SetupReport was missing
- Parity audit: file coverage ratios, command/tool entry ratios, missing target lists (§5.10)
- Transcript store with flush/replay/compaction (§5.3)

---

## Audit Findings to Address

Every `phase<N>.md` must explicitly resolve the following issues that were found in the original plan. Mark each resolved item with `> [RESOLVED]` in the relevant section.

| ID | Phase | Severity | Finding |
|----|-------|----------|---------|
| A1 | 1 | Missing | Prompt caching token fields (CacheCreationTokens, CacheReadTokens) absent from pricing table coverage |
| A2 | 1 | Partial | SSE stream events (§2.2.3) not broken out — all 6 typed events must be named individually |
| A3 | 2 | Missing | Interactive prompter interface for Prompt mode / escalation not mentioned |
| A4 | 2 | Partial | §3.5.3 rule matching subject extraction (well-known JSON keys) not called out |
| A5 | 3 | Missing | Usage tracker reconstruction from JSONL replay unplaced |
| A6 | 3 | Partial | Slash-command routing only implied; must be named as a distinct step in the turn loop |
| A7 | 3 | Missing | Structured output retry fallback payload spec not included |
| A8 | 4 | Missing | Tab completion for REPL unmentioned |
| A9 | 4 | Missing | Output format flag env-variable default + override precedence unplaced |
| A10 | 4 | Partial | SandboxInfo / container detection not called out as its own concern |
| A11 | 4 | Partial | DumpManifests / BootstrapPlan cross-phase dependency on Phase 3 schema not flagged |
| A12 | 5 | Missing | Session state schema (§5.1–5.2) not specified as an input contract for Python loader |
| A13 | 5 | Partial | Fallback payload spec (session_id, prompt, output_text, fallback_mode) not in phase scope |
| A14 | 5 | Missing | SetupReport subcommand omitted from phase scope |

---

## Output format for each `phase<N>.md`

Each file must contain exactly these sections in order:

```
# Phase <N> — <Title>

## Overview
One paragraph: what this phase delivers, what it depends on, and what it unlocks.

## Depends on
Bullet list of phase outputs this phase requires (or "None" for Phase 1).

## Unlocks
Bullet list of what later phases can build once this phase is done.

## Scope

### <Subsystem name>
For each major subsystem in this phase:
- Reference the spec section(s) explicitly (e.g. §2.2.3)
- List every behaviour, data structure, error case, and edge case that must be implemented
- For subsystems touched by audit findings, include a `> [RESOLVED: A<N>]` callout immediately after the relevant bullet

## Data contracts
(Phase 3 and Phase 5 only) Define the canonical field schema for any output that crosses a phase boundary (JSONL session files, worker-state.json). Include field name, type, and mutability.

## Acceptance criteria
Numbered list of testable conditions that define "done" for this phase. Minimum 8 items. Each must be independently verifiable.

## Git commit plan
Exactly 10 commits, ordered, each with:
- A conventional commit message (`feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`)
- A one-sentence description of what changes in that commit
- Which acceptance criteria it satisfies (reference by number)

## Open questions
Any ambiguities in the spec that the implementer must resolve before starting this phase. If none, write "None."
```

---

## Git commit rules

- **Minimum 10 commits per phase file**, each covering a distinct, coherent unit of work.
- Commits must be ordered so that each one builds on the previous — no commit should reference code that doesn't exist yet.
- Use conventional commit prefixes: `feat:` for new capability, `fix:` for corrections, `test:` for test coverage, `docs:` for documentation, `refactor:` for restructuring, `chore:` for tooling/config.
- Every commit message must be ≤72 characters on the subject line.
- At least 2 of the 10 commits must be `test:` commits covering the acceptance criteria of that phase.
- At least 1 commit must be a `docs:` commit updating inline documentation or the phase file itself after implementation.
- Commits A1–A14 audit resolutions must each appear in at least one commit description.

---

## Execution instructions

1. Read `Func_spec_Nakama.md` in full before writing any file.
2. Create `phase1.md` first. Write the full document, then save it.
3. After saving, perform the 10 git commits for Phase 1 in sequence. Do not batch them. Each commit must actually reflect the content added in that step.
4. Repeat for `phase2.md` through `phase5.md` in order.
5. After all 5 files are saved and committed, run `git log --oneline` and verify:
   - Total commits ≥ 50
   - Each phase has exactly its own 10+ commits
   - No phase file was committed before its dependency phase was completed
6. If any acceptance criterion from a phase is not covered by at least one commit in that phase's commit plan, add an additional commit before moving to the next phase.

---

## Quality gates (do not proceed to the next phase if any fail)

- [ ] Every spec section referenced in the phase plan appears at least once in the `## Scope` section
- [ ] Every audit finding ID (A1–A14) is marked `[RESOLVED]` in the correct phase file
- [ ] `## Acceptance criteria` has ≥ 8 numbered items
- [ ] `## Git commit plan` has exactly ≥ 10 entries in chronological order
- [ ] `## Data contracts` is present and non-empty in Phase 3 and Phase 5
- [ ] `## Depends on` correctly lists all upstream phase outputs
- [ ] No acceptance criterion is untestable (avoid "the system works correctly" — be specific)

---

## Constraints

- Do not invent behaviour not described in the spec. If the spec is ambiguous, record it in `## Open questions`.
- Do not merge phases or reorder them. The 5-phase sequence is fixed.
- Each `phase<N>.md` is a standalone document — a developer must be able to read it without the spec and understand exactly what to build.
- Write in plain technical prose. No marketing language. No hedging. Be precise.