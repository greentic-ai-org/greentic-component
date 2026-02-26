use greentic_qa_lib::{I18nConfig, WizardDriver, WizardFrontend, WizardRunConfig};
use serde_json::{Value, json};

#[test]
fn completes_after_all_required_answers_are_present() {
    // Minimal shape matching operator wizard semantics.
    let spec = json!({
        "id": "operator.wizard.create",
        "title": "Create bundle",
        "version": "1.0.0",
        "presentation": { "default_locale": "en-GB" },
        "questions": [
            { "id": "bundle_path", "type": "string", "title": "Bundle output path", "required": true },
            { "id": "bundle_name", "type": "string", "title": "Bundle name", "required": true },
            {
                "id": "targets",
                "type": "list",
                "title": "Tenants and teams",
                "required": true,
                "list": { "fields": [
                    { "id": "tenant_id", "type": "string", "title": "Tenant id", "required": true },
                    { "id": "team_id", "type": "string", "title": "Team id", "required": false }
                ]}
            },
            {
                "id": "access_mode",
                "type": "enum",
                "title": "Access mode",
                "required": true,
                "choices": ["all_selected_get_all_packs", "per_pack_matrix"]
            },
            {
                "id": "execution_mode",
                "type": "enum",
                "title": "Execution mode",
                "required": true,
                "choices": ["dry run", "execute"]
            }
        ]
    });

    // Mirrors operator prefill behavior.
    let initial_answers = json!({
        "bundle_path": "/tmp/repro-bundle",
        "targets": [{ "tenant_id": "demo" }]
    });

    let mut driver = WizardDriver::new(WizardRunConfig {
        spec_json: spec.to_string(),
        initial_answers_json: Some(initial_answers.to_string()),
        frontend: WizardFrontend::JsonUi,
        i18n: I18nConfig {
            locale: Some("en-GB".into()),
            resolved: None,
            debug: false,
        },
        verbose: false,
    })
    .expect("driver should be created");

    // First render should request more input.
    let _ = driver.next_payload_json().expect("first payload");
    assert!(!driver.is_complete(), "should need additional answers");

    // Submit all remaining required answers in one patch.
    let patch = json!({
        "bundle_name": "wiz temp",
        "access_mode": "all_selected_get_all_packs",
        "execution_mode": "execute"
    });
    let submit = driver
        .submit_patch_json(&patch.to_string())
        .expect("submit should not crash");

    // Repro symptom: accepted but still loops on need_input with already-answered fields.
    assert_ne!(
        submit.status, "error",
        "validation unexpectedly failed: {}",
        submit.response_json
    );

    let ui_raw = driver.next_payload_json().expect("next payload");
    let ui: Value = serde_json::from_str(&ui_raw).expect("parse ui payload");

    // Expected behavior: once all required fields are present, wizard should be complete.
    assert!(
        driver.is_complete() || ui.get("status").and_then(Value::as_str) == Some("complete"),
        "expected complete, got ui={}",
        ui
    );
}
