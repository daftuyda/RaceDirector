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
    previous: [bool; 6],
}
impl ToggleKeys {
    fn sample(&mut self, down: [bool; 6], focused: bool) -> [bool; 6] {
        let pressed = std::array::from_fn(|i| focused && down[i] && !self.previous[i]);
        self.previous = down;
        pressed
    }
    fn poll(&mut self, focused: bool) -> [bool; 6] {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
        let down =
            [0x2D, 0x75, 0x76, 0x79, 0x77, 0x78].map(|vk| unsafe { GetAsyncKeyState(vk) < 0 });
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
    tower_motion: crate::timing::TowerMotion,
    timing_generation: u64,
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
            tower_motion: crate::timing::TowerMotion::default(),
            timing_generation: 0,
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
        let dark = state()
            .lock()
            .map(|s| s.settings.dark_mode)
            .unwrap_or(false);
        crate::design::apply_theme(ctx, dark);
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
        let [menu_key, camera_key, ui_key, restore_key, capture_key, battle_key] =
            self.keys.poll(has_focus);
        let Ok(mut s) = state().lock() else { return };
        if let Some(result) = self.autosave.tick(&s) {
            self.message = match result {
                Ok(()) => "Settings saved automatically".into(),
                Err(e) => format!("Auto-save failed: {e}"),
            };
        }
        if !has_focus {
            s.capture = false;
            self.draw_coverage(ui, &s);
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
            s.restore_visuals();
        }
        if !restore_key
            && (s.capture || (!ui.io().want_capture_keyboard && !ui.io().want_text_input))
        {
            if camera_key {
                s.camera = !s.camera;
                if !s.camera {
                    s.capture = false;
                }
            }
            if ui_key {
                s.hide_ui = !s.hide_ui;
            }
            if battle_key && s.race && s.camera && !s.freecam {
                if s.settings.director.enabled {
                    s.settings.director.enabled = false;
                    s.settings.battle.enabled = false;
                } else {
                    s.settings.battle.enabled = !s.settings.battle.enabled;
                }
                s.coverage_gate = None;
                s.battle_info = None;
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
        self.draw_coverage(ui, &s);
        if !self.open || s.capture {
            return;
        }

        self.draw_settings(ui, &mut s);
    }
}

impl Panel {
    fn draw_coverage(&mut self, ui: &Ui, s: &crate::state::State) {
        if self.timing_generation != s.timing_generation || !s.race {
            self.tower_motion.reset();
            self.timing_generation = s.timing_generation;
        }
        if !s.race {
            return;
        }
        let selected = (s.camera && !s.freecam).then_some(s.coverage_gate.unwrap_or(s.gate));
        let battle = s.battle_info.as_ref().filter(|info| {
            selected == Some(info.anchor)
                && (s.settings.battle.enabled || s.settings.director.enabled)
                && s.timing_rows.iter().any(|r| r.gate == info.partner)
        });
        if s.settings.broadcast.focus_markers {
            if let Some(focus) = selected {
                crate::markers::draw(
                    ui,
                    &s.focus_markers,
                    &s.timing_rows,
                    focus,
                    battle.map(|b| b.partner),
                );
            }
        }
        if s.tower {
            crate::timing::draw(
                ui,
                &s.timing_rows,
                &s.settings.tower,
                selected.unwrap_or(-1),
                battle.map(|b| b.partner),
                &mut self.tower_motion,
            );
        } else {
            self.tower_motion.reset();
        }
        let mut broadcast = s.settings.broadcast.clone();
        broadcast.race_distance_m = s.race_distance();
        crate::broadcast::draw(
            ui,
            &s.timing_rows,
            &s.settings.tower,
            &broadcast,
            if battle.is_some() && s.settings.broadcast.battle_card {
                None
            } else {
                selected
            },
            &s.finished_gates,
            s.tower,
        );
        if let Some(info) =
            battle.filter(|_| !(s.settings.broadcast.results && !s.finished_gates.is_empty()))
        {
            crate::broadcast::draw_battle(
                ui,
                &s.timing_rows,
                &s.settings.tower,
                &s.settings.broadcast,
                info,
                s.tower,
            );
        }
    }
    fn draw_settings(&mut self, ui: &Ui, s: &mut crate::state::State) {
        let display = ui.io().display_size;
        ui.window("RaceDirector Airframe")
            .flags(hudhook::imgui::WindowFlags::NO_TITLE_BAR)
            .size([600_f32.min(display[0] - 24.).max(280.), 820_f32.min(display[1] - 24.).max(300.)], Condition::FirstUseEver)
            .position([12., 12.], Condition::FirstUseEver)
            .build(|| {
                ui.text("RACE DIRECTOR");
                let available = ui.content_region_avail()[0];
                if available > 370. { ui.same_line_with_pos(available - 142.); }
                for (label, dark) in [("Light", false), ("Dark", true)] {
                    if label == "Dark" { ui.same_line(); }
                    let selected = s.settings.dark_mode == dark;
                    let _button = ui.push_style_color(StyleColor::Button, if selected {ui.style_color(StyleColor::SliderGrab)} else {ui.style_color(StyleColor::FrameBg)});
                    let _text = ui.push_style_color(StyleColor::Text, if selected {[1.,1.,1.,1.]} else {ui.style_color(StyleColor::Text)});
                    if ui.button_with_size(label,[62.,28.]) { s.settings.dark_mode = dark; }
                }
                ui.text_disabled(format!("{}   /   {:.0} FPS   /   v{}", if s.race {"IN RACE"} else {"STANDBY"}, ui.io().framerate,env!("CARGO_PKG_VERSION")));
                ui.separator();
                let labels = ["Camera", "Visuals", "Shots", "Perf", "Tower", "On air", "Help"];
                let columns = if ui.content_region_avail()[0] < 500. {3} else {7};
                let tab_width = (ui.content_region_avail()[0] - (columns - 1) as f32 * 6.) / columns as f32;
                for (i, label) in labels.iter().enumerate() {
                    if i % columns != 0 {ui.same_line_with_spacing(0.,6.);}
                    let selected = self.page == i;
                    let p = ui.cursor_screen_pos();
                    let _bg = ui.push_style_color(StyleColor::Button,[0.,0.,0.,0.]);
                    let _text = ui.push_style_color(StyleColor::Text,ui.style_color(if selected {StyleColor::SliderGrab}else{StyleColor::TextDisabled}));
                    if ui.button_with_size(label,[tab_width,36.]) {self.page=i;}
                    if selected {ui.get_window_draw_list().add_line([p[0],p[1]+35.],[p[0]+tab_width,p[1]+35.],ui.style_color(StyleColor::SliderGrab)).thickness(2.).build();}
                }
                ui.spacing();
                ui.child_window("page").size([0., -92.]).build(|| {
                    let _width = ui.push_item_width(-1.);
                    match self.page {
                        0 => {
                            if ui.collapsing_header("Mode", hudhook::imgui::TreeNodeFlags::DEFAULT_OPEN) {
                                option(ui, "Camera control  [F6]", "", &mut s.camera);
                                let old_freecam = s.freecam;
                                crate::design::choice(ui, "##camera-mode", "Follow", "Freecam", &mut s.freecam);
                                if old_freecam != s.freecam { s.free_pose = None; }
                                if !s.freecam {
                                    let preview = s.timing_rows.iter().find(|r| r.gate == s.gate)
                                        .map(|r|format!("Gate {:02}  {}",r.gate,r.name)).unwrap_or_else(||format!("Gate {:02}",s.gate));
                                    if let Some(_combo) = ui.begin_combo("##followrunner", &preview) {
                                        for gate in s.gates.clone() {
                                            let label = s.timing_rows.iter().find(|r|r.gate==gate)
                                                .map(|r|format!("Gate {gate:02}  {}",r.name)).unwrap_or_else(||format!("Gate {gate:02}"));
                                            if ui.selectable_config(&label).selected(s.gate==gate).build(){s.gate=gate;}
                                        }
                                    }
                                    if s.gates.is_empty(){ui.text_disabled("Enter a race to choose a runner.");}
                                    option(ui, "Battle camera  [F9]", "Frame this runner and a nearby rival together.", &mut s.settings.battle.enabled);
                                    if s.settings.battle.enabled {
                                        let partner = s.settings.battle.partner_gate;
                                        let partner_label = partner.map(|g| runner_label(s, g)).unwrap_or_else(|| "Auto / nearest rival".into());
                                        if let Some(_combo) = ui.begin_combo("##battlepartner", &partner_label) {
                                            if ui.selectable_config("Auto / nearest rival").selected(partner.is_none()).build() {s.settings.battle.partner_gate=None;}
                                            for gate in s.gates.clone().into_iter().filter(|g| *g != s.gate) {
                                                if ui.selectable_config(&runner_label(s, gate)).selected(partner==Some(gate)).build() {s.settings.battle.partner_gate=Some(gate);}
                                            }
                                        }
                                        if let Some(info) = &s.battle_info {
                                            ui.text_wrapped(format!("Following {} + {}", runner_label(s,info.anchor),runner_label(s,info.partner)));
                                        } else { ui.text_wrapped("Waiting for an eligible rival and camera aspect. Normal follow continues until a pair is available."); }
                                        if ui.collapsing_header("Battle selection",hudhook::imgui::TreeNodeFlags::empty()) {
                                            inspector_slider(ui,"Minimum hold (s)",1.,15.,&mut s.settings.battle.hold_seconds);
                                            inspector_slider(ui,"Maximum gap (m)",5.,100.,&mut s.settings.battle.max_gap);
                                            ui.text_wrapped("Auto considers the runners immediately ahead and behind. The pair stays together through an overtake. Manual partners are saved by gate; review for each race.");
                                        }
                                    }
                                } else {
                                    ui.text_wrapped("WASD + Q/E to fly. Shift boosts; Ctrl slows. Right-drag looks around; F8 locks mouse-look.");
                                }
                            }
                            ui.spacing();
                            if ui.collapsing_header("Automatic director",hudhook::imgui::TreeNodeFlags::DEFAULT_OPEN) {
                                let was=s.settings.director.enabled;
                                option(ui,"Automatic directing","Enable Follow camera, tour the runners and cover the finish.",&mut s.settings.director.enabled);
                                if s.settings.director.enabled && !was { s.start_director(); }
                                ui.text_wrapped(&s.director_status);
                                ui.text_wrapped(s.distance_status());
                                if s.settings.director.enabled && (!s.camera || s.freecam) && ui.button("Resume automatic Follow") { s.start_director(); }
                                ui.text("Race length override (0 = Auto)");
                                ui.input_float("##directorracelength",&mut s.settings.broadcast.race_distance_m).step(100.).build();
                                if s.settings.broadcast.race_distance_m > 0. && ui.button("Use game race length") { s.settings.broadcast.race_distance_m=0.; }
                                option(ui,"Include battles","Frame a nearby rival during the tour and regular Follow.",&mut s.settings.director.battles);
                                if ui.collapsing_header("Continuous movement",hudhook::imgui::TreeNodeFlags::DEFAULT_OPEN) {
                                    option(ui,"Gentle camera movement","Slow orbit, dolly and zoom from the moment directing starts, including while paused.",&mut s.settings.director.ambient_motion);
                                    inspector_slider(ui,"Movement amount",0.,1.5,&mut s.settings.director.motion_amount);
                                    inspector_slider(ui,"Movement cycle (s)",12.,60.,&mut s.settings.director.motion_period);
                                    ui.text_wrapped("Movement continues between target changes. Freecam and manual Follow are unaffected. A fixed finish shot keeps its position.");
                                }
                                if ui.collapsing_header("Runner tour",hudhook::imgui::TreeNodeFlags::empty()) {
                                    option(ui,"Mid-race runner tour","Visit each runner once, then return to your selected runner.",&mut s.settings.director.tour_enabled);
                                    inspector_slider(ui,"Tour start (race fraction)",0.05,0.75,&mut s.settings.director.start_fraction);
                                    inspector_slider(ui,"Seconds per runner",5.,15.,&mut s.settings.director.dwell);
                                    if ui.button("Start / restart runner tour now") { s.start_director(); s.settings.director.tour_enabled=true; s.restart_tour=true; }
                                    ui.text_wrapped("The tour starts at your selected runner and visits nearby runners without flipping camera sides. Holds use viewing time, including during fast-forward; pausing stops runner changes. Finish coverage takes priority.");
                                }
                                if ui.collapsing_header("Finish coverage",hudhook::imgui::TreeNodeFlags::empty()) {
                                    option(ui,"Automatic finish coverage","Follow the lead contest near the finish. Requires director or Battle camera.",&mut s.settings.battle.finish_coverage);
                                    inspector_slider(ui,"Switch with metres remaining",50.,1000.,&mut s.settings.battle.finish_distance);
                                    option(ui,"Front-quarter finish view","Use a moving front angle before the stationary shot.",&mut s.settings.battle.finish_front_view);
                                    option(ui,"Fixed finish camera","Pan and zoom from a stationary position near the line.",&mut s.settings.director.static_finish);
                                    option(ui,"Estimate finish position automatically","In the last 120 m, use the leaders' direction and course scale to place the camera. An armed saved position takes priority.",&mut s.settings.director.auto_finish);
                                    ui.text("Finish camera side");
                                    crate::design::choice(ui,"##finishside","Left of travel","Right of travel",&mut s.settings.director.finish_right_side);
                                    ui.text_wrapped("Left is the opposite side from v0.1.22. This changes the moving finish angle and estimated fixed position. An armed saved position stays where you placed it.");
                                    ui.text_wrapped("Automatic placement estimates a straight finish from current race geometry. Use a saved spot for precise placement. No final-corner detector is used.");
                                    if s.camera && s.race && s.freecam {
                                        if ui.button("Save current Freecam as finish camera") { s.save_finish_camera=true; }
                                    } else { ui.text_wrapped("For a precise spot, position Freecam beside and beyond the finish line, save it here, then return to Follow."); }
                                    if s.settings.director.finish_camera.is_some() {
                                        if s.race && s.camera && ui.button("Arm saved finish camera for this race") { s.finish_camera_armed=true; }
                                        if ui.button("Clear saved finish camera") { s.settings.director.finish_camera=None; s.finish_camera_armed=false; }
                                        ui.text_disabled(if s.finish_camera_armed {"Saved position armed"} else {"Saved position unarmed / verify course"});
                                    }
                                    if s.race_distance()<=s.settings.battle.finish_distance { ui.text_wrapped("Finish coverage needs a race length longer than the threshold. Enter an override if Auto is unavailable."); }
                                    ui.text_wrapped("Saved positions must be armed for each new race. Without an available fixed position, moving lead-battle coverage continues.");
                                }
                                ui.text_wrapped("F9 stops automatic directing. F10 restores the game camera.");
                            }
                            ui.spacing();
                            if ui.collapsing_header("Framing", hudhook::imgui::TreeNodeFlags::DEFAULT_OPEN) {
                                if s.freecam {
                                    inspector_slider(ui,"Flight speed",0.5,150.,&mut s.settings.free_speed);
                                    if let Some(free) = &mut s.free_pose { inspector_slider(ui,"Field of view",5.,100.,&mut free.fov); }
                                } else {
                                    inspector_slider(ui,"Field of view",5.,80.,&mut s.settings.pose.fov);
                                    inspector_slider(ui,"Distance",2.,200.,&mut s.settings.pose.distance);
                                    inspector_slider(ui,"Height",-1.,20.,&mut s.settings.pose.height);
                                    if ui.collapsing_header("Orbit & pitch", hudhook::imgui::TreeNodeFlags::empty()) {
                                        inspector_slider(ui,"Orbit",-std::f32::consts::PI,std::f32::consts::PI,&mut s.settings.pose.yaw);
                                        inspector_slider(ui,"Pitch",-1.2,1.4,&mut s.settings.pose.pitch);
                                    }
                                }
                                if ui.button("Reset framing") { s.settings.pose = Pose::default(); s.free_pose = None; }
                            }
                            ui.spacing();
                            if ui.collapsing_header("Motion", hudhook::imgui::TreeNodeFlags::DEFAULT_OPEN) {
                                inspector_slider(ui,"Smoothing response",0.5,30.,&mut s.settings.response);
                                inspector_slider(ui,"Target transition (s)",0.,3.,&mut s.settings.target_transition);
                                inspector_slider(ui,"Ground clearance",0.15,3.,&mut s.settings.ground_clearance);
                                ui.text_wrapped("Lower response gives smoother motion. Zero transition switches targets instantly.");
                                if ui.collapsing_header("Advanced motion", hudhook::imgui::TreeNodeFlags::empty()) {
                                    inspector_slider(ui,"Mouse sensitivity",0.001,0.02,&mut s.settings.mouse_sensitivity);
                                    ui.text_wrapped(&s.ground_status);
                                }
                            }
                            ui.spacing();
                            ui.text_wrapped("F8 hides the cursor and settings. F8 or Escape releases capture.");
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
                            ui.text_colored(ui.style_color(StyleColor::SliderGrab), format!("{:.0} FPS", ui.io().framerate));
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
                            crate::design::section(ui, "TRAINERS");
                            option(ui, "Show trainer names", "An optional second line beneath each runner.", &mut s.settings.tower.show_trainers);
                            option(ui,"Automatic trainer names","Use names supplied by the game for this race.",&mut s.settings.tower.auto_trainers);
                            ui.text_wrapped("Nonblank manual names override automatic names. Clear an override to use the game name. Manual labels persist by gate; review them each race. Some race modes may not supply names.");
                            if ui.button("Clear manual overrides") { s.settings.tower.trainer_names.clear(); }
                            ui.child_window("trainer_names").size([0.,190.]).build(|| {
                                let gates = if s.gates.is_empty() {(1..=18).collect::<Vec<i32>>()}else{s.gates.clone()};
                                for gate in gates {
                                    let _id = ui.push_id(format!("trainer_{gate}"));
                                    let label = s.timing_rows.iter().find(|r|r.gate==gate).map(|r|format!("Gate {gate:02}  {}",r.name)).unwrap_or_else(||format!("Gate {gate:02}"));
                                    ui.text(&label);
                                    if let Some(name) = s.timing_rows.iter().find(|r|r.gate==gate).and_then(|r|r.trainer_name.as_deref()) {ui.text_disabled(format!("Game: {name}"));}
                                    let mut value = s.settings.tower.trainer_names.get(&gate).cloned().unwrap_or_default();
                                    if ui.input_text("##trainer", &mut value).hint("Manual override (optional)").build() {
                                        s.settings.tower.trainer_names.insert(gate,value);
                                    }
                                }
                            });
                            ui.separator();
                            crate::design::section(ui, "DATA & UNITS");
                            option(ui, "Speed column", "Show current race velocity alongside each runner.", &mut s.settings.tower.speed);
                            crate::design::choice(ui, "Speed units", "km/h", "m/s", &mut s.settings.tower.metres_per_second);
                            crate::design::choice(ui, "Gap reference", "Leader", "Runner ahead", &mut s.settings.tower.interval);
                            crate::design::choice(ui, "Gap units", "Metres", "Estimated seconds", &mut s.settings.tower.estimated_seconds);
                            ui.text_wrapped("Seconds estimate = track-distance gap / trailing runner speed. This is not a measured crossing-time interval. Missing values show --.");
                            ui.separator();
                            crate::design::section(ui, "APPEARANCE");
                            option(ui, "Animate position changes", "Rows glide into their new positions.", &mut s.settings.tower.animated_rows);
                            option(ui, "Show places gained / lost", "Brief indicators after a confirmed order change.", &mut s.settings.tower.position_changes);
                            crate::design::choice(ui, "Screen side", "Left", "Right", &mut s.settings.tower.right);
                            ui.text("Size"); ui.slider("##towerscale",0.6,1.5,&mut s.settings.tower.scale);
                            ui.text("Background opacity"); ui.slider("##toweralpha",0.2,1.,&mut s.settings.tower.opacity);
                            ui.text("Vertical position"); ui.slider("##towery",0.,1.,&mut s.settings.tower.vertical);
                            ui.text_wrapped("Automatically shrinks to fit all runners on screen. The followed runner is highlighted. Layout and visibility are saved automatically.");
                            ui.separator();ui.text_wrapped(&s.timing_status);
                        }
                        5 => {
                            heading(ui,"Broadcast graphics","Race context and runner cards, visible during clean capture.");
                            if ui.button("Show coverage graphics") {
                                s.tower=true;
                                s.settings.broadcast.spotlight=true;
                                s.settings.broadcast.focus_markers=true;
                                s.settings.broadcast.battle_card=true;
                                s.settings.broadcast.progress=true;
                                s.settings.broadcast.results=true;
                            }
                            if ui.button("Hide coverage graphics") {
                                s.tower=false;
                                s.settings.broadcast.spotlight=false;
                                s.settings.broadcast.focus_markers=false;
                                s.settings.broadcast.battle_card=false;
                                s.settings.broadcast.progress=false;
                                s.settings.broadcast.results=false;
                            }
                            option(ui,"Runner focus markers","Labels above the focused runner and battle rival. Visible in clean capture; hidden in Freecam.",&mut s.settings.broadcast.focus_markers);
                            if s.settings.broadcast.focus_markers { inspector_slider(ui,"Marker height above ground",1.4,3.5,&mut s.settings.broadcast.marker_height); }
                            option(ui,"Runner spotlight","Name, trainer, position and speed for the followed runner.",&mut s.settings.broadcast.spotlight);
                            option(ui,"Battle card","Both contenders, distance gap and relative closing speed.",&mut s.settings.broadcast.battle_card);
                            option(ui,"Race progress","Distance remaining from game metadata or your manual override.",&mut s.settings.broadcast.progress);
                            option(ui,"Finish and results","Winner and finish order appear only as runners finish.",&mut s.settings.broadcast.results);
                            ui.text_wrapped("Cards sit opposite the tower. Freecam has no followed-runner card. F10 hides all coverage graphics.");
                            ui.separator();
                            crate::design::section(ui,"EVENT DETAILS");
                            ui.text("Event / round");
                            ui.input_text("##eventname",&mut s.settings.broadcast.event_name).hint("Optional event title").build();
                            ui.text("Course");
                            ui.input_text("##coursename",&mut s.settings.broadcast.course_name).hint("Optional course name").build();
                            ui.text("Race length override (0 = Auto)");
                            ui.input_float("##racedistance",&mut s.settings.broadcast.race_distance_m).step(100.).build();
                            ui.text_wrapped("Event labels and length overrides are saved. Leave length at 0 to use game metadata when available; otherwise covered metres are shown. Review overrides for each race. Trainer labels come from Tower > Trainers.");
                            ui.separator();
                            crate::design::section(ui,"APPEARANCE");
                            inspector_slider(ui,"Graphics size",0.6,1.5,&mut s.settings.broadcast.scale);
                            inspector_slider(ui,"Background opacity",0.2,1.,&mut s.settings.broadcast.opacity);
                            ui.text_wrapped(&s.timing_status);
                        }
                        _ => {
                            heading(ui, "Help & status", "Controls stay available with the panel closed.");
                            for line in ["F8 / Clean capture lock (F8 or Escape releases)", "F9 / Stop director or toggle battle camera", "WASD + Q/E / Freecam flight", "Shift / Freecam boost; Ctrl / slow", "Right-drag  /  Orbit and pitch", "Mouse wheel  /  Zoom", "Arrow keys  /  Orbit and pitch", "Page Up / Down  /  Height", "[ and ]  /  Previous or next runner", "Shift  /  Fine keyboard movement", "F10  /  Restore camera and all visual hiding"] { ui.text(line); }
                            if ui.button("Save settings now") {self.message = match save(&s.snapshot()) {Ok(()) => "Settings saved".into(),Err(e) => format!("Save failed: {e}")};}
                            ui.separator(); ui.text_wrapped(&s.status);
                            ui.text_wrapped(&s.visual_status); ui.text_wrapped(&s.fps_status);
                            ui.text_wrapped("Settings and enabled features are saved automatically every second when changed. F8 cursor capture remains manual. F10 restores visuals and remembers that choice.");
                        }
                    }
                });
                ui.separator();
                let footer_width = (ui.content_region_avail()[0] - 10.) * 0.5;
                if ui.button_with_size("Capture  [F8]", [footer_width,34.]) {
                    if s.camera && s.race && crate::capture::available() { s.capture = true; self.open = false; }
                    else { self.message = "Enable camera during a race to start capture.".into(); }
                }
                ui.same_line();
                if ui.button_with_size("Restore  [F10]", [footer_width,34.]) {
                    s.restore_visuals();
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
            k.sample([true, false, false, false, false, false], true),
            [true, false, false, false, false, false]
        );
        assert_eq!(
            k.sample([true, false, false, false, false, false], true),
            [false; 6]
        );
        k.sample([false; 6], true);
        assert!(k.sample([true, false, false, false, false, false], true)[0]);
    }
    #[test]
    fn unfocused_press_is_not_replayed() {
        let mut k = ToggleKeys::default();
        assert_eq!(
            k.sample([false, true, false, false, false, false], false),
            [false; 6]
        );
        assert_eq!(
            k.sample([false, true, false, false, false, false], true),
            [false; 6]
        );
    }
}

fn heading(ui: &Ui, title: &str, subtitle: &str) {
    crate::design::heading(ui, title, subtitle);
}
fn option(ui: &Ui, title: &str, description: &str, enabled: &mut bool) {
    crate::design::toggle(ui, title, description, enabled);
}

fn inspector_slider(ui: &Ui, label: &str, min: f32, max: f32, value: &mut f32) {
    let _id = ui.push_id(label);
    let start = ui.cursor_pos()[0];
    let width = ui.content_region_avail()[0];
    let label_width = if width > 420. { width * 0.42 } else { 0. };
    ui.text(label);
    if label_width > 0. {
        ui.same_line_with_pos(start + label_width);
    }
    let slider_width = (width - label_width - 65.).max(70.);
    let _width = ui.push_item_width(slider_width);
    let p = ui.cursor_screen_pos();
    let track = ui.style_color(StyleColor::Border);
    let accent = ui.style_color(StyleColor::SliderGrab);
    {
        let _padding = ui.push_style_var(hudhook::imgui::StyleVar::FramePadding([4., 1.]));
        let _a = ui.push_style_color(StyleColor::FrameBg, [0.; 4]);
        let _b = ui.push_style_color(StyleColor::FrameBgHovered, [0.; 4]);
        let _c = ui.push_style_color(StyleColor::FrameBgActive, [0.; 4]);
        let _d = ui.push_style_color(StyleColor::SliderGrab, [0.; 4]);
        let _e = ui.push_style_color(StyleColor::SliderGrabActive, [0.; 4]);
        ui.slider_config("##value", min, max)
            .display_format("")
            .build(value);
    }
    let y = (ui.item_rect_min()[1] + ui.item_rect_max()[1]) * 0.5;
    let left = p[0] + 7.;
    let right = p[0] + slider_width - 7.;
    let position = left + ((*value - min) / (max - min)).clamp(0., 1.) * (right - left);
    let d = ui.get_window_draw_list();
    d.add_line([left, y], [right, y], track)
        .thickness(3.)
        .build();
    d.add_line([left, y], [position, y], accent)
        .thickness(3.)
        .build();
    d.add_circle([position, y], 5.5, accent)
        .filled(true)
        .build();
    if ui.is_item_focused() {
        d.add_circle([position, y], 8., accent).build();
    }
    let value_text = if max <= 0.02 {
        format!("{value:.3}")
    } else {
        format!("{value:.1}")
    };
    ui.same_line_with_pos(start + width - ui.calc_text_size(&value_text)[0]);
    ui.text_disabled(value_text);
}

#[cfg(test)]
pub(crate) fn draw_preview(ui: &Ui, dark: bool, rows: &[crate::timing::Runner]) {
    let mut settings = crate::model::Settings::default();
    settings.dark_mode = dark;
    settings.tower.right = true;
    settings.tower.show_trainers = true;
    for r in rows {
        settings.tower.trainer_names.insert(
            r.gate,
            if r.gate == 3 {
                "MaybeVoid".into()
            } else {
                format!("Trainer {}", r.gate)
            },
        );
    }
    let mut s = crate::state::State::from_settings(settings);
    s.race = true;
    s.camera = true;
    s.gate = 3;
    s.gates = rows.iter().map(|r| r.gate).collect();
    s.timing_rows = rows.to_vec();
    s.tower = true;
    let mut panel = Panel {
        keys: ToggleKeys::default(),
        autosave: crate::state::AutoSave::new(),
        open: true,
        filter: String::new(),
        shot_name: String::new(),
        message: "Settings saved automatically".into(),
        page: 0,
        tower_motion: crate::timing::TowerMotion::default(),
        timing_generation: 0,
    };
    let scene = std::env::var("RACEDIRECTOR_PREVIEW_SCENE").unwrap_or_default();
    if !scene.is_empty() {
        s.settings.broadcast.spotlight = true;
        s.settings.broadcast.progress = true;
        s.settings.broadcast.event_name = "Championship / Final".into();
        s.settings.broadcast.course_name = "Tokyo / Turf".into();
        s.settings.broadcast.race_distance_m = 2000.;
        if scene == "battle" {
            s.settings.battle.enabled = true;
            s.focus_markers = vec![
                crate::markers::Marker {
                    gate: 3,
                    point: [0.40, 0.45],
                },
                crate::markers::Marker {
                    gate: 2,
                    point: [0.49, 0.46],
                },
            ];
            s.settings.broadcast.battle_card = true;
            s.battle_info = Some(crate::battle::BattleInfo {
                anchor: 3,
                partner: 2,
                position: Some(2),
                gap_m: Some(1.7),
                closing_mps: Some(0.4),
            });
        }
        if scene == "results" {
            s.settings.broadcast.results = true;
            s.finished_gates = rows.iter().map(|r| r.gate).collect();
        }
    }
    panel.draw_coverage(ui, &s);
    if scene.is_empty() || scene == "settings" || scene == "director" {
        if scene == "settings" {
            panel.page = 5;
        }
        if scene == "director" {
            s.settings.director.enabled = true;
            s.settings.battle.finish_coverage = true;
            s.settings.director.finish_camera = Some(crate::director::FixedCamera {
                eye: crate::model::Vec3 {
                    x: 120.,
                    y: 4.,
                    z: 60.,
                },
                fov: 30.,
            });
            s.finish_camera_armed = true;
            s.director_status = "Ready / tour starts at 25%".into();
        }
        panel.draw_settings(ui, &mut s);
    }
}

fn runner_label(s: &crate::state::State, gate: i32) -> String {
    s.timing_rows
        .iter()
        .find(|r| r.gate == gate)
        .map(|r| format!("Gate {gate:02}  {}", r.name))
        .unwrap_or_else(|| format!("Gate {gate:02}"))
}
