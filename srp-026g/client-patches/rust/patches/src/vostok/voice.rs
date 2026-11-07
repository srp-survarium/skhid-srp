//! Patch 66's boundaries (voice chat, docs/notes/voice-chat.md): the voice's
//! console variables, the match a connect starts, the chat keys, the match's
//! places as a frame or a menu sees them, the movies, and the optional
//! versioned DLL (`skhid_voice.dll`, `voice_abi`). Every game and DLL value
//! is read here once into a type; who talks, who is heard and who is muted
//! is the patch's (`crate::voice`).

use core::ffi::{CStr, c_void};
use core::num::NonZeroU32;
use core::ptr::NonNull;

use super::context::Context;
use super::hook::Thread;
use super::input::Action;
use super::keyboard::{Dik, GameWorldKey, KeyKind};
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

/// The variables in the engine's list before user.cfg (`configs.start`):
/// the stock sound tab's controls name three of them, user.cfg keeps all.
pub fn register(thread: &Thread) {
    VOLUME.register(thread, c"s_chat_volume");
    SENSITIVITY.register(thread, c"s_mic_sens");
    MICROPHONE.register(thread, c"s_use_microphone");
    OFF.register(thread, c"g_hide_spam");
    MUTES.register(thread, c"srp_voice_mutes");
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
}

/// A chat key of voice's (the binder's kind 8, the chat keys' group).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatKey {
    /// `ptt` (47h): push-to-talk.
    Talk,
    /// `mute_last_speaker` (37h, the debug entry renamed).
    MuteLast,
}

/// A press or a release of one, at the game world's keys' start (`tab_cursor.key`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatKeyEvent {
    Down(ChatKey, Dik),
    Up(ChatKey, Dik),
}

impl ChatKeyEvent {
    /// The event when the key is bound to one of the two (a held key's
    /// repeats are none).
    pub fn of(key: &GameWorldKey<'_, '_>) -> Option<ChatKeyEvent> {
        let chat = match key.chat_action()? {
            Action::PTT => ChatKey::Talk,
            Action::MUTE_LAST_SPEAKER => ChatKey::MuteLast,
            _ => return None,
        };
        match key.kind() {
            KeyKind::Down => Some(ChatKeyEvent::Down(chat, key.key())),
            KeyKind::Up => Some(ChatKeyEvent::Up(chat, key.key())),
            KeyKind::Held => None,
        }
    }
}

/// A place of the match as voice needs it: the name and team of its record
/// (the match options') and, while its player is alive in the world, where.
#[derive(Clone, Copy)]
pub struct Seat {
    pub name: [u8; 64],
    pub team: u32,
    pub position: Option<[f32; 3]>,
}

/// The match's places, the local player's among them.
#[derive(Clone, Copy)]
pub struct Roster {
    seats: [Option<Seat>; Place::COUNT],
    local: Option<Place>,
}

impl Roster {
    pub const EMPTY: Roster = Roster { seats: [None; Place::COUNT], local: None };

    fn of(client: &NetworkClient<'_>, positions: bool) -> Roster {
        let mut roster = Roster::EMPTY;
        let teams = client.place_teams();
        for place in Place::all() {
            let Some(team) = teams[place.index()] else { continue };
            let position = if positions {
                client.player(u32::from(place.get())).filter(|p| p.alive() && p.inserted()).map(|p| p.feet())
            } else {
                None
            };
            roster.seats[place.index()] = Some(Seat { name: client.place_name(place), team, position });
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

    /// Another player of the local player's team (0 or 1).
    pub fn teammate(&self, place: Place) -> bool {
        let Some((me, team)) = self.local.and_then(|me| self.seat(me).map(|seat| (me, seat.team))) else {
            return false;
        };
        me != place && team < 2 && self.seat(place).is_some_and(|seat| seat.team == team)
    }

    pub fn teammates(&self) -> impl Iterator<Item = (Place, &Seat)> {
        Place::all().filter(|&p| self.teammate(p)).filter_map(|p| self.seat(p).map(|seat| (p, seat)))
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

/// A text of the running language (text_translator::translate_text
/// 0x1FF0F0; the key itself when the dictionary has none) into `out`.
fn translate<'o>(game: raw::Game, key: &CStr, out: &'o mut [u8; 0x200]) -> &'o CStr {
    out[0] = 0;
    // SAFETY: the game and its text translator, alive for the hook;
    // translate_text copies a text shorter than 200h bytes with its zero
    // into `out` (0x1FF15D) and leaves it alone otherwise.
    unsafe { raw::engine_translate_text(Some(game.text_translator()), key.as_ptr().cast(), out.as_mut_ptr()) };
    out[0x1FF] = 0;
    CStr::from_bytes_until_nul(out).unwrap_or(c"")
}

/// A record's name (64 bytes; its 64th never read, so the text ends even
/// when the record's does not).
pub fn name_text(name: &[u8; 64]) -> &CStr {
    let end = name[..63].iter().position(|&b| b == 0).unwrap_or(63);
    CStr::from_bytes_until_nul(&name[..=end]).unwrap_or(c"")
}

/// What a hook has of the game for voice: the match's places, a movie to
/// tell and the language's texts.
pub struct Scene<'t> {
    game: Option<raw::Game>,
    movie: Option<Movie<'t>>,
    thread: &'t Thread,
}

impl<'t> Scene<'t> {
    pub fn thread(&self) -> &'t Thread {
        self.thread
    }

    /// The match's places; with `positions`, the alive players' feet.
    pub fn roster(&self, positions: bool) -> Roster {
        self.game
            .and_then(|game| NetworkClient::of_game(game, self.thread))
            .map_or(Roster::EMPTY, |client| Roster::of(&client, positions))
    }

    pub fn movie(&self) -> Option<&Movie<'t>> {
        self.movie.as_ref()
    }

    pub fn translate<'o>(&self, key: &CStr, out: &'o mut [u8; 0x200]) -> &'o CStr {
        match self.game {
            Some(game) => translate(game, key, out),
            None => c"",
        }
    }

    /// Each frame of a match (`game_world.after_move`): the world's game and its hud.
    pub fn at_frame(ctx: &'t Context<'_>) -> Option<Scene<'t>> {
        let world = GameWorld::at_after_move(ctx)?;
        // SAFETY: the world's game and its hud (inside it), alive for the hook.
        let (game, movie) = unsafe { (world.this.game(), movie_of(world.this.ui().hud_movie())) };
        Some(Scene { game, movie, thread: ctx.thread() })
    }

    /// At a chat key (`tab_cursor.key`): the hud's game and movie.
    pub fn at_key(key: &'t GameWorldKey<'_, '_>) -> Scene<'t> {
        let hud = key.hud();
        Scene { game: hud.game(), movie: hud.movie(), thread: key.thread() }
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

    /// The key is down now (a DirectInput code is its scan code, bit 80h
    /// the E0h-prefixed ones: MAPVK_VSC_TO_VK_EX's 0xE0nn).
    fn down(self, key: Dik) -> bool {
        let code = u32::from(key.code());
        let scan = if code & 0x80 != 0 { 0xE000 | (code & 0x7F) } else { code };
        // SAFETY: user32's exports by their rows' signatures.
        unsafe {
            let vk = raw::engine_map_virtual_key_a(self.virtual_key, scan, 3);
            vk != 0 && raw::engine_get_async_key_state(self.key_state, vk) & 0x8000 != 0
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

    pub fn radio(self, places: Places) {
        // SAFETY: the checked API, a mask.
        unsafe { (self.api.radio)(places.0) }
    }

    /// The teammates it plays now (the DLL's word; its bit 31, the local
    /// microphone, is not one of them).
    pub fn talking(self) -> Places {
        // SAFETY: the checked API.
        Places::of_word(unsafe { (self.api.talking)() })
    }

    /// The teammate heard last (the DLL's -1: none yet).
    pub fn last_speaker(self) -> Option<Place> {
        // SAFETY: the checked API.
        let n = unsafe { (self.api.last_speaker)() };
        u32::try_from(n).ok().and_then(Place::new)
    }

    /// The place's voice id, from the roster the relay sent.
    pub fn voice_id(self, place: Place) -> Option<VoiceId> {
        // SAFETY: the checked API, a place below 20.
        VoiceId::new(unsafe { (self.api.voice_id)(i32::from(place.get())) })
    }

    /// The game has the keyboard (so when user32 lacked the functions).
    pub fn focused(self) -> bool {
        self.window.is_none_or(Window::focused)
    }

    /// The key is still down (so when user32 lacked the functions: its up
    /// event and the DLL's 30 s limit still end a press).
    pub fn key_down(self, key: Dik) -> bool {
        self.window.is_none_or(|window| window.down(key))
    }
}

/// The action table entry 37h's new name (patch_026g.py's VOICE_ACTION
/// write points the entry at it).
#[unsafe(no_mangle)]
#[unsafe(link_section = ".data")]
pub static mute_action_name: [u8; 18] = *b"mute_last_speaker\0";
