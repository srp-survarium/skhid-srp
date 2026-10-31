//! The 0.21d lobby (`../srp-021d/docs/notes/lobby_spec.md`). The client asks
//! for almost nothing: after its sign-in the server pushes the whole account,
//! and then answers its actions.

mod actions;
pub mod frame;

use std::cell::{Cell, RefCell};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use session::{Account, LockExt, Player, QueueState, SessionStore, Squad};
use survarium::dictionary::Dictionary;
use survarium::v021d::drops::Rng;
use survarium::v021d::lobby::{self, ClientState, REROLL_COST, SquadMember, op};
use survarium::v021d::{premium, quests};
use vostok::network_client::TcpClient;

use crate::UnknownSessions;
use crate::messaging_server::ChatRoom;
use actions::Relocation;
use frame::{Frame, Payload, Reader};

/// The first message of a lobby connection: `0x26`, 11 bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SignIn {
    pub session_id: u32,
    pub just_left_match: bool,
    /// `0xFFFFFFFF` for none.
    pub pending_match_stats_id: u32,
    pub need_shop_items: bool,
}

pub const SIGN_IN: u8 = 0x26;

impl SignIn {
    pub fn parse(payload: &[u8]) -> Option<Self> {
        let mut reader = Reader::new(payload);
        if reader.u8()? != SIGN_IN {
            return None;
        }
        let sign_in = Self {
            session_id: reader.u32()?,
            just_left_match: reader.bool()?,
            pending_match_stats_id: reader.u32()?,
            need_shop_items: reader.bool()?,
        };
        reader.is_empty().then_some(sign_in)
    }
}

/// One lobby connection's account: the session's, or for a lobby without a
/// login server, a default one of its own.
struct Connection<'a> {
    session_id: u32,
    account_id: u32,
    account: Arc<Mutex<Account>>,
    store: &'a SessionStore,
    dictionary: &'static Dictionary,
    /// When the player pressed Play, while it waits in the queue.
    queued_since: Cell<Option<Instant>>,
    /// The squad status (0x3A) the client has.
    squad_sent: RefCell<Vec<u8>>,
    /// Where this client finds the match server: a numeric IPv4 address,
    /// as the client's `address::from_string` takes no name.
    match_host: String,
    /// The match the client was sent to (0x33), while it lasts.
    sent_to_match: Cell<Option<u32>>,
}

/// The squad mates' changes reach a lobby connection on its next wake-up.
/// A player whose lobby connection ends is no longer ready.
struct Unready<'a> {
    store: &'a SessionStore,
    account_id: u32,
}

impl Drop for Unready<'_> {
    fn drop(&mut self) {
        self.store.squads().set_ready(self.account_id, false);
    }
}

/// How often the lobby updates a client unasked: a waiting player's queue
/// screen (the client shows the wait the server last sent,
/// `set_mm_current_time`; it doesn't count), and the online counts (0x38)
/// when they change.
const TICK: Duration = Duration::from_secs(1);

/// Serve one lobby connection until the client disconnects (the client drops
/// it while in a match and reconnects afterwards).
pub fn run(
    store: Arc<SessionStore>,
    chat: &ChatRoom,
    mut tcp_client: TcpClient,
    unknown: UnknownSessions,
) {
    let sign_in = match tcp_client.read::<Frame>() {
        Ok(Frame(payload)) => match SignIn::parse(&payload) {
            Some(sign_in) => sign_in,
            None => {
                log::error!("First lobby message must be a 0.21d sign-in, got {payload:02X?}");
                return;
            }
        },
        Err(error) if error.is_disconnect() => {
            log::info!("Lobby connection closed before sign-in");
            return;
        }
        Err(error) => {
            log::error!("Unreadable lobby sign-in: {error}");
            return;
        }
    };
    let session_id = sign_in.session_id;
    log::debug!("Lobby sign-in {sign_in:?}");

    // The connection holds its session while it lasts. The 0.21d client has
    // no refusal message (0x31 and 0x32 have no handler), so a session the
    // login server didn't open, or one that has ended, is refused by closing
    // the connection: the client retries every 5 s and, after three
    // failures, goes back to the login screen (spec §1.4).
    let hold = store.hold(session_id);
    let account = hold
        .as_ref()
        .and_then(|_| store.account_for_session(session_id));
    let account = match (account, unknown) {
        (Some(account), _) => account,
        (None, UnknownSessions::Refuse) => {
            log::warn!(
                "Session {session_id:#x} is unknown or has ended; closed its lobby connection"
            );
            return;
        }
        (None, UnknownSessions::UseDefaultAccount) => {
            log::warn!("Unknown session {session_id:#x}; using a default account");
            Arc::new(Mutex::new(Account::new(1, "dummy_name".to_string())))
        }
    };
    if let Some(hold) = &hold {
        hold.attach(tcp_client.stream());
    }
    let _hold = hold;
    let account_id = account.locked().id;
    let match_host = match (tcp_client.peer_addr(), tcp_client.local_addr()) {
        (Ok(peer), Ok(reached)) => {
            let config = vostok::config::get();
            let host = config
                .match_server
                .host_for(&config.lobby_server, peer.ip(), reached.ip());
            ipv4_host(&host, config.match_server.port)
        }
        _ => vostok::config::get().match_server.public_host.clone(),
    };
    let connection = Connection {
        session_id,
        account_id,
        account,
        store: &store,
        dictionary: Dictionary::shared(),
        queued_since: Cell::new(None),
        // Nothing yet: the client may still show a squad the server no longer
        // has (a restart forgets the squads, a mate's game was found gone),
        // so the first update tells it how things are.
        squad_sent: RefCell::new(Vec::new()),
        match_host,
        sent_to_match: Cell::new(None),
    };

    let push = {
        // Before this account is locked: the ranking locks every account.
        let ranked = connection.ranked();
        let mut account = connection.account.locked();
        let now = quests::now();
        let notices = quests::refresh(&mut account.game, now, &mut Rng::from_time());
        account.notices.extend(notices);
        let (players, games, minutes) = online_counts(&store);
        lobby::sign_in_push(&account.game, now, players, games, minutes, ranked)
    };
    // Signing in may have refreshed its quests.
    store.mark_dirty(account_id);
    for message in push {
        if let Err(error) = tcp_client.send(Payload(message)) {
            log::info!("Session {session_id:#x} left the lobby while signing in: {error}");
            return;
        }
    }
    log::info!("Session {session_id:#x} connected to the lobby");
    let _unready = Unready {
        store: &store,
        account_id,
    };

    let mut online = online_counts(&store);
    // The balance the client has: the web page can change it.
    let mut money = connection.balance();
    let mut next_tick = Instant::now() + TICK;
    loop {
        // Wake up every tick even when the client is quiet.
        let timeout = next_tick
            .saturating_duration_since(Instant::now())
            .max(Duration::from_millis(1));
        if let Err(error) = tcp_client.set_read_timeout(Some(timeout)) {
            log::error!("Session {session_id:#x}: {error}; dropping the lobby connection");
            return;
        }
        let replies = match tcp_client.read::<Frame>() {
            Ok(Frame(payload)) => connection.answer(&payload),
            Err(error) if error.is_timeout() => Vec::new(),
            Err(error) if error.is_disconnect() => {
                log::info!("Session {session_id:#x} disconnected from the lobby ({error})");
                return;
            }
            Err(error) => {
                log::error!("Session {session_id:#x}: {error}; dropping the lobby connection");
                return;
            }
        };
        let mut updates = connection.squad_update();
        updates.extend(connection.results_update());
        connection.deliver_notices(chat);
        if Instant::now() >= next_tick {
            next_tick = Instant::now() + TICK;
            updates.extend(connection.queue_status());
            if connection.balance() != money {
                money = connection.balance();
                updates.push(lobby::money(&connection.account.locked().game));
            }
            let counts = online_counts(&store);
            if counts != online {
                online = counts;
                updates.push(lobby::online_stats(counts.0, counts.1, counts.2));
            }
        }
        for message in replies.into_iter().chain(updates) {
            if let Err(error) = tcp_client.send(Payload(message)) {
                log::info!("Session {session_id:#x} disconnected from the lobby: {error}");
                return;
            }
        }
    }
}

/// The players and matches online, as the lobby's status bar shows them,
/// and the minutes to maintenance (a day when none is set).
fn online_counts(store: &SessionStore) -> (u32, u32, u32) {
    (
        store.players_online() as u32,
        store.match_count() as u32,
        store
            .minutes_to_maintenance()
            .unwrap_or(lobby::MINUTES_TO_MAINTENANCE),
    )
}

impl Connection<'_> {
    /// Silver and gold now.
    fn balance(&self) -> (u32, u32) {
        let account = self.account.locked();
        (account.game.generic_money, account.game.premium_money)
    }

    /// What the client is told of its place in the queue:
    /// - while it waits, its place and how long it has waited (the client
    ///   shows the wait the server last sent; there is no average to show);
    /// - a squad member whose squad is not all ready waits on its own, and
    ///   its client shows "squad not ready";
    /// - once its match has formed, "match found" and where the match
    ///   server is (0x33), once: by this connection if it pressed Play, else
    ///   by any of the session's that finds the seat not yet sent (the one
    ///   that pressed Play dropped while it waited; seen live, match 0x130);
    /// - once that match is over, the lobby.
    fn queue_status(&self) -> Vec<Vec<u8>> {
        let state = self
            .store
            .matchmaking()
            .poll(self.session_id, Instant::now());
        match state {
            Some(QueueState::Placed { match_id, team, .. })
                if self.queued_since.get().is_some()
                    || !self.store.matchmaking().is_told(self.session_id) =>
            {
                self.queued_since.set(None);
                self.sent_to_match.set(Some(match_id));
                self.store.matchmaking().mark_told(self.session_id);
                log::info!(
                    "Session {:#x} goes to match {match_id:#x} on team {team} at {}:{}",
                    self.session_id,
                    self.match_host,
                    vostok::config::get().match_server.port
                );
                vec![
                    lobby::client_state(ClientState::InQueue {
                        order_id: self.session_id,
                        place: 0,
                        queue_total: 0,
                        current_wait_seconds: 0,
                        average_wait_seconds: 0,
                    }),
                    lobby::connect_to_match_server(
                        &self.match_host,
                        vostok::config::get().match_server.port,
                        match_id,
                        team,
                    ),
                ]
            }
            Some(QueueState::Placed { .. }) => Vec::new(),
            Some(QueueState::Waiting {
                place,
                total,
                since,
            }) => vec![lobby::client_state(ClientState::InQueue {
                order_id: self.session_id,
                place: place as u32,
                queue_total: total as u32,
                current_wait_seconds: since.elapsed().as_secs() as u32,
                average_wait_seconds: 0,
            })],
            None if self.sent_to_match.take().is_some() => {
                vec![lobby::client_state(ClientState::Lobby)]
            }
            None => match self.queued_since.get() {
                Some(since) => vec![lobby::client_state(ClientState::InQueue {
                    order_id: self.session_id,
                    place: 1,
                    queue_total: 1,
                    current_wait_seconds: since.elapsed().as_secs() as u32,
                    average_wait_seconds: 0,
                })],
                None => Vec::new(),
            },
        }
    }

    /// A match the player finished and hasn't seen the results of: the
    /// money, profiles and skills it changed, then the results screen, which
    /// opens by itself.
    /// Send the account's waiting important messages (quest results,
    /// reputation levels) once its chat client is there.
    fn deliver_notices(&self, chat: &ChatRoom) {
        if !chat.is_online(self.account_id) {
            return;
        }
        let notices = std::mem::take(&mut self.account.locked().notices);
        if !notices.is_empty() {
            self.store.mark_dirty(self.account_id);
        }
        for (kind, text) in notices {
            log::info!("Session {:#x}: notice {text}", self.session_id);
            chat.notify(self.account_id, kind, &text);
        }
    }

    /// `21 0C <start> <count>`: the ranking rows the page asks for.
    fn ranking_answer(&self, payload: &[u8]) -> Option<Vec<Vec<u8>>> {
        let mut reader = Reader::new(&payload[2..]);
        let start = reader.u32()?;
        let count = reader.u8()?;
        let ranking = self.store.ranking();
        let from = (start as usize).min(ranking.len());
        let to = (from + usize::from(count)).min(ranking.len());
        let mut answer = Vec::new();
        if start == 0 {
            // The page opens with this: the client's own row and the count
            // were sent at sign-in and after a match, and players who signed
            // up or moved since have changed its place.
            let place = ranking
                .iter()
                .position(|row| row.account_id == self.account_id)
                .map_or(0, |index| index as u32 + 1);
            let account = self.account.locked();
            answer.push(lobby::base_info(&account.game, ranking.len() as u32));
            answer.push(lobby::player_elo(place, &account.game));
        }
        answer.push(lobby::elo_list(start, &ranking[from..to]));
        Some(answer)
    }

    /// How many are ranked, and this account's place (from 1). Locks every
    /// account: call it with none locked.
    fn ranked(&self) -> (u32, u32) {
        let ranking = self.store.ranking();
        let place = ranking
            .iter()
            .position(|row| row.account_id == self.account_id)
            .map_or(0, |index| index as u32 + 1);
        (ranking.len() as u32, place)
    }

    fn results_update(&self) -> Vec<Vec<u8>> {
        let Some(report) = self.store.reports().take_unseen(self.account_id) else {
            return Vec::new();
        };
        let Some(screen) = report.screen_for(self.account_id) else {
            return Vec::new();
        };
        log::info!(
            "Session {:#x}: the results of match {:#x}",
            self.session_id,
            report.match_id
        );
        let (_, place) = self.ranked();
        let account = self.account.locked();
        let game = &account.game;
        let mut updates = vec![
            lobby::money(game),
            // Drops are new items: the inventory before the profiles.
            lobby::inventory(game),
            lobby::profiles(game),
            lobby::unlocked_factions(0x0F),
            lobby::reputations(game),
            // The match moved the rating.
            lobby::player_elo(place, game),
            // Quests done or failed moved the loyalties.
            lobby::services(game, quests::now()),
        ];
        updates.extend(shown_profile(&account).map(lobby::skills));
        updates.push(screen);
        updates
    }

    /// Play: the player waits for a match with this profile. In a squad,
    /// Play makes it ready, and the squad queues once all are. Mates'
    /// accounts are read one at a time, never while this one is locked.
    fn play_answer(&self, payload: &[u8]) -> Option<Vec<Vec<u8>>> {
        let mut reader = Reader::new(payload);
        reader.u8()?;
        let profile_id = reader.u32()?;
        let player = Player {
            session_id: self.session_id,
            account_id: self.account_id,
            profile_id,
        };
        {
            let mut account = self.account.locked();
            if account.game.profile(profile_id).is_none() {
                return Some(vec![lobby::operation_denied(
                    op::PLAY,
                    "st_profile_check_failed",
                )]);
            }
            account.play = Some(player);
        }
        if self.queued_since.get().is_none() {
            self.queued_since.set(Some(Instant::now()));
        }
        let now = Instant::now();
        match self.store.squads().set_ready(self.account_id, true) {
            None => self.store.matchmaking().queue(&[player], false, now),
            Some(squad) if squad.all_ready() => {
                let players: Option<Vec<Player>> = squad
                    .members
                    .iter()
                    .map(|member| self.store.account(*member)?.locked().play)
                    .collect();
                match players {
                    Some(players) => {
                        log::info!(
                            "Squad {} is ready: all {} in the queue",
                            squad.id,
                            players.len()
                        );
                        self.store.matchmaking().queue(&players, true, now);
                    }
                    None => log::warn!("Squad {}: a ready member has no Play", squad.id),
                }
            }
            Some(_) => {}
        }
        log::info!(
            "Session {:#x} asked to play with profile {profile_id}",
            self.session_id
        );
        Some(self.queue_status())
    }

    /// The replies to one client message, in order.
    fn answer(&self, payload: &[u8]) -> Vec<Vec<u8>> {
        let session_id = self.session_id;
        // Squad messages read other accounts, so they run without this one
        // locked: two mates' connections locking each other's accounts
        // could otherwise wait on each other.
        let replies = match payload {
            [op::SQUAD, ..] | [0x21, SQUAD_MEMBER_PROFILE, ..] => self.squad_answer(payload),
            [op::PLAY, ..] => self.play_answer(payload),
            // The ranking reads every account.
            [0x21, lobby::status::ELO_LIST, ..] => self.ranking_answer(payload),
            _ => self.try_answer(payload),
        };
        // Most messages change the account (items, money, autobuy, skills,
        // quests, the active profile); the saver writes only what did.
        self.store.mark_dirty(self.account_id);
        let Some(replies) = replies else {
            log::warn!("Session {session_id:#x}: unreadable lobby message {payload:02X?}");
            return Vec::new();
        };
        log::debug!(
            "Session {session_id:#x}: {payload:02X?} -> {} replies",
            replies.len()
        );
        replies
    }

    /// `None` when the message is cut short.
    fn try_answer(&self, payload: &[u8]) -> Option<Vec<Vec<u8>>> {
        let mut reader = Reader::new(payload);
        let mut account = self.account.locked();
        let game = &mut account.game;
        let replies = match reader.u8()? {
            // Queries (0x21): the few the client asks; the rest are pushed.
            0x21 => match reader.u8()? {
                0x02 => {
                    let profile_id = reader.u32()?;
                    game.profile(profile_id)
                        .map(lobby::profile_contents)
                        .into_iter()
                        .collect()
                }
                0x05 => vec![lobby::money(game)],
                0x09 => vec![lobby::reputations(game)],
                0x0D => {
                    let now = quests::now();
                    let notices = quests::refresh(game, now, &mut Rng::from_time());
                    // A failed quest resets its faction's loyalty.
                    let reply = vec![lobby::quests(game, now), lobby::services(game, now)];
                    account.notices.extend(notices);
                    reply
                }
                0x0F => vec![lobby::services(game, quests::now())],
                0x12 => vec![lobby::unlocked_factions(0x0F)],
                // A finished match's results screen, and one player's part.
                survarium::v021d::results::MATCH_STATS => {
                    let match_id = reader.u32()?;
                    self.store
                        .reports()
                        .find(match_id)
                        .and_then(|report| report.screen_for(self.account_id))
                        .into_iter()
                        .collect()
                }
                survarium::v021d::results::PLAYER_MATCH_STATS => {
                    let player_id = reader.u32()?;
                    let match_id = reader.u32()?;
                    self.store
                        .reports()
                        .find(match_id)
                        .and_then(|report| report.player_details(player_id))
                        .into_iter()
                        .collect()
                }
                query => {
                    log::debug!(
                        "Session {:#x}: no answer to query {query:#04x}",
                        self.session_id
                    );
                    Vec::new()
                }
            },

            op::MOVE_ITEMS => {
                let _event = reader.u8()?;
                let profile_id = reader.u32()?;
                let count = reader.u8()?;
                let relocations = (0..count)
                    .map(|_| {
                        Some(Relocation {
                            item_id: reader.u32()?,
                            dict_id: reader.u16()?,
                            source_slot: reader.u8()?,
                            target_slot: reader.u8()?,
                            count: reader.u32()?,
                            count_in_inventory: reader.u32()?,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                match actions::move_items(game, self.dictionary, profile_id, &relocations) {
                    // The client has no confirmation for a move: the new
                    // inventory and profile are it, and the money, which
                    // missing ammo was bought with.
                    Ok(()) => [lobby::money(game), lobby::inventory(game)]
                        .into_iter()
                        .chain(game.profile(profile_id).map(lobby::profile_contents))
                        .collect(),
                    Err(text) => vec![
                        lobby::operation_denied(op::MOVE_ITEMS, text),
                        lobby::inventory(game),
                    ]
                    .into_iter()
                    .chain(game.profile(profile_id).map(lobby::profile_contents))
                    .collect(),
                }
            }

            op::SHOP => match reader.u8()? {
                0 => {
                    let dict_id = reader.u16()?;
                    let count = reader.u32()?;
                    let faction = reader.u8()?;
                    let _premium = reader.u8()?;
                    let _target_slot = reader.u8()?;
                    let _profile_id = reader.u32()?;
                    // The client then asks for its money (`21 05`).
                    let revisions: Vec<u32> = game
                        .profiles
                        .iter()
                        .map(|profile| profile.revision)
                        .collect();
                    match actions::buy(game, self.dictionary, dict_id, count, faction) {
                        // The slots holding the stack carry its new amount.
                        Ok(bought) => std::iter::once(lobby::bought(&bought))
                            .chain(
                                game.profiles
                                    .iter()
                                    .zip(&revisions)
                                    .filter(|(profile, revision)| profile.revision != **revision)
                                    .map(|(profile, _)| lobby::profile_contents(profile)),
                            )
                            .collect(),
                        Err(text) => vec![lobby::operation_denied(op::SHOP, text)],
                    }
                }
                1 => {
                    let item_id = reader.u32()?;
                    let amount = reader.u32()?;
                    match actions::sell(game, self.dictionary, item_id, amount) {
                        Ok((sold, profiles)) => vec![lobby::sold(item_id, sold, &profiles)],
                        Err(text) => vec![lobby::operation_denied(op::SHOP, text)],
                    }
                }
                // A premium store offer; the client then asks for its money
                // and services (`21 05`, `21 0F`).
                2 => {
                    let offer_id = reader.u32()?;
                    match premium::buy(game, offer_id, quests::now()) {
                        Ok(offer) => {
                            log::info!(
                                "Account {} bought {} hours of premium for {} gold",
                                self.account_id,
                                offer.hours,
                                offer.price
                            );
                            vec![lobby::premium_bought()]
                        }
                        Err(text) => vec![lobby::operation_denied(op::SHOP, text)],
                    }
                }
                _ => vec![lobby::operation_denied(op::SHOP, actions::CANNOT_BUY)],
            },

            // Changing a quest for a proposal: `2B 00 <active> <chosen>`.
            op::QUEST => match reader.u8()? {
                0 => {
                    let active = reader.u32()?;
                    let chosen = reader.u32()?;
                    let now = quests::now();
                    if quests::swap(game, active, chosen, now) {
                        log::info!(
                            "Session {:#x} changed quest {active} for {chosen}",
                            self.session_id
                        );
                        // The client asks for its quests after it; the
                        // change cost loyalty.
                        vec![lobby::quest_changed(), lobby::services(game, now)]
                    } else {
                        // No text of the game's fits; the list as it is.
                        vec![lobby::quests(game, now)]
                    }
                }
                _ => Vec::new(),
            },
            op::SKILLS => match reader.u8()? {
                0 => {
                    let profile_id = reader.u32()?;
                    let skills = (0..reader.u8()?)
                        .map(|_| Some((reader.u8()?, reader.u8()?)))
                        .collect::<Option<Vec<_>>>()?;
                    let perks = (0..reader.u8()?)
                        .map(|_| reader.u8())
                        .collect::<Option<Vec<_>>>()?;
                    let result =
                        actions::set_skills(game, self.dictionary, profile_id, &skills, &perks);
                    self.skills_reply(game, profile_id, result)
                }
                _ => {
                    let profile_id = reader.u32()?;
                    let result = actions::reroll_skills(game, profile_id, REROLL_COST);
                    let mut replies = self.skills_reply(game, profile_id, result);
                    replies.push(lobby::money(game));
                    replies
                }
            },

            op::PROFILES => match reader.u8()? {
                3 => {
                    let profile_id = reader.u32()?;
                    let on = reader.bool()?;
                    match game
                        .profiles
                        .iter_mut()
                        .find(|profile| profile.id == profile_id)
                    {
                        Some(profile) => {
                            profile.autobuy = on;
                            vec![lobby::autobuy_set(profile_id, on)]
                        }
                        None => Vec::new(),
                    }
                }
                // `29 04`, the active profile: answering it with the profile
                // would make the client send it again (spec R8). Squad mates
                // see it.
                _ => {
                    account.active_profile = Some(reader.u32()?);
                    Vec::new()
                }
            },

            // Leaving the queue, with the squad.
            0x27 => {
                account.play = None;
                self.store.matchmaking().leave_queue(self.session_id);
                self.store.squads().set_ready(self.account_id, false);
                self.queued_since.set(None);
                vec![lobby::client_state(ClientState::Lobby)]
            }

            id => {
                log::debug!(
                    "Session {:#x}: ignored lobby message {id:#04x}",
                    self.session_id
                );
                Vec::new()
            }
        };
        Some(replies)
    }

    /// `2A` squad changes and `21 10` a mate's profile. The client has no
    /// answer for 0x2A: the squad status (0x3A) that follows a change is it,
    /// and a refused change is only logged.
    fn squad_answer(&self, payload: &[u8]) -> Option<Vec<Vec<u8>>> {
        let mut reader = Reader::new(payload);
        let squads = self.store.squads();
        let me = self.account_id;
        if reader.u8()? == 0x21 {
            reader.u8()?;
            let mate = reader.u32()?;
            let reply = squads
                .of(me)
                .filter(|squad| mate != me && squad.members.contains(&mate))
                .and_then(|_| self.store.account(mate))
                .and_then(|account| {
                    let account = account.locked();
                    shown_profile(&account)
                        .map(|profile| lobby::squad_member_profile(mate, profile))
                });
            return Some(reply.into_iter().collect());
        }
        let result = match reader.u8()? {
            1 => squads.disband(me),
            2 => {
                let name = reader.string()?;
                match self.account_named(name) {
                    Some(member) => squads.add(me, member).map(|_| ()),
                    None => Err(session::SquadError::NotInvited),
                }
            }
            3 => squads.drop_member(me, reader.u32()?),
            _ => squads.leave(me),
        };
        match result {
            Ok(()) => log::info!(
                "Session {:#x}: squad change {payload:02X?}",
                self.session_id
            ),
            Err(error) => {
                log::info!(
                    "Session {:#x}: squad change {payload:02X?} refused: {error:?}",
                    self.session_id
                );
                // Its client asked for a change to a squad the server does
                // not have as it shows it: it gets the squad as it is.
                self.squad_sent.replace(Vec::new());
            }
        }
        Some(Vec::new())
    }

    /// The account named `name`, ignoring ASCII case.
    fn account_named(&self, name: &[u8]) -> Option<u32> {
        self.store
            .find_accounts(name, self.account_id, usize::MAX)
            .into_iter()
            .find(|(_, found)| found.as_bytes().eq_ignore_ascii_case(name))
            .map(|(id, _)| id)
    }

    /// The squad status when it changed since the client last got it, and
    /// the lobby state for a player taken out of the queue because the
    /// squad changed.
    fn squad_update(&self) -> Vec<Vec<u8>> {
        let squad = self.store.squads().of(self.account_id);
        let status = match &squad {
            None => lobby::squad_status(0, &[], false, 0),
            Some(squad) => {
                let members: Vec<_> = squad
                    .members
                    .iter()
                    .map(|member| self.squad_member(squad, *member))
                    .collect();
                let commander = squad.commander() == self.account_id;
                lobby::squad_status(squad.id, &members, commander, squad.revision)
            }
        };
        let mut updates = Vec::new();
        let none = lobby::squad_status(0, &[], false, 0);
        if *self.squad_sent.borrow() != status {
            let had_squad = {
                let sent = self.squad_sent.borrow();
                !sent.is_empty() && *sent != none
            };
            self.squad_sent.replace(status.clone());
            updates.push(status);
            // Its squad is no more (a mate left it alone, or its game was
            // found gone): out of the queue the squad was in, as for any
            // squad change.
            if squad.is_none() && had_squad && self.queued_since.get().is_some() {
                self.queued_since.set(None);
                self.store.matchmaking().leave_queue(self.session_id);
                updates.push(lobby::client_state(ClientState::Lobby));
            }
        }
        if let Some(squad) = &squad
            && self.queued_since.get().is_some()
            && !squad.is_ready(self.account_id)
        {
            self.queued_since.set(None);
            self.store.matchmaking().leave_queue(self.session_id);
            updates.push(lobby::client_state(ClientState::Lobby));
        }
        updates
    }

    /// One member as this player's client is shown it. Its own entry
    /// carries its own profile id; a mate's carries the mate's account id,
    /// which is none of this player's profile ids, so the client asks for
    /// the mate's profile (`21 10`) to show its character.
    fn squad_member(&self, squad: &Squad, member: u32) -> SquadMember {
        let (name, profile) = match self.store.account(member) {
            Some(account) => {
                let account = account.locked();
                let profile = shown_profile(&account).map(|profile| (profile.id, profile.revision));
                (account.name.clone(), profile)
            }
            None => (String::new(), None),
        };
        let (profile_id, profile_revision) = profile.unwrap_or_default();
        SquadMember {
            account_id: member,
            profile_id: if member == self.account_id {
                profile_id
            } else {
                member
            },
            profile_revision,
            name,
            online: self.store.is_online(member),
            ready: squad.is_ready(member),
        }
    }

    fn skills_reply(
        &self,
        game: &survarium::v021d::account::Account,
        profile_id: u32,
        result: Result<(), &'static str>,
    ) -> Vec<Vec<u8>> {
        let mut replies = Vec::new();
        let changed = result.is_ok();
        if let Err(text) = result {
            replies.push(lobby::operation_denied(op::SKILLS, text));
        }
        // The profile's skills as they are now, changed or not, and once
        // changed its contents, which carry the modifiers they give (the
        // carried weight the inventory shows).
        if let Some(profile) = game.profile(profile_id) {
            replies.push(lobby::skills(profile));
            if changed {
                replies.push(lobby::profile_contents(profile));
            }
        }
        replies
    }
}

/// `host` as a numeric IPv4 address, resolved if it is a name.
fn ipv4_host(host: &str, port: u16) -> String {
    use std::net::{Ipv4Addr, SocketAddr, ToSocketAddrs};
    if host.parse::<Ipv4Addr>().is_ok() {
        return host.to_string();
    }
    let resolved = (host, port).to_socket_addrs().ok().and_then(|mut addrs| {
        addrs.find_map(|addr| match addr {
            SocketAddr::V4(addr) => Some(addr.ip().to_string()),
            SocketAddr::V6(_) => None,
        })
    });
    resolved.unwrap_or_else(|| {
        log::warn!("The match server host `{host}` has no IPv4 address; the client can't use it");
        host.to_string()
    })
}

/// The profile squad mates see: the one the player last chose, else its first.
fn shown_profile(account: &Account) -> Option<&survarium::v021d::account::Profile> {
    account
        .active_profile
        .and_then(|id| account.game.profile(id))
        .or_else(|| account.game.profiles.first())
}

/// `21 10`: the query for a squad mate's profile.
const SQUAD_MEMBER_PROFILE: u8 = 0x10;

#[cfg(test)]
mod tests {
    use super::*;

    fn connection(store: &SessionStore) -> Connection<'_> {
        Connection {
            session_id: 1,
            account_id: 1,
            account: Arc::new(Mutex::new(Account::new(1, "Player".into()))),
            store,
            dictionary: Dictionary::shared(),
            queued_since: Cell::new(None),
            squad_sent: RefCell::new(lobby::squad_status(0, &[], false, 0)),
            match_host: "127.0.0.1".into(),
            sent_to_match: Cell::new(None),
        }
    }

    /// A connection for an account in the store, as signing in makes it.
    fn player<'a>(store: &'a SessionStore, email: &str) -> Connection<'a> {
        let localhost = std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST);
        let account_id = store.get_or_create_account(localhost, email);
        Connection {
            session_id: account_id,
            account_id,
            account: store.account(account_id).unwrap(),
            store,
            dictionary: Dictionary::shared(),
            queued_since: Cell::new(None),
            squad_sent: RefCell::new(lobby::squad_status(0, &[], false, 0)),
            match_host: "127.0.0.1".into(),
            sent_to_match: Cell::new(None),
        }
    }

    fn first_profile(connection: &Connection) -> (u32, u32) {
        let account = connection.account.locked();
        let profile = &account.game.profiles[0];
        (profile.id, profile.revision)
    }

    /// The squad status `viewer` should get: itself with its own profile id,
    /// mates with their account ids, nobody online (no sessions here).
    fn expected_status(viewer: &Connection, members: &[(&Connection, &str, bool)]) -> Vec<u8> {
        let squad = viewer.store.squads().of(viewer.account_id).unwrap();
        let members: Vec<_> = members
            .iter()
            .map(|(member, name, ready)| {
                let (profile_id, profile_revision) = first_profile(member);
                SquadMember {
                    account_id: member.account_id,
                    profile_id: if member.account_id == viewer.account_id {
                        profile_id
                    } else {
                        member.account_id
                    },
                    profile_revision,
                    name: name.to_string(),
                    online: false,
                    ready: *ready,
                }
            })
            .collect();
        let commander = squad.commander() == viewer.account_id;
        lobby::squad_status(squad.id, &members, commander, squad.revision)
    }

    #[test]
    fn a_client_back_in_the_lobby_is_told_its_squad_as_it_is() {
        let store = SessionStore::new();
        let alice = player(&store, "alice");
        // A new lobby connection: its client may show a squad from before a
        // restart, which the server forgot.
        alice.squad_sent.replace(Vec::new());
        assert_eq!(
            alice.squad_update(),
            [lobby::squad_status(0, &[], false, 0)]
        );
        assert!(alice.squad_update().is_empty());
    }

    #[test]
    fn a_squad_gone_under_a_waiting_member_takes_it_out_of_the_queue() {
        let store = SessionStore::new();
        let alice = player(&store, "alice");
        let bob = player(&store, "bob");
        store
            .squads()
            .invite_accepted(alice.account_id, bob.account_id);
        store.squads().add(alice.account_id, bob.account_id).unwrap();
        alice.squad_update();
        // Waiting, as a squad member does for the others.
        alice.queued_since.set(Some(Instant::now()));
        // Bob's game is found gone: a squad of one is none.
        store.squads().leave(bob.account_id).unwrap();
        assert_eq!(
            alice.squad_update(),
            [
                lobby::squad_status(0, &[], false, 0),
                lobby::client_state(ClientState::Lobby)
            ]
        );
        assert_eq!(alice.queued_since.get(), None);
    }

    fn add_to_squad(name: &str) -> Vec<u8> {
        let mut payload = vec![op::SQUAD, 2, name.len() as u8];
        payload.extend(name.as_bytes());
        payload
    }

    #[test]
    fn a_squad_forms_from_an_accepted_invite_and_readies_with_play() {
        let store = SessionStore::new();
        let alice = player(&store, "alice");
        let bob = player(&store, "bob");

        // Without an accepted invite there is no squad, and the client is
        // told so (it may show one the server doesn't have).
        assert!(alice.answer(&add_to_squad("bob")).is_empty());
        assert_eq!(
            alice.squad_update(),
            [lobby::squad_status(0, &[], false, 0)]
        );
        assert!(alice.squad_update().is_empty());

        store
            .squads()
            .invite_accepted(alice.account_id, bob.account_id);
        assert!(alice.answer(&add_to_squad("BOB")).is_empty());
        assert_eq!(
            alice.squad_update(),
            [expected_status(
                &alice,
                &[(&alice, "alice", false), (&bob, "bob", false)]
            )]
        );
        assert_eq!(
            bob.squad_update(),
            [expected_status(
                &bob,
                &[(&alice, "alice", false), (&bob, "bob", false)]
            )]
        );
        // Sent once.
        assert!(alice.squad_update().is_empty());

        // Alice's client asks for bob's profile to show his character.
        let mut query = vec![0x21, 0x10];
        query.extend(bob.account_id.to_le_bytes());
        let bob_profile = {
            let account = bob.account.locked();
            lobby::squad_member_profile(bob.account_id, &account.game.profiles[0])
        };
        assert_eq!(alice.answer(&query), [bob_profile]);

        // Bob's Play makes him ready; his queue screen shows the squad isn't.
        let mut play = vec![op::PLAY];
        play.extend(first_profile(&bob).0.to_le_bytes());
        assert_eq!(bob.answer(&play).len(), 1);
        assert_eq!(
            alice.squad_update(),
            [expected_status(
                &alice,
                &[(&alice, "alice", false), (&bob, "bob", true)]
            )]
        );

        // Leaving the queue unreadies him.
        assert_eq!(
            bob.answer(&[0x27, 0, 0, 0, 0]),
            [lobby::client_state(ClientState::Lobby)]
        );
        assert!(
            !store
                .squads()
                .of(alice.account_id)
                .unwrap()
                .is_ready(bob.account_id)
        );

        // Bob leaves: a squad of one is none.
        assert!(bob.answer(&[op::SQUAD, 4]).is_empty());
        assert_eq!(
            alice.squad_update(),
            [lobby::squad_status(0, &[], false, 0)]
        );
        assert_eq!(bob.squad_update(), [lobby::squad_status(0, &[], false, 0)]);
    }

    #[test]
    fn a_squad_change_takes_the_waiting_members_out_of_the_queue() {
        let store = SessionStore::new();
        let alice = player(&store, "alice");
        let bob = player(&store, "bob");
        store
            .squads()
            .invite_accepted(alice.account_id, bob.account_id);
        let name = bob.account.locked().name.clone();
        alice.answer(&add_to_squad(&name));
        let mut play = vec![op::PLAY];
        play.extend(first_profile(&bob).0.to_le_bytes());
        bob.answer(&play);
        bob.squad_update();

        // Alice leaves: a squad of one is none.
        assert!(alice.answer(&[op::SQUAD, 4]).is_empty());
        let updates = bob.squad_update();
        assert_eq!(updates.len(), 2);
        assert_eq!(updates[1], lobby::client_state(ClientState::Lobby));
        assert!(bob.queue_status().is_empty());
    }

    #[test]
    fn the_sign_in_is_read_whole() {
        let mut payload = vec![SIGN_IN];
        payload.extend(0xDD00_u32.to_le_bytes());
        payload.push(0xFF);
        payload.extend(u32::MAX.to_le_bytes());
        payload.push(0);
        assert_eq!(
            SignIn::parse(&payload),
            Some(SignIn {
                session_id: 0xDD00,
                just_left_match: true,
                pending_match_stats_id: u32::MAX,
                need_shop_items: false,
            })
        );
        // The 0.100b sign-in (5 bytes) is no 0.21d one.
        assert_eq!(SignIn::parse(&payload[..5]), None);
    }

    #[test]
    fn the_ranking_page_gets_the_places_it_asks_for() {
        let store = SessionStore::new();
        let alice = player(&store, "alice");
        let bob = player(&store, "bob");
        store.account(bob.account_id).unwrap().locked().game.rating = 1016;
        // From place 1, 100 asked: bob, then alice.
        let replies = alice.answer(&[0x21, 0x0C, 0, 0, 0, 0, 100]);
        // The first page also brings the count and the player's own row,
        // as they stand now: alice is second.
        let alice_game = store
            .account(alice.account_id)
            .unwrap()
            .locked()
            .game
            .clone();
        assert_eq!(
            replies,
            [
                lobby::base_info(&alice_game, 2),
                lobby::player_elo(2, &alice_game),
                lobby::elo_list(0, &store.ranking()),
            ]
        );
        assert_eq!(&replies[2][..7], &[0x36, 0x0C, 2, 3, b'b', b'o', b'b']);
        // From place 2.
        let replies = alice.answer(&[0x21, 0x0C, 1, 0, 0, 0, 100]);
        assert_eq!(replies, [lobby::elo_list(1, &store.ranking()[1..])]);
        // Past the end: none.
        let replies = alice.answer(&[0x21, 0x0C, 9, 0, 0, 0, 100]);
        assert_eq!(replies, [lobby::elo_list(9, &[])]);
        assert_eq!(alice.ranked(), (2, 2));
    }

    #[test]
    fn a_profile_query_gets_the_profile() {
        let store = SessionStore::new();
        let connection = connection(&store);
        let replies = connection.answer(&[0x21, 0x02, 1, 0, 0, 0]);
        assert_eq!(replies.len(), 1);
        assert_eq!(&replies[0][..6], &[0x36, 0x02, 1, 0, 0, 0]);
        // An unknown profile: nothing, rather than a profile the client
        // would not find.
        assert!(connection.answer(&[0x21, 0x02, 9, 0, 0, 0]).is_empty());
        // The active profile is not answered.
        assert!(connection.answer(&[0x29, 0x04, 1, 0, 0, 0]).is_empty());
    }

    #[test]
    fn buying_answers_with_what_was_bought() {
        let store = SessionStore::new();
        let connection = connection(&store);
        // 40 rounds of buckshot from anyone, for the active profile 1.
        let mut buy = vec![0x24, 0x00, 22, 0];
        buy.extend(40_u32.to_le_bytes());
        buy.extend([0, 0, 100, 1, 0, 0, 0]);
        let replies = connection.answer(&buy);
        assert_eq!(&replies[0][..5], &[0x34, 0x24, 0x00, 1, 22]);
        // Cut short: no reply, nothing bought.
        let money = connection.account.locked().game.generic_money;
        assert!(connection.answer(&buy[..6]).is_empty());
        assert_eq!(connection.account.locked().game.generic_money, money);
    }

    #[test]
    fn a_refused_move_resends_the_profile() {
        let store = SessionStore::new();
        let connection = connection(&store);
        // The starting pistol (slot 7) into the helmet slot.
        let pistol = connection.account.locked().game.profiles[0].slots[7]
            .clone()
            .unwrap();
        let mut moved = vec![0x23, 0x00, 1, 0, 0, 0, 1];
        moved.extend(pistol.item_id.to_le_bytes());
        moved.extend(pistol.dict_id.to_le_bytes());
        moved.extend([7, 0]);
        moved.extend(1_u32.to_le_bytes());
        moved.extend(1_u32.to_le_bytes());
        let replies = connection.answer(&moved);
        let kinds: Vec<&[u8]> = replies.iter().map(|reply| &reply[..2]).collect();
        assert_eq!(kinds, [&[0x35, 0x23][..], &[0x36, 0x03], &[0x36, 0x02]]);
    }

    #[test]
    fn play_waits_in_the_queue_and_leaving_returns_to_the_lobby() {
        let store = SessionStore::with_gather_window(Duration::from_secs(600));
        let connection = connection(&store);
        let replies = connection.answer(&[0x20, 1, 0, 0, 0]);
        // In the queue: first of one.
        assert_eq!(&replies[0][..3], &[0x36, 0x00, 1]);
        assert_eq!(&replies[0][7..15], &[1, 0, 0, 0, 1, 0, 0, 0]);
        // Pressing Play again keeps its place.
        connection.answer(&[0x20, 1, 0, 0, 0]);
        assert_eq!(connection.queue_status().len(), 1);
        assert_eq!(
            connection.answer(&[0x27, 1, 0, 0, 0]),
            [vec![0x36, 0x00, 0]]
        );
        assert!(connection.queue_status().is_empty());
    }

    #[test]
    fn a_formed_match_sends_the_player_to_the_match_server_once() {
        let store = SessionStore::with_gather_window(Duration::ZERO);
        let connection = connection(&store);
        // A match needs a second player.
        store.matchmaking().queue(
            &[session::Player {
                session_id: 9999,
                account_id: 9999,
                profile_id: 1,
            }],
            false,
            Instant::now(),
        );
        let replies = connection.answer(&[0x20, 1, 0, 0, 0]);
        let port = vostok::config::get().match_server.port;
        assert_eq!(
            replies,
            [
                lobby::client_state(ClientState::InQueue {
                    order_id: 1,
                    place: 0,
                    queue_total: 0,
                    current_wait_seconds: 0,
                    average_wait_seconds: 0,
                }),
                lobby::connect_to_match_server("127.0.0.1", port, 0x123, 1),
            ]
        );
        assert!(connection.queue_status().is_empty());
        // The match ends: the lobby is back.
        store.matchmaking().end_match(0x123);
        assert_eq!(
            connection.queue_status(),
            [lobby::client_state(ClientState::Lobby)]
        );
        assert!(connection.queue_status().is_empty());
    }

    #[test]
    fn a_seat_made_while_its_lobby_connection_was_away_is_sent_on_the_next() {
        let store = SessionStore::with_gather_window(Duration::ZERO);
        // Session 1 pressed Play on a connection that dropped; its match
        // formed while it had none.
        let other = session::Player {
            session_id: 9999,
            account_id: 9999,
            profile_id: 1,
        };
        let me = session::Player {
            session_id: 1,
            account_id: 1,
            profile_id: 1,
        };
        store.matchmaking().queue(&[me], false, Instant::now());
        store.matchmaking().queue(&[other], false, Instant::now());
        // Its new lobby connection never pressed Play.
        let connection = connection(&store);
        let port = vostok::config::get().match_server.port;
        let updates = connection.queue_status();
        assert_eq!(updates.len(), 2);
        assert_eq!(
            updates[1],
            lobby::connect_to_match_server("127.0.0.1", port, 0x123, 0)
        );
        assert!(connection.queue_status().is_empty());
        // Another connection of the session later: sent already.
        assert!(super::tests::connection(&store).queue_status().is_empty());
    }

    #[test]
    fn a_squad_queues_when_its_last_member_plays() {
        let store = SessionStore::with_gather_window(Duration::ZERO);
        let alice = player(&store, "alice");
        let bob = player(&store, "bob");
        let carol = player(&store, "carol");
        store
            .squads()
            .invite_accepted(alice.account_id, bob.account_id);
        alice.answer(&add_to_squad("bob"));
        let play = |connection: &Connection| {
            let mut play = vec![op::PLAY];
            play.extend(first_profile(connection).0.to_le_bytes());
            connection.answer(&play)
        };
        // Bob alone ready: nobody is placed.
        play(&bob);
        assert_eq!(store.matchmaking().seat(bob.session_id), None);
        // The whole squad ready: it is in the queue, and waits. The two
        // play on one team, so a match of them alone has nobody on the
        // other (they were given one until 2026-10-12).
        let replies = play(&alice);
        assert_eq!(replies.len(), 1, "0x33: waiting");
        assert_eq!(store.matchmaking().seat(alice.session_id), None);
        assert_eq!(store.matchmaking().seat(bob.session_id), None);
        assert_eq!(bob.queue_status().len(), 1);
        // One player who is not of the squad is enough.
        let replies = play(&carol);
        assert_eq!(replies.len(), 2, "match found and 0x33");
        let (match_id, _) = store.matchmaking().seat(carol.session_id).unwrap();
        assert_eq!(
            store.matchmaking().seat(alice.session_id),
            Some((match_id, 0))
        );
        assert_eq!(
            store.matchmaking().seat(bob.session_id),
            Some((match_id, 1))
        );
        let seats = store.matchmaking().formed_match(match_id).unwrap().seats;
        assert_eq!(seats.len(), 3);
        assert_eq!(seats[0].team, seats[1].team);
        assert_ne!(seats[0].team, seats[2].team);
        assert_eq!(bob.queue_status().len(), 2);
    }

    #[test]
    fn a_finished_match_shows_its_results_once_and_on_asking() {
        use survarium::v021d::results::{PlayerStats, Reward};
        let store = SessionStore::new();
        let alice = player(&store, "alice");
        store.reports().add(session::MatchReport {
            match_id: 0x123,
            winner: 0,
            team_points: [1, 0],
            players: vec![PlayerStats {
                id: 1,
                name: "alice".into(),
                ..PlayerStats::default()
            }],
            accounts: vec![alice.account_id],
            rewards: vec![Reward::default()],
        });
        let updates = alice.results_update();
        let kinds: Vec<&[u8]> = updates.iter().map(|update| &update[..2]).collect();
        assert_eq!(
            kinds,
            [
                &[0x36, 0x05][..],
                &[0x36, 0x03],
                &[0x36, 0x01],
                &[0x36, 0x12],
                &[0x36, 0x09],
                &[0x36, 0x0B],
                &[0x36, 0x0F],
                &[0x36, 0x06],
                &[0x36, 0x13]
            ]
        );
        assert!(alice.results_update().is_empty());
        // Asked again (`21 13`), and one player's part (`21 0E`).
        assert_eq!(
            alice.answer(&[0x21, 0x13, 0x23, 1, 0, 0]),
            [updates.last().unwrap().clone()]
        );
        let details = alice.answer(&[0x21, 0x0E, 1, 0, 0, 0, 0x23, 1, 0, 0]);
        assert_eq!(details[0][..2], [0x36, 0x0E]);
        assert!(alice.answer(&[0x21, 0x13, 0x24, 1, 0, 0]).is_empty());
    }

    #[test]
    fn the_ranking_table_is_empty() {
        let store = SessionStore::new();
        let connection = connection(&store);
        // No rows; the page's first request still brings the count and the
        // player's own row (unranked here, so place 0).
        let replies = connection.answer(&[0x21, 0x0C, 0, 0, 0, 0, 1]);
        assert_eq!(replies.len(), 3);
        assert_eq!(replies[0][..2], [0x36, 0x0A]);
        assert_eq!(replies[1][..2], [0x36, 0x0B]);
        assert_eq!(replies[2], [0x36, 0x0C, 0]);
    }

    /// Every lobby message that changes the account marks it, and the
    /// database then holds the account exactly as it is in memory; a
    /// restart brings it back the same.
    #[test]
    fn every_change_is_saved_and_survives_a_restart() {
        use session::persist::{AccountRecord, AccountRepository, SqliteRepository};
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("srp-lobby-persist-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("srp.db");
        let open = || {
            let repository = SqliteRepository::open(&path).unwrap();
            SessionStore::with_repository(Duration::from_secs(30), Box::new(repository)).unwrap()
        };
        let kept = |account_id: u32| {
            let mut repository = SqliteRepository::open(&path).unwrap();
            let loaded = repository.load_all().unwrap().accounts;
            loaded
                .into_iter()
                .find(|record| record.id == account_id)
                .unwrap()
        };

        let store = open();
        let connection = player(&store, "Keeper");
        let id = connection.account_id;
        assert_eq!(store.flush(), 1);
        let step = |payload: &[u8]| {
            connection.answer(payload);
            assert!(
                store.is_dirty(id),
                "{payload:02X?} left the account unmarked"
            );
            assert_eq!(store.flush(), 1, "{payload:02X?} changed nothing");
            assert!(!store.is_dirty(id));
            let account = AccountRecord::of(&connection.account.locked());
            assert_eq!(kept(id), account, "{payload:02X?} saved something else");
            account
        };

        // Autobuy on and off.
        let on = step(&[0x29, 3, 1, 0, 0, 0, 1]);
        assert!(on.game.profiles[0].autobuy);
        let off = step(&[0x29, 3, 1, 0, 0, 0, 0]);
        assert!(!off.game.profiles[0].autobuy);
        // The active profile.
        assert_eq!(step(&[0x29, 4, 1, 0, 0, 0]).active_profile, Some(1));

        // 40 rounds of buckshot bought, then 20 sold.
        let mut buy = vec![0x24, 0x00, 22, 0];
        buy.extend(40_u32.to_le_bytes());
        buy.extend([0, 0, 100, 1, 0, 0, 0]);
        let bought = step(&buy);
        let buckshot = bought.game.inventory.iter().find(|item| item.dict_id == 22);
        let buckshot = buckshot.unwrap().item_id;
        let mut sell = vec![0x24, 0x01];
        sell.extend(buckshot.to_le_bytes());
        sell.extend(20_u32.to_le_bytes());
        step(&sell);

        // The ammo dialog: 30 rounds a life in the pistol's ammo slot, 100
        // owned, the missing ones bought.
        let ammo = bought.game.profiles[0].slots[8].clone().unwrap();
        let mut moved = vec![0x23, 0x00, 1, 0, 0, 0, 1];
        moved.extend(ammo.item_id.to_le_bytes());
        moved.extend(ammo.dict_id.to_le_bytes());
        moved.extend([8, 8]);
        moved.extend(30_u32.to_le_bytes());
        moved.extend(100_u32.to_le_bytes());
        let moved = step(&moved);
        let slot = moved.game.profiles[0].slots[8].clone().unwrap();
        assert_eq!((slot.condition_or_stack, slot.amount), (30, 100));

        // A skill point spent.
        let skill = Dictionary::shared().skills[0];
        let skilled = step(&[0x25, 0, 1, 0, 0, 0, 1, skill, 1, 0]);
        assert_eq!(skilled.game.profiles[0].skills, [(skill, 1)]);
        // Its modifiers are in its contents: a new revision.
        assert_eq!(
            skilled.game.profiles[0].revision,
            moved.game.profiles[0].revision.wrapping_add(1)
        );

        // Quests given, then one changed for a proposal.
        let given = step(&[0x21, 0x0D]);
        let now = quests::now();
        let pair = given.game.quests.iter().find_map(|active| {
            given.game.quests.iter().find_map(|chosen| {
                let mut game = given.game.clone();
                quests::swap(&mut game, active.instance_id, chosen.instance_id, now)
                    .then_some((active.instance_id, chosen.instance_id))
            })
        });
        let (active, chosen) = pair.expect("a quest to change for a proposal");
        let mut swap = vec![0x2B, 0];
        swap.extend(active.to_le_bytes());
        swap.extend(chosen.to_le_bytes());
        step(&swap);

        // A day of premium.
        let premium = step(&[0x24, 2, 1, 0, 0, 0]);
        assert!(premium.game.premium_until > now);

        // Autobuy on, as a restart must find it.
        let last = step(&[0x29, 3, 1, 0, 0, 0, 1]);
        drop(connection);
        drop(store);
        let store = open();
        let account = store.account(id).unwrap();
        assert_eq!(AccountRecord::of(&account.locked()), last);
        assert!(account.locked().game.profiles[0].autobuy);
        drop(store);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
