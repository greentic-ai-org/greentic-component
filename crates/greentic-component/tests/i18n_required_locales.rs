#![cfg(feature = "cli")]

use std::collections::BTreeMap;
use std::path::Path;

fn read_locale_dir(dir: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let entries = std::fs::read_dir(dir).expect("read locale directory");
    for entry in entries {
        let entry = entry.expect("read locale entry");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("locale file name")
            .to_string();
        let raw = std::fs::read_to_string(&path).expect("read locale file");
        out.insert(name, raw);
    }
    out
}

#[test]
fn required_wizard_locales_exist() {
    let crate_i18n = Path::new(env!("CARGO_MANIFEST_DIR")).join("i18n");
    let root_i18n = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("i18n");
    assert!(crate_i18n.is_dir(), "missing {}", crate_i18n.display());
    assert!(root_i18n.is_dir(), "missing {}", root_i18n.display());

    let crate_locales = read_locale_dir(&crate_i18n);
    let root_locales = read_locale_dir(&root_i18n);
    assert!(
        !crate_locales.is_empty(),
        "crate i18n dir is empty: {}",
        crate_i18n.display()
    );
    assert_eq!(
        crate_locales, root_locales,
        "crate and root locale catalogs diverged"
    );
}
