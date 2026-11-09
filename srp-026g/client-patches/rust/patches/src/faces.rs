//! Patch 123: every player is drawn with the face srp gives the account
//! (docs/notes/face-choice.md). The faces are the game data's:
//! gameplay/bodyparts/default's `body_parts.heads`, each with its `data`,
//! `data_helmet` and `data_mask` models; face 0 is today's head. The hooks
//! are `faces.toml`'s:
//!
//! - what srp says of the faces, known by the player's name: the account's
//!   (`lobby_client.own_account`, srp's byte after `0x87`), a squad member's
//!   (`lobby_client.squad_member`, after `0x88`), and every place's of the
//!   match or replay just read (`static_info.options_read`,
//!   `replay.options_read`: srp's block entry 3 after the options, a byte a
//!   place; no entry, every face 0), and a late joiner's of the match in play
//!   (`match_dispatch.message`, after patch 114's check: the entry of its one
//!   place after its proposal's profile). The lobby's are asked first: they
//!   are the freshest;
//! - the character's cook (`skin_cook.head`, profile_skin_visual_cook's
//!   default head, reached only when no item's `remove_head_model` took the
//!   head): the head is the player's face's, its variant by what the kit's
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
use crate::vostok::match_message::Message;
use crate::vostok::log::{self, Piece::*};
use crate::vostok::parse::Reader;
use crate::vostok::skin_cook::{Cover, HeadCook, Model};
use crate::vostok::{Context, FacesOnHeadExit, FacesOnMatchMessageExit, Flow, Lock, NoExit, OptionsRead, Thread};

/// srp's block after the options: its magic, then `{u8 tag, u8 len, bytes}`.
const SRPX: [u8; 4] = *b"SRPX";
/// The faces' entry: a byte a place.
const TAG_FACES: u8 = 3;
/// A match's places.
const PLACES: usize = 20;
/// The lobby's faces kept: the account's and its squad's, and room to spare.
const LOBBY: usize = 8;
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
        Name::of_bytes(name.to_bytes())
    }

    /// A name's bytes, without its zero (a zero in them: none).
    pub fn of_bytes(text: &[u8]) -> Option<Name> {
        if text.is_empty() || text.len() > NAME || text.contains(&0) {
            return None;
        }
        let mut bytes = [0; NAME];
        bytes[..text.len()].copy_from_slice(text);
        Some(Name { len: text.len() as u8, bytes })
    }
}

/// What srp said of the faces.
struct Known {
    /// The lobby's: the account's and its squad members', each name once.
    lobby: [Option<(Name, Face)>; LOBBY],
    /// The lobby entry a new name takes when every one is held.
    oldest: usize,
    /// The last match's or replay's, by place, its late joiners' too.
    world: [Option<(Name, Face)>; PLACES],
}

impl Known {
    const fn new() -> Known {
        Known { lobby: [None; LOBBY], oldest: 0, world: [None; PLACES] }
    }

    /// The face of `name`: the lobby's, else the last match's (its latest
    /// place: one who left and came back has a new place), else face 0.
    fn face_of(&self, name: &Name) -> Face {
        self.lobby.iter().chain(self.world.iter().rev()).flatten()
            .find(|(known, _)| known == name)
            .map_or(Face::FIRST, |&(_, face)| face)
    }

    /// The lobby says `name` has `face`.
    fn learn(&mut self, name: Name, face: Face) {
        let slot = self.lobby.iter().position(|entry| entry.is_some_and(|(known, _)| known == name))
            .or_else(|| self.lobby.iter().position(Option::is_none))
            .unwrap_or_else(|| {
                let oldest = self.oldest;
                self.oldest = (oldest + 1) % LOBBY;
                oldest
            });
        self.lobby[slot] = Some((name, face));
    }

    /// A match's (or replay's) places, place 0 first: its names, and srp's
    /// faces of them (a place past them face 0).
    fn set_world<'n>(&mut self, names: impl Iterator<Item = Option<&'n CStr>>, faces: &[u8]) {
        self.world = [None; PLACES];
        for (place, name) in names.take(PLACES).enumerate() {
            let face = faces.get(place).map_or(Face::FIRST, |&byte| Face::of_byte(byte));
            self.world[place] = name.and_then(Name::of).map(|name| (name, face));
        }
    }

    /// A late joiner of the match: its place, its name and srp's face of it.
    fn set_place(&mut self, place: usize, name: Name, face: Face) {
        if let Some(entry) = self.world.get_mut(place) {
            *entry = Some((name, face));
        }
    }
}

static KNOWN: Lock<Known> = Lock::new(Known::new());

/// The faces' entry of srp's block (the bytes after the options' squads, or
/// after a late joiner's profile): its magic, then entries; an entry of
/// another tag is stepped over, a cut one ends the block. None: no entry
/// (every face 0).
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

fn learn(thread: &Thread, name: Option<&CStr>, byte: Option<u8>, what: &str) {
    match (name.and_then(Name::of), byte) {
        (Some(name), Some(byte)) => KNOWN.with(thread, |known| known.learn(name, Face::of_byte(byte))),
        (_, None) => log::line(&[Text("faces: "), Text(what), Text(" without srp's face byte")]),
        (None, Some(_)) => {}
    }
}

/// process_own_account after its last field (0x27030B).
pub fn on_own_account(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(read) = OwnAccountRead::at(ctx) {
        learn(ctx.thread(), read.name(), read.srp_byte(), "an account (0x87)");
    }
    Flow::Continue
}

/// The squad member's account after its revision (0x274C01).
pub fn on_squad_member(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(read) = SquadMemberRead::at(ctx) {
        learn(ctx.thread(), read.name(), read.srp_byte(), "a squad member's account (0x88)");
    }
    Flow::Continue
}

fn options_read(thread: &Thread, read: Option<OptionsRead<'_>>) -> Flow<NoExit> {
    if let Some(read) = read {
        let faces = faces_in(read.rest()).unwrap_or(&[]);
        KNOWN.with(thread, |known| known.set_world(read.place_names(), faces));
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

/// srp's block after a late joiner's profile: none (face 0), or its magic
/// and the faces entry of its one place, any byte. Anything else is no
/// block srp sends: none.
fn late_joiner_face(block: &[u8]) -> Option<Face> {
    match block {
        [] => Some(Face::FIRST),
        [b'S', b'R', b'P', b'X', TAG_FACES, 1, face] => Some(Face::of_byte(*face)),
        _ => None,
    }
}

/// A match message at the dispatch (0x27A31F), after patch 114's check: a
/// late joiner's proposal says its place's name and, in srp's block after
/// the profile, its face. Kept before the late join's code takes the
/// proposal and cooks its character (`lj_prepare`, the native handler after
/// this), and the block taken off the message: that code reads the native
/// profile to the message's end. Bytes after the profile that are no such
/// block are left for its check to refuse the proposal.
pub fn on_match_message(ctx: &mut Context<'_>) -> Flow<FacesOnMatchMessageExit> {
    let Some(mut message) = Message::at_dispatch(ctx) else { return Flow::Continue };
    let Some(proposal) = message.proposal() else { return Flow::Continue };
    let Some(face) = late_joiner_face(proposal.block) else {
        log::line(&[Text("faces: a late joiner's proposal ends in bytes that are no srp block; left as they are")]);
        return Flow::Continue;
    };
    if let Some(name) = Name::of_bytes(proposal.name) {
        KNOWN.with(ctx.thread(), |known| known.set_place(usize::from(proposal.place), name, face));
    }
    message.end_at_profile(proposal);
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
    let face = cook.player_name().and_then(Name::of)
        .map_or(Face::FIRST, |name| KNOWN.with(ctx.thread(), |known| known.face_of(&name)));
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

    #[test]
    fn the_lobby_is_asked_before_the_match() {
        let mut known = Known::new();
        assert_eq!(known.face_of(&name(c"me")), Face::FIRST);
        known.set_world([Some(c"me"), Some(c"you"), None].into_iter(), &[4, 7]);
        assert_eq!(known.face_of(&name(c"me")), Face(4));
        assert_eq!(known.face_of(&name(c"you")), Face(7));
        known.learn(name(c"me"), Face(2));
        assert_eq!(known.face_of(&name(c"me")), Face(2));
        // A place past srp's bytes is face 0; a new match forgets the old.
        known.set_world([Some(c"you"), Some(c"them")].into_iter(), &[1]);
        assert_eq!(known.face_of(&name(c"you")), Face(1));
        assert_eq!(known.face_of(&name(c"them")), Face::FIRST);
        known.set_world(core::iter::empty(), &[]);
        assert_eq!(known.face_of(&name(c"you")), Face::FIRST);
        assert_eq!(known.face_of(&name(c"me")), Face(2));
    }

    #[test]
    fn a_late_joiner_is_known_by_its_place_and_a_new_place_wins() {
        let mut known = Known::new();
        known.set_world([Some(c"you"), Some(c"back")].into_iter(), &[1, 3]);
        known.set_place(2, name(c"late"), Face(5));
        assert_eq!(known.face_of(&name(c"late")), Face(5));
        assert_eq!(known.face_of(&name(c"you")), Face(1));
        // One who left and came back is a new place: its face there.
        known.set_place(3, name(c"back"), Face(4));
        assert_eq!(known.face_of(&name(c"back")), Face(4));
        // No place past the match's 20; a new match forgets them.
        known.set_place(PLACES, name(c"none"), Face(6));
        assert_eq!(known.face_of(&name(c"none")), Face::FIRST);
        known.set_world([Some(c"you")].into_iter(), &[]);
        assert_eq!(known.face_of(&name(c"late")), Face::FIRST);
        assert!(Name::of_bytes(b"").is_none() && Name::of_bytes(b"a\0b").is_none());
    }

    #[test]
    fn the_lobby_keeps_each_name_once_and_drops_the_oldest() {
        let mut known = Known::new();
        known.learn(name(c"me"), Face(1));
        known.learn(name(c"me"), Face(5));
        assert_eq!(known.lobby.iter().flatten().count(), 1);
        assert_eq!(known.face_of(&name(c"me")), Face(5));
        let names: [&CStr; LOBBY] = [c"a", c"b", c"c", c"d", c"e", c"f", c"g", c"h"];
        for (i, other) in names.iter().enumerate() {
            known.learn(name(other), Face(i as u8));
        }
        // "me" was the oldest: its place went to the eighth.
        assert_eq!(known.face_of(&name(c"me")), Face::FIRST);
        assert_eq!(known.face_of(&name(c"h")), Face(7));
        assert_eq!(known.face_of(&name(c"a")), Face(0));
        assert_eq!(known.face_of(&name(c"b")), Face(1));
    }

    #[test]
    fn the_faces_entry_is_read_among_others() {
        assert_eq!(faces_in(Reader::new(b"")), None);
        assert_eq!(faces_in(Reader::new(b"SRPX")), None);
        assert_eq!(faces_in(Reader::new(b"SRPX\x02\x01\x01")), None);
        assert_eq!(faces_in(Reader::new(b"SRPX\x03\x02\x05\x00")), Some(&[5, 0][..]));
        assert_eq!(faces_in(Reader::new(b"SRPX\x02\x01\x01\x03\x01\x09")), Some(&[9][..]));
        assert_eq!(faces_in(Reader::new(b"SRPX\x03\x03\x01")), None);
        assert_eq!(faces_in(Reader::new(b"SRPY\x03\x01\x01")), None);
    }

    #[test]
    fn a_late_joiners_block_is_its_one_face_or_none() {
        assert_eq!(late_joiner_face(b""), Some(Face::FIRST));
        assert_eq!(late_joiner_face(b"SRPX\x03\x01\x09"), Some(Face(9)));
        for other in [&b"SRPX"[..], b"SRPX\x03\x01", b"SRPX\x03\x02\x01\x01", b"SRPX\x02\x01\x01",
                      b"SRPX\x03\x01\x01\x00", b"SRPY\x03\x01\x01", b"\x00"] {
            assert_eq!(late_joiner_face(other), None, "{other:?}");
        }
    }
}
