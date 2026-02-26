pub const I18N_KEYS: &[&str] = &[
    "qa.default.title",
    "qa.default.description",
    "qa.default.enabled.label",
    "qa.default.enabled.help",
    "qa.setup.title",
    "qa.setup.description",
    "qa.setup.enabled.label",
    "qa.setup.enabled.help",
    "qa.update.title",
    "qa.remove.title",
];

pub fn all_keys() -> Vec<String> {
    I18N_KEYS.iter().map(|key| (*key).to_string()).collect()
}
