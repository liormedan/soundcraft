//! The Trimio effect shell shared by every Trimio-styled plugin editor: palette, header (power,
//! title, preset, A/B, undo/redo), knobs, cards, and the parameter writes that merge one gesture
//! into one undo step. Effects draw only their core panel on top of this.

use crate::SoundApp;
use crate::theme::{bold, regular};
use crate::trimio_tokens::{HEADER_HEIGHT, TrimioPalette, radius, space};
use egui::{Color32, CornerRadius, CursorIcon, Pos2, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::{Map, Value, json};
use soundcraft_dsp::{ParamInfo, PluginInfo, Taper};
use soundcraft_model::{Insert, TrackId};

pub(crate) fn palette() -> TrimioPalette {
    if crate::theme::is_light() { TrimioPalette::LIGHT } else { TrimioPalette::DARK }
}

/// The window frame the editor sits in.
pub fn frame(style: &egui::Style) -> egui::Frame {
    let p = palette();
    egui::Frame::window(style).fill(p.bg).stroke(Stroke::new(1.0, p.border)).corner_radius(CornerRadius::same(radius::XL as u8))
}

pub(crate) struct Ctx<'a> {
    pub info: &'static PluginInfo,
    pub ins: &'a Insert,
    pub tid: TrackId,
    pub slot: usize,
}

impl Ctx<'_> {
    pub fn param(&self, id: &str) -> Option<&'static ParamInfo> {
        self.info.params.iter().find(|p| p.id == id)
    }
    pub fn value(&self, id: &str) -> f32 {
        let def = self.param(id).map_or(0.0, |p| p.default);
        self.ins.params.get(id).copied().unwrap_or(def)
    }
}

/// Sets parameters of one band. One drag gesture merges into one undo step (see [`end_gesture`]).
pub(crate) fn set(app: &mut SoundApp, c: &Ctx, band: usize, values: &[(&str, f32)]) {
    let mut params = Map::new();
    for (id, v) in values {
        if let Some(p) = c.param(id) {
            params.insert((*id).to_string(), json!(p.clamp(*v)));
        }
    }
    if params.is_empty() {
        return;
    }
    let key = format!("trimio:{}:{}:{band}", c.tid.0, c.slot);
    let _ = app.engine.execute_merged("mix.insert_params", &json!({"track": c.tid.0, "slot": c.slot, "params": Value::Object(params)}), &key);
}

pub(crate) fn end_gesture(app: &mut SoundApp) {
    app.engine.end_merge();
}

pub(crate) fn fmt_freq(f: f32) -> String {
    if f >= 1000.0 {
        format!("{:.2} kHz", f / 1000.0)
    } else if f < 10.0 {
        format!("{f:.2} Hz")
    } else {
        format!("{f:.0} Hz")
    }
}

pub(crate) fn fmt_db(db: f32) -> String {
    format!("{db:+.1} dB")
}

pub(crate) fn pill(ui: &mut Ui, p: &TrimioPalette, text: &str, active: bool, tip: &str) -> egui::Response {
    let (fill, fg, border) = if active { (p.accent_muted, p.text_primary, p.accent) } else { (p.surface, p.text_secondary, p.border) };
    let b = egui::Button::new(egui::RichText::new(text).font(bold(12.0)).color(fg))
        .fill(fill)
        .stroke(Stroke::new(1.0, border))
        .corner_radius(CornerRadius::same(radius::LG as u8))
        .min_size(vec2(32.0, 28.0));
    ui.add(b).on_hover_text(tip)
}

#[derive(Clone, Copy)]
pub(crate) enum Icon {
    Power,
    Undo,
    Redo,
}

/// A square header button with an icon drawn in code (Trimio uses Lucide-style line icons).
pub(crate) fn icon_button(ui: &mut Ui, p: &TrimioPalette, icon: Icon, active: bool, tip: &str) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(32.0, 28.0), Sense::click());
    let hover = resp.hovered();
    let (fill, fg, border) = if active {
        (p.accent_muted, p.text_primary, p.accent)
    } else {
        (if hover { p.surface_raised } else { p.surface }, if hover { p.text_primary } else { p.text_secondary }, p.border)
    };
    let painter = ui.painter();
    painter.rect_filled(r, CornerRadius::same(radius::LG as u8), fill);
    painter.rect_stroke(r, CornerRadius::same(radius::LG as u8), Stroke::new(1.0, border), StrokeKind::Inside);
    let c = r.center();
    let st = Stroke::new(1.6, fg);
    let arc = |a0: f32, a1: f32, rad: f32| -> Vec<Pos2> {
        (0..=20).map(|k| a0 + (a1 - a0) * k as f32 / 20.0).map(|a| c + vec2(a.cos(), a.sin()) * rad).collect()
    };
    use std::f32::consts::PI;
    match icon {
        Icon::Power => {
            painter.add(egui::Shape::line(arc(-PI * 0.5 + 0.7, PI * 1.5 - 0.7, 6.5), st));
            painter.line_segment([c + vec2(0.0, -8.0), c + vec2(0.0, -1.5)], st);
        }
        Icon::Undo | Icon::Redo => {
            let flip = if matches!(icon, Icon::Redo) { -1.0 } else { 1.0 };
            let pts: Vec<Pos2> = arc(PI * 1.1, PI * 2.2, 5.5).into_iter().map(|q| pos2(c.x + (q.x - c.x) * flip, q.y + 1.5)).collect();
            if let Some(tip_pt) = pts.first().copied() {
                painter.line_segment([tip_pt, tip_pt + vec2(4.0 * flip, -1.0)], st);
                painter.line_segment([tip_pt, tip_pt + vec2(0.5 * flip, -4.5)], st);
            }
            painter.add(egui::Shape::line(pts, st));
        }
    }
    resp.on_hover_text(tip)
}

pub(crate) fn header(app: &mut SoundApp, ui: &mut Ui, c: &Ctx, p: &TrimioPalette, title: &str) {
    let width = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(width, HEADER_HEIGHT), Sense::hover());
    ui.painter().rect_filled(r, CornerRadius::same(radius::LG as u8), p.header_bg);
    let mut child =
        ui.new_child(egui::UiBuilder::new().max_rect(r.shrink2(vec2(space::S3, 0.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
    let ui = &mut child;
    // Power: on = processing, off = bypassed.
    if icon_button(ui, p, Icon::Power, !c.ins.bypass, if c.ins.bypass { "Bypassed — click to enable" } else { "Enabled — click to bypass" }).clicked()
    {
        let _ = app.run("mix.insert_bypass", json!({"track": c.tid.0, "slot": c.slot}));
    }
    ui.label(egui::RichText::new(title).font(bold(16.0)).color(p.text_primary));
    let preset = if c.ins.preset.is_empty() || c.ins.preset == "<factory default>" { "Default" } else { c.ins.preset.as_str() };
    ui.menu_button(egui::RichText::new(format!("{preset}  v")).font(regular(13.0)).color(p.text_secondary), |ui| {
        crate::panels::preset_menu(app, ui, c.tid, c.slot, c.info, c.ins);
    });
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if icon_button(ui, p, Icon::Redo, false, "Redo").clicked() {
            let _ = app.run("edit.redo", json!({}));
        }
        if icon_button(ui, p, Icon::Undo, false, "Undo").clicked() {
            let _ = app.run("edit.undo", json!({}));
        }
        ab_buttons(app, ui, c, p);
    });
}

/// A/B comparison: two parameter snapshots per insert, kept for the session.
pub(crate) fn ab_buttons(app: &mut SoundApp, ui: &mut Ui, c: &Ctx, p: &TrimioPalette) {
    let id = egui::Id::new(("trimio_eq_ab", c.tid.0, c.slot));
    let (mut active, mut other): (u8, Option<Map<String, Value>>) = ui.ctx().data_mut(|d| d.get_temp(id)).unwrap_or((0, None));
    let current: Map<String, Value> = c.ins.params.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
    // Right-to-left layout: B first.
    for (slot_ab, label) in [(1u8, "B"), (0u8, "A")] {
        if pill(ui, p, label, active == slot_ab, "A/B compare").clicked() && active != slot_ab {
            let target = other.take().unwrap_or_else(|| current.clone());
            other = Some(current.clone());
            active = slot_ab;
            let _ = app.run("mix.insert_params", json!({"track": c.tid.0, "slot": c.slot, "params": Value::Object(target)}));
        }
    }
    ui.ctx().data_mut(|d| d.insert_temp(id, (active, other)));
}

pub(crate) fn on_value(c: &Ctx, id: &str) -> bool {
    c.value(id) >= 0.5
}

/// A Trimio knob: a value arc around a flat disc, label above and value below.
pub(crate) fn knob(app: &mut SoundApp, ui: &mut Ui, c: &Ctx, band: usize, id: &str, label: &str, color: Color32, p: &TrimioPalette) {
    let Some(info) = c.param(id) else { return };
    let size = 48.0;
    let v = c.value(id);
    let to_t = |x: f32| match info.taper {
        Taper::Log if info.min > 0.0 => (x.max(info.min) / info.min).ln() / (info.max / info.min).ln(),
        _ => (x - info.min) / (info.max - info.min),
    };
    let from_t = |t: f32| {
        let t = t.clamp(0.0, 1.0);
        match info.taper {
            Taper::Log if info.min > 0.0 => info.min * (info.max / info.min).powf(t),
            _ => info.min + (info.max - info.min) * t,
        }
    };
    ui.allocate_ui_with_layout(vec2(84.0, 112.0), egui::Layout::top_down(egui::Align::Center), |ui| {
        ui.set_width(84.0);
        ui.label(egui::RichText::new(label).font(regular(12.0)).color(p.text_secondary));
        let (r, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click_and_drag());
        let resp = resp.on_hover_cursor(CursorIcon::ResizeVertical);
        if resp.dragged() {
            let fine = ui.input(|i| i.modifiers.shift || i.modifiers.alt);
            let k = if fine { 0.0015 } else { 0.006 };
            let d = resp.drag_delta();
            set(app, c, band, &[(id, from_t(to_t(v) + (d.x - d.y) * k))]);
        }
        if resp.drag_stopped() {
            end_gesture(app);
        }
        if resp.double_clicked() {
            set(app, c, band, &[(id, info.default)]);
            end_gesture(app);
        }
        let painter = ui.painter();
        let center = r.center();
        let rad = size * 0.5 - 3.0;
        let start = std::f32::consts::PI * 0.75;
        let sweep = std::f32::consts::PI * 1.5;
        let arc = |a0: f32, a1: f32| -> Vec<Pos2> {
            (0..=32).map(|k| a0 + (a1 - a0) * k as f32 / 32.0).map(|a| center + vec2(a.cos(), a.sin()) * rad).collect()
        };
        painter.add(egui::Shape::line(arc(start, start + sweep), Stroke::new(3.0, p.surface_raised)));
        let bipolar = info.min < 0.0 && info.max > 0.0;
        let at = start + sweep * to_t(v).clamp(0.0, 1.0);
        let from = if bipolar { start + sweep * to_t(0.0) } else { start };
        let (a0, a1) = if at < from { (at, from) } else { (from, at) };
        painter.add(egui::Shape::line(arc(a0, a1), Stroke::new(3.0, color)));
        painter.circle_filled(center, rad - 6.0, p.surface_raised);
        let dir = vec2(at.cos(), at.sin());
        painter.line_segment([center + dir * (rad - 16.0), center + dir * (rad - 8.0)], Stroke::new(2.0, p.text_primary));
        let shown = match info.unit {
            soundcraft_dsp::Unit::Hz => fmt_freq(v),
            soundcraft_dsp::Unit::Db => fmt_db(v),
            _ => info.format(v),
        };
        ui.label(egui::RichText::new(shown).font(bold(13.0)).color(p.text_primary));
    });
}

pub(crate) fn card(ui: &mut Ui, p: &TrimioPalette, add: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0, p.border_subtle))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::same(space::S3 as i8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
}

/// One parameter as a card (knob, value) in the core panel's inspector grid.
pub(crate) fn param_card(app: &mut SoundApp, ui: &mut Ui, c: &Ctx, group: usize, id: &str, label: &str, p: &TrimioPalette) {
    egui::Frame::new()
        .fill(p.surface_raised.gamma_multiply(0.35))
        .stroke(Stroke::new(1.0, p.border_subtle))
        .corner_radius(CornerRadius::same(radius::MD as u8))
        .inner_margin(egui::Margin::same(space::S1 as i8))
        .show(ui, |ui| knob(app, ui, c, group, id, label, p.accent, p));
}

/// Meter segment colour by position (0 = first lit): green, then amber, then red, as the GR meter.
fn seg_color(i: usize, segs: usize, p: &TrimioPalette) -> Color32 {
    let t = (i as f32 + 1.0) / segs.max(1) as f32;
    if t > 9.0 / 16.0 {
        p.error
    } else if t > 3.0 / 16.0 {
        p.warning
    } else {
        p.success
    }
}

/// The always-visible vertical meter of an effect, lit top-down to `frac` (0..1), with a readout.
pub(crate) fn seg_meter(ui: &mut Ui, p: &TrimioPalette, label: &str, frac: f32, readout: &str, hot: bool) {
    let (r, _) = ui.allocate_exact_size(vec2(26.0, 288.0), Sense::hover());
    let painter = ui.painter_at(r.expand(2.0));
    painter.text(pos2(r.center().x, r.min.y), egui::Align2::CENTER_TOP, label, bold(10.0), p.text_muted);
    let body = egui::Rect::from_min_max(r.min + vec2(4.0, 16.0), r.max - vec2(4.0, 20.0));
    painter.rect_filled(body, CornerRadius::same(radius::SM as u8), p.workspace_bg);
    painter.rect_stroke(body, CornerRadius::same(radius::SM as u8), Stroke::new(1.0, p.border_subtle), StrokeKind::Inside);
    let segs = 16;
    let lit = (frac.clamp(0.0, 1.0) * segs as f32).round() as usize;
    let h = (body.height() - 4.0) / segs as f32;
    for i in 0..segs {
        let y0 = body.min.y + 2.0 + h * i as f32;
        let sr = egui::Rect::from_min_max(pos2(body.min.x + 2.0, y0 + 1.0), pos2(body.max.x - 2.0, y0 + h - 1.0));
        painter.rect_filled(sr, CornerRadius::same(1), if i < lit { seg_color(i, segs, p) } else { p.surface });
    }
    let color = if hot && lit > 0 { seg_color(lit.saturating_sub(1), segs, p) } else { p.text_muted };
    painter.text(pos2(r.center().x, r.max.y), egui::Align2::CENTER_BOTTOM, readout, crate::theme::mono(10.0), color);
}

/// The preset row: one pill per factory preset; the applied one is highlighted.
pub(crate) fn preset_row(app: &mut SoundApp, ui: &mut Ui, c: &Ctx, p: &TrimioPalette, presets: &[(&str, Vec<(&str, f32)>)]) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("PRESETS").font(bold(11.0)).color(p.text_muted));
        ui.add_space(space::S2);
        for (name, values) in presets {
            if pill(ui, p, name, c.ins.preset == *name, "Trimio factory preset").clicked() {
                let params: Map<String, Value> =
                    values.iter().filter(|(k, _)| c.param(k).is_some()).map(|(k, v)| ((*k).to_string(), json!(v))).collect();
                let _ = app.run("mix.insert_params", json!({"track": c.tid.0, "slot": c.slot, "params": Value::Object(params), "preset": name}));
            }
        }
    });
}
