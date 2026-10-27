//! The start-up mount of resources.db (client_patch.md section 3.27):
//! query_db's one call of query_mount_archive (sites_026g.tsv `mount.query`),
//! the callback the file system calls on the main thread for each mount,
//! in the order they were queued (on_mounted_resources, `mount.mounted`),
//! the mounts' reference counts, the native path strings a query takes, and
//! the patches an exe requires (patch 49), a list the release tools read
//! back from a built exe.

use core::ffi::{CStr, c_void};
use core::marker::PhantomData;
use core::ptr::null_mut;
use core::sync::atomic::{AtomicU32, Ordering};

use super::context::{Context, Reg};
use super::hook::Thread;
use super::object::{Object, read};
use crate::raw;

/// The most patches a game mounts: resources.db.patch_1 to _255 (the patch
/// revision is a byte).
pub const MAX_PATCHES: u32 = 255;

/// query_db's query of resources.db at `mount.query`: ESI the allocator, its
/// five cdecl arguments on the site's stack.
#[derive(Clone, Copy)]
pub struct MountQuery<'t> {
    allocator: u32,
    virtual_path: u32,
    table: u32,
    archive: u32,
    descriptor: u32,
    callback: u32,
    thread: PhantomData<&'t Thread>,
}

impl<'t> MountQuery<'t> {
    pub fn at(ctx: &'t Context<'_>) -> MountQuery<'t> {
        let frame = ctx.site_esp() as usize as *mut c_void;
        // SAFETY: query_db's pushed arguments at the site (query_db_frame,
        // by the site's ESP).
        unsafe {
            MountQuery {
                allocator: ctx.reg(Reg::Esi),
                virtual_path: read(raw::query_db_frame::virtual_path(frame)),
                table: read(raw::query_db_frame::table(frame)),
                archive: read(raw::query_db_frame::archive(frame)),
                descriptor: read(raw::query_db_frame::descriptor(frame)),
                callback: read(raw::query_db_frame::callback(frame)),
                thread: PhantomData,
            }
        }
    }

    /// Queues the game's own mount, with its own arguments.
    pub fn queue(&self) {
        self.query(self.table, self.archive);
    }

    /// Queues the archive at `path` (its file table the same file) under the
    /// game's virtual path, descriptor and callback.
    pub fn queue_archive(&self, path: &NativePath) {
        let at = path.address();
        self.query(at, at);
    }

    fn query(&self, table: u32, archive: u32) {
        let pointer = |value: u32| value as usize as *const c_void;
        // SAFETY: query_mount_archive as query_db calls it, with its
        // allocator, virtual path, descriptor and callback; the two path
        // strings are the game's or a NativePath of ours, live for the call
        // (the task copies them).
        unsafe {
            raw::engine_query_mount_archive(self.allocator as usize as *mut c_void, pointer(self.virtual_path),
                                            pointer(table), pointer(archive), self.descriptor as usize as *const u8,
                                            pointer(self.callback))
        }
    }

    /// `path` made from `format` (printf-like, each `/` the separator `\`)
    /// with the mount's descriptor for its `%s` and `number` for its `%u`, as
    /// query_db makes resources.db's.
    pub fn format(&self, path: &mut NativePath, format: &CStr, number: u32) {
        let at = path.reset();
        // SAFETY: the path string formatting on a native path string of
        // ours (NativePath::ptr: its pointers into its own buffer), a
        // NUL-ended format, the game's descriptor (a C string).
        unsafe { raw::engine_format_path(at, format.as_ptr().cast(), self.descriptor as usize as *const u8, number) };
    }
}

/// The size of a native path string: begin, end, the buffer's end, 260
/// characters, the separator (fields_026g.tsv native_path_string).
const PATH_SIZE: usize = 0x114;

/// A native path string (vostok::fs::native_path_string) of ours, as
/// query_db keeps resources.db's on its stack.
pub struct NativePath {
    bytes: [u32; PATH_SIZE / 4],
}

impl Default for NativePath {
    fn default() -> NativePath {
        NativePath { bytes: [0; PATH_SIZE / 4] }
    }
}

impl NativePath {
    /// The string's address, its pointers made again into its own buffer
    /// (it may have moved since), its text empty, its separator `\`.
    fn reset(&mut self) -> *mut c_void {
        let this = self.bytes.as_mut_ptr().cast::<c_void>();
        // SAFETY: our own PATH_SIZE bytes, the fields' offsets inside them.
        unsafe {
            let text = raw::native_path_string::text(this);
            raw::native_path_string::begin(this).write(text as usize as u32);
            raw::native_path_string::end(this).write(text as usize as u32);
            text.write(0);
            let separator = raw::native_path_string::separator(this);
            raw::native_path_string::capacity(this).write(separator as usize as u32);
            separator.write(b'\\');
        }
        this
    }

    /// Its address, as the game is given it (after a `reset`, the string
    /// not moved since: a `&mut` borrow from the formatting to here).
    fn address(&self) -> u32 {
        self.bytes.as_ptr() as usize as u32
    }

    /// The text (from begin to its NUL).
    pub fn text(&self) -> &CStr {
        let this = self.bytes.as_ptr().cast::<c_void>().cast_mut();
        // SAFETY: our own bytes: the formatting wrote the text NUL-ended in
        // the buffer (the 260 characters at +0Ch); its end is ours too.
        let bytes = unsafe {
            core::slice::from_raw_parts(raw::native_path_string::text(this).cast_const(),
                                        raw::native_path_string::separator(this) as usize
                                            - raw::native_path_string::text(this) as usize)
        };
        CStr::from_bytes_until_nul(bytes).unwrap_or(c"")
    }
}

/// A mount (a vfs mount root), by its reference count.
#[derive(Clone, Copy)]
pub struct Mount<'t> {
    mount: Object,
    thread: PhantomData<&'t Thread>,
}

impl Mount<'_> {
    fn references(&self) -> &AtomicU32 {
        // SAFETY: a live mount's reference count, which the game changes
        // with lock-prefixed adds alone (0x14805, 0x1481C).
        unsafe { AtomicU32::from_ptr(raw::mount::references(self.mount.ptr())) }
    }

    /// Takes a reference.
    pub fn hold(&self) {
        self.references().fetch_add(1, Ordering::SeqCst);
    }

    /// Lets a reference go as the game does: the last one unmounts it.
    pub fn release(&self) {
        if self.references().fetch_sub(1, Ordering::SeqCst) == 1 {
            // SAFETY: a mount nobody holds any more, its unmount queued as
            // on_mounted_resources does (0x14822).
            unsafe { raw::engine_unmount(self.mount.ptr()) }
        }
    }
}

/// on_mounted_resources at `mount.mounted`: (where the mount is kept, the
/// next step, &the mount), cdecl.
#[derive(Clone, Copy)]
pub struct Mounted<'t> {
    holder: u32,
    bound: u32,
    result: u32,
    thread: PhantomData<&'t Thread>,
}

impl<'t> Mounted<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Mounted<'t> {
        let frame = ctx.site_esp() as usize as *mut c_void;
        // SAFETY: the callback's three arguments (mounted_frame, by the
        // site's ESP, the function's start).
        unsafe {
            Mounted {
                holder: read(raw::mounted_frame::holder(frame)),
                bound: read(raw::mounted_frame::bound(frame)),
                result: read(raw::mounted_frame::result(frame)),
                thread: PhantomData,
            }
        }
    }

    /// The mount (None: the mount failed).
    pub fn mount(&self) -> Option<Mount<'t>> {
        let result = Object::at(self.result)?;
        // SAFETY: the callback's &mount_ptr: the mount or 0.
        let mount = Object::at(unsafe { read(result.ptr().cast::<u32>()) })?;
        Some(Mount { mount, thread: PhantomData })
    }

    /// The game's own on_mounted_resources with the mount `slot` keeps in
    /// place of this one (its hook, `mount.mounted`, runs again: the patch's
    /// handler sends it to the game's code); then it is back here.
    pub fn hand_to_game(&self, slot: &MountSlot) {
        // SAFETY: the callback as the file system calls it, with its own
        // first two arguments and the address of a mount_ptr of ours (a
        // static, live for the run) holding a mount held.
        unsafe {
            raw::engine_on_mounted_resources(self.holder as usize as *mut c_void, self.bound as usize as *mut c_void,
                                             slot.0.as_ptr().cast_const().cast())
        }
    }
}

/// A mount_ptr of a patch's state: a mount it holds, which the game may be
/// given (its address).
pub struct MountSlot(AtomicU32);

impl MountSlot {
    #[allow(clippy::new_without_default)] // a static's
    pub const fn new() -> MountSlot {
        MountSlot(AtomicU32::new(0))
    }

    /// Keeps `mount` (a reference it holds).
    pub fn keep(&self, mount: Mount<'_>) {
        self.0.store(mount.mount.id(), Ordering::Relaxed);
    }

    /// The mount it keeps (None: none).
    pub fn mount<'t>(&self, _: &'t Thread) -> Option<Mount<'t>> {
        Object::at(self.0.load(Ordering::Relaxed)).map(|mount| Mount { mount, thread: PhantomData })
    }
}

/// Whether the file `path` can be opened (no access asked, OPEN_EXISTING):
/// Ok, its handle closed again, or the error GetLastError gives.
pub fn probe(path: &CStr) -> Result<(), u32> {
    const NO_ACCESS: u32 = 0;
    const SHARE_ALL: u32 = 7; // FILE_SHARE_READ | WRITE | DELETE
    const OPEN_EXISTING: u32 = 3;
    const NORMAL: u32 = 0x80; // FILE_ATTRIBUTE_NORMAL
    // SAFETY: CreateFileA, GetLastError and CloseHandle through the exe's
    // import slots; a NUL-ended name, a handle CreateFileA gave.
    unsafe {
        let file = (raw::engine_create_file_a)(path.as_ptr().cast(), NO_ACCESS, SHARE_ALL, null_mut(), OPEN_EXISTING,
                                               NORMAL, null_mut());
        if file as isize == -1 {
            return Err((raw::engine_get_last_error)());
        }
        (raw::engine_close_handle)(file);
    }
    Ok(())
}

/// The game's fatal error (it never returns): `format` printf-like with
/// `first` and `second` (the ones it asks for).
pub fn fatal(format: &CStr, first: u32, second: u32) -> ! {
    // SAFETY: vostok::debug::terminate, a NUL-ended format; it takes what
    // the format asks for of the two.
    unsafe { raw::engine_terminate(format.as_ptr().cast(), first, second) };
    unreachable!("vostok::debug::terminate returned")
}

/// The text of a patch that cannot be opened (its %u the patch's number,
/// then the error), with the exe's list of required patches right after
/// it: bit n of byte n / 8, patch n required. patch_026g.py writes the bits
/// into a build (its --required-patches; zeros in the link), and the
/// release tools (release/lib.sh exe_required_patches,
/// patch_026g.required_patches_of) read them back after the text, which is
/// in the exe once.
#[repr(C)]
pub struct RequiredPatches {
    unreadable: [u8; UNREADABLE.len()],
    bits: [u8; (MAX_PATCHES as usize + 1) / 8],
}

const UNREADABLE: &[u8] = b"Cannot open resources.db.patch_%u (error %u). Please run the Skhid launcher again.\0";

/// UNREADABLE as an array.
const fn unreadable<const N: usize>() -> [u8; N] {
    let mut out = [0; N];
    let mut i = 0;
    while i < N {
        out[i] = UNREADABLE[i];
        i += 1;
    }
    out
}

/// The list (its address the arena's symbol `SRP_REQUIRED_PATCHES`).
#[unsafe(no_mangle)]
pub static SRP_REQUIRED_PATCHES: RequiredPatches = RequiredPatches {
    unreadable: unreadable::<{ UNREADABLE.len() }>(),
    bits: [0; (MAX_PATCHES as usize + 1) / 8],
};

/// The fatal error's format for a patch that cannot be opened: %u its
/// number, %u the error.
pub fn unreadable_format() -> &'static CStr {
    CStr::from_bytes_with_nul(&SRP_REQUIRED_PATCHES.unreadable).unwrap_or(c"")
}

/// Whether this exe requires patch `n` (1 to MAX_PATCHES).
pub fn required(n: u32) -> bool {
    let Some(byte) = SRP_REQUIRED_PATCHES.bits.get((n >> 3) as usize) else { return false };
    // SAFETY: a static of ours; patch_026g.py writes it in the image only,
    // so it is read as it is there, never as the link left it.
    unsafe { core::ptr::read_volatile(byte) >> (n & 7) & 1 != 0 }
}

const _: () = assert!(size_of::<*mut c_void>() == 4);
