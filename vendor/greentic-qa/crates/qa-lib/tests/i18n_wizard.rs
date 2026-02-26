use greentic_qa_lib::{I18nConfig, ResolvedI18nMap, WizardDriver, WizardFrontend, WizardRunConfig};

#[test]
fn localized_title_is_rendered_with_resolved_map() {
    let spec = r#"
    {
      "id": "i18n-form",
      "title": "I18n Form",
      "version": "1.0.0",
      "presentation": {
        "default_locale": "nl-NL"
      },
      "questions": [
        {
          "id": "q1",
          "type": "string",
          "title": "Fallback title",
          "title_i18n": { "key": "q1.title" },
          "required": true
        }
      ]
    }
    "#;

    let mut resolved = ResolvedI18nMap::new();
    resolved.insert("q1.title".into(), "Naam".into());

    let mut driver = WizardDriver::new(WizardRunConfig {
        spec_json: spec.to_string(),
        initial_answers_json: None,
        frontend: WizardFrontend::JsonUi,
        i18n: I18nConfig {
            locale: Some("nl-NL".into()),
            resolved: Some(resolved),
            debug: false,
        },
        verbose: false,
    })
    .expect("driver should be created");

    let payload = driver.next_payload_json().expect("payload should render");
    assert!(payload.contains("Naam"));
}
