//! The match messages a client takes (0xE3..0xF6), checked before a native
//! reader touches them (client_world_check.md section 10, relay_trust.md
//! section 3).
//!
//! - [`check`]: every message at the dispatch
//!   (`network_client::on_match_packet_received` 0x27A300), its payload
//!   after the id: the exact length of a fixed one, the envelope of 0xE4
//!   and 0xE8, and the bytes a handler takes as an index with no bound.
//! - [`options`]: 0xE4's match options, after the native decompressor
//!   (`match_options::deserialize` 0x1F0E70): srp-survarium/srp PR #38's
//!   validator, fitted to what srp sends (static_info.md section 6).
//! - [`input`]: a relayed input's rotation and action bits, which the
//!   server checks of a client's input before it relays it.

use crate::reader::{Reader, Short};
use crate::{MAX_PLAYERS, Part, Reason, Refusal, finite, transport};

/// What the game knows that 0xE4 is checked against: its own items
/// dictionary and maps (never the message's).
pub trait Dictionary {
    /// `Some(is_stack)` for an item of the dictionary; `None` for a dict id
    /// it does not have (`items_dictionary::item_by_id` 0x1F0090 finds
    /// nothing, and the client crashes later).
    fn item(&self, dict_id: u16) -> Option<bool>;
    /// Whether the client has the map (a maps_dict id).
    fn map(&self, map_id: u8) -> bool;
}

/// The ids srp sends and a client takes. 0xE2 and every id outside
/// 0xE3..0xF6 the client drops or crashes on (transport.md 3.4, 4); srp
/// never sends 0xED/0xEE (match.md section 13).
pub mod id {
    pub const PORTS: u8 = 0xE3;
    pub const STATIC_INFO: u8 = 0xE4;
    pub const SET_TIME: u8 = 0xE5;
    pub const PING: u8 = 0xE6;
    pub const CLOCK_OFFSET: u8 = 0xE7;
    pub const SNAPSHOT: u8 = 0xE8;
    pub const PLAYER_ENTERED: u8 = 0xE9;
    pub const PLAYER_LEFT: u8 = 0xEA;
    pub const INPUT_RELAY: u8 = 0xEB;
    pub const INPUT_RELAY_PRECISE: u8 = 0xEC;
    pub const INPUTS_RELAY: u8 = 0xED;
    pub const INPUTS_RELAY_PRECISE: u8 = 0xEE;
    pub const MOVE_INPUT: u8 = 0xEF;
    pub const MOVE_INPUT_PRECISE: u8 = 0xF0;
    pub const DISCARD_INPUT: u8 = 0xF1;
    pub const DISCARD_INPUT_PRECISE: u8 = 0xF2;
    pub const PROFILE_INPUT_RELAY: u8 = 0xF3;
    pub const MOVE_PROFILE_INPUT: u8 = 0xF4;
    pub const DISCARD_PROFILE_INPUT: u8 = 0xF5;
    pub const UPLOAD_REQUEST: u8 = 0xF6;
}

/// The payload length of a fixed message (the client's table 0x279670).
pub const fn fixed_length(id: u8) -> Option<usize> {
    Some(match id {
        id::PORTS => 4,
        id::SET_TIME => 7,
        id::PING => 4,
        id::CLOCK_OFFSET => 2,
        id::PLAYER_ENTERED | id::PLAYER_LEFT => 2,
        id::INPUT_RELAY => 10,
        id::INPUT_RELAY_PRECISE => 11,
        id::MOVE_INPUT => 2,
        id::MOVE_INPUT_PRECISE => 4,
        id::DISCARD_INPUT => 1,
        id::DISCARD_INPUT_PRECISE => 2,
        id::PROFILE_INPUT_RELAY => 3,
        id::MOVE_PROFILE_INPUT => 2,
        id::DISCARD_PROFILE_INPUT => 1,
        // srp's 0xF6 is one byte the patch ignores (the client's length
        // getter refuses an empty one).
        id::UPLOAD_REQUEST => 1,
        _ => return None,
    })
}

/// The action bits no stock client sets: 29 and 30 (the builder 0x241250
/// sets 0..23 and 25..28, patch 34 adds 24; bit 31 is the per-input flag
/// the reader 0x318400 splits off). Tested by no code found [LIKELY]; a
/// mask with one is no stock client's.
pub const UNUSED_ACTIONS: u32 = 1 << 29 | 1 << 30;

/// A half float's exponent field, and that of 1024.0.
const HALF_EXPONENT: u16 = 0x7C00;
const HALF_EXPONENT_1024: u16 = 25 << 10;

/// Whether an input's rotation (two IEEE halves, dx then dy) is one a
/// mouse makes: both numbers, each under 1024. Every client adds them to
/// the player's angles with no check (0x2D89A4 -> 0x2D8AD0), and two merged
/// into one node add as halves (0x2C5B60 -> 0x2C6640), which a half under
/// 1024 keeps a number (match.md section 20).
pub fn rotation_is_sane(rotation: [u8; 4]) -> bool {
    let [a, b, c, d] = rotation;
    [u16::from_le_bytes([a, b]), u16::from_le_bytes([c, d])]
        .into_iter()
        .all(|half| half & HALF_EXPONENT < HALF_EXPONENT_1024)
}

/// A relayed input's rotation and actions (relay_trust.md 0xDC/0xDD).
pub fn input(rotation: [u8; 4], actions: u32) -> Result<(), Reason> {
    if !rotation_is_sane(rotation) {
        let [a, b, c, d] = rotation;
        let [dx, dy] = [u16::from_le_bytes([a, b]), u16::from_le_bytes([c, d])];
        let half = if dx & HALF_EXPONENT >= HALF_EXPONENT_1024 {
            dx
        } else {
            dy
        };
        return Err(Reason::RotationNotSane { half });
    }
    if actions & UNUSED_ACTIONS != 0 {
        return Err(Reason::UnusedActions { actions });
    }
    Ok(())
}

/// A message the client takes, at the dispatch: `payload` is its bytes
/// after the id. 0xE4's options and 0xE8's world are checked once they
/// are unpacked ([`options`], [`crate::world::validate`]); here their
/// envelopes, and the LZMA2 output bound of the two that are compressed
/// ([`transport::lzma2_bound`]), before the native decoder takes them.
pub fn check(message: u8, payload: &[u8]) -> Result<(), Refusal> {
    let refuse = |offset: usize, reason| Refusal::new(Part::Message(message), offset, reason);
    let len = payload.len();
    if matches!(message, id::INPUTS_RELAY | id::INPUTS_RELAY_PRECISE) {
        return Err(refuse(0, Reason::NeverSent));
    }
    if let Some(wanted) = fixed_length(message) {
        if len != wanted {
            return Err(refuse(0, Reason::MessageLength { len, wanted }));
        }
    } else if !matches!(message, id::STATIC_INFO | id::SNAPSHOT) {
        return Err(refuse(0, Reason::UnknownMessage));
    }
    // A place or player byte names an entry of the 20-place table.
    let place = |at: usize| -> Result<(), Refusal> {
        let place = payload.get(at).copied().unwrap_or(0);
        if usize::from(place) < MAX_PLAYERS {
            Ok(())
        } else {
            Err(refuse(at, Reason::PlacePast { place }))
        }
    };
    match message {
        // The handler passes the place to `enter_match` 0x2D1470, which
        // tests only the time (0x27C652..0x27C67C, 0x2D1489).
        id::PLAYER_ENTERED | id::PLAYER_LEFT => place(1),
        // The input reader 0x318400 stores the player byte at node+0xC
        // (0x31844F..0x318458) and the rest with no check.
        id::INPUT_RELAY | id::INPUT_RELAY_PRECISE => {
            let at = if message == id::INPUT_RELAY { 1 } else { 2 };
            place(at)?;
            let mut reader = Reader::new(payload, 0);
            let read = (|| {
                reader.skip(at.saturating_add(1))?;
                Ok::<_, Short>((reader.array::<4>()?, reader.u32()?))
            })();
            let (rotation, actions) =
                read.map_err(|_| refuse(0, Reason::MessageLength { len, wanted: len }))?;
            input(rotation, actions).map_err(|reason| refuse(at.saturating_add(1), reason))
        }
        id::PROFILE_INPUT_RELAY => place(1),
        // The local index, then the LZMA2 stream of the options: it decodes
        // into the client's 0x578A0 buffer (`transport`), bounded here so a
        // stream over it, or a cut one, goes nowhere.
        // The late join's profile proposal (`late_join::Prepare`, every
        // client of the dialect): its magic, the match, the serial, the
        // place, the squad, then the profile, which the late join's own
        // check takes (`late_join_validate_026g.c`). Here the header: a
        // place the first player cannot have, under 20, a serial not 0.
        id::STATIC_INFO if payload.starts_with(&LATE_JOIN_PREPARE) => {
            if len < LATE_JOIN_HEADER {
                return Err(refuse(
                    0,
                    Reason::MessageShort {
                        len,
                        least: LATE_JOIN_HEADER,
                    },
                ));
            }
            let serial = payload.get(8..12).unwrap_or_default();
            if serial == [0; 4] || payload.get(12) == Some(&0) {
                return Err(refuse(8, Reason::LateJoinProposal));
            }
            place(12)
        }
        id::STATIC_INFO => {
            if len < 2 {
                return Err(refuse(0, Reason::MessageShort { len, least: 2 }));
            }
            place(0)?;
            let stream = payload.get(1..).unwrap_or_default();
            transport::lzma2_bound(stream, transport::OPTIONS_CAP)
                .map(drop)
                .map_err(|(at, reason)| refuse(at.saturating_add(1), reason))
        }
        // The generation, the compressed flag (srp sends 0 or 1), then the
        // state: raw (at least its time and the body's mask), or an LZMA2
        // stream that decodes into the client's 0x8000 buffer (`transport`).
        id::SNAPSHOT => match payload.get(1) {
            Some(0) => {
                let least = 2 + 4 + 4;
                if len < least {
                    return Err(refuse(0, Reason::MessageShort { len, least }));
                }
                Ok(())
            }
            Some(1) => {
                if len < 3 {
                    return Err(refuse(0, Reason::MessageShort { len, least: 3 }));
                }
                let stream = payload.get(2..).unwrap_or_default();
                transport::lzma2_bound(stream, transport::SNAPSHOT_STATE_CAP)
                    .map(drop)
                    .map_err(|(at, reason)| refuse(at.saturating_add(2), reason))
            }
            Some(&flag) => Err(refuse(1, Reason::SnapshotFlag { flag })),
            None => Err(refuse(0, Reason::MessageShort { len, least: 2 })),
        },
        _ => Ok(()),
    }
}

/// A 0xE4 that is the late join's profile proposal starts with these
/// (`late_join::PREPARE_MAGIC` in the server; the stock local index is
/// under 20, so 0xFF tells them apart), and has this header.
pub const LATE_JOIN_PREPARE: [u8; 4] = [0xFF, b'L', b'J', 1];
const LATE_JOIN_HEADER: usize = 14;

/// The client's buffer for the unpacked options (0x578A0 bytes).
pub const OPTIONS_BUFFER: usize = 0x578A0;
/// A profile's kits and items (`player_profile`: 3 kits, 69 items).
const MAX_KITS: usize = 3;
const MAX_ITEMS: usize = 69;
/// A kit's slots (the mask's loop count 0x17 at 0x1F11DF).
const SLOTS: u32 = 23;
/// The name buffers: 64 and 15 bytes with their NUL.
const NAME_BUFFER: usize = 64;
const CLAN_BUFFER: usize = 15;
/// An item's modifications and upgrades held inline (more is a fatal
/// assert).
const MAX_MODIFICATIONS: u8 = 8;
const MAX_UPGRADES: u8 = 3;
/// An upgrade: u32 id, u32, u64, u8.
const UPGRADE: usize = 17;
/// The score events, each with a weight.
const SCORE_EVENTS: usize = 28;

/// What a place's profile holds, for the checks that need the roster or
/// the items (they come after the kits on the wire).
#[derive(Clone, Copy)]
struct Place {
    kits: u8,
    items: u8,
    /// Bit i: item i is an empty entry (dict id 0).
    empty_items: u128,
    /// Bit i: a kit's slot names item i.
    slotted: u128,
    /// The offset of the profile, for a refusal of it.
    at: usize,
}

/// The match options after the decompressor (`match_options::deserialize`
/// 0x1F0E70; layout static_info.md section 6): 20 profiles, the map, the
/// roster N, the match id, the level, 28 score weights, 20 squad bytes,
/// and nothing after. The client checks none of it: a text, a kit or an
/// item too many writes over its memory, an item or map it does not have
/// crashes it later. `Ok(N)`.
///
/// Ported from srp-survarium/srp PR #38 (`packet_guard::application::
/// options`), fitted to what srp sends: a place past the roster is 9 zero
/// bytes (team 0, no texts, no kits, no items), not team 0xFF; a team is 0
/// or 1; an item of dict id 0 is an empty entry with nothing after it; a
/// kit's slot names an item that is not empty. Not ported: #38's rule that
/// item ids are unique and not 0 (no index use of them is known).
pub fn options(bytes: &[u8], dictionary: &dyn Dictionary) -> Result<u8, Refusal> {
    let refuse = |offset: usize, reason| Refusal::new(Part::Options, offset, reason);
    if bytes.len() > OPTIONS_BUFFER {
        return Err(refuse(0, Reason::OptionsTooLarge { len: bytes.len() }));
    }
    let mut reader = Reader::new(bytes, 0);
    let short = |error: Short| refuse(error.at, Reason::OptionsShort);
    let mut places = [Place {
        kits: 0,
        items: 0,
        empty_items: 0,
        slotted: 0,
        at: 0,
    }; MAX_PLAYERS];
    let mut teams = [0u8; MAX_PLAYERS];
    for (place, (held, team)) in places.iter_mut().zip(&mut teams).enumerate() {
        held.at = reader.at();
        *team = reader.u8().map_err(short)?;
        text(&mut reader, NAME_BUFFER, place).map_err(|fault| fault.refusal(&refuse, short))?;
        text(&mut reader, CLAN_BUFFER, place).map_err(|fault| fault.refusal(&refuse, short))?;
        let _protection = reader.u32().map_err(short)?;
        let at = reader.at();
        held.kits = reader.u8().map_err(short)?;
        held.items = reader.u8().map_err(short)?;
        if usize::from(held.kits) > MAX_KITS {
            return Err(refuse(
                at,
                Reason::OptionsKits {
                    place,
                    kits: held.kits,
                },
            ));
        }
        if usize::from(held.items) > MAX_ITEMS {
            return Err(refuse(
                at.saturating_add(1),
                Reason::OptionsItems {
                    place,
                    items: held.items,
                },
            ));
        }
        for _ in 0..held.kits {
            kit(&mut reader, place, held).map_err(|fault| fault.refusal(&refuse, short))?;
        }
        for item in 0..held.items {
            let at = reader.at();
            let _instance = reader.u32().map_err(short)?;
            let dict_id = reader.u16().map_err(short)?;
            if dict_id == 0 {
                held.empty_items |= 1u128.checked_shl(u32::from(item)).unwrap_or(0);
                continue;
            }
            let Some(stack) = dictionary.item(dict_id) else {
                return Err(refuse(at, Reason::OptionsDictId { place, dict_id }));
            };
            // The condition or count; an item that is no stack has its
            // repair and reroll times too (the client's `is_stack`, 0x1F1393).
            reader.skip(if stack { 4 } else { 12 }).map_err(short)?;
            let at = reader.at();
            let modifications = reader.u8().map_err(short)?;
            if modifications > MAX_MODIFICATIONS {
                return Err(refuse(
                    at,
                    Reason::OptionsModifications {
                        place,
                        count: modifications,
                    },
                ));
            }
            for _ in 0..modifications {
                let _id = reader.u32().map_err(short)?;
                let at = reader.at();
                let byte = reader.u8().map_err(short)?;
                if !matches!(byte, 0 | 1 | 0xFF) {
                    return Err(refuse(at, Reason::OptionsBool { place, byte }));
                }
            }
            let at = reader.at();
            let upgrades = reader.u8().map_err(short)?;
            if upgrades > MAX_UPGRADES {
                return Err(refuse(
                    at,
                    Reason::OptionsUpgrades {
                        place,
                        count: upgrades,
                    },
                ));
            }
            reader
                .skip(usize::from(upgrades).saturating_mul(UPGRADE))
                .map_err(short)?;
        }
        // A kit's slot names an item of the table that is not empty: the
        // client looks it up in the dictionary unchecked (0x2036D9 ->
        // 0x203742).
        let items = 1u128
            .checked_shl(u32::from(held.items))
            .map_or(u128::MAX, |bit| bit.wrapping_sub(1));
        let wrong = held.slotted & (!items | held.empty_items);
        if wrong != 0 {
            return Err(refuse(
                held.at,
                Reason::OptionsSlotItem {
                    place,
                    index: wrong.trailing_zeros() as u16,
                },
            ));
        }
    }
    let at = reader.at();
    let map = reader.u8().map_err(short)?;
    let roster = reader.u8().map_err(short)?;
    if !(1..=MAX_PLAYERS).contains(&usize::from(roster)) {
        return Err(refuse(
            at.saturating_add(1),
            Reason::OptionsRoster { roster },
        ));
    }
    if !dictionary.map(map) {
        return Err(refuse(at, Reason::OptionsMap { map }));
    }
    for (place, (held, team)) in places.iter().zip(teams).enumerate() {
        if place < usize::from(roster) {
            if team > 1 {
                return Err(refuse(held.at, Reason::OptionsTeam { place, team }));
            }
            if held.kits == 0 {
                return Err(refuse(held.at, Reason::OptionsNoKit { place }));
            }
        } else if held.kits != 0 || held.items != 0 {
            return Err(refuse(held.at, Reason::OptionsPastRoster { place }));
        }
    }
    let _match_id = reader.u32().map_err(short)?;
    let _level = reader.u16().map_err(short)?;
    for event in 0..SCORE_EVENTS {
        let at = reader.at();
        if !finite(reader.f32_bits().map_err(short)?) {
            return Err(refuse(at, Reason::OptionsWeight { event }));
        }
    }
    // The squad bytes.
    reader.skip(MAX_PLAYERS).map_err(short)?;
    if !reader.is_done() {
        return Err(refuse(
            reader.at(),
            Reason::OptionsTrailing {
                len: reader.rest().len(),
            },
        ));
    }
    Ok(roster)
}

/// A fault while reading a part of a profile.
enum Fault {
    Short(Short),
    At(usize, Reason),
}

impl From<Short> for Fault {
    fn from(short: Short) -> Self {
        Fault::Short(short)
    }
}

impl Fault {
    fn refusal(
        self,
        refuse: &dyn Fn(usize, Reason) -> Refusal,
        short: impl Fn(Short) -> Refusal,
    ) -> Refusal {
        match self {
            Fault::Short(error) => short(error),
            Fault::At(at, reason) => refuse(at, reason),
        }
    }
}

/// A string: a length byte, then that many bytes with no NUL, under the
/// buffer of `buffer` bytes with its NUL.
fn text(reader: &mut Reader, buffer: usize, place: usize) -> Result<(), Fault> {
    let at = reader.at();
    let len = reader.u8()?;
    if usize::from(len) >= buffer {
        return Err(Fault::At(at, Reason::OptionsText { place, len }));
    }
    if reader.take(usize::from(len))?.contains(&0) {
        return Err(Fault::At(at, Reason::OptionsText { place, len }));
    }
    Ok(())
}

/// A kit (0x1F112A..0x1F12F5): state, id, name, the slot mask and each
/// slot's item index and count, the modifier mask (high qword first) and a
/// float for each modifier set.
fn kit(reader: &mut Reader, place: usize, held: &mut Place) -> Result<(), Fault> {
    let _state = reader.u8()?;
    let _id = reader.u32()?;
    text(reader, NAME_BUFFER, place)?;
    let at = reader.at();
    let mask = reader.u32()?;
    if mask.checked_shr(SLOTS).unwrap_or(0) != 0 {
        return Err(Fault::At(at, Reason::OptionsSlotMask { place, mask }));
    }
    for _ in 0..mask.count_ones() {
        let at = reader.at();
        let index = reader.u16()?;
        let _count = reader.u16()?;
        if index >= u16::from(held.items) {
            return Err(Fault::At(at, Reason::OptionsSlotItem { place, index }));
        }
        held.slotted |= 1u128.checked_shl(u32::from(index)).unwrap_or(0);
    }
    // 68 modifiers: bits 64..67 in the high qword's low word.
    let at = reader.at();
    let words = [reader.u32()?, reader.u32()?, reader.u32()?, reader.u32()?];
    let [high, top, low, middle] = words;
    if high.checked_shr(4).unwrap_or(0) != 0 || top != 0 {
        return Err(Fault::At(at, Reason::OptionsModifierMask { place }));
    }
    let count = high
        .count_ones()
        .saturating_add(low.count_ones())
        .saturating_add(middle.count_ones());
    for _ in 0..count {
        let at = reader.at();
        if !finite(reader.f32_bits()?) {
            return Err(Fault::At(at, Reason::OptionsNotNumber { place }));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    /// The late join's proposal as `late_join::Prepare::encode` puts it:
    /// match 0x80001234, serial 1, place 2, squad 7, then a profile.
    fn prepare() -> Vec<u8> {
        let mut bytes = LATE_JOIN_PREPARE.to_vec();
        bytes.extend(0x8000_1234u32.to_le_bytes());
        bytes.extend(1u32.to_le_bytes());
        bytes.extend([2, 7]);
        bytes.extend([1, 0, 0, 0, 0x0A]);
        bytes
    }

    fn reason(id: u8, payload: &[u8]) -> Option<Reason> {
        check(id, payload).err().map(|refusal| refusal.reason)
    }

    #[test]
    fn a_late_join_proposal_is_no_static_info_of_local_place_255() {
        assert_eq!(check(id::STATIC_INFO, &prepare()), Ok(()));
        // Its header cut, its place the first or past the 20, its serial 0.
        assert_eq!(
            reason(id::STATIC_INFO, &prepare()[..13]),
            Some(Reason::MessageShort { len: 13, least: 14 })
        );
        let mut wrong = prepare();
        wrong[12] = 0;
        assert_eq!(
            reason(id::STATIC_INFO, &wrong),
            Some(Reason::LateJoinProposal)
        );
        wrong[12] = 20;
        assert_eq!(
            reason(id::STATIC_INFO, &wrong),
            Some(Reason::PlacePast { place: 20 })
        );
        let mut wrong = prepare();
        wrong[8] = 0;
        assert_eq!(
            reason(id::STATIC_INFO, &wrong),
            Some(Reason::LateJoinProposal)
        );
        // Another version, or a stock 0xE4 of local place 255: refused.
        let mut wrong = prepare();
        wrong[3] = 2;
        assert_eq!(
            reason(id::STATIC_INFO, &wrong),
            Some(Reason::PlacePast { place: 255 })
        );
    }
}
