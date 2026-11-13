//! The only file sources of the reports, each a path Windows itself named:
//! the current process's image, hashed and (when the server asks) sent, with
//! write/delete sharing denied; and the exe of a process whose window is over
//! the game's ([`super::modules`]), only hashed: no way from that file to
//! [`Client`], so nothing of it but its size and SHA-256 leaves the PC. Both
//! are opened read-only. No remote path, DLL path, directory walk or
//! process-memory read enters this API.
use super::{Context, Lock, Thread, ThreadBound};
use super::packet_recovery::Connection;
use crate::{raw, executable_report::{Client, Source}};
use core::{ffi::c_void, ptr::{NonNull, null_mut}};
use srp_validate::executable::Reply;

const GENERIC_READ: u32 = 1 << 31;
const FILE_SHARE_READ: u32 = 1;
const FILE_SHARE_DELETE: u32 = 4;
const FILE_FLAG_SEQUENTIAL_SCAN: u32 = 1 << 27;
const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;

pub(super) struct Executable { file: *mut c_void, seek: NonNull<c_void> }
impl Executable {
    /// The file at `path` (its last unit the terminator), read-only and
    /// never written meanwhile; `share`: what else others may do with it.
    fn at(path: &[u16], share: u32) -> Option<Self> {
        if path.last() != Some(&0) { return None; }
        // SAFETY: existing module, NUL-terminated export names and path; all
        // native calls use generated stdcall bindings. No file name leaves here.
        unsafe {
            let kernel = (raw::engine_get_module_handle_a)(c"kernel32.dll".as_ptr().cast());
            if kernel.is_null() { return None; }
            let function = |name: &core::ffi::CStr| NonNull::new((raw::engine_get_proc_address)(kernel, name.as_ptr().cast()));
            let open = function(c"CreateFileW")?;
            let seek = function(c"SetFilePointer")?;
            let file = raw::engine_create_file_w(open, path.as_ptr(), GENERIC_READ, share, null_mut(), 3, FILE_FLAG_SEQUENTIAL_SCAN | FILE_ATTRIBUTE_NORMAL, null_mut());
            (file as usize != usize::MAX).then_some(Self { file, seek })
        }
    }
    fn open() -> Option<Self> {
        let mut path = [0u16; 1024];
        // SAFETY: as `at`; GetModuleFileNameW's own export, given its buffer's room.
        let n = unsafe {
            let kernel = (raw::engine_get_module_handle_a)(c"kernel32.dll".as_ptr().cast());
            if kernel.is_null() { return None; }
            let path_fn = NonNull::new((raw::engine_get_proc_address)(kernel, c"GetModuleFileNameW".as_ptr().cast()))?;
            raw::engine_get_module_file_name_w(path_fn, null_mut(), path.as_mut_ptr(), path.len() as u32) as usize
        };
        if n == 0 { return None; }
        Self::at(path.get(..=n)?, FILE_SHARE_READ)
    }
    /// The exe of a process whose window is over the game's, at the path
    /// Windows named for it. Another program may rename or delete the file
    /// meanwhile (an updater does): the game is never in its way.
    pub(super) fn of_window(path: &super::modules::Path) -> Option<Self> {
        Self::at(path.terminated()?, FILE_SHARE_READ | FILE_SHARE_DELETE)
    }
}
impl Source for Executable {
    fn size(&mut self) -> Option<u64> {
        /// `FILE_END`: the distance is from the file's end.
        const END: u32 = 2;
        let mut high = 0u32;
        // SAFETY: live handle; no distance from the end, so the position the
        // call returns is the size: its low half, the high half into `high`.
        let low = unsafe { raw::engine_set_file_pointer(self.seek, self.file, 0, (&raw mut high).cast(), END) };
        // INVALID_SET_FILE_POINTER: the call failed (a size whose low half is
        // all ones is taken for that too). Then back to the start.
        (low != u32::MAX && self.seek(0)).then_some(u64::from(high) << 32 | u64::from(low))
    }
    fn read(&mut self, into: &mut [u8]) -> Option<usize> {
        let mut got = 0;
        // SAFETY: owned open handle, writable buffer and DWORD; synchronous read.
        let ok = unsafe { (raw::engine_read_file)(self.file, into.as_mut_ptr().cast(), into.len() as u32, &mut got, null_mut()) };
        (ok != 0 && got as usize <= into.len()).then_some(got as usize)
    }
    fn seek(&mut self, offset: u32) -> bool {
        if offset > srp_validate::executable::MAX_FILE { return false; }
        // SAFETY: live handle, offset <=16 MiB fits signed LONG, FILE_BEGIN.
        unsafe { raw::engine_set_file_pointer(self.seek, self.file, offset, null_mut(), 0) == offset }
    }
}
impl Drop for Executable {
    fn drop(&mut self) {
        // SAFETY: uniquely owned handle, closed exactly once.
        unsafe { (raw::engine_close_handle)(self.file); }
    }
}
struct Active { key: (usize, u32), client: ThreadBound<Client<Executable>> }
static ACTIVE: Lock<Option<Active>> = Lock::new(None);
pub fn loaded(ctx: &Context<'_>) {
    let Some(connection) = Connection::at_loaded(ctx) else { return };
    ACTIVE.with(ctx.thread(), |active| {
        if active.as_ref().is_some_and(|a| a.key == connection.key()) { return; }
        *active = Executable::open().map(|file| Active { key: connection.key(), client: ThreadBound::new(ctx.thread(), Client::new(file)) });
    });
}
pub fn frame(ctx: &Context<'_>) {
    let Some(connection) = Connection::at_game_tick(ctx) else { return };
    ACTIVE.with(ctx.thread(), |active| {
        if let Some(active) = active.as_mut().filter(|a| a.key == connection.key())
            && let Some(client) = active.client.get_mut(ctx.thread()) {
            client.tick(super::files::tick_count(), connection.match_id(), |carrier| connection.executable(carrier));
        }
    });
}
pub fn close(ctx: &Context<'_>) {
    let Some(connection) = Connection::at_close(ctx) else { return };
    ACTIVE.with(ctx.thread(), |active| {
        if active.as_ref().is_some_and(|a| a.key == connection.key()) { *active = None; }
    });
}
pub fn reply(thread: &Thread, key: (usize, u32), reply: Reply) {
    if reply.match_id != key.1 { return; }
    ACTIVE.with(thread, |active| {
        if let Some(active) = active.as_mut().filter(|a| a.key == key)
            && let Some(client) = active.client.get_mut(thread) { client.reply(reply); }
    });
}
