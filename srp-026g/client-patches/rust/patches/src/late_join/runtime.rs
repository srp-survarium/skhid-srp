//! The proposals' lifecycle: what the late join keeps between the server's
//! 0xE4 for a newcomer, the players' query and their destruction (before
//! the Rust port `late_join_runtime_026g.c`; client_patch.md section
//! 3.11). Pure: no game memory, only keys (game addresses as values, never
//! read through) and the storage the caller allocated; the caller
//! (late_join.rs) makes the game's calls between the steps, never inside one
//! (a step runs under the late join's lock, and the game may call back: a
//! query can complete at once, a world's release can destroy players).
//!
//! One activation is the match a client plays (`ready`); every proposal
//! belongs to the activation it came in, and is *current* while that one is
//! active. A current proposal answers its server's retransmissions and pins
//! its place; a proposal is freed once it is no longer current, its query has
//! come back and none of the players it loaded lives (their profiles point
//! into its storage).

use core::num::NonZeroU32;

/// The most proposals kept at once: a match's nineteen places, for three
/// matches whose players have not all gone yet. One more is answered busy
/// (the server asks again), as an allocation that fails is.
pub const PROPOSALS: usize = 64;
/// The places of a match (patch 11's fixed capacity).
pub const PLACES: u8 = 20;
/// A place's kits, a player each.
pub const KITS: usize = 3;
/// The most bytes of a proposal's profile (its 14-byte header aside).
pub const MAX_PROFILE: usize = 32768;
/// The proposal's header: magic, seed, serial, place, squad.
pub const HEADER: usize = 14;
const MAGIC: [u8; 4] = [0xFF, b'L', b'J', 1];

/// The match's seed (options+577B0h): what names it in every message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seed(pub u32);

/// A proposal's serial, never 0: the server numbers its tries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Serial(NonZeroU32);

impl Serial {
    pub fn parse(n: u32) -> Option<Serial> {
        NonZeroU32::new(n).map(Serial)
    }

    pub fn get(self) -> u32 {
        self.0.get()
    }
}

/// A newcomer's place: 1..19 (place 0 is always the first player's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place(u8);

impl Place {
    pub fn parse(n: u8) -> Option<Place> {
        (1..PLACES).contains(&n).then_some(Place(n))
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

/// The squad byte the server gives a newcomer (options+5782Ch + place).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Squad(pub u8);

/// The places taken (the options' count, +577B5h): the next newcomer's
/// place, when it is one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Roster(pub u8);

impl Roster {
    /// Whether `place` is the next one: a newcomer takes the roster's end.
    pub fn takes(self, place: Place) -> bool {
        self.0 == place.0
    }
}

/// The kits of a place whose players a proposal asks for: its profile's
/// usable kits, at least one of the three.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KitMask(u8);

impl KitMask {
    pub fn parse(bits: u8) -> Option<KitMask> {
        (bits != 0 && bits < 1 << KITS).then_some(KitMask(bits))
    }

    pub fn has(self, kit: usize) -> bool {
        kit < KITS && self.0 & 1 << kit != 0
    }

    pub fn bits(self) -> u8 {
        self.0
    }

    /// The kits asked for, in order.
    pub fn kits(self) -> impl Iterator<Item = usize> {
        (0..KITS).filter(move |&kit| self.has(kit))
    }

    /// The players, when there is one for each kit asked for and none for
    /// another: the query loaded all of them.
    pub fn matched<P: Copy>(self, players: [Option<P>; KITS]) -> Option<KitPlayers<P>> {
        (0..KITS).all(|kit| players[kit].is_some() == self.has(kit)).then_some(KitPlayers { mask: self, players })
    }
}

/// A place's players, one for each kit of its mask and none for another.
#[derive(Clone, Copy, Debug)]
pub struct KitPlayers<P> {
    mask: KitMask,
    players: [Option<P>; KITS],
}

impl<P: Copy> KitPlayers<P> {
    pub fn mask(&self) -> KitMask {
        self.mask
    }

    /// (kit, its player) for each kit of the mask, in order.
    pub fn each(&self) -> impl Iterator<Item = (usize, P)> + '_ {
        self.mask.kits().filter_map(|kit| self.players[kit].map(|player| (kit, player)))
    }

    pub fn map<Q: Copy>(&self, f: impl Fn(P) -> Q) -> KitPlayers<Q> {
        KitPlayers { mask: self.mask, players: self.players.map(|p| p.map(&f)) }
    }
}

/// A game object's address as a value (never read through): what tells
/// clients, worlds, players and profiles apart between hooks.
macro_rules! key {
    ($($(#[$doc:meta])* $name:ident;)*) => {$(
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct $name(pub u32);
    )*};
}
key! {
    /// A network client.
    ClientKey;
    /// A match's world (its pvp_match).
    WorldKey;
    /// The items' dictionary a match was activated with.
    DictionaryKey;
    /// A player a query loaded.
    PlayerKey;
    /// The profile a proposal's players were loaded from (its storage's).
    ProfileKey;
}

/// The match a client plays, as its ready names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchKey {
    pub client: ClientKey,
    pub world: WorldKey,
    pub seed: Seed,
}

/// An E4's late-join proposal, its header read: the match, the server's try,
/// the newcomer's place and squad, then its profile (unchecked here:
/// profile.rs parses it).
#[derive(Clone, Copy, Debug)]
pub struct Prepare<'a> {
    pub seed: Seed,
    pub serial: Serial,
    pub place: Place,
    pub squad: Squad,
    pub body: &'a [u8],
}

impl<'a> Prepare<'a> {
    /// The proposal `bytes` are, or none: the magic, a serial, a newcomer's
    /// place, a profile of at most MAX_PROFILE bytes.
    pub fn parse(bytes: &'a [u8]) -> Option<Prepare<'a>> {
        if bytes.len() > HEADER + MAX_PROFILE {
            return None;
        }
        let (head, body) = bytes.split_first_chunk::<HEADER>()?;
        let word = |at: usize| u32::from_le_bytes([head[at], head[at + 1], head[at + 2], head[at + 3]]);
        (head[..4] == MAGIC).then_some(())?;
        Some(Prepare {
            seed: Seed(word(4)),
            serial: Serial::parse(word(8))?,
            place: Place::parse(head[12])?,
            squad: Squad(head[13]),
            body,
        })
    }
}

/// What a proposal's client is told (0xE0's late-join answer).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    Installed,
    Failed,
    /// Not yet: the server asks again.
    Busy,
}

impl Answer {
    /// Its byte on the wire.
    pub fn wire(self) -> u8 {
        match self {
            Answer::Installed => 0,
            Answer::Failed => 1,
            Answer::Busy => 2,
        }
    }
}

/// An answer to send: to the active client, of its match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ack {
    pub seed: Seed,
    pub serial: Serial,
    pub place: Place,
    pub answer: Answer,
}

/// The players' query's end, once it is told.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Installed,
    Failed,
}

impl From<Outcome> for Answer {
    fn from(outcome: Outcome) -> Answer {
        match outcome {
            Outcome::Installed => Answer::Installed,
            Outcome::Failed => Answer::Failed,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Generation(u32);

/// The client's match while the late join may take newcomers into it.
#[derive(Clone, Copy, Debug)]
struct Active {
    generation: Generation,
    matched: MatchKey,
    dictionary: DictionaryKey,
    roster: Roster,
}

#[derive(Clone, Copy, Debug)]
enum State {
    Inactive,
    Active(Active),
}

/// Where a proposal is. Its storage is shared with its query, which holds
/// another reference until its callback is done.
enum Phase<S> {
    /// Its players' query runs.
    Loading { storage: S },
    /// Told: immutable, and pinning its place while its match is active.
    Answered { storage: S, outcome: Outcome },
    /// Its players came after its match was retired: never told, kept for
    /// the players its query loaded.
    Unanswered { storage: S },
}

struct Proposal<S> {
    generation: Generation,
    serial: Serial,
    place: Place,
    squad: Squad,
    requested: KitMask,
    profile: ProfileKey,
    /// The players its query loaded that live (their profile is its storage's).
    players: [Option<PlayerKey>; KITS],
    phase: Phase<S>,
}

impl<S> Proposal<S> {
    fn completed(&self) -> bool {
        matches!(self.phase, Phase::Answered { .. } | Phase::Unanswered { .. })
    }
}

/// What a proposal's E4 asks of the caller.
#[derive(Debug)]
pub enum Admission {
    /// Nothing: no match of the late join's, or another match's.
    Ignore,
    /// Answer it at once.
    Answer(Ack),
    /// A new proposal for the roster's next place: check its profile, load
    /// its storage and hold its world, then `admit` it.
    Load(Admitted),
}

/// A proposal the runtime would take, for `admit`.
#[derive(Clone, Copy, Debug)]
pub struct Admitted {
    generation: Generation,
    pub matched: MatchKey,
    pub dictionary: DictionaryKey,
    pub serial: Serial,
    pub place: Place,
    pub squad: Squad,
}

impl Admitted {
    /// The answer when the proposal cannot be loaded.
    pub fn refused(&self, answer: Answer) -> Ack {
        Ack { seed: self.matched.seed, serial: self.serial, place: self.place, answer }
    }
}

/// Storages no proposal needs any more: freed when this is dropped (outside
/// the lock).
pub struct Dead<S>([Option<S>; PROPOSALS]);

impl<S> Default for Dead<S> {
    fn default() -> Dead<S> {
        Dead::none()
    }
}

impl<S> Dead<S> {
    fn none() -> Dead<S> {
        Dead([const { None }; PROPOSALS])
    }

    pub fn count(&self) -> usize {
        self.0.iter().flatten().count()
    }
}

/// A query's end, for the callback.
#[derive(Clone, Copy, Debug)]
pub enum Completion {
    /// No loading proposal has the profile (never: a query comes back once).
    Unknown,
    /// The players fill the roster's next place of the active match:
    /// install them, then `answered`.
    Install { players: KitPlayers<PlayerKey>, place: Place, squad: Squad, matched: MatchKey,
              dictionary: DictionaryKey },
    /// Told failed: not every kit's player came, or the place is not the
    /// roster's next any more. `answered` with Failed.
    Refuse,
    /// Its match was retired meanwhile: nothing is told. `answered` with none.
    Stale,
}

/// After `answered`: what to free, and the answer to send to the active
/// match's client.
pub struct Answered<S> {
    pub dead: Dead<S>,
    pub ack: Option<(ClientKey, Ack)>,
}

/// The late join's proposals and the activation they belong to.
pub struct Runtime<S> {
    state: State,
    proposals: [Option<Proposal<S>>; PROPOSALS],
    generation: u32,
}

impl<S> Default for Runtime<S> {
    fn default() -> Self {
        Runtime::new()
    }
}

impl<S> Runtime<S> {
    pub const fn new() -> Runtime<S> {
        Runtime { state: State::Inactive, proposals: [const { None }; PROPOSALS], generation: 0 }
    }

    fn active(&self) -> Option<Active> {
        match self.state {
            State::Active(active) => Some(active),
            State::Inactive => None,
        }
    }

    fn current(&self, proposal: &Proposal<S>) -> bool {
        self.active().is_some_and(|a| a.generation == proposal.generation)
    }

    /// The roster's count now, for the checks.
    pub fn roster(&self) -> Option<Roster> {
        self.active().map(|a| a.roster)
    }

    /// The active match's client, or none.
    pub fn client(&self) -> Option<ClientKey> {
        self.active().map(|a| a.matched.client)
    }

    /// A match is ready to play (on_match_ready): the active one goes on if
    /// it is this client's and world's; else it retires and this one is
    /// active, the places taken `roster`.
    pub fn ready(&mut self, matched: MatchKey, dictionary: DictionaryKey, roster: Roster) -> Dead<S> {
        if self.active().is_some_and(|a| a.matched.client == matched.client && a.matched.world == matched.world) {
            return Dead::none();
        }
        let dead = self.deactivate();
        self.generation = self.generation.wrapping_add(1);
        self.state = State::Active(Active { generation: Generation(self.generation), matched, dictionary, roster });
        dead
    }

    /// No match takes newcomers (a match's first world on its way, its
    /// client destroyed): every proposal retires.
    pub fn deactivate(&mut self) -> Dead<S> {
        self.state = State::Inactive;
        self.collect()
    }

    /// `client` left its match (disconnected, destroyed): it retires, when it
    /// is the active one's.
    pub fn retire(&mut self, client: ClientKey) -> Dead<S> {
        if self.client() == Some(client) { self.deactivate() } else { Dead::none() }
    }

    /// An E4's proposal from `client`. `same(storage, body)`: whether a kept
    /// proposal's profile is `body`. A current proposal for the place
    /// answers it (a retransmission: its answer so far, the newest serial
    /// kept for its own answer; another profile or squad: failed), and only
    /// the roster's next place is loaded.
    pub fn propose(&mut self, client: ClientKey, prepare: &Prepare<'_>, same: impl Fn(&S, &[u8]) -> bool)
                   -> Admission {
        let Some(active) = self.active().filter(|a| a.matched.client == client && a.matched.seed == prepare.seed)
        else {
            return Admission::Ignore;
        };
        let ack = |answer| Ack { seed: prepare.seed, serial: prepare.serial, place: prepare.place, answer };
        let found = self.proposals.iter_mut().flatten()
            .find(|p| p.generation == active.generation && p.place == prepare.place);
        if let Some(found) = found {
            let storage = match &found.phase {
                Phase::Loading { storage } | Phase::Answered { storage, .. } | Phase::Unanswered { storage } =>
                    storage,
            };
            let answer = if found.squad == prepare.squad && same(storage, prepare.body) {
                found.serial = found.serial.max(prepare.serial);
                match found.phase {
                    Phase::Answered { outcome, .. } => outcome.into(),
                    _ => Answer::Busy,
                }
            } else {
                Answer::Failed
            };
            return Admission::Answer(ack(answer));
        }
        if !active.roster.takes(prepare.place) {
            return Admission::Answer(ack(Answer::Failed));
        }
        Admission::Load(Admitted {
            generation: active.generation,
            matched: active.matched,
            dictionary: active.dictionary,
            serial: prepare.serial,
            place: prepare.place,
            squad: prepare.squad,
        })
    }

    /// Keeps a loaded proposal: its storage (the profile its players are
    /// loaded from, `profile` its key: what its query's callback names), until
    /// it is retired and its players are gone. Given back when its match is
    /// no longer active or no room is left: answer it busy.
    pub fn admit(&mut self, admitted: Admitted, requested: KitMask, profile: ProfileKey, storage: S)
                 -> Result<(), S> {
        let current = self.active().is_some_and(|a| a.generation == admitted.generation);
        let Some(slot) = self.proposals.iter_mut().find(|p| p.is_none()).filter(|_| current) else {
            return Err(storage);
        };
        *slot = Some(Proposal {
            generation: admitted.generation,
            serial: admitted.serial,
            place: admitted.place,
            squad: admitted.squad,
            requested,
            profile,
            players: [None; KITS],
            phase: Phase::Loading { storage },
        });
        Ok(())
    }

    /// The query of the proposal whose profile is `profile` came back with
    /// these players (by kit, none for a kit not asked for or not loaded).
    /// They are kept (they live on its storage's profile until they are
    /// destroyed). Then `answered`.
    pub fn loaded(&mut self, profile: ProfileKey, players: [Option<PlayerKey>; KITS]) -> Completion {
        let active = self.active();
        let Some(proposal) = self.proposals.iter_mut().flatten()
            .find(|p| p.profile == profile && matches!(p.phase, Phase::Loading { .. }))
        else {
            return Completion::Unknown;
        };
        proposal.players = players;
        match active.filter(|a| a.generation == proposal.generation) {
            None => Completion::Stale,
            Some(active) => match proposal.requested.matched(players) {
                Some(players) if active.roster.takes(proposal.place) => Completion::Install {
                    players, place: proposal.place, squad: proposal.squad,
                    matched: active.matched, dictionary: active.dictionary,
                },
                _ => Completion::Refuse,
            },
        }
    }

    /// The completion of `profile`'s query is done, with what it was told
    /// (none for a stale one). An install takes the roster's next place. The
    /// caller sends the ack, frees the dead, then lets its world go (which
    /// may destroy players at once).
    pub fn answered(&mut self, profile: ProfileKey, outcome: Option<Outcome>) -> Answered<S> {
        let mut ack = None;
        let slot = self.proposals.iter_mut()
            .find(|p| p.as_ref().is_some_and(|p| p.profile == profile && matches!(p.phase, Phase::Loading { .. })));
        if let Some(slot) = slot {
            let proposal = slot.take().expect("found");
            let Phase::Loading { storage } = proposal.phase else { unreachable!("found Loading") };
            let current = match &mut self.state {
                State::Active(active) if active.generation == proposal.generation => Some(active),
                _ => None,
            };
            let phase = match (outcome, current) {
                (Some(outcome), Some(active)) => {
                    if outcome == Outcome::Installed {
                        active.roster = Roster(proposal.place.get() + 1);
                    }
                    ack = Some((active.matched.client, Ack { seed: active.matched.seed, serial: proposal.serial,
                                                             place: proposal.place, answer: outcome.into() }));
                    Phase::Answered { storage, outcome }
                }
                _ => Phase::Unanswered { storage },
            };
            *slot = Some(Proposal { phase, ..proposal });
        }
        Answered { dead: self.collect(), ack }
    }

    /// A player was destroyed (on any thread): the proposal whose storage
    /// its profile was lets go of it.
    pub fn destroyed(&mut self, player: PlayerKey, profile: ProfileKey) -> Dead<S> {
        if let Some(proposal) = self.proposals.iter_mut().flatten().find(|p| p.profile == profile) {
            for kept in &mut proposal.players {
                if *kept == Some(player) {
                    *kept = None;
                }
            }
        }
        self.collect()
    }

    /// Takes out every proposal no longer current whose query came back and
    /// whose players are all gone.
    fn collect(&mut self) -> Dead<S> {
        let mut dead = Dead::none();
        let active = self.active().map(|a| a.generation);
        for (slot, out) in self.proposals.iter_mut().zip(&mut dead.0) {
            let gone = slot.as_ref().is_some_and(|p| {
                Some(p.generation) != active && p.completed() && p.players.iter().all(Option::is_none)
            });
            if gone {
                match slot.take().map(|p| p.phase) {
                    Some(Phase::Answered { storage, .. } | Phase::Unanswered { storage }) => *out = Some(storage),
                    _ => unreachable!("only completed proposals are collected"),
                }
            }
        }
        dead
    }

    /// The proposals kept, for the tests.
    pub fn kept(&self) -> usize {
        self.proposals.iter().flatten().count()
    }

    /// The profiles of the proposals kept (the tests).
    #[cfg(test)]
    pub fn profiles(&self) -> std::vec::Vec<ProfileKey> {
        self.proposals.iter().flatten().map(|p| p.profile).collect()
    }

    /// Whether the proposal of `profile` is current (the tests).
    pub fn is_current(&self, profile: ProfileKey) -> bool {
        self.proposals.iter().flatten().any(|p| p.profile == profile && self.current(p))
    }
}

