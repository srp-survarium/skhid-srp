//! Patch 62: one key hides the whole interface and shows it again, in a
//! match and in the lobby, for screenshots (client_patch.md section 3.54;
//! the rules: docs/notes/hide-ui-key.md).
//!
//! The key is the game's own action toggle_replay_ui (19h, F11 by
//! default), which in the original only shows and hides the replay
//! viewer's controls.
//! - The match: without the replay's movie the hud movie is asked
//!   root.interface_hidden, and both it and the player icons' movie get
//!   root.hide_interface(not that); the movies go on running. The key is
//!   not taken, as in the original. A new match starts shown: after the hud
//!   movie's root.reset the hud and the icons get root.hide_interface(false).
//! - The lobby: a key with no lobby action is asked again for the match's
//!   kind; toggle_replay_ui invokes the lobby movie's root.toggle_interface
//!   and is taken (the movie never gets it). While the movie says its
//!   interface is hidden, any other key goes to the movie with no action of
//!   the game (a window's key, F6's Play): the movie shows the interface.
//! - The options' controls: a 29th row, toggle_replay_ui in the game's
//!   group (vostok::interface::SRP_CONTROLS, which the build fills).

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::interface::{Hud, LobbyAction, MatchReset, MatchToggle};
use crate::vostok::{Context, Flow, HideUiOnLobbyActionExit, HideUiOnMatchResetExit, HideUiOnMatchToggleExit, Reg};

/// toggle_replay_ui.
const TOGGLE_UI: u32 = 0x19;
/// The binder's answer for a key without an action.
const NO_ACTION: u32 = 0x49;
/// The kind the game world asks the binder for (0x25CD5F).
const MATCH_KINDS: u32 = 1;
const INTERFACE_HIDDEN: &CStr = c"root.interface_hidden";
const HIDE_INTERFACE: &CStr = c"root.hide_interface";
const TOGGLE_INTERFACE: &CStr = c"root.toggle_interface";

/// The hud movie and the player icons' movie hidden or shown.
fn hide_hud(hud: &Hud<'_>, hidden: bool) {
    for movie in [hud.movie(), hud.icons_movie()].into_iter().flatten() {
        movie.invoke_bool(HIDE_INTERFACE, hidden);
    }
}

/// `hide_ui.match_toggle`: the key in a match.
pub fn on_match_toggle(ctx: &mut Context<'_>) -> Flow<HideUiOnMatchToggleExit> {
    let toggle = MatchToggle::at(ctx);
    if toggle.replay() {
        return Flow::Continue; // the replay's controls, as before
    }
    if let Some(hud) = toggle.hud()
        && let Some(movie) = hud.movie()
    {
        let hidden = movie.ask_bool(INTERFACE_HIDDEN);
        hide_hud(&hud, !hidden);
    }
    Flow::Exit(HideUiOnMatchToggleExit::NotTaken)
}

/// What the lobby's call ends in.
enum Lobby {
    Answered(u32),
    Taken,
    ToMovie,
}

/// `hide_ui.lobby_action`: a key in the lobby.
pub fn on_lobby_action(ctx: &mut Context<'_>) -> Flow<HideUiOnLobbyActionExit> {
    let end = {
        let call = LobbyAction::at(ctx);
        let answer = call.lobby_action();
        if answer == NO_ACTION && call.action(MATCH_KINDS) == TOGGLE_UI
            && let Some(movie) = call.movie()
        {
            movie.invoke_none(TOGGLE_INTERFACE);
            Lobby::Taken
        } else if call.movie().is_some_and(|movie| movie.ask_bool(INTERFACE_HIDDEN)) {
            Lobby::ToMovie
        } else {
            Lobby::Answered(answer)
        }
    };
    match end {
        Lobby::Answered(answer) => {
            ctx.set_reg(Reg::Eax, answer);
            Flow::Exit(HideUiOnLobbyActionExit::Answered)
        }
        Lobby::Taken => Flow::Exit(HideUiOnLobbyActionExit::Taken),
        Lobby::ToMovie => Flow::Exit(HideUiOnLobbyActionExit::ToMovie),
    }
}

/// `hide_ui.match_reset`: the reset, then the hud and the icons shown.
pub fn on_match_reset(ctx: &mut Context<'_>) -> Flow<HideUiOnMatchResetExit> {
    let answer = {
        let reset = MatchReset::at(ctx);
        let answer = reset.invoke();
        if let Some(hud) = reset.hud() {
            hide_hud(&hud, false);
        }
        answer
    };
    ctx.set_reg(Reg::Eax, answer);
    Flow::Exit(HideUiOnMatchResetExit::Reset)
}
