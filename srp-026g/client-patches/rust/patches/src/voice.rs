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
//!   mutes and how each other player is heard on each of its two channels
//!   (its nearby voice in the world: the engine's own pan of a sound at its
//!   place, and a gain by its distance, nothing from 12 m or with either
//!   player dead; its radio when it is a mate) told to the DLL when they
//!   change, a
//!   held push-to-talk let go when the key is up or the game lost the
//!   keyboard, and the hud's list of who talks;
//! - the game world's keys (`tab_cursor.key`) and mouse buttons
//!   (`game_world.mouse_key`): the two push-to-talk keys, `ptt` (P) the
//!   radio and `ptt_nearby` (O) the nearby voice (the binder's chat kind,
//!   listed with the game's keys in the options: patch 62's `controls`; a
//!   key or a button);
//! - the hud's calls (`hud.callback`): the Tab list's player menu (patch 86's)
//!   asks `srp_voice_menu(place)`, answered `root.srp_voice_menu(place,
//!   offered, muted)` (another player of the match, of either team, whose
//!   voice id is known), and sends
//!   `srp_voice_mute(place, "1"|"0")`, answered `root.srp_voice_mute(place,
//!   done)`;
//! - the options' microphone test (the owner, 2026-11-14; the DLL's, in a
//!   match or not: voice-chat.md, "Microphone test"): the Settings' sound tab
//!   asks `srp_mic_test` (`options.callback`), answered `root.srp_mic_test(
//!   text)`, its button's text now, and sends `srp_mic_test("<volume>")`,
//!   a press with the "Voice chat" slider as it stands, or
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
use crate::vostok::settings::{Menu, OptionsCall};
use crate::vostok::types::Place;
use crate::vostok::voice::{
    self as engine, Channel, Connect, Dll, Ears, Heard, Hearing, Listener, MUTES_SIZE, Mates, MicTest, Microphone, Muffle,
    MicrophoneChoice, MicrophoneId, PAN_CENTRE, Places, Roster, Scene, Settings, Side, TalkInput, TalkKey, TestAsk, VoiceId,
    name_text,
};
use crate::vostok::{
    Context, Flow, GameCell, GameWorldButton, GameWorldKey, HudCall, NoExit, Thread, VoiceOnHudCallExit,
    VoiceOnOptionsCallExit,
};

/// Where the nearby voice ends: from this distance a speaker's nearby voice
/// is not heard, and it falls to nothing on the way here (the owner,
/// 2026-11-16, of a voice 3 dB down at 9 m and cut at 10: "voice should go
/// from high to zero depending on distance", "so falloff should be bigger
/// and maybe the total distance should be a bit bigger", "15 might be a bit
/// too much, you decide"). The radio has no distance: it is the other key's.
pub const PLAIN_TO_M: f32 = 12.0;

/// The plain voice beside the listener and straight ahead plays as decoded
/// in both ears, as the radio does and as revision 58's near voice did: the
/// engine pans a point sound at constant power, 0.707 in each ear ahead (1
/// in one at the side), so its gains are raised by this.
const LEVEL: f32 = core::f32::consts::SQRT_2;

/// The plain voice's gain by its distance: a volume curve as the engine's
/// distance presets have them (`sounds/spl/*.spl`: the gain by metres, read
/// with the distance not under 1 m and held to 0..1, calculate_hdr_audio_frame
/// 0x5C9B5), of two linear keys: 1 to 1 m, 0 at [`PLAIN_TO_M`]. Two thirds
/// (4 dB down) at 5 m, half (6 dB) at 6.5 m, a third (9 dB) at 8 m, a sixth
/// (15 dB) at 10 m.
fn gain(metres: f32) -> f32 {
    let past = if metres > 1.0 { metres - 1.0 } else { 0.0 };
    (1.0 - past / (PLAIN_TO_M - 1.0)).clamp(0.0, 1.0)
}

/// The lines tried from the listener's head: to the speaker's head and this
/// far to either side of it, so that a post or a door's edge takes a part of
/// the voice and not all of it (the owner, 2026-11-16, of the nearby voice:
/// "it doesn't follow geometry at all right now"). Well inside a player's
/// own width (its capsule's radius is 0.38 m), so a speaker who stands
/// against a wall has no line's end in it.
const WALL_SIDE_M: f32 = 0.15;

/// How much of each player's nearby voice is behind a wall now (0 none, 1 all
/// of it); None for a place that is not asked (it is out of reach, or one
/// of the two is not in the world). Asked of everyone in reach, talking
/// or not, so a voice is behind its wall from its first sound. The DLL plays
/// that part quieter and dull, and goes from one frame's answer to the
/// next's in its own time.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Walled([Option<f32>; Place::COUNT]);

impl Walled {
    const NONE: Walled = Walled([None; Place::COUNT]);

    /// As the level is now, of the places in reach both of whose players
    /// are in the world: the part of the three lines, from the listener's
    /// head to the speaker's and to either side of it, that `blocked` says
    /// has something solid on it.
    fn now(roster: &Roster, blocked: impl Fn([f32; 3], [f32; 3]) -> bool) -> Walled {
        let mut walled = Walled::NONE;
        let Some(me) = roster.local().and_then(|me| roster.seat(me)).and_then(|seat| seat.body) else { return walled };
        for (place, seat, _) in roster.others() {
            let Some(them) = seat.body else { continue };
            // The reach as the voice's own (`in_the_world`: between the feet).
            let apart = [them.feet[0] - me.feet[0], them.feet[1] - me.feet[1], them.feet[2] - me.feet[2]];
            if sqrt(apart[0] * apart[0] + apart[1] * apart[1] + apart[2] * apart[2]) >= PLAIN_TO_M {
                continue;
            }
            // Level with the ground, across the line between the two heads.
            let (dx, dz) = (them.eye[0] - me.eye[0], them.eye[2] - me.eye[2]);
            let across = sqrt(dx * dx + dz * dz);
            let side = if across > 0.01 { [-dz / across * WALL_SIDE_M, 0.0, dx / across * WALL_SIDE_M] } else { [0.0; 3] };
            let lines = [0.0, 1.0, -1.0].map(|k| [them.eye[0] + k * side[0], them.eye[1], them.eye[2] + k * side[2]]);
            let behind = lines.iter().filter(|&&to| blocked(me.eye, to)).count();
            walled.0[place.index()] = Some(behind as f32 / lines.len() as f32);
        }
        walled
    }

    fn of(&self, place: Place) -> f32 {
        self.0[place.index()].unwrap_or(0.0)
    }
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
    /// The key or button push-to-talk is held by, and its channel.
    talk: Option<(TalkInput, Channel)>,
    /// Whether the local player was alive in the world at the last frame:
    /// only then has it a nearby voice.
    alive: bool,
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
        dll.talk(None);
        dll.stop();
    }
    dll.talk(None);
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
        Voice::Live(dll, Live { mutes: load_mutes(thread), told: Some(told), talk: None, alive: false, shown: None, trace })
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
        dll.talk(None);
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

/// How each other player of the match is heard on its two channels. Its
/// nearby voice (the owner, 2026-11-16: "10m is the one everyone can hear"):
/// by how far it is ([`in_the_world`]) while both players are alive in the
/// world, a mate's and an opponent's alike; not at all when either is dead or
/// a spectator. Its radio ("radio is team only", "or in ffa for everyone"):
/// a mate's (a teammate; in a free-for-all, any other player) at any
/// distance, alive or dead; an opponent's never. `pan` is the engine's, of a
/// sound at an offset from the listener. With a wall between the two
/// (`walled`: how much of the speaker is behind one) the DLL is told so, and
/// plays the nearby voice quieter and dull; the radio is not touched. The places
/// of nobody in the match, and the local player's own, are silent.
fn hearing(roster: &Roster, pan: impl Fn([f32; 3]) -> [f32; 2], walled: &Walled) -> Hearing {
    let me = roster.local().and_then(|me| roster.seat(me)).and_then(|seat| seat.body);
    roster.others().fold(Hearing::SILENT, |hearing, (place, seat, side)| {
        let near = match (me, seat.body) {
            (Some(me), Some(them)) => {
                let offset = [them.feet[0] - me.feet[0], them.feet[1] - me.feet[1], them.feet[2] - me.feet[2]];
                in_the_world(offset, &pan)
            }
            _ => Ears::SILENT,
        };
        hearing.with(place, Heard { near, muffle: Muffle::new(walled.of(place)), radio: side == Side::Mate })
    })
}

/// The nearby voice of a speaker `offset` from the listener (feet to feet, so
/// one on the same floor is at the ears' height), both alive: nearer than
/// [`PLAIN_TO_M`], by the engine's pan of a sound there and [`gain`]; from
/// it on, and at a distance that is not a number (a broken place), silent.
fn in_the_world(offset: [f32; 3], pan: &impl Fn([f32; 3]) -> [f32; 2]) -> Ears {
    let squared = offset[0] * offset[0] + offset[1] * offset[1] + offset[2] * offset[2];
    if !squared.is_finite() {
        return Ears::SILENT;
    }
    let metres = sqrt(squared);
    if metres >= PLAIN_TO_M {
        return Ears::SILENT;
    }
    let level = LEVEL * gain(metres);
    let [left, right] = pan(offset);
    Ears::new(left * level, right * level)
}

/// Who the local player's mates are (on its radio), by the match's mode: in
/// a free-for-all (patch 142's) every player of the match, else its team.
/// The one place the game decides it; srp's relay sends a radio utterance to
/// the same players.
fn mates(thread: &Thread) -> Mates {
    if crate::free_for_all::running(thread) { Mates::Match } else { Mates::Team }
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
    let roster = scene.roster(true, mates(thread));
    live.alive = roster.local().and_then(|me| roster.seat(me)).is_some_and(|seat| seat.body.is_some());
    if let Some((key, channel)) = live.talk {
        let ended = if !speaking {
            Some("the microphone is off in the options")
        } else if !dll.focused() {
            Some("the game is not the foreground window")
        } else if !dll.key_down(key) {
            Some("its key is up (GetAsyncKeyState)")
        } else if channel == Channel::Near && !live.alive {
            // Nobody hears the nearby voice of a player not in the world.
            Some("the nearby voice needs the player alive")
        } else {
            None
        };
        if let Some(why) = ended {
            dll.talk(None);
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
    // The level between the listener and everyone in reach (three lines each).
    let walls = scene.walls();
    let walled = Walled::now(&roster, |from, to| walls.as_ref().is_some_and(|walls| walls.between(from, to)));
    // No listener yet (the world's first tick): the engine's centre.
    let listener = Listener::at_frame(ctx);
    let pan = |offset| listener.as_ref().map_or(PAN_CENTRE, |listener| listener.pan(offset));
    let told = Told { settings, muted: muted(dll, &roster, &live.mutes), hearing: hearing(&roster, pan, &walled) };
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
    let speakers = speakers(talking, &roster, told.muted);
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

/// Who the hud's list shows of those `talking` (the DLL's: the players heard
/// talking, each on the channel of its utterance as [`hearing`] says its
/// place is heard), and the local player's own place while its microphone is sent
/// (the owner, 2026-11-11: "when I speak I want to see this with an icon");
/// none `muted`.
fn speakers(talking: Places, roster: &Roster, muted: Places) -> Speakers {
    let mut shown = Speakers::NONE;
    for p in Place::all().filter(|&p| talking.without(muted).has(p)) {
        shown.0[p.index()] = match roster.side(p) {
            Some(side) => Some(side),
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
        // The key held first talks: the other key pressed over it does nothing.
        TalkKey::Down(code, channel) => {
            if live.talk.is_none() {
                if channel == Channel::Near && !live.alive {
                    live.trace.line(&[Text("push-to-talk down, not taken: the nearby voice needs the player alive")]);
                } else if matches!(engine::settings(thread), Settings::On { microphone: true, .. }) {
                    dll.talk(Some(channel));
                    live.talk = Some((code, channel));
                    live.trace.line(&[Text(match channel {
                        Channel::Near => "push-to-talk down: the nearby voice",
                        Channel::Radio => "push-to-talk down: the radio",
                    })]);
                } else {
                    live.trace.line(&[Text("push-to-talk down, not taken: voice or the microphone is off in the options")]);
                }
            }
        }
        TalkKey::Up(code) => {
            if live.talk.is_some_and(|(held, _)| held == code) {
                dll.talk(None);
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
        let roster = Scene::at_call(&call).roster(false, mates(thread));
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
/// and the button stays away); with `<volume>` it is a press,
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
        Some(Some(TestAsk::Press(volume))) => dll.test_press(volume),
        Some(Some(TestAsk::Stop)) => dll.test_stop(),
        Some(None) => log::line(&[Text("voice: srp_mic_test with neither the volume slider's steps nor stop")]),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vostok::voice::{Body, Seat};

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
            let body = feet.map(|feet| Body { feet, eye: [feet[0], feet[1] + 1.6, feet[2]] });
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
    fn the_nearby_voice_in_reach_alive_the_radio_a_mates_anywhere() {
        let r = roster(&[
            (0, Some([5.0, 1.0, 5.0])),
            (0, Some([5.0, 1.0, 16.9])),
            (0, Some([5.0, 1.0, 17.0])),
            (1, None),
            (0, None),
            (0, Some([5.0, 1.0, 55.0])),
        ]);
        let heard = hearing(&r, pan, &Walled::NONE);
        // A mate near: both channels; far or dead: its radio alone.
        assert!(heard.of(p(1)).near != Ears::SILENT && heard.of(p(1)).radio);
        for far in [p(2), p(4), p(5)] {
            assert_eq!(heard.of(far), Heard { near: Ears::SILENT, muffle: Muffle::NONE, radio: true });
        }
        // A dead opponent, the local player, a place of nobody: silent.
        for nobody in [p(3), p(0), p(19)] {
            assert_eq!(heard.of(nobody), Heard::SILENT);
        }
        // The listener dead: no nearby voice of anyone beside its body's place.
        let dead = roster(&[(0, None), (0, Some([1.0, 0.0, 0.0]))]);
        assert_eq!(hearing(&dead, pan, &Walled::NONE).of(p(1)), Heard { near: Ears::SILENT, muffle: Muffle::NONE, radio: true });
        let broken = roster(&[(0, Some([0.0; 3])), (0, Some([f32::NAN, 0.0, 0.0]))]);
        assert_eq!(hearing(&broken, pan, &Walled::NONE).of(p(1)).near, Ears::SILENT);
    }

    #[test]
    fn the_nearby_voice_has_the_engines_pan_of_its_offset_and_falls_to_nothing() {
        // The pan is asked of the speaker's offset from the listener, feet to feet.
        let r = roster(&[(0, Some([10.0, 2.0, 10.0])), (0, Some([10.5, 2.0, 10.25]))]);
        let ears = hearing(&r, pan, &Walled::NONE).of(p(1)).near;
        // Under 1 m the gain is whole: the pan times the level.
        let level = gain(1.0) * LEVEL;
        assert!(close(ears.get()[0], 0.5 * level) && close(ears.get()[1], 0.25 * level), "{ears:?}");
        // Ahead beside the listener, as decoded in both ears.
        let ahead = |metres: f32| in_the_world([0.0, 0.0, metres], &|_| PAN_CENTRE).get();
        assert!(close(ahead(0.0)[0], 1.0) && close(ahead(1.0)[1], 1.0));
        // The fall: none to 1 m, then linear to nothing at the end.
        assert!(close(gain(0.3), 1.0) && close(gain(1.0), 1.0) && close(gain(6.5), 0.5));
        assert!(close(gain(10.0), 2.0 / 11.0) && close(ahead(6.5)[0], 0.5) && close(ahead(6.5)[1], 0.5));
        // No step at the end: next to nothing just before it, nothing from it.
        assert!(ahead(11.99)[0] < 0.002 && ahead(11.99)[0] > 0.0 && ahead(PLAIN_TO_M) == [0.0; 2] && ahead(40.0) == [0.0; 2]);
        // A pan that is not a number is silent, never noise.
        assert_eq!(in_the_world([1.0, 0.0, 0.0], &|_| [f32::NAN; 2]), Ears::SILENT);
    }

    #[test]
    fn a_wall_between_two_heads_takes_the_part_of_the_voice_behind_it() {
        // The local player at the origin; 1 a teammate 4 m ahead, 2 an
        // opponent 4 m to the side, 3 a teammate 16 m behind: out of reach.
        let r = roster(&[(0, Some([0.0; 3])), (0, Some([0.0, 0.0, 4.0])), (1, Some([4.0, 0.0, 0.0])), (0, Some([0.0, 0.0, -16.0]))]);
        // The lines go from the listener's head to the speaker's and 0.15 m
        // to either side of it, level with the ground.
        let lines = core::cell::RefCell::new(Vec::new());
        let open = Walled::now(&r, |from, to| {
            lines.borrow_mut().push((from, to));
            false
        });
        assert_eq!(open.0[1], Some(0.0));
        assert_eq!((open.0[2], open.0[3], open.0[0]), (Some(0.0), None, None));
        assert_eq!(lines.borrow().len(), 6);
        let ahead: Vec<[f32; 3]> = lines.borrow()[..3].iter().map(|&(from, to)| { assert_eq!(from, [0.0, 1.6, 0.0]); to }).collect();
        assert_eq!(ahead[0], [0.0, 1.6, 4.0]);
        assert!(close(ahead[1][0], -WALL_SIDE_M) && close(ahead[2][0], WALL_SIDE_M) && ahead[1][2] == 4.0 && ahead[1][1] == 1.6, "{ahead:?}");
        // A wall across the way ahead: all of 1; a post beside 2's head: a third of it.
        let wall = Walled::now(&r, |_, to| to[2] > 3.0 || (to[0] > 3.0 && to[2] > 0.1));
        assert_eq!(wall.0[1], Some(1.0));
        assert!(close(wall.0[2].unwrap(), 1.0 / 3.0), "{wall:?}");
        // Heard: the DLL is told how much (it makes the voice quieter and
        // dull); the ears are the voice's in the open, the radio is not touched.
        let heard = hearing(&r, pan, &wall);
        let clear = hearing(&r, pan, &Walled::NONE);
        assert_eq!(heard.of(p(1)).near, clear.of(p(1)).near);
        assert_eq!((heard.of(p(1)).muffle, heard.of(p(1)).radio), (Muffle::WHOLE, true));
        assert!(close(heard.of(p(2)).muffle.get(), 1.0 / 3.0));
        assert_eq!(heard.of(p(3)), clear.of(p(3)));
        // The listener out of the world: nobody is followed.
        let dead = roster(&[(0, None), (0, Some([0.0, 0.0, 4.0]))]);
        assert_eq!(Walled::now(&dead, |_, _| true), Walled::NONE);
    }

    #[test]
    fn in_a_free_for_all_everyone_is_a_mate_in_a_team_mode_teammates() {
        let sides = |r: &Roster| r.others().map(|(place, _, side)| (place, side)).collect::<Vec<_>>();
        // A free-for-all's places, each a side of its own (4 + its place).
        let mut ffa = roster(&[(4, Some([0.0; 3])), (5, Some([3.0, 0.0, 0.0])), (6, Some([0.0, 0.0, 40.0])), (7, None)]);
        // As a team mode reads them: sides past the two, nobody's mate.
        assert_eq!(sides(&ffa), [(p(1), Side::Opponent), (p(2), Side::Opponent), (p(3), Side::Opponent)]);
        ffa.set_mates(Mates::Match);
        assert_eq!(sides(&ffa), [(p(1), Side::Mate), (p(2), Side::Mate), (p(3), Side::Mate)]);
        // Everyone's radio is heard, near, far and dead; the nearby voice near.
        let heard = hearing(&ffa, pan, &Walled::NONE);
        assert!(heard.of(p(1)).near.get()[0] > 0.0 && heard.of(p(1)).near.get()[1] == 0.0);
        assert!([p(1), p(2), p(3)].iter().all(|&place| heard.of(place).radio));
        assert_eq!(heard.of(p(2)).near, Ears::SILENT);
        // A team mode: the teammates mates, the other team opponents.
        let teams = roster(&[(0, None), (0, None), (1, None)]);
        assert_eq!(sides(&teams), [(p(1), Side::Mate), (p(2), Side::Opponent)]);
        // Nobody's side while the local player's own place is not known.
        let mut unknown = teams;
        unknown.set_local(None);
        assert_eq!(sides(&unknown), []);
    }

    #[test]
    fn an_opponents_nearby_voice_is_heard_as_a_teammates_its_radio_never() {
        // The local player (team 0) at the origin; a teammate and an opponent
        // side by side at each distance.
        let at = |metres: f32| Some([metres, 0.0, 0.0]);
        let r = roster(&[(0, at(0.0)), (0, at(4.0)), (1, at(4.0)), (0, at(16.0)), (1, at(16.0)), (0, None), (1, None)]);
        let heard = hearing(&r, pan, &Walled::NONE);
        // Near: the same nearby voice, the engine's pan and the fall alike.
        assert!(heard.of(p(2)).near != Ears::SILENT);
        assert_eq!(heard.of(p(2)).near, heard.of(p(1)).near);
        // The radio: the teammates', wherever they are; no opponent's.
        for (mate, opponent) in [(p(1), p(2)), (p(3), p(4)), (p(5), p(6))] {
            assert!(heard.of(mate).radio && !heard.of(opponent).radio);
        }
        // Far or dead, an opponent is not heard at all.
        assert_eq!(heard.of(p(4)), Heard::SILENT);
        assert_eq!(heard.of(p(6)), Heard::SILENT);
    }

    #[test]
    fn the_list_shows_who_talks_with_its_side_and_the_own_place() {
        let at = |metres: f32| Some([metres, 0.0, 0.0]);
        // 0 the local player; 1 a teammate; 2 an opponent.
        let r = roster(&[(0, at(0.0)), (0, at(40.0)), (1, at(3.0))]);
        let all = [p(0), p(1), p(2)].into_iter().fold(Places::NONE, Places::with);
        let shown = |s: Speakers| s.iter().collect::<Vec<_>>();
        // The own place drawn as a mate's; the opponent as an opponent.
        assert_eq!(shown(speakers(all, &r, Places::NONE)), [(p(0), Side::Mate), (p(1), Side::Mate), (p(2), Side::Opponent)]);
        // Muted ones are not; nor anyone not talking, nor a place of nobody.
        assert_eq!(shown(speakers(all, &r, Places::NONE.with(p(2)))), [(p(0), Side::Mate), (p(1), Side::Mate)]);
        assert_eq!(speakers(Places::NONE.with(p(7)), &r, Places::NONE), Speakers::NONE);
        // In a free-for-all everyone is a mate.
        let mut ffa = r;
        ffa.set_mates(Mates::Match);
        assert_eq!(shown(speakers(all, &ffa, Places::NONE)), [(p(0), Side::Mate), (p(1), Side::Mate), (p(2), Side::Mate)]);
    }
}
