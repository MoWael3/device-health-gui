//! Cross-platform system metrics (sysinfo) and the background sampler thread.
//!
//! One `SystemStats` is owned by the sampler thread and never touched from the
//! UI thread; snapshots are handed over through a channel.

use std::path::Path;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant};

use sysinfo::{
    Components, CpuRefreshKind, Disks, MemoryRefreshKind, Networks, ProcessRefreshKind,
    ProcessesToUpdate, RefreshKind, System, UpdateKind,
};

/// Ignore wall-clock deltas shorter than this for rate metrics.
const MIN_DT: f64 = 0.5;
/// Processes reported per snapshot.
pub const TOP_N: usize = 8;

#[derive(Clone, Debug)]
pub struct ProcInfo {
    pub pid: u32,
    pub name: String,
    /// % of one core since the previous sample.
    pub cpu: f32,
    /// % of total RAM.
    pub mem: f32,
}

/// One consistent reading of every metric the dashboard displays.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub cpu: f32,
    pub per_core: Vec<f32>,
    pub mem_used: u64,
    pub mem_total: u64,
    pub mem_pct: f32,
    pub disk_used: u64,
    pub disk_total: u64,
    pub disk_pct: f32,
    /// Hottest sensor in °C, `None` when the OS exposes no readable sensor.
    pub temp: Option<f32>,
    pub uptime: u64,
    /// 1/5/15-minute load average, `None` where the OS has no such concept (Windows).
    pub load: Option<(f64, f64, f64)>,
    pub net_rx: f32,
    pub net_tx: f32,
    pub procs: Vec<ProcInfo>,
}

pub struct SystemStats {
    sys: System,
    disks: Disks,
    components: Components,
    networks: Networks,
    prev_net: (u64, u64),
    prev_net_t: Instant,
    ticks: u64,
    disk_cache: (u64, u64, f32),
    temp_cache: Option<f32>,
}

/// Disks and sensors change slowly and are comparatively costly to rescan.
const SLOW_EVERY: u64 = 5;

fn proc_kind() -> ProcessRefreshKind {
    // Threads are listed as processes by default on Linux, which is slow and
    // not what the table shows.
    ProcessRefreshKind::nothing()
        .without_tasks()
        .with_cpu()
        .with_memory()
        .with_cmd(UpdateKind::OnlyIfNotSet)
}

/// Loopback interfaces: `lo` (Linux), `lo0` (macOS/BSD), "Loopback Pseudo-Interface 1" (Windows).
fn is_loopback(name: &str) -> bool {
    name == "lo" || name.starts_with("lo0") || name.to_ascii_lowercase().contains("loopback")
}

impl SystemStats {
    pub fn new() -> Self {
        let mut sys = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::nothing().with_cpu_usage())
                .with_memory(MemoryRefreshKind::nothing().with_ram()),
        );
        sys.refresh_processes_specifics(ProcessesToUpdate::All, true, proc_kind());
        let networks = Networks::new_with_refreshed_list();
        let prev_net = Self::net_counters(&networks);
        Self {
            sys,
            disks: Disks::new_with_refreshed_list(),
            components: Components::new_with_refreshed_list(),
            networks,
            prev_net,
            prev_net_t: Instant::now(),
            ticks: 0,
            disk_cache: (0, 0, 0.0),
            temp_cache: None,
        }
    }

    fn net_counters(networks: &Networks) -> (u64, u64) {
        networks
            .list()
            .iter()
            .filter(|(name, _)| !is_loopback(name))
            .fold((0, 0), |(rx, tx), (_, d)| {
                (rx + d.total_received(), tx + d.total_transmitted())
            })
    }

    /// Returns (rx_kbps, tx_kbps), excluding loopback interfaces.
    fn network(&mut self) -> (f32, f32) {
        self.networks.refresh(true);
        let now = Instant::now();
        let cur = Self::net_counters(&self.networks);
        let (prev, prev_t) = (self.prev_net, self.prev_net_t);
        self.prev_net = cur;
        self.prev_net_t = now;
        let dt = (now - prev_t).as_secs_f64();
        if dt < MIN_DT {
            return (0.0, 0.0);
        }
        let rate = |c: u64, p: u64| (c.saturating_sub(p) as f64 / dt / 1024.0) as f32;
        (rate(cur.0, prev.0), rate(cur.1, prev.1))
    }

    /// The disk holding the OS: `/` on Unix, the system drive on Windows.
    fn disk(&mut self) -> (u64, u64, f32) {
        self.disks.refresh(true);
        let root = if cfg!(windows) {
            let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
            format!("{drive}\\")
        } else {
            "/".to_string()
        };
        let list = self.disks.list();
        let disk = list
            .iter()
            .find(|d| d.mount_point() == Path::new(&root))
            .or_else(|| list.iter().max_by_key(|d| d.total_space()));
        match disk {
            Some(d) if d.total_space() > 0 => {
                let total = d.total_space();
                let used = total.saturating_sub(d.available_space());
                (used, total, 100.0 * used as f32 / total as f32)
            }
            _ => (0, 0, 0.0),
        }
    }

    /// Hottest sensor in °C, or None when no sensor is readable.
    fn temperature(&mut self) -> Option<f32> {
        self.components.refresh(true);
        self.components
            .list()
            .iter()
            .filter_map(|c| c.temperature())
            .filter(|t| t.is_finite() && *t > 0.0)
            .reduce(f32::max)
    }

    fn load_avg() -> Option<(f64, f64, f64)> {
        if cfg!(windows) {
            return None;
        }
        let l = System::load_average();
        Some((l.one, l.five, l.fifteen))
    }

    /// Program name: argv[0]'s basename where available (Linux truncates the
    /// process name to 15 chars), falling back to the OS-reported name.
    fn proc_name(p: &sysinfo::Process) -> String {
        if let Some(arg0) = p.cmd().first() {
            let exe = arg0.to_string_lossy();
            // Some programs (Chrome, Electron) rewrite argv into one
            // space-joined string; keep only the program part.
            let exe = exe.split(" --").next().unwrap_or("").trim();
            if !exe.is_empty() {
                let base = Path::new(exe)
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| exe.to_string());
                if !base.is_empty() {
                    return base;
                }
            }
        }
        let name = p.name().to_string_lossy();
        if name.is_empty() {
            p.pid().to_string()
        } else {
            name.into_owned()
        }
    }

    fn processes(&mut self, mem_total: u64) -> Vec<ProcInfo> {
        self.sys
            .refresh_processes_specifics(ProcessesToUpdate::All, true, proc_kind());
        let mut rows: Vec<_> = self
            .sys
            .processes()
            .values()
            .filter(|p| p.thread_kind().is_none())
            .collect();
        rows.sort_by(|a, b| {
            b.cpu_usage()
                .total_cmp(&a.cpu_usage())
                .then(b.memory().cmp(&a.memory()))
        });
        rows.into_iter()
            .take(TOP_N)
            .map(|p| ProcInfo {
                pid: p.pid().as_u32(),
                name: Self::proc_name(p),
                cpu: p.cpu_usage(),
                mem: if mem_total > 0 {
                    100.0 * p.memory() as f32 / mem_total as f32
                } else {
                    0.0
                },
            })
            .collect()
    }

    pub fn sample(&mut self) -> Snapshot {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        let per_core: Vec<f32> = self.sys.cpus().iter().map(|c| c.cpu_usage()).collect();
        let mem_total = self.sys.total_memory();
        let mem_used = self.sys.used_memory().min(mem_total);
        let mem_pct = if mem_total > 0 {
            100.0 * mem_used as f32 / mem_total as f32
        } else {
            0.0
        };
        if self.ticks % SLOW_EVERY == 0 {
            self.disk_cache = self.disk();
            self.temp_cache = self.temperature();
        }
        self.ticks += 1;
        let (disk_used, disk_total, disk_pct) = self.disk_cache;
        let (net_rx, net_tx) = self.network();
        Snapshot {
            cpu: self.sys.global_cpu_usage().clamp(0.0, 100.0),
            per_core,
            mem_used,
            mem_total,
            mem_pct,
            disk_used,
            disk_total,
            disk_pct,
            temp: self.temp_cache,
            uptime: System::uptime(),
            load: Self::load_avg(),
            net_rx,
            net_tx,
            procs: self.processes(mem_total),
        }
    }
}

/// Samples every `interval` on a background thread until the receiver is dropped.
pub fn spawn_sampler(interval: Duration, tx: Sender<Snapshot>, ctx: egui::Context) {
    thread::Builder::new()
        .name("sampler".into())
        .spawn(move || {
            let mut stats = SystemStats::new();
            // sysinfo needs a gap between CPU refreshes to compute usage.
            thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
            loop {
                let started = Instant::now();
                if tx.send(stats.sample()).is_err() {
                    break;
                }
                ctx.request_repaint();
                if let Some(rest) = interval.checked_sub(started.elapsed()) {
                    thread::sleep(rest);
                }
            }
        })
        .expect("failed to start sampler thread");
}

/// Everything the process details popup shows, read once when a row is clicked.
#[derive(Clone, Debug)]
pub struct ProcDetails {
    pub pid: u32,
    pub name: String,
    pub exe: String,
    pub cmd: String,
    pub memory: u64,
    pub status: String,
    pub run_time: u64,
    pub parent: Option<u32>,
}

fn one_process(pid: u32) -> (System, sysinfo::Pid) {
    let pid = sysinfo::Pid::from_u32(pid);
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing()
            .without_tasks()
            .with_memory()
            .with_cmd(UpdateKind::Always)
            .with_exe(UpdateKind::Always),
    );
    (sys, pid)
}

/// Details for `pid`, or None if it has exited.
pub fn proc_details(pid: u32) -> Option<ProcDetails> {
    let (sys, spid) = one_process(pid);
    let p = sys.process(spid)?;
    let cmd = p
        .cmd()
        .iter()
        .map(|a| a.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");
    Some(ProcDetails {
        pid,
        name: SystemStats::proc_name(p),
        exe: p.exe().map_or_else(|| "unavailable".into(), |e| e.display().to_string()),
        cmd: if cmd.is_empty() { "unavailable".into() } else { cmd },
        memory: p.memory(),
        status: p.status().to_string(),
        run_time: p.run_time(),
        parent: p.parent().map(|pp| pp.as_u32()),
    })
}

/// Ask `pid` to exit (SIGTERM on Unix, TerminateProcess on Windows).
pub fn end_process(pid: u32) -> Result<(), String> {
    let (sys, spid) = one_process(pid);
    let p = sys.process(spid).ok_or("The process has already exited.")?;
    let ok = p.kill_with(sysinfo::Signal::Term).unwrap_or_else(|| p.kill());
    if ok {
        Ok(())
    } else {
        Err("Could not end the process (it may belong to another user or need admin rights).".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_has_sane_values() {
        let mut stats = SystemStats::new();
        thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        let s = stats.sample();
        assert!(!s.per_core.is_empty());
        assert!((0.0..=100.0).contains(&s.cpu));
        assert!(s.mem_total > 0 && s.mem_used <= s.mem_total);
        assert!(s.disk_total > 0);
        assert!(s.procs.len() <= TOP_N);
    }

    #[test]
    fn details_and_end_process() {
        let mut cmd = if cfg!(windows) {
            let mut c = std::process::Command::new("ping");
            c.args(["-n", "60", "127.0.0.1"]);
            c
        } else {
            let mut c = std::process::Command::new("sleep");
            c.arg("60");
            c
        };
        let mut child = cmd.stdout(std::process::Stdio::null()).spawn().unwrap();
        let d = proc_details(child.id()).expect("child should be visible");
        assert_eq!(d.pid, child.id());
        assert!(d.cmd.contains("60"));
        end_process(child.id()).unwrap();
        let status = child.wait().unwrap();
        assert!(!status.success());
    }
}
