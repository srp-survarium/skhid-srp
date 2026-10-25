//! The voice relay's rules, on a table of seats and a clock the test moves;
//! one test over real sockets and the match registry.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use super::*;
use crate::v026g::registry::Seat;

const MATCH: u32 = 0x10;

/// Sessions 1..=4 in match 0x10: 1, 2 and 3 on team 0 (places 0, 2, 4), 4
/// on team 1 (place 1). Session 9 is in another match, on team 0.
#[derive(Clone, Default)]
struct Table(Rc<RefCell<HashMap<u32, VoiceSeat>>>);

impl Table {
    fn new() -> Self {
        let table = Self::default();
        for (session, place, team) in [(1, 0, 0), (2, 2, 0), (3, 4, 0), (4, 1, 1)] {
            table.seat(session, MATCH, place, team);
        }
        table.seat(9, 0x20, 0, 0);
        table
    }

    fn seat(&self, session: u32, match_id: u32, place: u8, team: u8) {
        self.0.borrow_mut().insert(
            session,
            VoiceSeat {
                match_id,
                place,
                team,
                ip: Some(addr(session).ip()),
            },
        );
    }

    fn unseat(&self, session: u32) {
        self.0.borrow_mut().remove(&session);
    }
}

impl Seats for Table {
    fn voice_seat(&self, session_id: u32) -> Option<VoiceSeat> {
        self.0.borrow().get(&session_id).copied()
    }

    fn is_open(&self, match_id: u32) -> bool {
        self.0
            .borrow()
            .values()
            .any(|seat| seat.match_id == match_id)
    }

    /// Each seat's account is 100 + its session.
    fn roster(&self, match_id: u32, team: u8) -> Vec<(u8, u32)> {
        let mut places: Vec<(u8, u32)> = self
            .0
            .borrow()
            .iter()
            .filter(|(_, seat)| seat.match_id == match_id && seat.team == team)
            .map(|(session, seat)| (seat.place, 100 + session))
            .collect();
        places.sort_unstable();
        places
    }
}

/// Each session's game: 10.0.0.<session>, port 5000 + session.
fn addr(session: u32) -> SocketAddr {
    SocketAddr::new(ip(&format!("10.0.0.{session}")), 5000 + session as u16)
}

fn hello(session: u32, hears: bool) -> Vec<u8> {
    hello_muting(session, hears, 0)
}

fn hello_muting(session: u32, hears: bool, muted: u32) -> Vec<u8> {
    let mut datagram = vec![MAGIC, VERSION, kind::HELLO];
    datagram.extend_from_slice(&session.to_le_bytes());
    datagram.push(u8::from(hears));
    datagram.extend_from_slice(&muted.to_le_bytes());
    datagram
}

fn start(session: u32, utt: u16) -> Vec<u8> {
    let mut datagram = vec![MAGIC, VERSION, kind::START];
    datagram.extend_from_slice(&session.to_le_bytes());
    datagram.extend_from_slice(&utt.to_le_bytes());
    datagram.extend_from_slice(&1234u32.to_le_bytes());
    datagram
}

fn audio(session: u32, utt: u16, seq: u16) -> Vec<u8> {
    let mut datagram = vec![MAGIC, VERSION, kind::AUDIO];
    datagram.extend_from_slice(&session.to_le_bytes());
    datagram.extend_from_slice(&utt.to_le_bytes());
    datagram.extend_from_slice(&seq.to_le_bytes());
    datagram.push(2);
    for frame in [[0xA1u8; 3].as_slice(), [0xB2u8; 5].as_slice()] {
        datagram.push(frame.len() as u8);
        datagram.extend_from_slice(frame);
    }
    datagram
}

fn end(session: u32, utt: u16) -> Vec<u8> {
    let mut datagram = vec![MAGIC, VERSION, kind::END];
    datagram.extend_from_slice(&session.to_le_bytes());
    datagram.extend_from_slice(&utt.to_le_bytes());
    datagram.extend_from_slice(&7u16.to_le_bytes());
    datagram
}

struct Bench {
    table: Table,
    relay: Relay<Table>,
    start: Instant,
    now: Instant,
}

impl Bench {
    fn new() -> Self {
        Self::with_tap(Box::new(NoTap))
    }

    fn with_tap(tap: Box<dyn UtteranceTap>) -> Self {
        let table = Table::new();
        let now = Instant::now();
        Self {
            relay: Relay::with_tap(table.clone(), tap, now),
            table,
            start: now,
            now,
        }
    }

    fn at(&mut self, ms: u64) {
        self.now = self.start + Duration::from_millis(ms);
    }

    fn send(&mut self, session: u32, datagram: &[u8]) -> Vec<(SocketAddr, Vec<u8>)> {
        self.send_from(addr(session), datagram)
    }

    fn send_from(&mut self, from: SocketAddr, datagram: &[u8]) -> Vec<(SocketAddr, Vec<u8>)> {
        self.relay.receive(from, datagram, self.now)
    }

    fn poll(&mut self) -> Vec<(SocketAddr, Vec<u8>)> {
        self.relay.poll(self.now)
    }

    /// Everyone says HELLO (and hears); the WELCOMEs are dropped.
    fn all_hello(&mut self) {
        for session in [1, 2, 3, 4, 9] {
            self.send(session, &hello(session, true));
        }
    }
}

/// Who got what: the sessions (by address) and the datagrams' types.
fn sent_to(out: &[(SocketAddr, Vec<u8>)]) -> Vec<(u32, u8)> {
    let mut to: Vec<(u32, u8)> = out
        .iter()
        .map(|(to, datagram)| (u32::from(to.port()) - 5000, datagram[2]))
        .collect();
    to.sort_unstable();
    to
}

/// The WELCOMEs of `out` (the ROSTERs left out).
fn welcomes(out: Vec<(SocketAddr, Vec<u8>)>) -> Vec<(SocketAddr, Vec<u8>)> {
    out.into_iter()
        .filter(|(_, datagram)| datagram[2] != kind::ROSTER)
        .collect()
}

#[test]
fn hello_is_welcomed_with_the_slot() {
    let mut bench = Bench::new();
    let out = bench.send(2, &hello(2, true));
    assert_eq!(
        welcomes(out),
        vec![(addr(2), vec![MAGIC, VERSION, kind::WELCOME, 2])]
    );
    // The old HELLO without a mask is taken too.
    let mut short = hello(3, true);
    short.truncate(8);
    assert_eq!(
        welcomes(bench.send(3, &short)),
        vec![(addr(3), vec![MAGIC, VERSION, kind::WELCOME, 4])]
    );
}

#[test]
fn each_welcome_comes_with_the_teams_voice_ids() {
    let key = [9; 32];
    let mut relay = Relay::new(Table::new(), Instant::now()).with_id_key(key);
    let out = relay.receive(addr(2), &hello(2, true), Instant::now());
    assert_eq!(out.len(), 2);
    // Team 0: places 0, 2 and 4 (sessions 1, 2, 3: accounts 101, 102, 103).
    let mut want = vec![MAGIC, VERSION, kind::ROSTER, 3];
    for (place, account) in [(0u8, 101u32), (2, 102), (4, 103)] {
        want.push(place);
        want.extend_from_slice(&voice_id(&key, account).to_le_bytes());
    }
    assert_eq!(out[1], (addr(2), want));
    // Stable for an account; another for another account or another key; never 0.
    assert_eq!(voice_id(&key, 101), voice_id(&key, 101));
    assert_ne!(voice_id(&key, 101), voice_id(&key, 102));
    assert_ne!(voice_id(&key, 101), voice_id(&[8; 32], 101));
    assert!((0..10_000).all(|account| voice_id(&key, account) != 0));
}

#[test]
fn talk_reaches_the_team_only_and_not_the_speaker() {
    let mut bench = Bench::new();
    bench.all_hello();
    let out = bench.send(1, &start(1, 7));
    assert_eq!(sent_to(&out), vec![(2, 0x82), (3, 0x82)]);
    // The relayed START: the slot, the utt, the server's ms.
    assert_eq!(out[0].1[..6], [MAGIC, VERSION, 0x82, 0, 7, 0]);
    let out = bench.send(1, &audio(1, 7, 0));
    assert_eq!(sent_to(&out), vec![(2, 0x83), (3, 0x83)]);
    // The body is the talker's from the utt on, behind the slot.
    let sent = &audio(1, 7, 0);
    let mut relayed = vec![MAGIC, VERSION, 0x83, 0];
    relayed.extend_from_slice(&sent[7..]);
    assert_eq!(out[0].1, relayed);
    let out = bench.send(1, &end(1, 7));
    assert_eq!(sent_to(&out), vec![(2, 0x84), (3, 0x84)]);
    assert_eq!(out[0].1, vec![MAGIC, VERSION, 0x84, 0, 7, 0, 0]);
    // The END's repeats change nothing.
    assert!(bench.send(1, &end(1, 7)).is_empty());
    assert!(bench.send(1, &end(1, 7)).is_empty());
}

#[test]
fn a_listener_that_does_not_hear_or_is_stale_gets_nothing() {
    let mut bench = Bench::new();
    bench.send(1, &hello(1, true));
    bench.send(2, &hello(2, false));
    bench.send(3, &hello(3, true));
    assert_eq!(sent_to(&bench.send(1, &start(1, 1))), vec![(3, 0x82)]);
    // Session 3's HELLO is 7 s old: no more.
    bench.at(7_000);
    bench.send(1, &hello(1, true));
    assert!(bench.send(1, &start(1, 2)).is_empty());
}

#[test]
fn a_muted_speaker_is_not_sent_to_the_listener_that_mutes_it() {
    let mut bench = Bench::new();
    bench.all_hello();
    // Session 2 mutes slot 0 (session 1), session 3 mutes slot 2.
    bench.send(2, &hello_muting(2, true, 1 << 0));
    bench.send(3, &hello_muting(3, true, 1 << 2));
    assert_eq!(sent_to(&bench.send(1, &start(1, 1))), vec![(3, 0x82)]);
    assert_eq!(sent_to(&bench.send(1, &audio(1, 1, 0))), vec![(3, 0x83)]);
    assert_eq!(sent_to(&bench.send(2, &start(2, 1))), vec![(1, 0x82)]);
    // Unmuted again by the next HELLO.
    bench.send(2, &hello(2, true));
    assert_eq!(
        sent_to(&bench.send(1, &audio(1, 1, 1))),
        vec![(2, 0x83), (3, 0x83)]
    );
}

#[test]
fn a_datagram_from_another_ip_or_without_a_seat_is_dropped() {
    let mut bench = Bench::new();
    bench.all_hello();
    // Session 1's own IP on another port (a NAT): fine.
    let other_port = SocketAddr::new(addr(1).ip(), 6000);
    assert_eq!(sent_to(&bench.send_from(other_port, &start(1, 1))).len(), 2);
    // Another IP with session 1's number: dropped.
    let spoofed = SocketAddr::new(ip("10.9.9.9"), 5001);
    assert!(bench.send_from(spoofed, &audio(1, 1, 0)).is_empty());
    assert!(bench.send_from(spoofed, &hello(1, true)).is_empty());
    assert_eq!(bench.relay.counts(MATCH).map(|c| c.dropped), Some(2));
    // A session with no seat.
    assert!(bench.send(7, &hello(7, true)).is_empty());
    assert!(bench.send(7, &start(7, 1)).is_empty());
    // A seat whose connection's IP is not known yet.
    bench.table.0.borrow_mut().get_mut(&3).unwrap().ip = None;
    assert!(bench.send(3, &start(3, 1)).is_empty());
}

#[test]
fn malformed_and_large_datagrams_are_dropped() {
    let mut bench = Bench::new();
    bench.all_hello();
    let mut bad = audio(1, 1, 0);
    bad.push(0); // a byte past the frames
    assert!(bench.send(1, &bad).is_empty());
    let mut bad = audio(1, 1, 0);
    bad.truncate(bad.len() - 1); // a frame cut short
    assert!(bench.send(1, &bad).is_empty());
    let mut bad = audio(1, 1, 0);
    bad[1] = 2; // another version
    assert!(bench.send(1, &bad).is_empty());
    let mut bad = audio(1, 1, 0);
    bad[0] = 0; // not the magic
    assert!(bench.send(1, &bad).is_empty());
    // A server type from a client, an unknown type.
    let mut bad = start(1, 1);
    bad[2] = 0x82;
    assert!(bench.send(1, &bad).is_empty());
    bad[2] = 0x40;
    assert!(bench.send(1, &bad).is_empty());
    // No frames.
    let mut none = vec![MAGIC, VERSION, kind::AUDIO];
    none.extend_from_slice(&1u32.to_le_bytes());
    none.extend_from_slice(&[1, 0, 0, 0, 0]);
    assert!(bench.send(1, &none).is_empty());
    // Too large: 401 bytes; 400 is taken.
    let large = |total: usize| {
        let mut datagram = vec![MAGIC, VERSION, kind::AUDIO];
        datagram.extend_from_slice(&1u32.to_le_bytes());
        datagram.extend_from_slice(&[1, 0, 0, 0, 2]);
        datagram.push(255);
        datagram.extend_from_slice(&[1; 255]);
        let rest = total - datagram.len() - 1;
        datagram.push(rest as u8);
        datagram.extend(std::iter::repeat_n(1, rest));
        datagram
    };
    assert_eq!(large(401).len(), 401);
    assert!(bench.send(1, &large(401)).is_empty());
    assert_eq!(
        sent_to(&bench.send(1, &large(400))),
        vec![(2, 0x82), (2, 0x83), (3, 0x82), (3, 0x83)]
    );
    assert!(bench.relay.dropped_unknown >= 8);
}

#[test]
fn audio_without_its_start_opens_the_utterance() {
    let mut bench = Bench::new();
    bench.all_hello();
    let out = bench.send(1, &audio(1, 5, 3));
    assert_eq!(
        out.iter()
            .map(|(_, datagram)| datagram[2])
            .collect::<Vec<_>>(),
        vec![0x82, 0x82, 0x83, 0x83]
    );
    // Its START comes late: nothing more.
    assert!(bench.send(1, &start(1, 5)).is_empty());
}

#[test]
fn silence_closes_an_utterance_and_its_late_audio_reopens_nothing() {
    let mut bench = Bench::new();
    bench.all_hello();
    bench.send(1, &start(1, 1));
    bench.at(500);
    bench.send(1, &audio(1, 1, 0));
    bench.at(1_400);
    assert!(bench.poll().is_empty());
    bench.at(1_500);
    let out = bench.poll();
    assert_eq!(sent_to(&out), vec![(2, 0x84), (3, 0x84)]);
    assert_eq!(out[0].1[6], EndReason::Timeout as u8);
    assert!(bench.send(1, &audio(1, 1, 1)).is_empty());
    // The next utterance opens.
    assert_eq!(sent_to(&bench.send(1, &start(1, 2))).len(), 2);
}

#[test]
fn an_utterance_is_cut_at_thirty_seconds() {
    let mut bench = Bench::new();
    bench.send(2, &hello(2, true));
    bench.send(1, &start(1, 1));
    let mut ms = 0;
    let mut seq = 0;
    let mut ended = None;
    while ms <= 31_000 {
        bench.at(ms);
        if ms % 2_000 == 0 {
            // HELLOs keep the listener (and the talker under the rate).
            bench.send(2, &hello(2, true));
        }
        let mut out = bench.send(1, &audio(1, 1, seq));
        out.extend(bench.poll());
        if let Some((_, datagram)) = out.iter().find(|(_, datagram)| datagram[2] == 0x84) {
            ended = Some((ms, datagram[6]));
            break;
        }
        seq += 1;
        ms += 40;
    }
    assert_eq!(ended, Some((30_000, EndReason::TooLong as u8)));
    // Its audio after the cut opens nothing; a new START does.
    bench.at(30_040);
    assert!(bench.send(1, &audio(1, 1, seq + 1)).is_empty());
    assert_eq!(sent_to(&bench.send(1, &start(1, 2))), vec![(2, 0x82)]);
    let counts = bench.relay.counts(MATCH).unwrap();
    assert_eq!(counts.utterances, 2);
    assert!(counts.talked >= Duration::from_secs(29));
}

#[test]
fn a_new_start_ends_the_open_utterance() {
    let mut bench = Bench::new();
    bench.all_hello();
    bench.send(1, &start(1, 1));
    let out = bench.send(1, &start(1, 2));
    assert_eq!(
        out.iter()
            .map(|(_, datagram)| datagram[2])
            .collect::<Vec<_>>(),
        vec![0x84, 0x84, 0x82, 0x82]
    );
}

#[test]
fn a_talker_over_sixty_datagrams_a_second_is_cut_off() {
    let mut bench = Bench::new();
    bench.send(2, &hello(2, true));
    bench.send(1, &start(1, 1));
    let mut relayed = 0;
    for seq in 0..80 {
        relayed += bench.send(1, &audio(1, 1, seq)).len();
    }
    // START was the first of 60 this second.
    assert_eq!(relayed, 59);
    assert_eq!(bench.relay.counts(MATCH).unwrap().dropped, 21);
    bench.at(1_000);
    assert_eq!(bench.send(1, &audio(1, 1, 80)).len(), 1);
}

#[test]
fn a_released_seat_neither_talks_nor_hears() {
    let mut bench = Bench::new();
    bench.all_hello();
    bench.send(1, &start(1, 1));
    bench.table.unseat(2);
    bench.at(1_000);
    // Housekeeping drops listener 2; talker 1's open utterance is not
    // touched (its seat holds) but times out.
    let out = bench.poll();
    assert_eq!(sent_to(&out), vec![(3, 0x84)]);
    bench.send(1, &start(1, 2));
    bench.table.unseat(1);
    assert!(bench.send(1, &audio(1, 2, 0)).is_empty());
    bench.at(2_000);
    // The talker without a seat: its utterance ends.
    assert_eq!(sent_to(&bench.poll()), vec![(3, 0x84)]);
}

#[test]
fn a_match_over_is_logged_and_forgotten() {
    let mut bench = Bench::new();
    bench.all_hello();
    bench.send(1, &start(1, 1));
    bench.send(1, &end(1, 1));
    assert!(bench.relay.counts(MATCH).is_some());
    for session in [1, 2, 3, 4] {
        bench.table.unseat(session);
    }
    bench.at(1_000);
    bench.poll();
    assert!(bench.relay.counts(MATCH).is_none());
}

/// A tap that writes down what it is told and answers each START with a
/// line of text.
struct Recorder(Arc<Mutex<Vec<String>>>);

impl UtteranceTap for Recorder {
    fn started(&mut self, utterance: &Utterance, out: &mut RelayOut) {
        self.0
            .lock()
            .unwrap()
            .push(format!("start {}", utterance.utt));
        out.text(utterance, *b"en", false, "héllo");
    }

    fn audio(&mut self, utterance: &Utterance, seq: u16, frames: &[&[u8]], _: &mut RelayOut) {
        let lens: Vec<usize> = frames.iter().map(|frame| frame.len()).collect();
        self.0
            .lock()
            .unwrap()
            .push(format!("audio {} {seq} {lens:?}", utterance.utt));
    }

    fn ended(&mut self, utterance: &Utterance, reason: EndReason, out: &mut RelayOut) {
        self.0
            .lock()
            .unwrap()
            .push(format!("end {} {reason:?}", utterance.utt));
        out.text(utterance, *b"en", true, &"x".repeat(300));
    }
}

#[test]
fn the_tap_sees_every_utterance_and_its_text_goes_to_the_team() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mut bench = Bench::with_tap(Box::new(Recorder(seen.clone())));
    bench.all_hello();
    let out = bench.send(1, &start(1, 3));
    // START to 2 and 3, the text to 2, 3 and the speaker.
    assert_eq!(
        sent_to(&out),
        vec![(1, 0x85), (2, 0x82), (2, 0x85), (3, 0x82), (3, 0x85)]
    );
    let text = &out
        .iter()
        .find(|(_, datagram)| datagram[2] == 0x85)
        .unwrap()
        .1;
    assert_eq!(text[3..9], [0, 3, 0, 0, b'e', b'n']);
    assert_eq!(text[9] as usize, "héllo".len());
    assert_eq!(&text[10..], "héllo".as_bytes());
    bench.send(1, &audio(1, 3, 0));
    let out = bench.send(1, &end(1, 3));
    let text = &out
        .iter()
        .find(|(_, datagram)| datagram[2] == 0x85)
        .unwrap()
        .1;
    assert_eq!(text[6], 1);
    assert_eq!(text[9], 255);
    assert_eq!(text.len(), 10 + 255);
    assert_eq!(
        *seen.lock().unwrap(),
        vec!["start 3", "audio 3 0 [3, 5]", "end 3 Released"]
    );
}

#[test]
fn the_relay_over_sockets_with_the_registry() {
    use super::super::tests::setup;
    let registry = Arc::new(MatchRegistry::new("127.0.0.1", 1));
    let ticket = registry
        .create(setup(4, super::super::tests::MAP_TDM))
        .unwrap();
    let session = super::super::tests::session_of;
    // Places 0 and 2 on team 0, 1 and 3 on team 1; 0, 1 and 2 connected.
    for place in 0..3u8 {
        registry.set_connection(
            Seat {
                match_id: ticket.match_id,
                place,
            },
            ip("127.0.0.1"),
        );
    }
    assert_eq!(registry.voice_seat(session(3)).unwrap().ip, None);
    let server = UdpSocket::bind("127.0.0.1:0").unwrap();
    let server_addr = server.local_addr().unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let thread = {
        let (registry, stop) = (registry.clone(), stop.clone());
        std::thread::spawn(move || run(&server, registry, [7; 32], &stop))
    };
    let games: Vec<UdpSocket> = (0..3)
        .map(|_| {
            let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
            socket
                .set_read_timeout(Some(Duration::from_millis(500)))
                .unwrap();
            socket
        })
        .collect();
    let receive = |socket: &UdpSocket| {
        let mut buffer = [0; 512];
        socket
            .recv(&mut buffer)
            .ok()
            .map(|len| buffer[..len].to_vec())
    };
    for (place, game) in games.iter().enumerate() {
        game.send_to(&hello(session(place), true), server_addr)
            .unwrap();
        assert_eq!(
            receive(game),
            Some(vec![MAGIC, VERSION, kind::WELCOME, place as u8])
        );
        assert_eq!(
            receive(game).map(|datagram| datagram[2]),
            Some(kind::ROSTER)
        );
    }
    games[0]
        .send_to(&start(session(0), 9), server_addr)
        .unwrap();
    games[0]
        .send_to(&audio(session(0), 9, 0), server_addr)
        .unwrap();
    assert_eq!(receive(&games[2]).map(|datagram| datagram[2]), Some(0x82));
    assert_eq!(receive(&games[2]).map(|datagram| datagram[2]), Some(0x83));
    // The other team hears nothing.
    games[1]
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    assert_eq!(receive(&games[1]), None);
    stop.store(true, Ordering::Relaxed);
    thread.join().unwrap().unwrap();
    // The match over: no seat, no voice.
    registry.end(ticket.match_id);
    assert_eq!(registry.voice_seat(session(0)), None);
}

#[test]
fn the_voice_port_is_the_next_one() {
    assert_eq!(port_for(2236), Some(2237));
    assert_eq!(port_for(u16::MAX), None);
}

#[test]
fn a_game_connected_to_its_match_is_a_voice_seat_from_its_ip() {
    use super::super::tests::{Bench, MAP_BATTERIES, session_of};
    let mut bench = Bench::new(2, MAP_BATTERIES, crate::v026g::Settings::default());
    assert_eq!(bench.registry.voice_seat(session_of(0)).unwrap().ip, None);
    bench.join(0);
    let seat = bench.registry.voice_seat(session_of(0)).unwrap();
    assert_eq!(
        (seat.match_id, seat.place, seat.team, seat.ip),
        (bench.match_id, 0, 0, Some(ip("127.0.0.1")))
    );
    assert_eq!(bench.registry.voice_seat(session_of(1)).unwrap().ip, None);
    assert_eq!(bench.registry.voice_seat(session_of(1)).unwrap().team, 1);
}
