//! Research (exe patch 53): the rule on 0.26g build 2777's own interfaces.
//!
//! - a rule cook of class [`CLASS`] (the mode configs' rules [93, 91, 1E0h,
//!   94]), registered by the game's constructor (research_026g.asm, entry
//!   `register`), whose resources are [`Rule`]s: game_match_rule_base (type 5)
//!   with the twenty slots of 0.26g's rules, the tick a delegate of the
//!   world's second tick list, as timelimit_rule_core's;
//! - the hud (entry `hud`, every frame of game_world_ui::update_ui): the
//!   movie's root.research_update, the units' messages, the end's.
//!
//! Every address is a VA of the original exe (image base 0x10000; it is never
//! rebased). The proof of each is in docs/notes/client_patch.md section 3.50
//! and tools/re-kit/v026g. The logic is the crate research-rule; this file
//! reads the game's memory into its types once and writes its results back.
//! The game enters only through research_026g.asm's stubs (`rn_*`, the stack
//! aligned to 16), which call the `research_*` functions here (cdecl, the
//! rule or cook first). The mutable state of the patch is [`STATE`], the
//! arena's block research.state. The game's functions are called through the
//! engine-function library (`engine_026g.rs`: plain cdecl functions, whatever
//! registers the game's take).

#![no_std]

use core::ffi::{CStr, c_void};
use core::mem::{MaybeUninit, offset_of, size_of};
use core::ptr::{NonNull, null_mut};

use arena_rt::c_u32;
use research_rule::{self as rr, Award, AwardKind, Awards, Body, Params, Place, ShapeKind, State, Team};
use research_rule::{MAX_CAPTURE, MAX_OWNED, MAX_SHAPES, MAX_ZONES, MICRO, PLAYERS, RATE_MS};

mod engine {
    include!("../../engine_026g.rs");
}
use engine::*;

/// The rule's resource class (free: register_cook's table has 0x206).
const CLASS: u32 = 0x1E0;
/// game_match_rule_type: 0 respawn, 1 gather, 2 timelimit, 3 kd, 4 tdm.
const RULE_TYPE: u32 = 5;
/// The configs' mode.mode.
const MODE: u32 = 5;
/// The client rule cooks' size (tdm's: 0x2261B54..0x2261B78).
const COOK_SIZE: usize = 0x24;

// ---- the game --------------------------------------------------------------

/// register_cook's table: begin, end.
const COOKS: usize = 0xA86E00;
/// [import] GetCurrentThreadId.
const CURRENT_THREAD_ID: usize = 0x870118;
/// The rule cooks' allocator (engine_malloc's).
const ALLOCATOR: usize = 0x1F1D9C4;
/// The client tdm rule cook's allocation name (0x420789: ecx + 8).
const ALLOCATION_INFO: usize = 0xACCC60 + 8;

// the world (game_world_core)
/// -> places, 30h each: [3] players, +Ch the current kit.
const CORE_PLACES: usize = 0x18458;
/// The second tick list: count, head, tail (timelimit's).
const CORE_TICK_LIST: usize = 0x18850;
/// -> the tick delegates' counter.
const CORE_TICK_ORDER: usize = 0x18FB0;
/// -> the statistics handler (below).
const CORE_STATISTICS: usize = 0x18FA8;
/// The time rule's state: 3 in play.
const CORE_STATE: usize = 0x18FE4;
const IN_PLAY: u32 = 3;
/// u8: the game's own roster place.
const CORE_LOCAL_PLACE: usize = 0x18FDC;
/// begin, end of the rules.
const CORE_RULES: usize = 0x18824;
const PLACE_SIZE: usize = 0x30;
const PLACE_KIT: usize = 0xC;
// a player (base_player)
/// m_is_alive.
const PLAYER_ALIVE: usize = 0x33C;
/// In the world's tick lists.
const PLAYER_IN_WORLD: usize = 0x33F;
/// m_transform's translation (+40Ch row 3): the feet.
const PLAYER_POSITION: usize = 0x43C;
/// -> -> its character controller (0x2D942A).
const PLAYER_PHYSICS: usize = 0x494;
// the controller (get_transform 0x7F5C0: the player's transform is the
// capsule's centre moved down by heights[(stance + 2) % 3] + heights[stance],
// half the capsule's height: 0.9 m standing)
/// float[3].
const CONTROLLER_HEIGHTS: usize = 0x1D0;
/// 0..2.
const CONTROLLER_STANCE: usize = 0x1F0;
/// -> its match profile.
const PLAYER_PROFILE: usize = 0x6B8;
const PROFILE_TEAM: usize = 0x45EC;
// [core+18FA8h] is the network client's base_game_statistics_handler (its
// slot +34h, deserialize 0x26DB60, is what a world's load calls at
// 0x2D2B0F), not the shared statistics: those are at +8 in it (the handler's
// slots pass ecx + 8 to shared_statistics' functions, 0x26DA3C), and the
// offsets below are the handler's.
/// shared_statistics.
const STATS_SHARED: usize = 0x8;
/// A player's record.
const STATS_PLAYER: usize = 0x1F8;
/// u16 events[28][3] at the record's start (recalculate_scores 0x26DC30;
/// the death at shared + place x 1F8h + 0Ch, 0x2F4F4E, is event 2's kit 0).
const STATS_EVENTS: usize = STATS_SHARED;
/// u16 m_team_points[2]: shared +31CCh.
const STATS_TEAM_POINTS: usize = 0x31D4;
/// m_winner_team: shared +31DCh.
const STATS_WINNER: usize = 0x31E4;
/// The shared statistics' event listener (a boost::function, shared +31A0h:
/// 0x2F4D9F; 0 when none).
const STATS_LISTENER: usize = STATS_SHARED + 0x31A0;
// the statistics' events a unit counts (snapshot_rules.md section 3.1)
/// Hundredths: a unit taken, the shares of `award`.
const EVENT_CAPTURE: u32 = 6;
/// One: an enemy killed inside a unit his team was taking.
const EVENT_KILL_INVADER: u32 = 10;
/// Hundredths: a unit taken from the other team.
const EVENT_NEUTRALISE: u32 = 18;
const EVENTS: u32 = 28;
const KITS: u8 = 3;
/// match_hud_state (fill_match_hud_state).
const HUD_TEAM_POINTS: usize = 0x90;
// game_world_ui
const UI_GAME: usize = 0x18;
const UI_HUD: usize = 0x8;
const UI_MODE: usize = 0x20;
/// -> the match; its world at +1F8h.
const GAME_MATCH: usize = 0x4D4C;
const MATCH_WORLD: usize = 0x1F8;

// the rules' default slots (game_match_rule_base and tdm_rule_core)
const RULE_DELETE: u32 = 0x266710;
const RESOURCE_SLOT1: u32 = 0xF1900;
const RESOURCE_SLOT2: u32 = 0xF1F00;
const RESOURCE_SLOT3: u32 = 0xF1F40;
const RESOURCE_NONE: u32 = 0xFDA90;
const RETURN_4: u32 = 0x75AF0;
const RETURN_0: u32 = 0x1AF330;
const PRIORITY_3: u32 = 0x8FE10;
/// `ret 1Ch`.
const RESOLVE_LINKS: u32 = 0x316540;
/// game_match_rule_base's RTTI locators.
const RULE_BASE_LOCATOR: u32 = 0xA4C970;
const LINK_RESOLVER_LOCATOR: u32 = 0xA4C95C;
// MSVC's RTTI of the cook (rn_cook_locator): type_info's vftable (every type
// descriptor's first word, tdm_rule_cook's 0xACCC38 among them) and the base
// class descriptors of tdm_rule_core_cook's hierarchy (0xA4718C), which is
// the cook's shape: translate_query_cook (with cook_base) and boost's
// noncopyable at +20h.
const TYPE_INFO_VFTABLE: u32 = 0x886A78;
const BCD_TRANSLATE_QUERY_COOK: u32 = 0xA38D74;
const BCD_COOK_BASE: u32 = 0xA38D24;
const BCD_NONCOPYABLE: u32 = 0xA40010;

/// A word of the game's memory at `base + offset`.
unsafe fn get<T: Copy>(base: *const u8, offset: usize) -> T {
    unsafe { base.add(offset).cast::<T>().read_unaligned() }
}

unsafe fn put<T>(base: *mut u8, offset: usize, value: T) {
    unsafe { base.add(offset).cast::<T>().write_unaligned(value) }
}

// ---- the state page -------------------------------------------------------

/// The hud's cache, of the rule it was filled from.
#[repr(C)]
struct Hud {
    /// The rule the cache is of; none: the next frame fills it.
    rule: Option<NonNull<Rule>>,
    owner: [Option<Team>; MAX_ZONES],
    /// The end was announced.
    completed: bool,
    /// Frames since the movie got its last update.
    shown: u32,
    /// A checksum of what it got.
    sum: u32,
}

/// Whether the cook is the game's.
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum Registration {
    Pending = 0,
    Done = 1,
    /// The class was another cook's: no Research.
    ClassTaken = 2,
}

#[repr(C)]
struct Globals {
    cook: [u8; COOK_SIZE],
    registration: Registration,
    hud: Hud,
}
// check_research_026g.py reads the registration at +24h and the hud's rule at +28h.
const _: () = assert!(offset_of!(Globals, registration) == 0x24 && offset_of!(Globals, hud) == 0x28);
const _: () = assert!(size_of::<Globals>() == 0x38);

/// Zeroed: the block research.state (link_026g.py).
static mut STATE: Globals = Globals {
    cook: [0; COOK_SIZE],
    registration: Registration::Pending,
    hud: Hud { rule: None, owner: [Some(Team::Zero); MAX_ZONES], completed: false, shown: 0, sum: 0 },
};

// ---- the rule ---------------------------------------------------------------

/// A delegate of a world's tick list.
#[repr(C)]
struct Delegate {
    object: *mut Rule,
    function: *const c_void,
    previous: *mut Delegate,
    next: *mut Delegate,
    order: u32,
    enabled: u8,
    pad: [u8; 3],
}

/// Whether the mode config was read into the parameters.
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum Config {
    Unread = 0,
    Read = 1,
}

/// The rule the cook makes.
#[repr(C)]
pub struct Rule {
    /// vtable, unmanaged_resource, +108h link_resolver, +10Ch type.
    base: [u8; 0x110],
    tick: Delegate,
    core: Option<NonNull<u8>>,
    blackboard: *mut c_void,
    game: *mut c_void,
    config: Config,
    params: Params,
    state: State,
}
// check_research_026g.py reads the rule by these offsets; research_026g.asm's
// rn_set_core writes the tick delegate's function at +114h.
const _: () = assert!(offset_of!(Rule, tick) == 0x110 && offset_of!(Rule, core) == 0x128);
const _: () = assert!(offset_of!(Rule, tick) + offset_of!(Delegate, function) == 0x114);
const _: () = assert!(offset_of!(Rule, config) == 0x134 && offset_of!(Rule, params) == 0x138);
const _: () = assert!(offset_of!(Rule, state) == 0x7E0 && size_of::<Rule>() == 0x918);

impl Rule {
    /// The world, when the rule has one and its config was read.
    fn world(&self) -> Option<World> {
        match self.config {
            Config::Read => self.core.map(|core| World(core.as_ptr())),
            Config::Unread => None,
        }
    }
}

/// A world (game_world_core).
#[derive(Clone, Copy)]
struct World(*mut u8);

impl World {
    unsafe fn place(self, place: usize) -> Option<*mut u8> {
        let places: *mut u8 = unsafe { get(self.0, CORE_PLACES) };
        (!places.is_null()).then(|| unsafe { places.add(place * PLACE_SIZE) })
    }

    /// The player of a place, in the kit it plays.
    unsafe fn player(self, place: usize) -> Option<*const u8> {
        let at = unsafe { self.place(place) }?;
        let kit: u8 = unsafe { get(at, PLACE_KIT) };
        if kit >= KITS {
            return None;
        }
        let player: *const u8 = unsafe { get(at, 4 * usize::from(kit)) };
        (!player.is_null()).then_some(player)
    }

    unsafe fn statistics(self) -> Option<*mut u8> {
        let statistics: *mut u8 = unsafe { get(self.0, CORE_STATISTICS) };
        (!statistics.is_null()).then_some(statistics)
    }
}

/// A player's team as the game holds it (2 when it has no player or no
/// profile; whatever the profile says otherwise).
unsafe fn team_of(player: Option<*const u8>) -> u32 {
    let Some(player) = player else { return u32::from(rr::NOBODY) };
    let profile: *const u8 = unsafe { get(player, PLAYER_PROFILE) };
    if profile.is_null() { u32::from(rr::NOBODY) } else { unsafe { get(profile, PROFILE_TEAM) } }
}

/// Half the player's capsule's height, as get_transform (0x7F5C0) takes it
/// off the capsule's centre to give the player's transform; the config's
/// when the player has no controller or it says nothing sensible.
unsafe fn half_height_of(params: &Params, player: *const u8) -> f32 {
    let physics: *const *const u8 = unsafe { get(player, PLAYER_PHYSICS) };
    let controller = if physics.is_null() { core::ptr::null() } else { unsafe { *physics } };
    if controller.is_null() {
        return params.half_height;
    }
    let stance: i32 = unsafe { get(controller, CONTROLLER_STANCE) };
    if !(0..=2).contains(&stance) {
        return params.half_height;
    }
    let height = |i: i32| -> f32 { unsafe { get(controller, CONTROLLER_HEIGHTS + 4 * i as usize) } };
    let half = height((stance + 2) % 3) + height(stance);
    if half > 0.1 && half < 2.0 { half } else { params.half_height }
}

/// The world's players as the rule reads them: an entered place's player,
/// alive, in the world and of a team.
#[inline(never)]
unsafe fn players_of(rule: &Rule, world: World) -> [Option<Body>; PLAYERS] {
    let mut out = [None; PLAYERS];
    for (i, slot) in out.iter_mut().enumerate() {
        if !rule.state.entered.has(i) {
            continue;
        }
        let Some(player) = (unsafe { world.player(i) }) else { continue };
        let team = Team::from_raw(unsafe { team_of(Some(player)) });
        let alive: u8 = unsafe { get(player, PLAYER_ALIVE) };
        let in_world: u8 = unsafe { get(player, PLAYER_IN_WORLD) };
        let (Some(team), true) = (team, alive != 0 && in_world != 0) else { continue };
        let feet = unsafe { get::<[f32; 3]>(player, PLAYER_POSITION) };
        *slot = Some(Body { team, feet, half_height: unsafe { half_height_of(&rule.params, player) } });
    }
    out
}

/// An event counted as the game's own sites count one (the kill handler
/// 0x2F4EB0, 0x2F4DE1): the player's cell, then, when the statistics have
/// their event listener (shared +31A0h, a boost::function, not 0 once set),
/// its call through 0x2F5D40 (engine_statistics_notify: the listener, place,
/// event, amount). The listener is base_game_statistics_handler::on_event
/// 0x26DB80 (vtable 0xA0755C slot +44h, client_patch.md 3.16), ecx the
/// handler ([core+18FA8h]): the player's score (m_scores, +31F0h) grows by
/// the event's weight times the amount (hundredths for events 6, 17, 18), so
/// the Tab list shows it at once, not at the next world loaded.
///
/// Revision 30's build took the handler for the shared statistics: its cell
/// was 4 bytes on (kit 0's award in kit 2's cell, kit 1's and 2's in the
/// next event's: a capture in 7, a teammate cured, a neutralisation in 19,
/// a battery found, both weighed 2.5 by ones, 250 for a capture, and a kill
/// invader in 11, weighed 0), and its listener 8 bytes short (shared
/// +3198h, the batteries' records, 0 in Research: never told, the score
/// moved at the next world loaded).
unsafe fn count(world: World, place: Place, event: u32, amount: u32) {
    let (Some(statistics), Some(at)) = (unsafe { world.statistics() }, unsafe { world.place(place.index()) }) else {
        return;
    };
    if event >= EVENTS {
        return;
    }
    let kit: u8 = unsafe { get(at, PLACE_KIT) };
    let kit = if kit < KITS { usize::from(kit) } else { 0 };
    let cell = STATS_PLAYER * place.index() + STATS_EVENTS + event as usize * 2 * usize::from(KITS) + 2 * kit;
    let old = u32::from(unsafe { get::<u16>(statistics, cell) });
    let sum = old.wrapping_add(amount).min(0xFFFF);
    unsafe { put(statistics, cell, sum as u16) };
    let added = sum.wrapping_sub(old);
    if added == 0 || unsafe { get::<u32>(statistics, STATS_LISTENER) } == 0 {
        return;
    }
    unsafe { engine_statistics_notify(statistics.add(STATS_LISTENER).cast(), u32::from(place.get()), event, added) };
}

unsafe fn give(world: World, awards: &[Award]) {
    for award in awards {
        let event = match award.kind {
            AwardKind::Capture => EVENT_CAPTURE,
            AwardKind::Neutralise => EVENT_NEUTRALISE,
            AwardKind::KillInvader => EVENT_KILL_INVADER,
        };
        unsafe { count(world, award.player, event, award.amount) };
    }
}

/// The statistics' tail: the teams' points and the leader, as tdm_rule_core
/// writes them (0x3161F0).
unsafe fn publish(state: &State, world: World) {
    let Some(statistics) = (unsafe { world.statistics() }) else { return };
    unsafe {
        put(statistics, STATS_TEAM_POINTS, state.score[0]);
        put(statistics, STATS_TEAM_POINTS + 2, state.score[1]);
        put(statistics, STATS_WINNER, u32::from(rr::team_byte(rr::leader(state))));
    }
}

// ---- the rule's slots -------------------------------------------------------

// research_026g.asm's stubs: what the game calls (the vtables' slots, the
// tick delegate), each the stack aligned and the function below called.
unsafe extern "C" {
    fn rn_on_event();
    fn rn_entered();
    fn rn_left();
    fn rn_set_core();
    fn rn_set_blackboard();
    fn rn_serialize();
    fn rn_deserialize();
    fn rn_fill_hud();
    fn rn_completed();
    fn rn_cook_create();
    fn rn_cook_size();
}

/// The tick (rn_tick, the delegate's function: ecx the rule, (dt, time, step)).
///
/// # Safety
/// `rule` is a rule of the cook's, on the logic thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn research_rule_tick(rule: &mut Rule, dt: u32, time: u32, _step: u32) {
    let Some(world) = rule.world() else { return };
    if unsafe { get::<u32>(world.0, CORE_STATE) } != IN_PLAY {
        return;
    }
    let players = unsafe { players_of(rule, world) };
    let mut awards = Awards::new();
    rr::tick(&mut rule.state, &rule.params, &players, dt, time, &mut awards);
    unsafe {
        give(world, awards.as_slice());
        publish(&rule.state, world);
    }
}

/// Slot 7, on_event(type, data, time): the world's dispatcher 0x2D1EA0
/// pushes the core's time ([core+18FC0h]), then its second argument, then
/// its first; the kill site (0x2D9EEE..0x2D9EF7) gives it 1 and
/// &{killer, victim} (tdm 0x316090 and kd 0x3237E0 read the data at
/// [esp+8]). The first build took (type, time, data): it read the time as
/// the kill's bytes.
///
/// # Safety
/// As [`research_rule_tick`]; `data` the event's.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn research_rule_on_event(rule: &mut Rule, kind: u32, data: *const u8, _time: u32) {
    let Some(world) = rule.world() else { return };
    match kind {
        0 => unsafe { publish(&rule.state, world) },
        1 if !data.is_null() => {
            let (raw_killer, raw_victim) = unsafe { (*data, *data.add(1)) };
            let (Some(killer), Some(victim)) = (Place::new(raw_killer), Place::new(raw_victim)) else { return };
            let team = |place: Place| Team::from_raw(unsafe { team_of(world.player(place.index())) });
            let (Some(killer_team), Some(victim_team)) = (team(killer), team(victim)) else { return };
            if let Some(award) = rr::kill(&rule.state, killer, victim, killer_team, victim_team) {
                unsafe { give(world, &[award]) };
            }
        }
        _ => {}
    }
}

/// # Safety
/// As [`research_rule_tick`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn research_rule_entered(rule: &mut Rule, place: u32, _time: u32) {
    if let Some(place) = Place::new(place as u8) {
        rr::entered(&mut rule.state, place);
    }
}

/// # Safety
/// As [`research_rule_tick`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn research_rule_left(rule: &mut Rule, place: u32, _time: u32) {
    if let Some(place) = Place::new(place as u8) {
        rr::left(&mut rule.state, place);
    }
}

/// timelimit_rule_core::set_game_world_core (0x318050): the tick a delegate
/// at the end of the world's second list, in its order of registration. The
/// delegate's function, rn_tick (research_026g.asm), is written by the stub
/// rn_set_core before it calls this (`mov dword [ecx+114h], rn_tick`: what
/// patch_026g.py's CALLBACKS finds it by).
///
/// # Safety
/// As [`research_rule_tick`]; `core` the game's world core.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn research_rule_set_core(rule: &mut Rule, core: *mut u8) {
    rule.core = NonNull::new(core);
    let this: *mut Rule = rule;
    let tick = &mut rule.tick;
    unsafe {
        let order: *mut u32 = get(core, CORE_TICK_ORDER);
        // tick.function is rn_tick: the stub rn_set_core put it there
        tick.object = this;
        tick.order = *order;
        *order = (*order).wrapping_add(1);
        tick.enabled = 1;
        let list = core.add(CORE_TICK_LIST).cast::<[*mut Delegate; 3]>();
        let count = list.cast::<u32>();
        *count = (*count).wrapping_add(1);
        tick.previous = (*list)[2];
        tick.next = null_mut();
        if (*list)[1].is_null() {
            (*list)[1] = tick;
        } else {
            (*(*list)[2]).next = tick;
        }
        (*list)[2] = tick;
    }
}

/// # Safety
/// As [`research_rule_tick`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn research_rule_set_blackboard(rule: &mut Rule, blackboard: *mut c_void) {
    rule.blackboard = blackboard;
}

/// A NUL-terminated name for the game.
#[derive(Clone, Copy)]
#[repr(transparent)]
struct Name(*const u8);
// SAFETY: a pointer to a string constant.
unsafe impl Sync for Name {}

/// The fields' names in the hash mismatch dump, in the wire's order; a
/// unit's by its letter (cp_id 0, 1, 2: A, B, C), short for the section's
/// room.
macro_rules! zone_names {
    ($z:literal: $($i:literal)*) => {[
        Name(concat!($z, ".progress\0").as_ptr()),
        Name(concat!($z, ".capturing\0").as_ptr()),
        Name(concat!($z, ".owner\0").as_ptr()),
        Name(concat!($z, ".count[0]\0").as_ptr()),
        Name(concat!($z, ".count[1]\0").as_ptr()),
        Name(concat!($z, ".present[0]\0").as_ptr()),
        Name(concat!($z, ".present[1]\0").as_ptr()),
        $(Name(concat!($z, ".marks[", $i, "]\0").as_ptr()),)*
    ]};
}
static ZONE_FIELDS: [[Name; 7 + PLAYERS]; MAX_ZONES] = [
    zone_names!("A": 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19),
    zone_names!("B": 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19),
    zone_names!("C": 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19),
];

/// The writers' file and function in the dump: the first build's names, so
/// that a dump reads as it did.
const FILE: &CStr = c".\\research_native_026g.c";
const SERIALIZE: &CStr = c"srp::research_rule::serialize";

/// One field through the game's writer of its size; `ordinal` stands for the
/// line (the field's place in the state).
unsafe fn field<T>(writer: *mut c_void, value: &T, ordinal: &mut u32, name: Name) {
    let write = match size_of::<T>() {
        1 => engine_write_u8,
        2 => engine_write_u16,
        _ => engine_write_u32,
    };
    let value: *const T = value;
    unsafe { write(value.cast(), writer, FILE.as_ptr().cast(), *ordinal, SERIALIZE.as_ptr().cast(), name.0) };
    *ordinal += 1;
}

/// The state field by field through the game's writers, as tdm_rule_core's
/// serialize (0x316340) does: the bytes [`rr::write`] gives (little-endian,
/// 21 + 96 a unit, what the server's decoder reads), and in the hash
/// mismatch dump each field named.
///
/// # Safety
/// As [`research_rule_tick`]; `writer` the game's buffer writer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn research_rule_serialize(rule: &Rule, writer: *mut c_void) {
    let state = &rule.state;
    let n = &mut 0;
    let head = |name: &'static str| Name(name.as_ptr());
    unsafe {
        field(writer, &state.points[0], n, head("m_points[0]\0"));
        field(writer, &state.points[1], n, head("m_points[1]\0"));
        field(writer, &state.score[0], n, head("m_score[0]\0"));
        field(writer, &state.score[1], n, head("m_score[1]\0"));
        field(writer, &state.end, n, head("m_end_time\0"));
        field(writer, &state.entered, n, head("m_entered\0"));
        field(writer, &(state.zone_count as u8), n, head("m_zone_count\0"));
        for (zone, names) in state.zones.iter().zip(&ZONE_FIELDS).take(state.zone_count as usize) {
            field(writer, &zone.progress, n, names[0]);
            field(writer, &zone.capturing, n, names[1]);
            field(writer, &zone.owner, n, names[2]);
            field(writer, &zone.count[0], n, names[3]);
            field(writer, &zone.count[1], n, names[4]);
            field(writer, &zone.present[0], n, names[5]);
            field(writer, &zone.present[1], n, names[6]);
            for (mark, &name) in zone.marks.iter().zip(&names[7..]) {
                field(writer, mark, n, name);
            }
        }
    }
}

/// Slot 16 takes two arguments and pops both (`ret 8`): the snapshot's rules
/// loop 0x2D2B42 pushes a dword of its own and the reader and does not clean
/// up, as tdm's 0x3163D0, timelimit's 0x317FD0 and kd's 0x323910 expect. The
/// dword is not read (none of the three reads it either).
///
/// # Safety
/// As [`research_rule_tick`]; `reader` the game's buffer reader, its bytes
/// a state's.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn research_rule_deserialize(rule: &mut Rule, reader: *mut u8, _unused: u32) {
    let at: *const u8 = unsafe { get(reader, 4) };
    let size = rr::serialized_size(&rule.state);
    let bytes = unsafe { core::slice::from_raw_parts(at, size) };
    if rr::read(&mut rule.state, &rule.params, bytes).is_none() {
        rr::reset(&mut rule.state, &rule.params);
    }
    unsafe { put(reader, 4, at.add(size)) };
    // A late joiner's first world: the time rule's deserialize (0x31803E)
    // sends event 0 before ours runs, so slot 7's publish() wrote the fresh
    // rule's 0 / 0 and a draw into the statistics' tail; the tail is the
    // loaded state's from here.
    if let Some(world) = rule.world() {
        unsafe { publish(&rule.state, world) };
    }
}

/// The teams' points, as tdm's 0x316430 puts its tickets. Its alive counts
/// (+94h, +95h) stay as the hud state's constructor (0x278D00) leaves them,
/// 0FFh, as in modes 2 and 3: only tdm's slot writes them (kd 0x323980 writes
/// +98h on, timelimit's 0x15A4B0 nothing) and only mode 4's last players read
/// them (update_ui 0x23B3DE).
///
/// # Safety
/// As [`research_rule_tick`]; `hud` the game's match_hud_state.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn research_rule_fill_hud(rule: &Rule, hud: *mut u8, _unused: u32) {
    unsafe {
        put(hud, HUD_TEAM_POINTS, rule.state.score[0]);
        put(hud, HUD_TEAM_POINTS + 2, rule.state.score[1]);
    }
}

/// # Safety
/// As [`research_rule_tick`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn research_rule_completed(rule: &Rule, time: u32) -> u32 {
    u32::from(rr::completed(&rule.state, time))
}

/// A word of a vtable or of RTTI: the game's address, our code's, our data's.
#[repr(C)]
#[derive(Clone, Copy)]
union Word {
    game: u32,
    code: unsafe extern "C" fn(),
    data: *const c_void,
}

#[repr(transparent)]
pub struct Table<const N: usize>([Word; N]);
// SAFETY: constants: addresses.
unsafe impl<const N: usize> Sync for Table<N> {}

const fn game(va: u32) -> Word {
    Word { game: va }
}

const fn ours(code: unsafe extern "C" fn()) -> Word {
    Word { code }
}

/// The rule's vtable: 0.26g's twenty slots (game_match_rule_base's, with
/// tdm_rule_core's defaults), its RTTI locator before them as MSVC lays them
/// out.
#[unsafe(no_mangle)]
pub static rn_rule_vtable: Table<{ 1 + 20 }> = Table([
    game(RULE_BASE_LOCATOR),
    game(RULE_DELETE),
    game(RESOURCE_SLOT1),
    game(RESOURCE_SLOT2),
    game(RESOURCE_SLOT3),
    game(RESOURCE_NONE),
    game(RESOURCE_NONE),
    game(RESOURCE_NONE),
    ours(rn_on_event),
    ours(rn_entered),
    ours(rn_left),
    game(RETURN_4),
    ours(rn_set_core),
    ours(rn_set_blackboard),
    game(RETURN_4),
    game(RETURN_0),
    ours(rn_serialize),
    ours(rn_deserialize),
    ours(rn_fill_hud),
    ours(rn_completed),
    game(PRIORITY_3),
]);

/// The link resolver's one slot.
#[unsafe(no_mangle)]
pub static rn_resolver_vtable: Table<2> = Table([game(LINK_RESOLVER_LOCATOR), game(RESOLVE_LINKS)]);

// ---- the cook -------------------------------------------------------------

/// The size of a rule (rn_cook_size: ecx the cook).
///
/// # Safety
/// Called by the game's translate.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn research_cook_rule_size(_cook: *mut u8) -> u32 {
    size_of::<Rule>() as u32
}

/// A config value of the game's (vostok::configs::binary_config_value).
#[repr(C)]
struct Cfg {
    /// A value, or a pointer (tables, vectors, strings).
    data: u32,
    data_high: u32,
    name: u32,
    name_high: u32,
    crc: u32,
    kind: u16,
    count: u16,
}
const _: () = assert!(size_of::<Cfg>() == 24);
const CFG_FLOAT: u16 = 2;
const CFG_INDEXED: u16 = 4;

unsafe fn child(table: *const Cfg, key: &CStr) -> *const Cfg {
    unsafe { engine_cfg_child(table.cast(), key.as_ptr().cast()).cast() }
}

unsafe fn has(table: *const Cfg, key: &CStr) -> bool {
    unsafe { engine_cfg_has(table.cast(), key.as_ptr().cast()) != 0 }
}

/// An indexed table's entry.
unsafe fn item(table: *const Cfg, index: usize) -> *const Cfg {
    unsafe { ((*table).data as *const Cfg).add(index) }
}

#[inline(never)]
unsafe fn number(value: *const Cfg) -> f32 {
    let value = unsafe { &*value };
    if value.kind == CFG_FLOAT { f32::from_bits(value.data) } else { value.data as i32 as f32 }
}

fn rounded(value: f32) -> u32 {
    if value <= 0.0 { 0 } else { c_u32(value + 0.5) }
}

#[inline(never)]
unsafe fn vector(value: *const Cfg) -> [f32; 3] {
    unsafe { *((*value).data as *const [f32; 3]) }
}

/// An indexed table's entries, when it is one of 1..=most.
unsafe fn entries(table: *const Cfg, most: usize) -> Option<usize> {
    let table = unsafe { &*table };
    let count = usize::from(table.count);
    (table.kind == CFG_INDEXED && count != 0 && count <= most).then_some(count)
}

/// mode.parameters of the mode config (0.27d's keys, and srp's for the
/// units: capture_zones[] {cp_id, position, shapes[] {type, position, axis_x,
/// axis_y, axis_z, half}}, capture_award, capture_reach,
/// capture_half_height), into `params` as far as they go; whether they can
/// be played.
unsafe fn read_params(params: &mut Params, root: *const Cfg) -> bool {
    unsafe {
        let mode = child(root, c"mode");
        if number(child(mode, c"mode")) != MODE as f32 {
            return false;
        }
        let parameters = child(mode, c"parameters");
        let times = child(parameters, c"capture_time_in_s");
        let rates = child(parameters, c"resource_per_s");
        if (*times).kind != CFG_INDEXED || (*rates).kind != CFG_INDEXED {
            return false;
        }
        params.capture_count = u32::from((*times).count).min(MAX_CAPTURE as u32);
        for i in 0..params.capture_count as usize {
            // 0.27d: trunc(s x 1000)
            params.capture_ms[i] = c_u32(number(item(times, i)) * 1000.0);
        }
        params.owned_count = u32::from((*rates).count).min(MAX_OWNED as u32);
        for i in 0..params.owned_count as usize {
            params.points_rate[i] = rounded(number(item(rates, i)) * (1000.0 * RATE_MS as f32));
        }
        params.target = c_u32(number(child(parameters, c"resources_count"))).wrapping_mul(MICRO);
        let restore = rounded(number(child(parameters, c"restore_multiplier")) * 1000.0);
        params.award = if has(parameters, c"capture_award") {
            c_u32(number(child(parameters, c"capture_award")))
        } else {
            100
        };
        params.reach = if has(parameters, c"capture_reach") { number(child(parameters, c"capture_reach")) } else { 0.38 };
        params.half_height = if has(parameters, c"capture_half_height") {
            number(child(parameters, c"capture_half_height"))
        } else {
            0.9
        };
        let zones = child(parameters, c"capture_zones");
        let Some(count) = entries(zones, MAX_ZONES) else { return false };
        params.zone_count = count as u32;
        for (z, def) in params.zones.iter_mut().enumerate().take(count) {
            let zone = item(zones, z);
            def.cp_id = c_u32(number(child(zone, c"cp_id")));
            def.position = vector(child(zone, c"position"));
            let shapes = child(zone, c"shapes");
            let Some(count) = entries(shapes, MAX_SHAPES) else { return false };
            def.shape_count = count as u32;
            for (s, out) in def.shapes.iter_mut().enumerate().take(count) {
                let shape = item(shapes, s);
                let Some(kind) = ShapeKind::from_raw(c_u32(number(child(shape, c"type")))) else { return false };
                out.kind = kind;
                out.centre = vector(child(shape, c"position"));
                out.axes[0] = vector(child(shape, c"axis_x"));
                out.axes[1] = vector(child(shape, c"axis_y"));
                out.axes[2] = vector(child(shape, c"axis_z"));
                out.half = vector(child(shape, c"half"));
            }
        }
        rr::prepare(params, restore)
    }
}

/// The client tdm cook's create (0x2063F0) for our rule (rn_cook_create:
/// ecx the cook): query = {match options, ..., +Ch the mode config's root}.
///
/// # Safety
/// Called by the game's translate.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn research_cook_create_rule(cook: *mut u8, query: *mut u8) -> *mut Rule {
    unsafe {
        let allocator: *mut c_void = get(ALLOCATOR as *const u8, 0);
        let rule = engine_malloc(allocator, size_of::<Rule>() as u32, ALLOCATION_INFO as *const c_void).cast::<Rule>();
        if rule.is_null() {
            return rule;
        }
        // The hud's cache is of a rule by its address; a new match's rule may
        // be allocated where the last one was: forget the last one's.
        (*&raw mut STATE).hud.rule = None;
        engine_match_rule_base(rule.cast(), RULE_TYPE);
        let base = rule.cast::<u8>();
        core::ptr::write_bytes(base.add(0x110), 0, size_of::<Rule>() - 0x110);
        // Zero is every field's first value from here: the rule is whole.
        let rule = &mut *rule;
        put(base, 0, rn_rule_vtable.0.as_ptr().add(1));
        put(base, 0x108, rn_resolver_vtable.0.as_ptr().add(1));
        rule.game = get(cook, 0x20);
        let root: *const Cfg = get(query, 0xC);
        rule.config = if read_params(&mut rule.params, root) { Config::Read } else { Config::Unread };
        rr::reset(&mut rule.state, &rule.params);
        rule
    }
}

/// The cook's own RTTI, as MSVC lays it out (x86): research_rule_cook, a
/// translate_query_cook; its complete object locator before the vtable. No
/// code of the game casts a cook or asks its type (no reference to any
/// cook's type descriptor in .text), but the locator names the cook for what
/// it is, not tdm's. The type descriptor's second word is type_info's cache
/// of the undecorated name, written by type_info::name: the section is
/// writable, and nothing here reads it.
#[repr(C)]
pub struct TypeDescriptor {
    vftable: u32,
    spare: u32,
    name: [u8; 29],
}

#[repr(C)]
pub struct BaseClass {
    kind: *const TypeDescriptor,
    contained: u32,
    mdisp: u32,
    pdisp: u32,
    vdisp: u32,
    attributes: u32,
    hierarchy: *const Hierarchy,
}

#[repr(C)]
pub struct Hierarchy {
    signature: u32,
    attributes: u32,
    count: u32,
    bases: *const Table<4>,
}

#[repr(C)]
pub struct Locator {
    signature: u32,
    offset: u32,
    cd_offset: u32,
    kind: *const TypeDescriptor,
    hierarchy: *const Hierarchy,
}

/// Its pointers are to constants.
#[repr(transparent)]
pub struct Rtti<T>(T);
// SAFETY: constants.
unsafe impl<T> Sync for Rtti<T> {}

static COOK_TYPE: TypeDescriptor =
    TypeDescriptor { vftable: TYPE_INFO_VFTABLE, spare: 0, name: *b".?AVresearch_rule_cook@srp@@\0" };
static COOK_BASE: Rtti<BaseClass> = Rtti(BaseClass {
    kind: &COOK_TYPE,
    contained: 3,
    mdisp: 0,
    pdisp: 0xFFFFFFFF,
    vdisp: 0,
    attributes: 0x40,
    hierarchy: &COOK_HIERARCHY.0,
});
static COOK_BASES: Table<4> = Table([
    Word { data: (&raw const COOK_BASE).cast() },
    game(BCD_TRANSLATE_QUERY_COOK),
    game(BCD_COOK_BASE),
    game(BCD_NONCOPYABLE),
]);
static COOK_HIERARCHY: Rtti<Hierarchy> = Rtti(Hierarchy { signature: 0, attributes: 1, count: 4, bases: &COOK_BASES });
static COOK_LOCATOR: Rtti<Locator> =
    Rtti(Locator { signature: 0, offset: 0, cd_offset: 0, kind: &COOK_TYPE, hierarchy: &COOK_HIERARCHY.0 });

/// The cook's vtable: tdm_rule_core_cook's, with our create and size.
#[unsafe(no_mangle)]
pub static rn_cook_vtable: Table<{ 1 + 11 }> = Table([
    Word { data: (&raw const COOK_LOCATOR).cast() },
    game(0x36760),
    game(0xFDA90),
    game(0xF7760),
    game(0x372420),
    game(0x372420),
    game(0x36780),
    game(0x1FA420),
    game(0x2F4290),
    game(0x75AF0),
    ours(rn_cook_create),
    ours(rn_cook_size),
]);

/// From the game's constructor, after its own client cooks (research_026g.asm
/// `register`): the cook made as tdm_rule_core_cook's (0x2F41E0), with the
/// client's game at +20h, registered unless its class is taken.
///
/// # Safety
/// The game's constructor, its game.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn research_register_cook(game: *mut c_void) {
    unsafe {
        let globals = &mut *&raw mut STATE;
        if globals.registration != Registration::Pending {
            return;
        }
        let cooks: *const u32 = get(COOKS as *const u8, 0);
        let end: *const u32 = get(COOKS as *const u8, 4);
        if cooks != end && ((end.offset_from(cooks) as u32) <= CLASS || *cooks.add(CLASS as usize) != 0) {
            globals.registration = Registration::ClassTaken;
            return;
        }
        let cook = globals.cook.as_mut_ptr();
        globals.cook = [0; COOK_SIZE];
        put(cook, 0, rn_cook_vtable.0.as_ptr().add(1));
        put(cook, 8, CLASS);
        put(cook, 0x10, 0xFFFFFFFFu32);
        let thread_id: unsafe extern "stdcall" fn() -> u32 = core::mem::transmute(get::<usize>(CURRENT_THREAD_ID as *const u8, 0));
        put(cook, 0x14, thread_id());
        put(cook, 0x18, 8u32);
        put(cook, 0x20, game);
        engine_register_cook(cook.cast());
        globals.registration = Registration::Done;
    }
}

// ---- the hud ----------------------------------------------------------------

unsafe fn find_rule(ui: *const u8) -> Option<&'static mut Rule> {
    unsafe {
        let game: *const u8 = get(ui, UI_GAME);
        if game.is_null() {
            return None;
        }
        let game_match: *mut u8 = get(game, GAME_MATCH);
        if game_match.is_null() {
            return None;
        }
        let core = game_match.add(MATCH_WORLD);
        let mut rule: *const *mut Rule = get(core, CORE_RULES);
        let end: *const *mut Rule = get(core, CORE_RULES + 4);
        let ours = rn_rule_vtable.0.as_ptr().add(1);
        while !rule.is_null() && rule < end {
            let at = *rule;
            if !at.is_null() && get::<*const Word>(at.cast(), 0) == ours {
                return Some(&mut *at);
            }
            rule = rule.add(1);
        }
        None
    }
}

unsafe fn message(ui: *mut u8, key: &CStr) {
    unsafe { engine_show_message(key.as_ptr().cast(), ui.cast(), 0x21, 0x14, 6000) };
}

const CAPTURED: [&CStr; MAX_ZONES] =
    [c"st_research_unit_a_captured", c"st_research_unit_b_captured", c"st_research_unit_c_captured"];
const LOST: [&CStr; MAX_ZONES] = [c"st_research_unit_a_lost", c"st_research_unit_b_lost", c"st_research_unit_c_lost"];

/// A Scaleform GFx::Value.
#[repr(C)]
struct GfxValue {
    object: *mut c_void,
    kind: u32,
    value: f64,
    extra: [u32; 2],
}
const _: () = assert!(size_of::<GfxValue>() == 0x18);
const GFX_NUMBER: u32 = 5;

/// The arguments of root.research_update.
struct Values {
    list: [MaybeUninit<GfxValue>; 5 + 9 * MAX_ZONES],
    len: usize,
}

impl Values {
    #[inline(never)]
    fn put(&mut self, value: f64) {
        if let Some(slot) = self.list.get_mut(self.len) {
            slot.write(GfxValue { object: null_mut(), kind: GFX_NUMBER, value, extra: [0; 2] });
            self.len += 1;
        }
    }
}

/// Every frame of the match's hud (game_world_ui::update_ui, after the teams'
/// points; research_026g.asm `hud`): root.research_update(local team, points
/// 0, 1, the goal, units, then per unit cp_id, x, -z, owner, capturing,
/// progress 0..100, inside 0, inside 1, whether the game's own player is
/// inside), and a message when a unit changes hands or the match ends.
///
/// # Safety
/// update_ui's game_world_ui.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn research_update_hud(ui: *mut u8) {
    unsafe {
        if get::<u32>(ui, UI_MODE) != MODE {
            return;
        }
        let hud = &mut (*&raw mut STATE).hud;
        let found = find_rule(ui);
        let Some((rule, world)) = found.and_then(|rule| rule.world().map(|world| (rule, world))) else {
            hud.rule = None;
            return;
        };
        let state = &rule.state;
        let place = u32::from(get::<u8>(world.0, CORE_LOCAL_PLACE));
        // the team as the game holds it (the checksum takes it as it is)
        let local_raw =
            if place < PLAYERS as u32 { team_of(world.player(place as usize)) } else { u32::from(rr::NOBODY) };
        let local = Team::from_raw(local_raw);
        let zones = state.zone_count as usize;
        let this = NonNull::from(&*rule);
        if hud.rule != Some(this) {
            hud.rule = Some(this);
            for (z, owner) in hud.owner.iter_mut().enumerate() {
                *owner = if z < zones { state.zones[z].owner } else { None };
            }
            hud.completed = state.end.get().is_some();
            hud.sum = 0;
            hud.shown = 0;
        }
        if let Some(local) = local {
            for (z, zone) in state.zones.iter().enumerate().take(zones) {
                let owner = zone.owner;
                if owner == hud.owner[z] {
                    continue;
                }
                if owner == Some(local) {
                    message(ui, CAPTURED[z]);
                } else if hud.owner[z] == Some(local) {
                    message(ui, LOST[z]);
                }
                hud.owner[z] = owner;
            }
            let completed = state.end.get().is_some();
            if completed && !hud.completed {
                message(ui, match rr::leader(state) {
                    None => c"st_finish_match_mode_05_draw_message",
                    Some(leader) if leader == local => c"st_finish_match_mode_05_winner_message",
                    Some(_) => c"st_finish_match_mode_05_loser_message",
                });
            }
            hud.completed = completed;
        }
        let mut values = Values { list: [const { MaybeUninit::uninit() }; 5 + 9 * MAX_ZONES], len: 0 };
        let mut sum: u32 = 0;
        values.put(f64::from(local.map_or(0, |t| t as u32)));
        values.put(f64::from(state.score[0]));
        values.put(f64::from(state.score[1]));
        values.put(f64::from(rule.params.target / MICRO));
        values.put(f64::from(state.zone_count));
        for (zone, def) in state.zones.iter().zip(&rule.params.zones).take(zones) {
            let progress = zone.progress / (rr::FULL / 1000);
            let owner = u32::from(rr::team_byte(zone.owner));
            let capturing = u32::from(rr::team_byte(zone.capturing));
            values.put(f64::from(def.cp_id));
            values.put(f64::from(def.position[0]));
            values.put(f64::from(-def.position[2]));
            values.put(f64::from(owner));
            values.put(f64::from(capturing));
            values.put(f64::from(progress) / 10.0);
            values.put(f64::from(zone.count[0]));
            values.put(f64::from(zone.count[1]));
            let inside = local.is_some_and(|t| place < PLAYERS as u32 && zone.present[t.index()].has(place as usize));
            let inside = u32::from(inside);
            values.put(f64::from(inside));
            sum = sum
                .wrapping_mul(31)
                .wrapping_add(progress.wrapping_mul(7))
                .wrapping_add(owner * 3)
                .wrapping_add(capturing)
                .wrapping_add(u32::from(zone.count[0]) * 11)
                .wrapping_add(u32::from(zone.count[1]) * 13)
                .wrapping_add(inside * 17);
        }
        sum = sum
            .wrapping_mul(31)
            .wrapping_add(u32::from(state.score[0]) * 1009)
            .wrapping_add(u32::from(state.score[1]) * 17)
            .wrapping_add(local_raw);
        // the movie hears of a change at once, and every second (60 frames) anyway
        if sum == hud.sum {
            hud.shown = hud.shown.wrapping_add(1);
            if hud.shown < 60 {
                return;
            }
        }
        hud.sum = sum;
        hud.shown = 0;
        let movie: *mut u8 = get(get::<*const u8>(get::<*const u8>(ui, UI_HUD), 0x108), 4);
        engine_movie_invoke(movie.cast(), c"root.research_update".as_ptr().cast(), null_mut(),
                            values.list.as_ptr().cast(), values.len as u32);
    }
}
