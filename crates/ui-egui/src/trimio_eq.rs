//! The EQ 7-Band editor in the Trimio Slate design language: the frequency graph is the primary
//! surface (draggable band nodes over the summed response), with a band inspector and output footer.
//! Colours, spacing and radii come from the generated [`crate::trimio_tokens`].

use crate::SoundApp;
use crate::theme::{bold, mono, regular};
use crate::trimio_tokens::{BANDS, HEADER_HEIGHT, TrimioPalette, radius, space};
use egui::{Align2, Color32, CornerRadius, CursorIcon, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::{Map, Value, json};
use soundcraft_dsp::{ParamInfo, PluginInfo, Taper};
use soundcraft_model::{Insert, TrackId};

const F_MIN: f32 = 20.0;
const F_MAX: f32 = 20_000.0;
const DB_RANGE: f32 = 24.0;
const SNAP_FREQS: [f32; 10] = [20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10_000.0, 20_000.0];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    HighPass,
    LowShelf,
    Bell,
    HighShelf,
    LowPass,
}

/// One band of the EQ 7-Band plugin, by its parameter ids.
struct Band {
    label: &'static str,
    kind: Kind,
    freq: &'static str,
    gain: Option<&'static str>,
    q: Option<&'static str>,
    on: Option<&'static str>,
    slope: Option<&'static str>,
}

const fn bell(label: &'static str, freq: &'static str, gain: &'static str, q: &'static str) -> Band {
    Band { label, kind: Kind::Bell, freq, gain: Some(gain), q: Some(q), on: None, slope: None }
}

static BAND_MAP: [Band; 7] = [
    Band { label: "Low Cut", kind: Kind::HighPass, freq: "hpf_freq", gain: None, q: None, on: Some("hpf_on"), slope: Some("hpf_slope") },
    Band {
        label: "Low Shelf",
        kind: Kind::LowShelf,
        freq: "low_shelf_freq",
        gain: Some("low_shelf_gain"),
        q: Some("low_shelf_q"),
        on: None,
        slope: None,
    },
    bell("Low Mid", "low_mid_freq", "low_mid_gain", "low_mid_q"),
    bell("Mid", "mid_freq", "mid_gain", "mid_q"),
    bell("High Mid", "high_mid_freq", "high_mid_gain", "high_mid_q"),
    Band {
        label: "High Shelf",
        kind: Kind::HighShelf,
        freq: "high_shelf_freq",
        gain: Some("high_shelf_gain"),
        q: Some("high_shelf_q"),
        on: None,
        slope: None,
    },
    Band { label: "High Cut", kind: Kind::LowPass, freq: "lpf_freq", gain: None, q: None, on: Some("lpf_on"), slope: Some("lpf_slope") },
];

fn palette() -> TrimioPalette {
    if crate::theme::is_light() { TrimioPalette::LIGHT } else { TrimioPalette::DARK }
}

fn band_color(i: usize) -> Color32 {
    BANDS.get(i).copied().unwrap_or(Color32::GRAY)
}

/// The window frame the editor sits in.
pub fn frame(style: &egui::Style) -> egui::Frame {
    let p = palette();
    egui::Frame::window(style).fill(p.bg).stroke(Stroke::new(1.0, p.border)).corner_radius(CornerRadius::same(radius::XL as u8))
}

struct Ctx<'a> {
    info: &'static PluginInfo,
    ins: &'a Insert,
    tid: TrackId,
    slot: usize,
}

impl Ctx<'_> {
    fn param(&self, id: &str) -> Option<&'static ParamInfo> {
        self.info.params.iter().find(|p| p.id == id)
    }
    fn value(&self, id: &str) -> f32 {
        let def = self.param(id).map_or(0.0, |p| p.default);
        self.ins.params.get(id).copied().unwrap_or(def)
    }
    fn is_on(&self, band: &Band) -> bool {
        band.on.is_none_or(|id| self.value(id) >= 0.5)
    }
}

/// Sets parameters of one band. One drag gesture merges into one undo step (see [`end_gesture`]).
fn set(app: &mut SoundApp, c: &Ctx, band: usize, values: &[(&str, f32)]) {
    let mut params = Map::new();
    for (id, v) in values {
        if let Some(p) = c.param(id) {
            params.insert((*id).to_string(), json!(p.clamp(*v)));
        }
    }
    if params.is_empty() {
        return;
    }
    let key = format!("trimio_eq:{}:{}:{band}", c.tid.0, c.slot);
    let _ = app.engine.execute_merged("mix.insert_params", &json!({"track": c.tid.0, "slot": c.slot, "params": Value::Object(params)}), &key);
}

fn end_gesture(app: &mut SoundApp) {
    app.engine.end_merge();
}

fn x_of(f: f32, r: Rect) -> f32 {
    let t = (f.max(F_MIN) / F_MIN).ln() / (F_MAX / F_MIN).ln();
    r.min.x + r.width() * t.clamp(0.0, 1.0)
}

fn f_of(x: f32, r: Rect) -> f32 {
    let t = ((x - r.min.x) / r.width().max(1.0)).clamp(0.0, 1.0);
    F_MIN * (F_MAX / F_MIN).powf(t)
}

fn y_of(db: f32, r: Rect) -> f32 {
    r.center().y - db.clamp(-DB_RANGE, DB_RANGE) / DB_RANGE * r.height() * 0.5
}

fn db_of(y: f32, r: Rect) -> f32 {
    (r.center().y - y) / (r.height() * 0.5).max(1.0) * DB_RANGE
}

fn fmt_freq(f: f32) -> String {
    if f >= 1000.0 { format!("{:.2} kHz", f / 1000.0) } else { format!("{f:.0} Hz") }
}

fn fmt_db(db: f32) -> String {
    format!("{db:+.1} dB")
}

fn freq_label(f: f32) -> String {
    if f >= 1000.0 { format!("{}k", f / 1000.0) } else { format!("{f}") }
}

/// The summed response, or one band's alone (`only`).
fn response(c: &Ctx, only: Option<usize>, freqs: &[f32]) -> Vec<f32> {
    let mut params: Vec<(&str, f32)> = c.info.params.iter().map(|p| (p.id, c.value(p.id))).collect();
    if let Some(keep) = only {
        for (i, b) in BAND_MAP.iter().enumerate() {
            if i == keep {
                continue;
            }
            for (id, v) in params.iter_mut() {
                if Some(*id) == b.gain || Some(*id) == b.on {
                    *v = 0.0;
                }
            }
        }
        for (id, v) in params.iter_mut() {
            if *id == "input_gain" || *id == "output_gain" {
                *v = 0.0;
            }
        }
    }
    soundcraft_dsp::eq7_response(&params, freqs, 48_000.0)
}

fn sel_id(c: &Ctx) -> egui::Id {
    egui::Id::new(("trimio_eq_sel", c.tid.0, c.slot))
}

/// Draws the whole editor into the plugin window.
pub fn editor(app: &mut SoundApp, ui: &mut Ui, tid: TrackId, slot: usize, info: &'static PluginInfo, ins: &Insert) {
    let c = Ctx { info, ins, tid, slot };
    let p = palette();
    ui.spacing_mut().item_spacing = vec2(space::S2, space::S2);
    let dim = ins.bypass;
    if dim {
        ui.set_opacity(0.6);
    }
    header(app, ui, &c, &p);
    ui.add_space(space::S2);
    graph(app, ui, &c, &p);
    ui.add_space(space::S3);
    inspector(app, ui, &c, &p);
    ui.add_space(space::S3);
    footer(app, ui, &c, &p);
}

fn pill(ui: &mut Ui, p: &TrimioPalette, text: &str, active: bool, tip: &str) -> egui::Response {
    let (fill, fg, border) = if active { (p.accent_muted, p.text_primary, p.accent) } else { (p.surface, p.text_secondary, p.border) };
    let b = egui::Button::new(egui::RichText::new(text).font(bold(12.0)).color(fg))
        .fill(fill)
        .stroke(Stroke::new(1.0, border))
        .corner_radius(CornerRadius::same(radius::LG as u8))
        .min_size(vec2(32.0, 28.0));
    ui.add(b).on_hover_text(tip)
}

#[derive(Clone, Copy)]
enum Icon {
    Power,
    Undo,
    Redo,
}

/// A square header button with an icon drawn in code (Trimio uses Lucide-style line icons).
fn icon_button(ui: &mut Ui, p: &TrimioPalette, icon: Icon, active: bool, tip: &str) -> egui::Response {
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

fn header(app: &mut SoundApp, ui: &mut Ui, c: &Ctx, p: &TrimioPalette) {
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
    ui.label(egui::RichText::new("Equalizer").font(bold(16.0)).color(p.text_primary));
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
fn ab_buttons(app: &mut SoundApp, ui: &mut Ui, c: &Ctx, p: &TrimioPalette) {
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

fn graph(app: &mut SoundApp, ui: &mut Ui, c: &Ctx, p: &TrimioPalette) {
    let width = ui.available_width().max(480.0);
    let height = (width * 0.45).clamp(260.0, 420.0);
    let (outer, bg_resp) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    let painter = ui.painter_at(outer);
    painter.rect_filled(outer, CornerRadius::same(radius::XL as u8), p.workspace_bg);
    painter.rect_stroke(outer, CornerRadius::same(radius::XL as u8), Stroke::new(1.0, p.border_subtle), StrokeKind::Inside);
    let r = Rect::from_min_max(outer.min + vec2(40.0, space::S4), outer.max - vec2(space::S4, 28.0));
    let grid = p.border_subtle;
    let label = |pos: Pos2, align: Align2, text: String| {
        painter.text(pos, align, text, regular(11.0), p.text_muted);
    };
    // Gain grid: 6 dB lines, labels every 12 dB.
    let mut db = -DB_RANGE;
    while db <= DB_RANGE + 0.1 {
        let y = y_of(db, r);
        let stroke = if db.abs() < 0.1 { Stroke::new(1.0, p.border_strong) } else { Stroke::new(1.0, grid) };
        painter.line_segment([pos2(r.min.x, y), pos2(r.max.x, y)], stroke);
        if (db as i32) % 12 == 0 {
            label(pos2(r.min.x - space::S2, y), Align2::RIGHT_CENTER, if db > 0.0 { format!("+{db:.0}") } else { format!("{db:.0}") });
        }
        db += 6.0;
    }
    // Frequency grid: decades and the 2/5 steps.
    for f in [20.0f32, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10_000.0, 20_000.0] {
        let x = x_of(f, r);
        painter.line_segment([pos2(x, r.min.y), pos2(x, r.max.y)], Stroke::new(1.0, grid));
        label(pos2(x, r.max.y + space::S2), Align2::CENTER_TOP, freq_label(f));
    }

    let n = 220;
    let freqs: Vec<f32> = (0..n).map(|i| F_MIN * (F_MAX / F_MIN).powf(i as f32 / (n - 1) as f32)).collect();
    let xs: Vec<f32> = freqs.iter().map(|f| x_of(*f, r)).collect();
    let zero = y_of(0.0, r);
    let selected: Option<usize> = ui.ctx().data_mut(|d| d.get_temp(sel_id(c)));

    // Each active band's own contribution, as a translucent area in its colour.
    for (i, band) in BAND_MAP.iter().enumerate() {
        let gain_active = band.gain.is_some_and(|g| c.value(g).abs() > 0.05);
        if !c.is_on(band) || (band.on.is_none() && !gain_active) {
            continue;
        }
        let resp = response(c, Some(i), &freqs);
        let alpha = if selected == Some(i) { 70 } else { 34 };
        let col = band_color(i).gamma_multiply(alpha as f32 / 255.0);
        for k in 1..n {
            let (Some(x0), Some(x1), Some(d0), Some(d1)) = (xs.get(k - 1), xs.get(k), resp.get(k - 1), resp.get(k)) else { continue };
            let (y0, y1) = (y_of(*d0, r), y_of(*d1, r));
            painter.add(egui::Shape::convex_polygon(vec![pos2(*x0, zero), pos2(*x0, y0), pos2(*x1, y1), pos2(*x1, zero)], col, Stroke::NONE));
        }
    }
    // The summed curve.
    let total = response(c, None, &freqs);
    let pts: Vec<Pos2> = xs.iter().zip(&total).map(|(x, d)| pos2(*x, y_of(*d, r))).collect();
    painter.add(egui::Shape::line(pts, Stroke::new(2.5, p.accent)));

    // Band nodes.
    let shift = ui.input(|i| i.modifiers.shift);
    let alt = ui.input(|i| i.modifiers.alt);
    let mut new_sel = selected;
    let mut hovered_band = None;
    for (i, band) in BAND_MAP.iter().enumerate() {
        let f = c.value(band.freq);
        let on = c.is_on(band);
        let y = match band.gain {
            Some(g) => y_of(c.value(g), r),
            None => y_of(if on { -3.0 } else { 0.0 }, r),
        };
        let center = pos2(x_of(f, r), y);
        let hit = Rect::from_center_size(center, vec2(22.0, 22.0));
        let resp = ui.interact(hit, ui.id().with(("trimio_node", c.tid.0, c.slot, i)), Sense::click_and_drag());
        let col = band_color(i);
        if resp.hovered() {
            hovered_band = Some(i);
            ui.ctx().set_cursor_icon(CursorIcon::Grab);
        }
        if resp.drag_started() || resp.clicked() {
            new_sel = Some(i);
        }
        if resp.dragged()
            && let Some(ptr) = resp.interact_pointer_pos()
        {
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            let mut values: Vec<(&str, f32)> = Vec::new();
            if !shift || band.gain.is_none() {
                let mut nf = if alt { f * (resp.drag_delta().x * 0.002).exp() } else { f_of(ptr.x, r) };
                if !alt && let Some(s) = SNAP_FREQS.iter().find(|s| (nf / **s - 1.0).abs() < 0.03) {
                    nf = *s;
                }
                values.push((band.freq, nf));
            }
            if let Some(g) = band.gain {
                let mut ng = if alt { c.value(g) - resp.drag_delta().y * 0.02 } else { db_of(ptr.y, r) };
                if !alt && ng.abs() < 0.4 {
                    ng = 0.0;
                }
                values.push((g, ng));
            }
            if let Some(on) = band.on
                && !on_value(c, on)
            {
                values.push((on, 1.0));
            }
            set(app, c, i, &values);
        }
        if resp.drag_stopped() {
            end_gesture(app);
        }
        if resp.double_clicked() {
            match (band.gain, band.on) {
                (Some(g), _) => set(app, c, i, &[(g, 0.0)]),
                (None, Some(on_id)) => set(app, c, i, &[(on_id, if on { 0.0 } else { 1.0 })]),
                _ => {}
            }
            end_gesture(app);
        }
        // Scroll over a node: Q (up narrows).
        if resp.hovered()
            && let Some(q) = band.q
        {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.0 {
                set(app, c, i, &[(q, c.value(q) * (scroll * 0.004).exp())]);
            }
        }
        resp.context_menu(|ui| {
            ui.label(egui::RichText::new(format!("{} · {}", i + 1, band.label)).font(bold(12.0)));
            if ui.button("Reset Band").clicked() {
                let mut v: Vec<(&str, f32)> = Vec::new();
                for id in [Some(band.freq), band.gain, band.q, band.slope].into_iter().flatten() {
                    v.push((id, c.param(id).map_or(0.0, |p| p.default)));
                }
                set(app, c, i, &v);
                end_gesture(app);
                ui.close();
            }
            if let Some(on_id) = band.on
                && ui.button(if on { "Disable Band" } else { "Enable Band" }).clicked()
            {
                set(app, c, i, &[(on_id, if on { 0.0 } else { 1.0 })]);
                end_gesture(app);
                ui.close();
            }
        });

        // Visual states: idle (hollow ring), hover (larger), selected (filled, white ring), disabled (muted dashed).
        let is_sel = new_sel == Some(i);
        let rad = if resp.dragged() {
            9.0
        } else if is_sel {
            8.0
        } else if resp.hovered() {
            7.0
        } else {
            6.0
        };
        if !on {
            painter.add(egui::Shape::dashed_line(&[center, pos2(center.x, r.max.y)], Stroke::new(1.0, p.text_disabled), 3.0, 3.0));
            painter.circle_filled(center, rad, p.workspace_bg);
            painter.circle_stroke(center, rad, Stroke::new(1.5, p.text_disabled));
        } else if is_sel {
            painter.circle_filled(center, rad, col);
            painter.circle_stroke(center, rad + 1.0, Stroke::new(2.0, Color32::WHITE));
        } else {
            painter.circle_filled(center, rad, p.workspace_bg);
            painter.circle_stroke(center, rad, Stroke::new(2.0, col));
        }
        painter.text(center, Align2::CENTER_CENTER, format!("{}", i + 1), bold(9.0), if is_sel { Color32::WHITE } else { col });
    }
    if bg_resp.clicked() && hovered_band.is_none() {
        new_sel = None;
    }
    // Hover tooltip with live numbers.
    if let Some(i) = hovered_band
        && let Some(band) = BAND_MAP.get(i)
    {
        let mut lines = vec![format!("{} {}", i + 1, band.label), format!("Freq: {}", fmt_freq(c.value(band.freq)))];
        if let Some(g) = band.gain {
            lines.push(format!("Gain: {}", fmt_db(c.value(g))));
        }
        if let Some(q) = band.q {
            lines.push(format!("Q: {:.2}", c.value(q)));
        }
        let text = lines.join("\n");
        let at = pos2(x_of(c.value(band.freq), r) + 14.0, r.min.y + space::S2);
        let galley = painter.layout_no_wrap(text, regular(11.0), p.text_primary);
        let box_r = Rect::from_min_size(at, galley.size() + vec2(space::S4, space::S3));
        let box_r = box_r.translate(vec2((r.max.x - box_r.max.x).min(0.0), 0.0));
        painter.rect_filled(box_r, CornerRadius::same(radius::LG as u8), p.surface_overlay);
        painter.rect_stroke(box_r, CornerRadius::same(radius::LG as u8), Stroke::new(1.0, p.border_strong), StrokeKind::Inside);
        painter.galley(box_r.min + vec2(space::S2, space::S1 + 2.0), galley, p.text_primary);
    }
    // Keyboard: arrows move the selected band (Shift = fine).
    if let Some(i) = new_sel
        && let Some(band) = BAND_MAP.get(i)
        && ui.memory(|m| m.focused().is_none())
    {
        let (l, rt, up, dn) = ui.input(|inp| {
            (
                inp.key_pressed(egui::Key::ArrowLeft),
                inp.key_pressed(egui::Key::ArrowRight),
                inp.key_pressed(egui::Key::ArrowUp),
                inp.key_pressed(egui::Key::ArrowDown),
            )
        });
        let fine = shift;
        if l || rt {
            let step: f32 = if fine { 1.0 / 240.0 } else { 1.0 / 24.0 };
            let f = c.value(band.freq) * 2f32.powf(if rt { step } else { -step } * 2.0);
            set(app, c, i, &[(band.freq, f)]);
            end_gesture(app);
        }
        if (up || dn)
            && let Some(g) = band.gain
        {
            let step = if fine { 0.1 } else { 0.5 };
            set(app, c, i, &[(g, c.value(g) + if up { step } else { -step })]);
            end_gesture(app);
        }
    }
    ui.ctx().data_mut(|d| {
        if let Some(s) = new_sel {
            d.insert_temp(sel_id(c), s);
        } else {
            d.remove::<usize>(sel_id(c));
        }
    });
}

fn on_value(c: &Ctx, id: &str) -> bool {
    c.value(id) >= 0.5
}

/// A Trimio knob: a value arc around a flat disc, label above and value below.
fn knob(app: &mut SoundApp, ui: &mut Ui, c: &Ctx, band: usize, id: &str, label: &str, color: Color32, p: &TrimioPalette) {
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

fn card(ui: &mut Ui, p: &TrimioPalette, add: impl FnOnce(&mut Ui)) {
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

fn inspector(app: &mut SoundApp, ui: &mut Ui, c: &Ctx, p: &TrimioPalette) {
    let selected: Option<usize> = ui.ctx().data_mut(|d| d.get_temp(sel_id(c)));
    card(ui, p, |ui| {
        ui.set_min_height(120.0);
        let Some((i, band)) = selected.and_then(|i| BAND_MAP.get(i).map(|b| (i, b))) else {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("BAND INSPECTOR").font(bold(11.0)).color(p.text_muted));
            });
            ui.add_space(space::S6);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("Select an EQ band node to adjust parameters").font(regular(13.0)).color(p.text_muted));
            });
            return;
        };
        let col = band_color(i);
        ui.horizontal(|ui| {
            let (dot, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
            ui.painter().circle_filled(dot.center(), 5.0, col);
            ui.label(egui::RichText::new(format!("Band {} · {}", i + 1, band.label)).font(bold(14.0)).color(p.text_primary));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(on_id) = band.on {
                    let on = on_value(c, on_id);
                    if pill(ui, p, if on { "On" } else { "Off" }, on, "Enable or disable this filter").clicked() {
                        set(app, c, i, &[(on_id, if on { 0.0 } else { 1.0 })]);
                        end_gesture(app);
                    }
                }
                let kind = match band.kind {
                    Kind::HighPass => "High Pass",
                    Kind::LowShelf => "Low Shelf",
                    Kind::Bell => "Bell",
                    Kind::HighShelf => "High Shelf",
                    Kind::LowPass => "Low Pass",
                };
                ui.label(egui::RichText::new(kind).font(regular(12.0)).color(p.text_secondary));
            });
        });
        ui.add_space(space::S2);
        ui.horizontal(|ui| {
            knob(app, ui, c, i, band.freq, "Freq", col, p);
            if let Some(g) = band.gain {
                knob(app, ui, c, i, g, "Gain", col, p);
            }
            if let Some(q) = band.q {
                knob(app, ui, c, i, q, "Q", col, p);
            }
            if let Some(slope) = band.slope
                && let Some(info) = c.param(slope)
            {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("Slope").font(regular(12.0)).color(p.text_secondary));
                    let mut idx = c.value(slope).round().max(0.0) as usize;
                    let before = idx;
                    egui::ComboBox::from_id_salt(("trimio_slope", c.tid.0, c.slot, i))
                        .selected_text(info.choices.get(idx).copied().unwrap_or("?"))
                        .show_ui(ui, |ui| {
                            for (k, name) in info.choices.iter().enumerate() {
                                ui.selectable_value(&mut idx, k, *name);
                            }
                        });
                    if idx != before {
                        set(app, c, i, &[(slope, idx as f32)]);
                        end_gesture(app);
                    }
                });
            }
        });
    });
}

fn footer(app: &mut SoundApp, ui: &mut Ui, c: &Ctx, p: &TrimioPalette) {
    card(ui, p, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("OUTPUT").font(bold(11.0)).color(p.text_muted));
            ui.add_space(space::S4);
            knob(app, ui, c, usize::MAX, "input_gain", "Input", p.accent, p);
            knob(app, ui, c, usize::MAX, "output_gain", "Output", p.accent, p);
            ui.add_space(space::S6);
            let active = BAND_MAP.iter().filter(|b| c.is_on(b) && (b.on.is_some() || b.gain.is_some_and(|g| c.value(g).abs() > 0.05))).count();
            ui.vertical(|ui| {
                ui.set_min_width(180.0);
                ui.add_space(space::S6);
                ui.label(egui::RichText::new(format!("{active} of 7 bands active")).font(mono(11.0)).color(p.text_secondary));
                ui.label(egui::RichText::new("Latency 0 samples").font(mono(11.0)).color(p.text_muted));
            });
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_band_names_real_eq7_parameters() {
        let ids: Vec<&str> = soundcraft_dsp::plugins::eq::EQ7_INFO.params.iter().map(|p| p.id).collect();
        for b in &BAND_MAP {
            for id in [Some(b.freq), b.gain, b.q, b.on, b.slope].into_iter().flatten() {
                assert!(ids.contains(&id), "{id} is not an EQ 7-Band parameter");
            }
        }
    }

    #[test]
    fn frequency_and_gain_axes_round_trip() {
        let r = Rect::from_min_size(pos2(10.0, 20.0), vec2(800.0, 400.0));
        for f in [20.0f32, 100.0, 1000.0, 12_345.0, 20_000.0] {
            assert!((f_of(x_of(f, r), r) / f - 1.0).abs() < 1e-3);
        }
        for db in [-24.0f32, -6.5, 0.0, 3.0, 24.0] {
            assert!((db_of(y_of(db, r), r) - db).abs() < 1e-3);
        }
        // Out-of-range input is clamped onto the graph, never off it.
        assert!(x_of(5.0, r) >= r.min.x && x_of(1e9, r) <= r.max.x);
        assert!(y_of(99.0, r) >= r.min.y && y_of(f32::NAN.max(-99.0), r) <= r.max.y);
    }

    #[test]
    fn band_colours_cover_every_band() {
        assert_eq!(BANDS.len(), BAND_MAP.len());
    }
}
