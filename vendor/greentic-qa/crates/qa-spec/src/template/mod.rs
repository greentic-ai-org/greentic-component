use crate::secrets::{SecretAccessResult, SecretAction, evaluate};
use crate::spec::form::{FormSpec, SecretsPolicy};
use handlebars::{
    Context, Handlebars, Helper, HelperResult, Output, RenderContext, RenderError,
    RenderErrorReason,
};
use serde_json::{Map, Value};
use thiserror::Error;

/// Modes describing how missing values are handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionMode {
    /// Missing values emit an error.
    Strict,
    /// Missing values leave handlebars tokens untouched.
    Relaxed,
}

/// Context passed into templates.
#[derive(Debug, Clone)]
pub struct TemplateContext {
    pub payload: Value,
    pub state: Value,
    pub config: Value,
    pub answers: Value,
    pub secrets: Option<SecretsContext>,
}

impl Default for TemplateContext {
    fn default() -> Self {
        let empty = Value::Object(Map::new());
        Self {
            payload: empty.clone(),
            state: empty.clone(),
            config: empty.clone(),
            answers: empty,
            secrets: None,
        }
    }
}

impl TemplateContext {
    /// Replace the payload value.
    pub fn with_payload(mut self, payload: Value) -> Self {
        self.payload = payload;
        self
    }

    /// Replace the state value.
    pub fn with_state(mut self, state: Value) -> Self {
        self.state = state;
        self
    }

    /// Replace the config value.
    pub fn with_config(mut self, config: Value) -> Self {
        self.config = config;
        self
    }

    /// Replace answers.
    pub fn with_answers(mut self, answers: Value) -> Self {
        self.answers = answers;
        self
    }

    /// Set optional secrets with policy metadata.
    pub fn with_secrets(
        mut self,
        secrets: Value,
        policy: Option<SecretsPolicy>,
        host_available: bool,
    ) -> Self {
        self.secrets = Some(SecretsContext::new(secrets, policy, host_available));
        self
    }

    fn to_value(&self) -> Value {
        let mut map = Map::new();
        map.insert("payload".into(), self.payload.clone());
        map.insert("state".into(), self.state.clone());
        map.insert("config".into(), self.config.clone());
        map.insert("answers".into(), self.answers.clone());
        if let Some(secrets) = &self.secrets {
            map.insert("secrets".into(), secrets.value());
            map.insert("__secrets_meta".into(), secrets.meta());
        }
        Value::Object(map)
    }
}

fn render_error(message: impl Into<String>) -> RenderError {
    RenderErrorReason::Other(message.into()).into()
}

#[derive(Debug, Clone)]
pub struct SecretsContext {
    values: Map<String, Value>,
    denied: Map<String, Value>,
    host_available: bool,
}

impl SecretsContext {
    fn new(secrets: Value, policy: Option<SecretsPolicy>, host_available: bool) -> Self {
        let mut values = Map::new();
        let mut denied = Map::new();

        if let Some(map) = secrets.as_object() {
            for (key, value) in map {
                match evaluate(policy.as_ref(), key, SecretAction::Read, host_available) {
                    SecretAccessResult::Allowed => {
                        values.insert(key.clone(), value.clone());
                    }
                    SecretAccessResult::Denied(code) => {
                        denied.insert(key.clone(), Value::String(code.into()));
                    }
                    SecretAccessResult::HostUnavailable => {
                        denied.insert(key.clone(), Value::String("secret_host_unavailable".into()));
                    }
                }
            }
        }

        Self {
            values,
            denied,
            host_available,
        }
    }

    fn value(&self) -> Value {
        Value::Object(self.values.clone())
    }

    fn meta(&self) -> Value {
        let mut meta = Map::new();
        meta.insert("host_available".into(), Value::Bool(self.host_available));
        meta.insert("denied".into(), Value::Object(self.denied.clone()));
        Value::Object(meta)
    }
}

/// Errors raised while resolving templates.
#[derive(Debug, Error)]
pub enum TemplateError {
    #[error("template render error: {0}")]
    Render(String),
}

/// Handlebars-based template engine for QA specs.
pub struct TemplateEngine {
    handlebars: Handlebars<'static>,
    mode: ResolutionMode,
}

impl TemplateEngine {
    /// Construct a templating engine.
    pub fn new(mode: ResolutionMode) -> Self {
        let mut handlebars = Handlebars::new();
        register_default_helpers(&mut handlebars);
        handlebars.set_strict_mode(true);
        Self { handlebars, mode }
    }

    /// Resolve a string field using the provided context.
    pub fn resolve_string(
        &self,
        template: &str,
        ctx: &TemplateContext,
    ) -> Result<String, TemplateError> {
        match self.handlebars.render_template(template, &ctx.to_value()) {
            Ok(result) => Ok(result),
            Err(err) => match self.mode {
                ResolutionMode::Relaxed => Ok(template.to_owned()),
                ResolutionMode::Strict => Err(TemplateError::Render(err.to_string())),
            },
        }
    }

    /// Resolve templated strings within a `FormSpec`.
    pub fn resolve_form_spec(
        &self,
        spec: &FormSpec,
        ctx: &TemplateContext,
    ) -> Result<FormSpec, TemplateError> {
        let mut resolved = spec.clone();
        resolved.title = self.resolve_string(&spec.title, ctx)?;
        resolved.description = spec
            .description
            .as_ref()
            .map(|value| self.resolve_string(value, ctx))
            .transpose()?;

        resolved.presentation = if let Some(presentation) = &spec.presentation {
            let mut next = presentation.clone();
            next.intro = presentation
                .intro
                .as_ref()
                .map(|value| self.resolve_string(value, ctx))
                .transpose()?;
            next.theme = presentation
                .theme
                .as_ref()
                .map(|value| self.resolve_string(value, ctx))
                .transpose()?;
            Some(next)
        } else {
            None
        };

        resolved.questions = spec
            .questions
            .iter()
            .map(|question| {
                let mut updated = question.clone();
                updated.title = self.resolve_string(&question.title, ctx)?;
                updated.description = question
                    .description
                    .as_ref()
                    .map(|value| self.resolve_string(value, ctx))
                    .transpose()?;
                updated.default_value = question
                    .default_value
                    .as_ref()
                    .map(|value| self.resolve_string(value, ctx))
                    .transpose()?;
                Ok(updated)
            })
            .collect::<Result<Vec<_>, TemplateError>>()?;

        Ok(resolved)
    }
}

pub fn register_default_helpers(handlebars: &mut Handlebars<'static>) {
    handlebars.register_helper("get", Box::new(helper_get));
    handlebars.register_helper("default", Box::new(helper_default));
    handlebars.register_helper("eq", Box::new(helper_eq));
    handlebars.register_helper("and", Box::new(helper_and));
    handlebars.register_helper("or", Box::new(helper_or));
    handlebars.register_helper("not", Box::new(helper_not));
    handlebars.register_helper("len", Box::new(helper_len));
    handlebars.register_helper("json", Box::new(helper_json));
    handlebars.register_helper("secret", Box::new(helper_secret));
}

fn helper_get(
    h: &Helper,
    _: &Handlebars,
    ctx: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let path = h
        .param(0)
        .and_then(|param| param.value().as_str())
        .ok_or_else(|| render_error("get helper requires a path"))?;
    let pointer = to_pointer(path);
    let root = ctx.data();
    let value = root
        .pointer(&pointer)
        .map(value_to_string)
        .or_else(|| h.param(1).map(|param| value_to_string(param.value())))
        .unwrap_or_default();
    out.write(&value)?;
    Ok(())
}

fn helper_default(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let first = h.param(0).map(|param| param.value());
    let fallback = h.param(1).map(|param| param.value());
    let chosen = if let Some(value) = first {
        if is_truthy(value) {
            value_to_string(value)
        } else {
            fallback.map(value_to_string).unwrap_or_default()
        }
    } else {
        fallback.map(value_to_string).unwrap_or_default()
    };
    out.write(&chosen)?;
    Ok(())
}

fn helper_eq(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let left = h
        .param(0)
        .map(|param| param.value())
        .unwrap_or(&Value::Null);
    let right = h
        .param(1)
        .map(|param| param.value())
        .unwrap_or(&Value::Null);
    let result = left == right;
    out.write(&result.to_string())?;
    Ok(())
}

fn helper_and(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let mut truthy = true;
    for param in h.params() {
        truthy &= is_truthy(param.value());
        if !truthy {
            break;
        }
    }
    out.write(&truthy.to_string())?;
    Ok(())
}

fn helper_or(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let mut truthy = false;
    for param in h.params() {
        if is_truthy(param.value()) {
            truthy = true;
            break;
        }
    }
    out.write(&truthy.to_string())?;
    Ok(())
}

fn helper_not(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let value = h
        .param(0)
        .map(|param| param.value())
        .unwrap_or(&Value::Bool(false));
    out.write(&(!is_truthy(value)).to_string())?;
    Ok(())
}

fn helper_len(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let value = h
        .param(0)
        .map(|param| param.value())
        .unwrap_or(&Value::Null);
    let len = match value {
        Value::String(s) => s.len(),
        Value::Array(arr) => arr.len(),
        Value::Object(obj) => obj.len(),
        _ => 0,
    };
    out.write(&len.to_string())?;
    Ok(())
}

fn helper_json(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let value = h
        .param(0)
        .map(|param| param.value())
        .unwrap_or(&Value::Null);
    let serialized = serde_json::to_string(value).unwrap_or_default();
    out.write(&serialized)?;
    Ok(())
}

fn helper_secret(
    h: &Helper,
    _: &Handlebars,
    ctx: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let key = h
        .param(0)
        .and_then(|param| param.value().as_str())
        .ok_or_else(|| render_error("secret helper requires a key"))?;

    let root = ctx.data();
    let host_available = root
        .get("__secrets_meta")
        .and_then(Value::as_object)
        .and_then(|meta| meta.get("host_available"))
        .and_then(Value::as_bool)
        .unwrap_or(false);

    if !host_available {
        return Err(render_error("secret_host_unavailable"));
    }

    if let Some(Value::Object(secrets)) = root.get("secrets")
        && let Some(value) = secrets.get(key)
    {
        out.write(&value_to_string(value))?;
        return Ok(());
    }

    if let Some(denied) = root
        .get("__secrets_meta")
        .and_then(Value::as_object)
        .and_then(|meta| meta.get("denied"))
        .and_then(Value::as_object)
        && let Some(Value::String(code)) = denied.get(key)
    {
        return Err(render_error(code.clone()));
    }

    Err(render_error("secret_access_denied"))
}

fn to_pointer(path: &str) -> String {
    let cleaned = path.replace('.', "/");
    if cleaned.starts_with('/') {
        cleaned
    } else {
        format!("/{}", cleaned)
    }
}

fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::String(text) => !text.is_empty(),
        Value::Number(num) => num.as_f64().is_some_and(|n| n != 0.0),
        Value::Array(arr) => !arr.is_empty(),
        Value::Object(map) => !map.is_empty(),
    }
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(num) => num.to_string(),
        Value::Null => String::new(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}
