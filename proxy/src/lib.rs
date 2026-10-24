#![allow(non_snake_case, clippy::missing_safety_doc)]

use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use windows_sys::Win32::Foundation::{CloseHandle, HMODULE};
use windows_sys::Win32::System::LibraryLoader::{
    DisableThreadLibraryCalls, GetModuleFileNameW, GetModuleHandleW, GetProcAddress, LoadLibraryW,
};

// CreateThread — declared directly (windows-sys gates it behind extra type
// features we don't otherwise need).
type ThreadStart = unsafe extern "system" fn(*mut c_void) -> u32;
extern "system" {
    fn CreateThread(
        attrs: *const c_void,
        stack: usize,
        start: Option<ThreadStart>,
        param: *mut c_void,
        flags: u32,
        thread_id: *mut u32,
    ) -> HMODULE;
}

type BOOL = i32;
type HINSTANCE = *mut c_void;

/// Our own directory (the game folder), captured in DllMain.
static OWN_DIR: OnceLock<PathBuf> = OnceLock::new();

// ── UnityMain / UnityMain2 passthroughs ─────────────────────────────────────────
// These can't be `.def` forwarders (link.exe rejects forwarding to UnityPlayer
// here), so we export tiny runtime-forwarders that resolve UnityPlayer.dll and
// jump to the genuine symbol. This is safe: nothing normally imports these from
// version.dll (UnityPlayer exports its own, bound directly), and they're NOT on
// the early version-API path that must stay code-free. They exist only so a
// stacked/renamed install forwarding through us still reaches the engine entry.
static REAL_UP: OnceLock<usize> = OnceLock::new();

unsafe fn unity_fn(name: &[u8]) -> *const c_void {
    let h = *REAL_UP.get_or_init(|| {
        let mut w: Vec<u16> = "UnityPlayer.dll".encode_utf16().collect();
        w.push(0);
        LoadLibraryW(w.as_ptr()) as usize
    }) as HMODULE;
    if h.is_null() {
        return std::ptr::null();
    }
    GetProcAddress(h, name.as_ptr())
        .map(|f| f as *const c_void)
        .unwrap_or(std::ptr::null())
}

#[no_mangle]
pub unsafe extern "system" fn UnityMain(
    inst: HINSTANCE,
    prev: HINSTANCE,
    cmd: *mut u8,
    show: i32,
) -> i32 {
    let p = unity_fn(b"UnityMain\0");
    if p.is_null() {
        return 0;
    }
    let f: unsafe extern "system" fn(HINSTANCE, HINSTANCE, *mut u8, i32) -> i32 =
        std::mem::transmute(p);
    f(inst, prev, cmd, show)
}

#[no_mangle]
pub unsafe extern "system" fn UnityMain2(
    inst: HINSTANCE,
    prev: HINSTANCE,
    cmd: *mut u8,
    show: i32,
) -> i32 {
    let p = unity_fn(b"UnityMain2\0");
    if p.is_null() {
        return 0;
    }
    let f: unsafe extern "system" fn(HINSTANCE, HINSTANCE, *mut u8, i32) -> i32 =
        std::mem::transmute(p);
    f(inst, prev, cmd, show)
}

/// The folder this proxy lives in (= the game folder).
fn own_dir(hinst: HMODULE) -> PathBuf {
    unsafe {
        let mut buf = [0u16; 1024];
        let n = GetModuleFileNameW(hinst, buf.as_mut_ptr(), buf.len() as u32) as usize;
        let full = PathBuf::from(String::from_utf16_lossy(&buf[..n]));
        full.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

/// Load the standalone overlay once.
fn load_once() {
    static DONE: AtomicBool = AtomicBool::new(false);
    if DONE.swap(true, Ordering::SeqCst) {
        return;
    }
    let dir = OWN_DIR.get().cloned().unwrap_or_else(|| PathBuf::from("."));
    let overlay = dir.join("racedirector.dll");
    let mut w: Vec<u16> = overlay
        .as_os_str()
        .to_string_lossy()
        .encode_utf16()
        .collect();
    w.push(0);
    unsafe {
        LoadLibraryW(w.as_ptr());
    }
}

/// Worker thread that replicates the overlay's original **late-injection** path:
/// wait until `GameAssembly.dll` is present (the game's IL2CPP runtime is up),
/// give it a settle window, THEN load the overlay. Loading the overlay's D3D/
/// hudhook vtable hook too early (in DllMain, or right after the loader lock
/// releases) trips an integrity check → int3 in GameAssembly.dll on boot. The
/// overlay was designed to be injected after the game is running; this restores
/// that. See CRASH-NOTES.md.
unsafe extern "system" fn loader_thread(_p: *mut c_void) -> u32 {
    let ga: Vec<u16> = "GameAssembly.dll\0".encode_utf16().collect();
    // Wait for GameAssembly.dll to be loaded (bounded ~120 s).
    let mut waited = 0u32;
    while GetModuleHandleW(ga.as_ptr()).is_null() {
        std::thread::sleep(std::time::Duration::from_millis(250));
        waited += 250;
        if waited > 120_000 {
            break;
        }
    }
    // Settle window so the runtime/anti-tamper finishes its own init before our
    // hooks land — mirrors the overlay's known-good post-injection settle.
    std::thread::sleep(std::time::Duration::from_secs(3));
    load_once();
    0
}

const DLL_PROCESS_ATTACH: u32 = 1;

#[no_mangle]
pub unsafe extern "system" fn DllMain(hinst: HMODULE, reason: u32, _reserved: *mut c_void) -> BOOL {
    if reason == DLL_PROCESS_ATTACH {
        DisableThreadLibraryCalls(hinst);
        let _ = OWN_DIR.set(own_dir(hinst));
        // Defer overlay loading to a worker thread that waits for GameAssembly +
        // a settle window (late-injection). Loading too early trips a GameAssembly
        // integrity check (int3 on boot). We CreateThread and never wait on it, so
        // no loader-lock deadlock. Version APIs are static forwarders (version.def).
        let h = CreateThread(
            std::ptr::null(),
            0,
            Some(loader_thread),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
        );
        if !h.is_null() {
            CloseHandle(h);
        }
    }
    1
}
