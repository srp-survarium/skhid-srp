//! What patch 83's renderer choice asks of Windows at the process's start
//! (docs/notes/dxvk-option.md), before the engine's memory exists: the user's
//! user.cfg (read into a static buffer: no engine allocator), the files of
//! the game's folder, the best Vulkan device (vulkan-1.dll loaded for the
//! look and let go), and DXVK's DLLs loaded from `dxvk\` by their full paths,
//! the game's delayed d3d9, dxgi and d3d11 bound to them; and, at each
//! Present, the output window's styles as DXGI's rule for them reads them.
//! kernel32's, shell32's and user32's functions through the exe's import
//! slots (raw).

use core::cell::UnsafeCell;
use core::ffi::{CStr, c_void};
use core::ptr::{NonNull, null, null_mut};

use super::context::Context;
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

/// A loaded DLL, never let go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Module(NonNull<c_void>);

/// Loads the DLL `name` of the game's folder by its full path (LoadLibraryA),
/// for the life of the process. Another module of the same name may be
/// loaded already (Windows' dxgi.dll, which the Vulkan loader loads to sort
/// its devices): a later load of the plain name may get either, so the game's
/// own imports are bound with [`bind_delayed_import`].
pub fn load_beside_exe(_: &Thread, name: &CStr) -> Option<Module> {
    let path = beside_exe(name)?;
    // SAFETY: LoadLibraryA with a NUL-terminated path; the module is never
    // let go.
    NonNull::new(unsafe { (raw::engine_load_library_a)(path.c()) }).map(Module)
}

/// Whether a module of the plain name `name` is loaded (GetModuleHandleA).
pub fn is_loaded(_: &Thread, name: &CStr) -> bool {
    // SAFETY: GetModuleHandleA with a NUL-terminated name; nothing is kept.
    !unsafe { (raw::engine_get_module_handle_a)(name.as_ptr().cast()) }.is_null()
}

/// Whether the process's environment variable `name` is set, to anything but
/// an empty value (GetEnvironmentVariableA answers 0 for both).
pub fn environment_set(_: &Thread, name: &CStr) -> bool {
    let mut room = [0u8; 1];
    // SAFETY: GetEnvironmentVariableA writes at most the one byte given; it
    // answers the size a value needs (more than that byte), 0 for none.
    unsafe { (raw::engine_get_environment_variable_a)(name.as_ptr().cast(), room.as_mut_ptr(), 1) != 0 }
}

/// The process's environment variable `name` into `room`, NUL-terminated:
/// its length, 0 for none or one longer than `room` holds.
pub fn get_environment(_: &Thread, name: &CStr, room: &mut [u8]) -> usize {
    // SAFETY: GetEnvironmentVariableA writes at most the size given, NUL
    // included, and answers the size it needs when that is more.
    let len = unsafe { (raw::engine_get_environment_variable_a)(name.as_ptr().cast(), room.as_mut_ptr(), room.len() as u32) } as usize;
    if len >= room.len() { 0 } else { len }
}

/// Deletes the process's environment variable `name` (SetEnvironmentVariableA
/// with no value).
pub fn clear_environment(_: &Thread, name: &CStr) {
    // SAFETY: SetEnvironmentVariableA with a NUL-terminated name and a null
    // value, which deletes the variable.
    unsafe { (raw::engine_set_environment_variable_a)(name.as_ptr().cast(), null()) };
}

/// Sets the process's environment variable `name` (SetEnvironmentVariableA):
/// DXVK reads its own (`DXVK_CONFIG`) with GetEnvironmentVariableW when its
/// first device is made, from the same Win32 environment, at most 259
/// characters (DXVK 3.1.1's util_env.cpp: a longer value reads as none).
pub fn set_environment(_: &Thread, name: &CStr, value: &CStr) -> bool {
    // SAFETY: SetEnvironmentVariableA with two NUL-terminated strings; it
    // copies them.
    unsafe { (raw::engine_set_environment_variable_a)(name.as_ptr().cast(), value.as_ptr().cast()) != 0 }
}

/// The game's delayed imports a DXVK DLL serves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DelayedImport {
    D3d9,
    Dxgi,
    D3d11,
}

impl DelayedImport {
    /// Its descriptor's module slot.
    fn slot(self) -> u32 {
        match self {
            DelayedImport::D3d9 => raw::image::D3D9_MODULE,
            DelayedImport::Dxgi => raw::image::DXGI_MODULE,
            DelayedImport::D3d11 => raw::image::D3D11_MODULE,
        }
    }
}

/// The game's delayed `import` bound to `module`: the delay-load helper
/// (0xE48E0) takes a module already in its slot and loads nothing. Only
/// while the slot is empty (the import not called yet: its functions'
/// slots would keep the module the helper bound); whether it was bound.
pub fn bind_delayed_import(_: &Thread, import: DelayedImport, module: Module) -> bool {
    let slot = import.slot() as *mut *mut c_void;
    // SAFETY: the slot is an HMODULE of the exe's .data, written by the
    // helper alone, at the import's first call, on the game's thread.
    unsafe {
        if !(*slot).is_null() {
            return false;
        }
        *slot = module.0.as_ptr();
    }
    true
}

/// The DirectDraw object the engine's video-memory probe made
/// (GetVideoMemoryViaDirectDraw 0x105DB0, the DirectX SDK's): the probe
/// releases only the IDirectDraw7 it asked it for, so the object, and with it
/// the GPU's own Direct3D driver DirectDrawCreate loaded (NVIDIA's
/// nvd3dum.dll, AMD's amdxn32.dll), would stay for the life of the process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProbeDirectDraw(NonNull<c_void>);

impl ProbeDirectDraw {
    /// At `video_memory.ddraw_probed` (the probe with the memory read, before
    /// it releases the IDirectDraw7 and lets ddraw.dll go): the object, None
    /// when DirectDrawCreate made none.
    pub fn at_probed(ctx: &Context<'_>) -> Option<ProbeDirectDraw> {
        // SAFETY: the context is this site's: the probe's frame, its slot 0
        // from the probe's start or the object DirectDrawCreate made, the
        // probe's alone, on the thread that runs it.
        let object = unsafe { *raw::site::video_memory_ddraw_probed::frame(ctx)?.ddraw() };
        NonNull::new(object as usize as *mut c_void).map(ProbeDirectDraw)
    }

    /// The probe's reference let go (IUnknown::Release): the references its
    /// IDirectDraw interface has left. The probe reads the slot no more; its
    /// IDirectDraw7 keeps the object until the probe releases that, which
    /// frees it, before ddraw.dll is let go.
    pub fn release(self, _: &Thread) -> u32 {
        // SAFETY: a COM object DirectDrawCreate made, alive (the probe holds
        // its one reference): its vtable's third entry is IUnknown::Release
        // (stdcall, the object its argument).
        unsafe {
            let vtable = *self.0.as_ptr().cast::<*const *mut c_void>();
            let Some(release) = NonNull::new(*vtable.add(2)) else { return 0 };
            raw::engine_com_release(release, self.0.as_ptr())
        }
    }
}

/// A window's two style words (GWL_STYLE, GWL_EXSTYLE), read together.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowStyles {
    pub style: u32,
    pub extended: u32,
}

impl WindowStyles {
    /// WS_POPUP (0x80000000).
    pub const WS_POPUP: u32 = 1 << 31;
    /// WS_POPUP, WS_BORDER (WS_CAPTION has it), WS_SYSMENU, WS_THICKFRAME,
    /// WS_MINIMIZEBOX, WS_MAXIMIZEBOX (bits 23, 19, 18, 17, 16).
    const FRAMED: u32 = WindowStyles::WS_POPUP | 1 << 23 | 1 << 19 | 1 << 18 | 1 << 17 | 1 << 16;
    /// WS_EX_DLGMODALFRAME, WS_EX_TOOLWINDOW, WS_EX_WINDOWEDGE,
    /// WS_EX_CLIENTEDGE, WS_EX_CONTEXTHELP.
    const FRAMED_EXTENDED: u32 = 0x0001 | 0x0080 | 0x0100 | 0x0200 | 0x0400;
    const WS_EX_TOPMOST: u32 = 0x0008;

    /// Whether DXGI makes a swap chain for this window: one of the styles
    /// above, or not topmost (the debug layer's "The specified output window
    /// has a style that is incompatible with DXGI", as AMD quoted it in
    /// doitsujin/dxvk#5258). AMD's Vulkan driver
    /// on Windows presents through a DXGI swap chain on the game's window:
    /// its vkCreateSwapchainKHR fails (VK_ERROR_UNKNOWN) for a window DXGI
    /// refuses.
    pub fn dxgi_takes(self) -> bool {
        self.style & WindowStyles::FRAMED != 0
            || self.extended & WindowStyles::FRAMED_EXTENDED != 0
            || self.extended & WindowStyles::WS_EX_TOPMOST == 0
    }

    /// The style that makes a window DXGI refuses one it takes: WS_POPUP
    /// added (DXVK's fullscreen keeps it: it takes away WS_OVERLAPPEDWINDOW's
    /// styles alone); None for a window DXGI takes.
    pub fn for_dxgi(self) -> Option<u32> {
        (!self.dxgi_takes()).then_some(self.style | WindowStyles::WS_POPUP)
    }
}

/// The game's output window (its HWND), as the render output it presents
/// with holds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputWindow(NonNull<c_void>);

impl OutputWindow {
    const GWL_STYLE: u32 = -16i32 as u32;
    const GWL_EXSTYLE: u32 = -20i32 as u32;

    /// At `render_output.present`: the window the game's swap chain presents
    /// to now.
    pub fn at_present(ctx: &Context<'_>) -> Option<OutputWindow> {
        // SAFETY: the context is this site's: ESI the render output present
        // runs on, alive while it runs on the game's main thread; its window field
        // is the constructor's, never written again.
        let window = unsafe { *raw::site::render_output_present::output(ctx)?.window() };
        NonNull::new(window as usize as *mut c_void).map(OutputWindow)
    }

    /// Its styles now (GetWindowLongA).
    pub fn styles(self, _: &Thread) -> WindowStyles {
        // SAFETY: GetWindowLongA through the game's delayed user32 slot (its
        // helper resolves it at the first call) with the game's window.
        unsafe {
            WindowStyles {
                style: (raw::engine_get_window_long_a)(self.0.as_ptr(), OutputWindow::GWL_STYLE),
                extended: (raw::engine_get_window_long_a)(self.0.as_ptr(), OutputWindow::GWL_EXSTYLE),
            }
        }
    }

    /// Its style set to `style` (SetWindowLongA, as the game's set_size and
    /// DXVK's fullscreen set it: from the window's own thread, so its
    /// WM_STYLECHANGING/WM_STYLECHANGED are sent within the call).
    pub fn set_style(self, _: &Thread, style: u32) {
        // SAFETY: SetWindowLongA through the game's delayed user32 slot with
        // the game's window; the old style it answers is not kept.
        unsafe {
            (raw::engine_set_window_long_a)(self.0.as_ptr(), OutputWindow::GWL_STYLE, style);
        }
    }
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
/// aligned): apiVersion, driverVersion, vendorID, deviceID and deviceType
/// its first five u32, deviceName (256 bytes) right after.
#[repr(C, align(8))]
struct Properties([u32; 256]);

impl Gpu {
    fn of_properties(room: &Properties) -> Gpu {
        let mut name = [0; GPU_NAME];
        for (i, byte) in name.iter_mut().enumerate() {
            *byte = (room.0[5 + i / 4] >> (8 * (i % 4))) as u8;
        }
        // a name of 64 bytes or more: cut, NUL-padded
        if let Some(end) = name.iter().position(|&b| b == 0) {
            name[end..].fill(0);
        }
        Gpu {
            vendor: room.0[2],
            device: room.0[3],
            driver: room.0[1],
            kind: GpuKind::of_device_type(room.0[4]),
            version: VulkanVersion::of_packed(room.0[0]),
            name,
        }
    }

    /// The same GPU as `other` (its vendor's and device's IDs), whatever
    /// driver gives it.
    pub fn same_gpu(&self, other: &Gpu) -> bool {
        self.vendor == other.vendor && self.device == other.device && self.name == other.name
    }
}

/// A Vulkan device as the look reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gpu {
    /// The PCI vendor (`vendorID`): 0x1002 AMD, 0x10DE NVIDIA, 0x8086 Intel.
    pub vendor: u32,
    /// The PCI device (`deviceID`).
    pub device: u32,
    /// `driverVersion`, the driver's own packing: two drivers of one GPU
    /// differ by it.
    pub driver: u32,
    /// `deviceType`.
    pub kind: GpuKind,
    /// `apiVersion`; None for another variant than 0.
    pub version: Option<VulkanVersion>,
    /// `deviceName`'s first bytes, NUL-padded.
    pub name: [u8; GPU_NAME],
}

/// A device's `deviceType` (VkPhysicalDeviceType).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuKind {
    /// 0, or a value Vulkan has no name for.
    Other,
    Integrated,
    Discrete,
    Virtual,
    /// A CPU's device (lavapipe, SwiftShader): never DXVK's.
    Cpu,
}

impl GpuKind {
    fn of_device_type(raw: u32) -> GpuKind {
        match raw {
            1 => GpuKind::Integrated,
            2 => GpuKind::Discrete,
            3 => GpuKind::Virtual,
            4 => GpuKind::Cpu,
            _ => GpuKind::Other,
        }
    }
}

/// DXVK 3.1.1's choice among adapters in their order (it sorts them so): the
/// first discrete one, else the first integrated, else the first virtual,
/// else the first.
pub fn dxvk_first<T: Copy>(items: impl Iterator<Item = T> + Clone, kind: impl Fn(T) -> GpuKind) -> Option<T> {
    [GpuKind::Discrete, GpuKind::Integrated, GpuKind::Virtual]
        .into_iter()
        .find_map(|wanted| items.clone().find(|&item| kind(item) == wanted))
        .or_else(|| items.clone().next())
}

/// How much of a device's name is kept.
pub const GPU_NAME: usize = 64;

impl Gpu {
    pub fn name(&self) -> &str {
        let len = self.name.iter().position(|&b| b == 0).unwrap_or(GPU_NAME);
        core::str::from_utf8(&self.name[..len]).unwrap_or("?")
    }
}

/// The look's devices, at most 16 (in the loader's order).
pub struct Gpus {
    pub list: [Option<Gpu>; 16],
}

/// The best version of a device that is not the CPU.
pub fn best_version(gpus: &Gpus) -> Vulkan {
    gpus.list
        .iter()
        .flatten()
        .filter(|gpu| gpu.kind != GpuKind::Cpu)
        .filter_map(|gpu| gpu.version)
        .max()
        .map_or(Vulkan::NoDevice, Vulkan::Device)
}

/// The device DXVK 3.1.1 makes its first adapter of (`DxvkInstance::
/// queryAdapters`, `DxvkDeviceFilter::testAdapter`): of those of at least
/// `needed` that are not the CPU and whose name has `filter` in it (its
/// `DXVK_FILTER_DEVICE_NAME`; empty, any), [`dxvk_first`]'s. The game makes
/// its device on DXGI's first adapter, this one.
pub fn dxvk_pick(gpus: &Gpus, needed: VulkanVersion, filter: &[u8]) -> Option<Gpu> {
    let name_has = |gpu: &Gpu| {
        let len = gpu.name.iter().position(|&b| b == 0).unwrap_or(GPU_NAME);
        filter.is_empty() || gpu.name[..len].windows(filter.len()).any(|part| part == filter)
    };
    let usable = gpus
        .list
        .iter()
        .flatten()
        .filter(|gpu| gpu.kind != GpuKind::Cpu && gpu.version.is_some_and(|v| v >= needed) && name_has(gpu));
    dxvk_first(usable, |gpu| gpu.kind).copied()
}

/// vulkan-1.dll loaded, an instance made, every device's properties read,
/// the instance destroyed and the DLL let go: the best version of them.
pub fn best_vulkan(thread: &Thread) -> Vulkan {
    match look(thread) {
        Ok(gpus) => best_version(&gpus),
        Err(failed) => failed,
    }
}

/// The devices the Vulkan loader gives now (its environment as it is), the
/// loader let go after; Err the look's failure.
pub fn look(thread: &Thread) -> Result<Gpus, Vulkan> {
    look_with(thread, None)
}

/// As [`look`], and the manifests of the Vulkan drivers the instance loaded
/// into `manifests` (each `.json` beside a DLL of the process that exports
/// `vk_icdNegotiateLoaderICDInterfaceVersion`, while the instance lives).
pub fn look_drivers(thread: &Thread, manifests: &mut Manifests) -> Result<Gpus, Vulkan> {
    look_with(thread, Some(manifests))
}

/// The longest manifest path kept (MAX_PATH, NUL included): the ANSI calls
/// that find them give no longer.
const MANIFEST_ROOM: usize = 260;

/// A Vulkan driver's manifest by its full path, NUL-terminated (what
/// `VK_DRIVER_FILES` takes).
#[derive(Clone, Copy)]
pub struct Manifest {
    path: [u8; MANIFEST_ROOM],
    len: usize,
}

impl Manifest {
    pub fn c(&self) -> &CStr {
        CStr::from_bytes_until_nul(&self.path).unwrap_or(c"")
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.path[..self.len]).unwrap_or("?")
    }
}

/// The manifests a look found, at most 8, in the order found.
#[derive(Default)]
pub struct Manifests {
    pub list: [Option<Manifest>; 8],
}

impl Manifests {
    fn add(&mut self, path: &[u8]) {
        // a NUL inside would cut it short for the loader
        if path.len() >= MANIFEST_ROOM
            || path.contains(&0)
            || self.list.iter().flatten().any(|known| known.path[..known.len].eq_ignore_ascii_case(path))
        {
            return;
        }
        if let Some(free) = self.list.iter_mut().find(|slot| slot.is_none()) {
            let mut manifest = Manifest { path: [0; MANIFEST_ROOM], len: path.len() };
            manifest.path[..path.len()].copy_from_slice(path);
            *free = Some(manifest);
        }
    }
}

/// WIN32_FIND_DATAA: dwFileAttributes first, cFileName (260 bytes) at 44.
#[repr(C, align(4))]
struct FindData([u8; 320]);

/// Into `manifests`, every `.json` in the folder of each Vulkan driver DLL
/// loaded now (K32EnumProcessModules through kernel32's GetProcAddress; a
/// driver exports vk_icdNegotiateLoaderICDInterfaceVersion).
///
/// # Safety
/// Called while the loader's instance lives, on the thread that made it.
unsafe fn driver_manifests(manifests: &mut Manifests) {
    const FILE_ATTRIBUTE_DIRECTORY: u8 = 0x10;
    unsafe {
        let kernel = (raw::engine_get_module_handle_a)(c"kernel32.dll".as_ptr().cast());
        let Some(enumerate) = NonNull::new((raw::engine_get_proc_address)(kernel, c"K32EnumProcessModules".as_ptr().cast()))
        else {
            return;
        };
        let mut modules = [null_mut(); 512];
        let mut needed = 0u32;
        // GetCurrentProcess's pseudo-handle, -1
        let process = usize::MAX as *mut c_void;
        if raw::engine_enum_process_modules(enumerate, process, modules.as_mut_ptr(), size_of_val(&modules) as u32, &mut needed) == 0 {
            return;
        }
        for &module in &modules[..(needed as usize / size_of::<*mut c_void>()).min(modules.len())] {
            if (raw::engine_get_proc_address)(module, c"vk_icdNegotiateLoaderICDInterfaceVersion".as_ptr().cast()).is_null() {
                continue;
            }
            let mut folder = Path { bytes: [0; PATH_ROOM], len: 0 };
            let len = (raw::engine_get_module_file_name_a)(module, folder.bytes.as_mut_ptr(), PATH_ROOM as u32) as usize;
            let Some(slash) = (len > 0 && len < PATH_ROOM).then(|| folder.bytes[..len].iter().rposition(|&b| b == b'\\')).flatten()
            else {
                continue;
            };
            folder.len = slash + 1;
            let mut pattern = Path { bytes: folder.bytes, len: folder.len };
            if pattern.push(b"*.json").is_none() {
                continue;
            }
            let mut data = FindData([0; 320]);
            let find = (raw::engine_find_first_file_a)(pattern.c(), (&raw mut data).cast());
            if find as isize == -1 {
                continue;
            }
            loop {
                if data.0[0] & FILE_ATTRIBUTE_DIRECTORY == 0 {
                    let name = &data.0[44..44 + 260];
                    let name = &name[..name.iter().position(|&b| b == 0).unwrap_or(260)];
                    let mut path = Path { bytes: folder.bytes, len: folder.len };
                    if path.push(name).is_some() {
                        manifests.add(&path.bytes[..path.len]);
                    }
                }
                if (raw::engine_find_next_file_a)(find, (&raw mut data).cast()) == 0 {
                    break;
                }
            }
            (raw::engine_find_close)(find);
        }
    }
}

fn look_with(_: &Thread, manifests: Option<&mut Manifests>) -> Result<Gpus, Vulkan> {
    // SAFETY: each function is the GetProcAddress result of its own name in
    // the loader, called with vulkan_core.h's structures; every handle made
    // here is destroyed or let go here.
    unsafe {
        let library = (raw::engine_load_library_a)(c"vulkan-1.dll".as_ptr().cast());
        if library.is_null() {
            return Err(Vulkan::NoLoader);
        }
        let function = |name: &CStr| NonNull::new((raw::engine_get_proc_address)(library, name.as_ptr().cast()));
        let (Some(create), Some(destroy), Some(enumerate), Some(properties)) = (
            function(c"vkCreateInstance"),
            function(c"vkDestroyInstance"),
            function(c"vkEnumeratePhysicalDevices"),
            function(c"vkGetPhysicalDeviceProperties"),
        ) else {
            (raw::engine_free_library)(library);
            return Err(Vulkan::NoLoader);
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
            return Err(Vulkan::NoInstance);
        }
        let mut devices = [null_mut(); 16];
        let mut count = devices.len() as u32;
        let mut gpus = Gpus { list: [None; 16] };
        // VK_INCOMPLETE (5) with more than 16: these 16 are enough; an error is negative
        if (raw::engine_vk_enumerate_physical_devices(enumerate, instance, &mut count, devices.as_mut_ptr()) as i32) >= 0 {
            for (slot, &device) in gpus.list.iter_mut().zip(&devices[..(count as usize).min(devices.len())]) {
                let mut room = Properties([0; 256]);
                raw::engine_vk_get_physical_device_properties(properties, device, (&raw mut room).cast());
                *slot = Some(Gpu::of_properties(&room));
            }
        }
        if let Some(manifests) = manifests {
            driver_manifests(manifests);
        }
        raw::engine_vk_destroy_instance(destroy, instance, null());
        (raw::engine_free_library)(library);
        Ok(gpus)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gpu(vendor: u32, kind: GpuKind, minor: u32, name: &str) -> Option<Gpu> {
        let mut bytes = [0; GPU_NAME];
        bytes[..name.len()].copy_from_slice(name.as_bytes());
        Some(Gpu { vendor, device: 0, driver: 0, kind, version: Some(VulkanVersion::new(1, minor, 0)), name: bytes })
    }

    fn gpus(list: &[Option<Gpu>]) -> Gpus {
        let mut gpus = Gpus { list: [None; 16] };
        gpus.list[..list.len()].copy_from_slice(list);
        gpus
    }

    const NEEDED: VulkanVersion = VulkanVersion::new(1, 4, 0);

    #[test]
    fn dxvk_takes_the_first_discrete_gpu_then_an_integrated_one() {
        let amd_igpu = gpu(0x1002, GpuKind::Integrated, 4, "AMD Radeon(TM) Graphics");
        let rtx = gpu(0x10DE, GpuKind::Discrete, 4, "NVIDIA GeForce RTX 5090");
        let pick = |list: &[Option<Gpu>], filter: &[u8]| dxvk_pick(&gpus(list), NEEDED, filter).map(|g| g.vendor);
        assert_eq!(pick(&[amd_igpu, rtx], b""), Some(0x10DE), "discrete before integrated, whatever the order");
        assert_eq!(pick(&[amd_igpu], b""), Some(0x1002), "an integrated GPU alone");
        assert_eq!(pick(&[amd_igpu, rtx], b"AMD"), Some(0x1002), "DXVK_FILTER_DEVICE_NAME, a part of the name");
        assert_eq!(pick(&[amd_igpu, rtx], b"Intel"), None);
        let old = gpu(0x10DE, GpuKind::Discrete, 3, "NVIDIA GeForce GTX 1060");
        assert_eq!(pick(&[old, amd_igpu], b""), Some(0x1002), "a device under Vulkan 1.4 is not DXVK's");
        let cpu = gpu(0x10005, GpuKind::Cpu, 4, "llvmpipe");
        assert_eq!(pick(&[cpu], b""), None, "never the CPU");
        assert_eq!(best_version(&gpus(&[cpu, old])), Vulkan::Device(VulkanVersion::new(1, 3, 0)));
    }

    const WS_VISIBLE: u32 = 0x1000_0000;
    const WS_CLIPSIBLINGS: u32 = 0x0400_0000;
    const WS_CAPTION: u32 = 0x00C0_0000;
    const WS_SYSMENU: u32 = 0x0008_0000;
    const WS_EX_TOPMOST: u32 = 0x0008;
    const WS_EX_WINDOWEDGE: u32 = 0x0100;
    /// WS_OVERLAPPEDWINDOW, WS_EX_OVERLAPPEDWINDOW: what DXVK 3.1.1's
    /// enterFullscreenMode takes away (wsi_window_win32.cpp).
    const OVERLAPPEDWINDOW: u32 = 0x00CF_0000;
    const EX_OVERLAPPEDWINDOW: u32 = 0x0300;

    fn styles(style: u32, extended: u32) -> WindowStyles {
        WindowStyles { style, extended }
    }

    /// DXVK's fullscreen of a window: its frame styles gone, topmost.
    fn dxvk_fullscreen(window: WindowStyles) -> WindowStyles {
        styles(window.style & !OVERLAPPEDWINDOW, (window.extended & !EX_OVERLAPPEDWINDOW) | WS_EX_TOPMOST)
    }

    #[test]
    fn the_games_window_in_dxvks_fullscreen_is_refused_and_a_popup_is_taken() {
        // CreateWindowExA(0, .., WS_CAPTION | WS_SYSMENU, ..) at 0x186FF; Windows adds
        // WS_CLIPSIBLINGS and WS_EX_WINDOWEDGE to such a window, show() WS_VISIBLE
        let windowed = styles(WS_CAPTION | WS_SYSMENU | WS_CLIPSIBLINGS | WS_VISIBLE, WS_EX_WINDOWEDGE);
        assert!(windowed.dxgi_takes() && windowed.for_dxgi().is_none());
        let fullscreen = dxvk_fullscreen(windowed);
        assert_eq!(fullscreen, styles(WS_CLIPSIBLINGS | WS_VISIBLE, WS_EX_TOPMOST));
        assert!(!fullscreen.dxgi_takes());
        let popup = fullscreen.for_dxgi().unwrap();
        assert_eq!(popup, WindowStyles::WS_POPUP | WS_CLIPSIBLINGS | WS_VISIBLE);
        let fixed = styles(popup, fullscreen.extended);
        assert!(fixed.dxgi_takes() && fixed.for_dxgi().is_none());
        // DXVK entering fullscreen again keeps the popup
        assert_eq!(dxvk_fullscreen(fixed), fixed);
    }

    #[test]
    fn the_games_own_fullscreen_style_is_refused() {
        // set_size: SetWindowLongA(window, GWL_STYLE, 0) then SetWindowPos(HWND_TOPMOST, .., SWP_SHOWWINDOW)
        let fullscreen = styles(WS_VISIBLE, WS_EX_TOPMOST);
        assert!(!fullscreen.dxgi_takes());
        assert_eq!(fullscreen.for_dxgi(), Some(WindowStyles::WS_POPUP | WS_VISIBLE));
    }

    #[test]
    fn any_frame_style_or_no_topmost_is_taken() {
        assert!(styles(WS_VISIBLE, 0).dxgi_takes());
        for style in [WindowStyles::WS_POPUP, 0x0080_0000, WS_SYSMENU, 0x0004_0000, 0x0002_0000, 0x0001_0000] {
            assert!(styles(style, WS_EX_TOPMOST).dxgi_takes(), "{style:#x}");
        }
        for extended in [0x0001, 0x0080, WS_EX_WINDOWEDGE, 0x0200, 0x0400] {
            assert!(styles(0, WS_EX_TOPMOST | extended).dxgi_takes(), "{extended:#x}");
        }
        // WS_DLGFRAME alone is not in DXGI's list: refused, a popup given
        assert!(!styles(0x0040_0000, WS_EX_TOPMOST).dxgi_takes());
    }
}
