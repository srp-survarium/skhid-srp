//! Patch 66's boundaries (voice chat, docs/notes/voice-chat.md): the voice's
//! console variables, the match a connect starts, the push-to-talk key, the match's
//! places as a frame or a menu sees them, the game's sound listener and its
//! engine's pan of a sound, the movies, and the optional
//! versioned DLL (`skhid_voice.dll`, `voice_abi`). Every game and DLL value
//! is read here once into a type; who talks, who is heard and who is muted
//! is the patch's (`crate::voice`).

use core::ffi::{CStr, c_void};
use core::marker::PhantomData;
use core::num::NonZeroU32;
use core::ptr::NonNull;

use super::context::Context;
use super::hook::Thread;
use super::input::Action;
use super::keyboard::{ButtonKind, Dik, GameWorldButton, GameWorldKey, KeyKind, MouseButton};
use super::language::ConsoleString;
use super::hud_input::HudCall;
use super::settings::{ConsoleBool, ConsoleFloat, save_user_config};
use super::types::Place;
use super::ui::{Movie, c_string, movie_of};
use super::world::{GameWorld, NetworkClient};
use crate::raw;

/// The DLL's file, beside the exe (make_client_026g.py --voice puts it there).
const DLL: &CStr = c"skhid_voice.dll";

/// Places as a set (the DLL's masks: bit n place n).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Places(u32);

impl Places {
    pub const NONE: Places = Places(0);

    pub fn with(self, place: Place) -> Places {
        Places(self.0 | 1 << place.get())
    }

    pub fn has(self, place: Place) -> bool {
        self.0 & 1 << place.get() != 0
    }

    pub fn without(self, other: Places) -> Places {
        Places(self.0 & !other.0)
    }

    /// A DLL's word: only the 20 places' bits.
    fn of_word(word: u32) -> Places {
        Places(word & ((1 << Place::COUNT) - 1))
    }
}

/// A teammate's voice id: srp's ROSTER gives each place the first four bytes
/// of SHA-256(key, account id); the DLL's 0 is "none yet".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VoiceId(NonZeroU32);

impl VoiceId {
    pub fn new(word: u32) -> Option<VoiceId> {
        NonZeroU32::new(word).map(VoiceId)
    }

    pub fn get(self) -> u32 {
        self.0.get()
    }
}

/// A 0..100 stock slider's value as the DLL's 0..1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fraction(f32);

impl Fraction {
    /// Out of 0..100, or not a number: 0 (the console stores the least for
    /// such a value; a hand-edited user.cfg is read so too).
    fn of_percent(value: f32) -> Fraction {
        Fraction(if (0.0..=100.0).contains(&value) { value / 100.0 } else { 0.0 })
    }

    pub fn get(self) -> f32 {
        self.0
    }
}

/// The sound tab's "Voice chat" volume and "Microphone sensitivity" as their
/// sliders stand on the screen, accepted or not: what the Settings send with
/// a press of "Test microphone", `<volume> <sensitivity>`, each the slider's
/// whole steps (0..100).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sliders {
    volume: Fraction,
    sensitivity: Fraction,
}

impl Sliders {
    /// Two numbers of one to three decimal digits, 100 at most, one space
    /// between them; nothing else.
    pub fn parse(text: &CStr) -> Option<Sliders> {
        let mut steps = text.to_bytes().split(|&byte| byte == b' ').map(|digits| {
            if digits.is_empty() || digits.len() > 3 || !digits.iter().all(u8::is_ascii_digit) {
                return None;
            }
            let steps = digits.iter().fold(0u16, |steps, digit| steps * 10 + u16::from(digit - b'0'));
            (steps <= 100).then(|| Fraction::of_percent(f32::from(steps)))
        });
        let (volume, sensitivity) = (steps.next()??, steps.next()??);
        steps.next().is_none().then_some(Sliders { volume, sensitivity })
    }
}

/// What the Settings say with a call of the microphone test: a press of its
/// button, or `stop` (the Settings window closed).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TestAsk {
    Press(Sliders),
    Stop,
}

impl TestAsk {
    pub fn parse(text: &CStr) -> Option<TestAsk> {
        if text == c"stop" { Some(TestAsk::Stop) } else { Sliders::parse(text).map(TestAsk::Press) }
    }
}

pub use voice_abi::{Microphone, MicrophoneId};

/// The capture device the options' Microphone list picks: Windows' default,
/// or a device by its endpoint ID (the DLL's list gives them).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MicrophoneChoice {
    Default,
    Device(MicrophoneId),
}

impl MicrophoneChoice {
    /// The list's text of it, and user.cfg's: empty for the default, else
    /// the device's ID; None for anything else.
    pub fn parse(text: &[u8]) -> Option<MicrophoneChoice> {
        if text.is_empty() { Some(MicrophoneChoice::Default) } else { MicrophoneId::parse(text).map(MicrophoneChoice::Device) }
    }

    pub fn text(&self) -> &CStr {
        match self {
            MicrophoneChoice::Default => c"",
            MicrophoneChoice::Device(id) => id.as_c_str(),
        }
    }
}

/// The voice options: off ("Disable voice chat", the gameplay tab's dead
/// "Hide spam", `g_hide_spam`: nothing played, no microphone, the relay sends
/// nothing) or on, with the sound tab's three stock voice controls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Settings {
    Off,
    On { volume: Fraction, sensitivity: Fraction, microphone: bool },
}

static VOLUME: ConsoleFloat = ConsoleFloat::new(100.0, 0.0, 100.0);
static SENSITIVITY: ConsoleFloat = ConsoleFloat::new(50.0, 0.0, 100.0);
static MICROPHONE: ConsoleBool = ConsoleBool::with(true);
static OFF: ConsoleBool = ConsoleBool::new();
/// `srp_voice_mutes`' room: the patch's `Mutes` text.
pub const MUTES_SIZE: usize = 512;
static MUTES: ConsoleString<MUTES_SIZE> = ConsoleString::new();
/// `srp_microphone`: the Microphone list's pick, [`MicrophoneChoice::text`].
static CAPTURE_DEVICE: ConsoleString<{ voice_abi::MICROPHONE_ID }> = ConsoleString::new();

/// The variables in the engine's list before user.cfg (`configs.start`):
/// the stock sound tab's controls name three of them, user.cfg keeps all.
pub fn register(thread: &Thread) {
    VOLUME.register(thread, c"s_chat_volume");
    SENSITIVITY.register(thread, c"s_mic_sens");
    MICROPHONE.register(thread, c"s_use_microphone");
    OFF.register(thread, c"g_hide_spam");
    MUTES.register(thread, c"srp_voice_mutes");
    CAPTURE_DEVICE.register(thread, c"srp_microphone");
}

/// The saved pick of the Microphone list; the default for a value that is
/// not one (a hand-edited user.cfg).
pub fn saved_microphone(thread: &Thread) -> MicrophoneChoice {
    MicrophoneChoice::parse(CAPTURE_DEVICE.get(thread)).unwrap_or(MicrophoneChoice::Default)
}

/// Keeps the Microphone list's pick, user.cfg written at once.
pub fn save_microphone(thread: &Thread, choice: MicrophoneChoice) {
    CAPTURE_DEVICE.set(thread, choice.text().to_bytes());
    save_user_config(thread);
}

pub fn settings(thread: &Thread) -> Settings {
    if OFF.get(thread) {
        return Settings::Off;
    }
    Settings::On {
        volume: Fraction::of_percent(VOLUME.get(thread)),
        sensitivity: Fraction::of_percent(SENSITIVITY.get(thread)),
        microphone: MICROPHONE.get(thread),
    }
}

/// The saved mutes' text, as user.cfg (or the console) left it.
pub fn saved_mutes(thread: &Thread) -> &[u8] {
    MUTES.get(thread)
}

/// Keeps the mutes' text, user.cfg written at once.
pub fn save_mutes(thread: &Thread, text: &[u8]) {
    MUTES.set(thread, text);
    save_user_config(thread);
}

/// The match a live connect starts (`match_client.connect`; a replay's is
/// another function): the match server's host, the voice port (its UDP port
/// + 1) and the session.
pub struct Connect<'c> {
    host: &'c CStr,
    port: u16,
    session: u32,
}

impl<'c> Connect<'c> {
    /// None without a host, or for a match port with no port after it
    /// (65535).
    pub fn at(ctx: &'c Context<'_>) -> Option<Connect<'c>> {
        // SAFETY: the context is this site's: match_client::connect's
        // arguments at its start, the host a C string its caller keeps for
        // the call, on the game's thread.
        let (host, port, session) = unsafe {
            let frame = raw::site::match_client_connect::frame(ctx)?;
            (c_string(frame.host(), 0x100)?, *frame.port(), *frame.session())
        };
        let port = port.checked_add(1)?;
        Some(Connect { host, port, session })
    }

    /// The relay's port: the match's and one.
    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn session(&self) -> u32 {
        self.session
    }
}

/// What push-to-talk is held by: a key, or a mouse button (the options bind
/// either).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TalkInput {
    Key(Dik),
    Button(MouseButton),
}

/// A press or a release of push-to-talk (`ptt`, 47h; the binder's kind 8, the
/// chat's, whatever group the options list it in), at the game world's keys'
/// start (`tab_cursor.key`) or its mouse buttons' (`game_world.mouse_key`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TalkKey {
    Down(TalkInput),
    Up(TalkInput),
}

impl TalkKey {
    /// The event when the key is bound to `ptt` (a held key's repeats are
    /// none).
    pub fn of(key: &GameWorldKey<'_, '_>) -> Option<TalkKey> {
        if key.chat_action()? != Action::PTT {
            return None;
        }
        let input = TalkInput::Key(key.key());
        match key.kind() {
            KeyKind::Down => Some(TalkKey::Down(input)),
            KeyKind::Up => Some(TalkKey::Up(input)),
            KeyKind::Held => None,
        }
    }

    /// The event when the button is bound to `ptt`.
    pub fn of_button(button: &GameWorldButton<'_>) -> Option<TalkKey> {
        if button.chat_action()? != Action::PTT {
            return None;
        }
        let input = TalkInput::Button(button.button());
        match button.kind() {
            ButtonKind::Press => Some(TalkKey::Down(input)),
            ButtonKind::Release => Some(TalkKey::Up(input)),
            ButtonKind::Held => None,
        }
    }
}

/// The virtual keys GetAsyncKeyState may report a held input by: a key's own,
/// as MapVirtualKeyA(MAPVK_VSC_TO_VK_EX) names its scan code (`mapped`), and
/// another where that differs from what a press sets. The numpad's digits and
/// its point map to the navigation keys, which a press sets only with NumLock
/// off (on, it sets VK_NUMPAD0..9 and VK_DECIMAL); NumLock and Pause map
/// swapped (Pause is E0h 45h to DirectInput, its own VK_PAUSE). A mouse button
/// is its own VK_*BUTTON; one past the side buttons has none to read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Polled {
    /// Down when one of these is (0: none).
    Keys(u32, u32),
    /// Nothing to read: only its release event ends it.
    Unknown,
}

const VK_LBUTTON: u32 = 0x01;
const VK_RBUTTON: u32 = 0x02;
const VK_MBUTTON: u32 = 0x04;
const VK_XBUTTON1: u32 = 0x05;
const VK_XBUTTON2: u32 = 0x06;
const VK_PAUSE: u32 = 0x13;
const VK_NUMPAD0: u32 = 0x60;
const VK_DECIMAL: u32 = 0x6E;
const VK_NUMLOCK: u32 = 0x90;

impl Polled {
    /// `mapped` is the input's scan code mapped by MapVirtualKeyA (0: none);
    /// a button needs none.
    fn of(input: TalkInput, mapped: impl FnOnce(u32) -> u32) -> Polled {
        match input {
            TalkInput::Button(button) => match button.number() {
                0 => Polled::Keys(VK_LBUTTON, 0),
                1 => Polled::Keys(VK_RBUTTON, 0),
                2 => Polled::Keys(VK_MBUTTON, 0),
                3 => Polled::Keys(VK_XBUTTON1, 0),
                4 => Polled::Keys(VK_XBUTTON2, 0),
                _ => Polled::Unknown,
            },
            TalkInput::Key(key) => {
                let code = u32::from(key.code());
                // DIK_NUMLOCK, DIK_PAUSE (E0h 45h)
                match code {
                    0x45 => return Polled::Keys(VK_NUMLOCK, 0),
                    0xC5 => return Polled::Keys(VK_PAUSE, 0),
                    _ => (),
                }
                // the numpad's 7 8 9 / 4 5 6 / 1 2 3 / 0 . (DIK_NUMPAD7.. DIK_DECIMAL)
                let numpad = match code {
                    0x47..=0x49 => VK_NUMPAD0 + 7 + (code - 0x47),
                    0x4B..=0x4D => VK_NUMPAD0 + 4 + (code - 0x4B),
                    0x4F..=0x51 => VK_NUMPAD0 + 1 + (code - 0x4F),
                    0x52 => VK_NUMPAD0,
                    0x53 => VK_DECIMAL,
                    _ => 0,
                };
                let scan = if code & 0x80 != 0 { 0xE000 | (code & 0x7F) } else { code };
                match (mapped(scan), numpad) {
                    (0, 0) => Polled::Unknown,
                    (vk, numpad) => Polled::Keys(vk, numpad),
                }
            }
        }
    }
}

pub use voice_abi::{Ears, Heard, MicTest};

/// How every place is heard (what the DLL is told: one `sv_hear` a place).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hearing([Heard; Place::COUNT]);

impl Hearing {
    /// Every place plain, as decoded in both ears.
    pub const UNITY: Hearing = Hearing([Heard::Plain(Ears::UNITY); Place::COUNT]);

    pub fn with(mut self, place: Place, heard: Heard) -> Hearing {
        self.0[place.index()] = heard;
        self
    }

    pub fn of(&self, place: Place) -> Heard {
        self.0[place.index()]
    }
}

/// A player alive in the world, as voice needs it: where its feet are.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub feet: [f32; 3],
}

/// The engine's pan of a sound straight ahead, behind, above, or at the
/// listener: the same in both ears, at constant power.
pub const PAN_CENTRE: [f32; 2] = [core::f32::consts::FRAC_1_SQRT_2; 2];

/// The game's sound listener at a frame of a match: the game world's sound
/// scene (its on_after_tick 0x1F7620 gives the scene the camera's place,
/// front and top every tick), with the sound world's panning table. What
/// the engine pans its own point sounds by.
pub struct Listener<'t> {
    scene: raw::SoundScene,
    table: raw::PanningLut,
    position: [f32; 3],
    _frame: PhantomData<&'t Thread>,
}

impl<'t> Listener<'t> {
    /// At `game_world.after_move`. None before the scene has a listener (the
    /// world's first tick has not set it), or without a scene.
    pub fn at_frame(ctx: &'t Context<'_>) -> Option<Listener<'t>> {
        let world = GameWorld::at_after_move(ctx)?;
        // SAFETY: the ticking game world's sound scene, which its own tick
        // gives the listener (0x1F764C, with no test: it is there while the
        // world ticks), that scene's sound world and the panning table
        // inside the world (filled by its constructor, never written
        // again). get_listenet_position (0x57560) reads the scene's place
        // (an atomic half3) with a locked cmpxchg8b that leaves it as it
        // is, as the sound thread reads it, and writes the three floats.
        unsafe {
            let scene = world.this.sound_scene()?;
            if scene.listener_set().read_volatile() == 0 {
                return None;
            }
            let table = scene.world()?.panning();
            let mut position = [0.0_f32; 3];
            raw::engine_listener_position(Some(scene), position.as_mut_ptr());
            Some(Listener { scene, table, position, _frame: PhantomData })
        }
    }

    /// The left and right gains the engine gives a point sound `offset`
    /// from the listener (metres, the map's frame): its own
    /// sound_scene::calculate_3d_pan (0x57D70), which every point sound's
    /// voice gets its output matrix from at each sound tick (notify_listener2
    /// 0x5C3CC). Constant power: [`PAN_CENTRE`] ahead and behind, at the side 1
    /// in that ear and nothing in the other, above or below towards the
    /// centre. No gain by distance (the engine keeps that in the voice's
    /// volume).
    pub fn pan(&self, offset: [f32; 3]) -> [f32; 2] {
        // A proxy of ours, all zero: its sound_type (+80h) 0, a point sound,
        // the one field calculate_3d_pan reads of one.
        let mut proxy = raw::SoundInstanceProxyStorage::zeroed();
        let position = [self.position[0] + offset[0], self.position[1] + offset[1], self.position[2] + offset[2]];
        let squared = offset[0] * offset[0] + offset[1] * offset[1] + offset[2] * offset[2];
        // As notify_listener2 gives it: the distance, 1 m at least.
        let distance = if squared > 1.0 { ::ragdoll::math::sqrt(squared) } else { 1.0 };
        let mut matrix = [0.0_f32; 4];
        let mut coefficient = 0.0_f32;
        // SAFETY: the engine's function by its row's signature, on the
        // game's thread. Run on the exe's bytes (check_voice_pan_026g.py): of
        // a point sound it reads the proxy's +80h, the position, the table
        // and the scene's three listener vectors (locked cmpxchg8b reads,
        // as above), and writes matrix[0], matrix[1] and the coefficient;
        // nothing else outside its stack.
        unsafe {
            raw::engine_sound_pan(
                Some(self.scene),
                Some(self.table),
                Some(proxy.handle()),
                position.as_ptr(),
                distance,
                matrix.as_mut_ptr(),
                &mut coefficient,
            );
        }
        if matrix[0].is_finite() && matrix[1].is_finite() { [matrix[0], matrix[1]] } else { PAN_CENTRE }
    }
}

/// A place of the match as voice needs it: the name and team of its record
/// (the match options') and, while its player is alive in the world, its body.
#[derive(Clone, Copy)]
pub struct Seat {
    pub name: [u8; 64],
    pub team: u32,
    pub body: Option<Body>,
}

/// Whom the local player's voice reaches and who it hears, as srp's relay
/// routes it (`registry::VoiceChannel`): the places of its team, or in a
/// free-for-all every place of the match (the owner, 2026-11-13: "in FFA I
/// would enable voice chat for all").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Team,
    Match,
}

/// The match's places, the local player's among them, and its voice channel.
#[derive(Clone, Copy)]
pub struct Roster {
    seats: [Option<Seat>; Place::COUNT],
    local: Option<Place>,
    channel: Channel,
}

impl Roster {
    pub const EMPTY: Roster = Roster { seats: [None; Place::COUNT], local: None, channel: Channel::Team };

    fn of(client: &NetworkClient<'_>, bodies: bool, channel: Channel) -> Roster {
        let mut roster = Roster { channel, ..Roster::EMPTY };
        let teams = client.place_teams();
        for place in Place::all() {
            let Some(team) = teams[place.index()] else { continue };
            let body = if bodies {
                client.player(u32::from(place.get())).filter(|p| p.alive() && p.inserted()).map(|p| Body { feet: p.feet() })
            } else {
                None
            };
            roster.seats[place.index()] = Some(Seat { name: client.place_name(place), team, body });
        }
        roster.local = client.local_profile().and_then(|p| Place::new(u32::from(p)));
        roster
    }

    pub fn local(&self) -> Option<Place> {
        self.local
    }

    pub fn seat(&self, place: Place) -> Option<&Seat> {
        self.seats[place.index()].as_ref()
    }

    /// Another player on the local player's voice channel: of its team (0
    /// or 1), or in a free-for-all any other in the match.
    pub fn mate(&self, place: Place) -> bool {
        let Some((me, team)) = self.local.and_then(|me| self.seat(me).map(|seat| (me, seat.team))) else {
            return false;
        };
        me != place
            && self.seat(place).is_some_and(|seat| match self.channel {
                Channel::Team => team < 2 && seat.team == team,
                Channel::Match => true,
            })
    }

    pub fn mates(&self) -> impl Iterator<Item = (Place, &Seat)> {
        Place::all().filter(|&p| self.mate(p)).filter_map(|p| self.seat(p).map(|seat| (p, seat)))
    }

    #[cfg(test)]
    pub fn set_channel(&mut self, channel: Channel) {
        self.channel = channel;
    }

    #[cfg(test)]
    pub fn set(&mut self, place: Place, seat: Seat) {
        self.seats[place.index()] = Some(seat);
    }

    #[cfg(test)]
    pub fn set_local(&mut self, place: Option<Place>) {
        self.local = place;
    }
}

/// A record's name (64 bytes; its 64th never read, so the text ends even
/// when the record's does not).
pub fn name_text(name: &[u8; 64]) -> &CStr {
    let end = name[..63].iter().position(|&b| b == 0).unwrap_or(63);
    CStr::from_bytes_until_nul(&name[..=end]).unwrap_or(c"")
}

/// What a hook has of the game for voice: the match's places and a movie to
/// tell.
pub struct Scene<'t> {
    game: Option<raw::Game>,
    movie: Option<Movie<'t>>,
    thread: &'t Thread,
}

impl<'t> Scene<'t> {
    pub fn thread(&self) -> &'t Thread {
        self.thread
    }

    /// The match's places on `channel`; with `bodies`, the alive players'
    /// bodies.
    pub fn roster(&self, bodies: bool, channel: Channel) -> Roster {
        self.game
            .and_then(|game| NetworkClient::of_game(game, self.thread))
            .map_or(Roster::EMPTY, |client| Roster::of(&client, bodies, channel))
    }

    pub fn movie(&self) -> Option<&Movie<'t>> {
        self.movie.as_ref()
    }

    /// Each frame of a match (`game_world.after_move`): the world's game and its hud.
    pub fn at_frame(ctx: &'t Context<'_>) -> Option<Scene<'t>> {
        Some(Scene::of(GameWorld::at_after_move(ctx)?, ctx.thread()))
    }

    /// The match over (`game_world.finish`): its world's hud, before it goes.
    pub fn at_finish(ctx: &'t Context<'_>) -> Option<Scene<'t>> {
        Some(Scene::of(GameWorld::at_finish(ctx)?, ctx.thread()))
    }

    /// The match left (`game_world.deactivate`): its world's hud, before it goes.
    pub fn at_deactivate(ctx: &'t Context<'_>) -> Option<Scene<'t>> {
        Some(Scene::of(GameWorld::at_deactivate(ctx)?, ctx.thread()))
    }

    fn of(world: GameWorld<'t>, thread: &'t Thread) -> Scene<'t> {
        // SAFETY: the world's game and its hud (inside it), alive for the hook.
        let (game, movie) = unsafe { (world.this.game(), movie_of(world.this.ui().hud_movie())) };
        Scene { game, movie, thread }
    }

    /// At a hud movie's call (`hud.callback`): the hud's game (the call
    /// answers its movie itself).
    pub fn at_call(call: &'t HudCall<'_>) -> Scene<'t> {
        Scene { game: call.hud().game(), movie: None, thread: call.thread() }
    }
}

/// user32's GetForegroundWindow, GetWindowThreadProcessId, GetAsyncKeyState
/// and MapVirtualKeyA (the game imports none of them): whether the game has
/// the keyboard and a key is still down.
#[derive(Clone, Copy)]
struct Window {
    foreground: NonNull<c_void>,
    process_of: NonNull<c_void>,
    key_state: NonNull<c_void>,
    virtual_key: NonNull<c_void>,
}

impl Window {
    fn load() -> Option<Window> {
        // SAFETY: kernel32's imports through the exe's slots; user32 is the
        // game's (its window's), loaded for the process' life.
        unsafe {
            let user = NonNull::new((raw::engine_get_module_handle_a)(c"user32.dll".as_ptr().cast()))?;
            let get = |name: &CStr| NonNull::new((raw::engine_get_proc_address)(user.as_ptr(), name.as_ptr().cast()));
            Some(Window {
                foreground: get(c"GetForegroundWindow")?,
                process_of: get(c"GetWindowThreadProcessId")?,
                key_state: get(c"GetAsyncKeyState")?,
                virtual_key: get(c"MapVirtualKeyA")?,
            })
        }
    }

    /// The foreground window is this process's.
    fn focused(self) -> bool {
        // SAFETY: user32's exports by their rows' signatures; kernel32's import.
        unsafe {
            let window = raw::engine_get_foreground_window(self.foreground);
            if window.is_null() {
                return false;
            }
            let mut owner = 0;
            raw::engine_get_window_thread_process_id(self.process_of, window, &mut owner);
            owner == (raw::engine_get_current_process_id)()
        }
    }

    /// The input is down now, or cannot be read (a DirectInput code is its
    /// scan code, bit 80h the E0h-prefixed ones: MAPVK_VSC_TO_VK_EX's 0xE0nn).
    fn down(self, input: TalkInput) -> bool {
        // SAFETY: user32's exports by their rows' signatures.
        let polled = Polled::of(input, |scan| unsafe { raw::engine_map_virtual_key_a(self.virtual_key, scan, 3) });
        match polled {
            Polled::Unknown => true,
            Polled::Keys(a, b) => [a, b].into_iter().filter(|&vk| vk != 0).any(|vk| {
                // SAFETY: user32's export by its row's signature.
                unsafe { raw::engine_get_async_key_state(self.key_state, vk) & 0x8000 != 0 }
            }),
        }
    }
}

/// The loaded, version-checked DLL. Once a check passed it stays loaded for
/// the process's life, so the token is Copy.
#[derive(Clone, Copy)]
pub struct Dll {
    api: voice_abi::Api,
    window: Option<Window>,
}

impl Dll {
    /// None without the file, or for one whose exports or version are not
    /// this ABI's (that one freed again).
    pub fn load(_: &Thread) -> Option<Dll> {
        // SAFETY: kernel32's imports through the exe's slots; Api::load
        // checks every export and the version before it is made.
        unsafe {
            let module = NonNull::new((raw::engine_load_library_a)(DLL.as_ptr().cast()))?;
            let api = voice_abi::Api::load(|name| (raw::engine_get_proc_address)(module.as_ptr(), name.as_ptr().cast()));
            let Some(api) = api else {
                (raw::engine_free_library)(module.as_ptr());
                return None;
            };
            Some(Dll { api, window: Window::load() })
        }
    }

    pub fn start(self, connect: &Connect<'_>) -> bool {
        // SAFETY: the checked API; the host a C string alive for the call
        // (the DLL copies it).
        unsafe { (self.api.start)(connect.host.as_ptr(), u32::from(connect.port), connect.session) == 0 }
    }

    pub fn stop(self) {
        // SAFETY: the checked API.
        unsafe { (self.api.stop)() }
    }

    pub fn talk(self, on: bool) {
        // SAFETY: the checked API, a scalar.
        unsafe { (self.api.ptt)(i32::from(on)) }
    }

    pub fn settings(self, settings: Settings) {
        let (volume, microphone, sensitivity) = match settings {
            Settings::Off => (0.0, false, 0.0),
            Settings::On { volume, sensitivity, microphone } => (volume.get(), microphone, sensitivity.get()),
        };
        // SAFETY: the checked API, scalars (each 0..1).
        unsafe { (self.api.settings)(volume, i32::from(microphone), sensitivity) }
    }

    pub fn mute(self, places: Places) {
        // SAFETY: the checked API, a mask.
        unsafe { (self.api.mute)(places.0) }
    }

    /// Tells how every place is heard, what changed since `before` (all of
    /// it without one): a place's whole state in one call.
    pub fn hearing(self, hearing: Hearing, before: Option<Hearing>) {
        for place in Place::all() {
            let heard = hearing.of(place);
            if before.map(|b| b.of(place)) != Some(heard) {
                let (left, right, radio) = heard.call();
                // SAFETY: the checked API, a place below 20 and scalars.
                unsafe { (self.api.hear)(i32::from(place.get()), left, right, radio) }
            }
        }
    }

    /// The places talking now (the DLL's word): the teammates it plays, and
    /// the local player's own place while its microphone is sent (the relay
    /// has answered and the capture runs). Bit 31, the microphone sent before
    /// the relay gave a place, is not one of them.
    pub fn talking(self) -> Places {
        // SAFETY: the checked API.
        Places::of_word(unsafe { (self.api.talking)() })
    }

    /// For the voice log: the place the relay gave this game (below 0: no
    /// answer yet) and the DLL's whole talking word, bit 31 too.
    pub fn state(self) -> (i32, u32) {
        // SAFETY: the checked API.
        unsafe { ((self.api.slot)(), (self.api.talking)()) }
    }

    /// The place's voice id, from the roster the relay sent.
    pub fn voice_id(self, place: Place) -> Option<VoiceId> {
        // SAFETY: the checked API, a place below 20.
        VoiceId::new(unsafe { (self.api.voice_id)(i32::from(place.get())) })
    }

    /// The Settings' "Test microphone" button pressed: the DLL starts its
    /// test with these sliders, or stops the one that runs. In a match or
    /// not.
    pub fn test_press(self, sliders: Sliders) {
        // SAFETY: the checked API, scalars (each 0..1).
        unsafe { (self.api.test_press)(sliders.volume.get(), sliders.sensitivity.get()) }
    }

    /// The microphone test stopped, if one runs.
    pub fn test_stop(self) {
        // SAFETY: the checked API.
        unsafe { (self.api.test_stop)() }
    }

    /// What the microphone test does now. The DLL's word is one of this
    /// ABI's (the version was checked); another would read as no test.
    pub fn test(self) -> MicTest {
        // SAFETY: the checked API.
        MicTest::of_word(unsafe { (self.api.test_state)() }).unwrap_or(MicTest::Idle)
    }

    /// The capture devices there are now (the DLL asks Windows), in its
    /// order, each read once: its ID and its name (UTF-8). At most the room's
    /// length; one the DLL did not write as an ABI's device is left out.
    pub fn microphones(self, room: &mut [Microphone]) -> impl Iterator<Item = (MicrophoneId, &CStr)> {
        let size = u32::try_from(room.len()).unwrap_or(u32::MAX);
        // SAFETY: the checked API; room for `size` devices, which the DLL
        // fills (at most) for the call.
        let written = unsafe { (self.api.microphones)(room.as_mut_ptr(), size) };
        let written = (written as usize).min(room.len());
        room[..written].iter().filter_map(Microphone::read)
    }

    /// The capture device push-to-talk and the microphone test open from
    /// their next opening.
    pub fn choose_microphone(self, choice: MicrophoneChoice) {
        let id = match &choice {
            MicrophoneChoice::Default => core::ptr::null(),
            MicrophoneChoice::Device(id) => id.as_c_str().as_ptr(),
        };
        // SAFETY: the checked API; null or a C string alive for the call
        // (the DLL copies it).
        unsafe { (self.api.choose_microphone)(id) }
    }

    /// The game has the keyboard (so when user32 lacked the functions).
    pub fn focused(self) -> bool {
        self.window.is_none_or(Window::focused)
    }

    /// The input is still down (so when user32 lacked the functions, or it
    /// cannot be read: its up event and the DLL's 30 s limit still end a press).
    pub fn key_down(self, input: TalkInput) -> bool {
        self.window.is_none_or(|window| window.down(input))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // MapVirtualKeyA(MAPVK_VSC_TO_VK_EX) as Windows maps these scan codes.
    fn windows(scan: u32) -> u32 {
        match scan {
            0x19 => 0x50,   // P
            0x52 => 0x2D,   // numpad 0 -> VK_INSERT
            0x47 => 0x24,   // numpad 7 -> VK_HOME
            0x53 => 0x2E,   // numpad . -> VK_DELETE
            0xE01D => 0xA3, // right Ctrl
            _ => 0,
        }
    }

    #[test]
    fn a_microphone_choice_is_the_default_or_an_id_and_reads_back() {
        let id = b"{0.0.1.00000000}.{6b1a1d3c-1f9e-4c0e-9a59-0d2b5e3f7a10}";
        let device = MicrophoneChoice::parse(id).unwrap();
        assert_eq!(device, MicrophoneChoice::Device(MicrophoneId::parse(id).unwrap()));
        assert_eq!(device.text().to_bytes(), id);
        assert_eq!(MicrophoneChoice::parse(b""), Some(MicrophoneChoice::Default));
        assert_eq!(MicrophoneChoice::Default.text(), c"");
        for bad in [&b" "[..], b"a b", b"\xD0\x9C", &[b'a'; 200]] {
            assert_eq!(MicrophoneChoice::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn the_test_buttons_press_carries_the_two_sliders() {
        let sliders = |volume, sensitivity| {
            Some(Sliders { volume: Fraction::of_percent(volume), sensitivity: Fraction::of_percent(sensitivity) })
        };
        assert_eq!(Sliders::parse(c"100 50"), sliders(100.0, 50.0));
        assert_eq!(Sliders::parse(c"0 0"), sliders(0.0, 0.0));
        assert_eq!(Sliders::parse(c"7 100"), sliders(7.0, 100.0));
        assert_eq!(Sliders::parse(c"7 100").map(|s| (s.volume.get(), s.sensitivity.get())), Some((0.07, 1.0)));
        for bad in [c"", c"50", c"50 ", c" 50", c"50  50", c"50 50 50", c"101 50", c"50 101", c"1000 5", c"5.5 5", c"-1 5",
                    c"a b", c"50,50", c"stop"] {
            assert_eq!(Sliders::parse(bad), None, "{bad:?}");
        }
        assert_eq!(TestAsk::parse(c"35 80"), Sliders::parse(c"35 80").map(TestAsk::Press));
        assert_eq!(TestAsk::parse(c"stop"), Some(TestAsk::Stop));
        assert_eq!(TestAsk::parse(c"Stop"), None);
        assert_eq!(TestAsk::parse(c""), None);
    }

    #[test]
    fn a_held_input_is_read_by_the_keys_a_press_sets() {
        let key = |code| Polled::of(TalkInput::Key(Dik::new(code)), windows);
        assert_eq!(key(0x19), Polled::Keys(0x50, 0));
        assert_eq!(key(0x9D), Polled::Keys(0xA3, 0));
        // NumLock on or off: the navigation key or the digit
        assert_eq!(key(0x52), Polled::Keys(0x2D, VK_NUMPAD0));
        assert_eq!(key(0x47), Polled::Keys(0x24, VK_NUMPAD0 + 7));
        assert_eq!(key(0x53), Polled::Keys(0x2E, VK_DECIMAL));
        assert_eq!(key(0x45), Polled::Keys(VK_NUMLOCK, 0));
        assert_eq!(key(0xC5), Polled::Keys(VK_PAUSE, 0));
        assert_eq!(key(0x7F), Polled::Unknown);
        let button = |n| Polled::of(TalkInput::Button(MouseButton::new(n)), |_| panic!("a button maps no scan code"));
        assert_eq!(button(0), Polled::Keys(VK_LBUTTON, 0));
        assert_eq!(button(3), Polled::Keys(VK_XBUTTON1, 0));
        assert_eq!(button(4), Polled::Keys(VK_XBUTTON2, 0));
        assert_eq!(button(5), Polled::Unknown);
    }
}
