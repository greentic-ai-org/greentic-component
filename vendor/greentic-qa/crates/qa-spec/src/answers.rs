use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_cbor::{to_vec, value::to_value};
use serde_json::Value;

/// Optional metadata paired with an `AnswerSet`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Meta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// Represents in-progress answers for a given form spec version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AnswerSet {
    pub form_id: String,
    pub spec_version: String,
    pub answers: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

impl AnswerSet {
    /// Creates a fresh empty answer set for a form.
    pub fn new(form_id: impl Into<String>, spec_version: impl Into<String>) -> Self {
        Self {
            form_id: form_id.into(),
            spec_version: spec_version.into(),
            answers: Value::Object(Default::default()),
            meta: None,
        }
    }

    /// Serializes the answers set as canonical CBOR bytes.
    pub fn to_cbor(&self) -> Result<Vec<u8>, serde_cbor::Error> {
        let canonical = to_value(self)?;
        to_vec(&canonical)
    }

    /// Serializes the answers set as indented JSON for debugging.
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// Progress tracking state for flows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ProgressState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_step: Option<String>,
    pub completed: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<String>,
}

/// Validation error metadata reported by the engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ValidationError {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// Result returned from `validate_answers`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ValidationResult {
    pub valid: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<ValidationError>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_required: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unknown_fields: Vec<String>,
}
