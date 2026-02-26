use std::collections::BTreeMap;

use greentic_types::cbor::canonical;
use greentic_types::schemas::common::schema_ir::{AdditionalProperties, SchemaIr};

pub fn input_schema() -> SchemaIr {
    object_schema(vec![(
        "message",
        SchemaIr::String {
            min_len: Some(1),
            max_len: Some(1024),
            regex: None,
            format: None,
        },
    )])
}

pub fn output_schema() -> SchemaIr {
    object_schema(vec![(
        "result",
        SchemaIr::String {
            min_len: Some(1),
            max_len: Some(1024),
            regex: None,
            format: None,
        },
    )])
}

pub fn config_schema() -> SchemaIr {
    object_schema(vec![("enabled", SchemaIr::Bool)])
}

pub fn input_schema_cbor() -> Vec<u8> {
    canonical::to_canonical_cbor_allow_floats(&input_schema()).unwrap_or_default()
}

pub fn output_schema_cbor() -> Vec<u8> {
    canonical::to_canonical_cbor_allow_floats(&output_schema()).unwrap_or_default()
}

pub fn config_schema_cbor() -> Vec<u8> {
    canonical::to_canonical_cbor_allow_floats(&config_schema()).unwrap_or_default()
}

fn object_schema(props: Vec<(&str, SchemaIr)>) -> SchemaIr {
    let mut properties = BTreeMap::new();
    let mut required = Vec::new();
    for (name, schema) in props {
        properties.insert(name.to_string(), schema);
        required.push(name.to_string());
    }
    SchemaIr::Object {
        properties,
        required,
        additional: AdditionalProperties::Forbid,
    }
}
