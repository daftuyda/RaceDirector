//! Cursor ownership is confined to the game's main-thread BackKey update.
use crate::{il2cpp as il, state::state};
use std::sync::{Mutex, OnceLock};
use windows_sys::Win32::{
    Foundation::POINT,
    Graphics::Gdi::ClientToScreen,
    UI::WindowsAndMessaging::{GetClientRect, GetCursorPos, GetForegroundWindow, SetCursorPos},
};
struct Api {
    get_visible: usize,
    set_visible: usize,
    get_lock: usize,
    set_lock: usize,
}
static API: OnceLock<Api> = OnceLock::new();
#[derive(Clone, Copy)]
struct Saved {
    visible: bool,
    lock: i32,
    position: POINT,
    center: POINT,
}
static SAVED: Mutex<Option<Saved>> = Mutex::new(None);
pub fn init() {
    let a = Api {
        get_visible: il::resolve_icall("UnityEngine.Cursor::get_visible()") as usize,
        set_visible: il::resolve_icall("UnityEngine.Cursor::set_visible(System.Boolean)") as usize,
        get_lock: il::resolve_icall("UnityEngine.Cursor::get_lockState()") as usize,
        set_lock: il::resolve_icall("UnityEngine.Cursor::set_lockState(UnityEngine.CursorLockMode)")
            as usize,
    };
    let available = [a.get_visible, a.set_visible, a.get_lock, a.set_lock]
        .iter()
        .all(|p| *p != 0);
    if available {
        let _ = API.set(a);
    }
    if let Ok(mut s) = state().lock() {
        s.capture_status = if available {
            "Cursor controls ready. F8 locks a clean view."
        } else {
            "Cursor controls unavailable; clean capture is disabled."
        }
        .into();
    }
}
pub fn available() -> bool {
    API.get().is_some()
}
/// Used by both cursor ownership and panel presentation; focus loss releases it.
pub fn active(requested: bool, camera: bool, racing: bool, focused: bool) -> bool {
    requested && camera && racing && focused
}
pub unsafe fn tick() {
    let Some(a) = API.get() else { return };
    let focused = crate::overlay::focused();
    let Ok(mut s) = state().lock() else { return };
    if !s.camera || !s.race || !focused {
        s.capture = false;
    }
    let enabled = active(s.capture, s.camera, s.race, focused);
    let Ok(mut saved) = SAVED.lock() else { return };
    let gv: unsafe extern "C" fn() -> bool = std::mem::transmute(a.get_visible);
    let sv: unsafe extern "C" fn(bool) = std::mem::transmute(a.set_visible);
    let gl: unsafe extern "C" fn() -> i32 = std::mem::transmute(a.get_lock);
    let sl: unsafe extern "C" fn(i32) = std::mem::transmute(a.set_lock);
    if !enabled {
        if let Some(old) = saved.take() {
            sl(old.lock);
            sv(old.visible);
            if focused {
                SetCursorPos(old.position.x, old.position.y);
            }
        }
        return;
    }
    let hwnd = GetForegroundWindow();
    let mut rect = std::mem::zeroed();
    let mut center = POINT { x: 0, y: 0 };
    let mut mouse = POINT { x: 0, y: 0 };
    if GetClientRect(hwnd, &mut rect) == 0 || GetCursorPos(&mut mouse) == 0 {
        return;
    }
    center.x = (rect.left + rect.right) / 2;
    center.y = (rect.top + rect.bottom) / 2;
    if ClientToScreen(hwnd, &mut center) == 0 {
        return;
    }
    // Confine to the game and recenter ourselves, retaining pixel mouse deltas.
    // Unity Locked continually warps the cursor and would erase these deltas.
    let entering = saved.is_none();
    if entering {
        *saved = Some(Saved {
            visible: gv(),
            lock: gl(),
            position: mouse,
            center,
        });
    }
    sl(2);
    sv(false);
    let unchanged_center = saved
        .as_ref()
        .is_some_and(|old| old.center.x == center.x && old.center.y == center.y);
    if !entering && unchanged_center {
        let dx = (mouse.x - center.x) as f32;
        let dy = (mouse.y - center.y) as f32;
        let sensitivity = s.settings.mouse_sensitivity;
        if s.freecam {
            if let Some(free) = &mut s.free_pose {
                free.yaw += dx * sensitivity;
                free.pitch += dy * sensitivity;
                free.sanitize();
            }
        } else {
            s.settings.pose.yaw += dx * sensitivity;
            s.settings.pose.pitch -= dy * sensitivity;
            s.settings.pose.sanitize();
        }
    }
    if let Some(old) = saved.as_mut() {
        old.center = center;
    }
    SetCursorPos(center.x, center.y);
}
#[cfg(test)]
mod tests {
    use super::active;
    #[test]
    fn capture_releases_on_focus_loss_camera_off_or_race_exit() {
        assert!(active(true, true, true, true));
        for flags in [
            (false, true, true, true),
            (true, false, true, true),
            (true, true, false, true),
            (true, true, true, false),
        ] {
            assert!(!active(flags.0, flags.1, flags.2, flags.3));
        }
    }
}
