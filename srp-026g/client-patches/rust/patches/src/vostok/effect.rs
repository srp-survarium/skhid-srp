//! The game effects our patches play on players, through the player's own
//! game_effect_player (+358h): `add` 0x2DE090 takes a reference to an effect
//! a game effect resource emitted (0x329050) and plays it with a time
//! calculator; `remove` 0x2DE350 stops one it plays, as damage zones stop
//! their inside effect (0x31A255). The effects it plays are a list (+2Ch the
//! first, each +4Ch the next); each is read where it is used, never kept.
//!
//! The calculators are ours, one static each ([`Clock`]): native default
//! clocks (vtable 0xA11398) with no registry index, which the local visual
//! stream writes as the pointer they are (they are never moved or freed).
//! `effect_clock` (default_game_effect_time_calculator::calculate 0x2E5D80)
//! answers for them and lets every native clock through.

use core::cell::UnsafeCell;
use core::ffi::CStr;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicU32, Ordering};

use super::context::{Context, Reg};
use super::hook::Thread;
use super::world::{Player, read};
use crate::raw;

/// The time calculators of ours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clock {
    /// Ends when the effect's own length (its serialized +24h, seconds) has
    /// passed since its start: Medic's shared Larkspur actives.
    Timed,
    /// Never ends on its own: the patch that holds it removes it (the
    /// artefact's own calculator holds its glow the same way, at time 0,
    /// while the artefact is active).
    Held,
}

impl Clock {
    const ALL: [Clock; 2] = [Clock::Timed, Clock::Held];

    fn cell(self) -> &'static ClockCell {
        &CLOCKS[self as usize]
    }

    fn listener(self, thread: &Thread) -> raw::GameEffectListener {
        self.cell().listener(thread)
    }

    /// Which of ours a calculator is, if it is one.
    fn of(calculator: Option<raw::GameEffectListener>, thread: &Thread) -> Option<Clock> {
        Clock::ALL.into_iter().find(|clock| calculator == Some(clock.listener(thread)))
    }
}

/// A process-lifetime, stateless native calculator: only each effect's
/// serialized start and length carry its timing; no player or item pointer.
struct ClockCell(UnsafeCell<raw::MedicalTimedClockStorage>);
// SAFETY: touched only on the simulation thread (a Thread); no borrow
// survives a native call, and the static is never moved or destroyed.
unsafe impl Sync for ClockCell {}
static CLOCKS: [ClockCell; 2] = [
    ClockCell(UnsafeCell::new(raw::MedicalTimedClockStorage::zeroed())),
    ClockCell(UnsafeCell::new(raw::MedicalTimedClockStorage::zeroed())),
];

impl ClockCell {
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

/// The default clock's call (`medical.effect_clock`), for `clock`: true when
/// the calculator was it and the answer is written (the hook then returns
/// what the native would); false for any other calculator, which goes on
/// through the native calculation unchanged.
pub fn effect_clock(ctx: &mut Context<'_>, clock: Clock) -> bool {
    use raw::site::medical_effect_clock as site;
    // SAFETY: this callback receives a live effect node and output record.
    unsafe {
        if Clock::of(site::calculator(ctx), ctx.thread()) != Some(clock) {
            return false;
        }
        let Some(frame) = site::frame(ctx) else { return false };
        let (Some(out), Some(node)) = (frame.result(), frame.node()) else { return false };
        let Some(effect) = node.effect() else { return false };
        let ended = match clock {
            Clock::Timed => {
                let elapsed = frame.now().read().wrapping_sub(*node.start());
                elapsed as f32 / 1000.0 >= *effect.length()
            }
            Clock::Held => false,
        };
        *out.interval() = 0;
        *out.time() = 0.0;
        *out.ended() = u8::from(ended);
        ctx.set_reg(Reg::Eax, out.address());
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
    /// An effect of `resource`, its length `length_ms` (what [`Clock::Timed`]
    /// ends it at; None leaves the resource's).
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

    /// Plays it on `target` from `time` (simulation milliseconds) with `clock`.
    pub(super) fn play_on(&mut self, thread: &Thread, target: raw::Player, time: u32, clock: Clock) {
        // SAFETY: the target and resource live for this hook. Native add takes
        // its own reference; Drop releases only our temporary emit reference.
        unsafe {
            if self.slot.handle().effect().is_some() {
                raw::engine_effect_play(
                    Some(target.effects()),
                    Some(self.slot.handle()),
                    time,
                    0,
                    2,
                    Some(clock.listener(thread)),
                );
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

/// An effect of a kept resource that a player plays with the held clock.
pub struct HeldEffect<'t> {
    player: raw::Player,
    effect: raw::GameEffect,
    _thread: PhantomData<&'t Thread>,
}

impl HeldEffect<'_> {
    /// Stops it (the player's effect player removes it from its list).
    pub fn remove(self) {
        // SAFETY: the effect is one the player's effect player plays (found
        // in its list in this hook), which remove requires.
        unsafe { raw::engine_effect_remove(Some(self.player.effects()), Some(self.effect)) };
    }
}

impl<'t> Player<'t> {
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

    /// The effect of `kept` it plays with the held clock, if it plays one.
    pub fn held_effect(&self, kept: KeptEffect) -> Option<HeldEffect<'t>> {
        let held = Clock::Held.listener(self.thread());
        // SAFETY: the player's effect list, walked on the simulation thread
        // that changes it (as remove walks it); each node and its effect are
        // the list's own while it plays them.
        unsafe {
            let mut node = self.this.effects().first();
            while let Some(at) = node {
                if at.calculator() == Some(held)
                    && let Some(effect) = at.effect()
                    && effect.resource() == Some(kept.0)
                {
                    return Some(HeldEffect { player: self.this, effect, _thread: PhantomData });
                }
                node = at.next();
            }
        }
        None
    }

    /// Plays an effect of `kept` with the held clock, from its own time.
    pub fn hold_effect(&self, kept: KeptEffect) {
        // SAFETY: a kept resource lives for the process; the player's
        // simulation time is its own field.
        unsafe {
            let time = read(self.this.time());
            OwnedEffect::emit(self.thread(), kept.0, None).play_on(self.thread(), self.this, time, Clock::Held);
        }
    }
}
