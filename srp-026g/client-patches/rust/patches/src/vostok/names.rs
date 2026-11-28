//! Names a patch hands the engine for good. begin_pass and set_texture keep
//! the names they are given by name (the resource registries are keyed by
//! them), so a name read from a material's config, which lives only as long
//! as its compile data, is copied here first: an append-only pool in the
//! arena, each name once, alive as long as the process.

use core::ffi::CStr;
use core::sync::atomic::{AtomicU8, Ordering};

use super::hook::Thread;
use super::sync::Lock;

/// The pool's bytes: a few dozen names of materials' shaders, textures and
/// constants fit many times over.
const POOL: usize = 4096;

/// The names, each with its zero, one after another. A byte is written once,
/// before its name is handed out, and never again.
static BYTES: [AtomicU8; POOL] = [const { AtomicU8::new(0) }; POOL];
/// How many of the bytes hold names.
static USED: Lock<usize> = Lock::new(0);

/// The name at `at` (a start the pool handed out).
fn name_at(at: usize) -> &'static CStr {
    // SAFETY: a name's bytes up to its zero were written before its start
    // was handed out and are never written again (appends go past `USED`);
    // AtomicU8 is u8 in memory; the static lives as long as the process.
    // (c_string, not CStr::from_ptr: the arena has no strlen.)
    unsafe { super::ui::c_string(BYTES.as_ptr().add(at).cast(), POOL - at) }.expect("a pool name ends with its zero")
}

/// `name` for good: the pool's copy of it (the same copy for the same name),
/// or None when the pool is full.
pub fn intern(thread: &Thread, name: &CStr) -> Option<&'static CStr> {
    let wanted = name.to_bytes_with_nul();
    USED.with(thread, |used| {
        let mut at = 0;
        while at < *used {
            let held = name_at(at);
            if held.to_bytes_with_nul() == wanted {
                return Some(at);
            }
            at += held.to_bytes_with_nul().len();
        }
        if *used + wanted.len() > POOL {
            return None;
        }
        let start = *used;
        for (i, &byte) in wanted.iter().enumerate() {
            BYTES[start + i].store(byte, Ordering::Relaxed);
        }
        *used += wanted.len();
        Some(start)
    })
    .map(name_at)
}
