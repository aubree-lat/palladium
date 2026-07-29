#!/usr/bin/env bash
set -euo pipefail

NSIS_VERSION="3.10"
NSIS_DIR="${HOME}/.cache/tauri/NSIS"
TARGET="x86_64-pc-windows-msvc"

info() { printf '\033[1;35m==>\033[0m %s\n' "$*"; }

for tool in rustup cargo wine python3 curl unzip; do
  command -v "$tool" >/dev/null || { echo "missing required tool: $tool" >&2; exit 1; }
done
for tool in clang-cl lld-link llvm-lib; do
  command -v "$tool" >/dev/null || {
    echo "missing $tool - install your distro's clang and lld packages" >&2; exit 1; }
done

info "Rust target $TARGET"
rustup target list --installed | grep -qx "$TARGET" || rustup target add "$TARGET"

info "cargo-xwin"
command -v cargo-xwin >/dev/null || cargo install cargo-xwin --locked

info "NSIS $NSIS_VERSION"
if [ ! -f "${NSIS_DIR}/Bin/makensis.exe" ]; then
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT
  curl -sSL -o "${tmp}/nsis.zip" \
    "https://downloads.sourceforge.net/project/nsis/NSIS%203/${NSIS_VERSION}/nsis-${NSIS_VERSION}.zip"
  unzip -q "${tmp}/nsis.zip" -d "$tmp"
  mkdir -p "$NSIS_DIR"
  cp -r "${tmp}/nsis-${NSIS_VERSION}/." "$NSIS_DIR/"
else
  info "NSIS already present, skipping download"
fi

info "makensis shim"
cat > "${NSIS_DIR}/makensis.exe" <<'SHIM'
#!/usr/bin/env bash
set -euo pipefail
NSIS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export NSISDIR="$(winepath -w "$NSIS_DIR" 2>/dev/null)"

args=()
for a in "$@"; do
  if [[ "$a" == *.nsi && -f "$a" ]]; then
    python3 - "$a" <<'PY'
import re, sys
p = sys.argv[1]
s = open(p, encoding="utf-8", errors="surrogateescape").read()
s = re.sub(r'"(/[^"\n]*)"',
           lambda m: '"Z:' + m.group(1).replace("/", "\\") + '"', s)
open(p, "w", encoding="utf-8", errors="surrogateescape").write(s)
PY
    args+=("$(winepath -w "$a")")
  elif [[ "$a" == /* && -e "$a" ]]; then
    args+=("$(winepath -w "$a")")
  else
    args+=("$a")
  fi
done

exec wine "$NSIS_DIR/Bin/makensis.exe" "${args[@]}"
SHIM
chmod +x "${NSIS_DIR}/makensis.exe"
ln -sf "${NSIS_DIR}/makensis.exe" "${NSIS_DIR}/makensis"

info "Done. Build with: npm run build:windows"
