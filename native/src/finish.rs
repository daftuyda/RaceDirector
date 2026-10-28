//! Distance-triggered finish coverage. No precomputed finish order is consulted.
use crate::{
    battle::{BattleInfo, BattleSettings},
    model::Pose,
    timing::Runner,
};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Selection {
    pub leader: i32,
    pub rival: Option<i32>,
}
impl Selection {
    pub fn key(self) -> i32 {
        let b = self.rival.unwrap_or(self.leader);
        -(100_000 + self.leader.min(b) * 128 + self.leader.max(b))
    }
    pub fn info(self, rows: &[Runner]) -> Option<BattleInfo> {
        let a = rows.iter().find(|r| r.gate == self.leader)?;
        let b = rows.iter().find(|r| Some(r.gate) == self.rival)?;
        let (ahead, behind) = if a.rank <= b.rank { (a, b) } else { (b, a) };
        let delta = valid(ahead.distance)? - valid(behind.distance)?;
        let gap = (delta >= -0.05).then_some(delta.max(0.));
        Some(BattleInfo {
            anchor: self.leader,
            partner: b.gate,
            position: ahead.rank,
            gap_m: gap,
            closing_mps: gap.and_then(|_| Some(valid(behind.speed)? - valid(ahead.speed)?)),
        })
    }
}
fn valid(v: Option<f32>) -> Option<f32> {
    v.filter(|v| v.is_finite() && *v >= 0.)
}
#[derive(Default)]
pub struct FinishDirector {
    active: bool,
    selection: Option<Selection>,
    locked: bool,
}
impl FinishDirector {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn update(
        &mut self,
        rows: &[Runner],
        finished: &[i32],
        settings: &BattleSettings,
        length: f32,
    ) -> Option<Selection> {
        if !settings.enabled
            || !settings.finish_coverage
            || !length.is_finite()
            || length <= settings.finish_distance
        {
            self.reset();
            return None;
        }
        if self.locked {
            return self
                .selection
                .filter(|s| rows.iter().any(|r| r.gate == s.leader));
        }
        let leader = rows.iter().find(|r| r.rank == Some(1))?;
        if rows.iter().filter(|r| r.rank == Some(1)).count() != 1 {
            return self.selection;
        }
        let distance = valid(leader.distance)?;
        if !self.active {
            if finished.contains(&leader.gate) || distance < length - settings.finish_distance {
                return None;
            }
            self.active = true;
        }
        // Hold the current pair when either crosses. Never jump to P3 on the line.
        if self.selection.is_some_and(|s| {
            finished.contains(&s.leader) || s.rival.is_some_and(|g| finished.contains(&g))
        }) {
            self.locked = true;
            return self.selection;
        }
        let rival = rows.iter().find(|r| r.rank == Some(2)).filter(|r| {
            valid(r.distance).is_some_and(|d| (distance - d).abs() <= settings.max_gap)
        });
        let next = Selection {
            leader: leader.gate,
            rival: rival.map(|r| r.gate),
        };
        self.selection = Some(next);
        Some(next)
    }
}
pub fn front_shot(mut pose: Pose, right_side: bool) -> Pose {
    // Orbit yaw zero is already ahead of the runners in this game.
    pose.yaw = std::f32::consts::FRAC_PI_4 * if right_side { 1. } else { -1. };
    pose.pitch = 0.12;
    pose.height = 1.8;
    pose.distance = 35.;
    pose.fov = 30.;
    pose
}
#[cfg(test)]
mod tests {
    use super::*;
    fn rows() -> Vec<Runner> {
        (1..=3)
            .map(|g| Runner {
                gate: g,
                name: format!("Runner {g}"),
                trainer_name: None,
                rank: Some(g),
                distance: Some(1600. - (g - 1) as f32 * 2.),
                speed: Some(20.),
            })
            .collect()
    }
    fn settings() -> BattleSettings {
        BattleSettings {
            enabled: true,
            finish_coverage: true,
            ..Default::default()
        }
    }
    #[test]
    fn finish_quarter_view_is_ahead_and_on_the_selected_side() {
        // Live lane rotation faces -X while the runners travel along +X.
        let target = crate::model::Vec3::default();
        for right in [false, true] {
            let pose = front_shot(Pose::default(), right);
            let (eye, _) = crate::model::orbit(pose, target, -std::f32::consts::FRAC_PI_2);
            assert!(eye.x > 20.);
            assert!(if right { eye.z < -20. } else { eye.z > 20. });
        }
    }
    #[test]
    fn trigger_requires_known_length_and_distance_boundary() {
        let mut d = FinishDirector::default();
        let mut r = rows();
        assert!(d.update(&r, &[], &settings(), 0.).is_none());
        assert!(d.update(&r, &[], &settings(), 400.).is_none());
        r[0].distance = Some(1599.);
        assert!(d.update(&r, &[], &settings(), 2000.).is_none());
        r[0].distance = Some(1600.);
        assert_eq!(d.update(&r, &[], &settings(), 2000.).unwrap().leader, 1);
    }
    #[test]
    fn lead_swap_has_stable_camera_key_and_finish_holds_pair() {
        let mut d = FinishDirector::default();
        let mut r = rows();
        let a = d.update(&r, &[], &settings(), 2000.).unwrap();
        r[0].rank = Some(2);
        r[1].rank = Some(1);
        let b = d.update(&r, &[], &settings(), 2000.).unwrap();
        assert_eq!(a.key(), b.key());
        assert_eq!(b.leader, 2);
        assert_eq!(d.update(&r, &[2], &settings(), 2000.), Some(b));
        r[2].rank = Some(2);
        r[0].rank = Some(3);
        assert_eq!(d.update(&r, &[2], &settings(), 2000.), Some(b));
    }
    #[test]
    fn disable_reset_and_missing_data_do_not_invent_a_finish() {
        let mut d = FinishDirector::default();
        let mut r = rows();
        assert!(d.update(&r, &[1], &settings(), 2000.).is_none());
        r[0].distance = None;
        assert!(d.update(&r, &[], &settings(), 2000.).is_none());
        r[0].distance = Some(1600.);
        assert!(d.update(&r, &[], &settings(), 2000.).is_some());
        assert!(d
            .update(&r, &[], &BattleSettings::default(), 2000.)
            .is_none());
        d.reset();
        r[0].distance = Some(100.);
        assert!(d.update(&r, &[], &settings(), 2000.).is_none());
    }
    #[test]
    fn distant_rival_uses_leader_and_preferences_persist() {
        let mut d = FinishDirector::default();
        let mut r = rows();
        r[1].distance = Some(1000.);
        assert_eq!(d.update(&r, &[], &settings(), 2000.).unwrap().rival, None);
        let saved = serde_json::to_string(&settings()).unwrap();
        let read: BattleSettings = serde_json::from_str(&saved).unwrap();
        assert!(read.finish_coverage);
        assert!(
            !serde_json::from_str::<BattleSettings>("{}")
                .unwrap()
                .finish_coverage
        );
    }
}
