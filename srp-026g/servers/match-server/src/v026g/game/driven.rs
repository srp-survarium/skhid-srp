//! The match server's own bots (`MatchPlayer::driven`, `SRP_SERVER_BOTS`):
//! places with no client (`docs/notes/player_bots.md` §6).
//!
//! A real client simulates every player from the inputs relayed to it,
//! and a relayed input is applied unchecked (`player_bots.md` §1). So the
//! server enters such a place itself (0xE9, as `finish_sync` does for a
//! client) and relays inputs it makes up for it, with nothing to receive:
//! - it enters once a real client simulates and the match has a clock,
//!   [`ROSTER_LEAD_MS`] ahead, so the 0xE9 reaches that client before its
//!   tick, as a second player's does; a client that starts later gets the
//!   entry and the inputs from the history, as it gets anyone's;
//! - its inputs are one a tick at most, on the grid, 50 to 100 ms ahead of
//!   the match clock, so never late, moved or dropped; each is relayed and
//!   kept in the history as a client's is (`Game::input`, from the relay
//!   on);
//! - it is no witness of the world (`Game::is_bot`): no reports, no
//!   uploads, never counted among the clients that agree;
//! - it never times out; a match with only bots in it is over after
//!   [`BOTS_ALONE`] (`check_bots_alone`). A match against bots ("Player vs
//!   Bots", the lobby's `pvb`) takes no late joiner: its teams are fixed,
//!   and its bots never leave.
//!
//! A room's bots (its "Bots per team", the owner's of 2026-10-26: "up to 3
//! vs 3 bots, which will be replaced with real players once those join";
//! `MatchSetup::room_minimum` and driven places): each team keeps the size
//! it was made with, its players and its bots (`room_bots`,
//! `matchmaking_policy::bot_move`):
//! - a player more than that (a late joiner, a player back) takes a bot's
//!   place: the team's last bot by place leaves as a player leaves (0xEA),
//!   a tick after the player's entry, so the team is never a player short.
//!   Just before its leave it presses drop (bit 22, which acts only while
//!   it carries a battery: a carrier lets it go where it stands, as a
//!   player may) and then lets every key go, so nothing stays pressed for
//!   its place;
//! - a player fewer (one left, by its own word or its game gone) brings
//!   one of the team's bots back, on its own place (0xE9 again, as a player
//!   whose game came back), while the match keeps a place for it beside the
//!   one of every bot in it: a place is never used twice, so the player who
//!   takes a bot's place comes on a new one, and the lobby keeps one for
//!   each bot (`matchmaking_policy::start_bots`, `place_joiner`);
//! - never more bots than the team started with, never a leave of a team
//!   that can no longer respawn (Team Deathmatch out of tickets: a bot may
//!   be its last life, and the mode's rule ends the match on that), and a
//!   bot's place changes again only once every game simulating has played
//!   its last change (a leave for a tick a game has not played the entry of
//!   is dropped, `start.rs`);
//! - the clients see an ordinary leave and an ordinary entry: no new
//!   message; the recordings and replays have them as any.
//!
//! Where its inputs come from:
//! - with `SRP_MAPDATA_DIR` ([`Walkers`]): `super::super::bots`, bots that
//!   play (`bots/mod.rs`). They get every player's inputs as relayed
//!   ([`Game::observe_input`]), and the bodies, statistics and batteries of
//!   the worlds the match trusts ([`Game::observe_driven`]); while they
//!   play the clients are asked for a world every [`PLAYING_REFRESH`], and
//!   every [`WAITING_REFRESH`] while one waits to see its body (just
//!   entered, or dead). Until the map's walk grid is built (0.3 to 0.8 s,
//!   on a thread of its own) they stand; if the map has no file they run
//!   the stage 0 script instead. `SRP_BOT_TUNING` (a JSON file, read when
//!   the match is made) changes their numbers; `SRP_BOT_TRACE_DIR` keeps
//!   their decisions; `SRP_BOT_ROUTES` (a JSON file, private: made from
//!   the players' saved worlds) gives them the routes humans took
//!   (`bots/routes.rs`);
//! - without it, the stage 0 script (`super::super::drive`).

use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};

use mapdata::{Vec3, World};
use survarium::v026g::match_msgs::motion::Motion;
use survarium::v026g::match_msgs::players;
use survarium::v026g::match_msgs::snapshot::{Battery, Decoded};

use session::matchmaking_policy::{BotMove, TeamBots, bot_move};
use survarium::v026g::match_msgs::MAX_PLAYERS;

use super::*;
use crate::v026g::bots::body::Fix;
use crate::v026g::bots::maps::{MapState, Maps};
use crate::v026g::bots::reflex::WeaponKind;
use crate::v026g::bots::routes::RouteTable;
use crate::v026g::bots::tactics::Tactics;
use crate::v026g::bots::trace::Trace;
use crate::v026g::bots::tuning::Tuning;
use crate::v026g::bots::view::{BatteryState, Clock, WorldFacts};
use crate::v026g::bots::{Bots, Note};
use crate::v026g::drive::{self, Drive};

/// While the bots play, a world this often (the clients' upload: two of
/// them, 30 to 60 KB each [LIKELY, the worlds saved]).
const PLAYING_REFRESH: Duration = Duration::from_secs(2);
/// While a bot waits to see its body, sooner.
const WAITING_REFRESH: Duration = Duration::from_secs(1);

/// Where a server's bot's place is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BotSeat {
    /// Not entered yet: it enters once a real game simulates.
    Waiting,
    /// In the match since its entry's tick.
    In { since: u32 },
    /// Out since its leave's tick: it gave its place up to a player, and
    /// comes back on it when its team needs it (a room's, `room_bots`).
    Out { since: u32 },
}

/// What happens to a bot's place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BotEvent {
    /// Its entry (0xE9) at `at`.
    Enters { at: u32 },
    /// Its leave (0xEA) at `at`.
    Leaves { at: u32 },
}

impl BotSeat {
    /// The seat after `event`; `None` when the event does not happen to
    /// it (a bot in the match does not enter, one not in it does not
    /// leave): then nothing is sent.
    pub(super) fn after(self, event: BotEvent) -> Option<Self> {
        match (self, event) {
            (Self::Waiting | Self::Out { .. }, BotEvent::Enters { at }) => {
                Some(Self::In { since: at })
            }
            (Self::In { .. }, BotEvent::Leaves { at }) => Some(Self::Out { since: at }),
            (Self::Waiting | Self::Out { .. }, BotEvent::Leaves { .. })
            | (Self::In { .. }, BotEvent::Enters { .. }) => None,
        }
    }

    /// The tick of its last change; none before its first entry.
    fn since(self) -> Option<u32> {
        match self {
            Self::Waiting => None,
            Self::In { since } | Self::Out { since } => Some(since),
        }
    }
}

/// A server's bot's place: where it is, and the stage 0 script its inputs
/// come from without the map's walk grid (started at each entry).
pub(super) struct BotPlace {
    pub seat: BotSeat,
    drive: Drive,
}

/// The bots of a match and their map.
pub(super) struct Walkers {
    bots: Bots,
    maps: Arc<Maps>,
    project: &'static str,
    /// Held once built, so the store may drop it while the match runs.
    world: Option<Arc<World>>,
    /// The bots' tactics being built on a thread of their own (0.4-0.8 s
    /// on the host for a big map, which the match loop, the server's one
    /// thread, must not wait for).
    tactics: Option<Receiver<Tactics>>,
    /// The map has no walk grid: the stage 0 script drives them.
    absent: bool,
    /// Since when the map waits for room in the store
    /// ([`MapState::Waiting`]), for the log.
    waiting: Option<std::time::Instant>,
    /// The roster events handed to the bots so far.
    events_seen: usize,
}

impl Walkers {
    /// With `SRP_MAPDATA_DIR` and a server's bot in the match; the map's
    /// build starts now, the tuning file is read now.
    pub(super) fn new(formed: &Formed, settings: &Settings) -> Option<Self> {
        let maps = settings.maps.clone()?;
        let driven: Vec<bool> = formed.places.iter().map(|place| place.driven).collect();
        if !driven.iter().any(|&driven| driven) {
            return None;
        }
        let id = formed.match_id;
        let tuning = match std::env::var_os("SRP_BOT_TUNING").filter(|path| !path.is_empty()) {
            Some(path) => match Tuning::load(path.as_ref()) {
                Ok(tuning) => tuning,
                Err(why) => {
                    log::warn!("Match {id:#x}: SRP_BOT_TUNING not used ({why}); the defaults");
                    Tuning::default()
                }
            },
            None => Tuning::default(),
        };
        let hash = tuning.hash();
        let trace = match std::env::var_os("SRP_BOT_TRACE_DIR").filter(|dir| !dir.is_empty()) {
            Some(dir) => Trace::open(dir.as_ref(), id, hash),
            None => Trace::none(hash),
        };
        log::info!("Match {id:#x}: the bots run on tuning {hash:08x}");
        let project = formed.map.project;
        maps.request(project);
        let teams: Vec<u8> = formed.places.iter().map(|place| place.team).collect();
        // A bot's role by its weapon 1's category; a player's weapon may
        // be of one too, and is no bot's.
        let roles: Vec<Option<&str>> = formed
            .profiles
            .iter()
            .zip(&driven)
            .map(|(profile, &driven)| {
                driven
                    .then(|| survarium::v026g::loadouts::bot_role_of(profile))
                    .flatten()
                    .map(|role| role.name())
            })
            .collect();
        log::info!("Match {id:#x}: the bots' roles by place {roles:?}");
        let weapons = formed
            .profiles
            .iter()
            .zip(&driven)
            .map(|(profile, &driven)| weapon_of(profile, driven))
            .collect();
        Some(Self {
            bots: Bots::new(
                id,
                formed.map.mode,
                formed.map.level,
                &teams,
                &driven,
                &roles,
                Arc::new(tuning),
                trace,
            )
            .with_container_teams(formed.map.container_teams),
            maps,
            project,
            world: None,
            tactics: None,
            absent: false,
            waiting: None,
            events_seen: 0,
        })
        .map(|mut walkers| {
            walkers.bots.set_weapons(weapons);
            // Each bot's kits as its profile has them (its match's
            // equipment level, `session::v026g::bot_kits`).
            for (place, profile) in formed.profiles.iter().enumerate() {
                if driven.get(place).copied().unwrap_or(false) {
                    walkers.bots.wear(place, kits_of(profile));
                }
            }
            // Play 17: the routes humans took (a private table; without it
            // the bots roam to the exits).
            if let Some(path) = std::env::var_os("SRP_BOT_ROUTES").filter(|path| !path.is_empty()) {
                match RouteTable::load(path.as_ref()) {
                    Ok(table) => walkers.bots.set_routes(&table),
                    Err(why) => log::warn!("Match {id:#x}: SRP_BOT_ROUTES not used ({why})"),
                }
            }
            walkers
        })
    }

    /// The map's walk grid, once built, and the bots' tactics on it (built
    /// on a thread, handed to the bots once); `absent` set if there is
    /// none.
    fn world(&mut self, id: u32) -> Option<Arc<World>> {
        if let Some(built) = self.tactics.as_ref() {
            match built.try_recv() {
                Ok(tactics) => {
                    self.tactics = None;
                    let floors = tactics.world.grid.len();
                    let summary = self.bots.tactics_ready(tactics);
                    log::info!(
                        "Match {id:#x}: the bots have the map {} ({floors} floors; {summary})",
                        self.project,
                    );
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    log::warn!(
                        "Match {id:#x}: the bots' tactics for {} were not built; the server's \
                         bots run the stage 0 script",
                        self.project
                    );
                    self.tactics = None;
                    self.absent = true;
                }
            }
        }
        if self.world.is_none() && !self.absent {
            match self.maps.get(self.project) {
                MapState::Ready(base) => {
                    if let Some(since) = self.waiting.take() {
                        log::info!(
                            "Match {id:#x}: the bots' map {} is loaded, {:.0} s after it began to wait",
                            self.project,
                            since.elapsed().as_secs_f32()
                        );
                    }
                    let world = base.world.clone();
                    let margin = self.bots.tuning.hazard_margin;
                    if margin.to_bits() == base.margin().to_bits() {
                        // The map's own tactics, shared by every match on it.
                        let floors = world.grid.len();
                        let summary = self.bots.tactics_ready(Tactics::of(base));
                        log::info!(
                            "Match {id:#x}: the bots have the map {} ({floors} floors; {summary})",
                            self.project,
                        );
                    } else {
                        // Tuned to another margin: tactics of its own.
                        let (send, built) = std::sync::mpsc::channel();
                        let for_tactics = world.clone();
                        let spawned = std::thread::Builder::new()
                            .name(format!("bot tactics {id:#x}"))
                            .spawn(move || {
                                let started = std::time::Instant::now();
                                let tactics = Tactics::new(for_tactics, margin);
                                log::info!(
                                    "Match {id:#x}: the bots' tactics for margin {margin} built \
                                     in {:.2} s",
                                    started.elapsed().as_secs_f32()
                                );
                                let _ = send.send(tactics);
                            });
                        match spawned {
                            Ok(_) => self.tactics = Some(built),
                            Err(error) => {
                                log::warn!(
                                    "Match {id:#x}: can't start the bots' tactics build \
                                     ({error}); the server's bots run the stage 0 script"
                                );
                                self.absent = true;
                            }
                        }
                    }
                    self.world = Some(world);
                }
                MapState::Loading => {}
                MapState::Waiting(why) => {
                    if self.waiting.is_none() {
                        log::info!(
                            "Match {id:#x}: the bots' map {} waits ({why}); the server's bots                              run the stage 0 script till it is loaded",
                            self.project
                        );
                        self.waiting = Some(std::time::Instant::now());
                    }
                }
                MapState::Absent(why) => {
                    log::warn!(
                        "Match {id:#x}: no walk grid for {} ({why}); the server's bots run the \
                         stage 0 script",
                        self.project
                    );
                    self.absent = true;
                }
            }
        }
        self.world.clone()
    }
}

/// A bot's kits from its profile, as its arms: each kit's weapon 1, its
/// rounds and its pockets' medkits and grenades (`bots::guns::Arms::of_kit`);
/// a kit whose weapon the bots do not know is left out.
fn kits_of(
    profile: &survarium::v026g::match_msgs::static_info::MatchProfile,
) -> Vec<crate::v026g::bots::guns::Arms> {
    use survarium::v026g::loadouts::{Loadout, SlotItem};
    profile
        .kits
        .iter()
        .filter_map(|kit| {
            let items = kit
                .slots
                .iter()
                .enumerate()
                .filter_map(|(slot, held)| {
                    let (index, count) = (*held)?;
                    let item = profile.items.get(usize::from(index))?;
                    Some(SlotItem {
                        slot: slot as u8,
                        dict_id: item.dict_id,
                        count: u32::from(count),
                    })
                })
                .collect();
            crate::v026g::bots::guns::Arms::of_kit(&Loadout { items })
        })
        .collect()
}

/// A place's weapon 1 kind from its profile: a bot's (`driven`) by its
/// role, a player's by the item in slot 7 (its dictionary category and
/// kind).
fn weapon_of(
    profile: &survarium::v026g::match_msgs::static_info::MatchProfile,
    driven: bool,
) -> Option<WeaponKind> {
    if let Some(role) = survarium::v026g::loadouts::bot_role_of(profile).filter(|_| driven) {
        return WeaponKind::of_role(role.name());
    }
    let kit = profile.kits.first()?;
    let (index, _) = kit.slots[7]?;
    let item = profile.items.get(usize::from(index))?;
    let kind = survarium::v026g::item_params::item_params(item.dict_id)?.kind;
    let category = survarium::v026g::dictionary::Dictionary::shared()
        .item(item.dict_id)?
        .category;
    WeaponKind::of_item(kind, category)
}

fn log_notes(id: u32, notes: Vec<Note>) {
    for note in notes {
        if note.place == usize::MAX {
            log::info!("Match {id:#x}: the bots at {} ms: {}", note.time, note.text);
        } else {
            log::info!(
                "Match {id:#x}: player {} (the server's bot) at {} ms {}",
                note.place,
                note.time,
                note.text
            );
        }
    }
}

/// A world's statistics and batteries for the bots.
fn world_facts(time: u32, decoded: &Decoded) -> WorldFacts {
    use survarium::v026g::match_msgs::snapshot::event;
    let stats = decoded
        .statistics
        .as_ref()
        .map(|statistics| {
            statistics
                .players
                .iter()
                .map(|player| {
                    let sum =
                        |e: usize| player.events[e].iter().map(|&n| u32::from(n)).sum::<u32>();
                    (
                        sum(event::KILL),
                        sum(event::DEATH),
                        u32::from(player.killstreak),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let rules = decoded.rules.as_ref();
    let points = rules.and_then(|rules| {
        rules
            .gather
            .as_ref()
            .map(|gather| gather.team_points.map(u32::from))
            .or_else(|| rules.tdm.map(|tdm| tdm.tickets.map(u32::from)))
    });
    let gather = rules.and_then(|rules| rules.gather.as_ref());
    let batteries = gather
        .map(|gather| {
            gather
                .batteries
                .iter()
                .map(|battery| match battery {
                    Battery::Carried => BatteryState::Carried(None),
                    Battery::InContainer(id) => BatteryState::Home(*id),
                    // As written; the bots settle its frame (`Bots::facts`).
                    Battery::Free { position, .. } => {
                        BatteryState::Free(Vec3::from_array(*position))
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    // The client's HUD of each lying battery (`m_spotted_mask`).
    let marks = gather
        .map(|gather| {
            gather
                .batteries
                .iter()
                .map(|battery| match battery {
                    Battery::Free { spotted_mask, .. } => *spotted_mask,
                    Battery::Carried | Battery::InContainer(_) => 0,
                })
                .collect()
        })
        .unwrap_or_default();
    let container_counts = gather
        .map(|gather| gather.containers.iter().map(|c| c.items.len()).collect())
        .unwrap_or_default();
    WorldFacts {
        time,
        stats,
        points,
        batteries,
        marks,
        container_counts,
        can_respawn: rules.map(|rules| rules.team_respawn),
        next_battery: gather
            .and_then(|gather| gather.timed.as_ref())
            .filter(|timed| timed.started)
            .map(|timed| timed.next_spawn),
    }
}

impl Game {
    /// The server's bots' places, by place: each waits to enter.
    pub(super) fn new_bot_places(formed: &Formed) -> Vec<Option<BotPlace>> {
        formed
            .places
            .iter()
            .map(|place| {
                place.driven.then(|| BotPlace {
                    seat: BotSeat::Waiting,
                    drive: Drive::new(drive::stage_0()),
                })
            })
            .collect()
    }

    /// A place a late joiner takes: never one of the server's bots.
    pub(super) fn push_driven_place(&mut self, team: u8) {
        self.bot_places.push(None);
        if let Some(walkers) = self.walkers.as_mut() {
            walkers.bots.push_place(team);
        }
    }

    /// Whether the place's last roster change was its leave (a stand-in's,
    /// `start`, is none of the place's own).
    pub(super) fn has_left(&self, index: usize) -> bool {
        self.players_events()
            .filter(|(_, who, _)| *who == index)
            .last()
            .is_some_and(|(_, _, entered)| !entered)
    }

    /// Whether the server's bots have their map's tactics (built on a
    /// thread of their own), for the tests.
    #[cfg(test)]
    pub(crate) fn bots_have_map(&self) -> bool {
        self.walkers
            .as_ref()
            .is_some_and(|walkers| walkers.bots.has_map())
    }

    /// How often to ask for a world, `every` being what the match asks:
    /// sooner while the server's bots play.
    pub(super) fn driven_refresh(&self, every: Duration) -> Duration {
        let Some(walkers) = self.walkers.as_ref().filter(|walkers| !walkers.absent) else {
            return every;
        };
        if walkers.bots.need_world() {
            every.min(WAITING_REFRESH)
        } else if walkers.bots.any() {
            every.min(PLAYING_REFRESH)
        } else {
            every
        }
    }

    /// A human's input as relayed: the bots hear it and track the body.
    pub(super) fn observe_input(&mut self, index: usize, time: u32, input: &PlayerInput) {
        if let Some(walkers) = self.walkers.as_mut() {
            walkers.bots.input(index, time, input);
        }
    }

    /// A trusted world: every player's body, and the statistics and
    /// batteries, for the bots.
    pub(super) fn observe_driven(&mut self, snapshot: &Snapshot, decoded: &Decoded) {
        let id = self.id;
        let Some(walkers) = self.walkers.as_mut() else {
            return;
        };
        let Some((blocks, _)) = players::walk(&snapshot.body) else {
            return;
        };
        for block in blocks {
            let place = usize::from(block.place);
            let Some((_, motion)) = Motion::find(block.state, snapshot.time) else {
                log::debug!(
                    "Match {id:#x}: player {place}'s body is not read in the world at {} ms",
                    snapshot.time
                );
                continue;
            };
            let fix = Fix {
                time: snapshot.time,
                alive: motion.alive,
                inserted: motion.inserted_time_ms,
                feet: motion
                    .body
                    .as_ref()
                    .map(|body| Vec3::from_array(body.feet())),
                yaw: motion.rotation_y,
                pitch: motion.look_pitch,
                keys: motion.actions_mask,
                speed: motion
                    .damage
                    .as_ref()
                    .map(|damage| damage.linear_horizontal_speed),
            };
            let health = motion
                .damage
                .as_ref()
                .and_then(|damage| damage.part_health.first().copied());
            let notes = walkers.bots.fixed(place, fix, health);
            log_notes(id, notes);
        }
        let notes = walkers.bots.facts(world_facts(snapshot.time, decoded));
        log_notes(id, notes);
        let (read, missed) = walkers
            .bots
            .anomalies_from_world(decoded.sections.objects, snapshot.time);
        if read + missed > 0 {
            log::debug!(
                "Match {id:#x}: anomalies in the world at {} ms: {read} read, {missed} missed",
                snapshot.time
            );
        }
    }

    /// The match's clock as the bots need it: the time left in play.
    fn bots_clock(&self, clock: u32) -> Clock {
        let total_ms = self.rule.params.match_ms;
        let left_ms = match self.rule.start {
            Some(start) if self.rule.state == rules::state::IN_PROCESS => {
                total_ms.saturating_sub(clock.saturating_sub(start))
            }
            _ => total_ms,
        };
        Clock {
            clock,
            time_left_s: left_ms as f32 / 1000.0,
            total_s: total_ms as f32 / 1000.0,
            playing: self.rule.state == rules::state::IN_PROCESS
                && self.rule.start.is_some_and(|start| clock >= start),
        }
    }

    /// Enter the server's bots once somebody simulates, and relay the
    /// inputs they have due.
    pub(super) fn poll_driven(&mut self, now: Instant) {
        if self.clock_start.is_none()
            || self.finished_at.is_some()
            || self.ending.is_some()
            || self.mode_end_at.is_some()
        {
            return;
        }
        let clock = self.clock(now);
        let mut due = Vec::new();
        for index in 0..self.bot_places.len() {
            let Some(seat) = self.bot_places[index].as_ref().map(|place| place.seat) else {
                continue;
            };
            match seat {
                BotSeat::Waiting => {
                    if self.live() > 0 {
                        let at = self.bot_enters(index, grid_up(clock + ROSTER_LEAD_MS), now);
                        log::info!(
                            "Match {:#x}: player {index} (the server's bot) entered at {at} ms",
                            self.id
                        );
                    }
                    continue;
                }
                BotSeat::Out { .. } => continue,
                BotSeat::In { .. } => {}
            }
            if self.walkers.as_ref().is_some_and(|walkers| !walkers.absent) {
                continue;
            }
            if let Some(place) = self.bot_places[index].as_mut() {
                for sent in place.drive.due(clock) {
                    if let Some(part) = sent.what {
                        log::debug!(
                            "Match {:#x}: player {index} (the server's bot) at {} ms: {part}",
                            self.id,
                            sent.time
                        );
                    }
                    due.push((index, sent.time, Relayed::Input(sent.input)));
                }
            }
        }
        let id = self.id;
        let bots_clock = self.bots_clock(clock);
        if let Some(walkers) = self.walkers.as_mut() {
            // Every entry and leave since the last poll, the humans' too.
            for &(at, who, entered) in &self.events[walkers.events_seen..] {
                walkers.bots.entered(who, entered, at);
            }
            walkers.events_seen = self.events.len();
            walkers.world(id);
            if !walkers.absent {
                let (made, notes) = walkers.bots.poll(bots_clock);
                log_notes(id, notes);
                due.extend(
                    made.into_iter()
                        .map(|made| (made.place, made.time, Relayed::Input(made.input))),
                );
                // A kit choice, as a player's 0xDE is relayed (0xF3): at
                // a tick ahead of the clock as its inputs are; it takes
                // effect at the place's next spawn.
                for (index, kit) in walkers.bots.take_kit_choices() {
                    let time = (clock / TICK_MS + 2) * TICK_MS;
                    due.push((
                        index,
                        time,
                        Relayed::Profile {
                            tick: tick_byte(time),
                            value: kit,
                        },
                    ));
                }
            }
        }
        for (index, time, what) in due {
            if matches!(what, Relayed::Profile { .. }) {
                self.note_kit_choice(index, time);
            }
            self.relay_to_others(index, &what, time, now);
            self.inputs.push((time, index, what));
        }
        // After the inputs made: one that gives way now makes none after
        // its drop.
        self.room_bots(now);
    }

    /// The bot at `index` enters at `at` (or the tick the roster change
    /// takes, after the stand-ins'), its script starting anew: its entry's
    /// tick.
    fn bot_enters(&mut self, index: usize, at: u32, now: Instant) -> u32 {
        let at = self.roster_change(None, index, true, at, now);
        if let Some(place) = self.bot_places[index].as_mut()
            && let Some(seat) = place.seat.after(BotEvent::Enters { at })
        {
            place.seat = seat;
            place.drive = Drive::new(drive::stage_0());
            place.drive.entered(at);
        }
        at
    }

    /// A room's match: each team keeps the size it was made with, players
    /// and bots together (the top of this file): a bot gives its place up
    /// to a player more, or comes back for a player fewer. One change a
    /// team at a time, once all its bots have entered.
    fn room_bots(&mut self, now: Instant) {
        if self.formed.room_minimum.is_none()
            || self.result_made
            || !self.bot_places.iter().any(Option::is_some)
        {
            return;
        }
        let seats: Vec<Option<BotSeat>> = self
            .bot_places
            .iter()
            .map(|place| place.as_ref().map(|place| place.seat))
            .collect();
        if seats.contains(&Some(BotSeat::Waiting)) {
            return;
        }
        // Each place's players' roster changes: in the match now.
        let mut in_match = vec![false; self.players()];
        for &(_, who, entered) in self.players_events() {
            if let Some(slot) = in_match.get_mut(who) {
                *slot = entered;
            }
        }
        let initial = self.formed.initial_players;
        let places_left = MAX_PLAYERS.saturating_sub(self.players());
        let mut bots_in_match = seats
            .iter()
            .filter(|seat| matches!(seat, Some(BotSeat::In { .. })))
            .count();
        for team in 0..2_u8 {
            let of_team = |index: &usize| self.formed.places[*index].team.min(1) == team;
            let bots: Vec<usize> = (0..self.players())
                .filter(of_team)
                .filter(|&index| seats[index].is_some())
                .collect();
            let bots_in: Vec<usize> = bots
                .iter()
                .copied()
                .filter(|&index| matches!(seats[index], Some(BotSeat::In { .. })))
                .collect();
            let shape = TeamBots {
                fill: (0..initial).filter(of_team).count(),
                places: bots.len(),
                players: (0..self.players())
                    .filter(of_team)
                    .filter(|&index| seats[index].is_none() && in_match[index])
                    .count(),
                bots: bots_in.len(),
            };
            match bot_move(shape, places_left, bots_in_match) {
                BotMove::Stay => {}
                BotMove::GiveWay => {
                    if !self.team_respawns(team) {
                        continue;
                    }
                    if let Some(&bot) = bots_in.iter().rev().find(|&&bot| self.bot_settled(bot)) {
                        self.bot_gives_way(bot, now);
                        bots_in_match -= 1;
                    }
                }
                BotMove::ComeBack => {
                    let back = bots.iter().copied().find(|&bot| {
                        matches!(seats[bot], Some(BotSeat::Out { .. })) && self.bot_settled(bot)
                    });
                    if let Some(bot) = back {
                        let last = self.events.iter().map(|(at, _, _)| *at).max().unwrap_or(0);
                        let at = grid_up(self.clock(now) + ROSTER_LEAD_MS).max(last + TICK_MS);
                        let at = self.bot_enters(bot, at, now);
                        bots_in_match += 1;
                        log::info!(
                            "Match {:#x}: player {bot} (the server's bot) comes back at {at} ms: \
                             team {team} has {} of its {} players and bots",
                            self.id,
                            shape.players + shape.bots,
                            shape.fill
                        );
                        let note = serde_json::json!({
                            "note": "bot back",
                            "place": bot,
                            "match_ms": at,
                        });
                        self.record_note(note, now);
                    }
                }
            }
        }
    }

    /// Whether `team` can still respawn (Team Deathmatch's tickets, as the
    /// match's trusted world has them; any other mode, or no world yet:
    /// yes).
    fn team_respawns(&self, team: u8) -> bool {
        self.state
            .as_ref()
            .and_then(|state| state.rules.as_ref())
            .is_none_or(|rules| rules.team_respawn[usize::from(team.min(1))])
    }

    /// Whether every game simulating has played the bot's last roster
    /// change (its report is of a tick at or past it): only then may its
    /// place change again, or a game would drop the change.
    fn bot_settled(&self, bot: usize) -> bool {
        let Some(since) = self.bot_places[bot]
            .as_ref()
            .and_then(|place| place.seat.since())
        else {
            return false;
        };
        (0..self.peers.len()).all(|index| {
            self.is_bot(index)
                || self.peers[index].as_ref().is_none_or(|peer| {
                    peer.stage != Stage::Live || peer.reported_tick.is_some_and(|tick| tick >= since)
                })
        })
    }

    /// The bot gives its place up to a player of its team: drop pressed
    /// (a carried battery let go) and then every key let go, after its last
    /// input, and its leave after them and after every roster change sent
    /// (the player's entry).
    fn bot_gives_way(&mut self, bot: usize, now: Instant) {
        let clock = self.clock(now);
        let last_input = self
            .inputs
            .iter()
            .filter(|(_, who, what)| *who == bot && matches!(what, Relayed::Input(_)))
            .map(|(time, _, _)| *time)
            .max();
        let drop_at = grid_up(clock + TICK_MS).max(last_input.map_or(0, |time| time + TICK_MS));
        for (time, actions) in [(drop_at, drive::key::DROP), (drop_at + TICK_MS, 0)] {
            let input = PlayerInput {
                time: WireTime::tick(time),
                rotation: [0; 4],
                actions,
            };
            self.relay_to_others(bot, &Relayed::Input(input), time, now);
            self.inputs.push((time, bot, Relayed::Input(input)));
        }
        let last = self.events.iter().map(|(at, _, _)| *at).max().unwrap_or(0);
        let at = grid_up(clock + ROSTER_LEAD_MS)
            .max(drop_at + 2 * TICK_MS)
            .max(last + TICK_MS);
        let at = self.roster_change(None, bot, false, at, now);
        if let Some(place) = self.bot_places[bot].as_mut()
            && let Some(seat) = place.seat.after(BotEvent::Leaves { at })
        {
            place.seat = seat;
        }
        log::info!(
            "Match {:#x}: player {bot} (the server's bot) gives its place up to a player of its \
             team: drop at {drop_at} ms, leaves at {at} ms",
            self.id
        );
        let note = serde_json::json!({
            "note": "gave way",
            "place": bot,
            "match_ms": at,
        });
        self.record_note(note, now);
    }
}
