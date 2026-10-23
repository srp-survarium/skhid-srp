//! Kit presets (`docs/notes/presets.md`): the lobby's side of
//! `session::v026g::presets` and of its wire
//! (`survarium::v026g::lobby::presets`, `lobby.md` §2.16.4). The Character
//! panel's title lists the presets and chooses the active one; a room
//! join may choose one first ("Switch and enter", `rooms`). The active
//! preset is the kits, so every kit change is already its own: nothing
//! here saves.
//!
//! Choosing changes the kits the way a move does: the client is sent the
//! inventory and the account ([`Core::push_account`]), as after `46 00`,
//! and squad mates see the new kits.

use session::LockExt;
use session::v026g::presets::{self, Applied, PresetError};
use session::v026g::quests;
use session::v026g::rules;
use survarium::v026g::lobby::presets::{self as wire, PresetDone, PresetRequest, PresetRow};

use super::core::{Core, Who};

impl Core<'_> {
    /// A presets request (`3B` at a presets start): answered with the list.
    pub(super) fn preset_request(&mut self, who: &Who, request: PresetRequest) {
        let (done, preset) = match request {
            PresetRequest::List => (PresetDone::Listed, 0),
            PresetRequest::Choose { preset } => match self.preset_choose(who, preset) {
                Ok(_) => (PresetDone::Chosen, preset),
                Err(_) => (PresetDone::Refused, preset),
            },
        };
        self.send_presets(who, done, preset);
    }

    /// Preset `preset` the asker's active one (`presets::choose`), not while
    /// it is in a match or on its way to one. Done: the client gets the
    /// inventory and the account, and squad mates the kits.
    pub(super) fn preset_choose(&mut self, who: &Who, preset: u8) -> Result<Applied, PresetError> {
        let free = self
            .player(who.addr)
            .is_some_and(|player| player.in_match.is_none() && player.connect.is_none());
        let chosen = if free {
            presets::choose(&mut who.account.locked(), preset, quests::now())
        } else {
            Err(PresetError::Busy)
        };
        match chosen {
            Ok(applied) => {
                log::info!(
                    "{}: preset {preset} active, {} slots left empty",
                    who.name,
                    applied.empty
                );
                self.push_account(who);
                self.squads_dirty = true;
            }
            Err(error) => log::info!("{}: choose preset {preset}: refused ({error})", who.name),
        }
        chosen
    }

    /// The asker's presets (`P2;L`), with what its request did, and the
    /// items they use. A slot left empty by a choice is said in the log
    /// only (the owner's of 2026-10-23: no notice).
    pub(super) fn send_presets(&mut self, who: &Who, done: PresetDone, preset: u8) {
        let (rows, active, playable, uses) = {
            let account = who.account.locked();
            let levels = presets::levels(&account, quests::now());
            let rows: Vec<PresetRow> = (1..=presets::PRESETS as u8)
                .zip(levels)
                .map(|(number, level)| PresetRow {
                    saved: presets::preset(&account, number).is_ok_and(|saved| saved.is_some()),
                    level,
                })
                .collect();
            (
                rows,
                account.active_preset,
                rules::kits_can_play(&account, quests::now()),
                presets::uses(&account),
            )
        };
        self.send_made(
            who.addr,
            wire::preset_list(done, preset, active, playable, &rows, &uses),
        );
    }
}
