//! Room Reverb and Plate Reverb in the Trimio Slate design language: the core panel draws the
//! impulse response (pre-delay marker, early reflections, exponential tail) with draggable
//! pre-delay and decay nodes, a TAIL meter, parameter cards and the Trimio factory presets.

use crate::SoundApp;
use crate::theme::{bold, mono, regular};
use crate::trimio_shell::{Ctx, card, end_gesture, header, knob, palette, param_card, preset_row, seg_meter, set};
use crate::trimio_tokens::{TrimioPalette, radius, space};
use egui::{Align2, CornerRadius, CursorIcon, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use soundcraft_dsp::PluginInfo;
use soundcraft_model::{Insert, TrackId};

const G_CURVE: usize = 0;
const G_CARDS: usize = 1;
const G_EXTRA: usize = 2;
/// The TAIL meter's full scale (seconds).
const TAIL_FULL: f32 = 10.0;

/// Trimio factory presets (effects/reverb/spec.md §4).
fn presets() -> Vec<(&'static str, Vec<(&'static str, f32)>)> {
    vec![
        ("Large Concert Hall", vec![("mix", 25.0), ("decay", 2.8), ("pre_delay", 35.0), ("size", 85.0)]),
        ("Warm Studio Room", vec![("mix", 12.0), ("decay", 0.8), ("pre_delay", 10.0), ("size", 30.0)]),
        ("Plate Vocal Space", vec![("mix", 18.0), ("decay", 2.0), ("pre_delay", 15.0), ("size", 60.0)]),
    ]
}

/// Span of the time axis (seconds): the pre-delay plus the decay, at least 0.5 s.
fn span(pre_ms: f32, decay: f32) -> f32 {
    (pre_ms / 1000.0 + decay * 1.15).clamp(0.5, 24.0)
}

/// RT60 envelope: amplitude (0..1) `t` seconds after the tail starts.
fn envelope(t: f32, decay: f32) -> f32 {
    (-6.907_755 * t.max(0.0) / decay.max(0.05)).exp()
}

/// Display height of an amplitude: the square root, so the tail reads like the ear hears it
/// instead of collapsing into the first few milliseconds.
fn shown(a: f32) -> f32 {
    a.max(0.0).sqrt()
}

/// Draws the whole editor into the plugin window.
pub fn editor(app: &mut SoundApp, ui: &mut Ui, tid: TrackId, slot: usize, info: &'static PluginInfo, ins: &Insert) {
    let c = Ctx { info, ins, tid, slot };
    let p = palette();
    ui.spacing_mut().item_spacing = vec2(space::S2, space::S2);
    if ins.bypass {
        ui.set_opacity(0.5);
    }
    let title = if info.id == "plate_reverb" { "Plate Reverb" } else { "Reverb" };
    header(app, ui, &c, &p, title);
    ui.add_space(space::S2);
    card(ui, &p, |ui| {
        ui.horizontal_top(|ui| {
            impulse(app, ui, &c, &p);
            ui.add_space(space::S2);
            let decay = c.value("decay");
            seg_meter(ui, &p, "TAIL", decay / TAIL_FULL, &format!("{decay:.1} s"), false);
            ui.add_space(space::S3);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    param_card(app, ui, &c, G_CARDS, "mix", "Mix", &p);
                    param_card(app, ui, &c, G_CARDS, "decay", "Decay", &p);
                    param_card(app, ui, &c, G_CARDS, "pre_delay", "Pre-Delay", &p);
                });
                ui.horizontal(|ui| {
                    param_card(app, ui, &c, G_CARDS, "size", "Room Size", &p);
                    param_card(app, ui, &c, G_CARDS, "diffusion", "Diffusion", &p);
                    param_card(app, ui, &c, G_CARDS, "width", "Width", &p);
                });
            });
        });
        ui.add_space(space::S3);
        preset_row(app, ui, &c, &p, &presets());
    });
    ui.add_space(space::S3);
    card(ui, &p, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("TONE").font(bold(11.0)).color(p.text_muted));
            ui.add_space(space::S4);
            knob(app, ui, &c, G_EXTRA, "damping", "HF Damping", p.accent, &p);
            knob(app, ui, &c, G_EXTRA, "low_cut", "Low Cut", p.accent, &p);
            ui.add_space(space::S6);
            ui.vertical(|ui| {
                ui.set_min_width(170.0);
                ui.add_space(space::S6);
                ui.label(egui::RichText::new(format!("RT60 {:.2} s", c.value("decay"))).font(mono(11.0)).color(p.text_secondary));
                ui.label(egui::RichText::new("Latency 0 samples").font(mono(11.0)).color(p.text_muted));
            });
        });
    });
}

fn impulse(app: &mut SoundApp, ui: &mut Ui, c: &Ctx, p: &TrimioPalette) {
    let (outer, _) = ui.allocate_exact_size(vec2(296.0, 288.0), Sense::hover());
    let painter = ui.painter_at(outer);
    painter.rect_filled(outer, CornerRadius::same(radius::LG as u8), p.workspace_bg);
    painter.rect_stroke(outer, CornerRadius::same(radius::LG as u8), Stroke::new(1.0, p.border_subtle), StrokeKind::Inside);
    let r = Rect::from_min_max(outer.min + vec2(space::S3, space::S6), outer.max - vec2(space::S3, 26.0));

    let pre_ms = c.value("pre_delay");
    let decay = c.value("decay");
    let size = c.value("size") / 100.0;
    let diffusion = c.value("diffusion") / 100.0;
    let total = span(pre_ms, decay);
    let x_of = |t: f32| r.min.x + r.width() * (t / total).clamp(0.0, 1.0);
    let y_of = |a: f32| r.max.y - r.height() * a.clamp(0.0, 1.0);

    // Grid: quarter divisions, time labels.
    for k in 0..=4 {
        let f = k as f32 / 4.0;
        let x = r.min.x + r.width() * f;
        let y = r.min.y + r.height() * f;
        painter.line_segment([pos2(x, r.min.y), pos2(x, r.max.y)], Stroke::new(1.0, p.border_subtle));
        painter.line_segment([pos2(r.min.x, y), pos2(r.max.x, y)], Stroke::new(1.0, p.border_subtle));
        let t = total * f;
        let label = if t < 1.0 { format!("{:.0} ms", t * 1000.0) } else { format!("{t:.1} s") };
        painter.text(pos2(x, r.max.y + space::S1), if k == 4 { Align2::RIGHT_TOP } else { Align2::CENTER_TOP }, label, regular(10.0), p.text_muted);
    }

    let t0 = pre_ms / 1000.0;
    let x0 = x_of(t0);
    // Pre-delay marker.
    painter.add(egui::Shape::dashed_line(&[pos2(x0, r.min.y - space::S3), pos2(x0, r.max.y)], Stroke::new(1.0, p.warning), 3.0, 3.0));
    painter.text(pos2(x0 + space::S1, r.min.y - space::S3), Align2::LEFT_TOP, "Pre-delay", regular(10.0), p.warning);

    // Early reflections: spacing grows with room size, density with diffusion.
    let n_refl = 4 + (diffusion * 6.0).round() as usize;
    let er_span = ((0.06 + 0.16 * size) * total).min(total - t0);
    for i in 0..n_refl {
        let f = (i as f32 + 1.0) / (n_refl as f32 + 1.0);
        let t = t0 + er_span * f.powf(0.8);
        let a = shown(envelope(t - t0, decay)) * (0.98 - 0.25 * f);
        painter.line_segment([pos2(x_of(t), r.max.y), pos2(x_of(t), y_of(a))], Stroke::new(1.5, p.accent.gamma_multiply(0.75)));
    }
    // Exponential tail, with a flat fill (≤ 18%).
    let n = 120;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            let t = t0 + (total - t0) * i as f32 / n as f32;
            pos2(x_of(t), y_of(shown(envelope(t - t0, decay))))
        })
        .collect();
    let fill = p.accent.gamma_multiply(0.14);
    for w in pts.windows(2) {
        if let [a, b] = w {
            painter.add(egui::Shape::convex_polygon(vec![pos2(a.x, r.max.y), *a, *b, pos2(b.x, r.max.y)], fill, Stroke::NONE));
        }
    }
    painter.add(egui::Shape::line(pts, Stroke::new(2.2, p.accent)));

    // Nodes: pre-delay (top of the tail; drag horizontally) and decay (where the tail reaches −60 dB).
    let pre_pos = pos2(x0, y_of(1.0));
    let dec_pos = pos2(x_of(t0 + decay), y_of(0.0));
    let pre_resp =
        ui.interact(Rect::from_center_size(pre_pos, vec2(22.0, 22.0)), ui.id().with(("trimio_rev_pre", c.tid.0, c.slot)), Sense::click_and_drag());
    let dec_resp =
        ui.interact(Rect::from_center_size(dec_pos, vec2(22.0, 22.0)), ui.id().with(("trimio_rev_dec", c.tid.0, c.slot)), Sense::click_and_drag());
    let fine = ui.input(|i| i.modifiers.alt || i.modifiers.shift);
    let k = if fine { 0.25 } else { 1.0 };
    let secs_per_px = total / r.width().max(1.0);
    if pre_resp.dragged() {
        set(app, c, G_CURVE, &[("pre_delay", pre_ms + pre_resp.drag_delta().x * secs_per_px * 1000.0 * k)]);
    }
    if dec_resp.dragged() {
        set(app, c, G_CURVE, &[("decay", decay + dec_resp.drag_delta().x * secs_per_px * k)]);
    }
    if pre_resp.drag_stopped() || dec_resp.drag_stopped() {
        end_gesture(app);
    }
    for (resp, id) in [(&pre_resp, "pre_delay"), (&dec_resp, "decay")] {
        if resp.double_clicked() {
            set(app, c, G_CURVE, &[(id, c.param(id).map_or(0.0, |q| q.default))]);
            end_gesture(app);
        }
    }
    for (resp, pos, color, tip) in
        [(&pre_resp, pre_pos, p.warning, format!("Pre-delay {pre_ms:.0} ms")), (&dec_resp, dec_pos, p.accent, format!("Decay {decay:.2} s"))]
    {
        let active = resp.hovered() || resp.dragged();
        if active {
            ui.ctx().set_cursor_icon(if resp.dragged() { CursorIcon::Grabbing } else { CursorIcon::ResizeHorizontal });
            let galley = painter.layout_no_wrap(tip, mono(11.0), p.text_primary);
            let at = pos2((pos.x + space::S2).min(r.max.x - galley.size().x - space::S3), (pos.y - galley.size().y - space::S4).max(r.min.y));
            let box_r = Rect::from_min_size(at, galley.size() + vec2(space::S3, space::S2));
            painter.rect_filled(box_r, CornerRadius::same(radius::MD as u8), p.surface_overlay);
            painter.galley(box_r.min + vec2(space::S1 + 2.0, space::S1), galley, p.text_primary);
        }
        let rad = if active { 6.5 } else { 5.0 };
        painter.circle_filled(pos, rad, color);
        painter.circle_stroke(pos, rad + 1.0, Stroke::new(1.5, p.workspace_bg));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_reaches_minus_60_db_at_decay() {
        assert!((envelope(0.0, 2.0) - 1.0).abs() < 1e-6);
        assert!((envelope(2.0, 2.0) - 0.001).abs() < 1e-4);
        assert!(envelope(-1.0, 2.0) <= 1.0);
        assert!(envelope(1.0, 0.0).is_finite());
    }

    #[test]
    fn span_covers_pre_delay_and_decay() {
        assert!(span(100.0, 2.0) > 2.1);
        assert!(span(0.0, 0.01) >= 0.5);
    }

    #[test]
    fn presets_name_real_parameters() {
        for info in [&soundcraft_dsp::plugins::reverb::ROOM_INFO, &soundcraft_dsp::plugins::reverb::PLATE_INFO] {
            let ids: Vec<&str> = info.params.iter().map(|p| p.id).collect();
            for (_, values) in presets() {
                for (id, _) in values {
                    assert!(ids.contains(&id), "{id}");
                }
            }
        }
    }
}
