//! Trimios skin for built-in plugin windows: the Trimio Slate dark tokens, raised panels,
//! recessed bays, LED-ring knobs, pill toggles and an EQ graph with draggable band nodes.
//!
//! Everything is drawn in code from the Trimios design tokens (no bitmaps). The window reads the
//! insert and returns what the user changed; the caller applies edits through the engine.

use crate::theme::{bold, mono, regular};
use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, pos2, vec2};
use soundcraft_dsp::{ParamInfo, PluginInfo, Unit};
use soundcraft_model::Insert;
use std::f32::consts::PI;

/// Trimio Slate v1.9 (dark): the Trimios Analyzer palette.
pub mod slate {
    use egui::Color32;
    pub const BG: Color32 = Color32::from_rgb(0x15, 0x19, 0x23);
    pub const SURFACE: Color32 = Color32::from_rgb(0x1A, 0x23, 0x37);
    pub const RAISED: Color32 = Color32::from_rgb(0x2A, 0x36, 0x48);
    pub const PLATE_LOW: Color32 = Color32::from_rgb(0x14, 0x1C, 0x2C);
    pub const BORDER: Color32 = Color32::from_rgb(0x2A, 0x36, 0x48);
    pub const BORDER_SUBTLE: Color32 = Color32::from_rgb(0x21, 0x2C, 0x3F);
    pub const BORDER_STRONG: Color32 = Color32::from_rgb(0x40, 0x4E, 0x63);
    pub const TEXT: Color32 = Color32::from_rgb(0xF1, 0xF5, 0xF9);
    pub const TEXT_2: Color32 = Color32::from_rgb(0xAA, 0xB6, 0xC8);
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(0x91, 0x9F, 0xB4);
    pub const TEXT_DISABLED: Color32 = Color32::from_rgb(0x47, 0x55, 0x69);
    pub const ACCENT: Color32 = Color32::from_rgb(0x60, 0xA5, 0xFA);
    pub const ACCENT_HOVER: Color32 = Color32::from_rgb(0x93, 0xC5, 0xFD);
    pub const ACCENT_MUTED: Color32 = Color32::from_rgb(0x3B, 0x82, 0xF6);
    pub const ACCENT_TEXT: Color32 = Color32::from_rgb(0x93, 0xC5, 0xFD);
    pub const AI: Color32 = Color32::from_rgb(0x10, 0xB9, 0x81);
    pub const SUCCESS: Color32 = Color32::from_rgb(0x22, 0xC5, 0x5E);
    pub const WARNING: Color32 = Color32::from_rgb(0xF5, 0x9E, 0x0B);
    pub const ERROR: Color32 = Color32::from_rgb(0xEF, 0x44, 0x44);
    pub const HEADER: Color32 = Color32::from_rgb(0x18, 0x20, 0x2C);
    pub const BAY_TOP: Color32 = Color32::from_rgb(0x07, 0x0A, 0x0F);
    pub const BAY: Color32 = Color32::from_rgb(0x0C, 0x10, 0x16);
    pub const BAY_LOW: Color32 = Color32::from_rgb(0x12, 0x19, 0x26);
    /// EQ band node colours, low to high.
    pub const BANDS: [Color32; 7] = [
        Color32::from_rgb(0x94, 0xA3, 0xB8),
        Color32::from_rgb(0xA8, 0x55, 0xF7),
        Color32::from_rgb(0x3B, 0x82, 0xF6),
        Color32::from_rgb(0x22, 0xC5, 0x5E),
        Color32::from_rgb(0xEA, 0xB3, 0x08),
        Color32::from_rgb(0xF9, 0x73, 0x16),
        Color32::from_rgb(0xEF, 0x44, 0x44),
    ];
}

/// What the user did in one frame of a Trimios plugin window.
#[derive(Default)]
pub struct Out {
    /// Parameter edits (id, new value), applied in order.
    pub edits: Vec<(&'static str, f32)>,
    pub toggle_bypass: bool,
    pub close: bool,
}

const KNOB: f32 = 48.0;
const COL_W: f32 = 74.0;

/// The whole window body: header, optional graph bay, then one section per parameter group.
/// `presets` fills the preset menu (it needs the app, which this module does not see).
pub fn body(ui: &mut Ui, salt: (u64, usize), title: &str, info: &PluginInfo, ins: &Insert, out: &mut Out, presets: &mut dyn FnMut(&mut Ui)) {
    header(ui, title, info, ins, out, presets);
    egui::Frame::new().inner_margin(egui::Margin { left: 10, right: 10, top: 10, bottom: 0 }).show(ui, |ui| {
        if info.id == "eq_7band" || info.id == "eq_1band" {
            eq_bay(ui, salt, info, ins, out);
            ui.add_space(8.0);
        }
        let groups = groups(info.params);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            for (name, params) in &groups {
                section(ui, salt, name, params, ins, out);
            }
        });
    });
}

/// Parameters grouped by their name's first word when two or more share it ("LF Freq", "LF Gain"),
/// the rest kept together; order follows the plugin's parameter list.
fn groups(params: &'static [ParamInfo]) -> Vec<(String, Vec<&'static ParamInfo>)> {
    let first = |p: &ParamInfo| p.name.split_whitespace().next().unwrap_or("").to_string();
    let mut out: Vec<(String, Vec<&'static ParamInfo>)> = Vec::new();
    for p in params {
        let key = first(p);
        let shared = params.iter().filter(|q| first(q) == key).count() >= 2;
        let key = if shared { key } else { String::new() };
        match out.iter_mut().find(|(k, _)| *k == key) {
            Some((_, v)) => v.push(p),
            None => out.push((key, vec![p])),
        }
    }
    out
}

fn header(ui: &mut Ui, title: &str, info: &PluginInfo, ins: &Insert, out: &mut Out, presets: &mut dyn FnMut(&mut Ui)) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width().max(320.0), 40.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 }, slate::HEADER);
    p.line_segment([pos2(r.min.x, r.max.y), pos2(r.max.x, r.max.y)], Stroke::new(1.0, slate::BORDER));

    // Power (bypass): lit accent ring when the plugin is running.
    let pw = Rect::from_center_size(pos2(r.min.x + 22.0, r.center().y), vec2(26.0, 26.0));
    let pr = ui.interact(pw, ui.id().with("tri-power"), Sense::click());
    let on = !ins.bypass;
    let ring = if on { slate::ACCENT } else { slate::TEXT_DISABLED };
    ui.painter().rect(
        pw,
        CornerRadius::same(6),
        if pr.hovered() { slate::RAISED } else { slate::SURFACE },
        Stroke::new(1.0, slate::BORDER_STRONG),
        StrokeKind::Inside,
    );
    power_icon(ui.painter(), pw.center(), 6.0, ring);
    if pr.on_hover_text(if on { "Bypass" } else { "Bypassed — click to enable" }).clicked() {
        out.toggle_bypass = true;
    }

    // Wordmark: TRIMIOS in mono bold, the plugin name in accent text.
    let x = pw.max.x + 12.0;
    let g = ui.painter().layout_no_wrap("TRIMIOS".into(), bold_mono(15.0), slate::TEXT);
    let w = g.size().x;
    ui.painter().galley(pos2(x, r.center().y - g.size().y * 0.5), g, slate::TEXT);
    ui.painter().text(pos2(x + w + 8.0, r.center().y), Align2::LEFT_CENTER, info.name.to_uppercase(), bold_mono(15.0), slate::ACCENT_TEXT);

    // Close.
    let cx = Rect::from_center_size(pos2(r.max.x - 18.0, r.center().y), vec2(22.0, 22.0));
    let cr = ui.interact(cx, ui.id().with("tri-close"), Sense::click());
    let col = if cr.hovered() { slate::TEXT } else { slate::TEXT_MUTED };
    let d = 4.5;
    ui.painter().line_segment([cx.center() + vec2(-d, -d), cx.center() + vec2(d, d)], Stroke::new(1.5, col));
    ui.painter().line_segment([cx.center() + vec2(-d, d), cx.center() + vec2(d, -d)], Stroke::new(1.5, col));
    if cr.clicked() {
        out.close = true;
    }

    // Preset pill, left of close: a menu button dressed in the Trimios pill.
    let preset = if ins.preset.is_empty() { "Default" } else { ins.preset.as_str() };
    let label = format!("{preset}  ▾");
    let pw = (ui.painter().layout_no_wrap(label.clone(), regular(12.0), slate::TEXT_2).size().x + 22.0).clamp(80.0, 200.0);
    let pb = Rect::from_center_size(pos2(cx.min.x - 10.0 - pw * 0.5, r.center().y), vec2(pw, 24.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(pb), |ui| {
        let v = ui.visuals_mut();
        for (w, fill) in [
            (&mut v.widgets.inactive, slate::SURFACE),
            (&mut v.widgets.hovered, slate::RAISED),
            (&mut v.widgets.active, slate::RAISED),
            (&mut v.widgets.open, slate::RAISED),
        ] {
            w.weak_bg_fill = fill;
            w.bg_fill = fill;
            w.bg_stroke = Stroke::new(1.0, slate::BORDER_STRONG);
            w.fg_stroke = Stroke::new(1.0, slate::TEXT_2);
            w.corner_radius = CornerRadius::same(8);
            w.expansion = 0.0;
        }
        ui.spacing_mut().button_padding = vec2(10.0, 4.0);
        ui.menu_button(egui::RichText::new(label).font(regular(12.0)), |ui| presets(ui)).response.on_hover_text(title);
    });
}

/// The wordmark face: the app has no bold monospace family, so the bold face stands in.
fn bold_mono(size: f32) -> FontId {
    bold(size)
}

fn power_icon(p: &egui::Painter, c: Pos2, r: f32, col: Color32) {
    let n = 20;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            let a = -PI * 0.5 + 0.55 + (2.0 * PI - 1.1) * i as f32 / n as f32;
            c + vec2(a.cos(), a.sin()) * r
        })
        .collect();
    p.add(Shape::line(pts, Stroke::new(1.6, col)));
    p.line_segment([c + vec2(0.0, -r - 1.0), c + vec2(0.0, -1.0)], Stroke::new(1.6, col));
}

/// A raised plate (panel) with the top highlight and soft bottom edge of the Trimios material.
pub fn plate(p: &egui::Painter, r: Rect) {
    gradient(p, r, slate::RAISED, slate::PLATE_LOW);
    p.rect_stroke(r, CornerRadius::same(8), Stroke::new(1.0, slate::BORDER_SUBTLE), StrokeKind::Inside);
    p.line_segment([pos2(r.min.x + 6.0, r.min.y + 1.0), pos2(r.max.x - 6.0, r.min.y + 1.0)], Stroke::new(1.0, Color32::from_white_alpha(19)));
}

/// A recessed screen (bay): near-black glass with an inner top shadow and a bottom hairline.
pub fn bay(p: &egui::Painter, r: Rect) {
    gradient(p, r, slate::BAY_TOP, slate::BAY_LOW);
    for i in 0..4 {
        let a = 120u8.saturating_sub(i * 30);
        let y = r.min.y + 0.5 + i as f32;
        p.line_segment([pos2(r.min.x + 2.0, y), pos2(r.max.x - 2.0, y)], Stroke::new(1.0, Color32::from_black_alpha(a)));
    }
    p.line_segment([pos2(r.min.x + 6.0, r.max.y - 0.5), pos2(r.max.x - 6.0, r.max.y - 0.5)], Stroke::new(1.0, Color32::from_white_alpha(14)));
    p.rect_stroke(r, CornerRadius::same(6), Stroke::new(1.0, Color32::from_black_alpha(160)), StrokeKind::Inside);
}

/// Vertical two-stop gradient (egui meshes carry per-vertex colour).
fn gradient(p: &egui::Painter, r: Rect, top: Color32, bottom: Color32) {
    let mut m = egui::Mesh::default();
    m.colored_vertex(r.left_top(), top);
    m.colored_vertex(r.right_top(), top);
    m.colored_vertex(r.left_bottom(), bottom);
    m.colored_vertex(r.right_bottom(), bottom);
    m.add_triangle(0, 1, 2);
    m.add_triangle(1, 3, 2);
    p.add(Shape::mesh(m));
}

/// Section size for a parameter group: up to four knob columns, two when it has switches.
fn section_size(name: &str, params: &[&'static ParamInfo]) -> (usize, usize, usize, egui::Vec2) {
    let switches = params.iter().filter(|p| matches!(p.unit, Unit::Toggle | Unit::Choice)).count();
    let knobs = params.len().saturating_sub(switches);
    let cols = knobs.clamp(1, 4).max(if switches > 0 { 2 } else { 1 });
    let rows = knobs.div_ceil(4);
    let label_h = if name.is_empty() { 8.0 } else { 26.0 };
    let h = label_h + 30.0 * switches.div_ceil(2) as f32 + rows as f32 * 96.0 + 6.0;
    (cols, rows, switches, vec2(cols as f32 * COL_W + 16.0, h))
}

/// The window width that fits every section on one row, capped so big plugins wrap.
pub fn width(info: &PluginInfo) -> f32 {
    let natural: f32 = groups(info.params).iter().map(|(n, ps)| section_size(n, ps).3.x + 8.0).sum::<f32>() + 12.0;
    let min = if info.id.starts_with("eq_") { 640.0 } else { 360.0 };
    natural.clamp(min, 760.0)
}

fn section(ui: &mut Ui, salt: (u64, usize), name: &str, params: &[&'static ParamInfo], ins: &Insert, out: &mut Out) {
    // Toggles and choices sit in a strip above the knobs.
    let (knobs, switches): (Vec<&'static ParamInfo>, Vec<_>) = params.iter().copied().partition(|p| !matches!(p.unit, Unit::Toggle | Unit::Choice));
    let (_, _, _, size) = section_size(name, params);
    let (r, _) = ui.allocate_exact_size(size, Sense::hover());
    plate(ui.painter(), r);
    let label_h = if name.is_empty() { 8.0 } else { 26.0 };
    if !name.is_empty() {
        let col = band_colour(name).unwrap_or(slate::TEXT_2);
        ui.painter().circle_filled(pos2(r.min.x + 14.0, r.min.y + 14.0), 3.5, col);
        ui.painter().text(pos2(r.min.x + 24.0, r.min.y + 14.0), Align2::LEFT_CENTER, name.to_uppercase(), bold(11.0), slate::TEXT_2);
    }
    let mut y = r.min.y + label_h;
    let cw = (size.x - 16.0) / 2.0;
    for (i, p) in switches.iter().enumerate() {
        let x = r.min.x + 8.0 + (i % 2) as f32 * cw;
        let rect = Rect::from_min_size(pos2(x + 2.0, y + (i / 2) as f32 * 30.0), vec2(cw - 4.0, 24.0));
        switch(ui, salt, name, p, ins, rect, out);
    }
    y += 30.0 * switches.len().div_ceil(2) as f32;
    for (i, p) in knobs.iter().enumerate() {
        let c = pos2(r.min.x + 8.0 + COL_W * ((i % 4) as f32 + 0.5), y + 96.0 * (i / 4) as f32 + 4.0 + KNOB * 0.5);
        knob(ui, salt, name, p, ins, c, out);
    }
}

fn value_of(p: &ParamInfo, ins: &Insert) -> f32 {
    ins.params.get(p.id).copied().unwrap_or(p.default)
}

/// LED colour by what the value means: boost green, cut blue, otherwise the accent.
fn led(p: &ParamInfo, v: f32) -> Color32 {
    match p.unit {
        Unit::Db if p.min < 0.0 && p.max > 0.0 && v > 0.05 => slate::SUCCESS,
        Unit::Db if p.min < 0.0 && p.max > 0.0 && v < -0.05 => slate::ACCENT_MUTED,
        Unit::Percent => slate::AI,
        _ => slate::ACCENT,
    }
}

fn band_colour(group: &str) -> Option<Color32> {
    let i = match group {
        "HPF" => 0,
        "LF" | "Low" => 1,
        "LMF" => 2,
        "MF" | "Mid" => 3,
        "HMF" => 4,
        "HF" | "High" => 5,
        "LPF" => 6,
        _ => return None,
    };
    slate::BANDS.get(i).copied()
}

/// Trimios knob: knurled cap, 270° LED ring (bipolar ranges fill from 12 o'clock), pointer,
/// caps label and a value readout. Drag up/down (Shift for fine), wheel nudges, double-click resets.
fn knob(ui: &mut Ui, salt: (u64, usize), group: &str, p: &'static ParamInfo, ins: &Insert, c: Pos2, out: &mut Out) {
    let v = value_of(p, ins);
    let rect = Rect::from_center_size(c, vec2(KNOB, KNOB));
    let resp = ui.interact(rect, ui.id().with(("tri-knob", salt, p.id)), Sense::click_and_drag());
    let mut t = p.to_normalized(v);
    if resp.dragged() {
        let fine = ui.input(|i| i.modifiers.shift);
        t = (t - resp.drag_delta().y * if fine { 0.0015 } else { 0.006 }).clamp(0.0, 1.0);
    }
    if resp.hovered() {
        let s = ui.input(|i| i.smooth_scroll_delta.y);
        if s.abs() > 0.0 {
            t = (t + s * 0.0008).clamp(0.0, 1.0);
        }
    }
    let mut nv = p.from_normalized(t);
    if resp.double_clicked() {
        nv = p.default;
    }
    if (nv - v).abs() > f32::EPSILON {
        out.edits.push((p.id, nv));
    }
    let v = nv;
    let t = p.to_normalized(v);
    let painter = ui.painter();
    let active = resp.dragged() || resp.hovered();
    let col = led(p, v);
    let r_led = KNOB * 0.452;
    let r_cap = KNOB * 0.388;
    let ang = |t: f32| -PI * 0.5 - PI * 0.75 + PI * 1.5 * t; // -135°..+135° around 12 o'clock
    let arc = |a0: f32, a1: f32, r: f32| -> Vec<Pos2> {
        let n = 40;
        (0..=n).map(|i| c + vec2((a0 + (a1 - a0) * i as f32 / n as f32).cos(), (a0 + (a1 - a0) * i as f32 / n as f32).sin()) * r).collect()
    };
    // LED track and lit segment.
    painter.add(Shape::line(arc(ang(0.0), ang(1.0), r_led), Stroke::new(3.0, Color32::from_white_alpha(13))));
    let origin = if p.min < 0.0 && p.max > 0.0 { p.to_normalized(0.0) } else { 0.0 };
    let (a0, a1) = if t >= origin { (ang(origin), ang(t)) } else { (ang(t), ang(origin)) };
    if (a1 - a0).abs() > 0.01 {
        if active {
            painter.add(Shape::line(arc(a0, a1, r_led), Stroke::new(7.0, col.gamma_multiply(0.18))));
        }
        painter.add(Shape::line(arc(a0, a1, r_led), Stroke::new(3.0, col)));
    }
    // Cap: shadow rim, body, knurled edge, top highlight.
    painter.circle_filled(c + vec2(0.0, 1.5), r_cap + 1.5, Color32::from_black_alpha(128));
    painter.circle_filled(c, r_cap, slate::BG);
    painter.circle_filled(c - vec2(0.0, r_cap * 0.12), r_cap * 0.86, slate::RAISED);
    painter.circle_filled(c - vec2(0.0, r_cap * 0.05), r_cap * 0.78, slate::SURFACE);
    for i in 0..28 {
        let a = 2.0 * PI * i as f32 / 28.0;
        let d = vec2(a.cos(), a.sin());
        painter.line_segment([c + d * r_cap * 0.84, c + d * r_cap * 0.98], Stroke::new(1.0, Color32::from_white_alpha(15)));
    }
    painter.circle_stroke(c, r_cap, Stroke::new(1.0, Color32::from_black_alpha(128)));
    let a = ang(t);
    let d = vec2(a.cos(), a.sin());
    painter.line_segment([c + d * r_cap * 0.3, c + d * r_cap * 0.82], Stroke::new((KNOB * 0.038).max(1.0) + 0.6, slate::TEXT));
    // Label and value box.
    painter.text(pos2(c.x, c.y + KNOB * 0.5 + 9.0), Align2::CENTER_CENTER, short(group, p.name).to_uppercase(), bold(9.5), slate::TEXT_MUTED);
    let vb = Rect::from_center_size(pos2(c.x, c.y + KNOB * 0.5 + 25.0), vec2(COL_W - 10.0, 18.0));
    painter.rect(
        vb,
        CornerRadius::same(4),
        slate::BAY,
        Stroke::new(1.0, if active { col.gamma_multiply(0.6) } else { slate::BORDER }),
        StrokeKind::Inside,
    );
    painter.text(vb.center(), Align2::CENTER_CENTER, p.format(v), mono(10.5), if active { slate::TEXT } else { slate::TEXT_2 });
    resp.on_hover_text(format!("{} — drag, Shift for fine, double-click resets", p.name));
}

/// Drop the section's name from a label ("LF Gain" in section LF → "Gain"); a label that is only
/// the section name stays ("HPF" toggle in section HPF → "On").
fn short<'a>(group: &str, name: &'a str) -> &'a str {
    if group.is_empty() {
        return name;
    }
    match name.strip_prefix(group) {
        Some("") => "On",
        Some(rest) => rest.trim_start(),
        None => name,
    }
}

/// Toggle (pill with accent ring when on) or choice (cycling pill with ‹ › arrows).
fn switch(ui: &mut Ui, salt: (u64, usize), group: &str, p: &'static ParamInfo, ins: &Insert, r: Rect, out: &mut Out) {
    let v = value_of(p, ins);
    let resp = ui.interact(r, ui.id().with(("tri-sw", salt, p.id)), Sense::click());
    let painter = ui.painter();
    match p.unit {
        Unit::Toggle => {
            let on = v >= 0.5;
            let fill = if on { slate::RAISED } else { slate::SURFACE };
            painter.rect(
                r,
                CornerRadius::same(12),
                fill,
                Stroke::new(if on { 1.5 } else { 1.0 }, if on { slate::ACCENT } else { slate::BORDER_STRONG }),
                StrokeKind::Inside,
            );
            painter.circle_filled(pos2(r.min.x + 12.0, r.center().y), 3.0, if on { slate::ACCENT } else { slate::TEXT_DISABLED });
            painter.text(
                pos2(r.min.x + 22.0, r.center().y),
                Align2::LEFT_CENTER,
                short(group, p.name).to_uppercase(),
                bold(10.0),
                if on { slate::TEXT } else { slate::TEXT_MUTED },
            );
            if resp.clicked() {
                out.edits.push((p.id, if on { 0.0 } else { 1.0 }));
            }
        }
        _ => {
            painter.rect(r, CornerRadius::same(12), slate::BAY, Stroke::new(1.0, slate::BORDER_STRONG), StrokeKind::Inside);
            painter.with_clip_rect(r.shrink(3.0)).text(r.center(), Align2::CENTER_CENTER, p.format(v), regular(10.5), slate::ACCENT_TEXT);
            let n = p.choices.len().max(1) as f32;
            if resp.clicked() {
                let left = resp.interact_pointer_pos().is_some_and(|q| q.x < r.center().x);
                let i = v.round().max(0.0);
                let ni = if left { (i - 1.0).rem_euclid(n) } else { (i + 1.0).rem_euclid(n) };
                out.edits.push((p.id, ni));
            }
        }
    }
    resp.on_hover_text(p.name);
}

/// One EQ band shown as a node on the graph: which parameters move it.
struct Node {
    freq: &'static str,
    gain: Option<&'static str>,
    colour: Color32,
    on: Option<&'static str>,
}

fn nodes(id: &str) -> Vec<Node> {
    let b = slate::BANDS;
    if id == "eq_1band" {
        return vec![Node { freq: "freq", gain: Some("gain"), colour: b[3], on: None }];
    }
    vec![
        Node { freq: "hpf_freq", gain: None, colour: b[0], on: Some("hpf_on") },
        Node { freq: "low_shelf_freq", gain: Some("low_shelf_gain"), colour: b[1], on: None },
        Node { freq: "low_mid_freq", gain: Some("low_mid_gain"), colour: b[2], on: None },
        Node { freq: "mid_freq", gain: Some("mid_gain"), colour: b[3], on: None },
        Node { freq: "high_mid_freq", gain: Some("high_mid_gain"), colour: b[4], on: None },
        Node { freq: "high_shelf_freq", gain: Some("high_shelf_gain"), colour: b[5], on: None },
        Node { freq: "lpf_freq", gain: None, colour: b[6], on: Some("lpf_on") },
    ]
}

const DB_RANGE: f32 = 24.0;

/// The EQ graph in a bay: frequency/dB grid, the response curve with a soft fill, and numbered
/// band nodes you can drag (x = frequency, y = gain).
fn eq_bay(ui: &mut Ui, salt: (u64, usize), info: &PluginInfo, ins: &Insert, out: &mut Out) {
    let w = ui.available_width().max(320.0);
    let (outer, _) = ui.allocate_exact_size(vec2(w, 210.0), Sense::hover());
    bay(ui.painter(), outer);
    let r = Rect::from_min_max(outer.min + vec2(34.0, 12.0), outer.max - vec2(34.0, 22.0));
    let fx = |f: f32| r.min.x + r.width() * ((f.max(20.0) / 20.0).ln() / 1000f32.ln()).clamp(0.0, 1.0);
    let xf = |x: f32| 20.0 * 1000f32.powf(((x - r.min.x) / r.width()).clamp(0.0, 1.0));
    let gy = |db: f32| r.center().y - db.clamp(-DB_RANGE, DB_RANGE) / DB_RANGE * r.height() * 0.5;
    let yg = |y: f32| ((r.center().y - y) / (r.height() * 0.5) * DB_RANGE).clamp(-DB_RANGE, DB_RANGE);
    let p = ui.painter().clone();
    let grid = Color32::from_white_alpha(18);
    for f in [50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0] {
        let x = fx(f);
        p.line_segment([pos2(x, r.min.y), pos2(x, r.max.y)], Stroke::new(1.0, grid));
        let l = if f >= 1000.0 { format!("{}k", f / 1000.0) } else { format!("{f}") };
        p.text(pos2(x, r.max.y + 10.0), Align2::CENTER_CENTER, l, mono(9.5), slate::TEXT_MUTED);
    }
    for db in [-24.0f32, -12.0, 0.0, 12.0, 24.0] {
        let y = gy(db);
        p.line_segment([pos2(r.min.x, y), pos2(r.max.x, y)], Stroke::new(1.0, if db == 0.0 { Color32::from_white_alpha(40) } else { grid }));
        let l = if db > 0.0 { format!("+{db}") } else { format!("{db}") };
        p.text(pos2(r.min.x - 6.0, y), Align2::RIGHT_CENTER, &l, mono(9.5), slate::TEXT_MUTED);
        p.text(pos2(r.max.x + 6.0, y), Align2::LEFT_CENTER, &l, mono(9.5), slate::TEXT_MUTED);
    }

    // Apply this frame's edits to a local copy so the curve follows a drag without lag.
    let mut params: Vec<(String, f32)> = ins.params.iter().map(|(k, v)| (k.clone(), *v)).collect();
    let get = |params: &Vec<(String, f32)>, id: &str| -> f32 {
        params.iter().find(|(k, _)| k == id).map(|(_, v)| *v).or_else(|| info.param(id).map(|q| q.default)).unwrap_or(0.0)
    };
    for (i, n) in nodes(info.id).iter().enumerate() {
        let f = get(&params, n.freq);
        let g = n.gain.map(|g| get(&params, g)).unwrap_or(0.0);
        let enabled = n.on.is_none_or(|o| get(&params, o) >= 0.5);
        let c = pos2(fx(f), gy(g));
        let hit = Rect::from_center_size(c, vec2(18.0, 18.0));
        let resp = ui.interact(hit, ui.id().with(("tri-node", salt, i)), Sense::click_and_drag());
        if resp.dragged()
            && let Some(q) = resp.interact_pointer_pos()
        {
            if let Some(pi) = info.param(n.freq) {
                let nf = pi.clamp(xf(q.x));
                out.edits.push((pi.id, nf));
                set(&mut params, pi.id, nf);
            }
            if let Some(pi) = n.gain.and_then(|g| info.param(g)) {
                let ng = pi.clamp(yg(q.y));
                out.edits.push((pi.id, ng));
                set(&mut params, pi.id, ng);
            }
        }
        if resp.double_clicked()
            && let Some(o) = n.on.and_then(|o| info.param(o))
        {
            out.edits.push((o.id, if enabled { 0.0 } else { 1.0 }));
        }
        resp.on_hover_text("Drag to move the band; double-click a filter to switch it");
    }

    let n = 200;
    let freqs: Vec<f32> = (0..n).map(|i| 20.0 * 1000f32.powf(i as f32 / (n - 1) as f32)).collect();
    let pv: Vec<(&str, f32)> = params.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    let resp =
        if info.id == "eq_7band" { soundcraft_dsp::eq7_response(&pv, &freqs, 48_000.0) } else { soundcraft_dsp::eq1_response(&pv, &freqs, 48_000.0) };
    let pts: Vec<Pos2> = resp.iter().zip(&freqs).map(|(db, f)| pos2(fx(*f), gy(*db))).collect();
    // Soft fill between the curve and 0 dB.
    let zero = gy(0.0);
    let fill = Color32::from_rgba_unmultiplied(0x60, 0xA5, 0xFA, 60);
    let mut m = egui::Mesh::default();
    for q in &pts {
        m.colored_vertex(*q, fill);
        m.colored_vertex(pos2(q.x, zero), Color32::from_rgba_unmultiplied(0x60, 0xA5, 0xFA, 10));
    }
    for i in 0..pts.len().saturating_sub(1) as u32 {
        let k = i * 2;
        m.add_triangle(k, k + 1, k + 2);
        m.add_triangle(k + 1, k + 3, k + 2);
    }
    p.add(Shape::mesh(m));
    p.add(Shape::line(pts.clone(), Stroke::new(5.0, Color32::from_rgba_unmultiplied(0x60, 0xA5, 0xFA, 40))));
    p.add(Shape::line(pts, Stroke::new(1.8, slate::ACCENT_HOVER)));

    for (i, nd) in nodes(info.id).iter().enumerate() {
        let f = get(&params, nd.freq);
        let g = nd.gain.map(|g| get(&params, g)).unwrap_or(0.0);
        let enabled = nd.on.is_none_or(|o| get(&params, o) >= 0.5);
        let c = pos2(fx(f), gy(g));
        let col = if enabled { nd.colour } else { slate::TEXT_DISABLED };
        p.circle_filled(c, 11.0, col.gamma_multiply(0.18));
        p.circle(c, 7.5, slate::BAY, Stroke::new(2.0, col));
        p.text(c, Align2::CENTER_CENTER, format!("{}", i + 1), bold(9.0), slate::TEXT);
    }
    // Output gain readout, top-right of the bay.
    if let Some(o) = info.param("output_gain") {
        let v = get(&params, o.id);
        p.text(pos2(r.min.x + 8.0, r.min.y + 4.0), Align2::LEFT_TOP, format!("OUT {}", o.format(v)), mono(10.0), slate::TEXT_2);
    }
}

fn set(params: &mut Vec<(String, f32)>, id: &str, v: f32) {
    match params.iter_mut().find(|(k, _)| k == id) {
        Some(e) => e.1 = v,
        None => params.push((id.to_string(), v)),
    }
}
