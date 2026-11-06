//! The game effects our patches play on players, through the player's own
//! game_effect_player (+358h): `add` 0x2DE090 takes a reference to an effect
//! a game effect resource emitted (0x329050) and plays it with a time
//! calculator, until the calculator says it ended. The effects it plays are
//! a list (+2Ch the first, each +4Ch the next); each is read where it is
//! used, never kept. An effect played in a simulation tick is in the world's
//! history (the local visual stream), so a rollback restores it and its
//! re-simulation plays it again; one played outside the simulation is lost
//! at the next restore.
//!
//! Two calculators, both default clocks (vtable 0xA11398) that
//! `effect_clock` (default_game_effect_time_calculator::calculate 0x2E5D80)
//! answers for:
//! - [`timed_clock`]: a static of ours with no registry index, which the
//!   local visual stream writes as the pointer it is (never moved or freed):
//!   it ends an effect at the effect's own length (Medic's shared actives);
//! - [`kept_clock`]: the player's own calculator (+394h, its hit effects'),
//!   for the effects of kept resources: it ends one when the patch says the
//!   player no longer shows it (Forest Protection's glow, the leg quarters).

use core::cell::UnsafeCell;
use core::ffi::CStr;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicU32, Ordering};

use super::context::{Context, Reg};
use super::hook::Thread;
use super::world::{Player, read};
use crate::raw;

/// Medic's timed clock: a process-lifetime, stateless native calculator;
/// only each effect's serialized start and length carry its timing, no
/// player or item pointer.
struct TimedClock(UnsafeCell<raw::MedicalTimedClockStorage>);
// SAFETY: touched only on the simulation thread (a Thread); no borrow
// survives a native call, and the static is never moved or destroyed.
unsafe impl Sync for TimedClock {}
static TIMED: TimedClock = TimedClock(UnsafeCell::new(raw::MedicalTimedClockStorage::zeroed()));

impl TimedClock {
    fn listener(&self, _: &Thread) -> raw::GameEffectListener {
        // SAFETY: the static owns this exact native layout for the process.
        unsafe {
            let clock = (*self.0.get()).handle();
            *clock.vtable() = raw::image::DEFAULT_EFFECT_CLOCK_VTABLE;
            *clock.registry_index() = u32::MAX;
            clock.listener()
        }
    }
}

/// The default clock's answer: interval 0, time 0 (the artefact's frozen
/// glow), ended or not; EAX the result as the native returns it.
///
/// # Safety
/// `out` is the clock call's live result record.
unsafe fn answer(ctx: &mut Context<'_>, out: raw::MedicalClockResult, ended: bool) {
    // SAFETY: the caller's record.
    unsafe {
        *out.interval() = 0;
        *out.time() = 0.0;
        *out.ended() = u8::from(ended);
    }
    ctx.set_reg(Reg::Eax, out.address());
}

/// The default clock's call (`medical.effect_clock`) for the timed clock:
/// true when the calculator was it and the answer is written (the hook then
/// returns what the native would); false for any other calculator, which
/// goes on through the native calculation unchanged.
pub fn timed_clock(ctx: &mut Context<'_>) -> bool {
    use raw::site::medical_effect_clock as site;
    // SAFETY: this callback receives a live effect node and output record.
    unsafe {
        if site::calculator(ctx) != Some(TIMED.listener(ctx.thread())) {
            return false;
        }
        let Some(frame) = site::frame(ctx) else { return false };
        let (Some(out), Some(node)) = (frame.result(), frame.node()) else { return false };
        let Some(effect) = node.effect() else { return false };
        let elapsed = frame.now().read().wrapping_sub(*node.start());
        answer(ctx, out, elapsed as f32 / 1000.0 >= *effect.length());
        true
    }
}

/// The default clock's call (`medical.effect_clock`) for an effect of one of
/// `kept`, which are only ever played with their player's own calculator
/// ([`Player::hold_effect`]): ended unless `shows` says the player still
/// shows that one (its index in `kept`). False for any other effect, which
/// goes on unchanged.
pub fn kept_clock<const N: usize>(
    ctx: &mut Context<'_>,
    kept: &[KeptEffect; N],
    shows: impl FnOnce(usize, &Player<'_>) -> bool,
) -> bool {
    use raw::site::medical_effect_clock as site;
    // SAFETY: this callback receives a live effect node and output record;
    // an effect of a kept resource is played only with a player's own
    // calculator (+394h), so that calculator's owner is a live player (the
    // effect player ticking it is the player's +358h).
    unsafe {
        let Some(frame) = site::frame(ctx) else { return false };
        let (Some(out), Some(node)) = (frame.result(), frame.node()) else { return false };
        let Some(resource) = node.effect().and_then(|effect| effect.resource()) else { return false };
        let Some(index) = kept.iter().position(|kept| kept.0 == resource) else { return false };
        let Some(calculator) = site::calculator(ctx) else { return false };
        let player = Player { this: calculator.owner(), thread: ctx.thread() };
        let ended = !shows(index, &player);
        answer(ctx, out, ended);
        true
    }
}

/// The game effect resource cook's instance count (`medical.effect_capacity`,
/// 0x2EC81E: ESI the instances, EBX the named query): at least `instances`
/// for the effect called `name` (its config gameplay/effects/effect_<name>).
/// An emitted effect has its own tracks but takes its particles from this
/// pool, before the cook's asynchronous children allocate it.
pub fn effect_capacity(ctx: &mut Context<'_>, name: &CStr, instances: u32) {
    // SAFETY: the paused named-resource cook owns the query and both C strings.
    unsafe {
        let Some(query) = raw::site::medical_effect_capacity::query(ctx) else {
            return;
        };
        let resolved = query.resolved();
        let asked = if resolved.is_null() { query.requested() } else { resolved };
        // byte by byte, its NUL included (no strlen in the arena)
        if !asked.is_null() && name.to_bytes_with_nul().iter().enumerate().all(|(i, byte)| *asked.add(i) == *byte) {
            ctx.set_reg(Reg::Esi, ctx.reg(Reg::Esi).max(instances));
        }
    }
}

/// A game effect resource kept for the rest of the process: one reference
/// of its own, taken when its query arrived and never given back, so the
/// resource outlives every effect emitted from it (a restored effect names
/// its resource by pointer, 0x2E5BB0). A value: whoever has it may emit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeptEffect(raw::MedicalEffectResource);

impl KeptEffect {
    /// Keeps `resource` (a reference of ours taken now).
    ///
    /// # Safety
    /// `resource` is alive (a query's result, for its callback).
    pub(super) unsafe fn keep(resource: raw::MedicalEffectResource) -> KeptEffect {
        // SAFETY: alive (the caller); the count is atomic, as the game's own
        // takers do it (damage_zone_core_cook 0x3113BF).
        unsafe { AtomicU32::from_ptr(resource.references()).fetch_add(1, Ordering::AcqRel) };
        KeptEffect(resource)
    }
}

/// One emitted effect, owned until the receiving effect player retains it.
/// Instances contain mutable tracks and cannot be shared between players:
/// native rollback restores each player's tracks independently.
pub(super) struct OwnedEffect<'t> {
    slot: raw::GameEffectSlotStorage,
    _thread: PhantomData<&'t Thread>,
}

impl<'t> OwnedEffect<'t> {
    /// An effect of `resource`, its length `length_ms` (what the timed
    /// clock ends it at; None leaves the resource's).
    ///
    /// # Safety
    /// `resource` is alive for the hook (an artefact's own, or kept).
    pub(super) unsafe fn emit(_: &'t Thread, resource: raw::MedicalEffectResource, length_ms: Option<u32>) -> Self {
        let mut slot = raw::GameEffectSlotStorage::zeroed();
        // SAFETY: the resource is alive (the caller); native emit initializes
        // an independent effect with one reference in our slot.
        unsafe {
            raw::engine_effect_resource_emit(Some(resource), Some(slot.handle()));
            if let (Some(effect), Some(ms)) = (slot.handle().effect(), length_ms) {
                *effect.length() = ms as f32 / 1000.0;
            }
        }
        Self {
            slot,
            _thread: PhantomData,
        }
    }

    /// Plays it on `target` from `time` (simulation milliseconds) with the
    /// timed clock (Medic's shared actives).
    pub(super) fn play_timed(&mut self, thread: &Thread, target: raw::Player, time: u32) {
        self.play(target, time, TIMED.listener(thread));
    }

    fn play(&mut self, target: raw::Player, time: u32, calculator: raw::GameEffectListener) {
        // SAFETY: the target and resource live for this hook. Native add takes
        // its own reference; Drop releases only our temporary emit reference.
        unsafe {
            if self.slot.handle().effect().is_some() {
                raw::engine_effect_play(Some(target.effects()), Some(self.slot.handle()), time, 0, 2, Some(calculator));
            }
        }
    }
}

impl Drop for OwnedEffect<'_> {
    fn drop(&mut self) {
        // SAFETY: this guard owns exactly one native effect reference. The
        // engine uses a non-atomic count on this simulation thread.
        unsafe {
            if let Some(effect) = self.slot.handle().effect() {
                *effect.count() -= 1;
                if *effect.count() == 0 {
                    raw::engine_emitter_destroy(effect.emitter(), Some(effect));
                }
            }
        }
    }
}

impl<'t> Player<'t> {
    /// The player base_player::tick ticks (`battery.carrier_tick`): every
    /// player of the world, every simulation tick (a re-simulation's too).
    pub fn at_tick(ctx: &'t Context<'_>) -> Option<Player<'t>> {
        // SAFETY: the context is this site's: the player base_player::tick
        // ticks, alive on the game's thread.
        let this = unsafe { raw::site::battery_carrier_tick::player(ctx) }?;
        Some(Player { this, thread: ctx.thread() })
    }

    /// The damage zones it is inside (+2E4h): what the anomaly hit reads.
    pub fn damage_zones(&self) -> u8 {
        // SAFETY: the live player's byte, on the hook's thread.
        unsafe { read(self.this.damage_zones()) }
    }

    /// The factor a hit with an owner is given while it is inside a damage
    /// zone: the native getter of modifier 41 (incoming_weapon_damage_in_anomaly,
    /// max(0, 1 + its sum) of its current kit, 0x33286A); 1 without a kit.
    pub fn weapon_damage_in_anomaly(&self) -> f32 {
        // SAFETY: the player owns its profile and its current kit's
        // modifiers; the getter only reads them.
        unsafe {
            let Some(profile) = self.this.profile() else { return 1.0 };
            let modifiers = profile.kits().at(usize::from(read(self.this.kit()))).modifiers();
            raw::engine_apply_modifier(Some(modifiers), 41, 1.0, 1.0)
        }
    }

    /// The local player of its game world (the one this game plays), none without one (a
    /// spectator's game, a replay): the world's game's network client's.
    pub fn local_player(&self) -> Option<Player<'t>> {
        // SAFETY: the live player's game world (+F474h), alive while the player is.
        let world = unsafe { self.this.game_world() }?;
        super::world::GameWorld::of(world, self.thread).network_client()?.local_player()
    }

    /// The factor its current kit gives an artefact's active time: the native getter of
    /// modifier 39 (artefact_duration, max(0, 1 + its sum)); above 1 with Knowledge of Artifacts.
    pub fn active_duration_factor(&self) -> f32 {
        // SAFETY: the player owns its profile and its current kit's modifiers; the getter only
        // reads them.
        unsafe {
            let Some(profile) = self.this.profile() else { return 1.0 };
            let modifiers = profile.kits().at(usize::from(read(self.this.kit()))).modifiers();
            raw::engine_apply_modifier(Some(modifiers), 39, 1.0, 1.0)
        }
    }

    /// Whether it plays an effect of `kept`.
    pub fn plays_effect(&self, kept: KeptEffect) -> bool {
        // SAFETY: the player's effect list, walked on the simulation thread
        // that changes it (as remove 0x2DE350 walks it); each node and its
        // effect are the list's own while it plays them.
        unsafe {
            let mut node = self.this.effects().first();
            while let Some(at) = node {
                if at.effect().and_then(|effect| effect.resource()) == Some(kept.0) {
                    return true;
                }
                node = at.next();
            }
        }
        false
    }

    /// Plays an effect of `kept` from its own time, with its own calculator
    /// (+394h): [`kept_clock`] ends it.
    pub fn hold_effect(&self, kept: KeptEffect) {
        // SAFETY: a kept resource lives for the process; the player's
        // simulation time and its calculator are its own.
        unsafe {
            let time = read(self.this.time());
            OwnedEffect::emit(self.thread(), kept.0, None).play(self.this, time, self.this.effect_calculator());
        }
    }
}
