//! State a patch shares between hooks the game runs on several of its
//! threads: mt_draw's tasks (one a player, at once with `mt_player_draw`)
//! and the bullet manager's (256 bullets a task). A [`GameCell`] holds a
//! hook's state on the one thread; a [`Lock`] holds it across threads, one
//! user at a time, while a [`Thread`] proves we are inside a hook. A value
//! only the thread that made it may touch (the engine's references, the
//! immediate context's work) sits in a lock as a [`ThreadBound`].
//!
//! A lock taken again on the thread that holds it is a bug of ours (a hook
//! reached from inside another's locked section): it panics, loudly, never
//! hangs the game.
//!
//! [`GameCell`]: super::GameCell

use super::hook::Thread;
use core::cell::UnsafeCell;
use core::mem::ManuallyDrop;
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

/// A thread's id (GetCurrentThreadId): whose a [`ThreadBound`] value is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThreadId(u32);

impl ThreadId {
    /// The hook's thread.
    pub fn current(_: &Thread) -> ThreadId {
        ThreadId(thread_id())
    }

    /// The id, for a log line.
    pub fn id(self) -> u32 {
        self.0
    }
}

/// A value only the thread that made it may touch (it holds the engine's
/// references, or work of the immediate context, which are that thread's),
/// kept in state a [`Lock`] shares between threads. Every way to the value
/// checks the calling thread: on another it is not there, and dropped on
/// another it is leaked, never touched. So the value need not be `Send`,
/// and no safe code can use it off its thread.
pub struct ThreadBound<T> {
    owner: ThreadId,
    value: ManuallyDrop<T>,
}

// SAFETY: another thread can only move a ThreadBound's bytes, read its
// owner or drop it: get, get_mut and into_inner give the value to its owner
// thread alone, and drop leaks it elsewhere. Windows gives each live thread
// its own id (the game's threads live until the process ends).
unsafe impl<T> Send for ThreadBound<T> {}

impl<T> ThreadBound<T> {
    /// `value`, the hook's thread's.
    pub fn new(thread: &Thread, value: T) -> ThreadBound<T> {
        ThreadBound { owner: ThreadId::current(thread), value: ManuallyDrop::new(value) }
    }

    /// The thread whose value it is.
    pub fn owner(&self) -> ThreadId {
        self.owner
    }

    /// The value, on its own thread.
    pub fn get(&self, thread: &Thread) -> Option<&T> {
        (ThreadId::current(thread) == self.owner).then_some(&*self.value)
    }

    /// The value, on its own thread.
    pub fn get_mut(&mut self, thread: &Thread) -> Option<&mut T> {
        (ThreadId::current(thread) == self.owner).then_some(&mut *self.value)
    }

    /// The value out, on its own thread; elsewhere it stays bound.
    pub fn into_inner(self, thread: &Thread) -> Result<T, ThreadBound<T>> {
        if ThreadId::current(thread) != self.owner {
            return Err(self);
        }
        let mut this = ManuallyDrop::new(self);
        // SAFETY: `this` is never dropped, so the value is taken once.
        Ok(unsafe { ManuallyDrop::take(&mut this.value) })
    }
}

impl<T> Drop for ThreadBound<T> {
    fn drop(&mut self) {
        if thread_id() == self.owner.0 {
            // SAFETY: dropped once, here; nothing reads it after.
            unsafe { ManuallyDrop::drop(&mut self.value) }
        }
    }
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

    #[test]
    fn a_bound_value_is_its_threads_alone() {
        use std::sync::{Arc, atomic::AtomicUsize};
        struct Counted(Arc<AtomicUsize>);
        impl Drop for Counted {
            fn drop(&mut self) { self.0.fetch_add(1, Ordering::Relaxed); }
        }
        let drops = Arc::new(AtomicUsize::new(0));
        // SAFETY: this host thread models its own native hook.
        let thread = unsafe { Thread::new() };
        let mine = ThreadBound::new(&thread, Counted(drops.clone()));
        let away = ThreadBound::new(&thread, Counted(drops.clone()));
        let owner = mine.owner();
        let back = std::thread::spawn(move || {
            // SAFETY: as above, on the other thread.
            let other = unsafe { Thread::new() };
            let mut mine = mine;
            assert!(mine.get(&other).is_none() && mine.get_mut(&other).is_none());
            assert_ne!(ThreadId::current(&other), owner);
            drop(away);
            mine.into_inner(&other).err().unwrap()
        }).join().unwrap();
        assert_eq!(drops.load(Ordering::Relaxed), 0, "dropped on another thread: leaked, never touched");
        assert!(back.get(&thread).is_some());
        drop(back.into_inner(&thread).ok().unwrap());
        assert_eq!(drops.load(Ordering::Relaxed), 1);
    }
}
