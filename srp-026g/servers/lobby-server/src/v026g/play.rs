//! Play: making a match, and coming back from it.
//!
//! A PvP match is a room's (`rooms`, the owner's of 2026-10-17: the queue,
//! its level search and the late joins it scored went with it). Play with
//! nothing else chosen goes into a room; Play after a choice of a match
//! against the server's bots ("Player vs Bots", `pvb`) makes that match at
//! once, with the player alone against the bots, or is refused. Two clan
//! teams that wait meet in a match of their own (`clans`).
//!
//! The match itself is the match server's: the lobby hands it the roster
//! through [`MatchHost`] and sends each player where the ticket says
//! (0x82). A running room match takes a late joiner on the side with fewer
//! players, then the side it played on before, then the one that counts
//! for less (`rooms`, `policy::place_joiner`) until its end.
//!
//! Each minute of a match being played the log has what its sides count for
//! and the chance that gives team 0 ([`Core::log_predictions`]): what the
//! numbers of the rule are fitted from.
//!
//! 0x82 is not ordered against the client states (0x85) sent before it
//! (transport.md §4.1), and a 0x85 "in the queue" read after it would take
//! the client's match id away again (`lobby.md` §2.4). So a 0x82 goes only
//! once the client state sent just before it ("match found") is acked: the
//! ordered channel delivers in order, so every earlier state is then read.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};
use std::time::{Duration, Instant};

use session::matchmaking_policy::{self as policy, LateRule, Worth};
use session::v026g::{Account, matchmaking, quests, rules, wire};
use session::{Level, LockExt, MAX_PLAYERS_PER_MATCH, QueuePool, QueueState};
use survarium::v026g::loadouts::BotRole;
use survarium::v026g::lobby::requests::id;
use survarium::v026g::lobby::{self, ClientState, limits};
use survarium::v026g::maps;
use survarium::v026g::match_msgs::static_info::{
    KIT_NOT_LOADED, MatchItem, MatchKit, MatchProfile, Upgrade,
};

use super::core::{Core, Who};

/// A match seated (a room's, a clan match, one against bots) is made at the
/// match server by the first of its players the lobby looks at; one not
/// made after this long has none.
const UNMADE_MATCH: Duration = Duration::from_secs(10);

/// How often the log has what the sides of a match being played count for.
const PREDICTION_EVERY: Duration = Duration::from_secs(60);

/// The seconds a step of the client's search screen takes (0x85 "in the
/// queue"; the game draws nothing of it with 0). Only a clan team's wait
/// shows that screen now.
const SEARCH_STEP: std::num::NonZeroU16 = std::num::NonZeroU16::new(4).unwrap();

/// The game's text for a Play with a kit that is none of the account's, or
/// one it may not use.
const PLAY_KIT: &str = "st_profile_check_failed";

/// The match server, as the lobby needs it. `servers/srp` gives the lobby
/// an adapter over the match server's registry.
pub trait MatchHost: Send + Sync {
    /// Make a match for this roster. The error is for the log; the players
    /// go back to the lobby.
    fn create(&self, setup: MatchSetup) -> Result<MatchTicket, String>;
    /// Atomically reserve an entire party; false leaves it in the lobby.
    fn join_running(&self, _match_id: u32, _players: Vec<MatchPlayer>) -> bool {
        false
    }
    /// Whether the match takes a late joiner now: in play and to its end,
    /// with no earlier joiner still on its way in.
    fn admits(&self, _match_id: u32, _now: Instant) -> bool {
        false
    }
    /// Why the match takes no late joiner now, when it says: what a room's
    /// waiting member is told. `None` when it takes one, or does not say.
    fn closed(&self, _match_id: u32, _now: Instant) -> Option<policy::Closed> {
        None
    }
    /// How the match stands now. `None`: it is over, or the host does not
    /// say; its whole roster is then taken to be in it, with no lead.
    fn live(&self, _match_id: u32, _now: Instant) -> Option<LiveView> {
        None
    }
    /// Whether the match is made and not over. The lobby asks about its
    /// matches every second and frees the seats of one that is over, also
    /// when nobody called [`super::Inbox::match_finished`].
    fn is_open(&self, match_id: u32) -> bool;
    /// The account signed in again while its match goes on: its place in
    /// the match is now its new session's (the client signs its match
    /// connection with the session, 0xD8). `false` when the match is over
    /// or the account has no place in it.
    fn reseat(&self, match_id: u32, account_id: u32, session_id: u32) -> bool;
}

/// A match as the match server has it now ([`MatchHost::live`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LiveView {
    /// Bit `i`: roster place `i` is in the match. A player who left is
    /// not; one on its way in is.
    pub present: u32,
    /// Batteries home (modes 2, 3) or tickets left (mode 4), by team.
    /// `None` until the match server trusted a world.
    pub points: Option<[u16; 2]>,
    /// Each team's kills in that world.
    pub kills: [u16; 2],
    /// Since play began. `None` before it has, and after the end.
    pub played: Option<Duration>,
    /// Until the time limit ends it, known from the countdown on; zero
    /// once the match is over.
    pub time_left: Option<Duration>,
}

/// One roster place: the index in [`MatchSetup::players`] is the player's
/// place in the match.
#[derive(Clone, Debug, PartialEq)]
pub struct MatchPlayer {
    /// The session the client signs its match connection with (0xD8).
    pub session_id: u32,
    pub account_id: u32,
    /// The player as the match shows it: its kits, items and the skills'
    /// modifiers; `team` (0 or 1) and `squad` (0 for none) are set.
    pub profile: MatchProfile,
    /// **Testing only**: the account is a test bot
    /// (`session::v026g::bots`, `SRP_BOT_ACCOUNTS`; false for everyone
    /// unless the operator named bots). The match server then takes the
    /// place as a player and never as a witness of the world.
    pub bot: bool,
    /// The match server's own bot of a match against bots (`pvb`; see
    /// [`server_bot`]): no account, no session (both 0), no client.
    pub driven: bool,
}

/// The name the server's `n`-th bot of a match plays under (1 on).
fn server_bot_name(n: usize) -> String {
    format!("Bot-{n}")
}

/// The match server's own bot of `role` on `team`, number `n` of its match
/// (1 on): a place with no account, its player with the role's two kits
/// (`loadouts::bot_kits`): the role's weapon in slot 7 with the most ammo
/// players carry, the role's armour, a backpack and trousers that open
/// three pockets (four for a support), and in them medkits, the radio and
/// two RGD-5 grenades; the second the same with another weapon and its
/// ammo (`BotRole::backup`), for when the first kit's ammo is spent. The match server reads the
/// role back from the profile (`loadouts::bot_role_of`).
pub fn server_bot(n: usize, team: u8, role: BotRole) -> MatchPlayer {
    let mut profile = MatchProfile::from_loadouts(
        &server_bot_name(n),
        team,
        &survarium::v026g::loadouts::bot_kits(role),
        [0.0; survarium::v026g::skills::MODIFIERS],
        survarium::v026g::dictionary::Dictionary::shared(),
        1,
    );
    profile.clan_tag = "BOT".to_owned();
    MatchPlayer {
        session_id: 0,
        account_id: 0,
        profile,
        bot: false,
        driven: true,
    }
}

/// The roles of bots on `teams`, in order: each team's `k`-th bot gets
/// `BotRole::nth(k)` (the shares by smooth weighted round robin: a
/// flanker, a rusher, a support, a camper, ...), so it depends only on the
/// bots' places and the same roster gets the same roles.
fn server_bot_roles(teams: &[u8]) -> Vec<BotRole> {
    let mut on_team = [0_usize; 2];
    teams
        .iter()
        .map(|&team| {
            let k = &mut on_team[usize::from(team.min(1))];
            let role = BotRole::nth(*k);
            *k += 1;
            role
        })
        .collect()
}

/// The teams of `count` bots added to a roster whose teams are `teams`:
/// each to the side with fewer places then, the weaker side (`counts`, what
/// the sides count for) on a tie; never past the 20 places of a match.
fn server_bot_teams(teams: &[u8], counts: [f64; 2], count: usize) -> Vec<u8> {
    let mut sizes = [0_usize; 2];
    for &team in teams {
        sizes[usize::from(team.min(1))] += 1;
    }
    let room = session::MAX_PLAYERS_PER_MATCH.saturating_sub(teams.len());
    (0..count.min(room))
        .map(|_| {
            let team = match sizes[0].cmp(&sizes[1]) {
                std::cmp::Ordering::Less => 0,
                std::cmp::Ordering::Greater => 1,
                std::cmp::Ordering::Equal => u8::from(counts[1] < counts[0]),
            };
            sizes[usize::from(team)] += 1;
            team
        })
        .collect()
}

/// A match the lobby formed.
#[derive(Clone, Debug, PartialEq)]
pub struct MatchSetup {
    pub pool: QueuePool,
    /// A playable `maps_dict` id: the map and the mode.
    pub map_id: u8,
    /// The row of `match_levels_dict` the match's rewards are multiplied
    /// by: the mean of its players' levels
    /// (`session::v026g::matchmaking::match_level`).
    pub match_level: u16,
    pub players: Vec<MatchPlayer>,
    /// One player against the server's own bots (`pvb`): the match
    /// server's `MatchSetup::vs_bots`.
    pub vs_bots: bool,
}

/// Where the players of a made match connect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchTicket {
    /// What the clients are told (0x82) and the match is known by from
    /// then on ([`MatchHost::is_open`], [`super::MatchFinished`]).
    pub match_id: u32,
    /// The match server's public host; a name is looked up, as the client
    /// takes only an IPv4 address.
    pub host: String,
    pub port: u16,
}

/// A match made and not over.
pub(super) struct Running {
    pub pool: QueuePool,
    pub fixed_teams: bool,
    pub ticket: MatchTicket,
    pub map_id: u8,
    /// Its roster's sessions and teams, by place.
    pub seats: Vec<(u32, u8)>,
    /// By place: what its player counted for when it was seated, beside
    /// being there (`Weights::strength`).
    pub strengths: Vec<i32>,
    /// By place: its player's name, for the log.
    pub names: Vec<String>,
    /// When the log last had what the sides count for.
    pub predicted_at: Option<Instant>,
    /// One player against the server's own bots (`pvb`).
    pub vs_bots: bool,
}

/// One player as the log's "sides" and "late join" lines have it: what it
/// counts for and the parts of that. `scripts/balance-refit` reads these.
fn counted_player(account: &Account, counts: f64, gear: u8) -> String {
    format!(
        "{} {counts:.1} (gear {gear}, level {}, rating {:+.2} of {} matches)",
        account.name,
        account.level(),
        f64::from(account.balance.rating) / f64::from(session::v026g::balance::UNIT),
        account.balance.matches
    )
}

/// The account as a match shows it: every kit with what it wears, the
/// items once, and the modifiers its skills give
/// (`survarium::v026g::skills`), on `team` and in `squad`.
///
/// Only a kit the player could press Play with ([`rules::check_play`], the
/// client's own check of a kit: a weapon, and its ammunition) is loaded in
/// the match; any other goes as [`KIT_NOT_LOADED`]. In a match a dead
/// player picks the kit of its next spawn among the loaded ones, with no
/// check at all, and a spawn with nothing in the weapon slots 7 and 10
/// ends every game of the match: `base_player::respawn` leaves the item in
/// the hands empty (0x2DADC6..0x2DAE0C), `player::respawn` then takes the
/// player's own at +0xF47C, which is null, and calls through it
/// (0x1F25AD..0x1F25BF). Every client plays that spawn, so every client
/// dies at that tick. PROVEN: a player's crash report of match 0x1f793b8
/// (2026-10-12, access violation reading 0 at 0x1F25BF, under the respawn
/// rule's tick 0x316820), and match 0x5cc8cc52, whose last agreed world
/// has a dead player with an empty kit as its next (three matches lost
/// that day). Until then every kit of an account went as usable, the
/// empty ones too.
pub fn match_profile(account: &Account, team: u8, squad: u8, now: u64) -> MatchProfile {
    let body = wire::account_body(account, now);
    MatchProfile {
        team,
        name: body.name,
        clan_tag: body.clan_tag,
        protection: body.protection,
        kits: body
            .kits
            .into_iter()
            .map(|kit| MatchKit {
                state: if rules::check_play(account, kit.id, now).is_ok() {
                    kit.state
                } else {
                    KIT_NOT_LOADED
                },
                id: kit.id,
                name: kit.name,
                slots: kit
                    .slots
                    .map(|slot| slot.map(|slot| (slot.item_index, slot.count))),
                modifiers: kit.modifiers,
            })
            .collect(),
        items: body
            .items
            .into_iter()
            .map(|item| MatchItem {
                instance_id: item.item_id,
                dict_id: item.dict_id,
                condition_or_stack: item.condition_or_stack,
                repair_time: item.repair_time,
                reroll_time: item.reroll_time,
                modifications: item
                    .modifications
                    .iter()
                    .map(|modification| (modification.id, modification.state))
                    .collect(),
                upgrades: item
                    .upgrades
                    .iter()
                    .map(|upgrade| Upgrade {
                        id: upgrade.id,
                        unknown_4: upgrade.a,
                        unknown_8: upgrade.value,
                        param: upgrade.param,
                    })
                    .collect(),
            })
            .collect(),
        squad,
    }
}

/// Whether `ip` is this machine or its local network
/// (`vostok::config`'s rule).
fn is_local(ip: IpAddr) -> bool {
    match ip.to_canonical() {
        IpAddr::V4(ip) => ip.is_loopback() || ip.is_private() || ip.is_link_local(),
        IpAddr::V6(ip) => ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local(),
    }
}

/// `host` as a numeric IPv4 address, looked up if it is a name.
fn ipv4_host(host: &str, port: u16) -> String {
    if host.parse::<Ipv4Addr>().is_ok() {
        return host.to_owned();
    }
    let found = (host, port).to_socket_addrs().ok().and_then(|mut addrs| {
        addrs.find_map(|addr| match addr {
            SocketAddr::V4(addr) => Some(addr.ip().to_string()),
            SocketAddr::V6(_) => None,
        })
    });
    found.unwrap_or_else(|| {
        log::warn!("The match server host `{host}` has no IPv4 address; the client can't use it");
        host.to_owned()
    })
}

impl Core<'_> {
    pub(super) fn pool_for_sessions(
        &self,
        sessions: impl IntoIterator<Item = u32>,
    ) -> Option<QueuePool> {
        let mut pools = sessions.into_iter().map(|session| {
            self.store.client_revision(session).map(|revision| {
                if survarium::v026g::match_msgs::late_join::spoken_by(revision) {
                    QueuePool::GrowingRoster
                } else {
                    QueuePool::Standard
                }
            })
        });
        let first = pools.next()??;
        pools.all(|pool| pool == Some(first)).then_some(first)
    }

    /// The sides of a running match as they are now: the players the
    /// match server says are in it (the whole roster when it says
    /// nothing), and what they count for together.
    fn sides_now(running: &Running, live: Option<&LiveView>) -> [Worth; 2] {
        let mut sides = [Worth::default(); 2];
        for (place, &(_, team)) in running.seats.iter().enumerate() {
            if live.is_some_and(|live| live.present >> place & 1 == 0) {
                continue;
            }
            let side = &mut sides[usize::from(team.min(1))];
            side.players += 1;
            side.strength += i64::from(running.strengths.get(place).copied().unwrap_or(0));
        }
        sides
    }

    /// The players of a running match's roster who are not in it now, by
    /// name and in the roster's order: what the log's per-minute line ends
    /// with.
    pub(super) fn out_now<'a>(running: &'a Running, live: &LiveView) -> Vec<&'a str> {
        (0..running.seats.len())
            .filter(|&place| live.present >> place & 1 == 0)
            .map(|place| running.names.get(place).map_or("", String::as_str))
            .collect()
    }

    /// What each side's lead counts for in a running match
    /// (`Weights::lead`).
    fn lead_now(&self, running: &Running, live: Option<&LiveView>) -> [i64; 2] {
        let mode = maps::map_mode(running.map_id).map_or(0, |map| map.mode);
        live.and_then(|live| live.points)
            .map_or([0; 2], |points| self.settings.balance.lead(mode, points))
            .map(i64::from)
    }

    /// Whether a running match takes players now: one of growing rosters
    /// whose teams are not fixed, to which the match server admits (in
    /// play, to its end, and no earlier joiner still on its way in).
    pub(super) fn late_admission(&self, running: &Running, now: Instant) -> bool {
        running.pool == QueuePool::GrowingRoster
            && !running.fixed_teams
            && self.host.admits(running.ticket.match_id, now)
    }

    /// The sides of a running match as they are now, and the rule a late
    /// joiner is put into it by ([`policy::place_joiner`]): sides of at
    /// most `capacity` each (the caller's, a room's size), and no more
    /// places than the match has left (places are never used twice).
    pub(super) fn late_rule(
        &self,
        running: &Running,
        live: Option<&LiveView>,
        capacity: usize,
    ) -> ([Worth; 2], LateRule) {
        let rule = LateRule {
            capacity: capacity.min(MAX_PLAYERS_PER_MATCH / 2),
            room: MAX_PLAYERS_PER_MATCH.saturating_sub(running.seats.len()),
            player_value: i64::from(self.settings.balance.player_value()),
            lead: self.lead_now(running, live),
        };
        (Self::sides_now(running, live), rule)
    }

    /// `(what a side counts for, the players in it, its lead)`, in gear
    /// levels, for the log.
    pub(super) fn counted(&self, sides: [Worth; 2], lead: [i64; 2]) -> [(f64, usize, f64); 2] {
        let scale = f64::from(session::STRENGTH_SCALE);
        let player = i64::from(self.settings.balance.player_value());
        std::array::from_fn(|side| {
            let all = player * sides[side].players as i64 + sides[side].strength + lead[side];
            (
                all as f64 / scale,
                sides[side].players,
                lead[side] as f64 / scale,
            )
        })
    }

    /// Each minute of a match being played: what its sides count for now
    /// (the players in it, their strengths, the lead), the score, the
    /// chance that count gives team 0, and who of its roster is not in it.
    /// For the log alone: the numbers of the balance rule are fitted from
    /// these lines and the results.
    pub(super) fn log_predictions(&mut self, now: Instant) {
        let due: Vec<u32> = self
            .matches
            .iter()
            .filter(|(_, running)| {
                running
                    .predicted_at
                    .is_none_or(|at| now.saturating_duration_since(at) >= PREDICTION_EVERY)
            })
            .map(|(id, _)| *id)
            .collect();
        for id in due {
            let running = &self.matches[&id];
            let Some(live) = self.host.live(running.ticket.match_id, now) else {
                continue;
            };
            let Some(played) = live.played else {
                continue;
            };
            let sides = Self::sides_now(running, Some(&live));
            let lead = self.lead_now(running, Some(&live));
            let counted = self.counted(sides, lead);
            let scale = f64::from(session::STRENGTH_SCALE);
            let apart = ((counted[0].0 - counted[1].0) * scale).round() as i64;
            let out = Self::out_now(running, &live);
            log::info!(
                "Match {:#x} at {} s: team 0 counts {:.1} ({} in, lead {:.1}), team 1 counts {:.1} \
                 ({} in, lead {:.1}); points {}, kills {:?}; team 0 expected at {:.0} %{}",
                running.ticket.match_id,
                played.as_secs(),
                counted[0].0,
                counted[0].1,
                counted[0].2,
                counted[1].0,
                counted[1].1,
                counted[1].2,
                live.points
                    .map_or("unknown".to_owned(), |points| format!("{points:?}")),
                live.kills,
                100.0 * self.settings.balance.expected(apart),
                if out.is_empty() {
                    String::new()
                } else {
                    format!("; out: {}", out.join(", "))
                }
            );
            if let Some(running) = self.matches.get_mut(&id) {
                running.predicted_at = Some(now);
            }
        }
    }

    /// Players the match server took into a running match (`joined`: each
    /// with its team and what it counted for): the lobby's roster of the
    /// match grows by them, the map is one more each has played, and the
    /// log has the "late join" line (`scripts/balance-refit` reads it) with
    /// the sides as `placement` has them.
    pub(super) fn seated_late(
        &mut self,
        id: u32,
        joined: &[(session::Player, u8, i32)],
        placement: &policy::LatePlacement,
        lead: [i64; 2],
    ) {
        let unix = quests::now();
        let counted = self.counted(placement.sides, lead);
        let weights = self.settings.balance;
        let scale = f64::from(session::STRENGTH_SCALE);
        // The seats as they are now, the joiners' with them: one that had
        // a place in this match before (it left by its own word and came
        // again) has played its map once already.
        let seats = self
            .store
            .matchmaking()
            .formed_match(id)
            .map(|formed| formed.seats)
            .unwrap_or_default();
        let Some(running) = self.matches.get_mut(&id) else {
            return;
        };
        let ticket = running.ticket.match_id;
        let map = maps::map_mode(running.map_id).map_or(0, |map| map.level);
        let mut said = Vec::new();
        for &(player, team, strength) in joined {
            running.seats.push((player.session_id, team));
            running.strengths.push(strength);
            let mut name = String::new();
            if let Some(account) = self.accounts.get(player.account_id) {
                let mut account = account.locked();
                name.clone_from(&account.name);
                said.push(format!(
                    "{} to team {team}",
                    counted_player(
                        &account,
                        weights.player + f64::from(strength) / scale,
                        matchmaking::level(&account, unix).min
                    )
                ));
                let places = seats
                    .iter()
                    .filter(|seat| seat.player.account_id == player.account_id)
                    .count();
                if places <= 1 {
                    matchmaking::count_map_play(&mut account, map);
                    self.accounts.mark_dirty(player.account_id);
                }
            }
            running.names.push(name);
        }
        log::info!(
            "Match {ticket:#x} late join: {}; with them team 0 counts {:.1} ({} in, lead \
             {:.1}), team 1 counts {:.1} ({} in, lead {:.1}); the sides were {:.1} apart",
            said.join(", "),
            counted[0].0,
            counted[0].1,
            counted[0].2,
            counted[1].0,
            counted[1].1,
            counted[1].2,
            placement.apart_before as f64 / scale
        );
    }

    /// The account's level (its gear's, `matchmaking::level`); the lowest
    /// for an account the lobby does not have.
    pub(super) fn level_of(&self, account_id: u32, unix: u64) -> Level {
        match self.accounts.get(account_id) {
            Some(account) => matchmaking::level(&account.locked(), unix),
            None => Level::default(),
        }
    }

    /// What the account counts for when the sides of a match are made, on
    /// top of what every player does (`session::v026g::balance`): its gear
    /// level (`level`), its own level and its balance rating as far as
    /// that is trusted.
    pub(super) fn strength_of(&self, account_id: u32, level: Level) -> i32 {
        let (player_level, balance) = match self.accounts.get(account_id) {
            Some(account) => {
                let account = account.locked();
                (account.level(), account.balance)
            }
            None => (0, Default::default()),
        };
        self.settings
            .balance
            .strength(level.min, player_level, balance)
    }

    /// `2F <kit id>`: Play. After a choice of a match against the server's
    /// bots (`pvb`) it makes that match; with a place in a match, back to
    /// it; else into a room (`rooms::room_play`).
    pub(super) fn play(&mut self, who: &Who, kit_id: u32, now: Instant) {
        // A choice of a match against bots, live or run out within
        // `pvb::EXPIRED_REFUSED`: used up by this Play, whatever comes of it.
        let pvb_hold = self
            .pvb_holds
            .remove(&who.account_id)
            .filter(|held| now < held.until + super::pvb::EXPIRED_REFUSED);
        if self.pool_for_sessions([who.session_id]).is_none() {
            return;
        }
        let unix = quests::now();
        let checked = {
            let mut account = who.account.locked();
            account.settle_active_kit(unix);
            rules::check_play(&account, kit_id, unix)
        };
        if let Err(error) = checked {
            log::info!("{}: Play refused: {error}", who.name);
            let key = match error {
                rules::ActionError::NoWeapon | rules::ActionError::NoAmmo => error.text_key(),
                _ => PLAY_KIT,
            };
            self.deny(who.addr, id::PLAY, key);
            return;
        }
        // A match that ended without a word frees its seats first: Play
        // would send the player back to it.
        self.check_matches(now);
        if let Some(held) = pvb_hold {
            if held.until > now {
                self.play_vs_bots(who, kit_id, held, now);
            } else {
                // The player meant bots: never a room.
                log::info!(
                    "{}: Play vs bots ({} v {} on map {}): refused (its choice ran out {} ms \
                     ago)",
                    who.name,
                    held.size,
                    held.size,
                    held.map_id,
                    now.saturating_duration_since(held.until).as_millis()
                );
                self.deny(who.addr, id::PLAY, super::pvb::PLAY_REFUSED);
            }
            return;
        }
        // A player with a place in a match (it closed the match early, or
        // signed in again) goes back to it, as in 0.21d.
        let player = session::Player {
            session_id: who.session_id,
            account_id: who.account_id,
            profile_id: kit_id,
        };
        if self.store.matchmaking().back_to_seat(player) {
            if let Some(player) = self.player_mut(who.addr) {
                player.queued_since = Some(now);
            }
            self.queue_status(who.addr, now);
            return;
        }
        self.room_play(who, now);
    }

    /// `31 <order> <left match>`: the player stopped waiting (a clan
    /// team's wait), or is back from a match it closed. Either way its
    /// client is in the lobby.
    pub(super) fn leave(&mut self, who: &Who, left_match: bool) {
        if !left_match && let Some(account) = self.store.account(who.account_id) {
            account.locked().play = None;
        }
        if let Some(player) = self.player_mut(who.addr) {
            player.queued_since = None;
            player.in_match = None;
            player.connect = None;
        }
        if !left_match {
            // A clan team it waited with does not wait any more.
            self.clan_left_queue(who.account_id);
        }
        log::info!(
            "{} {}",
            who.name,
            if left_match {
                "is back from its match"
            } else {
                "stopped waiting"
            }
        );
        self.send(who.addr, lobby::client_state(&ClientState::Lobby, None));
        if left_match {
            // Its match may be over already (the client closed it at the
            // end): the results.
            self.show_results(who.addr, false);
        }
        self.squads_dirty = true;
    }

    /// What the client is told of where it is:
    /// - while it waits (a clan team waiting for another), "searching" and
    ///   how long it has waited (the client shows the wait the server last
    ///   sent);
    /// - once its match is seated, "match found" and where the match server
    ///   is (0x82), once.
    pub(super) fn queue_status(&mut self, addr: SocketAddr, now: Instant) {
        let Some(player) = self.player(addr) else {
            return;
        };
        let (session_id, queued_since, order_id) =
            (player.session_id, player.queued_since, player.order_id);
        let in_match = player.in_match;
        let waiting = move |place: u32, total: u32, since: Instant| {
            lobby::client_state(
                &ClientState::InQueue {
                    order_id,
                    place,
                    queue_total: total,
                    current_wait_seconds: now.saturating_duration_since(since).as_secs() as u32,
                    average_wait_seconds: 0,
                    search_step: SEARCH_STEP,
                    search_properties: [0; limits::SEARCH_PROPERTIES],
                },
                None,
            )
        };
        match self.store.matchmaking().poll(session_id, now) {
            Some(QueueState::Placed {
                match_id,
                team,
                index,
            }) => {
                if queued_since.is_none() && self.store.matchmaking().is_told(session_id) {
                    return;
                }
                if !self.matches.contains_key(&match_id) && !self.launch(match_id) {
                    return;
                }
                let ticket = self.matches[&match_id].ticket.clone();
                // The queue keeps a seat by account; the match server finds
                // a connecting client's place by its session. A player who
                // signed in again and pressed Play comes with a new one.
                let seated_as = self.matches[&match_id]
                    .seats
                    .get(index)
                    .map(|(session, _)| *session);
                if seated_as.is_some_and(|session| session != session_id) {
                    let account_id = self.player(addr).map_or(0, |player| player.account_id);
                    if self.host.reseat(ticket.match_id, account_id, session_id) {
                        log::info!(
                            "Match {:#x}: place {index} is now session {session_id:#x}'s (its \
                             account signed in again)",
                            ticket.match_id
                        );
                    } else {
                        log::warn!(
                            "Match {:#x}: place {index} could not be given to session \
                             {session_id:#x}; its client will be turned away",
                            ticket.match_id
                        );
                    }
                    if let Some(seat) = self
                        .matches
                        .get_mut(&match_id)
                        .and_then(|running| running.seats.get_mut(index))
                    {
                        seat.0 = session_id;
                    }
                }
                self.store.matchmaking().mark_told(session_id);
                let host = self.match_host_for(addr, &ticket);
                let found = waiting(0, 0, now);
                let found = self.send(addr, found);
                // Its game loads a match now: from here on it can show a
                // results screen (`show_results`). Sessions that are over
                // are forgotten here.
                let store = self.store;
                self.sent_to_match.retain(|id| !store.has_ended(*id));
                self.sent_to_match.insert(session_id);
                if let Some(player) = self.player_mut(addr) {
                    player.queued_since = None;
                    player.in_match = Some(match_id);
                    player.connect = Some((
                        found,
                        lobby::connect_to_match(
                            &host,
                            ticket.port,
                            ticket.match_id,
                            team,
                            // The session the client gives the match server
                            // in 0xD8 and 0xD9: the one its seat is kept by.
                            session_id,
                        ),
                    ));
                    log::info!(
                        "{} goes to match {:#x} on team {team} at {host}:{}",
                        player.name,
                        ticket.match_id,
                        ticket.port
                    );
                }
            }
            Some(QueueState::Waiting {
                place,
                total,
                since,
            }) => {
                let status = waiting(place as u32, total as u32, since);
                self.send(addr, status);
            }
            // Its match is over.
            None if in_match.is_some() => {
                if let Some(player) = self.player_mut(addr) {
                    player.in_match = None;
                    player.connect = None;
                }
                self.send(addr, lobby::client_state(&ClientState::Lobby, None));
                self.show_results(addr, false);
            }
            None => {
                // A clan team waits for another.
                if let Some(since) = queued_since {
                    let status = waiting(1, 1, since);
                    self.send(addr, status);
                }
            }
        }
    }

    /// The match server's host for the client at `addr`: one on this
    /// machine or its network is told the address it reached srp at, when
    /// the match server is on this machine too.
    fn match_host_for(&self, addr: SocketAddr, ticket: &MatchTicket) -> String {
        let reached = self
            .player(addr)
            .and_then(|player| player.reached)
            .map(|reached| reached.to_canonical());
        match reached {
            Some(IpAddr::V4(reached))
                if self.settings.local_peers
                    && is_local(addr.ip())
                    && ticket.host == self.settings.public_host =>
            {
                reached.to_string()
            }
            _ => ipv4_host(&ticket.host, ticket.port),
        }
    }

    /// Make the match seated at the match server. `false` when that
    /// failed: its players are back in the lobby.
    ///
    /// A room's match (`rooms`) is on the map its vote chose. A match
    /// against bots (`pvb`, [`Core::pvb_launching`]) has the map chosen and
    /// 2N − 1 of the server's bots for its one player ([`server_bot_teams`]
    /// puts N − 1 of them on the player's side), and counts towards no
    /// player's map rotation. A clan match is on a map of the pool drawn at
    /// random.
    pub(super) fn launch(&mut self, match_id: u32) -> bool {
        let Some(formed) = self.store.matchmaking().formed_match(match_id) else {
            return false;
        };
        let pvb = self.pvb_launching.get(&match_id).copied();
        // A room's match: the map its vote chose (`rooms`).
        let room_map = self.room_launching.get(&match_id).copied();
        let unix = quests::now();
        // Its players' levels.
        let mut levels = Vec::with_capacity(formed.seats.len());
        // Why the sides are as they are, for the log.
        let weights = self.settings.balance;
        let mut sides: [(f64, Vec<String>); 2] = Default::default();
        let mut strengths = Vec::with_capacity(formed.seats.len());
        let mut names = Vec::with_capacity(formed.seats.len());
        let players: Option<Vec<MatchPlayer>> = formed
            .seats
            .iter()
            .map(|seat| {
                let account = self.accounts.get(seat.player.account_id)?;
                let mut account = account.locked();
                account.settle_active_kit(unix);
                let level = matchmaking::level(&account, unix).min;
                levels.push(level);
                let scale = f64::from(session::STRENGTH_SCALE);
                let strength = weights.strength(level, account.level(), account.balance);
                strengths.push(strength);
                names.push(account.name.clone());
                let counts = weights.player + f64::from(strength) / scale;
                let side = &mut sides[usize::from(seat.team.min(1))];
                side.0 += counts;
                side.1.push(counted_player(&account, counts, level));
                Some(MatchPlayer {
                    session_id: seat.player.session_id,
                    account_id: seat.player.account_id,
                    profile: match_profile(&account, seat.team, seat.squad, unix),
                    bot: self.accounts.is_bot_name(&account.name),
                    driven: false,
                })
            })
            .collect();
        // The match server's own bots, after the players: against bots,
        // 2N - 1 of them; none in any other match.
        let seat_teams: Vec<u8> = formed.seats.iter().map(|seat| seat.team).collect();
        let bots = pvb.map_or(0, |choice| {
            (2 * usize::from(choice.size)).saturating_sub(formed.seats.len())
        });
        let bot_teams = server_bot_teams(&seat_teams, [sides[0].0, sides[1].0], bots);
        let bot_roles = server_bot_roles(&bot_teams);
        let players = players.map(|mut players| {
            players.extend(
                bot_teams
                    .iter()
                    .zip(&bot_roles)
                    .enumerate()
                    .map(|(n, (&team, &role))| server_bot(n + 1, team, role)),
            );
            players
        });
        let map_id = match (pvb, room_map) {
            (Some(choice), _) => choice.map_id,
            (None, Some(map_id)) => map_id,
            (None, None) => match self.settings.maps.len() {
                0 => maps::playable().next().map_or(0, |map| map.id),
                count => self.settings.maps[self.rng.below(count as u32) as usize],
            },
        };
        let match_level = matchmaking::match_level(&levels);
        let compatible = self
            .pool_for_sessions(formed.seats.iter().map(|seat| seat.player.session_id))
            == Some(formed.pool);
        let made = match players.filter(|_| compatible) {
            Some(players) => self.host.create(MatchSetup {
                pool: formed.pool,
                map_id,
                match_level,
                players,
                vs_bots: pvb.is_some(),
            }),
            None => Err("a seated player is unavailable or uses an incompatible client".to_owned()),
        };
        match made {
            Ok(ticket) => {
                let map = maps::map_mode(map_id).map_or(0, |map| map.level);
                log::info!(
                    "Match {:#x} made for {} players (levels {levels:?}: match level \
                     {match_level}) on map {map_id} (level_{map:02}), at {}:{}",
                    ticket.match_id,
                    formed.seats.len(),
                    ticket.host,
                    ticket.port
                );
                log::info!(
                    "Match {:#x} sides: team 0 counts {:.1}: {} | team 1 counts {:.1}: {}",
                    ticket.match_id,
                    sides[0].0,
                    sides[0].1.join(", "),
                    sides[1].0,
                    sides[1].1.join(", ")
                );
                if let Some(choice) = pvb {
                    let mode = maps::map_mode(map_id).map_or(0, |map| map.mode);
                    log::info!(
                        "Match {:#x} vs bots: {} with {} of the server's bots against {}, {} v {} \
                         on map {map_id} (level_{map:02}, mode {mode}); {} bot matches of {} at \
                         most",
                        ticket.match_id,
                        sides[0]
                            .1
                            .first()
                            .or(sides[1].1.first())
                            .map_or("?", String::as_str),
                        usize::from(choice.size).saturating_sub(formed.seats.len()),
                        choice.size,
                        choice.size,
                        choice.size,
                        self.pvb_running() + 1,
                        self.settings.pvb.max_matches
                    );
                }
                // The map is one more that each of them has played; a
                // match against bots counts towards no map rotation.
                for seat in formed.seats.iter().filter(|_| pvb.is_none()) {
                    let account_id = seat.player.account_id;
                    if let Some(account) = self.accounts.get(account_id) {
                        matchmaking::count_map_play(&mut account.locked(), map);
                        self.accounts.mark_dirty(account_id);
                    }
                }
                self.matches.insert(
                    match_id,
                    Running {
                        pool: formed.pool,
                        fixed_teams: formed.fixed_teams,
                        ticket,
                        map_id,
                        seats: formed
                            .seats
                            .iter()
                            .map(|seat| (seat.player.session_id, seat.team))
                            .chain(bot_teams.iter().map(|&team| (0, team)))
                            .collect(),
                        strengths: strengths
                            .into_iter()
                            .chain(bot_teams.iter().map(|_| 0))
                            .collect(),
                        names: names
                            .into_iter()
                            .chain((1..=bot_teams.len()).map(server_bot_name))
                            .collect(),
                        predicted_at: None,
                        vs_bots: pvb.is_some(),
                    },
                );
                true
            }
            Err(error) => {
                log::error!(
                    "A match for {} players could not be made: {error}; they are back in the lobby",
                    formed.seats.len()
                );
                self.store.matchmaking().end_match(match_id);
                for seat in &formed.seats {
                    let account_id = seat.player.account_id;
                    if let Some(account) = self.store.account(account_id) {
                        account.locked().play = None;
                    }
                    let Some(addr) = self.by_account.get(&account_id).copied() else {
                        continue;
                    };
                    if let Some(player) = self.player_mut(addr) {
                        player.queued_since = None;
                        player.in_match = None;
                        player.connect = None;
                    }
                    self.send(addr, lobby::client_state(&ClientState::Lobby, None));
                }
                self.squads_dirty = true;
                false
            }
        }
    }

    /// Free the seats of the matches the match server no longer has, and
    /// of a match seated that never reached the match server
    /// (its players' links all went as it formed).
    pub(super) fn check_matches(&mut self, now: Instant) {
        let over: Vec<u32> = self
            .matches
            .iter()
            .filter(|(_, running)| !self.host.is_open(running.ticket.match_id))
            .map(|(id, _)| *id)
            .collect();
        for id in over {
            self.finish(id);
        }
        for (id, formed_at) in self.store.matchmaking().matches() {
            if !self.matches.contains_key(&id)
                && now.saturating_duration_since(formed_at) >= UNMADE_MATCH
            {
                log::warn!("Match {id:#x} was seated but never made; its seats are freed");
                self.store.matchmaking().end_match(id);
            }
        }
    }

    /// [`super::Inbox::match_finished`], by the id the clients were told.
    pub(super) fn match_finished(&mut self, ticket_match_id: u32, _now: Instant) {
        let id = self
            .matches
            .iter()
            .find(|(_, running)| running.ticket.match_id == ticket_match_id)
            .map(|(id, _)| *id);
        match id {
            Some(id) => self.finish(id),
            None => log::debug!("Match {ticket_match_id:#x} is over; the lobby had freed it"),
        }
    }

    /// The match is over: its seats are free, and its players who are
    /// still marked in it are told they are in the lobby (`85 00`; the
    /// client's Play is off in any other state).
    fn finish(&mut self, match_id: u32) {
        let Some(running) = self.matches.remove(&match_id) else {
            return;
        };
        self.store.matchmaking().end_match(match_id);
        // A room's match: its players are back in the room.
        self.room_match_over(match_id);
        log::info!(
            "Match {:#x} (map {}) is over: {} seats freed",
            running.ticket.match_id,
            running.map_id,
            running.seats.len()
        );
        for (session_id, _) in &running.seats {
            // A session that ended during the match was kept for it.
            self.store.seen_in_match(*session_id);
        }
        for addr in self.players() {
            let Some(player) = self.player_mut(addr) else {
                continue;
            };
            if player.in_match != Some(match_id) {
                continue;
            }
            player.in_match = None;
            player.connect = None;
            self.send(addr, lobby::client_state(&ClientState::Lobby, None));
            self.show_results(addr, false);
        }
    }

    /// The results of the matches the account's client has not been shown
    /// (`session::v026g::rewards`), once: the screen of the last one (`97`)
    /// and the notices of all of them. A client that was signed in through
    /// the match's end gets what the match changed of its account first
    /// (`MatchReport::lobby_push`); one whose link is new (`after_sign_in`:
    /// it signed in, or connected again) has that from the sign-in's pushes.
    /// Call it when the client is in the lobby (`85 00`).
    ///
    /// A client whose session the lobby has sent into no match
    /// (`sent_to_match`) gets the notices and **no screen**: the game would
    /// die of it. The screen's code translates the map's name it kept from
    /// the loading screen of its last match, and a game that has loaded no
    /// match since it was started has none (a null pointer: `results.md` §6
    /// hazard 10, seen in a crash dump on 2026-10-13). That is the player
    /// whose game ended in a match and who starts it again, at the sign-in
    /// when the match is over by then, and in the lobby when it ends later.
    /// What the match changed of the account, such a client has from its
    /// sign-in or gets with the account's next push.
    pub(super) fn show_results(&mut self, addr: SocketAddr, after_sign_in: bool) {
        let Some(who) = self.who(addr) else {
            return;
        };
        let Some(unseen) = self.results.take_unseen(who.account_id) else {
            return;
        };
        // What it earned is its clan's too.
        if let Some(player) = unseen.report.player(who.account_id) {
            self.clan_experience(who.account_id, player.reward.experience);
        }
        if !self.sent_to_match.contains(&who.session_id) {
            log::info!(
                "{} gets the notices of match {:#x} and no results screen: its game has loaded no match since it signed in, and would die of one",
                who.name,
                unseen.report.match_id
            );
            self.notify(&who, &unseen.notices);
            return;
        }
        let push = if after_sign_in {
            // The inventory once more if autobuy or auto-repair spent
            // anything: the screen lists it from there.
            let account = who.account.locked();
            unseen.report.screen_for(who.account_id).map(|screen| {
                let mut push = Vec::new();
                if let Some(spent) = unseen.report.spent_inventory(who.account_id, &account) {
                    push.push(spent?);
                }
                push.push(screen?);
                Ok(push)
            })
        } else {
            // With no account locked: the ranking locks each in turn.
            let place = self
                .accounts
                .ranking()
                .iter()
                .position(|row| row.0 == who.account_id)
                .map(|index| index as u32 + 1);
            let account = who.account.locked();
            unseen
                .report
                .lobby_push(who.account_id, &account, quests::now(), place)
        };
        match push {
            Some(Ok(push)) => {
                log::info!(
                    "{} is shown the results of match {:#x}",
                    who.name,
                    unseen.report.match_id
                );
                self.send_all(addr, push);
                let balance = {
                    let account = who.account.locked();
                    (account.generic_money, account.premium_money)
                };
                if let Some(player) = self.player_mut(addr) {
                    player.money_sent = balance;
                }
            }
            Some(Err(error)) => log::error!(
                "The results of match {:#x} could not be sent to {}: {error}",
                unseen.report.match_id,
                who.name
            ),
            None => {}
        }
        self.notify(&who, &unseen.notices);
        // The daily bonus as the match left it, and a premium offer
        // (`shop`).
        self.after_results(&who, &unseen.report);
    }

    /// The team of a player in the match it was sent to.
    pub(super) fn team_in_match(&self, match_id: u32, session_id: u32) -> Option<u8> {
        self.matches
            .get(&match_id)?
            .seats
            .iter()
            .find(|(session, _)| *session == session_id)
            .map(|(_, team)| *team)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_client_is_given_an_ipv4_address() {
        assert_eq!(ipv4_host("203.0.113.7", 1236), "203.0.113.7");
        // A name is looked up: the client takes no name.
        assert_eq!(ipv4_host("localhost", 1236), "127.0.0.1");
    }

    #[test]
    fn this_machine_and_its_network_are_local() {
        let local = |ip: &str| is_local(ip.parse().unwrap());
        assert!(local("127.0.0.1") && local("192.168.1.20") && local("10.0.0.5"));
        assert!(local("::1") && local("::ffff:192.168.1.20"));
        assert!(!local("203.0.113.7") && !local("100.64.0.1"));
    }

    #[test]
    fn a_server_bot_wears_its_roles_kit_and_a_client_reads_it() {
        use survarium::v026g::dictionary::Dictionary;
        use survarium::v026g::loadouts::{self, bot_kit_items, bot_role_of, bot_second_kit_items};
        use survarium::v026g::match_msgs::static_info::{
            KIT_USABLE, MatchOptions, SCORE_WEIGHTS, StaticInfo, parse_payload,
        };

        let dictionary = Dictionary::shared();
        let bots: Vec<MatchPlayer> = BotRole::ALL
            .iter()
            .enumerate()
            .map(|(n, &role)| server_bot(n + 1, (n % 2) as u8, role))
            .collect();
        for (bot, role) in bots.iter().zip(BotRole::ALL) {
            let profile = &bot.profile;
            // Its role's kit, then the second (its backup's weapon and
            // ammo), no item in both.
            assert_eq!(profile.kits.len(), 2);
            let used: Vec<u16> = profile
                .kits
                .iter()
                .flat_map(|kit| kit.slots.iter().flatten().map(|(index, _)| *index))
                .collect();
            let mut once = used.clone();
            once.sort_unstable();
            once.dedup();
            assert_eq!(once.len(), used.len(), "{role}: an item in two kits");
            for (k, kit) in profile.kits.iter().enumerate() {
                assert_eq!(kit.id, k as u32 + 1);
                assert_eq!(kit.state, KIT_USABLE);
                let items: Vec<_> = if k == 0 {
                    bot_kit_items(role).collect()
                } else {
                    bot_second_kit_items(role).collect()
                };
                // Every piece of the kit in its slot, the count as given;
                // nothing else.
                assert_eq!(
                    kit.slots.iter().filter(|slot| slot.is_some()).count(),
                    items.len()
                );
                for wanted in items {
                    let (index, count) = kit.slots[usize::from(wanted.slot)]
                        .unwrap_or_else(|| panic!("{role}: nothing in slot {}", wanted.slot));
                    let item = &profile.items[usize::from(index)];
                    assert_eq!(
                        dictionary.item(item.dict_id).unwrap().name,
                        wanted.name,
                        "{role}: slot {}",
                        wanted.slot
                    );
                    assert_eq!(u32::from(count), wanted.count, "{}", wanted.name);
                }
            }
            // The medkits, the radio and the grenades in the open pockets.
            let (pockets, _) = loadouts::pockets(loadouts::bot_kit(role));
            for item in [
                loadouts::BOT_MEDKIT,
                loadouts::BOT_RADIO,
                loadouts::BOT_GRENADE,
            ] {
                assert!(loadouts::POCKET_SLOTS[..pockets as usize].contains(&item.slot));
            }
        }
        // The client's reader takes the profiles as they are, and the role
        // reads back from what it read.
        let options = MatchOptions {
            players: bots.iter().map(|bot| bot.profile.clone()).collect(),
            map_id: maps::playable().next().unwrap().id,
            match_id: 0x1234,
            match_level: 1,
            score_weights: SCORE_WEIGHTS,
        };
        let info = StaticInfo::new(&options, dictionary).expect("a client can hold it");
        let (index, read) = parse_payload(&info.payload(1).unwrap(), dictionary).unwrap();
        assert_eq!(index, 1);
        for ((read, bot), role) in read.players.iter().zip(&bots).zip(BotRole::ALL) {
            assert_eq!(read.kits, bot.profile.kits);
            assert_eq!(read.items, bot.profile.items);
            assert_eq!(bot_role_of(read), Some(role));
        }
    }

    /// A scope on a weapon reaches the match as its record: the owned scope
    /// item's id and the variant made for the weapon (whose modification
    /// the client applies, `survarium::v026g::scopes`), in slot 3; the
    /// client's reader of the static info takes it as it is.
    #[test]
    fn a_scope_on_a_weapon_reaches_the_match_as_its_variant() {
        use session::v026g::account::Upgrade as Record;
        use survarium::v026g::dictionary::Dictionary;
        use survarium::v026g::lobby::FULL_CONDITION;
        use survarium::v026g::match_msgs::static_info::{
            MatchOptions, SCORE_WEIGHTS, StaticInfo, parse_payload,
        };

        let dictionary = Dictionary::shared();
        // A new account's kit, which a match loads; its weapon gets the
        // scope.
        let mut account = Account::new("Alice");
        let weapon = [7, 10]
            .into_iter()
            .find_map(|slot| account.kits[0].slots[slot])
            .expect("a new account's weapon")
            .item_id;
        // 0.26g's Leupold stands for a shop scope and its 1P69 (362) for
        // the variant made for the weapon (the test data has none; the wire
        // sends no record whose dict id the data lacks, and the record is
        // otherwise carried as it is).
        let scope = account.add_item(69, FULL_CONDITION);
        let record = Record {
            id: scope,
            a: 362,
            value: 0,
            param: 3,
        };
        account.item_mut(weapon).unwrap().upgrades = vec![record];

        let profile = match_profile(&account, 0, 0, 0);
        let item = profile
            .items
            .iter()
            .find(|item| item.instance_id == weapon)
            .expect("the worn weapon");
        assert_eq!(
            item.upgrades,
            [Upgrade {
                id: scope,
                unknown_4: 362,
                unknown_8: 0,
                param: 3,
            }]
        );
        // The scope item itself is in no kit: not in the match.
        assert!(profile.items.iter().all(|item| item.instance_id != scope));
        let options = MatchOptions {
            players: vec![profile.clone()],
            map_id: maps::playable().next().unwrap().id,
            match_id: 0x1234,
            match_level: 1,
            score_weights: SCORE_WEIGHTS,
        };
        let info = StaticInfo::new(&options, dictionary).expect("a client can hold it");
        let (_, read) = parse_payload(&info.payload(0).unwrap(), dictionary).unwrap();
        assert_eq!(read.players[0].items, profile.items);
    }

    #[test]
    fn each_teams_bots_take_the_roles_by_the_shares() {
        use BotRole::*;
        assert_eq!(
            server_bot_roles(&[0, 1, 0, 1, 0, 1, 0, 1]),
            [
                Flanker, Flanker, Rusher, Rusher, Support, Support, Camper, Camper
            ]
        );
        assert_eq!(server_bot_roles(&[1, 1, 1]), [Flanker, Rusher, Support]);
        assert!(server_bot_roles(&[]).is_empty());
    }
}
