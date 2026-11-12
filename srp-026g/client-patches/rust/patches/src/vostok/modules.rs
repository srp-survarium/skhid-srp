//! What is inside the game's process, for patch 125's report
//! (`srp_validate::modules`):
//! - the loader's modules, by kernel32's Toolhelp walk (`CreateToolhelp32Snapshot`,
//!   `Module32FirstW`/`NextW`), each one's folder told from the game's (the
//!   exe's, the walk's first entry), the temporary and the Windows folder;
//! - then the address space, by `VirtualQuery` from 0 to its end: each
//!   allocation that runs code and is no module of that list (`CodeScan`),
//!   where a DLL mapped by hand runs;
//! - what the watch kept since the game's start ([`super::module_watch`]):
//!   the DLLs the loader said it loaded;
//! - the other processes' windows that are always on top and over the
//!   game's ([`windows_over`]): overlays.
//!
//! Memory is read only with `ReadProcessMemory`, which fails instead of
//! faulting on a page made unreadable or freed meanwhile. The kernel32
//! functions are the library's dynamic rows.

use super::world::read;
use super::{Context, Thread, module_watch};
use crate::raw;
use core::ffi::{CStr, c_void};
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering};
use srp_validate::modules::{
    Affinity, CodeScan, Entry, Holder, Home, Module, Moment, Name, Over, PeMarks, Place, Rect, Region, Report, Unchecked, Unseen,
    START,
};

/// kernel32's `MODULEENTRY32W` (32-bit: 1064 bytes).
#[repr(C)]
struct ModuleEntry {
    size: u32,
    module_id: u32,
    process_id: u32,
    global_usage: u32,
    process_usage: u32,
    base: *const u8,
    base_size: u32,
    module: *mut c_void,
    name: [u16; 256],
    path: [u16; 260],
}

/// kernel32's `MEMORY_BASIC_INFORMATION` (32-bit: 28 bytes).
#[repr(C)]
#[derive(Default)]
pub(super) struct MemoryInfo {
    base: usize,
    allocation: usize,
    allocation_protect: u32,
    size: usize,
    state: u32,
    protect: u32,
    kind: u32,
}

const SNAP_MODULE: u32 = 8;
const SNAP_PROCESS: u32 = 2;
/// Windows over the game's told at most, the topmost first.
const MAX_OVER: usize = 16;
/// Windows walked at most from the top of their order (a desktop has hundreds).
const MAX_WINDOWS: usize = 4096;

/// kernel32's `PROCESSENTRY32W` (32-bit: 556 bytes).
#[repr(C)]
struct ProcessEntry {
    size: u32,
    usage: u32,
    process: u32,
    heap: usize,
    module: u32,
    threads: u32,
    parent: u32,
    priority: i32,
    flags: u32,
    file: [u16; 260],
}
/// `MAX_PATH` and room for its terminator.
pub(super) const PATH: usize = 261;
/// The module bases kept to tell a listed image from an unlisted one.
const MAX_BASES: usize = 1024;
/// Regions walked at most (a 32-bit process has some thousands).
const MAX_REGIONS: usize = 1 << 18;

/// A wide path a kernel32 call wrote: its units before the terminator.
#[derive(Clone, Copy)]
pub(super) struct Path {
    units: [u16; PATH],
    len: usize,
}
impl Path {
    pub(super) const NONE: Path = Path { units: [0; PATH], len: 0 };
    /// `written` as the call returned it: 0 is a failure, as much as the
    /// room or more a path that did not fit; both are no folder.
    pub(super) fn written(units: [u16; PATH], written: u32) -> Path {
        let len = written as usize;
        if len == 0 || len >= PATH { Path::NONE } else { Path { units, len } }
    }
    fn of_entry(entry: &ModuleEntry) -> &[u16] {
        let len = entry.path.iter().position(|&unit| unit == 0).unwrap_or(entry.path.len());
        entry.path.get(..len).unwrap_or_default()
    }
    pub(super) fn as_units(&self) -> &[u16] { self.units.get(..self.len).unwrap_or_default() }

    /// The folder of the file this is the path of.
    pub(super) fn folder(&self) -> Path {
        let mut folder = *self;
        folder.len = folder_of(self.as_units()).len();
        folder
    }
}

/// The folder of a file's path: its units up to and with its last `\`.
fn folder_of(path: &[u16]) -> &[u16] {
    let end = path.iter().rposition(|&unit| unit == 0x5C || unit == 0x2F).map_or(0, |at| at + 1);
    path.get(..end).unwrap_or_default()
}

/// Where the listed modules start, to tell an image the loader does not
/// list. `TooMany`: more than could be kept, so no image is told as unlisted.
pub(super) enum Bases {
    Known { bases: [usize; MAX_BASES], len: usize },
    TooMany,
}
impl Bases {
    fn add(&mut self, base: usize) {
        if let Bases::Known { bases, len } = self {
            match bases.get_mut(*len) {
                Some(slot) => { *slot = base; *len += 1; }
                None => *self = Bases::TooMany,
            }
        }
    }
    pub(super) fn listed(&self, base: usize) -> bool {
        match self {
            Bases::Known { bases, len } => bases.get(..*len).unwrap_or_default().contains(&base),
            Bases::TooMany => true,
        }
    }
}

/// The kernel32 functions the walks call, from its own exports.
pub(super) struct Kernel32 {
    module: *mut c_void,
    module_file_name: NonNull<c_void>,
    snapshot: NonNull<c_void>,
    first: NonNull<c_void>,
    next: NonNull<c_void>,
    temp_path: NonNull<c_void>,
    windows_directory: NonNull<c_void>,
    current_process: NonNull<c_void>,
    read_memory: NonNull<c_void>,
}
impl Kernel32 {
    pub(super) fn get() -> Option<Kernel32> {
        // SAFETY: GetModuleHandleA and GetProcAddress (the exe's imports)
        // with NUL-terminated names; kernel32 is in every process and never
        // unloaded, so its exports stay valid.
        unsafe {
            let kernel32 = (raw::engine_get_module_handle_a)(c"kernel32.dll".as_ptr().cast());
            if kernel32.is_null() { return None; }
            let function = |name: &CStr| NonNull::new((raw::engine_get_proc_address)(kernel32, name.as_ptr().cast()));
            Some(Kernel32 {
                module: kernel32,
                module_file_name: function(c"GetModuleFileNameW")?,
                snapshot: function(c"CreateToolhelp32Snapshot")?,
                first: function(c"Module32FirstW")?,
                next: function(c"Module32NextW")?,
                temp_path: function(c"GetTempPathW")?,
                windows_directory: function(c"GetWindowsDirectoryW")?,
                current_process: function(c"GetCurrentProcess")?,
                read_memory: function(c"ReadProcessMemory")?,
            })
        }
    }

    /// Where kernel32 has the function `name` (0: it has none).
    pub(super) fn export(&self, name: &CStr) -> usize {
        // SAFETY: GetProcAddress (the exe's import) on kernel32's module, a NUL-terminated name.
        unsafe { (raw::engine_get_proc_address)(self.module, name.as_ptr().cast()) as usize }
    }

    /// The full path of the file of the image at `module` (null: the exe's).
    pub(super) fn file_of(&self, module: *mut c_void) -> Path {
        let mut units = [0; PATH];
        // SAFETY: GetModuleFileNameW's own export, given its buffer's room; a module that is none fails.
        let written = unsafe { raw::engine_get_module_file_name_w(self.module_file_name, module, units.as_mut_ptr(), PATH as u32) };
        Path::written(units, written)
    }

    pub(super) fn temp(&self) -> Path {
        let mut units = [0; PATH];
        // SAFETY: GetTempPathW's own export, given its buffer's room.
        let written = unsafe { raw::engine_get_temp_path_w(self.temp_path, PATH as u32, units.as_mut_ptr()) };
        Path::written(units, written)
    }

    pub(super) fn windows(&self) -> Path {
        let mut units = [0; PATH];
        // SAFETY: GetWindowsDirectoryW's own export, given its buffer's room.
        let written = unsafe { raw::engine_get_windows_directory_w(self.windows_directory, units.as_mut_ptr(), PATH as u32) };
        Path::written(units, written)
    }

    /// `N` bytes of this process at `address`, or `None` where they cannot
    /// be read. The address is only handed to kernel32, never dereferenced
    /// here.
    fn read<const N: usize>(&self, address: usize) -> Option<[u8; N]> {
        let mut bytes = [0; N];
        let mut read = 0;
        // SAFETY: GetCurrentProcess's pseudo-handle and ReadProcessMemory's
        // own export: it copies into `bytes` (N bytes of room) or fails,
        // never faulting on the address.
        let done = unsafe {
            let process = raw::engine_get_current_process(self.current_process);
            raw::engine_read_process_memory(
                self.read_memory, process, address as *const c_void, bytes.as_mut_ptr().cast(), N as u32, &mut read,
            )
        };
        (done != 0 && read as usize == N).then_some(bytes)
    }

    /// The build marks of a PE image at `base`, if a 32-bit PE header is there.
    pub(super) fn pe_marks(&self, base: usize) -> Option<PeMarks> {
        PeMarks::of(&self.read::<0x40>(base)?, |at| self.read::<92>(base.checked_add(at as usize)?))
    }

    /// The region `address` is in; `None` past the address space's end.
    pub(super) fn query(&self, address: usize) -> Option<MemoryInfo> {
        Kernel32::region(address)
    }

    /// As `query`, which needs none of kernel32's own exports.
    pub(super) fn region(address: usize) -> Option<MemoryInfo> {
        let mut info = MemoryInfo::default();
        // SAFETY: VirtualQuery (the exe's import), given the structure's
        // size; it only describes the address, never touches it.
        let written = unsafe {
            (raw::engine_virtual_query)(address as *const c_void, (&raw mut info).cast(), size_of::<MemoryInfo>() as u32)
        };
        (written as usize == size_of::<MemoryInfo>()).then_some(info)
    }
}

/// The loader's modules outside the Windows folder, in Toolhelp's order (the
/// exe first), into `report`; where every listed module starts.
fn modules(kernel32: &Kernel32, report: &mut Report) -> Option<Bases> {
    let (temp, windows) = (kernel32.temp(), kernel32.windows());
    // SAFETY: CreateToolhelp32Snapshot's own export for this process (0);
    // the handle is closed below on every path that made one.
    let snapshot = unsafe { raw::engine_create_toolhelp32_snapshot(kernel32.snapshot, SNAP_MODULE, 0) };
    if snapshot.is_null() || snapshot as usize == usize::MAX { return None; }
    let mut entry = ModuleEntry {
        size: size_of::<ModuleEntry>() as u32, module_id: 0, process_id: 0, global_usage: 0, process_usage: 0,
        base: core::ptr::null(), base_size: 0, module: core::ptr::null_mut(), name: [0; 256], path: [0; 260],
    };
    let mut bases = Bases::Known { bases: [0; MAX_BASES], len: 0 };
    let mut game = Path::NONE;
    // SAFETY: Module32FirstW/NextW's own exports on the snapshot made above,
    // each given the entry with its size set; they write only into it.
    let mut more = unsafe { raw::engine_module32_first_w(kernel32.first, snapshot, (&raw mut entry).cast()) } != 0;
    while more {
        let path = Path::of_entry(&entry);
        let base = entry.base as usize;
        bases.add(base);
        if game.len == 0 {
            let folder = folder_of(path);
            game.units.iter_mut().zip(folder).for_each(|(unit, &from)| *unit = from);
            game.len = folder.len().min(PATH);
        }
        if let (Some(place), Some(name)) =
            (Place::of(path, game.as_units(), temp.as_units(), windows.as_units()), Name::of_path(path))
        {
            report.add(Entry::Module(Module { name, place, image_size: entry.base_size, pe: kernel32.pe_marks(base) }));
        }
        // SAFETY: as Module32FirstW.
        more = unsafe { raw::engine_module32_next_w(kernel32.next, snapshot, (&raw mut entry).cast()) } != 0;
    }
    // SAFETY: the snapshot's own handle, closed once.
    unsafe { (raw::engine_close_handle)(snapshot) };
    Some(bases)
}

/// The executable allocations that are no listed module, into `report`.
fn code(kernel32: &Kernel32, bases: &Bases, report: &mut Report) {
    let mut scan = CodeScan::default();
    let mut tell = |found: Option<srp_validate::modules::Found>| {
        if let Some(found) = found {
            let start = kernel32.read::<START>(found.first_exec);
            report.add(Entry::Code(found.code(start.as_ref(), kernel32.pe_marks(found.base))));
        }
    };
    let mut address = 0usize;
    for _ in 0..MAX_REGIONS {
        let Some(info) = kernel32.query(address) else { break };
        let region = Region {
            base: info.base, allocation: info.allocation, size: info.size,
            state: info.state, protect: info.protect, kind: info.kind,
        };
        tell(scan.region(region, |base| bases.listed(base)));
        match info.base.checked_add(info.size) {
            Some(next) if next > address => address = next,
            _ => break,
        }
    }
    tell(scan.end());
}

/// The window the game's swap chain presents to, as its last Present named
/// it (0: none yet). A handle only: user32 refuses one that is no window
/// any more.
static WINDOW: AtomicUsize = AtomicUsize::new(0);

/// render_output.present: the window the game is about to present to.
pub fn presenting(ctx: &Context<'_>) {
    // SAFETY: the context is this site's: ESI the render output, alive in its own present.
    let window = unsafe { raw::site::render_output_present::output(ctx).map(|output| read(output.window())) };
    if let Some(window) = window {
        WINDOW.store(window as usize, Ordering::Relaxed);
    }
}

/// A user32 export (`GetModuleHandleA` + `GetProcAddress`); user32 is the
/// exe's import, loaded and never unloaded.
fn user32(name: &CStr) -> Option<NonNull<c_void>> {
    export_of(c"user32.dll", name)
}

/// The window `window` as an [`Over`] if it is a window of another process
/// that is visible, always on top ([`Over::TOPMOST`]) and over the game's
/// window now; `None` otherwise (the game has no window yet, `window` is the
/// game's own, it is gone, or it is not over the game). For the event watch
/// ([`super::module_watch`]): one window tested when a window event tells of
/// it. The returned [`Over::since`]/[`Over::until`] are the watch's to set.
pub(super) fn over_now(window: *mut c_void) -> Option<Over> {
    const ROOT: u32 = 2;
    const GWL_EXSTYLE: u32 = -20i32 as u32;
    const CLASS: usize = 64;
    let kernel32 = Kernel32::get()?;
    let presented = NonNull::new(WINDOW.load(Ordering::Relaxed) as *mut c_void)?;
    let (root, visible, process_of, rect_of, class_of) =
        (user32(c"GetAncestor")?, user32(c"IsWindowVisible")?, user32(c"GetWindowThreadProcessId")?, user32(c"GetWindowRect")?, user32(c"GetClassNameW")?);
    let affinity_of = user32(c"GetWindowDisplayAffinity");
    let rect = |window: *mut c_void| {
        let mut rect = Rect::default();
        // SAFETY: GetWindowRect's own export; fails on what is no window.
        (unsafe { raw::engine_get_window_rect(rect_of, window, (&raw mut rect).cast()) } != 0).then_some(rect)
    };
    // SAFETY: GetAncestor's own export; null for what is no window. GetCurrentProcessId (the exe's import).
    let (game, own) = unsafe { (raw::engine_get_ancestor(root, presented.as_ptr(), ROOT), (raw::engine_get_current_process_id)()) };
    if window.is_null() || window == game {
        return None;
    }
    let mut process = 0;
    // SAFETY: user32's own exports on a window handle; one that is no window answers 0.
    let (thread, shown, style) = unsafe {
        (
            raw::engine_get_window_thread_process_id(process_of, window, &mut process),
            raw::engine_is_window_visible(visible, window) != 0,
            (raw::engine_get_window_long_a)(window, GWL_EXSTYLE),
        )
    };
    if !(thread != 0 && process != own && shown && style & Over::TOPMOST != 0) {
        return None;
    }
    let (width, height) = rect(window).zip(rect(game)).and_then(|(window, game)| window.overlap(game))?;
    let mut class = [0; CLASS];
    // SAFETY: GetClassNameW writes at most CLASS units into `class`.
    let len = unsafe { raw::engine_get_class_name_w(class_of, window, class.as_mut_ptr(), CLASS as u32) } as usize;
    let class = Name::of(class.get(..len).unwrap_or_default());
    let affinity = match affinity_of {
        None => Affinity::None,
        Some(affinity_of) => {
            let mut value = 0u32;
            // SAFETY: GetWindowDisplayAffinity's own export; fails on what is no window.
            let ok = unsafe { raw::engine_get_window_display_affinity(affinity_of, window, &mut value) } != 0;
            if ok { Affinity::of(value) } else { Affinity::None }
        }
    };
    let name = process_names(&kernel32, &[Some(process)])[0];
    let (folder, temp, windows) = (kernel32.file_of(core::ptr::null_mut()).folder(), kernel32.temp(), kernel32.windows());
    let home = exe_of(&kernel32, process)
        .map_or(Home::Unknown, |exe| Home::of(exe.as_units(), folder.as_units(), temp.as_units(), windows.as_units()));
    Some(Over { name, class, style, width, height, affinity, home, since: 0, until: None })
}

/// The exe file name of each process of `ids`, from the process list
/// (`None`: it lists none, or cannot be had).
fn process_names<const N: usize>(kernel32: &Kernel32, ids: &[Option<u32>; N]) -> [Option<Name>; N] {
    let function = |name: &CStr| NonNull::new(kernel32.export(name) as *mut c_void);
    let mut names = [None; N];
    let (Some(first), Some(next)) = (function(c"Process32FirstW"), function(c"Process32NextW")) else { return names };
    // SAFETY: CreateToolhelp32Snapshot's own export, of the processes; closed below if one was made.
    let snapshot = unsafe { raw::engine_create_toolhelp32_snapshot(kernel32.snapshot, SNAP_PROCESS, 0) };
    if snapshot.is_null() || snapshot as usize == usize::MAX {
        return names;
    }
    let mut entry = ProcessEntry {
        size: size_of::<ProcessEntry>() as u32, usage: 0, process: 0, heap: 0, module: 0, threads: 0, parent: 0,
        priority: 0, flags: 0, file: [0; 260],
    };
    // SAFETY: Process32FirstW/NextW's own exports on the snapshot made above, each given
    // the entry with its size set; they write only into it.
    let mut more = unsafe { raw::engine_process32_first_w(first, snapshot, (&raw mut entry).cast()) } != 0;
    while more {
        let len = entry.file.iter().position(|&unit| unit == 0).unwrap_or(entry.file.len());
        for (name, _) in names.iter_mut().zip(ids).filter(|(_, id)| **id == Some(entry.process)) {
            *name = Name::of_path(entry.file.get(..len).unwrap_or_default());
        }
        // SAFETY: as Process32FirstW.
        more = unsafe { raw::engine_process32_next_w(next, snapshot, (&raw mut entry).cast()) } != 0;
    }
    // SAFETY: the snapshot's own handle, closed once.
    unsafe { (raw::engine_close_handle)(snapshot) };
    names
}

/// The full path of the exe of the process `process`, asked of Windows
/// through a handle that can ask nothing else of the process
/// (`PROCESS_QUERY_LIMITED_INFORMATION`: not its memory, not its handles),
/// closed at once. `None`: Windows refuses the handle or the name.
fn exe_of(kernel32: &Kernel32, process: u32) -> Option<Path> {
    /// `PROCESS_QUERY_LIMITED_INFORMATION`: the least right a handle can have.
    const ASK: u32 = 0x1000;
    let function = |name: &CStr| NonNull::new(kernel32.export(name) as *mut c_void);
    let (open, name_of) = (function(c"OpenProcess")?, function(c"QueryFullProcessImageNameW")?);
    // SAFETY: OpenProcess's own export; null when Windows refuses.
    let handle = NonNull::new(unsafe { raw::engine_open_process(open, ASK, 0, process) })?;
    let mut units = [0; PATH];
    let mut len = PATH as u32;
    // SAFETY: QueryFullProcessImageNameW's own export on the handle opened above: it writes at
    // most `len` units into `units` and the units written into `len`; then the handle is closed, once.
    let named = unsafe {
        let named = raw::engine_query_full_process_image_name_w(name_of, handle.as_ptr(), 0, units.as_mut_ptr(), &mut len);
        (raw::engine_close_handle)(handle.as_ptr());
        named
    };
    (named != 0).then(|| Path::written(units, len)).filter(|path| !path.as_units().is_empty())
}

/// `GetModuleHandleA` + `GetProcAddress` for a loaded module's export.
/// `None`: the module is not loaded or has no such export.
pub(super) fn export_of(module: &CStr, name: &CStr) -> Option<NonNull<c_void>> {
    // SAFETY: the exe's imports, NUL-terminated names; the module is one the
    // exe keeps loaded (kernel32, ntdll, user32), never unloaded.
    unsafe {
        let module = (raw::engine_get_module_handle_a)(module.as_ptr().cast());
        if module.is_null() { None } else { NonNull::new((raw::engine_get_proc_address)(module, name.as_ptr().cast())) }
    }
}

/// Holders of a handle to the game process told at most, and holders the
/// game could not check told at most (each by holder exe).
const MAX_HOLDERS: usize = 16;
const MAX_UNCHECKED: usize = 16;
/// Process-object handles the scan duplicates to check at most. Windows 11
/// 24H2 zeroes the object address for a caller without the debug right, so a
/// match by address is not possible and each handle is a
/// duplicate-and-compare; beyond this budget a holder's remaining handles are
/// told unchecked, never silently dropped.
const DUP_BUDGET: usize = 512;
/// The system's extended handle list (`SystemExtendedHandleInformation`).
const EXTENDED_HANDLE_INFO: u32 = 64;
/// `STATUS_INFO_LENGTH_MISMATCH` (0xC0000004): the buffer was too small.
const STATUS_LENGTH_MISMATCH: u32 = (0xC000 << 16) | 4;
/// One `SYSTEM_HANDLE_TABLE_ENTRY_INFO_EX` (32-bit: 28 bytes); the list's
/// two-word header is before the first.
const HANDLE_HEADER: usize = 8;
const HANDLE_ENTRY: usize = 28;

/// What a scan of the system handle list found: the other processes with a
/// handle to the game's own process object that can read its memory
/// ([`Holder`]), and the process-object handles in holders the game could
/// not check ([`Unchecked`]).
pub(super) struct Holders {
    pub(super) holders: [Option<Holder>; MAX_HOLDERS],
    pub(super) unchecked: [Option<Unchecked>; MAX_UNCHECKED],
}

impl Holders {
    const EMPTY: Holders = Holders { holders: [None; MAX_HOLDERS], unchecked: [None; MAX_UNCHECKED] };
}

/// A process id and the access it was granted, or a count: a small tally the
/// scan keeps without the heap, one row per holder process.
#[derive(Clone, Copy)]
struct Tally {
    rows: [(u32, u32); MAX_HOLDERS],
    len: usize,
}

impl Tally {
    const NONE: Tally = Tally { rows: [(0, 0); MAX_HOLDERS], len: 0 };

    /// Merges `value` into the row of `pid` (ORs an access mask, or adds a
    /// count), or starts a row; a tally that is full drops the rest.
    fn merge(&mut self, pid: u32, value: u32, add: bool) {
        if let Some(row) = self.rows.get_mut(..self.len).and_then(|rows| rows.iter_mut().find(|row| row.0 == pid)) {
            row.1 = if add { row.1.saturating_add(value) } else { row.1 | value };
        } else if let Some(slot) = self.rows.get_mut(self.len) {
            *slot = (pid, value);
            self.len += 1;
        }
    }
}

/// `N` bytes of the scan buffer at `at` as a little-endian number; 0 past the
/// buffer's committed length `size`.
fn read_word(buffer: NonNull<c_void>, size: usize, at: usize, n: usize) -> u32 {
    if at.checked_add(n).is_none_or(|end| end > size) {
        return 0;
    }
    // SAFETY: `at + n <= size`, the committed length we allocated and asked
    // NtQuerySystemInformation to fill; we read our own memory, never deref a
    // handle or object value.
    unsafe {
        let mut bytes = [0u8; 4];
        core::ptr::copy_nonoverlapping(buffer.as_ptr().cast::<u8>().add(at), bytes.as_mut_ptr(), n.min(4));
        u32::from_le_bytes(bytes)
    }
}

/// The other processes that hold a handle to the game's own process object
/// with memory access ([`Holders`]). The system handle list is asked
/// (`NtQuerySystemInformation(SystemExtendedHandleInformation)`), the buffer
/// grown on `STATUS_INFO_LENGTH_MISMATCH`; the game's process-object type and
/// its own object are learnt from a handle the game opens to itself; another
/// process's entry of that type is the game's by its object address where the
/// kernel gives one, else by duplicating the handle into the game and asking
/// its process id. A handle whose holder the game cannot open, or whose
/// object the kernel zeroed past the duplicate budget, is told unchecked.
///
/// The scan reads the system-wide handle table and may duplicate handles, so
/// it is not cheap; it runs only at a report (a match's load, its end, a
/// minute), never on an ordinary frame.
fn holders(kernel32: &Kernel32) -> Holders {
    const QUERY_LIMITED: u32 = 0x1000;
    const DUP_HANDLE: u32 = 0x40;
    const DUPLICATE_SAME_ACCESS: u32 = 2;
    const MEM_COMMIT_RESERVE: u32 = 0x3000;
    const MEM_RELEASE: u32 = 0x8000;
    const PAGE_READWRITE: u32 = 4;
    const ACCESS: u32 = Holder::VM_READ | Holder::VM_WRITE | Holder::VM_OPERATION;
    let (Some(query), Some(open), Some(dup), Some(pid_of)) = (
        export_of(c"ntdll.dll", c"NtQuerySystemInformation"),
        export_of(c"kernel32.dll", c"OpenProcess"),
        export_of(c"kernel32.dll", c"DuplicateHandle"),
        export_of(c"kernel32.dll", c"GetProcessId"),
    ) else {
        return Holders::EMPTY;
    };
    // SAFETY: GetCurrentProcessId (the exe's import) and GetCurrentProcess
    // (its own export): the game's own pid and process pseudo-handle.
    let (own_pid, own_process) =
        unsafe { ((raw::engine_get_current_process_id)(), raw::engine_get_current_process(kernel32.current_process)) };

    let mut size: u32 = 1 << 20;
    const CAP: u32 = 64 << 20;
    loop {
        // SAFETY: VirtualAlloc (the exe's import): `size` committed read/write bytes, or null.
        let buffer = unsafe { (raw::engine_virtual_alloc)(core::ptr::null_mut(), size, MEM_COMMIT_RESERVE, PAGE_READWRITE) };
        let Some(buffer) = NonNull::new(buffer) else { return Holders::EMPTY };
        let mut needed = 0u32;
        // SAFETY: NtQuerySystemInformation's own export: it writes at most `size` bytes into the buffer.
        let status =
            unsafe { raw::engine_nt_query_system_information(query, EXTENDED_HANDLE_INFO, buffer.as_ptr(), size, &mut needed) };
        let out = (status == 0)
            .then(|| scan_handles(kernel32, buffer, size as usize, own_pid, own_process, open, dup, pid_of, ACCESS, QUERY_LIMITED, DUP_HANDLE, DUPLICATE_SAME_ACCESS));
        // SAFETY: the buffer we allocated; released once (MEM_RELEASE wants size 0).
        unsafe { (raw::engine_virtual_free)(buffer.as_ptr(), 0, MEM_RELEASE) };
        if let Some(out) = out {
            return out;
        }
        if status != STATUS_LENGTH_MISMATCH || size >= CAP {
            return Holders::EMPTY;
        }
        size = size.saturating_mul(2).min(CAP);
    }
}

/// The handle list in `buffer` (its two-word header, then 28-byte entries)
/// scanned for the game's process-object handles in other processes. The
/// helper the loop in [`holders`] runs once it has the list.
#[allow(clippy::too_many_arguments)]
fn scan_handles(
    kernel32: &Kernel32,
    buffer: NonNull<c_void>,
    size: usize,
    own_pid: u32,
    own_process: *mut c_void,
    open: NonNull<c_void>,
    dup: NonNull<c_void>,
    pid_of: NonNull<c_void>,
    access_mask: u32,
    query_limited: u32,
    dup_handle: u32,
    same_access: u32,
) -> Holders {
    let fits = size.saturating_sub(HANDLE_HEADER) / HANDLE_ENTRY;
    let count = (read_word(buffer, size, 0, 4) as usize).min(fits);
    let entry = |i: usize, at: usize, n: usize| read_word(buffer, size, HANDLE_HEADER + i * HANDLE_ENTRY + at, n);
    // learn the game's process-object type index and its own object from a
    // real handle the game opens to itself
    // SAFETY: OpenProcess's own export; null when refused.
    let me = unsafe { raw::engine_open_process(open, query_limited, 0, own_pid) };
    let mut self_type = None;
    for i in 0..count {
        if entry(i, 4, 4) == own_pid && entry(i, 8, 4) as usize == me as usize && !me.is_null() {
            self_type = Some((entry(i, 18, 2) as u16, entry(i, 0, 4)));
            break;
        }
    }
    // SAFETY: the handle opened above, closed once.
    if !me.is_null() {
        unsafe { (raw::engine_close_handle)(me) };
    }
    let Some((process_type, self_object)) = self_type else { return Holders::EMPTY };

    let (mut holders, mut unchecked) = (Tally::NONE, Tally::NONE);
    let mut dup_budget = DUP_BUDGET;
    for i in 0..count {
        if entry(i, 18, 2) as u16 != process_type {
            continue;
        }
        let pid = entry(i, 4, 4);
        if pid == own_pid {
            continue;
        }
        let (object, granted, handle) = (entry(i, 0, 4), entry(i, 12, 4), entry(i, 8, 4));
        // by object address where the kernel gives one, else by duplicating
        let to_us = if self_object != 0 && object != 0 {
            Some(object == self_object)
        } else if dup_budget == 0 {
            None
        } else {
            dup_budget -= 1;
            // SAFETY: OpenProcess's own export; the handle (DUP_HANDLE) and the
            // duplicate below are closed on every path that made one.
            let holder = unsafe { raw::engine_open_process(open, dup_handle, 0, pid) };
            let Some(holder) = NonNull::new(holder) else {
                unchecked.merge(pid, 1, true);
                continue;
            };
            let mut target = core::ptr::null_mut();
            // SAFETY: DuplicateHandle's own export: it copies the holder's handle
            // into the game, or fails; we deref neither the source nor the copy.
            let copied = unsafe {
                raw::engine_duplicate_handle(dup, holder.as_ptr(), handle as *mut c_void, own_process, &mut target, 0, 0, same_access)
            };
            let same = NonNull::new(target).is_some_and(|target| {
                // SAFETY: GetProcessId's own export on the copy; then it is closed, once.
                let pid = unsafe { raw::engine_get_process_id(pid_of, target.as_ptr()) };
                unsafe { (raw::engine_close_handle)(target.as_ptr()) };
                pid == own_pid
            });
            // SAFETY: the holder handle, closed once.
            unsafe { (raw::engine_close_handle)(holder.as_ptr()) };
            if copied != 0 { Some(same) } else { None }
        };
        match to_us {
            Some(true) if granted & access_mask != 0 => holders.merge(pid, granted, false),
            Some(true) | Some(false) => {}
            None => unchecked.merge(pid, 1, true),
        }
    }
    resolve_holders(kernel32, &holders, &unchecked)
}

/// The holder tallies, their exe names (one process-list walk) and homes
/// (one handle each, closed at once, as [`exe_of`]), into [`Holder`] and
/// [`Unchecked`] entries. No Windows user name leaves the PC (the folder masking).
fn resolve_holders(kernel32: &Kernel32, holders: &Tally, unchecked: &Tally) -> Holders {
    let (folder, temp, windows) = (kernel32.file_of(core::ptr::null_mut()).folder(), kernel32.temp(), kernel32.windows());
    let home = |pid: u32| {
        exe_of(kernel32, pid)
            .map_or(Home::Unknown, |exe| Home::of(exe.as_units(), folder.as_units(), temp.as_units(), windows.as_units()))
    };
    let names = |tally: &Tally| {
        let mut ids = [None; MAX_HOLDERS];
        ids.iter_mut().zip(tally.rows.get(..tally.len).unwrap_or_default()).for_each(|(id, row)| *id = Some(row.0));
        process_names(kernel32, &ids)
    };
    let holder_names = names(holders);
    let mut out = Holders::EMPTY;
    for ((pid, access), (slot, name)) in
        holders.rows.iter().take(holders.len).zip(out.holders.iter_mut().zip(holder_names))
    {
        *slot = Some(Holder { name, access: *access, home: home(*pid) });
    }
    let unchecked_names = names(unchecked);
    for ((pid, count), (slot, name)) in
        unchecked.rows.iter().take(unchecked.len).zip(out.unchecked.iter_mut().zip(unchecked_names))
    {
        *slot = Some(Unchecked { name, count: *count, home: home(*pid) });
    }
    out
}

/// The other processes' windows over the game's ([`Over`]): from the top of
/// the windows' order down to the game's own, each one that is visible,
/// always on top and over some of the game's window, the first
/// [`MAX_OVER`]. Only user32's own questions about a window are asked and
/// nothing of its content is read; its process is opened only to ask where
/// its exe is ([`exe_of`]).
fn windows_over(kernel32: &Kernel32) -> Result<[Option<Over>; MAX_OVER], Unseen> {
    /// `GW_HWNDNEXT`: the window below in the order.
    const BELOW: u32 = 2;
    /// `GA_ROOT`: the top-level window a window is in.
    const ROOT: u32 = 2;
    const GWL_EXSTYLE: u32 = -20i32 as u32;
    const CLASS: usize = 64;
    let presented = NonNull::new(WINDOW.load(Ordering::Relaxed) as *mut c_void).ok_or(Unseen::NoWindow)?;
    // SAFETY: GetModuleHandleA and GetProcAddress (the exe's imports), NUL-terminated names; the
    // exe imports user32, so it is loaded and never unloaded.
    let function = |name: &CStr| unsafe {
        let user32 = (raw::engine_get_module_handle_a)(c"user32.dll".as_ptr().cast());
        if user32.is_null() { None } else { NonNull::new((raw::engine_get_proc_address)(user32, name.as_ptr().cast())) }
    };
    let (Some(top), Some(below), Some(root), Some(visible), Some(process_of), Some(rect_of), Some(class_of)) = (
        function(c"GetTopWindow"),
        function(c"GetWindow"),
        function(c"GetAncestor"),
        function(c"IsWindowVisible"),
        function(c"GetWindowThreadProcessId"),
        function(c"GetWindowRect"),
        function(c"GetClassNameW"),
    ) else {
        return Err(Unseen::NoWindow);
    };
    // GetWindowDisplayAffinity is Windows 7's; a Windows without it leaves the
    // affinity [`Affinity::None`] (`shown`), as if a capture takes the window.
    let affinity_of = function(c"GetWindowDisplayAffinity");
    let rect = |window: *mut c_void| {
        let mut rect = Rect::default();
        // SAFETY: GetWindowRect's own export: it writes the RECT or fails on what is no window.
        (unsafe { raw::engine_get_window_rect(rect_of, window, (&raw mut rect).cast()) } != 0).then_some(rect)
    };
    let affinity = |window: *mut c_void| match affinity_of {
        None => Affinity::None,
        Some(affinity_of) => {
            let mut value = 0u32;
            // SAFETY: GetWindowDisplayAffinity's own export: it writes the affinity or fails on what is no window.
            let ok = unsafe { raw::engine_get_window_display_affinity(affinity_of, window, &mut value) } != 0;
            if ok { Affinity::of(value) } else { Affinity::None }
        }
    };
    // SAFETY: GetAncestor's own export; null for what is no window. GetCurrentProcessId (the exe's import).
    let (game, own) = unsafe { (raw::engine_get_ancestor(root, presented.as_ptr(), ROOT), (raw::engine_get_current_process_id)()) };
    let place = NonNull::new(game).and_then(|game| rect(game.as_ptr())).ok_or(Unseen::NoWindow)?;
    let mut found: [Option<(u32, Over)>; MAX_OVER] = [None; MAX_OVER];
    // SAFETY: GetTopWindow's own export, of the desktop: the top of the order.
    let mut window = unsafe { raw::engine_get_top_window(top, core::ptr::null_mut()) };
    for _ in 0..MAX_WINDOWS {
        if window.is_null() {
            break;
        }
        if window == game {
            // the usual look (once a minute in a match): nothing over the game, no process list taken
            if found.iter().all(Option::is_none) {
                return Ok([None; MAX_OVER]);
            }
            let names = process_names(kernel32, &found.map(|found| found.map(|(process, _)| process)));
            let (folder, temp, windows) = (kernel32.file_of(core::ptr::null_mut()).folder(), kernel32.temp(), kernel32.windows());
            let mut told = [None; MAX_OVER];
            for ((process, over), name) in found.iter().flatten().zip(names) {
                let home = exe_of(kernel32, *process).map_or(Home::Unknown, |exe| {
                    Home::of(exe.as_units(), folder.as_units(), temp.as_units(), windows.as_units())
                });
                let over = Some(Over { name, home, ..*over });
                // two windows alike are told once
                if let Some(slot) = told.iter_mut().find(|slot| slot.is_none() || **slot == over) {
                    *slot = over;
                }
            }
            return Ok(told);
        }
        let mut process = 0;
        // SAFETY: user32's own exports, each asked of a window handle: one that is no window
        // any more answers 0. GetClassNameW writes at most CLASS units into `class`.
        let (thread, shown, style) = unsafe {
            (
                raw::engine_get_window_thread_process_id(process_of, window, &mut process),
                raw::engine_is_window_visible(visible, window) != 0,
                (raw::engine_get_window_long_a)(window, GWL_EXSTYLE),
            )
        };
        let over = (thread != 0 && process != own && shown && style & Over::TOPMOST != 0)
            .then(|| rect(window).and_then(|rect| rect.overlap(place)))
            .flatten();
        if let (Some((width, height)), Some(slot)) = (over, found.iter_mut().find(|slot| slot.is_none())) {
            let mut class = [0; CLASS];
            // SAFETY: as above.
            let len = unsafe { raw::engine_get_class_name_w(class_of, window, class.as_mut_ptr(), CLASS as u32) } as usize;
            let class = Name::of(class.get(..len).unwrap_or_default());
            // a window the walk found is over the game now: its start is not
            // pinned by an event (since 0), and it has not gone (until None).
            let over = Over { name: None, class, style, width, height, affinity: affinity(window), home: Home::Unknown, since: 0, until: None };
            *slot = Some((process, over));
        }
        // SAFETY: GetWindow's own export; null at the order's end or for what is no window.
        window = unsafe { raw::engine_get_window(below, window, BELOW) };
    }
    Err(Unseen::NoOrder)
}

/// Event-observed windows-over episodes a look drains at most.
pub(super) const MAX_EPISODES: usize = 16;

/// What a periodic look sees of the outside at a moment: the other
/// processes' windows over the game now ([`windows_over`], or why it could
/// not tell), the windows-over episodes the event watch
/// ([`super::module_watch`]) saw come and go since the last look (with the
/// match seconds they were over for), and the other processes that hold a
/// handle to the game process with memory access ([`holders`]). The whole is
/// compared from one minute to the next so only a change is told.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Windows {
    over: Result<[Option<Over>; MAX_OVER], Unseen>,
    episodes: [Option<Over>; MAX_EPISODES],
    holders: [Option<Holder>; MAX_HOLDERS],
    unchecked: [Option<Unchecked>; MAX_UNCHECKED],
}

impl Windows {
    /// A look now: the windows over the game, the episodes the event watch
    /// saw come and go since the last look, and the handle holders.
    /// `None`: kernel32 lacks a function the walks need.
    pub fn look(thread: &Thread) -> Option<Windows> {
        let kernel32 = Kernel32::get()?;
        let episodes = module_watch::drain_episodes(thread);
        let Holders { holders, unchecked } = holders(&kernel32);
        Some(Windows { over: windows_over(&kernel32), episodes, holders, unchecked })
    }

    /// A look that could not tell, for `unseen`.
    #[cfg(test)]
    pub fn unseen(unseen: Unseen) -> Windows {
        Windows {
            over: Err(unseen),
            episodes: [None; MAX_EPISODES],
            holders: [None; MAX_HOLDERS],
            unchecked: [None; MAX_UNCHECKED],
        }
    }

    /// Its entries, into `report`, the rarest first so a long list of
    /// windows cannot crowd them out: handle holders, then holders the game
    /// could not check, then the episodes the event watch saw, then the
    /// windows over the game now (or why none could be told).
    fn add_to(self, report: &mut Report) {
        self.holders.into_iter().flatten().for_each(|holder| report.add(Entry::Holder(holder)));
        self.unchecked.into_iter().flatten().for_each(|unchecked| report.add(Entry::Unchecked(unchecked)));
        self.episodes.into_iter().flatten().for_each(|over| report.add(Entry::Over(over)));
        match self.over {
            Ok(overs) => overs.into_iter().flatten().for_each(|over| report.add(Entry::Over(over))),
            Err(unseen) => report.add(Entry::Unseen(unseen)),
        }
    }

    /// A report of these windows and holders only (a minute's look).
    pub fn report(self, moment: Moment) -> Report {
        let mut report = Report::new(moment);
        self.add_to(&mut report);
        report
    }
}

/// Everything patch 125 tells of this process at `moment`, with the
/// windows a look just saw. What is rare
/// and says most goes first, so a long module list cannot crowd it out:
/// the windows over the game, then the modules, the watch's loads and the
/// code that is no module. `None`: the
/// walks could not run (kernel32 lacks a function, or no snapshot); a
/// report then says nothing.
pub fn snapshot(thread: &Thread, moment: Moment, windows: Windows) -> Option<Report> {
    let kernel32 = Kernel32::get()?;
    let mut report = Report::new(moment);
    let kept = module_watch::kept(thread);
    windows.add_to(&mut report);
    let bases = modules(&kernel32, &mut report)?;
    kept.loads(|base| bases.listed(base)).for_each(|load| report.add(Entry::Load(load)));
    code(&kernel32, &bases, &mut report);
    Some(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel32s_structures_and_a_folder_ends_at_its_separator() {
        #[cfg(target_pointer_width = "32")]
        {
            assert_eq!(size_of::<ModuleEntry>(), 1064);
            assert_eq!(size_of::<ProcessEntry>(), 556);
            assert_eq!(size_of::<MemoryInfo>(), 28);
        }
        let path: std::vec::Vec<u16> = "C:\\Games\\Skhid\\Survarium.exe".encode_utf16().collect();
        assert_eq!(std::string::String::from_utf16(folder_of(&path)).unwrap(), "C:\\Games\\Skhid\\");
        assert!(folder_of(&[0x61]).is_empty());
        assert_eq!(Path::written([0; PATH], 0).len, 0);
        assert_eq!(Path::written([0; PATH], PATH as u32).len, 0);
        assert_eq!(Path::written([0; PATH], 7).len, 7);
    }

    #[test]
    fn too_many_bases_tell_no_image_as_unlisted() {
        let mut bases = Bases::Known { bases: [0; MAX_BASES], len: 0 };
        bases.add(0x40_0000);
        assert!(bases.listed(0x40_0000) && !bases.listed(0x50_0000));
        (0..MAX_BASES).for_each(|n| bases.add(n));
        assert!(matches!(bases, Bases::TooMany) && bases.listed(0x50_0000));
    }
}
