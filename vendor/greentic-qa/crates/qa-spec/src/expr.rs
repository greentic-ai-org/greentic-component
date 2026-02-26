use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Lightweight expression AST used for `visible_if`, computed fields, and validations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Expr {
    Literal { value: Value },
    Var { path: String },
    Answer { path: String },
    IsSet { path: String },
    And { expressions: Vec<Expr> },
    Or { expressions: Vec<Expr> },
    Not { expression: Box<Expr> },
    Eq { left: Box<Expr>, right: Box<Expr> },
    Ne { left: Box<Expr>, right: Box<Expr> },
    Lt { left: Box<Expr>, right: Box<Expr> },
    Lte { left: Box<Expr>, right: Box<Expr> },
    Gt { left: Box<Expr>, right: Box<Expr> },
    Gte { left: Box<Expr>, right: Box<Expr> },
}

impl Expr {
    /// Evaluates the expression and returns a JSON value when possible.
    pub fn evaluate_value(&self, ctx: &Value) -> Option<Value> {
        match self {
            Expr::Literal { value } => Some(value.clone()),
            Expr::Var { path } => Self::lookup(ctx, path).cloned(),
            Expr::Answer { path } => Self::lookup_answer(ctx, path).cloned(),
            Expr::IsSet { path } => {
                let present = Self::lookup_answer(ctx, path).is_some();
                Some(Value::Bool(present))
            }
            Expr::And { expressions } => Self::evaluate_and(expressions, ctx),
            Expr::Or { expressions } => Self::evaluate_or(expressions, ctx),
            Expr::Not { expression } => expression
                .evaluate_bool(ctx)
                .map(|value| Value::Bool(!value)),
            Expr::Eq { left, right } => {
                let left_value = left.evaluate_value(ctx)?;
                let right_value = right.evaluate_value(ctx)?;
                Some(Value::Bool(left_value == right_value))
            }
            Expr::Ne { left, right } => {
                let left_value = left.evaluate_value(ctx)?;
                let right_value = right.evaluate_value(ctx)?;
                Some(Value::Bool(left_value != right_value))
            }
            Expr::Lt { left, right } => {
                Self::evaluate_compare(left, right, ctx, |o| matches!(o, std::cmp::Ordering::Less))
            }
            Expr::Lte { left, right } => Self::evaluate_compare(left, right, ctx, |o| {
                matches!(o, std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
            }),
            Expr::Gt { left, right } => Self::evaluate_compare(left, right, ctx, |o| {
                matches!(o, std::cmp::Ordering::Greater)
            }),
            Expr::Gte { left, right } => Self::evaluate_compare(left, right, ctx, |o| {
                matches!(o, std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
            }),
        }
    }

    /// Evaluates the expression and coerces the result into a boolean when possible.
    pub fn evaluate_bool(&self, ctx: &Value) -> Option<bool> {
        let value = self.evaluate_value(ctx)?;
        match value {
            Value::Bool(value) => Some(value),
            Value::Number(number) => number.as_f64().map(|value| value != 0.0),
            Value::String(text) => match text.to_lowercase().as_str() {
                "true" | "t" | "yes" | "y" | "1" => Some(true),
                "false" | "f" | "no" | "n" | "0" => Some(false),
                _ => None,
            },
            Value::Null => Some(false),
            _ => None,
        }
    }

    fn evaluate_and(expressions: &[Expr], ctx: &Value) -> Option<Value> {
        let mut seen_none = false;
        for expression in expressions {
            match expression.evaluate_bool(ctx) {
                Some(false) => return Some(Value::Bool(false)),
                Some(true) => continue,
                None => seen_none = true,
            }
        }
        if seen_none {
            None
        } else {
            Some(Value::Bool(true))
        }
    }

    fn evaluate_or(expressions: &[Expr], ctx: &Value) -> Option<Value> {
        let mut seen_none = false;
        for expression in expressions {
            match expression.evaluate_bool(ctx) {
                Some(true) => return Some(Value::Bool(true)),
                Some(false) => continue,
                None => seen_none = true,
            }
        }
        if seen_none {
            None
        } else {
            Some(Value::Bool(false))
        }
    }

    fn evaluate_compare<F>(left: &Expr, right: &Expr, ctx: &Value, predicate: F) -> Option<Value>
    where
        F: Fn(std::cmp::Ordering) -> bool,
    {
        let left_value = left.evaluate_value(ctx)?;
        let right_value = right.evaluate_value(ctx)?;
        let ordering = Self::compare_values(&left_value, &right_value)?;
        if predicate(ordering) {
            Some(Value::Bool(true))
        } else {
            Some(Value::Bool(false))
        }
    }

    fn compare_values(left: &Value, right: &Value) -> Option<std::cmp::Ordering> {
        match (left, right) {
            (Value::Number(left), Value::Number(right)) => {
                let left_num = left.as_f64()?;
                let right_num = right.as_f64()?;
                left_num.partial_cmp(&right_num)
            }
            (Value::String(left_text), Value::String(right_text)) => {
                Some(left_text.cmp(right_text))
            }
            _ => {
                if left == right {
                    Some(std::cmp::Ordering::Equal)
                } else {
                    None
                }
            }
        }
    }

    fn lookup<'a>(ctx: &'a Value, path: &str) -> Option<&'a Value> {
        let pointer = Self::normalize_pointer(path);
        ctx.pointer(&pointer)
    }

    fn lookup_answer<'a>(ctx: &'a Value, path: &str) -> Option<&'a Value> {
        if let Some(value) = ctx.get("answers") {
            Self::fetch_nested(value, path)
        } else {
            Self::fetch_nested(ctx, path)
        }
    }

    fn fetch_nested<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
        if path.starts_with('/') {
            return value.pointer(path);
        }
        let mut current = value;
        for segment in path.split('.') {
            if segment.is_empty() {
                continue;
            }
            current = if let Ok(index) = segment.parse::<usize>() {
                current.get(index)?
            } else {
                current.get(segment)?
            };
        }
        Some(current)
    }

    fn normalize_pointer(path: &str) -> String {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return "/".to_string();
        }
        if trimmed.starts_with('/') {
            return trimmed.to_string();
        }
        let cleaned = trimmed
            .trim_start_matches('/')
            .split('.')
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>();
        format!("/{}", cleaned.join("/"))
    }
}
