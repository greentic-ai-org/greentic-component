use std::collections::BTreeMap;

use greentic_types::cbor::canonical;
use serde_json::Value as JsonValue;

pub fn run(input: Vec<u8>, state: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
    let input_map = decode_map(&input);
    let message = input_map
        .get("message")
        .and_then(|value| value.as_str())
        .unwrap_or("ok");
    let mut output = BTreeMap::new();
    output.insert(
        "result".to_string(),
        JsonValue::String(format!("processed: {message}")),
    );
    let output_cbor = canonical::to_canonical_cbor_allow_floats(&output).unwrap_or_default();
    let state_cbor = canonicalize_or_empty(&state);
    (output_cbor, state_cbor)
}

fn canonicalize_or_empty(bytes: &[u8]) -> Vec<u8> {
    let empty = || {
        canonical::to_canonical_cbor_allow_floats(&BTreeMap::<String, JsonValue>::new())
            .unwrap_or_default()
    };
    if bytes.is_empty() {
        return empty();
    }
    let value: JsonValue = match canonical::from_cbor(bytes) {
        Ok(value) => value,
        Err(_) => return empty(),
    };
    canonical::to_canonical_cbor_allow_floats(&value).unwrap_or_default()
}

fn decode_map(bytes: &[u8]) -> BTreeMap<String, JsonValue> {
    if bytes.is_empty() {
        return BTreeMap::new();
    }
    let value: JsonValue = match canonical::from_cbor(bytes) {
        Ok(value) => value,
        Err(_) => return BTreeMap::new(),
    };
    let JsonValue::Object(map) = value else {
        return BTreeMap::new();
    };
    map.into_iter().collect()
}
