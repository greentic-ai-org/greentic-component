use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::{Expr, FormSpec, QuestionSpec, spec::validation::CrossFieldValidation};

#[derive(Debug, Error)]
pub enum IncludeError {
    #[error("missing include target '{form_ref}'")]
    MissingIncludeTarget { form_ref: String },
    #[error("include cycle detected: {chain:?}")]
    IncludeCycleDetected { chain: Vec<String> },
    #[error("duplicate question id after include expansion: '{question_id}'")]
    DuplicateQuestionId { question_id: String },
}

/// Expand includes recursively into a flattened form spec with deterministic ordering.
pub fn expand_includes(
    root: &FormSpec,
    registry: &BTreeMap<String, FormSpec>,
) -> Result<FormSpec, IncludeError> {
    let mut chain = Vec::new();
    let mut seen = BTreeSet::new();
    expand_form(root, "", registry, &mut chain, &mut seen)
}

fn expand_form(
    form: &FormSpec,
    prefix: &str,
    registry: &BTreeMap<String, FormSpec>,
    chain: &mut Vec<String>,
    seen_ids: &mut BTreeSet<String>,
) -> Result<FormSpec, IncludeError> {
    if chain.contains(&form.id) {
        let start = chain.iter().position(|id| id == &form.id).unwrap_or(0);
        let mut cycle = chain[start..].to_vec();
        cycle.push(form.id.clone());
        return Err(IncludeError::IncludeCycleDetected { chain: cycle });
    }
    chain.push(form.id.clone());

    let mut out = form.clone();
    out.questions.clear();
    out.validations.clear();
    out.includes.clear();

    for question in &form.questions {
        let question = apply_prefix_question(question, prefix);
        if !seen_ids.insert(question.id.clone()) {
            return Err(IncludeError::DuplicateQuestionId {
                question_id: question.id,
            });
        }
        out.questions.push(question);
    }

    for validation in &form.validations {
        out.validations
            .push(apply_prefix_validation(validation, prefix));
    }

    for include in &form.includes {
        let included =
            registry
                .get(&include.form_ref)
                .ok_or_else(|| IncludeError::MissingIncludeTarget {
                    form_ref: include.form_ref.clone(),
                })?;
        let nested_prefix = combine_prefix(prefix, include.prefix.as_deref());
        let expanded = expand_form(included, &nested_prefix, registry, chain, seen_ids)?;
        out.questions.extend(expanded.questions);
        out.validations.extend(expanded.validations);
    }

    chain.pop();
    Ok(out)
}

fn apply_prefix_validation(
    validation: &CrossFieldValidation,
    prefix: &str,
) -> CrossFieldValidation {
    if prefix.is_empty() {
        return validation.clone();
    }
    let mut out = validation.clone();
    out.id = out.id.map(|id| prefix_key(prefix, &id));
    out.fields = out
        .fields
        .iter()
        .map(|field| prefix_key(prefix, field))
        .collect();
    out.condition = prefix_expr(out.condition, prefix);
    out
}

fn apply_prefix_question(question: &QuestionSpec, prefix: &str) -> QuestionSpec {
    if prefix.is_empty() {
        return question.clone();
    }
    let mut out = question.clone();
    out.id = prefix_key(prefix, &out.id);
    out.visible_if = out.visible_if.map(|expr| prefix_expr(expr, prefix));
    out.computed = out.computed.map(|expr| prefix_expr(expr, prefix));
    if let Some(list) = &mut out.list {
        list.fields = list
            .fields
            .iter()
            .map(|field| apply_prefix_question(field, prefix))
            .collect();
    }
    out
}

fn prefix_expr(expr: Expr, prefix: &str) -> Expr {
    match expr {
        Expr::Answer { path } => Expr::Answer {
            path: prefix_path(prefix, &path),
        },
        Expr::IsSet { path } => Expr::IsSet {
            path: prefix_path(prefix, &path),
        },
        Expr::And { expressions } => Expr::And {
            expressions: expressions
                .into_iter()
                .map(|expr| prefix_expr(expr, prefix))
                .collect(),
        },
        Expr::Or { expressions } => Expr::Or {
            expressions: expressions
                .into_iter()
                .map(|expr| prefix_expr(expr, prefix))
                .collect(),
        },
        Expr::Not { expression } => Expr::Not {
            expression: Box::new(prefix_expr(*expression, prefix)),
        },
        Expr::Eq { left, right } => Expr::Eq {
            left: Box::new(prefix_expr(*left, prefix)),
            right: Box::new(prefix_expr(*right, prefix)),
        },
        Expr::Ne { left, right } => Expr::Ne {
            left: Box::new(prefix_expr(*left, prefix)),
            right: Box::new(prefix_expr(*right, prefix)),
        },
        Expr::Lt { left, right } => Expr::Lt {
            left: Box::new(prefix_expr(*left, prefix)),
            right: Box::new(prefix_expr(*right, prefix)),
        },
        Expr::Lte { left, right } => Expr::Lte {
            left: Box::new(prefix_expr(*left, prefix)),
            right: Box::new(prefix_expr(*right, prefix)),
        },
        Expr::Gt { left, right } => Expr::Gt {
            left: Box::new(prefix_expr(*left, prefix)),
            right: Box::new(prefix_expr(*right, prefix)),
        },
        Expr::Gte { left, right } => Expr::Gte {
            left: Box::new(prefix_expr(*left, prefix)),
            right: Box::new(prefix_expr(*right, prefix)),
        },
        other => other,
    }
}

fn prefix_path(prefix: &str, path: &str) -> String {
    if path.is_empty() || path.starts_with('/') || prefix.is_empty() {
        return path.to_string();
    }
    format!("{}.{}", prefix, path)
}

fn prefix_key(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{}.{}", prefix, key)
    }
}

fn combine_prefix(parent: &str, child: Option<&str>) -> String {
    match (parent.is_empty(), child.unwrap_or("").is_empty()) {
        (true, true) => String::new(),
        (false, true) => parent.to_string(),
        (true, false) => child.unwrap_or_default().to_string(),
        (false, false) => format!("{}.{}", parent, child.unwrap_or_default()),
    }
}
