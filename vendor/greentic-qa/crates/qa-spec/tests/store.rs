use serde_json::json;

use qa_spec::spec::form::SecretsPolicy;
use qa_spec::{StoreContext, StoreOp, StoreTarget};

#[test]
fn store_applies_state_target() {
    let ctx = json!({ "state": {} });
    let mut store_ctx = StoreContext::from_value(&ctx);
    let op = StoreOp {
        target: StoreTarget::State,
        path: "/flag".into(),
        value: json!(true),
    };
    store_ctx.apply_ops(&[op], None, false).expect("apply ops");
    let updated = store_ctx.to_value();
    assert_eq!(updated["state"]["flag"], true);
}

#[test]
fn store_rejects_secret_without_host() {
    let ctx = json!({ "secrets": {} });
    let mut store_ctx = StoreContext::from_value(&ctx);
    let op = StoreOp {
        target: StoreTarget::Secrets,
        path: "/aws/secret".into(),
        value: json!("value"),
    };
    let policy = SecretsPolicy {
        enabled: true,
        read_enabled: true,
        write_enabled: true,
        allow: vec!["aws/*".into()],
        deny: vec![],
    };
    let err = store_ctx
        .apply_ops(&[op], Some(&policy), false)
        .expect_err("host unavailable");
    assert!(matches!(err, qa_spec::StoreError::SecretHostUnavailable));
}

#[test]
fn store_applies_secret_when_allowed() {
    let ctx = json!({ "secrets": {} });
    let mut store_ctx = StoreContext::from_value(&ctx);
    let op = StoreOp {
        target: StoreTarget::Secrets,
        path: "/aws/secret".into(),
        value: json!("value"),
    };
    let policy = SecretsPolicy {
        enabled: true,
        read_enabled: true,
        write_enabled: true,
        allow: vec!["aws/*".into()],
        deny: vec![],
    };
    store_ctx
        .apply_ops(&[op], Some(&policy), true)
        .expect("apply secret");
    let updated = store_ctx.to_value();
    assert_eq!(updated["secrets"]["aws"]["secret"], "value");
}
