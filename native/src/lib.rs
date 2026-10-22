use std::os::windows::ffi::OsStrExt;
use std::{fs, io::Write, path::PathBuf, sync::Once};
static START: Once = Once::new();
#[link(name = "kernel32")]
extern "system" {
    fn GetModuleFileNameW(module: *mut core::ffi::c_void, path: *mut u16, size: u32) -> u32;
    fn LoadLibraryExW(
        path: *const u16,
        file: *mut core::ffi::c_void,
        flags: u32,
    ) -> *mut core::ffi::c_void;
    fn GetLastError() -> u32;
}
fn log(root: &std::path::Path, message: &str) {
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(root.join("loader.log"))
    {
        let _ = writeln!(f, "{message}");
        let _ = f.flush();
    }
}
fn load() {
    let mut buf = vec![0u16; 32768];
    let len =
        unsafe { GetModuleFileNameW(std::ptr::null_mut(), buf.as_mut_ptr(), buf.len() as u32) };
    if len == 0 || len as usize >= buf.len() {
        return;
    }
    let exe = PathBuf::from(String::from_utf16_lossy(&buf[..len as usize]));
    if exe.file_name().map(|n| n.to_string_lossy().to_lowercase())
        != Some("minecraft.windows.exe".into())
    {
        return;
    }
    let Some(parent) = exe.parent() else {
        return;
    };
    let Ok(root) = parent.join("PEXMods").canonicalize() else {
        return;
    };
    log(&root, "--- PEX Mod Bridge 0.1.0 / session start ---");
    let previous = fs::read_to_string(root.join("loading.txt")).unwrap_or_default();
    let mut blocked: Vec<String> = fs::read_to_string(root.join("blocked.txt"))
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect();
    if !previous.trim().is_empty() && !blocked.iter().any(|s| s == previous.trim()) {
        blocked.push(previous.trim().to_owned());
        if fs::write(root.join("blocked.txt"), blocked.join("\n") + "\n").is_err() {
            log(&root, "ERROR: cannot save recovery block list");
            return;
        }
    }
    let _ = fs::remove_file(root.join("loading.txt"));
    let Ok(manifest) = fs::read_to_string(root.join("active.txt")) else {
        log(&root, "ERROR: active.txt unavailable");
        return;
    };
    for line in manifest.lines().filter(|s| !s.is_empty()) {
        if line.contains(':') || line.contains("..") || line.starts_with(['/', '\\']) {
            log(&root, "ERROR: invalid path");
            continue;
        }
        if blocked.iter().any(|s| s == line) {
            log(&root, &format!("SKIP {line}: previous initialization did not finish; disable/re-enable this mod to retry"));
            continue;
        }
        let Ok(path) = root.join(line).canonicalize() else {
            log(&root, &format!("MISSING {line}"));
            continue;
        };
        if !path.starts_with(&root)
            || path.extension().map(|s| s.to_string_lossy().to_lowercase()) != Some("dll".into())
        {
            continue;
        }
        if fs::write(root.join("loading.txt"), line).is_err() {
            log(
                &root,
                "ERROR: cannot write recovery marker; loading stopped",
            );
            return;
        }
        log(&root, &format!("LOADING {line}"));
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let module = unsafe { LoadLibraryExW(wide.as_ptr(), std::ptr::null_mut(), 0x100 | 0x1000) };
        if module.is_null() {
            log(
                &root,
                &format!("FAILED {line}: Win32 {}", unsafe { GetLastError() }),
            );
        } else {
            log(&root, &format!("LOADED {line}"));
        }
        let _ = fs::remove_file(root.join("loading.txt"));
    }
    log(&root, "Session initialization complete");
}
/// Ultimate ASI Loader calls this outside DllMain with DontLoadFromDllMain=1.
#[no_mangle]
pub extern "C" fn InitializeASI() {
    START.call_once(|| {
        let _ = std::panic::catch_unwind(load);
    });
}
