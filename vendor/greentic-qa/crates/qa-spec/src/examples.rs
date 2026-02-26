use serde_json::{Map, Number, Value};

use crate::spec::form::FormSpec;
use crate::spec::question::{QuestionSpec, QuestionType};
use crate::visibility::VisibilityMap;

pub fn generate(spec: &FormSpec, visibility: &VisibilityMap) -> Value {
    let mut output = Map::new();

    for question in &spec.questions {
        if !visibility.get(&question.id).copied().unwrap_or(true) {
            continue;
        }
        output.insert(question.id.clone(), example_for(question));
    }

    Value::Object(output)
}

fn example_for(question: &QuestionSpec) -> Value {
    if let Some(default_value) = &question.default_value {
        return Value::String(default_value.clone());
    }

    match question.kind {
        QuestionType::String | QuestionType::Enum => {
            Value::String(format!("example-{}", question.id))
        }
        QuestionType::Boolean => Value::Bool(false),
        QuestionType::Integer => Value::Number(Number::from(1)),
        QuestionType::Number => {
            Value::Number(Number::from_f64(1.0).unwrap_or_else(|| Number::from(1)))
        }
        QuestionType::List => Value::Array(Vec::new()),
    }
}
