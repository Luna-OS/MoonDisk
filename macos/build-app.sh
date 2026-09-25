#!/usr/bin/env bash
# Builds MoonDisk.app for macOS — the SwiftUI app plus the Rust helper that
# does the disk work (moondisk-helper) — and a DMG.
#
#   macos/build-app.sh            universal (Apple Silicon + Intel) release build
#   macos/build-app.sh --quick    native architecture only, for CI checks
#
# Output: macos/build/MoonDisk.app and macos/build/MoonDisk_<version>_universal.dmg
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT=$PWD
OUT="$ROOT/macos/build"
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' src-tauri/Cargo.toml | head -n1)
QUICK=${1:-}

rm -rf "$OUT"
mkdir -p "$OUT"

if [[ "$QUICK" == "--quick" ]]; then
  ARCHS=("$(uname -m)")
else
  ARCHS=(arm64 x86_64)
fi

# --- Rust helper -----------------------------------------------------------
HELPERS=()
for arch in "${ARCHS[@]}"; do
  case "$arch" in
    arm64) target=aarch64-apple-darwin ;;
    x86_64) target=x86_64-apple-darwin ;;
    *) echo "unsupported architecture $arch" >&2; exit 1 ;;
  esac
  cargo build --manifest-path src-tauri/Cargo.toml --release \
    --no-default-features --features helper --bin moondisk-helper --target "$target"
  HELPERS+=("src-tauri/target/$target/release/moondisk-helper")
done
lipo -create -output "$OUT/moondisk-helper" "${HELPERS[@]}"

# --- SwiftUI app -----------------------------------------------------------
SWIFT_ARCH_FLAGS=()
for arch in "${ARCHS[@]}"; do
  SWIFT_ARCH_FLAGS+=(--arch "$arch")
done
swift build --package-path macos -c release "${SWIFT_ARCH_FLAGS[@]}"
BIN_DIR=$(swift build --package-path macos -c release "${SWIFT_ARCH_FLAGS[@]}" --show-bin-path)

# --- Bundle ----------------------------------------------------------------
APP="$OUT/MoonDisk.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN_DIR/MoonDisk" "$APP/Contents/MacOS/MoonDisk"
cp "$OUT/moondisk-helper" "$APP/Contents/MacOS/moondisk-helper"
cp src-tauri/icons/icon.icns "$APP/Contents/Resources/AppIcon.icns"
sed "s/__VERSION__/$VERSION/g" macos/Info.plist > "$APP/Contents/Info.plist"
printf 'APPL????' > "$APP/Contents/PkgInfo"

# Ad-hoc signature: required to run on Apple Silicon. Not notarized.
codesign --force --sign - "$APP/Contents/MacOS/moondisk-helper"
codesign --force --sign - "$APP"
codesign --verify --deep --strict "$APP"
lipo -info "$APP/Contents/MacOS/MoonDisk" "$APP/Contents/MacOS/moondisk-helper"

# --- DMG -------------------------------------------------------------------
STAGE="$OUT/dmg"
mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
DMG="$OUT/MoonDisk_${VERSION}_universal.dmg"
[[ "$QUICK" == "--quick" ]] && DMG="$OUT/MoonDisk_${VERSION}_$(uname -m).dmg"
hdiutil create -volname MoonDisk -srcfolder "$STAGE" -ov -format UDZO "$DMG"
echo "Built $DMG"
