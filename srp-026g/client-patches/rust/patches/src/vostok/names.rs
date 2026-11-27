//! Names kept for the process: a pool a patch fills from what it parsed (a server message's model
//! paths, patch `reticle`) and hands out as `&'static CStr`. An entry, once in, is never written
//! again, so its C string may be read from any thread for as long as the process runs; a new one
//! goes into the next free slot, under the pool's own lock. Nothing allocates: the pool is a
//! static of `N` slots of `LEN` bytes.

use core::cell::UnsafeCell;
use core::ffi::CStr;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub struct NamePool<const N: usize, const LEN: usize> {
    /// A writer holds it while it looks for an entry and adds one.
    writing: AtomicBool,
    /// The slots published, `0..count`: each a C string that never changes.
    count: AtomicUsize,
    slots: UnsafeCell<[[u8; LEN]; N]>,
}

// SAFETY: a slot is written only before it is published (count raised with Release, under the
// lock), and read only after (count loaded with Acquire); a published slot is never written.
unsafe impl<const N: usize, const LEN: usize> Sync for NamePool<N, LEN> {}

impl<const N: usize, const LEN: usize> NamePool<N, LEN> {
    pub const fn new() -> NamePool<N, LEN> {
        NamePool { writing: AtomicBool::new(false), count: AtomicUsize::new(0), slots: UnsafeCell::new([[0; LEN]; N]) }
    }

    /// The published slot `index` (below the count the caller loaded).
    fn slot(&'static self, index: usize) -> Option<&'static CStr> {
        // SAFETY: a published slot (index below the Acquire-loaded count): written once, before it
        // was published, and never again; its bytes hold a zero (intern writes one).
        let bytes: &'static [u8; LEN] = unsafe { &*self.slots.get().cast::<[u8; LEN]>().add(index) };
        CStr::from_bytes_until_nul(bytes).ok()
    }

    /// `text` as a C string that lives for the process: the entry of the same bytes, or a new
    /// one. None for a text with a zero in it or of `LEN` bytes or more, or when the pool is full.
    pub fn intern(&'static self, text: &[u8]) -> Option<&'static CStr> {
        if text.len() >= LEN || text.contains(&0) {
            return None;
        }
        while self.writing.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            core::hint::spin_loop();
        }
        let count = self.count.load(Ordering::Acquire);
        let found = (0..count).filter_map(|i| self.slot(i)).find(|name| name.to_bytes() == text);
        let interned = match found {
            Some(name) => Some(name),
            None if count < N => {
                // SAFETY: slot `count` is past every published one: nothing refers to it, and the
                // lock makes this the only writer; the text fits with its zero.
                unsafe {
                    let place = self.slots.get().cast::<[u8; LEN]>().add(count).cast::<u8>();
                    core::ptr::copy_nonoverlapping(text.as_ptr(), place, text.len());
                    place.add(text.len()).write(0);
                }
                self.count.store(count + 1, Ordering::Release);
                self.slot(count)
            }
            None => None,
        };
        self.writing.store(false, Ordering::Release);
        interned
    }
}

impl<const N: usize, const LEN: usize> Default for NamePool<N, LEN> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::NamePool;

    #[test]
    fn an_entry_is_kept_once_and_never_moves() {
        static POOL: NamePool<2, 8> = NamePool::new();
        let dot = POOL.intern(b"dot").unwrap();
        assert_eq!(dot.to_bytes(), b"dot");
        let again = POOL.intern(b"dot").unwrap();
        assert!(core::ptr::eq(dot, again));
        let ring = POOL.intern(b"ring").unwrap();
        assert_eq!(ring.to_bytes(), b"ring");
        // full, too long, a zero inside
        assert_eq!(POOL.intern(b"cross"), None);
        assert_eq!(POOL.intern(b"dot").map(|name| name.as_ptr()), Some(dot.as_ptr()));
        assert_eq!(POOL.intern(b"12345678"), None);
        assert_eq!(POOL.intern(b"a\0b"), None);
        assert_eq!(dot.to_bytes(), b"dot");
    }
}
