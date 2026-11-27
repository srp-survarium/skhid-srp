//! The game's Settings as the patches extend them: the main menu's call to the
//! exe (`game_options::scaleform_callback`) with the movie it answers, the
//! save of user.cfg, and a console variable of ours that user.cfg keeps
//! (patch 79's Language, patch 83's "Hide hit markers" and "Hide player
//! scores").

use core::cell::UnsafeCell;
use core::ffi::CStr;
use core::marker::PhantomData;

use super::context::Context;
use super::hook::Thread;
use super::ui::{Movie, TEXT_LIMIT, c_string, movie_of, string_argument};
use crate::raw;

/// Writes user.cfg now (game::save_user_config 0x1E9030, what the game does
/// when it closes), so a choice survives a crash.
pub fn save_user_config(_: &Thread) {
    // SAFETY: the game's own save of every saved console variable.
    unsafe { raw::engine_save_user_config() }
}

// A file of the game's working folder (`binaries\x86`, where its certificate
// and srp_patches.log are found too), by name: kernel32's CreateFileA,
// CloseHandle and DeleteFileA through the exe's import slots, which the loader
// filled before the game ran; NUL-terminated names.
const OPEN_EXISTING: u32 = 3;
const CREATE_ALWAYS: u32 = 2;
const SHARE_ALL: u32 = 7;
const NORMAL: u32 = 0x80;

fn open(name: &CStr, access: u32, disposition: u32) -> bool {
    let null = core::ptr::null_mut();
    // SAFETY: see above; the handle is closed at once.
    unsafe {
        let file = (raw::engine_create_file_a)(name.as_ptr().cast(), access, SHARE_ALL, null, disposition, NORMAL, null);
        if file as isize == -1 {
            return false;
        }
        (raw::engine_close_handle)(file);
    }
    true
}

/// Whether the file is there.
pub fn file_exists(_: &Thread, name: &CStr) -> bool {
    open(name, 0, OPEN_EXISTING)
}

/// Makes the file, empty (or empties it).
pub fn create_file(_: &Thread, name: &CStr) {
    const GENERIC_WRITE: u32 = 1 << 30;
    open(name, GENERIC_WRITE, CREATE_ALWAYS);
}

/// Deletes the file when it is there.
pub fn delete_file(_: &Thread, name: &CStr) {
    // SAFETY: see above.
    unsafe {
        (raw::engine_delete_file_a)(name.as_ptr().cast());
    }
}

/// The console variable object, a cc_bool (cc_value<bool>::cc_value 0x1DDB0),
/// and its value.
#[repr(C)]
struct BoolStorage {
    object: raw::CcBoolStorage,
    value: u8,
}

/// Whether a console variable is in the engine's list yet (ConsoleBool's
/// and language's ConsoleString's).
pub(super) enum Registration {
    Unregistered,
    Registered,
}

/// The console's kinds a variable answers to: user.cfg's lines run with mask
/// 2 (load_cc_script, on_configs_loaded 0x1EA2C7), so a kind-2 variable is
/// set from the file (the hud's own show_quests_progress, patch 58's
/// g_private_messages_in_game).
const USER_CFG: u32 = 2;

/// A console variable on or off that the engine saves in user.cfg and sets
/// from it (`<name> on|off`, a line game::on_configs_loaded runs). The engine
/// writes the value itself (executing the line); a patch reads and writes it
/// inside a hook. Its default (off for `new`) until the file says otherwise.
pub struct ConsoleBool {
    storage: UnsafeCell<BoolStorage>,
    registration: UnsafeCell<Registration>,
    initial: bool,
}

// SAFETY: the game drives every hook, and the console's reads and writes of
// the value, on its one thread (Thread is not Send).
unsafe impl Sync for ConsoleBool {}

impl ConsoleBool {
    pub const fn new() -> ConsoleBool {
        ConsoleBool::with(false)
    }

    /// On or off until the file says otherwise (the engine's constructor
    /// keeps the value it is given).
    pub const fn with(default: bool) -> ConsoleBool {
        ConsoleBool {
            storage: UnsafeCell::new(BoolStorage { object: raw::CcBoolStorage::zeroed(), value: default as u8 }),
            registration: UnsafeCell::new(Registration::Unregistered),
            initial: default,
        }
    }

    /// Its value until user.cfg says otherwise (`with`'s).
    pub fn initial(&self) -> bool {
        self.initial
    }

    /// Puts the variable in the engine's list once, saved in user.cfg, so the
    /// file's line sets it and the game's save writes it. The constructor
    /// leaves its base's vtable; a cc_bool's is written after it, as the
    /// game's own variables have it.
    pub fn register(&'static self, _: &Thread, name: &'static CStr) {
        // SAFETY: on the game's thread; the object and the value are this
        // static's (never moved or freed), a cc_bool's size and its bool; the
        // name lives as long.
        unsafe {
            if matches!(*self.registration.get(), Registration::Registered) {
                return;
            }
            let storage = self.storage.get();
            let object = (*storage).object.handle();
            raw::engine_cc_bool(Some(object), name.as_ptr().cast(), &raw mut (*storage).value, 1, USER_CFG);
            *object.vtable() = raw::image::CC_BOOL_VTABLE;
            *self.registration.get() = Registration::Registered;
        }
    }

    pub fn get(&self, _: &Thread) -> bool {
        // SAFETY: on the game's thread, the value's only user while it runs.
        unsafe { (*self.storage.get()).value != 0 }
    }

    pub fn set(&self, _: &Thread, on: bool) {
        // SAFETY: as get.
        unsafe { (*self.storage.get()).value = u8::from(on) }
    }
}

impl Default for ConsoleBool {
    fn default() -> Self {
        Self::new()
    }
}

/// The console variable object, a cc_float (made as the game makes its own:
/// cc_value<bool>::cc_value's base, then the cc_float's vtable and range, as
/// s_general_volume's static initializer 0x84D440 has them), and its value.
#[repr(C)]
struct FloatStorage {
    object: raw::CcFloatStorage,
    value: f32,
}

/// A float console variable that the engine saves in user.cfg and sets from
/// it (`<name> <value>`), within its range: a value out of it, or not a
/// number, stores the least (the game's own setter 0xECB70). The stock
/// Settings' sliders name theirs by name (options_item_float), so a stock
/// slider whose variable the game lacks (`s_music_volume`) works once it is
/// registered. Its default until the file says otherwise.
pub struct ConsoleFloat {
    storage: UnsafeCell<FloatStorage>,
    range: (f32, f32),
    registration: UnsafeCell<Registration>,
}

// SAFETY: as ConsoleBool's: the hooks and the console's reads and writes of
// the value run on the game's one thread.
unsafe impl Sync for ConsoleFloat {}

impl ConsoleFloat {
    /// `least..=most`, `default` within it.
    pub const fn new(default: f32, least: f32, most: f32) -> ConsoleFloat {
        ConsoleFloat {
            storage: UnsafeCell::new(FloatStorage { object: raw::CcFloatStorage::zeroed(), value: default }),
            range: (least, most),
            registration: UnsafeCell::new(Registration::Unregistered),
        }
    }

    /// Puts the variable in the engine's list once, saved in user.cfg, as
    /// ConsoleBool::register; the cc_float's vtable and range written after
    /// the base constructor.
    pub fn register(&'static self, _: &Thread, name: &'static CStr) {
        // SAFETY: on the game's thread; the object (a cc_float's size) and the
        // value are this static's, never moved or freed; the name lives as
        // long. The base constructor writes the list link, the name, the
        // kind, saved and the value's address (+40h) and two bytes at +44h,
        // which the range then overwrites.
        unsafe {
            if matches!(*self.registration.get(), Registration::Registered) {
                return;
            }
            let storage = self.storage.get();
            let object = (*storage).object.handle();
            raw::engine_cc_float(Some(object), name.as_ptr().cast(), &raw mut (*storage).value, 1, USER_CFG);
            *object.vtable() = raw::image::CC_FLOAT_VTABLE;
            *object.least() = self.range.0;
            *object.most() = self.range.1;
            *self.registration.get() = Registration::Registered;
        }
    }

    pub fn get(&self, _: &Thread) -> f32 {
        // SAFETY: on the game's thread, the value's only user while it runs.
        unsafe { (*self.storage.get()).value }
    }

    /// A value within the range (the caller's type keeps it there), as the
    /// console's setter would store it.
    pub fn set(&self, _: &Thread, value: f32) {
        // SAFETY: as get.
        unsafe { (*self.storage.get()).value = value }
    }
}

/// The Settings' call to the exe at `options.callback`: the movie's
/// `ExternalInterface.call(method, ...)` at game_options::scaleform_callback's
/// start (0x1FF5D0), before the options match the method's name.
pub struct OptionsCall<'c> {
    options: raw::GameOptionsInterface,
    frame: raw::OptionsCallbackFrame,
    method: &'c CStr,
    _thread: &'c Thread,
}

impl<'c> OptionsCall<'c> {
    pub fn at(ctx: &'c Context<'_>) -> Option<OptionsCall<'c>> {
        use raw::site::options_callback as site;
        // SAFETY: the context is this site's: the options' interface and the
        // callback's frame, alive for the call, on the game's thread; the
        // method's name a C string the movie gave.
        let (options, frame, method) = unsafe {
            let (options, frame) = (site::options(ctx)?, site::frame(ctx)?);
            (options, frame, c_string(frame.method(), TEXT_LIMIT)?)
        };
        Some(OptionsCall { options, frame, method, _thread: ctx.thread() })
    }

    pub fn method(&self) -> &CStr {
        self.method
    }

    /// The call's argument `index` when it is a string.
    pub fn string_argument(&self, index: u32) -> Option<&CStr> {
        // SAFETY: the callback's arguments and their count, alive for the call.
        unsafe { string_argument(self.frame.arguments(), *self.frame.count(), index) }
    }

    /// A dictionary text of the running language (text_translator::
    /// translate_text 0x1FF0F0, the key itself when the dictionary has none)
    /// into `out`.
    pub fn translate<'o>(&self, key: &CStr, out: &'o mut [u8; 0x200]) -> &'o CStr {
        out[0] = 0;
        // SAFETY: the options' game and its text translator, alive for the
        // call; translate_text copies a text shorter than 200h bytes with its
        // zero into `out` (0x1FF15D) and leaves it alone otherwise.
        unsafe {
            if let Some(game) = self.options.game() {
                raw::engine_translate_text(Some(game.text_translator()), key.as_ptr().cast(), out.as_mut_ptr());
            }
        }
        out[0x1FF] = 0;
        CStr::from_bytes_until_nul(out).unwrap_or(c"")
    }

    /// Answers the main menu (the movie that called): invokes its `method`
    /// with these strings (UTF-8, as the dictionaries are), at most
    /// [`super::ui::MAX_STRINGS`]; whether the movie took it.
    pub fn answer(&self, method: &CStr, values: &[&CStr]) -> bool {
        // SAFETY: the options' menu, a movie holder alive for the call, on the
        // hook's thread (the options' own Invoke reads it so, 0x1FF98F).
        let Some(movie) = (unsafe { movie_of(self.options.menu()) }) else { return false };
        movie.invoke_strings(method, values)
    }

    /// The menu that called, to tell it something as [`Menu`] does.
    pub fn menu(&self) -> Option<Menu<'_>> {
        // SAFETY: as answer and translate: the options' menu and game, alive
        // for the call.
        let (movie, game) = unsafe { (movie_of(self.options.menu())?, self.options.game()?) };
        Some(Menu { movie, game, thread: PhantomData })
    }
}

/// A key the key binder names (its keys' table at 0xA74FA0, rows of 88h
/// bytes to one whose name is 0): its name, the dictionary's key of its text
/// ("kSPACE", "mouse1", "mouse_wheel_up"), and its code, what a press of it
/// gives the main menu's key wait (process_key_input's EDX). Made only from
/// the table ([`OptionsCall::binder_keys`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BinderKey<'t> {
    pub name: &'t CStr,
    pub code: u32,
}

/// Esc's code: it ends a key wait without a key (process_key_input 0x2010EF).
const ESCAPE: u32 = 1;

/// More rows than the binder's table has (135): a bound on the walk.
const BINDER_KEYS: usize = 0x200;

/// A row of the options' controls (patch 62's `controls`, hide_ui_026g.asm):
/// its action and its key as the menu has it now, set or not yet applied (a
/// binder key's name, "" none). Made only from the table
/// ([`OptionsCall::control_rows`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControlRow<'t> {
    pub action: u32,
    pub key: &'t CStr,
}

/// What [`OptionsCall::bind_key`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindOutcome {
    /// The key was given to the action's row as a press of it would give it:
    /// kept for Apply, or the conflict's message shown (the game's own path).
    Given,
    /// The menu waits for another key already (a click on a row's key box).
    Waiting,
    /// No game, or no scene for the menu's cursor movie: nothing done.
    NoMenu,
}

impl<'c> OptionsCall<'c> {
    /// The key binder's keys, in its table's order (a name the table lists
    /// twice, twice).
    pub fn binder_keys(&self) -> impl Iterator<Item = BinderKey<'c>> + 'c {
        let first = raw::image::key_names();
        (0..BINDER_KEYS)
            .map(move |index| {
                // SAFETY: the binder's static table, its rows read up to the
                // one whose name is 0 (map_while stops there), each name a C
                // string of the image.
                unsafe {
                    let row = first.at(index);
                    let name = row.name();
                    (!name.is_null()).then(|| (c_string(name, TEXT_LIMIT), *row.code()))
                }
            })
            .map_while(|row| row)
            .filter_map(|(name, code)| Some(BinderKey { name: name?, code }))
    }

    /// The binder's key of this code, when it has one: a code a key wait can be given.
    pub fn binder_key(&self, code: u32) -> Option<BinderKey<'c>> {
        (code != ESCAPE).then(|| self.binder_keys().find(|key| key.code == code)).flatten()
    }

    /// The options' controls, in their order.
    pub fn control_rows(&self) -> impl Iterator<Item = ControlRow<'c>> + 'c {
        // SAFETY: the code of assign_binding, whose table and count patch 62
        // points at `controls` (the game's own at 0xA9DB28 without it): a
        // static table of that many rows of 68h bytes.
        let (table, count) = unsafe {
            let code = raw::image::assign_binding_code();
            (code.table(), *code.count() as usize)
        };
        (0..if table.is_some() { count } else { 0 }).filter_map(move |index| {
            // SAFETY: a row of the table (index under its count); its key a
            // buffer_string of the row's own, its text zero-terminated.
            unsafe {
                let row = table?.at(index);
                let key = c_string(row.key().begin(), TEXT_LIMIT).unwrap_or(c"");
                Some(ControlRow { action: *row.action(), key })
            }
        })
    }

    /// Gives `key` to the row of `action` as a press of it gives it while the
    /// menu waits for that row's key: the wait started as the movie's
    /// start_bind_key starts it (scaleform_callback 0x1FF899..0x1FF8A9: the
    /// awaited action, the menu's cursor movie off the scene), then the press
    /// as on_keyboard_action hands one over (0x2015F1..0x201605:
    /// process_key_input, and finish_binding when that ends the wait). So the
    /// row keeps it for Apply and the movie hears `root.set_keybind`, or a
    /// conflict shows the game's message, exactly as the stock way.
    pub fn bind_key(&self, row: ControlRow<'_>, key: BinderKey<'_>) -> BindOutcome {
        // SAFETY: the options' game and its main menu inside it, alive for the
        // call, on the game's thread; the menu called, so its menu and cursor
        // movies are loaded (start_bind_key and finish_binding read both); a
        // row of the controls and a binder key's code (not Esc's).
        unsafe {
            let Some(game) = self.options.game() else { return BindOutcome::NoMenu };
            let menu = game.options();
            if super::hud_input::AwaitedKey::of(*menu.awaited_action()).is_some() {
                return BindOutcome::Waiting;
            }
            let Some(scene) = menu.scene() else { return BindOutcome::NoMenu };
            *menu.awaited_action() = row.action;
            raw::engine_hide_scene_movie(Some(menu.cursor_movie()), Some(scene));
            if raw::engine_process_key_input(key.code, Some(menu)) != 0 {
                raw::engine_finish_key_binding(Some(menu));
            }
        }
        BindOutcome::Given
    }

    /// Takes `key` from every row that has it, as a conflict's accepted reassign takes a key from the rows
    /// that had it (scaleform_callback's reassign_ok_clicked, 0x1FF8E0..0x1FF8E9: assign_binding of ""
    /// for each): the rows keep "" for Apply and the movie hears `root.set_keybind` for each.
    pub fn clear_key(&self, key: BinderKey<'_>) -> BindOutcome {
        // SAFETY: as bind_key: the menu inside the options' game, alive for the call; assign_binding keeps
        // "" in the row of an action of the controls and tells the menu's movie, which called.
        unsafe {
            let Some(game) = self.options.game() else { return BindOutcome::NoMenu };
            let menu = game.options();
            if super::hud_input::AwaitedKey::of(*menu.awaited_action()).is_some() {
                return BindOutcome::Waiting;
            }
            for row in self.control_rows().filter(|row| row.key == key.name) {
                raw::engine_assign_binding(c"".as_ptr().cast(), Some(menu), row.action);
            }
        }
        BindOutcome::Given
    }
}

/// The main menu's movie (the Settings are in it) with the game's
/// dictionary: what a patch tells the Settings by, in answer to a call of
/// theirs ([`OptionsCall::menu`]) or at a frame while the menu is shown
/// ([`Menu::shown_at_game_tick`]).
pub struct Menu<'t> {
    movie: Movie<'t>,
    game: raw::Game,
    thread: PhantomData<&'t Thread>,
}

impl<'t> Menu<'t> {
    /// At `game.tick` (every frame, in a match or not), while the main menu
    /// is shown; None while it is not.
    pub fn shown_at_game_tick(ctx: &'t Context<'_>) -> Option<Menu<'t>> {
        // SAFETY: the site's EBX is the game, alive and ticking on this
        // thread; the main menu is inside it, its shown byte its own, its
        // movie holder the one it shows while that byte is set.
        unsafe {
            let game = raw::site::game_tick::game(ctx)?;
            let options = game.options();
            if *options.shown() == 0 {
                return None;
            }
            Some(Menu { movie: movie_of(options.menu())?, game, thread: PhantomData })
        }
    }

    /// A dictionary text of the running language into `out`, as
    /// [`OptionsCall::translate`].
    pub fn translate<'o>(&self, key: &CStr, out: &'o mut [u8; 0x200]) -> &'o CStr {
        out[0] = 0;
        // SAFETY: the game and its text translator, alive for the hook;
        // translate_text copies a text shorter than 200h bytes with its zero
        // into `out` (0x1FF15D) and leaves it alone otherwise.
        unsafe {
            raw::engine_translate_text(Some(self.game.text_translator()), key.as_ptr().cast(), out.as_mut_ptr());
        }
        out[0x1FF] = 0;
        CStr::from_bytes_until_nul(out).unwrap_or(c"")
    }

    /// Invokes the movie's `method` with these strings (UTF-8); whether the
    /// movie took it.
    pub fn tell(&self, method: &CStr, values: &[&CStr]) -> bool {
        self.movie.invoke_strings(method, values)
    }
}

/// A Settings slider's value in its whole steps (the slider's snap: patch
/// 112's hundredths, the field of view's degrees), as the movie sends it to
/// the exe and the exe answers it: decimal digits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SliderSteps(u16);

impl SliderSteps {
    pub const fn new(steps: u16) -> SliderSteps {
        SliderSteps(steps)
    }

    pub fn get(self) -> u16 {
        self.0
    }

    /// The Settings' text of one: one to four decimal digits, nothing else.
    pub fn parse(text: &CStr) -> Option<SliderSteps> {
        let digits = text.to_bytes();
        if digits.is_empty() || digits.len() > 4 || !digits.iter().all(u8::is_ascii_digit) {
            return None;
        }
        Some(SliderSteps(digits.iter().fold(0, |n, d| n * 10 + u16::from(d - b'0'))))
    }

    /// The nearest whole steps to `value` (`per_unit` steps to a unit:
    /// 100 for hundredths), within `least..=most`.
    pub fn nearest(value: f32, per_unit: f64, least: SliderSteps, most: SliderSteps) -> SliderSteps {
        let n = (f64::from(value) * per_unit + 0.5) as u32;
        SliderSteps(n.clamp(u32::from(least.0), u32::from(most.0)) as u16)
    }

    /// Its digits into `out`, with the zero.
    pub fn text(self, out: &mut [u8; 8]) -> &CStr {
        let mut n = self.0;
        let mut digits = [0u8; 5];
        let mut len = 0;
        loop {
            digits[len] = b'0' + (n % 10) as u8;
            len += 1;
            n /= 10;
            if n == 0 {
                break;
            }
        }
        for i in 0..len {
            out[i] = digits[len - 1 - i];
        }
        out[len] = 0;
        CStr::from_bytes_until_nul(&out[..=len]).unwrap_or(c"")
    }
}

/// The game's field of view, `uro_fov` (patch 22: the camera's angle in
/// degrees unaimed, read at every frame; saved in user.cfg), and the range its
/// registration gives it (0x85ADF0: 50 by patch 22, 100).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FieldOfView {
    pub degrees: f32,
    pub least: f32,
    pub most: f32,
}

impl FieldOfView {
    /// The variable now; None before its static initializer ran (never at
    /// the Settings).
    pub fn now(_: &Thread) -> Option<FieldOfView> {
        let variable = raw::image::uro_fov();
        // SAFETY: the engine's own cc_float, a static of the image; its value
        // is the float its registration points at, on the game's thread.
        unsafe {
            let value = variable.value();
            if value.is_null() {
                return None;
            }
            Some(FieldOfView { degrees: *value, least: *variable.least(), most: *variable.most() })
        }
    }

    /// Sets the value when it is within the range, as the console's setter
    /// (0xECB70) would store it; whether it did.
    pub fn set(thread: &Thread, degrees: f32) -> bool {
        let Some(now) = FieldOfView::now(thread) else { return false };
        if !(now.least..=now.most).contains(&degrees) {
            return false;
        }
        // SAFETY: as now; the value's writers are the console and this, on
        // the game's thread.
        unsafe { *raw::image::uro_fov().value() = degrees }
        true
    }
}
