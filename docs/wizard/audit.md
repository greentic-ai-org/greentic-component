# Wizard Provider Audit (PR-COMP-01)

## Scope
Audit of current scaffold and wizard entrypoints in `greentic-component` to introduce a deterministic provider interface (`spec -> apply -> plan`) while preserving CLI UX.

## Findings

- CLI entrypoint:
  - `crates/greentic-component/src/cli.rs` routes `greentic-component wizard ...` into `cmd::wizard::run`.
- Existing wizard scaffold implementation:
  - `crates/greentic-component/src/cmd/wizard.rs` previously performed validation + direct file writes in one path.
  - Template emission is generated in-process (no external template directory) via render functions and file builders.
- Existing non-wizard scaffold command:
  - `crates/greentic-component/src/cmd/new.rs` delegates to `ScaffoldEngine` (separate `new` flow, not wizard).
- Existing QA/schema primitives already used by wizard templates:
  - Generated code references `greentic-types` QA/schema types (`ComponentQaSpec`, `QaMode`, `Question`, `QuestionKind`) in rendered `src/qa.rs`.
  - This confirms the repo already depends on `greentic-types` rather than defining a parallel QA type system.
- Existing tests:
  - `crates/greentic-component/tests/wizard_tests.rs` validates wizard output and key template behaviors.

## Chosen Integration

- Introduce provider module `crates/greentic-component/src/wizard/mod.rs` as canonical implementation.
- Keep `crates/greentic-component/src/cmd/wizard.rs` as compatibility adapter:
  - parse/validate CLI args
  - call provider `apply_scaffold(..., dry_run=true)` to produce plan
  - execute via `execute_plan(plan)`
  - preserve current CLI output/behavior
  - expose machine-consumable command surfaces:
    - `wizard spec` (returns QA spec JSON)
    - `wizard new --plan-json` (returns plan JSON, no writes)
- Plan model in provider includes:
  - `plan_version`
  - metadata (`generator`, template version, template digest, requested ABI)
  - structured steps (`ensure_dir`, `write_file`)

## Dependency Strategy

- Workspace keeps `greentic-types = "0.4"` and applies local sibling override via:
  - `[patch.crates-io] greentic-types = { path = "../greentic-types" }`
- Generated scaffold output remains publish-safe (`greentic-types = "0.4"` in generated `Cargo.toml`).

## Risks / Follow-up

- Current plan type is repo-local and intentionally small; keep it aligned for future shared plan types across `greentic-dev` and provider repos.
- Future step types like `copy_tree` / `run_command` can be added without breaking deterministic execution.
