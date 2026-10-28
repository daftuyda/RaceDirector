//! Screen labels projected from the final plugin camera, without creating game
//! UI objects or re-enabling native arrows. All drawing is input-transparent.
use crate::{model::Vec3, timing::Runner};
use hudhook::imgui::Ui;
#[derive(Clone, Copy, Debug)]
pub struct Marker {
    pub gate: i32,
    /// Normalized screen position, origin at top-left.
    pub point: [f32; 2],
}
/// Unity cameras look along local +Z. The quaternion passed here is the same
/// final rotation written to the camera, after fixed-shot and ground correction.
pub fn project(
    gate: i32,
    point: Vec3,
    eye: Vec3,
    q: [f32; 4],
    fov: f32,
    aspect: f32,
) -> Option<Marker> {
    if ![point.x, point.y, point.z, eye.x, eye.y, eye.z, fov, aspect]
        .into_iter()
        .all(f32::is_finite)
        || !q.into_iter().all(f32::is_finite)
        || !(1. ..179.).contains(&fov)
        || aspect <= 0.
    {
        return None;
    }
    let norm = q.into_iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm < 0.0001 {
        return None;
    }
    let [x, y, z, w] = q.map(|v| v / norm);
    let right = [
        1. - 2. * (y * y + z * z),
        2. * (x * y + w * z),
        2. * (x * z - w * y),
    ];
    let up = [
        2. * (x * y - w * z),
        1. - 2. * (x * x + z * z),
        2. * (y * z + w * x),
    ];
    let forward = [
        2. * (x * z + w * y),
        2. * (y * z - w * x),
        1. - 2. * (x * x + y * y),
    ];
    let delta = [point.x - eye.x, point.y - eye.y, point.z - eye.z];
    let dot = |axis: [f32; 3]| delta.into_iter().zip(axis).map(|(a, b)| a * b).sum::<f32>();
    let depth = dot(forward);
    if depth <= 0.05 {
        return None;
    }
    let half = (fov.to_radians() * 0.5).tan();
    let point = [
        0.5 + dot(right) / (2. * depth * half * aspect),
        0.5 - dot(up) / (2. * depth * half),
    ];
    point
        .into_iter()
        .all(|v| v.is_finite() && (0. ..=1.).contains(&v))
        .then_some(Marker { gate, point })
}
#[derive(Clone, Copy, Debug)]
struct Box2 {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}
impl Box2 {
    fn overlaps(self, b: Self) -> bool {
        self.x < b.x + b.w + 6.
            && self.x + self.w + 6. > b.x
            && self.y < b.y + b.h + 6.
            && self.y + self.h + 6. > b.y
    }
}
fn placement(
    anchor: [f32; 2],
    width: f32,
    height: f32,
    screen: [f32; 2],
    previous: Option<Box2>,
) -> Option<Box2> {
    if screen[0] < width + 16. || screen[1] < height + 16. {
        return None;
    }
    let mut rect = Box2 {
        x: (anchor[0] - width * 0.5).clamp(8., screen[0] - width - 8.),
        y: (anchor[1] - height - 4.).clamp(8., screen[1] - height - 8.),
        w: width,
        h: height,
    };
    if let Some(previous) = previous.filter(|previous| rect.overlaps(*previous)) {
        // Keep both labels at head height when horizontal space is available;
        // stack only as a fallback near screen edges.
        let left = previous.x - width - 8.;
        let right = previous.x + previous.w + 8.;
        let best = [left, right]
            .into_iter()
            .filter(|x| *x >= 8. && *x + width <= screen[0] - 8.)
            .min_by(|a, b| {
                (a + width * 0.5 - anchor[0])
                    .abs()
                    .total_cmp(&(b + width * 0.5 - anchor[0]).abs())
            });
        if let Some(x) = best {
            rect.x = x;
        } else {
            let top = previous.y - height - 8.;
            if top >= 8. {
                rect.y = top;
            } else {
                rect.y = previous.y + previous.h + 8.;
                if rect.y + height > screen[1] - 8. {
                    return None;
                }
            }
        }
    }
    Some(rect)
}
fn fit(ui: &Ui, text: &str, width: f32) -> String {
    if ui.calc_text_size(text)[0] <= width {
        return text.to_owned();
    }
    let mut text = text.to_owned();
    while !text.is_empty() && ui.calc_text_size(format!("{text}..."))[0] > width {
        text.pop();
    }
    format!("{text}...")
}
pub fn draw(ui: &Ui, markers: &[Marker], rows: &[Runner], focus: i32, rival: Option<i32>) {
    let screen = ui.io().display_size;
    let draw = ui.get_background_draw_list();
    let mut previous = None;
    // Deliberately only label the current focus and its one active battle rival.
    for gate in [Some(focus), rival.filter(|g| *g != focus)]
        .into_iter()
        .flatten()
    {
        let Some(marker) = markers.iter().find(|m| m.gate == gate) else {
            continue;
        };
        let name = rows
            .iter()
            .find(|r| r.gate == gate)
            .map(|r| r.name.as_str())
            .unwrap_or("Runner");
        let name = fit(ui, &format!("{gate:02}  {name}"), 204.);
        let role = if gate != focus {
            "BATTLE / RIVAL"
        } else if rival.is_some() {
            "BATTLE / FOCUS"
        } else {
            "FOCUS"
        };
        let width =
            (ui.calc_text_size(&name)[0].max(ui.calc_text_size(role)[0]) + 24.).clamp(120., 228.);
        let line = ui.text_line_height();
        let height = line * 2. + 15.;
        let anchor = [marker.point[0] * screen[0], marker.point[1] * screen[1]];
        let Some(rect) = placement(anchor, width, height, screen, previous) else {
            continue;
        };
        previous = Some(rect);
        let color = if gate == focus {
            [0.28, 0.57, 1., 1.]
        } else {
            [1., 0.70, 0.25, 1.]
        };
        let end = [
            anchor[0].clamp(rect.x + 8., rect.x + rect.w - 8.),
            rect.y + rect.h,
        ];
        draw.add_line(end, anchor, color).thickness(1.5).build();
        draw.add_circle(anchor, 3., color).filled(true).build();
        draw.add_rect(
            [rect.x, rect.y],
            [rect.x + rect.w, rect.y + rect.h],
            [0.035, 0.045, 0.065, 0.90],
        )
        .rounding(5.)
        .filled(true)
        .build();
        draw.add_rect([rect.x, rect.y], [rect.x + 3., rect.y + rect.h], color)
            .filled(true)
            .build();
        draw.add_text([rect.x + 12., rect.y + 5.], color, role);
        draw.add_text(
            [rect.x + 12., rect.y + line + 8.],
            [0.97, 0.98, 1., 1.],
            name,
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn projection_follows_final_camera_and_rejects_behind_or_offscreen() {
        let origin = Vec3::default();
        let q = [0., 0., 0., 1.];
        let p = Vec3 {
            x: 0.,
            y: 0.,
            z: 10.,
        };
        assert_eq!(project(1, p, origin, q, 90., 1.).unwrap().point, [0.5, 0.5]);
        let above = project(1, Vec3 { y: 2., ..p }, origin, q, 90., 1.).unwrap();
        assert!((above.point[1] - 0.4).abs() < 0.001);
        assert!(project(1, Vec3 { z: -10., ..p }, origin, q, 90., 1.).is_none());
        assert!(project(1, Vec3 { x: 30., ..p }, origin, q, 90., 1.).is_none());
        assert!(project(1, p, origin, q, f32::NAN, 1.).is_none());
        let target = Vec3 { x: 10., ..origin };
        let q = crate::camera_motion::look_at(origin, target);
        let marker = project(1, target, origin, q, 60., 16. / 9.).unwrap();
        assert!((marker.point[0] - 0.5).abs() < 0.001 && (marker.point[1] - 0.5).abs() < 0.001);
    }
    #[test]
    fn projection_matches_the_running_game_viewport_sample() {
        // Captured read-only from Unity WorldToViewportPoint on 2026-10-28.
        let marker = project(
            1,
            Vec3 {
                x: 112.947495,
                y: 2.9,
                z: 165.40247,
            },
            Vec3 {
                x: 177.91411,
                y: 2.2399123,
                z: 167.04552,
            },
            [-0.0055849627, 0.7159668, -0.005727996, -0.69808847],
            5.,
            1.7777778,
        )
        .unwrap();
        assert!((marker.point[0] - 0.49999624).abs() < 0.0001);
        assert!((marker.point[1] - (1. - 0.7996176)).abs() < 0.0001);
    }
    #[test]
    fn labels_sit_close_and_avoid_stacking_when_horizontal_space_exists() {
        let anchor = [500., 350.];
        let first = placement(anchor, 160., 46., [1280., 720.], None).unwrap();
        assert_eq!(anchor[1] - (first.y + first.h), 4.);
        let second = placement(anchor, 160., 46., [1280., 720.], Some(first)).unwrap();
        assert_eq!(second.y, first.y);
        assert!(!second.overlaps(first));
        let mut settings: crate::broadcast::BroadcastSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings.marker_height, 2.1);
        settings.marker_height = f32::NAN;
        settings.sanitize();
        assert_eq!(settings.marker_height, 2.1);
    }
    #[test]
    fn close_battle_labels_do_not_cover_each_other_or_leave_frame() {
        for anchor in [[500., 350.], [50., 20.], [1260., 710.]] {
            let first = placement(anchor, 220., 46., [1280., 720.], None).unwrap();
            let second = placement(anchor, 220., 46., [1280., 720.], Some(first)).unwrap();
            assert!(!first.overlaps(second));
            for rect in [first, second] {
                assert!(
                    rect.x >= 0.
                        && rect.y >= 0.
                        && rect.x + rect.w <= 1280.
                        && rect.y + rect.h <= 720.
                );
            }
        }
    }
}
