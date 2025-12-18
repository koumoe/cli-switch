#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="CliSwitch"
BIN_NAME="cliswitch"

cd "$ROOT_DIR"

VERSION="$(awk -F '\"' '/^version =/ { print $2; exit }' Cargo.toml || true)"
if [[ -z "${VERSION:-}" ]]; then
  VERSION="0.0.0"
fi

if [[ "${SKIP_UI:-0}" != "1" ]]; then
  pushd ui >/dev/null
  if [[ ! -d node_modules ]]; then
    npm ci
  fi
  npm run build
  popd >/dev/null
fi

cargo build --release

OUT_DIR="$ROOT_DIR/dist/macos"
APP_DIR="$OUT_DIR/${APP_NAME}.app"

rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources"

cp "$ROOT_DIR/target/release/$BIN_NAME" "$APP_DIR/Contents/MacOS/$BIN_NAME"
chmod +x "$APP_DIR/Contents/MacOS/$BIN_NAME"

cat >"$APP_DIR/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleExecutable</key>
  <string>${BIN_NAME}</string>
  <key>CFBundleIdentifier</key>
  <string>com.koumoe.cliswitch</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>${APP_NAME}</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>${VERSION}</string>
  <key>CFBundleVersion</key>
  <string>${VERSION}</string>
  <key>NSHighResolutionCapable</key>
  <true/>
  <key>NSAppTransportSecurity</key>
  <dict>
    <key>NSAllowsArbitraryLoads</key>
    <true/>
  </dict>
</dict>
</plist>
PLIST

echo "OK: ${APP_DIR}"
