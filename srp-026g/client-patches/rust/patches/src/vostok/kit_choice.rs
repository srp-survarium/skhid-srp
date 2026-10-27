//! A kit chosen before the start (patch 31), as the game has it:
//! - `respawn_rule.tick`: player_respawn_rule_base::tick at its start, ECX
//!   the rule, [esp+8] the tick's time; the rule's core, its places (each
//!   30h bytes: three kits' players, the kit in use, the next kit), the time
//!   rule's state, the statistics handler's records (a place's events and
//!   its damage received from players);
//! - `kit_view.camera`: the client's view of a spawn into another kit, its
//!   `push 1` / `call 1F8200h` (the player camera), ESI the network client,
//!   EAX the game world;
//! - `hud.alive` and `hud.update`: game_world_ui's state of the local
//!   player, ESI the ui, EBP the local player; at `hud.alive` AL and EDX as
//!   the original's call of show_stamina has them.
//!
//! What those registers and fields hold is said here, once.

use core::ffi::c_void;
use core::marker::PhantomData;

use super::context::{Context, Reg};
use super::hook::Thread;
use super::object::{Object, pointer, read};
use super::types::GameTime;
use crate::raw;

/// The places a match may have: the core's buffer holds 20 (0x2CFF50).
pub const PLACES: u32 = 20;
/// A place's kits: three.
pub const KITS: u8 = 3;
/// The time rule's state "in process" (play has begun).
const IN_PROCESS: u32 = 3;

/// The respawn rule's tick, at its start.
pub struct RespawnTick<'t> {
    rule: Object,
    core: Object,
    time: u32,
    thread: PhantomData<&'t Thread>,
}

impl<'t> RespawnTick<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<RespawnTick<'t>> {
        let rule = Object::at(ctx.reg(Reg::Ecx))?;
        // SAFETY: ECX at the site is the respawn rule; its core.
        let core = unsafe { pointer(raw::player_respawn_rule::core(rule.ptr())) }?;
        Some(RespawnTick { rule, core, time: ctx.stack(8), thread: PhantomData })
    }

    pub fn time(&self) -> GameTime {
        GameTime(self.time)
    }

    /// The time rule's state is waiting (1) or counting down (2): play has
    /// not begun before this tick.
    pub fn before_play(&self) -> bool {
        // SAFETY: the core's field.
        let state = unsafe { read(raw::game_core::time_state(self.core.ptr())) };
        state.wrapping_sub(1) >> 1 == 0
    }

    /// The match's place `index` (none: past the match's places).
    pub fn place(&self, index: u32) -> Option<Place<'t>> {
        // SAFETY: the core's places and their end; a place's address is
        // compared with the end before any read of it.
        unsafe {
            let first = Object::at(read(raw::game_core::places(self.core.ptr())))?;
            let end = read(raw::game_core::places_end(self.core.ptr()));
            let size = raw::place::next(first.ptr()) as usize as u32 - first.id();
            let at = first.id().wrapping_add(index.wrapping_mul(size));
            if at >= end {
                return None;
            }
            Object::at(at).map(|this| Place { this, index, thread: PhantomData })
        }
    }

    /// Whether the place has no statistics yet: no event of any kind, no
    /// damage received from a player (its record all zero; none without a
    /// statistics handler).
    pub fn clean(&self, place: &Place<'_>) -> bool {
        // SAFETY: the core's statistics handler, its records inline, a
        // place's record's bytes up to its end.
        unsafe {
            let Some(handler) = pointer(raw::game_core::statistics(self.core.ptr())) else { return false };
            let first = raw::statistics_handler::records(handler.ptr()).cast::<c_void>();
            let stride = raw::statistics_record::next(first) as usize - first as usize;
            let record = first.wrapping_byte_add(stride * place.index as usize);
            let words = raw::statistics_record::events(record).cast::<u32>();
            let size = raw::statistics_record::end(record) as usize - words as usize;
            (0..size / 4).all(|i| read(words.add(i)) == 0)
        }
    }

    /// The rule's spawn of `player` (the place's, out of the world first,
    /// as the tick takes a player it respawns out: vtable+24h with 1) at its
    /// feet and yaw with `protection`, at the tick's time (rule
    /// vtable+5Ch), as the tick makes them for a place it respawns.
    pub fn respawn(&self, place: &Place<'_>, player: &PlacePlayer<'_>, protection: u32) {
        // SAFETY: the player's own fields; its slot and the rule's slot
        // (thiscall, the respawn point a 24h-byte local the spawn reads).
        unsafe {
            let feet = raw::player::feet(player.this.ptr());
            let point = [protection, protection, read(feet.cast::<u32>()), read(feet.add(1).cast::<u32>()),
                         read(feet.add(2).cast::<u32>()), read(raw::player::yaw(player.this.ptr()).cast::<u32>()),
                         protection, protection, protection];
            raw::engine_remove_from_world(player.this.ptr(), 1);
            raw::engine_respawn_spawn(self.rule.ptr(), place.index, point.as_ptr().cast(), self.time);
        }
    }
}

/// A place of the match.
pub struct Place<'t> {
    this: Object,
    index: u32,
    thread: PhantomData<&'t Thread>,
}

impl<'t> Place<'t> {
    /// The kit in use.
    pub fn kit(&self) -> u8 {
        // SAFETY: a place's byte.
        unsafe { read(raw::place::kit(self.this.ptr())) }
    }

    /// The next kit.
    pub fn next_kit(&self) -> u8 {
        // SAFETY: as kit.
        unsafe { read(raw::place::next_kit(self.this.ptr())) }
    }

    /// The player of kit `kit` (0..3), if there is one.
    pub fn player(&self, kit: u8) -> Option<PlacePlayer<'t>> {
        if kit >= KITS {
            return None;
        }
        // SAFETY: the place's three players, each a pointer or 0.
        unsafe { pointer(raw::place::players(self.this.ptr()).add(usize::from(kit))) }
            .map(|this| PlacePlayer { this, thread: PhantomData })
    }
}

/// A place's player.
pub struct PlacePlayer<'t> {
    this: Object,
    thread: PhantomData<&'t Thread>,
}

impl PlacePlayer<'_> {
    /// Alive and in the world (so not due to spawn).
    pub fn alive_in_world(&self) -> bool {
        // SAFETY: the player's two bytes.
        unsafe { read(raw::player::alive(self.this.ptr())) != 0 && read(raw::player::inserted(self.this.ptr())) != 0 }
    }

    /// The time its invulnerability lasts until (u32::MAX: none), through
    /// its damage model's holder (vtable+8), as the spawn finds it.
    pub fn invulnerable_until(&self) -> Option<u32> {
        // SAFETY: the player's holder interface, its own slot (the damage
        // model pointer's address), the damage model's field.
        unsafe {
            let holder = raw::player::damage_holder(self.this.ptr()).cast::<c_void>();
            let model = pointer(raw::engine_damage_model_ptr(holder).cast())?;
            Some(read(raw::damage_model::invulnerable_until(model.ptr())))
        }
    }
}

/// The client's view of a spawn into another kit, at `kit_view.camera`.
pub struct KitView<'t> {
    client: Option<Object>,
    world: u32,
    thread: PhantomData<&'t Thread>,
}

impl<'t> KitView<'t> {
    pub fn at(ctx: &'t Context<'_>) -> KitView<'t> {
        KitView { client: Object::at(ctx.reg(Reg::Esi)), world: ctx.reg(Reg::Eax), thread: PhantomData }
    }

    /// The camera was last set for play (the time rule's state 3).
    pub fn in_play(&self) -> bool {
        // SAFETY: ESI at the site, the network client; its field.
        self.client.is_some_and(|client| unsafe { read(raw::network_client::shown_state(client.ptr())) } == IN_PROCESS)
    }

    /// The warmup's camera and input (switch_to_warmup_camera).
    pub fn warmup_camera(&self) {
        // SAFETY: the game's own switch (EDX the game world, EAX at the
        // site).
        unsafe { raw::engine_switch_to_warmup_camera(self.world as usize as *mut c_void) }
    }
}

/// game_world_ui's state of the local player, at `hud.alive` and
/// `hud.update`.
pub struct HudState<'t> {
    ui: Option<Object>,
    player: Option<Object>,
    al: u32,
    edx: u32,
    thread: PhantomData<&'t Thread>,
}

impl<'t> HudState<'t> {
    pub fn at(ctx: &'t Context<'_>) -> HudState<'t> {
        HudState { ui: Object::at(ctx.reg(Reg::Esi)), player: Object::at(ctx.reg(Reg::Ebp)), al: ctx.reg(Reg::Eax),
                   edx: ctx.reg(Reg::Edx), thread: PhantomData }
    }

    /// The local player is alive.
    pub fn player_alive(&self) -> bool {
        // SAFETY: EBP at the sites, the local player.
        self.player.is_some_and(|player| unsafe { read(raw::player::alive(player.ptr())) } != 0)
    }

    /// The original call at `hud.alive`: show_stamina with AL and EDX as
    /// the site has them.
    pub fn show_stamina(&self) {
        // SAFETY: the game's own call, its two registers the site's.
        unsafe { raw::engine_show_stamina(self.al, self.edx as usize as *mut c_void) }
    }

    /// The kits shown when the camera was set for anything but play, hidden
    /// when for play (show_profiles_selector returns at once when they are
    /// as asked).
    pub fn kits_until_play(&self) {
        let Some(ui) = self.ui else { return };
        // SAFETY: the ui's game world, its game and network client, the
        // client's field; the game's own show_profiles_selector (EAX the ui,
        // the flag's low byte on the stack).
        unsafe {
            let Some(client) = pointer(raw::game_world_ui::world(ui.ptr()))
                .and_then(|world| pointer(raw::game_world::game(world.ptr()).cast()))
                .and_then(|game| pointer(raw::game::network_client(game.ptr()).cast()))
            else {
                return;
            };
            let shown = read(raw::network_client::shown_state(client.ptr())) != IN_PROCESS;
            raw::engine_show_profiles_selector(ui.ptr(), u32::from(shown));
        }
    }
}
