use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

use crate::secrets::{SecretAccessResult, SecretAction, evaluate};
use crate::spec::form::SecretsPolicy;

/// Targets that store operations can write into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StoreTarget {
    Answers,
    State,
    Config,
    PayloadOut,
    Secrets,
}

/// Single store operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StoreOp {
    pub target: StoreTarget,
    pub path: String,
    pub value: Value,
}

/// Context mutated by store operations.
#[derive(Debug, Clone)]
pub struct StoreContext {
    pub answers: Value,
    pub state: Value,
    pub config: Value,
    pub payload_out: Value,
    pub secrets: Value,
}

impl StoreContext {
    pub fn from_value(ctx: &Value) -> Self {
        let default = || Value::Object(Map::new());
        Self {
            answers: ctx.get("answers").cloned().unwrap_or_else(default),
            state: ctx.get("state").cloned().unwrap_or_else(default),
            config: ctx.get("config").cloned().unwrap_or_else(default),
            payload_out: ctx.get("payload_out").cloned().unwrap_or_else(default),
            secrets: ctx.get("secrets").cloned().unwrap_or_else(default),
        }
    }

    pub fn apply_ops(
        &mut self,
        ops: &[StoreOp],
        policy: Option<&SecretsPolicy>,
        host_available: bool,
    ) -> Result<(), StoreError> {
        for op in ops {
            match op.target {
                StoreTarget::Answers => set_path(&mut self.answers, &op.path, op.value.clone())?,
                StoreTarget::State => set_path(&mut self.state, &op.path, op.value.clone())?,
                StoreTarget::Config => set_path(&mut self.config, &op.path, op.value.clone())?,
                StoreTarget::PayloadOut => {
                    set_path(&mut self.payload_out, &op.path, op.value.clone())?
                }
                StoreTarget::Secrets => {
                    let key = secret_key(&op.path)?;
                    match evaluate(policy, &key, SecretAction::Write, host_available) {
                        SecretAccessResult::Allowed => {
                            set_path(&mut self.secrets, &op.path, op.value.clone())?;
                        }
                        SecretAccessResult::Denied(code) => {
                            return Err(StoreError::SecretAccessDenied { key, code });
                        }
                        SecretAccessResult::HostUnavailable => {
                            return Err(StoreError::SecretHostUnavailable);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn to_value(&self) -> Value {
        let mut map = Map::new();
        map.insert("answers".into(), self.answers.clone());
        map.insert("state".into(), self.state.clone());
        map.insert("config".into(), self.config.clone());
        map.insert("payload_out".into(), self.payload_out.clone());
        map.insert("secrets".into(), self.secrets.clone());
        Value::Object(map)
    }
}

/// Errors raised while applying store operations.
#[derive(Debug, Error)]
pub enum StoreError {
    #[error("invalid pointer '{0}'")]
    InvalidPointer(String),
    #[error("secret access denied for '{key}' ({code})")]
    SecretAccessDenied { key: String, code: &'static str },
    #[error("secret host unavailable")]
    SecretHostUnavailable,
}

fn set_path(root: &mut Value, pointer: &str, value: Value) -> Result<(), StoreError> {
    if pointer.is_empty() {
        *root = value;
        return Ok(());
    }

    let segments = pointer
        .trim_start_matches('/')
        .split('/')
        .map(decode_segment)
        .collect::<Vec<_>>();

    let mut current = root;
    for (idx, segment) in segments.iter().enumerate() {
        if idx + 1 == segments.len() {
            ensure_object(current).insert(segment.clone(), value);
            return Ok(());
        }
        current = ensure_object(current)
            .entry(segment.clone())
            .or_insert_with(|| Value::Object(Map::new()));
    }

    Err(StoreError::InvalidPointer(pointer.to_string()))
}

fn ensure_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
    value.as_object_mut().expect("value is object")
}

fn decode_segment(segment: &str) -> String {
    segment.replace("~1", "/").replace("~0", "~")
}

fn secret_key(pointer: &str) -> Result<String, StoreError> {
    let trimmed = pointer.trim_start_matches('/');
    if trimmed.is_empty() {
        return Err(StoreError::InvalidPointer(pointer.to_string()));
    }
    Ok(trimmed.to_string())
}
