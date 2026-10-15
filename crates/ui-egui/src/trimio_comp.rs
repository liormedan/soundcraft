//! The Compressor/Limiter editor in the Trimio Slate design language: a transfer curve (input →
//! output, 1:1 reference, amber threshold marker, draggable threshold and ratio nodes), an
//! always-visible gain-reduction meter, parameter cards and the Trimio factory presets, inside the
//! shared [`crate::trimio_shell`].

use crate::SoundApp;
use crate::theme::{bold, mono, regular};
use crate::trimio_shell::{Ctx, card, end_gesture, header, knob, palette, param_card, pill, preset_row, seg_meter, set};
use crate::trimio_tokens::{TrimioPalette, radius, space};
use egui::{Align2, CornerRadius, CursorIcon, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use soundcraft_dsp::PluginInfo;
use soundcraft_model::{Insert, TrackId};

/// The graph's dB range on both axes.
const DB_MIN: f32 = -60.0;
/// Ratio drawn for the limiter (the plugin's ratio range ends at 100:1).
const LIMIT_RATIO: f32 = 100.0;
/// Gesture groups for undo merging (one per control family).
const G_CURVE: usize = 0;
const G_CARDS: usize = 1;
const G_EXTRA: usize = 2;

/// Trimio factory presets (effects/compressor/spec.md §4).
const PRESETS: [(&str, [(&str, f32); 5]); 3] = [
    ("Punchy Drums", [("threshold", -15.0), ("ratio", 4.0), ("attack", 25.0), ("release", 80.0), ("makeup", 3.0)]),
    ("Smooth Vocal", [("threshold", -20.0), ("ratio", 2.5), ("attack", 10.0), ("release", 150.0), ("makeup", 2.5)]),
    ("Bus Glue", [("threshold", -8.0), ("ratio", 1.5), ("attack", 30.0), ("release", 300.0), ("makeup", 1.0)]),
];

/// Static soft-knee transfer curve: output level (dB) for an input level (dB), before makeup.
fn transfer(x: f32, threshold: f32, ratio: f32, knee: f32) -> f32 {
    let ratio = ratio.max(1.0);
    let over = x - threshold;
    if knee > 0.0 && (2.0 * over).abs() <= knee {
        x + (1.0 / ratio - 1.0) * (over + knee / 2.0).powi(2) / (2.0 * knee)
    } else if 2.0 * over > knee {
        threshold + over / ratio
    } else {
        x
    }
}

fn to_x(db: f32, r: Rect) -> f32 {
    r.min.x + r.width() * ((db - DB_MIN) / -DB_MIN).clamp(0.0, 1.0)
}

fn to_y(db: f32, r: Rect) -> f32 {
    r.max.y - r.height() * ((db - DB_MIN) / -DB_MIN).clamp(0.0, 1.0)
}

fn from_x(x: f32, r: Rect) -> f32 {
    DB_MIN + -DB_MIN * ((x - r.min.x) / r.width().max(1.0)).clamp(0.0, 1.0)
}

fn from_y(y: f32, r: Rect) -> f32 {
    DB_MIN + -DB_MIN * ((r.max.y - y) / r.height().max(1.0)).clamp(0.0, 1.0)
}

/// Draws the whole editor into the plugin window.
pub fn editor(app: &mut SoundApp, ui: &mut Ui, tid: TrackId, slot: usize, info: &'static PluginInfo, ins: &Insert) {
    let c = Ctx { info, ins, tid, slot };
    let p = palette();
    ui.spacing_mut().item_spacing = vec2(space::S2, space::S2);
    if ins.bypass {
        ui.set_opacity(0.5);
    }
    let gr = if ins.bypass { 0.0 } else { app.meters.get(&tid).map_or(0.0, |m| m.gr.max(0.0)) };
    header(app, ui, &c, &p, "Compressor");
    ui.add_space(space::S2);
    card(ui, &p, |ui| {
        ui.horizontal_top(|ui| {
            curve(app, ui, &c, &p);
            ui.add_space(space::S2);
            let readout = if gr > 0.05 { format!("-{gr:.1}") } else { "0.0".into() };
            seg_meter(ui, &p, "GR", gr / 24.0, &readout, true);
            ui.add_space(space::S3);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    param_card(app, ui, &c, G_CARDS, "threshold", "Threshold", &p);
                    param_card(app, ui, &c, G_CARDS, "ratio", "Ratio", &p);
                    param_card(app, ui, &c, G_CARDS, "attack", "Attack", &p);
                });
                ui.horizontal(|ui| {
                    param_card(app, ui, &c, G_CARDS, "release", "Release", &p);
                    param_card(app, ui, &c, G_CARDS, "knee", "Knee", &p);
                    param_card(app, ui, &c, G_CARDS, "makeup", "Makeup", &p);
                });
            });
        });
        ui.add_space(space::S3);
        let presets: Vec<(&str, Vec<(&str, f32)>)> = PRESETS.iter().map(|(n, v)| (*n, v.to_vec())).collect();
        preset_row(app, ui, &c, &p, &presets);
    });
    ui.add_space(space::S3);
    extras(app, ui, &c, &p, gr);
}

fn curve(app: &mut SoundApp, ui: &mut Ui, c: &Ctx, p: &TrimioPalette) {
    let side = 260.0;
    let (outer, _) = ui.allocate_exact_size(vec2(side + 36.0, side + 28.0), Sense::hover());
    let painter = ui.painter_at(outer);
    painter.rect_filled(outer, CornerRadius::same(radius::LG as u8), p.workspace_bg);
    painter.rect_stroke(outer, CornerRadius::same(radius::LG as u8), Stroke::new(1.0, p.border_subtle), StrokeKind::Inside);
    let r = Rect::from_min_size(outer.min + vec2(30.0, space::S2), vec2(side, side));

    let limiter = c.value("limiter") >= 0.5;
    let threshold = c.value("threshold");
    let ratio = if limiter { LIMIT_RATIO } else { c.value("ratio") };
    let knee = c.value("knee");

    // Grid every 12 dB, labelled on the input axis and the output axis.
    let mut db = DB_MIN;
    while db <= 0.1 {
        let (x, y) = (to_x(db, r), to_y(db, r));
        painter.line_segment([pos2(x, r.min.y), pos2(x, r.max.y)], Stroke::new(1.0, p.border_subtle));
        painter.line_segment([pos2(r.min.x, y), pos2(r.max.x, y)], Stroke::new(1.0, p.border_subtle));
        painter.text(pos2(r.min.x - space::S1, y), Align2::RIGHT_CENTER, format!("{db:.0}"), regular(10.0), p.text_muted);
        painter.text(pos2(x, r.max.y + space::S1), Align2::CENTER_TOP, format!("{db:.0}"), regular(10.0), p.text_muted);
        db += 12.0;
    }
    // 1:1 reference.
    painter.add(egui::Shape::dashed_line(
        &[pos2(r.min.x, r.max.y), pos2(r.max.x, r.min.y)],
        Stroke::new(1.0, p.text_secondary.gamma_multiply(0.5)),
        4.0,
        4.0,
    ));
    // Threshold marker.
    let tx = to_x(threshold, r);
    painter.add(egui::Shape::dashed_line(&[pos2(tx, r.min.y), pos2(tx, r.max.y)], Stroke::new(1.0, p.warning), 3.0, 3.0));

    // The curve, with a flat 10% accent fill beneath it.
    let n = 120;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            let x = DB_MIN + -DB_MIN * i as f32 / n as f32;
            pos2(to_x(x, r), to_y(transfer(x, threshold, ratio, knee), r))
        })
        .collect();
    let fill = p.accent.gamma_multiply(0.10);
    for w in pts.windows(2) {
        if let [a, b] = w {
            painter.add(egui::Shape::convex_polygon(vec![pos2(a.x, r.max.y), *a, *b, pos2(b.x, r.max.y)], fill, Stroke::NONE));
        }
    }
    painter.add(egui::Shape::line(pts, Stroke::new(2.2, p.accent)));

    // Nodes: threshold (drag along the input axis) and ratio (right edge, drag vertically).
    let t_pos = pos2(tx, to_y(transfer(threshold, threshold, ratio, knee), r));
    let end_out = transfer(0.0, threshold, ratio, knee);
    let r_pos = pos2(r.max.x, to_y(end_out, r));
    let t_resp =
        ui.interact(Rect::from_center_size(t_pos, vec2(22.0, 22.0)), ui.id().with(("trimio_comp_t", c.tid.0, c.slot)), Sense::click_and_drag());
    let r_resp =
        ui.interact(Rect::from_center_size(r_pos, vec2(22.0, 22.0)), ui.id().with(("trimio_comp_r", c.tid.0, c.slot)), Sense::click_and_drag());
    let fine = ui.input(|i| i.modifiers.alt || i.modifiers.shift);
    if t_resp.dragged()
        && let Some(ptr) = t_resp.interact_pointer_pos()
    {
        let v = if fine { threshold + t_resp.drag_delta().x * 0.05 } else { from_x(ptr.x, r).round() };
        set(app, c, G_CURVE, &[("threshold", v)]);
    }
    if r_resp.dragged()
        && let Some(ptr) = r_resp.interact_pointer_pos()
        && !limiter
    {
        // Output at 0 dB input = threshold + (0 − threshold)/ratio  ⇒  ratio = −threshold / (out − threshold).
        let out = from_y(ptr.y, r).max(threshold + 0.05);
        let new_ratio = if fine { ratio * (r_resp.drag_delta().y * 0.01).exp() } else { (-threshold) / (out - threshold) };
        set(app, c, G_CURVE, &[("ratio", new_ratio)]);
    }
    if t_resp.drag_stopped() || r_resp.drag_stopped() {
        end_gesture(app);
    }
    if t_resp.double_clicked() {
        set(app, c, G_CURVE, &[("threshold", c.param("threshold").map_or(-20.0, |q| q.default))]);
        end_gesture(app);
    }
    if r_resp.double_clicked() {
        set(app, c, G_CURVE, &[("ratio", c.param("ratio").map_or(4.0, |q| q.default))]);
        end_gesture(app);
    }
    for (resp, pos, color, tip) in [
        (&t_resp, t_pos, p.warning, format!("Threshold {threshold:.1} dB")),
        (&r_resp, r_pos, p.accent, if limiter { "Limiter (∞:1)".into() } else { format!("Ratio {ratio:.1}:1") }),
    ] {
        let active = resp.hovered() || resp.dragged();
        if active {
            ui.ctx().set_cursor_icon(if resp.dragged() { CursorIcon::Grabbing } else { CursorIcon::Grab });
            let galley = painter.layout_no_wrap(tip, mono(11.0), p.text_primary);
            let at = pos - vec2(galley.size().x + space::S4, galley.size().y + space::S3);
            let at = pos2(at.x.max(r.min.x), at.y.max(r.min.y));
            let box_r = Rect::from_min_size(at, galley.size() + vec2(space::S3, space::S2));
            painter.rect_filled(box_r, CornerRadius::same(radius::MD as u8), p.surface_overlay);
            painter.galley(box_r.min + vec2(space::S1 + 2.0, space::S1), galley, p.text_primary);
        }
        let rad = if active { 6.5 } else { 5.0 };
        painter.circle_filled(pos, rad, color);
        painter.circle_stroke(pos, rad + 1.0, Stroke::new(1.5, p.workspace_bg));
    }
}

/// Limiter, lookahead, sidechain filter and mix, plus the output readouts.
fn extras(app: &mut SoundApp, ui: &mut Ui, c: &Ctx, p: &TrimioPalette, gr: f32) {
    card(ui, p, |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("DETECTOR").font(bold(11.0)).color(p.text_muted));
                ui.add_space(space::S2);
                for (id, label, tip) in [("limiter", "Limit", "Brick-wall limiting (∞:1)"), ("sc_hpf_on", "SC HPF", "Sidechain high-pass filter")] {
                    let on = c.value(id) >= 0.5;
                    if pill(ui, p, label, on, tip).clicked() {
                        set(app, c, G_EXTRA, &[(id, if on { 0.0 } else { 1.0 })]);
                        end_gesture(app);
                    }
                }
            });
            ui.add_space(space::S3);
            if c.value("sc_hpf_on") >= 0.5 {
                knob(app, ui, c, G_EXTRA, "sc_hpf_freq", "SC Freq", p.accent, p);
            }
            knob(app, ui, c, G_EXTRA, "lookahead", "Lookahead", p.accent, p);
            knob(app, ui, c, G_EXTRA, "mix", "Mix", p.accent, p);
            ui.add_space(space::S6);
            ui.vertical(|ui| {
                ui.set_min_width(170.0);
                ui.add_space(space::S6);
                ui.label(egui::RichText::new(format!("Gain reduction {gr:.1} dB")).font(mono(11.0)).color(p.text_secondary));
                ui.label(egui::RichText::new(format!("Latency {:.1} ms", c.value("lookahead"))).font(mono(11.0)).color(p.text_muted));
            });
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfer_curve_is_unity_below_and_ratio_above() {
        // Hard knee: unity below threshold, slope 1/ratio above.
        assert!((transfer(-40.0, -20.0, 4.0, 0.0) - -40.0).abs() < 1e-4);
        assert!((transfer(0.0, -20.0, 4.0, 0.0) - -15.0).abs() < 1e-4);
        // Soft knee meets both segments continuously at its edges.
        let (t, r, k) = (-20.0, 4.0, 6.0);
        assert!((transfer(t - k / 2.0, t, r, k) - (t - k / 2.0)).abs() < 1e-3);
        assert!((transfer(t + k / 2.0, t, r, k) - (t + k / 2.0 / r)).abs() < 1e-3);
        // Nonsense ratio never expands.
        assert!(transfer(0.0, -20.0, 0.0, 0.0) <= 0.0);
    }

    #[test]
    fn axes_round_trip() {
        let r = Rect::from_min_size(pos2(5.0, 7.0), vec2(260.0, 260.0));
        for db in [-60.0f32, -33.3, -12.0, 0.0] {
            assert!((from_x(to_x(db, r), r) - db).abs() < 1e-3);
            assert!((from_y(to_y(db, r), r) - db).abs() < 1e-3);
        }
    }

    #[test]
    fn presets_name_real_parameters() {
        let ids: Vec<&str> = soundcraft_dsp::plugins::dynamics::COMP_INFO.params.iter().map(|p| p.id).collect();
        for (_, values) in PRESETS {
            for (id, _) in values {
                assert!(ids.contains(&id), "{id}");
            }
        }
    }
}
