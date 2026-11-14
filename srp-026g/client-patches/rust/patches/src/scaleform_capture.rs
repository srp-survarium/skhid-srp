//! Patch 93: Scaleform's render thread no longer reaches a DrawableImage
//! command queue the main thread is destroying.
//!
//! A DrawableImageContext keeps a list of its command queues, which its
//! lock guards for the main thread's adds and removes, but the render
//! thread's walk at each capture (DrawableImageContext::OnNextCapture,
//! 0x572660) reads the list without it: a queue whose last reference goes
//! on the main thread is destroyed and freed while the walk is at it or is
//! about to go to it (the rev-40 crash at a match's end: EnterCriticalSection
//! of a freed queue in updateCPUModifiedImagesRT, 0x5ED340). The hooks are
//! `scaleform_capture.toml`'s:
//!
//! - the walk (`scaleform.di_next_capture`, after OnNextCapture's own
//!   ExecuteNextCapture): each queue the list holds whose count is not 0 is
//!   given the capture, in the list's order, a reference of ours held across
//!   its call; the context's lock is held only to read the list and take
//!   that reference, never across a call into the game
//!   ([`NextCapture::notify_queues`]);
//! - a queue's destructor (`scaleform.di_queue_destroy`): the queue leaves
//!   its context's list first, while its count is still 0; the stock body
//!   then writes the count 2 and removes it again, which finds it gone.
//!
//! The proof is client_patch.md section 3.61. Nothing a match computes: the
//! interface's rendering only.

#![forbid(unsafe_code)]

use crate::vostok::drawable_image::{DyingQueue, NextCapture};
use crate::vostok::{Context, Flow, NoExit, ScaleformCaptureOnNextCaptureExit};

/// OnNextCapture's walk of its queues (0x572675): ours instead, then its
/// epilogue.
pub fn on_next_capture(ctx: &mut Context<'_>) -> Flow<ScaleformCaptureOnNextCaptureExit> {
    let Some(capture) = NextCapture::at(ctx) else { return Flow::Continue };
    capture.notify_queues();
    Flow::Exit(ScaleformCaptureOnNextCaptureExit::Done)
}

/// DICommandQueue's destructor's start (0x5ED1E0).
pub fn on_queue_destroy(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(queue) = DyingQueue::at(ctx) {
        queue.leave_context();
    }
    Flow::Continue
}
