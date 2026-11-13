//! The match's interface: survarium::game_world_ui (the hud) and the
//! Scaleform movies it holds, the hud's and the player icons', at the site
//! `player_icons.update_start` (update_player_icons' start, 0x23AA10: the ui
//! its argument).

use core::ffi::CStr;
use core::marker::PhantomData;

use super::context::{Context, Reg};
use super::hook::Thread;
use super::types::Place;
use super::world::read;
use crate::raw;

/// A character event delivered after a text field lost its AS3 receiver.
/// Only this detached state permits skipping native dispatch/insertion.
pub struct DetachedTextInput<'c, 'm> {
    ctx: &'c mut Context<'m>,
}

impl<'c, 'm> DetachedTextInput<'c, 'm> {
    pub fn at_char_event(ctx: &'c mut Context<'m>) -> Option<Self> {
        // SAFETY: the context is text_input.char_target's: the live
        // AvmTextField interface, on the game's thread, whose two AS3 pointers
        // this site's native code reads next. The collectible low bit is the
        // GC mark, not part of its pointer.
        let target = unsafe {
            let field = raw::site::text_input_char_target::field(&*ctx)?;
            let raw = *field.as3_raw();
            if raw != 0 { raw } else { *field.as3_collectible() }
        } & !1;
        if target != 0 { return None; }
        Some(Self { ctx })
    }

    pub fn ignore(self) {
        // The original epilogue returns BL as AL; its stack restores EBX.
        self.ctx.set_reg8(Reg::Ebx, 0);
    }
}

/// A teammate's name decision at `player_names.visibility` (0x1F2F10).
/// The handle can only be obtained for a valid same-team player, and cannot
/// escape the hook. The engine already checked life, visibility and projection.
pub struct NameVisibility<'c, 'm> {
    ctx: &'c mut Context<'m>,
}

impl<'c, 'm> NameVisibility<'c, 'm> {
    pub fn for_teammate(ctx: &'c mut Context<'m>) -> Option<Self> {
        use raw::site::player_names_visibility as site;
        // SAFETY: the context is this site's: render_name's live player and
        // its frame, on the game's thread. The profile is the same one the
        // original code reads at 0x1F2F33, before any call can change it.
        let (local, team) = unsafe {
            let (player, frame) = (site::player(&*ctx)?, site::frame(&*ctx)?);
            (*frame.local_team(), *player.profile()?.team())
        };
        if local > 1 || local != team {
            return None;
        }
        Some(Self { ctx })
    }

    /// Show this eligible teammate's name; never modify input or player state.
    pub fn show(self) {
        self.ctx.set_reg8(Reg::Eax, 1);
    }
}



/// The hud, survarium::game_world_ui, as a hook's site has it.
pub struct GameWorldUi<'t> {
    this: raw::GameWorldUi,
    thread: PhantomData<&'t Thread>,
}

impl<'t> GameWorldUi<'t> {
    /// The hud a sibling module holds (alive with its match on the game's thread).
    pub(super) fn of(this: raw::GameWorldUi) -> GameWorldUi<'t> {
        GameWorldUi { this, thread: PhantomData }
    }

    /// At `player_icons.update_start`.
    pub fn at_update_icons(ctx: &'t Context<'_>) -> Option<GameWorldUi<'t>> {
        // SAFETY: the context is this site's: update_player_icons' frame, its
        // argument the ui, alive while it runs on the game's thread.
        let this = unsafe { raw::site::player_icons_update_start::frame(ctx)?.ui() }?;
        Some(GameWorldUi { this, thread: PhantomData })
    }

    /// At `player_icons.enemy_ids` (initialize_match, about to send its enemy
    /// list to the icons movie).
    pub fn at_enemy_ids(ctx: &'t Context<'_>) -> Option<GameWorldUi<'t>> {
        // SAFETY: the context is this site's: initialize_match's ui in ESI,
        // alive while it runs on the game's thread.
        let this = unsafe { raw::site::player_icons_enemy_ids::ui(ctx) }?;
        Some(GameWorldUi { this, thread: PhantomData })
    }

    /// At `game_world_ui.welcome` (update_ui's mark of the welcome message,
    /// 0x23B36A: EDI the ui).
    pub fn at_welcome(ctx: &'t Context<'_>) -> Option<GameWorldUi<'t>> {
        // SAFETY: the context is this site's: EDI update_ui's hud, alive
        // while it runs on the game's thread.
        let this = unsafe { raw::site::game_world_ui_welcome::ui(ctx) }?;
        Some(GameWorldUi { this, thread: PhantomData })
    }

    /// The place the hud shows the match of (its state's first byte, which
    /// game_world::tick writes each frame from the network client's player,
    /// +4: the local player, in a replay the one the camera is on, as
    /// is_player_current 0x242A40 reads it); None without one.
    pub fn viewed_place(&self) -> Option<Place> {
        // SAFETY: the ui is alive for the hook; a byte of its hud state.
        Place::new(u32::from(unsafe { read(self.this.viewed_place()) }))
    }

    /// The exact deadline against the completed simulation clock. The
    /// displayed whole seconds may already be zero while play continues.
    pub fn clock(&self) -> Option<super::MatchClock> {
        // SAFETY: the ui's world and its match core live for this hook.
        unsafe {
            let core = self.this.world()?.pvp_match()?.core();
            super::match_rules::clock_of(core, read(core.time()))
        }
    }

    /// The kills of `place` as the kd rule filled the hud state this frame.
    /// The rule fills the match's places only: a place past them holds what
    /// an earlier match left.
    pub fn kills(&self, place: Place) -> u16 {
        // SAFETY: the ui is alive for the hook; the hud state holds a record
        // for each of the twenty places.
        unsafe { read(self.this.kd().at(place.index()).kills()) }
    }

    /// The hud movie (hud.swf), when the hud has it.
    pub fn hud_movie(&self) -> Option<Movie<'t>> {
        // SAFETY: the ui is alive for the hook, on its thread.
        unsafe { movie_of(self.this.hud_movie()) }
    }

    /// The player icons movie (player_icons.swf, the game world's own), when the hud has it.
    pub fn icons_movie(&self) -> Option<Movie<'t>> {
        // SAFETY: as hud_movie.
        unsafe { movie_of(self.this.icons_movie()) }
    }

    /// A dictionary text of the running language (text_translator::
    /// translate_text 0x1FF0F0 of the ui's game; the key itself when the
    /// dictionary has none) into `out`; empty without a game.
    pub fn translate<'o>(&self, key: &CStr, out: &'o mut [u8; 0x200]) -> &'o CStr {
        out[0] = 0;
        // SAFETY: the ui's world and its game, alive for the hook;
        // translate_text copies a text shorter than 200h bytes with its zero
        // into `out` (0x1FF15D) and leaves it alone otherwise.
        unsafe {
            if let Some(game) = self.this.world().and_then(|world| world.game()) {
                raw::engine_translate_text(Some(game.text_translator()), key.as_ptr().cast(), out.as_mut_ptr());
            }
        }
        out[0x1FF] = 0;
        CStr::from_bytes_until_nul(out).unwrap_or(c"")
    }

    /// The live match's roster, from the exact options object that
    /// lj_native_install updates (pvp_match+124h). The network client's
    /// separately accessible options need not be the same object.
    pub fn places(&self) -> Option<u8> {
        // SAFETY: the ui's world and match wrapper live for the hook;
        // the wrapper retains these options throughout the match.
        unsafe {
            let places = read(self.this.world()?.pvp_match()?.options()?.places());
            (usize::from(places) <= Place::COUNT).then_some(places)
        }
    }
}

/// The movie a holder shows (its view's, as the game reads it, 0x2384EE).
///
/// # Safety
/// `holder` is alive on the hook's thread.
pub(super) unsafe fn movie_of<'t>(holder: Option<raw::MovieHolder>) -> Option<Movie<'t>> {
    // SAFETY: the caller's: a live holder; its view and the view's movie live as long.
    let this = unsafe { holder?.view()?.movie() }?;
    Some(Movie { this, thread: PhantomData })
}

/// A Scaleform movie the hud holds.
pub struct Movie<'t> {
    this: raw::GfxMovie,
    thread: PhantomData<&'t Thread>,
}

/// A GFx value's type (Scaleform::GFx::Value's, 0x2384E2): a bool, an
/// unsigned int, a string; with the managed bit, a string its node holds.
const GFX_BOOL: u32 = 2;
pub(super) const GFX_UINT: u32 = 4;
pub(super) const GFX_STRING: u32 = 6;
pub(super) const GFX_MANAGED: u32 = 0x40;

/// Up to this many strings in one Invoke.
pub const MAX_STRINGS: usize = 64;
/// Up to this many plain unsigned values in one Invoke.
pub const MAX_UINTS: usize = 4;
/// The longest text a GFx argument may be here.
pub(super) const TEXT_LIMIT: usize = 0x200;

/// The C string at `text`, read up to `limit` bytes for its zero (none in
/// them: not a string here). core's `CStr::from_ptr` would call a strlen the
/// arena does not have.
///
/// # Safety
/// `text` is readable up to its zero or `limit` bytes, alive for `'a`.
pub(super) unsafe fn c_string<'a>(text: *const u8, limit: usize) -> Option<&'a CStr> {
    if text.is_null() {
        return None;
    }
    let mut len = 0;
    // SAFETY: the caller's: readable up to the zero or `limit`.
    while len < limit && unsafe { *text.add(len) } != 0 {
        len += 1;
    }
    // SAFETY: as above; `len` bytes and the zero after them.
    (len < limit).then(|| unsafe { CStr::from_bytes_with_nul_unchecked(core::slice::from_raw_parts(text, len + 1)) })
}

/// A movie's ExternalInterface call's argument `index` when it is a string
/// (a scaleform_callback's arguments: `count` GFx values from `values`).
///
/// # Safety
/// `values` holds `count` GFx values, alive for `'a`; a string's text
/// (managed: its node's) a C string as long.
pub(super) unsafe fn string_argument<'a>(values: Option<raw::GfxValue>, count: u32, index: u32) -> Option<&'a CStr> {
    if index >= count {
        return None;
    }
    // SAFETY: the caller's: `index` is below the count.
    unsafe {
        let value = values?.at(index as usize);
        let kind = *value.kind();
        if kind & 0x3F != GFX_STRING {
            return None;
        }
        let text = if kind & GFX_MANAGED != 0 {
            value.managed().map_or(core::ptr::null_mut(), |node| node.text())
        } else {
            value.text()
        };
        c_string(text, TEXT_LIMIT)
    }
}

impl Movie<'_> {
    pub fn invoke_text(&self, method: &CStr, value: &CStr) -> bool {
        self.invoke_strings(method, &[value])
    }

    /// Which movie this is (a new game world's is another), to tell it once.
    pub fn id(&self) -> usize {
        self.this.address() as usize
    }

    /// Invokes the movie's `method` with our `values` (Movie::Invoke
    /// 0x511600, no result); whether the movie took it.
    fn invoke(&self, method: &CStr, values: &mut [raw::GfxValueStorage]) -> bool {
        let count = values.len() as u32;
        // SAFETY: a live movie on the hook's thread; the name a C string and
        // the arguments our GFx values, whose texts outlive the call, as the
        // game's own Invokes pass them.
        let taken = unsafe {
            raw::engine_movie_invoke(Some(self.this), method.as_ptr().cast(), None,
                                     raw::GfxValueStorage::first(values), count)
        };
        taken & 0xFF != 0
    }

    /// Invokes the movie's `method` with one bool; whether the movie took it
    /// (it has that method: its root is made).
    pub fn invoke_bool(&self, method: &CStr, value: bool) -> bool {
        self.invoke_bools(method, [value])
    }

    /// Invokes the movie's `method` with these bools; whether the movie took
    /// it.
    pub fn invoke_bools<const N: usize>(&self, method: &CStr, values: [bool; N]) -> bool {
        let mut arguments = [const { raw::GfxValueStorage::zeroed() }; N];
        for (room, value) in arguments.iter_mut().zip(values) {
            let gfx = room.handle();
            // SAFETY: our own value.
            unsafe {
                *gfx.kind() = GFX_BOOL;
                *gfx.value() = u32::from(value);
            }
        }
        self.invoke(method, &mut arguments)
    }

    /// Invokes the movie's `method` with these strings (UTF-8, as the
    /// dictionaries are), at most [`MAX_STRINGS`]; whether the movie took it.
    pub fn invoke_strings(&self, method: &CStr, values: &[&CStr]) -> bool {
        let mut arguments = [const { raw::GfxValueStorage::zeroed() }; MAX_STRINGS];
        let count = values.len().min(MAX_STRINGS);
        for (room, text) in arguments.iter_mut().zip(&values[..count]) {
            let gfx = room.handle();
            // SAFETY: our own value; unmanaged, its text the caller's, alive
            // for the call.
            unsafe {
                *gfx.kind() = GFX_STRING;
                gfx.set_text(text.as_ptr().cast::<u8>().cast_mut());
            }
        }
        self.invoke(method, &mut arguments[..count])
    }

    /// The hud's centre message with a text of ours, for `time_ms`: the
    /// movie's `root.set_parameterized_message(text, 21h, 14h, time)`, as the
    /// game's show_parametrized_message invokes it for a dictionary key
    /// (0x2376CF; the font size and the shift are the welcome's, 0x23B360).
    /// Whether the movie took it.
    pub fn show_message(&self, text: &CStr, time_ms: u32) -> bool {
        let mut arguments = [const { raw::GfxValueStorage::zeroed() }; 4];
        let [message, numbers @ ..] = &mut arguments;
        let gfx = message.handle();
        // SAFETY: our own values; the string unmanaged, its text the
        // caller's, alive for the call.
        unsafe {
            *gfx.kind() = GFX_STRING;
            gfx.set_text(text.as_ptr().cast::<u8>().cast_mut());
            for (room, value) in numbers.iter_mut().zip([0x21, 0x14, time_ms]) {
                let gfx = room.handle();
                *gfx.kind() = GFX_UINT;
                *gfx.value() = value;
            }
        }
        self.invoke(c"root.set_parameterized_message", &mut arguments)
    }

    /// Plain unsigned GFx values, at most [`MAX_UINTS`]; whether the movie took them.
    pub fn invoke_uints(&self, method: &CStr, values: &[u32]) -> bool {
        let mut arguments = [const { raw::GfxValueStorage::zeroed() }; MAX_UINTS];
        let count = values.len().min(MAX_UINTS);
        for (room, &value) in arguments.iter_mut().zip(&values[..count]) {
            let gfx = room.handle();
            // SAFETY: our own value.
            unsafe {
                *gfx.kind() = GFX_UINT;
                *gfx.value() = value;
            }
        }
        self.invoke(method, &mut arguments[..count])
    }
}

/// The mouse buttons the game hands a movie: 0x2AF610's callers map codes 151h.. to 0..2 (patch 102
/// takes the others away).
const MOUSE_BUTTONS: u32 = 3;

/// The buttons of a movie's held mask (MovieImpl's CurButtonsState) that the game could have pressed.
fn held_buttons(mask: u32) -> impl Iterator<Item = u32> {
    (0..MOUSE_BUTTONS).filter(move |button| mask >> button & 1 != 0)
}

/// GFx::MouseEvent's Event::MouseUp.
const MOUSE_UP: u32 = 3;

/// `button` let go off the stage, into `room` (GFx::MouseEvent, as 0x2AF610 fills it): over no
/// control, so a press in progress ends (MOUSE_UP on the stage, release outside on what was
/// pressed) and nothing is clicked.
fn release_off_stage(room: &mut raw::GfxMouseEventStorage, button: u32) -> raw::GfxMouseEvent {
    let event = room.handle();
    // SAFETY: our own event, its fields' places.
    unsafe {
        *event.kind() = MOUSE_UP;
        *event.modifiers() = 0;
        *event.x() = -1.0;
        *event.y() = -1.0;
        *event.scroll() = 0.0;
        *event.button() = button;
        *event.mouse() = 0;
    }
    event
}

impl Movie<'_> {
    /// Lets go of every button the movie still holds. A movie makes a press of a button's change only
    /// (GenerateMouseEvents), so one that never saw a release takes the next press as no press at all.
    /// HandleEvent queues it for the movie's next Advance, as it does every mouse event.
    pub fn release_held_buttons(&self) {
        // SAFETY: a live movie on the hook's thread; its first MouseState's buttons, a plain dword.
        let mask = unsafe { self.this.mouse_buttons().read() };
        for button in held_buttons(mask) {
            let mut room = raw::GfxMouseEventStorage::zeroed();
            let event = release_off_stage(&mut room, button);
            // SAFETY: a live movie on its thread; the event outlives the call, which copies what it needs.
            unsafe { raw::engine_movie_handle_event(Some(self.this), Some(event)) };
        }
    }
}

#[cfg(test)]
mod mouse_release_tests {
    use super::*;

    fn held(mask: u32) -> [Option<u32>; 3] {
        let mut buttons = held_buttons(mask);
        [buttons.next(), buttons.next(), buttons.next()]
    }

    #[test]
    fn only_the_buttons_held_are_let_go() {
        assert_eq!(held(0), [None, None, None]);
        assert_eq!(held(1), [Some(0), None, None]);
        assert_eq!(held(0b10), [Some(1), None, None]);
        assert_eq!(held(0b101), [Some(0), Some(2), None]);
        // Bits the game never presses.
        assert_eq!(held(0xFFFF_FFF8), [None, None, None]);
    }

    #[test]
    fn the_event_is_the_one_the_game_builds() {
        assert_eq!(raw::GfxMouseEvent::SIZE, 0x1C);
        let mut room = raw::GfxMouseEventStorage::zeroed();
        let event = release_off_stage(&mut room, 2);
        // SAFETY: the event just built in `room`.
        let words = unsafe {
            [*event.kind(), *event.modifiers(), (*event.x()).to_bits(), (*event.y()).to_bits(), (*event.scroll()).to_bits(), *event.button(), *event.mouse()]
        };
        assert_eq!(words, [3, 0, (-1.0f32).to_bits(), (-1.0f32).to_bits(), 0, 2, 0]);
    }
}
