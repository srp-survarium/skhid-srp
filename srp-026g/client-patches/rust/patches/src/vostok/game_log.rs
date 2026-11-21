//! The engine's own log, the game's `Survarium_<user>.log`, which the
//! console shows (console_impl::tick draws that file): a line at
//! logging::log_message's start (`logging.message`), before it is written,
//! known by its format, the exe's own C string at every caller.

use super::Context;
use super::ui::c_string;
use crate::raw;

/// The engine lines this crate knows, by their format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line<'a> {
    /// "Unknown Client status received. type = %d": the lobby menu's default
    /// for a message it has no work for (every 98 and 9B, lobby.md).
    UnknownClientStatus,
    /// "  %dx%d": a display mode of a monitor, at the renderer's start.
    DisplayMode,
    /// "atmosphere recalculated (...)": at a scene's or the weather's change.
    AtmosphereRecalculated,
    /// "QOS_CONTROLLER ENABLED": at every connection.
    QosControllerEnabled,
    /// "Console command [%s] not found for options_item [%d]": a stock option
    /// no variable backs.
    UnboundOption,
    /// "%s, new id = %u" (an error): a baked texture loaded from its folder,
    /// a cache hit.
    BakedTextureLoaded,
    /// "cook of %s failed": a resource query failed; `query` its description,
    /// `'<name>' [quid Q][class C]`.
    CookFailed { query: &'a [u8] },
    Other,
}

/// The longest query description read: a name of up to 260 and its numbers.
const QUERY: usize = 0x200;

/// The line log_message is about to write.
pub fn line<'c>(ctx: &'c Context<'_>) -> Line<'c> {
    // SAFETY: the site's ESP, log_message's cdecl arguments while it runs:
    // the format a C string of the exe at every caller; for "cook of %s
    // failed" the first argument the query's log_string, a C string its
    // caller (finish_query_impl 0xF0DBC) keeps until the call returns.
    unsafe {
        let Some(frame) = raw::site::logging_message::frame(ctx) else { return Line::Other };
        match frame.format() as usize as u32 {
            raw::image::CLIENT_STATUS_FORMAT => Line::UnknownClientStatus,
            raw::image::DISPLAY_MODE_FORMAT => Line::DisplayMode,
            raw::image::ATMOSPHERE_FORMAT => Line::AtmosphereRecalculated,
            raw::image::QOS_CONTROLLER_FORMAT => Line::QosControllerEnabled,
            raw::image::UNBOUND_OPTION_FORMAT => Line::UnboundOption,
            raw::image::BAKED_TEXTURE_LOADED_FORMAT => Line::BakedTextureLoaded,
            raw::image::COOK_FAILED_FORMAT => match c_string(frame.first(), QUERY) {
                Some(query) => Line::CookFailed { query: query.to_bytes() },
                None => Line::Other,
            },
            _ => Line::Other,
        }
    }
}
