//! Main window: lays out the cards and applies sampled snapshots to them.

use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use egui::text::{LayoutJob, TextFormat};
use egui::{Align, CornerRadius, Layout, Painter, Rect, RichText, Sense, Stroke, UiBuilder, pos2, vec2};
use sysinfo::System;

use crate::metrics::{self, Snapshot, TOP_N};
use crate::opencode::{self, Summary};
use crate::theme::{self, GIB, MIB, Status, fmt_rate, fmt_uptime};
use crate::widgets::{self, LineChart, MetricCard, badge, card, card_title, font, mono, text_left, text_right};

/// Seconds of history summarised for the AI.
const AI_WINDOW: usize = 60;
const SIGNATURE: &str = "Mo Wael";
/// Below this size the dashboard scrolls instead of being squashed.
const MIN_CONTENT: egui::Vec2 = vec2(960.0, 820.0);
/// Metric cards need room for header, ring and a >=56px history strip.
const METRIC_ROW_MIN: f32 = 260.0;

const CORE_WARN: f32 = 70.0;
const CORE_CRIT: f32 = 90.0;

fn os_label() -> String {
    let name = System::name().unwrap_or_else(|| std::env::consts::OS.to_string());
    let release = if cfg!(target_os = "linux") {
        System::kernel_version()
    } else {
        System::os_version()
    };
    match release {
        Some(r) => format!("{name} {r}"),
        None => name,
    }
}

fn avg_peak(values: impl Iterator<Item = Option<f32>>) -> (String, String) {
    let v: Vec<f32> = values.flatten().collect();
    if v.is_empty() {
        return ("unknown".into(), "unknown".into());
    }
    let avg = v.iter().sum::<f32>() / v.len() as f32;
    let peak = v.iter().copied().fold(f32::MIN, f32::max);
    (format!("{avg:.1}"), format!("{peak:.1}"))
}

/// Condense recent snapshots into the summary opencode::build_prompt expects.
fn ai_summary(history: &VecDeque<Snapshot>, os: &str) -> Summary {
    let last = history.back().expect("history is not empty");
    let (cpu_avg, cpu_peak) = avg_peak(history.iter().map(|h| Some(h.cpu)));
    let (mem_avg, mem_peak) = avg_peak(history.iter().map(|h| Some(h.mem_pct)));
    let (temp_avg, temp_peak) = avg_peak(history.iter().map(|h| h.temp));
    // The first sample has no previous counters, so skip it for rates.
    let skip = usize::from(history.len() > 1);
    let (rx_avg, rx_peak) = avg_peak(history.iter().skip(skip).map(|h| Some(h.net_rx)));
    let (tx_avg, tx_peak) = avg_peak(history.iter().skip(skip).map(|h| Some(h.net_tx)));
    Summary {
        os: os.to_string(),
        window: history.len(),
        cores: last.per_core.len(),
        cpu_avg,
        cpu_peak,
        load: match last.load {
            Some((a, b, c)) => format!("{a:.2} {b:.2} {c:.2}"),
            None => "unavailable on this OS".into(),
        },
        mem_used: (last.mem_used as f64 / MIB).round() as u64,
        mem_total: (last.mem_total as f64 / MIB).round() as u64,
        mem_avg,
        mem_peak,
        disk_used: (last.disk_used as f64 / GIB).round() as u64,
        disk_total: (last.disk_total as f64 / GIB).round() as u64,
        disk_pct: last.disk_pct.round() as u64,
        temp_avg,
        temp_peak,
        rx_avg,
        rx_peak,
        tx_avg,
        tx_peak,
        top_procs: last.procs.iter().take(3).map(|p| (p.name.clone(), p.cpu)).collect(),
        uptime: last.uptime,
    }
}

pub struct Dashboard {
    rx: Receiver<Snapshot>,
    history: VecDeque<Snapshot>,
    subtitle: String,
    os_name: String,
    clock: String,
    overall: Option<(Status, String)>,

    cpu: MetricCard,
    mem: MetricCard,
    disk: MetricCard,
    temp: MetricCard,
    net: LineChart,

    ai_rx: Option<Receiver<String>>,
    ai_lines: Vec<String>,
    ai_placeholder: String,
    ai_stamp: String,
    ai_has_run: bool,

    /// Process details popup: the process, whether "End process" awaits
    /// confirmation, and the outcome of the last attempt.
    selected: Option<metrics::ProcDetails>,
    confirm_end: bool,
    proc_msg: Option<(bool, String)>,
    /// Screen rects of the process rows drawn last frame.
    proc_rows: Vec<Rect>,
}

impl Dashboard {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (tx, rx) = mpsc::channel();
        metrics::spawn_sampler(Duration::from_secs(1), tx, cc.egui_ctx.clone());
        Self::with_receiver(&cc.egui_ctx, rx)
    }

    /// Builds the dashboard around an existing snapshot source (tests feed their own).
    fn with_receiver(ctx: &egui::Context, rx: Receiver<Snapshot>) -> Self {
        apply_style(ctx);
        let host = System::host_name()
            .map(|h| h.trim().to_string())
            .filter(|h| !h.is_empty())
            .unwrap_or_else(|| "Unknown device".into());
        let os = os_label();
        Self {
            rx,
            history: VecDeque::with_capacity(AI_WINDOW),
            subtitle: format!("{host}  ·  {os}"),
            os_name: System::name().unwrap_or_else(|| std::env::consts::OS.to_string()),
            clock: String::new(),
            overall: None,
            cpu: MetricCard::new("CPU Usage", 70.0, 90.0, "%"),
            mem: MetricCard::new("Memory", 75.0, 90.0, "%"),
            disk: MetricCard::new("Disk", 80.0, 92.0, "%"),
            temp: MetricCard::new("Temperature", 70.0, 85.0, "°C"),
            net: LineChart::new(vec![theme::RX, theme::TX], None, Some(fmt_rate)),
            ai_rx: None,
            ai_lines: Vec::new(),
            ai_placeholder: "Ask opencode to review the last minute of readings.".into(),
            ai_stamp: String::new(),
            ai_has_run: false,
            selected: None,
            confirm_end: false,
            proc_msg: None,
            proc_rows: Vec::new(),
        }
    }

    fn apply_snapshot(&mut self, s: Snapshot) {
        let mut statuses: Vec<(&str, Status)> = Vec::new();
        let cores = s.per_core.len();

        statuses.push(("CPU", self.cpu.update(s.cpu, format!("{:.0}", s.cpu), format!("{cores} logical cores"))));
        statuses.push((
            "Memory",
            self.mem.update(
                s.mem_pct,
                format!("{:.0}", s.mem_pct),
                format!("{:.1} of {:.1} GB used", s.mem_used as f64 / GIB, s.mem_total as f64 / GIB),
            ),
        ));
        statuses.push((
            "Disk",
            self.disk.update(
                s.disk_pct,
                format!("{:.0}", s.disk_pct),
                format!("{:.0} of {:.0} GB used", s.disk_used as f64 / GIB, s.disk_total as f64 / GIB),
            ),
        ));
        match s.temp {
            Some(t) => statuses.push(("Temperature", self.temp.update(t, format!("{t:.0}"), "Hottest sensor".into()))),
            None => self.temp.set_unavailable("No thermal sensor found"),
        }
        let worst = s.per_core.iter().copied().fold(0.0, f32::max);
        let core_status = Status::of(worst, CORE_WARN, CORE_CRIT);
        if core_status != Status::Ok {
            statuses.push(("CPU cores", core_status));
        }

        self.net.push(&[s.net_rx, s.net_tx]);

        let names = |want: Status| -> Vec<&str> {
            statuses.iter().filter(|(_, st)| *st == want).map(|(k, _)| *k).collect()
        };
        let (crit, warn) = (names(Status::Crit), names(Status::Warn));
        self.overall = Some(if !crit.is_empty() {
            (Status::Crit, format!("Critical: {}", crit.join(", ")))
        } else if !warn.is_empty() {
            (Status::Warn, format!("Warning: {}", warn.join(", ")))
        } else {
            (Status::Ok, "All systems healthy".into())
        });
        self.clock = chrono::Local::now().format("%a %d %b  %H:%M:%S").to_string();

        if self.history.len() == AI_WINDOW {
            self.history.pop_front();
        }
        self.history.push_back(s);
    }

    // --- AI summary ---------------------------------------------------------
    fn run_ai(&mut self, ctx: &egui::Context) {
        if self.history.is_empty() || self.ai_rx.is_some() {
            return;
        }
        let summary = ai_summary(&self.history, &self.os_name);
        self.ai_stamp = format!(
            "as of {} · {}s avg",
            chrono::Local::now().format("%H:%M"),
            summary.window
        );
        self.ai_placeholder = format!("Asking opencode about the last {}s of readings…", summary.window);
        self.ai_lines.clear();
        let (tx, rx) = mpsc::channel();
        self.ai_rx = Some(rx);
        let ctx = ctx.clone();
        thread::spawn(move || {
            let _ = tx.send(opencode::analyze(&summary));
            ctx.request_repaint();
        });
    }

    fn poll_ai(&mut self) {
        let Some(rx) = &self.ai_rx else { return };
        if let Ok(text) = rx.try_recv() {
            self.ai_lines = text
                .lines()
                .map(|l| l.trim().trim_start_matches(['-', '*', '•']).trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
            self.ai_rx = None;
            self.ai_has_run = true;
        }
    }

    // --- Process details ----------------------------------------------------
    fn open_process(&mut self, pid: u32) {
        self.confirm_end = false;
        self.proc_msg = None;
        self.selected = metrics::proc_details(pid);
        if self.selected.is_none() {
            self.proc_msg = Some((false, format!("Process {pid} has already exited.")));
        }
    }

    fn process_window(&mut self, ctx: &egui::Context) {
        let Some(d) = self.selected.clone() else { return };
        let mut open = true;
        let live = self
            .history
            .back()
            .and_then(|s| s.procs.iter().find(|p| p.pid == d.pid))
            .map(|p| (p.cpu, p.mem));
        let mut end_clicked = false;
        let mut confirm_clicked = false;
        let mut cancel_clicked = false;
        egui::Window::new(format!("{}  (PID {})", d.name, d.pid))
            .id(egui::Id::unique("process_details"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(460.0)
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .frame(
                egui::Frame::window(&ctx.global_style())
                    .fill(theme::SURFACE)
                    .stroke(Stroke::new(1.0, theme::BORDER))
                    .corner_radius(CornerRadius::same(theme::R_CARD))
                    .inner_margin(theme::S2 as i8),
            )
            .show(ctx, |ui| {
                ui.set_max_width(460.0);
                let row = |ui: &mut egui::Ui, key: &str, val: String| {
                    ui.label(RichText::new(key).color(theme::MUTED).size(theme::F_BODY));
                    ui.add(egui::Label::new(RichText::new(val).color(theme::TEXT).size(theme::F_BODY)).wrap().selectable(true));
                    ui.end_row();
                };
                egui::Grid::new("proc_grid").num_columns(2).spacing(vec2(theme::S2, 6.0)).show(ui, |ui| {
                    row(ui, "Name", d.name.clone());
                    row(ui, "PID", d.pid.to_string());
                    row(ui, "Parent PID", d.parent.map_or("--".into(), |p| p.to_string()));
                    row(ui, "Status", d.status.clone());
                    match live {
                        Some((cpu, mem)) => {
                            row(ui, "CPU", format!("{cpu:.1}% of one core"));
                            row(ui, "Memory", format!("{:.1} MB ({mem:.1}%)", d.memory as f64 / MIB));
                        }
                        None => row(ui, "Memory", format!("{:.1} MB", d.memory as f64 / MIB)),
                    }
                    row(ui, "Running for", fmt_uptime(d.run_time));
                    row(ui, "Executable", d.exe.clone());
                    row(ui, "Command", d.cmd.clone());
                });
                ui.add_space(theme::S2);
                let is_self = d.pid == std::process::id();
                ui.horizontal(|ui| {
                    let btn = |text: &str, fill| {
                        egui::Button::new(RichText::new(text).color(theme::BG).size(theme::F_BODY).strong())
                            .fill(fill)
                            .corner_radius(CornerRadius::same(6))
                            .min_size(vec2(120.0, 28.0))
                    };
                    if self.confirm_end {
                        ui.label(RichText::new(format!("End {}?", d.name)).color(theme::WARN));
                        confirm_clicked = ui.add(btn("Yes, end it", theme::CRIT)).clicked();
                        cancel_clicked = ui.add(btn("Cancel", theme::MUTED)).clicked();
                    } else {
                        end_clicked = ui
                            .add_enabled(!is_self, btn("End process", theme::CRIT))
                            .on_disabled_hover_text("This is the dashboard itself")
                            .clicked();
                    }
                });
                if let Some((ok, msg)) = &self.proc_msg {
                    ui.add_space(theme::S1);
                    ui.label(RichText::new(msg).color(if *ok { theme::OK } else { theme::CRIT }));
                }
            });
        if end_clicked {
            self.confirm_end = true;
        }
        if cancel_clicked {
            self.confirm_end = false;
        }
        if confirm_clicked {
            self.confirm_end = false;
            self.proc_msg = Some(match metrics::end_process(d.pid) {
                Ok(()) => (true, format!("Sent the end signal to {} (PID {}).", d.name, d.pid)),
                Err(e) => (false, e),
            });
        }
        if !open {
            self.selected = None;
        }
    }

    // --- Painting -------------------------------------------------------------
    fn header(&self, p: &Painter, r: Rect) {
        p.text(r.left_top(), egui::Align2::LEFT_TOP, "Device Health Dashboard", font(theme::F_TITLE), theme::TEXT);
        p.text(
            pos2(r.left(), r.top() + theme::F_TITLE + 8.0),
            egui::Align2::LEFT_TOP,
            &self.subtitle,
            font(theme::F_BODY),
            theme::MUTED,
        );
        let cy = r.center().y;
        let clock_w = text_right(p, r.right(), cy, &self.clock, font(theme::F_BODY), theme::MUTED);
        if let Some((status, text)) = &self.overall {
            badge(p, r.right() - clock_w - theme::S2, cy, status.color(), text);
        }
    }

    fn per_core(&self, p: &Painter, outer: Rect) {
        card(p, outer);
        let r = outer.shrink2(vec2(theme::S3 - 4.0, theme::S2));
        let p = p.with_clip_rect(outer);
        card_title(&p, r, "Per-core CPU");
        let values = self.history.back().map(|s| s.per_core.as_slice()).unwrap_or(&[]);
        if values.is_empty() {
            return;
        }
        let worst = values.iter().copied().fold(0.0, f32::max);
        let status = Status::of(worst, CORE_WARN, CORE_CRIT);
        if status != Status::Ok {
            badge(&p, r.right(), r.top() + 12.0, status.color(), status.label());
        }

        let n = values.len();
        let cols = if n <= 16 { 2 } else { 4 };
        let rows = n.div_ceil(cols);
        let top = r.top() + 24.0 + theme::S2;
        let pitch = ((r.bottom() - top) / rows as f32).min(24.0);
        let col_w = (r.width() - (cols - 1) as f32 * theme::S3) / cols as f32;
        for (i, v) in values.iter().enumerate() {
            let (row, col) = (i / cols, i % cols);
            let x = r.left() + col as f32 * (col_w + theme::S3);
            let cy = top + row as f32 * pitch + pitch / 2.0;
            let color = Status::of(*v, CORE_WARN, CORE_CRIT).color();
            text_left(&p, x, cy, &format!("cpu{i}"), mono(theme::F_LABEL), theme::MUTED, 44.0);
            let bar = Rect::from_x_y_ranges(x + 44.0 + theme::S1..=x + col_w - 40.0 - theme::S1, cy - 4.0..=cy + 4.0);
            widgets::hbar(&p, bar, *v, color);
            text_right(&p, x + col_w, cy, &format!("{v:.0}%"), font(theme::F_LABEL), color);
        }
    }

    /// Returns the PID of a clicked row.
    fn processes(&mut self, ui: &egui::Ui, outer: Rect) -> Option<u32> {
        self.proc_rows.clear();
        let p = ui.painter();
        card(p, outer);
        let r = outer.shrink2(vec2(theme::S3 - 4.0, theme::S2));
        let p = p.with_clip_rect(outer.shrink(1.0));
        card_title(&p, r, "Top processes");
        text_right(&p, r.right(), r.top() + 12.0, &format!("top {TOP_N} by CPU · click a row for details"), font(theme::F_LABEL), theme::MUTED);

        let col = 64.0;
        let right_edges = [r.right() - 2.0 * col, r.right() - col, r.right()];
        let pad = 6.0;
        let head_top = r.top() + 24.0 + theme::S2;
        let head_cy = head_top + 12.0;
        text_left(&p, r.left() + pad, head_cy, "PROCESS", font(theme::F_LABEL), theme::MUTED, 200.0);
        for (edge, h) in right_edges.iter().zip(["PID", "CPU", "MEM"]) {
            text_right(&p, edge - pad, head_cy, h, font(theme::F_LABEL), theme::MUTED);
        }
        let line_y = head_top + 24.0 - 0.5;
        p.line_segment([pos2(r.left(), line_y), pos2(r.right(), line_y)], Stroke::new(1.0, theme::BORDER));

        let procs = self.history.back().map(|s| s.procs.as_slice()).unwrap_or(&[]);
        let mut clicked = None;
        let name_w = right_edges[0] - col - r.left() - 2.0 * pad;
        let rows_top = head_top + 24.0;
        let pitch = ((outer.bottom() - 4.0 - rows_top) / TOP_N as f32).clamp(18.0, 24.0);
        for i in 0..TOP_N {
            let top = rows_top + i as f32 * pitch;
            let row = Rect::from_x_y_ranges(r.x_range(), top..=top + pitch);
            if i % 2 == 1 {
                p.rect_filled(row, 0.0, theme::ROW);
            }
            let Some(pr) = procs.get(i) else { continue };
            self.proc_rows.push(row);
            let resp = ui
                .interact(row.intersect(outer.shrink(1.0)), egui::Id::unique(("proc_row", i)), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Click for details");
            if resp.hovered() {
                p.rect_filled(row, 0.0, theme::BORDER);
            }
            if resp.clicked() {
                clicked = Some(pr.pid);
            }
            let cy = row.center().y;
            text_left(&p, r.left() + pad, cy, &pr.name, font(theme::F_BODY), theme::TEXT, name_w);
            let cpu_color = Status::of(pr.cpu, CORE_WARN, CORE_CRIT).color();
            let cells = [
                (pr.pid.to_string(), theme::TEXT),
                (format!("{:.1}%", pr.cpu), cpu_color),
                (format!("{:.1}%", pr.mem), theme::TEXT),
            ];
            for (edge, (text, color)) in right_edges.iter().zip(cells.iter()) {
                text_right(&p, edge - pad, cy, text, font(theme::F_BODY), *color);
            }
        }
        clicked
    }

    fn network(&self, p: &Painter, outer: Rect) {
        card(p, outer);
        let r = outer.shrink2(vec2(theme::S3 - 4.0, theme::S2));
        let p = p.with_clip_rect(outer);
        card_title(&p, r, "Network throughput");
        let last = self.history.back();
        let (rx, tx) = last.map_or((0.0, 0.0), |s| (s.net_rx, s.net_tx));
        if last.is_some() {
            let cy = r.top() + 12.0;
            text_right(&p, r.right(), cy, &format!("↑ {}", fmt_rate(tx)), font(theme::F_BODY), theme::TX);
            text_right(&p, r.right() - 110.0, cy, &format!("↓ {}", fmt_rate(rx)), font(theme::F_BODY), theme::RX);
        }
        let chart_top = r.top() + 24.0 + theme::S1;
        let chart_bottom = r.bottom() - 14.0 - theme::S1;
        self.net.paint(&p, Rect::from_x_y_ranges(r.x_range(), chart_top..=chart_bottom));
        widgets::axis_row(&p, r, chart_bottom + theme::S1);
    }

    fn system(&self, p: &Painter, outer: Rect) {
        card(p, outer);
        let r = outer.shrink2(vec2(theme::S3 - 4.0, theme::S2));
        let p = p.with_clip_rect(outer);
        card_title(&p, r, "System");
        let s = self.history.back();
        let dash = || "--".to_string();
        let rows = [
            ("Uptime", s.map_or_else(dash, |s| fmt_uptime(s.uptime))),
            (
                "Load average",
                s.map_or_else(dash, |s| match s.load {
                    Some((a, b, c)) => format!("{a:.2}  {b:.2}  {c:.2}"),
                    None => "N/A on this OS".into(),
                }),
            ),
            ("CPU cores", s.map_or_else(dash, |s| s.per_core.len().to_string())),
            ("Memory", s.map_or_else(dash, |s| format!("{:.1} GB total", s.mem_total as f64 / GIB))),
            ("Disk", s.map_or_else(dash, |s| format!("{:.0} GB total", s.disk_total as f64 / GIB))),
        ];
        let top = r.top() + 24.0 + theme::S1;
        let pitch = ((r.bottom() - top) / rows.len() as f32).min(28.0);
        for (i, (key, val)) in rows.iter().enumerate() {
            let cy = top + pitch / 2.0 + i as f32 * pitch;
            text_left(&p, r.left(), cy, &format!("{key}:"), font(theme::F_BODY), theme::MUTED, r.width());
            text_right(&p, r.right(), cy, val, font(theme::F_BODY), theme::TEXT);
        }
    }

    fn ai_card(&mut self, ui: &mut egui::Ui, outer: Rect) {
        card(ui.painter(), outer);
        let r = outer.shrink2(vec2(theme::S3 - 4.0, theme::S2));
        {
            let p = ui.painter().with_clip_rect(outer);
            card_title(&p, r, "AI health summary");
            text_right(&p, r.right(), r.top() + 12.0, &self.ai_stamp, font(theme::F_LABEL), theme::MUTED);
        }
        let busy = self.ai_rx.is_some();
        let body = Rect::from_x_y_ranges(r.x_range(), r.top() + 24.0 + theme::S1..=r.bottom());
        let mut clicked = false;
        ui.scope_builder(UiBuilder::new().max_rect(body).layout(Layout::top_down(Align::Min)), |ui| {
            let button_h = 30.0;
            let text_h = (body.height() - button_h - theme::S1).max(20.0);
            egui::ScrollArea::vertical()
                .max_height(text_h)
                .min_scrolled_height(text_h)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    if busy || self.ai_lines.is_empty() {
                        ui.add(
                            egui::Label::new(RichText::new(&self.ai_placeholder).color(theme::MUTED).size(theme::F_BODY))
                                .wrap(),
                        );
                    } else {
                        for line in &self.ai_lines {
                            ui.add(
                                egui::Label::new(RichText::new(format!("•  {line}")).color(theme::TEXT).size(theme::F_BODY))
                                    .wrap()
                                    .selectable(true),
                            );
                        }
                    }
                });
            ui.add_space(theme::S1);
            let label = if busy {
                "Analyzing…"
            } else if self.ai_has_run {
                "Analyze again"
            } else {
                "Analyze with opencode"
            };
            let (fill, fg) = if busy { (theme::TRACK, theme::MUTED) } else { (theme::ACCENT, theme::BG) };
            let button = egui::Button::new(RichText::new(label).color(fg).size(theme::F_BODY).strong())
                .fill(fill)
                .stroke(Stroke::NONE)
                .corner_radius(CornerRadius::same(6))
                .min_size(vec2(180.0, button_h));
            let resp = ui.add_enabled(!busy && !self.history.is_empty(), button);
            if resp.hovered() && !busy {
                // Hover tint, like QPushButton:hover.
                ui.painter().rect_filled(resp.rect, CornerRadius::same(6), theme::ACCENT_HOVER);
                ui.painter().text(resp.rect.center(), egui::Align2::CENTER_CENTER, label, font(theme::F_BODY), fg);
            }
            clicked = resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked();
        });
        if clicked {
            self.run_ai(ui.ctx());
        }
    }

    fn signature(&self, p: &Painter, r: Rect) {
        let mut job = LayoutJob::default();
        job.append(
            SIGNATURE,
            0.0,
            TextFormat { font_id: font(theme::F_BODY), color: theme::MUTED, italics: true, ..Default::default() },
        );
        let g = p.layout_job(job);
        let size = g.size();
        let c = r.center();
        p.galley(pos2(c.x - size.x / 2.0, c.y - size.y / 2.0), g, theme::MUTED);
        // Short rules on either side of the name.
        let gap = size.x / 2.0 + 10.0;
        for dir in [-1.0, 1.0] {
            let a = pos2(c.x + dir * gap, c.y + 0.5);
            let b = pos2(c.x + dir * (gap + 40.0), c.y + 0.5);
            p.line_segment([a, b], Stroke::new(1.0, theme::BORDER));
        }
    }

    fn layout(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let inner = rect.shrink2(vec2(theme::S4 - 4.0, theme::S3));
        let gap_v = theme::S3 - 4.0;
        let gap = theme::S2;

        let header = Rect::from_min_size(inner.min, vec2(inner.width(), 52.0));
        let footer = Rect::from_min_max(pos2(inner.left(), inner.bottom() - 20.0), inner.max);
        let rows_top = header.bottom() + gap_v;
        let rows_h = footer.top() - theme::S1 - rows_top - 2.0 * gap_v;
        // Row stretch 4 : 4 : 3, but never squash the metric cards.
        let row1_h = (rows_h * 4.0 / 11.0).max(METRIC_ROW_MIN);
        let unit = (rows_h - row1_h) / 7.0;
        let row1 = Rect::from_x_y_ranges(inner.x_range(), rows_top..=rows_top + row1_h);
        let row2 = Rect::from_x_y_ranges(inner.x_range(), row1.bottom() + gap_v..=row1.bottom() + gap_v + 4.0 * unit);
        let row3 = Rect::from_x_y_ranges(inner.x_range(), row2.bottom() + gap_v..=row2.bottom() + gap_v + 3.0 * unit);

        let split = |row: Rect, weights: &[f32]| -> Vec<Rect> {
            let total: f32 = weights.iter().sum();
            let avail = row.width() - gap * (weights.len() - 1) as f32;
            let mut x = row.left();
            weights
                .iter()
                .map(|w| {
                    let width = avail * w / total;
                    let r = Rect::from_x_y_ranges(x..=x + width, row.y_range());
                    x += width + gap;
                    r
                })
                .collect()
        };

        let p = ui.painter().clone();
        self.header(&p, header);
        let r1 = split(row1, &[1.0, 1.0, 1.0, 1.0]);
        for (card, rect) in [&self.cpu, &self.mem, &self.disk, &self.temp].into_iter().zip(&r1) {
            card.paint(&p, *rect);
        }
        let r2 = split(row2, &[1.0, 1.0]);
        self.per_core(&p, r2[0]);
        if let Some(pid) = self.processes(ui, r2[1]) {
            self.open_process(pid);
        }
        let r3 = split(row3, &[3.0, 2.0, 3.0]);
        self.network(&p, r3[0]);
        self.system(&p, r3[1]);
        self.ai_card(ui, r3[2]);
        self.signature(&p, footer);
    }
}

fn apply_style(ctx: &egui::Context) {
    // Always dark, whatever the OS theme is.
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.all_styles_mut(|s| {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = theme::BG;
        visuals.window_fill = theme::SURFACE;
        visuals.selection.bg_fill = theme::ACCENT.gamma_multiply(0.5);
        s.visuals = visuals;
        s.spacing.scroll.bar_width = 8.0;
        s.spacing.scroll.floating = true;
    });
}

impl eframe::App for Dashboard {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}

impl Dashboard {
    fn show(&mut self, ui: &mut egui::Ui) {
        while let Ok(s) = self.rx.try_recv() {
            self.apply_snapshot(s);
        }
        self.poll_ai();

        let viewport = ui.max_rect();
        ui.painter().rect_filled(viewport, 0.0, theme::BG);
        let size = viewport.size().max(MIN_CONTENT);
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            self.layout(ui, rect);
        });
        self.process_window(&ui.ctx().clone());
        // Keep the clock ticking even if a sample is late.
        ui.ctx().request_repaint_after(Duration::from_secs(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::ProcInfo;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    fn click(h: &Harness<'_, Dashboard>, pos: egui::Pos2) {
        h.event(egui::Event::PointerMoved(pos));
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            });
        }
    }

    #[test]
    fn clicking_a_process_row_opens_details() {
        let mut child = std::process::Command::new(if cfg!(windows) { "ping" } else { "sleep" })
            .args(if cfg!(windows) { vec!["-n", "60", "127.0.0.1"] } else { vec!["60"] })
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let (tx, rx) = mpsc::channel();
        tx.send(Snapshot {
            per_core: vec![10.0; 4],
            mem_total: 1 << 30,
            procs: vec![ProcInfo { pid: child.id(), name: "sleep".into(), cpu: 1.0, mem: 0.1 }],
            ..Default::default()
        })
        .unwrap();
        let ctx = egui::Context::default();
        let app = Dashboard::with_receiver(&ctx, rx);
        let mut h = Harness::builder()
            .with_size(vec2(1280.0, 860.0))
            // The app gets "●", "↑" etc. from system fonts; the test context has none.
            .allow_missing_glyphs()
            .build_ui_state(|ui, app: &mut Dashboard| app.show(ui), app);
        h.run_steps(3);

        let row = *h.state().proc_rows.first().expect("process row is drawn");
        click(&h, row.center());
        h.run_steps(3);
        let selected = h.state().selected.as_ref().map(|d| d.pid);
        assert_eq!(selected, Some(child.id()), "click should open the details popup");
        // The popup's End process button is a real widget.
        h.get_by_label("End process").click();
        h.run_steps(2);
        assert!(h.state().confirm_end, "End process should ask for confirmation");
        h.get_by_label("Yes, end it").click();
        h.run_steps(2);
        assert!(child.wait().is_ok());
        assert!(matches!(h.state().proc_msg, Some((true, _))), "{:?}", h.state().proc_msg);
    }
}
