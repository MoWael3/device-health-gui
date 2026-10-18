# Device Health Dashboard

A lightweight GUI desktop dashboard for monitoring device health on Linux, built with PyQt6.

## Features
- Overall health badge (Healthy / Warning / Critical) in the header
- Ring gauges with 60-second history for CPU, memory, disk (/) and temperature
- Threshold-based status colors (green / amber / red) per metric
- Live network throughput chart (RX/TX, loopback excluded)
- System panel: uptime, load average, core count, memory and disk totals

## Requirements
- Python 3.13
- PyQt6

## Installation & Run
Using uv (recommended):
```bash
cd /home/mwael/Documents/Default Project/device-health-gui
./run.sh
```

Or manually:
```bash
export PATH="$HOME/.hermes/tools/uv-0.12.3-linux-x64:$PATH"
cd /home/mwael/Documents/Default Project/device-health-gui
uv run python main_app.py
```

## System requirement
Qt 6.5+ needs `libxcb-cursor0` to open windows on X11:
```bash
sudo apt install libxcb-cursor0
```

## Desktop launcher
Install the app-menu entry ("Device Health Dashboard"):
```bash
cp device-health-dashboard.desktop ~/.local/share/applications/
```

## AI health summary
The "Analyze with opencode" button sends the last 60 s of averaged readings
(and peaks) to `opencode run`. Set `OPENCODE_BIN` / `OPENCODE_MODEL` to override.

## Notes
- The window resizes freely; below about 960×720 the dashboard scrolls
- Reads metrics from /proc and /sys (no external system commands required)
- Dark theme with custom-painted widgets (QPainter)
- Updates every second
