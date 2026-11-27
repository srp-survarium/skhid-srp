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
    /// The hud a site of `vostok` holds, alive on the hook's thread for `'t`.
    pub(super) fn of(this: raw::GameWorldUi, _: &'t Thread) -> GameWorldUi<'t> {
        GameWorldUi { this, thread: PhantomData }
    }

    /// Its handle, for `vostok`'s own reads.
    pub(super) fn raw(&self) -> raw::GameWorldUi {
        self.this
    }

    /// The hud's game world (+18h).
    pub fn game_world(&self, thread: &'t Thread) -> Option<super::world::GameWorld<'t>> {
        // SAFETY: the live ui's world, alive while the hud is.
        let world = unsafe { self.this.world() }?;
        Some(super::world::GameWorld::of(world, thread))
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

    /// At `hud.initialize_match` (0x234880, once a match, before its first frame).
    pub fn at_initialize_match(ctx: &'t Context<'_>) -> Option<GameWorldUi<'t>> {
        // SAFETY: the context is this site's: initialize_match's first argument by its ESP, the
        // hud, alive while it runs on the game's thread.
        let this = unsafe { raw::site::hud_initialize_match::frame(ctx)?.ui() }?;
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

    /// The teams' points in the hud state (+90h, +92h), which update_ui sends as the score bars
    /// (root.set_game_progress, the viewed team's first). A mode without a rule that fills them
    /// (Freeze Tag) writes them itself: nothing clears them at a new match, so the last match's
    /// stayed (the owner, 2026-11-22: "freeze tag counter doesn't reset after prev game").
    pub fn set_team_points(&self, points: [u16; 2]) {
        // SAFETY: the ui is alive for the hook, on the game's thread; two u16 of its hud state.
        unsafe {
            self.this.team_zero_points().write(points[0]);
            self.this.team_one_points().write(points[1]);
        }
    }

    /// Freeze Tag's rescue reach, from the HUD's current player to a
    /// frozen teammate. The same static-world query the rule uses.
    pub fn freeze_reachable(&self, from: srp_validate::Place, target: srp_validate::Place) -> bool {
        // SAFETY: the UI's owning match remains live for its update hook.
        // The hud outlives the match world (the results), and nothing is in reach then.
        let core = unsafe { self.this.world().and_then(|w| w.pvp_match()).map(|m| m.core()) };
        core.is_some_and(|core| super::freeze::clear(core, from, target))
    }

    /// Freeze Tag: the team of `place`'s player (its profile's), when it has one in the match.
    pub fn freeze_team(&self, place: srp_validate::Place) -> Option<srp_validate::Team> {
        // SAFETY: as freeze_reachable.
        let core = unsafe { self.this.world().and_then(|w| w.pvp_match()).map(|m| m.core()) };
        core.and_then(|core| super::freeze::team(core, place))
    }

    /// The match's world time (its core's last step), which its rule's times are of.
    pub fn now(&self) -> Option<u32> {
        // SAFETY: the ui's world and its match core live for this hook.
        unsafe { Some(read(self.this.world()?.pvp_match()?.core().time())) }
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
pub(super) const GFX_NUMBER: u32 = 5;
pub(super) const GFX_STRING: u32 = 6;
pub(super) const GFX_MANAGED: u32 = 0x40;

/// Up to this many strings in one Invoke.
pub const MAX_STRINGS: usize = 64;
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

/// `object`'s member `name` is the uint `value` (SetMember of the AS3 object interface,
/// 0x7C6A20); false when the value is no AS3 movie's object (nothing set) or the movie refused.
///
/// # Safety
/// `object` is a live GFx value of a movie on the hook's thread.
pub(super) unsafe fn set_uint_member(object: raw::GfxValue, name: &CStr, value: u32) -> bool {
    let mut room = raw::GfxValueStorage::zeroed();
    let given = room.handle();
    // SAFETY: the caller's live value; its interface is called only when its vtable is the AS3
    // interface's, whose SetMember 0x7C6A20 is that slot; the name a C string; the value ours, a
    // uint.
    unsafe {
        let Some(interface) = object.interface() else { return false };
        if *interface.vtable() != raw::image::AS3_OBJECT_INTERFACE {
            return false;
        }
        *given.kind() = GFX_UINT;
        *given.value() = value;
        let display = u32::from(*object.kind() & 0x8F == 0x0A);
        raw::engine_gfx_set_member(Some(interface), object.object(), name.as_ptr().cast(), Some(given), display) != 0
    }
}

impl Movie<'_> {
    /// Invokes the movie's `method` with the one value `value` (the game's own, which it keeps
    /// and releases); whether the movie took it.
    ///
    /// # Safety
    /// `value` is a live GFx value of this movie.
    pub(super) unsafe fn invoke_value(&self, method: &CStr, value: raw::GfxValue) -> bool {
        // SAFETY: a live movie on the hook's thread; the name a C string; the caller's value.
        let taken = unsafe { raw::engine_movie_invoke(Some(self.this), method.as_ptr().cast(), None, Some(value), 1) };
        taken & 0xFF != 0
    }

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

    /// Plain unsigned GFx values, every one (the room is sized by their
    /// count); whether the movie took them.
    pub fn invoke_uints<const N: usize>(&self, method: &CStr, values: &[u32; N]) -> bool {
        let mut arguments = [const { raw::GfxValueStorage::zeroed() }; N];
        for (room, &value) in arguments.iter_mut().zip(values) {
            let gfx = room.handle();
            // SAFETY: our own value.
            unsafe {
                *gfx.kind() = GFX_UINT;
                *gfx.value() = value;
            }
        }
        self.invoke(method, &mut arguments)
    }
}

impl Movie<'_> {
    /// GFx Numbers (type 5, an f64 each), every one (the room is sized by
    /// their count); whether the movie took them.
    pub fn invoke_numbers<const N: usize>(&self, method: &CStr, values: &[f64; N]) -> bool {
        let mut arguments = [const { raw::GfxValueStorage::zeroed() }; N];
        for (room, &value) in arguments.iter_mut().zip(values) {
            let gfx = room.handle();
            let bits = value.to_bits();
            // SAFETY: our own value: its type and the f64's two words.
            unsafe {
                *gfx.kind() = GFX_NUMBER;
                *gfx.value() = bits as u32;
                *gfx.value_high() = (bits >> 32) as u32;
            }
        }
        self.invoke(method, &mut arguments)
    }
}

/// A GFx value's type of an AS3 Object and of an Array (Scaleform::GFx::Value's VT_Object,
/// VT_Array), without the managed bit.
pub(super) const GFX_OBJECT: u32 = 8;
pub(super) const GFX_ARRAY: u32 = 9;

/// `value`'s object interface when it is the AS3 one (its vtable 0x8FC288): the only interface
/// whose slots these calls name.
///
/// # Safety
/// `value` is a live GFx value.
unsafe fn as3_interface(value: raw::GfxValue) -> Option<raw::GfxObjectInterface> {
    // SAFETY: the caller's live value; the interface's vtable only read.
    unsafe {
        let interface = value.interface()?;
        (*interface.vtable() == raw::image::AS3_OBJECT_INTERFACE).then_some(interface)
    }
}

/// `object`'s member `name` is `value` (SetMember of the AS3 object interface, 0x7C6A20); false
/// when `object` is no AS3 movie's value (nothing set) or the movie refused.
///
/// # Safety
/// `object` and `value` are live GFx values of a movie on the hook's thread.
pub(super) unsafe fn set_member(object: raw::GfxValue, name: &CStr, value: raw::GfxValue) -> bool {
    // SAFETY: the caller's live values; the interface is the AS3 one, whose SetMember 0x7C6A20
    // is that slot; the name a C string.
    unsafe {
        let Some(interface) = as3_interface(object) else { return false };
        let display = u32::from(*object.kind() & 0x8F == 0x0A);
        raw::engine_gfx_set_member(Some(interface), object.object(), name.as_ptr().cast(), Some(value), display) != 0
    }
}

/// `array`'s element `index` is `value` (SetElement of the AS3 object interface, 0x798340; the
/// array grows to it); false when `array` is no AS3 array.
///
/// # Safety
/// `array` and `value` are live GFx values of a movie on the hook's thread.
pub(super) unsafe fn set_element(array: raw::GfxValue, index: u32, value: raw::GfxValue) -> bool {
    // SAFETY: the caller's live values; the interface is the AS3 one, whose SetElement 0x798340
    // is its slot +34h; an array's own data its +8.
    unsafe {
        let Some(interface) = as3_interface(array) else { return false };
        if *array.kind() & 0x3F != GFX_ARRAY {
            return false;
        }
        raw::engine_gfx_set_element(Some(interface), array.object(), index, Some(value)) != 0
    }
}

/// `room` holds the unmanaged string `text` (the movie copies it where it is set).
///
/// # Safety
/// `text` outlives every use of the value.
pub(super) unsafe fn string_value(room: &mut raw::GfxValueStorage, text: &CStr) -> raw::GfxValue {
    let value = room.handle();
    // SAFETY: our own room; the text the caller's.
    unsafe {
        *value.kind() = GFX_STRING;
        value.set_text(text.as_ptr().cast::<u8>().cast_mut());
    }
    value
}

/// `room` holds the uint `number`.
pub(super) fn uint_value(room: &mut raw::GfxValueStorage, number: u32) -> raw::GfxValue {
    let value = room.handle();
    // SAFETY: our own room.
    unsafe {
        *value.kind() = GFX_UINT;
        *value.value() = number;
    }
    value
}

/// The AS3 objects and arrays [`Movie::made`] makes, in rooms of the caller's frame that never
/// move (the movie's AS3 values are handed back by their address), each released when dropped.
pub(super) struct Made<'r, 't> {
    value: raw::GfxValue,
    _room: PhantomData<&'r mut raw::GfxValueStorage>,
    _movie: PhantomData<&'t Thread>,
}

impl Made<'_, '_> {
    pub(super) fn value(&self) -> raw::GfxValue {
        self.value
    }
}

impl Drop for Made<'_, '_> {
    fn drop(&mut self) {
        // SAFETY: our live value of the movie, made by its CreateObject or CreateArray; a managed
        // one holds a reference the AS3 interface's ObjectRelease (0x78E610, slot +8) drops, as
        // the game's own values are released (0x2886A0).
        unsafe {
            if *self.value.kind() & GFX_MANAGED == 0 {
                return;
            }
            if let Some(interface) = as3_interface(self.value) {
                raw::engine_gfx_object_release(Some(interface), Some(self.value), self.value.object());
            }
        }
    }
}

impl<'t> Movie<'t> {
    /// A new AS3 Object (`kind` [`GFX_OBJECT`], CreateObject 0x511700) or Array ([`GFX_ARRAY`],
    /// CreateArray 0x511710) of the movie in `room`, released when the result is dropped; none
    /// when the movie gave no AS3 value of that type.
    pub(super) fn made<'r>(&self, room: &'r mut raw::GfxValueStorage, kind: u32) -> Option<Made<'r, 't>> {
        *room = raw::GfxValueStorage::zeroed();
        let value = room.handle();
        // SAFETY: a live movie on the hook's thread; the value our zeroed room (undefined), which
        // the call writes and nothing else holds.
        unsafe {
            match kind {
                GFX_OBJECT => raw::engine_movie_create_object(Some(self.this), Some(value), core::ptr::null(), None, 0),
                GFX_ARRAY => raw::engine_movie_create_array(Some(self.this), Some(value)),
                _ => return None,
            }
        }
        let made = Made { value, _room: PhantomData, _movie: PhantomData };
        // SAFETY: the value the movie just wrote, in our room.
        let fits = unsafe { *value.kind() & 0x3F == kind && as3_interface(value).is_some() };
        fits.then_some(made)
    }
}
