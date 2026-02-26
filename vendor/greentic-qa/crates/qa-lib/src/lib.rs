use component_qa::{render_card, render_json_ui, render_text, submit_patch};
use qa_spec::AnswerSet;
use serde_json::{Map, Value, json};
use thiserror::Error;

pub use qa_spec::i18n::ResolvedI18nMap;

#[derive(Clone, Debug)]
pub enum WizardFrontend {
    Text,
    JsonUi,
    Card,
}

#[derive(Clone, Debug, Default)]
pub struct I18nConfig {
    pub locale: Option<String>,
    pub resolved: Option<ResolvedI18nMap>,
    pub debug: bool,
}

#[derive(Clone, Debug)]
pub struct WizardRunConfig {
    pub spec_json: String,
    pub initial_answers_json: Option<String>,
    pub frontend: WizardFrontend,
    pub i18n: I18nConfig,
    pub verbose: bool,
}

#[derive(Clone, Debug)]
pub struct WizardRunResult {
    pub answer_set: AnswerSet,
    pub answer_set_cbor_hex: String,
}

#[derive(Clone, Debug)]
pub struct ValidationOrProgress {
    pub status: String,
    pub response_json: String,
}

#[derive(Debug, Error)]
pub enum QaLibError {
    #[error("json parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("wizard needs interaction")]
    NeedsInteraction,
    #[error("component error: {0}")]
    Component(String),
    #[error("invalid patch: {0}")]
    InvalidPatch(String),
    #[error("wizard payload missing field '{0}'")]
    MissingField(String),
    #[error("validation failed: {0}")]
    Validation(String),
}

pub struct QaRunner;

pub type AnswerProvider = dyn FnMut(&str, &Value) -> Result<Value, QaLibError>;

impl QaRunner {
    pub fn run_wizard(
        config: WizardRunConfig,
        mut answer_provider: Option<&mut AnswerProvider>,
    ) -> Result<WizardRunResult, QaLibError> {
        let mut driver = WizardDriver::new(config)?;

        loop {
            let _payload = driver.next_payload_json()?;
            if driver.is_complete() {
                break;
            }

            let ui_raw = driver
                .last_ui_json()
                .ok_or_else(|| QaLibError::MissingField("last_ui_json".into()))?
                .to_string();
            let ui: Value = serde_json::from_str(&ui_raw)?;
            let question_id = ui
                .get("next_question_id")
                .and_then(Value::as_str)
                .ok_or_else(|| QaLibError::MissingField("next_question_id".into()))?
                .to_string();
            let question = find_question(&ui, &question_id)?;

            let provider = answer_provider
                .as_mut()
                .ok_or(QaLibError::NeedsInteraction)?;
            let answer = provider(&question_id, &question)?;
            let patch = json!({ question_id: answer }).to_string();
            let submit = driver.submit_patch_json(&patch)?;
            if submit.status == "error" {
                return Err(QaLibError::Validation(submit.response_json));
            }
        }

        driver.finish()
    }

    pub fn run_wizard_non_interactive(
        config: WizardRunConfig,
    ) -> Result<WizardRunResult, QaLibError> {
        Self::run_wizard(config, None)
    }
}

pub struct WizardDriver {
    form_id: String,
    spec_version: String,
    config_json: String,
    ctx_json: String,
    frontend: WizardFrontend,
    answers: Value,
    complete: bool,
    last_ui_json: Option<String>,
}

impl WizardDriver {
    pub fn new(config: WizardRunConfig) -> Result<Self, QaLibError> {
        let spec_value: Value = serde_json::from_str(&config.spec_json)?;
        let form_id = spec_value
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| QaLibError::MissingField("id".into()))?
            .to_string();
        let spec_version = spec_value
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or("0.0.0")
            .to_string();

        let answers = if let Some(raw) = config.initial_answers_json {
            let parsed: Value = serde_json::from_str(&raw)?;
            normalize_answers(parsed)
        } else {
            Value::Object(Map::new())
        };

        Ok(Self {
            form_id,
            spec_version,
            config_json: json!({ "form_spec_json": config.spec_json }).to_string(),
            ctx_json: build_ctx_json(&config.i18n),
            frontend: config.frontend,
            answers,
            complete: false,
            last_ui_json: None,
        })
    }

    pub fn next_payload_json(&mut self) -> Result<String, QaLibError> {
        let answers_json = self.answers.to_string();

        let ui_raw = render_json_ui(
            &self.form_id,
            &self.config_json,
            &self.ctx_json,
            &answers_json,
        );
        let ui_value = parse_component_result(&ui_raw)?;
        self.complete = ui_value
            .get("status")
            .and_then(Value::as_str)
            .is_some_and(|status| status == "complete");
        self.last_ui_json = Some(ui_raw.clone());

        match self.frontend {
            WizardFrontend::JsonUi => Ok(ui_raw),
            WizardFrontend::Card => {
                let card_raw = render_card(
                    &self.form_id,
                    &self.config_json,
                    &self.ctx_json,
                    &answers_json,
                );
                parse_component_result(&card_raw)?;
                Ok(card_raw)
            }
            WizardFrontend::Text => {
                let text = render_text(
                    &self.form_id,
                    &self.config_json,
                    &self.ctx_json,
                    &answers_json,
                );
                let wrapped = json!({
                    "text": text,
                    "status": ui_value.get("status").cloned().unwrap_or(Value::String("need_input".into())),
                    "next_question_id": ui_value.get("next_question_id").cloned().unwrap_or(Value::Null),
                    "progress": ui_value.get("progress").cloned().unwrap_or_else(|| json!({"answered":0,"total":0}))
                });
                Ok(wrapped.to_string())
            }
        }
    }

    pub fn submit_patch_json(
        &mut self,
        patch_json: &str,
    ) -> Result<ValidationOrProgress, QaLibError> {
        let patch_value: Value = serde_json::from_str(patch_json)?;
        let patch_object = patch_value.as_object().ok_or_else(|| {
            QaLibError::InvalidPatch(
                "patch_json must be a JSON object map of question_id -> value".into(),
            )
        })?;
        if patch_object.is_empty() {
            return Err(QaLibError::InvalidPatch(
                "patch_json cannot be empty".into(),
            ));
        }

        let mut last_value = Value::Null;

        for (question_id, value) in patch_object {
            let value_json = serde_json::to_string(value)?;
            let submit_raw = submit_patch(
                &self.form_id,
                &self.config_json,
                &self.ctx_json,
                &self.answers.to_string(),
                question_id,
                &value_json,
            );
            let submit_value = parse_component_result(&submit_raw)?;
            if let Some(answers) = submit_value.get("answers") {
                self.answers = normalize_answers(answers.clone());
            }
            if submit_value
                .get("status")
                .and_then(Value::as_str)
                .is_some_and(|status| status == "complete")
            {
                self.complete = true;
            }
            last_value = submit_value;
        }

        let status = last_value
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("need_input")
            .to_string();

        Ok(ValidationOrProgress {
            status,
            response_json: serde_json::to_string(&last_value)?,
        })
    }

    pub fn is_complete(&self) -> bool {
        self.complete
    }

    pub fn last_ui_json(&self) -> Option<&str> {
        self.last_ui_json.as_deref()
    }

    pub fn finish(self) -> Result<WizardRunResult, QaLibError> {
        if !self.complete {
            return Err(QaLibError::NeedsInteraction);
        }

        let answer_set = AnswerSet {
            form_id: self.form_id,
            spec_version: self.spec_version,
            answers: self.answers,
            meta: None,
        };

        let cbor = answer_set
            .to_cbor()
            .map_err(|err| QaLibError::Component(err.to_string()))?;

        Ok(WizardRunResult {
            answer_set,
            answer_set_cbor_hex: encode_hex(&cbor),
        })
    }
}

fn build_ctx_json(i18n: &I18nConfig) -> String {
    let mut map = Map::new();
    if let Some(locale) = &i18n.locale {
        map.insert("locale".into(), Value::String(locale.clone()));
    }
    if let Some(resolved) = &i18n.resolved
        && let Ok(value) = serde_json::to_value(resolved)
    {
        map.insert("i18n_resolved".into(), value);
    }
    if i18n.debug {
        map.insert("i18n_debug".into(), Value::Bool(true));
        map.insert("debug_i18n".into(), Value::Bool(true));
    }
    Value::Object(map).to_string()
}

fn parse_component_result(raw: &str) -> Result<Value, QaLibError> {
    let value: Value = serde_json::from_str(raw)?;
    if let Some(error) = value.get("error").and_then(Value::as_str) {
        Err(QaLibError::Component(error.to_string()))
    } else {
        Ok(value)
    }
}

fn normalize_answers(value: Value) -> Value {
    if value.is_object() {
        value
    } else {
        Value::Object(Map::new())
    }
}

fn find_question(ui: &Value, question_id: &str) -> Result<Value, QaLibError> {
    let question = ui
        .get("questions")
        .and_then(Value::as_array)
        .and_then(|questions| {
            questions
                .iter()
                .find(|question| question.get("id").and_then(Value::as_str) == Some(question_id))
                .cloned()
        })
        .ok_or_else(|| QaLibError::MissingField(format!("questions[{}]", question_id)))?;
    Ok(question)
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{:02x}", byte);
    }
    out
}
