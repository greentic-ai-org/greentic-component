#![cfg(feature = "cli")]

use greentic_component::cmd::wizard::{ExecutionMode, RunMode, WizardArgs, run};
use std::fs;

fn create_answers(path: &std::path::Path, name: &str) {
    let root = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let payload = serde_json::json!({
        "schema": "component-wizard-run/v1",
        "mode": "create",
        "fields": {
            "component_name": name,
            "output_dir": root.join(name),
            "abi_version": "0.6.0"
        }
    });
    fs::write(path, serde_json::to_string_pretty(&payload).unwrap()).unwrap();
}

#[test]
fn wizard_create_execute_creates_template_files() {
    let temp = tempfile::TempDir::new().unwrap();
    let answers_path = temp.path().join("answers.json");
    create_answers(&answers_path, "demo-component");

    let args = WizardArgs {
        legacy_command: None,
        legacy_name: None,
        legacy_out: None,
        mode: RunMode::Create,
        execution: ExecutionMode::Execute,
        dry_run: false,
        qa_answers: Some(answers_path),
        qa_answers_out: None,
        plan_out: None,
        locale: None,
        project_root: temp.path().to_path_buf(),
        template: None,
        full_tests: false,
        json: false,
    };

    run(args).expect("wizard create should succeed");

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

    let cargo_toml = fs::read_to_string(root.join("Cargo.toml")).unwrap();
    assert!(cargo_toml.contains("name = \"demo-component\""));
    assert!(cargo_toml.contains("[package.metadata.greentic]"));
    assert!(cargo_toml.contains("abi_version = \"0.6.0\""));
}

#[test]
fn wizard_create_writes_answers_out_when_requested() {
    let temp = tempfile::TempDir::new().unwrap();
    let answers_path = temp.path().join("answers.json");
    let answers_out = temp.path().join("out/answers.out.json");
    create_answers(&answers_path, "answers-component");

    let args = WizardArgs {
        legacy_command: None,
        legacy_name: None,
        legacy_out: None,
        mode: RunMode::Create,
        execution: ExecutionMode::DryRun,
        dry_run: false,
        qa_answers: Some(answers_path),
        qa_answers_out: Some(answers_out.clone()),
        plan_out: Some(temp.path().join("out/plan.json")),
        locale: None,
        project_root: temp.path().to_path_buf(),
        template: None,
        full_tests: false,
        json: false,
    };

    run(args).expect("wizard dry-run should succeed");
    assert!(answers_out.exists());
}

#[test]
fn wizard_create_dry_run_does_not_write_files() {
    let temp = tempfile::TempDir::new().unwrap();
    let answers_path = temp.path().join("answers.json");
    create_answers(&answers_path, "component");

    let args = WizardArgs {
        legacy_command: None,
        legacy_name: None,
        legacy_out: None,
        mode: RunMode::Create,
        execution: ExecutionMode::DryRun,
        dry_run: false,
        qa_answers: Some(answers_path),
        qa_answers_out: None,
        plan_out: Some(temp.path().join("plan.json")),
        locale: None,
        project_root: temp.path().to_path_buf(),
        template: None,
        full_tests: false,
        json: false,
    };

    run(args).expect("wizard dry-run should succeed");
    let root = temp.path().join("component");
    assert!(
        !root.exists(),
        "dry-run mode should not execute file writes"
    );
}
