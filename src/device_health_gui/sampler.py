"""Background sampler: reads /proc and /sys off the UI thread."""
import time

from PyQt6.QtCore import QThread, pyqtSignal

from .metrics import Snapshot, SystemStats


class Sampler(QThread):
    """Emits a Snapshot every `interval` seconds until stopped."""

    sampled = pyqtSignal(object)   # Snapshot

    def __init__(self, interval=1.0, parent=None):
        super().__init__(parent)
        self.interval = interval
        self._stats = SystemStats()
        self._running = True

    def stop(self):
        self._running = False

    def run(self):
        while self._running:
            started = time.monotonic()
            try:
                snap = self._stats.sample()
            except Exception:
                snap = Snapshot()
            self.sampled.emit(snap)
            remaining = self.interval - (time.monotonic() - started)
            if remaining > 0:
                self.msleep(int(remaining * 1000))
