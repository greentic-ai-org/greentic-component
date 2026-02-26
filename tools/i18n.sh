#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

MODE="${1:-all}"
AUTH_MODE="${AUTH_MODE:-auto}"
LOCALE="${LOCALE:-en}"
EN_PATH="${EN_PATH:-i18n/en.json}"
I18N_TRANSLATOR_MANIFEST="${I18N_TRANSLATOR_MANIFEST:-../greentic-i18n/Cargo.toml}"

usage() {
  cat <<'EOF'
Usage: tools/i18n.sh [translate|validate|status|all]

Environment overrides:
  EN_PATH=...                     English source file path (default: i18n/en.json)
  AUTH_MODE=...                   Translator auth mode for translate (default: auto)
  LOCALE=...                      CLI locale used for translator output (default: en)
  I18N_TRANSLATOR_MANIFEST=...    Path to greentic-i18n Cargo.toml

Examples:
  tools/i18n.sh all
  AUTH_MODE=api-key tools/i18n.sh translate
  EN_PATH=i18n/en.json tools/i18n.sh validate
EOF
}

run_translate() {
  cargo run --manifest-path "$I18N_TRANSLATOR_MANIFEST" -p greentic-i18n-translator -- \
    --locale "$LOCALE" \
    translate --langs all --en "$EN_PATH" --auth-mode "$AUTH_MODE"
}

run_validate() {
  cargo run --manifest-path "$I18N_TRANSLATOR_MANIFEST" -p greentic-i18n-translator -- \
    --locale "$LOCALE" \
    validate --langs all --en "$EN_PATH"
}

run_status() {
  cargo run --manifest-path "$I18N_TRANSLATOR_MANIFEST" -p greentic-i18n-translator -- \
    --locale "$LOCALE" \
    status --langs all --en "$EN_PATH"
}

if [[ "${MODE}" == "-h" || "${MODE}" == "--help" ]]; then
  usage
  exit 0
fi

case "$MODE" in
  translate) run_translate ;;
  validate) run_validate ;;
  status) run_status ;;
  all)
    run_translate
    run_validate
    run_status
    ;;
  *)
    echo "Unknown mode: $MODE" >&2
    usage
    exit 2
    ;;
esac
