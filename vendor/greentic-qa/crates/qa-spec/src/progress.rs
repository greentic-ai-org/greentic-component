use serde_json::{Map, Value};

use crate::spec::form::FormSpec;
use crate::spec::question::QuestionSpec;
use crate::store::StoreTarget;
use crate::visibility::VisibilityMap;

/// Encapsulates runtime state for progress evaluation.
#[derive(Debug, Clone)]
pub struct ProgressContext {
    answers: Map<String, Value>,
    config: Value,
    state: Value,
    payload_out: Value,
    secrets: Value,
}

impl ProgressContext {
    pub fn new(answers: Value, ctx: &Value) -> Self {
        let answers_map = answers.as_object().cloned().unwrap_or_default();
        let default = || Value::Object(Map::new());
        Self {
            answers: answers_map,
            config: ctx.get("config").cloned().unwrap_or_else(default),
            state: ctx.get("state").cloned().unwrap_or_else(default),
            payload_out: ctx.get("payload_out").cloned().unwrap_or_else(default),
            secrets: ctx.get("secrets").cloned().unwrap_or_else(default),
        }
    }

    fn has_target(&self, target: StoreTarget, key: &str) -> bool {
        match target {
            StoreTarget::Answers => self.answers.contains_key(key),
            StoreTarget::Config => self.config.get(key).is_some(),
            StoreTarget::State => self.state.get(key).is_some(),
            StoreTarget::PayloadOut => self.payload_out.get(key).is_some(),
            StoreTarget::Secrets => self.secrets.get(key).is_some(),
        }
    }

    pub fn answered_count(&self, spec: &FormSpec, visibility: &VisibilityMap) -> usize {
        spec.questions
            .iter()
            .filter(|question| {
                visibility.get(&question.id).copied().unwrap_or(true)
                    && is_answered(question, self, spec.progress_policy.as_ref())
            })
            .count()
    }
}

pub fn next_question(
    spec: &FormSpec,
    ctx: &ProgressContext,
    visibility: &VisibilityMap,
) -> Option<String> {
    let progress_policy = spec.progress_policy.as_ref().copied().unwrap_or_default();

    for question in &spec.questions {
        if !visibility.get(&question.id).copied().unwrap_or(true) {
            continue;
        }

        if should_skip(question, ctx, &progress_policy) {
            continue;
        }

        return Some(question.id.clone());
    }

    None
}

fn should_skip(
    question: &QuestionSpec,
    ctx: &ProgressContext,
    policy: &crate::spec::form::ProgressPolicy,
) -> bool {
    if question
        .policy
        .skip_if_present_in
        .iter()
        .any(|target| ctx.has_target(*target, &question.id))
    {
        return true;
    }

    if policy.skip_answered && is_answered(question, ctx, Some(policy)) {
        return true;
    }

    false
}

fn is_answered(
    question: &QuestionSpec,
    ctx: &ProgressContext,
    policy: Option<&crate::spec::form::ProgressPolicy>,
) -> bool {
    let has_answer = ctx.answers.contains_key(&question.id);
    let defaults_policy = policy
        .copied()
        .unwrap_or_else(crate::spec::form::ProgressPolicy::default);

    if has_answer {
        return true;
    }

    if defaults_policy.autofill_defaults && question.default_value.is_some() {
        if question.policy.editable_if_from_default {
            return false;
        }
        return defaults_policy.treat_default_as_answered;
    }

    false
}
