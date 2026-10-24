use crate::{
    model::{Pose, Shot},
    runtime,
    state::{save, state},
};
use hudhook::{
    imgui::{Condition, Key, MouseButton, StyleColor, Ui},
    ImguiRenderLoop,
};
#[derive(Default)]
struct ToggleKeys {
    previous: [bool; 5],
}
impl ToggleKeys {
    fn sample(&mut self, down: [bool; 5], focused: bool) -> [bool; 5] {
        let pressed = std::array::from_fn(|i| focused && down[i] && !self.previous[i]);
        self.previous = down;
        pressed
    }
    fn poll(&mut self, focused: bool) -> [bool; 5] {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
        let down = [0x2D, 0x75, 0x76, 0x79, 0x77].map(|vk| unsafe { GetAsyncKeyState(vk) < 0 });
        self.sample(down, focused)
    }
}
pub struct Panel {
    keys: ToggleKeys,
    autosave: crate::state::AutoSave,
    open: bool,
    filter: String,
    shot_name: String,
    message: String,
    page: usize,
}
impl Panel {
    pub fn new() -> Self {
        runtime::start();
        Self {
            keys: ToggleKeys::default(),
            autosave: crate::state::AutoSave::new(),
            open: false,
            filter: String::new(),
            shot_name: "New shot".into(),
            message: String::new(),
            page: 0,
        }
    }
}
pub(crate) fn focused() -> bool {
    unsafe {
        use windows_sys::Win32::{
            System::Threading::GetCurrentProcessId,
            UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId},
        };
        let mut pid = 0;
        GetWindowThreadProcessId(GetForegroundWindow(), &mut pid);
        pid == GetCurrentProcessId()
    }
}
impl ImguiRenderLoop for Panel {
    fn initialize<'a>(
        &'a mut self,
        ctx: &mut hudhook::imgui::Context,
        _loader: hudhook::TextureLoader<'a>,
    ) {
        crate::design::theme(ctx);
    }

    fn before_render(&mut self, ctx: &mut hudhook::imgui::Context) {
        let locked = state()
            .lock()
            .map(|s| crate::capture::active(s.capture, s.camera, s.race, focused()))
            .unwrap_or(false);
        ctx.io_mut().mouse_draw_cursor = false;
        ctx.io_mut()
            .config_flags
            .set(hudhook::imgui::ConfigFlags::NO_MOUSE_CURSOR_CHANGE, locked);
    }
    fn should_block_messages(&self, io: &hudhook::imgui::Io) -> bool {
        if !focused() {
            return false;
        }
        let capture = state()
            .lock()
            .map(|s| s.capture && s.camera && s.race)
            .unwrap_or(false);
        capture
            || (self.open
                && (io.want_capture_mouse || io.want_capture_keyboard || io.want_text_input))
    }
    fn render(&mut self, ui: &mut Ui) {
        let has_focus = focused();
        let [menu_key, camera_key, ui_key, restore_key, capture_key] = self.keys.poll(has_focus);
        let Ok(mut s) = state().lock() else { return };
        if let Some(result) = self.autosave.tick(&s) {
            self.message = match result {
                Ok(()) => "Settings saved automatically".into(),
                Err(e) => format!("Auto-save failed: {e}"),
            };
        }
        if !has_focus {
            s.capture = false;
            if s.tower && s.race {
                crate::timing::draw(
                    ui,
                    &s.timing_rows,
                    &s.settings.tower,
                    if s.camera { s.gate } else { -1 },
                );
            }
            return;
        }
        if capture_key {
            if s.capture {
                s.capture = false;
            } else if s.camera && s.race && crate::capture::available() {
                s.capture = true;
                self.open = false;
            } else {
                self.message =
                    "Enable camera during a race; cursor bindings must be available.".into();
            }
        }
        if unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(0x1B) < 0 } {
            s.capture = false;
        }
        if !s.camera || !s.race {
            s.capture = false;
        }
        if menu_key && !s.capture {
            self.open = !self.open;
        }
        if restore_key {
            s.capture = false;
            s.camera = false;
            s.hide_ui = false;
            s.hide_bottom = false;
            s.hide_skill_popups = false;
            s.hide_markers = false;
            s.skip_cutins = false;
            s.hide_speed_lines = false;
            s.hide_overhead = false;
            s.hide_trees = false;
            s.clean_effects = false;
            s.graphics = false;
            s.tower = false;
        }
        if s.capture || (!ui.io().want_capture_keyboard && !ui.io().want_text_input) {
            if camera_key {
                s.camera = !s.camera;
                if !s.camera {
                    s.capture = false;
                }
            }
            if ui_key {
                s.hide_ui = !s.hide_ui;
            }
            if s.race && s.camera {
                let dt = ui.io().delta_time.min(0.1);
                let speed = if ui.io().key_shift { 0.3 } else { 1. };
                if s.freecam {
                    let axis = |positive: Key, negative: Key| {
                        (ui.is_key_down(positive) as i32 - ui.is_key_down(negative) as i32) as f32
                    };
                    let move_speed = s.settings.free_speed
                        * if ui.io().key_shift {
                            3.
                        } else if ui.io().key_ctrl {
                            0.2
                        } else {
                            1.
                        };
                    if let Some(free) = &mut s.free_pose {
                        free.translate(
                            axis(Key::D, Key::A),
                            axis(Key::E, Key::Q),
                            axis(Key::W, Key::S),
                            dt * move_speed,
                        );
                        free.yaw += axis(Key::RightArrow, Key::LeftArrow) * dt;
                        free.pitch += axis(Key::DownArrow, Key::UpArrow) * dt;
                        free.sanitize();
                    }
                } else {
                    if ui.is_key_down(Key::LeftArrow) {
                        s.settings.pose.yaw -= dt * speed;
                    }
                    if ui.is_key_down(Key::RightArrow) {
                        s.settings.pose.yaw += dt * speed;
                    }
                    if ui.is_key_down(Key::UpArrow) {
                        s.settings.pose.pitch += dt * speed * 0.5;
                    }
                    if ui.is_key_down(Key::DownArrow) {
                        s.settings.pose.pitch -= dt * speed * 0.5;
                    }
                    if ui.is_key_down(Key::PageUp) {
                        s.settings.pose.height += dt * speed * 3.;
                    }
                    if ui.is_key_down(Key::PageDown) {
                        s.settings.pose.height -= dt * speed * 3.;
                    }
                }
                if ui.is_key_pressed(Key::LeftBracket) || ui.is_key_pressed(Key::RightBracket) {
                    let i = s.gates.iter().position(|g| *g == s.gate).unwrap_or(0);
                    let n = s.gates.len();
                    if n > 0 {
                        s.gate = s.gates[(i + if ui.is_key_pressed(Key::RightBracket) {
                            1
                        } else {
                            n - 1
                        }) % n];
                    }
                }
            }
        }
        if s.camera && s.race && !s.capture && !ui.io().want_capture_mouse {
            let sensitivity = s.settings.mouse_sensitivity;
            if s.freecam {
                if let Some(free) = &mut s.free_pose {
                    if ui.is_mouse_down(MouseButton::Right) {
                        free.yaw += ui.io().mouse_delta[0] * sensitivity;
                        free.pitch += ui.io().mouse_delta[1] * sensitivity;
                    }
                    free.fov *= (-ui.io().mouse_wheel * 0.06).exp();
                    free.sanitize();
                }
            } else {
                if ui.is_mouse_down(MouseButton::Right) {
                    s.settings.pose.yaw += ui.io().mouse_delta[0] * sensitivity;
                    s.settings.pose.pitch -= ui.io().mouse_delta[1] * sensitivity;
                }
                s.settings.pose.distance *= (-ui.io().mouse_wheel * 0.12).exp();
            }
        }
        s.settings.sanitize();
        if s.tower && s.race {
            crate::timing::draw(
                ui,
                &s.timing_rows,
                &s.settings.tower,
                if s.camera { s.gate } else { -1 },
            );
        }
        if !self.open || s.capture {
            return;
        }

        let display = ui.io().display_size;
        ui.window("RaceDirector Studio")
            .size([940_f32.min(display[0] - 24.).max(320.), 760_f32.min(display[1] - 24.).max(300.)], Condition::FirstUseEver)
            .position([12., 12.], Condition::FirstUseEver)
            .build(|| {
                ui.text_colored(crate::design::ACCENT, "RACE DIRECTOR  /  STUDIO");
                ui.same_line();
                ui.text_disabled(format!("v{}  /  {:.0} FPS", env!("CARGO_PKG_VERSION"), ui.io().framerate));
                ui.text_colored(if s.race { [0.46,0.84,0.70,1.] } else { crate::design::MUTED }, if s.race { "CONNECTED   /   Race camera & broadcast controls" } else { "STANDBY   /   Enter a race to connect" });
                ui.separator();
                ui.child_window("navigation").size([166., -76.]).build(|| {
                    crate::design::section(ui, "WORKSPACE");
                    for (i, label) in ["Camera", "Visuals", "Saved shots", "Performance", "Timing tower", "Help"].iter().enumerate() {
                        if ui.selectable_config(format!("{:02}   {}", i+1, label)).selected(self.page == i).size([0., 42.]).build() {
                            self.page = i;
                        }
                    }
                    ui.separator();
                    crate::design::section(ui, "SHORTCUTS");
                    ui.text("Insert  Panel");
                    ui.text("F6  Camera");
                    ui.text("F7  Custom UI");
                    ui.text("F8  Clean lock");
                    ui.text("F10 Restore");
                });
                ui.same_line();
                ui.child_window("page").size([0., -76.]).build(|| {
                    let _width = ui.push_item_width(-1.);
                    match self.page {
                        0 => {
                            heading(ui, "Camera", "Frame the race. Follow a runner, then refine your shot.");
                            option(ui, "Camera control  [F6]", "Take control of race framing and runner tracking.", &mut s.camera);
                            ui.separator();
                            if crate::design::toggle(ui, "Free camera", "Fly freely around the pack. Turn off to follow a runner.", &mut s.freecam) { s.free_pose = None; }
                            if s.freecam {
                                ui.text_wrapped("WASD flies in the viewing direction. Q/E moves vertically. Shift boosts speed; Ctrl slows it. Right-drag looks around. F8 locks the cursor for mouse-look without holding a button.");
                                ui.text("Flight speed"); ui.slider("##flyspeed",0.5,150.,&mut s.settings.free_speed);
                                if let Some(free) = &mut s.free_pose {
                                    ui.text("Field of view"); ui.slider("##freefov",5.,100.,&mut free.fov);
                                }
                                if ui.button("Reset freecam from orbit framing") { s.free_pose = None; }
                            } else {
                            ui.text("Follow runner");
                            if s.gates.is_empty() { ui.text_disabled("Runner selection appears during a race."); }
                            let gates = s.gates.clone();
                            let columns = ((ui.content_region_avail()[0] + 10.) / 52.).floor().max(1.) as usize;
                            for (i, gate) in gates.iter().enumerate() {
                                if i % columns != 0 { ui.same_line(); }
                                let selected = *gate == s.gate;
                                let _color = selected.then(|| ui.push_style_color(StyleColor::Button, [0.35, 0.42, 0.64, 1.]));
                                if ui.button_with_size(format!("{gate}"), [42., 32.]) { s.gate = *gate; }
                            }
                            ui.spacing();
                            ui.text("Orbit"); ui.slider("##orbit", -3.14159, 3.14159, &mut s.settings.pose.yaw);
                            ui.text("Pitch"); ui.slider("##pitch", -1.2, 1.4, &mut s.settings.pose.pitch);
                            ui.text("Distance"); ui.slider("##distance", 2., 200., &mut s.settings.pose.distance);
                            ui.text("Height"); ui.slider("##height", -1., 20., &mut s.settings.pose.height);
                            ui.text("Field of view"); ui.slider("##fov", 5., 80., &mut s.settings.pose.fov);
                            }
                            ui.separator();
                            crate::design::section(ui, "CAMERA MOTION");
                            ui.text("Target switch duration (seconds)"); ui.slider("##targettransition",0.,3.,&mut s.settings.target_transition);
                            ui.text_wrapped("Ease between runners without falling behind the pack. Set to zero for instant switching.");
                            ui.text("Minimum ground clearance"); ui.slider("##groundclearance",0.15,3.,&mut s.settings.ground_clearance);
                            ui.text_wrapped("Keeps follow camera and freecam above the sampled course surface.");
                            ui.text_wrapped(&s.ground_status);
                            ui.text("Smoothing response"); ui.slider("##response", 0.5, 30., &mut s.settings.response);
                            ui.text_disabled("Lower values give smoother movement, look and zoom in both camera modes.");
                            ui.text("Mouse sensitivity"); ui.slider("##sensitivity", 0.001, 0.02, &mut s.settings.mouse_sensitivity);
                            if ui.button("Reset framing") { s.settings.pose = Pose::default(); s.free_pose = None; }
                            ui.separator();
                            ui.text_wrapped("F8 clean capture closes and locks this panel, hides the cursor and enables mouse-look. F8, Escape or F10 releases it; Insert opens settings afterwards.");
                            ui.text_wrapped(&s.capture_status);
                            if ui.button("Start clean capture  [F8]") && s.camera && s.race && crate::capture::available() { s.capture = true; self.open = false; }
                        }
                        1 => {
                            heading(ui, "Visuals", "Keep the shot clear. Each option works independently.");
                            crate::design::section(ui, "RACE HUD");
                            option(ui, "Hide bottom HUD", "Commentary, rank portrait, bottom controls and photo button.", &mut s.hide_bottom);
                            option(ui, "Hide skill popups + sidebar", "Hide runner skill-name plates and the side notification feed. Skill gameplay stays active.", &mut s.hide_skill_popups);
                            option(ui, "Hide runner arrows", "Remove the markers above runners; keep minimap icons.", &mut s.hide_markers);
                            crate::design::section(ui, "CINEMATICS & EFFECTS");
                            option(ui, "Skip skill cut-ins", "Skip new unique/skill animations. An animation already playing finishes normally.", &mut s.skip_cutins);
                            option(ui, "Hide speed lines", "Remove radial speed blur and skill-line particles.", &mut s.hide_speed_lines);
                            crate::design::section(ui, "SCENE & PERFORMANCE");
                            option(ui, "Disable overhead camera", "Stop the extra race camera and inset rendering; keep the minimap.", &mut s.hide_overhead);
                            option(ui, "Hide trees", "Hide supported course trees throughout the view.", &mut s.hide_trees);
                            option(ui, "Suppress skill glow + blur/bloom", "Remove skill particles and race bloom, depth of field and radial blur.", &mut s.clean_effects);
                            ui.text_wrapped("Hidden HUD buttons can still receive clicks. F10 brings them back.");
                            ui.separator();
                            crate::design::section(ui, "CUSTOM HUD SELECTION");
                            option(ui, "Custom selection  [F7]", "Hide only the UI branches selected below.", &mut s.hide_ui);
                            if ui.button("Scan race UI") { s.scan = true; }
                            ui.same_line();
                            if ui.button("Clear selection") { s.settings.hidden_paths.clear(); }
                            ui.input_text("##filter", &mut self.filter).hint("Filter UI paths...").build();
                            ui.text_disabled(format!("{} selected", s.settings.hidden_paths.len()));
                            ui.child_window("paths").size([0., 210.]).build(|| {
                                let filter = self.filter.to_lowercase();
                                if s.paths.is_empty() { ui.text_wrapped("Enter a race and choose Scan race UI to discover individual elements."); }
                                for (i, path) in s.paths.clone().iter().enumerate() {
                                    if !path.to_lowercase().contains(&filter) { continue; }
                                    let _id = ui.push_id_usize(i);
                                    let mut selected = s.settings.hidden_paths.contains(path);
                                    let leaf = path.rsplit('/').next().unwrap_or(path);
                                    if ui.checkbox(leaf, &mut selected) {
                                        if selected { s.settings.hidden_paths.push(path.clone()); }
                                        else { s.settings.hidden_paths.retain(|p| p != path); }
                                    }
                                    if ui.is_item_hovered() { ui.tooltip_text(path); }
                                }
                            });
                            ui.text_wrapped(&s.visual_status);
                        }
                        2 => {
                            heading(ui, "Saved shots", "Recall framing instantly. Shots are saved automatically.");
                            ui.input_text("##shotname", &mut self.shot_name).hint("Shot name").build();
                            if ui.button("Save current framing") && !self.shot_name.trim().is_empty() {
                                let name = self.shot_name.trim().to_owned();
                                let pose = s.settings.pose;
                                let free = if s.freecam { s.free_pose } else { None };
                                if let Some(shot) = s.settings.shots.iter_mut().find(|p| p.name == name) { shot.pose = pose; shot.free = free; }
                                else { s.settings.shots.push(Shot { name, pose, free }); }
                                self.message = "Shot captured. Auto-save will keep it.".into();
                            }
                            ui.separator();
                            if s.settings.shots.is_empty() { ui.text_wrapped("No saved shots yet. Set up a camera angle, name it, then save its framing."); }
                            let mut remove = None;
                            for (i, shot) in s.settings.shots.clone().iter().enumerate() {
                                let _id = ui.push_id_usize(i);
                                if ui.button_with_size(&shot.name, [(ui.content_region_avail()[0] - 86.).max(80.), 36.]) { s.settings.pose = shot.pose; s.freecam = shot.free.is_some(); s.free_pose = shot.free; s.camera = true; }
                                ui.same_line();
                                if ui.button_with_size("Delete", [76., 36.]) { remove = Some(i); }
                                if let Some(free) = shot.free { ui.text_disabled(format!("Freecam / pack offset {:.1}, {:.1}, {:.1} / FOV {:.0}",free.offset.x,free.offset.y,free.offset.z,free.fov)); }
                                else { ui.text_disabled(format!("Distance {:.1}   Height {:.1}   FOV {:.0}", shot.pose.distance, shot.pose.height, shot.pose.fov)); }
                            }
                            if let Some(i) = remove { s.settings.shots.remove(i); }
                        }
                        3 => {
                            heading(ui, "Performance", "A smoother view for racing and recording.");
                            ui.text_colored([0.64, 0.70, 1., 1.], format!("{:.0} FPS", ui.io().framerate));
                            ui.text_disabled(format!("{:.1} ms / frame", 1000. / ui.io().framerate.max(1.)));
                            ui.spacing();
                            option(ui, "Enable 60 FPS", "Request 60 FPS across the game. Enabled by default.", &mut s.settings.fps_60);
                            ui.text_wrapped(&s.fps_status);
                            ui.text_wrapped("Turns off Unity VSync while active. Disabling restores the values captured before the override. Actual FPS depends on GPU load and other game limits.");
                            ui.separator();
                            crate::design::section(ui, "MODEL CLARITY");
                            let was_enabled = s.graphics;
                            option(ui, "Enable model clarity", "Race-only supersampling and MSAA. Remembers whether it is enabled.", &mut s.graphics);
                            if s.graphics && !was_enabled {
                                s.graphics_applied = s.settings.graphics;
                            }
                            ui.text_wrapped(&s.graphics_status);
                            let scales = [100, 125, 150, 200];
                            let mut scale = scales.iter().position(|v| *v == s.settings.graphics.scale).unwrap_or(1);
                            ui.combo_simple_string("Internal resolution", &mut scale, &["100% (game resolution)", "125%", "150%", "200%"]);
                            s.settings.graphics.scale = scales[scale];
                            let samples = [1, 2, 4];
                            let mut aa = samples.iter().position(|v| *v == s.settings.graphics.msaa).unwrap_or(1);
                            ui.combo_simple_string("Anti-aliasing", &mut aa, &["Off", "2x MSAA", "4x MSAA"]);
                            s.settings.graphics.msaa = samples[aa];
                            if ui.button("Apply clarity settings") { s.graphics_applied = s.settings.graphics; s.graphics = true; }
                            ui.same_line();
                            if ui.button("Restore game rendering") { s.graphics = false; }
                            ui.text_wrapped("Higher internal resolution sharpens the race image before downsampling. MSAA smooths model edges. 200% renders four times as many pixels; start at 125% + 2x. Actual MSAA and resolution appear above after applying in a race. Resolution is capped at 16 megapixels / 8192 per axis.");
                            ui.text_wrapped("These controls use supersampling, not DLSS or FSR. HUD size stays unchanged. Your applied clarity levels and enabled state are saved automatically. F10 restores game rendering and camera visuals, leaving the FPS preference unchanged.");
                        }
                        4 => {
                            heading(ui, "Timing tower", "Live race order, speed and gaps for the entire field.");
                            option(ui, "Timing tower", "A clean broadcast overlay for every runner in the field.", &mut s.tower);
                            ui.text_wrapped("The tower stays visible with settings closed and during F8 clean capture. It is included in game captures and never takes mouse input. F10 hides it.");
                            ui.separator();
                            crate::design::section(ui, "DATA & UNITS");
                            option(ui, "Speed column", "Show current race velocity alongside each runner.", &mut s.settings.tower.speed);
                            crate::design::choice(ui, "Speed units", "km/h", "m/s", &mut s.settings.tower.metres_per_second);
                            crate::design::choice(ui, "Gap reference", "Leader", "Runner ahead", &mut s.settings.tower.interval);
                            crate::design::choice(ui, "Gap units", "Metres", "Estimated seconds", &mut s.settings.tower.estimated_seconds);
                            ui.text_wrapped("Seconds estimate = track-distance gap / trailing runner speed. This is not a measured crossing-time interval. Missing values show --.");
                            ui.separator();
                            crate::design::section(ui, "APPEARANCE");
                            crate::design::choice(ui, "Screen side", "Left", "Right", &mut s.settings.tower.right);
                            ui.text("Size"); ui.slider("##towerscale",0.6,1.5,&mut s.settings.tower.scale);
                            ui.text("Background opacity"); ui.slider("##toweralpha",0.2,1.,&mut s.settings.tower.opacity);
                            ui.text("Vertical position"); ui.slider("##towery",0.,1.,&mut s.settings.tower.vertical);
                            ui.text_wrapped("Automatically shrinks to fit all runners on screen. The followed runner is highlighted. Layout and visibility are saved automatically.");
                            ui.separator();ui.text_wrapped(&s.timing_status);
                        }
                        _ => {
                            heading(ui, "Help & status", "Controls stay available with the panel closed.");
                            for line in ["F8 / Clean capture lock (F8 or Escape releases)", "WASD + Q/E / Freecam flight", "Shift / Freecam boost; Ctrl / slow", "Right-drag  /  Orbit and pitch", "Mouse wheel  /  Zoom", "Arrow keys  /  Orbit and pitch", "Page Up / Down  /  Height", "[ and ]  /  Previous or next runner", "Shift  /  Fine keyboard movement", "F10  /  Restore camera and all visual hiding"] { ui.text(line); }
                            ui.separator(); ui.text_wrapped(&s.status);
                            ui.text_wrapped(&s.visual_status); ui.text_wrapped(&s.fps_status);
                            ui.text_wrapped("Settings and enabled features are saved automatically every second when changed. F8 cursor capture remains manual. F10 restores visuals and remembers that choice.");
                        }
                    }
                });
                ui.separator();
                if ui.button_with_size("Save settings", [150.,36.]) {
                    self.message = match save(&s.snapshot()) { Ok(()) => "Settings saved".into(), Err(e) => format!("Save failed: {e}") };
                }
                ui.same_line();
                if ui.button_with_size("Restore visuals  [F10]", [220.,36.]) {
                    s.capture = false; s.camera = false; s.hide_ui = false; s.hide_bottom = false; s.hide_skill_popups = false; s.hide_markers = false; s.skip_cutins = false; s.hide_speed_lines = false; s.hide_overhead = false; s.hide_trees = false; s.clean_effects = false; s.graphics = false; s.tower = false;
                }
                ui.text_wrapped(&self.message);
            });
    }
}

#[cfg(test)]
mod input_tests {
    use super::ToggleKeys;
    #[test]
    fn held_toggle_fires_once() {
        let mut k = ToggleKeys::default();
        assert_eq!(
            k.sample([true, false, false, false, false], true),
            [true, false, false, false, false]
        );
        assert_eq!(
            k.sample([true, false, false, false, false], true),
            [false; 5]
        );
        k.sample([false; 5], true);
        assert!(k.sample([true, false, false, false, false], true)[0]);
    }
    #[test]
    fn unfocused_press_is_not_replayed() {
        let mut k = ToggleKeys::default();
        assert_eq!(
            k.sample([false, true, false, false, false], false),
            [false; 5]
        );
        assert_eq!(
            k.sample([false, true, false, false, false], true),
            [false; 5]
        );
    }
}

fn heading(ui: &Ui, title: &str, subtitle: &str) {
    crate::design::heading(ui, title, subtitle);
}
fn option(ui: &Ui, title: &str, description: &str, enabled: &mut bool) {
    crate::design::toggle(ui, title, description, enabled);
}
