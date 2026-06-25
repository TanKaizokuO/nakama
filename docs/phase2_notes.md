# Phase 2 Notes

## PreToolUse Stdout Contract

The Hook system allows integrating external logic before or after tool executions using either legacy or object-based configurations. For the `PreToolUse` hook, the system expects a specific stdout contract to determine whether the execution should be overridden.

**Contract Specifications:**
- The process must write a JSON object to its standard output.
- The object must contain an `override` string field, which can be exactly one of: `"allow"`, `"deny"`, or `"ask"`.
- If the override is `"deny"`, an optional `reason` string field can be provided to indicate why the execution was denied.
- Any invalid JSON, missing `override` field, or empty stdout output implies NO OVERRIDE (the evaluation simply proceeds).

## AgentLaunch Stub Rationale

During Phase 2, the `AgentLaunch` tool was registered but implemented only as a stub. The primary reason for this deferral is that `AgentLaunch` requires launching an isolated secondary supervisor or invoking a robust task lifecycle model which belongs strictly to Phase 3 (Multi-Agent Orchestration). Implementing it in Phase 2 would require premature introduction of supervisor contexts, task communication channels, and recursive tool boundaries that have not yet been formalized.

Thus, `AgentLaunch` is properly stubbed in Phase 2 to ensure tool schema completeness and permission map consistency, while deferring actual execution logic to the correct architectural phase.
