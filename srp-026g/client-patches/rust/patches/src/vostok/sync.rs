//! State a patch shares between hooks the game runs on several of its
//! threads: mt_draw's tasks (one a player, at once with `mt_player_draw`)
//! and the bullet manager's (256 bullets a task). A [`GameCell`] holds a
//! hook's state on the one thread; a [`Lock`] holds it across threads, one
//! user at a time, while a [`Thread`] proves we are inside a hook.
//!
//! A lock taken again on the thread that holds it is a bug of ours (a hook
//! reached from inside another's locked section): it panics, loudly, never
//! hangs the game.
//!
//! [`GameCell`]: super::GameCell

use super::hook::Thread;
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU32, Ordering};

/// A value behind a spin lock, reached only from inside a hook.
pub struct Lock<T> {
    /// The thread that holds it (its id), 0 when free.
    owner: AtomicU32,
    value: UnsafeCell<T>,
}

// SAFETY: `with` gives the `&mut` to one thread at a time (the owner swap);
// the value never leaves the lock.
unsafe impl<T> Sync for Lock<T> {}

impl<T> Lock<T> {
    pub const fn new(value: T) -> Lock<T> {
        Lock { owner: AtomicU32::new(0), value: UnsafeCell::new(value) }
    }

    /// Runs `f` on the value, the lock held for it.
    pub fn with<R>(&self, _: &Thread, f: impl FnOnce(&mut T) -> R) -> R {
        let me = thread_id();
        loop {
            match self.owner.compare_exchange_weak(0, me, Ordering::Acquire, Ordering::Relaxed) {
                Ok(_) => break,
                Err(holder) if holder == me => panic!("a lock taken again on its own thread"),
                Err(_) => core::hint::spin_loop(),
            }
        }
        // SAFETY: the owner swap above: this thread alone holds the lock
        // until the store below, and `f` cannot keep the borrow.
        let out = f(unsafe { &mut *self.value.get() });
        self.owner.store(0, Ordering::Release);
        out
    }
}

/// The calling thread's id (kernel32's GetCurrentThreadId; never 0).
#[cfg(not(test))]
fn thread_id() -> u32 {
    // SAFETY: the exe's import, no arguments, no state.
    unsafe { crate::raw::engine_get_current_thread_id() }
}

#[cfg(test)]
fn thread_id() -> u32 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holds_and_updates() {
        static VALUE: Lock<[u32; 4]> = Lock::new([0; 4]);
        // SAFETY: the test is the only thread.
        let thread = unsafe { Thread::new() };
        VALUE.with(&thread, |v| v[2] = 7);
        assert_eq!(VALUE.with(&thread, |v| v[2]), 7);
    }
}
