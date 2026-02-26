use handlebars::Handlebars;
use serde_json::json;

use qa_spec::spec::form::FormPresentation;
use qa_spec::{
    QuestionSpec, QuestionType, ResolutionMode, TemplateContext, TemplateEngine,
    register_default_helpers,
};

fn build_sample_form() -> qa_spec::FormSpec {
    qa_spec::FormSpec {
        id: "sample".into(),
        title: "{{payload.title}}".into(),
        version: "1.0.0".into(),
        description: Some("desc {{payload.title}}".into()),
        presentation: Some(FormPresentation {
            intro: Some("intro {{answers.name}}".into()),
            theme: Some("theme-default".into()),
            default_locale: None,
        }),
        progress_policy: None,
        secrets_policy: None,
        store: vec![],
        validations: vec![],
        includes: vec![],
        questions: vec![QuestionSpec {
            id: "q1".into(),
            kind: QuestionType::String,
            title: "Name {{answers.name}}".into(),
            title_i18n: None,
            description: Some("desc {{payload.title}}".into()),
            description_i18n: None,
            required: true,
            choices: None,
            default_value: Some("{{default payload.default \"fallback\"}}".into()),
            secret: false,
            visible_if: None,
            constraint: None,
            list: None,
            policy: Default::default(),
            computed: None,
            computed_overridable: false,
        }],
    }
}

#[test]
fn form_spec_resolution_replaces_templates() {
    let engine = TemplateEngine::new(ResolutionMode::Strict);
    let ctx = TemplateContext::default()
        .with_payload(json!({"title": "Wizard", "default": "preset"}))
        .with_answers(json!({"name": "Greentic"}));
    let resolved = engine
        .resolve_form_spec(&build_sample_form(), &ctx)
        .expect("resolve spec");

    assert_eq!(resolved.title, "Wizard");
    assert_eq!(resolved.description.as_deref(), Some("desc Wizard"));
    let presentation = resolved.presentation.expect("presentation exists");
    assert_eq!(presentation.intro, Some("intro Greentic".into()));
    let question = &resolved.questions[0];
    assert_eq!(question.title, "Name Greentic");
    assert_eq!(question.default_value.as_deref(), Some("preset"));
}

#[test]
fn resolve_string_relaxed_keeps_missing_tokens() {
    let engine = TemplateEngine::new(ResolutionMode::Relaxed);
    let ctx = TemplateContext::default();
    let resolved = engine
        .resolve_form_spec(
            &qa_spec::FormSpec {
                id: "missing".into(),
                title: "{{payload.missing}}".into(),
                version: "1.0".into(),
                description: None,
                presentation: None,
                progress_policy: None,
                secrets_policy: None,
                store: vec![],
                validations: vec![],
                includes: vec![],
                questions: vec![],
            },
            &ctx,
        )
        .expect("resolve");
    assert_eq!(resolved.title, "{{payload.missing}}");
}

#[test]
fn default_helper_prefers_truthy_values() {
    let mut handlebars = Handlebars::new();
    register_default_helpers(&mut handlebars);
    let context = json!({"payload": {"name": "Greentic"}});
    let rendered = handlebars
        .render_template("{{default payload.name \"fallback\"}}", &context)
        .expect("rendered");
    assert_eq!(rendered, "Greentic");
}
