#!/bin/sh
# Build "Device Health Dashboard.app" (universal: Apple Silicon + Intel) and
# package it as a .dmg in dist/. Must run on macOS: it needs Xcode's command
# line tools (xcode-select --install) for linking, plus sips, iconutil,
# lipo and hdiutil, which ship with macOS.
#
# Optional, for an app every Mac trusts (needs an Apple Developer account):
#   MACOS_SIGN_IDENTITY   e.g. "Developer ID Application: Mo Wael (TEAMID)",
#                         a certificate already in the keychain
#   APPLE_ID, APPLE_TEAM_ID, APPLE_APP_PASSWORD
#                         notarize with Apple and staple the ticket
# Without them the app is ad-hoc signed and macOS asks before first launch.
set -eu
cd "$(dirname "$0")/.."

if [ "$(uname -s)" != Darwin ]; then
    echo "make-dmg.sh must run on macOS (the macOS SDK and hdiutil are Mac-only)." >&2
    exit 1
fi

APP_NAME="Device Health Dashboard"
BIN=device-health-dashboard
BUNDLE_ID=com.mwael.device-health-dashboard
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
TARGETS="aarch64-apple-darwin x86_64-apple-darwin"

OUT=dist
APP="$OUT/$APP_NAME.app"
DMG="$OUT/Device-Health-Dashboard-$VERSION-macos-universal.dmg"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

# 1. Build both architectures and merge them into one binary.
rustup target add $TARGETS
for t in $TARGETS; do
    cargo build --release --target "$t"
done
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
lipo -create -output "$APP/Contents/MacOS/$BIN" \
    target/aarch64-apple-darwin/release/$BIN \
    target/x86_64-apple-darwin/release/$BIN

# 2. App icon (.icns) from the 1024px PNG.
ICONSET="$WORK/AppIcon.iconset"
mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
    sips -z $s $s assets/icon-1024.png --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
    d=$((s * 2))
    sips -z $d $d assets/icon-1024.png --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"

# 3. Info.plist
cat > "$APP/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>$APP_NAME</string>
    <key>CFBundleDisplayName</key><string>$APP_NAME</string>
    <key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
    <key>CFBundleExecutable</key><string>$BIN</string>
    <key>CFBundleIconFile</key><string>AppIcon</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$VERSION</string>
    <key>CFBundleVersion</key><string>$VERSION</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>LSApplicationCategoryType</key><string>public.app-category.utilities</string>
    <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
EOF

# 4. Sign: Developer ID (hardened runtime, as notarization requires) when
#    available, otherwise ad-hoc so Apple Silicon will still run it.
SIGN_ID="${MACOS_SIGN_IDENTITY:-}"
if [ -n "$SIGN_ID" ]; then
    codesign --force --options runtime --timestamp --sign "$SIGN_ID" "$APP"
else
    codesign --force --deep --sign - "$APP"
fi

# 5. Disk image with the app and a shortcut to /Applications.
STAGE="$WORK/dmg"
mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
rm -f "$DMG"
hdiutil create -volname "$APP_NAME" -srcfolder "$STAGE" -ov -format UDZO "$DMG"

if [ -n "$SIGN_ID" ]; then
    codesign --force --timestamp --sign "$SIGN_ID" "$DMG"
    if [ -n "${APPLE_ID:-}" ] && [ -n "${APPLE_TEAM_ID:-}" ] && [ -n "${APPLE_APP_PASSWORD:-}" ]; then
        xcrun notarytool submit "$DMG" --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" \
            --password "$APPLE_APP_PASSWORD" --wait
        xcrun stapler staple "$DMG"
        echo "Notarized and stapled $DMG"
    fi
fi

echo "Built $DMG"
