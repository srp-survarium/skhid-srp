//! Voice's boundaries: game objects, console values, Scaleform and the optional
//! versioned DLL. Decisions about talking, distance and muting live in the patch.
use super::{Context, Handle, Reg, Thread};
use crate::raw;
use core::{
    cell::UnsafeCell,
    ffi::{CStr, c_void},
    marker::PhantomData,
};

pub const MAX_PLACES: usize = 20;
pub const MUTES_SIZE: usize = 512;

macro_rules! handle {
    ($name:ident) => {
        pub struct $name<'t> {
            at: *mut c_void,
            thread: &'t Thread,
        }
        impl<'t> Handle<'t> for $name<'t> {
            unsafe fn from_raw(thread: &'t Thread, at: u32) -> Option<Self> {
                Some(Self {
                    at: at as usize as *mut c_void,
                    thread,
                })
            }
        }
        impl $name<'_> {
            pub(in crate::vostok) fn ptr(&self) -> *mut c_void {
                let _ = self.thread;
                self.at
            }
        }
    };
}
handle!(VoiceWorld);
handle!(VoiceOptions);
handle!(VoiceText);
handle!(VoiceValues);
handle!(VoiceObject);

#[derive(Clone, Copy)]
pub struct Player {
    pub name: [u8; 64],
    pub team: u32,
    pub position: Option<[f32; 3]>,
}
impl Player {
    const EMPTY: Self = Self {
        name: [0; 64],
        team: 3,
        position: None,
    };
}
#[derive(Clone, Copy)]
pub struct Match {
    pub players: [Player; MAX_PLACES],
    pub count: usize,
    pub local: Option<usize>,
}
impl Match {
    pub const EMPTY: Self = Self {
        players: [Player::EMPTY; MAX_PLACES],
        count: 0,
        local: None,
    };
    pub fn teammate(&self, slot: usize) -> bool {
        self.local.is_some_and(|me| {
            slot < self.count
                && slot != me
                && self.players[me].team < 3
                && self.players[me].team == self.players[slot].team
        })
    }
}

// SAFETY: every caller names a generated field in an object supplied by the
// hook, while the game thread is paused in the hook; pointer fields may be null.
unsafe fn read<T: Copy>(p: *mut T) -> T {
    unsafe { p.read_unaligned() }
}
unsafe fn object(p: *mut u32) -> *mut c_void {
    // SAFETY: caller supplies a readable pointer-valued generated field.
    unsafe { read(p) as usize as *mut c_void }
}
unsafe fn movie(holder: *mut c_void) -> *mut c_void {
    if holder.is_null() {
        return core::ptr::null_mut();
    }
    // SAFETY: caller supplies a live movie holder; its nullable inner is checked.
    let inner = unsafe { object(raw::movie_holder::view(holder)) };
    if inner.is_null() {
        core::ptr::null_mut()
    } else {
        // SAFETY: non-null inner belongs to the live holder.
        unsafe { object(raw::movie_view::movie(inner)) }
    }
}
unsafe fn snapshot(game: *mut c_void, world: *mut c_void) -> Match {
    let mut out = Match::EMPTY;
    if game.is_null() {
        return out;
    }
    // SAFETY: caller holds the game thread and live game/world; nullable links
    // are checked, slot/kit counts bounded, and names copied from fixed arrays.
    unsafe {
        let net = object(raw::game::network_client(game));
        if net.is_null() {
            return out;
        }
        let client = object(raw::network_client::match_client(net));
        if client.is_null() {
            return out;
        }
        let info = raw::match_client::static_info(client).cast::<c_void>();
        out.count = usize::from(read(raw::match_options::places(info))).min(MAX_PLACES);
        let local = usize::from(read(raw::network_client::local_place(net)));
        out.local = (local < out.count).then_some(local);
        let mut places = core::ptr::null_mut();
        if !world.is_null() {
            let owner = object(raw::game_world::core_owner(world));
            if !owner.is_null() {
                places = object(raw::game_world_core::places(
                    raw::core_owner::core(owner).cast(),
                ));
            }
        }
        let stride = raw::profile::end(info) as usize - info as usize;
        for slot in 0..out.count {
            let profile = info.cast::<u8>().add(stride * slot).cast();
            let name = raw::profile::name(profile);
            // The profile owns a 64-byte name. Terminate even malformed data.
            core::ptr::copy_nonoverlapping(name, out.players[slot].name.as_mut_ptr(), 63);
            out.players[slot].team = read(raw::profile::team(profile));
            if !places.is_null() {
                let stride = raw::player_place::end(places) as usize - places as usize;
                let entry = places.cast::<u8>().add(stride * slot).cast();
                let kit = usize::from(read(raw::player_place::kit(entry)));
                if kit < 3 {
                    let player = object(raw::player_place::players(entry).add(kit));
                    if !player.is_null()
                        && read(raw::player::alive(player)) != 0
                        && read(raw::player::inserted(player)) != 0
                    {
                        out.players[slot].position =
                            Some(read(raw::player::position(player).cast::<[f32; 3]>()));
                    }
                }
            }
        }
    }
    out
}
impl<'t> VoiceWorld<'t> {
    pub fn thread(&self) -> &'t Thread {
        self.thread
    }
    pub fn snapshot(&self) -> Match {
        if self.at.is_null() {
            return Match::EMPTY;
        }
        // SAFETY: the hook's game world and its owning game.
        unsafe { snapshot(object(raw::game_world::game(self.at)), self.at) }
    }
    pub fn talking(&self, data: &Match, talking: u32, radio: u32) -> bool {
        if self.at.is_null() {
            return false;
        }
        let mut args = [Value::EMPTY; 2 * MAX_PLACES];
        let mut n = 0;
        if let Some(me) = data.local.filter(|&me| talking & (1 << me) != 0) {
            args[n] = Value::text(&data.players[me].name);
            args[n + 1] = Value::word(4, 1);
            n += 2;
        }
        for slot in 0..data.count {
            if data.local == Some(slot) || talking & (1 << slot) == 0 {
                continue;
            }
            args[n] = Value::text(&data.players[slot].name);
            args[n + 1] = Value::word(4, if radio & (1 << slot) != 0 { 2 } else { 0 });
            n += 2;
        }
        // SAFETY: the world's live movie holder; arguments live through Invoke.
        unsafe {
            let movie = movie(object(raw::game_world::hud_holder(self.at)));
            if movie.is_null() {
                return false;
            }
            invoke(movie, c"root.srp_voice_talking", &args[..n]);
        }
        true
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Value([u32; 6]);
impl Value {
    const EMPTY: Self = Self([0; 6]);
    fn text(s: &[u8]) -> Self {
        Self([0, 6, s.as_ptr() as usize as u32, 0, 0, 0])
    }
    const fn word(kind: u32, value: u32) -> Self {
        Self([0, kind, value, 0, 0, 0])
    }
    fn number(self) -> Option<u32> {
        match self.0[1] & 63 {
            2 => Some(u32::from(self.0[2] as u8 != 0)),
            3 | 4 => Some(self.0[2]),
            5 => {
                let n = f64::from_bits(u64::from(self.0[2]) | (u64::from(self.0[3]) << 32));
                (n >= 0.0 && n < (u32::MAX as f64 + 1.0)).then_some(n as u32)
            }
            _ => None,
        }
    }
}
unsafe fn invoke(movie: *mut c_void, name: &CStr, args: &[Value]) {
    if !movie.is_null() {
        // SAFETY: caller supplies a live movie and values lasting through Invoke.
        unsafe {
            raw::engine_movie_invoke(
                movie,
                name.as_ptr().cast(),
                core::ptr::null_mut(),
                args.as_ptr().cast(),
                args.len() as u32,
            );
        }
    }
}
unsafe fn translate(game: *mut c_void, key: &CStr) -> [u8; 512] {
    let mut out = [0; 512];
    if !game.is_null() {
        // SAFETY: live translator writes at most its documented 512-byte output.
        unsafe {
            raw::engine_translate(
                raw::game::translator(game).cast(),
                key.as_ptr().cast(),
                out.as_mut_ptr().cast(),
            );
        }
    }
    out[511] = 0;
    out
}

#[derive(Clone, Copy)]
pub struct MenuIdentity {
    game: u32,
}
impl MenuIdentity {
    pub fn snapshot(self, _: &Thread) -> Match {
        let game = self.game as usize as *mut c_void;
        // SAFETY: kept only during a running match, cleared by both world stop hooks.
        unsafe { snapshot(game, core::ptr::null_mut()) }
    }
}
impl VoiceOptions<'_> {
    pub fn thread(&self) -> &Thread {
        self.thread
    }
    pub fn panel(&self, state: u32, running: bool, is_muted: impl Fn(usize) -> Option<bool>) {
        if self.at.is_null() {
            return;
        }
        // SAFETY: options is the object at its own activate call.
        unsafe {
            let game = object(raw::game_options::game(self.at));
            if game.is_null() {
                return;
            }
            let data = snapshot(game, core::ptr::null_mut());
            let title = translate(game, c"st_voice_chat_volume_option");
            let all = translate(game, c"st_voice_mute_all");
            let mut args = [Value::EMPTY; 2 + 3 * MAX_PLACES];
            args[0] = Value::text(&title);
            args[1] = Value::text(&all);
            let mut n = 2;
            if running && state == raw::game::world(game) as usize as u32 {
                for slot in 0..data.count {
                    if !data.teammate(slot) || data.players[slot].name[0] == 0 {
                        continue;
                    }
                    if let Some(muted) = is_muted(slot) {
                        args[n] = Value::word(4, slot as u32);
                        args[n + 1] = Value::text(&data.players[slot].name);
                        args[n + 2] = Value::word(2, u32::from(muted));
                        n += 3;
                    }
                }
            }
            invoke(
                movie(object(raw::game_options::movie_holder(self.at))),
                c"root.srp_voice_panel",
                &args[..n],
            );
        }
    }
}
impl VoiceText<'_> {
    pub fn is(&self, text: &CStr) -> bool {
        if self.at.is_null() {
            return false;
        }
        // SAFETY: a Scaleform callback's name is a terminated string; compare only through expected NUL.
        unsafe {
            text.to_bytes_with_nul()
                .iter()
                .enumerate()
                .all(|(i, &b)| self.at.cast::<u8>().add(i).read() == b)
        }
    }
}
impl VoiceValues<'_> {
    pub fn number(&self, index: usize, count: u32) -> Option<u32> {
        if self.at.is_null() || index >= count as usize {
            return None;
        }
        // SAFETY: callback's count bounds its array of 24-byte GFx values.
        unsafe { read(self.at.cast::<Value>().add(index)).number() }
    }
}
impl VoiceObject<'_> {
    pub fn thread(&self) -> &Thread {
        self.thread
    }
}

#[derive(Clone, Copy)]
pub struct Key {
    pub action: u32,
    pub down: bool,
    pub code: u32,
    input: u32,
    game: u32,
}
impl Key {
    pub fn from_context(ctx: &Context<'_>) -> Option<Key> {
        // SAFETY: keyboard.player_list's documented EDI handler and EBP frame.
        unsafe {
            let frame = ctx.reg(Reg::Ebp) as usize as *mut c_void;
            let game = object(raw::keyboard_handler::game(
                ctx.reg(Reg::Edi) as usize as *mut c_void
            ));
            if game.is_null() {
                return None;
            }
            let kind = read(raw::keyboard_frame::kind(frame));
            if kind != 1 && kind != 3 {
                return None;
            }
            let binder = object(raw::game::binder(game));
            if binder.is_null() {
                return None;
            }
            let code = read(raw::keyboard_frame::key(frame));
            let mut toggle = 0;
            let action = raw::engine_binded_action(binder, 8, code, &mut toggle);
            Some(Key {
                action,
                down: kind == 1,
                code,
                input: read(raw::keyboard_frame::input_world(frame)),
                game: game as usize as u32,
            })
        }
    }
    pub fn identity(self) -> MenuIdentity {
        MenuIdentity { game: self.game }
    }
    pub fn notice(self, muted: bool, name: &[u8; 64]) {
        // SAFETY: a key hook's game; live world embedded in that game.
        unsafe {
            let game = self.game as usize as *mut c_void;
            let world = raw::game::world(game).cast();
            let text = translate(
                game,
                if muted {
                    c"st_voice_muted"
                } else {
                    c"st_voice_unmuted"
                },
            );
            invoke(
                movie(object(raw::game_world::hud_holder(world))),
                c"root.srp_voice_notice",
                &[Value::text(&text), Value::text(name)],
            );
        }
    }
    pub fn held(self, _: &Thread) -> bool {
        if self.input == 0 {
            return false;
        }
        // SAFETY: the game retains this input world for the match, and stop clears the key.
        unsafe {
            let input = self.input as usize as *mut c_void;
            let table = object(raw::input_world::vtable(input));
            let f: unsafe extern "thiscall" fn(*mut c_void) -> *mut c_void =
                function(object(raw::input_vtable::keyboard(table)));
            let keyboard = f(input);
            if keyboard.is_null() {
                return false;
            }
            let table = object(raw::keyboard::vtable(keyboard));
            let f: unsafe extern "thiscall" fn(*mut c_void, u32) -> u8 =
                function(object(raw::keyboard_vtable::pressed(table)));
            f(keyboard, self.code) != 0
        }
    }
}
impl VoiceWorld<'_> {
    pub fn identity(&self) -> Option<MenuIdentity> {
        if self.at.is_null() {
            return None;
        }
        // SAFETY: live world's owning game; the patch clears this on match stop.
        let game = unsafe { read(raw::game_world::game(self.at)) };
        (game != 0).then_some(MenuIdentity { game })
    }
}

/// Host/port/session are read only at the match-client connect prologue.
pub struct Connection<'t> {
    host: *const u8,
    pub port: u32,
    pub session: u32,
    _thread: PhantomData<&'t Thread>,
}
impl<'t> Connection<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<Self> {
        // Context exposes the original game ESP, before the entry saves its frame.
        let esp = ctx.reg(Reg::Esp) as usize as *const u32;
        // SAFETY: the connect entry's host, port and session arguments.
        unsafe {
            let host = esp.add(1).read() as usize as *const u8;
            (!host.is_null()).then(|| Self {
                host,
                port: (esp.add(2).read() & u32::from(u16::MAX)) + 1,
                session: esp.add(3).read(),
                _thread: PhantomData,
            })
        }
    }
}

// Function pointers obtained from a checked DLL export or a live typed vtable.
// A union keeps the conversion explicit without constructing a game binding:
// game virtual-call slots come from the generated field table, not literals.
unsafe fn function<T: Copy>(pointer: *mut c_void) -> T {
    union Function<T: Copy> {
        pointer: *mut c_void,
        typed: T,
    }
    // SAFETY: callers choose the checked export/vtable signature; all function
    // pointers have the same 32-bit representation as the supplied address.
    unsafe { Function { pointer }.typed }
}
#[derive(Clone, Copy)]
pub struct Dll {
    api: voice_abi::Api,
    focus: Option<Focus>,
}
#[derive(Clone, Copy)]
struct Focus {
    window: unsafe extern "stdcall" fn() -> *mut c_void,
    process_of: unsafe extern "stdcall" fn(*mut c_void, *mut u32) -> u32,
    process: unsafe extern "stdcall" fn() -> u32,
}
impl Dll {
    pub fn load(_: &Thread) -> Option<Self> {
        // SAFETY: imports from checked raw IAT; the typed API checks every export/version.
        unsafe {
            let module = raw::engine_load_library(c"skhid_voice.dll".as_ptr().cast());
            if module.is_null() {
                return None;
            }
            let Some(api) = voice_abi::Api::load(|name| {
                raw::engine_get_proc_address(module, name.as_ptr().cast())
            }) else {
                raw::engine_free_library(module);
                return None;
            };
            let user = raw::engine_get_module_handle(c"user32.dll".as_ptr().cast());
            let kernel = raw::engine_get_module_handle(c"kernel32.dll".as_ptr().cast());
            let win = raw::engine_get_proc_address(user, c"GetForegroundWindow".as_ptr().cast());
            let owner =
                raw::engine_get_proc_address(user, c"GetWindowThreadProcessId".as_ptr().cast());
            let process =
                raw::engine_get_proc_address(kernel, c"GetCurrentProcessId".as_ptr().cast());
            let focus = if win.is_null() || owner.is_null() || process.is_null() {
                None
            } else {
                Some(Focus {
                    window: function(win),
                    process_of: function(owner),
                    process: function(process),
                })
            };
            Some(Self { api, focus })
        }
    }
    pub fn start(self, connection: &Connection<'_>) -> bool {
        // SAFETY: loaded, version-checked API and connect's live terminated host.
        unsafe {
            (self.api.start)(connection.host.cast(), connection.port, connection.session) == 0
        }
    }
    pub fn stop(self) {
        // SAFETY: this token owns a loaded, version-checked API.
        unsafe {
            (self.api.stop)();
        }
    }
    pub fn ptt(self, down: bool) {
        // SAFETY: the checked API remains loaded and accepts a scalar boolean.
        unsafe {
            (self.api.ptt)(i32::from(down));
        }
    }
    pub fn settings(self, volume: f32, microphone: bool, sensitivity: f32) {
        // SAFETY: the checked API remains loaded and accepts these scalar values.
        unsafe {
            (self.api.settings)(volume, i32::from(microphone), sensitivity);
        }
    }
    pub fn mute(self, mask: u32) {
        // SAFETY: the checked API remains loaded and accepts a scalar slot mask.
        unsafe {
            (self.api.mute)(mask);
        }
    }
    pub fn radio(self, mask: u32) {
        // SAFETY: the checked API remains loaded and accepts a scalar slot mask.
        unsafe {
            (self.api.radio)(mask);
        }
    }
    pub fn talking(self) -> u32 {
        // SAFETY: the version-checked API remains loaded for this token.
        unsafe { (self.api.talking)() }
    }
    pub fn last(self) -> Option<usize> {
        // SAFETY: the version-checked API remains loaded for this token.
        let n = unsafe { (self.api.last_speaker)() };
        (n >= 0 && n < MAX_PLACES as i32).then_some(n as usize)
    }
    pub fn id(self, slot: usize) -> Option<u32> {
        if slot >= MAX_PLACES {
            return None;
        }
        // SAFETY: the API remains loaded and slot is in its documented range.
        let id = unsafe { (self.api.voice_id)(slot as i32) };
        (id != 0).then_some(id)
    }
    pub fn focused(self) -> bool {
        let Some(focus) = self.focus else {
            return true;
        };
        // SAFETY: the pointers were obtained from system module exports.
        unsafe {
            let window = (focus.window)();
            if window.is_null() {
                return false;
            }
            let mut owner = 0;
            (focus.process_of)(window, &mut owner);
            owner == (focus.process)()
        }
    }
}

#[repr(C)]
struct OptionsStorage {
    cc_volume: [u8; 76],
    cc_microphone: [u8; 72],
    cc_sensitivity: [u8; 76],
    cc_off: [u8; 72],
    cc_mutes: [u8; 72],
    volume: f32,
    sensitivity: f32,
    microphone: u8,
    off: u8,
    initialized: bool,
    mutes: [u8; MUTES_SIZE],
}
struct Store(UnsafeCell<OptionsStorage>);
// SAFETY: only the game's thread reads/writes console values, including console callbacks.
unsafe impl Sync for Store {}
static OPTIONS: Store = Store(UnsafeCell::new(OptionsStorage {
    cc_volume: [0; 76],
    cc_microphone: [0; 72],
    cc_sensitivity: [0; 76],
    cc_off: [0; 72],
    cc_mutes: [0; 72],
    volume: 0.0,
    sensitivity: 0.0,
    microphone: 0,
    off: 0,
    initialized: false,
    mutes: [0; MUTES_SIZE],
}));
#[derive(Clone, Copy, PartialEq)]
pub struct Settings {
    pub volume: f32,
    pub microphone: bool,
    pub sensitivity: f32,
    pub disabled: bool,
}
pub fn register(_: &Thread) {
    // SAFETY: the permanent console storage is initialized once on the game thread before user.cfg.
    unsafe {
        let p = OPTIONS.0.get();
        if (*p).initialized {
            return;
        }
        (*p).initialized = true;
        (*p).volume = 100.0;
        (*p).sensitivity = 50.0;
        (*p).microphone = 1;
        raw::engine_cc_float(
            (&raw mut (*p).cc_volume).cast(),
            0.0,
            c"s_chat_volume".as_ptr().cast(),
            &raw mut (*p).volume,
            100.0,
            1,
            2,
        );
        raw::engine_cc_float(
            (&raw mut (*p).cc_sensitivity).cast(),
            0.0,
            c"s_mic_sens".as_ptr().cast(),
            &raw mut (*p).sensitivity,
            100.0,
            1,
            2,
        );
        (&raw mut (*p).cc_volume)
            .cast::<u32>()
            .write(raw::image::CC_FLOAT_VTABLE);
        (&raw mut (*p).cc_sensitivity)
            .cast::<u32>()
            .write(raw::image::CC_FLOAT_VTABLE);
        raw::engine_cc_bool(
            (&raw mut (*p).cc_microphone).cast(),
            c"s_use_microphone".as_ptr().cast(),
            &raw mut (*p).microphone,
            1,
            2,
        );
        raw::engine_cc_bool(
            (&raw mut (*p).cc_off).cast(),
            c"g_hide_spam".as_ptr().cast(),
            &raw mut (*p).off,
            1,
            2,
        );
        (&raw mut (*p).cc_microphone)
            .cast::<u32>()
            .write(raw::image::CC_BOOL_VTABLE);
        (&raw mut (*p).cc_off)
            .cast::<u32>()
            .write(raw::image::CC_BOOL_VTABLE);
        raw::engine_cc_string(
            (&raw mut (*p).cc_mutes).cast(),
            c"srp_voice_mutes".as_ptr().cast(),
            (&raw mut (*p).mutes).cast(),
            MUTES_SIZE as u32,
            1,
        );
    }
}
pub fn settings(_: &Thread) -> Settings {
    // SAFETY: no borrow escapes; console values are read once on the game thread.
    unsafe {
        let p = OPTIONS.0.get();
        let disabled = (*p).off != 0;
        Settings {
            volume: if disabled { 0.0 } else { (*p).volume / 100.0 },
            microphone: !disabled && (*p).microphone != 0,
            sensitivity: (*p).sensitivity / 100.0,
            disabled,
        }
    }
}
pub fn mutes(_: &Thread) -> [u8; MUTES_SIZE] {
    unsafe { (*OPTIONS.0.get()).mutes }
}
pub fn write_mutes(_: &Thread, mutes: &[u8; MUTES_SIZE]) {
    unsafe {
        (*OPTIONS.0.get()).mutes = *mutes;
    }
}
#[unsafe(no_mangle)]
#[unsafe(link_section = ".data")]
pub static mute_action_name: [u8; 18] = *b"mute_last_speaker\0";
