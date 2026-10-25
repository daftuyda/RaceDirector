use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Pose {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub height: f32,
    pub fov: f32,
}
impl Default for Pose {
    fn default() -> Self {
        Self {
            yaw: 0.,
            pitch: 0.18,
            distance: 45.,
            height: 1.2,
            fov: 15.,
        }
    }
}
impl Pose {
    pub fn sanitize(&mut self) {
        let d = Self::default();
        if !self.yaw.is_finite() {
            self.yaw = d.yaw;
        }
        self.yaw = (self.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        self.pitch = finite(self.pitch, d.pitch).clamp(-1.2, 1.4);
        self.distance = finite(self.distance, d.distance).clamp(2., 200.);
        self.height = finite(self.height, d.height).clamp(-1., 20.);
        self.fov = finite(self.fov, d.fov).clamp(5., 80.);
    }
    pub fn approach(&mut self, target: Self, dt: f32, response: f32) {
        let a = 1. - (-response.max(0.1) * dt.clamp(0., 0.1)).exp();
        let dy = (target.yaw - self.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        self.yaw += dy * a;
        self.pitch += (target.pitch - self.pitch) * a;
        self.distance += (target.distance - self.distance) * a;
        self.height += (target.height - self.height) * a;
        self.fov += (target.fov - self.fov) * a;
    }
}
fn finite(x: f32, default: f32) -> f32 {
    if x.is_finite() {
        x
    } else {
        default
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Shot {
    #[serde(default)]
    pub free: Option<FreePose>,
    pub name: String,
    pub pose: Pose,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub dark_mode: bool,
    pub enabled: Enabled,
    pub tower: crate::timing::TowerSettings,
    pub fps_60: bool,
    pub graphics: crate::graphics::Quality,
    pub free_speed: f32,
    pub target_transition: f32,
    pub ground_clearance: f32,
    pub pose: Pose,
    pub response: f32,
    pub mouse_sensitivity: f32,
    pub shots: Vec<Shot>,
    pub hidden_paths: Vec<String>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            dark_mode: false,
            enabled: Enabled::default(),
            tower: crate::timing::TowerSettings::default(),
            fps_60: true,
            graphics: crate::graphics::Quality::default(),
            free_speed: 12.,
            target_transition: 0.7,
            ground_clearance: 0.4,
            pose: Pose::default(),
            response: 8.,
            mouse_sensitivity: 0.004,
            shots: vec![],
            hidden_paths: vec![],
        }
    }
}
impl Settings {
    pub fn sanitize(&mut self) {
        self.tower.sanitize();
        self.graphics.sanitize();
        self.enabled.graphics_applied.sanitize();
        self.target_transition = finite(self.target_transition, 0.7).clamp(0., 3.);
        self.ground_clearance = finite(self.ground_clearance, 0.4).clamp(0.15, 3.);
        self.pose.sanitize();
        self.free_speed = finite(self.free_speed, 12.).clamp(0.5, 150.);
        self.response = finite(self.response, 8.).clamp(0.5, 30.);
        self.mouse_sensitivity = finite(self.mouse_sensitivity, 0.004).clamp(0.001, 0.02);
        for s in &mut self.shots {
            s.pose.sanitize();
            if let Some(free) = &mut s.free {
                free.sanitize();
            }
        }
    }
}
pub fn path_matches(path: &str, selected: &str) -> bool {
    !selected.is_empty()
        && (path == selected
            || path
                .strip_prefix(selected)
                .is_some_and(|s| s.starts_with('/')))
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}
pub fn orbit(p: Pose, target: Vec3, heading: f32) -> (Vec3, [f32; 4]) {
    let angle = heading + std::f32::consts::PI + p.yaw;
    let eye = Vec3 {
        x: target.x + angle.sin() * p.pitch.cos() * p.distance,
        y: target.y + p.height + p.pitch.sin() * p.distance,
        z: target.z + angle.cos() * p.pitch.cos() * p.distance,
    };
    // Unity +Z forward, yaw then pitch; camera looks down by p.pitch.
    let yaw = angle + std::f32::consts::PI;
    let (sy, cy) = (yaw * 0.5).sin_cos();
    let (sx, cx) = (p.pitch * 0.5).sin_cos();
    (eye, [cy * sx, sy * cx, -sy * sx, cy * cx])
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_settings_enable_fps_and_explicit_opt_out_round_trips() {
        let legacy: Settings =
            serde_json::from_str(r#"{"response":12.0,"hidden_paths":["Race/Bottom"]}"#).unwrap();
        assert!(legacy.fps_60);
        assert_eq!(legacy.response, 12.);
        assert_eq!(legacy.hidden_paths, vec!["Race/Bottom"]);
        let disabled = Settings {
            fps_60: false,
            ..legacy
        };
        let loaded: Settings =
            serde_json::from_str(&serde_json::to_string(&disabled).unwrap()).unwrap();
        assert!(!loaded.fps_60);
    }
    #[test]
    fn bounded_settings() {
        let mut p = Pose {
            yaw: f32::NAN,
            pitch: 99.,
            distance: -5.,
            height: f32::INFINITY,
            fov: 0.,
        };
        p.sanitize();
        assert!(p.yaw.is_finite());
        assert_eq!(p.distance, 2.);
        assert_eq!(p.fov, 5.);
        assert_eq!(p.height, 1.2);
    }
    #[test]
    fn path_boundaries() {
        assert!(path_matches("Race/Skills/Icon", "Race/Skills"));
        assert!(!path_matches("Race/SkillsExtra", "Race/Skills"));
        assert!(!path_matches("Menu/Skills", "Race/Skills"));
        assert!(!path_matches("Race", ""));
    }
    #[test]
    fn smoothing_shortest_arc() {
        let mut p = Pose {
            yaw: 3.13,
            ..Pose::default()
        };
        p.approach(Pose { yaw: -3.13, ..p }, 0.016, 8.);
        assert!(p.yaw > 3.13 && p.yaw < 3.16);
    }
    #[test]
    fn smoothing_frame_rate_independent() {
        let mut a = Pose::default();
        let mut b = a;
        let t = Pose { distance: 80., ..a };
        for _ in 0..30 {
            a.approach(t, 1. / 30., 8.);
        }
        for _ in 0..120 {
            b.approach(t, 1. / 120., 8.);
        }
        assert!((a.distance - b.distance).abs() < 0.001);
    }
    #[test]
    fn orbit_looks_at_target() {
        let (p, q) = orbit(
            Pose {
                yaw: 0.,
                pitch: 0.,
                height: 0.,
                distance: 10.,
                ..Pose::default()
            },
            Vec3::default(),
            0.,
        );
        assert!(p.z < -9.99);
        let forward_z = 1. - 2. * (q[0] * q[0] + q[1] * q[1]);
        assert!(forward_z > 0.99);
    }
}

/// World-oriented displacement from the centroid of the current race pack.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct FreePose {
    pub offset: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub fov: f32,
}
impl FreePose {
    pub fn approach(&mut self, target: Self, dt: f32, response: f32) {
        let a = 1. - (-response.max(0.1) * dt.clamp(0., 0.1)).exp();
        let dy = (target.yaw - self.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        self.yaw += dy * a;
        self.pitch += (target.pitch - self.pitch) * a;
        self.fov += (target.fov - self.fov) * a;
        self.offset.x += (target.offset.x - self.offset.x) * a;
        self.offset.y += (target.offset.y - self.offset.y) * a;
        self.offset.z += (target.offset.z - self.offset.z) * a;
        self.sanitize();
    }
    pub fn from_orbit(p: Pose, target: Vec3, heading: f32, pack: Vec3) -> Self {
        let (eye, _) = orbit(p, target, heading);
        Self {
            offset: Vec3 {
                x: eye.x - pack.x,
                y: eye.y - pack.y,
                z: eye.z - pack.z,
            },
            yaw: heading + p.yaw,
            pitch: p.pitch,
            fov: p.fov,
        }
    }
    pub fn sanitize(&mut self) {
        self.yaw = finite(self.yaw, 0.).rem_euclid(std::f32::consts::TAU);
        self.pitch = finite(self.pitch, 0.).clamp(-1.55, 1.55);
        self.fov = finite(self.fov, 40.).clamp(5., 100.);
        self.offset.x = finite(self.offset.x, 0.).clamp(-10000., 10000.);
        self.offset.y = finite(self.offset.y, 5.).clamp(-10000., 10000.);
        self.offset.z = finite(self.offset.z, -20.).clamp(-10000., 10000.);
    }
    pub fn translate(&mut self, right: f32, up: f32, forward: f32, distance: f32) {
        let norm = (right * right + up * up + forward * forward).sqrt().max(1.);
        let d = distance / norm;
        self.offset.x += (right * self.yaw.cos() + forward * self.yaw.sin() * self.pitch.cos()) * d;
        self.offset.y += (up - forward * self.pitch.sin()) * d;
        self.offset.z +=
            (-right * self.yaw.sin() + forward * self.yaw.cos() * self.pitch.cos()) * d;
        self.sanitize();
    }
    pub fn camera(self, pack: Vec3) -> (Vec3, [f32; 4]) {
        let (sy, cy) = (self.yaw * 0.5).sin_cos();
        let (sx, cx) = (self.pitch * 0.5).sin_cos();
        (
            Vec3 {
                x: pack.x + self.offset.x,
                y: pack.y + self.offset.y,
                z: pack.z + self.offset.z,
            },
            [cy * sx, sy * cx, -sy * sx, cy * cx],
        )
    }
}
pub fn pack_center(points: impl Iterator<Item = Vec3>) -> Option<Vec3> {
    let mut sum = Vec3::default();
    let mut n = 0;
    for p in points {
        sum.x += p.x;
        sum.y += p.y;
        sum.z += p.z;
        n += 1;
    }
    (n > 0).then(|| Vec3 {
        x: sum.x / n as f32,
        y: sum.y / n as f32,
        z: sum.z / n as f32,
    })
}

#[cfg(test)]
mod freecam_tests {
    use super::*;
    fn near(a: f32, b: f32) {
        assert!((a - b).abs() < 0.0001, "{a} != {b}");
    }
    #[test]
    fn entering_freecam_preserves_orbit_view() {
        let pose = Pose {
            yaw: 0.7,
            pitch: -0.3,
            ..Pose::default()
        };
        let horse = Vec3 {
            x: 10.,
            y: 2.,
            z: -30.,
        };
        let pack = Vec3 {
            x: 12.,
            y: 1.,
            z: -25.,
        };
        let (eye, q) = orbit(pose, horse, 1.2);
        let free = FreePose::from_orbit(pose, horse, 1.2, pack);
        let (actual, r) = free.camera(pack);
        near(actual.x, eye.x);
        near(actual.y, eye.y);
        near(actual.z, eye.z);
        let dot: f32 = q.iter().zip(r).map(|(a, b)| a * b).sum();
        near(dot.abs(), 1.); // q and -q describe the same rotation.
    }
    #[test]
    fn pack_translation_moves_eye_without_rotating_or_reordering() {
        let a = Vec3 {
            x: 0.,
            y: 0.,
            z: 10.,
        };
        let b = Vec3 {
            x: 4.,
            y: 2.,
            z: 20.,
        };
        let center = pack_center([a, b].into_iter()).unwrap();
        let reversed = pack_center([b, a].into_iter()).unwrap();
        near(center.x, reversed.x);
        near(center.z, reversed.z);
        let free = FreePose::from_orbit(Pose::default(), a, 0., center);
        let (p, q) = free.camera(center);
        let (p2, q2) = free.camera(Vec3 {
            x: center.x + 7.,
            y: center.y + 1.,
            z: center.z - 3.,
        });
        near(p2.x - p.x, 7.);
        near(p2.y - p.y, 1.);
        near(p2.z - p.z, -3.);
        assert_eq!(q, q2);
        assert!(pack_center(std::iter::empty()).is_none());
    }
    #[test]
    fn flight_uses_view_axes_and_is_frame_rate_independent() {
        let mut free = FreePose {
            offset: Vec3::default(),
            yaw: std::f32::consts::FRAC_PI_2,
            pitch: 0.,
            fov: 40.,
        };
        free.translate(0., 0., 1., 5.);
        near(free.offset.x, 5.);
        near(free.offset.z, 0.);
        free.translate(1., 0., 0., 2.);
        near(free.offset.z, -2.);
        let mut a = free;
        let mut b = free;
        for _ in 0..30 {
            a.translate(0., 0., 1., 12. / 30.);
        }
        for _ in 0..120 {
            b.translate(0., 0., 1., 12. / 120.);
        }
        near(a.offset.x, b.offset.x);
    }
    #[test]
    fn saved_freecam_round_trips_and_legacy_orbit_shots_load() {
        let legacy = serde_json::json!({"name":"Old angle","pose":Pose::default()});
        let shot: Shot = serde_json::from_value(legacy).unwrap();
        assert!(shot.free.is_none());
        let free = FreePose::from_orbit(Pose::default(), Vec3::default(), 0., Vec3::default());
        let shot = Shot {
            free: Some(free),
            ..shot
        };
        let loaded: Shot = serde_json::from_str(&serde_json::to_string(&shot).unwrap()).unwrap();
        near(loaded.free.unwrap().offset.z, free.offset.z);
    }
}

#[cfg(test)]
mod free_smoothing_tests {
    use super::*;
    #[test]
    fn freecam_smooths_offset_rotation_fov_and_wraps_shortest_arc() {
        let start = FreePose {
            offset: Vec3::default(),
            yaw: std::f32::consts::TAU - 0.02,
            pitch: 0.,
            fov: 30.,
        };
        let target = FreePose {
            offset: Vec3 {
                x: 10.,
                y: 4.,
                z: -8.,
            },
            yaw: 0.02,
            pitch: 0.5,
            fov: 60.,
        };
        let mut a = start;
        a.approach(target, 1. / 60., 8.);
        assert!(a.offset.x > 0. && a.offset.x < 10.);
        assert!(a.offset.y > 0. && a.offset.y < 4.);
        assert!(a.offset.z < 0. && a.offset.z > -8.);
        assert!(a.pitch > 0. && a.pitch < 0.5);
        assert!(a.fov > 30. && a.fov < 60.);
        assert!(a.yaw > start.yaw || a.yaw < 0.02);
        let mut b = start;
        let mut c = start;
        for _ in 0..30 {
            b.approach(target, 1. / 30., 8.);
        }
        for _ in 0..120 {
            c.approach(target, 1. / 120., 8.);
        }
        for (x, y) in [
            (b.offset.x, c.offset.x),
            (b.offset.y, c.offset.y),
            (b.offset.z, c.offset.z),
            (b.yaw, c.yaw),
            (b.pitch, c.pitch),
            (b.fov, c.fov),
        ] {
            assert!((x - y).abs() < 0.0001);
        }
        let before = b.camera(Vec3::default()).0;
        let after = b
            .camera(Vec3 {
                x: 20.,
                y: 0.,
                z: 10.,
            })
            .0;
        assert!((after.x - before.x - 20.).abs() < 0.0001);
        assert!((after.z - before.z - 10.).abs() < 0.0001);
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Enabled {
    pub tower: bool,
    pub camera: bool,
    pub freecam: bool,
    pub hide_ui: bool,
    pub hide_bottom: bool,
    pub hide_skill_popups: bool,
    pub hide_markers: bool,
    pub skip_cutins: bool,
    pub hide_speed_lines: bool,
    pub hide_overhead: bool,
    pub hide_trees: bool,
    pub clean_effects: bool,
    pub graphics: bool,
    pub graphics_applied: crate::graphics::Quality,
}

#[cfg(test)]
mod appearance_tests {
    use super::*;
    #[test]
    fn legacy_defaults_to_light_and_dark_preference_round_trips() {
        let mut settings: Settings = serde_json::from_str("{}").unwrap();
        assert!(!settings.dark_mode);
        settings.dark_mode = true;
        let loaded: Settings =
            serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert!(loaded.dark_mode);
    }
}
