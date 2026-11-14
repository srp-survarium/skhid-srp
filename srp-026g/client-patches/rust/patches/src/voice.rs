//! Patch 66: team voice chat (docs/notes/voice-chat.md). The hooks are
//! `voice.toml`'s:
//!
//! - on_configs_loaded before user.cfg (`configs.start`): the voice's
//!   console variables (the sound tab's stock voice controls, "Disable voice
//!   chat", the mutes);
//! - a live match's connect (`match_client.connect`): the DLL loaded once,
//!   then started on the match server's UDP port + 1 with the session;
//!   at the match's end and when its scene is left (`game_world.finish`,
//!   `game_world.deactivate`) stopped, push-to-talk released first;
//! - every frame of a match (`game_world.after_move`): the options, the
//!   mutes and how each teammate is heard (the radio, or plain in the
//!   world: the engine's own pan of a sound at its place, and a gain by
//!   its distance) told to the DLL when they change, a
//!   held push-to-talk let go when the key is up or the game lost the
//!   keyboard, and the hud's list of who talks;
//! - the game world's keys (`tab_cursor.key`) and mouse buttons
//!   (`game_world.mouse_key`): push-to-talk (`ptt`, P; the binder's chat
//!   kind, listed with the game's keys in the options: patch 62's
//!   `controls`; a key or a button);
//! - the hud's calls (`hud.callback`): the Tab list's player menu (patch 86's)
//!   asks `srp_voice_menu(place)`, answered `root.srp_voice_menu(place,
//!   offered, muted)` (a teammate whose voice id is known), and sends
//!   `srp_voice_mute(place, "1"|"0")`, answered `root.srp_voice_mute(place,
//!   done)`.
//!
//! Nothing a match computes: audio goes beside the game, through the DLL.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use ::ragdoll::math::sqrt;

use crate::vostok::log::{self, Piece::*};
use crate::vostok::types::Place;
use crate::vostok::voice::{
    self as engine, Channel, Connect, Dll, Ears, Heard, Hearing, Listener, MUTES_SIZE, PAN_CENTRE, Places, Roster, Scene, Settings,
    TalkInput, TalkKey, VoiceId, name_text,
};
use crate::vostok::{Context, Flow, GameCell, GameWorldButton, GameWorldKey, HudCall, NoExit, Thread, VoiceOnHudCallExit};

/// Where the plain voice ends (the owner, 2026-11-14: "the voice fall of
/// start immediately and drops completely at 10 meters"): from this
/// distance a speaker's plain voice is not heard.
pub const PLAIN_TO_M: f32 = 10.0;

/// Where the radio starts ("radio starts at 10 too"; 15 m since
/// 2026-11-13, 25 before): from this distance a speaker is heard through
/// the radio. The two may differ: between them, where the plain voice still
/// reaches, both are heard; under both, were this the farther, neither.
pub const RADIO_FROM_M: f32 = 10.0;

/// How much of the plain voice is left at 9 m of what it is beside the
/// listener: 0.7, 3 dB down (the owner: "should be still fine at 9m",
/// "fall off should not be big", "so you always hear normally"). The one
/// number of the fall: 0.5 is 6 dB, 0.35 9 dB, 1 none.
pub const AT_9_M: f32 = 0.7;

/// The plain voice beside the listener and straight ahead plays as decoded
/// in both ears, as the radio does and as revision 58's near voice did: the
/// engine pans a point sound at constant power, 0.707 in each ear ahead (1
/// in one at the side), so its gains are raised by this.
const LEVEL: f32 = core::f32::consts::SQRT_2;

/// The plain voice's gain by its distance: a volume curve as the engine's
/// distance presets have them (`sounds/spl/*.spl`: the gain by metres, read
/// with the distance not under 1 m and held to 0..1, calculate_hdr_audio_frame
/// 0x5C9B5), of two linear keys: 1 to 1 m, [`AT_9_M`] at 9 m, on to
/// [`PLAIN_TO_M`]. With 0.7 it is the stock `weapon_shot_40` and
/// `weapon_echo_close_mid` within 0.4 dB (docs/notes/voice-chat.md).
fn gain(metres: f32) -> f32 {
    let past = if metres > 1.0 { metres - 1.0 } else { 0.0 };
    (1.0 - (1.0 - AT_9_M) * past / (9.0 - 1.0)).clamp(0.0, 1.0)
}

/// What the DLL was last told, so it is told again only on a change.
#[derive(Clone, Copy, PartialEq)]
struct Told {
    settings: Settings,
    muted: Places,
    hearing: Hearing,
}

/// `SKHID_VOICE_LOG` as the game was started with it (`on` or `1`), read at
/// each match connect: on, srp_patches.log gets what the match's voice does
/// (its start, each press of push-to-talk and why it ended, the relay's
/// answer and who talks). Off, nothing: a local diagnostic, never a player's.
#[derive(Clone, Copy, PartialEq)]
enum Trace {
    Off,
    /// With what the log last said of the DLL (`Dll::state`).
    On(Option<(i32, u32)>),
}

const TRACE_VARIABLE: &[u8] = b"SKHID_VOICE_LOG\0";

impl Trace {
    fn read(thread: &Thread) -> Trace {
        let mut value = [0_u8; 8];
        match crate::vostok::memory_profile::environment(thread, TRACE_VARIABLE, &mut value) {
            Some(value) if value.eq_ignore_ascii_case(b"on") || value == b"1" => Trace::On(None),
            _ => Trace::Off,
        }
    }

    fn line(self, pieces: &[log::Piece]) {
        if let Trace::On(_) = self {
            log::line_of(&[&[Text("voice log: ")], pieces]);
        }
    }
}

/// A match with voice.
#[derive(Clone, Copy)]
struct Live {
    mutes: Mutes,
    told: Option<Told>,
    /// The key or button push-to-talk is held by.
    talk: Option<TalkInput>,
    /// The speakers the hud's list was told.
    shown: Option<Places>,
    trace: Trace,
}

#[derive(Clone, Copy)]
enum Voice {
    /// The DLL not looked for yet (before the first live match).
    Unloaded,
    /// No DLL, or one of another ABI: voice stays off.
    Missing,
    Idle(Dll),
    Live(Dll, Live),
}

static VOICE: GameCell<Voice> = GameCell::new(Voice::Unloaded);

/// The muted teammates' voice ids, oldest first: `srp_voice_mutes` in
/// user.cfg, each id 8 hex digits and a `|`. Read once a match; a record not
/// so (a hand-edited or cut file) is left out, never an error.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Mutes {
    ids: [Option<VoiceId>; MAX_MUTES],
}

const RECORD: usize = 9;
const MAX_MUTES: usize = (MUTES_SIZE - 1) / RECORD;

impl Mutes {
    const NONE: Mutes = Mutes { ids: [None; MAX_MUTES] };

    /// The text's mutes and how many records were left out.
    fn parse(text: &[u8]) -> (Mutes, u32) {
        let mut mutes = Mutes::NONE;
        let mut dropped = 0;
        for record in text.split(|&b| b == b'|').filter(|r| !r.is_empty()) {
            match hex_id(record) {
                Some(id) if !mutes.has(id) => mutes.push(id),
                _ => dropped += 1,
            }
        }
        (mutes, dropped)
    }

    fn has(&self, id: VoiceId) -> bool {
        self.ids.contains(&Some(id))
    }

    /// At the end; the oldest goes when it is full.
    fn push(&mut self, id: VoiceId) {
        let used = self.ids.iter().take_while(|id| id.is_some()).count();
        if used == MAX_MUTES {
            self.ids.copy_within(1.., 0);
            self.ids[MAX_MUTES - 1] = Some(id);
        } else {
            self.ids[used] = Some(id);
        }
    }

    fn set(&mut self, id: VoiceId, muted: bool) {
        match (muted, self.has(id)) {
            (true, false) => self.push(id),
            (false, true) => {
                let mut kept = Mutes::NONE;
                for other in self.ids.iter().flatten().filter(|&&other| other != id) {
                    kept.push(*other);
                }
                *self = kept;
            }
            _ => {}
        }
    }

    fn text(&self, out: &mut [u8; MUTES_SIZE]) -> usize {
        let mut len = 0;
        for id in self.ids.iter().flatten() {
            for i in 0..8 {
                out[len + i] = b"0123456789abcdef"[((id.get() >> (28 - 4 * i)) & 15) as usize];
            }
            out[len + 8] = b'|';
            len += RECORD;
        }
        len
    }
}

/// Exactly 8 hex digits, not all zero.
fn hex_id(record: &[u8]) -> Option<VoiceId> {
    if record.len() != 8 {
        return None;
    }
    let mut word = 0u32;
    for &c in record {
        word = word << 4 | char::from(c).to_digit(16)?;
    }
    VoiceId::new(word)
}

fn load_mutes(thread: &Thread) -> Mutes {
    let (mutes, dropped) = Mutes::parse(engine::saved_mutes(thread));
    if dropped != 0 {
        log::line(&[Text("voice: srp_voice_mutes: "), Dec(dropped), Text(" malformed or repeated records left out")]);
    }
    mutes
}

fn save(thread: &Thread, mutes: &Mutes) {
    let mut text = [0; MUTES_SIZE];
    let len = mutes.text(&mut text);
    engine::save_mutes(thread, &text[..len]);
}

pub fn on_configs(ctx: &mut Context<'_>) -> Flow<NoExit> {
    engine::register(ctx.thread());
    Flow::Continue
}

pub fn on_connect(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let connect = Connect::at(ctx);
    let dll = match VOICE.get(thread) {
        Voice::Missing => return Flow::Continue,
        Voice::Unloaded => match Dll::load(thread) {
            Some(dll) => dll,
            None => {
                log::line(&[Text("voice: no skhid_voice.dll of this ABI: voice chat is off")]);
                VOICE.set(thread, Voice::Missing);
                return Flow::Continue;
            }
        },
        Voice::Idle(dll) => dll,
        Voice::Live(dll, _) => {
            dll.talk(false);
            dll.stop();
            dll
        }
    };
    dll.talk(false);
    let Some(connect) = connect else {
        log::line(&[Text("voice: a match connect without a host, or on port 65535: voice chat is off for this match")]);
        VOICE.set(thread, Voice::Idle(dll));
        return Flow::Continue;
    };
    let trace = Trace::read(thread);
    // What the DLL plays and sends by from its start: this game's options, and
    // no mutes and no radio until the match's first frame knows its places (the
    // DLL keeps the last match's, by place, or its defaults; the relay forwards
    // to the seat from its connect, during the loading).
    let told = Told { settings: engine::settings(thread), muted: Places::NONE, hearing: Hearing::UNITY };
    dll.settings(told.settings);
    dll.mute(told.muted);
    dll.hearing(told.hearing, None);
    let started = dll.start(&connect);
    trace.line(&[
        Text("the relay's port "),
        Dec(u32::from(connect.port())),
        Text(", session "),
        Hex(connect.session()),
        Text(if started { ": started" } else { ": the DLL refused the start" }),
    ]);
    let next = if started {
        Voice::Live(dll, Live { mutes: load_mutes(thread), told: Some(told), talk: None, shown: None, trace })
    } else {
        Voice::Idle(dll)
    };
    VOICE.set(thread, next);
    Flow::Continue
}

/// The match's voice stopped, and the hud's list emptied (a name shown when
/// it ended would stay to the next match's reset).
fn stop(thread: &Thread, scene: Option<Scene<'_>>) {
    if let Voice::Live(dll, live) = VOICE.get(thread) {
        dll.talk(false);
        dll.stop();
        if live.shown.is_some_and(|shown| shown != Places::NONE)
            && let Some(scene) = scene
        {
            show_speakers(&scene, &Roster::EMPTY, Places::NONE);
        }
        VOICE.set(thread, Voice::Idle(dll));
    }
}

pub fn on_finish(ctx: &mut Context<'_>) -> Flow<NoExit> {
    stop(ctx.thread(), Scene::at_finish(ctx));
    Flow::Continue
}

pub fn on_deactivate(ctx: &mut Context<'_>) -> Flow<NoExit> {
    stop(ctx.thread(), Scene::at_deactivate(ctx));
    Flow::Continue
}

/// How each mate (a teammate; in a free-for-all, any other player) is
/// heard: through the radio when either of the two is not alive in the
/// world (dead, a spectator); else by how far it is ([`in_the_world`]).
/// `pan` is the engine's, of a sound at an offset from the listener. The
/// other places are not heard at all: left plain, as decoded.
fn hearing(roster: &Roster, pan: impl Fn([f32; 3]) -> [f32; 2]) -> Hearing {
    let me = roster.local().and_then(|me| roster.seat(me)).and_then(|seat| seat.body);
    roster.mates().fold(Hearing::UNITY, |hearing, (place, seat)| {
        let heard = match (me, seat.body) {
            (Some(me), Some(them)) => {
                let offset = [them.feet[0] - me.feet[0], them.feet[1] - me.feet[1], them.feet[2] - me.feet[2]];
                in_the_world(offset, PLAIN_TO_M, RADIO_FROM_M, &pan)
            }
            _ => Heard::Radio,
        };
        hearing.with(place, heard)
    })
}

/// How a speaker `offset` from the listener (feet to feet, so one on the
/// same floor is at the ears' height) is heard, both alive: nearer than
/// `plain_to`, plain, by the engine's pan of a sound there and [`gain`];
/// from `radio_from` on, through the radio. A distance that is not a number
/// (a broken place): the radio.
fn in_the_world(offset: [f32; 3], plain_to: f32, radio_from: f32, pan: &impl Fn([f32; 3]) -> [f32; 2]) -> Heard {
    let squared = offset[0] * offset[0] + offset[1] * offset[1] + offset[2] * offset[2];
    if !squared.is_finite() {
        return Heard::Radio;
    }
    let metres = sqrt(squared);
    let radio = metres >= radio_from;
    if metres >= plain_to {
        return if radio { Heard::Radio } else { Heard::Plain(Ears::SILENT) };
    }
    let level = LEVEL * gain(metres);
    let [left, right] = pan(offset);
    let ears = Ears::new(left * level, right * level);
    if radio { Heard::Both(ears) } else { Heard::Plain(ears) }
}

/// Whom the local player hears and is heard by, by the match's mode: in a
/// free-for-all (patch 142's) every player of the match, else its team. The
/// one place the client decides it; srp's relay routes the same way.
fn channel(thread: &Thread) -> Channel {
    if crate::free_for_all::running(thread) { Channel::Match } else { Channel::Team }
}

/// The mates (teammates; in a free-for-all, everyone) whose voice id is muted.
fn muted(dll: Dll, roster: &Roster, mutes: &Mutes) -> Places {
    roster.mates().fold(Places::NONE, |muted, (place, _)| {
        if dll.voice_id(place).is_some_and(|id| mutes.has(id)) { muted.with(place) } else { muted }
    })
}

pub fn on_frame(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(scene) = Scene::at_frame(ctx) else { return Flow::Continue };
    let thread = scene.thread();
    let Voice::Live(dll, mut live) = VOICE.get(thread) else { return Flow::Continue };
    let settings = engine::settings(thread);
    let speaking = matches!(settings, Settings::On { microphone: true, .. });
    if let Some(key) = live.talk {
        let ended = if !speaking {
            Some("the microphone is off in the options")
        } else if !dll.focused() {
            Some("the game is not the foreground window")
        } else if !dll.key_down(key) {
            Some("its key is up (GetAsyncKeyState)")
        } else {
            None
        };
        if let Some(why) = ended {
            dll.talk(false);
            live.talk = None;
            live.trace.line(&[Text("push-to-talk let go: "), Text(why)]);
        }
    }
    if let Trace::On(said) = live.trace {
        let state = dll.state();
        if said != Some(state) {
            live.trace.line(&[
                Text(if state.0 < 0 { "no answer of the relay yet, place " } else { "the relay's place " }),
                Dec(state.0 as u32),
                Text(", talking "),
                Hex(state.1),
            ]);
            live.trace = Trace::On(Some(state));
        }
    }
    let roster = scene.roster(true, channel(thread));
    // No listener yet (the world's first tick): the engine's centre.
    let listener = Listener::at_frame(ctx);
    let pan = |offset| listener.as_ref().map_or(PAN_CENTRE, |listener| listener.pan(offset));
    let told = Told { settings, muted: muted(dll, &roster, &live.mutes), hearing: hearing(&roster, pan) };
    let before = live.told;
    if before.map(|t| t.settings) != Some(told.settings) {
        dll.settings(told.settings);
    }
    if before.map(|t| t.muted) != Some(told.muted) {
        dll.mute(told.muted);
    }
    dll.hearing(told.hearing, before.map(|t| t.hearing));
    live.told = Some(told);
    let talking = match settings {
        Settings::Off => Places::NONE,
        Settings::On { .. } => dll.talking(),
    };
    // The local player's own place too, while its microphone is sent (the
    // owner, 2026-11-11: "when I speak I want to see this with an icon").
    let own = roster.local();
    let speakers = Place::all()
        .filter(|&p| talking.has(p) && (roster.mate(p) || own == Some(p)))
        .fold(Places::NONE, Places::with)
        .without(told.muted);
    if live.shown != Some(speakers) && show_speakers(&scene, &roster, speakers) {
        live.shown = Some(speakers);
    }
    VOICE.set(thread, Voice::Live(dll, live));
    Flow::Continue
}

/// The hud's list (`root.srp_voice_talking`): the speakers' names (as 0.69's
/// list, the teammates; the local player's own while it is sent).
fn show_speakers(scene: &Scene<'_>, roster: &Roster, speakers: Places) -> bool {
    let Some(movie) = scene.movie() else { return false };
    let mut names: [&CStr; Place::COUNT] = [c""; Place::COUNT];
    let mut n = 0;
    for seat in Place::all().filter(|&p| speakers.has(p)).filter_map(|p| roster.seat(p)) {
        names[n] = name_text(&seat.name);
        n += 1;
    }
    movie.invoke_strings(c"root.srp_voice_talking", &names[..n])
}

pub fn on_key(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(key) = GameWorldKey::at_start(ctx) else { return Flow::Continue };
    let Some(event) = TalkKey::of(&key) else { return Flow::Continue };
    talk_key(key.thread(), event);
    Flow::Continue
}

/// A mouse button of the game world (`game_world.mouse_key`): push-to-talk
/// bound to one. The button goes on to the game as before.
pub fn on_mouse_key(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(button) = GameWorldButton::at_start(ctx) else { return Flow::Continue };
    let Some(event) = TalkKey::of_button(&button) else { return Flow::Continue };
    talk_key(ctx.thread(), event);
    Flow::Continue
}

fn talk_key(thread: &Thread, event: TalkKey) {
    let Voice::Live(dll, mut live) = VOICE.get(thread) else { return };
    match event {
        TalkKey::Down(code) => {
            if live.talk.is_none() {
                if matches!(engine::settings(thread), Settings::On { microphone: true, .. }) {
                    dll.talk(true);
                    live.talk = Some(code);
                    live.trace.line(&[Text("push-to-talk down")]);
                } else {
                    live.trace.line(&[Text("push-to-talk down, not taken: voice or the microphone is off in the options")]);
                }
            }
        }
        TalkKey::Up(code) => {
            if live.talk == Some(code) {
                dll.talk(false);
                live.talk = None;
                live.trace.line(&[Text("push-to-talk up")]);
            }
        }
    }
    VOICE.set(thread, Voice::Live(dll, live));
}

/// "1" or "0" from a movie.
fn choice(text: &CStr) -> Option<bool> {
    match text.to_bytes() {
        b"1" => Some(true),
        b"0" => Some(false),
        _ => None,
    }
}

/// The Tab list's player menu's voice calls. `srp_voice_menu(place)`: a
/// teammate whose voice id is known is offered, with its state;
/// `srp_voice_mute(place, "1"|"0")`: that teammate muted or not, kept in
/// user.cfg. A malformed argument changes nothing (logged); both are taken
/// here and always answered, so the menu never waits.
pub fn on_hud_call(ctx: &mut Context<'_>) -> Flow<VoiceOnHudCallExit> {
    let Some(call) = HudCall::at(ctx) else { return Flow::Continue };
    let menu = call.method() == c"srp_voice_menu";
    if !menu && call.method() != c"srp_voice_mute" {
        return Flow::Continue;
    }
    let thread = call.thread();
    let Some(place) = call.string_argument(0).and_then(|t| Place::parse(t.to_bytes())) else {
        log::line(&[Text("voice: a hud call without a place: "), Text(if menu { "srp_voice_menu" } else { "srp_voice_mute" })]);
        return Flow::Exit(VoiceOnHudCallExit::Done);
    };
    let number = u32::from(place.get());
    let live = match VOICE.get(thread) {
        Voice::Live(dll, live) => Some((dll, live)),
        _ => None,
    };
    let id = live.and_then(|(dll, _)| {
        let roster = Scene::at_call(&call).roster(false, channel(thread));
        dll.voice_id(place).filter(|_| roster.mate(place))
    });
    if menu {
        match (live, id) {
            (Some((_, live)), Some(id)) => call.answer(c"root.srp_voice_menu", &[number, 1, u32::from(live.mutes.has(id))]),
            _ => call.answer(c"root.srp_voice_menu", &[number, 0, 0]),
        };
        return Flow::Exit(VoiceOnHudCallExit::Done);
    }
    let muted = call.string_argument(1).and_then(choice);
    let done = match (live, id, muted) {
        (Some((dll, mut live)), Some(id), Some(muted)) => {
            live.mutes.set(id, muted);
            save(thread, &live.mutes);
            VOICE.set(thread, Voice::Live(dll, live));
            true
        }
        (_, _, None) => {
            log::line(&[Text("voice: srp_voice_mute without a choice of 1 or 0")]);
            false
        }
        _ => false,
    };
    call.answer(c"root.srp_voice_mute", &[number, u32::from(done)]);
    Flow::Exit(VoiceOnHudCallExit::Done)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vostok::voice::{Body, Seat};

    fn id(n: u32) -> VoiceId {
        VoiceId::new(n).unwrap()
    }

    #[test]
    fn mutes_round_trip_and_drop_the_oldest_when_full() {
        let mut m = Mutes::NONE;
        m.set(id(1), true);
        m.set(id(2), true);
        m.set(id(1), false);
        assert!(!m.has(id(1)) && m.has(id(2)));
        for n in 3..100 {
            m.set(id(n), true);
        }
        assert!(!m.has(id(2)) && m.has(id(99)));
        let mut text = [0; MUTES_SIZE];
        let len = m.text(&mut text);
        assert!(len < MUTES_SIZE);
        assert_eq!(Mutes::parse(&text[..len]), (m, 0));
    }

    #[test]
    fn malformed_saved_mutes_are_left_out() {
        let (m, dropped) = Mutes::parse(b"0000000a|zzzzzzzz|00000000|123|0000000a|0000000B||ffffffff");
        assert_eq!(dropped, 4);
        assert!(m.has(id(0xa)) && m.has(id(0xb)) && m.has(id(0xffff_ffff)));
        assert_eq!(Mutes::parse(&[0xFF; 600]).0, Mutes::NONE);
        assert_eq!(Mutes::parse(b"").1, 0);
    }

    fn roster(seats: &[(u32, Option<[f32; 3]>)]) -> Roster {
        let mut r = Roster::EMPTY;
        for (n, &(team, feet)) in seats.iter().enumerate() {
            let body = feet.map(|feet| Body { feet });
            r.set(Place::new(n as u32).unwrap(), Seat { name: [0; 64], team, body });
        }
        r.set_local(Place::new(0));
        r
    }

    fn p(n: u32) -> Place {
        Place::new(n).unwrap()
    }

    /// A stand-in for the engine's pan: the offset's X and Z, so a test
    /// sees which offset it was asked for.
    fn pan(offset: [f32; 3]) -> [f32; 2] {
        [offset[0], offset[2]]
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn plain_under_10_m_the_radio_from_it_and_for_the_dead() {
        let r = roster(&[
            (0, Some([5.0, 1.0, 5.0])),
            (0, Some([5.0, 1.0, 14.9])),
            (0, Some([5.0, 1.0, 15.0])),
            (1, None),
            (0, None),
            (0, Some([5.0, 1.0, 55.0])),
        ]);
        let heard = hearing(&r, pan);
        assert!(matches!(heard.of(p(1)), Heard::Plain(ears) if ears != Ears::SILENT));
        assert_eq!(heard.of(p(2)), Heard::Radio);
        assert_eq!(heard.of(p(4)), Heard::Radio);
        assert_eq!(heard.of(p(5)), Heard::Radio);
        // Not teammates (the other team, the local player): not heard, left plain.
        assert_eq!(heard.of(p(3)), Heard::Plain(Ears::UNITY));
        assert_eq!(heard.of(p(0)), Heard::Plain(Ears::UNITY));
        let dead = roster(&[(0, None), (0, Some([1.0, 0.0, 0.0]))]);
        assert_eq!(hearing(&dead, pan).of(p(1)), Heard::Radio);
        let broken = roster(&[(0, Some([0.0; 3])), (0, Some([f32::NAN, 0.0, 0.0]))]);
        assert_eq!(hearing(&broken, pan).of(p(1)), Heard::Radio);
    }

    #[test]
    fn a_plain_voice_has_the_engines_pan_of_its_offset_and_falls_gently() {
        // The pan is asked of the speaker's offset from the listener, feet to feet.
        let r = roster(&[(0, Some([10.0, 2.0, 10.0])), (0, Some([10.5, 2.0, 10.25]))]);
        let Heard::Plain(ears) = hearing(&r, pan).of(p(1)) else { panic!("near") };
        // Under 1 m the gain is whole: the pan times the level.
        let level = gain(1.0) * LEVEL;
        assert!(close(ears.get()[0], 0.5 * level) && close(ears.get()[1], 0.25 * level), "{ears:?}");
        // Ahead beside the listener, as decoded in both ears.
        let ahead = |metres: f32| match in_the_world([0.0, 0.0, metres], PLAIN_TO_M, RADIO_FROM_M, &|_| PAN_CENTRE) {
            Heard::Plain(ears) => ears.get(),
            other => panic!("{metres}: {other:?}"),
        };
        assert!(close(ahead(0.0)[0], 1.0) && close(ahead(1.0)[1], 1.0));
        // The fall: none to 1 m, AT_9_M at 9 m, linear between, on to the end.
        assert!(close(gain(0.3), 1.0) && close(gain(1.0), 1.0) && close(gain(9.0), AT_9_M));
        assert!(close(gain(5.0), (1.0 + AT_9_M) / 2.0) && gain(9.99) < AT_9_M && gain(9.99) > AT_9_M - 0.1);
        assert!(close(ahead(9.0)[0], AT_9_M) && close(ahead(9.0)[1], AT_9_M));
        // A pan that is not a number is silent, never noise.
        assert_eq!(in_the_world([1.0, 0.0, 0.0], 10.0, 10.0, &|_| [f32::NAN; 2]), Heard::Plain(Ears::SILENT));
    }

    #[test]
    fn the_two_distances_may_differ() {
        let at = |metres: f32, plain_to: f32, radio_from: f32| in_the_world([metres, 0.0, 0.0], plain_to, radio_from, &|_| PAN_CENTRE);
        // As set: one switch.
        assert!(matches!(at(9.99, 10.0, 10.0), Heard::Plain(_)));
        assert_eq!(at(10.0, 10.0, 10.0), Heard::Radio);
        // The plain voice reaching past the radio's start: both between them.
        assert!(matches!(at(9.0, 15.0, 10.0), Heard::Plain(_)));
        assert!(matches!(at(12.0, 15.0, 10.0), Heard::Both(ears) if ears != Ears::SILENT));
        assert_eq!(at(15.0, 15.0, 10.0), Heard::Radio);
        // The radio starting past the plain voice's end: neither between them.
        assert_eq!(at(11.0, 10.0, 12.0), Heard::Plain(Ears::SILENT));
        assert_eq!(at(12.0, 10.0, 12.0), Heard::Radio);
    }

    #[test]
    fn in_a_free_for_all_everyone_is_heard_in_a_team_mode_teammates() {
        use crate::vostok::voice::Channel;
        // A free-for-all's places, each a side of its own (4 + its place).
        let mut ffa = roster(&[(4, Some([0.0; 3])), (5, Some([3.0, 0.0, 0.0])), (6, Some([0.0, 0.0, 40.0])), (7, None)]);
        // As a team mode reads them: sides past the two, nobody's mate.
        assert_eq!(ffa.mates().count(), 0);
        ffa.set_channel(Channel::Match);
        assert_eq!(ffa.mates().map(|(place, _)| place).collect::<Vec<_>>(), [p(1), p(2), p(3)]);
        // The same near and far rule as teammates'.
        let heard = hearing(&ffa, pan);
        assert!(matches!(heard.of(p(1)), Heard::Plain(ears) if ears.get()[0] > 0.0 && ears.get()[1] == 0.0));
        assert_eq!(heard.of(p(2)), Heard::Radio);
        assert_eq!(heard.of(p(3)), Heard::Radio);
        // A team mode: the teammates only.
        let teams = roster(&[(0, None), (0, None), (1, None)]);
        assert_eq!(teams.mates().map(|(place, _)| place).collect::<Vec<_>>(), [p(1)]);
    }
}
