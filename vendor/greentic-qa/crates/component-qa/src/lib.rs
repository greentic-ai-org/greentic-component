use std::collections::BTreeMap;
use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use thiserror::Error;

use qa_spec::{
    FormSpec, ProgressContext, RenderPayload, StoreContext, StoreError, StoreOp, VisibilityMode,
    answers_schema, build_render_payload, example_answers, next_question,
    render_card as qa_render_card, render_json_ui as qa_render_json_ui,
    render_text as qa_render_text, resolve_visibility, validate,
};

const DEFAULT_SPEC: &str = include_str!("../tests/fixtures/simple_form.json");

#[derive(Debug, Error)]
enum ComponentError {
    #[error("failed to parse config/{0}")]
    ConfigParse(#[source] serde_json::Error),
    #[error("form '{0}' is not available")]
    FormUnavailable(String),
    #[error("json encode error: {0}")]
    JsonEncode(#[source] serde_json::Error),
    #[error("include expansion failed: {0}")]
    Include(String),
    #[error("store apply failed: {0}")]
    Store(#[from] StoreError),
}

#[derive(Debug, Deserialize, Serialize, Default)]
struct ComponentConfig {
    #[serde(default)]
    form_spec_json: Option<String>,
    #[serde(default)]
    include_registry: BTreeMap<String, String>,
}

fn load_form_spec(config_json: &str) -> Result<FormSpec, ComponentError> {
    let spec_value = load_form_spec_value(config_json)?;
    serde_json::from_value(spec_value).map_err(ComponentError::ConfigParse)
}

fn load_form_spec_value(config_json: &str) -> Result<Value, ComponentError> {
    if config_json.trim().is_empty() {
        return serde_json::from_str(DEFAULT_SPEC).map_err(ComponentError::ConfigParse);
    }

    let parsed: Value = serde_json::from_str(config_json).map_err(ComponentError::ConfigParse)?;

    // Compatibility: callers may pass raw FormSpec JSON directly.
    let (mut spec_value, include_registry_values) = if looks_like_form_spec_json(&parsed) {
        (parsed.clone(), BTreeMap::new())
    } else {
        let config: ComponentConfig =
            serde_json::from_value(parsed.clone()).map_err(ComponentError::ConfigParse)?;
        let raw_spec = config
            .form_spec_json
            .unwrap_or_else(|| DEFAULT_SPEC.to_string());
        let spec_value = serde_json::from_str(&raw_spec).map_err(ComponentError::ConfigParse)?;
        let mut registry = BTreeMap::new();
        for (form_ref, raw_form) in config.include_registry {
            let value = serde_json::from_str(&raw_form).map_err(ComponentError::ConfigParse)?;
            registry.insert(form_ref, value);
        }
        (spec_value, registry)
    };

    if !include_registry_values.is_empty() {
        spec_value = expand_includes_value(&spec_value, &include_registry_values)?;
    }
    Ok(spec_value)
}

fn expand_includes_value(
    root: &Value,
    registry: &BTreeMap<String, Value>,
) -> Result<Value, ComponentError> {
    let mut chain = Vec::new();
    let mut seen_ids = BTreeSet::new();
    expand_form_value(root, "", registry, &mut chain, &mut seen_ids)
}

fn expand_form_value(
    form: &Value,
    prefix: &str,
    registry: &BTreeMap<String, Value>,
    chain: &mut Vec<String>,
    seen_ids: &mut BTreeSet<String>,
) -> Result<Value, ComponentError> {
    let form_obj = form
        .as_object()
        .ok_or_else(|| ComponentError::Include("form spec must be a JSON object".into()))?;
    let form_id = form_obj
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("<unknown>")
        .to_string();
    if chain.contains(&form_id) {
        let pos = chain.iter().position(|id| id == &form_id).unwrap_or(0);
        let mut cycle = chain[pos..].to_vec();
        cycle.push(form_id);
        return Err(ComponentError::Include(format!(
            "include cycle detected: {:?}",
            cycle
        )));
    }
    chain.push(form_id);

    let mut out = form_obj.clone();
    out.insert("includes".into(), Value::Array(Vec::new()));
    out.insert("questions".into(), Value::Array(Vec::new()));
    out.insert("validations".into(), Value::Array(Vec::new()));

    let mut out_questions = Vec::new();
    let mut out_validations = Vec::new();

    for question in form_obj
        .get("questions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
    {
        let mut q = question;
        prefix_question_value(&mut q, prefix);
        if let Some(id) = q.get("id").and_then(Value::as_str)
            && !seen_ids.insert(id.to_string())
        {
            return Err(ComponentError::Include(format!(
                "duplicate question id after include expansion: '{}'",
                id
            )));
        }
        out_questions.push(q);
    }

    for validation in form_obj
        .get("validations")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
    {
        let mut v = validation;
        prefix_validation_value(&mut v, prefix);
        out_validations.push(v);
    }

    for include in form_obj
        .get("includes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
    {
        let form_ref = include
            .get("form_ref")
            .and_then(Value::as_str)
            .ok_or_else(|| ComponentError::Include("include missing form_ref".into()))?;
        let include_prefix = include.get("prefix").and_then(Value::as_str);
        let child_prefix = combine_prefix(prefix, include_prefix);
        let included = registry.get(form_ref).ok_or_else(|| {
            ComponentError::Include(format!("missing include target '{}'", form_ref))
        })?;
        let expanded = expand_form_value(included, &child_prefix, registry, chain, seen_ids)?;
        out_questions.extend(
            expanded
                .get("questions")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
        );
        out_validations.extend(
            expanded
                .get("validations")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
        );
    }

    out.insert("questions".into(), Value::Array(out_questions));
    out.insert("validations".into(), Value::Array(out_validations));
    chain.pop();

    Ok(Value::Object(out))
}

fn parse_context(ctx_json: &str) -> Value {
    serde_json::from_str(ctx_json).unwrap_or_else(|_| Value::Object(Map::new()))
}

fn parse_runtime_context(ctx_json: &str) -> Value {
    let parsed = parse_context(ctx_json);
    parsed
        .get("ctx")
        .and_then(Value::as_object)
        .map(|ctx| Value::Object(ctx.clone()))
        .unwrap_or(parsed)
}

fn looks_like_form_spec_json(value: &Value) -> bool {
    value.get("id").and_then(Value::as_str).is_some()
        && value.get("title").and_then(Value::as_str).is_some()
        && value.get("version").and_then(Value::as_str).is_some()
        && value.get("questions").and_then(Value::as_array).is_some()
}

fn combine_prefix(parent: &str, child: Option<&str>) -> String {
    match (parent.is_empty(), child.unwrap_or("").is_empty()) {
        (true, true) => String::new(),
        (false, true) => parent.to_string(),
        (true, false) => child.unwrap_or_default().to_string(),
        (false, false) => format!("{}.{}", parent, child.unwrap_or_default()),
    }
}

fn prefix_key(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{}.{}", prefix, key)
    }
}

fn prefix_path(prefix: &str, path: &str) -> String {
    if path.is_empty() || path.starts_with('/') || prefix.is_empty() {
        return path.to_string();
    }
    format!("{}.{}", prefix, path)
}

fn prefix_validation_value(validation: &mut Value, prefix: &str) {
    if prefix.is_empty() {
        return;
    }
    if let Some(fields) = validation.get_mut("fields").and_then(Value::as_array_mut) {
        for field in fields {
            if let Some(raw) = field.as_str() {
                *field = Value::String(prefix_key(prefix, raw));
            }
        }
    }
    if let Some(condition) = validation.get_mut("condition") {
        prefix_expr_value(condition, prefix);
    }
}

fn prefix_question_value(question: &mut Value, prefix: &str) {
    if prefix.is_empty() {
        return;
    }
    if let Some(id) = question.get_mut("id")
        && let Some(raw) = id.as_str()
    {
        *id = Value::String(prefix_key(prefix, raw));
    }
    if let Some(visible_if) = question.get_mut("visible_if") {
        prefix_expr_value(visible_if, prefix);
    }
    if let Some(computed) = question.get_mut("computed") {
        prefix_expr_value(computed, prefix);
    }
    if let Some(fields) = question
        .get_mut("list")
        .and_then(|list| list.get_mut("fields"))
        .and_then(Value::as_array_mut)
    {
        for field in fields {
            prefix_question_value(field, prefix);
        }
    }
}

fn prefix_expr_value(expr: &mut Value, prefix: &str) {
    if let Some(obj) = expr.as_object_mut() {
        if matches!(
            obj.get("op").and_then(Value::as_str),
            Some("answer") | Some("is_set")
        ) && let Some(path) = obj.get_mut("path")
            && let Some(raw) = path.as_str()
        {
            *path = Value::String(prefix_path(prefix, raw));
        }
        if let Some(inner) = obj.get_mut("expression") {
            prefix_expr_value(inner, prefix);
        }
        if let Some(left) = obj.get_mut("left") {
            prefix_expr_value(left, prefix);
        }
        if let Some(right) = obj.get_mut("right") {
            prefix_expr_value(right, prefix);
        }
        if let Some(items) = obj.get_mut("expressions").and_then(Value::as_array_mut) {
            for item in items {
                prefix_expr_value(item, prefix);
            }
        }
    }
}

fn resolve_context_answers(ctx: &Value) -> Value {
    ctx.get("answers")
        .cloned()
        .unwrap_or_else(|| Value::Object(Map::new()))
}

fn parse_answers(answers_json: &str) -> Value {
    serde_json::from_str(answers_json).unwrap_or_else(|_| Value::Object(Map::new()))
}

fn secrets_host_available(ctx: &Value) -> bool {
    ctx.get("secrets_host_available")
        .and_then(Value::as_bool)
        .or_else(|| {
            ctx.get("config")
                .and_then(Value::as_object)
                .and_then(|config| config.get("secrets_host_available"))
                .and_then(Value::as_bool)
        })
        .unwrap_or(false)
}

fn respond(result: Result<Value, ComponentError>) -> String {
    match result {
        Ok(value) => serde_json::to_string(&value).unwrap_or_else(|error| {
            json!({"error": format!("json encode: {}", error)}).to_string()
        }),
        Err(err) => json!({ "error": err.to_string() }).to_string(),
    }
}

pub fn describe(form_id: &str, config_json: &str) -> String {
    respond(load_form_spec(config_json).and_then(|spec| {
        if spec.id != form_id {
            Err(ComponentError::FormUnavailable(form_id.to_string()))
        } else {
            serde_json::to_value(spec).map_err(ComponentError::JsonEncode)
        }
    }))
}

fn ensure_form(form_id: &str, config_json: &str) -> Result<FormSpec, ComponentError> {
    let spec = load_form_spec(config_json)?;
    if spec.id != form_id {
        Err(ComponentError::FormUnavailable(form_id.to_string()))
    } else {
        Ok(spec)
    }
}

pub fn get_answer_schema(form_id: &str, config_json: &str, ctx_json: &str) -> String {
    let schema = ensure_form(form_id, config_json).map(|spec| {
        let ctx = parse_runtime_context(ctx_json);
        let answers = resolve_context_answers(&ctx);
        let visibility = resolve_visibility(&spec, &answers, VisibilityMode::Visible);
        answers_schema(&spec, &visibility)
    });
    respond(schema)
}

pub fn get_example_answers(form_id: &str, config_json: &str, ctx_json: &str) -> String {
    let result = ensure_form(form_id, config_json).map(|spec| {
        let ctx = parse_runtime_context(ctx_json);
        let answers = resolve_context_answers(&ctx);
        let visibility = resolve_visibility(&spec, &answers, VisibilityMode::Visible);
        example_answers(&spec, &visibility)
    });
    respond(result)
}

pub fn validate_answers(form_id: &str, config_json: &str, answers_json: &str) -> String {
    let validation = ensure_form(form_id, config_json).and_then(|spec| {
        let answers = serde_json::from_str(answers_json).map_err(ComponentError::ConfigParse)?;
        serde_json::to_value(validate(&spec, &answers)).map_err(ComponentError::JsonEncode)
    });
    respond(validation)
}

pub fn next_with_ctx(
    form_id: &str,
    config_json: &str,
    ctx_json: &str,
    answers_json: &str,
) -> String {
    let result = ensure_form(form_id, config_json).map(|spec| {
        let ctx = parse_runtime_context(ctx_json);
        let answers = parse_answers(answers_json);
        let visibility = resolve_visibility(&spec, &answers, VisibilityMode::Visible);
        let progress_ctx = ProgressContext::new(answers.clone(), &ctx);
        let next_q = next_question(&spec, &progress_ctx, &visibility);
        let answered = progress_ctx.answered_count(&spec, &visibility);
        let total = visibility.values().filter(|visible| **visible).count();
        json!({
            "status": if next_q.is_some() { "need_input" } else { "complete" },
            "next_question_id": next_q,
            "progress": {
                "answered": answered,
                "total": total
            }
        })
    });
    respond(result)
}

pub fn next(form_id: &str, config_json: &str, answers_json: &str) -> String {
    next_with_ctx(form_id, config_json, "{}", answers_json)
}

pub fn apply_store(form_id: &str, ctx_json: &str, answers_json: &str) -> String {
    let result = ensure_form(form_id, ctx_json).and_then(|spec| {
        let ctx = parse_runtime_context(ctx_json);
        let answers = parse_answers(answers_json);
        let mut store_ctx = StoreContext::from_value(&ctx);
        store_ctx.answers = answers;
        let host_available = secrets_host_available(&ctx);
        store_ctx.apply_ops(&spec.store, spec.secrets_policy.as_ref(), host_available)?;
        Ok(store_ctx.to_value())
    });
    respond(result)
}

fn render_payload(
    form_id: &str,
    config_json: &str,
    ctx_json: &str,
    answers_json: &str,
) -> Result<RenderPayload, ComponentError> {
    let spec = ensure_form(form_id, config_json)?;
    let ctx = parse_runtime_context(ctx_json);
    let answers = parse_answers(answers_json);
    let mut payload = build_render_payload(&spec, &ctx, &answers);
    let spec_value = load_form_spec_value(config_json)?;
    apply_i18n_to_payload(&mut payload, &spec_value, &ctx);
    Ok(payload)
}

type ResolvedI18nMap = BTreeMap<String, String>;

fn parse_resolved_i18n(ctx: &Value) -> ResolvedI18nMap {
    ctx.get("i18n_resolved")
        .and_then(Value::as_object)
        .map(|value| {
            value
                .iter()
                .filter_map(|(key, val)| val.as_str().map(|text| (key.clone(), text.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

fn i18n_debug_enabled(ctx: &Value) -> bool {
    ctx.get("debug_i18n")
        .and_then(Value::as_bool)
        .or_else(|| ctx.get("i18n_debug").and_then(Value::as_bool))
        .unwrap_or(false)
}

fn attach_i18n_debug_metadata(card: &mut Value, payload: &RenderPayload, spec_value: &Value) {
    let keys = build_question_i18n_key_map(spec_value);
    let question_metadata = payload
        .questions
        .iter()
        .filter_map(|question| {
            let (title_key, description_key) =
                keys.get(&question.id).cloned().unwrap_or((None, None));
            if title_key.is_none() && description_key.is_none() {
                return None;
            }
            Some(json!({
                "id": question.id,
                "title_key": title_key,
                "description_key": description_key,
            }))
        })
        .collect::<Vec<_>>();
    if question_metadata.is_empty() {
        return;
    }

    if let Some(map) = card.as_object_mut() {
        map.insert(
            "metadata".into(),
            json!({
                "qa": {
                    "i18n_debug": true,
                    "questions": question_metadata
                }
            }),
        );
    }
}

fn build_question_i18n_key_map(
    spec_value: &Value,
) -> BTreeMap<String, (Option<String>, Option<String>)> {
    let mut map = BTreeMap::new();
    for question in spec_value
        .get("questions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
    {
        if let Some(id) = question.get("id").and_then(Value::as_str) {
            let title_key = question
                .get("title_i18n")
                .and_then(|value| value.get("key"))
                .and_then(Value::as_str)
                .map(str::to_string);
            let description_key = question
                .get("description_i18n")
                .and_then(|value| value.get("key"))
                .and_then(Value::as_str)
                .map(str::to_string);
            map.insert(id.to_string(), (title_key, description_key));
        }
    }
    map
}

fn resolve_i18n_value(
    resolved: &ResolvedI18nMap,
    key: &str,
    requested_locale: Option<&str>,
    default_locale: Option<&str>,
) -> Option<String> {
    for locale in [requested_locale, default_locale].iter().flatten() {
        if let Some(value) = resolved.get(&format!("{}:{}", locale, key)) {
            return Some(value.clone());
        }
        if let Some(value) = resolved.get(&format!("{}/{}", locale, key)) {
            return Some(value.clone());
        }
    }
    resolved.get(key).cloned()
}

fn apply_i18n_to_payload(payload: &mut RenderPayload, spec_value: &Value, ctx: &Value) {
    let resolved = parse_resolved_i18n(ctx);
    if resolved.is_empty() {
        return;
    }
    let requested_locale = ctx.get("locale").and_then(Value::as_str);
    let default_locale = spec_value
        .get("presentation")
        .and_then(|value| value.get("default_locale"))
        .and_then(Value::as_str);

    let mut by_id = BTreeMap::new();
    for question in spec_value
        .get("questions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
    {
        if let Some(id) = question.get("id").and_then(Value::as_str) {
            by_id.insert(id.to_string(), question);
        }
    }

    for question in &mut payload.questions {
        let Some(spec_question) = by_id.get(&question.id) else {
            continue;
        };
        if let Some(key) = spec_question
            .get("title_i18n")
            .and_then(|value| value.get("key"))
            .and_then(Value::as_str)
            && let Some(value) =
                resolve_i18n_value(&resolved, key, requested_locale, default_locale)
        {
            question.title = value;
        }
        if let Some(key) = spec_question
            .get("description_i18n")
            .and_then(|value| value.get("key"))
            .and_then(Value::as_str)
            && let Some(value) =
                resolve_i18n_value(&resolved, key, requested_locale, default_locale)
        {
            question.description = Some(value);
        }
    }
}

fn respond_string(result: Result<String, ComponentError>) -> String {
    match result {
        Ok(value) => value,
        Err(err) => json!({ "error": err.to_string() }).to_string(),
    }
}

pub fn render_text(form_id: &str, config_json: &str, ctx_json: &str, answers_json: &str) -> String {
    respond_string(
        render_payload(form_id, config_json, ctx_json, answers_json)
            .map(|payload| qa_render_text(&payload)),
    )
}

pub fn render_json_ui(
    form_id: &str,
    config_json: &str,
    ctx_json: &str,
    answers_json: &str,
) -> String {
    respond(
        render_payload(form_id, config_json, ctx_json, answers_json)
            .map(|payload| qa_render_json_ui(&payload)),
    )
}

pub fn render_card(form_id: &str, config_json: &str, ctx_json: &str, answers_json: &str) -> String {
    respond(
        render_payload(form_id, config_json, ctx_json, answers_json).map(|payload| {
            let mut card = qa_render_card(&payload);
            let ctx = parse_runtime_context(ctx_json);
            if i18n_debug_enabled(&ctx)
                && let Ok(spec_value) = load_form_spec_value(config_json)
            {
                attach_i18n_debug_metadata(&mut card, &payload, &spec_value);
            }
            card
        }),
    )
}

fn submission_progress(payload: &RenderPayload) -> Value {
    json!({
        "answered": payload.progress.answered,
        "total": payload.progress.total,
    })
}

fn build_error_response(
    payload: &RenderPayload,
    answers: Value,
    validation: &qa_spec::ValidationResult,
) -> Result<Value, ComponentError> {
    let validation_value = serde_json::to_value(validation).map_err(ComponentError::JsonEncode)?;
    Ok(json!({
        "status": "error",
        "next_question_id": payload.next_question_id,
        "progress": submission_progress(payload),
        "answers": answers,
        "validation": validation_value,
    }))
}

fn build_success_response(
    payload: &RenderPayload,
    answers: Value,
    store_ctx: &StoreContext,
) -> Value {
    let status = if payload.next_question_id.is_some() {
        "need_input"
    } else {
        "complete"
    };

    json!({
        "status": status,
        "next_question_id": payload.next_question_id,
        "progress": submission_progress(payload),
        "answers": answers,
        "store": store_ctx.to_value(),
    })
}

#[derive(Debug, Clone)]
struct SubmissionPlan {
    validated_patch: Value,
    validation: qa_spec::ValidationResult,
    payload: RenderPayload,
    effects: Vec<StoreOp>,
}

fn build_submission_plan(spec: &FormSpec, ctx: &Value, answers: Value) -> SubmissionPlan {
    let validation = validate(spec, &answers);
    let payload = build_render_payload(spec, ctx, &answers);
    let effects = if validation.valid {
        spec.store.clone()
    } else {
        Vec::new()
    };
    SubmissionPlan {
        validated_patch: answers,
        validation,
        payload,
        effects,
    }
}

pub fn submit_patch(
    form_id: &str,
    config_json: &str,
    ctx_json: &str,
    answers_json: &str,
    question_id: &str,
    value_json: &str,
) -> String {
    // Compatibility wrapper: this endpoint now follows a deterministic
    // plan->execute split internally while preserving existing response shape.
    respond(ensure_form(form_id, config_json).and_then(|spec| {
        let ctx = parse_runtime_context(ctx_json);
        let value: Value = serde_json::from_str(value_json).map_err(ComponentError::ConfigParse)?;
        let mut answers = parse_answers(answers_json)
            .as_object()
            .cloned()
            .unwrap_or_default();
        answers.insert(question_id.to_string(), value);
        let plan = build_submission_plan(&spec, &ctx, Value::Object(answers));

        if !plan.validation.valid {
            return build_error_response(&plan.payload, plan.validated_patch, &plan.validation);
        }

        let mut store_ctx = StoreContext::from_value(&ctx);
        store_ctx.answers = plan.validated_patch.clone();
        let host_available = secrets_host_available(&ctx);
        store_ctx.apply_ops(&plan.effects, spec.secrets_policy.as_ref(), host_available)?;
        let response = build_success_response(&plan.payload, plan.validated_patch, &store_ctx);
        Ok(response)
    }))
}

pub fn submit_all(form_id: &str, config_json: &str, ctx_json: &str, answers_json: &str) -> String {
    // Compatibility wrapper: this endpoint now follows a deterministic
    // plan->execute split internally while preserving existing response shape.
    respond(ensure_form(form_id, config_json).and_then(|spec| {
        let ctx = parse_runtime_context(ctx_json);
        let answers = parse_answers(answers_json);
        let plan = build_submission_plan(&spec, &ctx, answers);

        if !plan.validation.valid {
            return build_error_response(&plan.payload, plan.validated_patch, &plan.validation);
        }

        let mut store_ctx = StoreContext::from_value(&ctx);
        store_ctx.answers = plan.validated_patch.clone();
        let host_available = secrets_host_available(&ctx);
        store_ctx.apply_ops(&plan.effects, spec.secrets_policy.as_ref(), host_available)?;
        let response = build_success_response(&plan.payload, plan.validated_patch, &store_ctx);
        Ok(response)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn describe_returns_spec_json() {
        let payload = describe("example-form", "");
        let spec: Value = serde_json::from_str(&payload).expect("valid json");
        assert_eq!(spec["id"], "example-form");
    }

    #[test]
    fn describe_accepts_raw_form_spec_as_config_json() {
        let spec = json!({
            "id": "raw-form",
            "title": "Raw",
            "version": "1.0",
            "questions": [
                { "id": "q1", "type": "string", "title": "Q1", "required": true }
            ]
        });
        let payload = describe("raw-form", &spec.to_string());
        let parsed: Value = serde_json::from_str(&payload).expect("json");
        assert_eq!(parsed["id"], "raw-form");
    }

    #[test]
    fn schema_matches_questions() {
        let schema = get_answer_schema("example-form", "", "{}");
        let value: Value = serde_json::from_str(&schema).expect("json");
        assert!(
            value
                .get("properties")
                .unwrap()
                .as_object()
                .unwrap()
                .contains_key("q1")
        );
    }

    #[test]
    fn example_answers_include_question_values() {
        let examples = get_example_answers("example-form", "", "{}");
        let parsed: Value = serde_json::from_str(&examples).expect("json");
        assert_eq!(parsed["q1"], "example-q1");
    }

    #[test]
    fn validate_answers_reports_valid_when_complete() {
        let answers = json!({ "q1": "tester", "q2": true });
        let result = validate_answers("example-form", "", &answers.to_string());
        let parsed: Value = serde_json::from_str(&result).expect("json");
        assert!(parsed["valid"].as_bool().unwrap_or(false));
    }

    #[test]
    fn next_returns_progress_payload() {
        let spec = json!({
            "id": "progress-form",
            "title": "Progress",
            "version": "1.0",
            "progress_policy": {
                "skip_answered": true
            },
            "questions": [
                { "id": "q1", "type": "string", "title": "q1", "required": true },
                { "id": "q2", "type": "string", "title": "q2", "required": true }
            ]
        });
        let ctx = json!({ "form_spec_json": spec.to_string() });
        let response = next("progress-form", &ctx.to_string(), r#"{"q1": "test"}"#);
        let parsed: Value = serde_json::from_str(&response).expect("json");
        assert_eq!(parsed["status"], "need_input");
        assert_eq!(parsed["next_question_id"], "q2");
        assert_eq!(parsed["progress"]["answered"], 1);
    }

    #[test]
    fn next_accepts_context_envelope_under_ctx_key() {
        let spec = json!({
            "id": "progress-form",
            "title": "Progress",
            "version": "1.0",
            "progress_policy": {
                "skip_answered": true
            },
            "questions": [
                { "id": "q1", "type": "string", "title": "q1", "required": true },
                { "id": "q2", "type": "string", "title": "q2", "required": true }
            ]
        });
        let cfg = json!({
            "form_spec_json": spec.to_string(),
            "ctx": {
                "state": {}
            }
        });
        let response = next("progress-form", &cfg.to_string(), r#"{"q1":"done"}"#);
        let parsed: Value = serde_json::from_str(&response).expect("json");
        assert_eq!(parsed["status"], "need_input");
        assert_eq!(parsed["next_question_id"], "q2");
    }

    #[test]
    fn apply_store_writes_state_value() {
        let spec = json!({
            "id": "store-form",
            "title": "Store",
            "version": "1.0",
            "questions": [
                { "id": "q1", "type": "string", "title": "q1", "required": true }
            ],
            "store": [
                {
                    "target": "state",
                    "path": "/flag",
                    "value": true
                }
            ]
        });
        let ctx = json!({
            "form_spec_json": spec.to_string(),
            "state": {}
        });
        let result = apply_store("store-form", &ctx.to_string(), "{}");
        let parsed: Value = serde_json::from_str(&result).expect("json");
        assert_eq!(parsed["state"]["flag"], true);
    }

    #[test]
    fn apply_store_writes_secret_when_allowed() {
        let spec = json!({
            "id": "store-secret",
            "title": "Store Secret",
            "version": "1.0",
            "questions": [
                { "id": "q1", "type": "string", "title": "q1", "required": true }
            ],
            "store": [
                {
                    "target": "secrets",
                    "path": "/aws/key",
                    "value": "value"
                }
            ],
            "secrets_policy": {
                "enabled": true,
                "read_enabled": true,
                "write_enabled": true,
                "allow": ["aws/*"]
            }
        });
        let ctx = json!({
            "form_spec_json": spec.to_string(),
            "state": {},
            "secrets_host_available": true
        });
        let result = apply_store("store-secret", &ctx.to_string(), "{}");
        let parsed: Value = serde_json::from_str(&result).expect("json");
        assert_eq!(parsed["secrets"]["aws"]["key"], "value");
    }

    #[test]
    fn render_text_outputs_summary() {
        let output = render_text("example-form", "", "{}", "{}");
        assert!(output.contains("Form:"));
        assert!(output.contains("Visible questions"));
    }

    #[test]
    fn render_json_ui_outputs_json_payload() {
        let payload = render_json_ui("example-form", "", "{}", r#"{"q1":"value"}"#);
        let parsed: Value = serde_json::from_str(&payload).expect("json");
        assert_eq!(parsed["form_id"], "example-form");
        assert_eq!(parsed["progress"]["total"], 2);
    }

    #[test]
    fn render_json_ui_expands_includes_from_registry() {
        let parent = json!({
            "id": "parent-form",
            "title": "Parent",
            "version": "1.0",
            "includes": [
                { "form_ref": "child", "prefix": "child" }
            ],
            "questions": [
                { "id": "root", "type": "string", "title": "Root", "required": true }
            ]
        });
        let child = json!({
            "id": "child-form",
            "title": "Child",
            "version": "1.0",
            "questions": [
                { "id": "name", "type": "string", "title": "Name", "required": true }
            ]
        });
        let config = json!({
            "form_spec_json": parent.to_string(),
            "include_registry": {
                "child": child.to_string()
            }
        });

        let payload = render_json_ui("parent-form", &config.to_string(), "{}", "{}");
        let parsed: Value = serde_json::from_str(&payload).expect("json");
        let questions = parsed["questions"].as_array().expect("questions array");
        assert!(questions.iter().any(|question| question["id"] == "root"));
        assert!(
            questions
                .iter()
                .any(|question| question["id"] == "child.name")
        );
    }

    #[test]
    fn render_card_outputs_patch_action() {
        let payload = render_card("example-form", "", "{}", "{}");
        let parsed: Value = serde_json::from_str(&payload).expect("json");
        assert_eq!(parsed["version"], "1.3");
        let actions = parsed["actions"].as_array().expect("actions");
        assert_eq!(actions[0]["data"]["qa"]["mode"], "patch");
    }

    #[test]
    fn render_card_attaches_i18n_debug_metadata_when_enabled() {
        let spec = json!({
            "id": "i18n-card-form",
            "title": "Card",
            "version": "1.0",
            "questions": [
                {
                    "id": "name",
                    "type": "string",
                    "title": "Name",
                    "title_i18n": { "key": "name.title" },
                    "required": true
                }
            ]
        });
        let config = json!({ "form_spec_json": spec.to_string() });
        let ctx = json!({
            "i18n_debug": true,
            "i18n_resolved": {
                "name.title": "Localized Name"
            }
        });
        let payload = render_card(
            "i18n-card-form",
            &config.to_string(),
            &ctx.to_string(),
            "{}",
        );
        let parsed: Value = serde_json::from_str(&payload).expect("json");
        assert_eq!(parsed["metadata"]["qa"]["i18n_debug"], true);
        let questions = parsed["metadata"]["qa"]["questions"]
            .as_array()
            .expect("questions metadata");
        assert_eq!(questions[0]["id"], "name");
        assert_eq!(questions[0]["title_key"], "name.title");
    }

    #[test]
    fn submit_patch_advances_and_updates_store() {
        let response = submit_patch("example-form", "", "{}", "{}", "q1", r#""Acme""#);
        let parsed: Value = serde_json::from_str(&response).expect("json");
        assert_eq!(parsed["status"], "need_input");
        assert_eq!(parsed["next_question_id"], "q2");
        assert_eq!(parsed["answers"]["q1"], "Acme");
        assert_eq!(parsed["store"]["answers"]["q1"], "Acme");
    }

    #[test]
    fn submit_patch_returns_validation_error() {
        let response = submit_patch("example-form", "", "{}", "{}", "q1", "true");
        let parsed: Value = serde_json::from_str(&response).expect("json");
        assert_eq!(parsed["status"], "error");
        assert_eq!(parsed["validation"]["errors"][0]["code"], "type_mismatch");
    }

    #[test]
    fn submit_all_completes_with_valid_answers() {
        let response = submit_all("example-form", "", "{}", r#"{"q1":"Acme","q2":true}"#);
        let parsed: Value = serde_json::from_str(&response).expect("json");
        assert_eq!(parsed["status"], "complete");
        assert!(parsed["next_question_id"].is_null());
        assert_eq!(parsed["answers"]["q2"], true);
        assert_eq!(parsed["store"]["answers"]["q2"], true);
    }
}
