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
//!   mutes and who is far (the radio) told to the DLL when they change, a
//!   held push-to-talk let go when the key is up or the game lost the
//!   keyboard, and the hud's list of who talks;
//! - the game world's keys (`tab_cursor.key`): push-to-talk (`ptt`, P, the
//!   chat keys' group);
//! - the hud's calls (`hud.callback`): the Tab list's player menu (patch 86's)
//!   asks `srp_voice_menu(place)`, answered `root.srp_voice_menu(place,
//!   offered, muted)` (a teammate whose voice id is known), and sends
//!   `srp_voice_mute(place, "1"|"0")`, answered `root.srp_voice_mute(place,
//!   done)`.
//!
//! Nothing a match computes: audio goes beside the game, through the DLL.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::log::{self, Piece::*};
use crate::vostok::types::Place;
use crate::vostok::voice::{
    self as engine, Connect, Dll, MUTES_SIZE, Places, Roster, Scene, Settings, TalkKey, VoiceId, name_text,
};
use crate::vostok::{Context, Dik, Flow, GameCell, GameWorldKey, HudCall, NoExit, Thread, VoiceOnHudCallExit};

/// A teammate farther than this is heard through the radio (the owner,
/// 2026-10-25); a dead one, or any when the local player is dead, too.
pub const NEAR_M: f32 = 25.0;

/// What the DLL was last told, so it is told again only on a change.
#[derive(Clone, Copy, PartialEq)]
struct Told {
    settings: Settings,
    muted: Places,
    radio: Places,
}

/// A match with voice.
#[derive(Clone, Copy)]
struct Live {
    mutes: Mutes,
    told: Option<Told>,
    /// The key push-to-talk is held by.
    talk: Option<Dik>,
    /// The speakers the hud's list was told.
    shown: Option<Places>,
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
    let next = if dll.start(&connect) {
        Voice::Live(dll, Live { mutes: load_mutes(thread), told: None, talk: None, shown: None })
    } else {
        Voice::Idle(dll)
    };
    VOICE.set(thread, next);
    Flow::Continue
}

fn stop(thread: &Thread) {
    if let Voice::Live(dll, _) = VOICE.get(thread) {
        dll.talk(false);
        dll.stop();
        VOICE.set(thread, Voice::Idle(dll));
    }
}

pub fn on_finish(ctx: &mut Context<'_>) -> Flow<NoExit> {
    stop(ctx.thread());
    Flow::Continue
}

pub fn on_deactivate(ctx: &mut Context<'_>) -> Flow<NoExit> {
    stop(ctx.thread());
    Flow::Continue
}

/// The teammates heard through the radio: farther than [`NEAR_M`], or
/// either of the two not alive in the world.
fn radio(roster: &Roster) -> Places {
    let me = roster.local().and_then(|me| roster.seat(me)).and_then(|seat| seat.position);
    roster.teammates().fold(Places::NONE, |far, (place, seat)| {
        let near = match (me, seat.position) {
            (Some(a), Some(b)) => {
                let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
                let squared = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
                squared.is_finite() && squared <= NEAR_M * NEAR_M
            }
            _ => false,
        };
        if near { far } else { far.with(place) }
    })
}

/// The teammates whose voice id is muted.
fn muted(dll: Dll, roster: &Roster, mutes: &Mutes) -> Places {
    roster.teammates().fold(Places::NONE, |muted, (place, _)| {
        if dll.voice_id(place).is_some_and(|id| mutes.has(id)) { muted.with(place) } else { muted }
    })
}

pub fn on_frame(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(scene) = Scene::at_frame(ctx) else { return Flow::Continue };
    let thread = scene.thread();
    let Voice::Live(dll, mut live) = VOICE.get(thread) else { return Flow::Continue };
    let settings = engine::settings(thread);
    let speaking = matches!(settings, Settings::On { microphone: true, .. });
    if let Some(key) = live.talk
        && (!speaking || !dll.focused() || !dll.key_down(key))
    {
        dll.talk(false);
        live.talk = None;
    }
    let roster = scene.roster(true);
    let told = Told { settings, muted: muted(dll, &roster, &live.mutes), radio: radio(&roster) };
    let before = live.told;
    if before.map(|t| t.settings) != Some(told.settings) {
        dll.settings(told.settings);
    }
    if before.map(|t| t.muted) != Some(told.muted) {
        dll.mute(told.muted);
    }
    if before.map(|t| t.radio) != Some(told.radio) {
        dll.radio(told.radio);
    }
    live.told = Some(told);
    let talking = match settings {
        Settings::Off => Places::NONE,
        Settings::On { .. } => dll.talking(),
    };
    let speakers =
        Place::all().filter(|&p| talking.has(p) && roster.teammate(p)).fold(Places::NONE, Places::with).without(told.muted);
    if live.shown != Some(speakers) && show_speakers(&scene, &roster, speakers) {
        live.shown = Some(speakers);
    }
    VOICE.set(thread, Voice::Live(dll, live));
    Flow::Continue
}

/// The hud's list (`root.srp_voice_talking`): the speakers' names (as 0.69's,
/// the teammates only).
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
    let thread = key.thread();
    let Voice::Live(dll, mut live) = VOICE.get(thread) else { return Flow::Continue };
    match event {
        TalkKey::Down(code) => {
            if live.talk.is_none() && matches!(engine::settings(thread), Settings::On { microphone: true, .. }) {
                dll.talk(true);
                live.talk = Some(code);
            }
        }
        TalkKey::Up(code) => {
            if live.talk == Some(code) {
                dll.talk(false);
                live.talk = None;
            }
        }
    }
    VOICE.set(thread, Voice::Live(dll, live));
    Flow::Continue
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
        let roster = Scene::at_call(&call).roster(false);
        dll.voice_id(place).filter(|_| roster.teammate(place))
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
    use crate::vostok::voice::Seat;

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
        for (n, &(team, position)) in seats.iter().enumerate() {
            r.set(Place::new(n as u32).unwrap(), Seat { name: [0; 64], team, position });
        }
        r.set_local(Place::new(0));
        r
    }

    #[test]
    fn radio_past_25_m_and_for_the_dead() {
        let p = |n| Place::new(n).unwrap();
        let r = roster(&[(0, Some([0.0; 3])), (0, Some([25.0, 0.0, 0.0])), (0, Some([25.01, 0.0, 0.0])), (1, None), (0, None)]);
        let far = radio(&r);
        assert!(!far.has(p(1)) && far.has(p(2)) && !far.has(p(3)) && far.has(p(4)) && !far.has(p(0)));
        let dead = roster(&[(0, None), (0, Some([1.0, 0.0, 0.0]))]);
        assert!(radio(&dead).has(p(1)));
    }
}
