#!/usr/bin/env bash
#
# Run Palladium against a WebRTC-enabled WebKitGTK, so voice and video work.
#
# Distribution WebKitGTK builds leave ENABLE_WEB_RTC off, which means
# RTCPeerConnection does not exist and Discord refuses voice. Build the patched
# library first with scripts/build-webkit-webrtc.sh, then launch through this.
#
# The library is loaded by path rather than installed system-wide, so nothing
# else on the machine is affected.
set -euo pipefail

PREFIX="${WEBKIT_PREFIX:-$HOME/.local/opt/webkit-webrtc}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="${PALLADIUM_BIN:-$ROOT/src-tauri/target/debug/palladium}"

if [ ! -f "$PREFIX/lib/libwebkit2gtk-4.1.so" ]; then
  echo "No WebRTC-enabled WebKitGTK at $PREFIX" >&2
  echo "Build it first: bash scripts/build-webkit-webrtc.sh" >&2
  exit 1
fi

if [ ! -x "$BIN" ]; then
  echo "Palladium binary not found at $BIN" >&2
  echo "Build it first: cargo build --manifest-path src-tauri/Cargo.toml" >&2
  exit 1
fi

export LD_LIBRARY_PATH="$PREFIX/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export GI_TYPELIB_PATH="$PREFIX/lib/girepository-1.0${GI_TYPELIB_PATH:+:$GI_TYPELIB_PATH}"

exec "$BIN" "$@"
