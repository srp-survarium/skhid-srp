//! Scaleform's DrawableImageContext and its DICommandQueues (patch 93).
//!
//! A context keeps a list of its queues: a queue is its own list link (prev
//! +8, next +0Ch), the list's end is inside the context (+40h), and the
//! context's lock (+24h) guards it. The main thread adds a queue (its
//! constructor, AddCaptureNotify 0x5726B0) and removes it (its destructor,
//! RemoveCaptureNotify 0x5726F0), each holding the lock for its few link
//! writes alone; the render thread walks the list at every capture
//! (OnNextCapture 0x572660).
//!
//! A queue is counted (RefCountImpl, +4); Release destroys it at 0, on
//! whichever thread let it go. So the walk here:
//!
//! - reads the list only with the context's lock held, and holds it for
//!   nothing else: no call into the game is made with it held, so it stays
//!   the leaf lock it is in the stock code (the main thread takes it with a
//!   queue's own lock held: mergeQueueWith 0x5731F0 releases a queue there);
//! - gives a queue its capture only with a reference of its own to it,
//!   taken with the lock held and only while its count is not 0: a queue
//!   whose count went to 0 is being destroyed (its destructor waits for the
//!   lock to leave the list), and is passed over;
//! - lets its reference go with the lock left (a last reference destroys the
//!   queue, whose destructor takes the lock).
//!
//! A queue at 0 still in the list is the one state the walk must recognise,
//! so the destructor's hook ([`DyingQueue`]) removes the queue before the
//! stock body writes its count 2 (0x5ED1ED): in the list, a queue's count
//! is its true one.

use core::marker::PhantomData;
use core::sync::atomic::{AtomicU32, Ordering};

use super::{Context, Thread};
use crate::raw;

/// A DrawableImageContext, on the hook's thread.
#[derive(Clone, Copy)]
struct DrawableImageContext<'t> {
    raw: raw::DrawableImageContext,
    _thread: PhantomData<&'t Thread>,
}

/// The context's lock, held: left when dropped.
struct Locked {
    section: raw::CriticalSection,
}

impl Locked {
    fn enter(context: DrawableImageContext<'_>) -> Locked {
        // SAFETY: the context is alive (DrawableImageContext's contract);
        // its +24h is the Scaleform::Lock (a CRITICAL_SECTION) that
        // Add/RemoveCaptureNotify take, initialised by its constructor.
        let section = unsafe { context.raw.queue_lock() };
        // SAFETY: kernel32's EnterCriticalSection on that section; Drop leaves
        // it on this thread, once.
        unsafe { (raw::engine_enter_critical_section)(Some(section)) };
        Locked { section }
    }
}

impl Drop for Locked {
    fn drop(&mut self) {
        // SAFETY: entered by `enter` on this thread.
        unsafe { (raw::engine_leave_critical_section)(Some(self.section)) };
    }
}

/// One more reference on a count, unless it is 0 (its object is being
/// destroyed): the reference taken.
fn take_reference(count: &AtomicU32) -> bool {
    let mut now = count.load(Ordering::Relaxed);
    loop {
        if now == 0 {
            return false;
        }
        match count.compare_exchange_weak(now, now.wrapping_add(1), Ordering::Acquire, Ordering::Relaxed) {
            Ok(_) => return true,
            Err(seen) => now = seen,
        }
    }
}

/// A queue with a reference of ours: alive, and in its context's list,
/// until dropped (Release, which may destroy it).
struct HeldQueue<'t> {
    raw: raw::DiCommandQueue,
    _thread: PhantomData<&'t Thread>,
}

impl<'t> HeldQueue<'t> {
    /// A reference to the queue `link` is, with its context's lock held
    /// (`_locked`): None when its count is 0.
    fn take(link: raw::DiQueueLink, _locked: &Locked) -> Option<HeldQueue<'t>> {
        // SAFETY: the link is in the list, read with the lock held: its
        // queue's destructor has not removed it yet (it takes the lock to),
        // so its memory is the queue's; its count is the dword the game
        // changes with InterlockedExchangeAdd alone, 4-aligned.
        let count = unsafe { AtomicU32::from_ptr(link.queue().references()) };
        // SAFETY: as above, the queue this link is.
        take_reference(count).then(|| HeldQueue { raw: unsafe { link.queue() }, _thread: PhantomData })
    }

    /// The link after this queue's (lock held).
    fn next(&self, _locked: &Locked) -> Option<raw::DiQueueLink> {
        // SAFETY: the queue is alive (our reference) and so still in its
        // list (its destructor removes it); its link read with the lock held.
        unsafe { self.raw.link().next() }
    }

    fn on_next_capture(&self, notify: raw::RenderNotify) {
        // SAFETY: DICommandQueue::OnNextCapture, what the stock walk calls
        // on each queue with the same notify, on this (the render) thread;
        // the queue alive (our reference); no lock of ours held.
        unsafe { raw::engine_di_queue_next_capture(Some(self.raw), Some(notify)) }
    }
}

impl Drop for HeldQueue<'_> {
    fn drop(&mut self) {
        // SAFETY: the reference `take` added; Release destroys the queue at
        // 0, and its destructor takes the context's lock, which is never
        // held here (a HeldQueue is dropped outside `next_live`).
        unsafe { raw::engine_di_queue_release(Some(self.raw)) }
    }
}

impl<'t> DrawableImageContext<'t> {
    /// With the lock held: the first queue from `link` on, the list's end
    /// excluded, whose count is not 0, a reference of ours taken.
    fn live_from(self, mut link: Option<raw::DiQueueLink>, locked: &Locked) -> Option<HeldQueue<'t>> {
        // SAFETY: the context is alive (its contract); +40h is the list's end.
        let end = unsafe { self.raw.queues() };
        loop {
            // A link of 0: a removed queue's, never in the list; the walk ends.
            let at = link?;
            if at.address() == end.address() {
                return None;
            }
            if let Some(queue) = HeldQueue::take(at, locked) {
                return Some(queue);
            }
            // SAFETY: a queue being destroyed is still in the list while the
            // lock is held: its link is read as the list's.
            link = unsafe { at.next() };
        }
    }

    /// The first live queue of the list (None), or the first after `after`.
    /// The lock is held for this alone.
    fn next_live(self, after: Option<&HeldQueue<'t>>) -> Option<HeldQueue<'t>> {
        let locked = Locked::enter(self);
        let start = match after {
            Some(queue) => queue.next(&locked),
            // SAFETY: the context is alive; the end's next is the first queue.
            None => unsafe { self.raw.queues().next() },
        };
        self.live_from(start, &locked)
    }

    /// `visit` on each queue of the list whose count is not 0, in the
    /// list's order, a reference of ours held across it and the lock not
    /// held. The reference to one is let go once the next one is taken: the
    /// context stays alive while we hold a queue (a queue holds a reference
    /// to its context), and once we hold none it is not touched again.
    fn for_each_queue(self, mut visit: impl FnMut(&HeldQueue<'t>)) {
        let mut held = self.next_live(None);
        while let Some(queue) = held {
            visit(&queue);
            held = self.next_live(Some(&queue));
            drop(queue);
        }
    }
}

/// A capture at `scaleform.di_next_capture` (OnNextCapture after its own
/// ExecuteNextCapture, the render thread): the context and the notify.
pub struct NextCapture<'t> {
    context: DrawableImageContext<'t>,
    notify: raw::RenderNotify,
}

impl<'t> NextCapture<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<NextCapture<'t>> {
        use raw::site::scaleform_di_next_capture as site;
        // SAFETY: the context is this site's: ESI the ContextCaptureNotify
        // base of the context whose OnNextCapture runs (RTHandle::NextCapture
        // calls it on the live context), EBX the notify it was given.
        let (capture, notify) = unsafe { (site::capture(ctx)?, site::notify(ctx)?) };
        // SAFETY: the base is part of that context (context+8).
        let context = unsafe { capture.context() };
        Some(NextCapture { context: DrawableImageContext { raw: context, _thread: PhantomData }, notify })
    }

    /// Each live queue's OnNextCapture with this capture's notify, as the
    /// stock walk calls it, safely ([`DrawableImageContext::for_each_queue`]).
    pub fn notify_queues(self) {
        let notify = self.notify;
        self.context.for_each_queue(|queue| queue.on_next_capture(notify));
    }
}

/// A queue at its destructor's start (`scaleform.di_queue_destroy`): its
/// count 0, still in its context's list.
pub struct DyingQueue<'t> {
    queue: raw::DiCommandQueue,
    context: raw::DrawableImageContext,
    _thread: PhantomData<&'t Thread>,
}

impl<'t> DyingQueue<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<DyingQueue<'t>> {
        // SAFETY: the context is this site's: ECX the queue being destroyed
        // (its memory not yet freed); its context (+34h) the stock body's
        // RemoveCaptureNotify's `this`, still referenced by the queue.
        let queue = unsafe { raw::site::scaleform_di_queue_destroy::queue(ctx)? };
        // SAFETY: as above.
        let context = unsafe { queue.context()? };
        Some(DyingQueue { queue, context, _thread: PhantomData })
    }

    /// Out of the context's list (RemoveCaptureNotify, the stock body's own
    /// call, made before the body writes the count 2; its own call then
    /// finds the queue's links 0 and leaves the list as it is).
    pub fn leave_context(self) {
        // SAFETY: the context's RemoveCaptureNotify with this queue, on the
        // destructor's thread, as the stock body calls it at 0x5ED1F4.
        unsafe { raw::engine_di_capture_remove(Some(self.context), Some(self.queue)) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reference_is_taken_only_while_the_count_is_not_0() {
        let count = AtomicU32::new(0);
        assert!(!take_reference(&count));
        assert_eq!(count.load(Ordering::Relaxed), 0);
        count.store(1, Ordering::Relaxed);
        assert!(take_reference(&count));
        assert_eq!(count.load(Ordering::Relaxed), 2);
        assert!(take_reference(&count));
        assert_eq!(count.load(Ordering::Relaxed), 3);
    }
}
