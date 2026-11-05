//! State a patch keeps between hooks, and shares with the patches that name
//! it (`uses` in the TOML, rust_patches.md). It lives in a `static` (the
//! arena's `patches.state`, zeros at load); the game is single-threaded
//! through the hooks, so a [`GameCell`] gives `&mut` while a
//! [`Thread`](super::Thread) proves we are inside a hook.

use super::hook::Thread;
use core::cell::UnsafeCell;

/// A cell a patch reaches only from inside a hook. `T: Copy` and small (a
/// flag, a handle value, a counter): no `Drop`, no borrow kept past the
/// hook.
pub struct GameCell<T> {
    value: UnsafeCell<T>,
}

// SAFETY: the game drives every hook on one thread (Thread is not Send), so
// the `&mut` get_mut gives never overlaps another; nothing here is touched
// off that thread.
unsafe impl<T> Sync for GameCell<T> {}

impl<T: Copy> GameCell<T> {
    pub const fn new(value: T) -> GameCell<T> {
        GameCell { value: UnsafeCell::new(value) }
    }

    pub fn get(&self, _: &Thread) -> T {
        // SAFETY: on the game's thread (the Thread), the only one that
        // touches the cell; no other borrow is live across a call here.
        unsafe { *self.value.get() }
    }

    pub fn set(&self, _: &Thread, value: T) {
        // SAFETY: as get.
        unsafe { *self.value.get() = value }
    }

    /// Reads, maps and writes back in one step (no borrow kept).
    pub fn update(&self, thread: &Thread, f: impl FnOnce(T) -> T) {
        let next = f(self.get(thread));
        self.set(thread, next);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holds_and_updates() {
        static COUNT: GameCell<u32> = GameCell::new(0);
        // SAFETY: the test is the only thread.
        let thread = unsafe { Thread::new() };
        assert_eq!(COUNT.get(&thread), 0);
        COUNT.update(&thread, |n| n + 2);
        assert_eq!(COUNT.get(&thread), 2);
    }
}
