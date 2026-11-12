//! Tests of the 0.26g lobby, against the real store, accounts and rules
//! and a scripted client (`client`):
//! - `core_tests`: the lobby's core with clients in memory and a clock the
//!   test moves, so a room's wait or a timeout takes no time;
//! - `udp_tests`: the whole server over real localhost UDP.

use super::test_client as client;

mod core_tests;
mod udp_tests;

use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use session::{LockExt, SessionStore};
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::lobby::client_model::Decoded;
use survarium::v026g::maps;
use survarium::v026g::match_msgs::static_info::{MatchOptions, SCORE_WEIGHTS, StaticInfo};

use super::{MatchHost, MatchSetup, MatchTicket};

/// The match server's side of the hand-off: it keeps what the lobby asked
/// for and checks it as the real registry does (the options must be ones a
/// client can hold).
#[derive(Default)]
pub(super) struct TestHost {
    pub made: Mutex<Vec<(u32, MatchSetup)>>,
    pub open: Mutex<HashSet<u32>>,
    pub fail: AtomicBool,
    pub admitting: Mutex<HashSet<u32>>,
    pub rejecting: Mutex<HashSet<u32>>,
    /// Why a match not admitting takes nobody, when the test says.
    pub closed: Mutex<std::collections::HashMap<u32, session::matchmaking_policy::Closed>>,
    /// How a match stands, when the test says: who is in it and the score.
    pub live: Mutex<std::collections::HashMap<u32, super::LiveView>>,
    /// Every place given to a new session: `(match, account, session)`.
    pub reseated: Mutex<Vec<(u32, u32, u32)>>,
    /// The match of every `join_running` asked, taken or not.
    pub join_asks: Mutex<Vec<u32>>,
}

/// Where the test host says its matches are.
pub(super) const MATCH_PORT: u16 = 1236;

impl MatchHost for TestHost {
    fn create(&self, setup: MatchSetup) -> Result<MatchTicket, String> {
        if self.fail.load(Ordering::Relaxed) {
            return Err("the match server is down".to_owned());
        }
        // A free-for-all, as the registry makes it (`MatchRegistry::create`):
        // no more players than its most, each place on a side of its own
        // and in no squad, whatever the lobby seated it as. `made` keeps
        // what the lobby asked for.
        let map = setup.map.map();
        if maps::player_cap(map).is_some_and(|cap| setup.players.len() > cap) {
            return Err(format!("a roster of {}", setup.players.len()));
        }
        let free_for_all = map.mode == maps::GameMode::FreeForAll;
        let mut made = self.made.locked();
        let match_id = 0xA000 + made.len() as u32;
        let options = MatchOptions {
            players: setup
                .players
                .iter()
                .enumerate()
                .map(|(place, player)| {
                    let mut profile = player.profile.clone();
                    if free_for_all {
                        profile.team = maps::free_for_all_team(place);
                        profile.squad = 0;
                    }
                    profile
                })
                .collect(),
            map: setup.map,
            match_id,
            match_level: setup.match_level,
            score_weights: SCORE_WEIGHTS,
        };
        StaticInfo::new(&options, Dictionary::shared()).map_err(|error| error.to_string())?;
        self.open.locked().insert(match_id);
        made.push((match_id, setup));
        Ok(MatchTicket {
            match_id,
            host: "127.0.0.1".to_owned(),
            port: MATCH_PORT,
        })
    }

    fn join_running(&self, match_id: u32, players: Vec<super::MatchPlayer>) -> bool {
        self.join_asks.locked().push(match_id);
        if !self.admitting.locked().contains(&match_id)
            || self.rejecting.locked().contains(&match_id)
        {
            return false;
        }
        let mut made = self.made.locked();
        let Some((_, setup)) = made.iter_mut().find(|(id, _)| *id == match_id) else {
            return false;
        };
        assert_eq!(setup.pool, session::QueuePool::GrowingRoster);
        assert!(!players.is_empty());
        // A free-for-all's every place is given once (`JoinError::Full`).
        if maps::player_cap(setup.map.map())
            .is_some_and(|cap| setup.players.len() + players.len() > cap)
        {
            return false;
        }
        setup.players.extend(players);
        self.admitting.locked().remove(&match_id);
        true
    }

    fn live(&self, match_id: u32, _: std::time::Instant) -> Option<super::LiveView> {
        self.live.locked().get(&match_id).copied()
    }

    fn admits(&self, match_id: u32, _: std::time::Instant) -> bool {
        self.admitting.locked().contains(&match_id)
    }

    fn closed(
        &self,
        match_id: u32,
        _: std::time::Instant,
    ) -> Option<session::matchmaking_policy::Closed> {
        if self.admitting.locked().contains(&match_id) {
            return None;
        }
        self.closed.locked().get(&match_id).copied()
    }

    fn is_open(&self, match_id: u32) -> bool {
        self.open.locked().contains(&match_id)
    }

    fn reseat(&self, match_id: u32, account_id: u32, session_id: u32) -> bool {
        let known = self.made.locked().iter().any(|(id, setup)| {
            *id == match_id
                && setup
                    .players
                    .iter()
                    .any(|player| player.account_id == account_id)
        });
        if known && self.is_open(match_id) {
            self.reseated
                .locked()
                .push((match_id, account_id, session_id));
        }
        known && self.is_open(match_id)
    }
}

/// An account and a session for it, as the login server makes them.
pub(super) fn signed_in(store: &SessionStore, name: &str) -> (u32, u32) {
    let account_id = store.get_or_create_account(IpAddr::V4(Ipv4Addr::LOCALHOST), name);
    (account_id, store.create_session(account_id))
}

/// The first message `pick` takes.
pub(super) fn find<T>(decoded: &[Decoded], pick: impl Fn(&Decoded) -> Option<T>) -> Option<T> {
    decoded.iter().find_map(pick)
}

/// The text of the first refusal of `op`.
pub(super) fn denial(decoded: &[Decoded], op: u8) -> Option<String> {
    find(decoded, |message| match message {
        Decoded::Denied { op: denied, text } if *denied == op => Some(text.clone()),
        _ => None,
    })
}

/// The match a client was sent to: `(match id, team)`.
pub(super) fn sent_to_match(decoded: &[Decoded]) -> Option<(u32, u8)> {
    find(decoded, |message| match message {
        Decoded::ConnectToMatch {
            host,
            port,
            match_id,
            team,
            ..
        } => {
            assert_eq!((host.as_str(), *port), ("127.0.0.1", MATCH_PORT));
            Some((*match_id, *team))
        }
        _ => None,
    })
}

#[test]
fn the_maps_played_are_playable_ones_of_the_modes_asked_for() {
    use super::{Settings, map_pool, parse_maps, parse_modes};
    use maps::GameMode::{Batteries, TeamDeathmatch, TimedBatteries};

    assert_eq!(parse_modes("2, 4"), Some(vec![Batteries, TeamDeathmatch]));
    // Blackout is 6 and free for all 8 (`GameMode::lobby_number`).
    assert_eq!(
        parse_modes("6,8"),
        Some(vec![maps::GameMode::Blackout, maps::GameMode::FreeForAll])
    );
    // 0.21d's modes 0 and 1 are none of 0.26g's; nothing is not a list.
    assert_eq!(parse_modes("1,2"), None);
    assert_eq!(parse_modes(""), None);
    assert_eq!(parse_maps(" 61 ,62"), Some(vec![61, 62]));
    assert_eq!(parse_maps("61,x"), None);

    let all = map_pool(&[Batteries, TimedBatteries, TeamDeathmatch], &[]);
    assert!(!all.is_empty());
    assert!(
        all.iter()
            .all(|id| maps::map_mode(*id).is_some_and(|map| map.is_playable()))
    );
    let deathmatch = map_pool(&[TeamDeathmatch], &[]);
    assert!(!deathmatch.is_empty() && deathmatch.len() < all.len());
    assert!(
        deathmatch
            .iter()
            .all(|id| maps::map_mode(*id).unwrap().mode == TeamDeathmatch)
    );
    // A list keeps only what can be played in the modes: the broken and
    // test ids of the dictionary never get in.
    assert_eq!(map_pool(&[Batteries, TimedBatteries, TeamDeathmatch], &[all[0], 11, 45, 250]), [all[0]]);
    assert!(map_pool(&[TeamDeathmatch], &[all[0]]).len() <= 1);

    // The default: every mode, Blackout's maps and the free-for-alls too
    // (the test data has no Research maps), by id.
    let settings = Settings::local("127.0.0.1:0");
    let blackout = map_pool(&[maps::GameMode::Blackout], &[]);
    assert!(!blackout.is_empty());
    let free_for_all = map_pool(&[maps::GameMode::FreeForAll], &[]);
    assert!(!free_for_all.is_empty());
    let mut every = [all, blackout, free_for_all].concat();
    every.sort_unstable();
    assert_eq!(settings.maps, every);
}

/// The settings of a lobby whose rooms play matches of two teams: the
/// defaults ([`Settings::local`]) without the free-for-alls in the match
/// pool. A room's cards are drawn at random from it, and what most tests say
/// of a room's match is a match of two teams' (a squad on one side, a late
/// joiner on the side of fewer, 8 v 8 with bots, twenty players); the
/// free-for-all's own tests put its maps into their pool
/// (`core_tests::rooms_tests::free_for_all`).
pub(super) fn two_team_settings() -> super::Settings {
    let mut settings = super::Settings::local("127.0.0.1:0");
    settings.maps.retain(|&id| {
        maps::map_mode(id).is_some_and(|map| map.mode != maps::GameMode::FreeForAll)
    });
    settings
}
