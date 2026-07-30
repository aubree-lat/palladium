#!/usr/bin/env bash
#
# Build librice and install it into the WebKitGTK prefix.
#
# WebKitGTK 2.52's GTK port defaults to USE_LIBRICE=ON for ICE. Building WebKit
# with USE_LIBRICE=OFF falls back to a legacy libnice path which fails to
# negotiate a route, so Discord voice sits at "Checking route" and reconnects
# forever with "libnice-WARNING: Could not find component 1 in stream 1".
#
# librice is a sans-IO ICE implementation in Rust and exposes the rice-io /
# rice-proto pkg-config modules WebKit looks for, via cargo-c. It is packaged in
# neither Arch's repos nor the AUR, hence building it here.
#
# Installs into the same prefix as the patched WebKitGTK so one LD_LIBRARY_PATH
# still covers everything at runtime. No root required.
set -euo pipefail

PREFIX="${WEBKIT_PREFIX:-$HOME/.local/opt/webkit-webrtc}"
SRC="${LIBRICE_SRC:-$HOME/.cache/librice}"

info() { printf '\033[1;35m==>\033[0m %s\n' "$*"; }

for t in cargo git pkg-config; do
  command -v "$t" >/dev/null || { echo "missing required tool: $t" >&2; exit 1; }
done

info "cargo-c"
if ! command -v cargo-cinstall >/dev/null; then
  cargo install cargo-c --locked
fi

info "librice source"
if [ -d "$SRC/.git" ]; then
  git -C "$SRC" pull --ff-only || info "could not fast-forward, using the existing checkout"
else
  git clone https://github.com/ystreet/librice "$SRC"
fi

cd "$SRC"

# rice-proto first: rice-io depends on it.
for crate in rice-proto rice-io; do
  info "building $crate"
  cargo cinstall -p "$crate" --release \
    --prefix="$PREFIX" \
    --libdir="$PREFIX/lib" \
    --includedir="$PREFIX/include"
done

info "verifying pkg-config can see them"
export PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
for m in rice-proto rice-io; do
  printf '  %-12s %s\n' "$m" "$(pkg-config --modversion "$m" 2>/dev/null || echo 'NOT FOUND')"
done

info "done"
echo
echo "Now rebuild WebKitGTK against it:"
echo "  bash scripts/build-webkit-webrtc.sh"
echo
echo "That script defaults to USE_LIBRICE=ON and will pick these up from $PREFIX."
