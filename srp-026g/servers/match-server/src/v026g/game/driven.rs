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
//!   the stage 0 script instead. `SRP_BOT_TUNING` (a JSON merge patch over
//!   the defaults, read once when the server starts: a file it cannot use stops the server) changes their numbers; `SRP_BOT_TRACE_DIR` keeps
//!   their decisions (`SRP_BOT_TRACE_MAX_MB` of them); `bot-routes.json` in
//!   the mapdata folder, or the file `SRP_BOT_ROUTES` names (a JSON file,
//!   private: made from the players' saved worlds; empty or `off`: none),
//!   gives them the routes humans took (`bots/routes.rs`);
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
use crate::v026g::bots::motor::Hurt;
use crate::v026g::bots::objective::Mode;
use crate::v026g::bots::pools::Pools;
use crate::v026g::bots::reflex::WeaponKind;
use crate::v026g::bots::foliage::{self, Foliage};
use crate::v026g::bots::grass::{self, Grass};
use crate::v026g::bots::live_grenades::{Player, Throws};
use crate::v026g::bots::routes::{self, RouteTable};
use crate::v026g::bots::switch_log;
use crate::v026g::bots::tactics::Tactics;
use crate::v026g::bots::trace::Trace;
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
    pub(super) bots: Bots,
    /// The worlds name the kit each place is in (`AvoidTuning::kit_in_use`).
    kit_in_use: bool,
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
    /// build starts now, with the tuning the server was started on. The bots
    /// run on the tuning taken now (`SRP_BOT_TUNING` read again, with the
    /// admin page's changes: [`crate::v026g::bots::tuning::LiveTuning::for_match`]).
    pub(super) fn new(formed: &Formed, settings: &Settings) -> Option<Self> {
        let maps = settings.maps.clone()?;
        let driven: Vec<bool> = formed.places.iter().map(|place| place.driven).collect();
        if !driven.iter().any(|&driven| driven) {
            return None;
        }
        let id = formed.match_id;
        let mode = match Mode::parse(formed.map.mode) {
            Ok(mode) => mode,
            Err(error) => {
                log::warn!("Match {id:#x}: no bots: {error}");
                return None;
            }
        };
        let loaded = settings.bot_tuning.for_match();
        let tuning = loaded.tuning().clone();
        let hash = tuning.hash();
        let trace = match std::env::var_os("SRP_BOT_TRACE_DIR").filter(|dir| !dir.is_empty()) {
            Some(dir) => {
                let max_mb = crate::v026g::parse_number(
                    "SRP_BOT_TRACE_MAX_MB",
                    std::env::var("SRP_BOT_TRACE_MAX_MB").ok().as_deref(),
                    crate::v026g::bots::trace::DEFAULT_MAX_MB,
                );
                Trace::open(dir.as_ref(), id, hash, u64::from(max_mb) << 20)
            }
            None => Trace::none(hash),
        };
        log::info!("Match {id:#x}: the bots run on tuning {hash:08x}");
        // What is on and off, and the data it runs on: one block per match (the report's lines), never per tick.
        for line in tuning.switch_report().lines() {
            log::info!("Match {id:#x}: {line}");
        }
        log::info!(
            "Match {id:#x}: {}",
            switch_log::data_files(
                &loaded.data_line(),
                std::env::var_os("SRP_BOT_ROUTES").as_deref(),
                std::env::var_os("SRP_BOT_FOLIAGE").as_deref(),
                std::env::var_os("SRP_BOT_GRASS").as_deref(),
                maps.dir(),
            )
        );
        log::info!("Match {id:#x}: {}", switch_log::env_switches());
        let project = formed.map.project;
        maps.request(project);
        let teams: Vec<u8> = formed.places.iter().map(|place| place.team).collect();
        let bots: Vec<_> = formed
            .profiles
            .iter()
            .map(survarium::v026g::loadouts::bot_equipment_of)
            .collect();
        let roles: Vec<Option<&str>> = bots
            .iter()
            .map(|bot| bot.map(|(_, role)| role.name()))
            .collect();
        let equipments: Vec<_> = bots
            .iter()
            .map(|bot| bot.map(|(equipment, _)| equipment))
            .collect();
        log::info!(
            "Match {id:#x}: the bots' roles by place {roles:?}, equipment {:?}",
            equipments
                .iter()
                .map(|equipment| equipment.map(|equipment| {
                    format!("{} {}", equipment.tier.name(), equipment.rusher.name())
                }))
                .collect::<Vec<_>>()
        );
        let by_data = tuning.aim.combat.fire.weapon_class_by_data;
        let weapons = formed
            .profiles
            .iter()
            .map(|profile| weapon_of(profile, by_data))
            .collect();
        let all_kits = tuning.avoid.kit_in_use;
        let throws = formed
            .profiles
            .iter()
            .zip(&driven)
            .map(|(profile, &driven)| Throws::of_profile(profile, Player::of(driven), all_kits))
            .collect();
        let pantries = formed.profiles.iter().map(crate::v026g::bots::stock::Pantry::of_profile).collect();
        let reach: Vec<Option<f32>> = formed.profiles.iter().map(shot_reach_of).collect();
        let armours: Vec<_> = formed.profiles.iter().map(zone_armour_of).collect();
        // The bodies' speed by their first kit's skills and weight, until a
        // world shows a life's own.
        let speed_levels: Vec<Option<f32>> = formed
            .profiles
            .iter()
            .map(|profile| survarium::v026g::bot_skills::profile_movement_level(profile, 0))
            .collect();
        Some(Self {
            bots: Bots::new_with(
                id,
                mode,
                formed.map.level,
                &teams,
                &driven,
                &roles,
                crate::v026g::bots::tuning::TeamTunings::new(tuning),
                trace,
            )
            .with_container_teams(formed.map.container_teams)
            .with_tiers(&equipments),
            maps,
            project,
            world: None,
            tactics: None,
            absent: false,
            waiting: None,
            events_seen: 0,
            kit_in_use: all_kits,
        })
        .map(|mut walkers| {
            walkers.bots.set_weapons(weapons);
            walkers.bots.set_throws(throws);
            walkers.bots.set_pantries(pantries);
            walkers.bots.set_shot_reach(reach);
            for (place, armour) in armours.into_iter().enumerate() {
                walkers.bots.set_zone_armour(place, armour);
            }
            walkers.bots.set_speed_levels(&speed_levels);
            // Play 17: the routes humans took (a private table; without it
            // the bots roam to the exits).
            let var = std::env::var_os("SRP_BOT_ROUTES");
            if let Some(table) = route_table(id, var.as_deref(), walkers.maps.dir()) {
                walkers.bots.set_routes(&table);
            }
            // The bushes a sight line crosses (a private file; without it no bush hides anyone).
            let var = std::env::var_os("SRP_BOT_FOLIAGE");
            if let Some(foliage) = foliage_of(id, project, var.as_deref(), walkers.maps.dir()) {
                walkers.bots.set_foliage(foliage);
            }
            // The grass a sight line crosses (the same: a private file, none without it).
            let var = std::env::var_os("SRP_BOT_GRASS");
            if let Some(grass) = grass_of(id, project, var.as_deref(), walkers.maps.dir()) {
                walkers.bots.set_grass(grass);
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
                    let margin = self.bots.match_wide_tuning().hazard.margin_m.get();
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

/// A place's weapon 1 kind from its profile: a bot's by its role, but the
/// rusher's, whose weapon is whatever kit it wears
/// (`loadouts::RusherKit`: an SMG or the SPAS-12), and a player's, by the
/// item in slot 7 (its dictionary category and kind). With `by_data`
/// (`FireTuning::weapon_class_by_data`) every place's is the kind of the item
/// in slot 7 of its first kit by the game data's class, a bot's too: it is
/// the kind the other bots take it for (its own follows the weapon it holds).
fn weapon_of(
    profile: &survarium::v026g::match_msgs::static_info::MatchProfile,
    by_data: bool,
) -> Option<WeaponKind> {
    use survarium::v026g::loadouts::BotRole;
    let role = survarium::v026g::loadouts::bot_role_of(profile);
    if let Some(role) = role.filter(|role| *role != BotRole::Rusher && !by_data) {
        return WeaponKind::of_role(role.name());
    }
    weapon_in_slot_7(profile, by_data)
        .or_else(|| role.and_then(|role| WeaponKind::of_role(role.name())))
}

/// The kind of the item in slot 7 of the profile's first kit.
fn weapon_in_slot_7(
    profile: &survarium::v026g::match_msgs::static_info::MatchProfile,
    by_data: bool,
) -> Option<WeaponKind> {
    let kit = profile.kits.first()?;
    let (index, _) = kit.slots[7]?;
    let item = profile.items.get(usize::from(index))?;
    if by_data {
        let weapon = survarium::v026g::weapons::weapon(item.dict_id)?;
        return Some(WeaponKind::of_class(
            crate::v026g::bots::guns::WeaponClass::of(weapon),
        ));
    }
    let kind = survarium::v026g::item_params::item_params(item.dict_id)?.kind;
    let category = survarium::v026g::dictionary::Dictionary::shared()
        .item(item.dict_id)?
        .category;
    if WeaponKind::by_fallback(kind, category) {
        log::info!(
            "weapon type {kind} (category {category}) is none the bots know: its band is its category's"
        );
    }
    WeaponKind::of_item(kind, category)
}

/// How far a shot of a place's weapon 1 (the item in slot 7 of its first
/// kit) is heard, metres: the game's sound data per weapon config
/// ([`crate::v026g::bots::shot_reach`]); None where the item is not in the
/// table.
fn shot_reach_of(profile: &survarium::v026g::match_msgs::static_info::MatchProfile) -> Option<f32> {
    let kit = profile.kits.first()?;
    let (index, _) = kit.slots[7]?;
    let item = profile.items.get(usize::from(index))?;
    let dictionary = survarium::v026g::dictionary::Dictionary::shared();
    crate::v026g::bots::shot_reach::reach_of(&dictionary.item(item.dict_id)?.cfg_name)
}

/// What a bot wears against a zone's hits (F2, `zone_cost.rs`): the armour among the items of its first kit,
/// as `HazardTuning::zone_pools` reads it.
fn zone_armour_of(
    profile: &survarium::v026g::match_msgs::static_info::MatchProfile,
) -> crate::v026g::bots::zone_cost::ZoneArmour {
    let ids = profile
        .kits
        .first()
        .into_iter()
        .flat_map(|kit| kit.slots.iter().flatten())
        .filter_map(|&(index, _)| profile.items.get(usize::from(index)).map(|item| item.dict_id));
    crate::v026g::bots::zone_cost::ZoneArmour::of_items(ids)
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

    /// What the server's bot at `place` counts of its drugs, for the tests.
    #[cfg(test)]
    pub(crate) fn driven_pantry(&self, place: usize) -> Option<&crate::v026g::bots::stock::Pantry> {
        self.walkers.as_ref()?.bots.pantry_of(place)
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

    /// A human's input as relayed, which reached the server `late_ms` after
    /// its tick: the bots hear it and track the body.
    pub(super) fn observe_input(
        &mut self,
        index: usize,
        time: u32,
        late_ms: u32,
        input: &PlayerInput,
    ) {
        if let Some(walkers) = self.walkers.as_mut() {
            walkers.bots.input(index, time, late_ms, input);
        }
    }

    /// TEST ONLY: the server's bots.
    #[cfg(test)]
    pub(crate) fn driven_bots(&self) -> Option<&Bots> {
        self.walkers.as_ref().map(|walkers| &walkers.bots)
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
            if let Some(damage) = motion.damage.as_ref() {
                let legs = [6, 7].map(|part| damage.part_health.get(part).copied());
                let hurt = Hurt::of(legs, damage.last_hit_time_ms, snapshot.time);
                walkers.bots.hurt(place, hurt);
                if let Some(pools) = Pools::of_world(&damage.part_health) {
                    walkers.bots.pools(place, pools);
                }
                walkers.bots.bullet_hit(place, damage.last_hit_time_ms);
            }
            let notes = walkers.bots.fixed(place, fix, health);
            log_notes(id, notes);
        }
        let notes = walkers.bots.facts(world_facts(snapshot.time, decoded));
        log_notes(id, notes);
        walkers
            .bots
            .traps_from_world(decoded.sections.objects, snapshot.time);
        walkers
            .bots
            .grenades_from_world(decoded.sections.objects, snapshot.time);
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

    /// A trusted world's word on what the server's bots hold of drugs:
    /// each bot's block as `players::Supplies` read it (`Observed`), the
    /// quick-use stacks of its kits. Called once `observe` has them.
    pub(super) fn observe_stock(&mut self, time: u32) {
        let Some(walkers) = self.walkers.as_mut() else {
            return;
        };
        let Some(state) = self.state.as_ref() else {
            return;
        };
        let dictionary = survarium::v026g::dictionary::Dictionary::shared();
        for (place, supplies) in state.supplies.iter().enumerate() {
            // The kit a place is in, for its throw keys' pockets: a human's too.
            if let (Some(supplies), true) = (supplies, walkers.kit_in_use) {
                walkers.bots.kit_in_use(place, usize::from(supplies.kit));
            }
            let driven = self.formed.places.get(place).is_some_and(|p| p.driven);
            if let (true, Some(supplies), Some(profile)) =
                (driven, supplies, self.formed.profiles.get(place))
            {
                let readings = crate::v026g::bots::stock::readings(supplies, profile, dictionary);
                walkers.bots.stock(place, time, &readings);
            }
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
        #[cfg(feature = "client-test")]
        let mut says: Vec<String> = Vec::new();
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
                #[cfg(feature = "client-test")]
                says.extend(walkers.bots.client_test_says());
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
        // TEST ONLY: the client tests' lines for the owner (the human of the match) go to his game's chat.
        #[cfg(feature = "client-test")]
        if let Some(owner) = self.formed.places.iter().find(|place| !place.driven && !place.bot) {
            for line in &says {
                crate::v026g::bots::client_tests::say(owner.account_id, line);
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

/// The route table a match's bots get (play 17): `var` is `SRP_BOT_ROUTES`,
/// `mapdata` the mapdata folder ([`routes::Source::choose`]). A file that
/// fails to load: a warning; no file: one info line until a restart.
fn route_table(
    id: u32,
    var: Option<&std::ffi::OsStr>,
    mapdata: Option<&std::path::Path>,
) -> Option<RouteTable> {
    let (path, named) = match routes::Source::choose(var, mapdata) {
        routes::Source::Set(path) => (path, "SRP_BOT_ROUTES"),
        routes::Source::Default(path) => (path, "bot routes"),
        routes::Source::Off => return None,
        routes::Source::Missing => {
            static SAID: std::sync::Once = std::sync::Once::new();
            SAID.call_once(|| {
                log::info!(
                    "No bot routes: SRP_BOT_ROUTES unset and no {} in the mapdata folder; the bots roam to the exits",
                    routes::DEFAULT_FILE
                )
            });
            return None;
        }
    };
    match RouteTable::load(&path) {
        Ok(table) => Some(table),
        Err(why) => {
            log::warn!("Match {id:#x}: {named} not used ({why})");
            None
        }
    }
}

/// The bushes of `project` a match's bots get ([`foliage::Source::choose`]): `var` is
/// `SRP_BOT_FOLIAGE`, `mapdata` the mapdata folder. A file that fails to load: a warning.
fn foliage_of(
    id: u32,
    project: &str,
    var: Option<&std::ffi::OsStr>,
    mapdata: Option<&std::path::Path>,
) -> Option<Foliage> {
    let path = match foliage::Source::choose(var, mapdata) {
        foliage::Source::Set(path) | foliage::Source::Default(path) => path,
        foliage::Source::None => return None,
    };
    match Foliage::load(&path, project) {
        Ok(foliage) => Some(foliage),
        Err(why) => {
            log::warn!("Match {id:#x}: the bot foliage not used ({why})");
            None
        }
    }
}

/// The grass of `project`'s level a match's bots get ([`grass::Source::choose_file`]): `var` is
/// `SRP_BOT_GRASS`, `mapdata` the mapdata folder. A file that fails to load: a warning.
fn grass_of(
    id: u32,
    project: &str,
    var: Option<&std::ffi::OsStr>,
    mapdata: Option<&std::path::Path>,
) -> Option<Grass> {
    let path = match grass::Source::choose_file(var, mapdata, grass::DEFAULT_FILE) {
        grass::Source::Set(path) | grass::Source::Default(path) => path,
        grass::Source::None => return None,
    };
    match Grass::load(&path, project) {
        Ok(grass) => Some(grass),
        Err(why) => {
            log::warn!("Match {id:#x}: the bot grass not used ({why})");
            None
        }
    }
}

/// TEST ONLY (`bots/jump_test.rs`, cargo feature `jump-test`): the jump test's reach into the match.
#[cfg(feature = "jump-test")]
impl Game {
    /// Whether tries remain: the match does not end.
    pub(super) fn jump_test_holds_end(&self) -> bool {
        self.walkers
            .as_ref()
            .and_then(|walkers| walkers.bots.jump_test.as_ref())
            .is_some_and(|test| test.holds_end())
    }

    /// A world for the test's burst, asked of the best-linked patched client when the test wants one.
    pub(super) fn jump_test_burst(&mut self, now: Instant) {
        let clock = self.clock(now);
        let wants = self
            .walkers
            .as_mut()
            .and_then(|walkers| walkers.bots.jump_test.as_mut())
            .is_some_and(|test| test.wants_world(clock));
        if !wants {
            return;
        }
        let Some(&index) = self.sources(usize::MAX).first() else {
            return;
        };
        if let Some(peer) = self.peers[index].as_mut() {
            send(peer, &ServerMessage::UploadRequest);
            peer.asked = Some(now);
            peer.asked_clock = Some(clock);
            peer.unanswered_since.get_or_insert(now);
        }
    }

    /// An upload came: its rows are logged; true when it answered a burst request.
    pub(super) fn jump_test_upload(&mut self, time: u32, clock: u32, bytes: usize, body: &[u8]) -> bool {
        let id = self.id;
        let Some(walkers) = self.walkers.as_mut() else {
            return false;
        };
        let asked = walkers.bots.jump_test.as_mut().is_some_and(|test| test.upload(time, clock, bytes, body));
        if asked {
            log_notes(id, walkers.bots.fix_from_burst());
        }
        asked
    }
}

#[cfg(test)]
mod tests {
    use crate::v026g::bots::live_grenades::KitPockets;
    use super::*;
    use survarium::v026g::loadouts::{BotEquipment, BotMask, BotRole, BotTier, RusherKit, bot_profile};

    /// What the other bots believe of a bot's weapon: its role's, but the
    /// rusher's follows the kit it wears (the SMG is an SMG, the SPAS-12 a
    /// shotgun), and the lowest tier's rusher keeps the shotgun it has.
    #[test]
    fn the_rushers_weapon_kind_follows_its_kit() {
        let kind =
            |tier, rusher, role| weapon_of(&bot_profile(1, 0, role, BotEquipment { tier, rusher, mask: BotMask::Table }), false);
        for kit in RusherKit::ALL {
            let shotgun = !matches!(kit.worn_in(BotTier::Mid), RusherKit::Smg | RusherKit::Alt);
            let wanted = if shotgun {
                WeaponKind::Shotgun
            } else {
                WeaponKind::Smg
            };
            assert_eq!(
                kind(BotTier::Mid, kit, BotRole::Rusher),
                Some(wanted),
                "{}",
                kit.name()
            );
            // The others by their role, whatever the rusher wears.
            assert_eq!(
                kind(BotTier::Mid, kit, BotRole::Flanker),
                Some(WeaponKind::Smg)
            );
            assert_eq!(
                kind(BotTier::Mid, kit, BotRole::Support),
                Some(WeaponKind::Rifle)
            );
            assert_eq!(
                kind(BotTier::Mid, kit, BotRole::Camper),
                Some(WeaponKind::Sniper)
            );
        }
        assert_eq!(
            kind(BotTier::Low, RusherKit::Smg, BotRole::Rusher),
            Some(WeaponKind::Shotgun)
        );
    }

    /// The profile of a bot with the owner's kit `new` (or the old pockets) and the pocket store given, built explicitly
    /// (`bot_profile` follows the process's `SRP_BOT_KIT` and `SRP_BOT_POCKET_STORE`).
    fn profile_of(
        role: BotRole,
        tier: BotTier,
        new: bool,
        store: survarium::v026g::loadouts::PocketStore,
    ) -> survarium::v026g::match_msgs::static_info::MatchProfile {
        let equipment = BotEquipment { tier, rusher: RusherKit::Smg, mask: BotMask::Table };
        let kits = survarium::v026g::loadouts::bot_kits_in_stored(equipment, role, new, false, store);
        let kits = [&kits[0], &kits[1], &kits[2]];
        survarium::v026g::match_msgs::static_info::MatchProfile::from_loadouts(
            "bot",
            0,
            &kits,
            survarium::v026g::bot_skills::bot_modifiers(&kits),
            survarium::v026g::dictionary::Dictionary::shared(),
            1,
        )
    }

    /// How many grenades a life of the profile's first kit throws with the throw key, and in the next life.
    fn grenades_in_lives(profile: &survarium::v026g::match_msgs::static_info::MatchProfile) -> (u32, u32) {
        use crate::v026g::drive::key;
        let Throws::Bot(mut pockets) = Throws::of_profile(profile, Player::Bot, true) else {
            panic!("a bot's pockets");
        };
        let life = |pockets: &mut KitPockets| {
            let mut thrown = 0;
            while pockets.throw_grenade(key::GRENADE).is_some() && thrown < 10 {
                thrown += 1;
            }
            thrown
        };
        let first = life(&mut pockets);
        pockets.refill();
        (first, life(&mut pockets))
    }

    /// The pockets a bot's profile gives are its kit's, exactly: with the owner's kit only the support carries a grenade,
    /// and only with four pockets open (gear 13-15), the old pockets give every role two; and a new life has the grenade
    /// again exactly when the storage behind the slot has it (the store of 100, not the old count).
    #[test]
    fn a_bots_pockets_are_its_kits_and_a_new_life_follows_the_storage() {
        use survarium::v026g::loadouts::{self, BOT_GRENADE, PocketStore};
        let mut supports_with_one = 0;
        for role in BotRole::ALL {
            for tier in BotTier::ALL {
                // What the owner's kit puts in the pockets the armour opens (`loadouts::bot_pockets`).
                let equipment = BotEquipment { tier, rusher: RusherKit::Smg, mask: BotMask::Table };
                let kit = &loadouts::bot_kits_in_stored(equipment, role, true, false, PocketStore::PerLife)[0];
                let (open, _) = loadouts::pockets(kit);
                let expect_new: u32 = loadouts::bot_pockets(role, open)
                    .iter()
                    .filter(|item| item.name == BOT_GRENADE.name)
                    .map(|item| item.count)
                    .sum();
                assert!(expect_new == 0 || role == BotRole::Support, "{role:?} {tier:?}: only the support carries one");
                supports_with_one += u32::from(expect_new == 1);
                let at = format!("{role:?} {tier:?}");
                let stocked = profile_of(role, tier, true, PocketStore::PerLife);
                assert_eq!(grenades_in_lives(&stocked), (expect_new, expect_new), "{at}: owner's kit, store of 100");
                let matched = profile_of(role, tier, true, PocketStore::PerMatch);
                assert_eq!(grenades_in_lives(&matched), (expect_new, 0), "{at}: owner's kit, storage at the count");
                let old = profile_of(role, tier, false, PocketStore::PerLife);
                assert_eq!(grenades_in_lives(&old), (2, 2), "{at}: the old pockets");
                assert!(matches!(Throws::of_profile(&stocked, Player::Bot, false), Throws::Bot(_)), "{at}");
            }
        }
        assert!(supports_with_one > 0, "some support kit carries the one grenade");
    }

    /// A profile of a human with a kit change: the first kit has no frag, the second one; with `all_kits` off (the
    /// switch `avoid.kit_in_use`) only the first is read and a throw key lists nothing, as at c3dc7241.
    #[test]
    fn a_humans_second_kit_is_read_only_with_the_switch_on() {
        use crate::v026g::drive::key;
        use survarium::v026g::loadouts::{Loadout, POCKET_SLOTS};
        let dictionary = survarium::v026g::dictionary::Dictionary::shared();
        let with = |name: &str| Loadout::from_names(dictionary, [(POCKET_SLOTS[0], name)]).expect("a kit");
        let (first, second) = (with("medkit"), with("rgd-5"));
        let profile = survarium::v026g::match_msgs::static_info::MatchProfile::from_loadouts(
            "human",
            0,
            &[&first, &second],
            [0.0; survarium::v026g::skills::MODIFIERS],
            dictionary,
            1,
        );
        let Throws::Human(mut first_only) = Throws::of_profile(&profile, Player::Human, false) else {
            panic!("pockets");
        };
        assert!(first_only.throw_grenade(key::GRENADE).is_none());
        let Throws::Human(mut all) = Throws::of_profile(&profile, Player::Human, true) else {
            panic!("pockets");
        };
        assert!(all.throw_grenade(key::GRENADE).is_none(), "kit 1 in use: a medkit");
        let kit = all.kit(1).expect("the profile has a second kit");
        all.use_kit(kit);
        assert!(all.throw_grenade(key::GRENADE).is_some(), "kit 2 in use: the frag");
        assert!(all.kit(2).is_none());
    }

    /// On the live game data (`SRP_GAME_DATA`): the 13-15 rusher's P90 and
    /// Kriss are SMGs too. Run alone:
    /// `SRP_GAME_DATA=<server-game-data.db> cargo test -p match-server --lib
    /// -- --ignored on_the_live_data_the_rushers_smgs`
    #[test]
    #[ignore = "needs SRP_GAME_DATA, the server's game data"]
    fn on_the_live_data_the_rushers_smgs_are_smgs() {
        survarium::v026g::game_data::init().expect("SRP_GAME_DATA reads and passes srp's checks");
        for tier in [BotTier::Mid, BotTier::High] {
            for kit in [RusherKit::Smg, RusherKit::Alt] {
                let profile =
                    bot_profile(1, 0, BotRole::Rusher, BotEquipment { tier, rusher: kit, mask: BotMask::Table });
                assert_eq!(
                    weapon_in_slot_7(&profile, false),
                    Some(WeaponKind::Smg),
                    "{tier} {}",
                    kit.name()
                );
            }
            for kit in [RusherKit::Slug, RusherKit::Helmet, RusherKit::Control] {
                let profile =
                    bot_profile(1, 0, BotRole::Rusher, BotEquipment { tier, rusher: kit, mask: BotMask::Table });
                assert_eq!(
                    weapon_in_slot_7(&profile, false),
                    Some(WeaponKind::Shotgun),
                    "{tier} {}",
                    kit.name()
                );
            }
        }
    }

    /// With `SRP_BOT_ROUTES` unset a match's bots get the table in the
    /// mapdata folder; empty or `off`, none; a path, that file.
    #[test]
    fn a_match_takes_the_route_table_in_the_mapdata_folder() {
        let dir = std::env::temp_dir().join(format!("srp-driven-routes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join(routes::DEFAULT_FILE);
        std::fs::write(
            &file,
            r#"{"format": 2, "routes": [{"level": "level_03", "mode": 2, "team": 1,
                "lives": 5, "reward": 1.0, "fight": 1.0, "battery": 0.0, "kd": 1.0,
                "died": 0.2, "points": [[1.0, 2.0], [3.0, 4.0]]}]}"#,
        )
        .unwrap();
        let table = route_table(1, None, Some(&dir)).expect("the mapdata folder's table");
        let routes = table.for_map(3, 2);
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].points.len(), 2);
        assert!(route_table(1, Some("off".as_ref()), Some(&dir)).is_none());
        assert!(route_table(1, Some("".as_ref()), Some(&dir)).is_none());
        assert!(route_table(1, Some(file.as_os_str()), None).is_some_and(|t| t.len() == 1));
        assert!(route_table(1, Some(dir.join("none.json").as_os_str()), Some(&dir)).is_none());
        std::fs::remove_file(&file).unwrap();
        assert!(route_table(1, None, Some(&dir)).is_none());
        std::fs::remove_dir(&dir).unwrap();
    }
}
