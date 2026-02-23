#![cfg(feature = "cli")]

use greentic_component::cmd::wizard::{WizardCommand, WizardMode, WizardNewArgs, run};
use std::fs;

#[test]
fn wizard_new_creates_template_files() {
    let temp = tempfile::TempDir::new().unwrap();
    let args = WizardNewArgs {
        name: "demo-component".into(),
        abi_version: "0.6.0".into(),
        mode: WizardMode::Default,
        answers: None,
        out: Some(temp.path().to_path_buf()),
        required_capabilities: Vec::new(),
        provided_capabilities: Vec::new(),
        plan_json: false,
    };

    run(WizardCommand::New(args)).expect("wizard new should succeed");

    let root = temp.path().join("demo-component");
    assert!(root.join("Cargo.toml").exists());
    assert!(root.join("src/lib.rs").exists());
    assert!(root.join("src/descriptor.rs").exists());
    assert!(root.join("src/schema.rs").exists());
    assert!(root.join("src/runtime.rs").exists());
    assert!(root.join("Makefile").exists());
    assert!(root.join("src/qa.rs").exists());
    assert!(root.join("src/i18n.rs").exists());
    assert!(root.join("assets/i18n/en.json").exists());
    assert!(!root.join("examples/default.answers.json").exists());

    let cargo_toml = fs::read_to_string(root.join("Cargo.toml")).unwrap();
    assert!(cargo_toml.contains("name = \"demo-component\""));
    assert!(cargo_toml.contains("[package.metadata.greentic]"));
    assert!(cargo_toml.contains("abi_version = \"0.6.0\""));

    let makefile = fs::read_to_string(root.join("Makefile")).unwrap();
    assert!(makefile.contains("WASM_OUT := $(DIST_DIR)/$(NAME)__$(ABI_VERSION_UNDERSCORE).wasm"));

    let lib_rs = fs::read_to_string(root.join("src/lib.rs")).unwrap();
    assert!(lib_rs.contains("greentic_interfaces_guest::export_component_v060!(Component);"));
}

#[test]
fn wizard_new_writes_answers_files_when_provided() {
    let temp = tempfile::TempDir::new().unwrap();
    let answers_path = temp.path().join("answers.json");
    fs::write(&answers_path, r#"{"enabled": true}"#).unwrap();
    let args = WizardNewArgs {
        name: "answers-component".into(),
        abi_version: "0.6.0".into(),
        mode: WizardMode::Default,
        answers: Some(answers_path),
        out: Some(temp.path().to_path_buf()),
        required_capabilities: Vec::new(),
        provided_capabilities: Vec::new(),
        plan_json: false,
    };

    run(WizardCommand::New(args)).expect("wizard new should succeed");

    let root = temp.path().join("answers-component");
    let json_path = root.join("examples/default.answers.json");
    let cbor_path = root.join("examples/default.answers.cbor");
    assert!(json_path.exists());
    assert!(cbor_path.exists());
    let json = fs::read_to_string(json_path).unwrap();
    assert!(json.contains("\"enabled\""));
    assert!(!root.join("examples/setup.answers.json").exists());
}

#[test]
fn wizard_new_embeds_declared_capabilities_in_descriptor() {
    let temp = tempfile::TempDir::new().unwrap();
    let args = WizardNewArgs {
        name: "cap-component".into(),
        abi_version: "0.6.0".into(),
        mode: WizardMode::Default,
        answers: None,
        out: Some(temp.path().to_path_buf()),
        required_capabilities: vec![
            "host.http.client".into(),
            "host.secrets.required".into(),
            "host.http.client".into(),
        ],
        provided_capabilities: vec!["telemetry.emit".into()],
        plan_json: false,
    };

    run(WizardCommand::New(args)).expect("wizard new should succeed");

    let root = temp.path().join("cap-component");
    let descriptor = fs::read_to_string(root.join("src/descriptor.rs")).unwrap();
    assert!(descriptor.contains(
        "const REQUIRED_CAPABILITIES: &[&str] = &[\"host.http.client\", \"host.secrets.required\"];"
    ));
    assert!(descriptor.contains("const PROVIDED_CAPABILITIES: &[&str] = &[\"telemetry.emit\"];"));
}

#[test]
fn wizard_new_qa_apply_answers_enforces_mode_contracts() {
    let temp = tempfile::TempDir::new().unwrap();
    let args = WizardNewArgs {
        name: "qa-contract-component".into(),
        abi_version: "0.6.0".into(),
        mode: WizardMode::Default,
        answers: None,
        out: Some(temp.path().to_path_buf()),
        required_capabilities: Vec::new(),
        provided_capabilities: Vec::new(),
        plan_json: false,
    };

    run(WizardCommand::New(args)).expect("wizard new should succeed");

    let root = temp.path().join("qa-contract-component");
    let qa_rs = fs::read_to_string(root.join("src/qa.rs")).unwrap();

    assert!(qa_rs.contains("Mode::Default | Mode::Setup | Mode::Update"));
    assert!(qa_rs.contains(".entry(\"enabled\".to_string())"));
    assert!(qa_rs.contains(".or_insert(JsonValue::Bool(true));"));
    assert!(qa_rs.contains("Mode::Remove => {"));
    assert!(qa_rs.contains("config.insert(\"enabled\".to_string(), JsonValue::Bool(false));"));
}

#[test]
fn wizard_new_maps_namespaced_answers_to_enabled_and_kind() {
    let temp = tempfile::TempDir::new().unwrap();
    let answers_path = temp.path().join("answers.json");
    fs::write(
        &answers_path,
        r#"{"component.features.enabled": false, "component.kind": "source"}"#,
    )
    .unwrap();
    let args = WizardNewArgs {
        name: "namespaced-component".into(),
        abi_version: "0.6.0".into(),
        mode: WizardMode::Default,
        answers: Some(answers_path),
        out: Some(temp.path().to_path_buf()),
        required_capabilities: Vec::new(),
        provided_capabilities: Vec::new(),
        plan_json: false,
    };

    run(WizardCommand::New(args)).expect("wizard new should succeed");

    let root = temp.path().join("namespaced-component");
    let mapped = fs::read_to_string(root.join("examples/default.answers.json")).unwrap();
    assert!(mapped.contains("\"enabled\": false"));

    let descriptor = fs::read_to_string(root.join("src/descriptor.rs")).unwrap();
    assert!(descriptor.contains("role: \"source\".to_string()"));
}

#[test]
fn wizard_new_plan_json_does_not_write_files() {
    let temp = tempfile::TempDir::new().unwrap();
    let args = WizardNewArgs {
        name: "dry-run-component".into(),
        abi_version: "0.6.0".into(),
        mode: WizardMode::Default,
        answers: None,
        out: Some(temp.path().to_path_buf()),
        required_capabilities: Vec::new(),
        provided_capabilities: Vec::new(),
        plan_json: true,
    };

    run(WizardCommand::New(args)).expect("wizard plan-json should succeed");
    let root = temp.path().join("dry-run-component");
    assert!(
        !root.exists(),
        "plan-json mode should not execute file writes"
    );
}
