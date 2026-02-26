#![cfg(feature = "cli")]

#[test]
fn required_wizard_locales_exist() {
    let root_en = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("i18n/en.json");
    assert!(root_en.exists(), "missing {}", root_en.display());
}
