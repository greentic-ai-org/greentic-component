use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// i18n text descriptor used by form/question display fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct I18nText {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<BTreeMap<String, Value>>,
}

/// Pre-resolved i18n map injected by adapters/callers.
pub type ResolvedI18nMap = BTreeMap<String, String>;

pub fn resolve_i18n_text(
    fallback: &str,
    text: Option<&I18nText>,
    resolved: Option<&ResolvedI18nMap>,
) -> String {
    resolve_i18n_text_with_locale(fallback, text, resolved, None, None)
}

pub fn resolve_i18n_text_with_locale(
    fallback: &str,
    text: Option<&I18nText>,
    resolved: Option<&ResolvedI18nMap>,
    requested_locale: Option<&str>,
    default_locale: Option<&str>,
) -> String {
    let Some(text) = text else {
        return fallback.to_string();
    };
    let Some(resolved) = resolved else {
        return fallback.to_string();
    };
    let Some(base) = resolve_by_locale(resolved, &text.key, requested_locale, default_locale)
    else {
        return fallback.to_string();
    };

    interpolate_args(base, text.args.as_ref())
}

fn resolve_by_locale<'a>(
    resolved: &'a ResolvedI18nMap,
    key: &str,
    requested_locale: Option<&str>,
    default_locale: Option<&str>,
) -> Option<&'a str> {
    for locale in [requested_locale, default_locale].iter().flatten() {
        if let Some(value) = resolved.get(&format!("{}:{}", locale, key)) {
            return Some(value);
        }
        if let Some(value) = resolved.get(&format!("{}/{}", locale, key)) {
            return Some(value);
        }
    }
    resolved.get(key).map(String::as_str)
}

fn interpolate_args(template: &str, args: Option<&BTreeMap<String, Value>>) -> String {
    let Some(args) = args else {
        return template.to_string();
    };
    let mut output = template.to_string();
    for (name, value) in args {
        let token = format!("{{{}}}", name);
        let value_text = match value {
            Value::String(v) => v.clone(),
            _ => value.to_string(),
        };
        output = output.replace(&token, &value_text);
    }
    output
}
