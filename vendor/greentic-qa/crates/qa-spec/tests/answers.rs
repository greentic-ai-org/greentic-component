use serde_cbor::from_slice;
use serde_json::json;

use qa_spec::{AnswerSet, answers::Meta};

#[test]
fn answer_set_serializes_to_cbor_and_json() {
    let answer_set = AnswerSet {
        form_id: "sample-form".into(),
        spec_version: "1.0.0".into(),
        answers: json!({
            "name": "Greentic",
            "flag": true,
            "nested": {
                "count": 3
            }
        }),
        meta: None,
    };

    let cbor = answer_set.to_cbor().expect("cbor serialization succeeds");
    assert!(!cbor.is_empty());

    let pretty = answer_set
        .to_json_pretty()
        .expect("json serialization succeeds");
    assert!(pretty.contains("\"form_id\""));
    assert!(pretty.contains("\"spec_version\""));
}

#[test]
fn answer_set_cbor_roundtrip_preserves_fields() {
    let answer_set = AnswerSet {
        form_id: "sample-form".into(),
        spec_version: "1.0.0".into(),
        answers: json!({
            "name": "Greentic",
            "flag": true,
            "nested": {
                "count": 3
            }
        }),
        meta: Some(Meta {
            created_at: Some("2026-01-01T00:00:00Z".into()),
            updated_at: None,
        }),
    };

    let cbor = answer_set.to_cbor().expect("cbor serialization succeeds");
    let decoded: AnswerSet = from_slice(&cbor).expect("cbor roundtrip succeeds");
    assert_eq!(decoded, answer_set);
}
