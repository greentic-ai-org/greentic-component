use crate::expr::Expr;
use crate::i18n::I18nText;
use crate::store::StoreTarget;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Supported question data types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QuestionType {
    String,
    Boolean,
    Integer,
    Number,
    Enum,
    List,
}

/// Constraints that can be enforced per question.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Constraint {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_len: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_len: Option<usize>,
}

/// Definition of a single question inside a form.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct QuestionSpec {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: QuestionType,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_i18n: Option<I18nText>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description_i18n: Option<I18nText>,
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choices: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,
    #[serde(default)]
    pub secret: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible_if: Option<Expr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constraint: Option<Constraint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list: Option<ListSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub computed: Option<Expr>,
    #[serde(default)]
    pub policy: QuestionPolicy,
    #[serde(default)]
    pub computed_overridable: bool,
}

/// Per-question overrides for progress behavior.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
pub struct QuestionPolicy {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skip_if_present_in: Vec<StoreTarget>,
    #[serde(default)]
    pub editable_if_from_default: bool,
}

/// Definition of a repeatable list whose entries reuse question definitions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
pub struct ListSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_items: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_items: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<QuestionSpec>,
}
