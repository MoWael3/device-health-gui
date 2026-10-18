"""Main window: assembles cards and applies sampled snapshots to the UI."""
import os
import platform
import sys
import threading
import time
from collections import deque

from PyQt6.QtCore import Qt, pyqtSignal
from PyQt6.QtGui import QColor, QIcon
from PyQt6.QtWidgets import (QApplication, QGridLayout, QHBoxLayout, QLabel,
                             QMainWindow, QScrollArea, QVBoxLayout,
                             QWidget)

from .metrics import Snapshot
from .opencode import analyze
from .sampler import Sampler
from .theme import CRIT, OK, STYLE, T, WARN, device_name, fmt_rate, fmt_uptime
from .widgets import (AICard, Badge, LineChart, MetricCard, PerCoreCard,
                      ProcessTable, axis_row, make_card, make_label)


AI_WINDOW = 60  # seconds of history summarised for the AI
ICON = os.path.join(os.path.dirname(os.path.abspath(__file__)), "icon.svg")


def _avg_peak(values):
    values = [v for v in values if v is not None]
    if not values:
        return "unknown", "unknown"
    return round(sum(values) / len(values), 1), round(max(values), 1)


def ai_summary(history):
    """Condense recent snapshots into the dict opencode.build_prompt expects."""
    last = history[-1]
    cpu_avg, cpu_peak = _avg_peak([h.cpu for h in history])
    mem_avg, mem_peak = _avg_peak([h.mem_pct for h in history])
    temp_avg, temp_peak = _avg_peak([h.temp for h in history])
    # The first sample has no previous counters, so skip it for rates.
    rates = history[1:] if len(history) > 1 else history
    rx_avg, rx_peak = _avg_peak([h.net_rx for h in rates])
    tx_avg, tx_peak = _avg_peak([h.net_tx for h in rates])
    return {
        "window": len(history), "cores": os.cpu_count(),
        "cpu_avg": cpu_avg, "cpu_peak": cpu_peak,
        "load": " ".join(f"{x:.2f}" for x in last.load),
        "mem_used": round(last.mem_used / 2**20), "mem_total": round(last.mem_total / 2**20),
        "mem_avg": mem_avg, "mem_peak": mem_peak,
        "disk_used": round(last.disk_used / 2**30), "disk_total": round(last.disk_total / 2**30),
        "disk_pct": round(last.disk_pct),
        "temp_avg": temp_avg, "temp_peak": temp_peak,
        "rx_avg": rx_avg, "rx_peak": rx_peak, "tx_avg": tx_avg, "tx_peak": tx_peak,
        "top_procs": [(p.name, round(p.cpu, 1)) for p in last.procs[:3]],
        "uptime": round(last.uptime),
    }


class Dashboard(QMainWindow):
    # Emitted from the AI worker thread; Qt queues it onto the GUI thread.
    ai_done = pyqtSignal(str)

    def __init__(self):
        super().__init__()
        self.history = deque(maxlen=AI_WINDOW)
        self.setWindowTitle("Device Health Dashboard")
        self.resize(1180, 800)
        # The window can shrink freely; below the dashboard's own minimum the
        # content scrolls instead of being squashed.
        self.setMinimumSize(480, 360)

        root = QWidget()
        root.setObjectName("root")
        root.setMinimumSize(960, 720)
        scroll = QScrollArea()
        scroll.setObjectName("rootScroll")
        scroll.setWidget(root)
        scroll.setWidgetResizable(True)
        scroll.setFrameShape(QScrollArea.Shape.NoFrame)
        self.setCentralWidget(scroll)
        outer = QVBoxLayout(root)
        outer.setContentsMargins(T.S4 - 4, T.S3, T.S4 - 4, T.S3)
        outer.setSpacing(T.S3 - 4)

        outer.addLayout(self._build_header())

        grid = QGridLayout()
        grid.setSpacing(T.S2)
        self.cpu = MetricCard("CPU Usage", 70, 90)
        self.mem = MetricCard("Memory", 75, 90)
        self.disk = MetricCard("Disk  /", 80, 92)
        self.temp = MetricCard("Temperature", 70, 85, unit="°C")
        for i, c in enumerate((self.cpu, self.mem, self.disk, self.temp)):
            grid.addWidget(c, 0, i)
            grid.setColumnStretch(i, 1)
        outer.addLayout(grid, 4)

        mid = QHBoxLayout()
        mid.setSpacing(T.S2)
        self.per_core = PerCoreCard()
        self.procs = ProcessTable()
        mid.addWidget(self.per_core, 1)
        mid.addWidget(self.procs, 1)
        outer.addLayout(mid, 4)

        bottom = QHBoxLayout()
        bottom.setSpacing(T.S2)
        bottom.addWidget(self._build_network_card(), 3)
        bottom.addWidget(self._build_system_card(), 2)
        self.ai = AICard()
        self.ai.button.clicked.connect(self._run_ai)
        self.ai_done.connect(self._show_ai_result)
        bottom.addWidget(self.ai, 3)
        outer.addLayout(bottom, 3)

        self.sampler = Sampler(interval=1.0, parent=self)
        self.sampler.sampled.connect(self.apply_snapshot)
        self.sampler.start()

    # --- UI construction ----------------------------------------------------
    def _build_header(self):
        row = QHBoxLayout()
        titles = QVBoxLayout()
        titles.setSpacing(2)
        titles.addWidget(make_label("Device Health Dashboard", "appTitle"))
        titles.addWidget(make_label(
            f"{device_name()}  ·  {platform.system()} {platform.release()}", "subtitle"))
        row.addLayout(titles)
        row.addStretch()
        self.overall = Badge()
        row.addWidget(self.overall)
        self.clock = make_label("", "subtitle")
        row.addSpacing(T.S2)
        row.addWidget(self.clock)
        return row

    def _build_network_card(self):
        card, lay = make_card()
        head = QHBoxLayout()
        head.addWidget(make_label("NETWORK THROUGHPUT", "cardTitle"))
        head.addStretch()
        self.rx_label = make_label("", "infoVal")
        self.rx_label.setStyleSheet(f"color: {T.RX};")
        self.tx_label = make_label("", "infoVal")
        self.tx_label.setStyleSheet(f"color: {T.TX};")
        for lab in (self.rx_label, self.tx_label):
            lab.setMinimumWidth(110)
            lab.setAlignment(Qt.AlignmentFlag.AlignRight)
            head.addWidget(lab)
        lay.addLayout(head)

        self.net_chart = LineChart([T.RX, T.TX], scale_fmt=fmt_rate)
        lay.addWidget(self.net_chart, 1)
        lay.addWidget(axis_row())
        return card

    def _build_system_card(self):
        card, lay = make_card()
        lay.addWidget(make_label("SYSTEM", "cardTitle"))
        self.info = {}
        rows = QGridLayout()
        rows.setVerticalSpacing(T.S2 - 4)
        for r, key in enumerate(("Uptime", "Load average", "CPU cores", "Memory",
                                 "Disk")):
            rows.addWidget(make_label(f"{key}:", "infoKey"), r, 0)
            val = make_label("--", "infoVal", Qt.AlignmentFlag.AlignRight)
            rows.addWidget(val, r, 1)
            self.info[key] = val
        rows.setColumnStretch(1, 1)
        lay.addLayout(rows)
        lay.addStretch()
        self.info["CPU cores"].setText(str(os.cpu_count() or "--"))
        return card

    # --- Snapshot → UI (runs on the GUI thread via queued signal) -----------
    def apply_snapshot(self, s: Snapshot):
        self.history.append(s)
        statuses = {}

        statuses["CPU"] = self.cpu.update_metric(
            s.cpu, f"{s.cpu:.0f}", f"{os.cpu_count()} logical cores")

        statuses["Memory"] = self.mem.update_metric(
            s.mem_pct, f"{s.mem_pct:.0f}",
            f"{s.mem_used / 2**30:.1f} of {s.mem_total / 2**30:.1f} GB used")

        statuses["Disk"] = self.disk.update_metric(
            s.disk_pct, f"{s.disk_pct:.0f}",
            f"{s.disk_used / 2**30:.0f} of {s.disk_total / 2**30:.0f} GB used")

        if s.temp is None:
            self.temp.ring.set_state(0, T.MUTED)
            self.temp.hero.setText("N/A")
            self.temp.detail.setText("No thermal sensor found")
            self.temp.badge.hide()
            self.temp.spark.colors[0] = QColor(T.MUTED)
        else:
            statuses["Temperature"] = self.temp.update_metric(
                s.temp, f"{s.temp:.0f}", "Hottest sensor")

        core_color = self.per_core.update_cores(s.per_core)
        if core_color != OK:
            statuses["CPU cores"] = core_color

        self.procs.update_procs(s.procs)

        self.net_chart.push(s.net_rx, s.net_tx)
        self.rx_label.setText(f"↓ {fmt_rate(s.net_rx)}")
        self.tx_label.setText(f"↑ {fmt_rate(s.net_tx)}")

        l1, l5, l15 = s.load
        self.info["Uptime"].setText(fmt_uptime(s.uptime))
        self.info["Load average"].setText(f"{l1:.2f}  {l5:.2f}  {l15:.2f}")
        self.info["Memory"].setText(f"{s.mem_total / 2**30:.1f} GB total")
        self.info["Disk"].setText(f"{s.disk_total / 2**30:.0f} GB total")

        crit = [k for k, c in statuses.items() if c == CRIT]
        warn = [k for k, c in statuses.items() if c == WARN]
        if crit:
            self.overall.set_status(CRIT, f"Critical: {', '.join(crit)}")
        elif warn:
            self.overall.set_status(WARN, f"Warning: {', '.join(warn)}")
        else:
            self.overall.set_status(OK, "All systems healthy")
        self.clock.setText(time.strftime("%a %d %b  %H:%M:%S"))

    # --- AI summary ---------------------------------------------------------
    def _run_ai(self):
        if not self.history:
            return
        summary = ai_summary(list(self.history))
        self._ai_stamp = f"as of {time.strftime('%H:%M')} · {summary['window']}s avg"
        self.ai.set_busy(summary["window"])
        threading.Thread(target=lambda: self.ai_done.emit(analyze(summary)),
                         daemon=True).start()

    def _show_ai_result(self, text):
        self.ai.set_result(text, self._ai_stamp)

    def closeEvent(self, event):
        self.sampler.stop()
        self.sampler.wait(3000)
        super().closeEvent(event)


def main(argv=None):
    app = QApplication(argv if argv is not None else sys.argv)
    app.setStyle("Fusion")
    app.setStyleSheet(STYLE)
    app.setApplicationName("Device Health Dashboard")
    app.setDesktopFileName("device-health-dashboard")
    app.setWindowIcon(QIcon(ICON))
    win = Dashboard()
    avail = app.primaryScreen().availableGeometry()
    win.resize(min(1280, int(avail.width() * 0.92)), min(900, int(avail.height() * 0.92)))
    win.show()
    return app.exec()
