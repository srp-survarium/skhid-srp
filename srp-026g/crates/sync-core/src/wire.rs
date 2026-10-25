//! The per-tick protocol's two messages (`netcode_port_069.md` 5.1, 5.2):
//! 0.69's layouts (`netcode_069.md` 2.3, 2.4) re-made for 0.26g's inputs,
//! one layout for both sides.
//!
//! - [`ClientUpdate`] (C->S 0xE2): every tick, empty or not, a client says
//!   which inputs of its own player it made in the tick that ends at `tick`
//!   (each as ms back from the tick, its precise time) and which kit it
//!   picked at the tick. An empty one says "nothing changed up to here".
//! - [`ServerUpdate`] (S->C 0xF7): the server closes a tick U and sends each
//!   client one update: the places entering and leaving at U, the other
//!   places' inputs and kit choices at U, what became of the receiver's own
//!   updates ([`OwnOutcome`]), and how early its latest updates came
//!   ([`ServerUpdate::arrival`], for its clock).
//!
//! Every time is a `u32` of ms, not 0.26g's one-byte ticks: no reach to
//! fall out of, whatever the tick.
//!
//! Parse, don't validate: [`ClientUpdate::parse`] and [`ServerUpdate::parse`]
//! check every count, place, time and length once against the match's
//! [`Grid`] and give typed values; nothing reads the bytes again. The
//! client side parses a server update into a view over the payload
//! ([`ServerUpdateView`]) and never allocates.
//!
//! Byte order: little-endian throughout, as the game's own writers.

use crate::time::{GameTime, Grid};

/// The layout's version: the first byte of both messages.
pub const VERSION: u8 = 1;

/// The places a match has (`player_profile[20]`).
pub const MAX_PLACES: usize = 20;

/// The most inputs of its own player a client puts in one update: its
/// player's nodes in one tick (the grid node, and a precise one for each
/// change of the precise bit in it).
pub const MAX_OWN_INPUTS: usize = 8;

/// The most kit choices in one update (the game keeps one a tick).
pub const MAX_OWN_PROFILES: usize = 2;

/// The most inputs of all places in one server update.
pub const MAX_INPUTS: usize = MAX_OWN_INPUTS * MAX_PLACES;

/// The most outcomes of one client's own updates in one server update.
pub const MAX_OUTCOMES: usize = 32;

/// A client update's size at most.
pub const CLIENT_UPDATE_MAX: usize =
    CLIENT_HEAD + MAX_OWN_INPUTS * INPUT_LEN + MAX_OWN_PROFILES;

/// A server update's size at most.
pub const SERVER_UPDATE_MAX: usize = SERVER_HEAD
    + 2
    + MAX_INPUTS * (1 + INPUT_LEN)
    + MAX_PLACES * MAX_OWN_PROFILES * 2
    + MAX_OUTCOMES * OUTCOME_MAX;

/// version, tick, inputs, profiles.
const CLIENT_HEAD: usize = 1 + 4 + 1 + 1;
/// version, tick, entered, left, arrival tag, inputs, profiles, outcomes
/// (the arrival's i16 when its tag is 1 comes after its tag).
const SERVER_HEAD: usize = 1 + 4 + 4 + 4 + 1 + 1 + 1 + 1;
/// back, rotation, actions.
const INPUT_LEN: usize = 1 + 4 + 4;
/// update, kind, the time moved to.
const OUTCOME_MAX: usize = 4 + 1 + 4;

/// Why a message was not taken, or could not be written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireError {
    /// Another layout version.
    Version(u8),
    /// Shorter than its counts say, or longer.
    Length,
    /// A count over its limit.
    Count,
    /// A time that is not a grid point where one must be.
    OffGrid(u32),
    /// An input's ms back from its tick not within the tick.
    Back(u8),
    /// A place outside the roster's 20.
    Place(u8),
    /// An outcome's kind byte, or a move that is not forward.
    Outcome,
    /// The buffer to write into is too short.
    Buffer,
}

/// A roster place, under [`MAX_PLACES`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Place(u8);

impl Place {
    pub const fn new(index: u8) -> Option<Place> {
        if (index as usize) < MAX_PLACES {
            Some(Place(index))
        } else {
            None
        }
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

/// A set of places (a 0xE9/0xEA of each, at one tick).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Places(u32);

impl Places {
    pub const EMPTY: Places = Places(0);

    #[must_use]
    pub const fn with(self, place: Place) -> Places {
        Places(self.0 | 1 << place.0)
    }

    pub const fn contains(self, place: Place) -> bool {
        self.0 & 1 << place.0 != 0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    /// The set of these bits; `None` with a bit past the 20 places.
    pub const fn from_bits(bits: u32) -> Option<Places> {
        if bits >> MAX_PLACES == 0 {
            Some(Places(bits))
        } else {
            None
        }
    }

    /// The places, ascending.
    pub fn iter(self) -> impl Iterator<Item = Place> {
        (0..MAX_PLACES as u8).filter_map(move |index| {
            let place = Place(index);
            self.contains(place).then_some(place)
        })
    }
}

/// A player input (`survarium::player_input::serialize` 0x3184A0): when in
/// its tick, the mouse's turn and the keys.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Input {
    /// Its time is the tick less this many ms (0: on the grid; else a
    /// precise input, its node flagged +24h).
    pub back: u8,
    /// `math::half2(rotation)`: two IEEE half floats, as they came; never
    /// decoded (re-encoding one would change the simulation).
    pub rotation: [u8; 4],
    /// `actions_mask`.
    pub actions: u32,
}

/// The action bits no stock client sets: 29 and 30 (the input builder
/// 0x241250 sets 0..23 and 25..28, patch 34 adds 24; bit 31 is the
/// per-input flag the reader 0x318400 splits off). An input with one is no
/// game's (srp-validate's `UNUSED_ACTIONS`).
pub const UNUSED_ACTIONS: u32 = 1 << 29 | 1 << 30;

/// A half float's exponent field, and that of 1024.0.
const HALF_EXPONENT: u16 = 0x7C00;
const HALF_EXPONENT_1024: u16 = 25 << 10;

impl Input {
    /// Its time in the tick ending at `tick`.
    pub const fn time(self, tick: GameTime) -> GameTime {
        GameTime(tick.0 - self.back as u32)
    }

    /// Whether a game makes it: its rotation's two halves numbers under
    /// 1024 (a sum of them stays a number: match.md section 20) and no
    /// action bit no game sets. The server takes no other (the update is
    /// discarded) and a game applies no other (the same test on every
    /// game: every one leaves it out alike).
    pub fn is_sane(&self) -> bool {
        let [a, b, c, d] = self.rotation;
        let halves = [u16::from_le_bytes([a, b]), u16::from_le_bytes([c, d])];
        halves
            .iter()
            .all(|half| half & HALF_EXPONENT < HALF_EXPONENT_1024)
            && self.actions & UNUSED_ACTIONS == 0
    }

    fn write(self, out: &mut Writer<'_>) -> Result<(), WireError> {
        out.u8(self.back)?;
        out.bytes(&self.rotation)?;
        out.u32(self.actions)
    }

    fn read(input: &mut Reader<'_>, grid: Grid) -> Result<Input, WireError> {
        let back = input.u8()?;
        if u32::from(back) >= grid.tick_ms() {
            return Err(WireError::Back(back));
        }
        Ok(Input {
            back,
            rotation: input.array()?,
            actions: input.u32()?,
        })
    }
}

/// An input of a place, in a server update.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlaceInput {
    pub place: Place,
    pub input: Input,
}

/// A kit choice of a place at the tick (0xDE's value), in a server update.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlaceProfile {
    pub place: Place,
    pub value: u8,
}

/// What became of one of the receiver's own updates (0.69's
/// `server_input_confirmation`, made explicit: 0.26g's 0xEF move and 0xF1
/// discard folded in). Only an update with inputs has one, exactly one,
/// and it is final.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Outcome {
    /// Its inputs are at their own times for every game.
    Confirmed,
    /// Its inputs are at `to` less their ms back for every game: it came
    /// after its tick was closed, and went to a tick still open. Always
    /// later than the update's tick.
    Moved { to: GameTime },
    /// No game has its inputs: it came too late to move, or a later update
    /// of the place made it stale.
    Discarded,
}

/// An [`Outcome`] of the receiver's update of the tick `update`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OwnOutcome {
    pub update: GameTime,
    pub outcome: Outcome,
}

impl OwnOutcome {
    fn write(self, out: &mut Writer<'_>) -> Result<(), WireError> {
        out.u32(self.update.0)?;
        match self.outcome {
            Outcome::Confirmed => out.u8(0),
            Outcome::Moved { to } => {
                out.u8(1)?;
                out.u32(to.0)
            }
            Outcome::Discarded => out.u8(2),
        }
    }

    fn read(input: &mut Reader<'_>, grid: Grid) -> Result<OwnOutcome, WireError> {
        let update = input.tick(grid)?;
        let outcome = match input.u8()? {
            0 => Outcome::Confirmed,
            1 => {
                let to = input.tick(grid)?;
                if to <= update {
                    return Err(WireError::Outcome);
                }
                Outcome::Moved { to }
            }
            2 => Outcome::Discarded,
            _ => return Err(WireError::Outcome),
        };
        Ok(OwnOutcome { update, outcome })
    }
}

/// C->S: a client's own inputs of the tick ending at `tick`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClientUpdate {
    /// A grid point: the update holds the own inputs of `(tick - tick
    /// length, tick]`.
    pub tick: GameTime,
    inputs: [Input; MAX_OWN_INPUTS],
    input_count: u8,
    profiles: [u8; MAX_OWN_PROFILES],
    profile_count: u8,
}

impl ClientUpdate {
    /// An update of the tick `tick` with nothing in it.
    pub const fn empty(tick: GameTime) -> ClientUpdate {
        ClientUpdate {
            tick,
            inputs: [Input {
                back: 0,
                rotation: [0; 4],
                actions: 0,
            }; MAX_OWN_INPUTS],
            input_count: 0,
            profiles: [0; MAX_OWN_PROFILES],
            profile_count: 0,
        }
    }

    /// Adds an input; `Err` when it is full.
    pub fn push_input(&mut self, input: Input) -> Result<(), WireError> {
        let slot = self
            .inputs
            .get_mut(usize::from(self.input_count))
            .ok_or(WireError::Count)?;
        *slot = input;
        self.input_count += 1;
        Ok(())
    }

    /// Adds a kit choice; `Err` when it is full.
    pub fn push_profile(&mut self, value: u8) -> Result<(), WireError> {
        let slot = self
            .profiles
            .get_mut(usize::from(self.profile_count))
            .ok_or(WireError::Count)?;
        *slot = value;
        self.profile_count += 1;
        Ok(())
    }

    pub fn inputs(&self) -> &[Input] {
        &self.inputs[..usize::from(self.input_count)]
    }

    pub fn profiles(&self) -> &[u8] {
        &self.profiles[..usize::from(self.profile_count)]
    }

    /// Whether it carries anything for the others (then it gets an
    /// [`OwnOutcome`]).
    pub fn has_inputs(&self) -> bool {
        self.input_count > 0 || self.profile_count > 0
    }

    /// Its bytes on the wire.
    pub fn wire_len(&self) -> usize {
        CLIENT_HEAD + self.inputs().len() * INPUT_LEN + self.profiles().len()
    }

    /// Writes it at the start of `out`: how many bytes.
    pub fn write(&self, out: &mut [u8]) -> Result<usize, WireError> {
        let mut out = Writer::new(out);
        out.u8(VERSION)?;
        out.u32(self.tick.0)?;
        out.u8(self.input_count)?;
        out.u8(self.profile_count)?;
        for input in self.inputs() {
            input.write(&mut out)?;
        }
        out.bytes(self.profiles())?;
        Ok(out.at)
    }

    /// Reads one: the whole payload.
    pub fn parse(bytes: &[u8], grid: Grid) -> Result<ClientUpdate, WireError> {
        let mut input = Reader::new(bytes);
        let version = input.u8()?;
        if version != VERSION {
            return Err(WireError::Version(version));
        }
        let mut update = ClientUpdate::empty(input.tick(grid)?);
        let inputs = input.u8()?;
        let profiles = input.u8()?;
        if usize::from(inputs) > MAX_OWN_INPUTS || usize::from(profiles) > MAX_OWN_PROFILES {
            return Err(WireError::Count);
        }
        for _ in 0..inputs {
            update.push_input(Input::read(&mut input, grid)?)?;
        }
        for _ in 0..profiles {
            update.push_profile(input.u8()?)?;
        }
        input.end()?;
        Ok(update)
    }
}

/// How early the receiver's updates came: the least `tick - match clock`
/// (ms) of those the server had since its update before this one, or none
/// if none came. Negative: late.
pub type Arrival = Option<i16>;

/// S->C: the tick `tick` closed, for one receiver. The server writes one
/// from its parts; a client reads one as a [`ServerUpdateView`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServerUpdate<'a> {
    /// A grid point: everything in it is at this tick.
    pub tick: GameTime,
    pub entered: Places,
    pub left: Places,
    pub arrival: Arrival,
    pub inputs: &'a [PlaceInput],
    pub profiles: &'a [PlaceProfile],
    pub outcomes: &'a [OwnOutcome],
}

impl ServerUpdate<'_> {
    pub fn wire_len(&self) -> usize {
        SERVER_HEAD
            + if self.arrival.is_some() { 2 } else { 0 }
            + self.inputs.len() * (1 + INPUT_LEN)
            + self.profiles.len() * 2
            + self
                .outcomes
                .iter()
                .map(|own| match own.outcome {
                    Outcome::Moved { .. } => OUTCOME_MAX,
                    _ => OUTCOME_MAX - 4,
                })
                .sum::<usize>()
    }

    /// Whether it says nothing but that the tick closed.
    pub fn is_empty(&self) -> bool {
        self.entered.is_empty()
            && self.left.is_empty()
            && self.inputs.is_empty()
            && self.profiles.is_empty()
            && self.outcomes.is_empty()
    }

    /// Writes it at the start of `out`: how many bytes.
    pub fn write(&self, out: &mut [u8]) -> Result<usize, WireError> {
        if self.inputs.len() > MAX_INPUTS
            || self.profiles.len() > MAX_PLACES * MAX_OWN_PROFILES
            || self.outcomes.len() > MAX_OUTCOMES
        {
            return Err(WireError::Count);
        }
        let mut out = Writer::new(out);
        out.u8(VERSION)?;
        out.u32(self.tick.0)?;
        out.u32(self.entered.bits())?;
        out.u32(self.left.bits())?;
        match self.arrival {
            None => out.u8(0)?,
            Some(early) => {
                out.u8(1)?;
                out.bytes(&early.to_le_bytes())?;
            }
        }
        out.u8(self.inputs.len() as u8)?;
        out.u8(self.profiles.len() as u8)?;
        out.u8(self.outcomes.len() as u8)?;
        for each in self.inputs {
            out.u8(each.place.get())?;
            each.input.write(&mut out)?;
        }
        for each in self.profiles {
            out.u8(each.place.get())?;
            out.u8(each.value)?;
        }
        for each in self.outcomes {
            each.write(&mut out)?;
        }
        Ok(out.at)
    }

    /// Reads one, checked whole: a view over `bytes`.
    pub fn parse(bytes: &[u8], grid: Grid) -> Result<ServerUpdateView<'_>, WireError> {
        let mut input = Reader::new(bytes);
        let version = input.u8()?;
        if version != VERSION {
            return Err(WireError::Version(version));
        }
        let tick = input.tick(grid)?;
        let entered = Places::from_bits(input.u32()?).ok_or(WireError::Place(0xFF))?;
        let left = Places::from_bits(input.u32()?).ok_or(WireError::Place(0xFF))?;
        let arrival = match input.u8()? {
            0 => None,
            1 => Some(i16::from_le_bytes(input.array()?)),
            _ => return Err(WireError::Length),
        };
        let inputs = input.u8()?;
        let profiles = input.u8()?;
        let outcomes = input.u8()?;
        if usize::from(inputs) > MAX_INPUTS
            || usize::from(profiles) > MAX_PLACES * MAX_OWN_PROFILES
            || usize::from(outcomes) > MAX_OUTCOMES
        {
            return Err(WireError::Count);
        }
        let records = input.at;
        for _ in 0..inputs {
            input.place()?;
            Input::read(&mut input, grid)?;
        }
        let profiles_at = input.at;
        for _ in 0..profiles {
            input.place()?;
            input.u8()?;
        }
        let outcomes_at = input.at;
        for _ in 0..outcomes {
            OwnOutcome::read(&mut input, grid)?;
        }
        input.end()?;
        Ok(ServerUpdateView {
            tick,
            entered,
            left,
            arrival,
            grid,
            inputs: (&bytes[records..profiles_at], inputs),
            profiles: (&bytes[profiles_at..outcomes_at], profiles),
            outcomes: (&bytes[outcomes_at..], outcomes),
        })
    }
}

/// A server update as a client reads it: its head, and its records over
/// the payload, each already checked by [`ServerUpdate::parse`].
#[derive(Clone, Copy, Debug)]
pub struct ServerUpdateView<'a> {
    pub tick: GameTime,
    pub entered: Places,
    pub left: Places,
    pub arrival: Arrival,
    grid: Grid,
    inputs: (&'a [u8], u8),
    profiles: (&'a [u8], u8),
    outcomes: (&'a [u8], u8),
}

impl<'a> ServerUpdateView<'a> {
    /// The other places' inputs at the tick.
    pub fn inputs(&self) -> impl Iterator<Item = PlaceInput> + 'a {
        let grid = self.grid;
        let mut input = Reader::new(self.inputs.0);
        (0..self.inputs.1).filter_map(move |_| {
            let place = input.place().ok()?;
            Some(PlaceInput {
                place,
                input: Input::read(&mut input, grid).ok()?,
            })
        })
    }

    /// The other places' kit choices at the tick.
    pub fn profiles(&self) -> impl Iterator<Item = PlaceProfile> + 'a {
        let mut input = Reader::new(self.profiles.0);
        (0..self.profiles.1).filter_map(move |_| {
            Some(PlaceProfile {
                place: input.place().ok()?,
                value: input.u8().ok()?,
            })
        })
    }

    /// What became of the receiver's own updates.
    pub fn outcomes(&self) -> impl Iterator<Item = OwnOutcome> + 'a {
        let grid = self.grid;
        let mut input = Reader::new(self.outcomes.0);
        (0..self.outcomes.1).filter_map(move |_| OwnOutcome::read(&mut input, grid).ok())
    }
}

struct Writer<'a> {
    out: &'a mut [u8],
    at: usize,
}

impl<'a> Writer<'a> {
    fn new(out: &'a mut [u8]) -> Self {
        Self { out, at: 0 }
    }

    fn bytes(&mut self, bytes: &[u8]) -> Result<(), WireError> {
        let end = self.at + bytes.len();
        let slot = self.out.get_mut(self.at..end).ok_or(WireError::Buffer)?;
        for (to, from) in slot.iter_mut().zip(bytes) {
            *to = *from;
        }
        self.at = end;
        Ok(())
    }

    fn u8(&mut self, value: u8) -> Result<(), WireError> {
        self.bytes(&[value])
    }

    fn u32(&mut self, value: u32) -> Result<(), WireError> {
        self.bytes(&value.to_le_bytes())
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], WireError> {
        let mut out = [0; N];
        let from = self
            .bytes
            .get(self.at..self.at + N)
            .ok_or(WireError::Length)?;
        for (to, byte) in out.iter_mut().zip(from) {
            *to = *byte;
        }
        self.at += N;
        Ok(out)
    }

    fn u8(&mut self) -> Result<u8, WireError> {
        Ok(self.array::<1>()?[0])
    }

    fn u32(&mut self) -> Result<u32, WireError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn tick(&mut self, grid: Grid) -> Result<GameTime, WireError> {
        let time = GameTime(self.u32()?);
        if grid.on_grid(time) {
            Ok(time)
        } else {
            Err(WireError::OffGrid(time.0))
        }
    }

    fn place(&mut self) -> Result<Place, WireError> {
        let index = self.u8()?;
        Place::new(index).ok_or(WireError::Place(index))
    }

    fn end(&self) -> Result<(), WireError> {
        if self.at == self.bytes.len() {
            Ok(())
        } else {
            Err(WireError::Length)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::TickLen;

    const GRID: Grid = Grid::STOCK;

    fn input(back: u8, actions: u32) -> Input {
        Input {
            back,
            rotation: [0x00, 0x3C, 0x00, 0xC0],
            actions,
        }
    }

    fn place(index: u8) -> Place {
        Place::new(index).unwrap()
    }

    #[test]
    fn a_client_update_round_trips_byte_for_byte() {
        let mut update = ClientUpdate::empty(GameTime(1050));
        update.push_input(input(17, 0x20)).unwrap();
        update.push_input(input(0, 0x21)).unwrap();
        update.push_profile(3).unwrap();
        let mut bytes = [0; CLIENT_UPDATE_MAX];
        let len = update.write(&mut bytes).unwrap();
        assert_eq!(len, update.wire_len());
        assert_eq!(
            &bytes[..len],
            [
                1, 0x1A, 0x04, 0, 0, 2, 1, // version, 1050, 2 inputs, 1 kit
                17, 0x00, 0x3C, 0x00, 0xC0, 0x20, 0, 0, 0, // back 17
                0, 0x00, 0x3C, 0x00, 0xC0, 0x21, 0, 0, 0, // on the grid
                3,
            ]
        );
        let parsed = ClientUpdate::parse(&bytes[..len], GRID).unwrap();
        assert_eq!(parsed, update);
        assert!(parsed.has_inputs());
        assert_eq!(parsed.inputs()[0].time(parsed.tick), GameTime(1033));
        assert!(!ClientUpdate::empty(GameTime(50)).has_inputs());
    }

    #[test]
    fn a_client_update_is_refused_whole_for_one_wrong_field() {
        let mut update = ClientUpdate::empty(GameTime(1050));
        update.push_input(input(10, 1)).unwrap();
        let mut bytes = [0; CLIENT_UPDATE_MAX];
        let len = update.write(&mut bytes).unwrap();
        let good = &bytes[..len];
        // Off the grid.
        let mut off = good.to_vec();
        off[1] = 0x1B;
        assert_eq!(
            ClientUpdate::parse(&off, GRID),
            Err(WireError::OffGrid(1051))
        );
        // A back of a whole tick.
        let mut back = good.to_vec();
        back[7] = 50;
        assert_eq!(ClientUpdate::parse(&back, GRID), Err(WireError::Back(50)));
        // A back of 30 is in a 50 ms tick, not in a 25 ms one.
        let mut thirty = good.to_vec();
        thirty[7] = 30;
        assert!(ClientUpdate::parse(&thirty, GRID).is_ok());
        assert_eq!(
            ClientUpdate::parse(&thirty, Grid::of(TickLen::Ms25)),
            Err(WireError::Back(30))
        );
        // Short, long, another version, too many.
        assert_eq!(
            ClientUpdate::parse(&good[..len - 1], GRID),
            Err(WireError::Length)
        );
        let mut long = good.to_vec();
        long.push(0);
        assert_eq!(ClientUpdate::parse(&long, GRID), Err(WireError::Length));
        let mut version = good.to_vec();
        version[0] = 2;
        assert_eq!(
            ClientUpdate::parse(&version, GRID),
            Err(WireError::Version(2))
        );
        let mut many = good.to_vec();
        many[5] = 9;
        assert_eq!(ClientUpdate::parse(&many, GRID), Err(WireError::Count));
        // Full.
        let mut full = ClientUpdate::empty(GameTime(50));
        for _ in 0..MAX_OWN_INPUTS {
            full.push_input(input(0, 0)).unwrap();
        }
        assert_eq!(full.push_input(input(0, 0)), Err(WireError::Count));
        assert_eq!(
            full.write(&mut [0; 8]),
            Err(WireError::Buffer),
            "a buffer too short is an error, never a cut message"
        );
    }

    #[test]
    fn a_server_update_round_trips_and_reads_as_a_view() {
        let inputs = [
            PlaceInput {
                place: place(3),
                input: input(0, 7),
            },
            PlaceInput {
                place: place(19),
                input: input(12, 8),
            },
        ];
        let profiles = [PlaceProfile {
            place: place(4),
            value: 2,
        }];
        let outcomes = [
            OwnOutcome {
                update: GameTime(950),
                outcome: Outcome::Moved {
                    to: GameTime(1100),
                },
            },
            OwnOutcome {
                update: GameTime(1000),
                outcome: Outcome::Discarded,
            },
            OwnOutcome {
                update: GameTime(1100),
                outcome: Outcome::Confirmed,
            },
        ];
        let update = ServerUpdate {
            tick: GameTime(1100),
            entered: Places::EMPTY.with(place(5)),
            left: Places::EMPTY.with(place(0)).with(place(19)),
            arrival: Some(-7),
            inputs: &inputs,
            profiles: &profiles,
            outcomes: &outcomes,
        };
        let mut bytes = [0; SERVER_UPDATE_MAX];
        let len = update.write(&mut bytes).unwrap();
        assert_eq!(len, update.wire_len());
        let view = ServerUpdate::parse(&bytes[..len], GRID).unwrap();
        assert_eq!(view.tick, GameTime(1100));
        assert_eq!(view.entered.iter().collect::<std::vec::Vec<_>>(), [place(5)]);
        assert_eq!(
            view.left.iter().collect::<std::vec::Vec<_>>(),
            [place(0), place(19)]
        );
        assert_eq!(view.arrival, Some(-7));
        assert_eq!(view.inputs().collect::<std::vec::Vec<_>>(), inputs);
        assert_eq!(view.profiles().collect::<std::vec::Vec<_>>(), profiles);
        assert_eq!(view.outcomes().collect::<std::vec::Vec<_>>(), outcomes);
        // The empty one: the tick closed, nothing else.
        let empty = ServerUpdate {
            tick: GameTime(1150),
            entered: Places::EMPTY,
            left: Places::EMPTY,
            arrival: None,
            inputs: &[],
            profiles: &[],
            outcomes: &[],
        };
        assert!(empty.is_empty());
        let len = empty.write(&mut bytes).unwrap();
        assert_eq!(len, 17);
        let view = ServerUpdate::parse(&bytes[..len], GRID).unwrap();
        assert_eq!(view.inputs().count() + view.outcomes().count(), 0);
    }

    #[test]
    fn an_input_is_sane_with_number_rotations_and_used_actions() {
        assert!(input(0, 0x0FFF_FFFF).is_sane());
        assert!(input(0, 1 << 31).is_sane(), "the per-input flag");
        assert!(!input(0, 1 << 29).is_sane());
        assert!(!input(0, 1 << 30).is_sane());
        let mut infinite = input(0, 0);
        infinite.rotation = [0x00, 0x7C, 0x00, 0x00];
        assert!(!infinite.is_sane());
        let mut big = input(0, 0);
        big.rotation = [0x00, 0x00, 0x00, 0x64]; // 1024.0 in the second half
        assert!(!big.is_sane());
    }

    #[test]
    fn a_server_update_is_refused_for_a_bad_place_time_or_move() {
        let outcome = |update: u32, to: u32| OwnOutcome {
            update: GameTime(update),
            outcome: Outcome::Moved { to: GameTime(to) },
        };
        let write = |outcomes: &[OwnOutcome]| {
            let mut bytes = [0; SERVER_UPDATE_MAX];
            let len = ServerUpdate {
                tick: GameTime(1100),
                entered: Places::EMPTY,
                left: Places::EMPTY,
                arrival: None,
                inputs: &[],
                profiles: &[],
                outcomes,
            }
            .write(&mut bytes)
            .unwrap();
            bytes[..len].to_vec()
        };
        // Never back in time.
        assert_eq!(
            ServerUpdate::parse(&write(&[outcome(1100, 1050)]), GRID).err(),
            Some(WireError::Outcome)
        );
        assert_eq!(
            ServerUpdate::parse(&write(&[outcome(1100, 1100)]), GRID).err(),
            Some(WireError::Outcome)
        );
        assert!(ServerUpdate::parse(&write(&[outcome(1050, 1100)]), GRID).is_ok());
        assert_eq!(
            ServerUpdate::parse(&write(&[outcome(1050, 1101)]), GRID).err(),
            Some(WireError::OffGrid(1101))
        );
        // A place past the roster in a record or a mask.
        let mut bytes = write(&[]);
        bytes[5] = 0x00;
        bytes[7] = 0x10; // entered bit 20
        assert_eq!(
            ServerUpdate::parse(&bytes, GRID).err(),
            Some(WireError::Place(0xFF))
        );
        assert_eq!(Place::new(20), None);
    }
}
