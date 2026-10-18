"""Design tokens and shared formatting helpers."""
import socket

HISTORY = 60  # samples kept for charts (1 per second)


class Theme:
    # Palette (neutrals follow device-health-dashboard/dashboard.py)
    BG = "#1e1e1e"
    SURFACE = "#262626"
    ROW = "#2c2c2c"
    BORDER = "#3a3a3a"
    GRID = "#323232"
    TRACK = "#383838"
    TEXT = "#ffffff"
    MUTED = "#a6a6a6"
    ACCENT = "#4c8dff"
    OK = "#2ecc9a"
    WARN = "#f5b335"
    CRIT = "#f0506e"
    RX = ACCENT
    TX = "#b07cff"
    # Spacing (8px grid) and radii
    S1, S2, S3, S4 = 8, 16, 24, 32
    R_CARD, R_PILL = 12, 12
    # Type scale (px): label / body / hero, plus title
    F_LABEL, F_BODY, F_HERO, F_TITLE = 11, 13, 30, 22
    FONT = 'Arial, "Liberation Sans", sans-serif'


T = Theme
OK, WARN, CRIT = T.OK, T.WARN, T.CRIT

STYLE = f"""
QMainWindow, #root, #rootScroll {{ background: {T.BG}; }}
QLabel {{ background: transparent; color: {T.TEXT}; font-family: {T.FONT}; font-size: {T.F_BODY}px; }}
#card {{ background: {T.SURFACE}; border: 1px solid {T.BORDER}; border-radius: {T.R_CARD}px; }}
#cardTitle {{ color: {T.MUTED}; font-size: {T.F_LABEL}px; font-weight: 700; letter-spacing: 1px; }}
#appTitle {{ font-size: {T.F_TITLE}px; font-weight: 700; }}
#subtitle, #detail, #infoKey, #axis {{ color: {T.MUTED}; }}
#axis {{ font-size: {T.F_LABEL}px; }}
#infoVal {{ font-weight: 700; }}
#hero {{ font-size: {T.F_HERO}px; font-weight: 700; }}
#coreName {{ font-family: monospace; font-size: {T.F_LABEL}px; color: {T.MUTED}; }}
#corePct {{ font-size: {T.F_LABEL}px; font-weight: 700; color: {T.MUTED}; }}
QTableWidget {{
    background: transparent; alternate-background-color: {T.ROW}; border: none;
    color: {T.TEXT}; font-size: {T.F_BODY}px; gridline-color: transparent;
}}
QTableWidget::item {{ border: none; padding: 0px 6px; }}
QTableWidget::item:selected {{ background: transparent; color: {T.TEXT}; }}
#aiText {{ color: {T.TEXT}; }}
QPushButton {{
    background: {T.ACCENT}; color: {T.BG}; border: none; border-radius: 6px;
    padding: 6px 14px; font-family: {T.FONT}; font-size: {T.F_BODY}px; font-weight: 700;
}}
QPushButton:hover {{ background: #6aa0ff; }}
QPushButton:disabled {{ background: {T.TRACK}; color: {T.MUTED}; }}
QScrollArea, QScrollArea > QWidget > QWidget {{ background: transparent; border: none; }}
QScrollBar:vertical {{ background: transparent; width: 8px; margin: 0; }}
QScrollBar:horizontal {{ background: transparent; height: 8px; margin: 0; }}
QScrollBar::handle:horizontal {{ background: {T.TRACK}; border-radius: 4px; min-width: 24px; }}
QScrollBar::add-line:horizontal, QScrollBar::sub-line:horizontal,
QScrollBar::add-page:horizontal, QScrollBar::sub-page:horizontal {{ width: 0; background: none; }}
QScrollBar::handle:vertical {{ background: {T.TRACK}; border-radius: 3px; min-height: 24px; }}
QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical,
QScrollBar::add-page:vertical, QScrollBar::sub-page:vertical {{ height: 0; background: none; }}
QHeaderView, QTableCornerButton::section {{ background: transparent; border: none; }}
QHeaderView::section {{
    background: transparent; color: {T.MUTED}; border: none;
    border-bottom: 1px solid {T.BORDER}; padding: 5px 6px;
    font-size: {T.F_LABEL}px; font-weight: 700; letter-spacing: 1px;
}}
"""


def status_color(value, warn, crit):
    return CRIT if value >= crit else WARN if value >= warn else OK


def status_label(color):
    return {OK: "Healthy", WARN: "Warning", CRIT: "Critical"}.get(color, "Unknown")


def fmt_rate(kbps):
    if kbps >= 1024:
        return f"{kbps / 1024:.1f} MB/s"
    return f"{kbps:.1f} KB/s"


def fmt_uptime(seconds):
    d, rem = divmod(int(seconds), 86400)
    h, rem = divmod(rem, 3600)
    m, s = divmod(rem, 60)
    return f"{d}d {h:02d}h {m:02d}m" if d else f"{h:02d}h {m:02d}m {s:02d}s"


def device_name():
    return socket.gethostname().strip() or "Linux device"
