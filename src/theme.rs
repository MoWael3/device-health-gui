//! Design tokens and shared formatting helpers (mirrors theme.py).

use egui::Color32;

/// Samples kept for charts (1 per second).
pub const HISTORY: usize = 60;

const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

pub const BG: Color32 = hex(0x1e1e1e);
pub const SURFACE: Color32 = hex(0x262626);
pub const ROW: Color32 = hex(0x2c2c2c);
pub const BORDER: Color32 = hex(0x3a3a3a);
pub const GRID: Color32 = hex(0x323232);
pub const TRACK: Color32 = hex(0x383838);
pub const TEXT: Color32 = hex(0xffffff);
pub const MUTED: Color32 = hex(0xa6a6a6);
pub const ACCENT: Color32 = hex(0x4c8dff);
pub const ACCENT_HOVER: Color32 = hex(0x6aa0ff);
pub const OK: Color32 = hex(0x2ecc9a);
pub const WARN: Color32 = hex(0xf5b335);
pub const CRIT: Color32 = hex(0xf0506e);
pub const RX: Color32 = ACCENT;
pub const TX: Color32 = hex(0xb07cff);

// Spacing (8px grid) and radii
pub const S1: f32 = 8.0;
pub const S2: f32 = 16.0;
pub const S3: f32 = 24.0;
pub const S4: f32 = 32.0;
pub const R_CARD: u8 = 12;
pub const R_PILL: u8 = 12;

// Type scale (px): label / body / hero, plus title
pub const F_LABEL: f32 = 11.0;
pub const F_BODY: f32 = 13.0;
pub const F_HERO: f32 = 30.0;
pub const F_TITLE: f32 = 22.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Ok,
    Warn,
    Crit,
}

impl Status {
    pub fn of(value: f32, warn: f32, crit: f32) -> Self {
        if value >= crit {
            Status::Crit
        } else if value >= warn {
            Status::Warn
        } else {
            Status::Ok
        }
    }

    pub fn color(self) -> Color32 {
        match self {
            Status::Ok => OK,
            Status::Warn => WARN,
            Status::Crit => CRIT,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Status::Ok => "Healthy",
            Status::Warn => "Warning",
            Status::Crit => "Critical",
        }
    }
}

pub fn fmt_rate(kbps: f32) -> String {
    if kbps >= 1024.0 {
        format!("{:.1} MB/s", kbps / 1024.0)
    } else {
        format!("{kbps:.1} KB/s")
    }
}

pub fn fmt_uptime(seconds: u64) -> String {
    let (d, rem) = (seconds / 86400, seconds % 86400);
    let (h, rem) = (rem / 3600, rem % 3600);
    let (m, s) = (rem / 60, rem % 60);
    if d > 0 {
        format!("{d}d {h:02}h {m:02}m")
    } else {
        format!("{h:02}h {m:02}m {s:02}s")
    }
}

pub const GIB: f64 = (1u64 << 30) as f64;
pub const MIB: f64 = (1u64 << 20) as f64;
