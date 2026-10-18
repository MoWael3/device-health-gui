"""Use opencode as the AI provider for device health analysis.

Shells out to `opencode run`, so whatever provider/model opencode is
configured with is what answers. Override with:
  OPENCODE_BIN    path to the opencode binary (default: "opencode")
  OPENCODE_MODEL  "provider/model" passed to --model (default: opencode's own)
"""
import os
import shutil
import subprocess

TIMEOUT = 120


def build_prompt(s: dict) -> str:
    procs = ", ".join(f"{n} {c}%" for n, c in s["top_procs"]) or "none"
    return (
        "You are a device health assistant. Below are readings from a Linux "
        f"machine averaged over the last {s['window']} seconds (peaks in brackets). "
        "Reply with at most 5 short lines, one point per line, no markdown: "
        "whether the machine looks healthy, anything concerning, and one action "
        "if needed. Do not use tools.\n"
        f"CPU: avg {s['cpu_avg']}% [peak {s['cpu_peak']}%], {s['cores']} cores, "
        f"load average {s['load']}\n"
        f"Memory: {s['mem_used']}/{s['mem_total']} MB, avg {s['mem_avg']}% "
        f"[peak {s['mem_peak']}%]\n"
        f"Disk /: {s['disk_used']}/{s['disk_total']} GB ({s['disk_pct']}%)\n"
        f"Temperature: avg {s['temp_avg']} C [peak {s['temp_peak']} C]\n"
        f"Network: down avg {s['rx_avg']} KB/s [peak {s['rx_peak']}], "
        f"up avg {s['tx_avg']} KB/s [peak {s['tx_peak']}]\n"
        f"Top processes by CPU now: {procs}\n"
        f"Uptime: {s['uptime']} s"
    )


def analyze(stats: dict) -> str:
    """Return opencode's analysis of `stats`, or an error message string."""
    binary = os.environ.get("OPENCODE_BIN", "opencode")
    if shutil.which(binary) is None and "OPENCODE_BIN" not in os.environ:
        # Apps launched from the desktop don't get ~/.bashrc's PATH.
        binary = os.path.expanduser("~/.opencode/bin/opencode")
    if shutil.which(binary) is None:
        return f"opencode not found ('{binary}'). Install it or set OPENCODE_BIN."
    cmd = [binary, "run"]
    model = os.environ.get("OPENCODE_MODEL")
    if model:
        cmd += ["--model", model]
    cmd.append(build_prompt(stats))
    try:
        p = subprocess.run(
            cmd, capture_output=True, text=True, timeout=TIMEOUT, stdin=subprocess.DEVNULL
        )
    except subprocess.TimeoutExpired:
        return f"opencode timed out after {TIMEOUT}s."
    out = p.stdout.strip()
    if p.returncode != 0:
        return f"opencode failed ({p.returncode}): {(p.stderr.strip() or out)[:500]}"
    return out or "opencode returned no output."
