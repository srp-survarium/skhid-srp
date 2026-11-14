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
/// The second field counts the lookups (the registry's lock).
#[derive(Clone, Default)]
struct Table(
    Rc<RefCell<HashMap<u32, VoiceSeat>>>,
    Rc<std::cell::Cell<u32>>,
);

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
                channel: VoiceChannel::Team(team),
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
        self.1.set(self.1.get() + 1);
        self.0.borrow().get(&session_id).copied()
    }

    fn is_open(&self, match_id: u32) -> bool {
        self.0
            .borrow()
            .values()
            .any(|seat| seat.match_id == match_id)
    }

    /// Each seat's account is 100 + its session.
    fn roster(&self, match_id: u32, channel: VoiceChannel) -> Vec<(u8, u32)> {
        let mut places: Vec<(u8, u32)> = self
            .0
            .borrow()
            .iter()
            .filter(|(_, seat)| seat.match_id == match_id && seat.channel == channel)
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
    // 0x48: the TOC of one 20 ms SILK frame (wide band).
    for frame in [
        [0x48, 0xA1, 0xA1].as_slice(),
        [0x48, 0xB2, 0xB2, 0xB2, 0xB2].as_slice(),
    ] {
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
    // A seat whose connection's IP is not known yet (its game's first HELLO,
    // before the 0xD8): nothing sent, nor counted or logged as a drop.
    bench.table.0.borrow_mut().get_mut(&3).unwrap().ip = None;
    assert!(bench.send(3, &hello(3, true)).is_empty());
    assert!(bench.send(3, &start(3, 1)).is_empty());
    assert_eq!(bench.relay.counts(MATCH).map(|c| c.dropped), Some(2));
    let lines = bench.relay.drop_lines(bench.now);
    assert!(
        lines.iter().all(|line| !line.contains("10.0.0.3")),
        "{lines:?}"
    );
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
    // Too large: 401 bytes.
    let mut large = audio(1, 1, 0);
    large.resize(MAX_DATAGRAM + 1, 0);
    assert!(bench.send(1, &large).is_empty());
    // The largest the games send: two frames of the most the encoder makes.
    let longest = [0x48; MAX_FRAME_BYTES];
    assert_eq!(
        sent_to(&bench.send(1, &audio_of(&[&longest, &longest]))),
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
fn an_utterance_is_cut_a_second_past_the_games_own_cap() {
    let mut bench = Bench::new();
    bench.send(2, &hello(2, true));
    bench.send(1, &start(1, 1));
    let mut ms = 0;
    let mut seq = 0;
    let mut ended = None;
    while ms <= 32_000 {
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
    assert_eq!(ended, Some((31_000, EndReason::TooLong as u8)));
    // Its audio after the cut opens nothing; a new START does, at once.
    bench.at(31_040);
    assert!(bench.send(1, &audio(1, 1, seq + 1)).is_empty());
    assert_eq!(sent_to(&bench.send(1, &start(1, 2))), vec![(2, 0x82)]);
    let counts = bench.relay.counts(MATCH).unwrap();
    assert_eq!(counts.utterances, 2);
    assert!(counts.talked >= Duration::from_secs(30));
}

#[test]
fn the_games_end_at_thirty_seconds_comes_before_the_cut() {
    let mut bench = Bench::new();
    bench.send(2, &hello(2, true));
    bench.send(1, &start(1, 1));
    for seq in 0..750u16 {
        bench.at(u64::from(seq) * 40);
        if seq % 50 == 0 {
            bench.send(2, &hello(2, true));
        }
        let mut out = bench.send(1, &audio(1, 1, seq));
        out.extend(bench.poll());
        assert_eq!(sent_to(&out), vec![(2, 0x83)], "{seq}");
    }
    // The game's cap of 30 s, its END a little late: released, not cut.
    bench.at(30_300);
    assert!(bench.poll().is_empty());
    let out = bench.send(1, &end(1, 1));
    assert_eq!(sent_to(&out), vec![(2, 0x84)]);
    assert_eq!(out[0].1[6], EndReason::Released as u8);
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
    bench.send(1, &start(1, 1));
    let mut welcomed = 0;
    for _ in 0..80 {
        welcomed += welcomes(bench.send(1, &hello(1, true))).len();
    }
    // START was the first of 60 this second.
    assert_eq!(welcomed, 59);
    assert_eq!(bench.relay.counts(MATCH).unwrap().dropped, 21);
    bench.at(1_000);
    assert_eq!(welcomes(bench.send(1, &hello(1, true))).len(), 1);
}

#[test]
fn a_talkers_audio_over_thirty_a_second_is_dropped() {
    let mut bench = Bench::new();
    bench.send(2, &hello(2, true));
    bench.send(1, &start(1, 1));
    // The games' 25 a second, with bursts of 3 after a stall: all relayed.
    let mut seq = 0;
    for tick in 0..250u64 {
        bench.at(tick * 40);
        if tick % 50 == 0 {
            bench.send(2, &hello(2, true));
        }
        if tick % 10 == 9 {
            continue;
        }
        for _ in 0..if tick % 10 == 0 { 2 } else { 1 } {
            assert_eq!(sent_to(&bench.send(1, &audio(1, 1, seq))), vec![(2, 0x83)]);
            seq += 1;
        }
    }
    assert_eq!(bench.relay.counts(MATCH).unwrap().dropped, 0);
    // A network hold of 0.8 s, then its 20 AUDIO at once: all taken (more
    // than a receiver's ring of 16); past them, nothing until the rate
    // refills.
    bench.at(10_800);
    bench.send(2, &hello(2, true));
    let mut relayed = 0;
    for _ in 0..25 {
        relayed += bench.send(1, &audio(1, 1, seq)).len();
        seq += 1;
    }
    assert_eq!(relayed, 20);
    // Another talker at twice the games' rate for 2 s: its burst and 30 a
    // second (59.4 in the 1.98 s from its first to its last).
    relayed = 0;
    for (seq, ms) in (20..=2_000).step_by(20).enumerate() {
        bench.at(12_000 + ms);
        relayed += bench
            .send(3, &audio(3, 1, seq as u16))
            .iter()
            .filter(|(_, datagram)| datagram[2] == kind::RELAYED_AUDIO)
            .count();
    }
    assert_eq!(relayed, 79);
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
fn in_a_free_for_all_everyone_hears_everyone_in_a_team_mode_teammates() {
    use super::super::tests::{MAP_TDM, session_of, setup};
    use survarium::v026g::maps::{self, GameMode};
    let free_for_all = maps::find(1, maps::weather::DAY, GameMode::FreeForAll)
        .expect("level_01's free-for-all")
        .id;
    // Three places: in team deathmatch 0 and 2 on team 0, 1 on team 1; in
    // the free-for-all each a side of its own (4, 5, 6).
    for (map_id, heard, roster) in [
        (MAP_TDM, vec![2], vec![0, 2]),
        (free_for_all, vec![1, 2], vec![0, 1, 2]),
    ] {
        let registry = Arc::new(MatchRegistry::new("127.0.0.1", 1));
        let ticket = registry.create(setup(3, map_id)).unwrap();
        // Each place's game at 10.0.0.<place + 1>, port 5000 + place + 1.
        let game = |place: u8| addr(u32::from(place) + 1);
        for place in 0..3 {
            registry.set_connection(
                Seat {
                    match_id: ticket.match_id,
                    place,
                },
                game(place).ip(),
            );
        }
        let now = Instant::now();
        let mut relay = Relay::new(registry.clone(), now);
        let mut rosters = Vec::new();
        for place in 0..3 {
            let session = session_of(usize::from(place));
            for (_, datagram) in relay.receive(game(place), &hello(session, true), now) {
                if datagram[2] == kind::ROSTER {
                    rosters.push(datagram);
                }
            }
        }
        // Place 0's ROSTER: the places it hears and is heard by.
        let listed: Vec<u8> = rosters[0][4..].chunks(5).map(|entry| entry[0]).collect();
        assert_eq!(listed, roster, "map {map_id}");
        let out = relay.receive(game(0), &start(session_of(0), 1), now);
        let mut to: Vec<u8> = out.iter().map(|(to, _)| (to.port() - 5001) as u8).collect();
        to.sort_unstable();
        assert_eq!(to, heard, "map {map_id}");
    }
}

#[test]
fn a_game_connected_to_its_match_is_a_voice_seat_from_its_ip() {
    use super::super::tests::{Bench, MAP_BATTERIES, session_of};
    let mut bench = Bench::new(2, MAP_BATTERIES, crate::v026g::Settings::default());
    assert_eq!(bench.registry.voice_seat(session_of(0)).unwrap().ip, None);
    bench.join(0);
    let seat = bench.registry.voice_seat(session_of(0)).unwrap();
    assert_eq!(
        (seat.match_id, seat.place, seat.channel, seat.ip),
        (
            bench.match_id,
            0,
            VoiceChannel::Team(0),
            Some(ip("127.0.0.1"))
        )
    );
    assert_eq!(bench.registry.voice_seat(session_of(1)).unwrap().ip, None);
    assert_eq!(
        bench.registry.voice_seat(session_of(1)).unwrap().channel,
        VoiceChannel::Team(1)
    );
}

/// A folder of its own under the system's temporary folder.
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("srp-voice-key-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn the_voice_id_key_is_made_once_and_kept() {
    let dir = scratch("made");
    let key = id_key(&dir).unwrap();
    let path = dir.join("voice-id.key");
    assert_eq!(std::fs::read(&path).unwrap(), key);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    // Read back, not made again.
    assert_eq!(id_key(&dir).unwrap(), key);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_wrong_or_unreadable_voice_id_key_is_an_error_and_left_alone() {
    let dir = scratch("wrong");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("voice-id.key");
    std::fs::write(&path, [7; 31]).unwrap();
    let error = id_key(&dir).unwrap_err();
    assert!(error.contains("31 bytes"), "{error}");
    assert_eq!(std::fs::read(&path).unwrap(), [7; 31]);
    // A folder where the file should be: it cannot be read; nothing made.
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    let error = id_key(&dir).unwrap_err();
    assert!(error.contains("can't read"), "{error}");
    assert!(path.is_dir());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn released_or_reseated_listeners_stop_hearing_before_housekeeping() {
    let mut bench = Bench::new();
    bench.all_hello();
    bench.send(1, &start(1, 1));
    bench.table.unseat(2);
    bench.table.seat(3, MATCH, 4, 1);
    bench.at(20);
    assert!(bench.send(1, &audio(1, 1, 0)).is_empty());
    // A new HELLO adopts the new team immediately.
    bench.send(3, &hello(3, true));
    assert_eq!(sent_to(&bench.send(4, &start(4, 1))), vec![(3, 0x82)]);
}

#[test]
fn a_reseated_talker_cannot_continue_sending_audio_to_its_old_team() {
    let mut bench = Bench::new();
    bench.all_hello();
    bench.send(1, &start(1, 1));
    bench.table.seat(1, MATCH, 0, 1);
    bench.at(20);
    // The old team's utterance ends; this AUDIO cannot reopen it there.
    assert_eq!(
        sent_to(&bench.send(1, &audio(1, 1, 0))),
        vec![(2, 0x84), (3, 0x84)]
    );
    assert_eq!(sent_to(&bench.send(1, &start(1, 2))), vec![(4, 0x82)]);
    assert_eq!(sent_to(&bench.send(1, &audio(1, 2, 0))), vec![(4, 0x83)]);
}

#[test]
fn delayed_utterances_cannot_replace_a_newer_one() {
    let mut bench = Bench::new();
    bench.all_hello();
    bench.send(1, &start(1, 10));
    bench.send(1, &start(1, 11));
    assert!(bench.send(1, &start(1, 9)).is_empty());
    assert!(bench.send(1, &audio(1, 10, 5)).is_empty());
    assert!(bench.send(1, &end(1, 10)).is_empty());
    assert_eq!(
        sent_to(&bench.send(1, &audio(1, 11, 0))),
        vec![(2, 0x83), (3, 0x83)]
    );
    bench.send(1, &end(1, 11));
    assert!(bench.send(1, &audio(1, 9, 6)).is_empty());
    assert!(bench.send(1, &start(1, 10)).is_empty());
}

#[test]
fn client_utterance_order_wraps_at_u16_max() {
    let mut bench = Bench::new();
    bench.all_hello();
    bench.send(1, &start(1, u16::MAX));
    bench.send(1, &end(1, u16::MAX));
    let out = bench.send(1, &start(1, 0));
    assert_eq!(out[0].1[4..6], [0, 0]);
    assert!(bench.send(1, &audio(1, u16::MAX, 0)).is_empty());
    assert_eq!(
        sent_to(&bench.send(1, &audio(1, 0, 0))),
        vec![(2, 0x83), (3, 0x83)]
    );
}

#[test]
fn restarted_process_keeps_the_same_account_but_gets_a_new_relay_utterance() {
    use skhid_voice::{Core, Encoder, FRAME, Hearing, wire};
    // Account identities stay attached to these slots even when their
    // authenticated sessions change, as a restarted player rejoins a match.
    struct StableAccounts(Table);
    impl Seats for StableAccounts {
        fn voice_seat(&self, session: u32) -> Option<VoiceSeat> {
            self.0.voice_seat(session)
        }
        fn is_open(&self, match_id: u32) -> bool {
            self.0.is_open(match_id)
        }
        fn roster(&self, match_id: u32, channel: VoiceChannel) -> Vec<(u8, u32)> {
            self.0
                .roster(match_id, channel)
                .into_iter()
                .map(|(slot, _)| (slot, u32::from(slot) + 100))
                .collect()
        }
    }
    let table = Table::new();
    let now = Instant::now();
    let mut relay = Relay::new(StableAccounts(table.clone()), now);
    let first_roster = relay.receive(addr(2), &hello(2, true), now);
    let mut core = Core::new().unwrap();
    for packet in [start(1, 10), end(1, 10)] {
        for (_, bytes) in relay.receive(addr(1), &packet, now) {
            core.receive(&wire::parse(&bytes).unwrap(), 0);
        }
    }
    table.unseat(1);
    table.seat(5, MATCH, 0, 0);
    // Do not run housekeeping between departure and rejoin. The listener
    // never observes a vacant slot or a changed stable account identity.
    assert_eq!(first_roster, relay.receive(addr(2), &hello(2, true), now));
    let restarted = relay.receive(addr(5), &start(5, 1), now);
    assert_eq!(restarted[0].1[4..6], 11u16.to_le_bytes());
    // Lose the new relayed START. Its AUDIO must still carry the new id,
    // otherwise the listener's old tombstone would reject this speaker.
    let mut encoder = Encoder::new().unwrap();
    for seq in 0..2 {
        let pcm: [i16; FRAME] = std::array::from_fn(|i| ((i as f32 * 0.25).sin() * 8000.) as i16);
        let frame = encoder.encode(&pcm).unwrap();
        let bytes = wire::Message::Audio {
            session: 5,
            utt: 1,
            seq,
            frames: vec![frame.clone(), frame],
        }
        .encode()
        .unwrap();
        for (_, bytes) in relay.receive(addr(5), &bytes, now) {
            assert_eq!(bytes[4..6], 11u16.to_le_bytes());
            core.receive(&wire::parse(&bytes).unwrap(), 20);
        }
    }
    let ended = relay.receive(addr(5), &end(5, 1), now);
    assert_eq!(ended[0].1[4..6], 11u16.to_le_bytes());
    core.receive(&wire::parse(&ended[0].1).unwrap(), 20);
    for tick in 0..4 {
        core.mix(1., &Hearing::radio(0), 40 + tick * 20);
    }
    assert_eq!(core.stats().decoded, 4);
    assert_eq!(core.stats().late, 0);
}

/// An AUDIO of session 1, utt 1 carrying `frames` as they are.
fn audio_of(frames: &[&[u8]]) -> Vec<u8> {
    let mut datagram = vec![MAGIC, VERSION, kind::AUDIO];
    datagram.extend_from_slice(&1u32.to_le_bytes());
    datagram.extend_from_slice(&[1, 0, 0, 0, frames.len() as u8]);
    for frame in frames {
        datagram.push(frame.len() as u8);
        datagram.extend_from_slice(frame);
    }
    datagram
}

#[test]
fn every_truncation_and_extension_is_dropped_with_its_reason() {
    for datagram in [hello(1, true), start(1, 1), audio(1, 1, 0), end(1, 1)] {
        assert!(parse(&datagram).is_ok());
        for len in 0..datagram.len() {
            // A HELLO without its mask is the old one, and taken.
            if datagram[2] == kind::HELLO && len == 8 {
                continue;
            }
            assert!(parse(&datagram[..len]).is_err(), "{:?}", &datagram[..len]);
        }
        let mut longer = datagram.clone();
        longer.push(0);
        assert!(parse(&longer).is_err(), "{longer:?}");
    }
    assert_eq!(parse(&[]), Err(Dropped::Short));
    assert_eq!(parse(&[MAGIC, VERSION]), Err(Dropped::Short));
    assert_eq!(
        parse(&[MAGIC, VERSION, kind::HELLO, 1, 0, 0]),
        Err(Dropped::Short)
    );
    assert_eq!(
        parse(&[MAGIC, VERSION, kind::HELLO, 1, 0, 0, 0, 1, 0]),
        Err(Dropped::Length)
    );
    assert_eq!(parse(&start(1, 1)[..12]), Err(Dropped::Length));
    assert_eq!(parse(&audio(1, 1, 0)[..12]), Err(Dropped::Frames));
}

#[test]
fn oversize_wrong_version_and_types_are_dropped_with_their_reason() {
    let mut large = audio(1, 1, 0);
    large.resize(MAX_DATAGRAM + 1, 0);
    assert_eq!(parse(&large), Err(Dropped::TooLarge));
    assert_eq!(parse(&[0; 2048]), Err(Dropped::TooLarge));
    let mut bad = start(1, 1);
    bad[0] = 0x57;
    assert_eq!(parse(&bad), Err(Dropped::NotVoice));
    bad[0] = MAGIC;
    bad[1] = 2;
    assert_eq!(parse(&bad), Err(Dropped::Version(2)));
    bad[1] = VERSION;
    for kind in [
        0x00,
        0x05,
        kind::WELCOME,
        kind::RELAYED_AUDIO,
        kind::ROSTER,
        0xFF,
    ] {
        bad[2] = kind;
        assert_eq!(parse(&bad), Err(Dropped::Kind(kind)));
    }
    // None of it reaches a teammate.
    let mut bench = Bench::new();
    bench.all_hello();
    for datagram in [large, bad] {
        assert!(bench.send(1, &datagram).is_empty());
    }
}

#[test]
fn audio_frames_must_be_the_games_encoders() {
    let frame: &[u8] = &[0x48, 1, 2];
    for count in 1..=MAX_FRAMES {
        assert!(matches!(
            parse(&audio_of(&vec![frame; count])),
            Ok(Datagram::Audio { .. })
        ));
    }
    assert_eq!(parse(&audio_of(&[])), Err(Dropped::Frames));
    // Three frames: more than the games send, and play.
    assert_eq!(parse(&audio_of(&[frame; 3])), Err(Dropped::Frames));
    assert_eq!(parse(&audio_of(&[&[]])), Err(Dropped::NotOpus));
    // SILK narrow, medium and wide band, 20 ms, mono, code 0.
    for toc in FRAME_TOCS {
        assert!(parse(&audio_of(&[&[toc, 1]])).is_ok());
    }
    // 20 ms all, but not the encoder's: stereo, code 1 (two of 10 ms), code
    // 3 (one, padded), hybrid and CELT; and 10 and 60 ms SILK.
    for toc in [0x4C, 0x01, 0x4B, 0x68, 0xF8, 0x00, 0x18] {
        assert_eq!(parse(&audio_of(&[&[toc, 1, 1]])), Err(Dropped::NotOpus));
    }
    // Longer than the encoder's longest.
    let longer = [0x48; MAX_FRAME_BYTES + 1];
    assert_eq!(parse(&audio_of(&[&longer])), Err(Dropped::NotOpus));
    // A frame said longer than what follows.
    let mut cut = audio_of(&[frame]);
    cut.pop();
    assert_eq!(parse(&cut), Err(Dropped::Frames));
    // Garbage Opus from a seated teammate is not forwarded, only counted.
    let mut bench = Bench::new();
    bench.all_hello();
    assert!(bench.send(1, &audio_of(&[&[0xF8, 0xFF, 0xFF]])).is_empty());
    assert_eq!(bench.relay.counts(MATCH), None);
    assert_eq!(
        sent_to(&bench.send(1, &audio_of(&[frame, frame]))),
        vec![(2, 0x82), (2, 0x83), (3, 0x82), (3, 0x83)]
    );
}

#[test]
fn the_frame_check_is_the_games_and_takes_their_encoders_frames() {
    use skhid_voice::{Decoder, Encoder, FRAME, wire};
    assert_eq!(MAX_FRAMES, wire::MAX_FRAMES);
    assert_eq!(MAX_FRAME_BYTES, wire::MAX_FRAME_BYTES);
    assert_eq!(FRAME_TOCS, wire::FRAME_TOCS);
    assert_eq!(MAX_DATAGRAM, wire::MAX_DATAGRAM);
    let mut decoder = Decoder::new().unwrap();
    for toc in 0..=255u8 {
        for len in [1, 2, MAX_FRAME_BYTES, MAX_FRAME_BYTES + 1] {
            let mut packet = vec![toc; len];
            packet[0] = toc;
            assert_eq!(our_frame(&packet), wire::our_frame(&packet), "{packet:?}");
            // What is taken, the games' decoder decodes.
            if our_frame(&packet) {
                assert!(decoder.decode(Some(&packet)).is_ok(), "{packet:?}");
                decoder.reset();
            }
        }
    }
    // The games' encoder, loud: what it makes is taken, as the sender sends it.
    let mut encoder = Encoder::new().unwrap();
    let mut sender = skhid_voice::Sender::new(1, 1);
    sender.start(0);
    for n in 0..51 {
        let pcm: [i16; FRAME] = std::array::from_fn(|i| {
            if ((n * FRAME + i) / 8).is_multiple_of(2) {
                i16::MAX
            } else {
                i16::MIN
            }
        });
        let frame = encoder.encode(&pcm).unwrap();
        if let Some(datagram) = sender.frame(&frame) {
            assert!(matches!(parse(&datagram), Ok(Datagram::Audio { .. })));
        }
    }
    let datagram = sender.flush().unwrap();
    assert!(matches!(parse(&datagram), Ok(Datagram::Audio { frames, .. }) if frames.len() == 1));
}

#[test]
fn a_slot_past_the_mute_mask_is_never_muted() {
    assert!(muted(u32::MAX, 31));
    assert!(!muted(u32::MAX, 32));
    assert!(!muted(u32::MAX, u8::MAX));
    let mut bench = Bench::new();
    bench.table.seat(5, MATCH, 40, 0);
    bench.send(2, &hello_muting(2, true, u32::MAX));
    assert_eq!(sent_to(&bench.send(5, &start(5, 1))), vec![(2, 0x82)]);
}

#[test]
fn a_flood_from_one_source_does_not_delay_a_good_sender() {
    let mut bench = Bench::new();
    bench.all_hello();
    let flood = SocketAddr::new(ip("10.9.9.9"), 4000);
    let mut junk = 0x1234_5678u32;
    for n in 0..10_000u32 {
        junk = junk.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let datagram = match n % 5 {
            // Session 1's number from another IP: the talker's rate is not
            // spent by it.
            0 => audio(1, 1, n as u16),
            1 => start(1, n as u16),
            // Sessions with no seat.
            2 => hello(junk, true),
            3 => junk.to_le_bytes().repeat(1 + (junk % 120) as usize),
            _ => {
                let mut datagram = audio(1, 1, 0);
                let at = junk as usize % datagram.len();
                datagram[at] ^= 0xFF;
                datagram
            }
        };
        assert!(bench.send_from(flood, &datagram).is_empty());
    }
    // Its seats were looked up only until its source's rate ran out: each
    // costs it 20 of its 400.
    assert!(bench.table.1.get() <= 5 + 20, "{}", bench.table.1.get());
    // The good sender, the same second: all of its datagrams are relayed
    // (its START and a burst of AUDIO).
    assert_eq!(
        sent_to(&bench.send(1, &start(1, 1))),
        vec![(2, 0x82), (3, 0x82)]
    );
    for seq in 0..10 {
        assert_eq!(bench.send(1, &audio(1, 1, seq)).len(), 2);
    }
    // The flood is one line in the log.
    let lines = bench.relay.drop_lines(bench.now);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].starts_with("Voice: dropped 10000 datagrams from 10.9.9.9"),
        "{lines:?}"
    );
}

#[test]
fn a_source_is_limited_before_its_session_is_looked_up() {
    let mut bench = Bench::new();
    bench.all_hello();
    let lookups = bench.table.1.get();
    // Guessing sessions from one address: 20 lookups, then none.
    let guesser = SocketAddr::new(ip("10.9.9.9"), 4000);
    for session in 100..10_100 {
        assert!(bench.send_from(guesser, &hello(session, true)).is_empty());
    }
    assert_eq!(bench.table.1.get() - lookups, 20);
    // Malformed datagrams spend its flood cap, one each.
    bench.at(1_000);
    let mut parsed = 0;
    for _ in 0..1_000 {
        bench.send_from(guesser, &[MAGIC]);
        parsed += 1;
    }
    assert_eq!(parsed, 1_000);
    let lines = bench.relay.drop_lines(bench.now);
    assert!(
        lines[0].starts_with("Voice: dropped 11000 datagrams from 10.9.9.9 (the first: no seat)"),
        "{lines:?}"
    );
    // The players are not limited by it.
    assert_eq!(
        sent_to(&bench.send(1, &start(1, 1))),
        vec![(2, 0x82), (3, 0x82)]
    );
    // A spoofed flood from more addresses than it keeps apart: its memory
    // is bounded, and those past it share one rate.
    bench.at(2_000);
    let before = bench.table.1.get();
    for n in 0..(SOURCES as u32 + 1_000) {
        let from = SocketAddr::new(std::net::Ipv4Addr::from(0x0B00_0000 + n).into(), 4000);
        bench.send_from(from, &hello(n + 100, true));
    }
    assert_eq!(bench.relay.sources.len(), SOURCES);
    assert_eq!(bench.relay.refused.len(), SOURCES);
    assert!(bench.table.1.get() - before <= SOURCES as u32 + 20);
    // Once back to their whole rate, sources are forgotten.
    bench.at(10_000);
    bench.poll();
    assert!(bench.relay.sources.is_empty());
    assert!(bench.relay.refused.is_empty());
}

#[test]
fn a_refused_game_does_not_stop_the_others_on_its_ip() {
    let mut bench = Bench::new();
    bench.all_hello();
    // Session 5 plays from session 1's IP (a shared exit), not seen yet.
    bench.table.seat(5, MATCH, 6, 0);
    bench.table.0.borrow_mut().get_mut(&5).unwrap().ip = Some(addr(1).ip());
    let shared = |port: u16| SocketAddr::new(addr(1).ip(), port);
    let lookups = bench.table.1.get();
    // A game of the same exit refused: its match over (no seat) while it
    // holds push-to-talk, and another whose voice left by another exit
    // than its match connection (session 4's, from session 1's IP).
    for seq in 0..1_000u16 {
        bench.at(u64::from(seq));
        assert!(bench.send_from(shared(7000), &audio(77, 1, seq)).is_empty());
        assert!(bench.send_from(shared(7001), &audio(4, 1, seq)).is_empty());
    }
    // Each was looked up once a second.
    assert_eq!(bench.table.1.get() - lookups, 2);
    // The others of that exit go on: session 1, known, and session 5, new.
    bench.at(1_000);
    assert_eq!(
        sent_to(&bench.send(1, &start(1, 1))),
        vec![(2, 0x82), (3, 0x82)]
    );
    assert_eq!(
        welcomes(bench.send_from(shared(7002), &hello(5, true))).len(),
        1
    );
    // A guessing flood from the exit: 20 failed lookups a second, and the
    // known games still looked up.
    for session in 1_000..2_000 {
        bench.send_from(shared(7003), &hello(session, true));
    }
    assert_eq!(
        sent_to(&bench.send(1, &audio(1, 1, 0))),
        vec![(2, 0x83), (3, 0x83), (2002, 0x83)]
    );
}

#[test]
fn forty_players_behind_one_exit_are_not_limited() {
    let mut bench = Bench::new();
    let exit = ip("10.7.7.7");
    for session in 100..140u32 {
        bench
            .table
            .seat(session, 0x30, (session - 100) as u8, (session % 2) as u8);
        bench.table.0.borrow_mut().get_mut(&session).unwrap().ip = Some(exit);
    }
    let from = |session: u32| SocketAddr::new(exit, session as u16);
    // Each says HELLO 4 times a second and talks (25 AUDIO a second), for
    // 2 s.
    for tick in 0..50u16 {
        bench.at(u64::from(tick) * 40);
        for session in 100..140u32 {
            if tick % 6 == 0 {
                bench.send_from(from(session), &hello(session, true));
            }
            bench.send_from(from(session), &audio(session, 1, tick));
        }
    }
    let dropped = bench.relay.counts(0x30).map_or(0, |counts| counts.dropped);
    assert_eq!(dropped, 0);
    assert_eq!(bench.relay.dropped_unknown, 0);
}

#[test]
fn the_drop_log_is_a_line_a_source_at_most_every_ten_seconds() {
    let mut bench = Bench::new();
    let flood = SocketAddr::new(ip("10.9.9.9"), 4000);
    for _ in 0..50 {
        bench.send_from(flood, &[MAGIC]);
    }
    // A spoofed flood from 2000 addresses: counted, its memory bounded.
    for n in 0..2000u32 {
        let from = SocketAddr::new(std::net::Ipv4Addr::from(0x0A01_0000 + n).into(), 4000);
        bench.send_from(from, &[0; 3]);
    }
    assert_eq!(bench.relay.drops.sources.len(), DROP_SOURCES);
    let lines = bench.relay.drop_lines(bench.now);
    assert_eq!(lines.len(), DROP_LOG_SOURCES + 1, "{lines:?}");
    assert_eq!(
        lines[0],
        "Voice: dropped 50 datagrams from 10.9.9.9 (the first: truncated header)"
    );
    assert_eq!(
        lines[1],
        "Voice: dropped 1 datagrams from 10.1.0.0 (the first: not voice)"
    );
    assert_eq!(
        lines[DROP_LOG_SOURCES],
        format!(
            "Voice: dropped {} more datagrams from other sources",
            2000 - (DROP_LOG_SOURCES - 1)
        )
    );
    // More within ten seconds: no line until they are up.
    bench.send_from(flood, &[MAGIC]);
    bench.at(9_999);
    assert!(bench.relay.drop_lines(bench.now).is_empty());
    bench.send_from(flood, &start(77, 1));
    bench.at(10_000);
    assert_eq!(
        bench.relay.drop_lines(bench.now),
        vec!["Voice: dropped 2 datagrams from 10.9.9.9 (the first: truncated header)"]
    );
    // Nothing dropped: nothing written.
    bench.at(30_000);
    assert!(bench.relay.drop_lines(bench.now).is_empty());
    assert!(bench.relay.drops.sources.is_empty());
}

#[test]
fn a_flood_over_the_socket_does_not_delay_a_good_sender() {
    use super::super::tests::{MAP_TDM, session_of as session};
    let registry = Arc::new(MatchRegistry::new("127.0.0.1", 1));
    let ticket = registry
        .create(super::super::tests::setup(4, MAP_TDM))
        .unwrap();
    for place in [0, 2] {
        registry.set_connection(
            Seat {
                match_id: ticket.match_id,
                place,
            },
            ip("127.0.0.1"),
        );
    }
    let server = UdpSocket::bind("127.0.0.1:0").unwrap();
    let server_addr = server.local_addr().unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let relay = {
        let (registry, stop) = (registry.clone(), stop.clone());
        std::thread::spawn(move || run(&server, registry, [7; 32], &stop))
    };
    let game = |place: usize| {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        socket
            .set_read_timeout(Some(Duration::from_millis(50)))
            .unwrap();
        socket
            .send_to(&hello(session(place), true), server_addr)
            .unwrap();
        socket
    };
    let (talker, listener) = (game(0), game(2));
    // Another address of the loopback floods: junk, and session 0's number.
    let flooding = Arc::new(AtomicBool::new(true));
    let flooded = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let flood = {
        let (flooding, flooded) = (flooding.clone(), flooded.clone());
        let spoofed = [audio(session(0), 9, 0), start(session(0), 9)];
        std::thread::spawn(move || {
            let socket = UdpSocket::bind("127.0.0.2:0").unwrap();
            let mut sent = 0u32;
            while flooding.load(Ordering::Relaxed) && sent < 2_000_000 {
                let datagram: &[u8] = match sent % 3 {
                    0 => &[MAGIC, VERSION, kind::AUDIO, 1],
                    1 => &spoofed[0],
                    _ => &spoofed[1],
                };
                let _ = socket.send_to(datagram, server_addr);
                sent += 1;
                flooded.store(sent, Ordering::Relaxed);
            }
            sent
        })
    };
    // The flood is on before the good talker starts.
    while flooded.load(Ordering::Relaxed) < 20_000 {
        std::thread::yield_now();
    }
    // The good talker's START gets to its teammate within a second, under
    // the flood (sent again while it does not: the system may drop one when
    // the socket's queue is full).
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    let mut buffer = [0; 512];
    let mut heard = false;
    let mut utt = 10;
    while !heard && std::time::Instant::now() < deadline {
        listener
            .send_to(&hello(session(2), true), server_addr)
            .unwrap();
        // A new utterance each time: a repeated START is not relayed again.
        talker
            .send_to(&start(session(0), utt), server_addr)
            .unwrap();
        utt += 1;
        while let Ok(len) = listener.recv(&mut buffer) {
            if buffer[..len].get(2) == Some(&kind::RELAYED_START) {
                heard = true;
                break;
            }
        }
    }
    flooding.store(false, Ordering::Relaxed);
    let sent = flood.join().unwrap();
    stop.store(true, Ordering::Relaxed);
    relay.join().unwrap().unwrap();
    assert!(sent > 20_000, "the flood sent only {sent}");
    assert!(heard, "the good START was not relayed under the flood");
}
