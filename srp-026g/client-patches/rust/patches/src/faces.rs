//! Patch 123: every player is drawn with the face srp gives the account
//! (docs/notes/face-choice.md). The faces are the game data's:
//! gameplay/bodyparts/default's `body_parts.heads`, each with its `data`,
//! `data_helmet` and `data_mask` models; face 0 is today's head. The hooks
//! are `faces.toml`'s:
//!
//! - what srp says of the faces, a table per thing it says it of: the
//!   account's own (`lobby_client.own_account`, srp's byte after `0x87`) and
//!   its squad members' (`lobby_client.squad_member`, after `0x88`), by
//!   name; and the faces of the match or replay being shown, by place
//!   (`static_info.options_read`, `replay.options_read`: srp's block entry 5
//!   after the options, a byte a place; no entry, every face 0), until its
//!   scene is left (`game_world.deactivate`);
//! - the character's cook (`skin_cook.head`, profile_skin_visual_cook's
//!   default head, reached only when no item's `remove_head_model` took the
//!   head): a profile that is a place of the match shown is drawn by the
//!   match's options, any other (the lobby's own and squad characters) by
//!   the lobby's messages; the head is that face's, its variant by what the kit's
//!   models cover (a mask, else a helmet, else none), a missing variant the
//!   face's `data`, a face the data does not list face 0; its additional
//!   models (hair) records of the head too; the skin's name (the cache key)
//!   gets `f<face>_` for a face but 0. Data without `heads`: the native head.
//!
//! Nothing a match computes: the look only.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::config::{ConfigEntries, ConfigTable};
use crate::vostok::lobby_menu::{OwnAccountRead, SquadMemberRead};
use crate::vostok::log::{self, Piece::*};
use crate::vostok::parse::Reader;
use crate::vostok::skin_cook::{Cover, HeadCook, Model};
use crate::vostok::{Context, FacesOnHeadExit, Flow, Lock, NoExit, OptionsRead, PLACES, Places, Thread};

/// srp's block after the options: its magic, then `{u8 tag, u8 len, bytes}`.
const SRPX: [u8; 4] = *b"SRPX";
/// The faces' entry: a byte a place.
const TAG_FACES: u8 = 5;
/// The squad members' faces kept: a squad's, and room to spare.
const SQUAD: usize = 8;
/// A name's bytes without its zero (the profile's 64).
const NAME: usize = 63;

/// A face: an index of the data's `heads`, as srp gives it. One the data
/// does not list is drawn as [`Face::FIRST`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Face(u8);

impl Face {
    /// Today's head: everyone's who has not chosen.
    pub const FIRST: Face = Face(0);

    pub const fn of_byte(byte: u8) -> Face {
        Face(byte)
    }

    fn index(self) -> usize {
        usize::from(self.0)
    }

    /// What it adds to the skin's name: `f<face>_`, nothing for face 0 (so
    /// today's skins keep their names).
    fn suffix(self) -> Option<Suffix> {
        if self == Face::FIRST {
            return None;
        }
        let mut suffix = Suffix { bytes: [0; 5], len: 0 };
        suffix.push(b'f');
        let n = self.0;
        for (digit, shown) in [(n / 100, n >= 100), (n / 10 % 10, n >= 10), (n % 10, true)] {
            if shown {
                suffix.push(b'0' + digit);
            }
        }
        suffix.push(b'_');
        Some(suffix)
    }
}

/// `f`, up to three digits, `_`.
#[derive(Clone, Copy, Debug)]
struct Suffix {
    bytes: [u8; 5],
    len: usize,
}

impl Suffix {
    fn push(&mut self, byte: u8) {
        self.bytes[self.len] = byte;
        self.len += 1;
    }

    fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

/// A player's name, what a face is known by: 1 to 63 bytes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Name {
    len: u8,
    bytes: [u8; NAME],
}

impl Name {
    pub fn of(name: &CStr) -> Option<Name> {
        let text = name.to_bytes();
        if text.is_empty() || text.len() > NAME {
            return None;
        }
        let mut bytes = [0; NAME];
        bytes[..text.len()].copy_from_slice(text);
        Some(Name { len: text.len() as u8, bytes })
    }
}

/// The squad members' faces as the lobby told them (`0x88`), each name
/// once, the one told last first: a name told again is young again, and
/// the one told longest ago makes room.
struct Squad([Option<(Name, Face)>; SQUAD]);

impl Squad {
    const fn new() -> Squad {
        Squad([None; SQUAD])
    }

    fn learn(&mut self, name: Name, face: Face) {
        let was = self.0.iter().position(|entry| entry.is_some_and(|(known, _)| known == name));
        let at = was.unwrap_or(SQUAD - 1);
        self.0.copy_within(..at, 1);
        self.0[0] = Some((name, face));
    }

    fn face_of(&self, name: &Name) -> Option<Face> {
        self.0.iter().flatten().find(|(known, _)| known == name).map(|&(_, face)| face)
    }
}

/// The match or replay being shown: where its places are, and srp's face
/// of each (a place srp's entry does not reach: face 0).
struct Roster {
    places: Places,
    faces: [Face; PLACES],
}

impl Roster {
    fn new(places: Places, faces: &[u8]) -> Roster {
        let mut roster = Roster { places, faces: [Face::FIRST; PLACES] };
        for (face, &byte) in roster.faces.iter_mut().zip(faces) {
            *face = Face::of_byte(byte);
        }
        roster
    }

    /// The face of the profile at `profile`, when it is one of its places.
    fn face_at(&self, profile: u32) -> Option<Face> {
        self.places.place_of(profile).and_then(|place| self.faces.get(place).copied())
    }
}

/// What srp said of the faces: a table per thing.
struct Known {
    /// The account's own (`0x87`).
    own: Option<(Name, Face)>,
    /// Its squad members' (`0x88`).
    squad: Squad,
    /// The match or replay shown, from its options' read until its scene
    /// is left.
    roster: Option<Roster>,
}

impl Known {
    const fn new() -> Known {
        Known { own: None, squad: Squad::new(), roster: None }
    }

    /// The face a profile is drawn with. A place of the match shown: the
    /// match's options', whatever the lobby says of its name (a replay is
    /// drawn as recorded; a player seen by every game alike). Any other
    /// profile is the lobby's: its own account's or a squad member's, by
    /// its name; else face 0.
    fn face_of(&self, profile: u32, name: Option<&Name>) -> Face {
        if let Some(face) = self.roster.as_ref().and_then(|roster| roster.face_at(profile)) {
            return face;
        }
        let Some(name) = name else { return Face::FIRST };
        match self.own {
            Some((own, face)) if own == *name => face,
            _ => self.squad.face_of(name).unwrap_or(Face::FIRST),
        }
    }
}

static KNOWN: Lock<Known> = Lock::new(Known::new());

/// The faces' entry of srp's block (the bytes after the options' squads):
/// its magic, then entries; an entry of another tag is stepped over, a cut
/// one ends the block. None: no entry (every face 0).
pub fn faces_in(mut rest: Reader<'_>) -> Option<&[u8]> {
    if rest.array::<4>() != Some(SRPX) {
        return None;
    }
    let mut faces = None;
    while let (Some(tag), Some(len)) = (rest.u8(), rest.u8()) {
        let Some(entry) = rest.take(usize::from(len)) else { break };
        if tag == TAG_FACES {
            faces = Some(entry);
        }
    }
    faces
}

/// What the lobby said: a name's face, when the message has srp's byte.
fn told(name: Option<&CStr>, byte: Option<u8>, what: &str) -> Option<(Name, Face)> {
    if byte.is_none() {
        log::line(&[Text("faces: "), Text(what), Text(" without srp's face byte")]);
    }
    Some((Name::of(name?)?, Face::of_byte(byte?)))
}

/// process_own_account after its last field (0x27030B).
pub fn on_own_account(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(read) = OwnAccountRead::at(ctx)
        && let Some(own) = told(read.name(), read.srp_byte(), "an account (0x87)")
    {
        KNOWN.with(ctx.thread(), |known| known.own = Some(own));
    }
    Flow::Continue
}

/// The squad member's account after its revision (0x274C01).
pub fn on_squad_member(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(read) = SquadMemberRead::at(ctx)
        && let Some((name, face)) = told(read.name(), read.srp_byte(), "a squad member's account (0x88)")
    {
        KNOWN.with(ctx.thread(), |known| known.squad.learn(name, face));
    }
    Flow::Continue
}

fn options_read(thread: &Thread, read: Option<OptionsRead<'_>>) -> Flow<NoExit> {
    if let Some(read) = read {
        let roster = Roster::new(read.place_array(), faces_in(read.rest()).unwrap_or(&[]));
        KNOWN.with(thread, |known| known.roster = Some(roster));
    }
    Flow::Continue
}

/// process_static_match_info after its deserialize (0x27B92D).
pub fn on_static_info(ctx: &mut Context<'_>) -> Flow<NoExit> {
    options_read(ctx.thread(), OptionsRead::at_static_info(ctx))
}

/// A replay's chunk 1 after its deserialize (0x1F0971).
pub fn on_replay_options(ctx: &mut Context<'_>) -> Flow<NoExit> {
    options_read(ctx.thread(), OptionsRead::at_replay(ctx))
}

/// game_world::on_deactivate (0x1F9520): the match's or the replay's scene
/// is left, and with it its faces.
pub fn on_world_left(ctx: &mut Context<'_>) -> Flow<NoExit> {
    KNOWN.with(ctx.thread(), |known| known.roster = None);
    Flow::Continue
}

/// The variant of a face the cover wears.
fn variant(cover: Cover) -> &'static CStr {
    match cover {
        Cover::Mask => c"data_mask",
        Cover::Helmet => c"data_helmet",
        Cover::Bare => c"data",
    }
}

/// `heads[face]`'s model for `cover` (its `data` without the variant), when
/// the entry has one.
fn model_of<'c>(heads: &ConfigEntries<'c>, face: Face, cover: Cover) -> Option<ConfigTable<'c>> {
    let head = heads.iter().nth(face.index())?.ok()?.table().ok()?;
    let model = match head.get(variant(cover)).ok()? {
        Some(model) => model,
        None => head.get(c"data").ok()??,
    };
    model.table().ok()
}

/// The face drawn and its model: `face`'s, else face 0's (a face the data
/// does not list, or a broken entry).
fn head_of<'c>(heads: &ConfigEntries<'c>, face: Face, cover: Cover) -> Option<(Face, ConfigTable<'c>, Model<'c>)> {
    [face, Face::FIRST].into_iter().find_map(|face| {
        let table = model_of(heads, face, cover)?;
        let model = Model::of(&table).ok()?;
        Some((face, table, model))
    })
}

/// profile_skin_visual_cook's default head (0x205662): the face's.
pub fn on_head(ctx: &mut Context<'_>) -> Flow<FacesOnHeadExit> {
    let Some(mut cook) = HeadCook::at(ctx) else { return Flow::Continue };
    let Some(body_parts) = cook.body_parts() else { return Flow::Continue };
    // Data without faces: the native head, as today.
    let Ok(Some(heads)) = body_parts.get(c"heads") else { return Flow::Continue };
    let Ok(heads) = heads.entries() else { return Flow::Continue };
    let name = cook.player_name().and_then(Name::of);
    let face = KNOWN.with(ctx.thread(), |known| known.face_of(cook.profile_address(), name.as_ref()));
    let Some((face, table, model)) = head_of(&heads, face, cook.cover()) else {
        log::line(&[Text("faces: body_parts.heads has no face 0 model; the native head drawn")]);
        return Flow::Continue;
    };
    cook.make_head(&model);
    if let Ok(Some(extra)) = table.get(c"additional_models")
        && let Ok(extra) = extra.entries()
    {
        for part in extra.iter() {
            let Some(part) = part.ok().and_then(|part| part.table().ok()) else { continue };
            let Ok(part) = Model::of(&part) else { continue };
            if cook.add_head_part(&part).is_err() {
                log::line(&[Text("faces: no record left for a head's additional model")]);
                break;
            }
        }
    }
    if let Some(suffix) = face.suffix()
        && cook.name_skin(suffix.as_bytes()).is_err()
    {
        log::line(&[Text("faces: the skin's name has no room for its face")]);
    }
    Flow::Exit(FacesOnHeadExit::Made)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(text: &CStr) -> Name {
        Name::of(text).unwrap()
    }

    #[test]
    fn a_face_adds_its_number_to_the_skin_name() {
        assert!(Face::FIRST.suffix().is_none());
        assert_eq!(Face(3).suffix().unwrap().as_bytes(), b"f3_");
        assert_eq!(Face(10).suffix().unwrap().as_bytes(), b"f10_");
        assert_eq!(Face(255).suffix().unwrap().as_bytes(), b"f255_");
    }

    #[test]
    fn names_are_one_to_63_bytes() {
        assert!(Name::of(c"").is_none());
        assert!(Name::of(c"a").is_some());
        let long = [b'x'; 64];
        let mut text = [0u8; 65];
        text[..64].copy_from_slice(&long);
        assert!(Name::of(CStr::from_bytes_until_nul(&text).unwrap()).is_none());
        assert!(Name::of(CStr::from_bytes_until_nul(&text[1..]).unwrap()).is_some());
    }

    /// Options whose places begin at 0x1000_0000, and the profile of a place.
    const OPTIONS: u32 = 0x1000_0000;
    fn place(n: u32) -> u32 {
        OPTIONS + n * 0x45FC
    }
    /// A profile of the lobby's: none of the options' places.
    const LOBBY_PROFILE: u32 = 0x2000_0000;

    #[test]
    fn a_match_is_drawn_by_its_options_and_the_lobby_by_its_messages() {
        let mut known = Known::new();
        known.own = Some((name(c"me"), Face(2)));
        known.squad.learn(name(c"mate"), Face(3));
        // No match shown: the lobby's characters, by name.
        assert_eq!(known.face_of(LOBBY_PROFILE, Some(&name(c"me"))), Face(2));
        assert_eq!(known.face_of(LOBBY_PROFILE, Some(&name(c"mate"))), Face(3));
        assert_eq!(known.face_of(LOBBY_PROFILE, Some(&name(c"else"))), Face::FIRST);
        assert_eq!(known.face_of(LOBBY_PROFILE, None), Face::FIRST);
        // A match (or a replay): its places by its options, whatever the
        // lobby says of the names in them; a place past srp's bytes face 0.
        known.roster = Some(Roster::new(Places::at(OPTIONS), &[4, 7]));
        assert_eq!(known.face_of(place(0), Some(&name(c"me"))), Face(4));
        assert_eq!(known.face_of(place(1), Some(&name(c"mate"))), Face(7));
        assert_eq!(known.face_of(place(2), Some(&name(c"mate"))), Face::FIRST);
        assert_eq!(known.face_of(place(19), None), Face::FIRST);
        // The lobby's own profiles while it is shown: the lobby's still.
        assert_eq!(known.face_of(LOBBY_PROFILE, Some(&name(c"me"))), Face(2));
        assert_eq!(known.face_of(place(20), Some(&name(c"me"))), Face(2));
        assert_eq!(known.face_of(place(0) + 8, Some(&name(c"mate"))), Face(3));
        // Its scene left: nothing of it stays.
        known.roster = None;
        assert_eq!(known.face_of(place(0), Some(&name(c"else"))), Face::FIRST);
        assert_eq!(known.face_of(place(0), Some(&name(c"me"))), Face(2));
    }

    #[test]
    fn the_squad_keeps_each_name_once_and_drops_the_one_told_longest_ago() {
        let mut known = Known::new();
        known.own = Some((name(c"me"), Face(9)));
        known.squad.learn(name(c"a"), Face(1));
        known.squad.learn(name(c"a"), Face(5));
        assert_eq!(known.squad.0.iter().flatten().count(), 1);
        assert_eq!(known.squad.face_of(&name(c"a")), Some(Face(5)));
        let names: [&CStr; SQUAD] = [c"b", c"c", c"d", c"e", c"f", c"g", c"h", c"i"];
        for (i, other) in names.iter().take(SQUAD - 1).enumerate() {
            known.squad.learn(name(other), Face(10 + i as u8));
        }
        // Full: "a" was told first, but told again it is the youngest, and
        // the ninth name takes "b"'s room.
        known.squad.learn(name(c"a"), Face(6));
        known.squad.learn(name(c"i"), Face(20));
        assert_eq!(known.squad.face_of(&name(c"a")), Some(Face(6)));
        assert_eq!(known.squad.face_of(&name(c"b")), None);
        assert_eq!(known.squad.face_of(&name(c"c")), Some(Face(11)));
        assert_eq!(known.squad.face_of(&name(c"i")), Some(Face(20)));
        // However many squad members are told, the account's own stays.
        assert_eq!(known.face_of(LOBBY_PROFILE, Some(&name(c"me"))), Face(9));
    }

    #[test]
    fn the_faces_entry_is_read_among_others() {
        assert_eq!(faces_in(Reader::new(b"")), None);
        assert_eq!(faces_in(Reader::new(b"SRPX")), None);
        assert_eq!(faces_in(Reader::new(b"SRPX\x02\x01\x01")), None);
        assert_eq!(faces_in(Reader::new(b"SRPX\x05\x02\x05\x00")), Some(&[5, 0][..]));
        assert_eq!(faces_in(Reader::new(b"SRPX\x02\x01\x01\x05\x01\x09")), Some(&[9][..]));
        assert_eq!(faces_in(Reader::new(b"SRPX\x05\x03\x01")), None);
        assert_eq!(faces_in(Reader::new(b"SRPY\x05\x01\x01")), None);
    }
}
