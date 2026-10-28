//! Procedural race tour and a world-fixed finish camera. No game calls here.
use crate::{
    model::{Pose, Vec3},
    timing::Runner,
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct FixedCamera {
    pub eye: Vec3,
    pub fov: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct DirectorSettings {
    pub enabled: bool,
    pub tour_enabled: bool,
    pub battles: bool,
    pub auto_finish: bool,
    pub ambient_motion: bool,
    pub motion_amount: f32,
    pub motion_period: f32,
    pub finish_right_side: bool,
    pub start_fraction: f32,
    pub dwell: f32,
    pub static_finish: bool,
    pub finish_camera: Option<FixedCamera>,
}
impl Default for DirectorSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            tour_enabled: true,
            battles: true,
            auto_finish: true,
            ambient_motion: true,
            motion_amount: 1.,
            motion_period: 24.,
            finish_right_side: false,
            start_fraction: 0.25,
            dwell: 6.,
            static_finish: true,
            finish_camera: None,
        }
    }
}
impl DirectorSettings {
    pub fn sanitize(&mut self) {
        self.start_fraction = bound(self.start_fraction, 0.25, 0.05, 0.75);
        self.dwell = bound(self.dwell, 6., 5., 15.);
        self.motion_amount = bound(self.motion_amount, 1., 0., 1.5);
        self.motion_period = bound(self.motion_period, 24., 12., 60.);
        self.finish_camera = self.finish_camera.filter(|c| {
            [c.eye.x, c.eye.y, c.eye.z]
                .iter()
                .all(|v| v.is_finite() && v.abs() < 100000.)
        });
        if let Some(c) = &mut self.finish_camera {
            c.fov = bound(c.fov, 30., 5., 100.);
        }
    }
}
fn bound(v: f32, d: f32, a: f32, b: f32) -> f32 {
    if v.is_finite() {
        v.clamp(a, b)
    } else {
        d
    }
}
/// Target selection has its own clock. Hold times use real viewing time,
/// accumulate only while telemetry advances, and cannot accelerate with replay.
#[derive(Default)]
pub struct Tour {
    visited: Vec<i32>,
    current: Option<i32>,
    last_distance: Option<f32>,
    elapsed: f32,
    last_progress: Option<f32>,
    since_progress: f32,
    done: bool,
    start_now: bool,
}
impl Tour {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn start_now(&mut self) {
        self.reset();
        self.start_now = true;
    }
    pub fn complete(&self) -> bool {
        self.done
    }
    pub fn cancel(&mut self) {
        self.done = true;
    }
    pub fn suspend(&mut self) {
        if self.current.is_some() {
            self.done = true;
        }
        self.last_progress = None;
        self.since_progress = 0.;
    }
    pub fn update(
        &mut self,
        rows: &[Runner],
        settings: &DirectorSettings,
        length: f32,
        preferred: i32,
        dt: f32,
    ) -> Option<i32> {
        if !settings.enabled || !settings.tour_enabled {
            self.reset();
            return None;
        }
        if self.done {
            return None;
        }
        let progress = rows
            .iter()
            .filter_map(|r| r.distance.filter(|d| d.is_finite() && *d >= 0.))
            .reduce(f32::max)?;
        self.since_progress = (self.since_progress + bound(dt, 0., 0., 0.1)).min(0.25);
        let old = self.last_progress.replace(progress);
        let usable = |r: &&Runner| r.distance.is_some_and(|d| d.is_finite() && d >= 0.);
        if self.current.is_none() {
            if !self.start_now && progress < tour_start(length, settings.start_fraction) {
                return None;
            }
            let first = rows
                .iter()
                .filter(usable)
                .find(|r| r.gate == preferred)
                .or_else(|| {
                    rows.iter()
                        .filter(usable)
                        .min_by_key(|r| r.rank.unwrap_or(99))
                })?;
            self.current = Some(first.gate);
            self.visited.push(first.gate);
            self.last_distance = first.distance;
            self.elapsed = 0.;
            self.since_progress = 0.;
        } else if old.is_some_and(|last| progress > last + 0.0001) {
            self.elapsed += self.since_progress;
            self.since_progress = 0.;
        }
        if let Some(current) = rows.iter().find(|r| Some(r.gate) == self.current) {
            if let Some(distance) = current.distance.filter(|d| d.is_finite()) {
                self.last_distance = Some(distance);
            }
        }
        let present = rows.iter().any(|r| Some(r.gate) == self.current);
        if self.elapsed >= bound(settings.dwell, 6., 5., 15.) || !present {
            let anchor = self.last_distance.unwrap_or(progress);
            let next = rows
                .iter()
                .filter(usable)
                .filter(|r| !self.visited.contains(&r.gate))
                .min_by(|a, b| {
                    (a.distance.unwrap() - anchor)
                        .abs()
                        .total_cmp(&(b.distance.unwrap() - anchor).abs())
                        .then(a.gate.cmp(&b.gate))
                });
            let Some(next) = next else {
                self.done = true;
                return None;
            };
            self.current = Some(next.gate);
            self.visited.push(next.gate);
            self.last_distance = next.distance;
            self.elapsed = 0.;
            self.since_progress = 0.;
        }
        self.current
    }
}
/// A continuous camera rig, independent of target changes and tour progress.
/// Starts immediately, including in a paused race; switching runners still waits
/// for racing motion. The saved operator pose is never modified.
#[derive(Default)]
pub struct AmbientMotion {
    phase: f64,
}
impl AmbientMotion {
    pub fn reset(&mut self) {
        self.phase = 0.;
    }
    pub fn apply(&mut self, mut pose: Pose, settings: &DirectorSettings, dt: f32) -> Pose {
        if !settings.enabled || !settings.ambient_motion {
            self.reset();
            return pose;
        }
        self.phase += bound(dt, 0., 0., 0.1) as f64 * std::f64::consts::TAU
            / bound(settings.motion_period, 24., 12., 60.) as f64;
        let phase = self.phase;
        let amount = bound(settings.motion_amount, 1., 0., 1.5);
        pose.yaw += amount * 0.26 * phase.sin() as f32;
        pose.pitch += amount * 0.018 * (phase * 0.79).sin() as f32;
        pose.distance *= 1. + amount * 0.12 * (phase * 0.91).sin() as f32;
        pose.height += amount * 0.22 * (phase * 0.67).sin() as f32;
        // Separate rates avoid the dolly and lens perfectly cancelling each other.
        pose.fov *= 1. + amount * 0.10 * (phase * 1.27).sin() as f32;
        pose.sanitize();
        pose
    }
}
/// Reciprocal focus/rival selections describe the same camera shot.
pub fn pair_key(a: i32, b: i32) -> i32 {
    -(a.min(b) * 128 + a.max(b))
}
/// Keep the pair orientation unchanged when its focus/rival roles swap.
pub fn pair_heading(a: f32, b: f32) -> f32 {
    (a.sin() + b.sin()).atan2(a.cos() + b.cos())
}

/// The same decision path is used by the game callback and race simulations.
/// Keeping activation, priority and pairing here prevents separate UI flags from
/// silently disabling an otherwise enabled director.
#[derive(Default)]
pub struct Controller {
    finish: crate::finish::FinishDirector,
    tour: Tour,
    motion: AmbientMotion,
}
pub struct Coverage {
    pub gate: i32,
    pub pose: Pose,
    pub finish: Option<crate::finish::Selection>,
    pub touring: bool,
    pub pairing: crate::battle::BattleSettings,
    pub transition: f32,
}
impl Controller {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn update(&mut self, s: &mut crate::state::State, available: &[i32], dt: f32) -> Coverage {
        let mut output = Coverage {
            gate: s.gate,
            pose: s.settings.pose,
            finish: None,
            touring: false,
            pairing: battle_settings(&s.settings.director, &s.settings.battle),
            transition: if s.settings.director.enabled {
                s.settings.target_transition.max(1.8)
            } else {
                s.settings.target_transition
            },
        };
        s.coverage_gate = None;
        if !s.camera || !s.race {
            self.reset();
            s.director_status = if s.race {
                "Camera off / enable Camera control [F6]"
            } else {
                "Waiting for a race"
            }
            .into();
            return output;
        }
        if s.freecam {
            self.finish.reset();
            self.tour.suspend();
            s.director_status = "Freecam / automatic directing suspended".into();
            return output;
        }
        if s.restart_tour {
            self.tour.start_now();
            s.restart_tour = false;
        }
        let mut finish_settings = s.settings.battle.clone();
        finish_settings.enabled |= s.settings.director.enabled;
        output.finish = self
            .finish
            .update(
                &s.timing_rows,
                &s.finished_gates,
                &finish_settings,
                s.race_distance(),
            )
            .filter(|selection| available.contains(&selection.leader));
        let tour = if output.finish.is_some() {
            self.tour.cancel();
            None
        } else if !s.timing_rows.is_empty() && s.finished_gates.len() == s.timing_rows.len() {
            self.tour.cancel();
            None
        } else {
            self.tour
                .update(
                    &s.timing_rows,
                    &s.settings.director,
                    s.race_distance(),
                    s.gate,
                    dt,
                )
                .filter(|gate| available.contains(gate))
        };
        s.coverage_gate = output.finish.map(|selection| selection.leader).or(tour);
        output.gate = s.coverage_gate.unwrap_or(s.gate);
        output.touring = tour.is_some();
        output.pose = if output.finish.is_some() && s.settings.battle.finish_front_view {
            crate::finish::front_shot(s.settings.pose, s.settings.director.finish_right_side)
        } else {
            s.settings.pose
        };
        output.pose = self.motion.apply(output.pose, &s.settings.director, dt);
        s.director_status = if output.finish.is_some() {
            "Finish / moving lead battle".into()
        } else if let Some(gate) = tour {
            format!("Runner tour / gate {gate}")
        } else if !s.settings.director.enabled {
            "Director off".into()
        } else if s.timing_rows.is_empty() {
            "Waiting for runner telemetry".into()
        } else if !s
            .timing_rows
            .iter()
            .any(|r| r.rank == Some(1) && r.distance.is_some_and(|d| d.is_finite()))
        {
            "Waiting for leader position and distance".into()
        } else if self.tour.complete() {
            "Tour complete / returned to selected runner".into()
        } else if !s.settings.director.tour_enabled {
            "Runner tour off / following selected runner".into()
        } else {
            format!(
                "Tour starts at {:.0} m",
                tour_start(s.race_distance(), s.settings.director.start_fraction)
            )
        };
        output
    }
}

/// Zero is an explicit Auto setting, never a guessed race length.
pub fn race_length(manual: f32, detected: Option<f32>) -> f32 {
    let valid = |v: f32| v.is_finite() && (100. ..=10000.).contains(&v);
    if valid(manual) {
        manual
    } else {
        detected.filter(|v| valid(*v)).unwrap_or(0.)
    }
}
pub fn tour_start(length: f32, fraction: f32) -> f32 {
    if length.is_finite() && length > 0. {
        length * fraction
    } else {
        200.
    }
}
/// Automatic directing owns pairing independently of the manual Battle toggle.
/// A pinned manual partner must not leak into the runner tour.
pub fn battle_settings(
    director: &DirectorSettings,
    manual: &crate::battle::BattleSettings,
) -> crate::battle::BattleSettings {
    let mut effective = manual.clone();
    if director.enabled {
        effective.enabled = director.battles;
        effective.partner_gate = None;
    }
    effective
}

/// One estimated finish camera per race. Only place it in the last 120 metres,
/// using observed travel direction and the course-to-world scale. This estimates
/// the finish straight; it is not a surveyed finish-line transform.
#[derive(Default)]
pub struct AutoFinish {
    camera: Option<FixedCamera>,
    sample: Option<(i32, f32, Vec3)>,
    measured_scale: Option<f32>,
    direction: Option<[f32; 2]>,
    placed_right: bool,
    camera_right: [f32; 2],
}
impl AutoFinish {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn clear_camera(&mut self) {
        self.camera = None;
    }
    pub fn observe(&mut self, gate: i32, progress: f32, position: Vec3) {
        if !progress.is_finite() || !finite_point(position) {
            return;
        }
        if let Some((old_gate, old_distance, old_position)) = self.sample {
            let distance = progress - old_distance;
            if old_gate == gate && (0.2..=20.).contains(&distance) {
                let world = (position.x - old_position.x).hypot(position.z - old_position.z);
                if let Some(scale) = valid_scale(Some(world / distance)) {
                    let direction = [
                        (position.x - old_position.x) / world,
                        (position.z - old_position.z) / world,
                    ];
                    self.direction = Some(direction);
                    self.measured_scale = Some(
                        self.measured_scale
                            .map_or(scale, |old| old * 0.8 + scale * 0.2),
                    );
                }
            }
            // Keep a baseline through paused/render-only frames.
            if old_gate == gate && (0. ..0.2).contains(&distance) {
                return;
            }
        }
        self.sample = Some((gate, progress, position));
    }
    pub fn place(
        &mut self,
        position: Vec3,
        remaining: f32,
        reported_scale: Option<f32>,
        right_side: bool,
    ) -> Option<FixedCamera> {
        if let Some(camera) = &mut self.camera {
            if self.placed_right != right_side {
                let change = if right_side { 28. } else { -28. };
                camera.eye.x += self.camera_right[0] * change;
                camera.eye.z += self.camera_right[1] * change;
                self.placed_right = right_side;
            }
            return self.camera;
        }
        if !finite_point(position) || !remaining.is_finite() || !(0. ..=120.).contains(&remaining) {
            return None;
        }
        let scale = valid_scale(reported_scale).or(self.measured_scale)?;
        let [sin, cos] = self.direction?;
        let forward = remaining * scale + 16.;
        let side = if right_side { 14. } else { -14. };
        self.placed_right = right_side;
        self.camera_right = [cos, -sin];
        self.camera = Some(FixedCamera {
            eye: Vec3 {
                x: position.x + sin * forward + cos * side,
                y: position.y + 5.,
                z: position.z + cos * forward - sin * side,
            },
            fov: 30.,
        });
        self.camera
    }
}
fn finite_point(p: Vec3) -> bool {
    [p.x, p.y, p.z].into_iter().all(|v| v.is_finite())
}
pub fn valid_scale(v: Option<f32>) -> Option<f32> {
    v.filter(|v| v.is_finite() && (0.01..=100.).contains(v))
}

#[derive(Default)]
pub struct FixedTracker {
    start: Option<Vec3>,
    destination: Option<Vec3>,
    shown: Option<Vec3>,
    aim: Option<Vec3>,
    elapsed: f32,
    fov: Option<f32>,
}
impl FixedTracker {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    /// Maintain subject scale as the runners approach a fixed camera. Widen
    /// immediately to protect framing; ease back into a tighter lens.
    pub fn lens(
        &mut self,
        eye: Vec3,
        aim: Vec3,
        a: Vec3,
        b: Vec3,
        reference_fov: f32,
        aspect: f32,
        dt: f32,
    ) -> f32 {
        let distance = (eye.x - aim.x)
            .hypot(eye.z - aim.z)
            .hypot(eye.y - aim.y)
            .max(0.1);
        let reference = bound(reference_fov, 30., 5., 100.);
        let preferred = (2. * ((reference.to_radians() * 0.5).tan() * 30. / distance).atan())
            .to_degrees()
            .clamp(5., 100.);
        let required = crate::battle::grounded_fov(eye, aim, a, b, preferred, aspect);
        let shown = self.fov.get_or_insert(reference);
        *shown += (required - *shown) * (1. - (-4. * bound(dt, 0., 0., 0.1)).exp());
        *shown = shown.max(required);
        *shown
    }
    pub fn view(
        &mut self,
        current: Vec3,
        camera: FixedCamera,
        target: Vec3,
        dt: f32,
        duration: f32,
    ) -> (Vec3, Vec3) {
        if self
            .destination
            .is_some_and(|p| p.x != camera.eye.x || p.y != camera.eye.y || p.z != camera.eye.z)
        {
            self.start = self.shown.or(Some(current));
            self.elapsed = 0.;
        }
        self.destination = Some(camera.eye);
        let start = *self.start.get_or_insert(current);
        let dt = bound(dt, 0., 0., 0.1);
        self.elapsed += dt;
        let t = if duration > 0. {
            (self.elapsed / duration).clamp(0., 1.)
        } else {
            1.
        };
        let t = t * t * (3. - 2. * t);
        let lerp = |a: Vec3, b: Vec3, t: f32| Vec3 {
            x: a.x + (b.x - a.x) * t,
            y: a.y + (b.y - a.y) * t,
            z: a.z + (b.z - a.z) * t,
        };
        let eye = lerp(start, camera.eye, t);
        self.shown = Some(eye);
        let a = self.aim.get_or_insert(target);
        *a = lerp(*a, target, 1. - (-10. * dt).exp());
        (eye, *a)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn rows(d: f32) -> Vec<Runner> {
        (1..=3)
            .map(|g| Runner {
                gate: g,
                name: format!("Runner {g}"),
                trainer_name: None,
                rank: Some(g),
                distance: Some(d - g as f32 + 1.),
                speed: Some(20.),
            })
            .collect()
    }
    fn active_state() -> crate::state::State {
        let mut s = crate::state::State::from_settings(crate::model::Settings::default());
        s.race = true;
        s.gate = 3;
        s.freecam = true;
        s.start_director();
        s
    }
    #[test]
    fn controller_tour_is_continuous_pairs_and_returns_to_the_selected_runner() {
        let mut s = active_state();
        let mut controller = Controller::default();
        let mut battle = crate::battle::BattleTracker::default();
        s.detected_race_distance = Some(2000.);
        assert!(s.camera && !s.freecam && s.needs_telemetry());
        assert!(!s.settings.battle.enabled && s.settings.broadcast.race_distance_m == 0.);
        let original = s.settings.pose;
        let mut seen = vec![];
        let mut previous: Option<Pose> = None;
        for i in 0..1500 {
            // 10 Hz telemetry rendered at 60 FPS, matching runtime.
            s.timing_rows = rows(500. + (i / 6) as f32 * 2.);
            let frame = controller.update(&mut s, &[1, 2, 3], 1. / 60.);
            assert!(frame.transition >= 1.8);
            if frame.touring {
                if seen.last() != Some(&frame.gate) {
                    seen.push(frame.gate);
                }
                assert!(frame.pairing.enabled);
                assert!(battle
                    .update(&s.timing_rows, frame.gate, &frame.pairing, 1. / 60.)
                    .is_some());
            }
            if let Some(previous) = previous {
                // No angle/lens jump at any tour entry, target switch or return.
                assert!((frame.pose.yaw - previous.yaw).abs() < 0.003);
                assert!((frame.pose.fov - previous.fov).abs() < 0.02);
            }
            previous = Some(frame.pose);
        }
        assert_eq!(seen, vec![3, 2, 1]);
        let follow = controller.update(&mut s, &[1, 2, 3], 1. / 60.);
        assert!(!follow.touring && follow.finish.is_none());
        assert_eq!(follow.gate, 3);
        assert_eq!(s.settings.pose.yaw, original.yaw);
        assert_eq!(s.settings.pose.fov, original.fov);
        assert_ne!(follow.pose.yaw, original.yaw); // Movement continues after the tour.
        s.timing_rows = rows(1600.);
        let finish = controller.update(&mut s, &[1, 2, 3], 1. / 60.);
        assert_eq!(finish.finish.unwrap().leader, 1);
        assert_eq!(finish.finish.unwrap().rival, Some(2));
        assert!(!finish.touring);
        assert_eq!(s.gate, 3);
    }
    #[test]
    fn ambient_motion_starts_before_the_tour_and_moves_without_target_changes() {
        let mut s = active_state();
        let mut c = Controller::default();
        s.detected_race_distance = Some(2000.);
        s.timing_rows = rows(1.);
        let first = c.update(&mut s, &[1, 2, 3], 1. / 60.);
        let mut last = first;
        for _ in 0..180 {
            let next = c.update(&mut s, &[1, 2, 3], 1. / 60.);
            assert_eq!(next.gate, 3);
            assert!(!next.touring);
            assert!((next.pose.yaw - last.pose.yaw).abs() < 0.003);
            last = next;
        }
        assert!((last.pose.yaw - s.settings.pose.yaw).abs() > 0.15);
        assert!((last.pose.distance - s.settings.pose.distance).abs() > 2.);
        assert!((last.pose.fov - s.settings.pose.fov).abs() > 0.5);
        s.settings.director.ambient_motion = false;
        let still = c.update(&mut s, &[1, 2, 3], 1. / 60.);
        assert_eq!(still.pose.yaw, s.settings.pose.yaw);
        assert_eq!(still.pose.distance, s.settings.pose.distance);
    }
    #[test]
    fn ambient_motion_is_frame_rate_independent_and_survives_telephoto_framing() {
        let s = DirectorSettings {
            enabled: true,
            ..Default::default()
        };
        let pose = Pose {
            fov: 5.,
            distance: 65.,
            pitch: 0.024,
            ..Default::default()
        };
        let mut result = vec![];
        for hz in [30, 60, 120] {
            let mut motion = AmbientMotion::default();
            let mut shown = pose;
            for _ in 0..hz * 3 {
                shown = motion.apply(pose, &s, 1. / hz as f32);
            }
            result.push(shown);
        }
        assert!((result[0].yaw - result[2].yaw).abs() < 0.0001);
        assert!((result[0].distance - result[2].distance).abs() < 0.0001);
        assert!(result[0].fov > 5.3);
        assert!(result[0].distance > 69.);
    }
    #[test]
    fn missing_length_keeps_tour_pause_and_manual_takeover_working() {
        let mut s = active_state();
        let mut c = Controller::default();
        s.timing_rows = rows(100.);
        assert!(!c.update(&mut s, &[1, 2, 3], 0.1).touring);
        assert!(s.distance_status().contains("unavailable"));
        s.timing_rows = rows(200.);
        let first = c.update(&mut s, &[1, 2, 3], 0.1);
        assert!(first.touring);
        let mut last_yaw = first.pose.yaw;
        for _ in 0..300 {
            let frame = c.update(&mut s, &[1, 2, 3], 0.1);
            assert_eq!(frame.gate, first.gate);
            last_yaw = frame.pose.yaw;
        }
        assert_ne!(last_yaw, first.pose.yaw); // Paused race holds target, not ambient camera motion.
        s.freecam = true;
        assert!(!c.update(&mut s, &[1, 2, 3], 0.1).touring);
        assert!(s.director_status.contains("Freecam"));
        s.start_director();
        s.restart_tour = true;
        assert!(c.update(&mut s, &[1, 2, 3], 0.1).touring);
        s.settings.director.enabled = false;
        let manual = c.update(&mut s, &[1, 2, 3], 0.1);
        assert!(!manual.touring && !manual.pairing.enabled);
        assert_eq!(manual.gate, 3);
        assert_eq!(manual.pose.yaw, s.settings.pose.yaw);
        assert_eq!(manual.transition, s.settings.target_transition);
    }
    #[test]
    fn restart_now_and_finish_priority_use_the_runtime_controller() {
        let mut s = active_state();
        let mut c = Controller::default();
        s.detected_race_distance = Some(2000.);
        s.timing_rows = rows(20.);
        s.restart_tour = true;
        assert!(c.update(&mut s, &[1, 2, 3], 0.1).touring);
        s.timing_rows = rows(1600.);
        s.restart_tour = true;
        let finish = c.update(&mut s, &[1, 2, 3], 0.1);
        assert!(finish.finish.is_some() && !finish.touring);
        s.camera = false;
        assert!(c.update(&mut s, &[1, 2, 3], 0.1).finish.is_none());
        assert!(s.director_status.contains("Camera off"));
        s.start_director();
        s.timing_rows = rows(100.);
        c.reset();
        assert!(c.update(&mut s, &[1, 2, 3], 0.1).finish.is_none());
    }
    #[test]
    fn automatic_pairing_ignores_manual_pins_and_respects_director_opt_out() {
        let mut s = DirectorSettings {
            enabled: true,
            ..Default::default()
        };
        let manual = crate::battle::BattleSettings {
            enabled: true,
            partner_gate: Some(9),
            ..Default::default()
        };
        assert_eq!(battle_settings(&s, &manual).partner_gate, None);
        s.battles = false;
        assert!(!battle_settings(&s, &manual).enabled);
        s.enabled = false;
        assert_eq!(battle_settings(&s, &manual).partner_gate, Some(9));
        assert!(battle_settings(&s, &manual).enabled);
    }
    #[test]
    fn reciprocal_battle_focus_does_not_restart_the_camera_tween() {
        let mut tween = crate::camera_motion::TargetTween::default();
        let p = Vec3::default();
        let first = pair_heading(3.13, -3.13);
        let swapped = pair_heading(-3.13, 3.13);
        assert!((first - swapped).abs() < 0.00001);
        assert!(first.abs() > 3.13);
        tween.follow(pair_key(5, 6), p, first, p, 0., 1.8);
        tween.follow(pair_key(6, 5), p, swapped, p, 0.1, 1.8);
        assert!(tween.settled());
        tween.follow(pair_key(9, 4), Vec3 { x: 20., ..p }, 0., p, 0.1, 1.8);
        assert!(!tween.settled());
    }
    #[test]
    fn automatic_finish_defaults_to_the_other_side_and_can_mirror_in_place() {
        let mut camera = AutoFinish::default();
        let p = Vec3 {
            x: 10.,
            y: 2.,
            z: 20.,
        };
        let side = DirectorSettings::default().finish_right_side;
        assert!(!side);
        assert!(camera.place(p, 400., Some(1.), side).is_none());
        assert!(camera.place(p, 120., Some(1.), side).is_none());
        camera.observe(1, 1878., Vec3 { z: 18., ..p });
        camera.observe(1, 1880., p);
        let spot = camera.place(p, 120., Some(1.), side).unwrap();
        assert_eq!(spot.eye.x, -4.);
        assert_eq!(spot.eye.y, 7.);
        assert_eq!(spot.eye.z, 156.);
        let held = camera
            .place(Vec3 { z: 200., ..p }, -30., Some(2.), side)
            .unwrap();
        assert_eq!(held.eye.z, spot.eye.z);
        let flipped = camera.place(p, -30., None, true).unwrap();
        assert_eq!(flipped.eye.x, 24.);
        assert_eq!(flipped.eye.z, spot.eye.z);
        camera.reset();
        assert!(camera.place(p, -1., Some(1.), side).is_none());
        camera.observe(1, 1878., Vec3 { x: 8., ..p });
        camera.observe(1, 1880., p);
        let rotated = camera.place(p, 120., Some(1.), side).unwrap();
        assert!((rotated.eye.x - 146.).abs() < 0.001);
        assert!((rotated.eye.z - 34.).abs() < 0.001);
    }
    #[test]
    fn automatic_finish_can_measure_scale_but_rejects_unusable_geometry() {
        let mut c = AutoFinish::default();
        let p = Vec3::default();
        c.observe(1, 1800., p);
        for _ in 0..50 {
            c.observe(1, 1800., p);
        }
        c.observe(1, 1802., Vec3 { z: 4., ..p });
        let spot = c.place(p, 100., None, false).unwrap();
        assert_eq!(spot.eye.z, 216.);
        c.reset();
        c.observe(1, 1800., p);
        c.observe(2, 1802., Vec3 { z: 100., ..p });
        assert!(c.place(p, 100., None, false).is_none());
        for invalid in [f32::NAN, f32::INFINITY, -1., 0.] {
            assert!(c.place(p, 100., Some(invalid), false).is_none());
        }
        assert!(c
            .place(Vec3 { x: f32::NAN, ..p }, 100., Some(1.), false)
            .is_none());
    }
    #[test]
    fn tour_holds_use_viewing_time_not_replay_speed() {
        let settings = DirectorSettings {
            enabled: true,
            dwell: 6.,
            ..Default::default()
        };
        let mut switch_frames = vec![];
        for multiplier in [1., 4.] {
            let mut tour = Tour::default();
            assert_eq!(tour.update(&rows(500.), &settings, 2000., 1, 0.1), Some(1));
            for frame in 1..100 {
                let selected = tour.update(
                    &rows(500. + frame as f32 * 2. * multiplier),
                    &settings,
                    2000.,
                    1,
                    0.1,
                );
                if selected != Some(1) {
                    switch_frames.push(frame);
                    break;
                }
            }
        }
        assert_eq!(switch_frames[0], switch_frames[1]);
        assert!(switch_frames[0] >= 60);
    }
    #[test]
    fn tour_visits_nearest_unvisited_runner_and_never_catches_up_after_pause() {
        let settings = DirectorSettings {
            enabled: true,
            dwell: 5.,
            ..Default::default()
        };
        let mut tour = Tour::default();
        let mut field = rows(500.);
        // Gate 3 is much closer to gate 1 than gate 2, despite race-order listing.
        field[1].distance = Some(450.);
        field[2].distance = Some(499.);
        assert_eq!(tour.update(&field, &settings, 2000., 1, 0.1), Some(1));
        for _ in 0..1000 {
            assert_eq!(tour.update(&field, &settings, 2000., 1, 0.1), Some(1));
        }
        let mut seen = vec![1];
        for _ in 0..170 {
            for r in &mut field {
                *r.distance.as_mut().unwrap() += 2.;
            }
            if let Some(gate) = tour.update(&field, &settings, 2000., 1, 0.1) {
                if seen.last() != Some(&gate) {
                    seen.push(gate);
                }
            }
        }
        assert_eq!(seen, vec![1, 3, 2]);
        assert!(tour.complete());
        tour.reset();
        assert!(tour.update(&rows(100.), &settings, 2000., 1, 0.1).is_none());
    }
    #[test]
    fn fixed_camera_stays_in_world_space_while_aim_moves() {
        let mut t = FixedTracker::default();
        let c = FixedCamera {
            eye: Vec3 {
                x: 10.,
                y: 4.,
                z: 30.,
            },
            fov: 30.,
        };
        for _ in 0..30 {
            t.view(Vec3::default(), c, Vec3::default(), 0.1, 0.7);
        }
        let (eye, a) = t.view(
            Vec3 {
                x: 100.,
                y: 9.,
                z: 60.,
            },
            c,
            Vec3 {
                x: 20.,
                y: 0.,
                z: 0.,
            },
            0.1,
            0.7,
        );
        assert_eq!(eye.x, c.eye.x);
        assert_eq!(eye.z, c.eye.z);
        assert!(a.x > 0. && a.x < 20.);
    }
    #[test]
    fn changing_fixed_camera_side_eases_from_the_displayed_position() {
        let mut tracker = FixedTracker::default();
        let p = Vec3::default();
        let left = FixedCamera {
            eye: Vec3 {
                x: -14.,
                y: 5.,
                z: 100.,
            },
            fov: 30.,
        };
        for _ in 0..30 {
            tracker.view(p, left, p, 0.1, 1.8);
        }
        let right = FixedCamera {
            eye: Vec3 { x: 14., ..left.eye },
            ..left
        };
        let (first, _) = tracker.view(p, right, p, 0.1, 1.8);
        assert!(first.x > -14. && first.x < -13.);
        for _ in 0..30 {
            tracker.view(p, right, p, 0.1, 1.8);
        }
        assert_eq!(tracker.view(p, right, p, 0.1, 1.8).0.x, 14.);
    }
    #[test]
    fn fixed_lens_tracks_approach_and_eases_zoom_in() {
        let mut tracker = FixedTracker::default();
        let runner = Vec3::default();
        let aim = Vec3 { y: 1.5, ..runner };
        let far = Vec3 {
            z: 300.,
            y: 4.,
            ..runner
        };
        let near = Vec3 {
            z: 8.,
            y: 4.,
            ..runner
        };
        let initial = tracker.lens(far, aim, runner, runner, 30., 16. / 9., 0.1);
        for _ in 0..100 {
            tracker.lens(far, aim, runner, runner, 30., 16. / 9., 0.1);
        }
        let distant = tracker.lens(far, aim, runner, runner, 30., 16. / 9., 0.1);
        assert!(distant >= 5. && distant < initial && initial < 30.);
        let close = tracker.lens(near, aim, runner, runner, 30., 16. / 9., 0.1);
        assert!(close > distant && close <= 100.);
        let retreat = tracker.lens(far, aim, runner, runner, 30., 16. / 9., 0.1);
        assert!(retreat < close && retreat > distant);
    }
    #[test]
    fn saved_settings_are_bounded_and_round_trip() {
        let mut s = DirectorSettings {
            dwell: f32::NAN,
            start_fraction: 3.,
            finish_camera: Some(FixedCamera {
                eye: Vec3 {
                    x: f32::NAN,
                    ..Default::default()
                },
                fov: 20.,
            }),
            ..Default::default()
        };
        s.sanitize();
        assert!(s.finish_camera.is_none());
        assert_eq!(s.dwell, 6.);
        assert_eq!(s.start_fraction, 0.75);
        let read: DirectorSettings =
            serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert!(!read.enabled);
        let mut legacy: DirectorSettings =
            serde_json::from_str(r#"{"dwell":2.5,"enabled":true}"#).unwrap();
        legacy.sanitize();
        assert_eq!(legacy.dwell, 5.);
        assert!(legacy.ambient_motion);
        assert!(!legacy.finish_right_side);
        assert_eq!(legacy.motion_period, 24.);
    }
}
