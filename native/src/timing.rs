//! Display-only race telemetry. Track-distance gaps are not world-space distances.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct TowerSettings {
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
pub fn draw(ui: &hudhook::imgui::Ui, rows: &[Runner], settings: &TowerSettings, selected: i32) {
    use hudhook::imgui::{Condition, WindowFlags};
    if rows.is_empty() {
        return;
    }
    let display = ui.io().display_size;
    if display[0] < 180. || display[1] < 100. {
        return;
    }
    let count = rows.len();
    let logical_width = if settings.speed { 480. } else { 400. };
    let logical_height = 90. + count as f32 * 30.;
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
            let white = [0.93, 0.95, 1., 1.];
            let muted = [0.58, 0.64, 0.75, 1.];
            let accent = crate::design::ACCENT;
            let gold = [0.96, 0.78, 0.44, 1.];
            let point = |dx: f32, dy: f32| [x + dx * scale, y + dy * scale];
            let rect = |l: f32, t: f32, r: f32, b: f32, c: [f32; 4], round: f32| {
                d.add_rect(point(l, t), point(r, b), c)
                    .rounding(round * scale)
                    .filled(true)
                    .build();
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
            rect(
                0.,
                0.,
                logical_width,
                logical_height,
                [0.035, 0.045, 0.065, alpha],
                8.,
            );
            rect(
                0.,
                0.,
                logical_width,
                3.,
                [accent[0], accent[1], accent[2], alpha],
                0.,
            );
            text(14., 13., white, "RACE ORDER");
            right(
                logical_width - 14.,
                13.,
                accent,
                &format!("{} RUNNERS", count),
            );
            text(
                14.,
                38.,
                muted,
                if settings.interval {
                    "INTERVAL / RUNNER AHEAD"
                } else {
                    "GAP / RACE LEADER"
                },
            );
            text(14., 65., muted, "POS");
            text(92., 65., muted, "RUNNER");
            let gap_right = logical_width - 14.;
            let speed_right = logical_width - 112.;
            if settings.speed {
                right(
                    speed_right,
                    65.,
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
                65.,
                muted,
                if settings.estimated_seconds {
                    "EST. S"
                } else {
                    "GAP M"
                },
            );
            for (i, r) in rows.iter().enumerate() {
                let top = 87. + i as f32 * 30.;
                let tracked = r.gate == selected;
                let color = if tracked {
                    [0.20, 0.26, 0.43, alpha]
                } else if i % 2 == 0 {
                    [0.075, 0.09, 0.13, alpha]
                } else {
                    [0.055, 0.068, 0.10, alpha]
                };
                rect(5., top, logical_width - 5., top + 28., color, 3.);
                if tracked {
                    rect(5., top + 4., 8., top + 24., accent, 1.);
                }
                let leader = r.rank == Some(1);
                rect(
                    12.,
                    top + 3.,
                    42.,
                    top + 25.,
                    if leader {
                        [0.40, 0.30, 0.14, alpha]
                    } else {
                        [0.14, 0.17, 0.23, alpha]
                    },
                    3.,
                );
                let rank = r.rank.map(|v| v.to_string()).unwrap_or_else(|| "--".into());
                let tw = ui.calc_text_size(&rank)[0] / scale;
                text(
                    27. - tw / 2.,
                    top + 5.,
                    if leader { gold } else { white },
                    &rank,
                );
                text(54., top + 5., muted, &format!("{:02}", r.gate));
                let name_end = if settings.speed {
                    logical_width - 180.
                } else {
                    logical_width - 104.
                };
                let max_name = (name_end - 92.) * scale;
                let mut label = r.name.clone();
                if ui.calc_text_size(&label)[0] > max_name {
                    while !label.is_empty()
                        && ui.calc_text_size(&format!("{label}..."))[0] > max_name
                    {
                        label.pop();
                    }
                    label.push_str("...");
                }
                text(92., top + 5., if tracked { accent } else { white }, &label);
                if settings.speed {
                    let value = r
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
                    right(speed_right, top + 5., muted, &value);
                }
                let value = if i == 0 {
                    "LEADER".into()
                } else {
                    gap(rows, i, settings.interval, settings.estimated_seconds)
                        .map(|v| format!("+{v:.1}"))
                        .unwrap_or_else(|| "--".into())
                };
                right(
                    gap_right,
                    top + 5.,
                    if leader { gold } else { white },
                    &value,
                );
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
