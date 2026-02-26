use schemars::schema_for;
use serde_json::Value;

use qa_spec::{FormSpec, QAFlowSpec};

fn fixture(name: &str) -> &'static str {
    match name {
        "simple_form" => include_str!("../tests/fixtures/simple_form.json"),
        "graph_flow" => include_str!("../tests/fixtures/graph_flow.json"),
        _ => panic!("unknown fixture {}", name),
    }
}

#[test]
fn form_spec_roundtrip() {
    let raw = fixture("simple_form");
    let parsed: FormSpec = serde_json::from_str(raw).expect("deserialize");
    let serialized = serde_json::to_string_pretty(&parsed).expect("serialize");
    let re_parsed: FormSpec = serde_json::from_str(&serialized).expect("roundtrip");
    assert_eq!(parsed, re_parsed);
}

#[test]
fn qa_flow_roundtrip() {
    let raw = fixture("graph_flow");
    let parsed: QAFlowSpec = serde_json::from_str(raw).expect("deserialize");
    let serialized = serde_json::to_string_pretty(&parsed).expect("serialize");
    let re_parsed: QAFlowSpec = serde_json::from_str(&serialized).expect("roundtrip");
    assert_eq!(parsed, re_parsed);
}

#[test]
fn schemas_compile() {
    let form_schema = schema_for!(FormSpec);
    let flow_schema = schema_for!(QAFlowSpec);
    let form_json = serde_json::to_string(&form_schema).expect("form schema serializes");
    let flow_json = serde_json::to_string(&flow_schema).expect("flow schema serializes");
    assert!(form_json.starts_with('{'));
    assert!(flow_json.starts_with('{'));
}

#[test]
fn expr_knows_visibility() {
    use qa_spec::Expr;

    let ctx: Value = serde_json::json!({
        "answers": {
            "name": "-",
            "flag": true
        }
    });
    let expr = Expr::Eq {
        left: Box::new(Expr::Var {
            path: "/answers/flag".into(),
        }),
        right: Box::new(Expr::Var {
            path: "/answers/flag".into(),
        }),
    };
    assert_eq!(expr.evaluate_bool(&ctx), Some(true));
}
