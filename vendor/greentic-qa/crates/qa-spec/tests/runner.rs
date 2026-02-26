use serde_json::json;

use qa_spec::{
    FormSpec, StoreContext, execute_plan_effects, plan_next, plan_submit_all, plan_submit_patch,
};

fn planning_fixture() -> FormSpec {
    serde_json::from_value(json!({
        "id": "runner-form",
        "title": "Runner",
        "version": "1.0",
        "questions": [
            { "id": "q1", "type": "string", "title": "Question 1", "required": true }
        ],
        "store": [
            { "target": "state", "path": "/applied", "value": true }
        ]
    }))
    .expect("fixture should deserialize")
}

#[test]
fn plan_submit_patch_is_pure_and_collects_effects() {
    let spec = planning_fixture();
    let ctx = json!({ "state": {} });
    let answers = json!({});

    let invalid = plan_submit_patch(&spec, &ctx, &answers, "q1", json!(true));
    assert_eq!(invalid.plan_version, 1);
    assert_eq!(invalid.form_id, "runner-form");
    assert!(!invalid.validation.valid);
    assert!(invalid.effects.is_empty());
    assert!(!invalid.errors.is_empty());

    let valid = plan_submit_patch(&spec, &ctx, &answers, "q1", json!("ok"));
    assert!(valid.validation.valid);
    assert_eq!(valid.validated_patch["q1"], "ok");
    assert_eq!(valid.effects.len(), 1);
    assert!(valid.errors.is_empty());
}

#[test]
fn plan_next_builds_a_plan_without_side_effect_application() {
    let spec = planning_fixture();
    let ctx = json!({});
    let next_plan = plan_next(&spec, &ctx, &json!({ "q1": "present" }));
    assert_eq!(next_plan.plan_version, 1);
    assert!(next_plan.validation.valid);
    assert_eq!(next_plan.validated_patch["q1"], "present");
}

#[test]
fn execute_plan_effects_applies_only_for_valid_plan() {
    let spec = planning_fixture();
    let ctx = json!({ "state": {} });

    let invalid_plan = plan_submit_all(&spec, &ctx, &json!({}));
    let mut invalid_store = StoreContext::from_value(&ctx);
    execute_plan_effects(
        &invalid_plan,
        &mut invalid_store,
        spec.secrets_policy.as_ref(),
        false,
    )
    .expect("invalid plan should be a no-op");
    assert!(invalid_store.state.get("applied").is_none());

    let valid_plan = plan_submit_all(&spec, &ctx, &json!({ "q1": "done" }));
    let mut valid_store = StoreContext::from_value(&ctx);
    execute_plan_effects(
        &valid_plan,
        &mut valid_store,
        spec.secrets_policy.as_ref(),
        false,
    )
    .expect("valid plan should apply effects");
    assert_eq!(valid_store.state["applied"], true);
}
