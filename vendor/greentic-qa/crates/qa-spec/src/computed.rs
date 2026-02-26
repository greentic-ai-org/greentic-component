use crate::spec::form::FormSpec;
use serde_json::{Map, Value};

/// Builds a context used by expressions so answers can be addressed via question ids and the special `answers` key.
pub fn build_expression_context(answers: &Value) -> Value {
    let mut map = Map::new();
    if let Some(object) = answers.as_object() {
        for (key, value) in object {
            map.insert(key.clone(), value.clone());
        }
    }
    map.insert("answers".into(), answers.clone());
    Value::Object(map)
}

/// Applies computed expressions defined in the spec and returns a new answer map that includes the derived values.
pub fn apply_computed_answers(spec: &FormSpec, answers: &Value) -> Value {
    let mut map = answers.as_object().cloned().unwrap_or_default();

    for question in &spec.questions {
        if let Some(expr) = &question.computed {
            if map.contains_key(&question.id) && question.computed_overridable {
                continue;
            }
            let context = build_expression_context(&Value::Object(map.clone()));
            if let Some(value) = expr.evaluate_value(&context) {
                map.insert(question.id.clone(), value);
            } else {
                map.remove(&question.id);
            }
        }
    }

    Value::Object(map)
}
