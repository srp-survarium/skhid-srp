//! A scripted 0.26g match client: the client's side of the transport (its
//! own datagram builder, the crate's parser), the
//! `match_msgs::client_model` deciding what it takes, and a stand-in for
//! the simulation that gives every client in step the same world for the
//! same tick. The tests drive it; the test bot (`srp-bot`) drives one that
//! simulates nothing ([`Client::idle`]).
//!
//! Anything a real client would drop or choke on lands in `faults`; a
//! correct server leaves it empty.

use std::collections::{BTreeMap, VecDeque};

use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::maps::{self, mode};
use survarium::v026g::match_msgs::client_model::{ClientModel, Skip, Taken};
use survarium::v026g::match_msgs::late_join::{CONNECT_CAPABILITY, Prepare, Prepared, Status};
use survarium::v026g::match_msgs::motion;
use survarium::v026g::match_msgs::players::{self, PlayerState};
use survarium::v026g::match_msgs::rules::{TimelimitParams, TimelimitRule, state};
use survarium::v026g::match_msgs::snapshot::{
    self, Battery, Container, Gather, KillsDeaths, PlayerStatistics, Respawn, Rules, Sections,
    Statistics, StatisticsTail, Tdm, Timed, Timelimit, VictoryItemStatus,
};
use survarium::v026g::match_msgs::{
    PlayerInput, ServerMessage, TICK_MS, WireTime, id, tick_byte, upload,
};
use udp_transport::seq::{Recorded, Window};
use udp_transport::wire::{self, low_level};
use udp_transport::{Class, Direction, Table, ack};

/// The client's event horizon: `max_ping_time_in_ms` + 250.
pub const HORIZON_MS: u32 = 500;
/// The initial world's time in the tests.
pub const INITIAL_MS: u32 = 50;

/// The stand-in for a client's simulation: its world at a tick, the same
/// bytes on every client that has the same history.
#[derive(Clone, Debug)]
pub struct Sim {
    pub players: usize,
    pub snapshot_players: usize,
    pub map_id: u8,
    /// The timelimit rule as the last snapshot gave it, or as the match
    /// was made ready.
    base: TimelimitRule,
    base_time: u32,
    /// Roster changes taken since: time, player, entered.
    events: Vec<(u32, u8, bool)>,
    /// From a tick on: the points in the statistics.
    pub points_from: Option<(u32, [u16; 2])>,
    /// From a tick on: Team Deathmatch's rule.
    pub tdm_from: Option<(u32, Tdm)>,
    /// From a tick on: whether each team may still respawn (the rules'
    /// `team_respawn`; both may otherwise).
    pub team_respawn_from: Option<(u32, [bool; 2])>,
    /// From a tick on: kills of player 0 and deaths of player 1.
    pub kills_from: Option<(u32, u16)>,
    /// From a tick on the world differs from the others' by this byte.
    pub salt_from: Option<(u32, u8)>,
    /// Extra bytes in every world (a bigger upload).
    pub ballast: usize,
    /// From a tick on the world's game objects have this many bytes: the
    /// grenades and traps of a place this game installed, or started with.
    /// A loaded world does not take them away, as it does the salt: they
    /// are this game's objects.
    pub late_objects_from: Option<(u32, usize)>,
    /// By place: the head of its player's state, for a place whose player
    /// block is in every world (alive, and what it has in store of each
    /// item of its profile).
    pub player_states: Vec<Option<PlayerState>>,
    /// By place: the end of its player's state (a body where the world
    /// has it, `motion::Tail`), at the world's time; with none the state
    /// ends at the head and an opaque inventory.
    pub player_tails: Vec<Option<motion::Tail>>,
    /// No world was loaded yet: the next is this game's first.
    first_world: bool,
    /// From a tick on the statistics have the tickets of the match's
    /// start: the 0.26g client's own defect, which a game has from a first
    /// world that is a Team Deathmatch in play until its next world
    /// (match.md section 21; `Decoded::first_load_loses_the_standing`).
    stale_tickets_from: Option<u32>,
}

impl Sim {
    pub fn new(players: usize, map_id: u8) -> Self {
        let map = maps::map_mode(map_id).expect("a map of the tests");
        let mut base = TimelimitRule::new(TimelimitParams::of(map, players as u8));
        base.on_match_ready(INITIAL_MS);
        Self {
            players,
            snapshot_players: players,
            map_id,
            base,
            base_time: INITIAL_MS,
            events: Vec::new(),
            points_from: None,
            tdm_from: None,
            team_respawn_from: None,
            kills_from: None,
            salt_from: None,
            ballast: 0,
            late_objects_from: None,
            player_states: vec![None; players],
            player_tails: vec![None; players],
            first_world: true,
            stale_tickets_from: None,
        }
    }

    /// A team's tickets at the start (`tickets_count`).
    fn tickets_count(&self) -> u16 {
        maps::map_mode(self.map_id).map_or(0, |map| map.tickets_count as u16)
    }

    fn game_mode(&self) -> u8 {
        maps::map_mode(self.map_id).map_or(mode::BATTERIES, |map| map.mode)
    }

    fn batteries(&self) -> usize {
        if self.game_mode() == mode::TEAM_DEATHMATCH {
            0
        } else {
            maps::map_mode(self.map_id).map_or(0, |map| map.victory_items_count as usize)
        }
    }

    /// The timelimit rule as it stands at `time`.
    pub fn rule_at(&self, time: u32) -> TimelimitRule {
        let mut rule = self.base;
        let mut events: Vec<_> = self
            .events
            .iter()
            .filter(|(at, _, _)| *at > self.base_time && *at <= time)
            .copied()
            .collect();
        events.sort();
        for (at, player, entered) in events {
            rule.tick(at);
            if entered {
                rule.on_player_entered(player, at);
            } else {
                rule.on_player_left(player, at);
            }
        }
        rule.tick(time);
        rule
    }

    /// No simulation at all, for a client that has none ([`Client::idle`]):
    /// never asked for a world.
    fn none() -> Self {
        let map = maps::playable().next().expect("the game has maps");
        Self::new(1, map.id)
    }

    /// The rule is as `rule` from now on, whatever came before (a test's
    /// shortcut past the countdown).
    pub fn force_rule(&mut self, rule: TimelimitRule) {
        self.base = rule;
        self.base_time = u32::MAX;
    }

    /// The world's body at `time`.
    pub fn world(&self, time: u32) -> Vec<u8> {
        let since = |from: u32| from <= time;
        let rule = self.rule_at(time);
        let mut players = vec![PlayerStatistics::default(); self.snapshot_players];
        let mut kills_deaths = vec![KillsDeaths::default(); self.snapshot_players];
        if let Some((_, kills)) = self.kills_from.filter(|(from, _)| since(*from)) {
            players[0].events[snapshot::event::KILL][0] = kills;
            kills_deaths[0].kills = kills;
            if let Some(victim) = players.get_mut(1) {
                victim.events[snapshot::event::DEATH][0] = kills;
                kills_deaths[1].deaths = kills;
            } else {
                players[0].events[snapshot::event::DEATH][0] = kills;
                kills_deaths[0].deaths = kills;
            }
        }
        let tdm = (self.game_mode() == mode::TEAM_DEATHMATCH).then(|| {
            self.tdm_from.filter(|(from, _)| since(*from)).map_or(
                Tdm {
                    tickets: [100, 100],
                    alive_players_mask: rule.mask,
                    active_players_mask: rule.mask,
                },
                |(_, tdm)| tdm,
            )
        });
        let points = match tdm {
            // The client's defect: the rule has the tickets, the
            // statistics the ones of the start.
            Some(_) if self.stale_tickets_from.is_some_and(since) => [self.tickets_count(); 2],
            Some(tdm) => tdm.tickets,
            None => self
                .points_from
                .filter(|(from, _)| since(*from))
                .map_or([0, 0], |(_, points)| points),
        };
        let statistics = Statistics {
            players,
            victory_items: vec![VictoryItemStatus::default(); self.batteries()],
            tail: StatisticsTail {
                team_points: if tdm.is_some() && rule.state != state::IN_PROCESS {
                    // The rule writes them on its first event.
                    [0, 0]
                } else {
                    points
                },
                team_victory_items: [0, 0],
                winner_team: 2,
                active_players_mask: rule.mask,
            },
        }
        .encode();
        // Modes 2 and 3: each team's points are batteries in its container
        // (team 0's is container 0 here); the others lie about.
        let gather = tdm.is_none().then(|| {
            let batteries = self.batteries();
            let home = [usize::from(points[0]), usize::from(points[1])];
            let mut containers = vec![Container::default(); 2];
            let state = (0..batteries)
                .map(|battery| {
                    let container = if battery < home[0] {
                        0
                    } else if battery < home[0] + home[1] {
                        1
                    } else {
                        return Battery::Free {
                            spotted_mask: 0,
                            position: [battery as f32, 0.0, 0.0],
                        };
                    };
                    containers[container].items.push(battery as u8);
                    Battery::InContainer(container as u8)
                })
                .collect();
            Gather {
                seed: 1,
                team_points: [points[0] as u8, points[1] as u8],
                batteries: state,
                containers,
                timed: (self.game_mode() == mode::TIMED_BATTERIES).then(|| Timed {
                    available: batteries as u32,
                    started: rule.state == state::IN_PROCESS,
                    ..Timed::default()
                }),
            }
        });
        let rules = Rules {
            timelimit: Timelimit {
                start: rule.start,
                current: time,
                players_mask: rule.mask,
                state: rule.state,
            },
            // As many respawn points as the map has.
            respawn: Some(Respawn {
                times: Vec::new(),
                seed: 2,
                points: vec![
                    [0, 0];
                    maps::map_mode(self.map_id).map_or(0, |map| map.respawn_points)
                ],
            }),
            gather,
            tdm,
            kills_deaths,
            team_respawn: self
                .team_respawn_from
                .filter(|(from, _)| since(*from))
                .map_or([true, true], |(_, teams)| teams),
        }
        .to_bytes();
        let mut bullets = vec![0xB0; self.ballast];
        if let Some((_, salt)) = self.salt_from.filter(|(from, _)| since(*from)) {
            bullets.push(salt);
        }
        let objects = vec![
            0;
            self.late_objects_from
                .filter(|(from, _)| since(*from))
                .map_or(0, |(_, bytes)| bytes)
        ];
        // The player blocks: a state, and an animation state after it.
        let mut mask = 0u32;
        let mut blocks = Vec::new();
        for (place, head) in self.player_states.iter().enumerate() {
            if let Some(head) = head {
                mask |= 1 << place;
                let animation = players::animation_bytes(&[true; 9], &[time, time + 7], &[0.5]);
                let state = match self.player_tails.get(place).and_then(Option::as_ref) {
                    Some(tail) => motion::state_bytes(
                        head,
                        &[0xA5; 16],
                        &motion::Tail {
                            time_ms: time,
                            ..tail.clone()
                        },
                    ),
                    None => players::state_bytes(head, &[0xA5; 16]),
                };
                blocks.extend(players::block_bytes(&state, Some(&animation)));
            }
        }
        Sections {
            active_clients_mask: mask,
            players: &blocks,
            bullets: &bullets,
            statistics: &statistics,
            objects: &objects,
            physics: &[],
            rules: &rules,
            effects: &[],
        }
        .to_body()
        .expect("a small world")
    }

    /// A snapshot was loaded: the world is the uploader's again. But for
    /// the statistics of a game whose first world is a Team Deathmatch in
    /// play, as in the 0.26g client: the timelimit rule's "match started"
    /// event of that load has the tdm rule write the tickets of the start
    /// there.
    fn load(&mut self, time: u32, body: &[u8]) {
        self.salt_from = None;
        self.events.retain(|(at, _, _)| *at > time);
        let decoded = snapshot::decode(
            time,
            body,
            self.snapshot_players,
            self.game_mode(),
            self.batteries(),
        );
        let first = std::mem::replace(&mut self.first_world, false);
        let in_play = decoded
            .as_ref()
            .and_then(|decoded| decoded.rules.as_ref())
            .is_some_and(|rules| rules.timelimit.state == state::IN_PROCESS);
        self.stale_tickets_from =
            (first && in_play && self.game_mode() == mode::TEAM_DEATHMATCH).then_some(time);
        if let Some(read) = decoded.and_then(|decoded| decoded.rules) {
            self.base.state = read.timelimit.state;
            self.base.start = read.timelimit.start;
            self.base.mask = read.timelimit.players_mask;
            self.base_time = time;
        }
    }
}

/// Says of a datagram from the server whether it is lost on the way.
pub type Lose = Box<dyn FnMut(&[u8]) -> bool + Send>;

struct Unacked {
    body: Vec<u8>,
    seqs: Vec<u8>,
    sent_ms: u64,
}

struct Assembly {
    id: u8,
    count: Option<u8>,
    parts: BTreeMap<u8, Vec<u8>>,
}

pub struct Client {
    pub session: u32,
    pub model: ClientModel,
    pub sim: Sim,
    /// The initial world's time it uploads when it is ready.
    pub initial_ms: u32,
    // The transport's client side.
    next_seq: u8,
    /// Whether it has sent anything: nothing goes before the connect.
    /// (Not `next_seq != 0`: the seq wraps every 256 datagrams.)
    sent_any: bool,
    received: Window,
    unique_in: Window,
    assemblies: BTreeMap<u8, Assembly>,
    /// Message bodies waiting to go, each alone in its datagram; whether it
    /// is resent until acked.
    queue: VecDeque<(Vec<u8>, bool)>,
    unacked: Vec<Unacked>,
    last_sent_ms: Option<u64>,
    ack_due: bool,
    now_ms: u64,
    /// Every message taken, in order.
    pub taken: Vec<ServerMessage>,
    pub skipped: Vec<(u8, Skip)>,
    /// What a correct server never causes.
    pub faults: Vec<String>,
    /// The scripted resource loader can be held pending while transport ACKs
    /// and normal simulation continue, independently of resource readiness.
    pub auto_prepare: bool,
    pub prepares: Vec<Prepare>,
    /// Every prepare received, resends too.
    pub prepares_received: u32,
    /// The bytes a place this game installs adds to its world's game
    /// objects, from the tick of the install on (a newcomer whose kits
    /// have grenades or traps).
    pub late_objects: usize,
    /// The server disconnected it (FA or FD).
    pub disconnected: bool,
    last_words: Option<Vec<u8>>,
    /// It was told it left (0xEA with its own index): the match is closed.
    pub told_to_leave: bool,
    /// The match is closed on it (told to, or it left): no more reports.
    pub closed: bool,
    pub upload_requests: u32,
    pub uploads: u32,
    pub datagrams_received: u32,
    // Behaviour switches for the tests.
    /// Sends nothing at all (a dead client).
    pub silent: bool,
    /// Sends no hash reports.
    pub no_hashes: bool,
    /// Its world stands still since this time of its clock, with the link
    /// up and the clock running: a game whose simulation has stopped (its
    /// window is not active, `engine_world::logic_tick` 0x1A0E1; or it has
    /// died and is not closed yet). Its event-horizon world stays the one
    /// of then: nothing new to hash, and an upload is of that old tick.
    pub frozen_at: Option<u32>,
    /// It has a simulation ([`Sim`]). Without one ([`Client::idle`]) it
    /// takes the time and the world and answers the pings, and has nothing
    /// to hash or upload.
    pub simulates: bool,
    /// Asked for its world, it uploads the tick it last hashed, not its
    /// horizon tick of that moment. With the stock 100 ms interval the two
    /// are the same about every second time; this is that case every time.
    pub uploads_hashed_tick: bool,
    /// Does not answer 0xF6 (as if its pool guard skipped every request).
    pub no_uploads: bool,
    /// Its datagrams ack nothing (as a game's acks that name only its
    /// newest few seqs miss most of a burst).
    pub mute_acks: bool,
    /// Does not answer 0xE5 (as if the answer were lost).
    pub mute_time_replies: bool,
    /// Acks as the real game was seen to (live, 2026-10-13): a pure ack
    /// only after 100 ms without a send (0x3FC1B1), and every datagram
    /// names the 13 server seqs before the newest, not 34. The default
    /// here acks at once and deep, which a game does only while it sends
    /// its inputs.
    pub slow_acks: bool,
    /// Its datagrams ack only the newest this many server seqs (the game
    /// acks the newest 6 between its datagram and the server's last ack).
    pub ack_depth: Option<u32>,
    /// Datagrams from the server for which this is true are lost.
    pub lose: Option<Lose>,
    /// `-hash_check_interval_in_ms`: 50 with the full client patch (the
    /// default here), 100 in a stock client.
    pub hash_interval_ms: u32,
    /// When it last passed the hash gate, on its own clock's 50 ms grid
    /// (the client's +0x1F1C50): 0 after every 0xE5, so the first pass
    /// after one fires, and with 100 ms that pass sets its phase.
    pub hash_last_sent: u32,
    last_hashed: u32,
    next_keep_alive_ms: u64,
}

impl Client {
    pub fn new(session: u32, patched: bool, players: usize, map_id: u8) -> Self {
        Self {
            session,
            model: ClientModel::new(patched),
            sim: Sim::new(players, map_id),
            initial_ms: INITIAL_MS,
            next_seq: 0,
            sent_any: false,
            received: Window::default(),
            unique_in: Window::default(),
            assemblies: BTreeMap::new(),
            queue: VecDeque::new(),
            unacked: Vec::new(),
            last_sent_ms: None,
            ack_due: false,
            now_ms: 0,
            taken: Vec::new(),
            skipped: Vec::new(),
            faults: Vec::new(),
            auto_prepare: true,
            late_objects: 0,
            prepares: Vec::new(),
            prepares_received: 0,
            disconnected: false,
            last_words: None,
            told_to_leave: false,
            closed: false,
            upload_requests: 0,
            uploads: 0,
            datagrams_received: 0,
            silent: false,
            no_hashes: false,
            frozen_at: None,
            simulates: true,
            uploads_hashed_tick: false,
            no_uploads: false,
            mute_acks: false,
            mute_time_replies: false,
            slow_acks: false,
            ack_depth: None,
            lose: None,
            hash_interval_ms: 50,
            hash_last_sent: 0,
            last_hashed: 0,
            next_keep_alive_ms: 0,
        }
    }

    /// A client that cannot simulate the game (the test bot): a stock
    /// (unpatched) client as far as the wire goes, with no world of its
    /// own. It connects, loads, takes the time and the world, acks, answers
    /// the pings and sends its keep-alives; it sends no hash report (it
    /// would have to make one up) and no world.
    pub fn idle(session: u32) -> Self {
        Self {
            sim: Sim::none(),
            simulates: false,
            no_hashes: true,
            ..Self::new(session, false, 1, Sim::none().map_id)
        }
    }

    /// The test bot of a game that speaks the late join (patch revision 5
    /// on): `idle`, with the capability in its 0xD8 and worlds of twenty
    /// places, so a match of the growing queue takes it.
    pub fn idle_growing(session: u32) -> Self {
        let mut client = Self::idle(session);
        client.model.growing_roster = true;
        client.sim.snapshot_players = 20;
        client
    }

    pub fn growing(session: u32, players: usize, map_id: u8) -> Self {
        let mut client = Self::new(session, true, players, map_id);
        client.model.growing_roster = true;
        client.sim.snapshot_players = 20;
        client
    }

    pub fn prepared(&mut self, prepare: &Prepare, status: Status) {
        if status == Status::Installed {
            let options = self.model.options.as_mut().expect("static info first");
            assert_eq!(options.match_id, prepare.proposal.match_id);
            let place = usize::from(prepare.proposal.place);
            if place == options.players.len() {
                options.players.push(prepare.profile.clone());
                self.sim.players = options.players.len();
                self.sim.player_states.resize(self.sim.players, None);
                self.sim.player_tails.resize(self.sim.players, None);
            } else {
                assert_eq!(options.players.get(place), Some(&prepare.profile));
            }
            if self.late_objects > 0 && self.sim.late_objects_from.is_none() {
                self.sim.late_objects_from = Some((self.clock(), self.late_objects));
            }
        }
        let bytes = Prepared {
            proposal: prepare.proposal,
            status,
        }
        .encode()
        .unwrap();
        self.send(id::LEAVE, &bytes, true);
    }

    pub fn clock(&self) -> u32 {
        self.model.clock.unwrap_or(0)
    }

    /// The time of its event-horizon world, if it has one it may upload or
    /// hash: none loaded from a snapshot, none younger than the horizon.
    pub fn horizon(&self) -> Option<u32> {
        let resync = self.model.last_resync?;
        let time = self.clock().checked_sub(HORIZON_MS)? / TICK_MS * TICK_MS;
        let time = self
            .frozen_at
            .map_or(time, |at| time.min(at / TICK_MS * TICK_MS));
        (time > resync).then_some(time)
    }

    // ------------------------------------------------------------ sending

    pub fn send(&mut self, id: u8, payload: &[u8], reliable: bool) {
        let mut body = vec![id];
        body.extend_from_slice(payload);
        self.queue.push_back((body, reliable));
    }

    /// 0xD8: the first message.
    pub fn connect(&mut self) {
        let mut payload = self.session.to_le_bytes().to_vec();
        payload.push(if self.model.growing_roster {
            CONNECT_CAPABILITY
        } else {
            0
        });
        self.send(id::CONNECT, &payload, true);
    }

    /// The level is loaded: a patched client uploads its initial world,
    /// then every client sends 0xDA.
    pub fn ready(&mut self) {
        if self.model.patched {
            let body = self.sim.world(self.initial_ms);
            self.upload(self.initial_ms, &body);
        }
        assert!(self.model.send_ready(), "ready without static info");
        self.send(id::READY, &[], true);
    }

    /// Its world as 0xE0 chunks.
    pub fn upload(&mut self, item_time: u32, body: &[u8]) {
        self.uploads += 1;
        for chunk in upload::split(item_time, body).expect("a world that fits") {
            self.send(id::LEAVE, &chunk, true);
        }
    }

    /// An input at its own clock's tick (or `ahead_ms` from it).
    pub fn input(&mut self, ahead_ms: i32, actions: u32) -> PlayerInput {
        let time = self.clock().saturating_add_signed(ahead_ms);
        let input = PlayerInput {
            time: WireTime::tick(time),
            rotation: [0x00, 0x3C, 0x00, 0xC0],
            actions,
        };
        self.send_input(input);
        input
    }

    pub fn send_input(&mut self, input: PlayerInput) {
        let mut payload = vec![input.time.tick];
        payload.extend(input.time.ms);
        payload.extend(input.rotation);
        payload.extend(input.actions.to_le_bytes());
        let id = if input.time.ms.is_some() {
            id::INPUT_PRECISE
        } else {
            id::INPUT
        };
        // The client resends an input for 500 ms.
        self.send(id, &payload, true);
    }

    pub fn profile_input(&mut self, value: u8) -> u8 {
        let tick = tick_byte(self.clock());
        self.send(id::PROFILE_INPUT, &[tick, value], true);
        tick
    }

    pub fn report(&mut self, time: u32, crc: u32) {
        let mut payload = vec![tick_byte(time)];
        payload.extend(crc.to_le_bytes());
        self.send(id::HASH, &payload, false);
    }

    /// 0xE0, empty: the match is closed on it, and it reports no more.
    /// The player left by its own will ("disconnected_by_initiator"), and
    /// has given its place up.
    pub fn leave(&mut self) {
        self.send(id::LEAVE, &[], true);
        self.closed = true;
    }

    /// Its link is closed with no 0xE0: the server sees the player go at
    /// once, as with `leave`, and keeps its place for it, as for a game
    /// whose link broke or that died (which the server sees by a timeout).
    pub fn drop_link(&mut self) {
        self.last_words = Some(self.datagram(&[low_level::INSTANT_DISCONNECT]));
        self.disconnected = true;
        self.closed = true;
    }

    fn datagram(&mut self, body: &[u8]) -> Vec<u8> {
        let (rseq, bits, tail) = match self.received.newest().filter(|_| !self.mute_acks) {
            Some(newest) => match self.ack_depth {
                Some(depth) => (
                    newest,
                    self.received.bits() & (u128::MAX << (128 - depth.clamp(1, 7))),
                    0,
                ),
                None => (
                    newest,
                    self.received.bits(),
                    if self.slow_acks { 1 } else { 4 },
                ),
            },
            None => (0xFF, 0, 0),
        };
        let (ack0, tail_bytes) = ack::encode(bits, tail, false);
        let mut datagram = vec![self.next_seq, rseq, ack0];
        self.next_seq = self.next_seq.wrapping_add(1);
        self.sent_any = true;
        datagram.extend_from_slice(body);
        datagram.extend(tail_bytes);
        datagram
    }

    /// Time passes to `now_ms`: the simulation's reports, then the
    /// datagrams to send.
    pub fn poll(&mut self, now_ms: u64) -> Vec<Vec<u8>> {
        let passed = now_ms.saturating_sub(self.now_ms);
        self.now_ms = now_ms;
        self.model.advance(passed as u32);
        if self.silent {
            return Vec::new();
        }
        if self.disconnected {
            return self.last_words.take().into_iter().collect();
        }
        if self.model.is_synced() && !self.closed {
            // The client's gate (0x27CD52, match.md section 14): on the
            // 50 ms grid of its own clock, `last_sent + interval <= now`,
            // and the timer is armed again even with nothing to report.
            // With 100 ms it reports every second tick, on its own phase.
            let grid = self.clock() / TICK_MS * TICK_MS;
            if self.hash_last_sent + self.hash_interval_ms <= grid {
                self.hash_last_sent = grid;
                if let Some(time) = self.horizon().filter(|time| *time > self.last_hashed)
                    && !self.no_hashes
                    && self.simulates
                {
                    self.last_hashed = time;
                    let crc = snapshot::crc(&self.sim.world(time));
                    self.report(time, crc);
                }
            }
            if now_ms >= self.next_keep_alive_ms {
                self.next_keep_alive_ms = now_ms + 1000;
                self.send(id::KEEP_ALIVE, &[], false);
            }
        }
        let mut datagrams = Vec::new();
        // Resends first, every 50 ms.
        for index in 0..self.unacked.len() {
            if now_ms.saturating_sub(self.unacked[index].sent_ms) >= 50 && datagrams.len() < 40 {
                let body = self.unacked[index].body.clone();
                let seq = self.next_seq;
                datagrams.push(self.datagram(&body));
                let unacked = &mut self.unacked[index];
                unacked.seqs.push(seq);
                unacked.sent_ms = now_ms;
            }
        }
        while datagrams.len() < 40 {
            let Some((body, reliable)) = self.queue.pop_front() else {
                break;
            };
            let seq = self.next_seq;
            datagrams.push(self.datagram(&body));
            if reliable {
                self.unacked.push(Unacked {
                    body,
                    seqs: vec![seq],
                    sent_ms: now_ms,
                });
            }
        }
        let quiet = self
            .last_sent_ms
            .is_none_or(|sent| now_ms.saturating_sub(sent) >= 100);
        let ack_now = self.ack_due && !self.slow_acks;
        if datagrams.is_empty() && self.sent_any && (ack_now || quiet) {
            datagrams.push(self.datagram(&[low_level::CONTINUOUS_FLOW]));
        }
        if !datagrams.is_empty() {
            self.last_sent_ms = Some(now_ms);
            self.ack_due = false;
        }
        datagrams
    }

    // ---------------------------------------------------------- receiving

    pub fn receive(&mut self, datagram: &[u8]) {
        if self.silent || self.disconnected {
            return;
        }
        if let Some(lose) = &mut self.lose
            && lose(datagram)
        {
            return;
        }
        self.datagrams_received += 1;
        let mask_bytes = udp_transport::table::mask_bytes(if self.model.growing_roster {
            20
        } else {
            self.model.roster()
        });
        let parsed = match wire::parse(Table::Match, Direction::ToClient, datagram, mask_bytes) {
            Ok(parsed) => parsed,
            Err(error) => {
                self.faults
                    .push(format!("a datagram did not parse: {error:?}"));
                return;
            }
        };
        // The server's acks.
        let acked = |seq: u8| {
            let back = u32::from(parsed.rseq.wrapping_sub(seq));
            back < parsed.acks.count && parsed.acks.bits >> (127 - back) & 1 != 0
        };
        self.unacked
            .retain(|unacked| !unacked.seqs.iter().any(|seq| acked(*seq)));
        self.ack_due = true;
        if self.received.record(parsed.seq) != Recorded::New {
            return;
        }
        let mut delivered = Vec::new();
        for message in &parsed.messages {
            if message.low_level {
                match message.id {
                    low_level::INITIATE_DISCONNECTION => {
                        // The confirm still goes, then nothing more.
                        self.disconnected = true;
                        let confirm = self.datagram(&[low_level::CONFIRM_DISCONNECTION]);
                        self.last_words = Some(confirm);
                    }
                    low_level::INSTANT_DISCONNECT => self.disconnected = true,
                    _ => {}
                }
                continue;
            }
            match (message.class, message.x, message.part) {
                (Class::Unique, Some(x), None) => {
                    if self.unique_in.record(x) != Recorded::Duplicate {
                        delivered.push((message.id, message.payload.to_vec()));
                    }
                }
                (Class::Unique, Some(x), Some(part)) => {
                    if self.unique_in.contains(x) {
                        continue;
                    }
                    let assembly = self.assemblies.entry(x).or_insert_with(|| Assembly {
                        id: message.id,
                        count: None,
                        parts: BTreeMap::new(),
                    });
                    if assembly.id != message.id {
                        self.faults
                            .push(format!("unique id {x} names two messages"));
                        continue;
                    }
                    if part.index == 0 {
                        assembly.count = part.count;
                    }
                    assembly
                        .parts
                        .entry(part.index)
                        .or_insert_with(|| message.payload.to_vec());
                    if assembly
                        .count
                        .is_some_and(|count| assembly.parts.len() >= usize::from(count))
                    {
                        let whole = assembly.parts.values().flatten().copied().collect();
                        self.assemblies.remove(&x);
                        self.unique_in.record(x);
                        delivered.push((message.id, whole));
                    }
                }
                _ => delivered.push((message.id, message.payload.to_vec())),
            }
        }
        for (id, payload) in delivered {
            self.deliver(id, &payload);
        }
    }

    fn deliver(&mut self, id: u8, payload: &[u8]) {
        if self.model.growing_roster && id == id::STATIC_INFO && payload.first() == Some(&0xff) {
            match Prepare::decode(payload, Dictionary::shared()) {
                Ok(prepare) => {
                    self.prepares_received += 1;
                    if self.auto_prepare {
                        self.prepared(&prepare, Status::Installed);
                    }
                    if !self.prepares.iter().any(|p| p.proposal == prepare.proposal) {
                        self.prepares.push(prepare);
                    }
                }
                Err(error) => self.faults.push(format!("prepare: {error:?}")),
            }
            return;
        }
        let taken = match self.model.receive(id, payload, Dictionary::shared()) {
            Ok(Taken::Applied(applied)) => applied,
            Ok(Taken::Skipped(skip)) => {
                self.skipped.push((id, skip));
                // 0xEA with the local index closes the match whatever
                // `leave_match` made of its tick (0x27C720..0x27C76F: the
                // result goes to the statistics, then the index is
                // compared and `close_current_match(3)` called). PROVEN.
                if id == id::PLAYER_LEFT
                    && skip != Skip::NoCore
                    && payload.get(1).copied() == self.model.local_index
                {
                    self.told_to_leave = true;
                    self.closed = true;
                }
                return;
            }
            Err(error) => {
                self.faults.push(format!("message {id:#04x}: {error}"));
                return;
            }
        };
        match &taken.message {
            ServerMessage::Ports { min, max } => {
                // One probe for each port of the range.
                for _ in *min..=*max {
                    self.send(id::PORT_PROBE, &self.session.to_le_bytes(), true);
                }
            }
            ServerMessage::SetTime { .. } | ServerMessage::Ping { .. } => {
                if matches!(taken.message, ServerMessage::SetTime { .. }) {
                    // The 0xE5 handler clears the hash timer (0x27B74F).
                    self.hash_last_sent = 0;
                }
                let muted = self.mute_time_replies
                    && matches!(taken.message, ServerMessage::SetTime { .. });
                if let Some(reply) = taken.reply.filter(|_| !muted) {
                    let mut payload = reply.first.to_le_bytes().to_vec();
                    payload.extend(reply.stamp.to_le_bytes());
                    payload.extend(reply.client_ms.to_le_bytes());
                    self.send(id::TIME_REPLY, &payload, false);
                }
            }
            ServerMessage::Snapshot { time, body, .. } => {
                if self.simulates {
                    self.sim.load(*time, body);
                }
                self.last_hashed = *time;
            }
            ServerMessage::PlayerEntered { player, .. } => {
                if let Some(at) = taken.time
                    && self.simulates
                {
                    self.sim.events.push((at, *player, true));
                }
            }
            ServerMessage::PlayerLeft { player, .. } => {
                if Some(*player) == self.model.local_index {
                    self.told_to_leave = true;
                    self.closed = true;
                } else if let Some(at) = taken.time
                    && self.simulates
                {
                    self.sim.events.push((at, *player, false));
                }
            }
            ServerMessage::UploadRequest => {
                self.upload_requests += 1;
                // Silently not answered without an event-horizon world.
                if let Some(horizon) = self.horizon().filter(|_| !self.no_uploads) {
                    let time = if self.uploads_hashed_tick {
                        self.last_hashed
                    } else {
                        horizon
                    };
                    let body = self.sim.world(time);
                    self.upload(time, &body);
                }
            }
            _ => {}
        }
        self.taken.push(taken.message);
    }

    // ------------------------------------------------------------ looking

    /// The messages taken that `pick` names.
    pub fn got<T>(&self, pick: impl Fn(&ServerMessage) -> Option<T>) -> Vec<T> {
        self.taken.iter().filter_map(pick).collect()
    }

    /// The distinct inputs of `player` relayed to it, in order (a relay
    /// not yet acked comes again: the client's world takes the copy as the
    /// same input).
    pub fn relays_of(&self, player: u8) -> Vec<PlayerInput> {
        let mut relays = Vec::new();
        for message in &self.taken {
            if let ServerMessage::InputRelay {
                player: from,
                input,
            } = message
                && *from == player
                && !relays.contains(input)
            {
                relays.push(*input);
            }
        }
        relays
    }

    pub fn snapshots(&self) -> Vec<(u8, u32)> {
        self.got(|message| match message {
            ServerMessage::Snapshot {
                generation, time, ..
            } => Some((*generation, *time)),
            _ => None,
        })
    }

    pub fn entered(&self) -> Vec<u8> {
        let mut players = self.got(|message| match message {
            ServerMessage::PlayerEntered { player, .. } => Some(*player),
            _ => None,
        });
        players.dedup();
        players
    }

    pub fn left(&self) -> Vec<u8> {
        let mut players = self.got(|message| match message {
            ServerMessage::PlayerLeft { player, .. } => Some(*player),
            _ => None,
        });
        players.dedup();
        players
    }
}
