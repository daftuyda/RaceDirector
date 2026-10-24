//! Race framebuffer overrides. Unity calls run only in AlterLateUpdate.
//! Rebuild through the game's own lifecycle so cameras/command buffers rebind.
use crate::{hooks::NativeHook, il2cpp as il, state::state};
use serde::{Deserialize, Serialize};
use std::{
    cell::Cell,
    ffi::c_void,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex, OnceLock,
    },
    time::{Duration, Instant},
};
type Obj = *mut c_void;
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Quality {
    pub scale: u32,
    pub msaa: i32,
}
impl Default for Quality {
    fn default() -> Self {
        Self {
            scale: 125,
            msaa: 2,
        }
    }
}
impl Quality {
    pub fn sanitize(&mut self) {
        if ![100, 125, 150, 200].contains(&self.scale) {
            self.scale = 125;
        }
        if ![1, 2, 4].contains(&self.msaa) {
            self.msaa = 2;
        }
    }
}
// Preserve aspect ratio, limit allocation to 16 megapixels and 8192 per axis.
fn dimensions(w: i32, h: i32, scale: u32) -> Option<(i32, i32)> {
    if w <= 0 || h <= 0 || w > 8192 || h > 8192 || (w as i64 * h as i64) > 16_777_216 {
        return None;
    }
    let f = (scale as f64 / 100.)
        .min(8192. / w.max(h) as f64)
        .min((16_777_216. / (w as f64 * h as f64)).sqrt());
    Some(((w as f64 * f).floor() as i32, (h as f64 * f).floor() as i32))
}
struct Api {
    fb_offset: usize,
    color: usize,
    result: usize,
    remake: usize,
    width: usize,
    height: usize,
    set_width: usize,
    set_height: usize,
    aa: usize,
    created: usize,
}
static API: OnceLock<Api> = OnceLock::new();
static CREATE: AtomicUsize = AtomicUsize::new(0);
static AA: AtomicUsize = AtomicUsize::new(0);
#[derive(Clone, Copy)]
struct Scope {
    fb: usize,
    quality: Quality,
    texture: usize,
    samples: i32,
    failed: bool,
}
thread_local! { static SCOPE:Cell<Option<Scope>>=const {Cell::new(None)}; }
#[derive(Default)]
struct Live {
    fb: usize,
    texture: usize,
    applied: Option<Quality>,
    last: Option<Instant>,
}
static LIVE: Mutex<Live> = Mutex::new(Live {
    fb: 0,
    texture: 0,
    applied: None,
    last: None,
});
fn method(c: &str, n: &str, a: i32) -> usize {
    il::method(il::class(c), n, a) as usize
}
unsafe fn call(m: usize, o: Obj, args: &mut [Obj]) -> Result<Obj, ()> {
    let (r, e) = il::runtime_invoke_exc(m as il::Method, o, args);
    if e.is_null() {
        Ok(r)
    } else {
        Err(())
    }
}
unsafe fn int(m: usize, o: Obj) -> Option<i32> {
    let r = call(m, o, &mut []).ok()?;
    if r.is_null() {
        None
    } else {
        Some(((r as usize + 16) as *const i32).read())
    }
}
unsafe fn set(m: usize, o: Obj, mut v: i32) -> bool {
    call(m, o, &mut [&mut v as *mut _ as Obj]).is_ok()
}
unsafe fn texture(m: usize, f: Obj) -> Option<Obj> {
    call(m, f, &mut []).ok().filter(|p| !p.is_null())
}
unsafe fn info(a: &Api, t: Obj) -> Option<(i32, i32, i32)> {
    let r = call(a.created, t, &mut []).ok()?;
    if r.is_null() || ((r as usize + 16) as *const u8).read() == 0 {
        return None;
    }
    Some((int(a.width, t)?, int(a.height, t)?, int(a.aa, t)?))
}
unsafe extern "C" fn on_aa(t: Obj, n: i32) {
    let samples = SCOPE
        .with(|s| {
            s.get()
                .filter(|x| x.texture == t as usize)
                .map(|x| x.samples)
        })
        .unwrap_or(n);
    let f: unsafe extern "C" fn(Obj, i32) = std::mem::transmute(AA.load(Ordering::Acquire));
    f(t, samples);
}
unsafe extern "C" fn on_create(fb: Obj, t: Obj, multisample: bool, name: Obj, mi: Obj) {
    let saved = SCOPE.with(|s| s.get());
    if let (Some(mut scope), Some(a)) = (saved, API.get()) {
        if scope.fb == fb as usize && !t.is_null() {
            if let Some((w, h)) = int(a.width, t)
                .zip(int(a.height, t))
                .and_then(|(w, h)| dimensions(w, h, scope.quality.scale))
            {
                scope.failed |= !set(a.set_width, t, w) || !set(a.set_height, t, h);
            } else {
                scope.failed = true;
            }
            scope.texture = t as usize;
            scope.samples = if multisample { scope.quality.msaa } else { 1 };
            SCOPE.with(|s| s.set(Some(scope)));
        }
    }
    let f: unsafe extern "C" fn(Obj, Obj, bool, Obj, Obj) =
        std::mem::transmute(CREATE.load(Ordering::Acquire));
    f(fb, t, multisample, name, mi);
    let failed = SCOPE.with(|s| s.get().is_some_and(|x| x.failed));
    SCOPE.with(|s| {
        s.set(saved.map(|mut x| {
            x.failed |= failed;
            x
        }))
    });
}
fn message(text: impl Into<String>) {
    if let Ok(mut s) = state().lock() {
        s.graphics_status = text.into();
    }
}
pub fn init() {
    let c = "Gallop.GallopFrameBuffer";
    let a = Api {
        fb_offset: il::field_offset(il::class("Gallop.RaceCameraManager"), "_frameBuffer")
            .unwrap_or(0),
        color: method(c, "get_ColorBuffer", 0),
        result: method(c, "get_ResultBuffer", 0),
        remake: method(c, "RemakeRenderTexture", 0),
        width: method("UnityEngine.RenderTexture", "get_width", 0),
        height: method("UnityEngine.RenderTexture", "get_height", 0),
        set_width: method("UnityEngine.RenderTexture", "set_width", 1),
        set_height: method("UnityEngine.RenderTexture", "set_height", 1),
        aa: method("UnityEngine.RenderTexture", "get_antiAliasing", 0),
        created: method("UnityEngine.RenderTexture", "IsCreated", 0),
    };
    let creation = method(c, "CreateRenderTexture", 3);
    let aa = il::resolve_icall("UnityEngine.RenderTexture::set_antiAliasing(System.Int32)");
    if [
        a.fb_offset,
        a.color,
        a.result,
        a.remake,
        a.width,
        a.height,
        a.set_width,
        a.set_height,
        a.aa,
        a.created,
        creation,
        aa as usize,
    ]
    .contains(&0)
    {
        message("Model clarity unavailable in this game build");
        return;
    }
    let result = (|| -> Result<(), String> {
        unsafe {
            let h1 = NativeHook::new(
                il::method_pointer(creation as il::Method) as *const (),
                on_create as *const (),
            )?;
            let h2 = NativeHook::new(aa as *const (), on_aa as *const ())?;
            CREATE.store(h1.trampoline(), Ordering::Release);
            AA.store(h2.trampoline(), Ordering::Release);
            let _ = API.set(a);
            h2.enable()?;
            h1.enable()?;
            Ok(())
        }
    })();
    match result {
        Ok(()) => {
            READY.store(1, Ordering::Release);
            message("Ready | enable model clarity during a race");
        }
        Err(e) => message(format!("Model clarity unavailable: {e}")),
    }
}
static READY: AtomicUsize = AtomicUsize::new(0);
/// Called before taking the camera runtime lock. Never invoked by the overlay thread.
pub unsafe fn tick(manager: Obj) {
    if READY.load(Ordering::Acquire) == 0 || manager.is_null() {
        return;
    }
    let a = API.get().unwrap();
    let fb = ((manager as usize + a.fb_offset) as *const Obj).read();
    if fb.is_null() {
        return;
    }
    let Some(t) = texture(a.color, fb) else {
        return;
    };
    let requested = state()
        .lock()
        .ok()
        .and_then(|s| s.graphics.then_some(s.graphics_applied));
    let Ok(mut live) = LIVE.lock() else {
        return;
    };
    let changed = live.fb != fb as usize || live.texture != t as usize;
    if !changed && live.applied == requested {
        return;
    }
    if requested.is_none() && (changed || live.applied.is_none()) {
        live.fb = fb as usize;
        live.texture = t as usize;
        live.applied = None;
        return;
    }
    if live
        .last
        .is_some_and(|at| at.elapsed() < Duration::from_secs(1))
    {
        return;
    }
    if info(a, t).is_none() {
        return;
    }
    live.last = Some(Instant::now());
    // No saved global graphics values: native rebuild derives fresh game defaults.
    SCOPE.with(|s| {
        s.set(requested.map(|quality| Scope {
            fb: fb as usize,
            quality,
            texture: 0,
            samples: 1,
            failed: false,
        }))
    });
    let ok = call(a.remake, fb, &mut []).is_ok();
    let failed = SCOPE.with(|s| s.replace(None).is_some_and(|x| x.failed));
    let observed = texture(a.color, fb).and_then(|t| info(a, t));
    let result = texture(a.result, fb).and_then(|t| info(a, t));
    let valid = observed
        .zip(result)
        .is_some_and(|(x, y)| x.0 == y.0 && x.1 == y.1 && y.2 == 1 && x.0 > 0 && x.1 > 0);
    if !ok || failed || !valid {
        let restored = call(a.remake, fb, &mut []).is_ok()
            && texture(a.color, fb).and_then(|t| info(a, t)).is_some();
        if let Ok(mut s) = state().lock() {
            s.graphics = false;
        }
        live.applied = None;
        message(if restored {
            "Clarity allocation failed; restored game rendering"
        } else {
            "Clarity rebuild failed; reload the race to recover rendering"
        });
    } else {
        live.applied = requested;
        let (w, h, aa) = observed.unwrap();
        message(if let Some(q) = requested {
            format!(
                "Race buffer: {w} x {h} | {aa}x MSAA (requested {}x, {}%)",
                q.msaa, q.scale
            )
        } else {
            format!("Game rendering restored: {w} x {h} | {aa}x MSAA")
        });
    }
    live.fb = fb as usize;
    live.texture = texture(a.color, fb).map(|p| p as usize).unwrap_or(0);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn samples_only_override_the_scoped_texture_and_restore_after_scope() {
        use std::sync::atomic::AtomicI32;
        static SEEN: AtomicI32 = AtomicI32::new(0);
        unsafe extern "C" fn original(_: Obj, n: i32) {
            SEEN.store(n, Ordering::SeqCst);
        }
        AA.store(original as *const () as usize, Ordering::Release);
        let t = 0x1000usize as Obj;
        unsafe {
            on_aa(t, 2);
            assert_eq!(SEEN.load(Ordering::SeqCst), 2);
            SCOPE.with(|s| {
                s.set(Some(Scope {
                    fb: 1,
                    quality: Quality::default(),
                    texture: t as usize,
                    samples: 4,
                    failed: false,
                }))
            });
            on_aa(t, 2);
            assert_eq!(SEEN.load(Ordering::SeqCst), 4);
            on_aa(0x2000usize as Obj, 2);
            assert_eq!(SEEN.load(Ordering::SeqCst), 2);
            SCOPE.with(|s| {
                let mut x = s.get().unwrap();
                x.samples = 1;
                s.set(Some(x));
            });
            on_aa(t, 2);
            assert_eq!(SEEN.load(Ordering::SeqCst), 1);
            SCOPE.with(|s| s.set(None));
            on_aa(t, 2);
            assert_eq!(SEEN.load(Ordering::SeqCst), 2);
        }
    }
    #[test]
    fn scale_and_limits_preserve_aspect() {
        assert_eq!(dimensions(1920, 1080, 150), Some((2880, 1620)));
        assert_eq!(dimensions(1920, 1080, 200), Some((3840, 2160)));
        let (w, h) = dimensions(3840, 2160, 200).unwrap();
        assert!(w as i64 * h as i64 <= 16_777_216);
        assert!((w as f64 / h as f64 - 16. / 9.).abs() < 0.001);
        assert_eq!(dimensions(0, 1080, 125), None);
        assert_eq!(dimensions(9000, 9000, 100), None);
    }
    #[test]
    fn quality_rejects_invalid_and_loads_defaults() {
        let mut q = Quality {
            scale: u32::MAX,
            msaa: 8,
        };
        q.sanitize();
        assert_eq!(q, Quality::default());
        let q: Quality = serde_json::from_str("{}").unwrap();
        assert_eq!(q, Quality::default());
    }
}
