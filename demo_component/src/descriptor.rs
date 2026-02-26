use std::collections::BTreeMap;

use greentic_types::cbor::canonical;
use greentic_types::schemas::component::v0_6_0::{
    ComponentDescribe, ComponentInfo, ComponentOperation, ComponentRunInput, ComponentRunOutput,
    RedactionRule, RedactionKind, schema_hash,
};

use crate::schema;

pub fn info() -> ComponentInfo {
    ComponentInfo {
        id: "com.example.demo_component".to_string(),
        version: "0.1.0".to_string(),
        role: "tool".to_string(),
        display_name: None,
    }
}

pub fn info_cbor() -> Vec<u8> {
    canonical::to_canonical_cbor_allow_floats(&info()).unwrap_or_default()
}

pub fn describe() -> ComponentDescribe {
    let input_schema = schema::input_schema();
    let output_schema = schema::output_schema();
    let config_schema = schema::config_schema();
    let op_hash = schema_hash(&input_schema, &output_schema, &config_schema)
        .expect("schema hash");
    let operation = ComponentOperation {
        id: "run".to_string(),
        display_name: None,
        input: ComponentRunInput { schema: input_schema },
        output: ComponentRunOutput { schema: output_schema },
        defaults: BTreeMap::new(),
        redactions: vec![RedactionRule {
            json_pointer: "/secret".to_string(),
            kind: RedactionKind::Secret,
        }],
        constraints: BTreeMap::new(),
        schema_hash: op_hash,
    };
    ComponentDescribe {
        info: info(),
        provided_capabilities: provided_capabilities(),
        required_capabilities: required_capabilities(),
        metadata: BTreeMap::new(),
        operations: vec![operation],
        config_schema,
    }
}

fn required_capabilities() -> Vec<String> {
    const REQUIRED_CAPABILITIES: &[&str] = &[];
    REQUIRED_CAPABILITIES
        .iter()
        .map(|capability| (*capability).to_string())
        .collect()
}

fn provided_capabilities() -> Vec<String> {
    const PROVIDED_CAPABILITIES: &[&str] = &[];
    PROVIDED_CAPABILITIES
        .iter()
        .map(|capability| (*capability).to_string())
        .collect()
}

pub fn describe_cbor() -> Vec<u8> {
    canonical::to_canonical_cbor_allow_floats(&describe()).unwrap_or_default()
}
