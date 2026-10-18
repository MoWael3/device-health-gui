//! Device Health Dashboard - a cross-platform system monitor (eframe/egui + sysinfo).

// No console window behind the GUI on Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod dashboard;
mod metrics;
mod opencode;
mod theme;
mod widgets;

const ICON_PNG: &[u8] = include_bytes!("../assets/icon.png");

fn main() -> eframe::Result {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Device Health Dashboard")
        .with_app_id("device-health-dashboard")
        .with_inner_size([1280.0, 860.0])
        .with_min_inner_size([480.0, 360.0]);
    if let Ok(icon) = eframe::icon_data::from_png_bytes(ICON_PNG) {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(
        "Device Health Dashboard",
        options,
        Box::new(|cc| Ok(Box::new(dashboard::Dashboard::new(cc)))),
    )
}
