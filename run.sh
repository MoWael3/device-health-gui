#!/bin/sh
# Launch the Device Health Dashboard (Rust build). Builds it first if needed.
set -e
cd "$(dirname "$0")"

BIN=target/release/device-health-dashboard

# Desktop launchers don't get ~/.bashrc's PATH, so look for cargo in ~/.cargo too.
if ! command -v cargo >/dev/null 2>&1 && [ -x "$HOME/.cargo/bin/cargo" ]; then
    PATH="$HOME/.cargo/bin:$PATH"
fi

if [ ! -x "$BIN" ]; then
    if ! command -v cargo >/dev/null 2>&1; then
        MSG="Rust is not installed. Install it from https://rustup.rs, then run ./run.sh again."
        echo "$MSG" >&2
        command -v notify-send >/dev/null 2>&1 && notify-send -i dialog-error "Device Health Dashboard" "$MSG"
        exit 1
    fi
    cargo build --release
fi

exec "$BIN" "$@"
