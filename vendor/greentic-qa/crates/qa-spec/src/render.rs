use serde_json::{Map, Value, json};

use crate::{
    answers_schema,
    computed::apply_computed_answers,
    i18n::{ResolvedI18nMap, resolve_i18n_text_with_locale},
    progress::{ProgressContext, next_question},
    spec::{
        form::FormSpec,
        question::{ListSpec, QuestionType},
    },
    visibility::{VisibilityMode, resolve_visibility},
};

/// Status labels returned by the renderers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderStatus {
    /// More input is required.
    NeedInput,
    /// All visible questions are filled.
    Complete,
    /// Something unexpected occurred.
    Error,
}

impl RenderStatus {
    /// Human-friendly label that matches the renderer requirements.
    pub fn as_str(&self) -> &'static str {
        match self {
            RenderStatus::NeedInput => "need_input",
            RenderStatus::Complete => "complete",
            RenderStatus::Error => "error",
        }
    }
}

/// Progress counters exposed to renderers.
#[derive(Debug, Clone)]
pub struct RenderProgress {
    pub answered: usize,
    pub total: usize,
}

/// Describes a single question for render outputs.
#[derive(Debug, Clone)]
pub struct RenderQuestion {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub title_i18n_key: Option<String>,
    pub description_i18n_key: Option<String>,
    pub kind: QuestionType,
    pub required: bool,
    pub default: Option<String>,
    pub secret: bool,
    pub visible: bool,
    pub current_value: Option<Value>,
    pub choices: Option<Vec<String>>,
    pub list: Option<ListSpec>,
}

/// Collected payload used by both text and JSON renderers.
#[derive(Debug, Clone)]
pub struct RenderPayload {
    pub form_id: String,
    pub form_title: String,
    pub form_version: String,
    pub status: RenderStatus,
    pub next_question_id: Option<String>,
    pub progress: RenderProgress,
    pub help: Option<String>,
    pub questions: Vec<RenderQuestion>,
    pub schema: Value,
}

/// Build the renderer payload from the specification, context, and answers.
pub fn build_render_payload(spec: &FormSpec, ctx: &Value, answers: &Value) -> RenderPayload {
    build_render_payload_with_i18n(spec, ctx, answers, None)
}

/// Build the renderer payload from the specification, context, and answers with optional i18n map.
pub fn build_render_payload_with_i18n(
    spec: &FormSpec,
    ctx: &Value,
    answers: &Value,
    resolved_i18n: Option<&ResolvedI18nMap>,
) -> RenderPayload {
    let computed_answers = apply_computed_answers(spec, answers);
    let visibility = resolve_visibility(spec, &computed_answers, VisibilityMode::Visible);
    let progress_ctx = ProgressContext::new(computed_answers.clone(), ctx);
    let next_question_id = next_question(spec, &progress_ctx, &visibility);

    let answered = progress_ctx.answered_count(spec, &visibility);
    let total = visibility.values().filter(|visible| **visible).count();

    let requested_locale = ctx.get("locale").and_then(Value::as_str);
    let default_locale = spec
        .presentation
        .as_ref()
        .and_then(|presentation| presentation.default_locale.as_deref());

    let questions = spec
        .questions
        .iter()
        .map(|question| RenderQuestion {
            id: question.id.clone(),
            title: resolve_i18n_text_with_locale(
                &question.title,
                question.title_i18n.as_ref(),
                resolved_i18n,
                requested_locale,
                default_locale,
            ),
            description: resolve_description(
                question.description.as_deref(),
                question.description_i18n.as_ref(),
                resolved_i18n,
                requested_locale,
                default_locale,
            ),
            title_i18n_key: question.title_i18n.as_ref().map(|text| text.key.clone()),
            description_i18n_key: question
                .description_i18n
                .as_ref()
                .map(|text| text.key.clone()),
            kind: question.kind,
            required: question.required,
            default: question.default_value.clone(),
            secret: question.secret,
            visible: visibility.get(&question.id).copied().unwrap_or(true),
            current_value: computed_answers.get(&question.id).cloned(),
            choices: question.choices.clone(),
            list: question.list.clone(),
        })
        .collect::<Vec<_>>();

    let help = spec
        .presentation
        .as_ref()
        .and_then(|presentation| presentation.intro.clone())
        .or_else(|| spec.description.clone());

    let schema = answers_schema::generate(spec, &visibility);

    let status = if next_question_id.is_some() {
        RenderStatus::NeedInput
    } else {
        RenderStatus::Complete
    };

    RenderPayload {
        form_id: spec.id.clone(),
        form_title: spec.title.clone(),
        form_version: spec.version.clone(),
        status,
        next_question_id,
        progress: RenderProgress { answered, total },
        help,
        questions,
        schema,
    }
}

/// Render the payload as a structured JSON-friendly value.
pub fn render_json_ui(payload: &RenderPayload) -> Value {
    let questions = payload
        .questions
        .iter()
        .map(|question| {
            let mut map = Map::new();
            map.insert("id".into(), Value::String(question.id.clone()));
            map.insert("title".into(), Value::String(question.title.clone()));
            map.insert(
                "description".into(),
                question
                    .description
                    .clone()
                    .map(Value::String)
                    .unwrap_or(Value::Null),
            );
            map.insert(
                "type".into(),
                Value::String(question_type_label(question.kind).to_string()),
            );
            map.insert("required".into(), Value::Bool(question.required));
            if let Some(default) = &question.default {
                map.insert("default".into(), Value::String(default.clone()));
            }
            if let Some(current_value) = &question.current_value {
                map.insert("current_value".into(), current_value.clone());
            }
            if let Some(choices) = &question.choices {
                map.insert(
                    "choices".into(),
                    Value::Array(
                        choices
                            .iter()
                            .map(|choice| Value::String(choice.clone()))
                            .collect(),
                    ),
                );
            }
            map.insert("visible".into(), Value::Bool(question.visible));
            map.insert("secret".into(), Value::Bool(question.secret));
            if let Some(list) = &question.list
                && let Ok(list_value) = serde_json::to_value(list)
            {
                map.insert("list".into(), list_value);
            }
            Value::Object(map)
        })
        .collect::<Vec<_>>();

    json!({
        "form_id": payload.form_id,
        "form_title": payload.form_title,
        "form_version": payload.form_version,
        "status": payload.status.as_str(),
        "next_question_id": payload.next_question_id,
        "progress": {
            "answered": payload.progress.answered,
            "total": payload.progress.total,
        },
        "help": payload.help,
        "questions": questions,
        "schema": payload.schema,
    })
}

/// Render the payload as human-friendly text.
pub fn render_text(payload: &RenderPayload) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "Form: {} ({})",
        payload.form_title, payload.form_id
    ));
    lines.push(format!(
        "Status: {} ({}/{})",
        payload.status.as_str(),
        payload.progress.answered,
        payload.progress.total
    ));
    if let Some(help) = &payload.help {
        lines.push(format!("Help: {}", help));
    }

    if let Some(next_question) = &payload.next_question_id {
        lines.push(format!("Next question: {}", next_question));
        if let Some(question) = payload
            .questions
            .iter()
            .find(|question| &question.id == next_question)
        {
            lines.push(format!("  Title: {}", question.title));
            if let Some(description) = &question.description {
                lines.push(format!("  Description: {}", description));
            }
            if question.required {
                lines.push("  Required: yes".to_string());
            }
            if let Some(default) = &question.default {
                lines.push(format!("  Default: {}", default));
            }
            if let Some(value) = &question.current_value {
                lines.push(format!("  Current value: {}", value_to_display(value)));
            }
        }
    } else {
        lines.push("All visible questions are answered.".to_string());
    }

    lines.push("Visible questions:".to_string());
    for question in payload.questions.iter().filter(|question| question.visible) {
        let mut entry = format!(" - {} ({})", question.id, question.title);
        if question.required {
            entry.push_str(" [required]");
        }
        if let Some(current_value) = &question.current_value {
            entry.push_str(&format!(" = {}", value_to_display(current_value)));
        }
        lines.push(entry);
    }

    lines.join("\n")
}

/// Render the payload as an Adaptive Card v1.3 transport.
pub fn render_card(payload: &RenderPayload) -> Value {
    let mut body = Vec::new();

    body.push(json!({
        "type": "TextBlock",
        "text": payload.form_title,
        "weight": "Bolder",
        "size": "Large",
        "wrap": true,
    }));

    if let Some(help) = &payload.help {
        body.push(json!({
            "type": "TextBlock",
            "text": help,
            "wrap": true,
        }));
    }

    body.push(json!({
        "type": "FactSet",
        "facts": [
            { "title": "Answered", "value": payload.progress.answered.to_string() },
            { "title": "Total", "value": payload.progress.total.to_string() }
        ]
    }));

    let mut actions = Vec::new();

    if let Some(question_id) = &payload.next_question_id {
        if let Some(question) = payload
            .questions
            .iter()
            .find(|question| &question.id == question_id)
        {
            let mut items = Vec::new();
            items.push(json!({
                "type": "TextBlock",
                "text": question.title,
                "weight": "Bolder",
                "wrap": true,
            }));
            if let Some(description) = &question.description {
                items.push(json!({
                    "type": "TextBlock",
                    "text": description,
                    "wrap": true,
                    "spacing": "Small",
                }));
            }
            items.push(question_input(question));

            body.push(json!({
                "type": "Container",
                "items": items,
            }));

            actions.push(json!({
                "type": "Action.Submit",
                "title": "Next ➡️",
                "data": {
                    "qa": {
                        "formId": payload.form_id,
                        "mode": "patch",
                        "questionId": question.id,
                        "field": "answer"
                    }
                }
            }));
        }
    } else {
        body.push(json!({
            "type": "TextBlock",
            "text": "All visible questions are answered.",
            "wrap": true,
        }));
    }

    json!({
        "$schema": "http://adaptivecards.io/schemas/adaptive-card.json",
        "type": "AdaptiveCard",
        "version": "1.3",
        "body": body,
        "actions": actions,
    })
}

fn question_input(question: &RenderQuestion) -> Value {
    match question.kind {
        QuestionType::String | QuestionType::Integer | QuestionType::Number => {
            let mut map = Map::new();
            map.insert("type".into(), Value::String("Input.Text".into()));
            map.insert("id".into(), Value::String(question.id.clone()));
            map.insert("isRequired".into(), Value::Bool(question.required));
            if let Some(value) = &question.current_value {
                map.insert("value".into(), Value::String(value_to_display(value)));
            }
            Value::Object(map)
        }
        QuestionType::Boolean => {
            let mut map = Map::new();
            map.insert("type".into(), Value::String("Input.Toggle".into()));
            map.insert("id".into(), Value::String(question.id.clone()));
            map.insert("title".into(), Value::String(question.title.clone()));
            map.insert("isRequired".into(), Value::Bool(question.required));
            map.insert("valueOn".into(), Value::String("true".into()));
            map.insert("valueOff".into(), Value::String("false".into()));
            if let Some(value) = &question.current_value {
                if value.as_bool() == Some(true) {
                    map.insert("value".into(), Value::String("true".into()));
                } else {
                    map.insert("value".into(), Value::String("false".into()));
                }
            }
            Value::Object(map)
        }
        QuestionType::Enum => {
            let mut map = Map::new();
            map.insert("type".into(), Value::String("Input.ChoiceSet".into()));
            map.insert("id".into(), Value::String(question.id.clone()));
            map.insert("style".into(), Value::String("compact".into()));
            map.insert("isRequired".into(), Value::Bool(question.required));
            let choices = question
                .choices
                .clone()
                .unwrap_or_default()
                .into_iter()
                .map(|choice| {
                    json!({
                        "title": choice,
                        "value": choice,
                    })
                })
                .collect::<Vec<_>>();
            map.insert("choices".into(), Value::Array(choices));
            if let Some(value) = &question.current_value {
                map.insert("value".into(), Value::String(value_to_display(value)));
            }
            Value::Object(map)
        }
        QuestionType::List => {
            let mut map = Map::new();
            map.insert("type".into(), Value::String("TextBlock".into()));
            map.insert(
                "text".into(),
                Value::String(format!(
                    "List group '{}' ({} entries)",
                    question.title,
                    question
                        .current_value
                        .as_ref()
                        .and_then(Value::as_array)
                        .map(|entries| entries.len())
                        .unwrap_or_default()
                )),
            );
            map.insert("wrap".into(), Value::Bool(true));
            Value::Object(map)
        }
    }
}

fn question_type_label(kind: QuestionType) -> &'static str {
    match kind {
        QuestionType::String => "string",
        QuestionType::Boolean => "boolean",
        QuestionType::Integer => "integer",
        QuestionType::Number => "number",
        QuestionType::Enum => "enum",
        QuestionType::List => "list",
    }
}

fn value_to_display(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(num) => num.to_string(),
        other => other.to_string(),
    }
}

fn resolve_description(
    fallback: Option<&str>,
    text: Option<&crate::i18n::I18nText>,
    resolved: Option<&ResolvedI18nMap>,
    requested_locale: Option<&str>,
    default_locale: Option<&str>,
) -> Option<String> {
    match (fallback, text) {
        (Some(raw), _) => Some(resolve_i18n_text_with_locale(
            raw,
            text,
            resolved,
            requested_locale,
            default_locale,
        )),
        (None, Some(i18n_text)) => {
            let resolved_text = resolve_i18n_text_with_locale(
                &i18n_text.key,
                Some(i18n_text),
                resolved,
                requested_locale,
                default_locale,
            );
            if resolved_text != i18n_text.key {
                return Some(resolved_text);
            }
            Some(i18n_text.key.clone())
        }
        (None, None) => None,
    }
}
