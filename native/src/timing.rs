//! Display-only race telemetry. Track-distance gaps are not world-space distances.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct TowerSettings {
    pub show_trainers: bool,
    pub trainer_names: std::collections::BTreeMap<i32, String>,
    pub speed: bool,
    pub metres_per_second: bool,
    pub interval: bool,
    pub estimated_seconds: bool,
    pub right: bool,
    pub scale: f32,
    pub opacity: f32,
    pub vertical: f32,
}
impl Default for TowerSettings {
    fn default() -> Self {
        Self {
            show_trainers: false,
            trainer_names: std::collections::BTreeMap::new(),
            speed: true,
            metres_per_second: false,
            interval: false,
            estimated_seconds: false,
            right: false,
            scale: 1.,
            opacity: 0.85,
            vertical: 0.12,
        }
    }
}
impl TowerSettings {
    pub fn sanitize(&mut self) {
        self.trainer_names.retain(|gate, name| {
            *name = name
                .chars()
                .filter(|c| !c.is_control())
                .take(64)
                .collect::<String>();
            (1..=64).contains(gate)
        });
        fn bound(v: f32, default: f32, low: f32, high: f32) -> f32 {
            if v.is_finite() {
                v.clamp(low, high)
            } else {
                default
            }
        }
        self.scale = bound(self.scale, 1., 0.6, 1.5);
        self.opacity = bound(self.opacity, 0.85, 0.2, 1.);
        self.vertical = bound(self.vertical, 0.12, 0., 1.);
    }
}
#[derive(Clone, Debug)]
pub struct Runner {
    pub gate: i32,
    pub name: String,
    pub rank: Option<i32>,
    pub distance: Option<f32>,
    pub speed: Option<f32>,
}
fn nonnegative(v: Option<f32>) -> Option<f32> {
    v.filter(|v| v.is_finite() && *v >= 0.)
}
pub fn ordered(mut runners: Vec<Runner>) -> Vec<Runner> {
    for r in &mut runners {
        r.distance = nonnegative(r.distance);
        r.speed = nonnegative(r.speed);
        r.rank = r.rank.filter(|v| *v > 0 && *v <= 64);
    }
    // Use the game's ranking, including photo-finish order. Only fall back to
    // progress if the entire field lacks a valid rank.
    if runners.iter().all(|r| r.rank.is_some()) {
        runners.sort_by_key(|r| (r.rank.unwrap(), r.gate));
    } else {
        runners.sort_by(|a, b| {
            b.distance
                .unwrap_or(-1.)
                .total_cmp(&a.distance.unwrap_or(-1.))
                .then(a.gate.cmp(&b.gate))
        });
    }
    runners
}
pub fn gap(rows: &[Runner], index: usize, interval: bool, seconds: bool) -> Option<f32> {
    if index == 0 {
        return Some(0.);
    }
    let r = rows.get(index)?;
    let ahead = rows.get(if interval { index - 1 } else { 0 })?;
    let delta = nonnegative(ahead.distance)? - nonnegative(r.distance)?;
    // Inconsistent finish/progress values should be missing, not a negative gap.
    if delta < -0.05 {
        return None;
    }
    if seconds {
        let speed = nonnegative(r.speed)?;
        if speed < 0.1 {
            return None;
        }
        Some(delta.max(0.) / speed)
    } else {
        Some(delta.max(0.))
    }
}
pub fn trainer_label<'a>(settings: &'a TowerSettings, runner: &Runner) -> Option<&'a str> {
    settings
        .show_trainers
        .then(|| settings.trainer_names.get(&runner.gate))
        .flatten()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
}
fn row_height(settings: &TowerSettings, runner: &Runner) -> f32 {
    if trainer_label(settings, runner).is_some() {
        48.
    } else {
        32.
    }
}
fn tower_height(settings: &TowerSettings, rows: &[Runner]) -> f32 {
    80. + rows.iter().map(|r| row_height(settings, r)).sum::<f32>()
}
fn fit_text(ui: &hudhook::imgui::Ui, value: &str, width: f32) -> String {
    if ui.calc_text_size(value)[0] <= width {
        return value.into();
    }
    let mut text = value.to_owned();
    while !text.is_empty() && ui.calc_text_size(&format!("{text}..."))[0] > width {
        text.pop();
    }
    format!("{text}...")
}
pub fn draw(ui: &hudhook::imgui::Ui, rows: &[Runner], settings: &TowerSettings, selected: i32) {
    use hudhook::imgui::{Condition, StyleColor, WindowFlags};
    if rows.is_empty() {
        return;
    }
    let display = ui.io().display_size;
    if display[0] < 180. || display[1] < 100. {
        return;
    }
    let count = rows.len();
    let logical_width = if settings.speed { 480. } else { 400. };
    let logical_height = tower_height(settings, rows);
    let scale = settings
        .scale
        .min((display[0] - 24.) / logical_width)
        .min((display[1] - 24.) / logical_height)
        .max(0.05);
    let width = logical_width * scale;
    let height = logical_height * scale;
    let x = if settings.right {
        display[0] - width - 12.
    } else {
        12.
    };
    let y = 12. + (display[1] - height - 24.).max(0.) * settings.vertical;
    let dark = ui.style_color(StyleColor::WindowBg)[0] < 0.5;
    let accent = ui.style_color(StyleColor::SliderGrab);
    let _padding = ui.push_style_var(hudhook::imgui::StyleVar::WindowPadding([0., 0.]));
    let _border = ui.push_style_var(hudhook::imgui::StyleVar::WindowBorderSize(0.));
    ui.window("##race_timing_tower")
        .position([x, y], Condition::Always)
        .size([width, height], Condition::Always)
        .flags(
            WindowFlags::NO_INPUTS
                | WindowFlags::NO_TITLE_BAR
                | WindowFlags::NO_RESIZE
                | WindowFlags::NO_MOVE
                | WindowFlags::NO_SCROLLBAR
                | WindowFlags::NO_SAVED_SETTINGS
                | WindowFlags::NO_FOCUS_ON_APPEARING
                | WindowFlags::NO_NAV
                | WindowFlags::NO_BACKGROUND,
        )
        .build(|| {
            ui.set_window_font_scale(scale);
            let d = ui.get_window_draw_list();
            let white = [0.96, 0.97, 0.99, 1.];
            let muted = [0.74, 0.78, 0.84, 1.];
            let point = |dx: f32, dy: f32| [x + dx * scale, y + dy * scale];
            let rect = |l: f32, t: f32, r: f32, b: f32, c: [f32; 4]| {
                d.add_rect(point(l, t), point(r, b), c).filled(true).build();
            };
            let text = |dx: f32, dy: f32, c: [f32; 4], s: &str| d.add_text(point(dx, dy), c, s);
            let right = |dx: f32, dy: f32, c: [f32; 4], s: &str| {
                d.add_text(
                    [x + dx * scale - ui.calc_text_size(s)[0], y + dy * scale],
                    c,
                    s,
                )
            };
            let alpha = settings.opacity;
            // The tower always uses translucent dark strips for readability over the track.
            // Light mode distinguishes the leader with an off-white strip; no outer panel.
            rect(0., 0., logical_width, 78., [0.075, 0.09, 0.115, alpha]);
            text(14., 10., white, "RACE ORDER");
            right(logical_width - 14., 10., muted, &format!("{count} RUNNERS"));
            text(
                14.,
                32.,
                muted,
                if settings.interval {
                    "INTERVAL / RUNNER AHEAD"
                } else {
                    "GAP / RACE LEADER"
                },
            );
            text(14., 56., muted, "POS");
            text(62., 56., muted, "RUNNER");
            let gap_right = logical_width - 14.;
            let speed_right = logical_width - 112.;
            if settings.speed {
                right(
                    speed_right,
                    56.,
                    muted,
                    if settings.metres_per_second {
                        "M/S"
                    } else {
                        "KM/H"
                    },
                );
            }
            right(
                gap_right,
                56.,
                muted,
                if settings.estimated_seconds {
                    "EST. S"
                } else {
                    "GAP M"
                },
            );
            let mut top = 80.;
            for (i, r) in rows.iter().enumerate() {
                let h = row_height(settings, r);
                let tracked = r.gate == selected;
                let leader = i == 0;
                let light_leader = leader && !tracked && !dark;
                let bg = if tracked {
                    [accent[0], accent[1], accent[2], alpha.max(0.9)]
                } else if light_leader {
                    [0.96, 0.955, 0.94, alpha.max(0.9)]
                } else if leader {
                    [0.21, 0.24, 0.29, alpha]
                } else {
                    [0.075, 0.09, 0.115, alpha]
                };
                let fg = if light_leader {
                    [0.08, 0.10, 0.14, 1.]
                } else {
                    white
                };
                let secondary = if light_leader {
                    [0.31, 0.35, 0.40, 1.]
                } else if tracked {
                    [0.92, 0.95, 1., 1.]
                } else {
                    muted
                };
                rect(0., top, logical_width, top + h, bg);
                d.add_line(
                    point(0., top + h),
                    point(logical_width, top + h),
                    [0.65, 0.70, 0.78, alpha * 0.3],
                )
                .build();
                let name_x = 62.;
                let name_end = if settings.speed {
                    logical_width - 180.
                } else {
                    logical_width - 104.
                };
                let text_y = top + if h > 32. { 4. } else { 6. };
                let numeric_y = top + (h - 18.) * 0.5;
                let rank = r.rank.map(|v| v.to_string()).unwrap_or_else(|| "--".into());
                text(14., numeric_y, fg, &rank);
                let label = fit_text(ui, &r.name, (name_end - name_x) * scale);
                text(name_x, text_y, fg, &label);
                if let Some(trainer) = trainer_label(settings, r) {
                    ui.set_window_font_scale(scale * 0.82);
                    let label = fit_text(ui, trainer, (name_end - name_x) * scale);
                    text(name_x, top + 26., secondary, &label);
                    ui.set_window_font_scale(scale);
                }
                if settings.speed {
                    let v = r
                        .speed
                        .map(|v| {
                            format!(
                                "{:.1}",
                                if settings.metres_per_second {
                                    v
                                } else {
                                    v * 3.6
                                }
                            )
                        })
                        .unwrap_or_else(|| "--".into());
                    right(speed_right, numeric_y, fg, &v);
                }
                let v = if i == 0 {
                    "LEADER".into()
                } else {
                    gap(rows, i, settings.interval, settings.estimated_seconds)
                        .map(|v| format!("+{v:.1}"))
                        .unwrap_or_else(|| "--".into())
                };
                right(gap_right, numeric_y, fg, &v);
                top += h;
            }
        });
}
#[cfg(test)]
mod tests {
    use super::*;
    fn r(gate: i32, rank: Option<i32>, distance: f32, speed: f32) -> Runner {
        Runner {
            gate,
            name: format!("Runner {gate}"),
            rank,
            distance: Some(distance),
            speed: Some(speed),
        }
    }
    #[test]
    fn official_order_wins_and_gaps_use_track_progress() {
        let rows = ordered(vec![
            r(2, Some(2), 980., 20.),
            r(1, Some(1), 1000., 21.),
            r(3, Some(3), 970., 10.),
        ]);
        assert_eq!(
            rows.iter().map(|r| r.gate).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(gap(&rows, 2, false, false), Some(30.));
        assert_eq!(gap(&rows, 2, true, false), Some(10.));
        assert_eq!(gap(&rows, 2, true, true), Some(1.));
        let finish = ordered(vec![r(2, Some(2), 1001., 20.), r(1, Some(1), 1000., 0.)]);
        assert_eq!(finish[0].gate, 1);
        assert_eq!(gap(&finish, 1, false, false), None);
    }
    #[test]
    fn invalid_and_stopped_values_do_not_fabricate_time_gaps() {
        let rows = ordered(vec![
            r(2, None, 50., 0.),
            r(1, None, 60., 20.),
            r(3, None, f32::NAN, f32::INFINITY),
        ]);
        assert_eq!(rows[0].gate, 1);
        assert_eq!(gap(&rows, 1, false, true), None);
        assert_eq!(gap(&rows, 2, false, false), None);
        assert_eq!(rows[2].speed, None);
    }
    #[test]
    fn legacy_and_saved_preferences_are_bounded() {
        let mut s: TowerSettings =
            serde_json::from_str(r#"{"scale":99,"opacity":-1,"vertical":2}"#).unwrap();
        s.sanitize();
        assert_eq!(s.scale, 1.5);
        assert_eq!(s.opacity, 0.2);
        assert_eq!(s.vertical, 1.);
        assert!(!s.estimated_seconds);
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    #[test]
    fn full_field_fits_small_and_large_capture_frames_without_taking_input() {
        let mut ctx = hudhook::imgui::Context::create();
        ctx.set_ini_filename(None);
        crate::design::theme(&mut ctx);
        ctx.fonts().build_rgba32_texture();
        let rows = ordered(
            (1..=18)
                .map(|n| Runner {
                    gate: n,
                    name: format!("A very long runner name number {n}"),
                    rank: Some(n),
                    distance: Some(1500. - n as f32),
                    speed: Some(20.),
                })
                .collect(),
        );
        for size in [[1920., 1080.], [1280., 720.], [640., 480.]] {
            ctx.io_mut().display_size = size;
            ctx.io_mut().delta_time = 1. / 60.;
            ctx.io_mut().mouse_pos = [20., 100.];
            let ui = ctx.frame();
            draw(
                ui,
                &rows,
                &TowerSettings {
                    scale: 1.5,
                    ..TowerSettings::default()
                },
                7,
            );
            let data = ctx.render();
            assert!(data.total_vtx_count > 500, "entire tower should be drawn");
            for list in data.draw_lists() {
                for v in list.vtx_buffer() {
                    assert!(
                        v.pos[0] >= 0.
                            && v.pos[0] <= size[0]
                            && v.pos[1] >= 0.
                            && v.pos[1] <= size[1],
                        "vertex outside capture: {:?}",
                        v.pos
                    );
                }
            }
            assert!(!ctx.io().want_capture_mouse);
        }
    }
}

#[cfg(test)]
mod trainer_tests {
    use super::*;
    fn runner(gate: i32) -> Runner {
        Runner {
            gate,
            name: "Tokai Teio".into(),
            rank: Some(1),
            distance: Some(100.),
            speed: Some(20.),
        }
    }
    #[test]
    fn labels_follow_gate_not_position_and_blank_rows_stay_compact() {
        let mut settings = TowerSettings::default();
        settings.show_trainers = true;
        settings.trainer_names.insert(3, "MaybeVoid".into());
        assert_eq!(trainer_label(&settings, &runner(3)), Some("MaybeVoid"));
        assert_eq!(trainer_label(&settings, &runner(1)), None);
        assert_eq!(row_height(&settings, &runner(3)), 48.);
        assert_eq!(row_height(&settings, &runner(1)), 32.);
        settings.show_trainers = false;
        assert_eq!(row_height(&settings, &runner(3)), 32.);
    }
    #[test]
    fn trainer_settings_round_trip_sanitize_and_full_field_fits() {
        let mut settings: TowerSettings = serde_json::from_str("{}").unwrap();
        assert!(!settings.show_trainers);
        settings.show_trainers = true;
        for gate in 1..=18 {
            settings
                .trainer_names
                .insert(gate, "  Trainer\nName  ".into());
        }
        settings.trainer_names.insert(99, "Bad gate".into());
        settings.sanitize();
        assert!(!settings.trainer_names.contains_key(&99));
        assert_eq!(trainer_label(&settings, &runner(1)), Some("TrainerName"));
        let loaded: TowerSettings =
            serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert_eq!(loaded.trainer_names, settings.trainer_names);
        let rows = (1..=18).map(runner).collect::<Vec<_>>();
        let h = tower_height(&loaded, &rows);
        for screen_height in [480., 720., 1080.] {
            let scale = ((screen_height - 24.) / h).min(1.);
            assert!(h * scale <= screen_height - 23.9);
        }
    }
}
