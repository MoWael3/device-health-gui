# Device Health Dashboard

A lightweight desktop dashboard for monitoring device health on **Linux, macOS
and Windows**, written in Rust with [eframe/egui](https://github.com/emilk/egui)
for the GUI and [sysinfo](https://github.com/GuillaumeGomez/sysinfo) for
cross-platform metrics.

The original PyQt6 version (Linux only) still lives in `src/device_health_gui/`;
see [Legacy Python app](#legacy-python-app).

## Features
- Overall health badge (Healthy / Warning / Critical) in the header
- Ring gauges with 60-second history for CPU, memory, disk and temperature
- Threshold-based status colors (green / amber / red) per metric
- Per-core CPU bars
- Top 8 processes by CPU. Click a row to see its details (command line,
  executable, memory, status, run time, parent PID) and end it if needed (you
  confirm first)
- Live network throughput chart (RX/TX, loopback excluded)
- System panel: uptime, load average, core count, memory and disk totals
- AI health summary card powered by `opencode`
- Updates every second. Below about 960×820 the dashboard scrolls instead of squashing

### Per-OS behaviour
| Metric | Linux | macOS | Windows |
|---|---|---|---|
| Disk | `/` | `/` | system drive (`%SystemDrive%`, usually `C:\`) |
| Temperature | hottest hwmon sensor | hottest SMC sensor | usually **N/A** (Windows rarely exposes sensors without admin rights or vendor tools) |
| Load average | yes | yes | **N/A** (Windows has no load average) |
| Loopback excluded | `lo` | `lo0` | "Loopback Pseudo-Interface" |

When a metric isn't available, its card shows "N/A" and it doesn't count
toward the overall health badge.

## Build and run

You need a Rust toolchain, version 1.95 or newer. Install it from
<https://rustup.rs>.

```bash
cargo build --release
```

The binary is `target/release/device-health-dashboard`
(`device-health-dashboard.exe` on Windows). It doesn't depend on Python or Qt.

### Linux
Build needs a C compiler and `pkg-config` (`sudo apt install build-essential pkg-config`).
At runtime it needs the usual desktop libraries (X11 or Wayland, `libxkbcommon`,
and a Vulkan or OpenGL driver), which any desktop install already has.

```bash
./run.sh            # builds on first run, then launches
# or
cargo run --release
```

Install the app-menu entry ("Device Health Dashboard"). It calls `run.sh`:
```bash
cp device-health-dashboard.desktop ~/.local/share/applications/
```
(Edit the paths inside it if the repo isn't at
`~/Documents/Default Project/device-health-gui`.)

### macOS
Install the Xcode command line tools (`xcode-select --install`), then:
```bash
cargo run --release
```
Works on both Apple Silicon and Intel Macs.

### Windows
Install Rust with `rustup-init.exe` and pick the MSVC toolchain (it prompts you
to install the Visual Studio C++ Build Tools). Then, in PowerShell or cmd:
```powershell
cargo run --release
```
Release builds open without a console window. To get a Start-menu entry,
create a shortcut to `target\release\device-health-dashboard.exe`.

### Cross-checking other targets
From any OS you can check that the other platforms still compile:
```bash
rustup target add x86_64-pc-windows-msvc x86_64-apple-darwin aarch64-apple-darwin
cargo check --release --target x86_64-pc-windows-msvc
cargo check --release --target aarch64-apple-darwin
```

### Tests
```bash
cargo test --release
```

## AI health summary
The "Analyze with opencode" button sends the last 60 s of averaged readings
(and peaks) to `opencode run`, using whatever provider and model opencode is
configured with. The binary is found on `PATH`, then in `~/.opencode/bin/`.
To override:

- `OPENCODE_BIN`: path to the opencode binary
- `OPENCODE_MODEL`: `provider/model` passed to `--model`

On Windows an npm-installed `opencode.cmd` works too. Its prompt is sent as a
single line, because batch-file arguments can't contain newlines.

## Project layout
```
Cargo.toml
src/main.rs        window setup and icon
src/dashboard.rs   layout, snapshot → UI, AI card, process details popup
src/metrics.rs     sysinfo sampling on a background thread
src/widgets.rs     ring gauge, line chart, badge, bars, metric card
src/opencode.rs    prompt and `opencode run` subprocess
src/theme.rs       colors, spacing, formatting helpers
assets/            app icon (SVG for the launcher, PNG for the window)
run.sh, device-health-dashboard.desktop   Linux launcher
```

## Legacy Python app
The PyQt6 version is still here for comparison. It's Linux only and reads
`/proc` and `/sys` directly:
```bash
uv run python main_app.py
```
It needs Python 3.13 and PyQt6, plus `libxcb-cursor0` on X11.
