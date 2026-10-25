use crate::model::Settings;
use std::sync::{Mutex, OnceLock};
pub struct State {
    pub settings: Settings,
    pub tower: bool,
    pub timing_rows: Vec<crate::timing::Runner>,
    pub timing_status: String,
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
