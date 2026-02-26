use serde_json::{Map, Value};

use crate::{FormSpec, RenderPayload, StoreOp, ValidationResult, build_render_payload, validate};

/// Versioned deterministic plan produced by runner planning functions.
#[derive(Debug, Clone)]
pub struct QaPlanV1 {
    pub plan_version: u16,
    pub form_id: String,
    pub validated_patch: Value,
    pub validation: ValidationResult,
    pub payload: RenderPayload,
    pub effects: Vec<StoreOp>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

impl QaPlanV1 {
    pub fn is_valid(&self) -> bool {
        self.validation.valid
    }
}

/// Build a deterministic plan for patch submission without applying side effects.
pub fn plan_submit_patch(
    spec: &FormSpec,
    ctx: &Value,
    answers: &Value,
    question_id: &str,
    value: Value,
) -> QaPlanV1 {
    let mut patched = answers.as_object().cloned().unwrap_or_default();
    patched.insert(question_id.to_string(), value);
    build_plan(spec, ctx, Value::Object(patched))
}

/// Build a deterministic plan for submit-all without applying side effects.
pub fn plan_submit_all(spec: &FormSpec, ctx: &Value, answers: &Value) -> QaPlanV1 {
    build_plan(spec, ctx, answers.clone())
}

/// Build a deterministic next-step plan for the current answers/context.
pub fn plan_next(spec: &FormSpec, ctx: &Value, answers: &Value) -> QaPlanV1 {
    build_plan(spec, ctx, normalize_answers(answers))
}

fn build_plan(spec: &FormSpec, ctx: &Value, answers: Value) -> QaPlanV1 {
    let validation = validate(spec, &answers);
    let payload = build_render_payload(spec, ctx, &answers);
    let effects = if validation.valid {
        spec.store.clone()
    } else {
        Vec::new()
    };

    let mut errors = Vec::new();
    if !validation.valid {
        errors.extend(validation.errors.iter().map(|error| {
            format!(
                "{}: {}",
                error.path.clone().unwrap_or_default(),
                error.message
            )
        }));
        errors.extend(
            validation
                .missing_required
                .iter()
                .map(|field| format!("missing required: {}", field)),
        );
        errors.extend(
            validation
                .unknown_fields
                .iter()
                .map(|field| format!("unknown field: {}", field)),
        );
    }

    QaPlanV1 {
        plan_version: 1,
        form_id: spec.id.clone(),
        validated_patch: answers,
        validation,
        payload,
        effects,
        warnings: Vec::new(),
        errors,
    }
}

/// Executes plan effects into the provided store context value.
pub fn execute_plan_effects(
    plan: &QaPlanV1,
    store_ctx: &mut crate::StoreContext,
    secrets_policy: Option<&crate::spec::form::SecretsPolicy>,
    secrets_host_available: bool,
) -> Result<(), crate::StoreError> {
    if !plan.is_valid() {
        return Ok(());
    }
    store_ctx.answers = plan.validated_patch.clone();
    store_ctx.apply_ops(&plan.effects, secrets_policy, secrets_host_available)
}

/// Canonicalize incoming answers into an object payload.
pub fn normalize_answers(answers: &Value) -> Value {
    Value::Object(
        answers
            .as_object()
            .cloned()
            .unwrap_or_else(Map::<String, Value>::new),
    )
}
