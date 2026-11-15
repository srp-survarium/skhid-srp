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
//!   mutes and how each other player is heard (plain in the world near: the
//!   engine's own pan of a sound at its place, and a gain by its distance;
//!   a teammate on the radio far or dead, an opponent not at all) told to the
//!   DLL when they change, a
//!   held push-to-talk let go when the key is up or the game lost the
//!   keyboard, and the hud's list of who talks;
//! - the game world's keys (`tab_cursor.key`) and mouse buttons
//!   (`game_world.mouse_key`): push-to-talk (`ptt`, P; the binder's chat
//!   kind, listed with the game's keys in the options: patch 62's
//!   `controls`; a key or a button);
//! - the options' test of a key two actions share (`options.key_conflict`):
//!   push-to-talk counts as heard with the player's keys too, so binding it
//!   onto a game key, or a game key onto it, says the key is in use;
//! - the hud's calls (`hud.callback`): the Tab list's player menu (patch 86's)
//!   asks `srp_voice_menu(place)`, answered `root.srp_voice_menu(place,
//!   offered, muted)` (another player of the match, of either team, whose
//!   voice id is known), and sends
//!   `srp_voice_mute(place, "1"|"0")`, answered `root.srp_voice_mute(place,
//!   done)`;
//! - the options' microphone test (the owner, 2026-11-14; the DLL's, in a
//!   match or not: voice-chat.md, "Microphone test"): the Settings' sound tab
//!   asks `srp_mic_test` (`options.callback`), answered `root.srp_mic_test(
//!   text)`, its button's text now, and sends `srp_mic_test("<volume>
//!   <sensitivity>")`, a press with the two sliders as they stand, or
//!   `srp_mic_test("stop")` as the Settings window closes; every frame of the
//!   game (`game.tick`) the button is told its text again when the DLL's
//!   state of the test, or its second, is no longer the one shown; the test
//!   is also stopped when the Settings change tab and when the main menu
//!   closes (`options.deactivate`: also a match's loading and its end);
//! - the options' Microphone list (voice-chat.md, "Microphone"): the sound tab
//!   asks `srp_microphone` (`options.callback`) as it is shown and as the list
//!   is opened, answered `root.srp_microphone(label, default, shown, id, name,
//!   ...)`, the capture devices the DLL finds now; a pick in the list sends
//!   `srp_microphone_use(id)` (empty: Windows' default), told to the DLL at
//!   once and not saved, and Cancel or leaving without Apply
//!   `srp_microphone_use` with nothing, the saved one again; Apply sends
//!   `srp_microphone(id)`, kept in user.cfg (`srp_microphone`) and told to
//!   the DLL, which is told the saved one as it is loaded.
//!
//! Nothing a match computes: audio goes beside the game, through the DLL.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use ::ragdoll::math::sqrt;

use crate::vostok::log::{self, Piece::*};
use crate::vostok::settings::{KeyTest, KeyedAction, Menu, OptionsCall};
use crate::vostok::types::Place;
use crate::vostok::voice::{
    self as engine, Channel, Connect, Dll, Ears, Heard, Hearing, Listener, MUTES_SIZE, MicTest, Microphone, MicrophoneChoice,
    MicrophoneId, PAN_CENTRE, Places, Roster, Scene, Settings, Side, TalkInput, TalkKey, TestAsk, VoiceId, name_text,
};
use crate::vostok::{
    Action, ActionKinds, Context, Flow, GameCell, GameWorldButton, GameWorldKey, HudCall, NoExit, Thread,
    VoiceOnHudCallExit, VoiceOnKeyConflictExit, VoiceOnOptionsCallExit,
};

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
    shown: Option<Speakers>,
    trace: Trace,
}

#[derive(Clone, Copy)]
enum Voice {
    /// The DLL not looked for yet (before the first live match and the
    /// Settings' first showing of their sound tab).
    Unloaded,
    /// No DLL, or one of another ABI: voice stays off.
    Missing,
    Idle(Dll),
    Live(Dll, Live),
}

static VOICE: GameCell<Voice> = GameCell::new(Voice::Unloaded);

/// The DLL, looked for now when it was not yet; None without one of this ABI
/// (said once in the log). Loaded, it is told the saved microphone.
fn dll(thread: &Thread) -> Option<Dll> {
    match VOICE.get(thread) {
        Voice::Unloaded => {
            let dll = Dll::load(thread);
            match dll {
                Some(dll) => dll.choose_microphone(engine::saved_microphone(thread)),
                None => log::line(&[Text("voice: no skhid_voice.dll of this ABI: voice chat is off")]),
            }
            VOICE.set(thread, dll.map_or(Voice::Missing, Voice::Idle));
            dll
        }
        Voice::Missing => None,
        Voice::Idle(dll) | Voice::Live(dll, _) => Some(dll),
    }
}

/// The DLL when it is loaded.
fn loaded(thread: &Thread) -> Option<Dll> {
    match VOICE.get(thread) {
        Voice::Idle(dll) | Voice::Live(dll, _) => Some(dll),
        Voice::Unloaded | Voice::Missing => None,
    }
}

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
    let Some(dll) = dll(thread) else { return Flow::Continue };
    if let Voice::Live(..) = VOICE.get(thread) {
        dll.talk(false);
        dll.stop();
    }
    dll.talk(false);
    // A match loads: a microphone test with no match ends (the start below
    // ends its worker too).
    dll.test_stop();
    let Some(connect) = connect else {
        log::line(&[Text("voice: a match connect without a host, or on port 65535: voice chat is off for this match")]);
        VOICE.set(thread, Voice::Idle(dll));
        return Flow::Continue;
    };
    let trace = Trace::read(thread);
    // What the DLL plays and sends by from its start: this game's options, no
    // mutes, and nobody heard until the match's first frame knows its places and
    // who is near: the relay forwards the whole match to the seat from its
    // connect, during the loading, opponents too.
    let told = Told { settings: engine::settings(thread), muted: Places::NONE, hearing: Hearing::SILENT };
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

/// The match's voice stopped (a microphone test with it), and the hud's list
/// emptied (a name shown when it ended would stay to the next match's reset).
fn stop(thread: &Thread, scene: Option<Scene<'_>>) {
    if let Voice::Idle(dll) = VOICE.get(thread) {
        // No voice in this match (a replay, or the start refused): a test over it.
        dll.test_stop();
    }
    if let Voice::Live(dll, live) = VOICE.get(thread) {
        dll.talk(false);
        dll.stop();
        if live.shown.is_some_and(|shown| shown != Speakers::NONE)
            && let Some(scene) = scene
        {
            show_speakers(&scene, &Roster::EMPTY, Speakers::NONE);
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

/// How each other player of the match is heard: through the radio when
/// either of the two is not alive in the world (dead, a spectator); else by
/// how far it is ([`in_the_world`]). The radio is a mate's only (a teammate;
/// in a free-for-all, any other player): an opponent is heard plain near, as a
/// mate is, and not at all where a mate would be on the radio ([`side`]; the
/// owner, 2026-11-14: "in all regimes everyone in 10meters + opponenets
/// should hear it", "only radio is team only"). `pan` is the engine's, of a
/// sound at an offset from the listener. The places of nobody in the match,
/// and the local player's own, are silent.
fn hearing(roster: &Roster, pan: impl Fn([f32; 3]) -> [f32; 2]) -> Hearing {
    let me = roster.local().and_then(|me| roster.seat(me)).and_then(|seat| seat.body);
    roster.others().fold(Hearing::SILENT, |hearing, (place, seat, side_of)| {
        let heard = match (me, seat.body) {
            (Some(me), Some(them)) => {
                let offset = [them.feet[0] - me.feet[0], them.feet[1] - me.feet[1], them.feet[2] - me.feet[2]];
                in_the_world(offset, PLAIN_TO_M, RADIO_FROM_M, &pan)
            }
            _ => Heard::Radio,
        };
        hearing.with(place, side(side_of, heard))
    })
}

/// How a player of `side` is heard where a mate is heard `heard`: a mate
/// so, an opponent without the radio (plain near; silent far, or dead).
fn side(side: Side, heard: Heard) -> Heard {
    match (side, heard) {
        (Side::Mate, heard) => heard,
        (Side::Opponent, Heard::Plain(ears) | Heard::Both(ears)) => Heard::Plain(ears),
        (Side::Opponent, Heard::Radio) => Heard::Plain(Ears::SILENT),
    }
}

/// Whether a place heard so is heard at all (plain, the radio or both).
fn audible(heard: Heard) -> bool {
    heard != Heard::Plain(Ears::SILENT)
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

/// Who the local player's mates are (heard on the radio too), by the match's
/// mode: in a free-for-all (patch 142's) every player of the match, else its
/// team. The one place it is decided; srp's relay sends the whole match.
fn channel(thread: &Thread) -> Channel {
    if crate::free_for_all::running(thread) { Channel::Match } else { Channel::Team }
}

/// The other players, of either team, whose voice id is muted.
fn muted(dll: Dll, roster: &Roster, mutes: &Mutes) -> Places {
    roster.others().fold(Places::NONE, |muted, (place, _, _)| {
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
    let speakers = speakers(talking, &told.hearing, &roster, told.muted);
    if live.shown != Some(speakers) && show_speakers(&scene, &roster, speakers) {
        live.shown = Some(speakers);
    }
    VOICE.set(thread, Voice::Live(dll, live));
    Flow::Continue
}

/// The speakers the hud's list shows, each with the [`Side`] its speaker is
/// drawn as: a mate's blue, an opponent's pale red (the owner, 2026-11-15);
/// the local player's own as a mate's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Speakers([Option<Side>; Place::COUNT]);

impl Speakers {
    const NONE: Speakers = Speakers([None; Place::COUNT]);

    fn iter(&self) -> impl Iterator<Item = (Place, Side)> + '_ {
        Place::all().filter_map(|p| Some((p, self.0[p.index()]?)))
    }
}

/// Who the hud's list shows of those `talking`: the players heard (a mate
/// always, an opponent while it is near: [`hearing`]), and the local player's
/// own place while its microphone is sent (the owner, 2026-11-11: "when I
/// speak I want to see this with an icon"); none `muted`.
fn speakers(talking: Places, hearing: &Hearing, roster: &Roster, muted: Places) -> Speakers {
    let mut shown = Speakers::NONE;
    for p in Place::all().filter(|&p| talking.without(muted).has(p)) {
        shown.0[p.index()] = match roster.side(p) {
            Some(side) => audible(hearing.of(p)).then_some(side),
            None => (roster.local() == Some(p)).then_some(Side::Mate),
        };
    }
    shown
}

/// The hud's list (`root.srp_voice_talking`): each speaker's name and side,
/// `"mate"` or `"opponent"` (as 0.69's list; the local player's own while it
/// is sent).
fn show_speakers(scene: &Scene<'_>, roster: &Roster, speakers: Speakers) -> bool {
    let Some(movie) = scene.movie() else { return false };
    let mut texts: [&CStr; 2 * Place::COUNT] = [c""; 2 * Place::COUNT];
    let mut n = 0;
    for (seat, side) in speakers.iter().filter_map(|(p, side)| Some((roster.seat(p)?, side))) {
        texts[n] = name_text(&seat.name);
        texts[n + 1] = match side {
            Side::Mate => c"mate",
            Side::Opponent => c"opponent",
        };
        n += 2;
    }
    movie.invoke_strings(c"root.srp_voice_talking", &texts[..n])
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

/// The Tab list's player menu's voice calls. `srp_voice_menu(place)`: another
/// player of the match, of either team (an opponent is heard near), whose
/// voice id is known is offered, with its state;
/// `srp_voice_mute(place, "1"|"0")`: that player muted or not, kept in
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
        dll.voice_id(place).filter(|_| roster.side(place).is_some())
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

/// The Settings' call of the microphone test, and its answer's method.
const MIC_TEST: &CStr = c"srp_mic_test";
const MIC_TEST_ANSWER: &CStr = c"root.srp_mic_test";

/// The button's text in a state: its dictionary key, and the seconds left
/// its `%1$s` is.
fn test_text(test: MicTest) -> (&'static CStr, Option<u8>) {
    match test {
        MicTest::Idle => (c"st_srp_mic_test", None),
        MicTest::Recording(seconds) => (c"st_srp_mic_test_recording", Some(seconds)),
        MicTest::Playing(seconds) => (c"st_srp_mic_test_playing", Some(seconds)),
        MicTest::NoMicrophone => (c"st_srp_mic_test_no_microphone", None),
        MicTest::NoPlayback => (c"st_srp_mic_test_no_playback", None),
    }
}

/// The test's state the Settings' button shows: what its movie was last
/// told. None: the sound tab has not asked yet, so there is no button to
/// tell.
static TEST_SHOWN: GameCell<Option<MicTest>> = GameCell::new(None);

/// Tells the Settings' button its text for `test` (the running language's,
/// with the seconds left); whether the movie took it.
fn show_test(menu: &Menu<'_>, test: MicTest) -> bool {
    let (key, seconds) = test_text(test);
    let (mut text, mut numbered) = ([0; 0x200], [0; 0x200]);
    let text = menu.translate(key, &mut text);
    let text = match seconds {
        Some(seconds) => crate::free_for_all::with_number(text.to_bytes(), u16::from(seconds), &mut numbered),
        None => text,
    };
    menu.tell(MIC_TEST_ANSWER, &[text])
}

/// The Settings' call of the Microphone list, and its answer's method.
const MICROPHONE: &CStr = c"srp_microphone";
const MICROPHONE_ANSWER: &CStr = c"root.srp_microphone";
/// The Settings' call of a pick used at once, not saved.
const MICROPHONE_USE: &CStr = c"srp_microphone_use";
/// The most capture devices the list shows (the answer's strings: three and
/// two a device, within the movie call's 64).
const MICROPHONES: usize = 16;

/// The list's pick as the list can show it: the saved device as the DLL
/// lists it now (its ID as listed), else the default (none saved, or the
/// saved one is not there: unplugged).
fn shown(saved: MicrophoneChoice, listed: impl IntoIterator<Item = MicrophoneId>) -> MicrophoneChoice {
    listed
        .into_iter()
        .find(|&id| saved == MicrophoneChoice::Device(id))
        .map_or(MicrophoneChoice::Default, MicrophoneChoice::Device)
}

/// `srp_microphone`: with an ID (or empty, the default) Apply's pick, kept
/// in user.cfg and told to the DLL; with or without, the list is answered:
/// its label, the default's text, the pick it shows ([`shown`]), then each
/// device's ID and name, in Windows' order. Anything else is no pick
/// (logged), answered the same.
fn microphone_call(call: &OptionsCall<'_>, thread: &Thread, dll: Dll) {
    if let Some(text) = call.string_argument(0) {
        match MicrophoneChoice::parse(text.to_bytes()) {
            Some(choice) => {
                engine::save_microphone(thread, choice);
                dll.choose_microphone(choice);
            }
            None => log::line(&[Text("voice: srp_microphone with neither a device's endpoint ID nor the default")]),
        }
    }
    let (mut label, mut default) = ([0; 0x200], [0; 0x200]);
    let label = call.translate(c"st_srp_microphone", &mut label);
    let default = call.translate(c"st_srp_microphone_default", &mut default);
    let mut room = [Microphone::EMPTY; MICROPHONES];
    let mut listed: [Option<(MicrophoneId, &CStr)>; MICROPHONES] = [None; MICROPHONES];
    for (slot, device) in listed.iter_mut().zip(dll.microphones(&mut room)) {
        *slot = Some(device);
    }
    let pick = shown(engine::saved_microphone(thread), listed.iter().flatten().map(|&(id, _)| id));
    let mut values: [&CStr; 3 + 2 * MICROPHONES] = [c""; 3 + 2 * MICROPHONES];
    values[..3].copy_from_slice(&[label, default, pick.text()]);
    let mut count = 3;
    for (id, name) in listed.iter().flatten() {
        values[count] = id.as_c_str();
        values[count + 1] = name;
        count += 2;
    }
    call.answer(MICROPHONE_ANSWER, &values[..count]);
}

/// `srp_microphone_use`: with an ID (or empty, the default) a pick in the
/// list, told to the DLL at once (the test and push-to-talk open it from
/// their next press) and not saved; with nothing the saved pick again
/// (Cancel, the Settings left without Apply, a pick whose device is gone).
/// Anything else is no pick (logged). Not answered.
fn microphone_use_call(call: &OptionsCall<'_>, thread: &Thread, dll: Dll) {
    let choice = match call.string_argument(0) {
        Some(text) => MicrophoneChoice::parse(text.to_bytes()),
        None => Some(engine::saved_microphone(thread)),
    };
    match choice {
        Some(choice) => dll.choose_microphone(choice),
        None => log::line(&[Text("voice: srp_microphone_use with neither a device's endpoint ID nor the default")]),
    }
}

/// The Settings' calls (`options.callback`). `srp_microphone`: the
/// Microphone list ([`microphone_call`]; with no DLL not answered, and the
/// list stays away); `srp_microphone_use`: a pick used at once
/// ([`microphone_use_call`]; with no DLL loaded nothing to tell, the list
/// was never shown). `srp_mic_test`: the sound tab
/// shown asks the button's text (answered at once; with no DLL not at all,
/// and the button stays away); with `<volume> <sensitivity>` it is a press,
/// which the DLL takes (it starts or stops its test: `on_tick` tells the
/// button); with `stop` the Settings window closed. `options_tab_changed`
/// (the game's own, left to it): the button is not seen on another tab, so a
/// test stops.
pub fn on_options_call(ctx: &mut Context<'_>) -> Flow<VoiceOnOptionsCallExit> {
    let Some(call) = OptionsCall::at(ctx) else { return Flow::Continue };
    let thread = ctx.thread();
    if call.method() == c"options_tab_changed" {
        if let Some(dll) = loaded(thread) {
            dll.test_stop();
        }
        return Flow::Continue;
    }
    if call.method() == MICROPHONE {
        if let Some(dll) = dll(thread) {
            microphone_call(&call, thread, dll);
        }
        return Flow::Exit(VoiceOnOptionsCallExit::Done);
    }
    if call.method() == MICROPHONE_USE {
        if let Some(dll) = loaded(thread) {
            microphone_use_call(&call, thread, dll);
        }
        return Flow::Exit(VoiceOnOptionsCallExit::Done);
    }
    if call.method() != MIC_TEST {
        return Flow::Continue;
    }
    let Some(dll) = dll(thread) else { return Flow::Exit(VoiceOnOptionsCallExit::Done) };
    match call.string_argument(0).map(TestAsk::parse) {
        Some(Some(TestAsk::Press(sliders))) => dll.test_press(sliders),
        Some(Some(TestAsk::Stop)) => dll.test_stop(),
        Some(None) => log::line(&[Text("voice: srp_mic_test with neither the two sliders' steps nor stop")]),
        None => {
            let test = dll.test();
            if call.menu().is_some_and(|menu| show_test(&menu, test)) {
                TEST_SHOWN.set(thread, Some(test));
            }
        }
    }
    Flow::Exit(VoiceOnOptionsCallExit::Done)
}

/// Every frame of the game (`game.tick`), in a match or not, while the main
/// menu is shown: the button is told its text when the DLL's test is no
/// longer as the button shows it (its state changed, or a second of it
/// passed; a test the menu's close stopped, when the menu is shown again).
/// The DLL is asked nothing while the menu is closed, or before the sound
/// tab first asked.
pub fn on_tick(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let Some(shown) = TEST_SHOWN.get(thread) else { return Flow::Continue };
    let Some(menu) = Menu::shown_at_game_tick(ctx) else { return Flow::Continue };
    let Some(dll) = loaded(thread) else { return Flow::Continue };
    let test = dll.test();
    if test != shown && show_test(&menu, test) {
        TEST_SHOWN.set(thread, Some(test));
    }
    Flow::Continue
}

/// The main menu closes (`options.deactivate`: its Resume or its key, a
/// match's end, the lobby left for a match's loading): a test stops.
pub fn on_menu_close(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    if TEST_SHOWN.get(thread).is_some()
        && let Some(dll) = loaded(thread)
    {
        dll.test_stop();
    }
    Flow::Continue
}

/// Where an action's key is heard, for the options' test of a key two
/// actions share: push-to-talk's row says the chat keys' kind, which patch 66
/// asks its key with, and the key is heard in a match as the player's keys are
/// (`tab_cursor.key`, before the player's own keys: aiming or not). Any
/// other action where its row says.
fn heard(keyed: KeyedAction) -> ActionKinds {
    if keyed.action == Action::PTT {
        keyed.kinds.with(ActionKinds::PLAYER).with(ActionKinds::AIMING)
    } else {
        keyed.kinds
    }
}

/// Options, Controls: a key pressed for an action that a row of the controls
/// has already (`options.key_conflict`). The row takes it, and the game says
/// the key is in use (its reassign message: OK unbinds the row), when the two
/// are heard in the same place: so push-to-talk and a game key (spot's Q, an
/// item key) take each other's key, as two game keys do; push-to-talk and a
/// lobby key do not. Any other two actions as the game decides.
pub fn on_key_conflict(ctx: &mut Context<'_>) -> Flow<VoiceOnKeyConflictExit> {
    let Some(test) = KeyTest::at(ctx) else { return Flow::Continue };
    if heard(test.bound).meet(heard(test.row)) {
        Flow::Exit(VoiceOnKeyConflictExit::Taken)
    } else {
        Flow::Exit(VoiceOnKeyConflictExit::Free)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vostok::voice::{Body, Seat};

    fn keyed(action: Action, kinds: ActionKinds) -> KeyedAction {
        KeyedAction { action, kinds }
    }

    #[test]
    fn push_to_talk_and_a_game_key_take_each_others_key() {
        let ptt = keyed(Action::PTT, ActionKinds::CHAT);
        let game = ActionKinds::PLAYER.with(ActionKinds::AIMING);
        let spot = keyed(Action::SPOT, game);
        let item = keyed(Action(0x4A), game);
        let accel = keyed(Action(0x07), ActionKinds::PLAYER);
        let delay_breath = keyed(Action(0x2A), ActionKinds::AIMING);
        for other in [spot, item, accel, delay_breath] {
            assert!(heard(ptt).meet(heard(other)), "{other:?} onto push-to-talk's key");
            assert!(heard(other).meet(heard(ptt)), "push-to-talk onto {other:?}'s key");
        }
    }

    #[test]
    fn push_to_talk_and_a_chat_key_still_take_each_others_key_and_a_lobby_key_never() {
        let ptt = keyed(Action::PTT, ActionKinds::CHAT);
        let send_message = keyed(Action(0x45), ActionKinds::CHAT);
        let friends = keyed(Action(0x44), ActionKinds::LOBBY);
        assert!(heard(ptt).meet(heard(send_message)));
        assert!(!heard(ptt).meet(heard(friends)));
        assert!(!heard(friends).meet(heard(ptt)));
    }

    #[test]
    fn other_actions_meet_as_their_rows_say() {
        let game = ActionKinds::PLAYER.with(ActionKinds::AIMING);
        let melee = keyed(Action(0x21), game);
        let friends = keyed(Action(0x44), ActionKinds::LOBBY);
        let send_message = keyed(Action(0x45), ActionKinds::CHAT);
        let accel = keyed(Action(0x07), ActionKinds::PLAYER);
        let delay_breath = keyed(Action(0x2A), ActionKinds::AIMING);
        assert!(heard(melee).meet(heard(accel)));
        assert!(!heard(melee).meet(heard(friends)));
        assert!(!heard(melee).meet(heard(send_message)));
        assert!(!heard(accel).meet(heard(delay_breath)));
    }

    fn id(n: u32) -> VoiceId {
        VoiceId::new(n).unwrap()
    }

    #[test]
    fn each_state_of_the_test_has_its_text_and_the_counted_ones_their_seconds() {
        let states = [MicTest::Idle, MicTest::Recording(7), MicTest::Playing(10), MicTest::NoMicrophone, MicTest::NoPlayback];
        let texts = states.map(test_text);
        assert_eq!(texts.map(|(_, seconds)| seconds), [None, Some(7), Some(10), None, None]);
        for (n, (key, _)) in texts.iter().enumerate() {
            assert!(key.to_bytes().starts_with(b"st_srp_mic_test"));
            assert!(!texts[..n].iter().any(|(other, _)| other == key), "{key:?}");
        }
        let mut out = [0; 0x200];
        assert_eq!(crate::free_for_all::with_number(b"Recording %1$s", 7, &mut out), c"Recording 7");
    }

    #[test]
    fn the_list_shows_the_saved_microphone_while_it_is_there_else_the_default() {
        let device = |text: &str| MicrophoneId::parse(text.as_bytes()).unwrap();
        let (headset, webcam) = (device("{0.0.1.00000000}.{headset}"), device("{0.0.1.00000000}.{webcam}"));
        let saved = MicrophoneChoice::Device(headset);
        assert_eq!(shown(saved, [webcam, headset]), saved);
        // unplugged: the default shown (the pick stays saved for when it is back)
        assert_eq!(shown(saved, [webcam]), MicrophoneChoice::Default);
        assert_eq!(shown(saved, []), MicrophoneChoice::Default);
        assert_eq!(shown(MicrophoneChoice::Default, [webcam, headset]), MicrophoneChoice::Default);
        // as listed: the movie finds the pick by the list's own text
        let listed = device("{0.0.1.00000000}.{HEADSET}");
        assert_eq!(shown(saved, [listed]).text(), listed.as_c_str());
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
        // A dead opponent, the local player, a place of nobody: silent.
        assert_eq!(heard.of(p(3)), Heard::Plain(Ears::SILENT));
        assert_eq!(heard.of(p(0)), Heard::Plain(Ears::SILENT));
        assert_eq!(heard.of(p(19)), Heard::Plain(Ears::SILENT));
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
    fn in_a_free_for_all_everyone_is_a_mate_in_a_team_mode_teammates() {
        use crate::vostok::voice::Channel;
        let sides = |r: &Roster| r.others().map(|(place, _, side)| (place, side)).collect::<Vec<_>>();
        // A free-for-all's places, each a side of its own (4 + its place).
        let mut ffa = roster(&[(4, Some([0.0; 3])), (5, Some([3.0, 0.0, 0.0])), (6, Some([0.0, 0.0, 40.0])), (7, None)]);
        // As a team mode reads them: sides past the two, nobody's mate.
        assert_eq!(sides(&ffa), [(p(1), Side::Opponent), (p(2), Side::Opponent), (p(3), Side::Opponent)]);
        ffa.set_channel(Channel::Match);
        assert_eq!(sides(&ffa), [(p(1), Side::Mate), (p(2), Side::Mate), (p(3), Side::Mate)]);
        // The same near and far rule as teammates': the radio far and dead.
        let heard = hearing(&ffa, pan);
        assert!(matches!(heard.of(p(1)), Heard::Plain(ears) if ears.get()[0] > 0.0 && ears.get()[1] == 0.0));
        assert_eq!(heard.of(p(2)), Heard::Radio);
        assert_eq!(heard.of(p(3)), Heard::Radio);
        // A team mode: the teammates mates, the other team opponents.
        let teams = roster(&[(0, None), (0, None), (1, None)]);
        assert_eq!(sides(&teams), [(p(1), Side::Mate), (p(2), Side::Opponent)]);
        // Nobody's side while the local player's own place is not known.
        let mut unknown = teams;
        unknown.set_local(None);
        assert_eq!(sides(&unknown), []);
    }

    #[test]
    fn an_opponent_is_heard_plain_near_as_a_teammate_and_never_on_the_radio() {
        // The local player (team 0) at the origin; a teammate and an opponent
        // side by side at each distance.
        let at = |metres: f32| Some([metres, 0.0, 0.0]);
        let r = roster(&[(0, at(0.0)), (0, at(4.0)), (1, at(4.0)), (0, at(12.0)), (1, at(12.0)), (0, None), (1, None)]);
        let heard = hearing(&r, pan);
        // Near: the same plain voice, the engine's pan and the fall alike.
        assert!(matches!(heard.of(p(2)), Heard::Plain(ears) if ears != Ears::SILENT));
        assert_eq!(heard.of(p(2)), heard.of(p(1)));
        // Far: the teammate on the radio, the opponent not at all.
        assert_eq!(heard.of(p(3)), Heard::Radio);
        assert_eq!(heard.of(p(4)), Heard::Plain(Ears::SILENT));
        // Dead (or the listener dead): the teammate on the radio, the opponent silent.
        assert_eq!(heard.of(p(5)), Heard::Radio);
        assert_eq!(heard.of(p(6)), Heard::Plain(Ears::SILENT));
        let mut dead = r;
        dead.set(p(0), Seat { name: [0; 64], team: 0, body: None });
        let heard = hearing(&dead, pan);
        assert_eq!(heard.of(p(1)), Heard::Radio);
        assert_eq!(heard.of(p(2)), Heard::Plain(Ears::SILENT));
        // Where a mate would hear both (the two distances apart), an
        // opponent hears the plain voice only.
        let ears = Ears::new(0.5, 0.25);
        assert_eq!(side(Side::Opponent, Heard::Both(ears)), Heard::Plain(ears));
        assert_eq!(side(Side::Mate, Heard::Both(ears)), Heard::Both(ears));
    }

    #[test]
    fn the_list_shows_who_is_heard_talking_and_the_own_place() {
        let at = |metres: f32| Some([metres, 0.0, 0.0]);
        // 0 the local player; 1 a teammate far (the radio); 2 an opponent near; 3 one far.
        let r = roster(&[(0, at(0.0)), (0, at(40.0)), (1, at(3.0)), (1, at(40.0))]);
        let heard = hearing(&r, pan);
        let all = [p(0), p(1), p(2), p(3)].into_iter().fold(Places::NONE, Places::with);
        let shown = |s: Speakers| s.iter().collect::<Vec<_>>();
        // The own place drawn as a mate's; the near opponent as an opponent.
        assert_eq!(
            shown(speakers(all, &heard, &r, Places::NONE)),
            [(p(0), Side::Mate), (p(1), Side::Mate), (p(2), Side::Opponent)]
        );
        // Muted ones are not; nor anyone not talking.
        assert_eq!(shown(speakers(all, &heard, &r, Places::NONE.with(p(2)))), [(p(0), Side::Mate), (p(1), Side::Mate)]);
        assert_eq!(speakers(Places::NONE.with(p(3)), &heard, &r, Places::NONE), Speakers::NONE);
        // In a free-for-all the near one is a mate.
        let mut ffa = r;
        ffa.set_channel(crate::vostok::voice::Channel::Match);
        let heard = hearing(&ffa, pan);
        assert_eq!(shown(speakers(all, &heard, &ffa, Places::NONE)), [(p(0), Side::Mate), (p(1), Side::Mate), (p(2), Side::Mate), (p(3), Side::Mate)]);
    }
}
