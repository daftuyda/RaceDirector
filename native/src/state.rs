use crate::model::Settings;
use std::sync::{Mutex, OnceLock};
pub struct State {
    pub settings: Settings,
    pub tower: bool,
    pub timing_rows: Vec<crate::timing::Runner>,
    pub timing_status: String,
    pub timing_generation: u64,
    timing_source: Option<u32>,
    pub finished_gates: Vec<i32>,
    pub coverage_gate: Option<i32>,
    pub focus_markers: Vec<crate::markers::Marker>,
    pub save_finish_camera: bool,
    pub finish_camera_armed: bool,
    pub director_status: String,
    pub detected_race_distance: Option<f32>,
    pub course_world_scale: Option<f32>,
    pub restart_tour: bool,
    pub battle_info: Option<crate::battle::BattleInfo>,
    pub camera: bool,
    pub freecam: bool,
    pub free_pose: Option<crate::model::FreePose>,
    pub capture: bool,
    pub capture_status: String,
    pub ground_status: String,
    pub hide_ui: bool,
    pub hide_bottom: bool,
    pub hide_skill_popups: bool,
    pub hide_markers: bool,
    pub skip_cutins: bool,
    pub hide_speed_lines: bool,
    pub hide_overhead: bool,
    pub hide_trees: bool,
    pub clean_effects: bool,
    pub visual_status: String,
    pub fps_status: String,
    pub graphics: bool,
    pub graphics_applied: crate::graphics::Quality,
    pub graphics_status: String,
    pub gate: i32,
    pub gates: Vec<i32>,
    pub paths: Vec<String>,
    pub scan: bool,
    pub status: String,
    pub race: bool,
}
pub fn state() -> &'static Mutex<State> {
    static S: OnceLock<Mutex<State>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(State::from_settings(load())))
}
impl State {
    pub(crate) fn from_settings(settings: Settings) -> Self {
        let enabled = settings.enabled.clone();
        Self {
            settings,
            tower: enabled.tower,
            timing_rows: vec![],
            timing_status: "Waiting for race telemetry".into(),
            timing_generation: 0,
            timing_source: None,
            finished_gates: vec![],
            coverage_gate: None,
            focus_markers: vec![],
            save_finish_camera: false,
            finish_camera_armed: false,
            director_status: "Director off".into(),
            detected_race_distance: None,
            course_world_scale: None,
            restart_tour: false,
            battle_info: None,
            camera: enabled.camera,
            freecam: enabled.freecam,
            free_pose: None,
            capture: false,
            ground_status: "Ground guard: nearest runner height until course surfaces load".into(),
            capture_status: "Waiting for cursor controls".into(),
            hide_ui: enabled.hide_ui,
            hide_bottom: enabled.hide_bottom,
            hide_skill_popups: enabled.hide_skill_popups,
            hide_markers: enabled.hide_markers,
            skip_cutins: enabled.skip_cutins,
            hide_speed_lines: enabled.hide_speed_lines,
            hide_overhead: enabled.hide_overhead,
            hide_trees: enabled.hide_trees,
            clean_effects: enabled.clean_effects,
            visual_status: "Waiting for visual controls".into(),
            fps_status: "Waiting for frame-rate controls".into(),
            graphics: enabled.graphics,
            graphics_applied: enabled.graphics_applied,
            graphics_status: "Waiting for model clarity controls".into(),
            gate: 1,
            gates: vec![],
            paths: vec![],
            scan: false,
            status: "Waiting for game runtime".into(),
            race: false,
        }
    }
    /// One action owns all prerequisites. Persisted preferences remain separate
    /// so a later explicit F6/Freecam choice can still suspend automation.
    pub fn start_director(&mut self) {
        self.settings.director.enabled = true;
        self.settings.battle.finish_coverage = true;
        self.camera = true;
        self.freecam = false;
        self.free_pose = None;
        self.director_status = "Director starting / waiting for race telemetry".into();
    }
    pub fn race_distance(&self) -> f32 {
        crate::director::race_length(
            self.settings.broadcast.race_distance_m,
            self.detected_race_distance,
        )
    }
    pub fn distance_status(&self) -> String {
        let length = self.race_distance();
        if self.settings.broadcast.race_distance_m > 0. && length > 0. {
            format!("Race length: {length:.0} m / manual override")
        } else if length > 0. {
            format!("Race length: {length:.0} m / supplied by game")
        } else {
            "Race length unavailable: tour starts at 200 m; enter length for automatic finish coverage.".into()
        }
    }
    pub fn needs_telemetry(&self) -> bool {
        self.tower
            || self.camera
            || self.settings.broadcast.spotlight
            || self.settings.broadcast.battle_card
            || self.settings.broadcast.progress
            || self.settings.broadcast.results
    }
    pub fn clear_telemetry(&mut self) {
        if self.timing_source.take().is_some() || !self.timing_rows.is_empty() {
            self.timing_generation = self.timing_generation.wrapping_add(1);
            self.finish_camera_armed = false;
        }
        self.detected_race_distance = None;
        self.course_world_scale = None;
        self.timing_rows.clear();
        self.finished_gates.clear();
        self.coverage_gate = None;
        self.focus_markers.clear();
        self.battle_info = None;
    }
    pub fn update_telemetry(
        &mut self,
        rows: Vec<crate::timing::Runner>,
        finished: Vec<i32>,
        source: u32,
    ) {
        let identity = |rs: &[crate::timing::Runner]| {
            let mut ids = rs
                .iter()
                .map(|r| (r.gate, r.name.clone()))
                .collect::<Vec<_>>();
            ids.sort();
            ids
        };
        let progress = |rs: &[crate::timing::Runner]| {
            rs.iter()
                .filter_map(|r| r.distance.filter(|v| v.is_finite() && *v >= 0.))
                .max_by(f32::total_cmp)
        };
        let rewind = progress(&self.timing_rows)
            .zip(progress(&rows))
            .is_some_and(|(old, new)| new + 5. < old);
        if self.timing_source != Some(source)
            || identity(&self.timing_rows) != identity(&rows)
            || rewind
        {
            self.timing_generation = self.timing_generation.wrapping_add(1);
            self.finish_camera_armed = false;
            self.detected_race_distance = None;
            self.course_world_scale = None;
            self.coverage_gate = None;
            self.focus_markers.clear();
            self.battle_info = None;
        }
        self.timing_source = Some(source);
        self.finished_gates = finished
            .into_iter()
            .filter(|g| rows.iter().any(|r| r.gate == *g && r.rank.is_some()))
            .collect();
        self.timing_rows = rows;
    }
    pub fn restore_visuals(&mut self) {
        self.capture = false;
        self.camera = false;
        self.hide_ui = false;
        self.hide_bottom = false;
        self.hide_skill_popups = false;
        self.hide_markers = false;
        self.skip_cutins = false;
        self.hide_speed_lines = false;
        self.hide_overhead = false;
        self.hide_trees = false;
        self.clean_effects = false;
        self.graphics = false;
        self.tower = false;
        self.settings.battle.enabled = false;
        self.settings.director.enabled = false;
        self.save_finish_camera = false;
        self.restart_tour = false;
        self.director_status = "Director off".into();
        self.settings.broadcast.spotlight = false;
        self.settings.broadcast.focus_markers = false;
        self.settings.broadcast.battle_card = false;
        self.settings.broadcast.progress = false;
        self.settings.broadcast.results = false;
        self.coverage_gate = None;
        self.focus_markers.clear();
        self.battle_info = None;
    }
    pub fn snapshot(&self) -> Settings {
        let mut settings = self.settings.clone();
        settings.enabled = crate::model::Enabled {
            tower: self.tower,
            camera: self.camera,
            freecam: self.freecam,
            hide_ui: self.hide_ui,
            hide_bottom: self.hide_bottom,
            hide_skill_popups: self.hide_skill_popups,
            hide_markers: self.hide_markers,
            skip_cutins: self.skip_cutins,
            hide_speed_lines: self.hide_speed_lines,
            hide_overhead: self.hide_overhead,
            hide_trees: self.hide_trees,
            clean_effects: self.clean_effects,
            graphics: self.graphics,
            graphics_applied: self.graphics_applied,
        };
        // Inputs may change after the frame's normal sanitization pass.
        // Persist bounded metadata even when autosave catches that next frame.
        settings.sanitize();
        settings
    }
}
fn legacy_path() -> std::path::PathBuf {
    std::env::current_exe()
        .unwrap_or_default()
        .with_file_name("racedirector-settings.json")
}
fn path() -> std::path::PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("RaceDirector")
        .join("settings.json")
}
fn load() -> Settings {
    let mut s: Settings = std::fs::read(path())
        .or_else(|_| std::fs::read(legacy_path()))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    s.sanitize();
    s
}
pub fn save(s: &Settings) -> Result<(), String> {
    let data = serde_json::to_vec_pretty(s).map_err(|e| e.to_string())?;
    write_settings(&path(), &data)
}
fn write_settings(path: &std::path::Path, data: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("Missing settings folder")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, data).map_err(|e| e.to_string())?;
    std::fs::rename(&temp, path).map_err(|e| e.to_string())
}
pub fn status(s: impl Into<String>) {
    let s = s.into();
    log(&s);
    if let Ok(mut state) = state().lock() {
        state.status = s;
    }
}

pub fn log(message: &str) {
    use std::io::Write;
    let base = std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("RaceDirector");
    if std::fs::create_dir_all(&base).is_err() {
        return;
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(base.join("startup.log"))
    {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let _ = writeln!(file, "[{now}] {} {message}", env!("CARGO_PKG_VERSION"));
    }
}

/// Throttle disk writes while sliders or camera controls are moving.
pub struct AutoSave {
    checked: std::time::Instant,
    saved: Vec<u8>,
}
impl AutoSave {
    pub fn new() -> Self {
        Self {
            checked: std::time::Instant::now(),
            saved: Vec::new(),
        }
    }
    pub fn tick(&mut self, state: &State) -> Option<Result<(), String>> {
        if self.checked.elapsed() < std::time::Duration::from_secs(1) {
            return None;
        }
        self.checked = std::time::Instant::now();
        let settings = state.snapshot();
        let data = match serde_json::to_vec_pretty(&settings) {
            Ok(x) => x,
            Err(e) => return Some(Err(e.to_string())),
        };
        if data == self.saved {
            return None;
        }
        let result = write_settings(&path(), &data);
        if result.is_ok() {
            self.saved = data;
        }
        Some(result)
    }
}
#[cfg(test)]
mod persistence_tests {
    use super::*;
    #[test]
    fn toggles_and_applied_quality_survive_restart_without_capture_or_telemetry() {
        let mut state = State::from_settings(Settings::default());
        state.tower = true;
        state.camera = true;
        state.freecam = true;
        state.hide_ui = true;
        state.hide_bottom = true;
        state.hide_skill_popups = true;
        state.hide_markers = true;
        state.skip_cutins = true;
        state.hide_speed_lines = true;
        state.hide_overhead = true;
        state.hide_trees = true;
        state.clean_effects = true;
        state.graphics = true;
        state.graphics_applied = crate::graphics::Quality {
            scale: 150,
            msaa: 4,
        };
        state.settings.graphics = crate::graphics::Quality {
            scale: 200,
            msaa: 2,
        };
        state.capture = true;
        state.race = true;
        state.gate = 9;
        let saved = state.snapshot();
        let loaded: Settings =
            serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
        let restored = State::from_settings(loaded);
        assert_eq!(restored.snapshot().enabled, saved.enabled);
        assert_eq!(restored.graphics_applied.scale, 150);
        assert_eq!(restored.settings.graphics.scale, 200);
        assert!(!restored.capture);
        assert!(!restored.race);
        assert_eq!(restored.gate, 1);
    }
    #[test]
    fn legacy_settings_keep_values_and_default_new_toggles_off() {
        let settings: Settings =
            serde_json::from_str(r#"{"fps_60":false,"response":14.0}"#).unwrap();
        let restored = State::from_settings(settings);
        assert!(!restored.camera && !restored.graphics && !restored.tower);
        assert!(!restored.settings.fps_60);
        assert_eq!(restored.settings.response, 14.);
    }
    #[test]
    fn settings_file_can_be_replaced_and_reloaded() {
        let dir =
            std::env::temp_dir().join(format!("racedirector-save-test-{}", std::process::id()));
        let path = dir.join("settings.json");
        write_settings(&path, b"{}").unwrap();
        write_settings(&path, br#"{"fps_60":false}"#).unwrap();
        let s: Settings = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert!(!s.fps_60);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}

#[cfg(test)]
mod broadcast_state_tests {
    use super::*;
    use crate::timing::Runner;
    fn row(gate: i32, rank: i32, distance: f32) -> Runner {
        Runner {
            gate,
            name: format!("Runner {gate}"),
            trainer_name: None,
            rank: Some(rank),
            distance: Some(distance),
            speed: Some(20.),
        }
    }
    #[test]
    fn automatic_names_are_not_saved_or_reused_next_race() {
        let mut s = State::from_settings(Settings::default());
        let mut r = row(1, 1, 100.);
        r.trainer_name = Some("RaceOnlyTrainer".into());
        s.update_telemetry(vec![r], vec![], 1);
        assert!(!serde_json::to_string(&s.snapshot())
            .unwrap()
            .contains("RaceOnlyTrainer"));
        s.clear_telemetry();
        assert!(s.timing_rows.is_empty());
        s.update_telemetry(vec![row(1, 1, 0.)], vec![], 2);
        assert!(s.timing_rows[0].trainer_name.is_none());
    }
    #[test]
    fn focus_marker_preference_persists_but_projected_points_never_do() {
        let mut s = State::from_settings(Settings::default());
        assert!(s.settings.broadcast.focus_markers);
        s.focus_markers.push(crate::markers::Marker {
            gate: 1,
            point: [0.5, 0.5],
        });
        let restored = State::from_settings(s.snapshot());
        assert!(restored.settings.broadcast.focus_markers && restored.focus_markers.is_empty());
        s.restore_visuals();
        assert!(!s.settings.broadcast.focus_markers && s.focus_markers.is_empty());
        assert!(
            !State::from_settings(s.snapshot())
                .settings
                .broadcast
                .focus_markers
        );
    }
    #[test]
    fn detected_course_data_is_ephemeral_and_manual_override_wins() {
        let mut s = State::from_settings(Settings::default());
        s.update_telemetry(vec![row(1, 1, 100.)], vec![], 7);
        s.detected_race_distance = Some(2000.);
        s.course_world_scale = Some(2.);
        assert_eq!(s.race_distance(), 2000.);
        assert!(s.distance_status().contains("supplied by game"));
        assert_eq!(s.snapshot().broadcast.race_distance_m, 0.);
        let restored = State::from_settings(s.snapshot());
        assert_eq!(restored.race_distance(), 0.);
        s.settings.broadcast.race_distance_m = 2400.;
        assert_eq!(s.race_distance(), 2400.);
        assert!(s.distance_status().contains("manual override"));
        s.update_telemetry(vec![row(1, 1, 0.)], vec![], 8);
        assert!(s.detected_race_distance.is_none() && s.course_world_scale.is_none());
        s.settings.broadcast.race_distance_m = 0.;
        assert_eq!(s.race_distance(), 0.);
    }
    #[test]
    fn finish_spot_persists_but_arming_and_coverage_are_race_local() {
        let mut s = State::from_settings(Settings::default());
        s.settings.director.enabled = true;
        s.settings.director.finish_camera = Some(crate::director::FixedCamera {
            eye: crate::model::Vec3 {
                x: 50.,
                y: 4.,
                z: 20.,
            },
            fov: 30.,
        });
        s.update_telemetry(vec![row(1, 1, 100.)], vec![], 7);
        s.finish_camera_armed = true;
        s.save_finish_camera = true;
        s.coverage_gate = Some(1);
        let restored = State::from_settings(
            serde_json::from_str(&serde_json::to_string(&s.snapshot()).unwrap()).unwrap(),
        );
        assert!(restored.settings.director.enabled);
        assert_eq!(restored.settings.director.finish_camera.unwrap().eye.x, 50.);
        assert!(!restored.finish_camera_armed && !restored.save_finish_camera);
        assert!(restored.coverage_gate.is_none());
        s.update_telemetry(vec![row(1, 1, 120.)], vec![], 7);
        assert!(s.finish_camera_armed);
        s.update_telemetry(vec![row(1, 1, 0.)], vec![], 7);
        assert!(!s.finish_camera_armed && s.coverage_gate.is_none());
        s.restore_visuals();
        assert!(!s.settings.director.enabled && !s.save_finish_camera);
        assert!(s.settings.director.finish_camera.is_some());
    }
    #[test]
    fn overlay_consumers_collect_data_independently_of_tower() {
        let mut s = State::from_settings(Settings::default());
        assert!(!s.needs_telemetry());
        s.settings.broadcast.spotlight = true;
        assert!(s.needs_telemetry());
        s.settings.broadcast.spotlight = false;
        s.settings.broadcast.progress = true;
        assert!(s.needs_telemetry());
        s.settings.broadcast.progress = false;
        s.settings.broadcast.results = true;
        assert!(s.needs_telemetry());
        s.settings.broadcast.results = false;
        s.camera = true;
        assert!(s.needs_telemetry());
    }
    #[test]
    fn overtakes_keep_session_but_rewind_lineup_and_source_reset_it() {
        let mut s = State::from_settings(Settings::default());
        s.update_telemetry(vec![row(1, 1, 100.), row(2, 2, 99.)], vec![], 7);
        let first = s.timing_generation;
        s.update_telemetry(vec![row(2, 1, 110.), row(1, 2, 109.)], vec![], 7);
        assert_eq!(s.timing_generation, first);
        s.update_telemetry(vec![row(2, 1, 120.), row(1, 2, 119.)], vec![2], 7);
        assert_eq!(s.finished_gates, vec![2]);
        s.update_telemetry(vec![row(1, 1, 10.), row(2, 2, 9.)], vec![], 7);
        assert_eq!(s.timing_generation, first + 1);
        assert!(s.finished_gates.is_empty());
        s.update_telemetry(vec![row(1, 1, 10.), row(3, 2, 9.)], vec![], 7);
        assert_eq!(s.timing_generation, first + 2);
        s.update_telemetry(vec![row(1, 1, 10.), row(3, 2, 9.)], vec![], 8);
        assert_eq!(s.timing_generation, first + 3);
        s.clear_telemetry();
        let cleared = s.timing_generation;
        s.clear_telemetry();
        assert_eq!(s.timing_generation, cleared);
        assert!(s.timing_rows.is_empty() && s.finished_gates.is_empty() && s.battle_info.is_none());
    }
    #[test]
    fn broadcast_preferences_persist_but_race_data_does_not() {
        let mut s = State::from_settings(Settings::default());
        s.settings.battle.enabled = true;
        s.settings.battle.partner_gate = Some(4);
        s.settings.broadcast.spotlight = true;
        s.settings.broadcast.results = true;
        s.settings.broadcast.event_name = "Club final".into();
        s.settings.broadcast.race_distance_m = 2400.;
        s.update_telemetry(vec![row(1, 1, 2400.)], vec![1], 7);
        let restored = State::from_settings(
            serde_json::from_str(&serde_json::to_string(&s.snapshot()).unwrap()).unwrap(),
        );
        assert!(
            restored.settings.battle.enabled
                && restored.settings.broadcast.spotlight
                && restored.settings.broadcast.results
        );
        assert_eq!(restored.settings.battle.partner_gate, Some(4));
        assert_eq!(restored.settings.broadcast.race_distance_m, 2400.);
        assert!(restored.timing_rows.is_empty() && restored.finished_gates.is_empty());
    }
    #[test]
    fn restore_clears_every_broadcast_toggle_and_keeps_event_details() {
        let mut s = State::from_settings(Settings::default());
        s.camera = true;
        s.tower = true;
        s.settings.battle.enabled = true;
        s.settings.broadcast.spotlight = true;
        s.settings.broadcast.battle_card = true;
        s.settings.broadcast.progress = true;
        s.settings.broadcast.results = true;
        s.settings.broadcast.event_name = "Club final".into();
        s.settings.broadcast.race_distance_m = 2400.;
        s.restore_visuals();
        assert!(!s.camera && !s.tower && !s.settings.battle.enabled);
        assert!(
            !s.settings.broadcast.spotlight
                && !s.settings.broadcast.battle_card
                && !s.settings.broadcast.progress
                && !s.settings.broadcast.results
        );
        assert!(!s.needs_telemetry());
        assert_eq!(s.settings.broadcast.event_name, "Club final");
        assert_eq!(s.settings.broadcast.race_distance_m, 2400.);
    }
}
