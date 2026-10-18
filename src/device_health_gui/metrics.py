"""Live system metrics read from /proc and /sys.

Everything here is plain Python (no Qt): one SystemStats instance is owned by
the sampler thread and never touched from the UI thread.
"""
import os
import time
from dataclasses import dataclass, field

try:
    _HZ = os.sysconf("SC_CLK_TCK")   # clock ticks per second
    _PAGE = os.sysconf("SC_PAGE_SIZE")
except (ValueError, OSError):        # pragma: no cover - unusual platforms
    _HZ, _PAGE = 100, 4096

MIN_DT = 0.5   # ignore wall-clock deltas shorter than this for rate metrics
TOP_N = 8      # processes reported per snapshot


@dataclass
class ProcInfo:
    pid: int
    name: str
    cpu: float   # % of one core since the previous sample
    mem: float   # % of total RAM


@dataclass
class Snapshot:
    """One consistent reading of every metric the dashboard displays."""
    cpu: float = 0.0
    per_core: list = field(default_factory=list)
    mem_used: int = 0
    mem_total: int = 0
    mem_pct: float = 0.0
    disk_used: int = 0
    disk_total: int = 0
    disk_pct: float = 0.0
    temp: float = None
    uptime: float = 0.0
    load: tuple = (0.0, 0.0, 0.0)
    net_rx: float = 0.0
    net_tx: float = 0.0
    procs: list = field(default_factory=list)


class SystemStats:
    """Samples every metric once. Create one instance per sampling thread."""

    def __init__(self):
        now = time.monotonic()
        self._prev_cpu = self._read_cpu()
        self._prev_net = self._read_net_counters()
        self._prev_net_t = now
        self._prev_proc_ticks = self._read_all_proc_ticks()
        self._prev_proc_t = now

    # --- CPU ---------------------------------------------------------------
    @staticmethod
    def _read_cpu():
        """{cpu line name: (total jiffies, idle jiffies)} for cpu and cpu0..N."""
        out = {}
        try:
            with open("/proc/stat") as f:
                for line in f:
                    if not line.startswith("cpu"):
                        continue
                    name, _, rest = line.partition(" ")
                    p = [int(x) for x in rest.split()[:8]]
                    out[name] = (sum(p), p[3] + p[4])  # idle + iowait
        except OSError:
            pass
        return out

    @staticmethod
    def _pct(prev, cur):
        (lt, li), (t, i) = prev, cur
        d = t - lt
        return max(0.0, min(100.0, 100.0 * (d - (i - li)) / d)) if d > 0 else 0.0

    def cpu(self):
        """Returns (overall percent, [percent per logical core])."""
        cur = self._read_cpu()
        prev, self._prev_cpu = self._prev_cpu, cur
        keys = sorted(
            (k for k in cur if k != "cpu"),
            key=lambda k: int(k[3:]) if k[3:].isdigit() else 0,
        )
        per_core = [
            self._pct(prev[k], cur[k])
            for k in keys
            if k in prev and k in cur
        ]
        if "cpu" in cur and "cpu" in prev:
            total = self._pct(prev["cpu"], cur["cpu"])
        elif per_core:
            total = sum(per_core) / len(per_core)
        else:
            total = 0.0
        return total, per_core

    # --- Memory / disk / sensors ------------------------------------------
    @staticmethod
    def memory():
        """Returns (used_bytes, total_bytes, percent)."""
        try:
            m = {}
            with open("/proc/meminfo") as f:
                for line in f:
                    k, _, v = line.partition(":")
                    m[k] = int(v.split()[0]) * 1024
            total = m["MemTotal"]
            used = total - m.get("MemAvailable", m.get("MemFree", 0))
            return used, total, 100.0 * used / total
        except Exception:
            return 0, 0, 0.0

    @staticmethod
    def disk(path="/"):
        try:
            s = os.statvfs(path)
            total = s.f_blocks * s.f_frsize
            used = total - s.f_bavail * s.f_frsize
            return used, total, 100.0 * used / total
        except Exception:
            return 0, 0, 0.0

    @staticmethod
    def temperature():
        """Hottest thermal zone in °C, or None when no sensor is readable."""
        temps = []
        try:
            for zone in os.listdir("/sys/class/thermal"):
                try:
                    with open(f"/sys/class/thermal/{zone}/temp") as f:
                        v = int(f.read().strip())
                    if v > 0:
                        temps.append(v / 1000.0)
                except Exception:
                    pass
        except Exception:
            pass
        return max(temps) if temps else None

    @staticmethod
    def uptime():
        try:
            with open("/proc/uptime") as f:
                return float(f.read().split()[0])
        except Exception:
            return 0.0

    @staticmethod
    def load_avg():
        try:
            return os.getloadavg()
        except Exception:
            return 0.0, 0.0, 0.0

    # --- Network -----------------------------------------------------------
    @staticmethod
    def _read_net_counters():
        rx = tx = 0
        try:
            with open("/proc/net/dev") as f:
                for line in f:
                    name, sep, data = line.partition(":")
                    if sep and name.strip() != "lo":
                        p = data.split()
                        rx += int(p[0])
                        tx += int(p[8])
        except OSError:
            pass
        return rx, tx

    def network(self):
        """Returns (rx_kbps, tx_kbps), excluding the loopback interface."""
        now = time.monotonic()
        cur = self._read_net_counters()
        prev, prev_t = self._prev_net, self._prev_net_t
        self._prev_net, self._prev_net_t = cur, now
        dt = now - prev_t
        if dt < MIN_DT:
            return 0.0, 0.0
        return (cur[0] - prev[0]) / dt / 1024, (cur[1] - prev[1]) / dt / 1024

    # --- Processes ---------------------------------------------------------
    @staticmethod
    def _read_proc(pid):
        """Returns (utime+stime jiffies, comm) for pid, or None."""
        try:
            with open(f"/proc/{pid}/stat") as f:
                s = f.read()
            r = s.rfind(")")
            comm = s[s.find("(") + 1:r].strip()
            fields = s[r + 1:].split()  # fields[0] is state (field 3)
            ticks = int(fields[11]) + int(fields[12])  # utime + stime
            return ticks, comm
        except (OSError, IndexError, ValueError):
            return None

    @staticmethod
    def _read_all_proc_ticks():
        out = {}
        try:
            entries = os.listdir("/proc")
        except OSError:
            return out
        for name in entries:
            if name.isdigit():
                info = SystemStats._read_proc(int(name))
                if info is not None:
                    out[int(name)] = info[0]
        return out

    @staticmethod
    def _proc_rss(pid):
        try:
            with open(f"/proc/{pid}/statm") as f:
                return int(f.read().split()[1]) * _PAGE
        except (OSError, IndexError, ValueError):
            return 0

    @staticmethod
    def _resolve_name(pid, comm):
        try:
            with open(f"/proc/{pid}/cmdline", "rb") as f:
                raw = f.read()
        except OSError:
            raw = b""
        if raw:
            exe = raw.split(b"\0", 1)[0].decode("utf-8", "replace").strip()
            # Some programs (Chrome, Electron) rewrite argv into one
            # space-joined string; keep only the program part.
            exe = exe.split(" --", 1)[0].strip()
            if exe:
                return os.path.basename(exe) or exe
        return comm or str(pid)

    def processes(self, mem_total):
        """Top processes by CPU since the previous sample."""
        now = time.monotonic()
        prev, prev_t = self._prev_proc_ticks, self._prev_proc_t
        cur, rows = {}, []
        try:
            entries = os.listdir("/proc")
        except OSError:
            entries = []
        for name in entries:
            if not name.isdigit():
                continue
            pid = int(name)
            info = self._read_proc(pid)
            if info is None:
                continue
            ticks, comm = info
            cur[pid] = ticks
            cpu = 0.0
            dt = now - prev_t
            if pid in prev and dt >= MIN_DT:
                delta = ticks - prev[pid]
                if delta >= 0:
                    cpu = delta / _HZ / dt * 100.0
            rows.append((pid, cpu, self._proc_rss(pid), comm))
        self._prev_proc_ticks, self._prev_proc_t = cur, now
        rows.sort(key=lambda r: (r[1], r[2]), reverse=True)
        out = []
        for pid, cpu, rss, comm in rows[:TOP_N]:
            name = self._resolve_name(pid, comm)
            mem = 100.0 * rss / mem_total if mem_total else 0.0
            out.append(ProcInfo(pid, name, cpu, mem))
        return out

    # --- Combined ----------------------------------------------------------
    def sample(self):
        cpu_pct, per_core = self.cpu()
        mem_used, mem_total, mem_pct = self.memory()
        d_used, d_total, d_pct = self.disk()
        rx, tx = self.network()
        return Snapshot(
            cpu=cpu_pct,
            per_core=per_core,
            mem_used=mem_used,
            mem_total=mem_total,
            mem_pct=mem_pct,
            disk_used=d_used,
            disk_total=d_total,
            disk_pct=d_pct,
            temp=self.temperature(),
            uptime=self.uptime(),
            load=self.load_avg(),
            net_rx=rx,
            net_tx=tx,
            procs=self.processes(mem_total),
        )
