//! The voice relay: team push-to-talk in a match, on a UDP port of its own
//! (the match port + 1), beside the match server (`docs/notes/voice-chat.md`).
//!
//! The players' games (`skhid_voice.dll`, `srp-026g/client-voice`) send
//! their talk here as utterances (START, AUDIO, END) and get their team's.
//! The relay knows a talker by the session of its match connection (the
//! 0xD8), and takes its datagrams only from the IP that connection came from
//! ([`VoiceSeat::ip`]). It forwards each AUDIO to the speaker's teammates who
//! said HELLO lately and hear (their volume is above 0), and keeps nothing:
//! no audio past the forward, no file, no audio, text or speaker list in the
//! logs (counts only, per match). The simulation never sees any of it, so no
//! replay or recording has voice.
//!
//! Every utterance goes past an [`UtteranceTap`] ([`NoTap`] by default): the
//! seam for subtitles and translation later. What a tap says goes back to the
//! team through [`RelayOut`] as a TEXT datagram (type 85h).
//!
//! A mute is kept by the speaker's **voice id**, not its name (an account can
//! be renamed, and another may take the old name): ROSTER gives each place
//! of the listener's team the first four bytes of SHA-256 of a server key
//! and the account id, stable for the account and opaque to the players
//! ([`voice_id`]). The key is `<SRP_DATA_DIR>/voice-id.key`, made once
//! ([`id_key_from_env`]).
//!
//! Muting is the listener's: its game plays nothing of a slot it mutes, and
//! its HELLO says which slots (the `muted` mask), so the relay sends it
//! nothing of them either. A game with voice turned off says it does not
//! hear, and gets nothing.
//!
//! The wire (little-endian; every datagram `56 01 <type>`, a type not known
//! ignored):
//!
//! ```text
//! C->S 01 HELLO  u32 session, u8 flags (bit 0: hears), u32 muted (bit n: slot n)
//! C->S 02 START  u32 session, u16 utt, u32 client_ms
//! C->S 03 AUDIO  u32 session, u16 utt, u16 seq, u8 n, n x (u8 len, Opus frame)
//! C->S 04 END    u32 session, u16 utt, u16 frames
//! S->C 81 WELCOME u8 slot
//! S->C 82 START  u8 slot, u16 utt, u32 server_ms
//! S->C 83 AUDIO  u8 slot, u16 utt, u16 seq, u8 n, frames
//! S->C 84 END    u8 slot, u16 utt, u8 reason (0 released, 1 timeout, 2 too long)
//! S->C 85 TEXT   u8 slot, u16 utt, u8 final, u8 lang[2], u8 len, utf8
//! S->C 90 ROSTER u8 count, count x (u8 slot, u32 voice id)   (with each WELCOME)
//! ```
//!
//! Settings: `SRP_VOICE=on|off` (default `on`): srp binds the match port + 1
//! for it (the games send to that port; there is no setting of its own).

use std::collections::HashMap;
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::registry::{MatchRegistry, VoiceSeat};

pub const MAGIC: u8 = 0x56;
pub const VERSION: u8 = 1;

/// The datagram types.
pub mod kind {
    pub const HELLO: u8 = 0x01;
    pub const START: u8 = 0x02;
    pub const AUDIO: u8 = 0x03;
    pub const END: u8 = 0x04;
    pub const WELCOME: u8 = 0x81;
    pub const RELAYED_START: u8 = 0x82;
    pub const RELAYED_AUDIO: u8 = 0x83;
    pub const RELAYED_END: u8 = 0x84;
    pub const TEXT: u8 = 0x85;
    pub const ROSTER: u8 = 0x90;
}

/// A datagram larger than this is dropped (16 kbit/s Opus: about 90 bytes).
pub const MAX_DATAGRAM: usize = 400;
/// A talker's datagrams in a second past this are dropped.
pub const MAX_PER_SECOND: u32 = 60;
/// A listener heard from this long ago gets nothing more.
pub const HELLO_FRESH: Duration = Duration::from_secs(6);
/// An utterance with no AUDIO this long is closed (END reason 1).
pub const SILENCE: Duration = Duration::from_secs(1);
/// An utterance this long is cut (END reason 2).
pub const LONGEST: Duration = Duration::from_secs(30);
const HOUSEKEEPING: Duration = Duration::from_secs(1);
const RECEIVE_WAIT: Duration = Duration::from_millis(10);

/// Why an utterance ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndReason {
    /// The talker let the key go (its END).
    Released = 0,
    /// No audio for [`SILENCE`].
    Timeout = 1,
    /// Cut at [`LONGEST`].
    TooLong = 2,
}

/// One utterance as the tap sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Utterance {
    pub match_id: u32,
    pub team: u8,
    /// The speaker's roster place: its slot on the wire.
    pub speaker: u8,
    pub utt: u16,
    /// The server's clock at its start.
    pub started_at: Instant,
}

/// What a tap sends back to the utterance's team.
#[derive(Default)]
pub struct RelayOut {
    texts: Vec<(Utterance, bool, [u8; 2], String)>,
}

impl RelayOut {
    /// A line of text about `utterance` (a subtitle, a translation) in the
    /// language `lang` (two ASCII letters), `final` when it will not change
    /// again. Cut to 255 bytes on a character's edge.
    pub fn text(&mut self, utterance: &Utterance, lang: [u8; 2], last: bool, text: &str) {
        let mut end = text.len().min(255);
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        self.texts
            .push((utterance.clone(), last, lang, text[..end].to_owned()));
    }
}

/// The seam for later steps (subtitles, translation): told of every
/// utterance. Frames are borrowed for the call only; a tap that wants a
/// whole utterance keeps it itself, in memory, and drops it at `ended`.
pub trait UtteranceTap: Send {
    fn started(&mut self, _utterance: &Utterance, _out: &mut RelayOut) {}
    fn audio(&mut self, _utterance: &Utterance, _seq: u16, _frames: &[&[u8]], _out: &mut RelayOut) {
    }
    fn ended(&mut self, _utterance: &Utterance, _reason: EndReason, _out: &mut RelayOut) {}
}

/// The default tap: nothing reads the audio.
pub struct NoTap;

impl UtteranceTap for NoTap {}

/// Where the relay finds a session's seat: the match registry, or a test's
/// table.
pub trait Seats {
    fn voice_seat(&self, session_id: u32) -> Option<VoiceSeat>;
    fn is_open(&self, match_id: u32) -> bool;
    /// The places of a match's team that a game plays (no bot's): the place
    /// and its account.
    fn roster(&self, match_id: u32, team: u8) -> Vec<(u8, u32)>;
}

impl Seats for MatchRegistry {
    fn voice_seat(&self, session_id: u32) -> Option<VoiceSeat> {
        MatchRegistry::voice_seat(self, session_id)
    }

    fn is_open(&self, match_id: u32) -> bool {
        MatchRegistry::is_open(self, match_id)
    }

    fn roster(&self, match_id: u32, team: u8) -> Vec<(u8, u32)> {
        self.voice_roster(match_id, team)
    }
}

impl<T: Seats + ?Sized> Seats for std::sync::Arc<T> {
    fn voice_seat(&self, session_id: u32) -> Option<VoiceSeat> {
        (**self).voice_seat(session_id)
    }

    fn is_open(&self, match_id: u32) -> bool {
        (**self).is_open(match_id)
    }

    fn roster(&self, match_id: u32, team: u8) -> Vec<(u8, u32)> {
        (**self).roster(match_id, team)
    }
}

/// An account's voice id: the first four bytes of SHA-256(key, account id),
/// never 0 (the games' "not known").
pub fn voice_id(key: &[u8; 32], account_id: u32) -> u32 {
    let mut hasher = openssl::sha::Sha256::new();
    hasher.update(key);
    hasher.update(&account_id.to_le_bytes());
    let digest = hasher.finish();
    match u32::from_le_bytes([digest[0], digest[1], digest[2], digest[3]]) {
        0 => 1,
        id => id,
    }
}

/// The voice ids' key of srp: `<SRP_DATA_DIR>/voice-id.key` (default
/// `data/`) through [`id_key`].
pub fn id_key_from_env() -> Result<[u8; 32], String> {
    let dir =
        std::env::var_os("SRP_DATA_DIR").map_or_else(|| "data".into(), std::path::PathBuf::from);
    id_key(&dir)
}

/// The voice ids' key in `dir` (`voice-id.key`): 32 random bytes made the
/// first time, when there is no such file, readable by srp's user alone
/// (mode 0600). Anything else that is wrong with it (it cannot be read, it
/// is not 32 bytes) is an error and the file is left as it is: the players'
/// mutes are kept by the ids it gives, so it is never made again over one.
pub fn id_key(dir: &std::path::Path) -> Result<[u8; 32], String> {
    let path = dir.join("voice-id.key");
    match std::fs::read(&path) {
        Ok(bytes) => {
            return <[u8; 32]>::try_from(bytes.as_slice()).map_err(|_| {
                format!(
                    "{} has {} bytes, not 32 (the players' mutes are kept by it: put the right \
                     file back, or delete it for a new key and lose them)",
                    path.display(),
                    bytes.len()
                )
            });
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("can't read {}: {error}", path.display())),
    }
    let mut key = [0; 32];
    openssl::rand::rand_bytes(&mut key).map_err(|error| format!("no random key: {error}"))?;
    std::fs::create_dir_all(dir)
        .map_err(|error| format!("can't make {}: {error}", dir.display()))?;
    let mut options = std::fs::OpenOptions::new();
    // A file that came meanwhile is not written over.
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let written = options.open(&path).and_then(|mut file| {
        use std::io::Write;
        file.write_all(&key)?;
        file.sync_all()
    });
    if let Err(error) = written {
        return Err(format!("can't make {}: {error}", path.display()));
    }
    log::info!("Voice: made the voice ids' key {}", path.display());
    Ok(key)
}

/// A client datagram, read.
#[derive(Debug, PartialEq, Eq)]
enum Datagram<'a> {
    Hello {
        session: u32,
        hears: bool,
        /// Bit n: the listener mutes slot n.
        muted: u32,
    },
    Start {
        session: u32,
        utt: u16,
    },
    Audio {
        session: u32,
        utt: u16,
        seq: u16,
        /// From `utt` to the end: the relayed AUDIO's body.
        body: &'a [u8],
        frames: Vec<&'a [u8]>,
    },
    End {
        session: u32,
        utt: u16,
    },
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// `None` for anything malformed, of another version, or of a type a
/// client does not send.
fn parse(datagram: &[u8]) -> Option<Datagram<'_>> {
    let (&[MAGIC, VERSION, kind], rest) = datagram.split_first_chunk::<3>()? else {
        return None;
    };
    let session = u32_at(rest, 0)?;
    let body = &rest[4..];
    match kind {
        // The mask may be left out (none muted).
        kind::HELLO if body.len() == 1 || body.len() == 5 => Some(Datagram::Hello {
            session,
            hears: body[0] & 1 != 0,
            muted: u32_at(body, 1).unwrap_or(0),
        }),
        kind::START if body.len() == 6 => Some(Datagram::Start {
            session,
            utt: u16_at(body, 0)?,
        }),
        kind::AUDIO => {
            let utt = u16_at(body, 0)?;
            let seq = u16_at(body, 2)?;
            let count = *body.get(4)?;
            if count == 0 {
                return None;
            }
            let mut frames = Vec::with_capacity(usize::from(count));
            let mut at = 5;
            for _ in 0..count {
                let len = usize::from(*body.get(at)?);
                let frame = body.get(at + 1..at + 1 + len)?;
                if frame.is_empty() {
                    return None;
                }
                frames.push(frame);
                at += 1 + len;
            }
            (at == body.len()).then_some(Datagram::Audio {
                session,
                utt,
                seq,
                body,
                frames,
            })
        }
        kind::END if body.len() == 4 => Some(Datagram::End {
            session,
            utt: u16_at(body, 0)?,
        }),
        _ => None,
    }
}

/// Whether a listener's mask mutes `slot`.
fn muted(mask: u32, slot: u8) -> bool {
    1u32.checked_shl(u32::from(slot))
        .is_some_and(|bit| mask & bit != 0)
}

fn header(kind: u8) -> Vec<u8> {
    vec![MAGIC, VERSION, kind]
}

struct Listener {
    addr: SocketAddr,
    seat: VoiceSeat,
    hears: bool,
    muted: u32,
    last_hello: Instant,
}

struct Open {
    utterance: Utterance,
    last_audio_at: Instant,
}

struct Talker {
    open: Option<Open>,
    /// The utterance closed last: its late AUDIO and repeated ENDs open
    /// nothing.
    closed: Option<u16>,
    /// The rate limit's second and its count.
    window: Instant,
    in_window: u32,
}

/// What a match's voice came to, for the log.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MatchCounts {
    pub utterances: u32,
    pub talked: Duration,
    pub dropped: u32,
}

/// The relay's state, driven by [`Relay::receive`] and [`Relay::poll`]
/// (the socket loop is [`run`]).
pub struct Relay<S: Seats> {
    seats: S,
    tap: Box<dyn UtteranceTap>,
    epoch: Instant,
    listeners: HashMap<u32, Listener>,
    talkers: HashMap<u32, Talker>,
    counts: HashMap<u32, MatchCounts>,
    /// Datagrams dropped before a session was known (too large, malformed).
    pub dropped_unknown: u64,
    /// The voice ids' key ([`voice_id`]).
    id_key: [u8; 32],
    next_housekeeping: Instant,
}

impl<S: Seats> Relay<S> {
    pub fn new(seats: S, now: Instant) -> Self {
        Self::with_tap(seats, Box::new(NoTap), now)
    }

    pub fn with_tap(seats: S, tap: Box<dyn UtteranceTap>, now: Instant) -> Self {
        Self {
            seats,
            tap,
            epoch: now,
            listeners: HashMap::new(),
            talkers: HashMap::new(),
            counts: HashMap::new(),
            dropped_unknown: 0,
            id_key: [0; 32],
            next_housekeeping: now + HOUSEKEEPING,
        }
    }

    /// The relay with the voice ids' key `key` (all zeros otherwise).
    pub fn with_id_key(mut self, key: [u8; 32]) -> Self {
        self.id_key = key;
        self
    }

    /// A match's counts so far (tests; the log has them at its end).
    pub fn counts(&self, match_id: u32) -> Option<MatchCounts> {
        self.counts.get(&match_id).copied()
    }

    fn server_ms(&self, now: Instant) -> u32 {
        now.saturating_duration_since(self.epoch).as_millis() as u32
    }

    fn drop_for(&mut self, match_id: Option<u32>) {
        match match_id {
            Some(match_id) => self.counts.entry(match_id).or_default().dropped += 1,
            None => self.dropped_unknown += 1,
        }
    }

    /// One datagram from `addr`: what to send, and where.
    pub fn receive(
        &mut self,
        addr: SocketAddr,
        datagram: &[u8],
        now: Instant,
    ) -> Vec<(SocketAddr, Vec<u8>)> {
        let mut out = Vec::new();
        if datagram.len() > MAX_DATAGRAM {
            self.drop_for(None);
            return out;
        }
        let Some(parsed) = parse(datagram) else {
            self.drop_for(None);
            return out;
        };
        let session = match &parsed {
            Datagram::Hello { session, .. }
            | Datagram::Start { session, .. }
            | Datagram::Audio { session, .. }
            | Datagram::End { session, .. } => *session,
        };
        // The session holds a seat, and the datagram comes from the IP its
        // game connected to the match from. Anything else: dropped, silently.
        let Some(seat) = self.seats.voice_seat(session) else {
            self.listeners.remove(&session);
            self.drop_for(None);
            return out;
        };
        if seat.ip != Some(addr.ip().to_canonical()) {
            self.drop_for(Some(seat.match_id));
            return out;
        }
        let talker = self.talkers.entry(session).or_insert(Talker {
            open: None,
            closed: None,
            window: now,
            in_window: 0,
        });
        if now.saturating_duration_since(talker.window) >= Duration::from_secs(1) {
            talker.window = now;
            talker.in_window = 0;
        }
        talker.in_window += 1;
        if talker.in_window > MAX_PER_SECOND {
            self.drop_for(Some(seat.match_id));
            return out;
        }
        match parsed {
            Datagram::Hello { hears, muted, .. } => {
                self.listeners.insert(
                    session,
                    Listener {
                        addr,
                        seat,
                        hears,
                        muted,
                        last_hello: now,
                    },
                );
                let mut welcome = header(kind::WELCOME);
                welcome.push(seat.place);
                out.push((addr, welcome));
                let places = self.seats.roster(seat.match_id, seat.team);
                let mut roster = header(kind::ROSTER);
                roster.push(places.len().min(20) as u8);
                for (place, account) in places.into_iter().take(20) {
                    roster.push(place);
                    roster.extend_from_slice(&voice_id(&self.id_key, account).to_le_bytes());
                }
                out.push((addr, roster));
            }
            Datagram::Start { utt, .. } => self.start(session, seat, utt, now, &mut out),
            Datagram::Audio {
                utt,
                seq,
                body,
                frames,
                ..
            } => {
                let talker = &self.talkers[&session];
                let fresh = match &talker.open {
                    Some(open) => open.utterance.utt != utt,
                    None => true,
                };
                if fresh {
                    if talker.closed == Some(utt) {
                        // Late audio of an utterance that ended (cut, timed
                        // out or released): nothing reopens it.
                        self.drop_for(Some(seat.match_id));
                        return out;
                    }
                    // Its START was lost.
                    self.start(session, seat, utt, now, &mut out);
                }
                let Some(open) = self
                    .talkers
                    .get_mut(&session)
                    .and_then(|talker| talker.open.as_mut())
                else {
                    return out;
                };
                open.last_audio_at = now;
                let utterance = open.utterance.clone();
                let mut relayed = header(kind::RELAYED_AUDIO);
                relayed.push(seat.place);
                relayed.extend_from_slice(body);
                self.to_team(&utterance, session, &relayed, now, &mut out);
                let mut tap_out = RelayOut::default();
                self.tap.audio(&utterance, seq, &frames, &mut tap_out);
                self.send_texts(tap_out, now, &mut out);
            }
            Datagram::End { utt, .. } => {
                let open = self.talkers[&session]
                    .open
                    .as_ref()
                    .is_some_and(|open| open.utterance.utt == utt);
                if open {
                    self.close(session, EndReason::Released, now, &mut out);
                }
            }
        }
        out
    }

    fn start(
        &mut self,
        session: u32,
        seat: VoiceSeat,
        utt: u16,
        now: Instant,
        out: &mut Vec<(SocketAddr, Vec<u8>)>,
    ) {
        let talker = &self.talkers[&session];
        if let Some(open) = &talker.open {
            if open.utterance.utt == utt {
                return;
            }
            // One open utterance a talker: a new one ends the last.
            self.close(session, EndReason::Released, now, out);
        }
        if self.talkers[&session].closed == Some(utt) {
            return;
        }
        let utterance = Utterance {
            match_id: seat.match_id,
            team: seat.team,
            speaker: seat.place,
            utt,
            started_at: now,
        };
        let mut relayed = header(kind::RELAYED_START);
        relayed.push(seat.place);
        relayed.extend_from_slice(&utt.to_le_bytes());
        relayed.extend_from_slice(&self.server_ms(now).to_le_bytes());
        self.to_team(&utterance, session, &relayed, now, out);
        self.counts.entry(seat.match_id).or_default().utterances += 1;
        let mut tap_out = RelayOut::default();
        self.tap.started(&utterance, &mut tap_out);
        if let Some(talker) = self.talkers.get_mut(&session) {
            talker.open = Some(Open {
                utterance,
                last_audio_at: now,
            });
        }
        self.send_texts(tap_out, now, out);
    }

    fn close(
        &mut self,
        session: u32,
        reason: EndReason,
        now: Instant,
        out: &mut Vec<(SocketAddr, Vec<u8>)>,
    ) {
        let Some(talker) = self.talkers.get_mut(&session) else {
            return;
        };
        let Some(open) = talker.open.take() else {
            return;
        };
        talker.closed = Some(open.utterance.utt);
        let utterance = open.utterance;
        let counts = self.counts.entry(utterance.match_id).or_default();
        counts.talked += open
            .last_audio_at
            .saturating_duration_since(utterance.started_at);
        let mut relayed = header(kind::RELAYED_END);
        relayed.push(utterance.speaker);
        relayed.extend_from_slice(&utterance.utt.to_le_bytes());
        relayed.push(reason as u8);
        self.to_team(&utterance, session, &relayed, now, out);
        let mut tap_out = RelayOut::default();
        self.tap.ended(&utterance, reason, &mut tap_out);
        self.send_texts(tap_out, now, out);
    }

    /// `datagram` to every teammate of the utterance's speaker that said
    /// HELLO lately and hears, but the speaker.
    fn to_team(
        &self,
        utterance: &Utterance,
        speaker: u32,
        datagram: &[u8],
        now: Instant,
        out: &mut Vec<(SocketAddr, Vec<u8>)>,
    ) {
        for (session, listener) in &self.listeners {
            if *session != speaker
                && listener.hears
                && listener.seat.match_id == utterance.match_id
                && listener.seat.team == utterance.team
                && listener.seat.place != utterance.speaker
                && !muted(listener.muted, utterance.speaker)
                && now.saturating_duration_since(listener.last_hello) <= HELLO_FRESH
            {
                out.push((listener.addr, datagram.to_vec()));
            }
        }
    }

    fn send_texts(&self, tap_out: RelayOut, now: Instant, out: &mut Vec<(SocketAddr, Vec<u8>)>) {
        for (utterance, last, lang, text) in tap_out.texts {
            let mut datagram = header(kind::TEXT);
            datagram.push(utterance.speaker);
            datagram.extend_from_slice(&utterance.utt.to_le_bytes());
            datagram.push(u8::from(last));
            datagram.extend_from_slice(&lang);
            datagram.push(text.len() as u8);
            datagram.extend_from_slice(text.as_bytes());
            // Subtitles are for everyone of the team, the speaker too.
            self.to_team(&utterance, u32::MAX, &datagram, now, out);
            if let Some((_, listener)) = self.listeners.iter().find(|(_, listener)| {
                listener.seat.match_id == utterance.match_id
                    && listener.seat.place == utterance.speaker
            }) {
                out.push((listener.addr, datagram));
            }
        }
    }

    /// The time-outs and cuts due at `now`, and once a second the seats
    /// checked again and each match that is over logged and forgotten.
    pub fn poll(&mut self, now: Instant) -> Vec<(SocketAddr, Vec<u8>)> {
        let mut out = Vec::new();
        if now >= self.next_housekeeping {
            self.next_housekeeping = now + HOUSEKEEPING;
            self.housekeeping(now, &mut out);
        }
        let due: Vec<(u32, EndReason)> = self
            .talkers
            .iter()
            .filter_map(|(session, talker)| {
                let open = talker.open.as_ref()?;
                if now.saturating_duration_since(open.utterance.started_at) >= LONGEST {
                    Some((*session, EndReason::TooLong))
                } else if now.saturating_duration_since(open.last_audio_at) >= SILENCE {
                    Some((*session, EndReason::Timeout))
                } else {
                    None
                }
            })
            .collect();
        for (session, reason) in due {
            self.close(session, reason, now, &mut out);
        }
        out
    }

    fn housekeeping(&mut self, now: Instant, out: &mut Vec<(SocketAddr, Vec<u8>)>) {
        // A listener whose place is gone or changed hears no more.
        let seats = &self.seats;
        self.listeners.retain(|session, listener| {
            now.saturating_duration_since(listener.last_hello) <= HELLO_FRESH
                && seats
                    .voice_seat(*session)
                    .is_some_and(|seat| seat == listener.seat)
        });
        // A talker without a seat: its utterance ends, and it is forgotten.
        let gone: Vec<u32> = self
            .talkers
            .keys()
            .copied()
            .filter(|session| self.seats.voice_seat(*session).is_none())
            .collect();
        for session in gone {
            self.close(session, EndReason::Timeout, now, out);
            self.talkers.remove(&session);
        }
        let over: Vec<u32> = self
            .counts
            .keys()
            .copied()
            .filter(|match_id| !self.seats.is_open(*match_id))
            .collect();
        for match_id in over {
            if let Some(counts) = self.counts.remove(&match_id) {
                log::info!(
                    "Match {match_id:#x}: voice: {} utterances, {} s talked, {} datagrams dropped",
                    counts.utterances,
                    counts.talked.as_secs(),
                    counts.dropped
                );
            }
        }
    }
}

/// Whether srp runs the voice relay (`SRP_VOICE`, default on).
pub fn enabled_from_env() -> bool {
    super::parse_switch(
        "SRP_VOICE",
        std::env::var("SRP_VOICE").ok().as_deref(),
        true,
    )
}

/// The voice port of a match server on `match_port`: the next one. The
/// games send to it; it has no setting of its own.
pub fn port_for(match_port: u16) -> Option<u16> {
    match_port.checked_add(1)
}

/// The relay on `socket` until `stop` is set. Returns only on a socket error.
pub fn run(
    socket: &UdpSocket,
    seats: impl Seats,
    id_key: [u8; 32],
    stop: &AtomicBool,
) -> io::Result<()> {
    socket.set_read_timeout(Some(RECEIVE_WAIT))?;
    log::info!("Voice relay on UDP {}", socket.local_addr()?);
    let mut relay = Relay::new(seats, Instant::now()).with_id_key(id_key);
    let mut buffer = [0; 2048];
    while !stop.load(Ordering::Relaxed) {
        let sends = match socket.recv_from(&mut buffer) {
            Ok((len, addr)) => relay.receive(addr, &buffer[..len], Instant::now()),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::ConnectionReset
                ) =>
            {
                Vec::new()
            }
            Err(error) => return Err(error),
        };
        for (addr, datagram) in sends.into_iter().chain(relay.poll(Instant::now())) {
            if let Err(error) = socket.send_to(&datagram, addr) {
                log::debug!("Voice: can't send to {addr}: {error}");
            }
        }
    }
    Ok(())
}

/// The IP of a seat, for tests.
#[cfg(test)]
fn ip(text: &str) -> std::net::IpAddr {
    text.parse().expect("an IP")
}

#[cfg(test)]
mod tests;
