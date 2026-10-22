//! Kit presets (`docs/notes/presets.md`): the lobby's side of
//! `session::v026g::presets` and of its wire
//! (`survarium::v026g::lobby::presets`, `lobby.md` §2.16.4). The Character
//! panel's dropdown lists, saves and puts on presets; a room join may put
//! one on first ("Switch and enter", `rooms`).
//!
//! Putting one on changes the kits the way a move does: the client is sent
//! the inventory and the account ([`Core::push_account`]), as after `46 00`,
//! and squad mates see the new kits.

use session::LockExt;
use session::v026g::presets::{self, Applied, PresetError};
use session::v026g::quests;
use survarium::v026g::lobby::presets::{self as wire, PresetDone, PresetRequest, PresetRow};

use super::core::{Core, Who};

impl Core<'_> {
    /// A presets request (`3B` at a presets start): answered with the list.
    pub(super) fn preset_request(&mut self, who: &Who, request: PresetRequest) {
        let (done, preset, empty) = match request {
            PresetRequest::List => (PresetDone::Listed, 0, 0),
            PresetRequest::Save { preset } => {
                let saved = presets::save(&mut who.account.locked(), preset);
                match saved {
                    Ok(any) => {
                        self.accounts.mark_dirty(who.account_id);
                        log::info!(
                            "{}: saved its kits into preset {preset}{}",
                            who.name,
                            if any { "" } else { " (empty)" }
                        );
                        (PresetDone::Saved, preset, 0)
                    }
                    Err(error) => {
                        log::info!("{}: save preset {preset}: refused ({error})", who.name);
                        (PresetDone::Refused, preset, 0)
                    }
                }
            }
            PresetRequest::Apply { preset } => match self.preset_apply(who, preset) {
                Ok(applied) => (PresetDone::Applied, preset, applied.empty),
                Err(_) => (PresetDone::Refused, preset, 0),
            },
        };
        self.send_presets(who, done, preset, empty);
    }

    /// Preset `preset` onto the asker's kits (`presets::apply`), not while it
    /// is in a match or on its way to one. Done: the client gets the
    /// inventory and the account, and squad mates the kits.
    pub(super) fn preset_apply(&mut self, who: &Who, preset: u8) -> Result<Applied, PresetError> {
        let free = self
            .player(who.addr)
            .is_some_and(|player| player.in_match.is_none() && player.connect.is_none());
        let applied = if free {
            presets::apply(&mut who.account.locked(), preset)
        } else {
            Err(PresetError::Busy)
        };
        match applied {
            Ok(applied) => {
                log::info!(
                    "{}: put preset {preset} on its kits, {} slots left empty",
                    who.name,
                    applied.empty
                );
                self.push_account(who);
                self.squads_dirty = true;
            }
            Err(error) => log::info!("{}: apply preset {preset}: refused ({error})", who.name),
        }
        applied
    }

    /// The asker's presets (`P1;L`), with what its request did.
    pub(super) fn send_presets(&mut self, who: &Who, done: PresetDone, preset: u8, empty: u32) {
        let rows: Vec<PresetRow> = {
            let account = who.account.locked();
            let levels = presets::levels(&account, quests::now());
            account
                .presets
                .iter()
                .zip(levels)
                .map(|(saved, level)| PresetRow {
                    saved: saved.is_some(),
                    level,
                })
                .collect()
        };
        self.send_made(who.addr, wire::preset_list(done, preset, empty, &rows));
    }
}
