//! The chat: survarium::chat_handler, which gets every received line
//! (add_message 0x208AD0, stdcall: the handler, the channel, the body, the
//! sender's name, the clan tag; `ret 14h`) and gives it to the hud's movie in
//! a match, else to the lobby's.

use core::ffi::c_void;
use core::ptr::NonNull;

use super::hook::{Handle, Thread};
use super::object::read;
use crate::raw;

/// A chat channel (add_message's second argument): 0 the server, 1
/// general, 2 system, 3 clan, 4 private, 5 the match, 6 and 7 the teams, 8
/// the squad, 9. A newtype: a line passes on with the channel it came with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Channel(pub u32);

impl Channel {
    /// A players' channel the hud shows in a match (3 clan, 5 the match, 6
    /// and 7 the teams, 8 the squad, 9); the server's (0, 2) and the
    /// general and private ones (1, 4, which add_message gives the lobby's
    /// movie) are not.
    pub fn shown_in_match(self) -> bool {
        matches!(self.0, 3 | 5..=9)
    }
}

/// A text of a line (its body, the sender's name or clan tag), passed on
/// as the game gave it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChatText(pub(in crate::vostok) u32);

impl ChatText {
    pub(in crate::vostok) fn ptr(self) -> *const c_void {
        self.0 as usize as *const c_void
    }
}

/// The chat handler, while a line comes in. A [`Handle`]: the generated
/// shim makes it from the hooked call's argument.
pub struct ChatHandler<'t> {
    this: NonNull<c_void>,
    thread: &'t Thread,
}

impl<'t> Handle<'t> for ChatHandler<'t> {
    unsafe fn from_raw(thread: &'t Thread, raw: u32) -> Option<Self> {
        NonNull::new(raw as usize as *mut c_void).map(|this| ChatHandler { this, thread })
    }
}

impl<'t> ChatHandler<'t> {
    pub(in crate::vostok) fn ptr(&self) -> *mut c_void {
        self.this.as_ptr()
    }

    /// The hook's thread (the handler is a hook's argument).
    pub fn thread(&self) -> &'t Thread {
        self.thread
    }

    /// In a match: set_mode's flag (+0Dh), the hud's movie the chat's.
    pub fn in_match(&self) -> bool {
        // SAFETY: the game's chat handler (Handle's contract), on its thread.
        unsafe { read(raw::chat_handler::in_match(self.ptr())) != 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_huds_channels() {
        let shown: [u32; 7] = [3, 5, 6, 7, 8, 9, 3];
        assert!(shown.iter().all(|&c| Channel(c).shown_in_match()));
        assert!([0, 1, 2, 4, 10, u32::MAX].iter().all(|&c| !Channel(c).shown_in_match()));
    }
}
