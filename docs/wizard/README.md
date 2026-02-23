# Wizard Provider

This module exposes deterministic scaffold planning for wizard flows.

## Design

- `spec_scaffold(mode) -> ComponentQaSpec`
- `apply_scaffold(request, dry_run) -> ApplyResult`
- `execute_plan(plan) -> Result<()>`

`apply_scaffold(..., dry_run=true)` is side-effect free and returns a reproducible plan.
Execution is explicit and separate via `execute_plan`.

## CLI Compatibility

`greentic-component wizard new ...` remains stable. The CLI adapter now:

1. validates CLI inputs,
2. calls `apply_scaffold(..., dry_run=true)`,
3. executes returned steps with `execute_plan`.

This preserves UX while switching internals to plan-driven execution.

Additional machine-friendly surfaces:

- `greentic-component wizard spec --mode <default|setup|update|remove>` prints QA spec JSON.
- `greentic-component wizard new ... --plan-json` prints deterministic plan JSON without writing files.

## Plan Shape

Current plan metadata includes:

- `plan_version`
- `generator`
- `template_version`
- `template_digest_blake3`
- `requested_abi_version`

Current step kinds:

- `ensure_dir`
- `write_file`

## Orchestrator Usage

A higher-level orchestrator (for example `greentic-dev wizard`) can:

1. request `spec_scaffold` to render prompts in any frontend,
2. submit answers/context to `apply_scaffold` in dry-run mode,
3. review or persist the plan,
4. execute the plan via `execute_plan` when approved.
