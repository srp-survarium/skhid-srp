//! What patch 83's renderer choice asks of Windows at the process's start
//! (docs/notes/dxvk-option.md), before the engine's memory exists: the user's
//! user.cfg (read into a static buffer: no engine allocator), the files of
//! the game's folder, the best Vulkan device (vulkan-1.dll loaded for the
//! look and let go), and DXVK's DLLs loaded from `dxvk\` by their full paths.
//! kernel32's and shell32's functions through the exe's import slots (raw).

use core::cell::UnsafeCell;
use core::ffi::{CStr, c_void};
use core::ptr::{NonNull, null, null_mut};

use super::hook::Thread;
use crate::raw;

/// user.cfg is a few kilobytes; a longer one is read this far.
const USER_CFG_ROOM: usize = 64 * 1024;
/// A Windows path (MAX_PATH) and what is joined to it.
const PATH_ROOM: usize = 0x200;

struct Buffer(UnsafeCell<[u8; USER_CFG_ROOM]>);
// SAFETY: touched only inside `with_user_cfg`, on the game's thread (the
// Thread token), and no borrow of it outlives that call.
unsafe impl Sync for Buffer {}
static USER_CFG: Buffer = Buffer(UnsafeCell::new([0; USER_CFG_ROOM]));

/// A path built in place, NUL-terminated.
struct Path {
    bytes: [u8; PATH_ROOM],
    len: usize,
}

impl Path {
    fn push(&mut self, part: &[u8]) -> Option<()> {
        let end = self.len.checked_add(part.len())?;
        // the NUL after it
        if end >= PATH_ROOM {
            return None;
        }
        self.bytes[self.len..end].copy_from_slice(part);
        self.bytes[end] = 0;
        self.len = end;
        Some(())
    }

    fn c(&self) -> *const u8 {
        self.bytes.as_ptr()
    }
}

/// `<Documents>\survarium\user.cfg` as the engine finds its user folder
/// (SHGetFolderPathA CSIDL_PERSONAL, 0x18BD0; client_patch.md "The user
/// folder"), read whole (at most 64 KiB) into the static buffer, handed to
/// `f`. None when there is no such file.
pub fn with_user_cfg<R>(_: &Thread, f: impl FnOnce(&[u8]) -> R) -> Option<R> {
    const CSIDL_PERSONAL: u32 = 5;
    let mut path = Path { bytes: [0; PATH_ROOM], len: 0 };
    // SAFETY: SHGetFolderPathA writes at most MAX_PATH (260) bytes and its
    // NUL into the 512 given (the delay-loaded slot: its helper loads
    // shell32 at the first call, as the engine's own call does).
    let found = unsafe {
        (raw::engine_sh_get_folder_path_a)(null_mut(), CSIDL_PERSONAL, null_mut(), 0, path.bytes.as_mut_ptr())
    };
    if found != 0 {
        return None;
    }
    path.len = path.bytes.iter().position(|&b| b == 0)?;
    path.push(b"\\survarium\\user.cfg")?;
    const GENERIC_READ: u32 = 1 << 31;
    const SHARE_ALL: u32 = 7;
    const OPEN_EXISTING: u32 = 3;
    // SAFETY: CreateFileA, ReadFile and CloseHandle through the import
    // slots; the buffer is the static's, touched on this thread alone, its
    // borrow ending with `f`.
    unsafe {
        let file = (raw::engine_create_file_a)(path.c(), GENERIC_READ, SHARE_ALL, null_mut(), OPEN_EXISTING, 0x80, null_mut());
        if file as isize == -1 {
            return None;
        }
        let buffer = &mut *USER_CFG.0.get();
        let mut filled = 0usize;
        while filled < USER_CFG_ROOM {
            let mut read = 0u32;
            let ok = (raw::engine_read_file)(
                file,
                buffer[filled..].as_mut_ptr().cast(),
                (USER_CFG_ROOM - filled) as u32,
                &mut read,
                null_mut(),
            );
            if ok == 0 || read == 0 {
                break;
            }
            filled += read as usize;
        }
        (raw::engine_close_handle)(file);
        Some(f(&buffer[..filled]))
    }
}

/// The game's folder (`binaries\x86`, the exe's own: GetModuleFileNameA)
/// joined with `name`.
fn beside_exe(name: &CStr) -> Option<Path> {
    let mut path = Path { bytes: [0; PATH_ROOM], len: 0 };
    // SAFETY: GetModuleFileNameA writes at most the size given, NUL included.
    let len = unsafe { (raw::engine_get_module_file_name_a)(null_mut(), path.bytes.as_mut_ptr(), PATH_ROOM as u32) } as usize;
    if len == 0 || len >= PATH_ROOM {
        return None;
    }
    path.len = path.bytes[..len].iter().rposition(|&b| b == b'\\')? + 1;
    path.push(name.to_bytes())?;
    Some(path)
}

/// Whether the file `name` of the game's folder is there.
pub fn exists_beside_exe(_: &Thread, name: &CStr) -> bool {
    const OPEN_EXISTING: u32 = 3;
    let Some(path) = beside_exe(name) else { return false };
    // SAFETY: CreateFileA (no access, everything shared) and CloseHandle.
    unsafe {
        let file = (raw::engine_create_file_a)(path.c(), 0, 7, null_mut(), OPEN_EXISTING, 0x80, null_mut());
        if file as isize == -1 {
            return false;
        }
        (raw::engine_close_handle)(file);
    }
    true
}

/// Loads the DLL `name` of the game's folder by its full path (LoadLibraryA),
/// for the life of the process: a module of that name then serves every
/// later load of the plain name (the game's delay-loaded `d3d11.dll`, DXVK's
/// own imports). Whether it loaded.
pub fn load_beside_exe(_: &Thread, name: &CStr) -> bool {
    let Some(path) = beside_exe(name) else { return false };
    // SAFETY: LoadLibraryA with a NUL-terminated path; the module is never
    // let go.
    !unsafe { (raw::engine_load_library_a)(path.c()) }.is_null()
}

/// A Vulkan version as Vulkan packs it (`VK_MAKE_API_VERSION`, variant 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct VulkanVersion(u32);

impl VulkanVersion {
    pub const fn new(major: u32, minor: u32, patch: u32) -> VulkanVersion {
        VulkanVersion((major << 22) | (minor << 12) | patch)
    }

    /// A device's `apiVersion`; None for another variant than 0.
    pub fn of_packed(packed: u32) -> Option<VulkanVersion> {
        (packed >> 29 == 0).then_some(VulkanVersion(packed))
    }

    pub fn major(self) -> u32 {
        self.0 >> 22
    }

    pub fn minor(self) -> u32 {
        (self.0 >> 12) & 0x3FF
    }
}

/// What the Vulkan look found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vulkan {
    /// The best version of a device that is not the CPU.
    Device(VulkanVersion),
    /// No `vulkan-1.dll`, or not its functions.
    NoLoader,
    /// `vkCreateInstance` failed.
    NoInstance,
    /// No device but the CPU.
    NoDevice,
}

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

/// Room for a VkPhysicalDeviceProperties (824 bytes on 32-bit x86, 8-byte
/// aligned); apiVersion is its first u32, deviceType its fifth.
#[repr(C, align(8))]
struct Properties([u32; 256]);

/// vulkan-1.dll loaded, an instance made, every device's properties read,
/// the instance destroyed and the DLL let go (the four functions are the
/// library's dynamic rows: VKAPI_CALL is stdcall on 32-bit Windows).
pub fn best_vulkan(_: &Thread) -> Vulkan {
    const PHYSICAL_DEVICE_TYPE_CPU: u32 = 4;
    // SAFETY: each function is the GetProcAddress result of its own name in
    // the loader, called with vulkan_core.h's structures; every handle made
    // here is destroyed or let go here.
    unsafe {
        let library = (raw::engine_load_library_a)(c"vulkan-1.dll".as_ptr().cast());
        if library.is_null() {
            return Vulkan::NoLoader;
        }
        let function = |name: &CStr| NonNull::new((raw::engine_get_proc_address)(library, name.as_ptr().cast()));
        let (Some(create), Some(destroy), Some(enumerate), Some(properties)) = (
            function(c"vkCreateInstance"),
            function(c"vkDestroyInstance"),
            function(c"vkEnumeratePhysicalDevices"),
            function(c"vkGetPhysicalDeviceProperties"),
        ) else {
            (raw::engine_free_library)(library);
            return Vulkan::NoLoader;
        };
        let application = ApplicationInfo {
            s_type: 0,
            next: null(),
            application_name: c"survarium".as_ptr().cast(),
            application_version: 0,
            engine_name: null(),
            engine_version: 0,
            // 1.0: any loader takes it; a device reports its own version anyway
            api_version: VulkanVersion::new(1, 0, 0).0,
        };
        let info = InstanceCreateInfo {
            s_type: 1,
            next: null(),
            flags: 0,
            application_info: &application,
            layer_count: 0,
            layers: null(),
            extension_count: 0,
            extensions: null(),
        };
        let mut instance: *mut c_void = null_mut();
        let made = raw::engine_vk_create_instance(create, (&raw const info).cast(), null(), &mut instance);
        if made != 0 || instance.is_null() {
            (raw::engine_free_library)(library);
            return Vulkan::NoInstance;
        }
        let mut devices = [null_mut(); 16];
        let mut count = devices.len() as u32;
        let mut best = None;
        // VK_INCOMPLETE (5) with more than 16: these 16 are enough; an error is negative
        if (raw::engine_vk_enumerate_physical_devices(enumerate, instance, &mut count, devices.as_mut_ptr()) as i32) >= 0 {
            for &device in &devices[..(count as usize).min(devices.len())] {
                let mut room = Properties([0; 256]);
                raw::engine_vk_get_physical_device_properties(properties, device, (&raw mut room).cast());
                if room.0[4] != PHYSICAL_DEVICE_TYPE_CPU
                    && let Some(version) = VulkanVersion::of_packed(room.0[0])
                {
                    best = best.max(Some(version));
                }
            }
        }
        raw::engine_vk_destroy_instance(destroy, instance, null());
        (raw::engine_free_library)(library);
        best.map_or(Vulkan::NoDevice, Vulkan::Device)
    }
}
