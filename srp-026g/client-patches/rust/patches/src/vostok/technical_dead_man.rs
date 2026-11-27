//! Dead Man's Switch (432, patch 180, technical-skills.md): the native side. A dead player's game
//! sends no player input (game_world::tick 0x1F7883; set_input skips a dead player, 0x2D86EE), but
//! it sends its profile input (select_profile 0x1F8070, 0xDE, relayed as 0xF3), which every game
//! keeps per place in time order ([core +89D8h] + place x 480h) and its place's tick reads
//! (apply_profile_input 0x321790, which keeps the next kit for a value of 3 or more). So the switch
//! is a profile input of [`INPUT`]; each game's place tick plays it at the tick that takes it:
//! pull_pin 0x30A120 on one of the player's smokes, then the grenade's own throw (vtable +18h)
//! from the body's feet with no speed. From then on it is an ordinary thrown grenade (saved,
//! exploding at its tick, in a late join and a rollback as any).
use technical_skills::dead_mans_switch::{DROP_HEIGHT, INPUT, ProfileInput, Tick, on_grid, wire_tick};

use super::context::Context;
use super::world::{floats, read};
use crate::raw;

/// `m_explode_time_ms`'s "not armed".
const NOT_ARMED: u32 = u32::MAX;
/// The throw_grenade action (the action table's 0xA74EB0: 20h, kind 5, default key G).
const THROW_GRENADE: u32 = 0x20;
/// The kits of a place.
const KITS: u8 = 3;
/// A bound on a place's kit-choice list walk (its 480h block holds a few dozen nodes).
const MOST_NODES: usize = 256;

/// One life of a player: its place, kit and spawn time (m_inserted_time_in_ms, one death a life).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Life {
    pub place: u8,
    pub kit: u8,
    pub spawned: u32,
}

/// Why a switch drops nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The place's current player is alive or out of the world.
    NotDead,
    /// Its kit has no Dead Man's Switch.
    NoSkill,
    /// It has no smoke left, or one of its smokes is armed or flying (one at a time).
    NoSmoke,
    /// The smoke would go off after the place's respawn, which takes it out unexploded.
    TooLate,
}

/// A dead player's smoke, ready to drop: its stack (pull_pin arms its grenades[amount - 1]) and fuse.
#[derive(Clone, Copy)]
struct Smoke {
    set: raw::GrenadeSet,
    ttl: u16,
}

/// `item` as a grenade stack, none for another item (a trap set answers no grenade set).
///
/// # Safety
/// `item` is a live item.
unsafe fn grenade_set(item: raw::Item) -> Option<raw::GrenadeSet> {
    // SAFETY: an item's own vtable word; the cast only after it is a grenade set's.
    unsafe {
        let table = read(item.vtable());
        (table == raw::image::GRENADE_SET_VTABLE || table == raw::image::GRENADE_SET_CORE_VTABLE)
            .then(|| item.grenade_set())
    }
}

/// Whether a grenade stack is of smoke: its splash hurts nothing (radius and damage 0, the
/// rdg-m's alone among the game's grenades: throwables.tsv).
///
/// # Safety
/// `set` is a live grenade stack.
unsafe fn is_smoke(set: raw::GrenadeSet) -> bool {
    // SAFETY: the stack's own config copies, set at its cook.
    unsafe { read(set.splash_radius()) == 0.0 && read(set.splash_max_damage()) == 0.0 }
}

/// A grenade object as its saved fields say: armed (its fuse running) or flying.
///
/// # Safety
/// `grenade` is a live grenade of a live stack.
unsafe fn live(grenade: raw::GrenadeCore) -> bool {
    // SAFETY: the grenade's own fields.
    unsafe { read(grenade.explode_time()) != NOT_ARMED || read(grenade.physics()) != 0 }
}

/// `player`'s smoke to drop: none while one of its smokes is armed or flying, else the first stack
/// (in slot order, the same in every game) with a unit left whose next object (pull_pin arms
/// grenades[amount - 1]) is there.
///
/// # Safety
/// `player` is a live match player; its inventory owns its slots, their items and each stack's
/// objects (made at the cook, kept for the match).
unsafe fn smoke_of(player: raw::Player) -> Option<Smoke> {
    unsafe {
        let inventory = player.inventory()?;
        let first = inventory.slots();
        let count = inventory.slots_end().index_from(first).unwrap_or(0);
        let mut found = None;
        for slot in 0..count {
            let Some(set) = first.at(slot).item().and_then(|item| grenade_set(item)) else { continue };
            if !is_smoke(set) {
                continue;
            }
            let (Some(objects), Some(end)) = (set.grenades(), set.grenades_end()) else { continue };
            let number = end.index_from(objects).unwrap_or(0);
            if (0..number).filter_map(|n| objects.at(n).grenade()).any(|grenade| live(grenade)) {
                return None;
            }
            let amount = usize::from(read(set.amount()));
            if found.is_none() && amount > 0 && amount <= number {
                found = Some(Smoke { set, ttl: read(set.ttl()) });
            }
        }
        found
    }
}

/// Whether `player`, at the tick `tick` of its world `core`, may drop a smoke: dead and still in
/// the world, the skill in its current kit, a smoke ready, and the fuse ending before its respawn.
///
/// # Safety
/// `player` is a live match player of `core`.
unsafe fn ready(player: raw::Player, core: raw::CoreWorld, tick: Tick) -> Result<Smoke, Refusal> {
    // SAFETY: the caller's live player and core.
    unsafe {
        if read(player.alive()) != 0 || read(player.inserted()) == 0 {
            return Err(Refusal::NotDead);
        }
        if !super::technical::skills(player).dead_mans_switch() {
            return Err(Refusal::NoSkill);
        }
        let smoke = smoke_of(player).ok_or(Refusal::NoSmoke)?;
        let place = read(player.place());
        match super::respawn::respawn_time(core, place) {
            Some(respawn) if !tick.before_respawn(smoke.ttl, respawn) => Err(Refusal::TooLate),
            _ => Ok(smoke),
        }
    }
}

/// A 4x4 the game reads with SSE (the rigid body's transform): 16-byte aligned.
#[repr(C, align(16))]
struct Matrix([f32; 16]);

/// The drop: pull_pin arms the stack's next grenade (its fuse from now, the stack one unit less,
/// the thrower the holder), then its throw puts it in the physics world at the body's feet, a few
/// centimetres up, with no speed. The calls grenade_set_core::pull_pin and throw_grenade make, in
/// one tick.
///
/// # Safety
/// `player` is the live dead player whose `smoke` [`ready`] found at `now`.
unsafe fn drop_smoke(player: raw::Player, smoke: Smoke, now: u32) -> bool {
    // SAFETY: the stack and its objects are live; both calls are the game's own on them.
    unsafe {
        let index = raw::engine_pull_pin(Some(smoke.set), now);
        let (Some(objects), Some(end)) = (smoke.set.grenades(), smoke.set.grenades_end()) else { return false };
        if usize::from(index) >= end.index_from(objects).unwrap_or(0) {
            return false;
        }
        let Some(grenade) = objects.at(usize::from(index)).grenade() else { return false };
        let mut body = Matrix(floats(player.transform()));
        body.0[13] += DROP_HEIGHT;
        let still = [0.0f32; 3];
        raw::engine_grenade_throw(Some(grenade), body.0.as_ptr(), still.as_ptr());
        true
    }
}

/// What a place's tick did with a switch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Played {
    Dropped { place: u8, now: u32 },
    Refused { place: u8, now: u32, why: Refusal },
}

/// Whether the place's kit-choice list has a switch this tick takes. The whole list is walked (in
/// every game the same nodes, whatever order the network put equal times in).
///
/// # Safety
/// `list` is a live place's list of a hook's core.
unsafe fn switch_in(list: raw::KitChoiceList, core: raw::CoreWorld, now: u32) -> bool {
    // SAFETY: the list's nodes, linked from its newest, live with the core.
    unsafe {
        let mut node = list.newest();
        let mut tick = None;
        for _ in 0..MOST_NODES {
            let Some(at) = node else { return false };
            if ProfileInput::parse(read(at.value())) == ProfileInput::Switch {
                let tick = *tick.get_or_insert_with(|| Tick { now, length: read(core.tick_ms()) });
                if tick.takes(read(at.time())) {
                    return true;
                }
            }
            node = at.older();
        }
        false
    }
}

/// The place's tick (`technical.place_tick`, every game): a switch of this tick drops the place's
/// player's smoke when it may. None when the tick takes no switch.
pub fn place_tick(ctx: &Context<'_>) -> Option<Played> {
    use raw::site::technical_place_tick as site;
    // SAFETY: the tick's live place, its world core, the core's lists and the place's players.
    unsafe {
        let (slot, frame) = (site::place(ctx)?, site::frame(ctx)?);
        let now = read(frame.time());
        let core = slot.core()?;
        let place = read(slot.place());
        let list = core.kit_choices()?.at(usize::from(place));
        if !switch_in(list, core, now) {
            return None;
        }
        let player = match read(slot.kit_in_use()) {
            0 => slot.kit0(),
            1 => slot.kit1(),
            2 => slot.kit2(),
            _ => None,
        };
        let Some(player) = player else {
            return Some(Played::Refused { place, now, why: Refusal::NotDead });
        };
        let tick = Tick { now, length: read(core.tick_ms()) };
        Some(match ready(player, core, tick) {
            Ok(smoke) if drop_smoke(player, smoke, now) => Played::Dropped { place, now },
            Ok(_) => Played::Refused { place, now, why: Refusal::NoSmoke },
            Err(why) => Played::Refused { place, now, why },
        })
    }
}

/// The local player's life when this game may send its switch now: dead, the skill, a smoke ready
/// and the fuse ending before its respawn by this game's own clock. `game` is a live game.
///
/// # Safety
/// `game` is the hook's live game.
unsafe fn local_ready(game: raw::Game) -> Option<Life> {
    // SAFETY: the game's network client, its local player (alive for the hook) and its world core.
    unsafe {
        let player = game.network_client()?.local_player()?;
        let core = player.world_core()?;
        let tick = Tick { now: read(core.time()), length: read(core.tick_ms()) };
        ready(player, core, tick).ok()?;
        let kit = read(player.kit());
        (kit < KITS).then(|| Life { place: read(player.place()), kit, spawned: read(player.spawned()) })
    }
}

/// A key down the game world's handler got (`technical.game_key`): none for another key than the
/// throw key; else the local player's life when the switch may go.
pub fn throw_key(ctx: &Context<'_>) -> Option<Option<Life>> {
    use raw::site::technical_game_key as site;
    // SAFETY: the site's registers: the binder's answer and the live game.
    unsafe {
        if site::action(ctx) != THROW_GRENADE {
            return None;
        }
        Some(site::game(ctx).and_then(|game| local_ready(game)))
    }
}

/// Whether a profile input this game queued at `time` could share its wire tick with the newest
/// one it queued before (its own are appended at the list's newest end, change_input<profile_input>
/// 0x1FBE67): that one's wire tick is the new one's or later. Two of a place in one wire tick are
/// the same time in the other games, ordered there by their relays and here by their ms, and the
/// server moves the second (`place`) by a tick byte that names both in this game.
///
/// # Safety
/// `game` is the hook's live game.
unsafe fn busy(game: raw::Game, time: u32) -> bool {
    // SAFETY: the game's match core and its lists, the local player for its place; all alive for
    // the hook on the game's thread.
    unsafe {
        let newest = || {
            let core = game.world().pvp_match()?.core();
            let place = read(game.network_client()?.local_player()?.place());
            let node = core.kit_choices()?.at(usize::from(place)).newest()?;
            Some(read(node.time()))
        };
        newest().is_some_and(|last| wire_tick(last) >= wire_tick(time))
    }
}

/// A profile input this game is about to queue (`technical.select_profile`): what it asks for, and
/// whether it would share a wire tick with the one before.
pub struct Queued {
    pub input: ProfileInput,
    pub busy: bool,
}

impl Queued {
    pub fn at(ctx: &Context<'_>) -> Option<Queued> {
        use raw::site::technical_select_profile as site;
        // SAFETY: select_profile's live game world, its game, and the argument slot.
        unsafe {
            let game = site::world(ctx)?.game()?;
            let value = read(site::frame(ctx)?.value());
            Some(Queued { input: ProfileInput::parse(value), busy: busy(game, read(game.time())) })
        }
    }
}

/// At the frame's end (`game.frame_end`), with the game's time still the one this frame's inputs
/// got: what may be queued now.
pub struct FrameEnd {
    game: raw::Game,
    time: u32,
}

impl FrameEnd {
    pub fn at(ctx: &Context<'_>) -> Option<FrameEnd> {
        // SAFETY: the site's live game; its time not yet the new frame's.
        unsafe {
            let game = raw::site::game_frame_end::game(ctx)?;
            Some(FrameEnd { game, time: read(game.time()) })
        }
    }

    /// The local player's life when the switch may go now.
    pub fn ready(&self) -> Option<Life> {
        // SAFETY: the site's live game.
        unsafe { local_ready(self.game) }
    }

    /// Whether an input queued now could share a wire tick with the last one ([`busy`]; the
    /// switch's time on the grid is in this one's wire tick or the next).
    pub fn busy(&self) -> bool {
        // SAFETY: the site's live game.
        unsafe { busy(self.game, self.time) }
    }

    /// Queues `value` as the hud queues a kit pick (select_profile on the game's world, which
    /// needs the local player and the world's match: checked), at `time` as the game's time.
    ///
    /// # Safety
    /// The site's live game.
    unsafe fn queue(&self, value: u8, time: u32) -> bool {
        // SAFETY: the game's embedded world, its match and local player alive for the hook; the
        // call is the hud's own (0x233F23); the game's time word is put back before the hook
        // returns, and the site's own instruction then sets the new frame's.
        unsafe {
            let world = self.game.world();
            if world.pvp_match().is_none() || self.game.network_client().and_then(|c| c.local_player()).is_none() {
                return false;
            }
            *self.game.time() = time;
            raw::engine_select_profile(Some(world), u32::from(value));
            *self.game.time() = self.time;
            true
        }
    }

    /// Queues a kit pick the hud made in the wire tick of this game's last input, at the game's
    /// time; nothing without a match.
    pub fn send_kit(&self, kit: u8) {
        // SAFETY: the site's live game.
        unsafe { self.queue(kit, self.time) };
    }

    /// Queues the switch at the grid's next time ([`on_grid`]): the wire carries a profile input's
    /// tick alone, so a time off the grid would be another here than in the other games.
    pub fn send_switch(&self) -> bool {
        // SAFETY: the site's live game.
        unsafe { self.queue(INPUT, on_grid(self.time)) }
    }
}
