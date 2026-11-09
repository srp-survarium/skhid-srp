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
// a mutable borrow can cross threads only when T is Send.
unsafe impl<T: Send> Sync for Lock<T> {}

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
        struct Unlock<'a>(&'a AtomicU32);
        impl Drop for Unlock<'_> { fn drop(&mut self) { self.0.store(0, Ordering::Release); } }
        let unlock = Unlock(&self.owner);
        let out = f(unsafe { &mut *self.value.get() });
        drop(unlock);
        out
    }
}

/// A [`Lock`] for state one thread may hold for long (patch 127's
/// profiler: every allocation of every thread takes it, a snapshot holds it
/// for its walks and writes): a waiter spins a while, then gives its time
/// slice away (SwitchToThread, then Sleep(0)), so a preempted holder is not
/// starved by waiters burning whole slices, which would distort what the
/// profile measures. [`Lock`] itself stays a pure spin: its users hold it
/// for a few instructions.
pub struct YieldingLock<T> {
    owner: AtomicU32,
    value: UnsafeCell<T>,
}

// SAFETY: as Lock's.
unsafe impl<T: Send> Sync for YieldingLock<T> {}

impl<T> YieldingLock<T> {
    /// Spins before the first yield.
    const SPINS: u32 = 256;

    pub const fn new(value: T) -> YieldingLock<T> {
        YieldingLock { owner: AtomicU32::new(0), value: UnsafeCell::new(value) }
    }

    /// Runs `f` on the value, the lock held for it.
    pub fn with<R>(&self, _: &Thread, f: impl FnOnce(&mut T) -> R) -> R {
        let me = thread_id();
        let mut tries = 0u32;
        loop {
            match self.owner.compare_exchange_weak(0, me, Ordering::Acquire, Ordering::Relaxed) {
                Ok(_) => break,
                Err(holder) if holder == me => panic!("a lock taken again on its own thread"),
                Err(_) => {
                    tries = tries.saturating_add(1);
                    if tries < Self::SPINS {
                        core::hint::spin_loop();
                    } else {
                        give_way(tries);
                    }
                }
            }
        }
        struct Unlock<'a>(&'a AtomicU32);
        impl Drop for Unlock<'_> { fn drop(&mut self) { self.0.store(0, Ordering::Release); } }
        let unlock = Unlock(&self.owner);
        // SAFETY: the owner swap above: this thread alone holds the lock
        // until the store, and `f` cannot keep the borrow.
        let out = f(unsafe { &mut *self.value.get() });
        drop(unlock);
        out
    }
}

/// A waiter's yield: another ready thread of this core first
/// (SwitchToThread), every other try any ready thread (Sleep(0)).
#[cfg(not(test))]
fn give_way(tries: u32) {
    // SAFETY: the exe's imports, no state of ours.
    unsafe {
        if tries.is_multiple_of(2) || (crate::raw::engine_switch_to_thread)() == 0 {
            (crate::raw::engine_sleep)(0);
        }
    }
}

#[cfg(test)]
fn give_way(_: u32) {
    std::thread::yield_now();
}

/// The calling thread's id (kernel32's GetCurrentThreadId; never 0).
#[cfg(not(test))]
pub(super) fn thread_id() -> u32 {
    // SAFETY: the exe's import, no arguments, no state.
    unsafe { crate::raw::engine_get_current_thread_id() }
}

#[cfg(test)]
pub(super) fn thread_id() -> u32 {
    static NEXT: AtomicU32 = AtomicU32::new(1);
    std::thread_local! { static ID: u32 = NEXT.fetch_add(1, Ordering::Relaxed); }
    ID.with(|id| *id)
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
    #[test]
    fn a_yielding_lock_shares_its_value_among_more_threads_than_cores() {
        use std::sync::Arc;
        let value = Arc::new(YieldingLock::new(0u64));
        let workers: std::vec::Vec<_> = (0..32).map(|_| {
            let value = value.clone();
            std::thread::spawn(move || {
                // SAFETY: the host test models one native hook on this thread.
                let thread = unsafe { Thread::new() };
                for _ in 0..2000 { value.with(&thread, |n| { *n += 1; std::hint::black_box(*n); }); }
            })
        }).collect();
        for worker in workers { worker.join().unwrap(); }
        // SAFETY: as above.
        let thread = unsafe { Thread::new() };
        assert_eq!(value.with(&thread, |n| *n), 64_000);
    }

    #[test]
    fn workers_share_one_transition_and_unlock_after_an_unwind() {
        use std::{panic::{AssertUnwindSafe, catch_unwind}, sync::Arc};
        let value = Arc::new(Lock::new(0u32));
        let workers: std::vec::Vec<_> = (0..8).map(|_| {
            let value = value.clone();
            std::thread::spawn(move || {
                // SAFETY: the host test models one native hook on this thread.
                let thread = unsafe { Thread::new() };
                for _ in 0..1000 { value.with(&thread, |n| *n += 1); }
            })
        }).collect();
        for worker in workers { worker.join().unwrap(); }
        // SAFETY: this host thread models its own native hook.
        let thread = unsafe { Thread::new() };
        assert_eq!(value.with(&thread, |n| *n), 8000);
        assert!(catch_unwind(AssertUnwindSafe(|| value.with(&thread, |_| {
            value.with(&thread, |_| ());
        }))).is_err());
        assert_eq!(value.with(&thread, |n| *n), 8000);
    }

}
