//! Called on Unity's main thread from the Canvas callback.
use crate::{il2cpp as il, state::state};
use std::{sync::Mutex, sync::OnceLock};
struct Api {
    get_fps: usize,
    set_fps: usize,
    get_sync: usize,
    set_sync: usize,
}
static API: OnceLock<Api> = OnceLock::new();
static ORIGINAL: Mutex<Option<(i32, i32)>> = Mutex::new(None);
pub fn init() {
    let a = Api {
        get_fps: il::resolve_icall("UnityEngine.Application::get_targetFrameRate()") as usize,
        set_fps: il::resolve_icall("UnityEngine.Application::set_targetFrameRate(System.Int32)")
            as usize,
        get_sync: il::resolve_icall("UnityEngine.QualitySettings::get_vSyncCount()") as usize,
        set_sync: il::resolve_icall("UnityEngine.QualitySettings::set_vSyncCount(System.Int32)")
            as usize,
    };
    let available = [a.get_fps, a.set_fps, a.get_sync, a.set_sync]
        .iter()
        .all(|p| *p != 0);
    if available {
        let _ = API.set(a);
    }
    if let Ok(mut s) = state().lock() {
        s.fps_status = if available {
            "Ready to apply 60 FPS".into()
        } else {
            "Frame-rate controls unavailable in this game build".into()
        };
    }
}
unsafe fn get(m: usize) -> Option<i32> {
    if m == 0 {
        return None;
    }
    let f: unsafe extern "C" fn() -> i32 = std::mem::transmute(m);
    Some(f())
}
unsafe fn set(m: usize, value: i32) {
    let f: unsafe extern "C" fn(i32) = std::mem::transmute(m);
    f(value);
}
pub unsafe fn tick() {
    let Some(a) = API.get() else { return };
    let enabled = state().lock().map(|s| s.settings.fps_60).unwrap_or(false);
    let (Some(fps), Some(sync)) = (get(a.get_fps), get(a.get_sync)) else {
        return;
    };
    let Ok(mut saved) = ORIGINAL.lock() else {
        return;
    };
    if enabled {
        saved.get_or_insert((fps, sync));
        if sync != 0 {
            set(a.set_sync, 0);
        }
        if fps != 60 {
            set(a.set_fps, 60);
        }
    } else if let Some((old_fps, old_sync)) = saved.take() {
        set(a.set_fps, old_fps);
        set(a.set_sync, old_sync);
    }
    let observed = (get(a.get_fps), get(a.get_sync));
    if let Ok(mut s) = state().lock() {
        s.fps_status = if enabled && observed == (Some(60), Some(0)) {
            "60 FPS requested | VSync override off".into()
        } else if enabled {
            "Game has not accepted the 60 FPS override".into()
        } else {
            "Using game frame-rate settings".into()
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI32, Ordering::SeqCst};
    static FPS: AtomicI32 = AtomicI32::new(30);
    static SYNC: AtomicI32 = AtomicI32::new(2);
    unsafe extern "C" fn get_fps() -> i32 {
        FPS.load(SeqCst)
    }
    unsafe extern "C" fn set_fps(v: i32) {
        FPS.store(v, SeqCst);
    }
    unsafe extern "C" fn get_sync() -> i32 {
        SYNC.load(SeqCst)
    }
    unsafe extern "C" fn set_sync(v: i32) {
        SYNC.store(v, SeqCst);
    }
    #[test]
    fn override_reasserts_and_restores_original_then_recaptures() {
        assert!(API
            .set(Api {
                get_fps: get_fps as *const () as usize,
                set_fps: set_fps as *const () as usize,
                get_sync: get_sync as *const () as usize,
                set_sync: set_sync as *const () as usize
            })
            .is_ok());
        state().lock().unwrap().settings.fps_60 = true;
        unsafe {
            tick();
        }
        assert_eq!((FPS.load(SeqCst), SYNC.load(SeqCst)), (60, 0));
        FPS.store(24, SeqCst);
        unsafe {
            tick();
        }
        assert_eq!(FPS.load(SeqCst), 60);
        state().lock().unwrap().settings.fps_60 = false;
        unsafe {
            tick();
        }
        assert_eq!((FPS.load(SeqCst), SYNC.load(SeqCst)), (30, 2));
        FPS.store(-1, SeqCst);
        SYNC.store(1, SeqCst);
        unsafe {
            tick();
        }
        assert_eq!(FPS.load(SeqCst), -1);
        state().lock().unwrap().settings.fps_60 = true;
        unsafe {
            tick();
        }
        state().lock().unwrap().settings.fps_60 = false;
        unsafe {
            tick();
        }
        assert_eq!((FPS.load(SeqCst), SYNC.load(SeqCst)), (-1, 1));
    }
}
