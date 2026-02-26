use crate::expr::Expr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Identifier for QA flow steps.
pub type StepId = String;

/// Card/render modes for message steps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CardMode {
    Text,
    Json,
    Card,
}

/// Single message/prompt step inside a flow.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MessageStep {
    pub mode: CardMode,
    pub template: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<StepId>,
}

/// Step that asks a question.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct QuestionStep {
    pub question_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<StepId>,
}

/// Conditional branch case.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DecisionCase {
    #[serde(rename = "if")]
    pub if_expr: Expr,
    pub goto: StepId,
}

/// Decision / branching step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DecisionStep {
    pub cases: Vec<DecisionCase>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_goto: Option<StepId>,
}

/// Flow-wide policies (placeholder for future expansion).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FlowPolicy {
    #[serde(default)]
    pub allow_back: bool,
    #[serde(default)]
    pub allow_submit_all: bool,
}

/// A single wire-up step in QA flows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StepSpec {
    Message(MessageStep),
    Question(QuestionStep),
    Decision(DecisionStep),
    Action { name: String },
    End,
}

/// QAFlow: directed graph of steps executed inside the wizard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct QAFlowSpec {
    pub id: String,
    pub title: String,
    pub version: String,
    pub entry: StepId,
    pub steps: BTreeMap<StepId, StepSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policies: Option<FlowPolicy>,
}
