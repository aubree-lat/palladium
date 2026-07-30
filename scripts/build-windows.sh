#!/usr/bin/env bash
set -euo pipefail

TARGET="x86_64-pc-windows-msvc"
NSIS_DIR="${HOME}/.cache/tauri/NSIS"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [ ! -x "${NSIS_DIR}/makensis.exe" ]; then
  echo "NSIS shim missing - run scripts/setup-windows-cross.sh first" >&2
  exit 1
fi

export PATH="${NSIS_DIR}:${PATH}"

cd "$ROOT"
npx tauri build --runner cargo-xwin --target "$TARGET" --bundles nsis "$@"

out="src-tauri/target/${TARGET}/release"
printf '\n\033[1;35m==>\033[0m Artifacts:\n'
ls -lh "${out}/palladium.exe" "${out}/bundle/nsis/"*.exe 2>/dev/null || true
