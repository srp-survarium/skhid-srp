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
//! - it never times out and never leaves; a match with only bots in it is
//!   over after [`BOTS_ALONE`] (`check_bots_alone`). Its match takes no
//!   late joiner: its teams are fixed ("Player vs Bots", the lobby's `pvb`;
//!   no PvP match has the server's bots since the owner's of 2026-10-17).
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
        let roles: Vec<Option<&str>> = formed
            .profiles
            .iter()
            .map(|profile| survarium::v026g::loadouts::bot_role_of(profile).map(|role| role.name()))
            .collect();
        log::info!("Match {id:#x}: the bots' roles by place {roles:?}");
        let weapons = formed.profiles.iter().map(weapon_of).collect();
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

/// A place's weapon 1 kind from its profile: a bot's by its role, a
/// player's by the item in slot 7 (its dictionary category and kind).
fn weapon_of(
    profile: &survarium::v026g::match_msgs::static_info::MatchProfile,
) -> Option<WeaponKind> {
    if let Some(role) = survarium::v026g::loadouts::bot_role_of(profile) {
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
    /// The scripts of the server's bots, by place.
    pub(super) fn new_drives(formed: &Formed) -> Vec<Option<Drive>> {
        formed
            .places
            .iter()
            .map(|place| place.driven.then(|| Drive::new(drive::stage_0())))
            .collect()
    }

    /// A place a late joiner takes: never one of the server's bots.
    pub(super) fn push_driven_place(&mut self, team: u8) {
        self.drives.push(None);
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
        for index in 0..self.drives.len() {
            if self.drives[index].is_none() {
                continue;
            }
            let entered = self
                .events
                .iter()
                .any(|(_, who, entered)| *who == index && *entered);
            if !entered {
                if self.live() == 0 {
                    continue;
                }
                let at = grid_up(clock + ROSTER_LEAD_MS);
                let at = self.roster_change(None, index, true, at, now);
                if let Some(drive) = self.drives[index].as_mut() {
                    drive.entered(at);
                }
                log::info!(
                    "Match {:#x}: player {index} (the server's bot) entered at {at} ms",
                    self.id
                );
                continue;
            }
            if self.walkers.as_ref().is_some_and(|walkers| !walkers.absent) {
                continue;
            }
            if let Some(drive) = self.drives[index].as_mut() {
                for sent in drive.due(clock) {
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
    }
}
