//! Patch 202: one line in the patches' log when the game cannot take the
//! mouse, with the reason, and one when it has it again (client_patch.md
//! 3.106). A poll with no mouse input is otherwise silent: a player who could
//! not aim in a match left no trace of why.
//!
//! A line only when the outcome of the mouse's poll changes, never a poll's:
//!
//! ```text
//! srp: mouse: cannot take the mouse: DIERR_OTHERAPPHASPRIO (0x80070005), exclusive, another window in front
//! srp: mouse: cannot read the mouse: 0x80001234, non-exclusive, the game in front
//! srp: mouse: taken again
//! ```

#![forbid(unsafe_code)]

use crate::vostok::log::{Piece, line, line_of};
use crate::vostok::mouse_device::{Failure, Front, Level, Unpolled};
use crate::vostok::{Context, Flow, Lock, NoExit, Thread};

/// What a poll of the mouse got.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Poll {
    /// Its state was read.
    Read,
    /// Acquire refused the device.
    Refused(Failure),
    /// The read failed with a code the game answers with no Acquire.
    Unread(Failure),
}

impl Poll {
    /// The poll after `self`: the new last poll, and whether it is told.
    pub fn then(self, next: Poll) -> (Poll, bool) {
        (next, next != self)
    }
}

/// The last poll's outcome. The game took the mouse before its first poll.
static LAST: Lock<Poll> = Lock::new(Poll::Read);

fn polled(thread: &Thread, poll: Poll) -> bool {
    LAST.with(thread, |last| {
        let (next, told) = last.then(poll);
        *last = next;
        told
    })
}

pub fn on_read(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if polled(ctx.thread(), Poll::Read) {
        line(&[Piece::Text("mouse: taken again")]);
    }
    Flow::Continue
}

pub fn on_read_failed(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(mouse) = Unpolled::read_failed(ctx) {
        tell(ctx.thread(), Poll::Unread(mouse.failure), "cannot read the mouse: ", mouse);
    }
    Flow::Continue
}

pub fn on_acquire_join(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(mouse) = Unpolled::acquire_failed(ctx) {
        tell(ctx.thread(), Poll::Refused(mouse.failure), "cannot take the mouse: ", mouse);
    }
    Flow::Continue
}

fn tell(thread: &Thread, poll: Poll, what: &'static str, mouse: Unpolled) {
    if !polled(thread, poll) {
        return;
    }
    let level = match mouse.level {
        Level::Exclusive => "exclusive",
        Level::NonExclusive => "non-exclusive",
    };
    let front = match mouse.front(thread) {
        Front::Game => "the game in front",
        Front::Other => "another window in front",
        Front::Nothing => "no window in front",
        Front::Unknown => "the window in front unknown",
    };
    let code = mouse.failure.code();
    let code: &[Piece] = match mouse.failure.name() {
        Some(name) => &[Piece::Text(name), Piece::Text(" ("), Piece::Hex(code), Piece::Text(")")],
        None => &[Piece::Hex(code)],
    };
    line_of(&[
        &[Piece::Text("mouse: "), Piece::Text(what)],
        code,
        &[Piece::Text(", "), Piece::Text(level), Piece::Text(", "), Piece::Text(front)],
    ]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_only_when_the_poll_changes() {
        let refused = Poll::Refused(Failure::of(0x8007_0005).unwrap());
        let other = Poll::Refused(Failure::of(0x8007_0006).unwrap());
        let unread = Poll::Unread(Failure::of(0x8007_0015).unwrap());
        let mut last = Poll::Read;
        let told: std::vec::Vec<bool> = [Poll::Read, refused, refused, other, unread, unread, Poll::Read, Poll::Read]
            .into_iter()
            .map(|poll| {
                let (next, told) = last.then(poll);
                last = next;
                told
            })
            .collect();
        assert_eq!(told, [false, true, false, true, true, false, true, false]);
    }
}
