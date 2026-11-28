//! `OnlineMatches` over `match-server`: the lobby's hook for online matches (docs/notes/online.md).
//!
//! [`Online`] owns the match hub (many matches behind one UDP port) and the worlds it may send. `create` builds the
//! `match_options` blob of the roster (`local_match::online_options`: the humans' real kits, the cached world's seed,
//! tick and send period), asks the hub for a match and returns the per-player tickets for `82`. Ended matches come back
//! as [`Ended`]. Production runs the hub on a UDP thread ([`Online::start_server`]); tests drive the hub directly.
//!
//! PvE (`create_pve`, docs/notes/pve.md): the world of the mission's project, options of the humans and the 17
//! script bots (`local_match::pve_options`), the bot records as server-owned slots of the relay (they enter with the
//! humans and get no input: idle until the server drives them), the G1 objects of all records.
//! The relay mirrors the mission script's world-free failure ends (`PveEndRule::PVE_LUA`: the pre-start cancel, the
//! 45-minute end, every human left) and reports them with winner 1 (the script's winner_team); a success or a death end
//! needs the world: the world host reports them (`take_outcomes`, match-server `host.rs`).

use std::collections::{BTreeSet, HashMap};
use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use game_data::{GameData, ModeRow};
use lobby_server::{BotFill, Ended, EndedPlayer, EndedStat, MatchCtx, OnlineMatches, PveSetup, Seat, Ticket82};
use local_match::{OnlineParams, OnlinePlayer};
use match_server::{Hub, HubConfig, MatchEnd, MatchServer, MatchSpec, PlayerSpec, PveEndRule, WorldStore};

pub struct Online {
    hub: Arc<Mutex<Hub>>,
    /// Matches the UDP driver thread took out of the hub (its `on_end` callback fills it).
    ended: Arc<Mutex<Vec<MatchEnd>>>,
    /// The level projects the world host serves (the daemon's answer to the project-list query, `srp::run`); None = not
    /// known yet: every row counts as served (a project the host lacks ends its match with `NoWorld`).
    host_projects: Mutex<Option<BTreeSet<String>>>,
    /// The name indexes `(first, second)` of each bot-filled match's AI players, in slot order (the result window
    /// shows them by name); taken when the match ends
    /// per bot-filled match: each bot's (first name index, second name index, ai_players index), in slot order
    bot_names: Mutex<HashMap<u32, Vec<(u8, u8, u16)>>>,
    /// The world host's room status and the matches created since it was read ([`crate::host_room`])
    room: crate::host_room::RoomBook,
}

impl Online {
    pub fn new(cfg: HubConfig, worlds: WorldStore) -> Online {
        Online {
            hub: Arc::new(Mutex::new(Hub::new(cfg, worlds))),
            ended: Arc::new(Mutex::new(Vec::new())),
            host_projects: Mutex::new(None),
            bot_names: Mutex::new(HashMap::new()),
            room: Default::default(),
        }
    }

    /// The world host's room status ([`crate::world_host::watch_room`] keeps it).
    pub fn room_book(&self) -> &crate::host_room::RoomBook {
        &self.room
    }

    /// Name the level projects the world host serves ([`host_projects`]): the PvP queue then offers and plays only those
    /// rows of the data.
    pub fn with_host_projects(self, projects: impl IntoIterator<Item = String>) -> Online {
        self.set_host_projects(projects);
        self
    }

    /// [`Online::with_host_projects`] on a shared hook (the daemon answered after the start).
    pub fn set_host_projects(&self, projects: impl IntoIterator<Item = String>) {
        *self.host_projects.lock().unwrap() = Some(projects.into_iter().collect());
    }

    /// Can a PvP match on `row` get a start world: a stored world of (map, mode) serves only its capture's row (variation
    /// 0, day: `resolve_map(map, mode, 0)`, online.md 2.2); else the world host serves the projects of its config (all
    /// when the config is not known); no host, no world: none.
    fn serves_row(&self, data: &GameData, row: &ModeRow) -> bool {
        let hub = self.hub.lock().unwrap();
        if !hub.host_builds(row.map, row.mode, None) {
            return hub.can_create(row.map, row.mode, None) && data.matches.resolve_map(row.map, row.mode, 0) == Some(row);
        }
        self.host_projects.lock().unwrap().as_ref().is_none_or(|p| p.contains(&row.project))
    }

    /// The hub (tests drive `on_datagram` / `poll` on a virtual clock through it).
    pub fn hub(&self) -> Arc<Mutex<Hub>> {
        self.hub.clone()
    }

    /// Bind the UDP socket and run the hub on its own thread.
    pub fn start_server(&self, bind: SocketAddr) -> io::Result<MatchServer> {
        let sink = self.ended.clone();
        MatchServer::start_with_hub(bind, self.hub.clone(), move |e| sink.lock().unwrap().push(e))
    }

    fn create_pvp(&self, data: &GameData, row: &ModeRow, match_type: u8, seats: &[Seat], fill: Option<BotFill>) -> Result<Ticket82, String> {
        let (map, mode, weather) = (row.map, row.mode, row.variation);
        // the row's level project: the world host builds its start world (without a host: the stored world of (map, mode))
        let project = row.project.clone();
        // a host-built world: only a project of the host's config (a stored world is used for any row)
        if self.hub.lock().unwrap().host_builds(map, mode, None) && !self.serves_row(data, row) {
            return Err(format!("the world host does not serve {project}"));
        }
        probe_host(&self.hub, map, mode, None)?;
        let mut hub = self.hub.lock().unwrap();
        let with_host = hub.host_builds(map, mode, None);
        if !hub.can_create(map, mode, None) {
            return Err(format!("no start world for map {map} mode {mode}"));
        }
        // every match its own id, the world's seed: two matches of one map run at once
        let match_id = hub.new_match_id();
        let tick = local_match::options::TICK_MS as u8;
        let players = online_players(data, seats, false);
        let params = OnlineParams { map, mode, weather, match_id, tick_ms: tick, send_period_ms: local_match::options::SEND_PERIOD_MS, match_type, max_delay_ms: local_match::options::max_delay_ms(tick) };
        // the AI players after the humans (the records' order is the slots' order: humans, then bots)
        let bots = fill.map(|f| local_match::pick_online_bots(data, f.need, f.level)).unwrap_or_default();
        if let Some(f) = fill {
            if bots.len() != f.need[0] + f.need[1] {
                return Err(format!("only {} AI players for {:?}", bots.len(), f.need));
            }
        }
        // Arms Race (gun race, mode 3; docs/notes/gun-race.md): humans only (arena.lua never spawns an AI record), each
        // record its own team, the nine race kits of the match's equipment level as its profiles
        let gun = row.mode == game_data::gunrace::MODE;
        if gun && !bots.is_empty() {
            return Err("the gun race has no AI players (arena.lua spawns humans only)".into());
        }
        let (options, players) = if gun {
            let tier = gunrace_tier(data, seats);
            let (blob, race) = local_match::gunrace_options(data, &params, &players, tier).map_err(|e| format!("gun race options: {e}"))?;
            for (i, s) in seats.iter().enumerate() {
                log::info!("gun race match {match_id:#x}: record {i} ({}) team {i}, equipment level {tier}, kits {:?}", s.name, local_match::gunrace::kit_names(data, tier, match_id, i));
            }
            (blob, race)
        } else {
            (local_match::online_options_row(data, &params, row, &players, &bots).map_err(|e| format!("match options: {e}"))?, players)
        };
        // the objects the same kits put in the starting world (the client builds its own list from these options)
        let spec = local_match::pvp_roster_spec(data, &players, &bots).map_err(|e| format!("starting world: {e}"))?;
        let roster = match_server::RosterObjects::from_spec(&spec).map_err(|e| format!("starting world: {e}"))?;
        log::info!("online match {project} (map {map} mode {mode} variation {weather} weather {}): roster objects {spec:?} ({} objects)", row.weather, roster.count());
        let spec = MatchSpec {
            match_id: Some(match_id),
            map,
            mode,
            options,
            // the team the options carry (the gun race: the record index)
            players: seats
                .iter()
                .zip(&players)
                .map(|(s, p)| PlayerSpec { account_id: s.account as u64, name: s.name.clone(), team: p.team, session: s.session, bot: false })
                .chain(bots.iter().enumerate().map(|(k, b)| PlayerSpec::bot(&b.name(data, k), b.team)))
                .collect(),
            level_checksum: None,
            roster: Some(roster),
            project: with_host.then_some(project),
            end_rule: end_rule(data, row),
            pve_end: None,
            // a custom room's "bots ignore players" (lobby rooms.rs): the world host sets the exe's own flag
            ai_ignore_players: fill.is_some_and(|f| f.ignore_players) && !bots.is_empty(),
        };
        if !bots.is_empty() {
            log::info!(
                "online match {match_id:#x}: {} human(s) + {} AI player(s) (ai ids {:?}, teams {:?}, level {}, ignore players {})",
                seats.len(),
                bots.len(),
                bots.iter().map(|b| b.ai).collect::<Vec<_>>(),
                bots.iter().map(|b| b.team).collect::<Vec<_>>(),
                fill.map_or(0, |f| f.level),
                spec.ai_ignore_players
            );
        }
        let t = hub.create_match(&spec, Instant::now()).map_err(|e| e.to_string())?;
        // counted against the world host's room until its status shows the match
        if let Some(p) = &spec.project {
            self.room.reserve(p, Instant::now());
        }
        if !bots.is_empty() {
            self.bot_names.lock().unwrap().insert(t.match_id, bots.iter().map(|b| (b.first, b.second, b.ai as u16)).collect());
        }
        // the relay hands out one order id per human (bots have no ticket)
        Ok(Ticket82 { match_id: t.match_id, host: t.host, port: t.port, orders: t.players.iter().map(|p| p.order_id).collect() })
    }

    pub fn worlds(&self) -> usize {
        self.hub.lock().unwrap().worlds_mut().len()
    }
}

/// The equipment level of a gun race match (options +0x1081, the race kits' tier): the highest equipment level of its
/// humans (`lobby_server::items::equipment_level`, the client's own rule), 1..=5 (CHOICE: one level per match, the script
/// has one `equipment_level`; the highest, as the booster row of a squad).
fn gunrace_tier(data: &GameData, seats: &[Seat]) -> u32 {
    let now = lobby_server::premium::unix_now();
    seats.iter().map(|s| u32::from(lobby_server::items::equipment_level(data, &s.profile, now))).max().unwrap_or(1).clamp(1, game_data::gunrace::TIERS as u32)
}

/// The humans of a match as the options carry them: the account's kit plus the service boosters in stock. A PvE
/// mission carries no booster: the lobby charges none for it and uses no service unit (`boosters::use_services`: online
/// PvP only), so nothing unpaid goes into the options (CHOICE; what 0.69d applied in PvE is OPEN).
fn online_players(data: &GameData, seats: &[Seat], pve: bool) -> Vec<OnlinePlayer> {
    seats
        .iter()
        .map(|s| {
            let ctx = MatchCtx { account_id: s.account, account_name: &s.name, profile: &s.profile, data };
            let mut human = crate::matches::human_loadout(&ctx);
            if pve {
                human.extra_modifiers.clear();
                return OnlinePlayer { team: s.team, human, hero: s.hero, pve_budget: (0, 0) };
            }
            // an online match also carries the service boosters in stock (the newbie's blessing; the lobby uses one
            // unit once the match server accepted the match: lobby_server::boosters::use_services)
            for (id, v) in lobby_server::boosters::service_modifier_ids(data, &s.profile) {
                *human.extra_modifiers.entry(id).or_insert(0.0) += v;
            }
            OnlinePlayer { team: s.team, human, hero: s.hero, pve_budget: (0, 0) }
        })
        .collect()
}

/// The time-limit rule of the row's mode file (game_modes_069d.json): the relay closes the match when the clients'
/// worlds end. None when the row or its numbers are unknown, and for Search and Destroy: its mode files list no
/// timelimit rule and its `match_time` (rule +0x308, ctor 0x2003AC0C0) has no reader; the world plays its rounds past
/// 900 s, so the world host's Outcome ends S&D (relay `host_poll`) and the relay's `max_duration` is the backstop.
fn end_rule(data: &GameData, row: &ModeRow) -> Option<match_server::EndRule> {
    if row.mode == lobby_server::queue::SAD_MODE {
        return None;
    }
    let t = data.game_modes.get(&row.project)?;
    let ms = |v: Option<f64>| v.map(|s| (s * 1000.0).round() as u32);
    Some(match_server::EndRule {
        countdown_ms: ms(t.countdown_time)?,
        match_ms: ms(t.match_time)?,
        advantage_ms: ms(t.advantage_time).unwrap_or(0),
        wait_ratio: t.wait_player_percent? as f32,
        leave_ratio: t.leave_player_percent.unwrap_or(1.0) as f32,
        // the tdm rule (TDM; slaughter has the same parameter block, LIKELY the same rule): an empty team ends it
        team_empty_end: row.project.ends_with("_tdm") || row.project.contains("_slr"),
        // artefact hunt plays a draw time after its time end on a tied score: `draw_additional_time` 180 s (mode file
        // level_07_day_ah; a world host run of level_01_day_ah ended at start + 900 + 180 s).
        // The mode JSON carries no such key, so the value is the mode files'
        draw_overtime_ms: if row.project.contains("_ah") { 180_000 } else { 0 },
    })
}

/// The project a PvE setup plays: a maze with a stored world first (the hub prefers it: tests, a host-less setup), else
/// the lobby's maze pick (a world host builds any project). `(maze 1 | 2, project)`.
fn pve_project(hub: &Hub, data: &GameData, setup: &PveSetup) -> Result<(u8, String), String> {
    let (map, mode) = (setup.mission, data.pve.mode);
    let mazes = [1u8, 2].map(|m| data.pve.project(setup.difficulty, m).unwrap_or_default());
    let first = mazes.iter().position(|p| *p == setup.project).unwrap_or(0);
    let ok = |k: usize| !mazes[k].is_empty() && hub.can_create(map, mode, Some(&mazes[k]));
    // a stored world first (the hub uses it before the host: tests, a host-less setup), else the lobby's pick
    let order = [first, 1 - first];
    order
        .into_iter()
        .find(|&k| ok(k) && !hub.host_builds(map, mode, Some(&mazes[k])))
        .or_else(|| order.into_iter().find(|&k| ok(k)))
        .map(|k| (k as u8 + 1, mazes[k].clone()))
        .ok_or_else(|| format!("no start world for PvE difficulty {} ({} or {})", setup.difficulty, mazes[0], mazes[1]))
}

/// The world host must answer before a match is created on it: a daemon that is down refuses the match here (the lobby
/// then refuses the start cleanly and charges nothing) instead of after the tickets went out. A connection that closes
/// without a frame is ignored by the daemon's front. The hub lock is not held (no socket IO under it, host.rs I8).
fn probe_host(hub: &Mutex<Hub>, map: u32, mode: u32, project: Option<&str>) -> Result<(), String> {
    let h = {
        let hub = hub.lock().unwrap();
        if !hub.host_builds(map, mode, project) {
            return Ok(());
        }
        hub.config().match_config.world_host.clone()
    };
    let Some(h) = h else { return Ok(()) };
    match (h.connector)() {
        Ok(s) => {
            s.shutdown_both();
            Ok(())
        }
        Err(e) => {
            log::warn!("online match not created: the world host {} does not answer: {e}", h.label);
            Err(format!("the world host does not answer: {e}"))
        }
    }
}

/// The world host's `Used` row of a slot (protocol 6) as the lobby's charge input; None = the host did not report it.
fn used_of(f: &match_server::FinalStats, slot: u8) -> Option<lobby_server::charge::MatchUse> {
    let u = f.used.as_ref()?;
    let p = u.iter().find(|p| p.slot == slot);
    Some(match p {
        Some(p) => lobby_server::charge::MatchUse {
            profiles: p.profiles,
            weapons: p.weapons.clone(),
            stacks: p.stacks.iter().map(|x| lobby_server::charge::StackUse { uid: x.uid, dict: x.dict, used: x.used }).collect(),
        },
        // a human the host listed no row for spawned with nothing
        None => lobby_server::charge::MatchUse::default(),
    })
}

fn convert(e: MatchEnd, bot_names: &[(u8, u8, u16)]) -> Ended {
    let account_of = |slot: u8| e.players.iter().find(|p| p.slot == slot).map(|p| p.account_id as i64);
    let stats = e.stats.as_ref().map(|f| {
        // the world's own data (world host protocol 3), one shared value for the players of the match (match_data.rs);
        // the roster is the humans of the match with their accounts and the server bots without one (their teams tell an
        // enemy bot's kill from a team kill: PvP bots share the humans' teams)
        let world = (!f.details.is_empty()).then(|| {
            std::sync::Arc::new(lobby_server::match_data::WorldData {
                roster: e
                    .players
                    .iter()
                    .map(|p| lobby_server::match_data::RosterSlot { slot: p.slot, account: Some(p.account_id as i64), team: p.team, entered: p.entered })
                    .chain(e.bots.iter().map(|b| lobby_server::match_data::RosterSlot { slot: b.slot, account: None, team: b.team, entered: b.entered }))
                    .collect(),
                details: f.details.clone(),
                kills: f.kills.clone(),
                // the AI players' rows for the result window: name indexes in slot order, the host's counters
                bots: {
                    let mut bs: Vec<_> = e.bots.iter().collect();
                    bs.sort_by_key(|b| b.slot);
                    bs.iter()
                        .enumerate()
                        .map(|(k, b)| {
                            let st = f.players.iter().find(|s| s.slot == b.slot);
                            lobby_server::match_data::BotRow {
                                slot: b.slot,
                                team: b.team,
                                names: bot_names.get(k).map(|b| (b.0, b.1)),
                                // the AI player whose kit the record carries (the result window's equipment view)
                                ai: bot_names.get(k).map(|b| b.2),
                                kills: st.map_or(0, |s| s.kills),
                                deaths: st.map_or(0, |s| s.deaths),
                                score: st.map_or(0, |s| s.score),
                            }
                        })
                        .collect()
                },
            })
        });
        let v = f
            .players
            .iter()
            .filter_map(|s| {
                account_of(s.slot).map(|account| EndedStat { account, kills: s.kills, deaths: s.deaths, score: s.score, world: world.clone(), used: used_of(f, s.slot) })
            })
            .collect();
        (v, f.team_scores)
    });
    Ended {
        match_id: e.match_id,
        duration: e.duration,
        players: e.players.iter().map(|p| EndedPlayer { account: p.account_id as i64, team: p.team, entered: p.entered, left_by_menu: p.leave == match_server::LeaveReason::Menu }).collect(),
        stats,
        winner: e.winner,
    }
}

impl OnlineMatches for Online {
    fn available(&self, map: u32, mode: u32) -> bool {
        self.hub.lock().unwrap().can_create(map, mode, None)
    }

    fn create(&self, data: &GameData, map: u32, mode: u32, weather: u32, match_type: u8, seats: &[Seat]) -> Result<Ticket82, String> {
        let row = data.matches.resolve_map(map, mode, weather).ok_or_else(|| format!("no match row for map {map} mode {mode} weather {weather}"))?;
        self.create_pvp(data, row, match_type, seats, None)
    }

    fn serves(&self, data: &GameData, row: &ModeRow) -> bool {
        self.serves_row(data, row)
    }

    /// A PvP queue match on its picked row (variation and weather); AI players only with a host that drives them.
    fn create_row(&self, data: &GameData, row: &ModeRow, match_type: u8, seats: &[Seat], fill: Option<BotFill>) -> Result<Ticket82, String> {
        if fill.is_some() && !self.bots_available() {
            return Err("no world host drives bots".into());
        }
        self.create_pvp(data, row, match_type, seats, fill)
    }

    /// AI players fill a PvP match only when a world host drives them (one is attached; it always drives bots): the
    /// clients run no AI online.
    fn bots_available(&self) -> bool {
        self.hub.lock().unwrap().config().match_config.world_host.is_some()
    }

    fn create_with_bots(&self, data: &GameData, map: u32, mode: u32, weather: u32, match_type: u8, seats: &[Seat], fill: BotFill) -> Result<Ticket82, String> {
        if !self.bots_available() {
            return Err("no world host drives bots".into());
        }
        let row = data.matches.resolve_map(map, mode, weather).ok_or_else(|| format!("no match row for map {map} mode {mode} weather {weather}"))?;
        self.create_pvp(data, row, match_type, seats, Some(fill))
    }

    fn create_pve(&self, data: &GameData, setup: &PveSetup, seats: &[Seat]) -> Result<Ticket82, String> {
        let (map, mode) = (setup.mission, data.pve.mode);
        let (maze, project) = pve_project(&self.hub.lock().unwrap(), data, setup)?;
        probe_host(&self.hub, map, mode, Some(&project))?;
        let mut hub = self.hub.lock().unwrap();
        if project != setup.project {
            log::info!("PvE match: no world for {}, the stored {project} is used", setup.project);
        }
        let match_id = hub.new_match_id();
        let tick = local_match::options::TICK_MS as u8;
        let mut humans = online_players(data, seats, true);
        // paid respawns (docs/notes/pve.md): only with a world host, which reports what
        // the world charged (`keys_spent` / `gold_spent`, charged by the lobby at the result): the spendable access keys
        // left after the entry fee and the gold balance. Without a host the spends cannot be known: no budget, nothing is
        // charged (the client then offers no paid respawn)
        if hub.config().match_config.world_host.is_some() {
            for (h, s) in humans.iter_mut().zip(seats) {
                h.pve_budget = lobby_server::pve::respawn_budget(&s.profile);
            }
            log::info!("PvE match: paid-respawn budgets (keys, gold) {:?}", humans.iter().map(|h| h.pve_budget).collect::<Vec<_>>());
        }
        let params = OnlineParams {
            map,
            mode,
            weather: data.pve.weather,
            match_id,
            tick_ms: tick,
            send_period_ms: local_match::options::SEND_PERIOD_MS,
            match_type: local_match::options::MATCH_TYPE_PLAIN,
            max_delay_ms: local_match::options::max_delay_ms(tick),
        };
        let level = local_match::options::PVE_EQUIPMENT_LEVEL;
        let options = local_match::pve_options(data, &params, &humans, setup.difficulty, maze, level).map_err(|e| format!("PvE match options: {e}"))?;
        // the objects of all records: the humans' kits and the 17 bots' two kits each
        let spec = local_match::pve_roster_spec(data, &humans, setup.difficulty).map_err(|e| format!("PvE starting world: {e}"))?;
        let roster = match_server::RosterObjects::from_spec(&spec).map_err(|e| format!("PvE starting world: {e}"))?;
        let bots = data.pve.bots(setup.difficulty);
        log::info!(
            "online PvE match {project} (difficulty {}, match id {match_id:#x}): {} human(s) + {} bots = {} records, roster objects {spec:?} ({} objects)",
            setup.difficulty,
            seats.len(),
            bots.len(),
            roster.records,
            roster.count()
        );
        let mut players: Vec<PlayerSpec> = seats.iter().map(|s| PlayerSpec { account_id: s.account as u64, name: s.name.clone(), team: s.team, session: s.session, bot: false }).collect();
        // the script's bots: server-owned slots after the humans, in record order
        players.extend(bots.iter().map(|b| PlayerSpec::bot(&b.name, b.team)));
        let spec = MatchSpec { match_id: Some(match_id), map, mode, options, players, level_checksum: None, roster: Some(roster), project: Some(project), end_rule: None, pve_end: Some(PveEndRule::PVE_LUA), ai_ignore_players: false };
        let t = hub.create_match(&spec, Instant::now()).map_err(|e| e.to_string())?;
        // counted against the world host's room until its status shows the match
        if hub.host_builds(map, mode, spec.project.as_deref())
            && let Some(p) = &spec.project
        {
            self.room.reserve(p, Instant::now());
        }
        Ok(Ticket82 { match_id: t.match_id, host: t.host, port: t.port, orders: t.players.iter().map(|p| p.order_id).collect() })
    }

    fn played(&self, match_id: u32) -> Option<std::time::Duration> {
        self.hub.lock().unwrap().played(match_id)
    }

    fn take_left(&self) -> Vec<lobby_server::online::Left> {
        self.hub
            .lock()
            .unwrap()
            .take_left()
            .into_iter()
            .map(|(match_id, account, why)| lobby_server::online::Left { match_id, account: account as i64, by_menu: why == match_server::LeaveReason::Menu })
            .collect()
    }

    fn end(&self, match_id: u32) -> bool {
        self.hub.lock().unwrap().end_match(match_id, Instant::now())
    }

    /// Rejoin: humans dropped with their slot kept (match-server `MatchConfig::rejoin`).
    fn take_dropped(&self) -> Vec<lobby_server::online::Left> {
        self.hub
            .lock()
            .unwrap()
            .take_dropped()
            .into_iter()
            .map(|(match_id, account, _)| lobby_server::online::Left { match_id, account: account as i64, by_menu: false })
            .collect()
    }

    fn take_rejoined(&self) -> Vec<(u32, i64)> {
        self.hub.lock().unwrap().take_rejoined().into_iter().map(|(m, a)| (m, a as i64)).collect()
    }

    fn allow_rejoin(&self, match_id: u32, account: i64, session: Option<u32>) -> bool {
        self.hub.lock().unwrap().allow_rejoin(match_id, account as u64, session)
    }

    fn leave_dropped(&self, match_id: u32, account: i64) -> bool {
        self.hub.lock().unwrap().leave_dropped(match_id, account as u64)
    }

    /// A PvE match after its result: the closing DFs keep the clients' ended world (code 2, the usable result screen).
    fn end_after_result(&self, match_id: u32) -> bool {
        self.hub.lock().unwrap().end_match_after_result(match_id, Instant::now())
    }

    fn holds_clients(&self, match_id: u32) -> bool {
        self.hub.lock().unwrap().holds_clients(match_id)
    }

    fn has_match(&self, match_id: u32) -> bool {
        self.hub.lock().unwrap().has_match(match_id)
    }

    /// The world host's outcomes, the humans' rows only.
    fn take_outcomes(&self) -> Vec<lobby_server::HostOutcome> {
        self.hub
            .lock()
            .unwrap()
            .take_outcomes()
            .into_iter()
            .map(|o| lobby_server::HostOutcome {
                match_id: o.match_id,
                state: o.state,
                winner_team: o.winner_team,
                success: o.pve_success,
                mission_ms: o.mission_ms,
                players: o
                    .players
                    .iter()
                    .filter(|p| !p.bot)
                    .map(|p| lobby_server::HostPlayerStat { account: p.account_id as i64, kills: p.kills, deaths: p.deaths, score: p.score, chests: p.chests, keys_spent: p.keys_spent, gold_spent: p.gold_spent })
                    .collect(),
            })
            .collect()
    }

    fn world_host(&self, match_id: u32) -> bool {
        self.hub.lock().unwrap().host_active(match_id)
    }

    /// The world host's room for the queues (lobby-server `host_room`): the latest status with the matches created
    /// since counted in; None without a world host or a fresh status.
    fn host_room(&self) -> Option<lobby_server::host_room::HostRoom> {
        self.room.room(Instant::now())
    }

    fn take_ended(&self) -> Vec<Ended> {
        let mut all: Vec<MatchEnd> = std::mem::take(&mut *self.ended.lock().unwrap());
        all.extend(self.hub.lock().unwrap().take_ended());
        let mut names = self.bot_names.lock().unwrap();
        all.into_iter()
            .map(|e| {
                let n = names.remove(&e.match_id).unwrap_or_default();
                convert(e, &n)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use match_server::{EndReason, FinalStats, LeaveReason, PlayerResult, PlayerStat};

    /// The AI players of a finished match reach the lobby with their slots, teams, the host's counters and the name
    /// indexes the match was created with (slot order), for the result window's bot rows.
    #[test]
    fn the_ended_world_carries_the_bots_with_names_and_counters() {
        let pr = |slot: u8, account_id: u64, team: u8| PlayerResult { slot, account_id, team, entered: true, leave: LeaveReason::Disconnected };
        let st = |slot: u8, kills: u16, deaths: u16, score: u16| PlayerStat { slot, kills, deaths, score, chests: 0 };
        let e = MatchEnd {
            match_id: 7,
            map: 1,
            mode: 1,
            reason: EndReason::TimeLimit,
            winner: Some(0),
            duration: std::time::Duration::from_secs(900),
            players: vec![pr(0, 42, 0)],
            // out of slot order on purpose
            bots: vec![pr(2, 0, 1), pr(1, 0, 0)],
            stats: Some(FinalStats { players: vec![st(0, 5, 3, 75), st(1, 7, 2, 105), st(2, 1, 9, 10)], team_scores: [5, 2], details: vec![Default::default()], kills: None, used: None }),
        };
        let ended = convert(e, &[(23, 17, 4), (0, 0, 9)]);
        let (stats, _) = ended.stats.unwrap();
        assert_eq!(stats.len(), 1, "the human only");
        let w = stats[0].world.clone().expect("the world's data");
        let b: Vec<_> = w.bots.iter().map(|b| (b.slot, b.team, b.names, b.ai, b.kills, b.deaths, b.score)).collect();
        assert_eq!(b, vec![(1, 0, Some((23, 17)), Some(4), 7, 2, 105), (2, 1, Some((0, 0)), Some(9), 1, 9, 10)]);
    }

    /// Search and Destroy gets no time-limit mirror (its world plays its rounds past `match_time`; the host's Outcome ends
    /// it); a tdm row keeps its 900 s rule.
    #[test]
    fn search_and_destroy_has_no_time_mirror() {
        game_data::require_tables!();
        let gd = game_data::testing::tables();
        let sad: Vec<&ModeRow> = gd.matches.modes().iter().filter(|r| r.mode == lobby_server::queue::SAD_MODE).collect();
        assert_eq!(sad.len(), 4);
        for r in sad {
            assert!(gd.game_modes.get(&r.project).is_some_and(|t| t.match_time == Some(900.0)), "{} has mode-file timings", r.project);
            assert_eq!(end_rule(&gd, r), None, "{}", r.project);
        }
        let tdm = gd.matches.modes().iter().find(|r| r.project == "level_03_day_tdm").unwrap();
        assert_eq!(end_rule(&gd, tdm).map(|e| e.match_ms), Some(900_000));
    }
}
