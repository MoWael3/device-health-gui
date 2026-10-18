#!/bin/sh
# Launch the Device Health Dashboard.
set -e
cd "$(dirname "$0")"

# Locate uv (may live in the hermes toolchain rather than PATH).
if ! command -v uv >/dev/null 2>&1; then
    UV=$(find "$HOME/.hermes/tools" -maxdepth 2 -name uv -type f 2>/dev/null | head -n1)
    [ -n "$UV" ] && PATH="$(dirname "$UV"):$PATH"
fi

# Qt >= 6.5 can't open an X11 window without libxcb-cursor. Say so clearly,
# including a desktop notification when started from the app menu.
if [ "${XDG_SESSION_TYPE:-x11}" = x11 ] && ! ldconfig -p 2>/dev/null | grep -q libxcb-cursor; then
    MSG="Missing system library libxcb-cursor0. Install it with: sudo apt install libxcb-cursor0"
    echo "$MSG" >&2
    command -v notify-send >/dev/null 2>&1 && notify-send -i dialog-error "Device Health Dashboard" "$MSG"
    exit 1
fi

if command -v uv >/dev/null 2>&1; then
    exec uv run python main_app.py "$@"
else
    exec python3 main_app.py "$@"
fi
