//! MinHook correctly relocates RIP-relative instructions with trailing immediates.
//! The previous retour backend assumed disp32 occupied the final four bytes.
use hudhook::mh::{MH_CreateHook, MH_EnableHook, MH_Initialize, MH_STATUS};
use std::ffi::c_void;
pub struct NativeHook {
    target: usize,
    trampoline: usize,
}
impl NativeHook {
    pub unsafe fn new(target: *const (), detour: *const ()) -> Result<Self, String> {
        match MH_Initialize() {
            MH_STATUS::MH_OK | MH_STATUS::MH_ERROR_ALREADY_INITIALIZED => {}
            s => return Err(format!("MinHook init: {s:?}")),
        }
        let mut original = std::ptr::null_mut();
        let s = MH_CreateHook(target as *mut c_void, detour as *mut c_void, &mut original);
        if s != MH_STATUS::MH_OK {
            return Err(format!("MinHook create: {s:?}"));
        }
        Ok(Self {
            target: target as usize,
            trampoline: original as usize,
        })
    }
    pub fn trampoline(&self) -> usize {
        self.trampoline
    }
    pub unsafe fn enable(&self) -> Result<(), String> {
        let s = MH_EnableHook(self.target as *mut c_void);
        if s == MH_STATUS::MH_OK {
            Ok(())
        } else {
            Err(format!("MinHook enable: {s:?}"))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use windows_sys::Win32::System::Memory::{
        VirtualAlloc, VirtualFree, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_EXECUTE_READWRITE,
    };
    static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
    unsafe extern "C" fn detour() -> i32 {
        let f: unsafe extern "C" fn() -> i32 =
            std::mem::transmute(ORIGINAL.load(Ordering::Acquire));
        f() + 10
    }
    #[test]
    fn rip_relative_compare_preserves_trailing_immediate() {
        unsafe {
            // Match SendWillRenderCanvases: sub rsp,28h; cmp byte ptr [rip+disp32],0.
            let memory = VirtualAlloc(
                std::ptr::null(),
                4096,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_EXECUTE_READWRITE,
            ) as *mut u8;
            assert!(!memory.is_null());
            let mut code = vec![
                0x48, 0x83, 0xec, 0x28, 0x80, 0x3d, 0, 0, 0, 0, 0, 0x0f, 0x95, 0xc0, 0x0f, 0xb6,
                0xc0, 0x48, 0x83, 0xc4, 0x28, 0xc3,
            ];
            code[6..10].copy_from_slice(&(64i32 - 11).to_le_bytes());
            std::ptr::copy_nonoverlapping(code.as_ptr(), memory, code.len());
            memory.add(64).write_volatile(1);
            let f: unsafe extern "C" fn() -> i32 = std::mem::transmute(memory);
            assert_eq!(f(), 1);
            let h = NativeHook::new(memory as *const (), detour as *const ()).unwrap();
            ORIGINAL.store(h.trampoline(), Ordering::Release);
            let trampoline = h.trampoline() as *const u8;
            assert_eq!(
                trampoline.add(10).read(),
                0,
                "cmp immediate must remain zero"
            );
            let relocated = trampoline
                .add(11)
                .offset((trampoline.add(6) as *const i32).read_unaligned() as isize);
            assert_eq!(relocated, memory.add(64));
            h.enable().unwrap();
            assert_eq!(f(), 11);
            memory.add(64).write_volatile(0);
            assert_eq!(f(), 10);
            assert_eq!(
                hudhook::mh::MH_DisableHook(memory as *mut c_void),
                MH_STATUS::MH_OK
            );
            assert_eq!(f(), 0);
            // Remove the disabled hook before releasing its target memory.
            extern "system" {
                fn MH_RemoveHook(target: *mut c_void) -> MH_STATUS;
            }
            assert_eq!(MH_RemoveHook(memory as *mut c_void), MH_STATUS::MH_OK);
            assert_ne!(VirtualFree(memory as *mut c_void, 0, MEM_RELEASE), 0);
        }
    }
}
