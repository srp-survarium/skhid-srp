//! Patch 218, a file check (`srp_validate::checks`): when a match's first
//! world is in, the game tells the match server the `resources.db.patch_<n>`
//! it mounted; when the server asks, the size and sha256 of each file it
//! names, as the game mounts it (the copy in the last archive that has
//! it, `run`), each table read a few pages at a time (`table`), a bounded
//! part a frame. The game's side: `vostok::file_check`.
#![forbid(unsafe_code)]
use crate::vostok::{Context, Flow, NoExit};

pub mod run;
pub mod table;

pub fn on_loaded(ctx: &mut Context<'_>) -> Flow<NoExit> {
    crate::vostok::file_check::loaded(ctx);
    Flow::Continue
}

pub fn on_frame(ctx: &mut Context<'_>) -> Flow<NoExit> {
    crate::vostok::file_check::frame(ctx);
    Flow::Continue
}

pub fn on_close(ctx: &mut Context<'_>) -> Flow<NoExit> {
    crate::vostok::file_check::close(ctx);
    Flow::Continue
}
