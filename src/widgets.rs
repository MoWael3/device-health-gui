//! Custom-painted widgets for the dashboard (mirrors widgets.py).

use std::collections::VecDeque;
use std::f32::consts::PI;

use egui::epaint::{Mesh, PathShape, PathStroke};
use egui::text::{LayoutJob, TextWrapping};
use egui::{Align2, Color32, CornerRadius, FontId, Galley, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2, pos2, vec2};
use std::sync::Arc;

use crate::theme::{self, HISTORY, Status};

pub fn font(size: f32) -> FontId {
    FontId::proportional(size)
}

pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

/// Single-line galley, elided with "…" when wider than `max_width`.
pub fn galley(painter: &Painter, text: &str, font: FontId, color: Color32, max_width: f32) -> Arc<Galley> {
    let mut job = LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = TextWrapping::truncate_at_width(max_width.max(1.0));
    painter.layout_job(job)
}

/// Draws text with its left edge at `x`, vertically centred on `cy`. Returns its width.
pub fn text_left(p: &Painter, x: f32, cy: f32, text: &str, font: FontId, color: Color32, max_w: f32) -> f32 {
    let g = galley(p, text, font, color, max_w);
    let w = g.size().x;
    let h = g.size().y;
    p.galley(pos2(x, cy - h / 2.0), g, color);
    w
}

/// Draws text with its right edge at `x`, vertically centred on `cy`. Returns its width.
pub fn text_right(p: &Painter, x: f32, cy: f32, text: &str, font: FontId, color: Color32) -> f32 {
    p.text(pos2(x, cy), Align2::RIGHT_CENTER, text, font, color).width()
}

/// Card background: surface fill, 1px border, rounded corners.
pub fn card(p: &Painter, rect: Rect) {
    p.rect(
        rect,
        CornerRadius::same(theme::R_CARD),
        theme::SURFACE,
        Stroke::new(1.0, theme::BORDER),
        StrokeKind::Inside,
    );
}

/// Muted, upper-case card title at the top-left of `rect`, centred in a 24px row.
pub fn card_title(p: &Painter, rect: Rect, title: &str) {
    text_left(p, rect.left(), rect.top() + 12.0, &title.to_uppercase(), font(theme::F_LABEL), theme::MUTED, rect.width());
}

/// Rounded status pill. `right` is its right edge, `cy` its vertical centre. Returns its width.
pub fn badge(p: &Painter, right: f32, cy: f32, color: Color32, text: &str) -> f32 {
    let g = p.layout_no_wrap(format!("●  {text}"), font(theme::F_LABEL), color);
    let w = g.size().x + 24.0;
    let rect = Rect::from_min_size(pos2(right - w, cy - 12.0), vec2(w, 24.0));
    let bg = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 36);
    p.rect_filled(rect, CornerRadius::same(theme::R_PILL), bg);
    let gs = g.size();
    p.galley(pos2(rect.left() + 12.0, cy - gs.y / 2.0), g, color);
    w
}

/// "60s ago ... now" axis caption under a chart, in a 14px row starting at `top`.
pub fn axis_row(p: &Painter, rect: Rect, top: f32) {
    let cy = top + 7.0;
    text_left(p, rect.left(), cy, "60s ago", font(theme::F_LABEL), theme::MUTED, rect.width());
    text_right(p, rect.right(), cy, "now", font(theme::F_LABEL), theme::MUTED);
}

fn arc_points(center: Pos2, radius: f32, start_deg: f32, sweep_deg: f32) -> Vec<Pos2> {
    let steps = ((sweep_deg.abs() / 3.0).ceil() as usize).max(2);
    (0..=steps)
        .map(|i| {
            // Qt-style angles: 0° = 3 o'clock, counter-clockwise positive; y grows downwards.
            let a = (start_deg + sweep_deg * i as f32 / steps as f32) * PI / 180.0;
            pos2(center.x + radius * a.cos(), center.y - radius * a.sin())
        })
        .collect()
}

fn round_arc(p: &Painter, center: Pos2, radius: f32, start: f32, sweep: f32, thick: f32, color: Color32) {
    let pts = arc_points(center, radius, start, sweep);
    let (first, last) = (pts[0], *pts.last().unwrap());
    p.add(Shape::Path(PathShape::line(pts, PathStroke::new(thick, color))));
    // Round caps
    p.circle_filled(first, thick / 2.0, color);
    p.circle_filled(last, thick / 2.0, color);
}

/// 270° ring gauge (opening at the bottom) filled to `value` percent.
pub fn ring_gauge(p: &Painter, rect: Rect, value: f32, color: Color32) {
    let thick = 9.0;
    let radius = rect.width().min(rect.height()) / 2.0 - thick / 2.0 - 1.0;
    let c = rect.center();
    round_arc(p, c, radius, 225.0, -270.0, thick, theme::TRACK);
    let sweep = -270.0 * value.clamp(0.0, 100.0) / 100.0;
    if sweep.abs() > 0.5 {
        round_arc(p, c, radius, 225.0, sweep, thick, color);
    }
}

/// Compact horizontal bar used by the per-core panel.
pub fn hbar(p: &Painter, rect: Rect, value: f32, color: Color32) {
    let r = rect.shrink(1.0);
    let radius = CornerRadius::same((r.height() / 2.0) as u8);
    p.rect_filled(r, radius, theme::TRACK);
    let w = r.width() * value.clamp(0.0, 100.0) / 100.0;
    if w > 0.5 {
        p.rect_filled(Rect::from_min_size(r.min, vec2(w, r.height())), radius, color);
    }
}

/// Filled line chart. History starts empty and fills in from the right.
pub struct LineChart {
    pub colors: Vec<Color32>,
    data: Vec<VecDeque<f32>>,
    fixed_max: Option<f32>,
    ymax: f32,
    /// Labels the top and middle gridlines when given.
    scale_fmt: Option<fn(f32) -> String>,
}

impl LineChart {
    pub fn new(colors: Vec<Color32>, fixed_max: Option<f32>, scale_fmt: Option<fn(f32) -> String>) -> Self {
        let data = colors.iter().map(|_| VecDeque::with_capacity(HISTORY)).collect();
        Self { colors, data, fixed_max, ymax: fixed_max.unwrap_or(1.0), scale_fmt }
    }

    pub fn push(&mut self, values: &[f32]) {
        for (d, v) in self.data.iter_mut().zip(values) {
            if d.len() == HISTORY {
                d.pop_front();
            }
            d.push_back(*v);
        }
        if self.fixed_max.is_none() {
            // Grow instantly, shrink slowly so the scale doesn't jump around.
            let peak = self.data.iter().flatten().copied().fold(0.0, f32::max) * 1.15;
            self.ymax = peak.max(self.ymax * 0.97).max(1.0);
        }
    }

    pub fn paint(&self, p: &Painter, rect: Rect) {
        let p = p.with_clip_rect(rect.expand(1.0));
        let (w, h) = (rect.width(), rect.height());
        let at = |x: f32, y: f32| pos2(rect.left() + x, rect.top() + y);

        for frac in [0.0, 0.5, 1.0] {
            let y = (h * frac).clamp(0.0, h - 1.0) + 0.5;
            p.line_segment([at(0.0, y), at(w, y)], Stroke::new(1.0, theme::GRID));
        }

        let step = w / (HISTORY - 1) as f32;
        for (color, d) in self.colors.iter().zip(&self.data) {
            if d.len() < 2 {
                continue;
            }
            let n = d.len();
            let pts: Vec<Pos2> = d
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    let x = w - (n - 1 - i) as f32 * step;
                    let y = h - 2.0 - (h - 4.0) * v.min(self.ymax) / self.ymax;
                    at(x, y)
                })
                .collect();

            // Gradient fill: alpha 70 at the top of the chart fading to 0 at the bottom.
            let fill = |y: f32| {
                let a = 70.0 * (1.0 - (y - rect.top()) / h).clamp(0.0, 1.0);
                Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), a as u8)
            };
            let mut mesh = Mesh::default();
            let transparent = fill(rect.bottom());
            for pair in pts.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                let base = mesh.vertices.len() as u32;
                mesh.colored_vertex(a, fill(a.y));
                mesh.colored_vertex(b, fill(b.y));
                mesh.colored_vertex(pos2(b.x, rect.bottom()), transparent);
                mesh.colored_vertex(pos2(a.x, rect.bottom()), transparent);
                mesh.add_triangle(base, base + 1, base + 2);
                mesh.add_triangle(base, base + 2, base + 3);
            }
            p.add(Shape::mesh(mesh));
            p.add(Shape::Path(PathShape::line(pts, PathStroke::new(1.8, *color))));
        }

        if let Some(fmt) = self.scale_fmt {
            let f = font(theme::F_LABEL - 1.0);
            for frac in [0.0, 0.5] {
                let g = p.layout_no_wrap(fmt(self.ymax * (1.0 - frac)), f.clone(), theme::MUTED);
                let size = g.size() + vec2(6.0, 0.0);
                let bx = Rect::from_min_size(at(0.0, h * frac + 2.0), size);
                p.rect_filled(bx, 0.0, theme::SURFACE); // keep labels readable over lines
                p.galley(bx.min + vec2(3.0, 0.0), g, theme::MUTED);
            }
        }
    }
}

/// Ring + hero value + detail + 60s history strip for one metric.
pub struct MetricCard {
    pub title: &'static str,
    warn: f32,
    crit: f32,
    unit: &'static str,
    value: f32,
    color: Color32,
    hero: String,
    detail: String,
    status: Option<Status>,
    pub spark: LineChart,
}

fn pct_label(v: f32) -> String {
    format!("{v:.0}%")
}

impl MetricCard {
    pub fn new(title: &'static str, warn: f32, crit: f32, unit: &'static str) -> Self {
        let scale: Option<fn(f32) -> String> = if unit == "%" { Some(pct_label) } else { None };
        Self {
            title,
            warn,
            crit,
            unit,
            value: 0.0,
            color: theme::MUTED,
            hero: "--".into(),
            detail: String::new(),
            status: None,
            spark: LineChart::new(vec![theme::ACCENT], Some(100.0), scale),
        }
    }

    pub fn update(&mut self, percent: f32, hero: String, detail: String) -> Status {
        let status = Status::of(percent, self.warn, self.crit);
        self.value = percent;
        self.color = status.color();
        self.hero = hero;
        self.detail = detail;
        self.status = Some(status);
        self.spark.colors[0] = self.color;
        self.spark.push(&[percent]);
        status
    }

    /// No reading available (e.g. no temperature sensor).
    pub fn set_unavailable(&mut self, detail: &str) {
        self.value = 0.0;
        self.color = theme::MUTED;
        self.hero = "N/A".into();
        self.detail = detail.into();
        self.status = None;
        self.spark.colors[0] = theme::MUTED;
    }

    pub fn paint(&self, p: &Painter, outer: Rect) {
        card(p, outer);
        let r = outer.shrink2(vec2(theme::S3 - 4.0, theme::S2));
        let p = p.with_clip_rect(outer);
        card_title(&p, r, self.title);
        if let Some(s) = self.status.filter(|s| *s != Status::Ok) {
            badge(&p, r.right(), r.top() + 12.0, s.color(), s.label());
        }

        // Body: ring at left, hero + detail at right.
        let body_top = r.top() + 24.0 + theme::S2;
        let ring = Rect::from_min_size(pos2(r.left(), body_top), Vec2::splat(88.0));
        ring_gauge(&p, ring, self.value, self.color);

        let tx = ring.right() + theme::S2;
        let tw = (r.right() - tx).max(10.0);
        let hero = p.layout_no_wrap(self.hero.clone(), font(theme::F_HERO), theme::TEXT);
        let unit = if self.hero == "N/A" {
            None
        } else {
            Some(p.layout_no_wrap(format!(" {}", self.unit), font(theme::F_BODY + 2.0), theme::MUTED))
        };
        let detail = p.layout(self.detail.clone(), font(theme::F_BODY), theme::MUTED, tw);
        let block_h = hero.size().y + 2.0 + detail.size().y;
        let hy = ring.center().y - block_h / 2.0;
        let hero_size = hero.size();
        p.galley(pos2(tx, hy), hero, theme::TEXT);
        if let Some(u) = unit {
            let us = u.size();
            p.galley(pos2(tx + hero_size.x, hy + hero_size.y - us.y - 4.0), u, theme::MUTED);
        }
        p.galley(pos2(tx, hy + hero_size.y + 2.0), detail, theme::MUTED);

        // History strip + axis caption.
        let spark_top = ring.bottom() + theme::S2;
        let spark_bottom = (r.bottom() - 14.0 - theme::S1).max(spark_top + 20.0);
        self.spark.paint(&p, Rect::from_x_y_ranges(r.x_range(), spark_top..=spark_bottom));
        axis_row(&p, r, spark_bottom + theme::S1);
    }
}
