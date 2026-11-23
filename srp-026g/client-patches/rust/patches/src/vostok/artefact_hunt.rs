//! Patch 171, Artifact Hunt (docs/notes/artefact_hunt.md, "The exe"): the
//! game's side of `artefact_hunt_rule`. What the game objects are and what
//! is safe to do with them: the rule's cook (a static of ours registered as
//! the game's are), the rule it makes (game_match_rule_base and our vtable
//! copy of tdm_rule_core's), its tick in the world's second list, the world
//! it reads (its places, the statistics, the blackboard), the respawn
//! rule's two sites, the world's writer and reader, the hud state.
//!
//! The rule's state is the patch's (by the rule object's address, the
//! [`RuleKey`]); nothing here keeps any.

use core::cell::UnsafeCell;
use core::ffi::{CStr, c_void};
use core::marker::PhantomData;

use super::config::{ConfigTable, ConfigValue};
use super::context::{Context, Reg};
use super::hook::{CallbackEntry, Handle, Thread};
use super::project::NativeVtable;
use super::statistics::{Score, ScoreEvent};
use super::world::{Player, WorldKey, floats, read};
use crate::raw;

/// The resource class the cook registers (1E1h: no registration and no query
/// of the game's names it; Research's is 1E0h).
pub const CLASS: usize = 481;
/// game_match_rule_base's type of our rule (the game's code compares it with
/// 0 and 1 only; 2 to 5 are the time rule's, kd's, tdm's and Research's).
pub const RULE_TYPE: u32 = 6;
/// The respawn rule's "no respawn" (the death handler never writes it, the
/// hud shows none, the world does not save it).
const NO_RESPAWN: u32 = u32::MAX;
/// The roster places a world keeps.
const PLACES: u8 = 20;
/// The level collision filter of patch 78's rays (the foot IK's): no
/// player's hit bodies.
const LEVEL_GROUP: u32 = 0x24;
const LEVEL_MASK: u32 = 2;

/// Whether the cook is the game's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Registration {
    Pending,
    Registered,
    /// Another cook has the class: no Artifact Hunt (a match of mode 13
    /// fails to load its rule).
    ClassTaken,
}

/// The cook: a static object the game keeps a pointer to for good.
pub struct Cook {
    storage: UnsafeCell<raw::HuntCookStorage>,
    registration: UnsafeCell<Registration>,
}

// SAFETY: registered and read on the game's thread only (Thread is not Send).
unsafe impl Sync for Cook {}

impl Cook {
    pub const fn new() -> Cook {
        Cook { storage: UnsafeCell::new(raw::HuntCookStorage::zeroed()), registration: UnsafeCell::new(Registration::Pending) }
    }

    /// At game::query_base_resources' start (the game's constructor, after
    /// its own client cooks): the cook made as the game's rule cooks are
    /// (tdm_rule_cook 0x2F41E0: {vtable, class, -1, thread, 8, the game}) and
    /// registered, unless its class is another's.
    pub fn register(&'static self, ctx: &Context<'_>, methods: NativeVtable<raw::TdmCookMethods>) -> Registration {
        // SAFETY: on the game's thread, in the constructor; the object is this
        // static's, never moved or freed; register_cook's table is the
        // image's (begin, end), grown to 206h places by its first call.
        unsafe {
            if *self.registration.get() != Registration::Pending {
                return *self.registration.get();
            }
            let Some(game) = raw::site::game_query_base_resources::game(ctx) else { return Registration::Pending };
            let table = raw::image::cooks();
            if let (Some(first), Some(end)) = (table.first(), table.end()) {
                let count = end.index_from(first).unwrap_or(0);
                if count <= CLASS || read(first.at(CLASS).cook()) != 0 {
                    *self.registration.get() = Registration::ClassTaken;
                    return Registration::ClassTaken;
                }
            }
            let cook = (*self.storage.get()).handle();
            *cook.vtable() = methods.address();
            *cook.class() = CLASS as u32;
            *cook.unused() = u32::MAX;
            *cook.thread() = (raw::engine_get_current_thread_id)();
            *cook.flags() = 8;
            cook.set_game(Some(game));
            raw::engine_register_hunt_cook(Some(cook));
            *self.registration.get() = Registration::Registered;
            Registration::Registered
        }
    }
}

impl Default for Cook {
    fn default() -> Self {
        Self::new()
    }
}

/// The cook the translate calls (ours: our vtable's slots are only in it).
pub struct HuntCook<'t> {
    native: raw::HuntCook,
    thread: &'t Thread,
}

impl<'t> Handle<'t> for HuntCook<'t> {
    type Raw = raw::HuntCook;
    unsafe fn from_raw(thread: &'t Thread, native: Option<raw::HuntCook>) -> Option<Self> {
        Some(HuntCook { native: native?, thread })
    }
}

/// A rule cook's query: the match's options, the mode config.
pub struct RuleQuery<'t> {
    native: Option<raw::RuleQuery>,
    _thread: PhantomData<&'t Thread>,
}

impl<'t> RuleQuery<'t> {
    /// # Safety
    /// The translate's query, alive while the cook's create runs.
    pub(in crate::vostok) unsafe fn from_raw(_: &'t Thread, native: Option<raw::RuleQuery>) -> RuleQuery<'t> {
        RuleQuery { native, _thread: PhantomData }
    }
    pub(in crate::vostok) fn raw(&self) -> Option<raw::RuleQuery> {
        self.native
    }
    /// The mode config's root table.
    pub fn config(&self) -> Option<ConfigTable<'t>> {
        // SAFETY: the query's config, alive while the cook runs.
        unsafe { ConfigValue::read(self.native?.config()?).ok()?.table().ok() }
    }
    /// The match's seed (match_options +577B0h, the static info's "match
    /// id": the seed of its anomalies, victory items and player objects).
    pub fn seed(&self) -> Option<u32> {
        // SAFETY: the query's match options, alive while the cook runs.
        unsafe { Some(read(self.native?.options()?.seed())) }
    }
}

/// A rule of ours, the object the cook made.
pub struct HuntRule<'t> {
    native: raw::HuntRule,
    thread: &'t Thread,
}

impl<'t> Handle<'t> for HuntRule<'t> {
    type Raw = raw::HuntRule;
    unsafe fn from_raw(thread: &'t Thread, native: Option<raw::HuntRule>) -> Option<Self> {
        Some(HuntRule { native: native?, thread })
    }
}

/// A rule object kept between hooks: whose state a slot is called for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleKey(u32);

impl HuntCook<'_> {
    pub(in crate::vostok) fn raw(&self) -> raw::HuntCook {
        self.native
    }

    /// tdm's create (0x2063F0) for our rule: the game's allocator under
    /// tdm_rule's name, game_match_rule_base of type [`RULE_TYPE`], our
    /// vtable, tdm's link resolver, the cook's game; none without memory.
    pub fn make_rule(&self, methods: NativeVtable<raw::TdmRuleMethods>) -> Option<RuleKey> {
        let _ = self.thread;
        let allocator = super::gun_cook::allocator()?;
        // SAFETY: on the loading thread (the translate's); the game's
        // allocator and an allocation name of the image; the room is the
        // rule's size, its base constructed before any field is written.
        unsafe {
            let name = raw::engine_allocation_name(raw::image::TDM_RULE_NAME as usize as *const c_void);
            let room = raw::engine_malloc(allocator, raw::HuntRule::SIZE as u32, name);
            if room.is_null() {
                return None;
            }
            let rule = raw::engine_hunt_rule_base(room, RULE_TYPE)?;
            *rule.vtable() = methods.address();
            *rule.resolver() = raw::image::TDM_RULE_RESOLVER_METHODS;
            rule.set_blackboard(None);
            let tick = rule.tick();
            *tick.object() = 0;
            *tick.function() = 0;
            tick.set_previous(None);
            tick.set_next(None);
            *tick.order() = 0;
            *tick.enabled() = 0;
            rule.set_core(None);
            rule.set_game(self.native.game());
            Some(RuleKey(rule.address()))
        }
    }
}

impl<'t> HuntRule<'t> {
    pub(in crate::vostok) fn raw(&self) -> raw::HuntRule {
        self.native
    }

    pub fn key(&self) -> RuleKey {
        RuleKey(self.native.address())
    }

    pub fn thread(&self) -> &'t Thread {
        self.thread
    }

    /// Slot 11: the core kept, and the tick a delegate at the end of the
    /// world's second list, keyed by the core's counter (the time rule's
    /// set_game_world_core 0x318050).
    pub fn set_core(&self, core: &RuleCore<'_>, tick: CallbackEntry<super::OrigTimelimitTick>) {
        let Some(core) = core.native else { return };
        // SAFETY: the game gives the rule its core once, on the loading
        // thread; the delegate is the rule's own (+120h), the list and the
        // counter the core's; nothing else touches them meanwhile.
        unsafe {
            self.native.set_core(Some(core));
            let delegate = self.native.tick();
            *delegate.object() = self.native.address();
            *delegate.function() = tick.entry() as usize as u32;
            if let Some(counter) = core.tick_counter() {
                *delegate.order() = read(counter.value());
                *counter.value() = read(counter.value()).wrapping_add(1);
            }
            *delegate.enabled() = 1;
            let list = core.logic_ticks();
            *list.count() = read(list.count()).wrapping_add(1);
            delegate.set_previous(list.tail());
            delegate.set_next(None);
            match list.tail() {
                Some(tail) if list.head().is_some() => tail.set_next(Some(delegate)),
                _ => list.set_head(Some(delegate)),
            }
            list.set_tail(Some(delegate));
        }
    }

    /// The world the rule plays in, once it has one.
    pub fn world(&self) -> Option<HuntWorld<'t>> {
        // SAFETY: the rule's core, alive as long as the rule.
        let core = unsafe { self.native.core() }?;
        Some(HuntWorld { core, thread: self.thread })
    }
}

/// What a rule event carries: a kill's places (none for the match's start).
pub struct RuleEvent<'t> {
    native: Option<raw::KillEvent>,
    _thread: PhantomData<&'t Thread>,
}

impl<'t> RuleEvent<'t> {
    /// # Safety
    /// The dispatcher's data, alive while the slot runs (0 for event 0).
    pub(in crate::vostok) unsafe fn from_raw(_: &'t Thread, native: Option<raw::KillEvent>) -> RuleEvent<'t> {
        RuleEvent { native, _thread: PhantomData }
    }
    pub(in crate::vostok) fn raw(&self) -> Option<raw::KillEvent> {
        self.native
    }
    /// A kill's (killer, victim) places as the game gives them.
    pub fn kill(&self) -> Option<(u8, u8)> {
        // SAFETY: a kill's two bytes (hit_impl 0x2D9EF1..0x2D9EF4).
        unsafe {
            let data = self.native?;
            Some((read(data.killer()), read(data.victim())))
        }
    }
}

/// The core slot 11 gets.
pub struct RuleCore<'t> {
    native: Option<raw::CoreWorld>,
    _thread: PhantomData<&'t Thread>,
}

impl<'t> RuleCore<'t> {
    /// # Safety
    /// The game's core, alive as long as its rules.
    pub(in crate::vostok) unsafe fn from_raw(_: &'t Thread, native: Option<raw::CoreWorld>) -> RuleCore<'t> {
        RuleCore { native, _thread: PhantomData }
    }
    pub(in crate::vostok) fn raw(&self) -> Option<raw::CoreWorld> {
        self.native
    }
}

/// The world's writer at slot 15: the state field by field.
pub struct RuleWriter<'t> {
    native: Option<raw::BufferWriter>,
    _thread: PhantomData<&'t Thread>,
}

/// The dump's file and function for our fields.
const FILE: &CStr = c"artefact_hunt.rs";
const SERIALIZE: &CStr = c"srp::artefact_hunt_rule::serialize";

impl<'t> RuleWriter<'t> {
    /// # Safety
    /// The snapshot's writer, alive while the slot runs.
    pub(in crate::vostok) unsafe fn from_raw(_: &'t Thread, native: Option<raw::BufferWriter>) -> RuleWriter<'t> {
        RuleWriter { native, _thread: PhantomData }
    }
    pub(in crate::vostok) fn raw(&self) -> Option<raw::BufferWriter> {
        self.native
    }
    /// One field through the game's writer of its size (as tdm's serialize
    /// 0x316340 writes its own): its bytes, and its name in the hash mismatch
    /// dump; `line` stands for its place in the state.
    pub fn field(&self, field: artefact_hunt_rule::Field, line: u32) {
        let Some(writer) = self.native else { return };
        // SAFETY: the snapshot's live writer; the value is a local the writer
        // copies before it returns; the names are terminated statics.
        unsafe {
            match field.value {
                artefact_hunt_rule::Value::U8(value) => raw::engine_write_u8((&raw const value).cast(), Some(writer),
                    FILE.as_ptr().cast(), line, SERIALIZE.as_ptr().cast(), field.name.as_ptr().cast()),
                artefact_hunt_rule::Value::U32(value) => raw::engine_write_u32((&raw const value).cast(), Some(writer),
                    FILE.as_ptr().cast(), line, SERIALIZE.as_ptr().cast(), field.name.as_ptr().cast()),
            }
        }
    }
}

/// The world's reader at slot 16.
pub struct RuleReader<'t> {
    native: Option<raw::BufferReader>,
    _thread: PhantomData<&'t Thread>,
}

impl<'t> RuleReader<'t> {
    /// # Safety
    /// The snapshot's reader, alive while the slot runs.
    pub(in crate::vostok) unsafe fn from_raw(_: &'t Thread, native: Option<raw::BufferReader>) -> RuleReader<'t> {
        RuleReader { native, _thread: PhantomData }
    }
    pub(in crate::vostok) fn raw(&self) -> Option<raw::BufferReader> {
        self.native
    }
    /// Gives `read` the bytes left in the reader's stream and moves the
    /// cursor past as many as it says it read (none: the cursor stays).
    pub fn take<R>(&self, read_with: impl FnOnce(&[u8]) -> Option<(R, usize)>) -> Option<R> {
        let reader = self.native?;
        // SAFETY: a live reader {begin, cursor, size}; its bytes from begin
        // are the snapshot's, alive while the slot runs; the cursor moves
        // only inside them.
        unsafe {
            let (begin, cursor, size) = (reader.begin() as usize, reader.cursor() as usize, read(reader.size()) as usize);
            let end = begin.checked_add(size)?;
            if cursor < begin || cursor > end {
                return None;
            }
            let rest = core::slice::from_raw_parts(cursor as *const u8, end - cursor);
            let (value, used) = read_with(rest)?;
            if used > rest.len() {
                return None;
            }
            reader.set_cursor((cursor + used) as *mut u8);
            Some(value)
        }
    }
}

/// The match's hud state slot 17 fills.
pub struct HudState<'t> {
    native: Option<raw::MatchHudState>,
    _thread: PhantomData<&'t Thread>,
}

impl<'t> HudState<'t> {
    /// # Safety
    /// fill_match_hud_state's hud state, alive while the slot runs.
    pub(in crate::vostok) unsafe fn from_raw(_: &'t Thread, native: Option<raw::MatchHudState>) -> HudState<'t> {
        HudState { native, _thread: PhantomData }
    }
    pub(in crate::vostok) fn raw(&self) -> Option<raw::MatchHudState> {
        self.native
    }
    /// The teams' points (+90h, +92h, tdm's) and the clock's whole seconds
    /// (+4, the time rule's, filled before ours).
    pub fn fill(&self, points: [u8; 2], seconds_left: u32) {
        let Some(hud) = self.native else { return };
        // SAFETY: the live hud state of the fill, on its thread.
        unsafe {
            *hud.team_zero_points() = u16::from(points[0]);
            *hud.team_one_points() = u16::from(points[1]);
            *hud.seconds_left() = seconds_left;
        }
    }
}

/// A world the rule plays in (its core), for a hook.
#[derive(Clone, Copy)]
pub struct HuntWorld<'t> {
    core: raw::CoreWorld,
    thread: &'t Thread,
}

/// A player of the world as the rule sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub team: u32,
    pub alive: bool,
    pub inserted: bool,
    pub feet: [f32; 3],
}

impl<'t> HuntWorld<'t> {
    pub fn thread(&self) -> &'t Thread {
        self.thread
    }

    pub fn key(&self) -> WorldKey {
        WorldKey::of(self.core)
    }

    /// The world of the match being played (the engine world's game's
    /// match: a live or a replay match alike).
    pub fn of_match(thread: &'t Thread) -> Option<HuntWorld<'t>> {
        Some(HuntWorld { core: super::artefact_skills::match_core()?, thread })
    }

    /// The world a player of a hook is in.
    pub fn of_player(player: &Player<'t>) -> Option<HuntWorld<'t>> {
        // SAFETY: the live player's core.
        let core = unsafe { player.this.world_core() }?;
        Some(HuntWorld { core, thread: player.thread })
    }

    /// The round's countdown on or off ([`super::round_countdown`]): in
    /// every game at the same tick and after every load (the rule's tick and
    /// deserialize, after the time rule's own).
    pub fn set_round_countdown(&self, on: bool) {
        // SAFETY: the live core of this world, on the simulation's thread.
        unsafe { super::round_countdown::set(self.core, on) }
    }

    /// Whether patch 31's kit change switches `place` at the next step, by
    /// its own test (kit_change_026g.asm `try_place`, less the clean record
    /// a round countdown does without): another next kit (under 3, with a
    /// player), the player in use alive and in the world.
    pub fn kit_switch_due(&self, place: u8) -> bool {
        let Some(player) = self.player(place) else { return false };
        // SAFETY: as player: the place's entry, bounded; the player's flags.
        unsafe {
            let Some(first) = self.core.places() else { return false };
            let entry = first.at(usize::from(place));
            let (kit, next) = (read(entry.kit()), read(entry.next_kit()));
            next != kit && next < 3 && entry.kits().at(usize::from(next)).player().is_some()
                && read(player.this.alive()) != 0 && read(player.this.inserted()) != 0
        }
    }

    /// The player of `place` in the match (in the world's active places, the
    /// kit it plays), as Research's tick takes them.
    pub fn player(&self, place: u8) -> Option<Player<'t>> {
        if place >= PLACES {
            return None;
        }
        // SAFETY: the core's places vector, bounded by its end; the active
        // bit of the place; the place's kit and that kit's player.
        unsafe {
            if read(self.core.active_places()) & (1 << place) == 0 {
                return None;
            }
            let (first, end) = (self.core.places()?, self.core.places_end()?);
            if usize::from(place) >= end.index_from(first)? {
                return None;
            }
            let entry = first.at(usize::from(place));
            let kit = read(entry.kit());
            if kit >= 3 {
                return None;
            }
            let this = entry.kits().at(usize::from(kit)).player()?;
            Some(Player { this, thread: self.thread })
        }
    }

    /// What the rule reads of a place's player.
    pub fn body(&self, place: u8) -> Option<Body> {
        let player = self.player(place)?;
        // SAFETY: the live player's profile, flags and feet.
        unsafe {
            Some(Body {
                team: read(player.this.profile()?.team()),
                alive: read(player.this.alive()) != 0,
                inserted: read(player.this.inserted()) != 0,
                feet: floats(player.this.feet()),
            })
        }
    }

    /// The ground under `place`'s feet: a ray straight down from 0.23 m over
    /// them, 200 m long, against the level (patch 78's drop); none without a
    /// physics world or ground.
    pub fn ground_under(&self, place: u8) -> Option<[f32; 3]> {
        let player = self.player(place)?;
        // SAFETY: the live player's feet.
        let feet: [f32; 3] = unsafe { floats(player.this.feet()) };
        let hit = player.physics_world()?.ray_test([feet[0], feet[1] + 0.23, feet[2]], [0.0, -1.0, 0.0], 200.0,
                                                    LEVEL_GROUP, LEVEL_MASK)?;
        Some(hit.point)
    }

    /// The time rule's start of play (its +12Ch), the rules vector's entry of
    /// type 2.
    pub fn play_start(&self) -> Option<u32> {
        // SAFETY: the live core's rules vector; each rule's base type read
        // before its layout is taken for the time rule's.
        unsafe {
            let first = self.core.rules_begin()?;
            let count = self.core.rules_end()?.index_from(first)?;
            for index in 0..count.min(64) {
                let rule = first.at(index).rule()?;
                if read(rule.rule_type()) == 2 {
                    return Some(read(rule.time_start()));
                }
            }
            None
        }
    }

    /// Counts a score event for `place`'s cell of the kit it plays, and tells
    /// the scoreboard (vostok::statistics' Score: as the game counts one).
    pub fn count(&self, place: u8, event: ScoreEvent, amount: u16) {
        let Some(player) = self.player(place) else { return };
        // SAFETY: the live core's statistics; the live player.
        unsafe {
            let Some(statistics) = self.core.statistics() else { return };
            if let Some(score) = Score::of(statistics.shared(), player.this) {
                score.count(event, amount);
            }
        }
    }

    /// The statistics' tail: the teams' points and the leader (0, 1, 2 a
    /// draw), as tdm's rule writes them (0x316257).
    pub fn publish(&self, points: [u8; 2], leader: u8) {
        // SAFETY: the live core's statistics handler.
        unsafe {
            let Some(statistics) = self.core.statistics() else { return };
            *statistics.team_zero_points() = u16::from(points[0]);
            *statistics.team_one_points() = u16::from(points[1]);
            *statistics.winner() = leader;
        }
    }

    /// The blackboard's two "may respawn" flags, both `respawns` (the death
    /// handler 0x316AFA gives no time while false), through the game's
    /// set<bool> (0x316630) when one differs.
    pub fn set_respawns(&self, respawns: bool) {
        // SAFETY: the live core's blackboard; the value a local the setter
        // copies.
        unsafe {
            let board = self.core.blackboard();
            let value = u8::from(respawns);
            for (index, now) in [(0, read(board.team_1_respawn())), (1, read(board.team_2_respawn()))] {
                if (now != 0) != respawns {
                    raw::engine_blackboard_set_bool(index, Some(board), &raw const value);
                }
            }
        }
    }

    /// The modifiers container of `place`'s current kit (what apply_modifier
    /// gets for it).
    pub fn modifiers(&self, place: u8) -> Option<ModifiersKey> {
        let player = self.player(place)?;
        super::medical::kit_modifiers(player.this).map(|modifiers| ModifiersKey(modifiers.address()))
    }
}

/// A kit's modifiers container, compared between hooks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModifiersKey(u32);

/// apply_modifier's own float of a list (patch 109's sites): the container,
/// the index, and XMM0 to add to.
pub struct ModifierTerm<'c, 'm> {
    ctx: &'c mut Context<'m>,
    modifiers: ModifiersKey,
    index: u32,
}

impl<'c, 'm> ModifierTerm<'c, 'm> {
    /// At `artefact.shared_modifier` (EAX the container, ESI the index).
    pub fn at_shared_modifier(ctx: &'c mut Context<'m>) -> Option<Self> {
        use raw::site::artefact_shared_modifier as site;
        // SAFETY: the getter's own registers.
        let (modifiers, index) = unsafe { (site::modifiers(ctx)?, site::index(ctx)) };
        Some(Self { modifiers: ModifiersKey(modifiers.address()), index, ctx })
    }
    /// At `artefact.shared_modifiers` (EBX the container, EAX the index).
    pub fn at_shared_modifiers(ctx: &'c mut Context<'m>) -> Option<Self> {
        use raw::site::artefact_shared_modifiers as site;
        // SAFETY: the getter's own registers.
        let (modifiers, index) = unsafe { (site::modifiers(ctx)?, site::index(ctx)) };
        Some(Self { modifiers: ModifiersKey(modifiers.address()), index, ctx })
    }
    pub fn modifiers(&self) -> ModifiersKey {
        self.modifiers
    }
    pub fn index(&self) -> u32 {
        self.index
    }
    pub fn thread(&self) -> &Thread {
        self.ctx.thread()
    }
    /// Adds `term` to the list's sum.
    pub fn add(&mut self, term: f32) {
        self.ctx.set_xmm_f32(0, self.ctx.xmm_f32(0) + term);
    }
}

/// The respawn rule's death handler after a death's delay (0x316B6B).
pub struct DeathDelay<'c, 'm> {
    ctx: &'c mut Context<'m>,
    rule: raw::PlayerRespawnRuleBase,
    time: u32,
}

impl<'c, 'm> DeathDelay<'c, 'm> {
    pub fn at(ctx: &'c mut Context<'m>) -> Option<Self> {
        use raw::site::respawn_death_delay as site;
        // SAFETY: the handler's EDI and its frame at the site.
        let (rule, time) = unsafe { (site::rule(ctx)?, read(site::frame(ctx)?.time())) };
        Some(Self { rule, time, ctx })
    }
    pub fn world(&self) -> Option<HuntWorld<'_>> {
        // SAFETY: the respawn rule's core, alive as long as the rule.
        Some(HuntWorld { core: unsafe { self.rule.core() }?, thread: self.ctx.thread() })
    }
    pub fn thread(&self) -> &Thread {
        self.ctx.thread()
    }
    /// The death's time (the event's: the step before's).
    pub fn time(&self) -> u32 {
        self.time
    }
    /// The delay the native rule computed: its respawn_time with the
    /// victim's modifier 60, rounded.
    pub fn delay(&self) -> u32 {
        self.ctx.reg(Reg::Eax)
    }
    /// The mode's respawn_time, ms (rule+114h).
    pub fn respawn_time(&self) -> u32 {
        // SAFETY: the live respawn rule's constant.
        unsafe { read(self.rule.respawn_time()) }
    }
    /// The delay the native grid and store then take.
    pub fn set_delay(&mut self, delay: u32) {
        self.ctx.set_reg(Reg::Eax, delay);
    }
}

/// The respawn rule's tick after its prologue (0x316826), before it reads
/// any place's time.
pub struct RespawnTick<'t> {
    rule: raw::PlayerRespawnRuleBase,
    time: u32,
    thread: &'t Thread,
}

impl<'t> RespawnTick<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<Self> {
        use raw::site::respawn_tick as site;
        // SAFETY: the tick's ECX and EBP frame at the site.
        let (rule, time) = unsafe { (site::rule(ctx)?, read(site::frame(ctx)?.time())) };
        Some(Self { rule, time, thread: ctx.thread() })
    }
    pub fn world(&self) -> Option<HuntWorld<'t>> {
        // SAFETY: the respawn rule's core.
        Some(HuntWorld { core: unsafe { self.rule.core() }?, thread: self.thread })
    }
    pub fn time(&self) -> u32 {
        self.time
    }
    /// Patch 31's switch for a round's countdown: `place` played in its chosen
    /// kit where it stands, as `kit_change_026g.asm`'s `try_place` does before
    /// the start (whose state test leaves the round's 4 to this tick, at the
    /// same place of the step: the respawn rule's tick after its prologue,
    /// before any place's time is read), less the clean record, which only
    /// the first start asks for: the player out of the world (player::remove
    /// 0x1F2870, the world player's vtable +24h, with 1), then the spawn
    /// (0x316DA0, the respawn rules' vtable +5Ch) with a point of its feet,
    /// its yaw and what is left of its protection (none left: -1 less now,
    /// which the spawn adds to now). Whether it switched.
    pub fn switch_kit(&self, place: u8) -> bool {
        let Some(world) = self.world() else { return false };
        if !world.kit_switch_due(place) {
            return false;
        }
        let Some(player) = world.player(place) else { return false };
        // SAFETY: the live player of the place, its feet, yaw and damage
        // model; the rule and the time the tick's; the point ours, read by
        // the spawn before it returns.
        unsafe {
            let until = match player.this.damage_model() {
                Some(model) => read(model.invulnerable_until()),
                None => u32::MAX,
            };
            let protection = match until.checked_sub(self.time) {
                Some(left) if until != u32::MAX && left > 0 => left,
                _ => !self.time,
            };
            let feet: [f32; 3] = floats(player.this.feet());
            let yaw: f32 = read(player.this.yaw());
            let point = [protection, protection, feet[0].to_bits(), feet[1].to_bits(), feet[2].to_bits(), yaw.to_bits(),
                         protection, protection, protection];
            raw::engine_player_remove(Some(player.this), 1);
            raw::engine_spawn_player(Some(self.rule), u32::from(place), point.as_ptr(), self.time);
        }
        true
    }

    /// Every place due now is due no more, but an entering one: a place in the match whose
    /// player is out of the world (entered or came back, not spawned yet) spawns at its entry
    /// time. Held out, every world load would put it half into the world (link 0: no models, no
    /// player effect +F540h) and its next removal with 1 read a null effect at 0x2DE36D (the
    /// round's start, or the respawn rule's leave handler 0x316BE0), as Freeze Tag's of 2026-11-22.
    pub fn hold_due(&self) {
        let world = self.world();
        for place in 0..PLACES {
            let entering = world.as_ref().and_then(|world| world.player(place))
                .is_some_and(|player| !player.inserted());
            if entering {
                continue;
            }
            // SAFETY: the rule's twenty respawn times (+138h).
            unsafe {
                let at = self.rule.times().add(usize::from(place));
                if read(at) == self.time {
                    at.write_volatile(NO_RESPAWN);
                }
            }
        }
    }
    /// Every place of the match is due now: this tick takes out and spawns
    /// each, alive or dead (0x31697F..0x3169BE).
    pub fn everyone_now(&self) {
        let Some(world) = self.world() else { return };
        for place in 0..PLACES {
            if world.player(place).is_some() {
                // SAFETY: the rule's twenty respawn times.
                unsafe { self.rule.times().add(usize::from(place)).write_volatile(self.time) };
            }
        }
    }
}

/// base_player::can_sprint's player (0x2DBF45).
pub fn sprinter<'t>(ctx: &'t Context<'_>) -> Option<Player<'t>> {
    // SAFETY: can_sprint's EBX, its live player.
    let this = unsafe { raw::site::hunt_can_sprint::player(ctx) }?;
    Some(Player { this, thread: ctx.thread() })
}

impl<'t> Player<'t> {
    /// Its place byte (+114h), the place it plays in its world.
    pub fn hunt_place(&self) -> u8 {
        // SAFETY: the live player's place byte.
        unsafe { read(self.this.place()) }
    }
    /// Marks its animations for re-selection (+33Eh), so a changed modifier
    /// is asked again.
    pub fn select_animations(&self) {
        // SAFETY: the live player's simulation byte, as set_input sets it.
        unsafe { self.this.need_to_select_animations().write_volatile(1) }
    }
}

/// The rule object's size (the cook's slot 10).
pub const RULE_SIZE: u32 = raw::HuntRule::SIZE as u32;

impl RuleKey {
    /// The rule object's address: what the cook's create returns the game.
    pub fn address(self) -> u32 {
        self.0
    }
}

impl<'t> HuntCook<'t> {
    pub fn thread(&self) -> &'t Thread {
        self.thread
    }
}

impl HuntWorld<'_> {
    /// The world's time ([core+18FC0h]: the last finished step's).
    pub fn now(&self) -> u32 {
        // SAFETY: the live core's time.
        unsafe { read(self.core.time()) }
    }
}

/// render_name's player at its spotted test of an enemy
/// (`player_names.enemy_spotted`, ESI).
pub fn name_enemy<'t>(ctx: &'t Context<'_>) -> Option<Player<'t>> {
    // SAFETY: render_name's live player.
    let this = unsafe { raw::site::player_names_enemy_spotted::player(ctx) }?;
    Some(Player { this, thread: ctx.thread() })
}

/// update_minimap_players' player at its spotted test of an enemy
/// (`minimap.enemy_spotted`, EAX).
pub fn minimap_enemy<'t>(ctx: &'t Context<'_>) -> Option<Player<'t>> {
    // SAFETY: the minimap's live player (alive and in the world).
    let this = unsafe { raw::site::minimap_enemy_spotted::player(ctx) }?;
    Some(Player { this, thread: ctx.thread() })
}

/// The rule a tick delegate of ours is called on (the delegate's object).
pub struct TickRule<'t> {
    native: Option<raw::HuntRule>,
    thread: &'t Thread,
}

impl<'t> TickRule<'t> {
    /// # Safety
    /// The delegate's object: a rule of ours (only its own delegate has our
    /// function), alive while the world ticks.
    pub(in crate::vostok) unsafe fn from_raw(thread: &'t Thread, native: Option<raw::HuntRule>) -> TickRule<'t> {
        TickRule { native, thread }
    }
    pub fn rule(&self) -> Option<HuntRule<'t>> {
        Some(HuntRule { native: self.native?, thread: self.thread })
    }
}

// ---- presentation: the artefact in the world, the announcer, the mark ----

/// The world scene a game world draws (the artefacts' clients' renderer and
/// scene: `[[[world+9Ch]+5360h]+200064h]`, world+4).
pub struct ArtefactScene<'t> {
    scene: super::carry_runtime::Scene,
    _thread: &'t Thread,
}

/// A model's or a particle's place: upright, at `at` (row-major, the
/// translation in the fourth row, as the eye's matrix).
fn placed(at: [f32; 3]) -> [f32; 16] {
    [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, at[0], at[1], at[2], 1.0]
}

impl<'t> ArtefactScene<'t> {
    pub fn of(world: &super::world::GameWorld<'t>, thread: &'t Thread) -> Option<Self> {
        // SAFETY: the live game world of the hook (after its move, or as it
        // is deactivated), its game's renderer and its scene.
        let scene = unsafe { super::carry_runtime::Scene::of_world(world.this) }?;
        Some(Self { scene, _thread: thread })
    }
    /// `particle` moved to `matrix` (a row-major 4x4).
    pub fn move_to(&self, particle: super::resources::KeptParticle, matrix: &[f32; 16]) {
        // SAFETY: a particle system kept for the process; the live scene.
        unsafe { self.scene.move_particle(particle.0, matrix) }
    }
    /// `particle` played at `matrix`.
    pub fn play_at(&self, particle: super::resources::KeptParticle, matrix: &[f32; 16]) {
        // SAFETY: as play.
        unsafe { self.scene.play_particle(particle.0, matrix, super::carry_runtime::Bones::NONE) }
    }
    /// The model at `at`, added or moved there.
    pub fn show(&self, model: super::resources::KeptModel, at: [f32; 3]) {
        // SAFETY: a model kept for the process; the scene the live world's.
        unsafe {
            super::carry_runtime::Scene::as_world(model.0);
            self.scene.present(model.0, &placed(at));
        }
    }
    /// The model out of the scene (nothing when it is not in it).
    pub fn hide(&self, model: super::resources::KeptModel) {
        // SAFETY: as show; the remove guarded by the model's own in-scene bit.
        unsafe {
            self.scene.remove(model.0);
        }
    }
    pub fn play(&self, particle: super::resources::KeptParticle, at: [f32; 3]) {
        // SAFETY: a particle system kept for the process; the live scene.
        unsafe { self.scene.play_particle(particle.0, &placed(at), super::carry_runtime::Bones::NONE) }
    }
    pub fn stop(&self, particle: super::resources::KeptParticle) {
        // SAFETY: as play.
        unsafe { self.scene.stop_particle(particle.0) }
    }
}

/// A line played once in the game world's scene (vostok::sound's voice: let
/// go, it stops).
pub fn announce(world: &super::world::GameWorld<'_>, thread: &Thread, emitter: &super::sound::Emitter)
                -> Option<super::sound::Voice> {
    super::sound::play_once(world.this, thread, emitter)
}

/// The icon record's carry test (`player_icons.carrier_mark`): its player.
pub fn marked<'t>(ctx: &'t Context<'_>) -> Option<Player<'t>> {
    // SAFETY: the site's ESI, the live player whose icon is filled.
    let this = unsafe { raw::site::player_icons_carrier_mark::player(ctx) }?;
    Some(Player { this, thread: ctx.thread() })
}

/// A player's simulated transform (+40Ch) and camera (+44Ch), row-major.
pub fn transforms(player: &Player<'_>) -> ([f32; 16], [f32; 16]) {
    // SAFETY: the live player's two 4x4 matrices of f32.
    unsafe { (floats(player.this.transform()), floats(player.this.eye())) }
}

/// The carrier's mark: the record's flag set (CL 1, the setnz's) and EAX
/// the place x 3 (the lea the exit skips).
pub fn mark_carrier(ctx: &mut Context<'_>) {
    let place = ctx.reg(Reg::Eax);
    ctx.set_reg(Reg::Eax, place.wrapping_mul(3));
    let ecx = ctx.reg(Reg::Ecx);
    ctx.set_reg(Reg::Ecx, (ecx & !0xFF) | 1);
}
