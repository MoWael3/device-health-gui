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

## Download

Ready-made builds are on the
[Releases page](https://github.com/MoWael3/device-health-gui/releases/latest):

| OS | File | How to install |
|---|---|---|
| macOS (Apple Silicon + Intel) | `Device-Health-Dashboard-<version>-macos-universal.dmg` | Open it, drag the app into **Applications** |
| Windows 10/11 (x64) | `device-health-dashboard-<version>-windows-x86_64.zip` | Unzip, double-click `device-health-dashboard.exe` |
| Debian, Ubuntu, Mint (x64) | `device-health-dashboard_<version>-1_amd64.deb` | `sudo apt install ./device-health-dashboard_*.deb`, then find it in the app menu |
| Other Linux (x64) | `device-health-dashboard-<version>-linux-x86_64.tar.gz` | Unpack, run `./device-health-dashboard` |

`SHA256SUMS.txt` lists each file's checksum.

Until the builds are signed (see [Code signing](#code-signing)), macOS and
Windows warn the first time you open the app:
- **macOS:** System Settings → Privacy & Security → **Open Anyway** (macOS 15+),
  or right-click the app → **Open** on older versions.
- **Windows:** on the "Windows protected your PC" screen, click **More info →
  Run anyway**.

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

#### Making a .dmg
On a Mac, one command builds a universal app (Apple Silicon + Intel) and wraps
it in a disk image:
```bash
./scripts/make-dmg.sh
```
The result is `dist/Device-Health-Dashboard-<version>-macos-universal.dmg`.
Open it and drag **Device Health Dashboard** into **Applications**.

No Mac? The [Release workflow](#releases) builds it on a GitHub-hosted Mac.

A `.dmg` can't be built on Linux or Windows: linking a macOS app needs Apple's
SDK, and `hdiutil` only exists on macOS.

Without a Developer ID (see [Code signing](#code-signing)) the app is ad-hoc
signed, so macOS asks before the first launch. You can also clear the download
flag:
```bash
xattr -dr com.apple.quarantine "/Applications/Device Health Dashboard.app"
```

### Linux packages
```bash
cargo install cargo-deb      # once, for the .deb
./scripts/package-linux.sh   # dist/*.tar.gz and dist/*.deb
```

### Windows
Install Rust with `rustup-init.exe` and pick the MSVC toolchain (it prompts you
to install the Visual Studio C++ Build Tools). Then, in PowerShell or cmd:
```powershell
cargo run --release
```
Release builds open without a console window. To get a Start-menu entry,
create a shortcut to `target\release\device-health-dashboard.exe`.

`./scripts/package-windows.ps1` builds the release `.zip` into `dist\`.

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

## Releases
`.github/workflows/release.yml` builds all three platforms on GitHub's
machines:

- **Actions tab → Release → Run workflow** builds everything and attaches the
  files to that run (GitHub keeps them 90 days).
- **Pushing a version tag** builds everything and publishes a GitHub Release
  with the files and checksums. Bump `version` in `Cargo.toml` first, then:
  ```bash
  git tag v0.1.2 && git push origin v0.1.2
  ```

## Code signing
Signing is what stops the "unknown developer" warnings. Operating systems only
trust certificates from their own programs, so it needs paid accounts. A
self-made certificate doesn't help. The workflow signs automatically as soon as
you add these as repository secrets (**Settings → Secrets and variables →
Actions**):

| Platform | What you need | Secrets |
|---|---|---|
| macOS | [Apple Developer Program](https://developer.apple.com/programs/) (US$99/year): create a **Developer ID Application** certificate, export it as `.p12`, and make an [app-specific password](https://support.apple.com/102654) for notarization | `MACOS_CERT_P12` (base64 of the `.p12`), `MACOS_CERT_PASSWORD`, `MACOS_SIGN_IDENTITY` (e.g. `Developer ID Application: Mo Wael (TEAMID)`), `APPLE_ID`, `APPLE_TEAM_ID`, `APPLE_APP_PASSWORD` |
| Windows | A code-signing certificate as a `.pfx` from a certificate authority (e.g. DigiCert, Sectigo, SSL.com). New certificates build SmartScreen reputation over time | `WINDOWS_CERT_PFX` (base64 of the `.pfx`), `WINDOWS_CERT_PASSWORD` |
| Linux | Nothing; Linux has no central app signing. Check downloads against `SHA256SUMS.txt` | none |

To base64-encode a certificate file: `base64 -i cert.p12 | pbcopy` on macOS, or
`base64 -w0 cert.pfx` on Linux.

With the macOS secrets set, the `.dmg` is signed with your Developer ID,
notarized by Apple and stapled, so it opens on any Mac without warnings.

## AI health summary
The "Analyze with opencode" button sends the last 60 s of averaged readings
(and peaks) to `opencode run`, using whatever provider and model opencode is
configured with. The binary is found on `PATH`, then in `~/.opencode/bin/`.
To override:

- `OPENCODE_BIN`: path to the opencode binary
- `OPENCODE_MODEL`: `provider/model` passed to `--model`

When the app is started from a menu, Finder or the Dock (which don't pass your
shell's PATH), it also looks in `~/.local/bin`, Homebrew's `/opt/homebrew/bin`
and `/usr/local/bin` on macOS, and `%APPDATA%\npm` on Windows.

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
assets/            app icon (SVG for the launcher, PNGs for the window and macOS app)
scripts/make-dmg.sh         macOS .app + .dmg packaging (run on a Mac)
scripts/package-windows.ps1 Windows .zip packaging (optional signing)
scripts/package-linux.sh    Linux .tar.gz + .deb packaging
packaging/linux/            .desktop entry used by the packages
.github/workflows/release.yml   builds all platforms and publishes releases
run.sh, device-health-dashboard.desktop   Linux launcher
```

## Legacy Python app
The PyQt6 version is still here for comparison. It's Linux only and reads
`/proc` and `/sys` directly:
```bash
uv run python main_app.py
```
It needs Python 3.13 and PyQt6, plus `libxcb-cursor0` on X11.
