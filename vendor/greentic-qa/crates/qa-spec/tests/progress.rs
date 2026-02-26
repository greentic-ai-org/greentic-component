use serde_json::json;

use qa_spec::{
    ProgressContext, StoreTarget, VisibilityMode, next_question, resolve_visibility,
    spec::form::{FormSpec, ProgressPolicy},
    spec::question::{QuestionSpec, QuestionType},
};

fn build_progress_form() -> FormSpec {
    FormSpec {
        id: "flow".into(),
        title: "Flow".into(),
        version: "1.0".into(),
        description: None,
        presentation: None,
        progress_policy: Some(ProgressPolicy {
            skip_answered: true,
            autofill_defaults: false,
            treat_default_as_answered: false,
        }),
        secrets_policy: None,
        store: vec![],
        validations: vec![],
        includes: vec![],
        questions: vec![
            QuestionSpec {
                id: "q1".into(),
                kind: QuestionType::String,
                title: "First".into(),
                title_i18n: None,
                description: None,
                description_i18n: None,
                required: true,
                choices: None,
                default_value: None,
                secret: false,
                visible_if: None,
                constraint: None,
                list: None,
                policy: Default::default(),
                computed: None,
                computed_overridable: false,
            },
            QuestionSpec {
                id: "q2".into(),
                kind: QuestionType::String,
                title: "Second".into(),
                title_i18n: None,
                description: None,
                description_i18n: None,
                required: true,
                choices: None,
                default_value: None,
                secret: false,
                visible_if: None,
                constraint: None,
                list: None,
                policy: Default::default(),
                computed: None,
                computed_overridable: false,
            },
        ],
    }
}

#[test]
fn next_question_skips_when_config_value_present() {
    let mut spec = build_progress_form();
    spec.questions[0].policy.skip_if_present_in = vec![StoreTarget::Config];
    let answers = json!({});
    let ctx = json!({ "config": { "q1": "preset" } });
    let visibility = resolve_visibility(&spec, &answers, VisibilityMode::Visible);
    let progress_ctx = ProgressContext::new(answers.clone(), &ctx);
    assert_eq!(
        next_question(&spec, &progress_ctx, &visibility),
        Some("q2".into())
    );
}

#[test]
fn next_question_skips_answered() {
    let spec = build_progress_form();
    let answers = json!({ "q1": "value" });
    let ctx = json!({});
    let visibility = resolve_visibility(&spec, &answers, VisibilityMode::Visible);
    let progress_ctx = ProgressContext::new(answers.clone(), &ctx);
    assert_eq!(
        next_question(&spec, &progress_ctx, &visibility),
        Some("q2".into())
    );
}

#[test]
fn default_progress_policy_skips_answered() {
    let mut spec = build_progress_form();
    spec.progress_policy = None;
    let answers = json!({ "q1": "value" });
    let ctx = json!({});
    let visibility = resolve_visibility(&spec, &answers, VisibilityMode::Visible);
    let progress_ctx = ProgressContext::new(answers.clone(), &ctx);
    assert_eq!(
        next_question(&spec, &progress_ctx, &visibility),
        Some("q2".into())
    );
}
