use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use ciborium::Value as CborValue;
use greentic_types::cbor::canonical;
use greentic_types::i18n_text::I18nText;
use greentic_types::schemas::component::v0_6_0::{
    ChoiceOption, ComponentQaSpec, QaMode, Question, QuestionKind,
};
use serde::Serialize;
use serde_json::Map as JsonMap;
use serde_json::Value as JsonValue;

pub const PLAN_VERSION: u32 = 1;
pub const TEMPLATE_VERSION: &str = "component-scaffold-v0.6.0";
pub const GENERATOR_ID: &str = "greentic-component/wizard-provider";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum WizardMode {
    Default,
    Setup,
    Update,
    Remove,
}

#[derive(Debug, Clone)]
pub struct AnswersPayload {
    pub json: String,
    pub cbor: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct WizardRequest {
    pub name: String,
    pub abi_version: String,
    pub mode: WizardMode,
    pub target: PathBuf,
    pub answers: Option<AnswersPayload>,
    pub required_capabilities: Vec<String>,
    pub provided_capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApplyResult {
    pub plan: WizardPlanEnvelope,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WizardPlanEnvelope {
    pub plan_version: u32,
    pub metadata: WizardPlanMetadata,
    pub target_root: PathBuf,
    pub plan: WizardPlan,
}

#[derive(Debug, Clone, Serialize)]
pub struct WizardPlanMetadata {
    pub generator: String,
    pub template_version: String,
    pub template_digest_blake3: String,
    pub requested_abi_version: String,
}

// Compat shim: keep deterministic plan JSON stable without requiring newer
// greentic-types exports during cargo package verification.
#[derive(Debug, Clone, Serialize)]
pub struct WizardPlan {
    pub meta: WizardPlanMeta,
    pub steps: Vec<WizardStep>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WizardPlanMeta {
    pub id: String,
    pub target: WizardTarget,
    pub mode: WizardPlanMode,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WizardTarget {
    Component,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WizardPlanMode {
    Scaffold,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WizardStep {
    EnsureDir { paths: Vec<String> },
    WriteFiles { files: BTreeMap<String, String> },
    RunCli { command: String },
    Delegate { id: String },
    BuildComponent { project_root: String },
    TestComponent { project_root: String, full: bool },
    Doctor { project_root: String },
}

pub fn spec_scaffold(mode: WizardMode) -> ComponentQaSpec {
    let title = match mode {
        WizardMode::Default => "wizard.component.default.title",
        WizardMode::Setup => "wizard.component.setup.title",
        WizardMode::Update => "wizard.component.update.title",
        WizardMode::Remove => "wizard.component.remove.title",
    };
    ComponentQaSpec {
        mode: qa_mode(mode),
        title: I18nText::new(title, None),
        description: Some(I18nText::new("wizard.component.description", None)),
        questions: vec![
            Question {
                id: "component.name".to_string(),
                label: I18nText::new("wizard.component.name.label", None),
                help: Some(I18nText::new("wizard.component.name.help", None)),
                error: None,
                kind: QuestionKind::Text,
                required: true,
                default: None,
            },
            Question {
                id: "component.path".to_string(),
                label: I18nText::new("wizard.component.path.label", None),
                help: Some(I18nText::new("wizard.component.path.help", None)),
                error: None,
                kind: QuestionKind::Text,
                required: false,
                default: None,
            },
            Question {
                id: "component.kind".to_string(),
                label: I18nText::new("wizard.component.kind.label", None),
                help: Some(I18nText::new("wizard.component.kind.help", None)),
                error: None,
                kind: QuestionKind::Choice {
                    options: vec![
                        ChoiceOption {
                            value: "tool".to_string(),
                            label: I18nText::new("wizard.component.kind.option.tool", None),
                        },
                        ChoiceOption {
                            value: "source".to_string(),
                            label: I18nText::new("wizard.component.kind.option.source", None),
                        },
                    ],
                },
                required: false,
                default: None,
            },
            Question {
                id: "component.features.enabled".to_string(),
                label: I18nText::new("wizard.component.features.enabled.label", None),
                help: Some(I18nText::new(
                    "wizard.component.features.enabled.help",
                    None,
                )),
                error: None,
                kind: QuestionKind::Bool,
                required: false,
                default: None,
            },
        ],
        defaults: BTreeMap::from([(
            "component.features.enabled".to_string(),
            CborValue::Bool(true),
        )]),
    }
}

pub fn apply_scaffold(request: WizardRequest, dry_run: bool) -> Result<ApplyResult> {
    let warnings = abi_warnings(&request.abi_version);
    let (prefill_answers_json, prefill_answers_cbor, component_kind, mut mapping_warnings) =
        normalize_answers(request.answers, request.mode)?;
    let mut all_warnings = warnings;
    all_warnings.append(&mut mapping_warnings);
    let context = WizardContext {
        name: request.name,
        abi_version: request.abi_version.clone(),
        prefill_mode: request.mode,
        prefill_answers_cbor,
        prefill_answers_json,
        required_capabilities: normalize_capabilities(request.required_capabilities)?,
        provided_capabilities: normalize_capabilities(request.provided_capabilities)?,
        component_kind,
    };

    let files = build_files(&context)?;
    let plan = build_plan(request.target, &request.abi_version, files);
    if !dry_run {
        execute_plan(&plan)?;
    }

    Ok(ApplyResult {
        plan,
        warnings: all_warnings,
    })
}

pub fn execute_plan(envelope: &WizardPlanEnvelope) -> Result<()> {
    for step in &envelope.plan.steps {
        match step {
            WizardStep::EnsureDir { paths } => {
                for path in paths {
                    let dir = envelope.target_root.join(path);
                    fs::create_dir_all(&dir).with_context(|| {
                        format!("wizard: failed to create directory {}", dir.display())
                    })?;
                }
            }
            WizardStep::WriteFiles { files } => {
                for (relative_path, content) in files {
                    let target = envelope.target_root.join(relative_path);
                    if let Some(parent) = target.parent() {
                        fs::create_dir_all(parent).with_context(|| {
                            format!("wizard: failed to create directory {}", parent.display())
                        })?;
                    }
                    let bytes = decode_step_content(relative_path, content)?;
                    fs::write(&target, bytes)
                        .with_context(|| format!("wizard: failed to write {}", target.display()))?;
                }
            }
            WizardStep::RunCli { command, .. } => {
                bail!("wizard: unsupported plan step run_cli ({command})")
            }
            WizardStep::Delegate { id, .. } => {
                bail!("wizard: unsupported plan step delegate ({})", id.as_str())
            }
            WizardStep::BuildComponent { project_root } => {
                bail!("wizard: unsupported plan step build_component ({project_root})")
            }
            WizardStep::TestComponent { project_root, .. } => {
                bail!("wizard: unsupported plan step test_component ({project_root})")
            }
            WizardStep::Doctor { project_root } => {
                bail!("wizard: unsupported plan step doctor ({project_root})")
            }
        }
    }
    Ok(())
}

pub fn load_answers_payload(path: &Path) -> Result<AnswersPayload> {
    let json = fs::read_to_string(path)
        .with_context(|| format!("wizard: failed to open answers file {}", path.display()))?;
    let value: JsonValue = serde_json::from_str(&json)
        .with_context(|| format!("wizard: answers file {} is not valid JSON", path.display()))?;
    let cbor = canonical::to_canonical_cbor_allow_floats(&value)
        .map_err(|err| anyhow!("wizard: failed to encode answers as CBOR: {err}"))?;
    Ok(AnswersPayload { json, cbor })
}

struct WizardContext {
    name: String,
    abi_version: String,
    prefill_mode: WizardMode,
    prefill_answers_cbor: Option<Vec<u8>>,
    prefill_answers_json: Option<String>,
    required_capabilities: Vec<String>,
    provided_capabilities: Vec<String>,
    component_kind: String,
}

type NormalizedAnswers = (Option<String>, Option<Vec<u8>>, String, Vec<String>);

fn normalize_answers(
    answers: Option<AnswersPayload>,
    mode: WizardMode,
) -> Result<NormalizedAnswers> {
    let mut warnings = Vec::new();
    let mut component_kind = "tool".to_string();
    let Some(payload) = answers else {
        return Ok((None, None, component_kind, warnings));
    };
    let mut value: JsonValue = serde_json::from_str(&payload.json).with_context(|| {
        "wizard: answers JSON payload should be valid after initial parse".to_string()
    })?;
    let JsonValue::Object(mut root) = value else {
        return Ok((
            Some(payload.json),
            Some(payload.cbor),
            component_kind,
            warnings,
        ));
    };

    let enabled = extract_bool(&root, &["component.features.enabled", "enabled"]);
    if let Some(flag) = enabled {
        root.insert("enabled".to_string(), JsonValue::Bool(flag));
    } else if matches!(
        mode,
        WizardMode::Default | WizardMode::Setup | WizardMode::Update
    ) {
        root.insert("enabled".to_string(), JsonValue::Bool(true));
    }

    if let Some(kind) = extract_string(&root, &["component.kind", "kind"]) {
        if matches!(kind.as_str(), "tool" | "source") {
            component_kind = kind;
        } else {
            warnings.push(format!(
                "wizard: unsupported component.kind value `{kind}`; using `tool`"
            ));
        }
    }

    value = JsonValue::Object(root);
    let json = serde_json::to_string_pretty(&value)?;
    let cbor = canonical::to_canonical_cbor_allow_floats(&value)
        .map_err(|err| anyhow!("wizard: failed to encode normalized answers as CBOR: {err}"))?;
    Ok((Some(json), Some(cbor), component_kind, warnings))
}

fn extract_bool(root: &JsonMap<String, JsonValue>, keys: &[&str]) -> Option<bool> {
    for key in keys {
        if let Some(value) = root.get(*key)
            && let Some(flag) = value.as_bool()
        {
            return Some(flag);
        }
        if let Some(flag) = nested_bool(root, key) {
            return Some(flag);
        }
    }
    None
}

fn extract_string(root: &JsonMap<String, JsonValue>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(value) = root.get(*key)
            && let Some(s) = value.as_str()
        {
            return Some(s.to_string());
        }
        if let Some(s) = nested_string(root, key) {
            return Some(s);
        }
    }
    None
}

fn nested_bool(root: &JsonMap<String, JsonValue>, dotted: &str) -> Option<bool> {
    nested_value(root, dotted).and_then(|value| value.as_bool())
}

fn nested_string(root: &JsonMap<String, JsonValue>, dotted: &str) -> Option<String> {
    nested_value(root, dotted)
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
}

fn nested_value<'a>(root: &'a JsonMap<String, JsonValue>, dotted: &str) -> Option<&'a JsonValue> {
    let mut parts = dotted.split('.');
    let first = parts.next()?;
    let mut current = root.get(first)?;
    for segment in parts {
        let JsonValue::Object(map) = current else {
            return None;
        };
        current = map.get(segment)?;
    }
    Some(current)
}

fn normalize_capabilities(capabilities: Vec<String>) -> Result<Vec<String>> {
    let mut cleaned = Vec::new();
    for capability in capabilities {
        let trimmed = capability.trim();
        if trimmed.is_empty() {
            bail!("wizard: capability values cannot be empty");
        }
        cleaned.push(trimmed.to_string());
    }
    cleaned.sort();
    cleaned.dedup();
    Ok(cleaned)
}

struct GeneratedFile {
    path: PathBuf,
    contents: Vec<u8>,
}

fn build_files(context: &WizardContext) -> Result<Vec<GeneratedFile>> {
    let mut files = vec![
        text_file("Cargo.toml", render_cargo_toml(context)),
        text_file("rust-toolchain.toml", render_rust_toolchain_toml()),
        text_file("README.md", render_readme(context)),
        text_file("component.manifest.json", render_manifest_json(context)),
        text_file("Makefile", render_makefile()),
        text_file("src/lib.rs", render_lib_rs()),
        text_file("src/descriptor.rs", render_descriptor_rs(context)),
        text_file("src/schema.rs", render_schema_rs()),
        text_file("src/runtime.rs", render_runtime_rs()),
        text_file("src/qa.rs", render_qa_rs(context)),
        text_file("src/i18n.rs", render_i18n_rs()),
        text_file("assets/i18n/en.json", render_i18n_bundle()),
    ];

    if let (Some(json), Some(cbor)) = (
        context.prefill_answers_json.as_ref(),
        context.prefill_answers_cbor.as_ref(),
    ) {
        let mode = match context.prefill_mode {
            WizardMode::Default => "default",
            WizardMode::Setup => "setup",
            WizardMode::Update => "update",
            WizardMode::Remove => "remove",
        };
        files.push(text_file(
            &format!("examples/{mode}.answers.json"),
            json.clone(),
        ));
        files.push(binary_file(
            &format!("examples/{mode}.answers.cbor"),
            cbor.clone(),
        ));
    }

    Ok(files)
}

fn build_plan(target: PathBuf, abi_version: &str, files: Vec<GeneratedFile>) -> WizardPlanEnvelope {
    let mut dirs = BTreeSet::new();
    for file in &files {
        if let Some(parent) = file.path.parent()
            && !parent.as_os_str().is_empty()
        {
            dirs.insert(parent.to_path_buf());
        }
    }
    let mut steps: Vec<WizardStep> = Vec::new();
    if !dirs.is_empty() {
        let paths = dirs
            .into_iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        steps.push(WizardStep::EnsureDir { paths });
    }

    let mut file_map = BTreeMap::new();
    for file in &files {
        let key = file.path.to_string_lossy().into_owned();
        file_map.insert(key, encode_step_content(&file.path, &file.contents));
    }
    if !file_map.is_empty() {
        steps.push(WizardStep::WriteFiles { files: file_map });
    }

    let plan = WizardPlan {
        meta: WizardPlanMeta {
            id: "greentic.component.scaffold".to_string(),
            target: WizardTarget::Component,
            mode: WizardPlanMode::Scaffold,
        },
        steps,
    };
    let metadata = WizardPlanMetadata {
        generator: GENERATOR_ID.to_string(),
        template_version: TEMPLATE_VERSION.to_string(),
        template_digest_blake3: template_digest_hex(&files),
        requested_abi_version: abi_version.to_string(),
    };
    WizardPlanEnvelope {
        plan_version: PLAN_VERSION,
        metadata,
        target_root: target,
        plan,
    }
}

const STEP_BASE64_PREFIX: &str = "base64:";

fn encode_step_content(path: &Path, bytes: &[u8]) -> String {
    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext == "cbor")
    {
        format!(
            "{STEP_BASE64_PREFIX}{}",
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes)
        )
    } else {
        String::from_utf8(bytes.to_vec()).unwrap_or_default()
    }
}

fn decode_step_content(relative_path: &str, content: &str) -> Result<Vec<u8>> {
    if relative_path.ends_with(".cbor") && content.starts_with(STEP_BASE64_PREFIX) {
        let raw = content.trim_start_matches(STEP_BASE64_PREFIX);
        let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, raw)
            .map_err(|err| anyhow!("wizard: invalid base64 content for {relative_path}: {err}"))?;
        return Ok(decoded);
    }
    Ok(content.as_bytes().to_vec())
}

fn template_digest_hex(files: &[GeneratedFile]) -> String {
    let mut hasher = blake3::Hasher::new();
    for file in files {
        hasher.update(file.path.to_string_lossy().as_bytes());
        hasher.update(&[0]);
        hasher.update(&file.contents);
        hasher.update(&[0xff]);
    }
    hasher.finalize().to_hex().to_string()
}

fn abi_warnings(abi_version: &str) -> Vec<String> {
    if abi_version == "0.6.0" {
        Vec::new()
    } else {
        vec![format!(
            "wizard: warning: only component@0.6.0 template is generated (requested {abi_version})"
        )]
    }
}

fn qa_mode(mode: WizardMode) -> QaMode {
    match mode {
        WizardMode::Default => QaMode::Default,
        WizardMode::Setup => QaMode::Setup,
        WizardMode::Update => QaMode::Update,
        WizardMode::Remove => QaMode::Remove,
    }
}

fn render_rust_toolchain_toml() -> String {
    r#"[toolchain]
channel = "1.91.0"
components = ["clippy", "rustfmt"]
targets = ["wasm32-wasip2", "x86_64-unknown-linux-gnu"]
profile = "minimal"
"#
    .to_string()
}

fn text_file(path: &str, contents: String) -> GeneratedFile {
    GeneratedFile {
        path: PathBuf::from(path),
        contents: contents.into_bytes(),
    }
}

fn binary_file(path: &str, contents: Vec<u8>) -> GeneratedFile {
    GeneratedFile {
        path: PathBuf::from(path),
        contents,
    }
}

fn render_cargo_toml(context: &WizardContext) -> String {
    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2024"
license = "MIT"
rust-version = "1.91"
description = "Greentic component {name}"

[lib]
crate-type = ["cdylib", "rlib"]

[package.metadata.greentic]
abi_version = "{abi_version}"

[package.metadata.component]
package = "greentic:component"

[package.metadata.component.target]
world = "greentic:component/component-v0-v6-v0@0.6.0"

[dependencies]
greentic-types = "0.4"
greentic-interfaces-guest = {{ version = "0.4", default-features = false, features = ["component-v0-6"] }}
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
"#,
        name = context.name,
        abi_version = context.abi_version
    )
}

fn render_readme(context: &WizardContext) -> String {
    format!(
        r#"# {name}

Generated by `greentic-component wizard` for component@0.6.0.

## Next steps
- Refine schemas in `src/schema.rs`.
- Implement runtime logic in `src/runtime.rs`.
- Extend QA flows in `src/qa.rs` and i18n keys in `src/i18n.rs`.

## ABI version
Requested ABI version: {abi_version}

Note: the wizard currently emits a fixed 0.6.0 template.
"#,
        name = context.name,
        abi_version = context.abi_version
    )
}

fn render_makefile() -> String {
    r#"SHELL := /bin/sh

NAME := $(shell awk 'BEGIN{in_pkg=0} /^\[package\]/{in_pkg=1; next} /^\[/{in_pkg=0} in_pkg && /^name = / {gsub(/"/ , "", $$3); print $$3; exit}' Cargo.toml)
NAME_UNDERSCORE := $(subst -,_,$(NAME))
ABI_VERSION := $(shell awk 'BEGIN{in_meta=0} /^\[package.metadata.greentic\]/{in_meta=1; next} /^\[/{in_meta=0} in_meta && /^abi_version = / {gsub(/"/ , "", $$3); print $$3; exit}' Cargo.toml)
ABI_VERSION_UNDERSCORE := $(subst .,_,$(ABI_VERSION))
DIST_DIR := dist
WASM_OUT := $(DIST_DIR)/$(NAME)__$(ABI_VERSION_UNDERSCORE).wasm

.PHONY: build test fmt clippy wasm doctor

build:
	cargo build

test:
	cargo test

fmt:
	cargo fmt

clippy:
	cargo clippy --all-targets --all-features -- -D warnings

wasm:
	if ! cargo component --version >/dev/null 2>&1; then \
		echo "cargo-component is required to produce a valid component@0.6.0 wasm"; \
		echo "install with: cargo install cargo-component --locked"; \
		exit 1; \
	fi
	RUSTFLAGS= CARGO_ENCODED_RUSTFLAGS= cargo component build --release --target wasm32-wasip2
	WASM_SRC=""; \
	for cand in \
		"$${CARGO_TARGET_DIR:-target}/wasm32-wasip2/release/$(NAME_UNDERSCORE).wasm" \
		"$${CARGO_TARGET_DIR:-target}/wasm32-wasip2/release/$(NAME).wasm" \
		"$${CARGO_TARGET_DIR:-target}/wasm32-wasip1/release/$(NAME_UNDERSCORE).wasm" \
		"$${CARGO_TARGET_DIR:-target}/wasm32-wasip1/release/$(NAME).wasm" \
		"target/wasm32-wasip2/release/$(NAME_UNDERSCORE).wasm" \
		"target/wasm32-wasip2/release/$(NAME).wasm" \
		"target/wasm32-wasip1/release/$(NAME_UNDERSCORE).wasm" \
		"target/wasm32-wasip1/release/$(NAME).wasm"; do \
		if [ -f "$$cand" ]; then WASM_SRC="$$cand"; break; fi; \
	done; \
	if [ -z "$$WASM_SRC" ]; then \
		echo "unable to locate wasm build artifact for $(NAME)"; \
		exit 1; \
	fi; \
	mkdir -p $(DIST_DIR); \
	cp "$$WASM_SRC" $(WASM_OUT)

doctor:
	greentic-component doctor $(WASM_OUT)
"#
    .to_string()
}

fn render_manifest_json(context: &WizardContext) -> String {
    let name_snake = context.name.replace('-', "_");
    format!(
        r#"{{
  "$schema": "https://greenticai.github.io/greentic-component/schemas/v1/component.manifest.schema.json",
  "id": "com.example.{name}",
  "name": "{name}",
  "version": "0.1.0",
  "world": "greentic:component/component-v0-v6-v0@0.6.0",
  "describe_export": "describe",
  "operations": [
    {{
      "name": "run",
      "input_schema": {{
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "{name} input",
        "type": "object",
        "required": ["message"],
        "properties": {{
          "message": {{
            "type": "string",
            "default": "hello"
          }}
        }},
        "additionalProperties": false
      }},
      "output_schema": {{
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "{name} output",
        "type": "object",
        "required": ["result"],
        "properties": {{
          "result": {{
            "type": "string"
          }}
        }},
        "additionalProperties": false
      }}
    }}
  ],
  "default_operation": "run",
  "config_schema": {{
    "type": "object",
    "required": ["enabled"],
    "properties": {{
      "enabled": {{
        "type": "boolean"
      }}
    }},
    "additionalProperties": false
  }},
  "supports": ["messaging"],
  "profiles": {{
    "default": "stateless",
    "supported": ["stateless"]
  }},
  "secret_requirements": [],
  "capabilities": {{
    "wasi": {{
      "filesystem": {{
        "mode": "none",
        "mounts": []
      }},
      "random": true,
      "clocks": true
    }},
    "host": {{
      "messaging": {{
        "inbound": true,
        "outbound": true
      }},
      "telemetry": {{
        "scope": "node"
      }},
      "secrets": {{
        "required": []
      }}
    }}
  }},
  "limits": {{
    "memory_mb": 128,
    "wall_time_ms": 1000
  }},
  "artifacts": {{
    "component_wasm": "target/wasm32-wasip2/release/{name_snake}.wasm"
  }},
  "hashes": {{
    "component_wasm": "blake3:0000000000000000000000000000000000000000000000000000000000000000"
  }},
  "dev_flows": {{
    "default": {{
      "format": "flow-ir-json",
      "graph": {{
        "nodes": [
          {{ "id": "start", "type": "start" }},
          {{ "id": "end", "type": "end" }}
        ],
        "edges": [
          {{ "from": "start", "to": "end" }}
        ]
      }}
    }}
  }}
}}
"#,
        name = context.name,
        name_snake = name_snake
    )
}

fn render_lib_rs() -> String {
    r#"use greentic_interfaces_guest::component_v0_6::node;

pub mod descriptor;
pub mod schema;
mod runtime;
pub mod qa;
pub mod i18n;

#[cfg(target_arch = "wasm32")]
#[used]
#[unsafe(link_section = ".greentic.wasi")]
static WASI_TARGET_MARKER: [u8; 13] = *b"wasm32-wasip2";

struct Component;

impl node::Guest for Component {
    fn describe() -> node::ComponentDescriptor {
        let info = descriptor::info();
        node::ComponentDescriptor {
            name: info.id,
            version: info.version,
            summary: Some("Generated by greentic-component wizard".to_string()),
            capabilities: Vec::new(),
            ops: vec![node::Op {
                name: "run".to_string(),
                summary: Some("Run the component with CBOR payload".to_string()),
                input: node::IoSchema {
                    schema: node::SchemaSource::InlineCbor(schema::input_schema_cbor()),
                    content_type: "application/cbor".to_string(),
                    schema_version: None,
                },
                output: node::IoSchema {
                    schema: node::SchemaSource::InlineCbor(schema::output_schema_cbor()),
                    content_type: "application/cbor".to_string(),
                    schema_version: None,
                },
                examples: Vec::new(),
            }],
            schemas: Vec::new(),
            setup: None,
        }
    }

    fn invoke(
        operation: String,
        envelope: node::InvocationEnvelope,
    ) -> Result<node::InvocationResult, node::NodeError> {
        let (output, _new_state) = runtime::run(envelope.payload_cbor, Vec::new());
        let output = if operation == "run" {
            output
        } else {
            runtime::run(
                greentic_types::cbor::canonical::to_canonical_cbor_allow_floats(&serde_json::json!({
                    "message": format!("unsupported operation: {operation}")
                }))
                .unwrap_or_default(),
                Vec::new(),
            )
            .0
        };
        Ok(node::InvocationResult {
            ok: true,
            output_cbor: output,
            output_metadata_cbor: None,
        })
    }
}

#[cfg(target_arch = "wasm32")]
greentic_interfaces_guest::export_component_v060!(Component);
"#
    .to_string()
}

fn render_qa_rs(context: &WizardContext) -> String {
    let (default_prefill, setup_prefill, update_prefill, remove_prefill) =
        match context.prefill_answers_cbor.as_ref() {
            Some(bytes) if context.prefill_mode == WizardMode::Default => (
                bytes_literal(bytes),
                "&[]".to_string(),
                "&[]".to_string(),
                "&[]".to_string(),
            ),
            Some(bytes) if context.prefill_mode == WizardMode::Setup => (
                "&[]".to_string(),
                bytes_literal(bytes),
                "&[]".to_string(),
                "&[]".to_string(),
            ),
            Some(bytes) if context.prefill_mode == WizardMode::Update => (
                "&[]".to_string(),
                "&[]".to_string(),
                bytes_literal(bytes),
                "&[]".to_string(),
            ),
            Some(bytes) if context.prefill_mode == WizardMode::Remove => (
                "&[]".to_string(),
                "&[]".to_string(),
                "&[]".to_string(),
                bytes_literal(bytes),
            ),
            _ => (
                "&[]".to_string(),
                "&[]".to_string(),
                "&[]".to_string(),
                "&[]".to_string(),
            ),
        };

    let template = r#"use std::collections::BTreeMap;

use greentic_types::cbor::canonical;
use greentic_types::i18n_text::I18nText;
use greentic_types::schemas::component::v0_6_0::{ComponentQaSpec, QaMode, Question, QuestionKind};
use serde_json::Value as JsonValue;

const DEFAULT_PREFILLED_ANSWERS_CBOR: &[u8] = __DEFAULT_PREFILL__;
const SETUP_PREFILLED_ANSWERS_CBOR: &[u8] = __SETUP_PREFILL__;
const UPDATE_PREFILLED_ANSWERS_CBOR: &[u8] = __UPDATE_PREFILL__;
const REMOVE_PREFILLED_ANSWERS_CBOR: &[u8] = __REMOVE_PREFILL__;

#[derive(Debug, Clone, Copy)]
pub enum Mode {
    Default,
    Setup,
    Update,
    Remove,
}

pub fn qa_spec_cbor(mode: Mode) -> Vec<u8> {
    let spec = qa_spec(mode);
    canonical::to_canonical_cbor_allow_floats(&spec).unwrap_or_default()
}

pub fn prefilled_answers_cbor(mode: Mode) -> &'static [u8] {
    match mode {
        Mode::Default => DEFAULT_PREFILLED_ANSWERS_CBOR,
        Mode::Setup => SETUP_PREFILLED_ANSWERS_CBOR,
        Mode::Update => UPDATE_PREFILLED_ANSWERS_CBOR,
        Mode::Remove => REMOVE_PREFILLED_ANSWERS_CBOR,
    }
}

pub fn apply_answers(mode: Mode, current_config: Vec<u8>, answers: Vec<u8>) -> Vec<u8> {
    let mut config = decode_map(&current_config);
    let updates = decode_map(&answers);
    match mode {
        Mode::Default | Mode::Setup | Mode::Update => {
            for (key, value) in updates {
                config.insert(key, value);
            }
            config
                .entry("enabled".to_string())
                .or_insert(JsonValue::Bool(true));
        }
        Mode::Remove => {
            config.clear();
            config.insert("enabled".to_string(), JsonValue::Bool(false));
        }
    }
    canonical::to_canonical_cbor_allow_floats(&config).unwrap_or_default()
}

fn qa_spec(mode: Mode) -> ComponentQaSpec {
    let (title_key, description_key, questions) = match mode {
        Mode::Default => (
            "qa.default.title",
            Some("qa.default.description"),
            vec![question_enabled("qa.default.enabled.label", "qa.default.enabled.help")],
        ),
        Mode::Setup => (
            "qa.setup.title",
            Some("qa.setup.description"),
            vec![question_enabled("qa.setup.enabled.label", "qa.setup.enabled.help")],
        ),
        Mode::Update => ("qa.update.title", None, Vec::new()),
        Mode::Remove => ("qa.remove.title", None, Vec::new()),
    };
    ComponentQaSpec {
        mode: match mode {
            Mode::Default => QaMode::Default,
            Mode::Setup => QaMode::Setup,
            Mode::Update => QaMode::Update,
            Mode::Remove => QaMode::Remove,
        },
        title: I18nText::new(title_key, None),
        description: description_key.map(|key| I18nText::new(key, None)),
        questions,
        defaults: BTreeMap::new(),
    }
}

fn question_enabled(label_key: &str, help_key: &str) -> Question {
    Question {
        id: "enabled".to_string(),
        label: I18nText::new(label_key, None),
        help: Some(I18nText::new(help_key, None)),
        error: None,
        kind: QuestionKind::Bool,
        required: true,
        default: None,
    }
}

fn decode_map(bytes: &[u8]) -> BTreeMap<String, JsonValue> {
    if bytes.is_empty() {
        return BTreeMap::new();
    }
    let value: JsonValue = match canonical::from_cbor(bytes) {
        Ok(value) => value,
        Err(_) => return BTreeMap::new(),
    };
    let JsonValue::Object(map) = value else {
        return BTreeMap::new();
    };
    map.into_iter().collect()
}
"#;
    template
        .replace("__DEFAULT_PREFILL__", &default_prefill)
        .replace("__SETUP_PREFILL__", &setup_prefill)
        .replace("__UPDATE_PREFILL__", &update_prefill)
        .replace("__REMOVE_PREFILL__", &remove_prefill)
}

fn render_descriptor_rs(context: &WizardContext) -> String {
    let required_capabilities = render_capability_list(&context.required_capabilities);
    let provided_capabilities = render_capability_list(&context.provided_capabilities);
    let template = r#"use std::collections::BTreeMap;

use greentic_types::cbor::canonical;
use greentic_types::schemas::component::v0_6_0::{
    ComponentDescribe, ComponentInfo, ComponentOperation, ComponentRunInput, ComponentRunOutput,
    RedactionRule, RedactionKind, schema_hash,
};

use crate::schema;

pub fn info() -> ComponentInfo {
    ComponentInfo {
        id: "com.example.__NAME__".to_string(),
        version: "0.1.0".to_string(),
        role: "__COMPONENT_ROLE__".to_string(),
        display_name: None,
    }
}

pub fn info_cbor() -> Vec<u8> {
    canonical::to_canonical_cbor_allow_floats(&info()).unwrap_or_default()
}

pub fn describe() -> ComponentDescribe {
    let input_schema = schema::input_schema();
    let output_schema = schema::output_schema();
    let config_schema = schema::config_schema();
    let op_hash = schema_hash(&input_schema, &output_schema, &config_schema)
        .expect("schema hash");
    let operation = ComponentOperation {
        id: "run".to_string(),
        display_name: None,
        input: ComponentRunInput { schema: input_schema },
        output: ComponentRunOutput { schema: output_schema },
        defaults: BTreeMap::new(),
        redactions: vec![RedactionRule {
            json_pointer: "/secret".to_string(),
            kind: RedactionKind::Secret,
        }],
        constraints: BTreeMap::new(),
        schema_hash: op_hash,
    };
    ComponentDescribe {
        info: info(),
        provided_capabilities: provided_capabilities(),
        required_capabilities: required_capabilities(),
        metadata: BTreeMap::new(),
        operations: vec![operation],
        config_schema,
    }
}

fn required_capabilities() -> Vec<String> {
    const REQUIRED_CAPABILITIES: &[&str] = __REQUIRED_CAPABILITIES__;
    REQUIRED_CAPABILITIES
        .iter()
        .map(|capability| (*capability).to_string())
        .collect()
}

fn provided_capabilities() -> Vec<String> {
    const PROVIDED_CAPABILITIES: &[&str] = __PROVIDED_CAPABILITIES__;
    PROVIDED_CAPABILITIES
        .iter()
        .map(|capability| (*capability).to_string())
        .collect()
}

pub fn describe_cbor() -> Vec<u8> {
    canonical::to_canonical_cbor_allow_floats(&describe()).unwrap_or_default()
}
"#;
    template
        .replace("__NAME__", &context.name)
        .replace("__COMPONENT_ROLE__", &context.component_kind)
        .replace("__REQUIRED_CAPABILITIES__", &required_capabilities)
        .replace("__PROVIDED_CAPABILITIES__", &provided_capabilities)
}

fn render_capability_list(capabilities: &[String]) -> String {
    if capabilities.is_empty() {
        return "&[]".to_string();
    }
    let values = capabilities
        .iter()
        .map(|capability| serde_json::to_string(capability).unwrap_or_else(|_| "\"\"".to_string()))
        .collect::<Vec<_>>()
        .join(", ");
    format!("&[{values}]")
}

fn render_schema_rs() -> String {
    r#"use std::collections::BTreeMap;

use greentic_types::cbor::canonical;
use greentic_types::schemas::common::schema_ir::{AdditionalProperties, SchemaIr};

pub fn input_schema() -> SchemaIr {
    object_schema(vec![(
        "message",
        SchemaIr::String {
            min_len: Some(1),
            max_len: Some(1024),
            regex: None,
            format: None,
        },
    )])
}

pub fn output_schema() -> SchemaIr {
    object_schema(vec![(
        "result",
        SchemaIr::String {
            min_len: Some(1),
            max_len: Some(1024),
            regex: None,
            format: None,
        },
    )])
}

pub fn config_schema() -> SchemaIr {
    object_schema(vec![("enabled", SchemaIr::Bool)])
}

pub fn input_schema_cbor() -> Vec<u8> {
    canonical::to_canonical_cbor_allow_floats(&input_schema()).unwrap_or_default()
}

pub fn output_schema_cbor() -> Vec<u8> {
    canonical::to_canonical_cbor_allow_floats(&output_schema()).unwrap_or_default()
}

pub fn config_schema_cbor() -> Vec<u8> {
    canonical::to_canonical_cbor_allow_floats(&config_schema()).unwrap_or_default()
}

fn object_schema(props: Vec<(&str, SchemaIr)>) -> SchemaIr {
    let mut properties = BTreeMap::new();
    let mut required = Vec::new();
    for (name, schema) in props {
        properties.insert(name.to_string(), schema);
        required.push(name.to_string());
    }
    SchemaIr::Object {
        properties,
        required,
        additional: AdditionalProperties::Forbid,
    }
}
"#
    .to_string()
}

fn render_runtime_rs() -> String {
    r#"use std::collections::BTreeMap;

use greentic_types::cbor::canonical;
use serde_json::Value as JsonValue;

pub fn run(input: Vec<u8>, state: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
    let input_map = decode_map(&input);
    let message = input_map
        .get("message")
        .and_then(|value| value.as_str())
        .unwrap_or("ok");
    let mut output = BTreeMap::new();
    output.insert(
        "result".to_string(),
        JsonValue::String(format!("processed: {message}")),
    );
    let output_cbor = canonical::to_canonical_cbor_allow_floats(&output).unwrap_or_default();
    let state_cbor = canonicalize_or_empty(&state);
    (output_cbor, state_cbor)
}

fn canonicalize_or_empty(bytes: &[u8]) -> Vec<u8> {
    let empty = || {
        canonical::to_canonical_cbor_allow_floats(&BTreeMap::<String, JsonValue>::new())
            .unwrap_or_default()
    };
    if bytes.is_empty() {
        return empty();
    }
    let value: JsonValue = match canonical::from_cbor(bytes) {
        Ok(value) => value,
        Err(_) => return empty(),
    };
    canonical::to_canonical_cbor_allow_floats(&value).unwrap_or_default()
}

fn decode_map(bytes: &[u8]) -> BTreeMap<String, JsonValue> {
    if bytes.is_empty() {
        return BTreeMap::new();
    }
    let value: JsonValue = match canonical::from_cbor(bytes) {
        Ok(value) => value,
        Err(_) => return BTreeMap::new(),
    };
    let JsonValue::Object(map) = value else {
        return BTreeMap::new();
    };
    map.into_iter().collect()
}
"#
    .to_string()
}

fn render_i18n_rs() -> String {
    r#"pub const I18N_KEYS: &[&str] = &[
    "qa.default.title",
    "qa.default.description",
    "qa.default.enabled.label",
    "qa.default.enabled.help",
    "qa.setup.title",
    "qa.setup.description",
    "qa.setup.enabled.label",
    "qa.setup.enabled.help",
    "qa.update.title",
    "qa.remove.title",
];

pub fn all_keys() -> Vec<String> {
    I18N_KEYS.iter().map(|key| (*key).to_string()).collect()
}
"#
    .to_string()
}

fn render_i18n_bundle() -> String {
    r#"{
  "qa.default.title": "Default configuration",
  "qa.default.description": "Review default settings for this component.",
  "qa.default.enabled.label": "Enable the component",
  "qa.default.enabled.help": "Toggle whether the component should run.",
  "qa.setup.title": "Initial setup",
  "qa.setup.description": "Provide initial configuration values.",
  "qa.setup.enabled.label": "Enable on setup",
  "qa.setup.enabled.help": "Enable the component after setup completes.",
  "qa.update.title": "Update configuration",
  "qa.remove.title": "Removal settings"
}
"#
    .to_string()
}

fn bytes_literal(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "&[]".to_string();
    }
    let rendered = bytes
        .iter()
        .map(|b| format!("0x{b:02x}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("&[{rendered}]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_answers_cbor() {
        let json = serde_json::json!({"b": 1, "a": 2});
        let cbor = canonical::to_canonical_cbor_allow_floats(&json).unwrap();
        assert!(!cbor.is_empty());
    }
}

