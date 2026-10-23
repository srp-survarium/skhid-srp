//! Kit presets in the Character panel (`presets.md`): srp's own wire, on
//! the ranking page's request (`3B`) at reserved starts and its reply
//! (`91`), as the rooms are (`lobby.md` §2.16.4). Only srp's changed lobby
//! movie asks; a stock game never does.
//!
//! Every start is from [`PRESETS_FIRST`] to [`PRESETS_LAST`], above the
//! rooms and within an `int`. Every request is answered with the list
//! ([`preset_list`]) at [`PRESETS_FIRST`].

use super::push::{Ranked, elo_rows};
use super::rooms::PRESET_COUNT;
use super::{ServerMessage, WireError, limits};

/// The first start: the list of the asker's presets.
pub const PRESETS_FIRST: u32 = 2_090_000_000;
/// `PRESET_CHOOSE_BASE + preset`, preset 1 to [`PRESET_COUNT`]: make that
/// preset the active one (its kits put on).
pub const PRESET_CHOOSE_BASE: u32 = PRESETS_FIRST + 10;
/// The last start.
pub const PRESETS_LAST: u32 = PRESETS_FIRST + 99;
/// The first field of the header's name. P2: no count of slots left empty
/// (the owner's of 2026-10-23: a slot whose item is gone is just empty),
/// and the items the presets use.
pub const PRESETS_VERSION: &str = "P2";
/// The first field of a row of the items the presets use.
pub const USES: &str = "U";

/// A presets request, by its start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresetRequest {
    List,
    Choose { preset: u8 },
}

/// The presets request a reserved start is, if it is one.
pub fn preset_request(start: u32) -> Option<PresetRequest> {
    let count = u32::from(PRESET_COUNT);
    match start {
        PRESETS_FIRST => Some(PresetRequest::List),
        _ if (PRESET_CHOOSE_BASE + 1..=PRESET_CHOOSE_BASE + count).contains(&start) => {
            Some(PresetRequest::Choose {
                preset: (start - PRESET_CHOOSE_BASE) as u8,
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

/// `91` at [`PRESETS_FIRST`]: the header `P2;L;<done>;<preset>;<active>`
/// (`done` a [`PresetDone`], `preset` the one it was about, 0 for the
/// list; `active` the active preset), then a row a preset, 1 first:
/// `<preset>;<saved>;<level>` (`level` -1: none of its kits can play, or it
/// is empty), then the items the presets use, `U;<item id>:<presets>,...`
/// (`presets` bit `n - 1` for preset n), as many to a row as a name
/// holds, item ids rising. The lobby asks before a sale of one of them
/// (`presets.md`). Rows past [`PRESETS_LAST`] are left out: the server
/// sells as ever, only the question is missing.
pub fn preset_list(
    done: PresetDone,
    preset: u8,
    active: u8,
    rows: &[PresetRow],
    uses: &[(u32, u8)],
) -> Result<ServerMessage, WireError> {
    let names = list_names(done, preset, active, rows, uses);
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

fn list_names(
    done: PresetDone,
    preset: u8,
    active: u8,
    rows: &[PresetRow],
    uses: &[(u32, u8)],
) -> Vec<String> {
    let most = (PRESETS_LAST - PRESETS_FIRST + 1) as usize;
    let mut names = vec![format!(
        "{PRESETS_VERSION};L;{};{preset};{active}",
        done as u8
    )];
    for (index, row) in rows.iter().take(usize::from(PRESET_COUNT)).enumerate() {
        let level = row.level.map_or(-1, i32::from);
        names.push(format!("{};{};{level}", index + 1, u8::from(row.saved)));
    }
    let mut row = String::new();
    for (item_id, presets) in uses {
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

    #[test]
    fn every_start_is_one_request_inside_an_int_past_the_rooms() {
        const {
            assert!(PRESETS_FIRST > super::super::rooms::ROOMS_LAST);
            assert!(PRESETS_LAST <= i32::MAX as u32);
            assert!(PRESETS_LAST < super::super::REPLAY_START);
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
        assert_eq!(
            list_names(PresetDone::Chosen, 1, 1, &rows, &[(12, 1), (40, 5)]),
            [
                "P2;L;1;1;1",
                "1;1;7",
                "2;0;-1",
                "3;1;-1",
                "4;0;-1",
                "5;0;-1",
                "U;12:1,40:5"
            ]
        );
        assert!(preset_list(PresetDone::Listed, 0, 3, &rows, &[]).is_ok());
    }

    /// The items of five full presets: rows of at most a name each, every
    /// item once, and never a place past the presets' last.
    #[test]
    fn the_items_the_presets_use_fill_rows_of_a_name_each() {
        let uses: Vec<(u32, u8)> = (0..345).map(|n| (1_000_000 + n, 31)).collect();
        let names = list_names(PresetDone::Listed, 0, 1, &[PresetRow::default(); 5], &uses);
        assert!(names.len() <= (PRESETS_LAST - PRESETS_FIRST + 1) as usize);
        assert!(names.iter().all(|name| name.len() <= limits::NAME));
        let listed: Vec<&str> = names[6..]
            .iter()
            .flat_map(|name| name.strip_prefix("U;").unwrap().split(','))
            .collect();
        assert_eq!(listed.len(), uses.len());
        assert_eq!(listed[0], "1000000:31");
        assert_eq!(listed[344], "1000344:31");
    }
}
