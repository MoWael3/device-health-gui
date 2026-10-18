"""Custom-painted widgets and cards for the dashboard."""
import html
import os
from collections import deque

from PyQt6.QtCore import QPointF, QRectF, Qt
from PyQt6.QtGui import (QBrush, QColor, QFont, QLinearGradient, QPainter,
                         QPainterPath, QPen)
from PyQt6.QtWidgets import (QFrame, QGridLayout, QHBoxLayout, QLabel,
                             QPushButton, QScrollArea, QTableWidget,
                             QTableWidgetItem, QVBoxLayout, QWidget)

from .theme import CRIT, HISTORY, OK, T, WARN, status_color, status_label


# --- Small helpers -----------------------------------------------------------
def make_label(text="", name=None, align=None):
    lab = QLabel(text)
    if name:
        lab.setObjectName(name)
    if align:
        lab.setAlignment(align)
    return lab


def make_card():
    card = QFrame()
    card.setObjectName("card")
    lay = QVBoxLayout(card)
    lay.setContentsMargins(T.S3 - 4, T.S2, T.S3 - 4, T.S2)
    lay.setSpacing(T.S1)
    return card, lay


def card_head(title):
    """Title row (title ... stretch ...) — caller appends the right side."""
    head = QHBoxLayout()
    head.setContentsMargins(0, 0, 0, 0)
    head.addWidget(make_label(title.upper(), "cardTitle"))
    head.addStretch()
    return head


# --- Custom painted widgets --------------------------------------------------
class RingGauge(QWidget):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.value = 0.0
        self.color = QColor(T.MUTED)
        self.setFixedSize(88, 88)

    def set_state(self, value, color):
        self.value, self.color = value, QColor(color)
        self.update()

    def paintEvent(self, _):
        p = QPainter(self)
        p.setRenderHint(QPainter.RenderHint.Antialiasing)
        thick = 9
        rect = QRectF(self.rect()).adjusted(thick / 2 + 1, thick / 2 + 1,
                                            -thick / 2 - 1, -thick / 2 - 1)
        pen = QPen(QColor(T.TRACK), thick)
        pen.setCapStyle(Qt.PenCapStyle.RoundCap)
        p.setPen(pen)
        p.drawArc(rect, 225 * 16, -270 * 16)
        span = int(-270 * 16 * max(0.0, min(100.0, self.value)) / 100)
        if span:
            pen.setColor(self.color)
            p.setPen(pen)
            p.drawArc(rect, 225 * 16, span)


class LineChart(QWidget):
    """Filled line chart. History starts empty and fills in from the right.

    `scale_fmt(value)` labels the top and middle gridlines when given.
    """

    def __init__(self, series_colors, fixed_max=None, scale_fmt=None, parent=None):
        super().__init__(parent)
        self.colors = [QColor(c) for c in series_colors]
        self.data = [deque(maxlen=HISTORY) for _ in series_colors]
        self.fixed_max = fixed_max
        self.ymax = fixed_max or 1.0
        self.scale_fmt = scale_fmt

    def push(self, *values):
        for d, v in zip(self.data, values):
            d.append(v)
        if not self.fixed_max:
            # Grow instantly, shrink slowly so the scale doesn't jump around.
            peak = max((max(d) for d in self.data if d), default=0) * 1.15
            self.ymax = max(peak, self.ymax * 0.97, 1.0)
        self.update()

    def paintEvent(self, _):
        p = QPainter(self)
        p.setRenderHint(QPainter.RenderHint.Antialiasing)
        w, h = self.width(), self.height()

        p.setPen(QPen(QColor(T.GRID), 1))
        for frac in (0.0, 0.5, 1.0):
            y = min(h - 1, max(0, h * frac))
            p.drawLine(QPointF(0, y), QPointF(w, y))

        if self.scale_fmt:
            font = QFont(self.font())
            font.setPixelSize(T.F_LABEL - 1)
            p.setFont(font)
            p.setPen(QColor(T.MUTED))
            fm = p.fontMetrics()
            for frac in (0.0, 0.5):
                text = self.scale_fmt(self.ymax * (1 - frac))
                tw = fm.horizontalAdvance(text) + 6
                box = QRectF(0, h * frac + 2, tw, fm.height())
                p.fillRect(box, QColor(T.SURFACE))  # keep labels readable over lines
                p.drawText(box, int(Qt.AlignmentFlag.AlignCenter), text)

        step = w / (HISTORY - 1)
        for color, d in zip(self.colors, self.data):
            if len(d) < 2:
                continue
            n = len(d)
            pts = [QPointF(w - (n - 1 - i) * step,
                           h - 2 - (h - 4) * min(v, self.ymax) / self.ymax)
                   for i, v in enumerate(d)]
            line = QPainterPath(pts[0])
            for pt in pts[1:]:
                line.lineTo(pt)
            area = QPainterPath(line)
            area.lineTo(pts[-1].x(), h)
            area.lineTo(pts[0].x(), h)
            area.closeSubpath()

            grad = QLinearGradient(0, 0, 0, h)
            top_c, bot_c = QColor(color), QColor(color)
            top_c.setAlpha(70)
            bot_c.setAlpha(0)
            grad.setColorAt(0, top_c)
            grad.setColorAt(1, bot_c)
            p.fillPath(area, QBrush(grad))
            p.setPen(QPen(color, 1.8, Qt.PenStyle.SolidLine, Qt.PenCapStyle.RoundCap,
                          Qt.PenJoinStyle.RoundJoin))
            p.drawPath(line)


class Badge(QLabel):
    """Rounded status pill whose color follows the current status."""

    def __init__(self, parent=None):
        super().__init__(parent)
        self.setFixedHeight(24)

    def set_status(self, color, text=None):
        self.setText(f"●  {text or status_label(color)}")
        c = QColor(color)
        self.setStyleSheet(
            f"color: {color}; background: rgba({c.red()},{c.green()},{c.blue()},0.14);"
            f"border-radius: {T.R_PILL}px; padding: 0px 12px; font-size: {T.F_LABEL}px;"
            "font-weight: 700;")


class HBar(QWidget):
    """Compact horizontal bar used by the per-core panel."""

    def __init__(self, parent=None):
        super().__init__(parent)
        self.value = 0.0
        self.color = QColor(T.ACCENT)
        self.setFixedHeight(8)
        self.setMinimumWidth(40)

    def set_state(self, value, color):
        self.value, self.color = max(0.0, min(100.0, value)), QColor(color)
        self.update()

    def paintEvent(self, _):
        p = QPainter(self)
        p.setRenderHint(QPainter.RenderHint.Antialiasing)
        r = QRectF(self.rect()).adjusted(1, 1, -1, -1)
        p.setPen(Qt.PenStyle.NoPen)
        p.setBrush(QColor(T.TRACK))
        p.drawRoundedRect(r, r.height() / 2, r.height() / 2)
        w = r.width() * self.value / 100.0
        if w > 0.5:
            p.setBrush(self.color)
            p.drawRoundedRect(QRectF(r.x(), r.y(), w, r.height()),
                              r.height() / 2, r.height() / 2)


# --- Cards -------------------------------------------------------------------
class MetricCard(QFrame):
    """Title row, ring + hero value, detail line, 60s history strip."""

    def __init__(self, title, warn, crit, unit="%"):
        super().__init__()
        self.warn, self.crit, self.unit = warn, crit, unit
        self.setObjectName("card")
        lay = QVBoxLayout(self)
        lay.setContentsMargins(T.S3 - 4, T.S2, T.S3 - 4, T.S2)
        lay.setSpacing(T.S2)

        head = card_head(title)
        self.badge = Badge()
        self.badge.hide()  # only shown when something needs attention
        head.addWidget(self.badge)
        head_wrap = QWidget()
        head_wrap.setFixedHeight(24)
        head_wrap.setLayout(head)
        lay.addWidget(head_wrap)

        body = QHBoxLayout()
        body.setSpacing(T.S2)
        self.ring = RingGauge()
        body.addWidget(self.ring)
        text = QVBoxLayout()
        text.setSpacing(2)
        text.addStretch()
        self.hero = make_label("--", "hero")
        self.detail = make_label("", "detail")
        self.detail.setWordWrap(True)
        text.addWidget(self.hero)
        text.addWidget(self.detail)
        text.addStretch()
        body.addLayout(text, 1)
        lay.addLayout(body)

        scale = (lambda v: f"{v:.0f}{unit}") if unit == "%" else None
        self.spark = LineChart([T.ACCENT], fixed_max=100, scale_fmt=scale)
        self.spark.setMinimumHeight(56)
        lay.addWidget(self.spark, 1)
        lay.addWidget(axis_row())

    def update_metric(self, percent, hero_value, detail, chart_value=None):
        color = status_color(percent, self.warn, self.crit)
        self.ring.set_state(percent, color)
        self.hero.setText(
            f"{hero_value}<span style='font-size:{T.F_BODY + 2}px;"
            f"color:{T.MUTED};font-weight:400'> {self.unit}</span>")
        self.detail.setText(detail)
        if color == OK:
            self.badge.hide()
        else:
            self.badge.set_status(color)
            self.badge.show()
        self.spark.colors[0] = QColor(color)
        self.spark.push(percent if chart_value is None else chart_value)
        return color


def axis_row():
    row = QWidget()
    r = QHBoxLayout(row)
    r.setContentsMargins(0, 0, 0, 0)
    r.addWidget(make_label("60s ago", "axis"))
    r.addStretch()
    r.addWidget(make_label("now", "axis"))
    return row


class PerCoreCard(QFrame):
    """Grid of one bar row per logical core."""

    WARN, CRIT = 70, 90

    def __init__(self):
        super().__init__()
        self.setObjectName("card")
        lay = QVBoxLayout(self)
        lay.setContentsMargins(T.S3 - 4, T.S2, T.S3 - 4, T.S2)
        lay.setSpacing(T.S2)

        head = card_head("Per-core CPU")
        self.badge = Badge()
        self.badge.hide()
        head.addWidget(self.badge)
        head_wrap = QWidget()
        head_wrap.setFixedHeight(24)
        head_wrap.setLayout(head)
        lay.addWidget(head_wrap)

        self.grid = QGridLayout()
        self.grid.setContentsMargins(0, 0, 0, 0)
        self.grid.setHorizontalSpacing(T.S3)
        self.grid.setVerticalSpacing(T.S1)
        lay.addLayout(self.grid, 1)
        lay.addWidget(axis_row())

        self.rows = []          # [(name_label, bar, pct_label), ...]
        self._n = 0

    def _build(self, n):
        for row in self.rows:
            for w in row:
                w.setParent(None)
        self.rows = []
        cols = 2 if n <= 16 else 4
        for i in range(n):
            name = make_label(f"cpu{i}", "coreName")
            name.setFixedWidth(44)
            bar = HBar()
            pct = make_label("--", "corePct", Qt.AlignmentFlag.AlignRight)
            pct.setFixedWidth(40)
            r, c = divmod(i, cols)
            self.grid.addWidget(name, r, c * 3)
            self.grid.addWidget(bar, r, c * 3 + 1)
            self.grid.addWidget(pct, r, c * 3 + 2)
            self.grid.setColumnStretch(c * 3 + 1, 1)
            self.rows.append((name, bar, pct))
        self._n = n

    def update_cores(self, values):
        if len(values) != self._n:
            self._build(len(values))
        worst = 0.0
        for (_, bar, pct), v in zip(self.rows, values):
            color = status_color(v, self.WARN, self.CRIT)
            bar.set_state(v, color)
            pct.setText(f"{v:.0f}%")
            pct.setStyleSheet(f"color: {color};")
            worst = max(worst, v)
        if not values:
            self.badge.hide()
            return OK
        color = status_color(worst, self.WARN, self.CRIT)
        if color == OK:
            self.badge.hide()
        else:
            self.badge.set_status(color)
            self.badge.show()
        return color


class ProcessTable(QFrame):
    """Top processes by CPU since the previous sample."""

    def __init__(self, rows=8):
        super().__init__()
        self.setObjectName("card")
        lay = QVBoxLayout(self)
        lay.setContentsMargins(T.S3 - 4, T.S2, T.S3 - 4, T.S2)
        lay.setSpacing(T.S2)

        head = card_head("Top processes")
        self.note = make_label("", "axis")
        head.addWidget(self.note)
        head_wrap = QWidget()
        head_wrap.setFixedHeight(24)
        head_wrap.setLayout(head)
        lay.addWidget(head_wrap)

        self.table = QTableWidget(rows, 4)
        self.table.setHorizontalHeaderLabels(("Process", "PID", "CPU", "MEM"))
        self.table.verticalHeader().hide()
        self.table.setEditTriggers(QTableWidget.EditTrigger.NoEditTriggers)
        self.table.setSelectionMode(QTableWidget.SelectionMode.NoSelection)
        self.table.setFocusPolicy(Qt.FocusPolicy.NoFocus)
        self.table.setShowGrid(False)
        self.table.setAlternatingRowColors(True)
        self.table.setWordWrap(False)
        self.table.verticalHeader().setDefaultSectionSize(24)
        self.table.horizontalHeader().setStretchLastSection(False)
        from PyQt6.QtWidgets import QHeaderView
        hdr = self.table.horizontalHeader()
        hdr.setSectionResizeMode(0, QHeaderView.ResizeMode.Stretch)
        for c in (1, 2, 3):
            hdr.setSectionResizeMode(c, QHeaderView.ResizeMode.ResizeToContents)
        self.table.setColumnWidth(1, 64)
        lay.addWidget(self.table, 1)
        self.note.setText("top 8 by CPU")

    def update_procs(self, procs):
        for r in range(self.table.rowCount()):
            p = procs[r] if r < len(procs) else None
            vals = (("", "", "", "") if p is None else
                    (p.name, str(p.pid), f"{p.cpu:.1f}%", f"{p.mem:.1f}%"))
            for c, text in enumerate(vals):
                item = self.table.item(r, c)
                if item is None:
                    item = QTableWidgetItem()
                    if c:
                        item.setTextAlignment(
                            Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)
                    self.table.setItem(r, c, item)
                item.setText(text)
                if p is None or not text:
                    item.setForeground(QColor(T.MUTED))
                elif c == 2:
                    item.setForeground(QColor(
                        status_color(p.cpu, ProcessTable.WARN, ProcessTable.CRIT)))
                else:
                    item.setForeground(QColor(T.TEXT))

    WARN, CRIT = 70, 90


class AICard(QFrame):
    """Asks opencode about recent readings and shows its answer as bullets."""

    def __init__(self):
        super().__init__()
        self.setObjectName("card")
        lay = QVBoxLayout(self)
        lay.setContentsMargins(T.S3 - 4, T.S2, T.S3 - 4, T.S2)
        lay.setSpacing(T.S1)

        head = card_head("AI health summary")
        self.stamp = make_label("", "axis")
        head.addWidget(self.stamp)
        lay.addLayout(head)

        self.text = make_label("Ask opencode to review the last minute of readings.",
                               "detail")
        self.text.setWordWrap(True)
        self.text.setAlignment(Qt.AlignmentFlag.AlignTop | Qt.AlignmentFlag.AlignLeft)
        self.text.setTextInteractionFlags(Qt.TextInteractionFlag.TextSelectableByMouse)
        scroll = QScrollArea()
        scroll.setWidget(self.text)
        scroll.setWidgetResizable(True)
        scroll.setFrameShape(QFrame.Shape.NoFrame)
        scroll.setHorizontalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        lay.addWidget(scroll, 1)

        self.button = QPushButton("Analyze with opencode")
        self.button.setCursor(Qt.CursorShape.PointingHandCursor)
        self.button.setMinimumWidth(self.button.sizeHint().width() + T.S3)
        lay.addWidget(self.button, 0, Qt.AlignmentFlag.AlignLeft)

    def set_busy(self, window_s):
        self.button.setEnabled(False)
        self.button.setText("Analyzing…")
        self.text.setStyleSheet(f"color: {T.MUTED};")
        self.text.setText(f"Asking opencode about the last {window_s}s of readings…")

    def set_result(self, text, stamp):
        lines = [ln.strip().lstrip("-*•").strip() for ln in text.splitlines()]
        lines = [html.escape(ln) for ln in lines if ln]
        self.text.setStyleSheet(f"color: {T.TEXT};")
        self.text.setText("".join(
            f"<p style='margin:0 0 6px 0'>•&nbsp;&nbsp;{ln}</p>" for ln in lines))
        self.stamp.setText(stamp)
        self.button.setText("Analyze again")
        self.button.setEnabled(True)
