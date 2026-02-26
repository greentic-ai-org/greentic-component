use regex::Regex;
use serde_json::Value;

use crate::answers::{ValidationError, ValidationResult};
use crate::computed::{apply_computed_answers, build_expression_context};
use crate::spec::form::FormSpec;
use crate::spec::question::{QuestionSpec, QuestionType};
use crate::visibility::{VisibilityMode, resolve_visibility};

pub fn validate(spec: &FormSpec, answers: &Value) -> ValidationResult {
    let computed_answers = apply_computed_answers(spec, answers);
    let visibility = resolve_visibility(spec, &computed_answers, VisibilityMode::Visible);
    let answers_map = computed_answers.as_object().cloned().unwrap_or_default();

    let mut errors = Vec::new();
    let mut missing_required = Vec::new();

    for question in &spec.questions {
        if !visibility.get(&question.id).copied().unwrap_or(true) {
            continue;
        }

        match answers_map.get(&question.id) {
            None => {
                if question.required {
                    missing_required.push(question.id.clone());
                }
            }
            Some(value) => {
                if let Some(error) = validate_value(question, value) {
                    errors.push(error);
                }
            }
        }
    }

    let all_ids: std::collections::BTreeSet<_> = spec
        .questions
        .iter()
        .map(|question| question.id.clone())
        .collect();
    let unknown_fields: Vec<String> = answers_map
        .keys()
        .filter(|key| !all_ids.contains(*key))
        .cloned()
        .collect();

    let ctx = build_expression_context(&computed_answers);
    for validation in &spec.validations {
        if let Some(true) = validation.condition.evaluate_bool(&ctx) {
            let question_id = validation
                .fields
                .first()
                .cloned()
                .or_else(|| validation.id.clone());
            let path = validation.fields.first().map(|field| format!("/{}", field));
            errors.push(ValidationError {
                question_id,
                path,
                message: validation.message.clone(),
                code: validation.code.clone(),
            });
        }
    }

    ValidationResult {
        valid: errors.is_empty() && missing_required.is_empty() && unknown_fields.is_empty(),
        errors,
        missing_required,
        unknown_fields,
    }
}

fn validate_value(question: &QuestionSpec, value: &Value) -> Option<ValidationError> {
    if !matches_type(question, value) {
        return Some(ValidationError {
            question_id: Some(question.id.clone()),
            path: Some(format!("/{}", question.id)),
            message: "type mismatch".into(),
            code: Some("type_mismatch".into()),
        });
    }

    if matches!(question.kind, QuestionType::List)
        && let Some(error) = validate_list(question, value)
    {
        return Some(error);
    }

    if let Some(constraint) = &question.constraint
        && let Some(error) = enforce_constraint(question, value, constraint)
    {
        return Some(error);
    }

    if matches!(question.kind, QuestionType::Enum)
        && let Some(choices) = &question.choices
        && let Some(text) = value.as_str()
        && !choices.contains(&text.to_string())
    {
        return Some(ValidationError {
            question_id: Some(question.id.clone()),
            path: Some(format!("/{}", question.id)),
            message: "invalid enum option".into(),
            code: Some("enum_mismatch".into()),
        });
    }

    None
}

fn matches_type(question: &QuestionSpec, value: &Value) -> bool {
    match question.kind {
        QuestionType::String | QuestionType::Enum => value.is_string(),
        QuestionType::Boolean => value.is_boolean(),
        QuestionType::Integer => value.is_i64(),
        QuestionType::Number => value.is_number(),
        QuestionType::List => value.is_array(),
    }
}

fn validate_list(question: &QuestionSpec, value: &Value) -> Option<ValidationError> {
    let list = match &question.list {
        Some(value) => value,
        None => {
            return Some(base_error(
                question,
                "list fields are not defined",
                "missing_list_definition",
            ));
        }
    };

    let items = match value.as_array() {
        Some(items) => items,
        None => {
            return Some(list_not_array_error(question));
        }
    };
    if let Some(min_items) = list.min_items
        && items.len() < min_items
    {
        return Some(list_count_error(
            question,
            min_items,
            items.len(),
            "not enough list entries",
            "min_items",
        ));
    }

    if let Some(max_items) = list.max_items
        && items.len() > max_items
    {
        return Some(list_count_error(
            question,
            max_items,
            items.len(),
            "too many list entries",
            "max_items",
        ));
    }

    for (idx, entry) in items.iter().enumerate() {
        let entry_map = match entry.as_object() {
            Some(map) => map,
            None => {
                return Some(list_entry_type_error(question, idx));
            }
        };

        for field in &list.fields {
            match entry_map.get(&field.id) {
                None => {
                    if field.required {
                        return Some(list_field_missing_error(question, idx, &field.id));
                    }
                }
                Some(field_value) => {
                    if let Some(error) = validate_value(field, field_value) {
                        return Some(apply_list_context(question, idx, field, error));
                    }
                }
            }
        }
    }

    None
}

fn apply_list_context(
    question: &QuestionSpec,
    idx: usize,
    field: &QuestionSpec,
    mut error: ValidationError,
) -> ValidationError {
    error.question_id = Some(format!("{}[{}].{}", question.id, idx, field.id));
    error.path = Some(format!("/{}/{}/{}", question.id, idx, field.id));
    error
}

fn list_count_error(
    question: &QuestionSpec,
    threshold: usize,
    actual: usize,
    message: &str,
    code: &str,
) -> ValidationError {
    ValidationError {
        question_id: Some(question.id.clone()),
        path: Some(format!("/{}", question.id)),
        message: format!("{} (expected {}, got {})", message, threshold, actual),
        code: Some(code.into()),
    }
}

fn list_entry_type_error(question: &QuestionSpec, idx: usize) -> ValidationError {
    ValidationError {
        question_id: Some(question.id.clone()),
        path: Some(format!("/{}/{}", question.id, idx)),
        message: "list entry must be an object".into(),
        code: Some("entry_type".into()),
    }
}

fn list_not_array_error(question: &QuestionSpec) -> ValidationError {
    ValidationError {
        question_id: Some(question.id.clone()),
        path: Some(format!("/{}", question.id)),
        message: "list value must be an array".into(),
        code: Some("list_type".into()),
    }
}

fn list_field_missing_error(
    question: &QuestionSpec,
    idx: usize,
    field_id: &str,
) -> ValidationError {
    ValidationError {
        question_id: Some(format!("{}[{}].{}", question.id, idx, field_id)),
        path: Some(format!("/{}/{}/{}", question.id, idx, field_id)),
        message: format!("field '{}' is required", field_id),
        code: Some("missing_field".into()),
    }
}

fn enforce_constraint(
    question: &QuestionSpec,
    value: &Value,
    constraint: &crate::spec::question::Constraint,
) -> Option<ValidationError> {
    if let Some(pattern) = &constraint.pattern
        && let Some(text) = value.as_str()
        && let Ok(regex) = Regex::new(pattern)
        && !regex.is_match(text)
    {
        return Some(base_error(
            question,
            "value does not match pattern",
            "pattern_mismatch",
        ));
    }

    if let Some(min_len) = constraint.min_len
        && let Some(text) = value.as_str()
        && text.len() < min_len
    {
        return Some(base_error(
            question,
            "string shorter than min length",
            "min_length",
        ));
    }

    if let Some(max_len) = constraint.max_len
        && let Some(text) = value.as_str()
        && text.len() > max_len
    {
        return Some(base_error(
            question,
            "string longer than max length",
            "max_length",
        ));
    }

    if let Some(min) = constraint.min
        && let Some(value) = value.as_f64()
        && value < min
    {
        return Some(base_error(question, "value below minimum", "min"));
    }

    if let Some(max) = constraint.max
        && let Some(value) = value.as_f64()
        && value > max
    {
        return Some(base_error(question, "value above maximum", "max"));
    }

    None
}

fn base_error(question: &QuestionSpec, message: &str, code: &str) -> ValidationError {
    ValidationError {
        question_id: Some(question.id.clone()),
        path: Some(format!("/{}", question.id)),
        message: message.into(),
        code: Some(code.into()),
    }
}
