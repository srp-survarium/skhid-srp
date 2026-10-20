//! The hand-off between the lobby and the match server: the lobby makes a
//! match here ([`MatchRegistry::create`]) and tells its players the address;
//! the match server finds each connecting client's place by its session and
//! hands the finished match back as a [`MatchResult`] ([`ResultsSink`]).
//!
//! The 0.21d server reads the same from `session::Matchmaking`
//! (`seat`, `formed_match`, `end_match`); this is its 0.26g counterpart, and
//! it depends on no lobby code.

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use session::matchmaking_policy::Closed;
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::maps::{self, MapMode};
use survarium::v026g::match_msgs::MAX_PLAYERS;
use survarium::v026g::match_msgs::players::Supplies;
use survarium::v026g::match_msgs::snapshot::{PlayerStatistics, Standing};
use survarium::v026g::match_msgs::static_info::{
    MatchOptions, MatchProfile, SCORE_EVENTS, SCORE_WEIGHTS, StaticInfo, StaticInfoError,
};

/// Set in every [`static_id`], clear in every match id.
const STATIC_ID_BIT: u32 = 0x8000_0000;

/// The id the static info of match `match_id` carries: what the client logs
/// as `Match id` and seeds the match with (the anomalies, the victory
/// items, each player's object: `static_info.md`). The same for every
/// client of the match, and never a match id.
///
/// It must differ from the id the lobby's `82` gives. The client keeps the
/// id of the last static info it took for as long as it runs (cleared only
/// by the constructor, 0x3183D2), and `network_client::connect_to_match`
/// 0x21BEB0 returns at once when `82` names that id (0x21BEBC..0x21BED5,
/// PROVEN). With the two equal, as they were until 2026-10-12, a client
/// whose match connection broke took the `82` back to its match and did
/// nothing: it stayed on "waiting for the server" until it was started
/// again (seen live that day).
pub fn static_id(match_id: u32) -> u32 {
    match_id | STATIC_ID_BIT
}

/// One player of a match to be made. The roster place is the player's
/// position in [`MatchSetup::players`].
#[derive(Clone, Debug, PartialEq)]
pub struct MatchPlayer {
    /// The session the client signs its match connection with (0xD8).
    pub session_id: u32,
    pub account_id: u32,
    /// What the others see and play against. Its `team` (0 or 1) and
    /// `squad` (0 for none) are the player's team and squad in the match.
    pub profile: MatchProfile,
    /// **Testing only** (`SRP_BOT_ACCOUNTS`; false for every player
    /// otherwise): the place is a scripted client's (`srp-bot`), which
    /// cannot simulate the game. It is a player for the others (roster,
    /// 0xE9, relays), and no witness of the world: its hash reports and
    /// uploads are not taken, it is never asked for a world, and the
    /// trust and resync rules count the other clients only (`game.rs`,
    /// "Bots").
    pub bot: bool,
    /// The place is the match server's own bot (`SRP_SERVER_BOTS`; false
    /// for every player otherwise): no account, no session, no client. Its
    /// player enters once a real client simulates, and its inputs are made
    /// by the match server (`super::drive`) and relayed as a player's are.
    /// Like a test bot it is no witness of the world. Its `session_id` and
    /// `account_id` are 0 and mean nothing: it has no seat, nobody can
    /// connect for it, and its result pays nobody.
    pub driven: bool,
}

/// A match the lobby formed.
#[derive(Clone, Debug, PartialEq)]
pub struct MatchSetup {
    /// A playable `maps_dict` id: the map, the mode and its timings
    /// (`survarium::v026g::maps`).
    pub map_id: u8,
    /// Logged by the client only.
    pub match_level: u16,
    /// The players who will really come: the roster's size is the N of the
    /// clients' start rule (the countdown starts once `floor(0.8 × N)` of
    /// them, and at least one, are in). 1 to 20.
    pub players: Vec<MatchPlayer>,
    /// The clients' score weights; `None` for the game's own.
    pub score_weights: Option<[f32; SCORE_EVENTS]>,
    /// A match of one player against the server's own bots ("Player vs
    /// Bots", the lobby's `pvb`): its teams are fixed and it takes no late
    /// joiner (the lobby's rule), it ends when its only player stops
    /// uploading worlds it is asked for (`Timing::pvb_stall`), and its
    /// result says so ([`MatchResult::vs_bots`]: the rewards pay less).
    pub vs_bots: bool,
}

/// Where the players of a made match connect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchTicket {
    /// What the lobby's `82` names. The static info carries
    /// [`static_id`] of it.
    pub match_id: u32,
    /// The match server's public host, as configured (`SRP_PUBLIC_HOST`).
    pub host: String,
    pub port: u16,
}

impl MatchTicket {
    /// `host:port`.
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

/// Why a match could not be made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CreateError {
    /// No players, or more than 20.
    Roster(usize),
    /// Not a map and mode the client can play.
    Map(u8),
    /// Two places have the same session.
    DuplicateSession(u32),
    /// A session still has a place in a match that is not over.
    AlreadySeated { session_id: u32, match_id: u32 },
    /// The clients could not hold these options (too long a name, an item
    /// their dictionary lacks, ...).
    StaticInfo(StaticInfoError),
}

impl std::fmt::Display for CreateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Roster(players) => write!(f, "a roster of {players}"),
            Self::Map(id) => write!(f, "map id {id} cannot be played"),
            Self::DuplicateSession(session) => {
                write!(f, "session {session:#x} has two places")
            }
            Self::AlreadySeated {
                session_id,
                match_id,
            } => write!(
                f,
                "session {session_id:#x} still has a place in match {match_id:#x}"
            ),
            Self::StaticInfo(error) => write!(f, "the match options: {error}"),
        }
    }
}

impl std::error::Error for CreateError {}

/// A place in a match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seat {
    pub match_id: u32,
    /// The roster place: the index in [`MatchSetup::players`].
    pub place: u8,
}

/// A roster place as the match server keeps it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Place {
    pub session_id: u32,
    pub account_id: u32,
    pub name: String,
    pub team: u8,
    /// [`MatchPlayer::bot`].
    pub bot: bool,
    /// [`MatchPlayer::driven`].
    pub driven: bool,
}

/// A made match, until it is over.
#[derive(Clone)]
pub(crate) struct Formed {
    pub match_id: u32,
    pub map: &'static MapMode,
    pub match_level: u16,
    pub places: Vec<Place>,
    /// The profiles the clients were given, by place: what a snapshot's
    /// player blocks are read against.
    pub profiles: Vec<MatchProfile>,
    pub score_weights: [f32; SCORE_EVENTS],
    pub info: StaticInfo,
    pub formed_at: Instant,
    /// Experimental dialect: fixed twenty-slot histories/statistics/masks.
    /// Every connecting client must advertise the matching capability.
    pub growing_roster: bool,
    pub initial_players: usize,
    /// [`MatchSetup::vs_bots`].
    pub vs_bots: bool,
}

/// How far a match is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LivePhase {
    /// Made: its players load the level, and the countdown has not begun.
    Gathering,
    /// "Match begins in … seconds".
    Countdown,
    /// In play.
    Playing,
    /// It has its result, and lingers for its clients to hear the end.
    Over,
}

/// What the match loop says of a match it runs ([`MatchRegistry::set_live`]):
/// the registry is all that the lobby and the web page see of a match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Live {
    /// When this was true. The times below are counted from it.
    pub at: Instant,
    pub phase: LivePhase,
    /// Each team's players who have not left: before the countdown the
    /// ones still to come count too.
    pub players: [u8; 2],
    /// The same by roster place: bit `i` is place `i`.
    pub present: u32,
    /// Each team's kills in the newest trusted snapshot.
    pub kills: [u16; 2],
    /// Since play began.
    pub played: Option<Duration>,
    /// Until the time limit, known from the countdown on.
    pub time_left: Option<Duration>,
    /// The newest trusted snapshot's standing, and how old that world is.
    pub standing: Option<(Standing, Duration)>,
}

/// The score of a match in play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiveStanding {
    /// Batteries brought (modes 2, 3) or tickets left (mode 4), by team.
    pub points: [u16; 2],
    /// The team that leads: 0, 1, or [`DRAW`].
    pub winner: u8,
    /// How old the world is that said so. The standing is the newest
    /// trusted snapshot's: it stays behind while the clients disagree.
    pub age: Duration,
}

/// A match made and not over, as it is now: for whoever shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveMatch {
    /// What the lobby knows the match by ([`MatchTicket::match_id`]). Not
    /// for the players' eyes.
    pub match_id: u32,
    /// The `maps_dict` id: the map and the mode.
    pub map_id: u8,
    pub phase: LivePhase,
    /// Each team's players who have not left: before the countdown the
    /// ones still to come count too.
    pub players: [u8; 2],
    /// The same by roster place: bit `i` is place `i`.
    pub present: u32,
    /// Each team's kills in the newest trusted snapshot.
    pub kills: [u16; 2],
    /// Since play began.
    pub played: Option<Duration>,
    /// Until the time limit ends it, known from the countdown on. The
    /// mode's own rule, or everyone leaving, ends it sooner.
    pub time_left: Option<Duration>,
    /// `None` until a world was trusted.
    pub standing: Option<LiveStanding>,
}

#[derive(Default)]
struct Inner {
    matches: HashMap<u32, std::sync::Arc<Formed>>,
    seats: HashMap<u32, Seat>,
    last_id: u32,
    /// A growing roster's admission as the match last said it: until when
    /// it takes a joiner, or why it takes none.
    admitting: HashMap<u32, Result<Instant, Closed>>,
    live: HashMap<u32, Live>,
}

/// A new account could not be appended to a running match. Appended slots
/// are never reused, including after failed preparation: some clients may
/// already own resources for that immutable profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JoinError {
    EmptyGroup,
    Unavailable,
    Full,
    AlreadyPlaying,
    StaticInfo(StaticInfoError),
}

impl std::fmt::Display for JoinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyGroup => write!(f, "the joining group is empty"),
            Self::Unavailable => write!(f, "the match is not accepting a new player"),
            Self::Full => write!(f, "all twenty roster places have been assigned"),
            Self::AlreadyPlaying => write!(f, "the account or session already has a match place"),
            Self::StaticInfo(error) => write!(f, "the new player's profile: {error}"),
        }
    }
}

impl std::error::Error for JoinError {}

/// The matches made and not yet over, shared by the lobby (which makes
/// them) and the match server (which runs them).
pub struct MatchRegistry {
    host: String,
    port: u16,
    inner: Mutex<Inner>,
}

impl MatchRegistry {
    /// A registry for a match server the clients reach at `host:port`.
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            host: host.to_owned(),
            port,
            inner: Mutex::new(Inner {
                last_id: random() | 1,
                ..Inner::default()
            }),
        }
    }

    /// The registry of the configured match server (`SRP_PUBLIC_HOST`,
    /// `SRP_MATCH_PORT`: the settings the 0.21d server has).
    pub fn from_config() -> Self {
        let config = &vostok::config::get().match_server;
        Self::new(&config.public_host, config.port)
    }

    /// The port the clients are told.
    pub fn port(&self) -> u16 {
        self.port
    }

    fn inner(&self) -> MutexGuard<'_, Inner> {
        // A panic elsewhere must not stop every match.
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Makes a match: the static info is built here, once, so what the
    /// clients cannot hold is refused before anyone is told to connect.
    /// The players then connect to the ticket's address with their
    /// sessions; a match no client reaches within two minutes is given up
    /// (its result says so).
    pub fn create(&self, setup: MatchSetup) -> Result<MatchTicket, CreateError> {
        self.create_at(setup, Instant::now())
    }

    /// Opt into the experimental growing-roster dialect. Release clients
    /// cannot play this match. The lobby does not select this mode yet.
    pub fn create_growing(&self, setup: MatchSetup) -> Result<MatchTicket, CreateError> {
        self.create_with_dialect(setup, Instant::now(), true)
    }

    pub(crate) fn create_at(
        &self,
        setup: MatchSetup,
        now: Instant,
    ) -> Result<MatchTicket, CreateError> {
        self.create_with_dialect(setup, now, false)
    }

    pub(crate) fn create_with_dialect(
        &self,
        setup: MatchSetup,
        now: Instant,
        growing_roster: bool,
    ) -> Result<MatchTicket, CreateError> {
        if setup.players.is_empty() || setup.players.len() > MAX_PLAYERS {
            return Err(CreateError::Roster(setup.players.len()));
        }
        let map = maps::map_mode(setup.map_id)
            .filter(|map| map.is_playable())
            .ok_or(CreateError::Map(setup.map_id))?;
        let mut inner = self.inner();
        for (place, player) in setup.players.iter().enumerate() {
            // The server's own bots have no session to clash.
            if player.driven {
                continue;
            }
            if setup.players[..place]
                .iter()
                .any(|other| !other.driven && other.session_id == player.session_id)
            {
                return Err(CreateError::DuplicateSession(player.session_id));
            }
            if let Some(seat) = inner.seats.get(&player.session_id) {
                return Err(CreateError::AlreadySeated {
                    session_id: player.session_id,
                    match_id: seat.match_id,
                });
            }
        }
        let mut match_id = inner.last_id;
        loop {
            // Never 0, never one in use, never a [`static_id`].
            match_id = match_id.wrapping_mul(0x9E37_79B1).wrapping_add(0x7F4A_7C15);
            if match_id != 0
                && match_id & STATIC_ID_BIT == 0
                && !inner.matches.contains_key(&match_id)
            {
                break;
            }
        }
        inner.last_id = match_id;
        let score_weights = setup.score_weights.unwrap_or(SCORE_WEIGHTS);
        let places = setup
            .players
            .iter()
            .map(|player| Place {
                session_id: player.session_id,
                account_id: player.account_id,
                name: player.profile.name.clone(),
                team: player.profile.team,
                bot: player.bot,
                driven: player.driven,
            })
            .collect();
        // A server bot's place has no seat: no client connects for it.
        let sessions: Vec<Option<u32>> = setup
            .players
            .iter()
            .map(|player| (!player.driven).then_some(player.session_id))
            .collect();
        let options = MatchOptions {
            players: setup
                .players
                .into_iter()
                .map(|player| player.profile)
                .collect(),
            map_id: setup.map_id,
            match_id: static_id(match_id),
            match_level: setup.match_level,
            score_weights,
        };
        let info =
            StaticInfo::new(&options, Dictionary::shared()).map_err(CreateError::StaticInfo)?;
        let profiles = options.players.clone();
        for (place, session_id) in sessions.into_iter().enumerate() {
            let Some(session_id) = session_id else {
                continue;
            };
            inner.seats.insert(
                session_id,
                Seat {
                    match_id,
                    place: place as u8,
                },
            );
        }
        inner.matches.insert(
            match_id,
            std::sync::Arc::new(Formed {
                match_id,
                map,
                match_level: setup.match_level,
                places,
                profiles,
                score_weights,
                info,
                formed_at: now,
                growing_roster,
                initial_players: options.players.len(),
                vs_bots: setup.vs_bots,
            }),
        );
        let count = |which: fn(&Place) -> bool| {
            inner.matches[&match_id]
                .places
                .iter()
                .filter(|place| which(place))
                .count()
        };
        let (bots, driven) = (count(|place| place.bot), count(|place| place.driven));
        log::info!(
            "Match {match_id:#x} made: map {} ({}, mode {}), {} players{}{}{}",
            map.id,
            map.name,
            map.mode,
            options.players.len(),
            if setup.vs_bots {
                ", one player against the server's bots"
            } else {
                ""
            },
            if bots == 0 {
                String::new()
            } else {
                format!(", {bots} of them test bots (SRP_BOT_ACCOUNTS): no witnesses of the world")
            },
            if driven == 0 {
                String::new()
            } else {
                format!(", {driven} of them the server's own bots (SRP_SERVER_BOTS)")
            }
        );
        Ok(MatchTicket {
            match_id,
            host: self.host.clone(),
            port: self.port,
        })
    }

    /// Reserve a genuinely new player's append-only place. The game loop
    /// opens admission only during play and closes it while a previous
    /// addition is being prepared. Reserving does not emit a player-entry
    /// event; the resource barrier and snapshot catch-up happen first.
    pub fn join_running(
        &self,
        match_id: u32,
        player: MatchPlayer,
    ) -> Result<MatchTicket, JoinError> {
        self.join_running_group(match_id, vec![player])
    }

    /// Reserve a whole party atomically. No profile or seat changes on failure.
    pub fn join_running_group(
        &self,
        match_id: u32,
        players: Vec<MatchPlayer>,
    ) -> Result<MatchTicket, JoinError> {
        if players.is_empty() {
            return Err(JoinError::EmptyGroup);
        }
        // The server's own bots are put in when a match is made, never
        // into one being played.
        if players.iter().any(|player| player.driven) {
            return Err(JoinError::Unavailable);
        }
        let mut inner = self.inner();
        let formed = inner.matches.get(&match_id).ok_or(JoinError::Unavailable)?;
        if !formed.growing_roster {
            return Err(JoinError::Unavailable);
        }
        if players.len() > MAX_PLAYERS - formed.places.len() {
            return Err(JoinError::Full);
        }
        if !open_at(inner.admitting.get(&match_id), Instant::now()) {
            return Err(JoinError::Unavailable);
        }
        // An account whose place in this match has no seat any more (it
        // left by its own word) comes as a new place, its old one gone for
        // good (the owner's of 2026-10-19); one with a seat anywhere, never.
        let mut sessions = HashSet::new();
        let mut accounts = HashSet::new();
        for player in &players {
            if !sessions.insert(player.session_id)
                || !accounts.insert(player.account_id)
                || inner.seats.contains_key(&player.session_id)
                || inner.seats.values().any(|seat| {
                    inner
                        .matches
                        .get(&seat.match_id)
                        .and_then(|formed| formed.places.get(usize::from(seat.place)))
                        .is_some_and(|place| place.account_id == player.account_id)
                })
            {
                return Err(JoinError::AlreadyPlaying);
            }
        }
        let mut next = (**formed).clone();
        let first = next.places.len();
        next.profiles
            .extend(players.iter().map(|player| player.profile.clone()));
        next.info = StaticInfo::new(
            &MatchOptions {
                players: next.profiles.clone(),
                map_id: next.map.id,
                match_id: static_id(match_id),
                match_level: next.match_level,
                score_weights: next.score_weights,
            },
            Dictionary::shared(),
        )
        .map_err(JoinError::StaticInfo)?;
        for (offset, player) in players.into_iter().enumerate() {
            next.places.push(Place {
                session_id: player.session_id,
                account_id: player.account_id,
                name: player.profile.name,
                team: player.profile.team,
                bot: player.bot,
                driven: false,
            });
            inner.seats.insert(
                player.session_id,
                Seat {
                    match_id,
                    place: (first + offset) as u8,
                },
            );
        }
        let count = next.places.len() - first;
        inner.matches.insert(match_id, std::sync::Arc::new(next));
        // Closed until the match says otherwise of the grown roster.
        inner.admitting.insert(match_id, Err(Closed::Joining));
        log::info!(
            "Match {match_id:#x}: reserved {count} new places from {first}; waiting for resource preparation"
        );
        Ok(MatchTicket {
            match_id,
            host: self.host.clone(),
            port: self.port,
        })
    }

    /// Do not let an observation of an older roster reopen admission after
    /// the lobby has concurrently reserved the next place.
    /// `admission`: until when it takes a joiner, or why it takes none
    /// (`Game::admission`).
    pub(crate) fn set_admitting(
        &self,
        match_id: u32,
        roster: usize,
        admission: Result<Instant, Closed>,
    ) {
        let mut inner = self.inner();
        if inner
            .matches
            .get(&match_id)
            .is_some_and(|m| m.growing_roster && m.places.len() == roster)
        {
            inner.admitting.insert(match_id, admission);
        }
    }

    /// Whether new reservations are allowed at `now`. The absolute deadline
    /// also closes admission if the match thread stops publishing updates.
    pub fn admits(&self, match_id: u32, now: Instant) -> bool {
        open_at(self.inner().admitting.get(&match_id), now)
    }

    /// Why the match takes no new reservation at `now`, as it last said;
    /// past its deadline, [`Closed::LastSeconds`]. `None`: it takes one, or
    /// it has said nothing (not a growing roster, not polled yet, or over).
    pub fn closed(&self, match_id: u32, now: Instant) -> Option<Closed> {
        match self.inner().admitting.get(&match_id)? {
            Ok(deadline) if now < *deadline => None,
            Ok(_) => Some(Closed::LastSeconds),
            Err(closed) => Some(*closed),
        }
    }

    /// The place a session has in a match that is not over.
    pub fn seat(&self, session_id: u32) -> Option<Seat> {
        self.inner().seats.get(&session_id).copied()
    }

    /// Gives up a session's place (the player will not come, or left for
    /// good): it can be put in another match. A client of it that still
    /// connects is turned away.
    pub fn release(&self, session_id: u32) -> Option<Seat> {
        self.inner().seats.remove(&session_id)
    }

    pub(crate) fn release_place(&self, match_id: u32, place: usize) -> bool {
        let mut inner = self.inner();
        let before = inner.seats.len();
        inner
            .seats
            .retain(|_, seat| seat.match_id != match_id || usize::from(seat.place) != place);
        inner.seats.len() != before
    }

    /// The account's place in the match is now `session_id`'s: the player
    /// signed in again while the match goes on, and its client signs its
    /// match connection (0xD8) with the new session. The session it was
    /// seated under has no place any more. Of an account's places in the
    /// match (it left one and came again) its latest: the others are gone
    /// for good. `None`, and nothing changed, when the match is not open,
    /// the account has no place in it, or the session is seated elsewhere.
    pub fn reseat(&self, match_id: u32, account_id: u32, session_id: u32) -> Option<Seat> {
        let mut inner = self.inner();
        let place = inner
            .matches
            .get(&match_id)?
            .places
            .iter()
            .rposition(|place| place.account_id == account_id)?;
        let seat = Seat {
            match_id,
            place: place as u8,
        };
        if inner
            .seats
            .get(&session_id)
            .is_some_and(|held| *held != seat)
        {
            return None;
        }
        inner.seats.retain(|_, held| *held != seat);
        inner.seats.insert(session_id, seat);
        Some(seat)
    }

    /// The match has its result: its players are free for another match
    /// while it lingers for its clients to hear the end.
    pub(crate) fn free_seats(&self, match_id: u32) {
        let mut inner = self.inner();
        if inner.admitting.contains_key(&match_id) {
            inner.admitting.insert(match_id, Err(Closed::Ending));
        }
        inner.seats.retain(|_, seat| seat.match_id != match_id);
    }

    /// Whether the match is made and not over.
    pub fn is_open(&self, match_id: u32) -> bool {
        self.inner().matches.contains_key(&match_id)
    }

    /// The matches made and not over.
    pub fn match_count(&self) -> usize {
        self.inner().matches.len()
    }

    pub(crate) fn formed(&self, match_id: u32) -> Option<std::sync::Arc<Formed>> {
        self.inner().matches.get(&match_id).cloned()
    }

    /// The match loop's word on a match that is open.
    pub(crate) fn set_live(&self, match_id: u32, live: Live) {
        let mut inner = self.inner();
        if inner.matches.contains_key(&match_id) {
            inner.live.insert(match_id, live);
        }
    }

    /// The matches made and not over as they are at `now`, the oldest
    /// first. One no client has reached yet is gathering its whole roster.
    pub fn live_matches(&self, now: Instant) -> Vec<LiveMatch> {
        let inner = self.inner();
        let mut formed: Vec<_> = inner.matches.values().collect();
        formed.sort_by_key(|formed| (formed.formed_at, formed.match_id));
        formed
            .into_iter()
            .map(|formed| Self::live_of(formed, inner.live.get(&formed.match_id), now))
            .collect()
    }

    /// One match of [`Self::live_matches`]; `None` once it is over.
    pub fn live_match(&self, match_id: u32, now: Instant) -> Option<LiveMatch> {
        let inner = self.inner();
        let formed = inner.matches.get(&match_id)?;
        Some(Self::live_of(formed, inner.live.get(&match_id), now))
    }

    fn live_of(formed: &Formed, live: Option<&Live>, now: Instant) -> LiveMatch {
        let Some(live) = live else {
            let mut players = [0; 2];
            for place in &formed.places {
                players[usize::from(place.team.min(1))] += 1;
            }
            return LiveMatch {
                match_id: formed.match_id,
                map_id: formed.map.id,
                phase: LivePhase::Gathering,
                players,
                present: (1_u32 << formed.places.len().min(31)) - 1,
                kills: [0; 2],
                played: None,
                time_left: None,
                standing: None,
            };
        };
        let since = now.saturating_duration_since(live.at);
        LiveMatch {
            match_id: formed.match_id,
            map_id: formed.map.id,
            phase: live.phase,
            players: live.players,
            present: live.present,
            kills: live.kills,
            played: live.played.map(|played| played + since),
            time_left: live.time_left.map(|left| left.saturating_sub(since)),
            standing: live.standing.map(|(standing, age)| LiveStanding {
                points: standing.points,
                winner: standing.winner,
                age: age + since,
            }),
        }
    }

    /// Every open match and when it was made.
    pub(crate) fn matches(&self) -> Vec<(u32, Instant)> {
        self.inner()
            .matches
            .values()
            .map(|formed| (formed.match_id, formed.formed_at))
            .collect()
    }

    /// The match is over: its seats are free.
    pub(crate) fn end(&self, match_id: u32) -> Option<std::sync::Arc<Formed>> {
        let mut inner = self.inner();
        inner.admitting.remove(&match_id);
        inner.live.remove(&match_id);
        inner.seats.retain(|_, seat| seat.match_id != match_id);
        inner.matches.remove(&match_id)
    }
}

/// Whether an admission as the match said it takes a joiner at `now`.
fn open_at(admission: Option<&Result<Instant, Closed>>, now: Instant) -> bool {
    matches!(admission, Some(Ok(deadline)) if now < *deadline)
}

/// A number that differs from run to run, for the first match id.
fn random() -> u32 {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    );
    hasher.finish() as u32
}

/// Why a match was over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchEnd {
    /// The mode's own rule: a team brought every battery home (modes 2 and
    /// 3), or has no tickets and nobody alive (mode 4).
    ModeRule,
    /// The match's time ran out, or it waited its whole waiting time for
    /// players without starting ([`MatchResult::played_ms`] is then
    /// `None`).
    TimeLimit,
    /// Everyone who was in it left.
    Deserted,
    /// No client ever connected.
    NoShow,
}

/// The standing value for "no team": a draw.
pub const DRAW: u8 = survarium::v026g::match_msgs::rules::DRAW;

/// One roster place of a finished match.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerResult {
    /// The roster place.
    pub place: u8,
    pub session_id: u32,
    pub account_id: u32,
    pub name: String,
    pub team: u8,
    /// [`MatchPlayer::driven`]: the server's own bot. Its session and
    /// account are 0; nobody is to be paid or told for it.
    pub driven: bool,
    /// Whether the player was ever in the match (0xE9 went out for it).
    /// One whose game never got that far neither won nor lost.
    pub entered: bool,
    /// Whether it was in the match when it ended.
    pub present_at_end: bool,
    /// Whether it left (0xE0, a timeout or a disconnect) at least once.
    pub left: bool,
    /// How long it was in the match, in match ms, over all its stays.
    pub time_in_match_ms: u32,
    /// Its counters in the last trusted snapshot, by event and kit
    /// (`survarium::v026g::match_msgs::snapshot::event`). `None`: no
    /// trusted snapshot had statistics.
    pub statistics: Option<PlayerStatistics>,
    /// The scoreboard's score from those counters and the match's weights;
    /// the client rounds each event as it happens, so its own last digit
    /// can differ [LIKELY].
    pub score: u32,
    /// From the kd-stats rule of that snapshot when it read, else from the
    /// counters.
    pub kills: u16,
    pub deaths: u16,
    /// The longest killstreak any trusted snapshot showed.
    pub longest_killstreak: u16,
    /// The kit choices it made for a tick before play began. A patched
    /// game (patch 31) respawns a live player who has no statistics yet in
    /// the chosen kit where it stands, with no death: each may be a life
    /// more than its deaths, with a kit's supplies of its own.
    pub kit_changes: u16,
    /// What the player had left in the last trusted snapshot: alive or
    /// not, the kit in use, and of every item of its profile what is in
    /// store, in its slots and in its weapons
    /// (`survarium::v026g::match_msgs::players`). `None`: no trusted
    /// snapshot, the player was not in it, or its block did not read.
    pub supplies: Option<Supplies>,
}

/// How a match ended, for the results screen and the rewards. Every match
/// made gives exactly one.
#[derive(Clone, Debug, PartialEq)]
pub struct MatchResult {
    pub match_id: u32,
    pub map_id: u8,
    /// `survarium::v026g::maps::mode`.
    pub game_mode: u8,
    pub match_level: u16,
    pub end: MatchEnd,
    /// 0 or 1, [`DRAW`], or `None` when no trusted snapshot gave a
    /// standing (nobody is to be paid as a winner then).
    pub winner: Option<u8>,
    /// Batteries home (modes 2, 3) or tickets left (mode 4), from the last
    /// trusted snapshot.
    pub team_points: [u16; 2],
    /// The match clock when it ended, in ms.
    pub duration_ms: u32,
    /// How long it was played after the countdown; `None` if play never
    /// began.
    pub played_ms: Option<u32>,
    /// The match time of the snapshot the standing and the statistics are
    /// from. A snapshot is trusted when its CRC is what the clients
    /// reported for its tick (two of them, or the only one in the match)
    /// and its numbers are possible.
    pub trusted_snapshot_ms: Option<u32>,
    /// How many clients vouched for that snapshot: 2 or more, or 1 when
    /// only one client was in the match then (it could claim anything
    /// within what is possible; the rewards code may want to pay less).
    /// 0 with no trusted snapshot. A test bot ([`MatchPlayer::bot`]) is
    /// never one of them, nor one of the clients "in the match" here.
    pub trusted_by: u8,
    /// By roster place.
    pub players: Vec<PlayerResult>,
    /// [`MatchSetup::vs_bots`].
    pub vs_bots: bool,
}

/// Where finished matches go: the lobby's results and rewards code.
///
/// Called on the match server's thread, once for every match made, before
/// the clients hear that the match is over (they are back in the lobby a
/// moment later and ask for the results). Keep it short, or hand the result
/// to another thread: a `std::sync::mpsc::Sender<MatchResult>` is a sink.
pub trait ResultsSink: Send {
    fn match_ended(&mut self, result: MatchResult);
}

impl ResultsSink for std::sync::mpsc::Sender<MatchResult> {
    fn match_ended(&mut self, result: MatchResult) {
        // Nobody listening is the listener's choice.
        let _ = self.send(result);
    }
}

impl<F: FnMut(MatchResult) + Send> ResultsSink for F {
    fn match_ended(&mut self, result: MatchResult) {
        self(result);
    }
}
