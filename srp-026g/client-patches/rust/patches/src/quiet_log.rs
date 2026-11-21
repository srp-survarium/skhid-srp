//! Patch 173: the engine's log, and so the console (it shows that file),
//! without the engine's lines that tell a player or a bug report nothing
//! (the owner, 2026-11-21: "what about console having million garbage
//! lines?"). At log_message's start (`logging.message`) a line of a format
//! below is not written; every other line is, as before. Each is native, at
//! every start, match or connection, and has no cause the game data or the
//! server could take away:
//! - the lobby menu's "Unknown Client status received" for every 98 and 9B,
//!   messages it has no work for (lobby.md: the client's own flaw);
//! - the renderer's list of every display mode of every monitor;
//! - "atmosphere recalculated (...)" at each scene and weather change;
//! - "QOS_CONTROLLER ENABLED" at each connection;
//! - "Console command [..] not found for options_item": the stock options no
//!   variable backs (the main menu greys them);
//! - "<file>, new id = N", an error for a baked texture loaded from its
//!   folder, the cache's hit (patch 70);
//! - "cook of '' [quid N][class 107] failed": the camouflage baker's type
//!   query for a type it bakes nothing for asks with no name and fails at
//!   once, one or two a baked surface (client_patch.md, patch 70). A failed
//!   cook of anything else is written.

#![forbid(unsafe_code)]

use crate::vostok::game_log::{self, Line};
use crate::vostok::{Context, Flow, QuietLogOnMessageExit};

pub fn on_message(ctx: &mut Context<'_>) -> Flow<QuietLogOnMessageExit> {
    if quiet(game_log::line(ctx)) {
        Flow::Exit(QuietLogOnMessageExit::Quiet)
    } else {
        Flow::Continue
    }
}

fn quiet(line: Line<'_>) -> bool {
    match line {
        Line::UnknownClientStatus
        | Line::DisplayMode
        | Line::AtmosphereRecalculated
        | Line::QosControllerEnabled
        | Line::UnboundOption
        | Line::BakedTextureLoaded => true,
        Line::CookFailed { query } => nameless_type_query(query),
        Line::Other => false,
    }
}

/// A camouflage_decal_texture_type_baker_cook's query (class 107, 6Bh) with
/// no name: `'' [quid N][class 107]`.
fn nameless_type_query(query: &[u8]) -> bool {
    query.starts_with(b"'' [quid ") && query.ends_with(b"][class 107]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_cook_is_quiet_only_for_the_bakers_nameless_type_query() {
        let cook = |query: &'static str| quiet(Line::CookFailed { query: query.as_bytes() });
        assert!(cook("'' [quid 74851][class 107]"));
        assert!(!cook("'unused string' [quid 212119][class 116]"));
        assert!(!cook("'resources/textures/a' [quid 3][class 107]"));
        assert!(!cook("'' [quid 3][class 1070]"));
        assert!(!quiet(Line::Other));
        assert!(quiet(Line::DisplayMode));
    }
}
