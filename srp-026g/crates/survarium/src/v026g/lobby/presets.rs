//! Kit presets in the Character panel (`presets.md`): srp's own wire, on
//! the ranking page's request (`3B`) at reserved starts and its reply
//! (`91`), as the rooms are (`lobby.md` §2.16.4). Only srp's changed lobby
//! movie asks; a stock game never does.
//!
//! Every start is from [`PRESETS_FIRST`] to [`PRESETS_LAST`], above the
//! rooms and within an `int`. Every request is answered with the list
//! ([`preset_list`]) at [`PRESETS_FIRST`].

use super::super::loadouts::{PerkSlot, SlotPerk};
use super::push::{Ranked, elo_rows};
use super::rooms::PRESET_COUNT;
use super::{ServerMessage, WireError, limits};

/// The first start: the list of the asker's presets.
pub const PRESETS_FIRST: u32 = 2_090_000_000;
/// `PRESET_CHOOSE_BASE + preset`, preset 1 to [`PRESET_COUNT`]: make that
/// preset the active one (its kits put on).
pub const PRESET_CHOOSE_BASE: u32 = PRESETS_FIRST + 10;
/// `PERK_BASE + kit × 10 + perk × 2 + code`: the kit `kit` (0 to 2, the account's kits in order,
/// as the list's [`PERKS`] row) takes or declines the slot of `perk` ([`SlotPerk::index`]: 0
/// Artifact Container, 1 Deep Pockets or Supply Pocket), `code` [`PerkSlot::code`] (1 taken, 2
/// declined): 1 the container, 2 the pocket again; 3 the pocket, 4 the second weapon again.
pub const PERK_BASE: u32 = PRESETS_FIRST + 20;
/// The last start.
pub const PRESETS_LAST: u32 = PRESETS_FIRST + 99;
/// The first field of the header's name. P2: no count of slots left empty
/// (the owner's of 2026-10-23: a slot whose item is gone is just empty),
/// and the items the presets use.
pub const PRESETS_VERSION: &str = "P2";
/// The reason bit of the maintenance rule in the list's `<blocks>`: next
/// to the kit's (`session::v026g::rules::PlayBlock`, bits 1 to 16), set by
/// the lobby (`Core::entry_blocks`), not by the kit.
pub const BLOCK_MAINTENANCE: u8 = 32;

/// The first field of a row of the items the presets use.
pub const USES: &str = "U";
/// The first field of the row of the kits' perk slots.
pub const PERKS: &str = "K";

/// A presets request, by its start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresetRequest {
    List,
    Choose { preset: u8 },
    /// The kit at `kit` (0 to 2) takes or declines `perk`'s slot.
    Perk {
        kit: u8,
        perk: SlotPerk,
        slot: PerkSlot,
    },
}

/// The presets request a reserved start is, if it is one.
pub fn preset_request(start: u32) -> Option<PresetRequest> {
    let count = u32::from(PRESET_COUNT);
    let kits = limits::KITS as u32;
    match start {
        PRESETS_FIRST => Some(PresetRequest::List),
        _ if (PRESET_CHOOSE_BASE + 1..=PRESET_CHOOSE_BASE + count).contains(&start) => {
            Some(PresetRequest::Choose {
                preset: (start - PRESET_CHOOSE_BASE) as u8,
            })
        }
        _ if (PERK_BASE..PERK_BASE + 10 * kits).contains(&start) => {
            let offset = start - PERK_BASE;
            let perk = *SlotPerk::ALL.get(((offset % 10).checked_sub(1)? / 2) as usize)?;
            let slot = PerkSlot::from_code(offset % 10 - perk.index() as u32 * 2)?;
            Some(PresetRequest::Perk {
                kit: (offset / 10) as u8,
                perk,
                slot,
            })
        }
        _ => None,
    }
}

/// What a request did, said in the list's header.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresetDone {
    /// Only the list.
    Listed = 0,
    /// The preset asked for is the active one now.
    Chosen = 1,
    /// Nothing changed: the asker is in a match, or a kit rule refused.
    Refused = 2,
}

/// One preset of the list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PresetRow {
    /// It holds something (the active one: the kits do).
    pub saved: bool,
    /// Its gear level, as a room's max level is checked; none: none of its
    /// kits can play.
    pub level: Option<u8>,
}

/// Everything the list says ([`preset_list`]).
#[derive(Clone, Copy, Debug)]
pub struct PresetList<'a> {
    /// What the request did.
    pub done: PresetDone,
    /// The preset it was about; 0 for none.
    pub preset: u8,
    /// The active preset.
    pub active: u8,
    /// Why no room may be entered now, a set of bits.
    pub blocks: u8,
    /// To a planned maintenance; none: none planned.
    pub minutes: Option<u32>,
    /// Each preset, 1 first.
    pub rows: &'a [PresetRow],
    /// Each kit's id and its choice of each perk ([`SlotPerk::ALL`]), the account's kits in
    /// order; none: the account lacks that perk.
    /// has no such slot (the account lacks the skill).
    pub perks: &'a [(u32, [Option<PerkSlot>; SlotPerk::ALL.len()])],
    /// The items the presets use, item ids rising, each with its presets.
    pub uses: &'a [(u32, u8)],
}

/// `91` at [`PRESETS_FIRST`]: the header
/// `P2;L;<done>;<preset>;<active>;<blocks>;<minutes>` (`done` a
/// [`PresetDone`], `preset` the one it was about, 0 for the list; `active`
/// the active preset; `blocks` why no room may be entered now, a set of
/// bits: the kit a match would take can not play
/// (`session::v026g::rules::PlayBlock`), [`BLOCK_MAINTENANCE`]; 0: none.
/// The rooms' entries' rule, by which the movie greys Play and Join and
/// lists the reasons in their tip. `minutes` to a planned maintenance, -1
/// for none: the tip's number, and the movie asks again while one is
/// planned), then a row a preset, 1 first:
/// `<preset>;<saved>;<level>` (`level` -1: none of its kits can play, or it
/// is empty), then the kits' perk slots, `K;<kit id>:<container>:<weapon>,...` (the kits in
/// order; each [`PerkSlot::code`] of [`SlotPerk::ALL`], 0 when the account lacks the perk; a movie
/// before it reads it as no preset), then the items the presets use, `U;<item id>:<presets>,...`
/// (`presets` bit `n - 1` for preset n), as many to a row as a name
/// holds, item ids rising. The lobby asks before a sale of one of them
/// (`presets.md`). Rows past [`PRESETS_LAST`] are left out: the server
/// sells as ever, only the question is missing.
pub fn preset_list(list: &PresetList<'_>) -> Result<ServerMessage, WireError> {
    let names = list_names(list);
    let rows: Vec<(u32, Ranked)> = names
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            (
                PRESETS_FIRST + index as u32,
                Ranked {
                    name,
                    rating: 0,
                    rating_before: 0,
                },
            )
        })
        .collect();
    let rows: Vec<(u32, &Ranked)> = rows.iter().map(|(place, row)| (*place, row)).collect();
    elo_rows(&rows)
}

fn list_names(list: &PresetList<'_>) -> Vec<String> {
    let minutes = list.minutes.map_or(-1, i64::from);
    let most = (PRESETS_LAST - PRESETS_FIRST + 1) as usize;
    let mut names = vec![format!(
        "{PRESETS_VERSION};L;{};{};{};{};{minutes}",
        list.done as u8, list.preset, list.active, list.blocks
    )];
    for (index, row) in list.rows.iter().take(usize::from(PRESET_COUNT)).enumerate() {
        let level = row.level.map_or(-1, i32::from);
        names.push(format!("{};{};{level}", index + 1, u8::from(row.saved)));
    }
    let perks: Vec<String> = list
        .perks
        .iter()
        .take(limits::KITS)
        .map(|(id, slots)| {
            let codes: Vec<String> = slots.iter().map(|slot| slot.map_or(0, PerkSlot::code).to_string()).collect();
            format!("{id}:{}", codes.join(":"))
        })
        .collect();
    names.push(format!("{PERKS};{}", perks.join(",")));
    let mut row = String::new();
    for (item_id, presets) in list.uses {
        let entry = format!("{item_id}:{presets}");
        if !row.is_empty() && row.len() + 1 + entry.len() > limits::NAME {
            names.push(std::mem::take(&mut row));
        }
        if row.is_empty() {
            row = format!("{USES};{entry}");
        } else {
            row.push(',');
            row.push_str(&entry);
        }
    }
    if !row.is_empty() {
        names.push(row);
    }
    names.truncate(most);
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list<'a>(
        done: PresetDone,
        rows: &'a [PresetRow],
        perks: &'a [(u32, [Option<PerkSlot>; 2])],
        uses: &'a [(u32, u8)],
    ) -> PresetList<'a> {
        PresetList {
            done,
            preset: u8::from(done == PresetDone::Chosen),
            active: 1,
            blocks: 0,
            minutes: None,
            rows,
            perks,
            uses,
        }
    }

    #[test]
    fn every_start_is_one_request_inside_an_int_past_the_rooms() {
        const {
            assert!(PRESETS_FIRST > super::super::rooms::ROOMS_LAST);
            assert!(PRESETS_LAST <= i32::MAX as u32);
            assert!(PRESETS_LAST < super::super::REPLAY_START);
            assert!(PERK_BASE + 10 * limits::KITS as u32 <= PRESETS_LAST);
        }
        assert_eq!(preset_request(PRESETS_FIRST), Some(PresetRequest::List));
        assert_eq!(preset_request(PRESET_CHOOSE_BASE), None);
        assert_eq!(
            preset_request(PRESET_CHOOSE_BASE + 1),
            Some(PresetRequest::Choose { preset: 1 })
        );
        assert_eq!(
            preset_request(PRESET_CHOOSE_BASE + 5),
            Some(PresetRequest::Choose { preset: 5 })
        );
        assert_eq!(preset_request(PRESET_CHOOSE_BASE + 6), None);
        assert_eq!(preset_request(PRESETS_FIRST + 1), None);
        assert_eq!(
            preset_request(PERK_BASE + 2),
            Some(PresetRequest::Perk {
                kit: 0,
                perk: SlotPerk::ArtifactContainer,
                slot: PerkSlot::Declined
            })
        );
        assert_eq!(
            preset_request(PERK_BASE + 21),
            Some(PresetRequest::Perk {
                kit: 2,
                perk: SlotPerk::ArtifactContainer,
                slot: PerkSlot::Taken
            })
        );
        assert_eq!(
            preset_request(PERK_BASE + 14),
            Some(PresetRequest::Perk {
                kit: 1,
                perk: SlotPerk::WeaponPocket,
                slot: PerkSlot::Declined
            })
        );
        assert_eq!(preset_request(PERK_BASE), None);
        assert_eq!(preset_request(PERK_BASE + 15), None);
        assert_eq!(preset_request(PERK_BASE + 31), None);
    }

    #[test]
    fn the_list_says_each_preset_the_active_one_and_what_was_done() {
        let rows = [
            PresetRow {
                saved: true,
                level: Some(7),
            },
            PresetRow::default(),
            PresetRow {
                saved: true,
                level: None,
            },
            PresetRow::default(),
            PresetRow::default(),
        ];
        let (taken, declined) = (Some(PerkSlot::Taken), Some(PerkSlot::Declined));
        let perks = [(1, [taken, None]), (2, [declined, taken]), (7, [taken, declined])];
        assert_eq!(
            list_names(&list(PresetDone::Chosen, &rows, &perks, &[(12, 1), (40, 5)])),
            [
                "P2;L;1;1;1;0;-1",
                "1;1;7",
                "2;0;-1",
                "3;1;-1",
                "4;0;-1",
                "5;0;-1",
                "K;1:1:0,2:2:1,7:1:2",
                "U;12:1,40:5"
            ]
        );
        let without = [(1, [None; 2]), (2, [None; 2]), (3, [None; 2])];
        assert_eq!(
            list_names(&list(PresetDone::Listed, &rows, &without, &[]))[6],
            "K;1:0:0,2:0:0,3:0:0"
        );
        assert!(preset_list(&list(PresetDone::Listed, &rows, &without, &[])).is_ok());
    }

    /// The items of five full presets: rows of at most a name each, every
    /// item once, and never a place past the presets' last.
    #[test]
    fn the_items_the_presets_use_fill_rows_of_a_name_each() {
        let uses: Vec<(u32, u8)> = (0..345).map(|n| (1_000_000 + n, 31)).collect();
        let names = list_names(&list(
            PresetDone::Listed,
            &[PresetRow::default(); 5],
            &[(1, [None; 2]), (2, [None; 2]), (3, [None; 2])],
            &uses,
        ));
        assert!(names.len() <= (PRESETS_LAST - PRESETS_FIRST + 1) as usize);
        assert!(names.iter().all(|name| name.len() <= limits::NAME));
        let listed: Vec<&str> = names[7..]
            .iter()
            .flat_map(|name| name.strip_prefix("U;").unwrap().split(','))
            .collect();
        assert_eq!(listed.len(), uses.len());
        assert_eq!(listed[0], "1000000:31");
        assert_eq!(listed[344], "1000344:31");
    }
}
