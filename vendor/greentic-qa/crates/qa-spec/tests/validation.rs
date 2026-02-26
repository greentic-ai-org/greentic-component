use serde_json::{Value, json};

use qa_spec::spec::form::FormSpec;
use qa_spec::spec::question::{ListSpec, QuestionSpec, QuestionType};
use qa_spec::spec::validation::CrossFieldValidation;
use qa_spec::{
    Expr, VisibilityMap, VisibilityMode, answers_schema, apply_computed_answers, example_answers,
    resolve_visibility, validate,
};

fn channel_field() -> QuestionSpec {
    QuestionSpec {
        id: "name".into(),
        kind: QuestionType::String,
        title: "Channel name".into(),
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
        computed: None,
        policy: Default::default(),
        computed_overridable: false,
    }
}

fn build_channel_form(min_items: Option<usize>, max_items: Option<usize>) -> FormSpec {
    FormSpec {
        id: "channels".into(),
        title: "Channels".into(),
        version: "1.0".into(),
        description: None,
        presentation: None,
        progress_policy: None,
        secrets_policy: None,
        store: vec![],
        validations: vec![],
        includes: vec![],
        questions: vec![QuestionSpec {
            id: "channels".into(),
            kind: QuestionType::List,
            title: "Channels".into(),
            title_i18n: None,
            description: None,
            description_i18n: None,
            required: false,
            choices: None,
            default_value: None,
            secret: false,
            visible_if: None,
            constraint: None,
            list: Some(ListSpec {
                min_items,
                max_items,
                fields: vec![channel_field()],
            }),
            computed: None,
            policy: Default::default(),
            computed_overridable: false,
        }],
    }
}

fn make_simple_form() -> FormSpec {
    FormSpec {
        id: "simple".into(),
        title: "Simple".into(),
        version: "1.0.0".into(),
        description: None,
        presentation: None,
        progress_policy: None,
        secrets_policy: None,
        store: vec![],
        validations: vec![],
        includes: vec![],
        questions: vec![
            QuestionSpec {
                id: "name".into(),
                kind: QuestionType::String,
                title: "Name".into(),
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
                computed: None,
                policy: Default::default(),
                computed_overridable: false,
            },
            QuestionSpec {
                id: "flag".into(),
                kind: QuestionType::Boolean,
                title: "flag".into(),
                title_i18n: None,
                description: None,
                description_i18n: None,
                required: false,
                choices: None,
                default_value: None,
                secret: false,
                visible_if: None,
                constraint: None,
                list: None,
                computed: None,
                policy: Default::default(),
                computed_overridable: false,
            },
        ],
    }
}

#[test]
fn schema_contains_required_properties() {
    let spec = make_simple_form();
    let visibility = resolve_visibility(&spec, &json!({}), VisibilityMode::Visible);
    let schema = answers_schema(&spec, &visibility);
    let props = schema.get("properties").unwrap().as_object().unwrap();
    assert!(props.contains_key("name"));
    assert!(props.contains_key("flag"));
    let required = schema.get("required").unwrap().as_array().unwrap();
    assert!(required.iter().any(|value| value.as_str() == Some("name")));
}

#[test]
fn example_answers_include_questions() {
    let spec = make_simple_form();
    let visibility = VisibilityMap::from([("name".into(), true), ("flag".into(), true)]);
    let examples = example_answers(&spec, &visibility);
    assert_eq!(examples["name"], Value::String("example-name".into()));
    assert_eq!(examples["flag"], Value::Bool(false));
}

#[test]
fn validation_reports_missing() {
    let spec = make_simple_form();
    let answers: Value = json!({});
    let result = validate(&spec, &answers);
    assert!(!result.valid);
    assert_eq!(result.missing_required, vec!["name"]);
}

#[test]
fn list_validation_respects_bounds() {
    let spec = build_channel_form(Some(1), Some(2));

    let too_few = validate(&spec, &json!({ "channels": [] }));
    assert!(!too_few.valid);
    assert_eq!(too_few.errors[0].code.as_deref(), Some("min_items"));

    let too_many = validate(
        &spec,
        &json!({ "channels": [{ "name": "a" }, { "name": "b" }, { "name": "c" }] }),
    );
    assert!(!too_many.valid);
    assert_eq!(too_many.errors[0].code.as_deref(), Some("max_items"));

    let missing_field = validate(&spec, &json!({ "channels": [{}] }));
    assert!(!missing_field.valid);
    assert_eq!(
        missing_field.errors[0].code.as_deref(),
        Some("missing_field")
    );

    let invalid_type = validate(&spec, &json!({ "channels": [{ "name": 123 }] }));
    assert!(!invalid_type.valid);
    assert_eq!(
        invalid_type.errors[0].code.as_deref(),
        Some("type_mismatch")
    );

    let valid = validate(&spec, &json!({ "channels": [{ "name": "alpha" }] }));
    assert!(valid.valid);
}

#[test]
fn list_schema_includes_items() {
    let spec = build_channel_form(Some(1), None);
    let visibility = VisibilityMap::from([("channels".into(), true)]);
    let schema = answers_schema(&spec, &visibility);
    let props = schema
        .get("properties")
        .and_then(Value::as_object)
        .expect("properties map");
    let channels = props
        .get("channels")
        .and_then(Value::as_object)
        .expect("channels schema");
    assert_eq!(channels["minItems"].as_u64(), Some(1));
    let items = channels["items"]
        .as_object()
        .expect("items schema should be object");
    let item_props = items["properties"]
        .as_object()
        .expect("items should describe properties");
    assert!(item_props.contains_key("name"));
}

#[test]
fn computed_fields_satisfy_required_answers() {
    let spec = FormSpec {
        id: "computed".into(),
        title: "Computed Form".into(),
        version: "1.0.0".into(),
        description: None,
        presentation: None,
        progress_policy: None,
        secrets_policy: None,
        store: vec![],
        validations: vec![],
        includes: vec![],
        questions: vec![
            QuestionSpec {
                id: "name".into(),
                kind: QuestionType::String,
                title: "Name".into(),
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
                id: "slug".into(),
                kind: QuestionType::String,
                title: "Slug".into(),
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
                computed: Some(Expr::Answer {
                    path: "name".into(),
                }),
                computed_overridable: false,
            },
        ],
    };

    let answers = json!({ "name": "Greentic" });
    let computed = apply_computed_answers(&spec, &answers);
    assert_eq!(computed["slug"], "Greentic");

    let result = validate(&spec, &answers);
    assert!(result.valid);
}

#[test]
fn computed_field_overwrites_user_values_when_not_overridable() {
    let spec = FormSpec {
        id: "computed_override".into(),
        title: "Computed Override".into(),
        version: "1.0.0".into(),
        description: None,
        presentation: None,
        progress_policy: None,
        secrets_policy: None,
        store: vec![],
        validations: vec![],
        includes: vec![],
        questions: vec![
            QuestionSpec {
                id: "source".into(),
                kind: QuestionType::String,
                title: "Source".into(),
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
                id: "derived".into(),
                kind: QuestionType::String,
                title: "Derived".into(),
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
                computed: Some(Expr::Answer {
                    path: "source".into(),
                }),
                computed_overridable: false,
            },
        ],
    };

    let answers = json!({
        "source": "Greentic",
        "derived": "custom"
    });
    let computed = apply_computed_answers(&spec, &answers);
    assert_eq!(computed["derived"], "Greentic");
}

#[test]
fn computed_field_respects_overrides_when_allowed() {
    let mut spec = FormSpec {
        id: "computed_override".into(),
        title: "Computed Override".into(),
        version: "1.0.0".into(),
        description: None,
        presentation: None,
        progress_policy: None,
        secrets_policy: None,
        store: vec![],
        validations: vec![],
        includes: vec![],
        questions: Vec::new(),
    };
    spec.questions = vec![
        QuestionSpec {
            id: "source".into(),
            kind: QuestionType::String,
            title: "Source".into(),
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
            id: "derived".into(),
            kind: QuestionType::String,
            title: "Derived".into(),
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
            computed: Some(Expr::Answer {
                path: "source".into(),
            }),
            computed_overridable: true,
        },
    ];

    let answers = json!({
        "source": "Greentic",
        "derived": "custom"
    });
    let computed = apply_computed_answers(&spec, &answers);
    assert_eq!(computed["derived"], "custom");
}

#[test]
fn cross_field_validation_fails_when_required_missing() {
    let spec = FormSpec {
        id: "cross".into(),
        title: "Cross Field".into(),
        version: "1.0.0".into(),
        description: None,
        presentation: None,
        progress_policy: None,
        secrets_policy: None,
        store: vec![],
        validations: vec![CrossFieldValidation {
            id: Some("dependent".into()),
            message: "B is required when A is set".into(),
            fields: vec!["b".into()],
            condition: Expr::And {
                expressions: vec![
                    Expr::IsSet { path: "a".into() },
                    Expr::Not {
                        expression: Box::new(Expr::IsSet { path: "b".into() }),
                    },
                ],
            },
            code: Some("missing_dependent".into()),
        }],
        includes: vec![],
        questions: vec![
            QuestionSpec {
                id: "a".into(),
                kind: QuestionType::String,
                title: "A".into(),
                title_i18n: None,
                description: None,
                description_i18n: None,
                required: false,
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
                id: "b".into(),
                kind: QuestionType::String,
                title: "B".into(),
                title_i18n: None,
                description: None,
                description_i18n: None,
                required: false,
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
    };

    let result = validate(&spec, &json!({ "a": "value" }));
    assert!(!result.valid);
    assert_eq!(result.errors.len(), 1);
    assert_eq!(result.errors[0].message, "B is required when A is set");

    let valid_result = validate(&spec, &json!({ "a": "value", "b": "present" }));
    assert!(valid_result.valid);
}

#[test]
fn cross_field_validation_requires_at_least_one_contact() {
    let spec = FormSpec {
        id: "contact".into(),
        title: "Contact".into(),
        version: "1.0.0".into(),
        description: None,
        presentation: None,
        progress_policy: None,
        secrets_policy: None,
        store: vec![],
        validations: vec![CrossFieldValidation {
            id: Some("contact".into()),
            message: "Provide email or phone".into(),
            fields: vec!["email".into(), "phone".into()],
            condition: Expr::And {
                expressions: vec![
                    Expr::Not {
                        expression: Box::new(Expr::IsSet {
                            path: "email".into(),
                        }),
                    },
                    Expr::Not {
                        expression: Box::new(Expr::IsSet {
                            path: "phone".into(),
                        }),
                    },
                ],
            },
            code: Some("contact_required".into()),
        }],
        includes: vec![],
        questions: vec![
            QuestionSpec {
                id: "email".into(),
                kind: QuestionType::String,
                title: "Email".into(),
                title_i18n: None,
                description: None,
                description_i18n: None,
                required: false,
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
                id: "phone".into(),
                kind: QuestionType::String,
                title: "Phone".into(),
                title_i18n: None,
                description: None,
                description_i18n: None,
                required: false,
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
    };

    let result = validate(&spec, &json!({}));
    assert!(!result.valid);
    assert_eq!(result.errors[0].code.as_deref(), Some("contact_required"));

    let valid_result = validate(&spec, &json!({ "email": "engagement@greentic.ai" }));
    assert!(valid_result.valid);
}

#[test]
fn answer_expression_controls_visibility() {
    let spec = FormSpec {
        id: "visibility".into(),
        title: "Visibility".into(),
        version: "1.0.0".into(),
        description: None,
        presentation: None,
        progress_policy: None,
        secrets_policy: None,
        store: vec![],
        validations: vec![],
        includes: vec![],
        questions: vec![
            QuestionSpec {
                id: "trigger".into(),
                kind: QuestionType::Boolean,
                title: "Trigger".into(),
                title_i18n: None,
                description: None,
                description_i18n: None,
                required: false,
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
                id: "dependent".into(),
                kind: QuestionType::String,
                title: "Dependent".into(),
                title_i18n: None,
                description: None,
                description_i18n: None,
                required: false,
                choices: None,
                default_value: None,
                secret: false,
                visible_if: Some(Expr::Answer {
                    path: "trigger".into(),
                }),
                constraint: None,
                list: None,
                policy: Default::default(),
                computed: None,
                computed_overridable: false,
            },
        ],
    };

    let visible = resolve_visibility(&spec, &json!({ "trigger": true }), VisibilityMode::Visible);
    assert!(visible["dependent"]);

    let hidden = resolve_visibility(&spec, &json!({ "trigger": false }), VisibilityMode::Visible);
    assert!(!hidden["dependent"]);
}

#[test]
fn visibility_not_expression_fires_when_trigger_unset() {
    let spec = FormSpec {
        id: "visibility".into(),
        title: "Visibility".into(),
        version: "1.0.0".into(),
        description: None,
        presentation: None,
        progress_policy: None,
        secrets_policy: None,
        store: vec![],
        validations: vec![],
        includes: vec![],
        questions: vec![
            QuestionSpec {
                id: "flag".into(),
                kind: QuestionType::Boolean,
                title: "Flag".into(),
                title_i18n: None,
                description: None,
                description_i18n: None,
                required: false,
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
                id: "dependent".into(),
                kind: QuestionType::String,
                title: "Dependent".into(),
                title_i18n: None,
                description: None,
                description_i18n: None,
                required: false,
                choices: None,
                default_value: None,
                secret: false,
                visible_if: Some(Expr::Not {
                    expression: Box::new(Expr::IsSet {
                        path: "flag".into(),
                    }),
                }),
                constraint: None,
                list: None,
                policy: Default::default(),
                computed: None,
                computed_overridable: false,
            },
        ],
    };

    let visible = resolve_visibility(&spec, &json!({}), VisibilityMode::Visible);
    assert!(visible["dependent"]);

    let hidden = resolve_visibility(&spec, &json!({ "flag": true }), VisibilityMode::Visible);
    assert!(!hidden["dependent"]);
}
