//! Report an unreadable replay as a user error, before either native assertion.
#![forbid(unsafe_code)]

use crate::vostok::replay_file;
use crate::vostok::{Context, Flow, NoExit};

pub fn before_open(ctx: &mut Context<'_>) -> Flow<NoExit> {
    replay_file::return_open_errors(ctx);
    Flow::Continue
}

pub fn open_result(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(failure) = replay_file::failure(ctx) {
        failure.tell_and_exit();
    }
    Flow::Continue
}
