//! Presentation-only broadcast graphics. Metadata is supplied by the operator;
//! result rows are restricted to runners confirmed as finished by the runtime.
use crate::timing::{trainer_label, Runner, TowerSettings};
use hudhook::imgui::{Condition, DrawListMut, StyleColor, StyleVar, Ui, WindowFlags};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct BroadcastSettings {
    pub spotlight: bool,
    pub focus_markers: bool,
    pub marker_height: f32,
    pub battle_card: bool,
    pub progress: bool,
    pub results: bool,
    pub scale: f32,
    pub opacity: f32,
    pub event_name: String,
    pub course_name: String,
    /// Operator-supplied course length. Zero means unknown.
    pub race_distance_m: f32,
}
impl Default for BroadcastSettings {
    fn default() -> Self {
        Self {
            spotlight: false,
            focus_markers: true,
            marker_height: 2.1,
            battle_card: false,
            progress: false,
            results: false,
            scale: 1.,
            opacity: 0.85,
            event_name: String::new(),
            course_name: String::new(),
            race_distance_m: 0.,
        }
    }
}
impl BroadcastSettings {
    pub fn sanitize(&mut self) {
        self.scale = bound(self.scale, 1., 0.6, 1.5);
        self.marker_height = bound(self.marker_height, 2.1, 1.4, 3.5);
        self.opacity = bound(self.opacity, 0.85, 0.2, 1.);
        self.race_distance_m = bound(self.race_distance_m, 0., 0., 10_000.);
        // Preserve trailing spaces during editing, as with trainer labels.
        for value in [&mut self.event_name, &mut self.course_name] {
            *value = value.chars().filter(|c| !c.is_control()).take(96).collect();
        }
    }
}
fn bound(v: f32, default: f32, low: f32, high: f32) -> f32 {
    if v.is_finite() {
        v.clamp(low, high)
    } else {
        default
    }
}
fn nonnegative(v: Option<f32>) -> Option<f32> {
    v.filter(|v| v.is_finite() && *v >= 0.)
}

#[derive(Debug, PartialEq)]
struct Progress {
    covered: f32,
    remaining: Option<f32>,
    fraction: Option<f32>,
}
fn progress(rows: &[Runner], length: f32) -> Option<Progress> {
    // No predicted finish order or final-straight boundary is consulted.
    let covered = rows
        .iter()
        .filter_map(|r| nonnegative(r.distance))
        .reduce(f32::max)?;
    let length = (length.is_finite() && length > 0.).then_some(length);
    Some(Progress {
        covered,
        remaining: length.map(|n| (n - covered).max(0.)),
        fraction: length.map(|n| (covered / n).clamp(0., 1.)),
    })
}

struct FinishView<'a> {
    rows: Vec<&'a Runner>,
    winner: Option<&'a Runner>,
    complete: bool,
}
fn finish_view<'a>(rows: &'a [Runner], finished: &[i32]) -> FinishView<'a> {
    let mut confirmed: Vec<_> = rows
        .iter()
        .filter(|r| {
            finished.contains(&r.gate) && r.rank.is_some_and(|rank| (1..=64).contains(&rank))
        })
        .collect();
    confirmed.sort_by_key(|r| (r.rank, r.gate));
    // Duplicate ranks indicate an inconsistent sample. Avoid presenting those
    // positions as confirmed until the runtime provides a coherent snapshot.
    let ranks = confirmed.iter().filter_map(|r| r.rank).collect::<Vec<_>>();
    confirmed.retain(|r| ranks.iter().filter(|rank| Some(**rank) == r.rank).count() == 1);
    let winner = confirmed.iter().copied().find(|r| r.rank == Some(1));
    let complete = !rows.is_empty() && confirmed.len() == rows.len();
    FinishView {
        rows: confirmed,
        winner,
        complete,
    }
}

#[derive(Clone, Copy, Debug)]
struct Layout {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    scale: f32,
}
#[derive(Clone, Copy)]
struct Area {
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
    align_right: bool,
}
fn area(
    display: [f32; 2],
    rows: &[Runner],
    tower: &TowerSettings,
    tower_visible: bool,
) -> Option<Area> {
    if !display.iter().all(|v| v.is_finite()) || display[0] < 180. || display[1] < 120. {
        return None;
    }
    if !tower_visible {
        return Some(Area {
            left: 12.,
            right: display[0] - 12.,
            top: 12.,
            bottom: display[1] - 12.,
            align_right: !tower.right,
        });
    }
    // Match the tower's current scale, including taller trainer-name rows.
    let tower_width = if tower.speed { 480. } else { 400. };
    let tower_height = 80.
        + rows
            .iter()
            .map(|r| {
                if trainer_label(tower, r).is_some() {
                    48.
                } else {
                    32.
                }
            })
            .sum::<f32>();
    let tower_scale = bound(tower.scale, 1., 0.6, 1.5)
        .min((display[0] - 24.) / tower_width)
        .min((display[1] - 24.) / tower_height)
        .max(0.05);
    let reserved = tower_width * tower_scale + 24.;
    let mut area = Area {
        left: if tower.right { 12. } else { reserved },
        right: if tower.right {
            display[0] - reserved
        } else {
            display[0] - 12.
        },
        top: 12.,
        bottom: display[1] - 12.,
        align_right: !tower.right,
    };
    // On very narrow screens a short tower leaves room beneath it. Use that
    // region rather than shrinking names to slivers or overlapping the tower.
    if area.right - area.left < 160. {
        let tower_y = 12.
            + (display[1] - tower_height * tower_scale - 24.).max(0.)
                * bound(tower.vertical, 0.12, 0., 1.);
        area.left = 12.;
        area.right = display[0] - 12.;
        area.top = tower_y + tower_height * tower_scale + 12.;
    }
    (area.right - area.left >= 120. && area.bottom - area.top >= 90.).then_some(area)
}
fn layout(area: Area, size: [f32; 2], desired: f32, bottom: bool) -> Option<Layout> {
    let scale = bound(desired, 1., 0.6, 1.5)
        .min((area.right - area.left) / size[0])
        .min((area.bottom - area.top) / size[1]);
    if !scale.is_finite() || scale < 0.2 {
        return None;
    }
    let width = size[0] * scale;
    let height = size[1] * scale;
    Some(Layout {
        x: if area.align_right {
            area.right - width
        } else {
            area.left
        },
        y: if bottom {
            area.bottom - height
        } else {
            area.top
        },
        width,
        height,
        scale,
    })
}

struct Paint<'ui> {
    ui: &'ui Ui,
    draw: DrawListMut<'ui>,
    layout: Layout,
}
impl Paint<'_> {
    fn point(&self, x: f32, y: f32) -> [f32; 2] {
        [
            self.layout.x + x * self.layout.scale,
            self.layout.y + y * self.layout.scale,
        ]
    }
    fn rect(&self, a: [f32; 2], b: [f32; 2], color: [f32; 4]) {
        self.draw
            .add_rect(self.point(a[0], a[1]), self.point(b[0], b[1]), color)
            .filled(true)
            .build();
    }
    fn text(&self, x: f32, y: f32, color: [f32; 4], value: &str, width: f32) {
        let value = fit(self.ui, value, width * self.layout.scale);
        self.draw.add_text(self.point(x, y), color, value);
    }
    fn right(&self, x: f32, y: f32, color: [f32; 4], value: &str) {
        let p = self.point(x, y);
        self.draw.add_text(
            [p[0] - self.ui.calc_text_size(value)[0], p[1]],
            color,
            value,
        );
    }
}
fn fit(ui: &Ui, value: &str, width: f32) -> String {
    if width <= 0. {
        return String::new();
    }
    if ui.calc_text_size(value)[0] <= width {
        return value.into();
    }
    if ui.calc_text_size("...")[0] > width {
        return String::new();
    }
    let mut text = value.to_owned();
    while !text.is_empty() && ui.calc_text_size(&format!("{text}..."))[0] > width {
        text.pop();
    }
    format!("{text}...")
}
const WHITE: [f32; 4] = [0.96, 0.97, 0.99, 1.];
const MUTED: [f32; 4] = [0.74, 0.78, 0.84, 1.];
fn panel(ui: &Ui, id: &str, placement: Layout, alpha: f32, paint: impl FnOnce(&Paint<'_>)) {
    let _padding = ui.push_style_var(StyleVar::WindowPadding([0., 0.]));
    let _border = ui.push_style_var(StyleVar::WindowBorderSize(0.));
    ui.window(id)
        .position([placement.x, placement.y], Condition::Always)
        .size([placement.width, placement.height], Condition::Always)
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
            ui.set_window_font_scale(placement.scale);
            let p = Paint {
                ui,
                draw: ui.get_window_draw_list(),
                layout: placement,
            };
            p.rect(
                [0., 0.],
                [
                    placement.width / placement.scale,
                    placement.height / placement.scale,
                ],
                [0.075, 0.09, 0.115, bound(alpha, 0.85, 0.2, 1.)],
            );
            p.rect(
                [0., 0.],
                [3., placement.height / placement.scale],
                ui.style_color(StyleColor::SliderGrab),
            );
            paint(&p);
        });
}
fn speed_label(r: &Runner, tower: &TowerSettings) -> String {
    match nonnegative(r.speed) {
        Some(v) if tower.metres_per_second => format!("{v:.1} m/s"),
        Some(v) => format!("{:.1} km/h", v * 3.6),
        None => "Speed --".into(),
    }
}
fn rank_label(r: &Runner) -> String {
    r.rank
        .filter(|r| (1..=64).contains(r))
        .map(|r| format!("P{r}"))
        .unwrap_or_else(|| "P--".into())
}

pub fn draw(
    ui: &Ui,
    rows: &[Runner],
    tower: &TowerSettings,
    settings: &BroadcastSettings,
    selected: Option<i32>,
    finished: &[i32],
    tower_visible: bool,
) {
    if rows.is_empty() {
        return;
    }
    let Some(mut available) = area(ui.io().display_size, rows, tower, tower_visible) else {
        return;
    };
    let finish = finish_view(rows, finished);
    // Once confirmed finish rows are available, give them the whole side region.
    // This keeps a full field legible and suppresses operator-entered distance
    // estimates that may disagree with the game's confirmed finish state.
    let show_results = settings.results && !finish.rows.is_empty();
    if settings.progress && !show_results {
        if let Some(placement) = layout(available, [560., 100.], settings.scale, false) {
            panel(ui, "##race_progress", placement, settings.opacity, |p| {
                let title = if settings.event_name.trim().is_empty() {
                    "RACE PROGRESS"
                } else {
                    settings.event_name.trim()
                };
                p.text(16., 12., WHITE, title, 528.);
                let course = settings.course_name.trim();
                let subtitle = if settings.race_distance_m > 0. {
                    if course.is_empty() {
                        format!("{:.0} m course", settings.race_distance_m)
                    } else {
                        format!("{course}  /  {:.0} m", settings.race_distance_m)
                    }
                } else if course.is_empty() {
                    "Live race distance".into()
                } else {
                    course.into()
                };
                p.text(16., 36., MUTED, &subtitle, 528.);
                let label = match progress(rows, settings.race_distance_m) {
                    Some(value) => {
                        if let Some(fraction) = value.fraction {
                            p.rect([16., 86.], [544., 89.], [0.24, 0.28, 0.34, 1.]);
                            p.rect(
                                [16., 86.],
                                [16. + 528. * fraction, 89.],
                                ui.style_color(StyleColor::SliderGrab),
                            );
                        }
                        match value.remaining {
                            Some(remaining) => format!("{remaining:.0} m remaining"),
                            None => format!("{:.0} m covered", value.covered),
                        }
                    }
                    None => "Distance unavailable".into(),
                };
                p.text(16., 62., WHITE, &label, 528.);
            });
            available.top = placement.y + placement.height + 12.;
        }
    }
    let spotlight = (settings.spotlight && !show_results)
        .then_some(selected)
        .flatten()
        .and_then(|gate| rows.iter().find(|r| r.gate == gate));
    if let Some(runner) = spotlight {
        if let Some(placement) = layout(available, [560., 120.], settings.scale, true) {
            panel(ui, "##runner_spotlight", placement, settings.opacity, |p| {
                p.text(16., 12., MUTED, "ON CAMERA", 360.);
                p.right(544., 12., WHITE, &rank_label(runner));
                p.rect(
                    [16., 40.],
                    [68., 98.],
                    ui.style_color(StyleColor::SliderGrab),
                );
                p.text(22., 43., WHITE, "GATE", 42.);
                p.text(29., 69., WHITE, &format!("{:02}", runner.gate), 37.);
                p.text(84., 40., WHITE, &runner.name, 460.);
                if let Some(trainer) = trainer_label(tower, runner) {
                    p.text(84., 65., MUTED, trainer, 460.);
                }
                p.text(84., 92., MUTED, &speed_label(runner, tower), 460.);
            });
            available.bottom = placement.y - 12.;
        }
    } else if settings.battle_card && !show_results {
        // The battle card is drawn separately; reserve its region here.
        if let Some(placement) = layout(available, [620., 172.], settings.scale, true) {
            available.bottom = placement.y - 12.;
        }
    }
    if show_results {
        let list_top = if finish.winner.is_some() { 118. } else { 48. };
        let logical_height = list_top + finish.rows.len() as f32 * 30. + 12.;
        if let Some(placement) = layout(available, [560., logical_height], settings.scale, false) {
            panel(ui, "##race_results", placement, settings.opacity, |p| {
                p.text(
                    16.,
                    12.,
                    WHITE,
                    if finish.complete {
                        "RESULTS"
                    } else {
                        "FINISH ORDER"
                    },
                    280.,
                );
                p.right(
                    544.,
                    12.,
                    MUTED,
                    &format!("{} / {} FINISHED", finish.rows.len(), rows.len()),
                );
                if let Some(winner) = finish.winner {
                    let accent = ui.style_color(StyleColor::SliderGrab);
                    p.rect(
                        [3., 40.],
                        [560., 106.],
                        [accent[0], accent[1], accent[2], 0.95],
                    );
                    p.text(16., 44., WHITE, "WINNER", 70.);
                    p.text(100., 44., WHITE, &winner.name, 444.);
                    if let Some(trainer) = trainer_label(tower, winner) {
                        p.text(100., 76., WHITE, trainer, 444.);
                    }
                }
                for (i, runner) in finish.rows.iter().enumerate() {
                    let y = list_top + i as f32 * 30.;
                    p.text(16., y, MUTED, &rank_label(runner), 52.);
                    p.text(74., y, WHITE, &runner.name, 400.);
                    p.right(544., y, MUTED, &format!("#{:02}", runner.gate));
                    p.rect([16., y + 25.], [544., y + 26.], [0.22, 0.25, 0.3, 0.6]);
                }
            });
        }
    }
}

pub fn draw_battle(
    ui: &Ui,
    rows: &[Runner],
    tower: &TowerSettings,
    settings: &BroadcastSettings,
    info: &crate::battle::BattleInfo,
    tower_visible: bool,
) {
    if !settings.battle_card {
        return;
    }
    let Some(anchor) = rows.iter().find(|r| r.gate == info.anchor) else {
        return;
    };
    let Some(partner) = rows.iter().find(|r| r.gate == info.partner) else {
        return;
    };
    let Some(mut available) = area(ui.io().display_size, rows, tower, tower_visible) else {
        return;
    };
    if settings.progress {
        if let Some(placement) = layout(available, [560., 100.], settings.scale, false) {
            available.top = placement.y + placement.height + 12.;
        }
    }
    let Some(placement) = layout(available, [620., 172.], settings.scale, true) else {
        return;
    };
    panel(ui, "##battle_spotlight", placement, settings.opacity, |p| {
        let title = info
            .position
            .filter(|r| (1..=64).contains(r))
            .map(|rank| format!("BATTLE FOR P{rank}"))
            .unwrap_or_else(|| "ON CAMERA / BATTLE".into());
        p.text(16., 12., WHITE, &title, 588.);
        let mut pair = [anchor, partner];
        pair.sort_by_key(|r| (r.rank.unwrap_or(65), r.gate));
        for (i, runner) in pair.iter().enumerate() {
            let y = 44. + i as f32 * 44.;
            p.text(16., y, MUTED, &rank_label(runner), 48.);
            p.text(76., y, WHITE, &runner.name, 528.);
            if let Some(trainer) = trainer_label(tower, runner) {
                p.text(76., y + 21., MUTED, trainer, 528.);
            }
        }
        let gap = nonnegative(info.gap_m)
            .map(|v| format!("GAP  {v:.1} m"))
            .unwrap_or_else(|| "GAP  --".into());
        p.text(16., 145., WHITE, &gap, 210.);
        if let Some(closing) = info.closing_mps.filter(|v| v.is_finite()) {
            let movement = if closing > 0.1 {
                format!("CLOSING  {closing:.1} m/s")
            } else if closing < -0.1 {
                format!("OPENING  {:.1} m/s", -closing)
            } else {
                "GAP STEADY".into()
            };
            p.right(604., 145., MUTED, &movement);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn runner(gate: i32, rank: Option<i32>, distance: Option<f32>) -> Runner {
        Runner {
            gate,
            name: format!("Runner {gate}"),
            trainer_name: None,
            rank,
            distance,
            speed: Some(20.),
        }
    }
    #[test]
    fn legacy_defaults_and_metadata_round_trip() {
        let mut s: BroadcastSettings = serde_json::from_str("{}").unwrap();
        assert!(!s.spotlight && !s.battle_card && !s.progress && !s.results);
        s.event_name = "  Autumn\nCup  ".into();
        s.course_name = "Long course".repeat(30);
        s.race_distance_m = f32::INFINITY;
        s.scale = f32::NAN;
        s.opacity = -1.;
        s.sanitize();
        assert_eq!(s.event_name, "  AutumnCup  ");
        assert_eq!(s.course_name.chars().count(), 96);
        assert_eq!(s.race_distance_m, 0.);
        assert_eq!(s.scale, 1.);
        assert_eq!(s.opacity, 0.2);
        let loaded: BroadcastSettings =
            serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(loaded.event_name, s.event_name);
    }
    #[test]
    fn unknown_distance_never_invents_remaining_or_phase() {
        let rows = [
            runner(1, Some(1), Some(1500.)),
            runner(2, Some(2), Some(1490.)),
        ];
        assert_eq!(
            progress(&rows, 0.),
            Some(Progress {
                covered: 1500.,
                remaining: None,
                fraction: None
            })
        );
        assert_eq!(
            progress(&rows, 2000.),
            Some(Progress {
                covered: 1500.,
                remaining: Some(500.),
                fraction: Some(0.75)
            })
        );
        assert_eq!(progress(&rows, 1000.).unwrap().remaining, Some(0.));
        assert_eq!(progress(&[runner(1, None, Some(f32::NAN))], 2000.), None);
        assert_eq!(progress(&[], 2000.), None);
        assert!(progress(&rows, f32::NAN).unwrap().remaining.is_none());
    }
    #[test]
    fn final_order_is_not_exposed_until_each_runner_finishes() {
        let rows = [
            runner(1, Some(1), Some(2000.)),
            runner(2, Some(2), Some(1990.)),
            runner(3, Some(3), Some(1980.)),
        ];
        let empty = finish_view(&rows, &[]);
        assert!(empty.rows.is_empty() && empty.winner.is_none() && !empty.complete);
        let partial = finish_view(&rows, &[2]);
        assert_eq!(partial.rows.len(), 1);
        assert!(partial.winner.is_none() && !partial.complete);
        let winner = finish_view(&rows, &[1, 2]);
        assert_eq!(winner.winner.unwrap().gate, 1);
        assert!(!winner.complete);
        let all = finish_view(&rows, &[3, 1, 2, 2]);
        assert!(all.complete);
        assert_eq!(
            all.rows.iter().map(|r| r.gate).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        let inconsistent = [runner(1, Some(1), None), runner(2, Some(1), None)];
        assert!(finish_view(&inconsistent, &[1, 2]).winner.is_none());
    }
    #[test]
    fn layouts_fit_full_fields_small_screens_and_both_tower_sides() {
        let rows: Vec<_> = (1..=18).map(|n| runner(n, Some(n), Some(1000.))).collect();
        for display in [[1920., 1080.], [1280., 720.], [854., 480.], [480., 854.]] {
            for right in [false, true] {
                let tower = TowerSettings {
                    right,
                    ..TowerSettings::default()
                };
                let available = area(display, &rows, &tower, true);
                // Extremely narrow portrait view can leave no usable side region.
                if let Some(available) = available {
                    for size in [[560., 120.], [620., 172.], [560., 670.]] {
                        if let Some(p) = layout(available, size, 1.5, true) {
                            assert!(p.x >= 0. && p.y >= 0.);
                            assert!(p.x + p.width <= display[0] - 11.9);
                            assert!(p.y + p.height <= display[1] - 11.9);
                            assert!(p.scale.is_finite() && p.scale > 0.);
                        }
                    }
                }
            }
        }
        assert!(area([f32::NAN, 1080.], &rows, &TowerSettings::default(), true).is_none());
        assert!(area([100., 100.], &rows, &TowerSettings::default(), true).is_none());
    }
    #[test]
    fn standalone_cards_use_available_width_and_stay_below_progress() {
        let rows: Vec<_> = (1..=18).map(|n| runner(n, Some(n), Some(1000.))).collect();
        let tower = TowerSettings::default();
        for display in [[1280., 720.], [854., 480.], [480., 854.], [640., 180.]] {
            let mut available = area(display, &rows, &tower, false).unwrap();
            assert_eq!(available.right - available.left, display[0] - 24.);
            let progress = layout(available, [560., 100.], 1., false).unwrap();
            available.top = progress.y + progress.height + 12.;
            if let Some(battle) = layout(available, [620., 172.], 1., true) {
                assert!(battle.y >= progress.y + progress.height + 11.9);
                assert!(battle.y + battle.height <= display[1] - 11.9);
            }
        }
    }
    #[test]
    fn speed_units_and_missing_values_are_explicit() {
        let mut r = runner(1, None, None);
        let mut tower = TowerSettings::default();
        assert_eq!(speed_label(&r, &tower), "72.0 km/h");
        tower.metres_per_second = true;
        assert_eq!(speed_label(&r, &tower), "20.0 m/s");
        r.speed = Some(f32::NAN);
        assert_eq!(speed_label(&r, &tower), "Speed --");
        assert_eq!(rank_label(&r), "P--");
    }
}
