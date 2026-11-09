//! Patch 131: the Settings' "Allow others to watch my matches live", under
//! "Friends:" on the Gameplay tab (docs/notes/replay-stream.md, "Live"). The
//! setting is the account's and srp's, not this computer's (the HUD group's
//! are user.cfg's, patch 83): srp says it in `0xA1`'s last byte, and a box
//! applied sends `3B` at a reserved start, the wire of a lobby pick
//! (patch 81). The match list and the watch ask are srp's alone: this patch
//! only shows the box and carries the answer.
//!
//! Until `0xA1` says (and at the login screen, whose next sign-in may be
//! another account's) the state is not known and the box is not shown; a
//! lobby connection that is down shows none and sends nothing.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::lobby_menu::{SquadId, SquadRead};
use crate::vostok::settings::OptionsCall;
use crate::vostok::{Context, Flow, GameCell, LiveWatchOnOptionsCallExit, NoExit, Thread};

/// The Settings' call, and the box's name.
pub const NAME: &CStr = c"srp_live_watch";
/// The box's label, a dictionary key.
pub const LABEL: &CStr = c"st_srp_live_watch_option";
/// srp's start of the setting (`3B`, its `LIVE_WATCH_BASE`, 2 096 000 000):
/// a ranking start, not an address, so made of small numbers.
const BASE: u32 = 2096 * 1000 * 1000;

/// The account's setting as this game knows it. Zero is not known (the
/// state starts as zeros; the wire's ids are 1 and 2).
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    Unknown = 0,
    Allowed = 1,
    Hidden = 2,
}

impl Setting {
    /// The setting `0xA1` says; none for another byte.
    pub fn of_id(id: u8) -> Option<Setting> {
        match id {
            1 => Some(Setting::Allowed),
            2 => Some(Setting::Hidden),
            _ => None,
        }
    }

    /// The Settings' "1" (the box ticked: allowed) or "0".
    pub fn of_box(value: &CStr) -> Option<Setting> {
        match value.to_bytes() {
            b"1" => Some(Setting::Allowed),
            b"0" => Some(Setting::Hidden),
            _ => None,
        }
    }

    /// What the box is told: "1", "0", or "" while not known (no box).
    pub fn box_text(self) -> &'static CStr {
        match self {
            Setting::Unknown => c"",
            Setting::Allowed => c"1",
            Setting::Hidden => c"0",
        }
    }

    /// The `3B` start of a known setting.
    pub fn start(self) -> Option<u32> {
        match self {
            Setting::Unknown => None,
            known => Some(BASE + known as u32),
        }
    }
}

static SETTING: GameCell<Setting> = GameCell::new(Setting::Unknown);

/// `0xA1`'s last byte, when it has one (an older srp's has not).
fn told(thread: &Thread, byte: Option<u8>) {
    if let Some(setting) = byte.and_then(Setting::of_id) {
        SETTING.set(thread, setting);
    }
}

/// `0xA1` with its squad id read: none (0) has the byte after the id.
pub fn on_squad_id(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(read) = SquadId::at(ctx)
        && read.squad_id() == 0
    {
        told(ctx.thread(), read.live_watch());
    }
    Flow::Continue
}

/// A squad's `0xA1`, read to its end: the byte after the squad.
pub fn on_squad(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(read) = SquadRead::at(ctx) {
        told(ctx.thread(), read.live_watch());
    }
    Flow::Continue
}

/// The login screen shown: the next sign-in may be another account.
pub fn on_login(ctx: &mut Context<'_>) -> Flow<NoExit> {
    SETTING.set(ctx.thread(), Setting::Unknown);
    Flow::Continue
}

/// The Settings' call: "1" or "0" applied is sent to srp when connected and
/// known; every call answers the name, its label and the state shown
/// (`root.srp_check_option`).
pub fn on_options_call(ctx: &mut Context<'_>) -> Flow<LiveWatchOnOptionsCallExit> {
    let Some(call) = OptionsCall::at(ctx) else { return Flow::Continue };
    if call.method() != NAME {
        return Flow::Continue;
    }
    let thread = ctx.thread();
    let lobby = call.lobby();
    if SETTING.get(thread) != Setting::Unknown
        && let Some(wanted) = call.string_argument(0).and_then(Setting::of_box)
        && let Some(start) = wanted.start()
        && let Some(lobby) = &lobby
        && lobby.send_reserved(start).is_ok()
    {
        SETTING.set(thread, wanted);
    }
    let shown = if lobby.is_some_and(|lobby| lobby.connected()) { SETTING.get(thread) } else { Setting::Unknown };
    let mut label = [0; 0x200];
    let label = call.translate(LABEL, &mut label);
    call.answer(c"root.srp_check_option", &[NAME, label, shown.box_text()]);
    Flow::Exit(LiveWatchOnOptionsCallExit::Done)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wire_id_is_a_known_setting_and_no_other_is() {
        assert_eq!(Setting::of_id(1), Some(Setting::Allowed));
        assert_eq!(Setting::of_id(2), Some(Setting::Hidden));
        for id in [0, 3, 255] {
            assert_eq!(Setting::of_id(id), None);
        }
    }

    #[test]
    fn the_box_is_ticked_when_allowed_and_only_one_or_zero_is_a_choice() {
        assert_eq!(Setting::of_box(c"1"), Some(Setting::Allowed));
        assert_eq!(Setting::of_box(c"0"), Some(Setting::Hidden));
        for value in [c"", c"2", c"10", c"yes"] {
            assert_eq!(Setting::of_box(value), None);
        }
        assert_eq!(Setting::Allowed.box_text(), c"1");
        assert_eq!(Setting::Hidden.box_text(), c"0");
        assert_eq!(Setting::Unknown.box_text(), c"");
    }

    #[test]
    fn the_starts_are_the_servers() {
        assert_eq!(Setting::Allowed.start(), Some(BASE + 1));
        assert_eq!(Setting::Hidden.start(), Some(BASE + 2));
        assert_eq!(Setting::Unknown.start(), None);
    }
}
