//! The replay viewer's control messages: a recording played to a game as a
//! spectated match (path A, `docs/notes/replay-stream.md`). One encoding,
//! shared by two builds:
//! - the match server, which plays the recording and decides the playback;
//! - the game, in the exe's arena (patch 130,
//!   `client-patches/rust/patches/src/replay_viewer.rs`), which asks for a
//!   change from its keys and applies what the server tells it.
//!
//! After the message's id, both ways, a first byte [`TAG`]; little endian;
//! every time is the match's clock in ms, the one an 0xE5 sets ([`MatchMs`]).
//!
//! - Server -> game, an 0xF6 (reliable, unique, length-prefixed; an 0xF6
//!   whose payload is `00` stays the snapshot upload's request), a
//!   [`ServerNote`]:
//!   - `52 01 revision paused speed:u16 start:u32 end:u32`, the
//!     [`Playback`] (14 bytes). The first one makes a spectator's game a
//!     replay's viewer;
//!   - `52 02 first:u16 total:u16`, then up to [`EVENTS_PER_NOTE`] events of
//!     7 bytes `time:u32 kind:u8 actor:u8 victim:u8` (a place, 0xFF none):
//!     the [`EventsNote`], the events `first..` of the replay's `total`;
//!   - `52 03 place`, [`ServerNote::View`]: the place the watcher asked to
//!     see, once its game has its first world (a replay's or a live match's
//!     spectator); the game follows it;
//!   - `52 04 place count`, then `count` kits of `kit items` and each item's
//!     `len name`: the [`KitsNote`], the kits a place used in the replay,
//!     for the viewer's read-only kit view. The names are the items' own
//!     (`st_<name>_name` the game's translation of each).
//! - Game -> server, an 0xE0 (reliable and plain: it is resent every 500 ms
//!   until acked, and may come twice or late), a [`Request`]:
//!   [`REQUEST_HEAD`] then `seq op value:u32` (14 bytes). The game's
//!   datagram may keep its multi flag, so the transport finds the 0xE0's end
//!   by that head: as a snapshot chunk's header it is none (time 0xFFFFFFFF,
//!   offset 0xFFFF), nor is it a resource ack's (`FF x6, 0|1, 0`). The server
//!   takes a request only when its `seq` is [`newer`] than the last one it
//!   took.
//!
//! `no_std`, no `alloc`, no `unsafe`; a read never panics.

#![no_std]

/// The first byte of every server note here ('R').
pub const TAG: u8 = 0x52;

/// A request's first bytes: no snapshot chunk's header and no resource
/// ack's, so `udp-transport` can tell its length in a multi datagram.
pub const REQUEST_HEAD: [u8; 8] = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, TAG, 0];

/// A time on the match's clock, in ms (the clock an 0xE5 sets, the
/// recording's own).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MatchMs(pub u32);

impl MatchMs {
    pub fn plus(self, ms: u32) -> MatchMs {
        MatchMs(self.0.saturating_add(ms))
    }

    pub fn minus(self, ms: u32) -> MatchMs {
        MatchMs(self.0.saturating_sub(ms))
    }

    /// The time held within `start..=end`.
    pub fn within(self, start: MatchMs, end: MatchMs) -> MatchMs {
        self.max(start).min(end)
    }
}

/// `revision` (or a request's `seq`) came after `than`: by a wrapping u8,
/// up to 127 ahead.
pub fn newer(revision: u8, than: u8) -> bool {
    (revision.wrapping_sub(than) as i8) > 0
}

/// A playback speed: one of the viewer's steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Speed {
    Quarter,
    Half,
    Normal,
    Double,
    Quadruple,
    Octuple,
}

impl Speed {
    /// Every step, slowest first.
    pub const ALL: [Speed; 6] = [Speed::Quarter, Speed::Half, Speed::Normal, Speed::Double, Speed::Quadruple, Speed::Octuple];

    /// The speed in hundredths, as the wire has it.
    pub const fn percent(self) -> u16 {
        match self {
            Speed::Quarter => 25,
            Speed::Half => 50,
            Speed::Normal => 100,
            Speed::Double => 200,
            Speed::Quadruple => 400,
            Speed::Octuple => 800,
        }
    }

    pub fn of_percent(percent: u16) -> Option<Speed> {
        Speed::ALL.into_iter().find(|speed| speed.percent() == percent)
    }

    /// The factor the game's clock runs at.
    pub fn factor(self) -> f32 {
        f32::from(self.percent()) / 100.0
    }

    fn index(self) -> usize {
        self as usize
    }

    /// The next step up (the fastest stays).
    pub fn faster(self) -> Speed {
        Speed::ALL.get(self.index() + 1).copied().unwrap_or(self)
    }

    /// The next step down (the slowest stays).
    pub fn slower(self) -> Speed {
        self.index().checked_sub(1).map_or(self, |i| Speed::ALL[i])
    }

    /// The step nearest `factor` by ratio (the replay controls' stepper gives
    /// any factor from 0.01 to 10); a factor that is no positive number is
    /// [`Speed::Normal`].
    pub fn nearest(factor: f32) -> Speed {
        if !(factor > 0.0 && factor < f32::INFINITY) {
            return Speed::Normal;
        }
        let ratio = |speed: Speed| {
            let step = speed.factor();
            if factor > step { factor / step } else { step / factor }
        };
        Speed::ALL.into_iter().fold(Speed::Normal, |best, speed| if ratio(speed) < ratio(best) { speed } else { best })
    }
}

/// The playback as the server runs it: absolute, so a note taken twice or
/// one of several changes in a row leaves the game right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Playback {
    /// Counts the server's notes: the game takes one only when it is
    /// [`newer`] than the last it took (the transport may reorder them).
    pub revision: u8,
    pub paused: bool,
    pub speed: Speed,
    /// The replay's bar: where it opens (the end of the match's countdown)
    /// and the recording's last time.
    pub start: MatchMs,
    pub end: MatchMs,
}

const PLAYBACK: u8 = 1;
const EVENTS: u8 = 2;
const VIEW: u8 = 3;
const KITS: u8 = 4;

impl Playback {
    /// Its bytes after the message's id.
    pub const LEN: usize = 14;

    pub fn encode(&self) -> [u8; Playback::LEN] {
        let mut out = [0; Playback::LEN];
        out[0] = TAG;
        out[1] = PLAYBACK;
        out[2] = self.revision;
        out[3] = u8::from(self.paused);
        out[4..6].copy_from_slice(&self.speed.percent().to_le_bytes());
        out[6..10].copy_from_slice(&self.start.0.to_le_bytes());
        out[10..14].copy_from_slice(&self.end.0.to_le_bytes());
        out
    }

    fn parse(body: &[u8]) -> Option<Playback> {
        let [revision, paused, s0, s1, a0, a1, a2, a3, b0, b1, b2, b3] = *body else { return None };
        let paused = match paused {
            0 => false,
            1 => true,
            _ => return None,
        };
        let speed = Speed::of_percent(u16::from_le_bytes([s0, s1]))?;
        let (start, end) = (MatchMs(u32::from_le_bytes([a0, a1, a2, a3])), MatchMs(u32::from_le_bytes([b0, b1, b2, b3])));
        (start <= end).then_some(Playback { revision, paused, speed, start, end })
    }
}

/// What a replay's event is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EventKind {
    /// A player killed another (or himself: actor and victim the same, or
    /// no actor).
    Kill,
    /// A battery, a point or a mode's goal.
    Objective,
}

impl EventKind {
    fn code(self) -> u8 {
        match self {
            EventKind::Kill => 1,
            EventKind::Objective => 2,
        }
    }

    fn of(code: u8) -> Option<EventKind> {
        match code {
            1 => Some(EventKind::Kill),
            2 => Some(EventKind::Objective),
            _ => None,
        }
    }
}

/// One of a replay's events (the timeline's markers, the keys' previous
/// and next kill).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Event {
    pub time: MatchMs,
    pub kind: EventKind,
    /// The places of the match it names (0xFF on the wire: none).
    pub actor: Option<u8>,
    pub victim: Option<u8>,
}

const NO_PLACE: u8 = 0xFF;
/// The bytes of one [`Event`] ([`Event::encode`]).
pub const EVENT_LEN: usize = 7;
const EVENTS_HEAD: usize = 4;

/// The most events one note carries.
pub const EVENTS_PER_NOTE: usize = 64;

impl Event {
    /// Its [`EVENT_LEN`] bytes into `out`: the time (u32 LE), the kind, the
    /// actor and the victim (0xFF: none).
    pub fn encode(&self, out: &mut [u8]) {
        out[0..4].copy_from_slice(&self.time.0.to_le_bytes());
        out[4] = self.kind.code();
        out[5] = self.actor.unwrap_or(NO_PLACE);
        out[6] = self.victim.unwrap_or(NO_PLACE);
    }

    /// An event of a kind this build knows; `None` for another kind (a newer
    /// server's), which a game skips.
    pub fn parse(bytes: &[u8]) -> Option<Event> {
        let [t0, t1, t2, t3, kind, actor, victim] = *bytes else { return None };
        let place = |place: u8| (place != NO_PLACE).then_some(place);
        Some(Event {
            time: MatchMs(u32::from_le_bytes([t0, t1, t2, t3])),
            kind: EventKind::of(kind)?,
            actor: place(actor),
            victim: place(victim),
        })
    }
}

/// The events `first..first + n` of a replay's `total`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventsNote<'a> {
    pub first: u16,
    pub total: u16,
    events: &'a [u8],
}

impl<'a> EventsNote<'a> {
    /// Each event with its index in the replay's list; `None` for one of a
    /// kind this build does not know.
    pub fn events(&self) -> impl Iterator<Item = (u16, Option<Event>)> + 'a {
        let first = self.first;
        self.events.chunks_exact(EVENT_LEN).zip(first..).map(|(bytes, index)| (index, Event::parse(bytes)))
    }

    fn parse(body: &'a [u8]) -> Option<EventsNote<'a>> {
        let (&[f0, f1, n0, n1], events) = body.split_first_chunk::<EVENTS_HEAD>()?;
        let (first, total) = (u16::from_le_bytes([f0, f1]), u16::from_le_bytes([n0, n1]));
        let count = events.len() / EVENT_LEN;
        let fits = events.len() % EVENT_LEN == 0 && count <= EVENTS_PER_NOTE
            && usize::from(first) + count <= usize::from(total);
        fits.then_some(EventsNote { first, total, events })
    }

    /// The note of `events`, the replay's `first..` of `total`, into `out`:
    /// its length, or `None` when they do not fit the note, the list or `out`.
    pub fn encode(first: u16, total: u16, events: &[Event], out: &mut [u8]) -> Option<usize> {
        let len = 2 + EVENTS_HEAD + events.len() * EVENT_LEN;
        if events.len() > EVENTS_PER_NOTE || usize::from(first) + events.len() > usize::from(total) || out.len() < len {
            return None;
        }
        out[0] = TAG;
        out[1] = EVENTS;
        out[2..4].copy_from_slice(&first.to_le_bytes());
        out[4..6].copy_from_slice(&total.to_le_bytes());
        for (event, room) in events.iter().zip(out[2 + EVENTS_HEAD..len].chunks_exact_mut(EVENT_LEN)) {
            event.encode(room);
        }
        Some(len)
    }
}

/// The most kits one [`KitsNote`] names: a player's three.
pub const KITS_PER_NOTE: usize = 3;
/// A kit's number: `0..KITS_PER_NOTE`.
pub const KIT_NUMBERS: u8 = KITS_PER_NOTE as u8;
/// The most items one kit lists.
pub const ITEMS_PER_KIT: usize = 32;
/// The longest item name a note carries.
pub const ITEM_NAME_MAX: usize = 48;

/// An item name as a note carries it: 1 to [`ITEM_NAME_MAX`] of ASCII letters, digits, `_` and `-`
/// (what makes a dictionary key, `st_<name>_name`, and nothing else).
pub fn is_item_name(name: &[u8]) -> bool {
    (1..=ITEM_NAME_MAX).contains(&name.len())
        && name.iter().all(|&b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// One kit a place used, read from a [`KitsNote`]: its number and its items' names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UsedKit<'a> {
    pub kit: u8,
    items: u8,
    names: &'a [u8],
}

impl<'a> UsedKit<'a> {
    /// The items' names, in the kit's order.
    pub fn items(&self) -> impl Iterator<Item = &'a [u8]> {
        let mut rest = self.names;
        (0..self.items).map_while(move |_| {
            let (&len, tail) = rest.split_first()?;
            let (name, after) = tail.split_at_checked(usize::from(len))?;
            rest = after;
            Some(name)
        })
    }
}

/// The kits a place of the replay used, in the order the note has them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KitsNote<'a> {
    pub place: u8,
    count: u8,
    kits: &'a [u8],
}

impl<'a> KitsNote<'a> {
    fn parse(body: &'a [u8]) -> Option<KitsNote<'a>> {
        let (&[place, count], kits) = body.split_first_chunk::<2>()?;
        if usize::from(count) > KITS_PER_NOTE {
            return None;
        }
        let note = KitsNote { place, count, kits };
        let mut rest = kits;
        for _ in 0..count {
            let (_, tail) = KitsNote::one(rest)?;
            rest = tail;
        }
        rest.is_empty().then_some(note)
    }

    /// The kit at the front of `bytes`, whole and valid, and what follows it.
    fn one(bytes: &'a [u8]) -> Option<(UsedKit<'a>, &'a [u8])> {
        let (&[kit, items], body) = bytes.split_first_chunk::<2>()?;
        if kit >= KIT_NUMBERS || usize::from(items) > ITEMS_PER_KIT {
            return None;
        }
        let mut rest = body;
        for _ in 0..items {
            let (&len, tail) = rest.split_first()?;
            let (name, after) = tail.split_at_checked(usize::from(len))?;
            if !is_item_name(name) {
                return None;
            }
            rest = after;
        }
        let used = body.len() - rest.len();
        Some((UsedKit { kit, items, names: &body[..used] }, rest))
    }

    /// Its kits, in order.
    pub fn kits(&self) -> impl Iterator<Item = UsedKit<'a>> {
        let mut rest = self.kits;
        (0..self.count).map_while(move |_| {
            let (kit, tail) = KitsNote::one(rest)?;
            rest = tail;
            Some(kit)
        })
    }

    /// The note of `kits` (a kit's number and its items' names) for `place`, into `out`: its
    /// length, or `None` when they are not valid or do not fit `out`.
    pub fn encode(place: u8, kits: &[(u8, &[&[u8]])], out: &mut [u8]) -> Option<usize> {
        if kits.len() > KITS_PER_NOTE {
            return None;
        }
        let mut len = 0;
        let mut put = |bytes: &[u8]| -> Option<()> {
            out.get_mut(len..len + bytes.len())?.copy_from_slice(bytes);
            len += bytes.len();
            Some(())
        };
        put(&[TAG, KITS, place, kits.len() as u8])?;
        for (kit, items) in kits {
            if *kit >= KIT_NUMBERS || items.len() > ITEMS_PER_KIT || !items.iter().all(|name| is_item_name(name)) {
                return None;
            }
            put(&[*kit, items.len() as u8])?;
            for name in *items {
                put(&[name.len() as u8])?;
                put(name)?;
            }
        }
        Some(len)
    }
}

/// What the server tells a replay's viewer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerNote<'a> {
    Playback(Playback),
    Events(EventsNote<'a>),
    /// The kits a place used in the replay, for the kit view.
    Kits(KitsNote<'a>),
    /// The place to follow (a roster place; the game checks it is one).
    View(u8),
}

impl<'a> ServerNote<'a> {
    /// An 0xF6's payload is one of these (its first byte [`TAG`]), not the
    /// snapshot upload's request.
    pub fn is_ours(payload: &[u8]) -> bool {
        payload.first() == Some(&TAG)
    }

    /// The note an 0xF6's payload is; `None` for anything else (another
    /// payload, a kind this build does not know, a malformed one).
    pub fn parse(payload: &'a [u8]) -> Option<ServerNote<'a>> {
        match payload {
            [TAG, PLAYBACK, body @ ..] => Playback::parse(body).map(ServerNote::Playback),
            [TAG, EVENTS, body @ ..] => EventsNote::parse(body).map(ServerNote::Events),
            [TAG, KITS, body @ ..] => KitsNote::parse(body).map(ServerNote::Kits),
            [TAG, VIEW, place] => Some(ServerNote::View(*place)),
            _ => None,
        }
    }

    /// The view note's bytes for `place`.
    pub fn view(place: u8) -> [u8; 3] {
        [TAG, VIEW, place]
    }
}

/// What a viewer asks the server for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Pause(bool),
    Speed(Speed),
    /// Play from this time on (the server starts the game again from the
    /// recording's world at or before it).
    Seek(MatchMs),
    /// The viewer is done (its controls' Back): the server lets its game
    /// go, as at the recording's end, and the game ends its world in the
    /// match's own message handling (ending it from the controls' call
    /// crashed the game).
    Leave,
}

/// A viewer's request, numbered: `seq` grows by one a request (wrapping).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request {
    pub seq: u8,
    pub op: Op,
}

impl Request {
    /// Its bytes after the message's id.
    pub const LEN: usize = 14;

    pub fn encode(self) -> [u8; Request::LEN] {
        let (code, value) = match self.op {
            Op::Pause(paused) => (1, u32::from(paused)),
            Op::Speed(speed) => (2, u32::from(speed.percent())),
            Op::Seek(time) => (3, time.0),
            Op::Leave => (4, 0),
        };
        let mut out = [0; Request::LEN];
        out[..8].copy_from_slice(&REQUEST_HEAD);
        out[8] = self.seq;
        out[9] = code;
        out[10..].copy_from_slice(&value.to_le_bytes());
        out
    }

    /// The request an 0xE0's payload is; `None` for anything else (a leave,
    /// a snapshot chunk, a malformed one).
    pub fn parse(payload: &[u8]) -> Option<Request> {
        let [h0, h1, h2, h3, h4, h5, h6, h7, seq, code, v0, v1, v2, v3] = *payload else { return None };
        if [h0, h1, h2, h3, h4, h5, h6, h7] != REQUEST_HEAD {
            return None;
        }
        let value = u32::from_le_bytes([v0, v1, v2, v3]);
        let op = match (code, value) {
            (1, 0) => Op::Pause(false),
            (1, 1) => Op::Pause(true),
            (2, percent) => Op::Speed(Speed::of_percent(u16::try_from(percent).ok()?)?),
            (3, ms) => Op::Seek(MatchMs(ms)),
            (4, 0) => Op::Leave,
            _ => return None,
        };
        Some(Request { seq, op })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_playback_goes_and_comes_back() {
        let playback = Playback { revision: 7, paused: true, speed: Speed::Quadruple, start: MatchMs(1000), end: MatchMs(600_000) };
        let bytes = playback.encode();
        assert_eq!(bytes[..6], [0x52, 1, 7, 1, 0x90, 1]);
        assert_eq!(ServerNote::parse(&bytes), Some(ServerNote::Playback(playback)));
        assert!(ServerNote::is_ours(&bytes));
    }

    #[test]
    fn a_view_goes_and_comes_back() {
        let bytes = ServerNote::view(7);
        assert_eq!(bytes, [0x52, 3, 7]);
        assert_eq!(ServerNote::parse(&bytes), Some(ServerNote::View(7)));
        assert_eq!(ServerNote::parse(&[0x52, 3]), None);
        assert_eq!(ServerNote::parse(&[0x52, 3, 7, 0]), None);
    }

    #[test]
    fn kits_go_and_come_back() {
        let first: &[&[u8]] = &[b"ak74m", b"base_medkit"];
        let third: &[&[u8]] = &[b"svd"];
        let mut out = [0; 64];
        let len = KitsNote::encode(9, &[(0, first), (2, third)], &mut out).unwrap();
        assert_eq!(out[..4], [0x52, 4, 9, 2]);
        let Some(ServerNote::Kits(note)) = ServerNote::parse(&out[..len]) else { panic!() };
        assert_eq!(note.place, 9);
        let mut kits = note.kits();
        let kit = kits.next().unwrap();
        assert_eq!(kit.kit, 0);
        let names: [&[u8]; 2] = {
            let mut it = kit.items();
            [it.next().unwrap(), it.next().unwrap()]
        };
        assert_eq!(names, [&b"ak74m"[..], &b"base_medkit"[..]]);
        let last = kits.next().unwrap();
        assert_eq!((last.kit, last.items().next(), last.items().count()), (2, Some(&b"svd"[..]), 1));
        assert!(kits.next().is_none());
    }

    #[test]
    fn no_kits_is_a_note() {
        let mut out = [0; 8];
        let len = KitsNote::encode(1, &[], &mut out).unwrap();
        let Some(ServerNote::Kits(note)) = ServerNote::parse(&out[..len]) else { panic!() };
        assert_eq!(note.kits().count(), 0);
    }

    #[test]
    fn a_wrong_kits_note_is_refused() {
        let items: &[&[u8]] = &[b"pm"];
        let mut out = [0; 32];
        let len = KitsNote::encode(1, &[(1, items)], &mut out).unwrap();
        let good = out;
        let reject = |bytes: &[u8]| assert_eq!(ServerNote::parse(bytes), None, "{bytes:?}");
        // Cut short, a byte more, four kits, kit number 3, more items than it has, a name's length past the end.
        reject(&good[..len - 1]);
        reject(&[&good[..len], &[0][..]].concat());
        reject(&[0x52, 4, 1, 4]);
        let mut wrong = good;
        wrong[4] = 3;
        reject(&wrong[..len]);
        let mut wrong = good;
        wrong[5] = 2;
        reject(&wrong[..len]);
        let mut wrong = good;
        wrong[6] = 9;
        reject(&wrong[..len]);
        // A name that is no name: empty, a space, too long.
        for bad in [&b""[..], b"a b", &[b'x'; ITEM_NAME_MAX + 1]] {
            let names: &[&[u8]] = &[bad];
            assert_eq!(KitsNote::encode(0, &[(0, names)], &mut [0; 128]), None, "{bad:?}");
        }
        assert!(!is_item_name(b"st_a-b\0"));
        // Encoding refuses what the parser would.
        assert_eq!(KitsNote::encode(0, &[(3, items)], &mut out), None);
        assert_eq!(KitsNote::encode(0, &[(0, items); 4], &mut out), None);
        assert_eq!(KitsNote::encode(0, &[(0, items)], &mut [0; 3]), None);
    }

    #[test]
    fn a_wrong_playback_is_refused() {
        let good = Playback { revision: 0, paused: false, speed: Speed::Normal, start: MatchMs(5), end: MatchMs(9) }.encode();
        let mut paused2 = good;
        paused2[3] = 2;
        let mut speed3 = good;
        speed3[4] = 3;
        let backwards = Playback { revision: 0, paused: false, speed: Speed::Normal, start: MatchMs(9), end: MatchMs(5) }.encode();
        for bad in [&paused2[..], &speed3[..], &backwards[..], &good[..13], &[0x52, 1][..]] {
            assert_eq!(ServerNote::parse(bad), None, "{bad:?}");
        }
        // The snapshot upload's request is not ours.
        assert!(!ServerNote::is_ours(&[0]));
        assert_eq!(ServerNote::parse(&[0]), None);
    }

    #[test]
    fn events_go_in_notes_of_their_place_in_the_list() {
        let kill = Event { time: MatchMs(65_000), kind: EventKind::Kill, actor: Some(3), victim: Some(11) };
        let flag = Event { time: MatchMs(70_000), kind: EventKind::Objective, actor: None, victim: None };
        let mut out = [0; 64];
        let len = EventsNote::encode(5, 7, &[kill, flag], &mut out).unwrap();
        assert_eq!(len, 6 + 14);
        let Some(ServerNote::Events(note)) = ServerNote::parse(&out[..len]) else { panic!() };
        assert_eq!((note.first, note.total), (5, 7));
        let got: [(u16, Option<Event>); 2] = {
            let mut it = note.events();
            [it.next().unwrap(), it.next().unwrap()]
        };
        assert_eq!(got, [(5, Some(kill)), (6, Some(flag))]);
        // Past the list, or a broken event, is refused whole.
        assert_eq!(EventsNote::encode(6, 7, &[kill, flag], &mut out), None);
        assert_eq!(ServerNote::parse(&out[..len - 1]), None);
    }

    #[test]
    fn an_unknown_kind_is_skipped_not_the_note() {
        let mut out = [0; 32];
        let len = EventsNote::encode(0, 2, &[Event { time: MatchMs(1), kind: EventKind::Kill, actor: None, victim: None }; 2], &mut out).unwrap();
        out[6 + 4] = 9;
        let Some(ServerNote::Events(note)) = ServerNote::parse(&out[..len]) else { panic!() };
        let kinds: [bool; 2] = {
            let mut it = note.events().map(|(_, event)| event.is_some());
            [it.next().unwrap(), it.next().unwrap()]
        };
        assert_eq!(kinds, [false, true]);
    }

    #[test]
    fn requests_go_and_come_back() {
        for op in [Op::Pause(true), Op::Pause(false), Op::Speed(Speed::Quarter), Op::Speed(Speed::Octuple), Op::Seek(MatchMs(123_456)), Op::Leave] {
            let request = Request { seq: 250, op };
            let bytes = request.encode();
            assert_eq!(bytes.len(), Request::LEN);
            assert_eq!(Request::parse(&bytes), Some(request));
        }
        let with = |tail: [u8; 6]| {
            let mut out = [0; Request::LEN];
            out[..8].copy_from_slice(&REQUEST_HEAD);
            out[8..].copy_from_slice(&tail);
            out
        };
        assert_eq!(Request { seq: 1, op: Op::Seek(MatchMs(0x01020304)) }.encode(), with([1, 3, 4, 3, 2, 1]));
        // Other ops and values, a byte more or less, a leave (empty), the old 8-byte form: no request.
        for bad in [with([1, 1, 2, 0, 0, 0]), with([1, 2, 30, 0, 0, 0]), with([1, 4, 1, 0, 0, 0]), with([1, 5, 0, 0, 0, 0])] {
            assert_eq!(Request::parse(&bad), None, "{bad:?}");
        }
        let good = with([1, 1, 1, 0, 0, 0]);
        for bad in [&good[..13], &[][..], &[0x52, 1, 1, 0, 1, 0, 0, 0][..]] {
            assert_eq!(Request::parse(bad), None, "{bad:?}");
        }
        let mut wrong = good;
        wrong[7] = 1;
        assert_eq!(Request::parse(&wrong), None);
    }

    #[test]
    fn newer_wraps() {
        assert!(newer(1, 0));
        assert!(newer(0, 255));
        assert!(newer(100, 0));
        assert!(!newer(0, 0));
        assert!(!newer(0, 1));
        assert!(!newer(200, 0));
    }

    #[test]
    fn speeds_step_and_round() {
        assert_eq!(Speed::Normal.faster(), Speed::Double);
        assert_eq!(Speed::Octuple.faster(), Speed::Octuple);
        assert_eq!(Speed::Half.slower(), Speed::Quarter);
        assert_eq!(Speed::Quarter.slower(), Speed::Quarter);
        assert_eq!(Speed::nearest(1.0), Speed::Normal);
        assert_eq!(Speed::nearest(1.3), Speed::Normal);
        assert_eq!(Speed::nearest(1.5), Speed::Double);
        assert_eq!(Speed::nearest(0.01), Speed::Quarter);
        assert_eq!(Speed::nearest(10.0), Speed::Octuple);
        assert_eq!(Speed::nearest(f32::NAN), Speed::Normal);
        assert_eq!(Speed::nearest(-2.0), Speed::Normal);
        for speed in Speed::ALL {
            assert_eq!(Speed::of_percent(speed.percent()), Some(speed));
        }
    }

    #[test]
    fn a_time_is_held_within_the_replay() {
        assert_eq!(MatchMs(5).minus(10).within(MatchMs(2), MatchMs(9)), MatchMs(2));
        assert_eq!(MatchMs(5).plus(10).within(MatchMs(2), MatchMs(9)), MatchMs(9));
        assert_eq!(MatchMs(u32::MAX).plus(1), MatchMs(u32::MAX));
    }
}
