#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

mkdir -p i18n

langs=(
  ar ar-AE ar-DZ ar-EG ar-IQ ar-MA ar-SA ar-SD ar-SY ar-TN
  ay bg bn cs da de el en-GB es et fa fi fr gn gu hi hr ht hu
  id it ja km kn ko lo lt lv ml mr ms my nah ne nl no pa pl pt
  qu ro ru si sk sr sv ta te th tl tr uk ur vi zh
)

for lang in "${langs[@]}"; do
  path="i18n/${lang}.json"
  if [[ ! -f "$path" ]]; then
    printf "{\n}\n" > "$path"
    echo "created $path"
  fi
done
