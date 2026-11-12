//! The match server's socket: one UDP port for every match. A datagram goes
//! to the match of the client that sent it; a new client is placed by the
//! session in its 0x40, through the roster the lobby's matchmaking formed.

use std::collections::{HashMap, HashSet};
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::time::{Duration, Instant};

use session::{LockExt, SessionStore};
use survarium::v021d::match_start::{MAPS, MatchOptions, MatchPlayer, VARIANTS, game_mode};

use crate::game::{Game, Report, RosterEntry, Setup};
use crate::transport::{Event, Transport};

/// How long a receive waits before the matches are advanced anyway.
const RECEIVE_WAIT: Duration = Duration::from_millis(5);
/// A formed match no client has reached in this long is given up.
const UNSTARTED: Duration = Duration::from_secs(120);
const HOUSEKEEPING: Duration = Duration::from_secs(1);
/// Where accepted snapshots are kept, for decoding results offline.
const SNAPSHOT_DIR: &str = "logs/snapshots";
/// `SRP_SNAPSHOT_DIR` (`off` or empty for none) or [`SNAPSHOT_DIR`], read once at the server's start ([`run`]) and kept in
/// [`Server::snapshot_dir`]: a game that starts later reads the field, not the environment.
fn snapshot_dir_from(value: Option<&str>) -> Option<std::path::PathBuf> {
    match value {
        Some(value) if value.trim().is_empty() || value.trim().eq_ignore_ascii_case("off") => None,
        Some(value) => Some(value.into()),
        None => Some(SNAPSHOT_DIR.into()),
    }
}
/// `categories_dict`: rifles, sniper rifles, shotguns, SMGs and pistols.
const WEAPON_CATEGORIES: [u8; 6] = [8, 13, 14, 15, 16, 17];
/// Larger than any client datagram (508 bytes).
const RECEIVE_BUFFER: usize = 2048;
/// How many stray addresses are remembered before the list starts over.
const STRAYS_KEPT: usize = 256;
/// An address that sent this many datagrams without connecting (0x40) is
/// told to disconnect. A client of a match that ended with a restart does
/// this: it keeps sending, and what answers it (acks, keepalives) tells it
/// the connection is fine, so it plays on in a match nobody else is in.
const STRAY_LIMIT: u32 = 3;
/// The modes played when `SRP_MATCH_MODES` is unset: the two with map data.
const DEFAULT_MODES: [u8; 2] = [
    game_mode::GATHER_VICTORY_ITEMS,
    game_mode::MATCH_TIME_VI_SPAWN,
];

pub fn run(store: Arc<SessionStore>) -> io::Result<()> {
    let config = vostok::config::get();
    let socket = UdpSocket::bind(config.match_server.bind_addr())?;
    socket.set_read_timeout(Some(RECEIVE_WAIT))?;
    let modes = modes_from_env();
    let snapshot_dir = snapshot_dir_from(std::env::var("SRP_SNAPSHOT_DIR").ok().as_deref());
    log::info!(
        "Match server on {} (modes {modes:?})",
        config.match_server.bind_addr()
    );
    let mut server = Server {
        store,
        port: config.match_server.port,
        modes,
        snapshot_dir,
        games: HashMap::new(),
        pending: HashMap::new(),
        routes: HashMap::new(),
        strays_logged: HashSet::new(),
        stray_datagrams: HashMap::new(),
        next_housekeeping: Instant::now(),
    };
    let mut buffer = [0; RECEIVE_BUFFER];
    loop {
        match socket.recv_from(&mut buffer) {
            Ok((len, addr)) => server.receive(addr, &buffer[..len], Instant::now()),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            // Windows reports a datagram to a closed port on the next receive.
            Err(error) if error.kind() == io::ErrorKind::ConnectionReset => {}
            Err(error) => return Err(error),
        }
        for (addr, datagram) in server.poll(Instant::now()) {
            if let Err(error) = socket.send_to(&datagram, addr) {
                log::debug!("Can't send to {addr}: {error}");
            }
        }
    }
}

struct Server {
    store: Arc<SessionStore>,
    port: u16,
    modes: Vec<u8>,
    /// Where accepted snapshots are kept (`SRP_SNAPSHOT_DIR`, read at the start).
    snapshot_dir: Option<std::path::PathBuf>,
    games: HashMap<u32, Game>,
    /// Clients not yet placed: their 0x40 hasn't come.
    pending: HashMap<SocketAddr, Transport>,
    /// Each placed client's match and roster index.
    routes: HashMap<SocketAddr, (u32, usize)>,
    /// Addresses already logged as sending without being placed.
    strays_logged: HashSet<SocketAddr>,
    /// Datagrams each address sent without being placed or connecting.
    stray_datagrams: HashMap<SocketAddr, u32>,
    next_housekeeping: Instant,
}

impl Server {
    fn receive(&mut self, addr: SocketAddr, datagram: &[u8], now: Instant) {
        if let Some(&(match_id, index)) = self.routes.get(&addr) {
            if let Some(game) = self.games.get_mut(&match_id)
                && game.peer_index(addr) == Some(index)
            {
                game.receive(index, datagram, now);
                for place in game.take_quits() {
                    // The player left by its own word (0x47), not by a link
                    // or a game that broke: it has given its seat up, and
                    // Play queues it for another match instead of sending
                    // it back to this one. (Until 2026-10-12 it was sent
                    // back for as long as the match ran; seen live on the
                    // 0.26g server.) The place stays the account's in the
                    // roster and in the results.
                    let matchmaking = self.store.matchmaking();
                    let account = matchmaking.formed_match(match_id).and_then(|formed| {
                        formed.seats.get(place).map(|seat| seat.player.account_id)
                    });
                    if let Some(account_id) = account {
                        matchmaking.give_up_seat(account_id);
                    }
                    log::info!(
                        "Match {match_id:#x}: player {place} left by its own word (0x47): its \
                         seat is given up, and Play queues it for another match"
                    );
                }
                return;
            }
            self.routes.remove(&addr);
        }
        let transport = self
            .pending
            .entry(addr)
            .or_insert_with(|| Transport::new(now));
        for event in transport.receive(datagram, now) {
            if let Event::Message { id: 0x40, payload } = event
                && let Some(session) = payload.first_chunk::<4>()
            {
                self.stray_datagrams.remove(&addr);
                self.connect(addr, u32::from_le_bytes(*session), now);
                return;
            }
        }
        if self.stray_datagrams.len() >= STRAYS_KEPT {
            self.stray_datagrams.clear();
        }
        let sent = self.stray_datagrams.entry(addr).or_insert(0);
        *sent += 1;
        if *sent == STRAY_LIMIT {
            // Answered until now, as any client is; now it hears that this
            // is not its match.
            transport.disconnect(now);
            log::info!(
                "{addr} sent {STRAY_LIMIT} datagrams without connecting and is not in a match here (its match ended with a restart?); told to disconnect"
            );
        }
        self.log_stray(addr, datagram.len());
    }

    /// A datagram from an address that is not placed and did not connect. If
    /// the same host is placed under another port, its client's port changed
    /// after the first datagram (a NAT), and nothing it sends gets through.
    fn log_stray(&mut self, addr: SocketAddr, len: usize) {
        let placed = self.routes.keys().find(|other| other.ip() == addr.ip());
        let Some(placed) = placed.copied() else {
            return;
        };
        if self.strays_logged.len() >= STRAYS_KEPT {
            self.strays_logged.clear();
        }
        if self.strays_logged.insert(addr) {
            log::info!(
                "{addr} sent {len} bytes but is not a placed client; its host is placed as                  {placed}: the client's port may have changed"
            );
        }
    }

    /// A client's 0x40: put it in its place in its match.
    fn connect(&mut self, addr: SocketAddr, session_id: u32, now: Instant) {
        let Some(mut transport) = self.pending.remove(&addr) else {
            return;
        };
        let Some((match_id, index)) = self.store.matchmaking().seat(session_id) else {
            log::warn!("{addr} connected for session {session_id:#x}, which has no match seat");
            transport.disconnect(now);
            self.pending.insert(addr, transport);
            return;
        };
        if !self.games.contains_key(&match_id) {
            match self.new_game(match_id, now) {
                Some(game) => {
                    self.games.insert(match_id, game);
                }
                None => {
                    log::error!("Match {match_id:#x} can't be set up; {addr} is turned away");
                    transport.disconnect(now);
                    self.pending.insert(addr, transport);
                    return;
                }
            }
        }
        let game = self.games.get_mut(&match_id).expect("set up above");
        game.attach(index, addr, session_id, transport, now);
        self.routes.insert(addr, (match_id, index));
        self.store.seen_in_match(session_id);
    }

    /// A match for a roster the matchmaking formed: a map, variant and mode
    /// at random, the players' chosen profiles under their names.
    fn new_game(&self, match_id: u32, now: Instant) -> Option<Game> {
        let formed = self.store.matchmaking().formed_match(match_id)?;
        let mut players = Vec::new();
        for seat in &formed.seats {
            let account = self.store.account(seat.player.account_id)?;
            let account = account.locked();
            let profile = account
                .game
                .profile(seat.player.profile_id)
                .or_else(|| account.game.profiles.first())?;
            // Its slots' pools from the stacks owned now.
            let profile = survarium::v021d::supplies::match_profile(&account.game, profile);
            players.push((seat.team, seat.squad, account.name.clone(), profile));
        }
        let roll = random();
        let options = MatchOptions::new(
            (roll % MAPS.len() as u64) as usize,
            (roll / 8 % VARIANTS.len() as u64) as usize,
            self.modes[(roll / 64 % self.modes.len() as u64) as usize],
            (roll >> 32) as u32,
        );
        let roster: Vec<MatchPlayer> = players
            .iter()
            .map(|(team, squad, name, profile)| MatchPlayer {
                team: *team,
                squad: *squad,
                name,
                profile,
            })
            .collect();
        let info = survarium::v021d::match_start::static_info(&options, &roster);
        log::info!(
            "Match {match_id:#x}: {} in mode {}, {} players, {} batteries, {} s",
            options.project,
            options.mode,
            roster.len(),
            options.batteries,
            options.duration_seconds
        );
        let dictionary = survarium::dictionary::Dictionary::shared();
        let setup = Setup {
            roster: players
                .iter()
                .map(|(team, _, name, profile)| {
                    let mut weapons = profile
                        .slots
                        .iter()
                        .flatten()
                        .map(|item| item.dict_id)
                        .filter(|dict_id| {
                            dictionary
                                .item(*dict_id)
                                .is_some_and(|item| WEAPON_CATEGORIES.contains(&item.category))
                        });
                    RosterEntry {
                        name: name.clone(),
                        team: *team,
                        level: profile.level,
                        weapons: [weapons.next().unwrap_or(0), weapons.next().unwrap_or(0)],
                    }
                })
                .collect(),
            countdown_seconds: options.countdown_seconds,
            duration_seconds: options.duration_seconds,
            batteries: options.batteries,
            score_weights: options.score_weights,
            loadouts: players
                .iter()
                .map(|(_, _, _, profile)| survarium::v021d::supplies::loadout(dictionary, profile))
                .collect(),
        };
        let mut game = Game::new(match_id, self.port, info, setup, now);
        if let Some(dir) = self.snapshot_dir.clone() {
            game.dump_snapshots_to(dir);
        }
        Some(game)
    }

    fn poll(&mut self, now: Instant) -> Vec<(SocketAddr, Vec<u8>)> {
        let mut datagrams = Vec::new();
        let mut reports = Vec::new();
        for game in self.games.values_mut() {
            datagrams.extend(game.poll(now));
            reports.extend(game.take_report());
        }
        // Before the clients hear the end: back in the lobby, they get it.
        for report in reports {
            self.reward(report);
        }
        self.pending.retain(|addr, transport| {
            datagrams.extend(
                transport
                    .poll(now)
                    .into_iter()
                    .map(|datagram| (*addr, datagram)),
            );
            !transport.is_gone(now)
        });
        let pending = &self.pending;
        self.stray_datagrams
            .retain(|addr, _| pending.contains_key(addr));
        if now >= self.next_housekeeping {
            self.next_housekeeping = now + HOUSEKEEPING;
            self.housekeeping(now);
        }
        datagrams
    }

    /// Finished matches free their seats; the clients in a match count as
    /// seen; a formed match nobody reached is given up.
    fn housekeeping(&mut self, now: Instant) {
        let over: Vec<u32> = self
            .games
            .iter()
            .filter(|(_, game)| game.is_over(now))
            .map(|(id, _)| *id)
            .collect();
        for match_id in over {
            self.games.remove(&match_id);
            self.routes.retain(|_, (id, _)| *id != match_id);
            self.store.matchmaking().end_match(match_id);
            log::info!("Match {match_id:#x} is over");
        }
        for game in self.games.values() {
            for session_id in game.sessions() {
                self.store.seen_in_match(session_id);
            }
        }
        for (match_id, formed_at) in self.store.matchmaking().matches() {
            if !self.games.contains_key(&match_id)
                && now.saturating_duration_since(formed_at) >= UNSTARTED
            {
                self.store.matchmaking().end_match(match_id);
                log::info!("Match {match_id:#x}: no client came; given up");
            }
        }
    }
}

/// How the match moves each player's rating (by roster index). A player
/// that was never in the match (its game crashed or hung while loading)
/// neither wins nor loses, and is left out of its team's average too.
fn rating_changes(ratings: &[(u8, u16, bool)], entered: &[bool], winner: u8) -> Vec<i32> {
    let played: Vec<usize> = (0..ratings.len())
        .filter(|&index| entered.get(index).copied().unwrap_or(true))
        .collect();
    let moved = survarium::v021d::rating::changes(
        &played.iter().map(|&index| ratings[index]).collect::<Vec<_>>(),
        winner,
    );
    let mut changes = vec![0; ratings.len()];
    for (&index, change) in played.iter().zip(moved) {
        changes[index] = change;
    }
    changes
}

impl Server {
    /// Give each player what the match earned, and keep the results for the
    /// lobby.
    fn reward(&self, report: Report) {
        let Some(formed) = self.store.matchmaking().formed_match(report.match_id) else {
            return;
        };
        // The ratings as the match ended, and how it moves them.
        let ratings: Vec<(u8, u16, bool)> = formed
            .seats
            .iter()
            .zip(&report.players)
            .map(|(seat, stats)| {
                let rating = self
                    .store
                    .account(seat.player.account_id)
                    .map_or(survarium::v021d::rating::START, |account| {
                        account.locked().game.rating
                    });
                (stats.team, rating, stats.finished)
            })
            .collect();
        let changes = rating_changes(&ratings, &report.entered, report.winner);
        let mut accounts = Vec::new();
        let mut rewards = Vec::new();
        for (((seat, stats), (owned, left)), &change) in formed
            .seats
            .iter()
            .zip(&report.players)
            .zip(&report.supplies)
            .zip(&changes)
        {
            let account_id = seat.player.account_id;
            let reward = match self.store.account(account_id) {
                Some(account) => {
                    use survarium::v021d::{drops::Rng, quests, results, supplies};
                    let mut account = account.locked();
                    let account = &mut *account;
                    let mut rng = Rng::from_time();
                    let profile_id = seat.player.profile_id;
                    // The gear faction the match was played in.
                    let gear = account
                        .game
                        .profile(profile_id)
                        .map_or(0, |profile| results::gear_faction(profile).0);
                    let mut reward =
                        results::apply_with(&mut account.game, profile_id, stats, &mut rng);
                    let game = &mut account.game;
                    game.rating_before = game.rating;
                    game.rating = survarium::v021d::rating::moved(game.rating, change);
                    reward.rating_before = game.rating_before;
                    reward.rating_after = game.rating;
                    // What the match used, off the stacks; autobuy buys it
                    // back, with the match's money already paid.
                    match left {
                        Some(left) => {
                            let settled =
                                supplies::settle(&mut account.game, profile_id, owned, left);
                            log::info!(
                                "Match {:#x}: {} used {:?}, autobuy bought {:?} for {}",
                                report.match_id,
                                stats.name,
                                settled.used,
                                settled.bought,
                                settled.cost
                            );
                        }
                        None if !owned.is_empty() => log::warn!(
                            "Match {:#x}: {}'s supplies could not be read; nothing is taken",
                            report.match_id,
                            stats.name
                        ),
                        None => {}
                    }
                    // Reputation levels the match itself reached.
                    for &level in &reward.reputation_levels {
                        account.notices.push((
                            quests::REPUTATION_LEVEL_MESSAGE,
                            format!("#ru[{}:{level}]", reward.reputation_faction),
                        ));
                    }
                    let notices =
                        quests::evaluate(&mut account.game, stats, gear, quests::now(), &mut rng);
                    account.notices.extend(notices);
                    reward
                }
                None => Default::default(),
            };
            if !report.entered.get(accounts.len()).copied().unwrap_or(true) {
                log::info!(
                    "Match {:#x}: {} was never in the match; its rating stays",
                    report.match_id,
                    stats.name
                );
            }
            log::info!(
                "Match {:#x}: {} gets {} experience, {} money, {} reputation with faction {} and drops {:?}, rating {} -> {} (score {}, {} kills, {} artefacts)",
                report.match_id,
                stats.name,
                reward.experience,
                reward.money,
                reward.reputation,
                reward.reputation_faction,
                reward.drops,
                reward.rating_before,
                reward.rating_after,
                stats.score,
                stats.kills,
                stats.artefacts
            );
            self.store.mark_dirty(account_id);
            accounts.push(account_id);
            rewards.push(reward);
        }
        // Rewards are what a player would miss most after a crash.
        self.store.flush();
        self.store.reports().add(session::MatchReport {
            match_id: report.match_id,
            winner: report.winner,
            team_points: report.team_points,
            players: report.players,
            accounts,
            rewards,
        });
    }
}

/// A random number for the map, mode and seed.
fn random() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    );
    hasher.finish()
}

/// `SRP_MATCH_MODES`: the game modes to play, comma-separated (default
/// `2,3`). Modes 0 and 1 need base points no map has; they play as mode 2.
fn modes_from_env() -> Vec<u8> {
    match std::env::var("SRP_MATCH_MODES") {
        Ok(value) => parse_modes(&value).unwrap_or_else(|| {
            log::error!("SRP_MATCH_MODES is `{value}`; it must list modes 0 to 3. Using 2,3");
            DEFAULT_MODES.to_vec()
        }),
        Err(_) => DEFAULT_MODES.to_vec(),
    }
}

fn parse_modes(value: &str) -> Option<Vec<u8>> {
    let modes: Option<Vec<u8>> = value
        .split(',')
        .map(|mode| mode.trim().parse::<u8>().ok().filter(|mode| *mode <= 3))
        .collect();
    modes.filter(|modes| !modes.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A client of a match that ended with a restart keeps sending; it is
    /// answered at first, then told to disconnect, and forgotten once it has.
    #[test]
    fn a_client_of_a_match_that_is_gone_is_told_to_disconnect() {
        use session::persist::MemoryRepository;

        let store = Arc::new(
            SessionStore::with_repository(
                Duration::from_secs(60),
                Box::new(MemoryRepository::default()),
            )
            .unwrap(),
        );
        let start = Instant::now();
        let mut server = Server {
            store,
            port: 0,
            modes: vec![2],
            snapshot_dir: None,
            games: HashMap::new(),
            pending: HashMap::new(),
            routes: HashMap::new(),
            strays_logged: HashSet::new(),
            stray_datagrams: HashMap::new(),
            next_housekeeping: start + Duration::from_secs(3600),
        };
        let addr: SocketAddr = "127.0.0.1:41000".parse().unwrap();
        // A datagram with nothing in it but the header.
        let datagram = [0u8; 12];
        server.receive(addr, &datagram, start);
        server.receive(addr, &datagram, start);
        assert_eq!(server.stray_datagrams[&addr], 2);
        let before = server.poll(start + Duration::from_millis(10));
        server.receive(addr, &datagram, start + Duration::from_millis(20));
        assert_eq!(server.stray_datagrams[&addr], STRAY_LIMIT);
        // It hears the disconnection, and is gone a little later.
        let after = server.poll(start + Duration::from_millis(120));
        assert!(after.iter().any(|(to, _)| *to == addr));
        assert!(after.len() > before.len() || !after.is_empty());
        server.poll(start + Duration::from_secs(20));
        assert!(server.pending.is_empty());
        assert!(server.stray_datagrams.is_empty());
    }

    /// A player who left its match by its own word (the 4-byte 0x47) has
    /// given its seat up: Play queues it, and a game of its that comes for
    /// the seat is turned away. The other player keeps its seat.
    #[test]
    fn a_player_who_left_by_its_own_word_has_given_its_seat_up() {
        use session::{Player, QueueState, persist::MemoryRepository};
        use survarium::dictionary::Dictionary;
        use survarium::v021d::account::Account;

        let store = Arc::new(
            SessionStore::with_repository(
                Duration::from_secs(60),
                Box::new(MemoryRepository::default()),
            )
            .unwrap(),
        );
        let start = Instant::now();
        let mut players = Vec::new();
        for n in 0..2u32 {
            let name = format!("quit{n}");
            let account_id = store.sign_in(&name, b"test-password-1").unwrap();
            let session_id = store.create_session(account_id);
            store.account(account_id).unwrap().locked().game =
                Account::with_test_loadout(&name, Dictionary::shared());
            let player = Player {
                session_id,
                account_id,
                profile_id: 1,
            };
            store.matchmaking().queue(&[player], false, start);
            players.push(player);
        }
        let formed = start + Duration::from_secs(3600);
        store.matchmaking().poll(players[0].session_id, formed);
        let (match_id, _) = store.matchmaking().seat(players[1].session_id).unwrap();

        let mut server = Server {
            store: store.clone(),
            port: 0,
            modes: vec![2],
            snapshot_dir: None,
            games: HashMap::new(),
            pending: HashMap::new(),
            routes: HashMap::new(),
            strays_logged: HashSet::new(),
            stray_datagrams: HashMap::new(),
            next_housekeeping: formed + Duration::from_secs(3600),
        };
        // A datagram as a client sends it: its seq, no acks, one message.
        let datagram = |seq: u16, id: u8, payload: &[u8]| {
            let mut datagram = seq.to_le_bytes().to_vec();
            datagram.extend(0xFFFF_u16.to_le_bytes());
            datagram.extend(0_u64.to_le_bytes());
            datagram.push(id);
            datagram.extend(payload);
            datagram
        };
        let addrs: Vec<SocketAddr> = (0..2)
            .map(|n| format!("127.0.0.1:{}", 42000 + n).parse().unwrap())
            .collect();
        for (player, addr) in players.iter().zip(&addrs) {
            let mut connect = player.session_id.to_le_bytes().to_vec();
            connect.push(0);
            server.receive(*addr, &datagram(0, 0x40, &connect), formed);
        }
        assert_eq!(server.routes.len(), 2);

        // Player 1 leaves by the menu.
        server.receive(addrs[1], &datagram(1, 0x47, &0_u32.to_le_bytes()), formed);
        assert_eq!(store.matchmaking().seat(players[1].session_id), None);
        assert_eq!(
            store.matchmaking().seat(players[0].session_id),
            Some((match_id, 0))
        );
        assert_eq!(store.matchmaking().match_count(), 1);

        // Play: it waits in the queue, and is not sent back.
        store.matchmaking().queue(&[players[1]], false, formed);
        assert!(matches!(
            store.matchmaking().poll(players[1].session_id, formed),
            Some(QueueState::Waiting {
                place: 1,
                total: 1,
                ..
            })
        ));

        // A game of its that comes for the seat all the same has none.
        let again: SocketAddr = "127.0.0.1:42009".parse().unwrap();
        let mut connect = players[1].session_id.to_le_bytes().to_vec();
        connect.push(0);
        server.receive(again, &datagram(0, 0x40, &connect), formed);
        assert!(!server.routes.contains_key(&again));
    }

    #[test]
    fn the_snapshot_dir_is_the_variable_or_the_default_and_off_means_none() {
        assert_eq!(snapshot_dir_from(None), Some(SNAPSHOT_DIR.into()));
        assert_eq!(snapshot_dir_from(Some("/tmp/worlds")), Some("/tmp/worlds".into()));
        assert_eq!(snapshot_dir_from(Some("off")), None);
        assert_eq!(snapshot_dir_from(Some("  ")), None);
    }

    #[test]
    fn modes_are_listed_by_number() {
        assert_eq!(parse_modes("2, 3"), Some(vec![2, 3]));
        assert_eq!(parse_modes("3"), Some(vec![3]));
        assert_eq!(parse_modes("4"), None);
        assert_eq!(parse_modes("two"), None);
    }

    /// 35 players press Play: two matches (20, then 15) run side by side,
    /// each client finds its seat in its own match, and nobody is in both.
    #[test]
    fn thirty_five_players_fill_two_matches_that_run_together() {
        use session::{Player, persist::MemoryRepository};
        use survarium::dictionary::Dictionary;
        use survarium::v021d::account::Account;

        let store = Arc::new(
            SessionStore::with_repository(
                Duration::from_secs(60),
                Box::new(MemoryRepository::default()),
            )
            .unwrap(),
        );
        let start = Instant::now();
        let mut sessions = Vec::new();
        for n in 0..35u32 {
            let name = format!("bot{n}");
            let account_id = store.sign_in(&name, b"test-password-1").unwrap();
            let session_id = store.create_session(account_id);
            store.account(account_id).unwrap().locked().game =
                Account::with_test_loadout(&name, Dictionary::shared());
            store.matchmaking().queue(
                &[Player {
                    session_id,
                    account_id,
                    profile_id: 1,
                }],
                false,
                start,
            );
            sessions.push(session_id);
        }
        // 20 at once; the other 15 have been quiet since they all joined.
        store.matchmaking().poll(sessions[0], start);
        assert_eq!(store.matchmaking().match_count(), 1);
        store
            .matchmaking()
            .poll(sessions[34], start + Duration::from_secs(30));
        assert_eq!(store.matchmaking().match_count(), 2);

        let mut server = Server {
            store: store.clone(),
            port: 0,
            modes: vec![2],
            snapshot_dir: None,
            games: HashMap::new(),
            pending: HashMap::new(),
            routes: HashMap::new(),
            strays_logged: HashSet::new(),
            stray_datagrams: HashMap::new(),
            next_housekeeping: start,
        };
        let now = start + Duration::from_secs(31);
        for (n, session_id) in sessions.iter().enumerate() {
            let addr: SocketAddr = format!("127.0.0.1:{}", 40000 + n).parse().unwrap();
            server.pending.insert(addr, Transport::new(now));
            server.connect(addr, *session_id, now);
        }
        assert_eq!(server.games.len(), 2);
        assert_eq!(server.routes.len(), 35);
        let mut per_match: HashMap<u32, usize> = HashMap::new();
        for (match_id, _) in server.routes.values() {
            *per_match.entry(*match_id).or_default() += 1;
        }
        let mut sizes: Vec<usize> = per_match.values().copied().collect();
        sizes.sort_unstable();
        assert_eq!(sizes, vec![15, 20]);
        // A tick of the whole server with both games running.
        let _ = server.poll(now + Duration::from_millis(50));
        assert_eq!(server.games.len(), 2);
    }

    /// A match's rewards (experience, money, reputation, drops, quests,
    /// rating, ammo used and bought back by autobuy) are written at once,
    /// and a store reloaded from the repository has them.
    #[test]
    fn match_rewards_are_saved_at_once() {
        use session::persist::{AccountRecord, MemoryRepository};
        use std::collections::BTreeMap;
        use survarium::dictionary::Dictionary;
        use survarium::v021d::{account::Account, drops::Rng, quests, results::PlayerStats};

        let repository = MemoryRepository::default();
        let store = Arc::new(
            SessionStore::with_repository(Duration::ZERO, Box::new(repository.clone())).unwrap(),
        );
        let account_id = store.sign_in("fighter", b"test-password-1").unwrap();
        let account = store.account(account_id).unwrap();
        {
            let mut account = account.locked();
            account.game = Account::with_test_loadout("fighter", Dictionary::shared());
            account.game.profiles[0].autobuy = true;
            quests::refresh(&mut account.game, quests::now(), &mut Rng::new(3));
        }
        let session_id = store.create_session(account_id);
        let player = session::Player {
            session_id,
            account_id,
            profile_id: 1,
        };
        // A match needs a second player.
        let other = session::Player {
            session_id: session_id + 1,
            account_id: account_id + 1,
            profile_id: 1,
        };
        store
            .matchmaking()
            .queue(&[player, other], false, Instant::now());
        store
            .matchmaking()
            .poll(session_id, Instant::now() + Duration::from_secs(3600));
        let (match_id, _) = store.matchmaking().seat(session_id).unwrap();
        store.flush();
        let before = AccountRecord::of(&account.locked());
        let saves = repository.saves();

        let server = Server {
            store: store.clone(),
            port: 0,
            modes: vec![3],
            snapshot_dir: None,
            games: HashMap::new(),
            pending: HashMap::new(),
            routes: HashMap::new(),
            strays_logged: HashSet::new(),
            stray_datagrams: HashMap::new(),
            next_housekeeping: Instant::now(),
        };
        server.reward(Report {
            match_id,
            winner: 0,
            team_points: [3, 0],
            players: vec![PlayerStats {
                name: "fighter".into(),
                team: 0,
                score: 120,
                kills: 3,
                finished: true,
                won: true,
                team_batteries: 3,
                ..PlayerStats::default()
            }],
            // 35 rounds of buckshot used; autobuy buys them back.
            supplies: vec![(BTreeMap::from([(22, 50)]), Some(BTreeMap::from([(22, 15)])))],
            entered: vec![true],
        });

        // Written already, without waiting for the saver.
        assert!(!store.is_dirty(account_id));
        assert_eq!(repository.saves(), saves + 1);
        let after = AccountRecord::of(&account.locked());
        assert_ne!(after.game, before.game);
        assert!(after.game.profiles[0].exp_total > before.game.profiles[0].exp_total);
        assert!(after.game.profiles[0].autobuy);
        // 15 left, and autobuy bought clips back.
        let buckshot = after.game.inventory.iter().find(|item| item.dict_id == 22);
        assert!(buckshot.unwrap().condition_or_stack > 15);
        assert_eq!(repository.get(account_id), Some(after.clone()));
        let reloaded =
            SessionStore::with_repository(Duration::ZERO, Box::new(repository.clone())).unwrap();
        let reloaded = AccountRecord::of(&reloaded.account(account_id).unwrap().locked());
        assert_eq!(reloaded, after);
    }

    #[test]
    fn a_player_never_in_the_match_keeps_its_rating() {
        // Team 0 won; player 1 (team 1) played and lost, player 2 (team 1)
        // never got in.
        let ratings = [(0, 1000, true), (1, 1000, true), (1, 1000, false)];
        let changes = rating_changes(&ratings, &[true, true, false], 0);
        assert_eq!(changes[2], 0);
        assert!(changes[0] > 0 && changes[1] < 0);
        // As if the match had been of the two who played.
        assert_eq!(
            changes[..2],
            survarium::v021d::rating::changes(&ratings[..2], 0)[..]
        );
        // Everyone in: as before.
        assert_eq!(
            rating_changes(&ratings, &[true; 3], 0),
            survarium::v021d::rating::changes(&ratings, 0)
        );
    }
}
