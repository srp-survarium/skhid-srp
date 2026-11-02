//! "Player vs Bots" ("Гравець проти ботів"): one player against the match
//! server's own bots, chosen in the match settings panel of srp's changed
//! lobby movie (`private/pvb-mode/DESIGN.md`, the owner's decisions of
//! 2026-10-16; the wire contract `docs/notes/lobby.md` §2.16.2).
//!
//! The movie asks with the ranking page's own request (`3B`) at starts no
//! ranking page has, as the list of the matches does:
//!
//! 1. **Status** (`3B 00 <PVB_STATUS_START> <count>`, when the panel opens):
//!    whether the player may play against bots now and why not
//!    ([`lobby::PvbCode`]), its equipment level and the most allowed, the
//!    bot matches running and the most at once, the sizes and the maps
//!    offered with their free slots ([`lobby::pvb_status`]).
//! 2. **Choose** (`3B 00 <PVB_CHOOSE_BASE + size × 1000 + map> 1`): checked
//!    by the same rule ([`Core::pvb_fit`]); `B2;ok` holds the choice for the
//!    player for [`HOLD`] (one hold an account, a new choice replaces it),
//!    `B2;no;<code>;…` says why not.
//! 3. **Play** (`2F <kit>`, the movie presses it on `ok`): with a live hold
//!    the rule is checked again and the match made at once with this player
//!    alone, on team 0 against the server's bots, the teams fixed
//!    (`Matchmaking::seat_solo`): N − 1 bots on its side and N on the
//!    other (`play::server_bot_teams`), on the map chosen. No rule of two
//!    parties, of levels or of a gather window; the play counts towards no
//!    map rotation. A Play whose hold ran out within [`EXPIRED_REFUSED`]
//!    is refused (`st_cant_change_ready_state`) and never goes into a
//!    room: the player meant bots.
//!
//! A match against bots takes no late joiner (its teams are fixed:
//! `play::late_admission`). Its rewards are
//! `session::v026g::rewards`' for `vs_bots`: [`SRP_PVB_PAY`] of the silver
//! and experience, no quest, no daily bonus, no rating. The match server
//! ends it when its player stops uploading the worlds it is asked for
//! (`SRP_PVB_STALL_S`), and keeps one bots' map in memory for each match
//! that may run (`SRP_PVB_MAX_MATCHES`).
//!
//! # The rule ([`Core::pvb_fit`])
//!
//! The first reason of this order is the one said: the mode is off (6);
//! the player's game is too old (5: it does not play in growing rosters);
//! it is in a match, sent to one, or has a place in one (4); it waits with
//! its clan team (3); it is in a squad with others (2); it is in
//! its clan's team (9); its equipment level (the client's, of the kits it
//! can play: `matchmaking::gear_level`) is over [`Config::max_gear`] (1);
//! a planned maintenance is 30 minutes away or less (8); as many matches
//! against bots run, or are held for players who chose one, as
//! [`Config::max_matches`] (7).
//!
//! # Settings (environment, read once at start)
//!
//! | Setting | Default | What | Read by |
//! |---|---|---|---|
//! | `SRP_PVB` | off | the mode is offered (`on`) | the lobby, the match server |
//! | `SRP_PVB_MAX_MATCHES` | 5 | the most matches against bots at once, 1 to 20; the match server keeps up to that many bots' maps in memory | the lobby, the match server |
//! | `SRP_PVB_PAY` | 0.5 | the share of a match's silver and experience it pays, 0 to 1 | the rewards (`session::v026g::pvb`) |
//! | `SRP_PVB_STALL_S` | 60 | a match whose only player uploads no world it is asked for this long is over, 10 to 3600 | the match server |
//!
//! [`SRP_PVB_PAY`]: session::v026g::pvb::pay

use std::time::{Duration, Instant};

use session::v026g::{matchmaking, quests};
use session::{LockExt, QueuePool};
use survarium::v026g::lobby::requests::id;
use survarium::v026g::lobby::rooms::rules;
use survarium::v026g::lobby::{self, PvbCode, PvbMap, PvbNumbers, PvbStatus};
use survarium::v026g::maps::{self, MapMode};

use super::core::{Core, Who};

/// The game's text for a Play the choice no longer makes: "Error changing
/// readiness status." in the player's language, what the game says of a
/// Play-like press it could not carry out (the clan team's ready, `clans`).
pub(super) const PLAY_REFUSED: &str = "st_cant_change_ready_state";

/// The maps offered, by `maps_dict` id (the owner's list of 2026-10-16):
/// the day of each of the eight levels in batteries (mode 2: 61 Rudnya to
/// 103) and in Team Deathmatch (mode 4: 117 to 138). No timed batteries
/// (mode 3), no Research (mode 5: the server's bots play modes 2 to 4), no
/// London (level 9, maps 181 to 183) and no level 88. The movie shows only
/// the maps the status lists.
pub const MAPS: [u8; 16] = [
    61, 67, 73, 79, 85, 91, 97, 103, 117, 120, 123, 126, 129, 132, 135, 138,
];
/// The map the movie shows first (the team sizes and the size it shows first: `PvbRules`).
pub const DEFAULT_MAP: u8 = 61;
/// How long a choice is held for the player's Play.
pub const HOLD: Duration = Duration::from_secs(10);
/// A Play this long after its hold ran out is still taken as one meant for
/// bots: refused, never into a room.
pub const EXPIRED_REFUSED: Duration = Duration::from_secs(30);

/// The mode's settings in the lobby.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// `SRP_PVB` (default off).
    pub enabled: bool,
    /// The highest equipment level let in (`PvbRules::max_gear`).
    pub max_gear: u8,
    /// `SRP_PVB_MAX_MATCHES` (default 5).
    pub max_matches: usize,
}

impl Default for Config {
    /// Off; the owner's numbers.
    fn default() -> Self {
        Self {
            enabled: false,
            max_gear: rules().pvb.max_gear,
            max_matches: session::v026g::pvb::DEFAULT_MAX_MATCHES,
        }
    }
}

impl Config {
    /// The environment's.
    pub fn from_env() -> Self {
        let config = Self {
            enabled: session::v026g::pvb::enabled(),
            max_gear: rules().pvb.max_gear,
            max_matches: session::v026g::pvb::max_matches(),
        };
        if config.enabled {
            log::info!(
                "Player vs Bots is on: equipment level {} at most, {} matches at once, pay {} of \
                 silver and experience",
                config.max_gear,
                config.max_matches,
                session::v026g::pvb::pay()
            );
        }
        config
    }
}

/// The offered maps as the dictionary has them.
pub fn offered_maps() -> impl Iterator<Item = &'static MapMode> {
    MAPS.iter().filter_map(|&id| maps::map_mode(id))
}

/// A choice held for a player's Play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Hold {
    pub size: u8,
    pub map_id: u8,
    pub until: Instant,
}

/// What a match against bots being made was chosen with: its size and
/// map (`play::launch` takes them).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Choice {
    pub size: u8,
    pub map_id: u8,
}

/// Why not, in words for the log.
fn why(code: PvbCode) -> &'static str {
    match code {
        PvbCode::Allowed => "allowed",
        PvbCode::GearTooHigh => "equipment level too high",
        PvbCode::InSquad => "in a squad",
        PvbCode::Searching => "waits in the queue",
        PvbCode::InMatch => "in a match or holding a place in one",
        PvbCode::Outdated => "its game is too old",
        PvbCode::Off => "the mode is off",
        PvbCode::AtLimit => "the most bot matches run",
        PvbCode::Maintenance => "a maintenance is near",
        PvbCode::ClanTeam => "in its clan's team",
        PvbCode::UnknownChoice => "no size or map offered",
    }
}

impl Core<'_> {
    /// The matches against bots running now (made, and not over at the
    /// match server).
    pub(super) fn pvb_running(&self) -> usize {
        self.matches
            .values()
            .filter(|running| running.vs_bots && self.host.is_open(running.ticket.match_id))
            .count()
    }

    /// The choices held now for players other than `asker`.
    fn pvb_held(&self, asker: u32, now: Instant) -> usize {
        self.pvb_holds
            .iter()
            .filter(|(account_id, hold)| **account_id != asker && hold.until > now)
            .count()
    }

    /// What a refusal is shown with: the asker's equipment level and the
    /// most, the matches running or held for others and the most.
    fn pvb_numbers(&self, who: &Who, now: Instant) -> PvbNumbers {
        let gear = matchmaking::gear_level(&who.account.locked(), quests::now());
        PvbNumbers {
            gear,
            max_gear: self.settings.pvb.max_gear,
            running: (self.pvb_running() + self.pvb_held(who.account_id, now)) as u32,
            max_matches: self.settings.pvb.max_matches as u32,
        }
    }

    /// Whether `who` may start a match against bots now (the module's
    /// rule, its reasons in its order). A choice it holds itself is no
    /// other player's.
    pub(super) fn pvb_fit(&self, who: &Who, now: Instant) -> Result<(), PvbCode> {
        let config = &self.settings.pvb;
        if !config.enabled {
            return Err(PvbCode::Off);
        }
        if self.pool_for_sessions([who.session_id]) != Some(QueuePool::GrowingRoster) {
            return Err(PvbCode::Outdated);
        }
        let Some(player) = self.player(who.addr) else {
            return Err(PvbCode::InMatch);
        };
        if player.in_match.is_some()
            || player.connect.is_some()
            || self
                .store
                .matchmaking()
                .pool_of_account(who.account_id)
                .is_some()
        {
            return Err(PvbCode::InMatch);
        }
        if player.queued_since.is_some() {
            return Err(PvbCode::Searching);
        }
        if self
            .store
            .squads()
            .of(who.account_id)
            .is_some_and(|squad| squad.members.len() > 1)
        {
            return Err(PvbCode::InSquad);
        }
        if self.accounts.clans().team_of(who.account_id).is_some() {
            return Err(PvbCode::ClanTeam);
        }
        if matchmaking::gear_level(&who.account.locked(), quests::now()) > config.max_gear {
            return Err(PvbCode::GearTooHigh);
        }
        if self.maintenance_is_near() {
            return Err(PvbCode::Maintenance);
        }
        if self.pvb_running() + self.pvb_held(who.account_id, now) >= config.max_matches {
            return Err(PvbCode::AtLimit);
        }
        Ok(())
    }

    /// Holds that ran out longer ago than [`EXPIRED_REFUSED`] go.
    fn pvb_forget_old(&mut self, now: Instant) {
        self.pvb_holds
            .retain(|_, hold| now < hold.until + EXPIRED_REFUSED);
    }

    /// `3B 00 <PVB_STATUS_START> <count>`: the status ([`lobby::pvb_status`]).
    pub(super) fn pvb_status(&mut self, who: &Who, count: u8, now: Instant) {
        self.pvb_forget_old(now);
        let code = self.pvb_fit(who, now).err().unwrap_or(PvbCode::Allowed);
        let numbers = self.pvb_numbers(who, now);
        let free = if self.settings.pvb.enabled {
            numbers.max_matches.saturating_sub(numbers.running)
        } else {
            0
        };
        let maps: Vec<PvbMap> = offered_maps()
            .filter(|map| map.is_playable())
            .map(|map| PvbMap {
                map_id: map.id,
                level: map.level,
                mode: map.mode,
                free,
            })
            .collect();
        let status = PvbStatus {
            code,
            numbers,
            sizes: rules().pvb.sizes.clone(),
            default_size: rules().pvb.default_size,
            default_map: DEFAULT_MAP,
        };
        log::debug!(
            "{}: vs bots status: code {} ({}), gear {} of {}, {} of {} bot matches",
            who.name,
            code as u8,
            why(code),
            numbers.gear,
            numbers.max_gear,
            numbers.running,
            numbers.max_matches
        );
        self.send_made(who.addr, lobby::pvb_status(&status, &maps, count));
    }

    /// `3B 00 <PVB_CHOOSE_BASE + size × 1000 + map> <count>`: a choice. It
    /// replaces the asker's choice held before, granted or not.
    pub(super) fn pvb_choose(&mut self, who: &Who, start: u32, now: Instant) {
        self.pvb_forget_old(now);
        self.pvb_holds.remove(&who.account_id);
        let choice = lobby::pvb_choice(start)
            .filter(|(size, map_id)| rules().pvb.sizes.contains(size) && MAPS.contains(map_id));
        let fit = match choice {
            None => Err(PvbCode::UnknownChoice),
            Some(choice) => self.pvb_fit(who, now).map(|()| choice),
        };
        match fit {
            Ok((size, map_id)) => {
                self.pvb_holds.insert(
                    who.account_id,
                    Hold {
                        size,
                        map_id,
                        until: now + HOLD,
                    },
                );
                log::info!(
                    "{}: vs bots, {size} v {size} on map {map_id}: ok, held for {} s",
                    who.name,
                    HOLD.as_secs()
                );
                self.send_made(who.addr, lobby::pvb_choose_ok(size, map_id));
            }
            Err(code) => {
                let numbers = self.pvb_numbers(who, now);
                log::info!(
                    "{}: vs bots, start {start}: no, code {} ({}); gear {} of {}, {} of {} bot \
                     matches",
                    who.name,
                    code as u8,
                    why(code),
                    numbers.gear,
                    numbers.max_gear,
                    numbers.running,
                    numbers.max_matches
                );
                self.send_made(who.addr, lobby::pvb_choose_no(start, code, numbers));
            }
        }
    }

    /// Play with a choice held (`hold`, live): the rule is checked again,
    /// and the match is made at once with this player alone against the
    /// server's bots (`play::launch` takes the choice). Otherwise the Play
    /// is refused ([`PLAY_REFUSED`]) and the player stays in the lobby, in
    /// no room.
    pub(super) fn play_vs_bots(&mut self, who: &Who, kit_id: u32, hold: Hold, now: Instant) {
        let refuse = |core: &mut Self, why: String| {
            log::info!(
                "{}: Play vs bots ({} v {} on map {}): refused ({why}); not queued",
                who.name,
                hold.size,
                hold.size,
                hold.map_id
            );
            core.deny(who.addr, id::PLAY, PLAY_REFUSED);
        };
        if let Err(code) = self.pvb_fit(who, now) {
            refuse(self, format!("{}; code {}", why(code), code as u8));
            return;
        }
        let Some(pool) = self.pool_for_sessions([who.session_id]) else {
            refuse(self, "its game's revision is not known".to_owned());
            return;
        };
        let player = session::Player {
            session_id: who.session_id,
            account_id: who.account_id,
            profile_id: kit_id,
        };
        let Some(match_id) = self.store.matchmaking().seat_solo(player, 0, now, pool) else {
            refuse(self, "it has a place in a match".to_owned());
            return;
        };
        if let Some(account) = self.store.account(who.account_id) {
            account.locked().play = Some(player);
        }
        let order_id = self.next_order_id;
        // 0xFFFFFFFF is the client's "not queued".
        self.next_order_id = (self.next_order_id + 1) % 0x7FFF_FFFF + 1;
        if let Some(state) = self.player_mut(who.addr) {
            state.queued_since = Some(now);
            state.order_id = order_id;
        }
        log::info!(
            "{} pressed Play vs bots with kit {kit_id}: {} v {} on map {}",
            who.name,
            hold.size,
            hold.size,
            hold.map_id
        );
        self.pvb_launching.insert(
            match_id,
            Choice {
                size: hold.size,
                map_id: hold.map_id,
            },
        );
        self.squads_dirty = true;
        // "Match found" and where the match server is (0x82): the match is
        // made on the way (`launch`).
        self.queue_status(who.addr, now);
        self.pvb_launching.remove(&match_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The owner's list: the day of each level in batteries and in Team
    /// Deathmatch, every one playable on this server's data; no London
    /// (level 9: maps 181 to 183), no level 88 and no timed batteries.
    #[test]
    fn the_maps_are_the_days_of_batteries_and_team_deathmatch_and_playable() {
        assert_eq!(offered_maps().count(), MAPS.len());
        let mut seen = std::collections::HashSet::new();
        for map in offered_maps() {
            assert!(map.is_playable(), "{} is not playable", map.id);
            assert_eq!(map.weather, maps::weather::DAY, "{} is not day", map.id);
            assert!(
                matches!(
                    map.mode,
                    maps::mode::BATTERIES | maps::mode::TEAM_DEATHMATCH
                ),
                "{} is mode {}",
                map.id,
                map.mode
            );
            assert!(
                (1..=8).contains(&map.level),
                "{} is level {}",
                map.id,
                map.level
            );
            assert!(seen.insert((map.level, map.mode)), "{} twice", map.id);
        }
        // Each of the eight levels in both modes.
        assert_eq!(seen.len(), 16);
        for london in [181, 182, 183] {
            assert!(!MAPS.contains(&london));
        }
        for map in maps::all() {
            if map.level == 9
                || map.level == 88
                || matches!(map.mode, maps::mode::TIMED_BATTERIES | maps::mode::RESEARCH)
            {
                assert!(!MAPS.contains(&map.id), "{} is offered", map.id);
            }
        }
        assert!(MAPS.contains(&DEFAULT_MAP));
        let rudnya = maps::map_mode(DEFAULT_MAP).unwrap();
        assert_eq!((rudnya.level, rudnya.mode), (1, maps::mode::BATTERIES));
    }

    #[test]
    fn the_settings_read_as_documented() {
        let config = Config::default();
        assert!(!config.enabled);
        assert_eq!(
            (config.max_gear, config.max_matches),
            (rules().pvb.max_gear, 5)
        );
    }
}
