use crate::model::Vec3;
#[derive(Clone, Copy)]
struct Anchor {
    offset: Vec3,
    heading: f32,
}
#[derive(Default)]
pub struct TargetTween {
    gate: Option<i32>,
    shown: Option<Anchor>,
    from: Option<Anchor>,
    elapsed: f32,
}
impl TargetTween {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn settled(&self) -> bool {
        self.from.is_none()
    }
    pub fn follow(
        &mut self,
        gate: i32,
        target: Vec3,
        heading: f32,
        pack: Vec3,
        dt: f32,
        duration: f32,
    ) -> (Vec3, f32) {
        let goal = Anchor {
            offset: Vec3 {
                x: target.x - pack.x,
                y: target.y - pack.y,
                z: target.z - pack.z,
            },
            heading,
        };
        if self.gate != Some(gate) {
            self.from = self.shown;
            self.elapsed = 0.;
            self.gate = Some(gate);
        }
        self.elapsed += dt.max(0.);
        let shown = if let Some(from) = self
            .from
            .filter(|_| duration > 0. && self.elapsed < duration)
        {
            let t = (self.elapsed / duration).clamp(0., 1.);
            let t = t * t * (3. - 2. * t);
            let lerp = |a: f32, b: f32| a + (b - a) * t;
            let angle = (goal.heading - from.heading + std::f32::consts::PI)
                .rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            Anchor {
                offset: Vec3 {
                    x: lerp(from.offset.x, goal.offset.x),
                    y: lerp(from.offset.y, goal.offset.y),
                    z: lerp(from.offset.z, goal.offset.z),
                },
                heading: from.heading + angle * t,
            }
        } else {
            self.from = None;
            goal
        };
        self.shown = Some(shown);
        (
            Vec3 {
                x: pack.x + shown.offset.x,
                y: pack.y + shown.offset.y,
                z: pack.z + shown.offset.z,
            },
            shown.heading,
        )
    }
}
pub fn look_at(eye: Vec3, target: Vec3) -> [f32; 4] {
    let dx = target.x - eye.x;
    let dy = target.y - eye.y;
    let dz = target.z - eye.z;
    let yaw = dx.atan2(dz);
    let pitch = (-dy).atan2((dx * dx + dz * dz).sqrt());
    let (sy, cy) = (yaw * 0.5).sin_cos();
    let (sx, cx) = (pitch * 0.5).sin_cos();
    [cy * sx, sy * cx, -sy * sx, cy * cx]
}
pub fn clamp_height(p: &mut Vec3, ground: f32, clearance: f32) -> bool {
    let floor = ground + clearance;
    if floor.is_finite() && p.y < floor {
        p.y = floor;
        true
    } else {
        false
    }
}
#[derive(Default)]
pub struct Surface {
    triangles: Vec<[Vec3; 3]>,
    cells: std::collections::HashMap<(i32, i32), Vec<usize>>,
}
impl Surface {
    pub fn len(&self) -> usize {
        self.triangles.len()
    }
    pub fn add(&mut self, t: [Vec3; 3]) {
        if self.triangles.len() >= 200_000
            || t.iter()
                .any(|v| ![v.x, v.y, v.z].iter().all(|f| f.is_finite()))
        {
            return;
        }
        let minx = t.iter().map(|v| v.x).fold(f32::INFINITY, f32::min);
        let maxx = t.iter().map(|v| v.x).fold(f32::NEG_INFINITY, f32::max);
        let minz = t.iter().map(|v| v.z).fold(f32::INFINITY, f32::min);
        let maxz = t.iter().map(|v| v.z).fold(f32::NEG_INFINITY, f32::max);
        let (x0, x1, z0, z1) = (
            (minx / 16.).floor() as i32,
            (maxx / 16.).floor() as i32,
            (minz / 16.).floor() as i32,
            (maxz / 16.).floor() as i32,
        );
        if (i64::from(x1) - i64::from(x0) + 1) * (i64::from(z1) - i64::from(z0) + 1) > 4096 {
            return;
        }
        let index = self.triangles.len();
        self.triangles.push(t);
        for x in x0..=x1 {
            for z in z0..=z1 {
                self.cells.entry((x, z)).or_default().push(index);
            }
        }
    }
    pub fn height(&self, p: Vec3) -> Option<f32> {
        let candidates = self
            .cells
            .get(&((p.x / 16.).floor() as i32, (p.z / 16.).floor() as i32))?;
        candidates
            .iter()
            .filter_map(|i| {
                let [a, b, c] = self.triangles[*i];
                let den = (b.z - c.z) * (a.x - c.x) + (c.x - b.x) * (a.z - c.z);
                if den.abs() < 1e-6 {
                    return None;
                }
                let u = ((b.z - c.z) * (p.x - c.x) + (c.x - b.x) * (p.z - c.z)) / den;
                let v = ((c.z - a.z) * (p.x - c.x) + (a.x - c.x) * (p.z - c.z)) / den;
                (u >= -0.0001 && v >= -0.0001 && u + v <= 1.0001)
                    .then_some(u * a.y + v * b.y + (1. - u - v) * c.y)
            })
            .filter(|y| y.is_finite())
            .max_by(f32::total_cmp)
    }
}
pub fn ground_mesh(path: &str) -> bool {
    if !path.starts_with("RaceScene/RaceManager(Clone)/RaceView/Course/") {
        return false;
    }
    let name = path.rsplit('/').next().unwrap_or("");
    ["turf_lm", "dirt_lm", "base_lm"].iter().any(|prefix| {
        name.strip_prefix(prefix)
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn p(x: f32, y: f32, z: f32) -> Vec3 {
        Vec3 { x, y, z }
    }
    #[test]
    fn target_switch_is_continuous_retargetable_and_moves_with_pack() {
        let mut t = TargetTween::default();
        t.follow(1, p(0., 0., 0.), 3.13, p(0., 0., 0.), 0., 1.);
        let (a, h) = t.follow(2, p(10., 0., 0.), -3.13, p(0., 0., 0.), 0.5, 1.);
        assert!((a.x - 5.).abs() < 1e-5);
        assert!((h - std::f32::consts::PI).abs() < 0.001);
        let (b, _) = t.follow(3, p(40., 0., 0.), 0., p(20., 0., 0.), 0., 1.);
        assert!((b.x - 25.).abs() < 1e-5);
        let (c, _) = t.follow(3, p(50., 0., 0.), 0., p(30., 0., 0.), 1., 1.);
        assert!((c.x - 50.).abs() < 1e-5);
    }
    #[test]
    fn switch_progress_is_frame_rate_independent_and_zero_duration_snaps() {
        let run = |hz: i32| {
            let mut t = TargetTween::default();
            t.follow(1, p(0., 0., 0.), 0., p(0., 0., 0.), 0., 1.);
            let mut out = p(0., 0., 0.);
            for _ in 0..hz / 2 {
                out = t
                    .follow(2, p(10., 0., 0.), 0., p(0., 0., 0.), 1. / hz as f32, 1.)
                    .0;
            }
            out.x
        };
        assert!((run(30) - run(120)).abs() < 0.001);
        let mut t = TargetTween::default();
        t.follow(1, p(0., 0., 0.), 0., p(0., 0., 0.), 0., 0.);
        assert_eq!(
            t.follow(2, p(10., 0., 0.), 0., p(0., 0., 0.), 0., 0.).0.x,
            10.
        );
        t.reset();
        assert_eq!(
            t.follow(3, p(30., 0., 0.), 0., p(0., 0., 0.), 0., 1.).0.x,
            30.
        );
    }
    #[test]
    fn terrain_interpolates_slopes_negative_coordinates_and_overlapping_surfaces() {
        let mut s = Surface::default();
        s.add([p(-10., 0., -10.), p(10., 2., -10.), p(-10., 0., 10.)]);
        assert!((s.height(p(-5., -99., -5.)).unwrap() - 0.5).abs() < 1e-5);
        assert!(s.height(p(50., 0., 50.)).is_none());
        s.add([p(-10., 3., -10.), p(10., 3., -10.), p(-10., 3., 10.)]);
        assert_eq!(s.height(p(-5., 0., -5.)), Some(3.));
        let mut eye = p(-5., -10., -5.);
        assert!(clamp_height(&mut eye, 3., 0.4));
        assert_eq!(eye.y, 3.4);
        assert!(!clamp_height(&mut eye, 2., 0.4));
    }
    #[test]
    fn surface_classifier_excludes_props_and_other_scenes() {
        let root = "RaceScene/RaceManager(Clone)/RaceView/Course/course/";
        assert!(ground_mesh(&format!("{root}turf_lm01")));
        assert!(ground_mesh(&format!("{root}base_lm00")));
        assert!(!ground_mesh(&format!("{root}turf_lm01_extra")));
        assert!(!ground_mesh(&format!("{root}trees_pack01")));
        assert!(!ground_mesh("Menu/turf_lm01"));
    }
}
