use crate::spec::question::QuestionSpec;
use crate::spec::validation::CrossFieldValidation;
use crate::store::StoreOp;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Presentation hints for a form.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FormPresentation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intro: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_locale: Option<String>,
}

/// Execution policies shared by question navigation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ProgressPolicy {
    #[serde(default)]
    pub skip_answered: bool,
    #[serde(default)]
    pub autofill_defaults: bool,
    #[serde(default)]
    pub treat_default_as_answered: bool,
}

impl Default for ProgressPolicy {
    fn default() -> Self {
        Self {
            skip_answered: true,
            autofill_defaults: false,
            treat_default_as_answered: false,
        }
    }
}

/// Secrets policy for the form.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SecretsPolicy {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub read_enabled: bool,
    #[serde(default)]
    pub write_enabled: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allow: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deny: Vec<String>,
}

/// Include reference for composing forms from a registry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct IncludeSpec {
    pub form_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
}

/// Top-level QA form definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FormSpec {
    pub id: String,
    pub title: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presentation: Option<FormPresentation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress_policy: Option<ProgressPolicy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets_policy: Option<SecretsPolicy>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub store: Vec<StoreOp>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub validations: Vec<CrossFieldValidation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub includes: Vec<IncludeSpec>,
    pub questions: Vec<QuestionSpec>,
}
