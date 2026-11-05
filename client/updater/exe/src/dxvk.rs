//! The opt-in Vulkan renderer (`updater_logic::dxvk`,
//! `srp-026g/docs/notes/dxvk-option.md`): after the overlay, at every
//! update, DXVK's `d3d11.dll` and `dxgi.dll` are copied from `dxvk\` beside
//! the game when user.cfg says `srp_dxvk on` and a Vulkan device can run
//! DXVK, and ours are removed from beside the game otherwise. Best effort:
//! what fails is logged, and the update goes on as if nothing were asked.
//!
//! The Vulkan check runs in a second run of this program
//! (`updater_skhid.exe vulkan`, [`probe`]): a driver or a Vulkan layer that
//! crashes or hangs takes only that one down, never the update.

use alloc::format;
use alloc::string::String;
use core::ffi::c_void;
use core::ptr::{null, null_mut};

use updater_logic::dxvk::{
    DLLS, Decision, FOLDER, File, NEEDED, Probe, Version, asked, decide, is_ours,
};
use updater_logic::sha256::digest;

use crate::Session;
use crate::install::Layout;
use crate::log::log;
use crate::win;

/// The command word of the Vulkan check.
pub const PROBE_WORD: &str = "vulkan";
/// How long the Vulkan check may take before it is ended.
const PROBE_MS: u64 = 10_000;
/// Larger than either DLL (8 MB, 6 MB).
const MAX_DLL: usize = 64 << 20;
/// A copy while it is written beside its place (the overlay's name: the
/// next start's clean-up deletes one left over).
const NEW: &str = ".skhid-new";

/// The step: the decision for this run, carried out, and logged.
pub fn run(session: &mut Session, layout: &Layout) {
    let wanted = wanted(session, layout);
    let beside = DLLS.map(|name| look(&format!("{}\\{name}", layout.exe_dir)));
    let source = DLLS.map(|name| look(&source_path(layout, name)));
    match decide(wanted, beside, source) {
        Decision::Nothing => {}
        Decision::Copy(which) => {
            for i in which {
                if let Err(why) = copy(layout, DLLS[i], source[i]) {
                    log!("dxvk: {why}; DXVK not used this time");
                    remove_ours(layout, &[0, 1]);
                    return;
                }
                log!("dxvk: {} put beside the game", DLLS[i]);
            }
        }
        Decision::Remove(which) => remove_ours(layout, &which),
        Decision::NoSource(i) => log!("dxvk: {FOLDER}\\{} is not DXVK's: DXVK not used", DLLS[i]),
        Decision::Foreign(i) => log!(
            "dxvk: the {} beside the game is not DXVK's (the player's own?): left as it is, DXVK not used",
            DLLS[i]
        ),
    }
}

/// Whether DXVK is to be used: asked in user.cfg, and a Vulkan device can
/// run it (checked only when asked).
fn wanted(session: &mut Session, layout: &Layout) -> bool {
    if !win::user_cfg().is_some_and(|text| asked(&text)) {
        return false;
    }
    let probe = run_probe(session, layout);
    if probe.suffices() {
        log!("dxvk: asked; {probe}");
        return true;
    }
    log!("dxvk: asked, but not used: {probe} (DXVK needs Vulkan {NEEDED})");
    false
}

fn run_probe(session: &mut Session, layout: &Layout) -> Probe {
    let program = match win::module_path() {
        Ok(path) => path,
        Err(code) => return Probe::Failed(code),
    };
    let until = win::ticks() + PROBE_MS;
    let mut pump = || session.pump() && win::ticks() < until;
    match win::run_wait(
        &program,
        &format!("\"{program}\" {PROBE_WORD}"),
        &layout.exe_dir,
        "NUL",
        &mut pump,
    ) {
        Ok(code) => Probe::of_exit_code(code),
        Err(code) => Probe::Failed(code),
    }
}

fn source_path(layout: &Layout, name: &str) -> String {
    format!("{}\\{FOLDER}\\{name}", layout.exe_dir)
}

/// What a place holds: nothing, one of ours, or anything else (also a file
/// that cannot be read).
fn look(path: &str) -> File {
    match win::hash_file(path) {
        Ok((_, hash)) => File::of_hash(hash),
        Err(code) if win::is_missing(code) => File::Missing,
        Err(_) => File::Other,
    }
}

/// `name` from `dxvk\` beside the game: the bytes read are the ones checked
/// (`source`, one of ours), written beside their place, then renamed into it.
fn copy(layout: &Layout, name: &str, source: File) -> Result<(), String> {
    let from = source_path(layout, name);
    let data = win::read_file(&from, MAX_DLL)
        .map_err(|code| format!("{from}: {}", win::error_text(code)))?;
    let hash = digest(&data);
    if source != File::Ours(hash) || !is_ours(&hash) {
        return Err(format!("{from} changed while it was read"));
    }
    let to = format!("{}\\{name}", layout.exe_dir);
    let staged = format!("{to}{NEW}");
    let result = win::write_file(&staged, &data).and_then(|()| win::rename(&staged, &to, true));
    result.map_err(|code| {
        let _ = win::delete(&staged);
        format!("{to}: {}", win::error_text(code))
    })
}

/// Removes these DLLs (indices of `DLLS`) from beside the game when they are
/// ours; a DLL of the player's own stays. One that cannot be removed now (a
/// running game holds it) is logged and goes at a later start.
fn remove_ours(layout: &Layout, which: &[usize]) {
    for name in which.iter().map(|&i| DLLS[i]) {
        let path = format!("{}\\{name}", layout.exe_dir);
        if !matches!(look(&path), File::Ours(_)) {
            continue;
        }
        match win::delete(&path) {
            Ok(()) => log!("dxvk: {name} removed from beside the game"),
            Err(code) => log!("dxvk: {path} not removed: {}", win::error_text(code)),
        }
    }
}

// Vulkan, as far as the check needs it (vulkan_core.h). VKAPI_CALL is
// __stdcall on 32-bit Windows: `extern "system"`.
#[repr(C)]
struct ApplicationInfo {
    s_type: u32,
    next: *const c_void,
    application_name: *const u8,
    application_version: u32,
    engine_name: *const u8,
    engine_version: u32,
    api_version: u32,
}

#[repr(C)]
struct InstanceCreateInfo {
    s_type: u32,
    next: *const c_void,
    flags: u32,
    application_info: *const ApplicationInfo,
    layer_count: u32,
    layers: *const *const u8,
    extension_count: u32,
    extensions: *const *const u8,
}

const STRUCTURE_TYPE_APPLICATION_INFO: u32 = 0;
const STRUCTURE_TYPE_INSTANCE_CREATE_INFO: u32 = 1;
const PHYSICAL_DEVICE_TYPE_CPU: u32 = 4;
/// Room for a VkPhysicalDeviceProperties (824 bytes on 32-bit x86, 8-byte
/// aligned); the check reads two of its first fields: apiVersion (the first
/// u32) and deviceType (the fifth).
#[repr(C, align(8))]
struct Properties([u32; 512]);

type Handle = *mut c_void;
/// What GetProcAddress gives.
type Export = unsafe extern "system" fn() -> isize;
type CreateInstance =
    unsafe extern "system" fn(*const InstanceCreateInfo, *const c_void, *mut Handle) -> i32;
type DestroyInstance = unsafe extern "system" fn(Handle, *const c_void);
type EnumerateDevices = unsafe extern "system" fn(Handle, *mut u32, *mut Handle) -> i32;
type DeviceProperties = unsafe extern "system" fn(Handle, *mut Properties);

/// `updater_skhid.exe vulkan`: the best Vulkan version of a device that is
/// not the CPU, as the exit code (`Probe::exit_code`). No window, no log.
pub fn probe() -> u32 {
    use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
    let library = unsafe { LoadLibraryW(win::wide("vulkan-1.dll").as_ptr()) };
    if library.is_null() {
        return Probe::NoLoader.exit_code();
    }
    let function = |name: &[u8]| unsafe { GetProcAddress(library, name.as_ptr()) };
    let (Some(create), Some(destroy), Some(enumerate), Some(properties)) = (
        function(b"vkCreateInstance\0"),
        function(b"vkDestroyInstance\0"),
        function(b"vkEnumeratePhysicalDevices\0"),
        function(b"vkGetPhysicalDeviceProperties\0"),
    ) else {
        return Probe::NoLoader.exit_code();
    };
    // SAFETY: the loader's exports, of these signatures (vulkan_core.h).
    let (create, destroy, enumerate, properties) = unsafe {
        (
            core::mem::transmute::<Export, CreateInstance>(create),
            core::mem::transmute::<Export, DestroyInstance>(destroy),
            core::mem::transmute::<Export, EnumerateDevices>(enumerate),
            core::mem::transmute::<Export, DeviceProperties>(properties),
        )
    };
    let application = ApplicationInfo {
        s_type: STRUCTURE_TYPE_APPLICATION_INFO,
        next: null(),
        application_name: c"updater_skhid".as_ptr().cast(),
        application_version: 0,
        engine_name: null(),
        engine_version: 0,
        // 1.0: any loader takes it; a device reports its own version anyway.
        api_version: Version::new(1, 0, 0).packed(),
    };
    let info = InstanceCreateInfo {
        s_type: STRUCTURE_TYPE_INSTANCE_CREATE_INFO,
        next: null(),
        flags: 0,
        application_info: &application,
        layer_count: 0,
        layers: null(),
        extension_count: 0,
        extensions: null(),
    };
    let mut instance: Handle = null_mut();
    if unsafe { create(&info, null(), &mut instance) } != 0 || instance.is_null() {
        return Probe::NoInstance.exit_code();
    }
    let mut count = 0_u32;
    let mut devices = [null_mut(); 16];
    let mut best = None;
    if unsafe { enumerate(instance, &mut count, null_mut()) } >= 0 {
        count = count.min(devices.len() as u32);
        // VK_INCOMPLETE (5) with more than 16: these 16 are enough.
        if unsafe { enumerate(instance, &mut count, devices.as_mut_ptr()) } >= 0 {
            for &device in &devices[..count as usize] {
                let mut room = Properties([0; 512]);
                unsafe { properties(device, &mut room) };
                let (api, kind) = (room.0[0], room.0[4]);
                if kind != PHYSICAL_DEVICE_TYPE_CPU
                    && let Some(version) = Version::of_packed(api)
                {
                    best = best.max(Some(version));
                }
            }
        }
    }
    unsafe { destroy(instance, null()) };
    best.map_or(Probe::NoDevice, Probe::Device).exit_code()
}
