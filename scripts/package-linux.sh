#!/bin/sh
# Build the Linux release packages into dist/:
#   device-health-dashboard-<version>-linux-x86_64.tar.gz  (binary + launcher + icon)
#   device-health-dashboard_<version>-1_amd64.deb          (needs `cargo install cargo-deb`)
set -eu
cd "$(dirname "$0")/.."

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
ARCH=$(uname -m)
NAME="device-health-dashboard-$VERSION-linux-$ARCH"
mkdir -p dist

cargo build --release

STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT
mkdir -p "$STAGE/$NAME"
cp target/release/device-health-dashboard "$STAGE/$NAME/"
cp packaging/linux/device-health-dashboard.desktop assets/icon.svg assets/icon.png "$STAGE/$NAME/"
cat > "$STAGE/$NAME/README.txt" <<TXT
Device Health Dashboard $VERSION for Linux ($ARCH)

Run it:      ./device-health-dashboard
App menu:    install the .deb instead, or copy device-health-dashboard to
             ~/.local/bin, icon.svg to
             ~/.local/share/icons/hicolor/scalable/apps/device-health-dashboard.svg
             and device-health-dashboard.desktop to ~/.local/share/applications/
TXT
tar -C "$STAGE" -czf "dist/$NAME.tar.gz" "$NAME"
echo "Built dist/$NAME.tar.gz"

if command -v cargo-deb >/dev/null 2>&1; then
    cargo deb --no-build --output dist/
else
    echo "cargo-deb not installed; skipping .deb (cargo install cargo-deb)" >&2
fi
