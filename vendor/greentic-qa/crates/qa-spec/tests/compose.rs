use std::collections::BTreeMap;

use qa_spec::{Expr, FormSpec, IncludeSpec, QuestionSpec, QuestionType, expand_includes};

fn question(id: &str) -> QuestionSpec {
    QuestionSpec {
        id: id.into(),
        kind: QuestionType::String,
        title: id.into(),
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

fn form(id: &str, questions: Vec<QuestionSpec>) -> FormSpec {
    FormSpec {
        id: id.into(),
        title: id.into(),
        version: "1.0".into(),
        description: None,
        presentation: None,
        progress_policy: None,
        secrets_policy: None,
        store: vec![],
        validations: vec![],
        includes: vec![],
        questions,
    }
}

#[test]
fn expands_include_with_prefix_in_stable_order() {
    let mut parent = form("parent", vec![question("root")]);
    parent.includes = vec![IncludeSpec {
        form_ref: "child-form".into(),
        prefix: Some("child".into()),
    }];
    let child = form("child", vec![question("q1"), question("q2")]);
    let registry = BTreeMap::from([("child-form".into(), child)]);

    let expanded = expand_includes(&parent, &registry).expect("expansion should succeed");
    let ids = expanded
        .questions
        .iter()
        .map(|q| q.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["root", "child.q1", "child.q2"]);
}

#[test]
fn include_cycle_is_reported() {
    let mut a = form("a", vec![question("a1")]);
    a.includes = vec![IncludeSpec {
        form_ref: "b-ref".into(),
        prefix: Some("b".into()),
    }];

    let mut b = form("b", vec![question("b1")]);
    b.includes = vec![IncludeSpec {
        form_ref: "a-ref".into(),
        prefix: Some("a".into()),
    }];

    let registry = BTreeMap::from([("b-ref".into(), b), ("a-ref".into(), a.clone())]);
    let err = expand_includes(&a, &registry).expect_err("cycle should fail");
    assert!(err.to_string().contains("include cycle detected"));
}

#[test]
fn missing_include_target_is_reported() {
    let mut parent = form("parent", vec![question("q1")]);
    parent.includes = vec![IncludeSpec {
        form_ref: "missing".into(),
        prefix: Some("x".into()),
    }];
    let registry: BTreeMap<String, FormSpec> = BTreeMap::new();

    let err = expand_includes(&parent, &registry).expect_err("missing include should fail");
    assert!(err.to_string().contains("missing include target"));
}

#[test]
fn prefixed_expression_paths_follow_question_namespace() {
    let mut child_question = question("gate");
    child_question.visible_if = Some(Expr::Answer {
        path: "flag".into(),
    });
    let child = form("child", vec![child_question]);

    let mut parent = form("parent", vec![question("flag")]);
    parent.includes = vec![IncludeSpec {
        form_ref: "child-form".into(),
        prefix: Some("child".into()),
    }];

    let registry = BTreeMap::from([("child-form".into(), child)]);
    let expanded = expand_includes(&parent, &registry).expect("expansion should succeed");
    let gate = expanded
        .questions
        .iter()
        .find(|question| question.id == "child.gate")
        .expect("prefixed gate question should exist");

    assert_eq!(
        gate.visible_if,
        Some(Expr::Answer {
            path: "child.flag".into()
        })
    );
}
