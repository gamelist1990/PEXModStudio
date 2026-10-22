use std::os::windows::ffi::OsStrExt;
#[link(name="kernel32")]
extern "system" {
    fn LoadLibraryW(path: *const u16) -> *mut core::ffi::c_void;
    fn GetProcAddress(module: *mut core::ffi::c_void, name: *const u8) -> *mut core::ffi::c_void;
}
fn main() {
    let path = std::env::args_os().nth(1).expect("bridge path");
    let wide: Vec<u16> = path.encode_wide().chain(Some(0)).collect();
    unsafe {
        let module = LoadLibraryW(wide.as_ptr()); assert!(!module.is_null());
        let p = GetProcAddress(module,b"InitializeASI\0".as_ptr()); assert!(!p.is_null());
        let init: extern "C" fn() = std::mem::transmute(p); init();
    }
}
