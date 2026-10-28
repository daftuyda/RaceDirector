//! Stable two-runner selection and framing. This module never reads game memory.
use crate::model::{Pose, Vec3};
use crate::timing::Runner;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct BattleSettings {
    pub enabled: bool,
    pub finish_coverage: bool,
    pub finish_distance: f32,
    pub finish_front_view: bool,
    /// None chooses a nearby contender; Some pins an explicit gate.
    pub partner_gate: Option<i32>,
    pub hold_seconds: f32,
    /// Maximum track-distance separation for an eligible pair, in metres.
    pub max_gap: f32,
}
impl Default for BattleSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            finish_coverage: false,
            finish_distance: 400.,
            finish_front_view: true,
            partner_gate: None,
            hold_seconds: 4.,
            max_gap: 30.,
        }
    }
}
impl BattleSettings {
    pub fn sanitize(&mut self) {
        self.finish_distance = bounded(self.finish_distance, 400., 50., 1000.);
        self.partner_gate = self.partner_gate.filter(|gate| (1..=64).contains(gate));
        self.hold_seconds = bounded(self.hold_seconds, 4., 1., 15.);
        self.max_gap = bounded(self.max_gap, 30., 5., 100.);
    }
}
#[derive(Clone, Copy, Debug)]
pub struct BattleInfo {
    pub anchor: i32,
    pub partner: i32,
    pub position: Option<i32>,
    pub gap_m: Option<f32>,
    /// Trailing runner's speed minus leading runner's speed; positive is closing.
    pub closing_mps: Option<f32>,
}
#[derive(Default)]
pub struct BattleTracker {
    anchor: Option<i32>,
    partner: Option<i32>,
    requested_partner: Option<i32>,
    held: f32,
    last_distance: Option<f32>,
}
impl BattleTracker {
    /// Call on race identity changes, exit/entry, replay seek, or camera mode changes.
    /// The runtime owns full-roster identity; rows here may exclude finishers.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn update(
        &mut self,
        rows: &[Runner],
        anchor: i32,
        settings: &BattleSettings,
        dt: f32,
    ) -> Option<BattleInfo> {
        if !settings.enabled {
            self.reset();
            return None;
        }
        let Some(selected) = rows.iter().find(|r| r.gate == anchor && usable(r)) else {
            self.reset();
            return None;
        };
        let requested = settings.partner_gate.filter(|gate| (1..=64).contains(gate));
        let progress = selected.distance.unwrap();
        if self.anchor != Some(anchor)
            || self.requested_partner != requested
            || self.last_distance.is_some_and(|last| progress < last - 5.)
        {
            self.reset();
            self.anchor = Some(anchor);
            self.requested_partner = requested;
        }
        self.last_distance = Some(progress);
        self.held = (self.held + bounded(dt, 0., 0., 0.25)).min(60.);
        let max_gap = bounded(settings.max_gap, 30., 5., 100.);
        let hold = bounded(settings.hold_seconds, 4., 1., 15.);
        let separation = |r: &Runner| (r.distance.unwrap() - progress).abs();
        let eligible = |r: &&Runner| r.gate != anchor && usable(r) && separation(r) <= max_gap;
        let partner = if let Some(gate) = requested {
            // A missing/too-distant explicit target stays unavailable; do not
            // quietly change the operator's chosen pairing.
            rows.iter().filter(eligible).find(|r| r.gate == gate)
        } else {
            let current = rows
                .iter()
                .filter(eligible)
                .find(|r| Some(r.gate) == self.partner);
            let candidate = rows
                .iter()
                .filter(eligible)
                .filter(|r| (r.rank.unwrap() - selected.rank.unwrap()).abs() == 1)
                .min_by(|a, b| {
                    separation(a)
                        .total_cmp(&separation(b))
                        .then(a.rank.cmp(&b.rank))
                        .then(a.gate.cmp(&b.gate))
                });
            match (current, candidate) {
                // Keep a battle intact when its runners exchange positions.
                (Some(current), _) if self.held < hold => Some(current),
                (Some(current), Some(candidate)) => {
                    let current_gap = separation(current);
                    let candidate_gap = separation(candidate);
                    if (current.rank.unwrap() - selected.rank.unwrap()).abs() > 1
                        || (candidate_gap + 1. < current_gap && candidate_gap < current_gap * 0.8)
                    {
                        Some(candidate)
                    } else {
                        Some(current)
                    }
                }
                (None, candidate) => candidate,
                (Some(current), None)
                    if (current.rank.unwrap() - selected.rank.unwrap()).abs() == 1 =>
                {
                    Some(current)
                }
                _ => None,
            }
        };
        let Some(partner) = partner else {
            self.partner = None;
            self.held = 0.;
            return None;
        };
        if self.partner != Some(partner.gate) {
            self.partner = Some(partner.gate);
            self.held = 0.;
        }
        let (ahead, behind) = if selected.rank <= partner.rank {
            (selected, partner)
        } else {
            (partner, selected)
        };
        let signed_gap = ahead.distance.unwrap() - behind.distance.unwrap();
        // Finish order and track progress may disagree. Do not manufacture an
        // interval or closing claim from inconsistent telemetry.
        let gap_m = (signed_gap >= -0.05).then_some(signed_gap.max(0.));
        let closing_mps =
            gap_m.and_then(|_| Some(nonnegative(behind.speed)? - nonnegative(ahead.speed)?));
        Some(BattleInfo {
            anchor,
            partner: partner.gate,
            position: ahead.rank,
            gap_m,
            closing_mps,
        })
    }
}
fn nonnegative(value: Option<f32>) -> Option<f32> {
    value.filter(|v| v.is_finite() && *v >= 0.)
}
fn usable(runner: &Runner) -> bool {
    (1..=64).contains(&runner.gate)
        && runner.rank.is_some_and(|rank| (1..=64).contains(&rank))
        && nonnegative(runner.distance).is_some()
}
fn bounded(value: f32, fallback: f32, min: f32, max: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}
fn finite_point(p: Vec3) -> bool {
    p.x.is_finite() && p.y.is_finite() && p.z.is_finite()
}
fn dot(a: Vec3, b: Vec3) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

/// Keep the operator's angle and preferred distance, expanding the framing to
/// contain two runners. Uses vertical camera FOV and the actual viewport aspect.
/// Existing runtime smoothing and ground avoidance must run on the result.
pub fn frame(mut pose: Pose, a: Vec3, b: Vec3, heading: f32, aspect: f32) -> (Pose, Vec3) {
    pose.sanitize();
    if !finite_point(a) || !finite_point(b) {
        let target = if finite_point(a) {
            a
        } else if finite_point(b) {
            b
        } else {
            Vec3::default()
        };
        return (pose, target);
    }
    // Half each operand first to avoid overflow in the midpoint addition.
    let target = Vec3 {
        x: a.x * 0.5 + b.x * 0.5,
        y: a.y * 0.5 + b.y * 0.5,
        z: a.z * 0.5 + b.z * 0.5,
    };
    (fit_at(pose, a, b, target, heading, aspect), target)
}

/// Expand a smoothed camera at its actual (possibly tweening) anchor so the
/// selected pair remains inside the frame during a transition. Keep the caller's
/// smoothing state separately; apply this returned pose to the rendered view.
pub fn fit_at(mut pose: Pose, a: Vec3, b: Vec3, target: Vec3, heading: f32, aspect: f32) -> Pose {
    pose.sanitize();
    if !finite_point(a) || !finite_point(b) || !finite_point(target) {
        return pose;
    }
    let yaw = bounded(heading, 0., -1.0e6, 1.0e6) + pose.yaw;
    let (sy, cy) = yaw.sin_cos();
    let (sp, cp) = pose.pitch.sin_cos();
    let forward = Vec3 {
        x: sy * cp,
        y: -sp,
        z: cy * cp,
    };
    let right = Vec3 {
        x: cy,
        y: 0.,
        z: -sy,
    };
    let up = Vec3 {
        x: sy * sp,
        y: cp,
        z: cy * sp,
    };
    let aspect = bounded(aspect, 16. / 9., 0.1, 10.);
    const SAFE_FRAME: f32 = 0.8;
    let tan_vertical = (pose.fov.to_radians() * 0.5).tan() * SAFE_FRAME;
    let tan_horizontal = tan_vertical * aspect;
    let mut points = Vec::with_capacity(16);
    let mut required = pose.distance;
    // Conservative body envelopes include ears/legs and room around each runner.
    for p in [a, b] {
        for x in [-0.7, 0.7] {
            for y in [-0.2, 2.7] {
                for z in [-0.7, 0.7] {
                    let delta = Vec3 {
                        x: p.x - target.x + x,
                        y: p.y - target.y + y - pose.height,
                        z: p.z - target.z + z,
                    };
                    let camera = Vec3 {
                        x: dot(delta, right),
                        y: dot(delta, up),
                        z: dot(delta, forward),
                    };
                    required = required
                        .max(camera.x.abs() / tan_horizontal - camera.z)
                        .max(camera.y.abs() / tan_vertical - camera.z)
                        .max(1. - camera.z);
                    points.push(camera);
                }
            }
        }
    }
    pose.distance = required.clamp(pose.distance, 200.);
    if required > 200. {
        // Very narrow lenses or portrait viewports may reach the distance cap.
        // Widen the lens only then; finite caps still apply to malformed input.
        let mut tangent = (pose.fov.to_radians() * 0.5).tan();
        for p in points {
            let depth = (pose.distance + p.z).max(0.1);
            tangent = tangent
                .max(p.y.abs() / (depth * SAFE_FRAME))
                .max(p.x.abs() / (depth * aspect * SAFE_FRAME));
        }
        pose.fov = (2. * tangent.atan()).to_degrees().clamp(pose.fov, 80.);
    }
    pose.sanitize();
    pose
}

/// Widen the final projection after the ground guard raises the eye and turns
/// it toward aim. This uses that actual look-at view, not the pre-clamp orbit.
/// An impossible/degenerate view returns the bounded 100-degree lens; callers
/// retain terrain-safe eye placement. Preferred follow-pose limits are unchanged.
pub fn grounded_fov(eye: Vec3, aim: Vec3, a: Vec3, b: Vec3, fov: f32, aspect: f32) -> f32 {
    let preferred = bounded(fov, 15., 5., 100.);
    if ![eye, aim, a, b].into_iter().all(finite_point) {
        return 100.;
    }
    let delta = Vec3 {
        x: aim.x - eye.x,
        y: aim.y - eye.y,
        z: aim.z - eye.z,
    };
    if !finite_point(delta) {
        return 100.;
    }
    let horizontal = delta.x.hypot(delta.z);
    let length = horizontal.hypot(delta.y);
    if !length.is_finite() || length < 0.0001 {
        return 100.;
    }
    let yaw = delta.x.atan2(delta.z);
    let pitch = (-delta.y).atan2(horizontal);
    let (sy, cy) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    let forward = Vec3 {
        x: sy * cp,
        y: -sp,
        z: cy * cp,
    };
    let right = Vec3 {
        x: cy,
        y: 0.,
        z: -sy,
    };
    let up = Vec3 {
        x: sy * sp,
        y: cp,
        z: cy * sp,
    };
    let aspect = bounded(aspect, 16. / 9., 0.1, 10.);
    let mut tangent = (preferred.to_radians() * 0.5).tan();
    for p in [a, b] {
        for x in [-0.7, 0.7] {
            for y in [-0.2, 2.7] {
                for z in [-0.7, 0.7] {
                    let v = Vec3 {
                        x: p.x - eye.x + x,
                        y: p.y - eye.y + y,
                        z: p.z - eye.z + z,
                    };
                    if !finite_point(v) {
                        return 100.;
                    }
                    let depth = dot(v, forward);
                    if !depth.is_finite() || depth <= 0.05 {
                        return 100.;
                    }
                    tangent = tangent
                        .max(dot(v, up).abs() / (depth * 0.8))
                        .max(dot(v, right).abs() / (depth * aspect * 0.8));
                }
            }
        }
    }
    bounded((2. * tangent.atan()).to_degrees(), 100., preferred, 100.)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn runner(gate: i32, rank: i32, distance: f32, speed: f32) -> Runner {
        Runner {
            gate,
            rank: Some(rank),
            name: format!("Runner {gate}"),
            trainer_name: None,
            distance: Some(distance),
            speed: Some(speed),
        }
    }
    fn settings() -> BattleSettings {
        BattleSettings {
            enabled: true,
            ..BattleSettings::default()
        }
    }
    #[test]
    fn closest_adjacent_pair_is_stable_through_overtake_and_small_gap_changes() {
        let settings = settings();
        let mut tracker = BattleTracker::default();
        let mut rows = vec![
            runner(1, 1, 120., 18.),
            runner(2, 2, 110., 20.),
            runner(3, 3, 109., 21.),
        ];
        assert_eq!(tracker.update(&rows, 2, &settings, 0.).unwrap().partner, 3);
        rows[2].rank = Some(2);
        rows[2].distance = Some(111.);
        rows[1].rank = Some(3);
        let info = tracker.update(&rows, 2, &settings, 0.1).unwrap();
        assert_eq!(info.partner, 3);
        assert_eq!(info.position, Some(2));
        assert_eq!(info.gap_m, Some(1.));
        assert_eq!(info.closing_mps, Some(-1.));
        rows[1].rank = Some(2);
        rows[2].rank = Some(3);
        rows[1].distance = Some(115.);
        rows[2].distance = Some(109.);
        for _ in 0..100 {
            assert_eq!(tracker.update(&rows, 2, &settings, 0.1).unwrap().partner, 3);
        }
        rows[0].distance = Some(116.);
        assert_eq!(tracker.update(&rows, 2, &settings, 0.1).unwrap().partner, 1);
    }
    #[test]
    fn unrelated_finisher_does_not_interrupt_the_current_minimum_hold() {
        let settings = settings();
        let mut tracker = BattleTracker::default();
        let mut rows = vec![
            runner(1, 1, 130., 20.),
            runner(2, 2, 120., 20.),
            runner(3, 3, 110., 20.),
            runner(4, 4, 109., 21.),
        ];
        assert_eq!(tracker.update(&rows, 3, &settings, 0.).unwrap().partner, 4);
        rows.remove(0); // Race leader finishes; neither featured runner has finished.
        rows[0].distance = Some(111.);
        rows[2].distance = Some(107.);
        assert_eq!(tracker.update(&rows, 3, &settings, 0.1).unwrap().partner, 4);
        for _ in 0..50 {
            tracker.update(&rows, 3, &settings, 0.1);
        }
        assert_eq!(tracker.update(&rows, 3, &settings, 0.1).unwrap().partner, 2);
        rows.retain(|r| r.gate != 2); // The chosen partner disappears: reacquire immediately.
        assert_eq!(tracker.update(&rows, 3, &settings, 0.1).unwrap().partner, 4);
    }
    #[test]
    fn invalid_missing_and_distant_metadata_never_produces_a_pair() {
        let mut tracker = BattleTracker::default();
        let mut rows = vec![runner(1, 1, 100., 20.), runner(2, 2, 98., 21.)];
        assert!(tracker
            .update(&rows, 1, &BattleSettings::default(), 0.1)
            .is_none());
        let settings = settings();
        assert!(tracker.update(&rows, 1, &settings, 0.1).is_some());
        rows[1].distance = Some(f32::NAN);
        assert!(tracker.update(&rows, 1, &settings, 0.1).is_none());
        rows[1].distance = Some(1.);
        assert!(tracker.update(&rows, 1, &settings, 0.1).is_none());
        rows[1].distance = Some(98.);
        rows[1].rank = None;
        assert!(tracker.update(&rows, 1, &settings, 0.1).is_none());
        assert!(tracker.update(&rows, 99, &settings, 0.1).is_none());
    }
    #[test]
    fn pinned_pair_and_session_reset_override_automatic_hold() {
        let mut tracker = BattleTracker::default();
        let mut settings = settings();
        let mut rows = vec![
            runner(1, 1, 120., 19.),
            runner(2, 2, 110., 20.),
            runner(3, 3, 109., 21.),
        ];
        assert_eq!(tracker.update(&rows, 2, &settings, 0.).unwrap().partner, 3);
        settings.partner_gate = Some(1);
        assert_eq!(tracker.update(&rows, 2, &settings, 0.).unwrap().partner, 1);
        settings.partner_gate = Some(2);
        assert!(tracker.update(&rows, 2, &settings, 0.).is_none());
        settings.partner_gate = None;
        assert_eq!(tracker.update(&rows, 2, &settings, 0.).unwrap().partner, 3);
        rows[0].distance = Some(22.);
        rows[1].distance = Some(21.);
        rows[2].distance = Some(12.);
        assert_eq!(tracker.update(&rows, 2, &settings, 0.).unwrap().partner, 1);
        tracker.reset();
        assert_eq!(tracker.partner, None);
    }
    #[test]
    fn missing_speed_or_conflicting_progress_does_not_invent_closing_data() {
        let mut tracker = BattleTracker::default();
        let mut rows = vec![runner(1, 1, 100., 20.), runner(2, 2, 98., 21.)];
        rows[1].speed = None;
        let info = tracker.update(&rows, 1, &settings(), 0.).unwrap();
        assert_eq!(info.gap_m, Some(2.));
        assert!(info.closing_mps.is_none());
        rows[1].distance = Some(101.);
        let info = tracker.update(&rows, 1, &settings(), 0.).unwrap();
        assert!(info.gap_m.is_none() && info.closing_mps.is_none());
    }
    fn assert_in_view(pose: Pose, center: Vec3, heading: f32, aspect: f32, a: Vec3, b: Vec3) {
        let (eye, [x, y, z, w]) = crate::model::orbit(pose, center, heading);
        let right = Vec3 {
            x: 1. - 2. * (y * y + z * z),
            y: 2. * (x * y + w * z),
            z: 2. * (x * z - w * y),
        };
        let up = Vec3 {
            x: 2. * (x * y - w * z),
            y: 1. - 2. * (x * x + z * z),
            z: 2. * (y * z + w * x),
        };
        let forward = Vec3 {
            x: 2. * (x * z + w * y),
            y: 2. * (y * z - w * x),
            z: 1. - 2. * (x * x + y * y),
        };
        let tan = (pose.fov.to_radians() * 0.5).tan();
        for p in [a, b] {
            for dx in [-0.7, 0.7] {
                for dy in [-0.2, 2.7] {
                    for dz in [-0.7, 0.7] {
                        let v = Vec3 {
                            x: p.x + dx - eye.x,
                            y: p.y + dy - eye.y,
                            z: p.z + dz - eye.z,
                        };
                        let depth = dot(v, forward);
                        assert!(depth > 0.);
                        assert!(dot(v, right).abs() / depth < tan * aspect * 0.801);
                        assert!(dot(v, up).abs() / depth < tan * 0.801);
                    }
                }
            }
        }
    }
    #[test]
    fn framing_covers_body_envelopes_for_angles_aspects_and_height_offsets() {
        let a = Vec3 {
            x: -12.,
            y: 1.,
            z: 3.,
        };
        let b = Vec3 {
            x: 9.,
            y: 2.,
            z: 18.,
        };
        for aspect in [9. / 16., 16. / 9., 32. / 9.] {
            for yaw in [0., 1.2, -2.4] {
                for pitch in [-0.3, 0.18, 1.] {
                    for height in [-1., 1.2, 20.] {
                        let preferred = Pose {
                            yaw,
                            pitch,
                            height,
                            distance: 5.,
                            fov: 15.,
                        };
                        let (pose, center) = frame(preferred, a, b, 0.8, aspect);
                        assert!(pose.distance >= preferred.distance && pose.distance <= 200.);
                        assert!((pose.yaw - preferred.yaw).abs() < 0.0001);
                        assert_in_view(pose, center, 0.8, aspect, a, b);
                    }
                }
            }
        }
    }
    #[test]
    fn framing_uses_the_actual_narrow_viewport_width() {
        let a = Vec3 {
            x: -2.,
            y: 0.,
            z: 0.,
        };
        let b = Vec3 {
            x: 2.,
            y: 0.,
            z: 0.,
        };
        let (pose, center) = frame(Pose::default(), a, b, 0., 0.12);
        assert_in_view(pose, center, 0., 0.12, a, b);
    }
    #[test]
    fn transition_guard_fits_pair_at_a_tweened_anchor_with_a_smooth_lens() {
        let a = Vec3 {
            x: -5.,
            y: 1.,
            z: 0.,
        };
        let b = Vec3 {
            x: 8.,
            y: 1.,
            z: 20.,
        };
        let anchor = Vec3 {
            x: -10.,
            y: 1.,
            z: -8.,
        };
        let pose = fit_at(
            Pose {
                distance: 8.,
                yaw: 0.4,
                fov: 12.,
                ..Pose::default()
            },
            a,
            b,
            anchor,
            0.7,
            16. / 9.,
        );
        assert_in_view(pose, anchor, 0.7, 16. / 9., a, b);
    }
    #[test]
    fn framing_is_finite_for_identical_positions_and_invalid_inputs() {
        for p in [
            Vec3::default(),
            Vec3 {
                x: f32::NAN,
                y: 0.,
                z: 0.,
            },
        ] {
            let (pose, center) = frame(Pose::default(), p, p, f32::NAN, f32::INFINITY);
            assert!(pose.distance.is_finite() && pose.fov.is_finite() && finite_point(center));
        }
        let a = Vec3 {
            x: -15.,
            y: 0.,
            z: 0.,
        };
        let b = Vec3 {
            x: 15.,
            y: 0.,
            z: 0.,
        };
        let preferred = Pose {
            distance: 5.,
            fov: 5.,
            ..Pose::default()
        };
        let (pose, center) = frame(preferred, a, b, 0., 9. / 16.);
        assert_eq!(pose.distance, 200.);
        assert!(pose.fov > preferred.fov);
        assert_in_view(pose, center, 0., 9. / 16., a, b);
    }
    fn assert_grounded_projection(eye: Vec3, aim: Vec3, a: Vec3, b: Vec3, fov: f32, aspect: f32) {
        let [x, y, z, w] = crate::camera_motion::look_at(eye, aim);
        let right = Vec3 {
            x: 1. - 2. * (y * y + z * z),
            y: 2. * (x * y + w * z),
            z: 2. * (x * z - w * y),
        };
        let up = Vec3 {
            x: 2. * (x * y - w * z),
            y: 1. - 2. * (x * x + z * z),
            z: 2. * (y * z + w * x),
        };
        let forward = Vec3 {
            x: 2. * (x * z + w * y),
            y: 2. * (y * z - w * x),
            z: 1. - 2. * (x * x + y * y),
        };
        let tan = (fov.to_radians() * 0.5).tan();
        for p in [a, b] {
            for dx in [-0.7, 0.7] {
                for dy in [-0.2, 2.7] {
                    for dz in [-0.7, 0.7] {
                        let v = Vec3 {
                            x: p.x + dx - eye.x,
                            y: p.y + dy - eye.y,
                            z: p.z + dz - eye.z,
                        };
                        let depth = dot(v, forward);
                        assert!(depth > 0.);
                        assert!(dot(v, right).abs() / depth < tan * aspect * 0.801);
                        assert!(dot(v, up).abs() / depth < tan * 0.801);
                    }
                }
            }
        }
    }
    #[test]
    fn ground_correction_refits_a_low_negative_pitch_battle_view() {
        let a = Vec3 {
            x: -15.,
            y: 0.,
            z: 0.,
        };
        let b = Vec3 {
            x: 15.,
            y: 0.,
            z: 0.,
        };
        let (pose, anchor) = frame(
            Pose {
                pitch: -1.2,
                ..Pose::default()
            },
            a,
            b,
            0.,
            16. / 9.,
        );
        let (mut eye, _) = crate::model::orbit(pose, anchor, 0.);
        let aim = Vec3 {
            x: anchor.x,
            y: anchor.y + pose.height,
            z: anchor.z,
        };
        assert!(crate::camera_motion::clamp_height(&mut eye, 0., 0.4));
        let fov = grounded_fov(eye, aim, a, b, pose.fov, 16. / 9.);
        assert!(fov > pose.fov && fov <= 100.);
        assert_grounded_projection(eye, aim, a, b, fov, 16. / 9.);
    }
    #[test]
    fn grounded_lens_handles_narrow_aspect_and_degenerate_views() {
        let eye = Vec3 {
            x: 0.,
            y: 0.4,
            z: -40.,
        };
        let aim = Vec3 {
            x: 0.,
            y: 1.2,
            z: 0.,
        };
        let a = Vec3 {
            x: -2.,
            y: 0.,
            z: 0.,
        };
        let b = Vec3 {
            x: 2.,
            y: 0.,
            z: 0.,
        };
        let fov = grounded_fov(eye, aim, a, b, 15., 0.12);
        assert!(fov.is_finite() && fov > 15. && fov <= 100.);
        assert_grounded_projection(eye, aim, a, b, fov, 0.12);
        assert_eq!(grounded_fov(eye, eye, a, b, 15., 0.12), 100.);
        assert_eq!(
            grounded_fov(eye, aim, Vec3 { x: f32::NAN, ..a }, b, 15., 0.12),
            100.
        );
        let impossible = Vec3 {
            x: 1000.,
            y: 0.,
            z: 0.,
        };
        assert_eq!(grounded_fov(eye, aim, impossible, b, 15., 0.12), 100.);
        assert!(grounded_fov(eye, aim, a, b, f32::NAN, f32::NAN).is_finite());
    }
    #[test]
    fn old_settings_and_invalid_preferences_are_safe_and_round_trip() {
        let mut settings: BattleSettings = serde_json::from_str("{}").unwrap();
        assert!(!settings.enabled && settings.partner_gate.is_none());
        settings.enabled = true;
        settings.partner_gate = Some(3);
        settings.hold_seconds = f32::NAN;
        settings.max_gap = 1000.;
        settings.sanitize();
        let loaded: BattleSettings =
            serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert!(loaded.enabled);
        assert_eq!(loaded.partner_gate, Some(3));
        assert_eq!(loaded.hold_seconds, 4.);
        assert_eq!(loaded.max_gap, 100.);
    }
}
