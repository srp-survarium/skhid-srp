//! The only file source for executable reporting: Windows' current process
//! image path, opened read-only with write/delete sharing denied. No remote
//! path, DLL path, directory walk or process-memory read enters this API.
use super::{Context, Lock, Thread, ThreadBound};
use super::packet_recovery::Connection;
use crate::{raw, executable_report::{Client, Source}};
use core::{ffi::c_void, ptr::{NonNull, null_mut}};
use srp_validate::executable::Reply;

const GENERIC_READ: u32 = 1 << 31;
const FILE_FLAG_SEQUENTIAL_SCAN: u32 = 1 << 27;
const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;

struct Executable { file: *mut c_void, seek: NonNull<c_void> }
impl Executable {
    fn open() -> Option<Self> {
        // SAFETY: existing module, NUL-terminated export names; all native
        // calls use generated stdcall bindings. No file name leaves here.
        unsafe {
            let kernel = (raw::engine_get_module_handle_a)(c"kernel32.dll".as_ptr().cast());
            if kernel.is_null() { return None; }
            let function = |name: &core::ffi::CStr| NonNull::new((raw::engine_get_proc_address)(kernel, name.as_ptr().cast()));
            let path_fn = function(c"GetModuleFileNameW")?;
            let open = function(c"CreateFileW")?;
            let seek = function(c"SetFilePointer")?;
            let mut path = [0u16; 1024];
            let n = raw::engine_get_module_file_name_w(path_fn, null_mut(), path.as_mut_ptr(), path.len() as u32) as usize;
            if n == 0 || n >= path.len() || path[n] != 0 { return None; }
            let file = raw::engine_create_file_w(open, path.as_ptr(), GENERIC_READ, 1, null_mut(), 3, FILE_FLAG_SEQUENTIAL_SCAN | FILE_ATTRIBUTE_NORMAL, null_mut());
            (file as usize != usize::MAX).then_some(Self { file, seek })
        }
    }
}
impl Source for Executable {
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
