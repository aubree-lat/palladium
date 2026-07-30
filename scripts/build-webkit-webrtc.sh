#!/usr/bin/env bash
#
# Build WebKitGTK with WebRTC enabled, into a private prefix.
#
# Why this exists: WebKitGTK gates ENABLE_WEB_RTC behind
# ENABLE_EXPERIMENTAL_FEATURES, which is OFF in release builds, so every
# distribution ships a WebKitGTK where RTCPeerConnection does not exist. That is
# what makes Discord voice impossible in any wry/Tauri client on Linux.
# ENABLE_MEDIA_STREAM is already ON upstream, which is why getUserMedia works
# but WebRTC does not.
#
# Installs to $PREFIX rather than /usr on purpose: replacing the system
# libwebkit2gtk-4.1 would put every dependent package at risk (lutris,
# evolution-data-server, gnome-boxes, cloudflare-warp-bin). Point consumers at
# this build with LD_LIBRARY_PATH instead. No root required.
#
# Requires: gperf unifdef ruby ruby-stdlib cmake ninja lld gst-plugins-bad
set -euo pipefail

VER="${WEBKIT_VERSION:-2.52.5}"
PREFIX="${WEBKIT_PREFIX:-$HOME/.local/opt/webkit-webrtc}"
SRC="${WEBKIT_SRC:-$HOME/.cache/webkit-build}"
JOBS="${JOBS:-$(nproc)}"
USE_LIBRICE="${USE_LIBRICE:-ON}"

export PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"

info() { printf '\033[1;35m==>\033[0m %s\n' "$*"; }

for t in cmake ninja gperf unifdef ruby; do
  command -v "$t" >/dev/null || {
    echo "missing build tool: $t" >&2
    echo "install with: sudo pacman -S --needed gperf unifdef ruby ruby-stdlib" >&2
    exit 1
  }
done

if [ "$USE_LIBRICE" = "ON" ]; then
  missing=""
  for m in rice-proto rice-io; do
    pkg-config --exists "$m" 2>/dev/null || missing="$missing $m"
  done
  if [ -n "$missing" ]; then
    echo "USE_LIBRICE=ON but pkg-config cannot find:$missing" >&2
    echo "Build it first:  bash scripts/build-librice.sh" >&2
    echo "Or build without it:  USE_LIBRICE=OFF bash scripts/build-webkit-webrtc.sh" >&2
    echo "(USE_LIBRICE=OFF gives the legacy libnice ICE path, where voice fails" >&2
    echo " at \"Checking route\" — it is not recommended.)" >&2
    exit 1
  fi
  info "librice found: rice-proto $(pkg-config --modversion rice-proto), rice-io $(pkg-config --modversion rice-io)"
fi

mkdir -p "$SRC"
cd "$SRC"

if [ ! -f "webkitgtk-$VER.tar.xz" ]; then
  info "downloading webkitgtk-$VER"
  curl -sSLO "https://webkitgtk.org/releases/webkitgtk-$VER.tar.xz"
fi

if [ ! -d "webkitgtk-$VER" ]; then
  info "extracting"
  tar xf "webkitgtk-$VER.tar.xz"
fi

cd "webkitgtk-$VER"

# A failed configure can leave a stale cache that survives flag changes.
# CLEAN=1 forces a fresh tree; otherwise cmake reconfigures incrementally.
if [ "${CLEAN:-0}" = "1" ]; then
  info "removing previous build tree"
  rm -rf build
fi

info "configuring (prefix: $PREFIX)"
# Flags mirror Arch's PKGBUILD so behaviour matches the system build, with
# WebRTC turned on and documentation dropped to save build time. MiniBrowser is
# kept because it is the quickest way to sanity-check the result.
cmake -B build -G Ninja \
  -D PORT=GTK \
  -D CMAKE_BUILD_TYPE=Release \
  -D CMAKE_INSTALL_PREFIX="$PREFIX" \
  -D CMAKE_INSTALL_LIBDIR=lib \
  -D CMAKE_INSTALL_LIBEXECDIR=lib \
  -D ENABLE_WEB_RTC=ON \
  -D ENABLE_MEDIA_STREAM=ON \
  -D ENABLE_MEDIA_RECORDER=ON \
  -D USE_LIBRICE="$USE_LIBRICE" \
  -D ENABLE_DOCUMENTATION=OFF \
  -D ENABLE_MINIBROWSER=ON \
  -D ENABLE_SPEECH_SYNTHESIS=OFF \
  -D USE_FLITE=OFF \
  -D USE_GTK4=OFF \
  -D USE_LIBBACKTRACE=OFF \
  -D USE_SOUP2=OFF \
  -D CMAKE_EXE_LINKER_FLAGS="-fuse-ld=lld" \
  -D CMAKE_SHARED_LINKER_FLAGS="-fuse-ld=lld"

info "confirming WebRTC actually got enabled before spending hours on it"
if ! grep -qE "^ENABLE_WEB_RTC:.*=ON" build/CMakeCache.txt; then
  echo "ENABLE_WEB_RTC did not stick - aborting" >&2
  grep -E "ENABLE_WEB_RTC|ENABLE_MEDIA_STREAM" build/CMakeCache.txt >&2 || true
  exit 1
fi
grep -E "^(ENABLE_WEB_RTC|ENABLE_MEDIA_STREAM|ENABLE_MEDIA_RECORDER):" build/CMakeCache.txt

info "building with $JOBS jobs (this takes 1-3 hours)"
ninja -C build -j "$JOBS"

info "installing to $PREFIX"
ninja -C build install

info "done"
echo
echo "Verify with:"
echo "  LD_LIBRARY_PATH=$PREFIX/lib python3 scripts/webkit-probe.py"
echo
echo "Run Palladium against it with:"
echo "  LD_LIBRARY_PATH=$PREFIX/lib ./src-tauri/target/debug/palladium"
