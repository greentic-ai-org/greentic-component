use serde_json::{Map, Value};

use crate::spec::form::FormSpec;
use crate::spec::question::{Constraint, QuestionSpec, QuestionType};
use crate::visibility::VisibilityMap;

/// Generates an answer JSON schema restricted to the visible questions.
pub fn generate(spec: &FormSpec, visibility: &VisibilityMap) -> Value {
    let mut properties = Map::new();
    let mut required = Vec::new();

    for question in &spec.questions {
        if !visibility.get(&question.id).copied().unwrap_or(true) {
            continue;
        }
        let schema = question_schema(question);
        properties.insert(question.id.clone(), schema);
        if question.required {
            required.push(Value::String(question.id.clone()));
        }
    }

    let mut root = Map::new();
    root.insert("type".into(), Value::String("object".into()));
    root.insert("properties".into(), Value::Object(properties));
    if !required.is_empty() {
        root.insert("required".into(), Value::Array(required));
    }

    Value::Object(root)
}

fn question_schema(question: &QuestionSpec) -> Value {
    let mut schema = Map::new();
    match question.kind {
        QuestionType::String => {
            schema.insert("type".into(), Value::String("string".into()));
        }
        QuestionType::Boolean => {
            schema.insert("type".into(), Value::String("boolean".into()));
        }
        QuestionType::Integer => {
            schema.insert("type".into(), Value::String("integer".into()));
        }
        QuestionType::Number => {
            schema.insert("type".into(), Value::String("number".into()));
        }
        QuestionType::Enum => {
            schema.insert("type".into(), Value::String("string".into()));
            if let Some(choices) = &question.choices {
                schema.insert(
                    "enum".into(),
                    Value::Array(
                        choices
                            .iter()
                            .map(|value| Value::String(value.clone()))
                            .collect(),
                    ),
                );
            }
        }
        QuestionType::List => {
            schema.insert("type".into(), Value::String("array".into()));
            if let Some(list) = &question.list {
                if let Some(min_items) = list.min_items {
                    schema.insert("minItems".into(), Value::Number(min_items.into()));
                }
                if let Some(max_items) = list.max_items {
                    schema.insert("maxItems".into(), Value::Number(max_items.into()));
                }
                let mut item_props = Map::new();
                let mut required_fields = Vec::new();
                for field in &list.fields {
                    item_props.insert(field.id.clone(), question_schema(field));
                    if field.required {
                        required_fields.push(Value::String(field.id.clone()));
                    }
                }
                let mut item_schema = Map::new();
                item_schema.insert("type".into(), Value::String("object".into()));
                item_schema.insert("properties".into(), Value::Object(item_props));
                if !required_fields.is_empty() {
                    item_schema.insert("required".into(), Value::Array(required_fields));
                }
                schema.insert("items".into(), Value::Object(item_schema));
            } else {
                schema.insert("items".into(), Value::Object(Map::new()));
            }
        }
    }

    if let Some(Constraint {
        pattern,
        min,
        max,
        min_len,
        max_len,
    }) = &question.constraint
    {
        if let Some(pattern) = pattern {
            schema.insert("pattern".into(), Value::String(pattern.clone()));
        }
        if let Some(min) = min
            && let Some(num) = number_from_f64(*min)
        {
            schema.insert("minimum".into(), num);
        }
        if let Some(max) = max
            && let Some(num) = number_from_f64(*max)
        {
            schema.insert("maximum".into(), num);
        }
        if let Some(min_len) = min_len {
            schema.insert("minLength".into(), Value::Number((*min_len).into()));
        }
        if let Some(max_len) = max_len {
            schema.insert("maxLength".into(), Value::Number((*max_len).into()));
        }
    }

    if let Some(default_value) = &question.default_value {
        schema.insert("default".into(), Value::String(default_value.clone()));
    }

    if question.secret {
        schema.insert("x-secret".into(), Value::Bool(true));
    }

    Value::Object(schema)
}

fn number_from_f64(value: f64) -> Option<Value> {
    serde_json::Number::from_f64(value).map(Value::Number)
}
