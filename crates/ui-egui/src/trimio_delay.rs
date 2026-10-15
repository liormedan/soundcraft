//! Mod Delay in the Trimio Slate design language: the core panel draws the dry hit and the echo
//! taps decaying by feedback along a time axis (drag the first tap for time, the second for
//! feedback), tempo-synced note values, an FB meter, parameter cards and the Trimio presets.

use crate::SoundApp;
use crate::theme::{bold, mono, regular};
use crate::trimio_shell::{Ctx, card, end_gesture, header, palette, param_card, pill, preset_row, seg_meter, set};
use crate::trimio_tokens::{TrimioPalette, radius, space};
use egui::{Align2, CornerRadius, CursorIcon, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use soundcraft_dsp::PluginInfo;
use soundcraft_model::{Insert, TrackId};

const G_CURVE: usize = 0;
const G_CARDS: usize = 1;
const G_EXTRA: usize = 2;
/// Echoes drawn after the dry hit.
const TAPS: usize = 7;

/// Note values as fractions of a quarter note.
const NOTES: [(&str, f32); 6] = [("1/4", 1.0), ("1/8D", 0.75), ("1/8", 0.5), ("1/8T", 1.0 / 3.0), ("1/16", 0.25), ("1/16D", 0.375)];

fn note_ms(bpm: f32, quarters: f32) -> f32 {
    60_000.0 / bpm.max(1.0) * quarters
}

/// The note value `ms` matches at `bpm` (within 1%), if any.
fn note_name(ms: f32, bpm: f32) -> Option<&'static str> {
    NOTES.iter().find(|(_, q)| (note_ms(bpm, *q) / ms.max(0.001) - 1.0).abs() < 0.01).map(|(n, _)| *n)
}

/// Trimio factory presets (effects/delay/spec.md §4); note values resolve at the session tempo.
/// SoundCraft's delay filters with a high-cut (`lpf`), not the spec's low-cut, so that is left out.
fn presets(bpm: f32) -> Vec<(&'static str, Vec<(&'static str, f32)>)> {
    vec![
        ("Eighth Note Echo", vec![("time", note_ms(bpm, 0.5)), ("feedback", 40.0), ("mix", 20.0)]),
        ("Slapback Echo", vec![("time", 85.0), ("feedback", 8.0), ("mix", 25.0)]),
        ("Dotted Eighth Lead", vec![("time", note_ms(bpm, 0.75)), ("feedback", 45.0), ("mix", 30.0)]),
    ]
}

fn session_bpm(app: &SoundApp) -> f32 {
    let s = app.engine.session();
    let tick = s.tempo.samples_to_ticks(app.position(), s.sample_rate);
    (s.tempo.tempo_at_tick(tick) as f32).clamp(20.0, 400.0)
}

/// Draws the whole editor into the plugin window.
pub fn editor(app: &mut SoundApp, ui: &mut Ui, tid: TrackId, slot: usize, info: &'static PluginInfo, ins: &Insert) {
    let c = Ctx { info, ins, tid, slot };
    let p = palette();
    let bpm = session_bpm(app);
    ui.spacing_mut().item_spacing = vec2(space::S2, space::S2);
    if ins.bypass {
        ui.set_opacity(0.5);
    }
    header(app, ui, &c, &p, "Delay");
    ui.add_space(space::S2);
    card(ui, &p, |ui| {
        ui.horizontal_top(|ui| {
            taps(app, ui, &c, &p, bpm);
            ui.add_space(space::S2);
            let fb = c.value("feedback");
            seg_meter(ui, &p, "FB", fb.abs() / 100.0, &format!("{fb:.0}%"), false);
            ui.add_space(space::S3);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    param_card(app, ui, &c, G_CARDS, "time", "Time", &p);
                    param_card(app, ui, &c, G_CARDS, "feedback", "Feedback", &p);
                    param_card(app, ui, &c, G_CARDS, "mix", "Mix", &p);
                });
                ui.horizontal(|ui| {
                    param_card(app, ui, &c, G_CARDS, "lpf", "High Cut", &p);
                    param_card(app, ui, &c, G_CARDS, "mod_rate", "Mod Rate", &p);
                    param_card(app, ui, &c, G_CARDS, "mod_depth", "Mod Depth", &p);
                });
            });
        });
        ui.add_space(space::S3);
        preset_row(app, ui, &c, &p, &presets(bpm));
    });
    ui.add_space(space::S3);
    card(ui, &p, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("SYNC").font(bold(11.0)).color(p.text_muted));
            ui.add_space(space::S2);
            let current = note_name(c.value("time"), bpm);
            for (name, q) in NOTES {
                if pill(ui, &p, name, current == Some(name), "Set the time to this note value at the session tempo").clicked() {
                    set(app, &c, G_EXTRA, &[("time", note_ms(bpm, q))]);
                    end_gesture(app);
                }
            }
            ui.add_space(space::S4);
            ui.vertical(|ui| {
                ui.set_min_width(170.0);
                ui.label(egui::RichText::new(format!("Tempo {bpm:.1} BPM")).font(mono(11.0)).color(p.text_secondary));
                ui.label(egui::RichText::new("Latency 0 samples").font(mono(11.0)).color(p.text_muted));
            });
        });
    });
}

fn taps(app: &mut SoundApp, ui: &mut Ui, c: &Ctx, p: &TrimioPalette, bpm: f32) {
    let (outer, _) = ui.allocate_exact_size(vec2(296.0, 288.0), Sense::hover());
    let painter = ui.painter_at(outer);
    painter.rect_filled(outer, CornerRadius::same(radius::LG as u8), p.workspace_bg);
    painter.rect_stroke(outer, CornerRadius::same(radius::LG as u8), Stroke::new(1.0, p.border_subtle), StrokeKind::Inside);
    let r = Rect::from_min_max(outer.min + vec2(space::S4, space::S6), outer.max - vec2(space::S4, 26.0));

    let time = c.value("time");
    let fb = c.value("feedback") / 100.0;
    let mix = c.value("mix") / 100.0;
    let total = time * (TAPS as f32 + 0.6);
    let x_of = |ms: f32| r.min.x + r.width() * (ms / total.max(1.0)).clamp(0.0, 1.0);
    let y_of = |a: f32| r.max.y - r.height() * a.clamp(0.0, 1.0);

    for k in 0..=4 {
        let f = k as f32 / 4.0;
        let y = r.min.y + r.height() * f;
        painter.line_segment([pos2(r.min.x, y), pos2(r.max.x, y)], Stroke::new(1.0, p.border_subtle));
    }
    // Beat grid at the session tempo, labelled in ms.
    let beat = note_ms(bpm, 1.0);
    let mut t = 0.0;
    while t <= total && beat > 1.0 {
        painter.line_segment([pos2(x_of(t), r.min.y), pos2(x_of(t), r.max.y)], Stroke::new(1.0, p.border_subtle));
        t += beat;
    }
    painter.text(pos2(r.min.x, r.max.y + space::S1), Align2::LEFT_TOP, "0", regular(10.0), p.text_muted);
    painter.text(pos2(r.max.x, r.max.y + space::S1), Align2::RIGHT_TOP, format!("{:.0} ms", total), regular(10.0), p.text_muted);

    // Dry hit, then echoes: tap k at k·time with level mix·|fb|^(k−1); negative feedback flips polarity (dashed).
    let dry = pos2(x_of(0.0) + 3.0, y_of(1.0));
    painter.line_segment([pos2(dry.x, r.max.y), dry], Stroke::new(2.0, p.text_secondary));
    painter.circle_filled(dry, 3.5, p.text_secondary);
    painter.text(dry + vec2(6.0, -2.0), Align2::LEFT_BOTTOM, "dry", regular(10.0), p.text_muted);
    let mut tap_pos = Vec::new();
    for k in 1..=TAPS {
        let a = mix * fb.abs().powi(k as i32 - 1);
        let top = pos2(x_of(time * k as f32), y_of(a.max(0.004)));
        let flipped = fb < 0.0 && k % 2 == 0;
        let col = p.accent.gamma_multiply((1.0 - 0.08 * k as f32).max(0.45));
        if flipped {
            painter.add(egui::Shape::dashed_line(&[pos2(top.x, r.max.y), top], Stroke::new(2.0, col), 3.0, 2.0));
        } else {
            painter.line_segment([pos2(top.x, r.max.y), top], Stroke::new(2.0, col));
        }
        painter.circle_filled(top, 3.5, col);
        tap_pos.push(top);
    }

    // Tap 1: drag horizontally for time. Tap 2: drag vertically for feedback.
    let (Some(first), Some(second)) = (tap_pos.first().copied(), tap_pos.get(1).copied()) else { return };
    let t_resp =
        ui.interact(Rect::from_center_size(first, vec2(22.0, 22.0)), ui.id().with(("trimio_dly_t", c.tid.0, c.slot)), Sense::click_and_drag());
    let f_resp =
        ui.interact(Rect::from_center_size(second, vec2(22.0, 22.0)), ui.id().with(("trimio_dly_f", c.tid.0, c.slot)), Sense::click_and_drag());
    let fine = ui.input(|i| i.modifiers.alt || i.modifiers.shift);
    let k = if fine { 0.25 } else { 1.0 };
    if t_resp.dragged() {
        let ms_per_px = total / r.width().max(1.0);
        let mut nt = time + t_resp.drag_delta().x * ms_per_px * k;
        if !fine && let Some((_, q)) = NOTES.iter().find(|(_, q)| (note_ms(bpm, *q) / nt.max(0.001) - 1.0).abs() < 0.02) {
            nt = note_ms(bpm, *q);
        }
        set(app, c, G_CURVE, &[("time", nt)]);
    }
    if f_resp.dragged() && mix > 0.001 {
        // Tap 2's height is mix·|fb|: invert the pointer's height into a feedback percentage, keeping the sign.
        let a =
            if let Some(ptr) = f_resp.interact_pointer_pos() { ((r.max.y - ptr.y) / r.height().max(1.0)).clamp(0.0, 1.0) } else { mix * fb.abs() };
        let new_fb = (a / mix).clamp(0.0, 0.99) * 100.0 * if fb < 0.0 { -1.0 } else { 1.0 };
        set(app, c, G_CURVE, &[("feedback", if fine { c.value("feedback") - f_resp.drag_delta().y * 0.1 } else { new_fb })]);
    }
    if t_resp.drag_stopped() || f_resp.drag_stopped() {
        end_gesture(app);
    }
    let fb_pct = c.value("feedback");
    for (resp, pos, tip) in [
        (&t_resp, first, format!("Time {time:.0} ms{}", note_name(time, bpm).map(|n| format!(" ({n})")).unwrap_or_default())),
        (&f_resp, second, format!("Feedback {fb_pct:.0}%")),
    ] {
        if resp.hovered() || resp.dragged() {
            ui.ctx().set_cursor_icon(if resp.dragged() { CursorIcon::Grabbing } else { CursorIcon::Grab });
            let galley = painter.layout_no_wrap(tip, mono(11.0), p.text_primary);
            let at = pos2((pos.x + space::S2).min(r.max.x - galley.size().x - space::S3), (pos.y - galley.size().y - space::S4).max(r.min.y));
            let box_r = Rect::from_min_size(at, galley.size() + vec2(space::S3, space::S2));
            painter.rect_filled(box_r, CornerRadius::same(radius::MD as u8), p.surface_overlay);
            painter.galley(box_r.min + vec2(space::S1 + 2.0, space::S1), galley, p.text_primary);
            painter.circle_stroke(pos, 7.0, Stroke::new(1.5, p.accent));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_values_at_120_bpm() {
        assert!((note_ms(120.0, 1.0) - 500.0).abs() < 1e-3);
        assert!((note_ms(120.0, 0.75) - 375.0).abs() < 1e-3);
        assert_eq!(note_name(250.0, 120.0), Some("1/8"));
        assert_eq!(note_name(260.0, 120.0), None);
        assert!(note_ms(0.0, 1.0).is_finite());
    }

    #[test]
    fn presets_name_real_parameters() {
        let ids: Vec<&str> = soundcraft_dsp::plugins::modulation::DELAY_INFO.params.iter().map(|p| p.id).collect();
        for (_, values) in presets(104.0) {
            for (id, _) in values {
                assert!(ids.contains(&id), "{id}");
            }
        }
    }
}
