// A harmless x64 DLL: no game hooks, no DllMain work, no network or file access.
// The bridge's LOADED line confirms Windows accepted and mapped this module.
#[no_mangle]
pub extern "C" fn PexProbeVersion() -> u32 { 1 }
