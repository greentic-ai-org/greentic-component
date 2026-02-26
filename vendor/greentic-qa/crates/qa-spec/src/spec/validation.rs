use crate::expr::Expr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Cross-question validation rules expressed as reusable conditions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CrossFieldValidation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<String>,
    pub condition: Expr,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}
