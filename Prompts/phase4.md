# Phase 4 — CLI, Bootstrap & Plugin System

## Overview

Phase 4 delivers the user-facing CLI surface, the bootstrap startup pipeline, the plugin lifecycle state machine, and all supporting subsystems that tie the runtime together. It implements the REPL with tab completion, one-shot and shorthand prompt modes, all global CLI flags with output format flag precedence, all 12 direct subcommands including SandboxInfo and container detection, the 12-phase bootstrap sequence with deduplication, the plugin lifecycle state machine (Discovered → Installed → Enabled ⇆ Disabled → Uninstalled) with healthcheck and degraded mode, configuration error handling, authentication error handling, and instruction file loading from the CLAUDE.md/CLAW.md/AGENTS.md priority chain. This phase depends on Phases 1–3 (config, provider API, tool system, session runtime). It unlocks Phase 5 (the Python companion workspace references the CLI command inventory, and DumpManifests/BootstrapPlan output schemas must match the Phase 3 JSONL contract freeze).

## Depends on

- **Phase 1 — Core Infrastructure**: configuration loading (for all config-dependent subcommands), model alias resolution (for model selection), provider routing (for HealthCheck API key validation), usage tracking (for cost display).
- **Phase 2 — Tool System & Permission Engine**: tool pool (for StatusReport, SkillsInspect, allowed tools validation), permission engine (for StatusReport permission mode display), MCP lifecycle (for McpInspect).
- **Phase 3 — Session & Conversation Runtime**: conversation runtime (MainRuntime bootstrap phase enters the REPL or one-shot loop), session persistence (for WorkerState, session resume via CLI flag), worker-state.json (for WorkerState subcommand), JSONL schema (for session display).

## Unlocks

- **Phase 5** — Python Companion Workspace: the Python companion mirrors the command inventory surfaced by DumpManifests; BootstrapPlan output schema must be consumable by Phase 5's BootstrapGraph. SetupReport subcommand provides platform info that Phase 5's equivalent must match.

> [RESOLVED: A11] Cross-phase dependency between DumpManifests/BootstrapPlan (Phase 4) and the Phase 3 JSONL contract freeze is explicitly flagged. The output schemas of DumpManifests and BootstrapPlan must use the same data types and field names as the Phase 3 data contracts where they overlap (e.g., tool names, session identifiers, permission modes).

## Scope

### CLI Entry Points

**Spec references:** §2.1.1, §2.1.2, §2.1.3, §2.1.5

#### Interactive REPL (§2.1.1)

- Launch: binary invoked with no subcommand argument.
- Input: user-typed prompts or slash commands, delivered via a readline-compatible input handler.
- **Tab completion**: implement context-aware tab completion for:
  - Slash commands (e.g., `/help`, `/clear`, `/compact`, `/cost`).
  - Tool names (when the context suggests a tool reference).
  - File paths (when the input starts with a path-like prefix).
  - Tab completion is triggered by the TAB key and cycles through matching candidates.

  > [RESOLVED: A8] Tab completion for the REPL is specified with completion targets (slash commands, tool names, file paths) and trigger behavior.

- Output: streamed assistant responses rendered as ANSI-formatted Markdown. Tool calls and results are rendered inline with indentation and color coding.
- Session lifecycle: set liveness flag on REPL start (Phase 3); clear on exit (including SIGINT/SIGTERM handlers).

#### One-Shot Prompt (§2.1.2)

- Invocation: binary followed by `prompt` subcommand and a text argument.
- Stdin: if no text argument is provided, read the prompt from stdin (pipe support).
- Output: complete response text. In JSON output mode (`--output-format json`), wrap the response in a JSON envelope with `response_text`, `usage`, and `stop_reason` fields.
- Return code: 0 on success; non-zero on provider or permission error.

#### Shorthand Prompt (§2.1.3)

- Invocation: binary followed by a bare string argument that does not match any known subcommand name.
- Behavior: equivalent to the one-shot prompt subcommand.
- POSIX separator: a `--` token before the string prevents dash-prefixed text from being parsed as flags. Example: `nakama -- -explain this error`.

#### Global CLI Flags (§2.1.5)

| Flag | Type | Behavior |
|------|------|----------|
| `--model` / `ModelSelection` | String | Selects the LLM model; supports aliases and pass-through |
| `--output-format` / `OutputFormat` | Enum: `text`, `json` | Human-readable or machine-readable output |
| `--permission` / `PermissionMode` | Enum (5 modes) | Sets the active permission level |
| `--cwd` / `WorkingDirectory` | Path | Overrides the process working directory |
| `--dangerously-skip-permissions` / `SkipPermissions` | Boolean | Escalates to `Allow` mode |
| `--allowed-tools` / `AllowedTools` | Comma-separated list | Restricts the tool pool to named tools |
| `--resume` / `ResumeSession` | String | Resumes a session by ID or `latest` |

> [RESOLVED: A9] Output format flag precedence is explicitly specified: environment variable (`CLAW_OUTPUT_FORMAT`) provides the default; CLI flag `--output-format` overrides the environment variable. If neither is set, default to `text`.

Output format precedence:
1. CLI flag `--output-format` (highest priority).
2. Environment variable `CLAW_OUTPUT_FORMAT`.
3. Hardcoded default: `text`.

Flag validation errors:
- `--allowed-tools` with an unknown tool name → typed error `invalid_tool_name` with the name, available tools list, and aliases.
- `--allowed-tools` missing its argument before a subcommand → typed error `missing_argument` with `argument: "--allowedTools"`.
- `--output-format` with an invalid value → typed error `invalid_output_format` with value and expected `["text", "json"]`.
- `--output-format` specified multiple times → warning on stderr; last value wins; JSON status includes `format_overridden: true`.
- `--cwd` with an invalid path → typed error `invalid_cwd` in JSON mode.

### Direct CLI Subcommands

**Spec references:** §2.1.4

All subcommands accept the optional `--output-format` flag. In `json` mode, output is a structured JSON object. In `text` mode, output is human-readable prose.

#### HealthCheck

Validates API keys, model access, tool configuration, MCP servers, memory files, and hooks. Output: an array of check objects, each with:
- `name` (String): check identifier (e.g., `"api_key"`, `"model_access"`, `"mcp_server_x"`).
- `status` (String enum: `"pass"`, `"fail"`, `"warn"`).
- `detail` (String): human-readable explanation.

Checks performed:
- API key presence: verify at least one credential environment variable is set.
- API key validity: attempt a minimal API call to the resolved provider.
- Model access: verify the resolved model name is accepted by the provider.
- Tool config: verify all tool definitions are valid.
- MCP servers: run the MCP lifecycle through the Initialize phase for each configured server.
- Hook config: validate all hook definitions.

#### StatusReport

Reports comprehensive workspace state. Output fields:
- `workspace`: root path, git info, instruction files found.
- `model`: resolved model name, alias chain, provider.
- `permissions`: active mode, rule counts (allow, deny, ask, denied_tools).
- `memory_files`: list of loaded instruction files with paths and sizes.
- `mcp_validation`: per-server status (valid/invalid/degraded).
- `hook_validation`: per-hook status (valid/invalid).
- `allowed_tools`: the effective tool pool after `--allowed-tools` filtering.

#### SandboxInfo

**Spec references:** §2.1.4

> [RESOLVED: A10] SandboxInfo and container detection are called out as a distinct subcommand concern with specific detection logic.

Reports container/sandbox detection and isolation mode:
- `container_environment`: enum value detecting the runtime environment:
  - `"docker"` — detected via `/.dockerenv` file or `docker` in `/proc/1/cgroup`.
  - `"kubernetes"` — detected via `KUBERNETES_SERVICE_HOST` environment variable.
  - `"codespaces"` — detected via `CODESPACES` environment variable.
  - `"gitpod"` — detected via `GITPOD_WORKSPACE_ID` environment variable.
  - `"wsl"` — detected via `/proc/version` containing `Microsoft` or `WSL`.
  - `"none"` — no container detected.
- `filesystem_isolation`: mode description (read-only root, writable workspace, full access).

#### VersionInfo

Reports build provenance:
- `git_sha`, `git_sha_short`, `is_dirty`, `branch`, `commit_date`: from compile-time git metadata.
- `rustc_version`: compiler version used for the build.
- `executable_path`: absolute path to the running binary.
- `binary_provenance`: how the binary was obtained (e.g., `"cargo install"`, `"release download"`, `"source build"`).

#### InitWorkspace

Scaffolds project configuration files:
- Creates `.claw/settings.json` with default configuration.
- Creates `.claw/rules/` directory.
- Creates a guidance file (`CLAW.md` or `AGENTS.md`).
- Output arrays: `created[]`, `updated[]`, `partial[]`, `deferred[]`, `skipped[]` — each containing artifact paths with status.

#### DumpManifests

Emits the resolver inventory as JSON arrays:
- `commands[]`: all registered commands with name, source, and kind.
- `tools[]`: all registered tools with name, permission level, and source.
- `agents[]`: all registered agent definitions.
- `skills[]`: all registered skills with name and source.
- `bootstrap_phases[]`: the ordered bootstrap phase list.

Output schema must be compatible with the Phase 3 data contracts (tool names use the same canonical format, permission modes use the same enum values).

#### SystemPrompt

Renders the assembled system prompt for the current workspace:
- Concatenates instruction file contents in priority order (§2.4.3).
- Includes runtime context (workspace path, model, permission mode).
- Output: the full system prompt text.

#### AgentList

Lists, shows, or scaffolds agent definitions:
- `list` mode: returns agent metadata objects (name, description, model, tools).
- `show <name>` mode: returns detailed agent configuration.
- `scaffold <name>` mode: creates a new agent definition file.

#### McpInspect

Lists configured MCP servers with validation status:
- `servers[]`: valid servers with name, transport type, tool count, lifecycle phase.
- `invalid_servers[]`: failed servers with name, error_field, reason.
- `total_configured`, `valid_count`, `invalid_count`: summary counts.

#### SkillsInspect

Lists, shows, installs, or uninstalls skills:
- `list` mode: returns skill metadata (name, source, installed status).
- `show <name>` mode: returns skill details.
- `install <name>` mode: installs a skill.
- `uninstall <name>` mode: removes a skill.

#### BootstrapPlan

Displays the ordered startup phase plan:
- Output: ordered list of phase identifiers matching the 12-phase bootstrap sequence (§3.1).
- Output schema must be compatible with Phase 5's BootstrapGraph consumer.

#### WorkerState

Reads the persisted worker state from `<workspace>/.claw/worker-state.json`:
- Output: `worker_id`, `session_id`, `model`, `permission_mode`.
- Error if file not found: structured error with hint.

### Bootstrap Sequence

**Spec references:** §3.1

The system starts through a 12-phase pipeline, executed in strict order:

1. **CLIEntry** — Parse command-line arguments. Exit early for fast-path subcommands (version, help) without loading the full runtime.
2. **FastPathVersion** — Handle `--version` and `version` subcommand immediately. No config loading.
3. **StartupProfiler** — Initialize timing infrastructure for startup performance tracking.
4. **SystemPromptFastPath** — Pre-assemble the system prompt if the subcommand requires it (e.g., `system-prompt` subcommand).
5. **ChromeMcpFastPath** — Pre-connect to Chrome-related MCP servers if configured. This is a fast-path optimization for browser-integrated workflows.
6. **DaemonWorkerFastPath** — Check for a running daemon worker and resume if possible. Avoids full startup when a daemon is already active.
7. **BridgeFastPath** — Initialize bridge connections to external processes (e.g., IDE integrations).
8. **DaemonFastPath** — Detect or launch daemon processes for persistent background operation.
9. **BackgroundSessionFastPath** — Resume background sessions if applicable (e.g., sessions started by a daemon).
10. **TemplateFastPath** — Load template configurations for project scaffolding.
11. **EnvironmentRunnerFastPath** — Configure environment-specific runners (e.g., Docker, SSH).
12. **MainRuntime** — Enter the full conversation runtime (REPL or one-shot prompt). This phase invokes the Phase 3 conversation loop.

Phase deduplication: if the phase list contains duplicates (e.g., from plugin contributions), deduplicate while preserving insertion order (first occurrence wins).

### Plugin Lifecycle State Machine

**Spec references:** §3.11, §5.9

Plugins follow a state machine with five states:

```
Discovered → Installed → Enabled ⇆ Disabled → Uninstalled
```

State transitions:
- **Discovered → Installed**: plugin files are present and structurally valid. Installation may involve dependency resolution.
- **Installed → Enabled**: plugin is activated. Its tools, commands, and MCP servers are registered with the runtime.
- **Enabled → Disabled**: plugin is deactivated. Its tools and commands are removed from the runtime pool. Configuration is preserved.
- **Disabled → Enabled**: plugin is re-activated. Tools and commands are re-registered.
- **Disabled → Uninstalled**: plugin files and configuration are removed.

Each transition fires a lifecycle event that can be observed by the runtime for logging or cleanup.

Plugin capabilities:
- **Healthcheck**: validates that the plugin's resources (MCP servers, tools, skills) are reachable and functional. Returns a health status object with `server_health`, `tool_info`, `resource_info`.
- **Degraded mode**: if a plugin's MCP server is unreachable but the plugin's non-MCP tools are functional, the plugin enters degraded mode. Its `DegradedMode` field contains details about which capabilities are reduced.
- **Tool registration**: plugins contribute tools to the runtime tool pool. Plugin-contributed tools are registered with the plugin identifier as a prefix (e.g., `plugin_name__tool_name`).

Plugin state persistence:
- Each plugin's state (`PluginIdentifier`, `State`, `HealthStatus`, `DegradedMode`) is tracked in memory during the session.
- Plugin enable/disable state is persisted in the configuration file (`PluginConfig` field).

### Configuration Error Handling

**Spec references:** §4.4

| Scenario | Error Type | Output |
|----------|-----------|--------|
| Malformed MCP server config (missing required field) | `invalid_mcp_server` | Server in `invalid_servers[]` with `error_field` and `reason`; valid siblings load normally |
| Unknown hook event name | `invalid_hook` | Recorded with `kind: unknown_hook_event`; valid hooks load normally |
| Invalid output format value | `invalid_output_format` | Error with `value` and `expected: ["text", "json"]` |
| Invalid tool name in `--allowedTools` | `invalid_tool_name` | Error with `name`, `available_tools[]`, `aliases[]` |
| Missing argument for `--allowedTools` | `missing_argument` | Error with `argument: "--allowedTools"` |
| Repeated `--output-format` flags | Warning | Stderr warning; last value wins; JSON: `format_overridden: true` |
| Invalid working directory path | `invalid_cwd` | Error in JSON mode |

Graceful degradation principle: invalid configuration entries are isolated and recorded without preventing valid sibling entries from loading. The system continues with reduced functionality rather than failing entirely.

### Authentication Error Handling

**Spec references:** §4.1

| Scenario | Behavior |
|----------|----------|
| No credential environment variable set | Error listing expected variables; hint if non-Anthropic credential detected (e.g., "Found OPENAI_API_KEY — use `openai/` model prefix") |
| API key in wrong variable (`sk-ant-*` in `ANTHROPIC_AUTH_TOKEN`) | 401 error with hint: `"Move the key to ANTHROPIC_API_KEY"` |
| Expired OAuth token | Refresh attempt; if refresh fails, error with re-authentication instructions |
| Invalid or revoked key | Provider 401/403 surfaced with HTTP status |

### Instruction File Loading

**Spec references:** §2.4.3

Load instruction/guidance content from the following paths in priority order:

1. `CLAUDE.md` (highest priority)
2. `CLAW.md`
3. `AGENTS.md`
4. `.claw/CLAUDE.md`, `.claude/CLAUDE.md`, `.claw/instructions.md`
5. Sorted files from `.claw/rules/` (extensions: `.md`, `.txt`, `.mdc`)
6. Sorted files from `.claw/rules.local/` (gitignored, personal overrides)

Discovery boundary: bounded to the current Git root when present (found by walking parent directories for `.git/`). Otherwise, bounded to the working directory only. Files outside this boundary are not loaded.

Loading behavior:
- All discovered files are read and concatenated (with separator markers) to form the system prompt instruction section.
- Files are loaded in priority order; higher-priority files appear earlier in the assembled prompt.
- Missing files are silently skipped.
- Binary files (detected by null byte presence) are skipped with a warning.

## Acceptance criteria

1. The REPL starts with no subcommand argument, accepts user input via readline, renders ANSI-formatted Markdown responses, and persists session state on each turn.
2. Tab completion provides candidates for slash commands, tool names, and file paths when triggered by the TAB key in the REPL.
3. One-shot prompt accepts a text argument or stdin pipe, outputs the complete response (or JSON envelope in `--output-format json` mode), and returns exit code 0 on success.
4. Shorthand prompt treats a bare string argument (not matching a subcommand) as a one-shot prompt, and `--` prevents dash-prefixed text from being parsed as flags.
5. Output format precedence follows: CLI flag → environment variable `CLAW_OUTPUT_FORMAT` → default `text`.
6. All 12 direct subcommands (HealthCheck through WorkerState) produce correct output in both `text` and `json` modes, including SandboxInfo with container detection.
7. The bootstrap sequence executes all 12 phases in order with deduplication (duplicate phases removed, first occurrence preserved).
8. The plugin lifecycle state machine correctly transitions through Discovered → Installed → Enabled ⇆ Disabled → Uninstalled, with healthcheck and degraded mode support.
9. Configuration error handling isolates invalid MCP servers, unknown hook events, and invalid flag values without preventing valid entries from loading.
10. Authentication error handling produces provider-specific diagnostic hints (e.g., wrong variable hint for `sk-ant-*` in `ANTHROPIC_AUTH_TOKEN`).
11. Instruction file loading discovers files in the correct priority order, respects the Git root boundary, and skips binary files.
12. DumpManifests and BootstrapPlan output schemas are compatible with Phase 3 data contracts and consumable by Phase 5.

## Git commit plan

1. **`feat: implement REPL with tab completion`**
   Build the interactive REPL with readline input, ANSI-formatted Markdown output, session lifecycle (liveness flag), and tab completion for slash commands, tool names, and file paths. Resolves audit finding A8.
   *Satisfies acceptance criteria: 1, 2*

2. **`feat: add one-shot and shorthand prompt modes`**
   Implement the `prompt` subcommand with stdin pipe support, JSON envelope output, exit codes, and shorthand prompt with `--` separator.
   *Satisfies acceptance criteria: 3, 4*

3. **`feat: implement global CLI flags with precedence`**
   Add all global flags (--model, --output-format, --permission, --cwd, --dangerously-skip-permissions, --allowed-tools, --resume) with output format precedence (CLI → env → default). Resolves audit finding A9.
   *Satisfies acceptance criteria: 5*

4. **`feat: add all 12 direct CLI subcommands`**
   Implement HealthCheck, StatusReport, SandboxInfo, VersionInfo, InitWorkspace, DumpManifests, SystemPrompt, AgentList, McpInspect, SkillsInspect, BootstrapPlan, WorkerState with text and JSON output. Resolves audit finding A10 (SandboxInfo).
   *Satisfies acceptance criteria: 6, 12*

5. **`feat: implement 12-phase bootstrap sequence`**
   Build the ordered startup pipeline (CLIEntry through MainRuntime) with phase deduplication preserving insertion order.
   *Satisfies acceptance criteria: 7*

6. **`feat: add plugin lifecycle state machine`**
   Implement Discovered → Installed → Enabled ⇆ Disabled → Uninstalled transitions, healthcheck, degraded mode, and tool registration.
   *Satisfies acceptance criteria: 8*

7. **`feat: add config and auth error handling`**
   Implement graceful degradation for invalid MCP servers, unknown hooks, invalid flags. Add authentication error hints. Resolves audit finding A11 (cross-phase schema compatibility).
   *Satisfies acceptance criteria: 9, 10*

8. **`feat: implement instruction file loading`**
   Load CLAUDE.md / CLAW.md / AGENTS.md in priority order, scan .claw/rules/, respect Git root boundary, skip binary files.
   *Satisfies acceptance criteria: 11*

9. **`test: add CLI flag and subcommand tests`**
   Test output format precedence, flag validation errors, each subcommand's text and JSON output, and SandboxInfo container detection.
   *Satisfies acceptance criteria: 3, 5, 6*

10. **`test: add bootstrap and plugin lifecycle tests`**
    Test phase ordering, deduplication, plugin state transitions, healthcheck, and degraded mode. Verify DumpManifests/BootstrapPlan schema compatibility.
    *Satisfies acceptance criteria: 7, 8, 12*

## Open questions

1. **Tab completion implementation**: The spec does not specify whether tab completion should use a library (e.g., `rustyline`) or a custom implementation. The implementer should use `rustyline` or equivalent for readline compatibility.
2. **SandboxInfo detection order**: The spec does not specify priority when multiple container signals are present (e.g., Docker inside Kubernetes). The implementer should check in the order listed and return the first match.
3. **Plugin discovery mechanism**: The spec does not define where plugins are discovered from (directory, registry, config). The implementer should scan a `<workspace>/.claw/plugins/` directory and the user-level `~/.config/claw/plugins/` directory.
4. **Bootstrap phase extensibility**: The spec does not specify whether plugins can contribute custom bootstrap phases. The implementer should treat the 12-phase list as fixed, with plugin initialization occurring within the MainRuntime phase.
5. **Instruction file size limits**: The spec does not cap instruction file sizes. Very large files could inflate the system prompt beyond the context window. The implementer should consider a configurable size limit per file (default: 100 KB).
