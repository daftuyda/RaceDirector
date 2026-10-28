//! Unity operations are confined to game-thread callbacks. The camera callback
//! continues during pause; runner data is read directly, not from motion events.
use crate::hooks::NativeHook;
use crate::{
    il2cpp as il,
    model::{orbit, pack_center, path_matches, FreePose, Pose, Vec3},
    state::{state, status},
};
use std::{
    ffi::c_void,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex, OnceLock,
    },
    time::{Duration, Instant},
};
type Obj = *mut c_void;
static LATE: AtomicUsize = AtomicUsize::new(0);
static BACK: AtomicUsize = AtomicUsize::new(0);
static BACK_MOUSE_OFFSET: AtomicUsize = AtomicUsize::new(0);
static D_BACK: OnceLock<NativeHook> = OnceLock::new();
static CANVAS: AtomicUsize = AtomicUsize::new(0);
static D_LATE: OnceLock<NativeHook> = OnceLock::new();
static D_CANVAS: OnceLock<NativeHook> = OnceLock::new();
#[derive(Clone, Copy)]
struct Horse {
    gate: i32,
    pos: Vec3,
    heading: f32,
}
struct Live {
    pose: Pose,
    target_tween: crate::camera_motion::TargetTween,
    battle: crate::battle::BattleTracker,
    battle_generation: u64,
    director: crate::director::Controller,
    director_log: String,
    fixed: crate::director::FixedTracker,
    auto_finish: crate::director::AutoFinish,
    ground: GroundCache,
    free_pose: Option<FreePose>,
    last: Instant,
    race_seen: Option<Instant>,
    timing_last: Option<Instant>,
    timing_bindings: Option<(usize, TimingApi)>,
    course_bindings: Option<(usize, CourseApi)>,
    manager: Option<u32>,
    restore: Vec<(u32, f32)>,
    world_restore: Vec<(u32, bool)>,
    trees: Vec<u32>,
    tree_scan: Option<Instant>,
    camera_restore: Option<(u32, f32, f32, Vec3, [f32; 4])>,
}
fn live() -> &'static Mutex<Live> {
    static L: OnceLock<Mutex<Live>> = OnceLock::new();
    L.get_or_init(|| {
        Mutex::new(Live {
            pose: Pose::default(),
            target_tween: crate::camera_motion::TargetTween::default(),
            battle: crate::battle::BattleTracker::default(),
            battle_generation: 0,
            director: crate::director::Controller::default(),
            director_log: String::new(),
            fixed: crate::director::FixedTracker::default(),
            auto_finish: crate::director::AutoFinish::default(),
            ground: GroundCache::default(),
            free_pose: None,
            last: Instant::now(),
            race_seen: None,
            timing_last: None,
            timing_bindings: None,
            course_bindings: None,
            manager: None,
            restore: vec![],
            world_restore: vec![],
            trees: vec![],
            tree_scan: None,
            camera_restore: None,
        })
    })
}
struct Api {
    pos: usize,
    rot: usize,
    horse_class: usize,
    horse_data: usize,
    trainer_name: usize,
    gate: usize,
    find: usize,
    manager_type: usize,
    manager_horses: usize,
    current_camera: usize,
    name: usize,
    transform: usize,
    parent: usize,
    renderer_type: usize,
    color_get: usize,
    alpha_set: usize,
    aspect_get: usize,
    fov_get: usize,
    fov_set: usize,
    near_get: usize,
    near_set: usize,
    mesh_type: usize,
    world_renderer_type: usize,
    particle_type: usize,
    renderer_enabled_get: usize,
    renderer_enabled_set: usize,
    alive: usize,
    getpos: usize,
    getrot: usize,
    setpos: usize,
    setrot: usize,
    gc_new: usize,
    gc_get: usize,
    gc_free: usize,
}
static API: OnceLock<Api> = OnceLock::new();
fn method(c: &str, n: &str, a: i32) -> usize {
    il::method(il::class(c), n, a) as usize
}
unsafe fn invoke(m: usize, o: Obj, args: &mut [Obj]) -> Obj {
    if m == 0 {
        return std::ptr::null_mut();
    }
    il::runtime_invoke(m as il::Method, o, args)
}
unsafe fn get(m: usize, o: Obj) -> Obj {
    invoke(m, o, &mut [])
}
unsafe fn scalar<T: Copy>(m: usize, o: Obj) -> Option<T> {
    let b = get(m, o);
    if b.is_null() {
        None
    } else {
        Some((b.cast::<u8>().add(16) as *const T).read_unaligned())
    }
}
unsafe fn set_float(m: usize, o: Obj, mut f: f32) {
    invoke(m, o, &mut [&mut f as *mut f32 as Obj]);
}
unsafe fn array(arr: Obj, limit: usize) -> Vec<Obj> {
    if arr.is_null() {
        return vec![];
    }
    let p = arr.cast::<u8>();
    let len = (p.add(24) as *const usize).read_unaligned();
    if len > limit {
        return vec![];
    }
    (0..len)
        .map(|i| (p.add(32 + i * 8) as *const Obj).read_unaligned())
        .filter(|o| !o.is_null())
        .collect()
}
unsafe fn objects(typ: usize) -> Vec<Obj> {
    if typ == 0 {
        return vec![];
    }
    array(
        invoke(
            API.get().unwrap().find,
            std::ptr::null_mut(),
            &mut [typ as Obj],
        ),
        100_000,
    )
}
unsafe fn name(o: Obj) -> String {
    il::read_string(get(API.get().unwrap().name, o))
}
unsafe fn valid(o: Obj) -> bool {
    if o.is_null() {
        return false;
    }
    let b = invoke(API.get().unwrap().alive, std::ptr::null_mut(), &mut [o]);
    !b.is_null() && (b.cast::<u8>().add(16) as *const bool).read_unaligned()
}
unsafe fn hold(o: Obj) -> u32 {
    let f: unsafe extern "C" fn(Obj, bool) -> u32 = std::mem::transmute(API.get().unwrap().gc_new);
    f(o, false)
}
unsafe fn target(h: u32) -> Obj {
    let f: unsafe extern "C" fn(u32) -> Obj = std::mem::transmute(API.get().unwrap().gc_get);
    f(h)
}
unsafe fn release(h: u32) -> Obj {
    let o = target(h);
    let f: unsafe extern "C" fn(u32) = std::mem::transmute(API.get().unwrap().gc_free);
    f(h);
    o
}
unsafe fn path(o: Obj) -> String {
    let a = API.get().unwrap();
    let mut t = get(a.transform, o);
    let mut parts = vec![];
    for _ in 0..48 {
        if t.is_null() {
            break;
        }
        parts.push(name(t));
        t = get(a.parent, t);
    }
    parts.reverse();
    parts.join("/")
}
unsafe fn restore_ui(l: &mut Live) {
    for (h, v) in l.restore.drain(..) {
        let o = release(h);
        if valid(o) {
            set_float(API.get().unwrap().alpha_set, o, v);
        }
    }
}
unsafe fn restore_camera(l: &mut Live) {
    if let Some((h, v, near, mut p, mut q)) = l.camera_restore.take() {
        let o = release(h);
        if valid(o) {
            let a = API.get().unwrap();
            set_float(a.fov_set, o, v);
            set_float(a.near_set, o, near);
            let tf = get(a.transform, o);
            if !tf.is_null() {
                let sp: unsafe extern "C" fn(Obj, *mut Vec3) = std::mem::transmute(a.setpos);
                sp(tf, &mut p);
                let sr: unsafe extern "C" fn(Obj, *mut [f32; 4]) = std::mem::transmute(a.setrot);
                sr(tf, &mut q);
            }
        }
    }
}
unsafe fn horses(l: &mut Live) -> Vec<Horse> {
    let a = API.get().unwrap();
    let mut mgr = l.manager.map(|h| target(h)).unwrap_or(std::ptr::null_mut());
    if !valid(mgr) {
        // A new manager can appear without an intervening empty race frame.
        // Invalidate before the telemetry throttle to avoid cross-race overlays.
        l.timing_last = None;
        l.timing_bindings = None;
        l.course_bindings = None;
        l.battle.reset();
        l.target_tween.reset();
        if let Ok(mut s) = state().lock() {
            s.clear_telemetry();
        }
        if let Some(h) = l.manager.take() {
            release(h);
        }
        mgr = objects(a.manager_type)
            .into_iter()
            .next()
            .unwrap_or(std::ptr::null_mut());
        if !mgr.is_null() {
            l.manager = Some(hold(mgr));
        }
    }
    if mgr.is_null() {
        return vec![];
    }
    array(get(a.manager_horses, mgr), 64)
        .into_iter()
        .filter_map(|o| {
            if !il::object_is(o, a.horse_class as il::Class) {
                return None;
            }
            let data = get(a.horse_data, o);
            if data.is_null() {
                return None;
            }
            let gate = scalar::<i32>(a.gate, data)?;
            if !(1..=64).contains(&gate) {
                return None;
            }
            let p = (o.cast::<u8>().add(a.pos) as *const Vec3).read_unaligned();
            let q = (o.cast::<u8>().add(a.rot) as *const [f32; 4]).read_unaligned();
            if ![p.x, p.y, p.z].iter().all(|v| v.is_finite()) || !q.iter().all(|v| v.is_finite()) {
                return None;
            }
            Some(Horse {
                gate,
                pos: p,
                heading: (2. * (q[0] * q[2] + q[3] * q[1]))
                    .atan2(1. - 2. * (q[0] * q[0] + q[1] * q[1])),
            })
        })
        .collect()
}
unsafe extern "C" fn on_late(this: Obj, mi: Obj) {
    restore_overhead();
    if let Ok(mut l) = live().lock() {
        restore_camera(&mut l);
    }
    let f: unsafe extern "C" fn(Obj, Obj) = std::mem::transmute(LATE.load(Ordering::Acquire));
    f(this, mi);
    mask_overhead(this);
    crate::graphics::tick(this);
    let a = API.get().unwrap();
    let Ok(mut l) = live().lock() else { return };
    let now = Instant::now();
    let hs = horses(&mut l);
    let cam = get(a.current_camera, this);
    let racing = !hs.is_empty() && valid(cam);
    let (enabled, pose, response) = {
        let Ok(mut s) = state().lock() else { return };
        s.race = racing;
        if !racing || !s.camera {
            s.battle_info = None;
            s.coverage_gate = None;
            s.focus_markers.clear();
            s.free_pose = None;
            s.capture = false;
            s.director_status = if !racing {
                "Waiting for a race"
            } else {
                "Camera off / enable Camera control [F6]"
            }
            .into();
        }
        s.gates = hs.iter().map(|h| h.gate).collect();
        s.gates.sort();
        s.gates.dedup();
        if !s.gates.contains(&s.gate) {
            if let Some(g) = s.gates.first() {
                s.gate = *g;
            }
        }
        (s.camera, s.settings.pose, s.settings.response)
    };
    update_timing(&mut l, racing);
    if !racing {
        l.director.reset();
        l.fixed.reset();
        l.auto_finish.reset();
        l.race_seen = None;
        l.battle.reset();
        l.target_tween.reset();
        clear_ground(&mut l.ground);
        l.free_pose = None;
        return;
    }
    l.race_seen = Some(now);
    let dt = now.duration_since(l.last).as_secs_f32();
    l.last = now;
    if !enabled {
        l.director.reset();
        l.fixed.reset();
        l.auto_finish.reset();
        l.battle.reset();
        l.pose = pose;
        l.target_tween.reset();
        l.free_pose = None;
        return;
    }
    let coverage = {
        let Ok(mut s) = state().lock() else { return };
        if s.timing_generation != l.battle_generation {
            l.director.reset();
            l.fixed.reset();
            l.auto_finish.reset();
            l.battle.reset();
            l.target_tween.reset();
            l.battle_generation = s.timing_generation;
        }
        l.director
            .update(&mut s, &hs.iter().map(|h| h.gate).collect::<Vec<_>>(), dt)
    };
    let crate::director::Coverage {
        gate,
        pose,
        finish: finish_selection,
        touring: tour_active,
        pairing,
        transition,
    } = coverage;
    let Some(h) = hs.iter().find(|h| h.gate == gate) else {
        return;
    };
    if let Ok(s) = state().lock() {
        if let Some(leader) = s.timing_rows.iter().find(|r| r.rank == Some(1)) {
            if let (Some(progress), Some(horse)) =
                (leader.distance, hs.iter().find(|h| h.gate == leader.gate))
            {
                l.auto_finish.observe(leader.gate, progress, horse.pos);
            }
        }
    }
    update_ground(&mut l);
    let aspect = scalar::<f32>(a.aspect_get, cam).filter(|v| v.is_finite() && *v > 0.1 && *v < 10.);
    let pack = pack_center(hs.iter().map(|h| h.pos)).unwrap();
    let mut battle_points = None;
    let (mut eye, mut q, mut fov, mut aim, clearance) = {
        let Ok(mut s) = state().lock() else { return };
        let clearance = s.settings.ground_clearance;
        if s.timing_generation != l.battle_generation {
            l.battle.reset();
            l.target_tween.reset();
            l.battle_generation = s.timing_generation;
        }
        if s.freecam {
            s.battle_info = None;
            l.battle.reset();
            l.pose.approach(pose, dt, response);
            l.target_tween.reset();
            if s.free_pose.is_none() {
                l.free_pose = None;
            }
            let free = s
                .free_pose
                .get_or_insert_with(|| FreePose::from_orbit(l.pose, h.pos, h.heading, pack));
            free.sanitize();
            let mut wanted = free.camera(pack).0;
            let floor = ground_at(&l.ground.surface, wanted, &hs);
            if crate::camera_motion::clamp_height(&mut wanted, floor, clearance) {
                free.offset.y = wanted.y - pack.y;
            }
            // Clamp the movement target as well as the smoothed pose: downward
            // input cannot accumulate an invisible offset beneath the ground.
            let displayed = l.free_pose.get_or_insert(*free);
            displayed.approach(*free, dt, response);
            let (eye, q) = displayed.camera(pack);
            (eye, q, displayed.fov, None, clearance)
        } else {
            s.free_pose = None;
            l.free_pose = None;
            // Keep both runners in one view. The gate-pair key gives the existing
            // target tween a distinct identity when battle mode enters or changes pair.
            let info = if let Some(selection) = finish_selection {
                l.battle.reset();
                selection.info(&s.timing_rows).filter(|_| aspect.is_some())
            } else if aspect.is_some() && pairing.enabled {
                let contenders = s
                    .timing_rows
                    .iter()
                    .filter(|r| !s.finished_gates.contains(&r.gate))
                    .cloned()
                    .collect::<Vec<_>>();
                l.battle.update(&contenders, gate, &pairing, dt.min(0.1))
            } else {
                l.battle.reset();
                None
            };
            if s.settings.director.enabled && finish_selection.is_none() {
                if let Some(info) = info {
                    let segment = if tour_active {
                        "Runner tour + battle"
                    } else {
                        "Battle"
                    };
                    s.director_status =
                        format!("{segment} / gates {} + {}", info.anchor, info.partner);
                }
            }
            let pair = info.and_then(|info| {
                hs.iter()
                    .find(|p| p.gate == info.partner)
                    .map(|p| (info, *p))
            });
            let (wanted, target, target_key, target_heading) = if let Some((info, other)) = pair {
                battle_points = Some((h.pos, other.pos));
                let heading = crate::director::pair_heading(h.heading, other.heading);
                let (wanted, target) =
                    crate::battle::frame(pose, h.pos, other.pos, heading, aspect.unwrap());
                let key = finish_selection
                    .map_or(crate::director::pair_key(gate, info.partner), |x| x.key());
                s.battle_info = Some(info);
                (wanted, target, key, heading)
            } else {
                s.battle_info = None;
                (
                    pose,
                    h.pos,
                    finish_selection.map_or(gate, |x| x.key()),
                    h.heading,
                )
            };
            let response = if s.settings.director.enabled {
                response.min(1.5)
            } else {
                response
            };
            l.pose.approach(wanted, dt, response);
            let (anchor, heading) =
                l.target_tween
                    .follow(target_key, target, target_heading, pack, dt, transition);
            // Once a shot has arrived, protect pair framing. During automatic
            // travel, retain the smooth rig instead of abruptly pulling back.
            let displayed = if s.settings.director.enabled && !l.target_tween.settled() {
                // Let the shot travel to its new subject before enforcing pair
                // framing. Immediate fitting at the old anchor caused zoom jumps.
                l.pose
            } else if let Some((_, other)) = pair {
                crate::battle::fit_at(l.pose, h.pos, other.pos, anchor, heading, aspect.unwrap())
            } else {
                l.pose
            };
            let (eye, q) = orbit(displayed, anchor, heading);
            (
                eye,
                q,
                displayed.fov,
                Some(Vec3 {
                    x: anchor.x,
                    y: anchor.y + displayed.height,
                    z: anchor.z,
                }),
                clearance,
            )
        }
    };
    {
        let Ok(mut s) = state().lock() else { return };
        let saved = s
            .settings
            .director
            .finish_camera
            .filter(|_| s.finish_camera_armed);
        let fixed = if finish_selection.is_some() && s.settings.director.static_finish {
            if let Some(camera) = saved {
                Some(camera)
            } else if s.settings.director.enabled && s.settings.director.auto_finish {
                let remaining = s
                    .timing_rows
                    .iter()
                    .find(|r| r.gate == gate)
                    .and_then(|r| r.distance)
                    .map(|d| s.race_distance() - d);
                remaining.and_then(|remaining| {
                    l.auto_finish.place(
                        h.pos,
                        remaining,
                        s.course_world_scale,
                        s.settings.director.finish_right_side,
                    )
                })
            } else {
                None
            }
        } else {
            None
        };
        if finish_selection.is_none() {
            l.auto_finish.clear_camera();
        }
        if let (Some(camera), Some(target)) = (fixed, aim) {
            let (position, tracking) = l.fixed.view(eye, camera, target, dt, transition);
            eye = position;
            q = crate::camera_motion::look_at(eye, tracking);
            aim = Some(tracking);
            let (first, second) = battle_points.unwrap_or((h.pos, h.pos));
            fov = l.fixed.lens(
                eye,
                tracking,
                first,
                second,
                camera.fov,
                aspect.unwrap_or(16. / 9.),
                dt,
            );
            s.director_status = if saved.is_some() {
                "Finish / saved fixed camera"
            } else {
                "Finish / estimated fixed camera"
            }
            .into();
        } else {
            l.fixed.reset();
            if finish_selection.is_some() && s.settings.director.static_finish {
                let reason = if s.settings.director.enabled && s.settings.director.auto_finish {
                    " / fixed shot waits for last 120 m and movement samples"
                } else {
                    " / no saved position armed"
                };
                s.director_status.push_str(reason);
            }
        }
    }
    let floor = ground_at(&l.ground.surface, eye, &hs);
    if crate::camera_motion::clamp_height(&mut eye, floor, clearance) {
        if let Some(aim) = aim {
            q = crate::camera_motion::look_at(eye, aim);
            if let Some((first, second)) = battle_points {
                // Raising the eye changes projection; refit the lens at the
                // final ground-safe position, not the old below-ground orbit.
                fov = crate::battle::grounded_fov(eye, aim, first, second, fov, aspect.unwrap());
            }
        }
        if let Some(displayed) = &mut l.free_pose {
            displayed.offset.y = eye.y - pack.y;
        }
    }
    if let Ok(mut s) = state().lock() {
        if s.save_finish_camera {
            if s.freecam {
                s.settings.director.finish_camera = Some(crate::director::FixedCamera { eye, fov });
                s.finish_camera_armed = true;
                s.director_status = "Finish camera saved and armed for this race".into();
            }
            s.save_finish_camera = false;
        }
    }
    let tf = get(a.transform, cam);
    if tf.is_null() {
        return;
    }
    // Capture every overridden projection value before touching the camera.
    let (Some(old), Some(near)) = (
        scalar::<f32>(a.fov_get, cam),
        scalar::<f32>(a.near_get, cam),
    ) else {
        return;
    };
    if !near.is_finite() || near <= 0. {
        return;
    }
    {
        let mut native_pos = Vec3::default();
        let mut native_rot = [0.; 4];
        let gp: unsafe extern "C" fn(Obj, *mut Vec3) = std::mem::transmute(a.getpos);
        gp(tf, &mut native_pos);
        let gr: unsafe extern "C" fn(Obj, *mut [f32; 4]) = std::mem::transmute(a.getrot);
        gr(tf, &mut native_rot);
        l.camera_restore = Some((hold(cam), old, near, native_pos, native_rot));
        set_float(a.fov_set, cam, fov);
        // Keep an already smaller game near plane; never use zero.
        set_float(a.near_set, cam, near.min(0.05));
    }
    let pos: unsafe extern "C" fn(Obj, *mut Vec3) = std::mem::transmute(a.setpos);
    pos(tf, &mut eye);
    let rot: unsafe extern "C" fn(Obj, *mut [f32; 4]) = std::mem::transmute(a.setrot);
    rot(tf, &mut q);
    if let Ok(mut s) = state().lock() {
        s.focus_markers.clear();
        if s.settings.broadcast.focus_markers && !s.freecam {
            if let Some(aspect) = aspect {
                let rival = s.battle_info.as_ref().map(|b| b.partner);
                s.focus_markers = hs
                    .iter()
                    .filter(|h| h.gate == gate || Some(h.gate) == rival)
                    .filter_map(|h| {
                        crate::markers::project(
                            h.gate,
                            Vec3 {
                                y: h.pos.y + s.settings.broadcast.marker_height,
                                ..h.pos
                            },
                            eye,
                            q,
                            fov,
                            aspect,
                        )
                    })
                    .collect();
            }
        }
    }
    if let Ok(s) = state().lock() {
        if l.director_log != s.director_status {
            l.director_log = s.director_status.clone();
            crate::state::log(&format!("Director: {} / gate {} / length {:.0} m / camera ({:.1}, {:.1}, {:.1}) / fov {:.1}",
                s.director_status, gate, s.race_distance(), eye.x, eye.y, eye.z, fov));
        }
    }
}

unsafe fn set_renderer_enabled(o: Obj, mut value: bool) {
    invoke(
        API.get().unwrap().renderer_enabled_set,
        o,
        &mut [&mut value as *mut bool as Obj],
    );
}
unsafe fn restore_world(l: &mut Live) {
    for (h, old) in l.world_restore.drain(..) {
        let o = release(h);
        if valid(o) {
            set_renderer_enabled(o, old);
        }
    }
}
unsafe fn hide_renderer(l: &mut Live, o: Obj) {
    if let Some(old) = scalar::<bool>(API.get().unwrap().renderer_enabled_get, o) {
        if old {
            l.world_restore.push((hold(o), old));
            set_renderer_enabled(o, false);
        }
    }
}
unsafe fn mask_world(
    l: &mut Live,
    trees: bool,
    effects: bool,
    popups: bool,
    markers: bool,
    speed_lines: bool,
) {
    let a = API.get().unwrap();
    if a.renderer_enabled_get == 0 || a.renderer_enabled_set == 0 {
        return;
    }
    if !trees {
        for h in l.trees.drain(..) {
            release(h);
        }
        l.tree_scan = None;
    } else {
        // Tree meshes are static. Avoid walking the entire course every frame.
        if l.tree_scan
            .is_none_or(|t| t.elapsed() > Duration::from_secs(2))
        {
            for h in l.trees.drain(..) {
                release(h);
            }
            l.trees = objects(a.mesh_type)
                .into_iter()
                .filter(|o| crate::visual::tree(&path(*o)))
                .map(|o| hold(o))
                .collect();
            l.tree_scan = Some(Instant::now());
        }
        for h in l.trees.clone() {
            let o = target(h);
            if valid(o) {
                hide_renderer(l, o);
            }
        }
    }
    if popups || markers {
        for o in objects(a.world_renderer_type) {
            let p = path(o);
            if (popups && crate::visual::skill_popup(&p))
                || (markers && crate::visual::runner_marker(&p))
            {
                hide_renderer(l, o);
            }
        }
    }
    if effects || speed_lines {
        // Skill particles are pooled and can appear on any frame.
        for o in objects(a.particle_type) {
            let p = path(o);
            if (effects && crate::visual::skill_glow(&p))
                || (speed_lines && crate::visual::speed_line(&p))
            {
                hide_renderer(l, o);
            }
        }
    }
}

static POST: AtomicUsize = AtomicUsize::new(0);
static D_POST: OnceLock<NativeHook> = OnceLock::new();
struct PostApi {
    race_class: usize,
    dof_get: usize,
    dof_mode: usize,
    radial_get: usize,
    radial_mode: usize,
}
static POST_API: OnceLock<PostApi> = OnceLock::new();
// Restore even on an early return or nested render invocation.
struct ScopedMode {
    address: *mut i32,
    old: i32,
}
impl ScopedMode {
    unsafe fn disable(address: *mut i32) -> Self {
        let old = address.read_unaligned();
        address.write_unaligned(0);
        Self { address, old }
    }
}
impl Drop for ScopedMode {
    fn drop(&mut self) {
        unsafe {
            self.address.write_unaligned(self.old);
        }
    }
}
unsafe fn disable_mode(getter: usize, offset: usize, this: Obj) -> Option<ScopedMode> {
    let param = get(getter, this);
    if param.is_null() {
        None
    } else {
        Some(ScopedMode::disable(
            param.cast::<u8>().add(offset).cast::<i32>(),
        ))
    }
}
unsafe extern "C" fn on_post(this: Obj, source: Obj, current: Obj, depth: Obj, mi: Obj) -> Obj {
    let f: unsafe extern "C" fn(Obj, Obj, Obj, Obj, Obj) -> Obj =
        std::mem::transmute(POST.load(Ordering::Acquire));
    let (effects, speed_lines) = state()
        .lock()
        .map(|s| (s.race && s.clean_effects, s.race && s.hide_speed_lines))
        .unwrap_or_default();
    let Some(a) = POST_API.get() else {
        return f(this, source, current, depth, mi);
    };
    if !(effects || speed_lines) || !il::object_is(this, a.race_class as il::Class) {
        return f(this, source, current, depth, mi);
    }
    // The dispatcher inlines the enable checks. Disable its actual mode fields
    // only for this race pass, preserving native texture allocation/compositing.
    let _dof = if effects {
        disable_mode(a.dof_get, a.dof_mode, this)
    } else {
        None
    };
    let _radial = disable_mode(a.radial_get, a.radial_mode, this);
    f(this, source, current, depth, mi)
}
unsafe fn install_post_hook() -> Result<(), String> {
    let a = PostApi {
        race_class: il::class("Gallop.RaceImageEffect") as usize,
        dof_get: method(
            "Gallop.GallopImageEffect",
            "get_DofDiffusionBloomOverlayParam",
            0,
        ),
        dof_mode: il::field_offset(
            il::class("Gallop.ImageEffect.DofDiffusionBloomOverlayParam"),
            "_useDofDiffusionBloomType",
        )
        .ok_or("Bloom mode field unavailable")?,
        radial_get: method("Gallop.GallopImageEffect", "get_RadialBlurParam", 0),
        radial_mode: il::field_offset(il::class("Gallop.ImageEffect.RadialBlurParam"), "Type")
            .ok_or("Radial mode field unavailable")?,
    };
    if a.race_class == 0 || a.dof_get == 0 || a.radial_get == 0 {
        return Err("Race post-processing parameter bindings unavailable".into());
    }
    let hook = prepare_args(
        "Gallop.GallopImageEffect",
        "OnRenderImageEffect",
        3,
        on_post as *const (),
        &POST,
    )?;
    let _ = POST_API.set(a);
    hook.enable()?;
    let _ = D_POST.set(hook);
    Ok(())
}

#[derive(Clone, Copy)]
struct PopupApi {
    visible: usize,
    set_visible: usize,
}
static POPUP_API: OnceLock<PopupApi> = OnceLock::new();
static POPUP_LATE: AtomicUsize = AtomicUsize::new(0);
static D_POPUP: OnceLock<NativeHook> = OnceLock::new();
static SIDEBAR_API: OnceLock<PopupApi> = OnceLock::new();
static SIDEBAR_UPDATE: AtomicUsize = AtomicUsize::new(0);
static D_SIDEBAR: OnceLock<NativeHook> = OnceLock::new();
static POPUP_RESTORE: Mutex<Vec<(u32, bool, PopupApi)>> = Mutex::new(Vec::new());
unsafe fn popup_visibility(a: &PopupApi, obj: Obj, mut visible: bool) {
    invoke(a.set_visible, obj, &mut [&mut visible as *mut bool as Obj]);
}
unsafe fn restore_popups() {
    if let Ok(mut saved) = POPUP_RESTORE.lock() {
        for (h, visible, a) in saved.drain(..) {
            let obj = release(h);
            if !obj.is_null() {
                popup_visibility(&a, obj, visible);
            }
        }
    }
}
unsafe extern "C" fn on_popup_late(this: Obj, dt: f32, mi: Obj) {
    let f: unsafe extern "C" fn(Obj, f32, Obj) =
        std::mem::transmute(POPUP_LATE.load(Ordering::Acquire));
    // Let timers and plate pooling advance normally, including while hidden.
    f(this, dt, mi);
    enforce_popup_visibility(this, POPUP_API.get());
}
unsafe extern "C" fn on_sidebar_update(this: Obj, dt: f32, mi: Obj) {
    let f: unsafe extern "C" fn(Obj, f32, Obj) =
        std::mem::transmute(SIDEBAR_UPDATE.load(Ordering::Acquire));
    f(this, dt, mi);
    enforce_popup_visibility(this, SIDEBAR_API.get());
}
unsafe fn enforce_popup_visibility(this: Obj, a: Option<&PopupApi>) {
    let hidden = state()
        .lock()
        .map(|s| s.race && s.hide_skill_popups)
        .unwrap_or(false);
    if !hidden {
        restore_popups();
        return;
    }
    let Some(a) = a else { return };
    let Some(visible) = scalar::<bool>(a.visible, this) else {
        return;
    };
    if let Ok(mut saved) = POPUP_RESTORE.lock() {
        if !saved.iter().any(|(h, _, _)| target(*h) == this) {
            saved.push((hold(this), visible, *a));
        }
        if visible {
            popup_visibility(a, this, false);
        }
    }
}
unsafe fn install_popup_hook() -> Result<(), String> {
    let a = PopupApi {
        visible: method("Gallop.RaceUISkillInfoController", "IsVisible", 0),
        set_visible: method("Gallop.RaceUISkillInfoController", "SetVisible", 1),
    };
    if a.visible == 0 || a.set_visible == 0 {
        return Err("Skill popup visibility bindings unavailable".into());
    }
    let hook = prepare_args(
        "Gallop.RaceUISkillInfoController",
        "LateUpdate",
        1,
        on_popup_late as *const (),
        &POPUP_LATE,
    )?;
    let _ = POPUP_API.set(a);
    hook.enable()?;
    let _ = D_POPUP.set(hook);
    Ok(())
}

unsafe fn install_sidebar_hook() -> Result<(), String> {
    let a = PopupApi {
        visible: method("Gallop.RaceUIMessagePlate", "IsVisible", 0),
        set_visible: method("Gallop.RaceUIMessagePlate", "SetVisible", 1),
    };
    if a.visible == 0 || a.set_visible == 0 {
        return Err("Skill sidebar visibility bindings unavailable".into());
    }
    let hook = prepare_args(
        "Gallop.RaceUIMessagePlate",
        "Update",
        1,
        on_sidebar_update as *const (),
        &SIDEBAR_UPDATE,
    )?;
    let _ = SIDEBAR_API.set(a);
    hook.enable()?;
    let _ = D_SIDEBAR.set(hook);
    Ok(())
}

unsafe fn install_visual_hooks() -> String {
    let mut notes = vec![];
    if let Err(e) = install_sidebar_hook() {
        notes.push(e);
    }
    if let Err(e) = install_popup_hook() {
        notes.push(e);
    }
    if let Err(e) = install_cutin_hook() {
        notes.push(e);
    }
    if let Err(e) = install_overhead_controls() {
        notes.push(e);
    }
    if let Err(e) = install_post_hook() {
        notes.push(e);
    }
    let a = API.get().unwrap();
    if a.mesh_type == 0
        || a.particle_type == 0
        || a.renderer_enabled_get == 0
        || a.renderer_enabled_set == 0
    {
        notes.push("Tree/skill renderer bindings unavailable".into());
    }
    if notes.is_empty() {
        "Bottom HUD, trees and render-pass effect cleanup available".into()
    } else {
        format!("Visual controls partially available: {}", notes.join("; "))
    }
}

unsafe extern "C" fn on_canvas(mi: Obj) {
    crate::framerate::tick();
    if let Ok(mut l) = live().lock() {
        restore_ui(&mut l);
        restore_world(&mut l);
    }
    let f: unsafe extern "C" fn(Obj) = std::mem::transmute(CANVAS.load(Ordering::Acquire));
    f(mi);
    let a = API.get().unwrap();
    let Ok(mut l) = live().lock() else { return };
    let racing = l
        .race_seen
        .is_some_and(|t| t.elapsed() < Duration::from_millis(500));
    if !racing {
        l.free_pose = None;
        l.target_tween.reset();
        clear_ground(&mut l.ground);
        restore_camera(&mut l);
    }
    let (hide, bottom, trees, effects, hide_skills, markers, speed_lines, overhead, scan, paths) = {
        let Ok(mut s) = state().lock() else { return };
        s.race = racing;
        if !racing {
            s.gates.clear();
            s.clear_telemetry();
            l.timing_last = None;
            l.director.reset();
            l.fixed.reset();
            l.auto_finish.reset();
            l.battle.reset();
        }
        let scan = s.scan && racing;
        if scan {
            s.scan = false;
        }
        (
            s.hide_ui,
            s.hide_bottom,
            s.hide_trees,
            s.clean_effects,
            s.hide_skill_popups,
            s.hide_markers,
            s.hide_speed_lines,
            s.hide_overhead,
            scan,
            s.settings.hidden_paths.clone(),
        )
    };
    if !racing || !overhead {
        restore_overhead();
    }
    if !racing || !hide_skills {
        restore_popups();
    }
    mask_world(
        &mut l,
        racing && trees,
        racing && effects,
        racing && hide_skills,
        racing && markers,
        racing && speed_lines,
    );
    if !racing || !(bottom || hide_skills || scan || (hide && !paths.is_empty())) {
        return;
    }
    let mut discovered = std::collections::BTreeSet::new();
    for r in objects(a.renderer_type) {
        let p = path(r);
        if scan {
            let mut prefix = String::new();
            for part in p.split('/') {
                if !prefix.is_empty() {
                    prefix.push('/');
                }
                prefix.push_str(part);
                discovered.insert(prefix.clone());
            }
        }
        let hidden = (bottom && crate::visual::bottom_hud(&p))
            || (hide && paths.iter().any(|s| path_matches(&p, s)));
        let faded = hide_skills && crate::visual::skill_popup(&p);
        if hidden || faded {
            // GetAlpha is stripped in Steam Global; GetColor().a is the local alpha.
            if let Some(color) = scalar::<[f32; 4]>(a.color_get, r) {
                let old = color[3];
                if old.is_finite() {
                    l.restore.push((hold(r), old));
                    set_float(a.alpha_set, r, 0.);
                }
            }
        }
    }
    if scan {
        if let Ok(mut s) = state().lock() {
            s.paths = discovered.into_iter().collect();
        }
    }
}
// BackKeyInputManager.Update checks this flag before polling right-click. Its
// keyboard branch runs independently. Keep the override scoped to this call.
unsafe fn back_update_scoped(
    this: Obj,
    mi: Obj,
    offset: usize,
    suppress: bool,
    original: unsafe extern "C" fn(Obj, Obj),
) {
    if !suppress || this.is_null() {
        original(this, mi);
        return;
    }
    let flag = this.cast::<u8>().add(offset);
    let saved = flag.read();
    flag.write(0);
    original(this, mi);
    flag.write(saved);
}
unsafe extern "C" fn on_back_update(this: Obj, mi: Obj) {
    let suppress =
        crate::overlay::focused() && state().lock().map(|s| s.camera && s.race).unwrap_or(false);
    let original = std::mem::transmute(BACK.load(Ordering::Acquire));
    back_update_scoped(
        this,
        mi,
        BACK_MOUSE_OFFSET.load(Ordering::Acquire),
        suppress,
        original,
    );
    crate::capture::tick();
}

// Steam input moves the OS pointer directly; window-message filtering cannot
// prevent it. Keep its software movement/navigation and native warp idle while
// RaceDirector owns the race camera. Physical mouse movement remains untouched.
static CURSOR_MOVE: AtomicUsize = AtomicUsize::new(0);
static CURSOR_LERP: AtomicUsize = AtomicUsize::new(0);
static CURSOR_NAV: AtomicUsize = AtomicUsize::new(0);
static CURSOR_NATIVE: AtomicUsize = AtomicUsize::new(0);
static D_CURSOR_MOVE: OnceLock<NativeHook> = OnceLock::new();
static D_CURSOR_LERP: OnceLock<NativeHook> = OnceLock::new();
static D_CURSOR_NAV: OnceLock<NativeHook> = OnceLock::new();
static D_CURSOR_NATIVE: OnceLock<NativeHook> = OnceLock::new();
fn owns_cursor_navigation(camera: bool, race: bool, focused: bool) -> bool {
    camera && race && focused
}
fn suppress_cursor_navigation() -> bool {
    let focused = crate::overlay::focused();
    state()
        .lock()
        .map(|s| owns_cursor_navigation(s.camera, s.race, focused))
        .unwrap_or(false)
}
unsafe fn cursor_update_scoped(
    this: Obj,
    mi: Obj,
    suppress: bool,
    original: unsafe extern "C" fn(Obj, Obj),
) {
    if !suppress {
        original(this, mi);
    }
}
unsafe extern "C" fn on_cursor_move(this: Obj, mi: Obj) {
    cursor_update_scoped(
        this,
        mi,
        suppress_cursor_navigation(),
        std::mem::transmute(CURSOR_MOVE.load(Ordering::Acquire)),
    );
}
unsafe extern "C" fn on_cursor_lerp(this: Obj, mi: Obj) {
    cursor_update_scoped(
        this,
        mi,
        suppress_cursor_navigation(),
        std::mem::transmute(CURSOR_LERP.load(Ordering::Acquire)),
    );
}
unsafe extern "C" fn on_cursor_nav(this: Obj, mi: Obj) {
    cursor_update_scoped(
        this,
        mi,
        suppress_cursor_navigation(),
        std::mem::transmute(CURSOR_NAV.load(Ordering::Acquire)),
    );
}
unsafe fn cursor_native_scoped(
    this: Obj,
    x: i32,
    y: i32,
    mi: Obj,
    suppress: bool,
    original: unsafe extern "C" fn(Obj, i32, i32, Obj),
) {
    if !suppress {
        original(this, x, y, mi);
    }
}
unsafe extern "C" fn on_cursor_native(this: Obj, x: i32, y: i32, mi: Obj) {
    cursor_native_scoped(
        this,
        x,
        y,
        mi,
        suppress_cursor_navigation(),
        std::mem::transmute(CURSOR_NATIVE.load(Ordering::Acquire)),
    );
}
unsafe fn install_cursor_hooks() -> Result<(), String> {
    let cls = "Gallop.SteamGamepadControl";
    // Resolve all entry points before enabling any hook.
    let movement = prepare(
        cls,
        "HandleSoftwareCursorMovement",
        on_cursor_move as *const (),
        &CURSOR_MOVE,
    )?;
    let lerp = prepare(
        cls,
        "HandleSoftwareCursorLerpMovement",
        on_cursor_lerp as *const (),
        &CURSOR_LERP,
    )?;
    let nav = prepare(
        cls,
        "HandleDPadNavigation",
        on_cursor_nav as *const (),
        &CURSOR_NAV,
    )?;
    let native = prepare_args(
        cls,
        "MoveNativeCursor",
        2,
        on_cursor_native as *const (),
        &CURSOR_NATIVE,
    )?;
    movement.enable()?;
    let _ = D_CURSOR_MOVE.set(movement);
    lerp.enable()?;
    let _ = D_CURSOR_LERP.set(lerp);
    nav.enable()?;
    let _ = D_CURSOR_NAV.set(nav);
    native.enable()?;
    let _ = D_CURSOR_NATIVE.set(native);
    crate::state::log(
        "Steam keyboard/gamepad cursor navigation isolated during race camera ownership",
    );
    Ok(())
}

pub fn start() {
    crate::state::log("Starting runtime initialization");
    std::thread::spawn(|| {
        for _ in 0..120 {
            if il::game_loaded() && il::init().is_ok() {
                break;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        if !il::ready() {
            status("GameAssembly unavailable; controls disabled");
            return;
        }
        let th = il::attach_current_thread();
        let result = unsafe { install() };
        il::detach_thread(th);
        status(match result {
            Ok(message) => message,
            Err(e) => format!("Camera unavailable: {e}"),
        });
    });
}
unsafe fn prepare(
    cls: &str,
    n: &str,
    detour: *const (),
    tr: &AtomicUsize,
) -> Result<NativeHook, String> {
    prepare_args(cls, n, 0, detour, tr)
}
unsafe fn prepare_args(
    cls: &str,
    n: &str,
    argc: i32,
    detour: *const (),
    tr: &AtomicUsize,
) -> Result<NativeHook, String> {
    let p = il::method_pointer(il::method(il::class(cls), n, argc));
    if p.is_null() {
        return Err(format!("{cls}.{n} is missing"));
    }
    if il::is_detoured(p) {
        return Err(format!("{cls}.{n} is already hooked"));
    }
    let d = NativeHook::new(p as *const (), detour).map_err(|e| format!("{cls}.{n}: {e}"))?;
    tr.store(d.trampoline(), Ordering::Release);
    Ok(d)
}
unsafe fn install() -> Result<String, String> {
    let export = |n: &[u8]| -> usize {
        use windows_sys::Win32::System::LibraryLoader::*;
        let w: Vec<u16> = "GameAssembly.dll\0".encode_utf16().collect();
        GetProcAddress(GetModuleHandleW(w.as_ptr()), n.as_ptr())
            .map(|p| p as usize)
            .unwrap_or(0)
    };
    let hc = il::class("Gallop.HorseRaceInfo");
    let a = Api {
        pos: il::field_offset(hc, "_position").ok_or("HorseRaceInfo._position missing")?,
        rot: il::field_offset(hc, "_rotationOnLane")
            .ok_or("HorseRaceInfo._rotationOnLane missing")?,
        horse_class: hc as usize,
        horse_data: method("Gallop.HorseRaceInfo", "get_HorseData", 0),
        // Optional, observed in HorseData metadata. Never call InitTrainerName/setter.
        trainer_name: method("Gallop.HorseData", "get_TrainerName", 0),
        gate: method("Gallop.HorseData", "get_GateNo", 0),
        find: il::method_with_param_types(
            il::class("UnityEngine.Object"),
            "FindObjectsOfType",
            &["System.Type"],
        ) as usize,
        manager_type: il::type_object(il::class("Gallop.RaceManager")) as usize,
        manager_horses: method("Gallop.RaceManager", "GetHorseRaceInfos", 0),
        current_camera: method("Gallop.RaceCameraManager", "get_CurrentCamera", 0),
        name: method("UnityEngine.Object", "get_name", 0),
        transform: method("UnityEngine.Component", "get_transform", 0),
        parent: method("UnityEngine.Transform", "get_parent", 0),
        renderer_type: il::type_object(il::class("UnityEngine.CanvasRenderer")) as usize,
        color_get: method("UnityEngine.CanvasRenderer", "GetColor", 0),
        alpha_set: method("UnityEngine.CanvasRenderer", "SetAlpha", 1),
        aspect_get: method("UnityEngine.Camera", "get_aspect", 0),
        fov_get: method("UnityEngine.Camera", "get_fieldOfView", 0),
        fov_set: method("UnityEngine.Camera", "set_fieldOfView", 1),
        near_get: method("UnityEngine.Camera", "get_nearClipPlane", 0),
        near_set: method("UnityEngine.Camera", "set_nearClipPlane", 1),
        world_renderer_type: il::type_object(il::class("UnityEngine.Renderer")) as usize,
        mesh_type: il::type_object(il::class("UnityEngine.MeshRenderer")) as usize,
        particle_type: il::type_object(il::class("UnityEngine.ParticleSystemRenderer")) as usize,
        renderer_enabled_get: method("UnityEngine.Renderer", "get_enabled", 0),
        renderer_enabled_set: method("UnityEngine.Renderer", "set_enabled", 1),
        alive: method("UnityEngine.Object", "op_Implicit", 1),
        getpos: il::resolve_icall(
            "UnityEngine.Transform::get_position_Injected(UnityEngine.Vector3&)",
        ) as usize,
        getrot: il::resolve_icall(
            "UnityEngine.Transform::get_rotation_Injected(UnityEngine.Quaternion&)",
        ) as usize,
        setpos: il::resolve_icall(
            "UnityEngine.Transform::set_position_Injected(UnityEngine.Vector3&)",
        ) as usize,
        setrot: il::resolve_icall(
            "UnityEngine.Transform::set_rotation_Injected(UnityEngine.Quaternion&)",
        ) as usize,
        gc_new: export(b"il2cpp_gchandle_new\0"),
        gc_get: export(b"il2cpp_gchandle_get_target\0"),
        gc_free: export(b"il2cpp_gchandle_free\0"),
    };
    let required = [
        ("HorseRaceInfo", a.horse_class),
        ("HorseRaceInfo.get_HorseData", a.horse_data),
        ("HorseData.get_GateNo", a.gate),
        ("Object.FindObjectsOfType(Type)", a.find),
        ("RaceManager type", a.manager_type),
        ("RaceManager.GetHorseRaceInfos", a.manager_horses),
        ("RaceCameraManager.get_CurrentCamera", a.current_camera),
        ("Component.get_transform", a.transform),
        ("Camera.get_fieldOfView", a.fov_get),
        ("Camera.set_fieldOfView", a.fov_set),
        ("Camera.get_nearClipPlane", a.near_get),
        ("Camera.set_nearClipPlane", a.near_set),
        ("Object.op_Implicit", a.alive),
        ("Transform.get_position_Injected", a.getpos),
        ("Transform.get_rotation_Injected", a.getrot),
        ("Transform.set_position_Injected", a.setpos),
        ("Transform.set_rotation_Injected", a.setrot),
        ("il2cpp_gchandle_new", a.gc_new),
        ("il2cpp_gchandle_get_target", a.gc_get),
        ("il2cpp_gchandle_free", a.gc_free),
    ];
    let missing: Vec<_> = required
        .iter()
        .filter(|(_, p)| *p == 0)
        .map(|(n, _)| *n)
        .collect();
    if !missing.is_empty() {
        return Err(format!("missing {}", missing.join(", ")));
    }
    let ui_missing: Vec<_> = [
        ("Object.get_name", a.name),
        ("Transform.get_parent", a.parent),
        ("CanvasRenderer type", a.renderer_type),
        ("CanvasRenderer.GetColor", a.color_get),
        ("CanvasRenderer.SetAlpha", a.alpha_set),
    ]
    .into_iter()
    .filter(|(_, p)| *p == 0)
    .map(|(n, _)| n)
    .collect();
    let late = prepare(
        "Gallop.RaceCameraManager",
        "AlterLateUpdate",
        on_late as *const (),
        &LATE,
    )?;
    let canvas = if ui_missing.is_empty() {
        prepare(
            "UnityEngine.Canvas",
            "SendWillRenderCanvases",
            on_canvas as *const (),
            &CANVAS,
        )
    } else {
        Err(format!("missing {}", ui_missing.join(", ")))
    };
    // Require input isolation before enabling the camera. A missing binding must
    // not silently reintroduce right-drag opening the pause menu.
    let mouse_offset = il::field_offset(
        il::class("Gallop.BackKeyInputManager"),
        "<EnableMouseClick>k__BackingField",
    )
    .ok_or("BackKeyInputManager.EnableMouseClick field missing")?;
    let back = prepare(
        "Gallop.BackKeyInputManager",
        "Update",
        on_back_update as *const (),
        &BACK,
    )?;
    install_cursor_hooks()?;
    BACK_MOUSE_OFFSET.store(mouse_offset, Ordering::Release);
    back.enable()?;
    let _ = D_BACK.set(back);
    crate::state::log("Right-click back action isolated during race camera control");
    let _ = API.set(a);
    crate::framerate::init();
    crate::graphics::init();
    crate::capture::init();
    init_ground();
    crate::state::log("Enabling race camera hook (MinHook)");
    late.enable().map_err(|e| e.to_string())?;
    let _ = D_LATE.set(late);
    crate::state::log("Enabling optional Canvas hook (MinHook)");
    match canvas {
        Ok(d) => match d.enable() {
            Ok(()) => {
                let _ = D_CANVAS.set(d);
                let visual_status = install_visual_hooks();
                crate::state::log(&visual_status);
                if let Ok(mut s) = state().lock() {
                    s.visual_status = visual_status;
                }
                Ok("Ready — camera and UI available, including paused races".into())
            }
            Err(e) => Ok(format!("Camera ready; UI unavailable: {e}")),
        },
        Err(e) => Ok(format!("Camera ready; UI unavailable: {e}")),
    }
}

#[cfg(test)]
mod back_input_tests {
    use super::*;
    unsafe extern "C" fn sample(this: Obj, _: Obj) {
        let bytes = this.cast::<u8>();
        // Record what the game's original Update sees; unrelated work still runs.
        bytes.add(2).write(bytes.add(1).read());
        bytes.add(3).write(bytes.add(3).read() + 1);
    }
    #[test]
    fn camera_suppresses_mouse_only_during_original_update() {
        let mut data = [99u8, 1, 9, 0];
        unsafe {
            back_update_scoped(
                data.as_mut_ptr().cast(),
                std::ptr::null_mut(),
                1,
                true,
                sample,
            );
        }
        assert_eq!(data, [99, 1, 0, 1]);
    }
    #[test]
    fn disabled_camera_passes_mouse_and_preserves_disabled_game_flag() {
        for (suppress, value) in [(false, 1), (false, 0), (true, 0)] {
            let mut data = [99u8, value, 9, 0];
            unsafe {
                back_update_scoped(
                    data.as_mut_ptr().cast(),
                    std::ptr::null_mut(),
                    1,
                    suppress,
                    sample,
                );
            }
            assert_eq!(data, [99, value, value, 1]);
        }
    }
}

#[cfg(test)]
mod post_tests {
    use super::ScopedMode;
    #[test]
    fn nested_render_modes_restore_in_order() {
        let mut mode = 4i32;
        let p = &mut mode as *mut i32;
        unsafe {
            let outer = ScopedMode::disable(p);
            assert_eq!(p.read(), 0);
            {
                let _inner = ScopedMode::disable(p);
                assert_eq!(p.read(), 0);
            }
            assert_eq!(p.read(), 0);
            drop(outer);
            assert_eq!(p.read(), 4);
        }
    }
    #[test]
    fn already_disabled_mode_stays_disabled() {
        let mut mode = 0i32;
        {
            let _scope = unsafe { ScopedMode::disable(&mut mode) };
        }
        assert_eq!(mode, 0);
    }
}

#[cfg(test)]
mod cursor_input_tests {
    use super::*;
    unsafe extern "C" fn update(this: Obj, mi: Obj) {
        assert_eq!(mi as usize, 123);
        *this.cast::<i32>() += 1;
    }
    unsafe extern "C" fn warp(this: Obj, x: i32, y: i32, mi: Obj) {
        assert_eq!(mi as usize, 123);
        let values = std::slice::from_raw_parts_mut(this.cast::<i32>(), 3);
        values[0] += 1;
        values[1] = x;
        values[2] = y;
    }
    #[test]
    fn cursor_navigation_suppresses_updates_and_warps_only_during_camera_ownership() {
        for (camera, race, focused) in [
            (true, true, true),
            (false, true, true),
            (true, false, true),
            (true, true, false),
        ] {
            let suppress = owns_cursor_navigation(camera, race, focused);
            let mut values = [0, 0, 0];
            let obj = values.as_mut_ptr().cast();
            unsafe {
                cursor_update_scoped(obj, 123usize as Obj, suppress, update);
                cursor_native_scoped(obj, -420, 1080, 123usize as Obj, suppress, warp);
            }
            if camera && race && focused {
                assert_eq!(values, [0, 0, 0]);
            } else {
                assert_eq!(values, [2, -420, 1080]);
            }
        }
    }
    #[test]
    fn releasing_camera_resumes_original_cursor_handler() {
        let mut count = 0i32;
        unsafe {
            cursor_update_scoped(
                (&mut count as *mut i32).cast(),
                123usize as Obj,
                true,
                update,
            );
            assert_eq!(count, 0);
            cursor_update_scoped(
                (&mut count as *mut i32).cast(),
                123usize as Obj,
                false,
                update,
            );
            assert_eq!(count, 1);
        }
    }
}

// PlaySkillCutIn only constructs and queues presentation data. Suppress before
// the cut-in state machine pauses the race; never skip its enter/exit callbacks.
static CUTIN: AtomicUsize = AtomicUsize::new(0);
static D_CUTIN: OnceLock<NativeHook> = OnceLock::new();
unsafe extern "C" fn on_cutin(
    this: Obj,
    horse: Obj,
    skill: i32,
    level: i32,
    debuff: bool,
    category: i32,
    cutin: i32,
    mi: Obj,
) {
    let skip = state()
        .lock()
        .map(|s| s.race && s.skip_cutins)
        .unwrap_or(false);
    if !skip {
        let f: unsafe extern "C" fn(Obj, Obj, i32, i32, bool, i32, i32, Obj) =
            std::mem::transmute(CUTIN.load(Ordering::Acquire));
        f(this, horse, skill, level, debuff, category, cutin, mi);
    }
}
unsafe fn install_cutin_hook() -> Result<(), String> {
    let m = method("Gallop.RaceManagerReplayBase", "PlaySkillCutIn", 6);
    let expected = [
        "Gallop.IHorseRaceInfo",
        "System.Int32",
        "System.Int32",
        "System.Boolean",
        "Gallop.SkillDefine.SkillCategory",
        "Gallop.RaceManager.CutInCategory",
    ];
    if m == 0 || il::method_params_of(m as il::Method) != expected {
        return Err("Skill cut-in signature unavailable".into());
    }
    let hook = prepare_args(
        "Gallop.RaceManagerReplayBase",
        "PlaySkillCutIn",
        6,
        on_cutin as *const (),
        &CUTIN,
    )?;
    hook.enable()?;
    let _ = D_CUTIN.set(hook);
    Ok(())
}
struct OverheadApi {
    effect: usize,
    cameras: [usize; 4],
    enabled: usize,
    set_enabled: usize,
}
static OVERHEAD_API: OnceLock<OverheadApi> = OnceLock::new();
static OVERHEAD_RESTORE: Mutex<Vec<(u32, bool)>> = Mutex::new(Vec::new());
unsafe fn set_camera_enabled(a: &OverheadApi, camera: Obj, mut enabled: bool) {
    invoke(
        a.set_enabled,
        camera,
        &mut [&mut enabled as *mut bool as Obj],
    );
}
unsafe fn restore_overhead() {
    let Some(a) = OVERHEAD_API.get() else { return };
    if let Ok(mut saved) = OVERHEAD_RESTORE.lock() {
        for (h, old) in saved.drain(..) {
            let camera = release(h);
            if valid(camera) {
                set_camera_enabled(a, camera, old);
            }
        }
    }
}
unsafe fn mask_overhead(manager: Obj) {
    let hide = state()
        .lock()
        .map(|s| s.race && s.hide_overhead)
        .unwrap_or(false);
    if !hide {
        return;
    }
    let Some(a) = OVERHEAD_API.get() else { return };
    let effect = get(a.effect, manager);
    if !valid(effect) {
        return;
    }
    if let Ok(mut saved) = OVERHEAD_RESTORE.lock() {
        for getter in a.cameras {
            let camera = get(getter, effect);
            if !valid(camera) || saved.iter().any(|(h, _)| target(*h) == camera) {
                continue;
            }
            if scalar::<bool>(a.enabled, camera) == Some(true) {
                saved.push((hold(camera), true));
                set_camera_enabled(a, camera, false);
            }
        }
    }
    // BeginMultiCameraPostRender checks MultiCamera.enabled before allocating
    // its texture. The native null-texture path skips inset composition and
    // retains ordinary marker drawing, final blits, and resource cleanup.
}
unsafe fn install_overhead_controls() -> Result<(), String> {
    let a = OverheadApi {
        effect: method("Gallop.RaceCameraManager", "get_ImageEffect", 0),
        cameras: [
            "get_MultiCamera",
            "get_MultiEnvCamera",
            "get_MarkerMultiCamera",
            "get_MaskMultiCamera",
        ]
        .map(|n| method("Gallop.RaceImageEffect", n, 0)),
        enabled: method("UnityEngine.Behaviour", "get_enabled", 0),
        set_enabled: method("UnityEngine.Behaviour", "set_enabled", 1),
    };
    if a.effect == 0 || a.enabled == 0 || a.set_enabled == 0 || a.cameras.contains(&0) {
        return Err("Overhead camera bindings unavailable".into());
    }
    let _ = OVERHEAD_API.set(a);
    Ok(())
}

#[derive(Clone, Copy)]
struct CourseApi {
    distance: usize,
    world_scale: usize,
}
impl CourseApi {
    fn resolve(class: il::Class) -> Self {
        Self {
            distance: il::method(class, "GetCourseDistance", 0) as usize,
            world_scale: il::method(class, "get_CourseDistance2World", 0) as usize,
        }
    }
}
#[derive(Clone, Copy)]
struct TimingApi {
    name: usize,
    order: usize,
    finish_order: usize,
    finished: usize,
    distance: usize,
    speed: usize,
}
impl TimingApi {
    fn resolve(class: il::Class) -> Self {
        let m = |n| il::method(class, n, 0) as usize;
        Self {
            name: m("get_CharaName"),
            order: m("get_CurOrder"),
            finish_order: m("get_FinishOrder"),
            finished: m("IsFinished"),
            distance: m("GetDistance"),
            speed: m("GetLastSpeed"),
        }
    }
    fn complete(self) -> bool {
        [
            self.name,
            self.order,
            self.finish_order,
            self.finished,
            self.distance,
            self.speed,
        ]
        .iter()
        .all(|m| *m != 0)
    }
}
unsafe fn update_timing(l: &mut Live, racing: bool) {
    let enabled = state().lock().map(|s| s.needs_telemetry()).unwrap_or(false);
    if !enabled || !racing {
        l.timing_last = None;
        if let Ok(mut s) = state().lock() {
            s.clear_telemetry();
            s.timing_status = "Waiting for race telemetry".into();
        }
        return;
    }
    if l.timing_last
        .is_some_and(|t| t.elapsed() < Duration::from_millis(100))
    {
        return;
    }
    l.timing_last = Some(Instant::now());
    let Some(h) = l.manager else { return };
    let manager = target(h);
    if !valid(manager) {
        if let Ok(mut s) = state().lock() {
            s.clear_telemetry();
        }
        return;
    }
    let manager_class = il::object_class(manager);
    if l.course_bindings
        .is_none_or(|(class, _)| class != manager_class as usize)
    {
        l.course_bindings = Some((manager_class as usize, CourseApi::resolve(manager_class)));
    }
    let course_api = l.course_bindings.unwrap().1;
    // Optional replay/course metadata. Missing getters or managed exceptions
    // produce None; these values belong to this race and are never persisted.
    let detected_length = scalar::<f32>(course_api.distance, manager)
        .filter(|n| n.is_finite() && (100. ..=10000.).contains(n));
    let world_scale = crate::director::valid_scale(scalar::<f32>(course_api.world_scale, manager));
    let api = API.get().unwrap();
    let lookup_trainers = state()
        .lock()
        .map(|s| s.settings.tower.auto_trainers && s.settings.tower.show_trainers)
        .unwrap_or(false);
    let mut rows = Vec::new();
    let mut finished_gates = Vec::new();
    let mut complete = true;
    for horse in array(get(api.manager_horses, manager), 64) {
        if !il::object_is(horse, api.horse_class as il::Class) {
            continue;
        }
        let class = il::object_class(horse);
        // Replay implementations override abstract base getters. Resolve against
        // the concrete class, not HorseRaceInfo's abstract invoker stubs.
        if l.timing_bindings.is_none_or(|(c, _)| c != class as usize) {
            l.timing_bindings = Some((class as usize, TimingApi::resolve(class)));
        }
        let a = l.timing_bindings.unwrap().1;
        complete &= a.complete();
        let data = get(api.horse_data, horse);
        if data.is_null() {
            continue;
        }
        let Some(gate) = scalar::<i32>(api.gate, data).filter(|v| (1..=64).contains(v)) else {
            continue;
        };
        let finished = scalar::<bool>(a.finished, horse).unwrap_or(false);
        // Replay finish order is precomputed: do not reveal it before finishing.
        let order = scalar::<i32>(if finished { a.finish_order } else { a.order }, horse);
        let rank = order.filter(|v| (0..64).contains(v)).map(|v| v + 1);
        if finished && rank.is_some() {
            finished_gates.push(gate);
        }
        let mut label = il::read_string(get(a.name, horse));
        if label.trim().is_empty() {
            label = format!("Runner {gate}");
        }
        rows.push(crate::timing::Runner {
            gate,
            name: label,
            trainer_name: if lookup_trainers && api.trainer_name != 0 {
                crate::timing::clean_trainer_name(&il::read_string(get(api.trainer_name, data)))
            } else {
                None
            },
            rank,
            distance: scalar::<f32>(a.distance, horse),
            speed: scalar::<f32>(a.speed, horse),
        });
    }
    let rows = crate::timing::ordered(rows);
    if let Ok(mut s) = state().lock() {
        s.timing_status = if complete {
            format!("{} runners / updates 10 times per second. Speed is the race velocity, retained while paused.", rows.len())
        } else {
            "Some telemetry getters are unavailable. Missing values display --.".into()
        };
        if lookup_trainers {
            let found = rows.iter().filter(|r| r.trainer_name.is_some()).count();
            s.timing_status.push_str(&if api.trainer_name == 0 {
                " Trainer lookup unavailable in this game build.".into()
            } else {
                format!(" Trainer names: {found}/{} supplied by game.", rows.len())
            });
        }
        s.update_telemetry(rows, finished_gates, h);
        s.detected_race_distance = detected_length;
        s.course_world_scale = world_scale;
    }
}

#[derive(Default)]
struct GroundCache {
    source: usize,
    scan: Option<Instant>,
    pending: Vec<u32>,
    surface: crate::camera_motion::Surface,
}
struct GroundApi {
    mesh_type: usize,
    shared: usize,
    readable: usize,
    vertices: usize,
    triangles: usize,
    matrix: usize,
}
static GROUND_API: OnceLock<GroundApi> = OnceLock::new();
unsafe fn init_ground() {
    let a = GroundApi {
        mesh_type: il::type_object(il::class("UnityEngine.MeshFilter")) as usize,
        shared: method("UnityEngine.MeshFilter", "get_sharedMesh", 0),
        readable: method("UnityEngine.Mesh", "get_isReadable", 0),
        vertices: method("UnityEngine.Mesh", "get_vertices", 0),
        triangles: method("UnityEngine.Mesh", "get_triangles", 0),
        matrix: method("UnityEngine.Transform", "get_localToWorldMatrix", 0),
    };
    if [
        a.mesh_type,
        a.shared,
        a.readable,
        a.vertices,
        a.triangles,
        a.matrix,
    ]
    .contains(&0)
    {
        crate::state::log("Ground mesh bindings unavailable; using nearest runner height");
        return;
    }
    let _ = GROUND_API.set(a);
}
unsafe fn clear_ground(g: &mut GroundCache) {
    for h in g.pending.drain(..) {
        release(h);
    }
    *g = GroundCache::default();
}
unsafe fn values<T: Copy>(array: Obj, limit: usize) -> Vec<T> {
    if array.is_null() {
        return vec![];
    }
    let p = array.cast::<u8>();
    let n = p.add(24).cast::<usize>().read_unaligned();
    if n > limit {
        return vec![];
    }
    (0..n)
        .map(|i| {
            p.add(32 + i * std::mem::size_of::<T>())
                .cast::<T>()
                .read_unaligned()
        })
        .collect()
}
unsafe fn update_ground(l: &mut Live) {
    let source = l.manager.map(|h| target(h) as usize).unwrap_or(0);
    if l.ground.source != source {
        l.target_tween.reset();
        clear_ground(&mut l.ground);
        l.ground.source = source;
    }
    let Some(a) = GROUND_API.get() else { return };
    let g = &mut l.ground;
    if g.scan.is_none()
        || (g.surface.len() == 0
            && g.pending.is_empty()
            && g.scan.is_some_and(|t| t.elapsed() > Duration::from_secs(3)))
    {
        g.pending = objects(a.mesh_type)
            .into_iter()
            .filter(|o| crate::camera_motion::ground_mesh(&path(*o)))
            .map(|o| hold(o))
            .collect();
        g.scan = Some(Instant::now());
    }
    // Load one static surface per frame, then retain only CPU geometry. No
    // physics objects, renderers or game assets are modified.
    if let Some(h) = g.pending.pop() {
        let obj = release(h);
        if valid(obj) {
            let mesh = get(a.shared, obj);
            if valid(mesh) && scalar::<bool>(a.readable, mesh) == Some(true) {
                let vertices = values::<Vec3>(get(a.vertices, mesh), 300_000);
                let indices = values::<i32>(get(a.triangles, mesh), 600_000);
                if let Some(m) =
                    scalar::<[f32; 16]>(a.matrix, get(API.get().unwrap().transform, obj))
                {
                    let world = vertices
                        .into_iter()
                        .map(|v| Vec3 {
                            x: m[0] * v.x + m[4] * v.y + m[8] * v.z + m[12],
                            y: m[1] * v.x + m[5] * v.y + m[9] * v.z + m[13],
                            z: m[2] * v.x + m[6] * v.y + m[10] * v.z + m[14],
                        })
                        .collect::<Vec<_>>();
                    for ids in indices.chunks_exact(3) {
                        if let (Some(a), Some(b), Some(c)) = (
                            world.get(ids[0] as usize),
                            world.get(ids[1] as usize),
                            world.get(ids[2] as usize),
                        ) {
                            g.surface.add([*a, *b, *c]);
                        }
                    }
                }
            }
        }
        let message=format!("Ground guard: {} surface triangles cached; {} meshes pending. Outside coverage, uses nearest runner height.",g.surface.len(),g.pending.len());
        if let Ok(mut s) = state().lock() {
            s.ground_status = message;
        }
    }
}
fn ground_at(surface: &crate::camera_motion::Surface, eye: Vec3, horses: &[Horse]) -> f32 {
    surface.height(eye).unwrap_or_else(|| {
        horses
            .iter()
            .min_by(|a, b| {
                let dist = |h: &Horse| (h.pos.x - eye.x).powi(2) + (h.pos.z - eye.z).powi(2);
                dist(a).total_cmp(&dist(b))
            })
            .map(|h| h.pos.y)
            .unwrap_or(0.)
    })
}
