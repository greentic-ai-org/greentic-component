# Component Wizard

The component wizard generates a ready-to-edit component@0.6.0 scaffold with Greentic conventions baked in. It focuses on deterministic templates and leaves runtime integration to follow-up work.

Legacy naming/compatibility details are in `docs/vision/legacy.md`.

**Quickstart**
1. `greentic-component wizard --mode create --execution execute --project-root .`
2. `cd hello-component`
3. `make wasm`
4. `greentic-component doctor ./dist/hello-component__0_6_0.wasm`

**What You Get**
- `Cargo.toml` with ABI metadata.
- `src/lib.rs` with guest trait wiring and `export_component_v060!`.
- `src/descriptor.rs` for `get-component-info` and `describe`.
- `src/schema.rs` for SchemaIR and canonical CBOR helpers.
- `src/runtime.rs` for CBOR run handling.
- `src/qa.rs` with QA specs and `apply-answers`.
- `src/i18n.rs` key registry.
- `assets/i18n/en.json` default bundle for i18n keys.
- A `Makefile` with `build`, `test`, `fmt`, `clippy`, `wasm`, and `doctor` targets.

**ABI Versioning + WASM Naming**
The wizard stores ABI version in `Cargo.toml` under `[package.metadata.greentic]` and uses it to name the wasm artifact:
- Output: `dist/<name>__<abi_with_underscores>.wasm`
- Example: `dist/hello-component__0_6_0.wasm`

**Wizard Modes**
The CLI supports `--mode create|build_test|doctor` with `--execution dry-run|execute`. Use `--qa-answers` for deterministic replay and `--qa-answers-out` to persist answers payloads.

**Capabilities in describe()**
Use repeatable flags to embed explicit capability declarations in generated `src/descriptor.rs`:
- `--required-capability host.http.client`
- `--required-capability host.secrets.required`
- `--provided-capability telemetry.emit`

Example:
`greentic-component wizard --mode create --execution execute --qa-answers ./answers.json`

**Doctor Validation**
`greentic-component doctor` validates the built wasm artifact for:
- required WIT exports
- QA modes and i18n coverage
- strict SchemaIR + schema hash

**Flow Integration**
After implementing your component, use Greentic Flow tooling to connect the component to a distribution client and flow registry. This keeps the wizard focused on scaffolding while flow integration is handled in the flow repo.
