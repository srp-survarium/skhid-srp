//! What the lobby's sharing (patch 150) needs of Windows and of
//! torrent_client::start_seeding_impl (0x373C20): the process's number, the
//! pipe's name given to its win_utils::pipe::accept, and the
//! updater started beside the exe.
//!
//! start_seeding_impl runs on the network world's thread only: start_seeding
//! and its callers post it to that thread's task queue (re-kit, abi.tsv
//! 0x3737F0), so its sites never run on two threads at once.

use core::cell::UnsafeCell;
use core::ffi::c_void;
use core::ptr::{NonNull, null_mut};

use super::context::{Context, Reg};
use crate::raw;

/// This process's number.
pub fn process_id() -> u32 {
    // SAFETY: kernel32's GetCurrentProcessId (the game's import), no argument.
    unsafe { (raw::engine_get_current_process_id)() }
}

/// UTF-16 units of the pipe's name, its zero included.
const NAME_UNITS: usize = 64;

struct Name(UnsafeCell<[u16; NAME_UNITS]>);

// SAFETY: written and read on the network world's thread alone (module doc).
unsafe impl Sync for Name {}

/// The name given to the pipe's accept: it reads it in the call after the
/// hook, so it lives here, not on the hook's stack.
static NAME: Name = Name(UnsafeCell::new([0; NAME_UNITS]));

/// At `seeding.accept` (start_seeding_impl's `lea eax,<its wide name>`
/// before win_utils::pipe::accept): EAX is `name` (ASCII, shorter than
/// [`NAME_UNITS`]) instead, for a Skip of the lea. `false` (EAX not set, the
/// lea is to run): a name that is not.
pub fn accept_as(ctx: &mut Context<'_>, name: &[u8]) -> bool {
    if name.len() >= NAME_UNITS || !name.is_ascii() {
        return false;
    }
    // SAFETY: the network world's thread alone writes and reads NAME (module
    // doc); accept, the only reader, runs after this on the same thread.
    let units = unsafe { &mut *NAME.0.get() };
    units.fill(0);
    for (unit, byte) in units.iter_mut().zip(name) {
        *unit = u16::from(*byte);
    }
    ctx.set_reg(Reg::Eax, units.as_ptr() as usize as u32);
    true
}

/// `CREATE_NO_WINDOW`: no console for it (the updater has none either).
const CREATE_NO_WINDOW: u32 = 1 << 27;
/// UTF-16 units of a path, and of the command line.
const PATH_UNITS: usize = 520;
const COMMAND_UNITS: usize = PATH_UNITS + 128;

/// Puts `units` into `into` from `at` on, leaving room for a zero after
/// them; `false` when they do not fit.
fn put(into: &mut [u16], at: &mut usize, units: impl Iterator<Item = u16>) -> bool {
    for unit in units {
        if *at + 1 >= into.len() {
            return false;
        }
        into[*at] = unit;
        *at += 1;
    }
    true
}

/// Starts `<this exe's folder>\<program>` with `arguments` (ASCII), its
/// command line `"<path>" <arguments>`, from that folder, with no window;
/// its handles closed at once (nothing waits for it). `Err`: Windows' error
/// code (0: a path or command line that does not fit, or no CreateProcessW).
pub fn start_beside(program: &str, arguments: &[u8]) -> Result<(), u32> {
    if !program.is_ascii() || !arguments.is_ascii() {
        return Err(0);
    }
    let mut path = [0u16; PATH_UNITS];
    // SAFETY: kernel32's own exports (GetModuleHandleA, GetProcAddress: the
    // game's imports) given zero-terminated names; GetModuleFileNameW and
    // CreateProcessW through their generated stdcall bindings, each buffer
    // with its room; the process's handles closed once, by their owner here.
    unsafe {
        let kernel = (raw::engine_get_module_handle_a)(c"kernel32.dll".as_ptr().cast());
        if kernel.is_null() {
            return Err(0);
        }
        let function = |name: &core::ffi::CStr| NonNull::new((raw::engine_get_proc_address)(kernel, name.as_ptr().cast()));
        let (Some(module_file_name), Some(create_process)) = (function(c"GetModuleFileNameW"), function(c"CreateProcessW")) else {
            return Err(0);
        };
        let n = raw::engine_get_module_file_name_w(module_file_name, null_mut(), path.as_mut_ptr(), PATH_UNITS as u32) as usize;
        if n == 0 || n >= PATH_UNITS {
            return Err(0);
        }
        let Some(slash) = path[..n].iter().rposition(|&unit| unit == u16::from(b'\\')) else { return Err(0) };
        // The folder, zero-terminated where its last backslash was, and the
        // program's path after it.
        let mut folder = [0u16; PATH_UNITS];
        folder[..slash].copy_from_slice(&path[..slash]);
        let mut application = [0u16; PATH_UNITS];
        let mut at = 0;
        if !put(&mut application, &mut at, path[..=slash].iter().copied()) || !put(&mut application, &mut at, program.bytes().map(u16::from)) {
            return Err(0);
        }
        let application_units = at;
        let mut command = [0u16; COMMAND_UNITS];
        let mut at = 0;
        let quote = u16::from(b'"');
        let fits = put(&mut command, &mut at, core::iter::once(quote))
            && put(&mut command, &mut at, application[..application_units].iter().copied())
            && put(&mut command, &mut at, [quote, u16::from(b' ')].into_iter())
            && put(&mut command, &mut at, arguments.iter().map(|&byte| u16::from(byte)));
        if !fits {
            return Err(0);
        }
        // STARTUPINFOW (44h bytes, its size first) and PROCESS_INFORMATION.
        let mut startup = [0u32; 17];
        startup[0] = 0x44;
        let mut information = [0u32; 4];
        let started = raw::engine_create_process_w(
            create_process,
            application.as_ptr(),
            command.as_mut_ptr(),
            null_mut(),
            null_mut(),
            0,
            CREATE_NO_WINDOW,
            null_mut(),
            folder.as_ptr(),
            startup.as_mut_ptr().cast::<c_void>(),
            information.as_mut_ptr().cast::<c_void>(),
        );
        if started == 0 {
            return Err((raw::engine_get_last_error)());
        }
        for handle in &information[..2] {
            (raw::engine_close_handle)(*handle as usize as *mut c_void);
        }
    }
    Ok(())
}
