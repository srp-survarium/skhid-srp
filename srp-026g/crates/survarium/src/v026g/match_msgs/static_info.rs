//! The static info, 0xE4: what a 0.26g match is started with
//! (`docs/notes/static_info.md`, all of it; layout §6).
//!
//! The message is `u8 local_player_index` and then a raw LZMA2 stream of
//! the match options, which decode to:
//!
//! ```text
//! 20 × player_profile     always 20; the places past N are empty
//! u8  map_id              a maps_dict id: the map, the mode and its timings
//! u8  N                   the roster size
//! u32 match id            the match's seed; not the lobby's match id
//! u16 match level         logged only
//! 28 × f32                score weights
//! 20 × u8                 squad ids
//! ```
//!
//! The client reads the fields above and nothing after them; srp's block
//! after them ("SRPX", then tagged entries) is read by exe patches only:
//! tag 2 is the Blackout flag (patch 65, `docs/notes/blackout.md`; its
//! reader is at the two calls of `match_options::deserialize`, 0x27B928 and
//! 0x1F096C); tag 1 is kept for team damage (#370).
//!
//! The client checks nothing of this: a string, a kit or an item too many
//! writes over its memory, an item the dictionary lacks crashes it later.
//! So [`StaticInfo::new`] refuses what the client cannot hold, and
//! [`parse_options`] reads the bytes back the way the client does, for
//! tests.
//!
//! The profile body (team … items) is the lobby's account body too
//! (`lobby.md` §2.5; the same client function 0x1F1070).

use super::super::dictionary::{Dictionary, PROFILE_SLOTS};
use super::super::loadouts::Loadout;
use super::super::skills::{MODIFIERS, write_modifier_block};
use super::super::{lzma2, maps};
use super::snapshot::event;
use super::{MAX_PLAYERS, ServerMessage};

/// A profile holds 3 kits.
pub const MAX_KITS: usize = 3;
/// A profile's item table holds 69 items.
pub const MAX_ITEMS: usize = 69;
/// Name buffers are 64 bytes.
pub const MAX_NAME: usize = 63;
/// The clan abbreviation's buffer is 15 bytes.
pub const MAX_CLAN: usize = 14;
/// An item holds 8 modifications and 3 upgrades inline; more is a fatal
/// assert.
pub const MAX_MODIFICATIONS: usize = 8;
pub const MAX_UPGRADES: usize = 3;
/// Score events, each with a weight.
pub const SCORE_EVENTS: usize = 28;
/// The client decodes the options into a buffer of this size.
pub const OPTIONS_BUFFER: usize = 0x578A0;
/// What one transport message carries at most: 255 parts of at least 483
/// bytes (`transport.md` §5).
pub const MAX_MESSAGE: usize = 255 * 483;

/// The new-player protection value that protects nothing: the damage
/// factor is `max(0, 1 − 0.1 × value)`.
pub const NO_PROTECTION: u32 = 10;

/// A kit the player may use in the match: the client loads a player
/// resource only for these.
pub const KIT_USABLE: u8 = 0;
/// A kit that is not loaded in the match (the kit's constructor default,
/// and what the lobby calls a premium kit). The client makes a player
/// object only for a kit of state 0 (0x2CC684..0x2CC7C7), and a kit
/// choice (0xDE, relayed as 0xF3) for a kit without one changes nothing
/// on any client (0x3217CC: `cmp dword [slot + kit*4], 0`). PROVEN.
pub const KIT_NOT_LOADED: u8 = 2;
/// A whole item's condition [LIKELY: the lobby shows `value × 0.0001`].
pub const FULL_CONDITION: u32 = 1_000_000;

/// Weights of the 28 score events for the scoreboard. They count score
/// only, not the winner. The original server's values are not in the
/// client or its data (PROVEN: the options' weights are only read from
/// the wire, 0x1F0EE5, and used at 0x26DBD6 / 0x26DC77; no config lists
/// per-event numbers).
///
/// Three are sourced (the wiki's "Activity Rewards" of February 2015,
/// 0.26i; `progression_sources.md` §1): a kill 10, an assist 5, a stolen
/// battery 35. The two results screens of the time fit the first two (the
/// score less 10 a kill and 5 a help is small and positive in both). The
/// stolen battery is event 17 [LIKELY]: the credit for a battery that was
/// the enemy's, shared out among those who carried it when it is put into
/// the container (`snapshot_rules.md` §3.1); the client counts it in
/// hundredths and weighs it × 0.01, so a battery stolen alone scores 35.
///
/// The others are srp's own: the weights chosen for 0.21d divided by ten,
/// as they were ten times this scale (a kill was 100). 24..=27 (solo
/// battery delivery, grenade kill, stolen battery brought home, a new
/// player's match: `snapshot::event`) stay 0, so that a new event changes
/// nobody's score until the owner sets a weight; 26 would count a stolen
/// battery a second time.
///
/// The match pays by the score of the events that are no kill and no help
/// (`session::v026g::pay`), with these same weights.
///
/// An anomaly's artefact scores [`ARTEFACT_SCORE`] taken into an empty
/// artefact slot and [`ARTEFACT_SCORE_SLOTS_FULL`] with every slot full
/// (the owner, 2026-10-29; `docs/notes/artefact-score.md`): the game counts
/// event 5 for each, exe patch 80 counts event 16 too for the first.
///
/// A battery (the owner, 2026-11-01; `docs/notes/battery_credit.md`): found
/// 5, put in 5, a delivery's share 5 (event 6, in hundredths), and 2 a 10 m
/// it was brought nearer its container in its life (exe patch 92: event 6,
/// paid at the delivery).
pub const SCORE_WEIGHTS: [f32; SCORE_EVENTS] = [
    10.0, // kill (sourced)
    5.0,  // assist (sourced)
    0.0,  // death
    0.0,  // suicide
    0.0,  // team kill
    ARTEFACT_SCORE_SLOTS_FULL, // artefact (every one an anomaly gives)
    5.0,  // bring victory item (x0.01 by the client): a delivery's share 5; patch 92: 2 a 10 m of progress, paid at the delivery
    2.5,  // teammate cure
    2.5,  // teammate rescue
    2.5,  // revenge
    5.0,  // victory item carrier kill
    0.0,  // participating
    0.0,  // won
    2.5,  // melee kill
    2.5,  // silent kill
    2.5,  // headshot kill
    ARTEFACT_SCORE - ARTEFACT_SCORE_SLOTS_FULL, // an artefact into an empty slot (patch 80)
    35.0, // steal victory item (x0.01) (sourced)
    10.0, // recover victory item (x0.01)
    5.0,  // victory item found (the owner, 2026-11-01)
    5.0,  // put victory item in container
    2.5,  // entrap enemy
    0.0,  // bullet fired
    0.0,  // bullet hit
    0.0,  // solo battery delivery
    0.0,  // grenade kill
    0.0,  // stolen battery brought home
    0.0,  // a new player's match
];

/// An anomaly's artefact taken into an empty artefact slot: event 5 and
/// event 16 together (the owner, 2026-10-29).
pub const ARTEFACT_SCORE: f32 = 15.0;
/// An anomaly's artefact taken with every artefact slot full (it takes the
/// first slot's place): event 5 alone. Less, so that a player hands one to
/// a teammate first (the owner, 2026-10-29).
pub const ARTEFACT_SCORE_SLOTS_FULL: f32 = 5.0;

/// What Research (mode 5) weighs otherwise: its rule counts a unit's
/// capture in event 6 and a neutralisation in event 18 (hundredths, a
/// player's share of the 100 a capture gives), and a "kill invader" in
/// event 10 (`research_mode.md`, "Score events"). The weights are 0.27d's
/// awards (`gameplay/awards/events.options`: `capture_the_point` 30,
/// `kill_invader` 10): a whole capture or neutralisation scores 30.
pub const RESEARCH_WEIGHTS: [(usize, f32); 3] = [
    (event::RESEARCH_CAPTURE, 30.0),
    (event::RESEARCH_NEUTRALISE, 30.0),
    (event::KILL_INVADER, 10.0),
];

/// The weights of a match of `game_mode`: `weights`, with Research's own
/// in mode 5. The other modes play with `weights` as they are.
pub fn weights_of(game_mode: u8, weights: [f32; SCORE_EVENTS]) -> [f32; SCORE_EVENTS] {
    let mut weights = weights;
    if game_mode == maps::mode::RESEARCH {
        for (event, weight) in RESEARCH_WEIGHTS {
            weights[event] = weight;
        }
    }
    weights
}

/// srp's weights for a match of `game_mode` ([`SCORE_WEIGHTS`],
/// [`weights_of`]).
pub fn score_weights(game_mode: u8) -> [f32; SCORE_EVENTS] {
    weights_of(game_mode, SCORE_WEIGHTS)
}

/// An upgrade on an item: `u32 id, u32, u64, u8 param`. Only the id and
/// the param are understood.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Upgrade {
    pub id: u32,
    pub unknown_4: u32,
    pub unknown_8: u64,
    pub param: u8,
}

/// An item of a profile's table (`inventory_item_descr`, 0x1F1350).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MatchItem {
    /// The item's own id.
    pub instance_id: u32,
    /// 0 is an empty entry: nothing after it is written.
    pub dict_id: u16,
    /// A stack's count; anything else's condition ([`FULL_CONDITION`]).
    pub condition_or_stack: u32,
    /// Written for items that are no stack.
    pub repair_time: u32,
    pub reroll_time: u32,
    /// `(id, state)`: a modification (false) or a defect (true). The
    /// client looks the id up in its table of that kind and crashes if it
    /// is not there (`super::super::modifications`).
    pub modifications: Vec<(u32, bool)>,
    pub upgrades: Vec<Upgrade>,
}

/// One of a profile's kits.
#[derive(Clone, Debug, PartialEq)]
pub struct MatchKit {
    /// [`KIT_USABLE`], or 2 (the client's default; GUESS locked).
    pub state: u8,
    pub id: u32,
    /// Empty: the client shows its own default name.
    pub name: String,
    /// By slot: `(index into the profile's items, count)`. A count of 0
    /// is skipped by the client.
    pub slots: [Option<(u16, u16)>; PROFILE_SLOTS],
    /// The 68 modifiers the kit's skills give
    /// (`skills::SkillsTree::modifiers`); zeros are not written.
    pub modifiers: [f32; MODIFIERS],
}

impl MatchKit {
    /// A usable kit with nothing in it.
    pub fn empty(id: u32, name: &str) -> Self {
        Self {
            state: KIT_USABLE,
            id,
            name: name.to_owned(),
            slots: [None; PROFILE_SLOTS],
            modifiers: [0.0; MODIFIERS],
        }
    }
}

/// One roster place.
#[derive(Clone, Debug, PartialEq)]
pub struct MatchProfile {
    /// 0 or 1: the maps have spawn points for those two.
    pub team: u8,
    pub name: String,
    /// The clan abbreviation, shown as `[tag] name`; empty for none.
    pub clan_tag: String,
    /// [`NO_PROTECTION`] for none.
    pub protection: u32,
    pub kits: Vec<MatchKit>,
    /// The table the kits' slots index.
    pub items: Vec<MatchItem>,
    /// The squad table's byte for this place, not a part of the profile
    /// on the wire: players with the same non-zero id are a squad.
    pub squad: u8,
}

impl MatchProfile {
    /// A player with one usable kit holding `loadout`, each item whole and
    /// with no modifications; the items' ids count up from
    /// `first_instance_id`.
    pub fn from_loadout(
        name: &str,
        team: u8,
        loadout: &Loadout,
        modifiers: [f32; MODIFIERS],
        dictionary: &Dictionary,
        first_instance_id: u32,
    ) -> Self {
        Self::from_loadouts(
            name,
            team,
            &[loadout],
            modifiers,
            dictionary,
            first_instance_id,
        )
    }

    /// A player with a usable kit for each of `loadouts` (ids 1 on, at
    /// most [`MAX_KITS`]), each with items of its own (no item is in two
    /// kits, so each kit's ammo is its own), whole and with no
    /// modifications; the items' ids count up from `first_instance_id`.
    pub fn from_loadouts(
        name: &str,
        team: u8,
        loadouts: &[&Loadout],
        modifiers: [f32; MODIFIERS],
        dictionary: &Dictionary,
        first_instance_id: u32,
    ) -> Self {
        let mut items = Vec::new();
        let mut kits = Vec::new();
        for (index, loadout) in loadouts.iter().take(MAX_KITS).enumerate() {
            let mut kit = MatchKit::empty(index as u32 + 1, "");
            kit.modifiers = modifiers;
            for item in &loadout.items {
                let Some(slot) = kit.slots.get_mut(usize::from(item.slot)) else {
                    continue;
                };
                let is_stack = dictionary.is_stack(item.dict_id);
                let count = if is_stack { item.count } else { 1 };
                *slot = Some((items.len() as u16, count.min(u32::from(u16::MAX)) as u16));
                items.push(MatchItem {
                    instance_id: first_instance_id.wrapping_add(items.len() as u32),
                    dict_id: item.dict_id,
                    condition_or_stack: if is_stack { item.count } else { FULL_CONDITION },
                    ..MatchItem::default()
                });
            }
            kits.push(kit);
        }
        Self {
            team,
            name: name.to_owned(),
            clan_tag: String::new(),
            protection: NO_PROTECTION,
            kits,
            items,
            squad: 0,
        }
    }
}

/// The match options.
#[derive(Clone, Debug, PartialEq)]
pub struct MatchOptions {
    /// The roster, by place: N is its length. The wire's other places are
    /// written empty.
    pub players: Vec<MatchProfile>,
    /// A playable `maps_dict` id ([`maps`]).
    pub map_id: u8,
    /// What the client logs as `Match id`: the seed of the match's
    /// anomalies, victory items and player objects. It must not be the id
    /// the lobby's `82` gives (`lobby::client_rules::connects_to_match`).
    pub match_id: u32,
    pub match_level: u16,
    pub score_weights: [f32; SCORE_EVENTS],
    /// A Blackout match (srp's block, tag 2; exe patch 65,
    /// `docs/notes/blackout.md`): no interface for anyone, the players'
    /// markers on Tab. `false`: the entry not sent.
    pub blackout: bool,
}

/// srp's block after the options: its magic, then tagged entries
/// `{u8 tag, u8 len, len bytes}`.
pub const SRPX_MAGIC: &[u8; 4] = b"SRPX";
/// The entry of [`MatchOptions::blackout`]: one byte, 1.
pub const SRPX_BLACKOUT: u8 = 2;

/// Why options cannot go to a client, or why bytes are no options.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StaticInfoError {
    /// No players, or more than 20.
    Roster(usize),
    /// Not a map and mode the client can load.
    Map(u8),
    /// A player's team is not 0 or 1.
    Team { player: usize, team: u8 },
    /// A player's name, clan tag or kit name is too long for its buffer.
    Text {
        player: usize,
        len: usize,
        max: usize,
    },
    /// More than 3 kits.
    Kits { player: usize, kits: usize },
    /// No kit the player can use (state 0): the client would load no
    /// player for the place.
    NoUsableKit { player: usize },
    /// More than 69 items.
    Items { player: usize, items: usize },
    /// A slot names an item that is not in the table, or an empty one.
    SlotItem {
        player: usize,
        slot: usize,
        index: u16,
    },
    /// An item the client's dictionary lacks.
    DictId { player: usize, dict_id: u16 },
    /// More than 8 modifications or 3 upgrades on an item.
    ItemParts { player: usize, dict_id: u16 },
    /// A modification (state clear) or defect (state set) the client's
    /// table lacks: it reads the row it did not find.
    Modification { player: usize, id: u32, state: bool },
    /// The options are too large for the client's buffer or one message.
    TooLarge(usize),
    /// Reading: the bytes end early.
    Truncated,
    /// Reading: bytes are left over.
    Trailing(usize),
    /// Reading: srp's block after the options is not one the patches read.
    SrpBlock,
    /// Reading: the stream is not stored LZMA2 chunks.
    Lzma2,
    /// Reading: the local index is not below N.
    LocalIndex { index: u8, roster: u8 },
}

impl std::fmt::Display for StaticInfoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Roster(players) => write!(f, "a roster of {players}"),
            Self::Map(id) => write!(f, "map id {id} cannot be played"),
            Self::Team { player, team } => write!(f, "player {player} is in team {team}"),
            Self::Text { player, len, max } => {
                write!(f, "player {player}: a text of {len} bytes, over {max}")
            }
            Self::Kits { player, kits } => write!(f, "player {player} has {kits} kits"),
            Self::NoUsableKit { player } => write!(f, "player {player} has no usable kit"),
            Self::Items { player, items } => write!(f, "player {player} has {items} items"),
            Self::SlotItem {
                player,
                slot,
                index,
            } => write!(f, "player {player}: slot {slot} names item {index}"),
            Self::DictId { player, dict_id } => {
                write!(f, "player {player}: no dictionary item {dict_id}")
            }
            Self::ItemParts { player, dict_id } => {
                write!(f, "player {player}: item {dict_id} has too many parts")
            }
            Self::Modification { player, id, state } => {
                let what = if *state { "defect" } else { "modification" };
                write!(f, "player {player}: no {what} {id}")
            }
            Self::TooLarge(len) => write!(f, "{len} bytes of options"),
            Self::Truncated => write!(f, "the options end early"),
            Self::Trailing(len) => write!(f, "{len} bytes after the options"),
            Self::SrpBlock => write!(f, "srp's block after the options is not one the patches read"),
            Self::Lzma2 => write!(f, "not a stored LZMA2 stream"),
            Self::LocalIndex { index, roster } => {
                write!(f, "local index {index} in a roster of {roster}")
            }
        }
    }
}

impl std::error::Error for StaticInfoError {}

/// A place nobody has: team 0, no name, no clan, protection 0, no kits,
/// no items. The client's loops stop at N, so it is never looked at.
const EMPTY_PROFILE: [u8; 9] = [0; 9];

fn check_text(player: usize, text: &str, max: usize) -> Result<(), StaticInfoError> {
    if text.len() > max {
        return Err(StaticInfoError::Text {
            player,
            len: text.len(),
            max,
        });
    }
    Ok(())
}

fn write_str(text: &str, out: &mut Vec<u8>) {
    out.push(text.len() as u8);
    out.extend_from_slice(text.as_bytes());
}

fn check_profile(
    player: usize,
    profile: &MatchProfile,
    dictionary: &Dictionary,
) -> Result<(), StaticInfoError> {
    if profile.team > 1 {
        return Err(StaticInfoError::Team {
            player,
            team: profile.team,
        });
    }
    check_text(player, &profile.name, MAX_NAME)?;
    check_text(player, &profile.clan_tag, MAX_CLAN)?;
    if profile.kits.len() > MAX_KITS {
        return Err(StaticInfoError::Kits {
            player,
            kits: profile.kits.len(),
        });
    }
    if !profile.kits.iter().any(|kit| kit.state == KIT_USABLE) {
        return Err(StaticInfoError::NoUsableKit { player });
    }
    if profile.items.len() > MAX_ITEMS {
        return Err(StaticInfoError::Items {
            player,
            items: profile.items.len(),
        });
    }
    for item in &profile.items {
        let dict_id = item.dict_id;
        if dict_id != 0 && dictionary.item(dict_id).is_none() {
            return Err(StaticInfoError::DictId { player, dict_id });
        }
        if item.modifications.len() > MAX_MODIFICATIONS || item.upgrades.len() > MAX_UPGRADES {
            return Err(StaticInfoError::ItemParts { player, dict_id });
        }
        let table = super::super::modifications::Modifications::shared();
        if let Some(&(id, state)) = item
            .modifications
            .iter()
            .find(|&&(id, state)| !table.is_known(id, state))
        {
            return Err(StaticInfoError::Modification { player, id, state });
        }
    }
    for kit in &profile.kits {
        check_text(player, &kit.name, MAX_NAME)?;
        for (slot, held) in kit.slots.iter().enumerate() {
            let Some((index, _)) = *held else {
                continue;
            };
            // The slot's item is looked up in the dictionary unchecked.
            if profile
                .items
                .get(usize::from(index))
                .is_none_or(|item| item.dict_id == 0)
            {
                return Err(StaticInfoError::SlotItem {
                    player,
                    slot,
                    index,
                });
            }
        }
    }
    Ok(())
}

fn write_item(item: &MatchItem, dictionary: &Dictionary, out: &mut Vec<u8>) {
    out.extend(item.instance_id.to_le_bytes());
    out.extend(item.dict_id.to_le_bytes());
    if item.dict_id == 0 {
        return;
    }
    out.extend(item.condition_or_stack.to_le_bytes());
    // The client's own `is_stack` decides what it reads here (0x1F1393).
    if !dictionary.is_stack(item.dict_id) {
        out.extend(item.repair_time.to_le_bytes());
        out.extend(item.reroll_time.to_le_bytes());
    }
    out.push(item.modifications.len() as u8);
    for &(id, state) in &item.modifications {
        out.extend(id.to_le_bytes());
        out.push(state.into());
    }
    out.push(item.upgrades.len() as u8);
    for upgrade in &item.upgrades {
        out.extend(upgrade.id.to_le_bytes());
        out.extend(upgrade.unknown_4.to_le_bytes());
        out.extend(upgrade.unknown_8.to_le_bytes());
        out.push(upgrade.param);
    }
}

fn write_profile(profile: &MatchProfile, dictionary: &Dictionary, out: &mut Vec<u8>) {
    out.push(profile.team);
    write_str(&profile.name, out);
    write_str(&profile.clan_tag, out);
    out.extend(profile.protection.to_le_bytes());
    out.push(profile.kits.len() as u8);
    out.push(profile.items.len() as u8);
    for kit in &profile.kits {
        out.push(kit.state);
        out.extend(kit.id.to_le_bytes());
        write_str(&kit.name, out);
        let mask = kit
            .slots
            .iter()
            .enumerate()
            .filter(|(_, held)| held.is_some())
            .fold(0u32, |mask, (slot, _)| mask | 1 << slot);
        out.extend(mask.to_le_bytes());
        for (index, count) in kit.slots.iter().flatten() {
            out.extend(index.to_le_bytes());
            out.extend(count.to_le_bytes());
        }
        write_modifier_block(&kit.modifiers, out);
    }
    for item in &profile.items {
        write_item(item, dictionary, out);
    }
}

/// A single validated native profile body, without its squad byte.
/// Used by the experimental growing-roster protocol as well as static info.
pub(super) fn profile_bytes(
    player: usize,
    profile: &MatchProfile,
    dictionary: &Dictionary,
) -> Result<Vec<u8>, StaticInfoError> {
    check_profile(player, profile, dictionary)?;
    let mut bytes = Vec::new();
    write_profile(profile, dictionary, &mut bytes);
    Ok(bytes)
}

pub(super) fn parse_profile(
    bytes: &[u8],
    player: usize,
    dictionary: &Dictionary,
) -> Result<MatchProfile, StaticInfoError> {
    let mut reader = Reader { bytes };
    let profile = read_profile(&mut reader, player, dictionary)?;
    if !reader.bytes.is_empty() {
        return Err(StaticInfoError::Trailing(reader.bytes.len()));
    }
    check_profile(player, &profile, dictionary)?;
    Ok(profile)
}

/// The options' bytes, before LZMA2. `dictionary` must be the client's:
/// its `is_stack` shapes each item.
pub fn options_bytes(
    options: &MatchOptions,
    dictionary: &Dictionary,
) -> Result<Vec<u8>, StaticInfoError> {
    let roster = options.players.len();
    if roster == 0 || roster > MAX_PLAYERS {
        return Err(StaticInfoError::Roster(roster));
    }
    if !maps::map_mode(options.map_id).is_some_and(|map| map.is_playable()) {
        return Err(StaticInfoError::Map(options.map_id));
    }
    let mut out = Vec::new();
    for (player, profile) in options.players.iter().enumerate() {
        check_profile(player, profile, dictionary)?;
        write_profile(profile, dictionary, &mut out);
    }
    for _ in roster..MAX_PLAYERS {
        out.extend(EMPTY_PROFILE);
    }
    out.push(options.map_id);
    out.push(roster as u8);
    out.extend(options.match_id.to_le_bytes());
    out.extend(options.match_level.to_le_bytes());
    for weight in options.score_weights {
        out.extend(weight.to_le_bytes());
    }
    let mut squads = [0u8; MAX_PLAYERS];
    for (squad, profile) in squads.iter_mut().zip(&options.players) {
        *squad = profile.squad;
    }
    out.extend(squads);
    let mut block = Vec::new();
    if options.blackout {
        block.extend([SRPX_BLACKOUT, 1, 1]);
    }
    if !block.is_empty() {
        out.extend(SRPX_MAGIC);
        out.extend(block);
    }
    if out.len() > OPTIONS_BUFFER {
        return Err(StaticInfoError::TooLarge(out.len()));
    }
    Ok(out)
}

/// A match's static info, built once: every client gets the same options
/// and its own local index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaticInfo {
    roster: u8,
    options: Vec<u8>,
    lzma2: Vec<u8>,
}

impl StaticInfo {
    pub fn new(options: &MatchOptions, dictionary: &Dictionary) -> Result<Self, StaticInfoError> {
        let bytes = options_bytes(options, dictionary)?;
        let lzma2 = lzma2::stored(&bytes);
        if 1 + lzma2.len() > MAX_MESSAGE {
            return Err(StaticInfoError::TooLarge(bytes.len()));
        }
        Ok(Self {
            roster: options.players.len() as u8,
            options: bytes,
            lzma2,
        })
    }

    /// N: the roster size. It also sizes the player mask of 0xED / 0xEE
    /// and is the N of the start rule ([`super::rules`]).
    pub fn roster(&self) -> u8 {
        self.roster
    }

    /// The options before LZMA2.
    pub fn options(&self) -> &[u8] {
        &self.options
    }

    /// 0xE4 for the client at roster place `local_index`. `None`: no such
    /// place (the client would have no local player).
    pub fn message(&self, local_index: u8) -> Option<ServerMessage> {
        (local_index < self.roster).then(|| ServerMessage::StaticInfo {
            local_index,
            lzma2: self.lzma2.clone(),
        })
    }

    /// 0xE4's payload for the client at `local_index`: the index, then the
    /// stream. The transport splits it into parts.
    pub fn payload(&self, local_index: u8) -> Option<Vec<u8>> {
        self.message(local_index).map(|message| message.payload())
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8], StaticInfoError> {
        if self.bytes.len() < len {
            return Err(StaticInfoError::Truncated);
        }
        let (taken, rest) = self.bytes.split_at(len);
        self.bytes = rest;
        Ok(taken)
    }

    fn u8(&mut self) -> Result<u8, StaticInfoError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, StaticInfoError> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn u32(&mut self) -> Result<u32, StaticInfoError> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn u64(&mut self) -> Result<u64, StaticInfoError> {
        Ok(u64::from(self.u32()?) | u64::from(self.u32()?) << 32)
    }

    fn f32(&mut self) -> Result<f32, StaticInfoError> {
        Ok(f32::from_bits(self.u32()?))
    }

    fn str(&mut self, player: usize, max: usize) -> Result<String, StaticInfoError> {
        let len = usize::from(self.u8()?);
        let bytes = self.take(len)?;
        if len > max {
            return Err(StaticInfoError::Text { player, len, max });
        }
        Ok(String::from_utf8_lossy(bytes).into_owned())
    }
}

fn read_item(
    reader: &mut Reader,
    player: usize,
    dictionary: &Dictionary,
) -> Result<MatchItem, StaticInfoError> {
    let mut item = MatchItem {
        instance_id: reader.u32()?,
        dict_id: reader.u16()?,
        ..MatchItem::default()
    };
    let dict_id = item.dict_id;
    if dict_id == 0 {
        return Ok(item);
    }
    if dictionary.item(dict_id).is_none() {
        return Err(StaticInfoError::DictId { player, dict_id });
    }
    item.condition_or_stack = reader.u32()?;
    if !dictionary.is_stack(dict_id) {
        item.repair_time = reader.u32()?;
        item.reroll_time = reader.u32()?;
    }
    let modifications = usize::from(reader.u8()?);
    if modifications > MAX_MODIFICATIONS {
        return Err(StaticInfoError::ItemParts { player, dict_id });
    }
    for _ in 0..modifications {
        item.modifications.push((reader.u32()?, reader.u8()? != 0));
    }
    let upgrades = usize::from(reader.u8()?);
    if upgrades > MAX_UPGRADES {
        return Err(StaticInfoError::ItemParts { player, dict_id });
    }
    for _ in 0..upgrades {
        item.upgrades.push(Upgrade {
            id: reader.u32()?,
            unknown_4: reader.u32()?,
            unknown_8: reader.u64()?,
            param: reader.u8()?,
        });
    }
    Ok(item)
}

fn read_profile(
    reader: &mut Reader,
    player: usize,
    dictionary: &Dictionary,
) -> Result<MatchProfile, StaticInfoError> {
    let team = reader.u8()?;
    let name = reader.str(player, MAX_NAME)?;
    let clan_tag = reader.str(player, MAX_CLAN)?;
    let protection = reader.u32()?;
    let kit_count = usize::from(reader.u8()?);
    let item_count = usize::from(reader.u8()?);
    if kit_count > MAX_KITS {
        return Err(StaticInfoError::Kits {
            player,
            kits: kit_count,
        });
    }
    if item_count > MAX_ITEMS {
        return Err(StaticInfoError::Items {
            player,
            items: item_count,
        });
    }
    let mut kits = Vec::with_capacity(kit_count);
    for _ in 0..kit_count {
        let state = reader.u8()?;
        let id = reader.u32()?;
        let kit_name = reader.str(player, MAX_NAME)?;
        let mut kit = MatchKit {
            state,
            ..MatchKit::empty(id, &kit_name)
        };
        // The client's loop looks at bits 0..=22 only.
        let slot_mask = reader.u32()?;
        for (slot, held) in kit.slots.iter_mut().enumerate() {
            if slot_mask & (1 << slot) != 0 {
                *held = Some((reader.u16()?, reader.u16()?));
            }
        }
        // The mask's high qword comes first.
        let mask = u128::from(reader.u64()?) << 64 | u128::from(reader.u64()?);
        for (index, modifier) in kit.modifiers.iter_mut().enumerate() {
            if mask & (1 << index) != 0 {
                *modifier = reader.f32()?;
            }
        }
        kits.push(kit);
    }
    let items = (0..item_count)
        .map(|_| read_item(reader, player, dictionary))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(MatchProfile {
        team,
        name,
        clan_tag,
        protection,
        kits,
        items,
        squad: 0,
    })
}

/// Reads options the way the client does (`match_options::deserialize`
/// 0x1F0E70), and refuses what would harm it: every limit of
/// [`options_bytes`], checked on the bytes. For tests of what a server
/// sends.
pub fn parse_options(
    bytes: &[u8],
    dictionary: &Dictionary,
) -> Result<MatchOptions, StaticInfoError> {
    if bytes.len() > OPTIONS_BUFFER {
        return Err(StaticInfoError::TooLarge(bytes.len()));
    }
    let mut reader = Reader { bytes };
    let mut players = (0..MAX_PLAYERS)
        .map(|player| read_profile(&mut reader, player, dictionary))
        .collect::<Result<Vec<_>, _>>()?;
    let map_id = reader.u8()?;
    let roster = usize::from(reader.u8()?);
    let match_id = reader.u32()?;
    let match_level = reader.u16()?;
    let mut score_weights = [0.0; SCORE_EVENTS];
    for weight in &mut score_weights {
        *weight = reader.f32()?;
    }
    let squads = reader.take(MAX_PLAYERS)?;
    let blackout = read_srp_block(&mut reader)?;
    if !reader.bytes.is_empty() {
        return Err(StaticInfoError::Trailing(reader.bytes.len()));
    }
    if roster == 0 || roster > MAX_PLAYERS {
        return Err(StaticInfoError::Roster(roster));
    }
    if !maps::map_mode(map_id).is_some_and(|map| map.is_playable()) {
        return Err(StaticInfoError::Map(map_id));
    }
    players.truncate(roster);
    for (player, (profile, &squad)) in players.iter_mut().zip(squads).enumerate() {
        profile.squad = squad;
        check_profile(player, profile, dictionary)?;
    }
    Ok(MatchOptions {
        players,
        map_id,
        match_id,
        match_level,
        score_weights,
        blackout,
    })
}

/// srp's block after the options, if any: as patch 65's reader reads it, but
/// strict (an unknown tag or a short entry is refused, so a test sees it).
fn read_srp_block(reader: &mut Reader) -> Result<bool, StaticInfoError> {
    if reader.bytes.is_empty() {
        return Ok(false);
    }
    if reader.take(4).map_err(|_| StaticInfoError::SrpBlock)? != SRPX_MAGIC {
        return Err(StaticInfoError::SrpBlock);
    }
    let mut blackout = false;
    while !reader.bytes.is_empty() {
        let tag = reader.u8().map_err(|_| StaticInfoError::SrpBlock)?;
        let len = reader.u8().map_err(|_| StaticInfoError::SrpBlock)?;
        let mut entry = Reader {
            bytes: reader
                .take(usize::from(len))
                .map_err(|_| StaticInfoError::SrpBlock)?,
        };
        match (tag, len) {
            (SRPX_BLACKOUT, 1) if entry.u8()? == 1 => blackout = true,
            _ => return Err(StaticInfoError::SrpBlock),
        }
    }
    Ok(blackout)
}

/// Reads a 0xE4 payload as the client at its local index would: the
/// index, the stored LZMA2 stream, the options. For tests.
pub fn parse_payload(
    payload: &[u8],
    dictionary: &Dictionary,
) -> Result<(u8, MatchOptions), StaticInfoError> {
    let (&index, stream) = payload.split_first().ok_or(StaticInfoError::Truncated)?;
    let bytes = lzma2::unstore(stream).ok_or(StaticInfoError::Lzma2)?;
    let options = parse_options(&bytes, dictionary)?;
    let roster = options.players.len() as u8;
    if index >= roster {
        return Err(StaticInfoError::LocalIndex { index, roster });
    }
    Ok((index, options))
}

#[cfg(test)]
mod tests {
    use super::super::super::loadouts;
    use super::super::super::skills::modifier_index;
    use super::*;

    fn dictionary() -> &'static Dictionary {
        Dictionary::shared()
    }

    /// The TT-33 (no stack) and its ammo (a stack).
    fn pistol() -> u16 {
        dictionary().item_named("tt_33").unwrap().dict_id
    }

    fn ammo() -> u16 {
        dictionary().item_named("ammo_7.62x25").unwrap().dict_id
    }

    /// static_info.md §6's example: "alice", team 0, one kit "kit" with one
    /// weapon in slot 7 and no modifiers.
    fn alice() -> MatchProfile {
        let mut kit = MatchKit::empty(1, "kit");
        kit.slots[7] = Some((0, 1));
        MatchProfile {
            team: 0,
            name: "alice".into(),
            clan_tag: String::new(),
            protection: NO_PROTECTION,
            kits: vec![kit],
            items: vec![MatchItem {
                instance_id: 0x4433_2211,
                dict_id: pistol(),
                condition_or_stack: 100,
                ..MatchItem::default()
            }],
            squad: 0,
        }
    }

    fn alice_bytes() -> Vec<u8> {
        let mut bytes = vec![0x00, 0x05];
        bytes.extend(b"alice");
        bytes.extend([0x00, 0x0A, 0, 0, 0, 0x01, 0x01]); // no clan, protection 10, K, M
        bytes.extend([0x00, 0x01, 0, 0, 0, 0x03]); // state 0, kit id 1
        bytes.extend(b"kit");
        bytes.extend([0x80, 0, 0, 0]); // slot 7
        bytes.extend([0x00, 0x00, 0x01, 0x00]); // item 0, count 1
        bytes.extend([0; 16]); // no modifiers
        bytes.extend([0x11, 0x22, 0x33, 0x44]);
        bytes.extend(pistol().to_le_bytes());
        bytes.extend([0x64, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x00, 0x00]);
        bytes
    }

    fn options(players: Vec<MatchProfile>) -> MatchOptions {
        MatchOptions {
            players,
            map_id: 61,
            match_id: 0x1234_5678,
            match_level: 3,
            score_weights: SCORE_WEIGHTS,
            blackout: false,
        }
    }

    #[test]
    fn blackout_is_an_entry_of_the_block_and_reads_back() {
        let mut with = options(vec![alice()]);
        let without = options_bytes(&with, dictionary()).unwrap();
        with.blackout = true;
        let bytes = options_bytes(&with, dictionary()).unwrap();
        assert_eq!(bytes[..without.len()], without[..]);
        assert_eq!(bytes[without.len()..], b"SRPX\x02\x01\x01"[..]);
        assert_eq!(parse_options(&bytes, dictionary()), Ok(with.clone()));
        // Another value or length, an unknown tag, another magic: refused.
        for bad in [
            &b"SRPX\x02\x01\x00"[..],
            &b"SRPX\x02\x02\x01\x01"[..],
            &b"SRPX\x09\x00"[..],
            &b"SRPY\x02\x01\x01"[..],
        ] {
            assert_eq!(
                parse_options(&[&without[..], bad].concat(), dictionary()),
                Err(StaticInfoError::SrpBlock)
            );
        }
    }

    /// The options' end for map 61 and N players: 140 bytes.
    fn tail_bytes(roster: u8, squads: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0x3D, roster, 0x78, 0x56, 0x34, 0x12, 0x03, 0x00];
        // The first weights: 10.0, 5.0, 0.0.
        bytes.extend([0x00, 0x00, 0x20, 0x41, 0x00, 0x00, 0xA0, 0x40, 0, 0, 0, 0]);
        for weight in &SCORE_WEIGHTS[3..] {
            bytes.extend(weight.to_le_bytes());
        }
        let mut squad_bytes = [0u8; 20];
        squad_bytes[..squads.len()].copy_from_slice(squads);
        bytes.extend(squad_bytes);
        assert_eq!(bytes.len(), 140);
        bytes
    }

    #[test]
    fn the_options_are_twenty_profiles_then_the_map_and_the_rest() {
        let mut bob = alice();
        bob.team = 1;
        bob.name = "bob".into();
        bob.squad = 4;
        let bytes = options_bytes(&options(vec![alice(), bob]), dictionary()).unwrap();

        let mut expected = alice_bytes();
        let bob_bytes = {
            let mut bytes = vec![0x01, 0x03];
            bytes.extend(b"bob");
            bytes.extend(&alice_bytes()[7..]);
            bytes
        };
        expected.extend(&bob_bytes);
        // 18 empty places of 9 bytes.
        expected.extend([0; 18 * 9]);
        expected.extend(tail_bytes(2, &[0, 4]));
        assert_eq!(bytes, expected);
        // static_info.md §2: the profiles and 140 bytes.
        assert_eq!(
            bytes.len(),
            alice_bytes().len() + bob_bytes.len() + 162 + 140
        );
    }

    #[test]
    fn the_sourced_weights_are_a_kill_an_assist_and_a_stolen_battery() {
        assert_eq!(SCORE_WEIGHTS[0], 10.0);
        assert_eq!(SCORE_WEIGHTS[1], 5.0);
        assert_eq!(SCORE_WEIGHTS[17], 35.0);
        // The events the client adds at the end, and the new ones, score
        // nothing.
        for event in [2, 3, 4, 11, 12, 22, 23, 24, 25, 26, 27] {
            assert_eq!(SCORE_WEIGHTS[event], 0.0, "{event}");
        }
        // On the wire: little-endian f32, 28 of them before the squads.
        let bytes = options_bytes(&options(vec![alice()]), dictionary()).unwrap();
        let weights = &bytes[bytes.len() - 20 - 28 * 4..bytes.len() - 20];
        assert_eq!(
            weights[..8],
            [0x00, 0x00, 0x20, 0x41, 0x00, 0x00, 0xA0, 0x40]
        );
        assert_eq!(weights[17 * 4..18 * 4], [0x00, 0x00, 0x0C, 0x42]);
    }

    #[test]
    fn an_artefact_scores_15_into_an_empty_slot_and_5_with_every_slot_full() {
        use super::super::snapshot::{self, event};
        // What the clients count for one artefact taken with kit 1.
        let taken = |slot_free: bool| {
            let mut player = snapshot::PlayerStatistics::default();
            player.events[event::ARTEFACT][1] = 1;
            if slot_free {
                player.events[event::WINNING_TEAM_VI_BONUS][1] = 1;
            }
            snapshot::score(&player, &SCORE_WEIGHTS)
        };
        assert_eq!(taken(true), 15);
        assert_eq!(taken(false), 5);
        // Research weighs them the same.
        assert_eq!(
            score_weights(super::super::super::maps::mode::RESEARCH)[event::WINNING_TEAM_VI_BONUS],
            10.0
        );
    }

    #[test]
    fn research_weighs_its_captures_and_kill_invader() {
        use super::super::super::maps::mode;
        let research = score_weights(mode::RESEARCH);
        assert_eq!(
            (research[6], research[18], research[10]),
            (30.0, 30.0, 10.0)
        );
        // The rest as every mode's.
        for event in (0..SCORE_EVENTS).filter(|event| ![6, 10, 18].contains(event)) {
            assert_eq!(research[event], SCORE_WEIGHTS[event], "{event}");
        }
        // Modes 2 to 4 are unchanged.
        for game_mode in [
            mode::BATTERIES,
            mode::TIMED_BATTERIES,
            mode::TEAM_DEATHMATCH,
        ] {
            assert_eq!(score_weights(game_mode), SCORE_WEIGHTS);
        }
        // Over weights of another's (a test's, a tool's): the three only.
        let mut zero = weights_of(mode::RESEARCH, [0.0; SCORE_EVENTS]);
        assert_eq!(zero.iter().filter(|weight| **weight != 0.0).count(), 3);
        zero[10] = 0.0;
        assert_eq!(weights_of(mode::BATTERIES, zero), zero);
    }

    #[test]
    fn the_message_is_the_local_index_and_a_stored_lzma2_stream() {
        let info = StaticInfo::new(&options(vec![alice()]), dictionary()).unwrap();
        assert_eq!(info.roster(), 1);
        let payload = info.payload(0).unwrap();
        let size = info.options().len();
        // E4: `u8 local index`, `01 <u16 BE size − 1> <options> 00`.
        assert_eq!(payload[0], 0);
        assert_eq!(payload[1], 0x01);
        assert_eq!(payload[2..4], ((size - 1) as u16).to_be_bytes());
        assert_eq!(payload[4..4 + size], *info.options());
        assert_eq!(payload[4 + size..], [0x00]);
        assert_eq!(info.message(0).unwrap().id(), 0xE4);
        // No place 1 in a roster of 1.
        assert_eq!(info.message(1), None);
    }

    #[test]
    fn each_client_gets_the_same_options_and_its_own_index() {
        let mut bob = alice();
        bob.team = 1;
        let info = StaticInfo::new(&options(vec![alice(), bob]), dictionary()).unwrap();
        let first = info.payload(0).unwrap();
        let second = info.payload(1).unwrap();
        assert_eq!((first[0], second[0]), (0, 1));
        assert_eq!(first[1..], second[1..]);
        assert_eq!(info.payload(2), None);
    }

    #[test]
    fn a_stack_has_a_count_and_no_times() {
        // An item that is no stack: id, dict, condition, two times, no
        // modifications, no upgrades. A stack: id, dict, count, the counts.
        let mut out = Vec::new();
        let item = MatchItem {
            instance_id: 7,
            dict_id: ammo(),
            condition_or_stack: 16,
            repair_time: 99,
            reroll_time: 99,
            ..MatchItem::default()
        };
        write_item(&item, dictionary(), &mut out);
        let mut expected = vec![7, 0, 0, 0];
        expected.extend(ammo().to_le_bytes());
        expected.extend([16, 0, 0, 0, 0, 0]);
        assert_eq!(out, expected);

        // An empty entry ends after the dict id.
        let mut out = Vec::new();
        write_item(
            &MatchItem {
                instance_id: 0x0102_0304,
                ..MatchItem::default()
            },
            dictionary(),
            &mut out,
        );
        assert_eq!(out, [4, 3, 2, 1, 0, 0]);
    }

    #[test]
    fn modifications_and_upgrades_follow_an_item() {
        let item = MatchItem {
            instance_id: 1,
            dict_id: pistol(),
            condition_or_stack: FULL_CONDITION,
            repair_time: 2,
            reroll_time: 3,
            // A defect and a modification.
            modifications: vec![(3000, true), (2000, false)],
            upgrades: vec![Upgrade {
                id: 9,
                unknown_4: 0x1111_1111,
                unknown_8: 0x2222_2222_3333_3333,
                param: 4,
            }],
        };
        let mut out = Vec::new();
        write_item(&item, dictionary(), &mut out);
        let mut expected = vec![1, 0, 0, 0];
        expected.extend(pistol().to_le_bytes());
        expected.extend([0x40, 0x42, 0x0F, 0x00]); // 1 000 000
        expected.extend([2, 0, 0, 0, 3, 0, 0, 0]);
        // n1, then `u32 id, u8 state` each.
        expected.extend([2, 0xB8, 0x0B, 0, 0, 1, 0xD0, 0x07, 0, 0, 0]);
        // n2, then `u32 id, u32, u64, u8` each.
        expected.extend([1, 9, 0, 0, 0, 0x11, 0x11, 0x11, 0x11]);
        expected.extend([0x33, 0x33, 0x33, 0x33, 0x22, 0x22, 0x22, 0x22, 4]);
        assert_eq!(out, expected);
        let read = read_item(&mut Reader { bytes: &out }, 0, dictionary()).unwrap();
        assert_eq!(read, item);
    }

    #[test]
    fn a_kit_carries_its_modifiers_behind_a_mask_with_the_high_half_first() {
        let mut profile = alice();
        // Modifiers 4 (movement_speed) and 67, the last.
        profile.kits[0].modifiers[4] = 0.05;
        profile.kits[0].modifiers[67] = -1.5;
        assert_eq!(modifier_index("movement_speed_modifier"), Some(4));
        let mut out = Vec::new();
        write_profile(&profile, dictionary(), &mut out);
        // The kit: after team, name, clan, protection, K, M (14 bytes) and
        // state, id, name, slot mask, one slot (17 bytes).
        let block = &out[31..31 + 24];
        let mut expected = vec![0x08, 0, 0, 0, 0, 0, 0, 0]; // bits 64..=127: bit 67
        expected.extend([0x10, 0, 0, 0, 0, 0, 0, 0]); // bits 0..=63: bit 4
        expected.extend(0.05f32.to_le_bytes());
        expected.extend([0x00, 0x00, 0xC0, 0xBF]); // -1.5
        assert_eq!(block, expected);
    }

    #[test]
    fn what_is_written_reads_back_as_the_client_reads_it() {
        let mut alice = alice();
        alice.clan_tag = "SRP".into();
        alice.squad = 2;
        alice.kits[0].modifiers[0] = -0.1;
        alice.kits[0].modifiers[66] = 2.0;
        // A second kit, not usable, and a stack in the first.
        alice.kits.push(MatchKit {
            state: 2,
            ..MatchKit::empty(2, "")
        });
        alice.items.push(MatchItem {
            instance_id: 5,
            dict_id: ammo(),
            condition_or_stack: 24,
            ..MatchItem::default()
        });
        alice.kits[0].slots[8] = Some((1, 24));
        let starter = MatchProfile::from_loadout(
            "bob",
            1,
            loadouts::starter(),
            [0.0; MODIFIERS],
            dictionary(),
            100,
        );
        let sent = options(vec![alice, starter]);
        let info = StaticInfo::new(&sent, dictionary()).unwrap();
        let (index, read) = parse_payload(&info.payload(1).unwrap(), dictionary()).unwrap();
        assert_eq!(index, 1);
        assert_eq!(read, sent);
    }

    #[test]
    fn a_starter_loadout_becomes_a_profile() {
        let profile = MatchProfile::from_loadout(
            "bob",
            1,
            loadouts::starter(),
            [0.0; MODIFIERS],
            dictionary(),
            100,
        );
        assert_eq!(profile.kits.len(), 1);
        assert_eq!(profile.kits[0].state, KIT_USABLE);
        // The pistol in slot 7, whole; its ammo in slot 8, a stack.
        assert_eq!(profile.kits[0].slots[7], Some((0, 1)));
        assert_eq!(profile.items[0].dict_id, pistol());
        assert_eq!(profile.items[0].condition_or_stack, FULL_CONDITION);
        assert_eq!(profile.items[0].instance_id, 100);
        let (index, count) = profile.kits[0].slots[8].unwrap();
        assert_eq!(index, 1);
        assert_eq!(profile.items[1].dict_id, ammo());
        assert_eq!(u32::from(count), profile.items[1].condition_or_stack);
        assert_eq!(profile.items[1].instance_id, 101);
    }

    #[test]
    fn what_the_client_cannot_hold_is_refused() {
        let refused = |change: &dyn Fn(&mut MatchOptions)| {
            let mut options = options(vec![alice()]);
            change(&mut options);
            StaticInfo::new(&options, dictionary()).unwrap_err()
        };
        use StaticInfoError as E;
        assert_eq!(refused(&|o| o.players.clear()), E::Roster(0));
        assert_eq!(refused(&|o| o.players = vec![alice(); 21]), E::Roster(21));
        // 107 is level_08 at night, which has no project; 0 is no map.
        assert_eq!(refused(&|o| o.map_id = 107), E::Map(107));
        assert_eq!(refused(&|o| o.map_id = 0), E::Map(0));
        assert_eq!(
            refused(&|o| o.players[0].team = 2),
            E::Team { player: 0, team: 2 }
        );
        assert_eq!(
            refused(&|o| o.players[0].name = "n".repeat(64)),
            E::Text {
                player: 0,
                len: 64,
                max: 63
            }
        );
        assert_eq!(
            refused(&|o| o.players[0].clan_tag = "c".repeat(15)),
            E::Text {
                player: 0,
                len: 15,
                max: 14
            }
        );
        assert_eq!(
            refused(&|o| o.players[0].kits[0].name = "k".repeat(64)),
            E::Text {
                player: 0,
                len: 64,
                max: 63
            }
        );
        assert_eq!(
            refused(&|o| {
                let kit = o.players[0].kits[0].clone();
                o.players[0].kits = vec![kit; 4];
            }),
            E::Kits { player: 0, kits: 4 }
        );
        assert_eq!(
            refused(&|o| o.players[0].kits[0].state = 2),
            E::NoUsableKit { player: 0 }
        );
        assert_eq!(
            refused(&|o| o.players[0].kits.clear()),
            E::NoUsableKit { player: 0 }
        );
        assert_eq!(
            refused(&|o| {
                let item = o.players[0].items[0].clone();
                o.players[0].items = vec![item; 70];
            }),
            E::Items {
                player: 0,
                items: 70
            }
        );
        assert_eq!(
            refused(&|o| o.players[0].kits[0].slots[3] = Some((1, 1))),
            E::SlotItem {
                player: 0,
                slot: 3,
                index: 1
            }
        );
        assert_eq!(
            refused(&|o| o.players[0].items[0].dict_id = 0),
            E::SlotItem {
                player: 0,
                slot: 7,
                index: 0
            }
        );
        assert_eq!(
            refused(&|o| o.players[0].items[0].dict_id = 0xFFFE),
            E::DictId {
                player: 0,
                dict_id: 0xFFFE
            }
        );
        assert_eq!(
            refused(&|o| o.players[0].items[0].modifications = vec![(1, false); 9]),
            E::ItemParts {
                player: 0,
                dict_id: pistol()
            }
        );
        // An id the client's table lacks, and a modification's id sent as
        // a defect: the client would read a row it did not find.
        assert_eq!(
            refused(&|o| o.players[0].items[0].modifications = vec![(1, false)]),
            E::Modification {
                player: 0,
                id: 1,
                state: false
            }
        );
        assert_eq!(
            refused(&|o| o.players[0].items[0].modifications = vec![(2000, true)]),
            E::Modification {
                player: 0,
                id: 2000,
                state: true
            }
        );
        assert_eq!(
            refused(&|o| o.players[0].items[0].upgrades = vec![Upgrade::default(); 4]),
            E::ItemParts {
                player: 0,
                dict_id: pistol()
            }
        );
        // The limits themselves are fine.
        let mut most = options(vec![alice(); 20]);
        most.players[0].name = "n".repeat(63);
        most.players[0].clan_tag = "c".repeat(14);
        let item = most.players[0].items[0].clone();
        most.players[0].items = vec![item; 69];
        assert!(StaticInfo::new(&most, dictionary()).is_ok());
    }

    #[test]
    fn the_reader_refuses_what_the_writer_would() {
        let good = options_bytes(&options(vec![alice()]), dictionary()).unwrap();
        assert!(parse_options(&good, dictionary()).is_ok());
        // Cut, and with a byte too many.
        assert_eq!(
            parse_options(&good[..good.len() - 1], dictionary()),
            Err(StaticInfoError::Truncated)
        );
        let mut long = good.clone();
        long.push(0);
        assert_eq!(
            parse_options(&long, dictionary()),
            Err(StaticInfoError::SrpBlock)
        );
        // Four kits announced: the fourth would overwrite the item table.
        let mut bytes = good.clone();
        bytes[12] = 4;
        assert_eq!(
            parse_options(&bytes, dictionary()),
            Err(StaticInfoError::Kits { player: 0, kits: 4 })
        );
        // N = 0 and N = 21.
        let roster_at = good.len() - 139;
        for roster in [0, 21] {
            let mut bytes = good.clone();
            bytes[roster_at] = roster;
            assert_eq!(
                parse_options(&bytes, dictionary()),
                Err(StaticInfoError::Roster(roster.into()))
            );
        }
        // N = 2 with place 1 empty: a player with no kit.
        let mut bytes = good.clone();
        bytes[roster_at] = 2;
        assert_eq!(
            parse_options(&bytes, dictionary()),
            Err(StaticInfoError::NoUsableKit { player: 1 })
        );
        // A local index outside the roster, and a stream that is none.
        let info = StaticInfo::new(&options(vec![alice()]), dictionary()).unwrap();
        let mut payload = info.payload(0).unwrap();
        payload[0] = 1;
        assert_eq!(
            parse_payload(&payload, dictionary()),
            Err(StaticInfoError::LocalIndex {
                index: 1,
                roster: 1
            })
        );
        payload[1] = 0xE0;
        assert_eq!(
            parse_payload(&payload, dictionary()),
            Err(StaticInfoError::Lzma2)
        );
        assert_eq!(
            parse_payload(&[], dictionary()),
            Err(StaticInfoError::Truncated)
        );
    }

    #[test]
    fn twenty_full_rosters_fit_one_message() {
        // 20 players with 3 kits and 30 items each stay far below both
        // limits.
        let mut player = alice();
        let item = player.items[0].clone();
        player.items = vec![item; 30];
        let kit = player.kits[0].clone();
        player.kits = vec![kit; 3];
        let info = StaticInfo::new(&options(vec![player; 20]), dictionary()).unwrap();
        assert!(info.options().len() < OPTIONS_BUFFER);
        assert!(info.payload(19).unwrap().len() < MAX_MESSAGE);
    }
}
