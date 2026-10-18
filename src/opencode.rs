//! Use opencode as the AI provider for device health analysis.
//!
//! Shells out to `opencode run`, so whatever provider/model opencode is
//! configured with is what answers. Override with:
//!   OPENCODE_BIN    path to the opencode binary (default: "opencode")
//!   OPENCODE_MODEL  "provider/model" passed to --model (default: opencode's own)

use std::env;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(120);

/// Averaged readings handed to the model (built by `dashboard::ai_summary`).
pub struct Summary {
    pub os: String,
    pub window: usize,
    pub cores: usize,
    pub cpu_avg: String,
    pub cpu_peak: String,
    pub load: String,
    pub mem_used: u64,
    pub mem_total: u64,
    pub mem_avg: String,
    pub mem_peak: String,
    pub disk_used: u64,
    pub disk_total: u64,
    pub disk_pct: u64,
    pub temp_avg: String,
    pub temp_peak: String,
    pub rx_avg: String,
    pub rx_peak: String,
    pub tx_avg: String,
    pub tx_peak: String,
    pub top_procs: Vec<(String, f32)>,
    pub uptime: u64,
}

pub fn build_prompt(s: &Summary) -> String {
    let procs = if s.top_procs.is_empty() {
        "none".to_string()
    } else {
        s.top_procs
            .iter()
            .map(|(n, c)| format!("{n} {c:.1}%"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "You are a device health assistant. Below are readings from a {os} \
         machine averaged over the last {window} seconds (peaks in brackets). \
         Reply with at most 5 short lines, one point per line, no markdown: \
         whether the machine looks healthy, anything concerning, and one action \
         if needed. Do not use tools.\n\
         CPU: avg {cpu_avg}% [peak {cpu_peak}%], {cores} cores, load average {load}\n\
         Memory: {mem_used}/{mem_total} MB, avg {mem_avg}% [peak {mem_peak}%]\n\
         Disk: {disk_used}/{disk_total} GB ({disk_pct}%)\n\
         Temperature: avg {temp_avg} C [peak {temp_peak} C]\n\
         Network: down avg {rx_avg} KB/s [peak {rx_peak}], up avg {tx_avg} KB/s [peak {tx_peak}]\n\
         Top processes by CPU now: {procs}\n\
         Uptime: {uptime} s",
        os = s.os,
        window = s.window,
        cpu_avg = s.cpu_avg,
        cpu_peak = s.cpu_peak,
        cores = s.cores,
        load = s.load,
        mem_used = s.mem_used,
        mem_total = s.mem_total,
        mem_avg = s.mem_avg,
        mem_peak = s.mem_peak,
        disk_used = s.disk_used,
        disk_total = s.disk_total,
        disk_pct = s.disk_pct,
        temp_avg = s.temp_avg,
        temp_peak = s.temp_peak,
        rx_avg = s.rx_avg,
        rx_peak = s.rx_peak,
        tx_avg = s.tx_avg,
        tx_peak = s.tx_peak,
        uptime = s.uptime,
    )
}

/// Resolve `name` against PATH (and PATHEXT on Windows), like `shutil.which`.
fn which(name: &str) -> Option<PathBuf> {
    let candidate = Path::new(name);
    let exts: Vec<String> = if cfg!(windows) {
        let pathext = env::var("PATHEXT").unwrap_or_else(|_| ".EXE;.CMD;.BAT".into());
        std::iter::once(String::new())
            .chain(pathext.split(';').map(|e| e.to_ascii_lowercase()))
            .collect()
    } else {
        vec![String::new()]
    };
    let try_file = |base: &Path| {
        exts.iter()
            .map(|e| PathBuf::from(format!("{}{e}", base.display())))
            .find(|p| p.is_file())
    };
    if candidate.components().count() > 1 || candidate.is_absolute() {
        return try_file(candidate);
    }
    env::split_paths(&env::var_os("PATH")?).find_map(|dir| try_file(&dir.join(name)))
}

fn home_dir() -> Option<PathBuf> {
    env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

fn find_binary() -> Result<PathBuf, String> {
    if let Ok(bin) = env::var("OPENCODE_BIN") {
        return which(&bin)
            .ok_or_else(|| format!("opencode not found ('{bin}'). Install it or set OPENCODE_BIN."));
    }
    if let Some(p) = which("opencode") {
        return Ok(p);
    }
    // Apps launched from a desktop menu, Finder or the Dock don't get the
    // shell's PATH, so also look where the installers usually put opencode.
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(h) = home_dir() {
        dirs.push(h.join(".opencode").join("bin"));
        dirs.push(h.join(".local").join("bin"));
    }
    if cfg!(target_os = "macos") {
        dirs.push("/opt/homebrew/bin".into()); // Homebrew, Apple Silicon
        dirs.push("/usr/local/bin".into()); // Homebrew, Intel
    }
    if cfg!(windows) {
        if let Some(appdata) = env::var_os("APPDATA") {
            dirs.push(PathBuf::from(appdata).join("npm"));
        }
    }
    dirs.iter()
        .find_map(|d| which(&d.join("opencode").to_string_lossy()))
        .ok_or_else(|| "opencode not found ('opencode'). Install it or set OPENCODE_BIN.".into())
}

fn read_all(r: Option<impl Read + Send + 'static>) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut s = String::new();
        if let Some(mut r) = r {
            let _ = r.read_to_string(&mut s);
        }
        s
    })
}

/// Return opencode's analysis of `summary`, or an error message string.
pub fn analyze(summary: &Summary) -> String {
    let binary = match find_binary() {
        Ok(b) => b,
        Err(e) => return e,
    };
    let mut prompt = build_prompt(summary);
    // cmd.exe can't receive newlines in batch-file arguments (npm installs
    // opencode as opencode.cmd on Windows).
    let ext = binary.extension().map(|e| e.to_string_lossy().to_ascii_lowercase());
    if matches!(ext.as_deref(), Some("cmd" | "bat")) {
        prompt = prompt.replace('\n', " | ");
    }
    let mut cmd = Command::new(&binary);
    cmd.arg("run");
    if let Ok(model) = env::var("OPENCODE_MODEL") {
        if !model.is_empty() {
            cmd.args(["--model", &model]);
        }
    }
    cmd.arg(prompt)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return format!("Could not start opencode ({}): {e}", binary.display()),
    };
    let out = read_all(child.stdout.take());
    let err = read_all(child.stderr.take());
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() >= TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return format!("opencode timed out after {}s.", TIMEOUT.as_secs());
            }
            Ok(None) => thread::sleep(Duration::from_millis(100)),
            Err(e) => return format!("opencode failed: {e}"),
        }
    };
    let out = out.join().unwrap_or_default().trim().to_string();
    let err = err.join().unwrap_or_default().trim().to_string();
    if !status.success() {
        let detail: String = if err.is_empty() { &out } else { &err }.chars().take(500).collect();
        let code = status.code().map_or("signal".to_string(), |c| c.to_string());
        return format!("opencode failed ({code}): {detail}");
    }
    if out.is_empty() {
        "opencode returned no output.".into()
    } else {
        out
    }
}
