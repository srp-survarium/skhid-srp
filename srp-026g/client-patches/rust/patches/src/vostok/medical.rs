//! Medical hooks over generated typed native handles. A borrowed player lives
//! for the paused hook; a core lookup owns one reference until its guard drops.
use super::{Context, Reg, Thread, drug::DrugsInUse};
use crate::{
    medical_skills::{Painkiller, Regeneration, Skills, Stance},
    raw,
};
use super::effect::OwnedEffect;
use core::{
    ffi::CStr,
    sync::atomic::{AtomicU32, Ordering},
};

/// The modifiers of `player`'s current kit (the match profile's, as the static info gave it).
pub(super) fn kit_modifiers(player: raw::Player) -> Option<raw::PlayerParamsModifiersContainer> {
    // SAFETY: a hook's player owns its profile and current kit.
    unsafe { Some(player.profile()?.kits().at(usize::from(*player.kit())).modifiers()) }
}

/// A player a hook reads (the artefact tree's aura too, vostok::artefact_skills).
#[derive(Clone, Copy)]
pub(super) struct Player<'t> {
    at: raw::Player,
    thread: &'t Thread,
}
impl<'t> Player<'t> {
    pub(super) fn new(thread: &'t Thread, at: raw::Player) -> Self {
        Self { at, thread }
    }
    pub(super) fn get(self) -> raw::Player {
        self.at
    }
    fn modifiers(self) -> Option<raw::PlayerParamsModifiersContainer> {
        kit_modifiers(self.at)
    }
    fn skills(self) -> Skills {
        // SAFETY: generated modifier fields belong to the live current kit.
        self.modifiers()
            .and_then(|m| unsafe { Skills::parse(*m.medical_ranks(), *m.combo_ranks()) })
            .unwrap_or_default()
    }
    /// Any painkiller of its inventory in use (set_active's +2B0h, the time
    /// its protection records are linked into the body parts, 0x33D638).
    fn painkiller(self) -> Painkiller {
        // SAFETY: a hook's player is alive on its thread; a drug in use is
        // its inventory's medkit, which owns its protection records.
        let active = unsafe {
            DrugsInUse::of(self.thread, self.at).any(|drug| painkiller(drug.medkit()))
        };
        if active { Painkiller::Active } else { Painkiller::Inactive }
    }
    /// The controller's effective crouch (+201h): it stays set under a low
    /// ceiling after the crouch key is released (0x337830).
    fn stance(self) -> Stance {
        // SAFETY: the player owns its controller wrapper and Bullet controller.
        let crouched = unsafe {
            self.at
                .character_controller()
                .and_then(|wrapper| wrapper.controller())
                .is_some_and(|controller| *controller.effective_crouch() != 0)
        };
        if crouched { Stance::Crouched } else { Stance::Standing }
    }
    fn damage(self) -> Option<raw::MedicalDamage> {
        // SAFETY: the model is bound before the initialization hook.
        unsafe { self.at.damage_model() }
    }
    fn team(self) -> Option<u32> {
        // SAFETY: the profile is owned by the live player.
        unsafe { Some(*self.at.profile()?.team()) }
    }
    pub(super) fn ally(self, other: Self) -> bool {
        self.allies()(other)
    }
    /// [`ally`](Self::ally) of this player, its team read once: the test of a whole roster.
    pub(super) fn allies(self) -> impl Fn(Self) -> bool {
        let team = self.team().filter(|team| matches!(team, 0 | 1));
        move |other| team.is_some() && self.at != other.at && other.team() == team
    }
    pub(super) fn active(self) -> bool {
        // The core lookup already checked the synchronized active roster.
        // +33F is a temporary insertion flag, cleared by native remove during
        // restoration; it is not serialized and cannot gate shared gameplay.
        // SAFETY: alive is the borrowed player's serialized simulation state.
        unsafe { *self.at.alive() != 0 }
    }
    fn pool(self, name: &CStr) -> Option<raw::MedicalPool> {
        let name = name.as_ptr().cast::<u8>();
        // SAFETY: lookup borrows the model and the NUL-terminated name.
        unsafe { raw::engine_find_pool(Some(self.damage()?.pools()), &name) }
    }
    fn health(self) -> f32 {
        // SAFETY: successful lookup belongs to this player's model.
        self.pool(c"body")
            .map(|p| unsafe { *p.health() / *p.maximum() })
            .unwrap_or(1.0)
    }
    pub(super) fn refresh(self) {
        // SAFETY: native invalidation requests movement/item selection again.
        unsafe {
            *self.at.refresh_selection() = 1;
        }
    }
    pub(super) fn distance_squared(self, other: Self) -> f32 {
        // SAFETY: both translation fields have three contiguous floats.
        unsafe {
            let (a, b) = (self.at.feet(), other.at.feet());
            (0..3)
                .map(|i| {
                    let d = *a.add(i) - *b.add(i);
                    d * d
                })
                .sum()
        }
    }
    /// The player at `place` of this player's world (its active roster).
    pub(super) fn at_place(self, thread: &'t Thread, place: u8) -> Option<OwnedPlayer<'t>> {
        // SAFETY: the live player's world core.
        Self::in_core(thread, unsafe { self.at.world_core() }, place)
    }
    /// The player at `place` of `core`'s active roster (core_active_player: a departed place,
    /// one past the places or a place without a player is none).
    /// The active player at `place` of `core`'s world; none without a world core (a lobby's
    /// player: core_active_player reads through it unchecked, 0x2D0B12).
    pub(super) fn in_core(thread: &'t Thread, core: Option<raw::CoreWorld>, place: u8) -> Option<OwnedPlayer<'t>> {
        let core = core?;
        let mut out = None;
        // SAFETY: native lookup in this simulation world adds one reference.
        unsafe {
            raw::engine_core_active_player(Some(core), u32::from(place), &mut out);
        }
        out.map(|at| OwnedPlayer(Self::new(thread, at)))
    }
}
pub(super) struct OwnedPlayer<'t>(Player<'t>);
impl<'t> OwnedPlayer<'t> {
    pub(super) fn get(&self) -> Player<'t> {
        self.0
    }
}
impl Drop for OwnedPlayer<'_> {
    fn drop(&mut self) {
        // SAFETY: release exactly the one reference acquired by core lookup.
        unsafe {
            let count = self.0.at.references();
            if AtomicU32::from_ptr(count).fetch_sub(1, Ordering::AcqRel) == 1 {
                raw::engine_release_object(Some(self.0.at), count);
            }
        }
    }
}
// Native pool names have at most 15 bytes and a NUL terminator.
pub(super) unsafe fn name_equal(a: *const u8, b: *const u8) -> bool {
    if a.is_null() || b.is_null() {
        return false;
    }
    for i in 0..16 {
        // SAFETY: native names are terminated; stop at the first NUL.
        let (a, b) = unsafe { (*a.add(i), *b.add(i)) };
        if a != b {
            return false;
        }
        if a == 0 {
            return true;
        }
    }
    false
}
unsafe fn painkiller(item: raw::Medkit) -> bool {
    // SAFETY: item owns its declared array of protection records.
    unsafe {
        let Some(first) = item.protection() else {
            return false;
        };
        (0..usize::from(*item.protection_count()))
            .any(|i| name_equal(first.at(i).name(), c"pain".as_ptr().cast()))
    }
}
/// # Safety
/// The player and medkit are live on this hook's simulation thread.
pub(super) unsafe fn adjusted_duration(
    thread: &Thread,
    player: raw::Player,
    item: raw::Medkit,
    native: f32,
) -> f32 {
    // SAFETY: callers supply the typed objects retained by the native hook.
    if unsafe { painkiller(item) } {
        Player::new(thread, player)
            .skills()
            .pain_duration_ms(native)
    } else {
        native
    }
}
/// # Safety
/// Same live-object contract as adjusted_duration.
pub(super) unsafe fn adjusted_use_time(
    thread: &Thread,
    player: raw::Player,
    item: raw::Medkit,
    native: f32,
) -> f32 {
    // SAFETY: influences is the live medkit's healing-record count.
    if unsafe { *item.influences() != 0 } {
        Player::new(thread, player).skills().use_time(native)
    } else {
        native
    }
}
macro_rules! duration_hook {
    ($name:ident,$site:ident) => {
        pub fn $name(ctx: &mut Context<'_>) {
            use raw::site::$site as site;
            // SAFETY: these typed registers belong to the declared hook site.
            let value = unsafe {
                match (site::player(ctx), site::item(ctx)) {
                    (Some(player), Some(item)) => {
                        adjusted_duration(ctx.thread(), player, item, ctx.xmm_f32(0))
                    }
                    _ => return,
                }
            };
            ctx.set_xmm_f32(0, value);
        }
    };
}
duration_hook!(duration_start, medical_duration_start);
duration_hook!(duration_tick, medical_duration_tick);
duration_hook!(duration_restore, medical_duration_restore);
pub fn use_time(ctx: &mut Context<'_>) {
    use raw::site::medical_use_time as site;
    // SAFETY: set_active's native player and medicine remain live.
    let value = unsafe {
        match (site::player(ctx), site::item(ctx)) {
            (Some(player), Some(item)) => {
                adjusted_use_time(ctx.thread(), player, item, ctx.xmm_f32(0))
            }
            _ => return,
        }
    };
    ctx.set_xmm_f32(0, value);
}
/// Numb Legs at player_stamina::sprint: XMM0 the stamina a second.
pub fn sprint(ctx: &mut Context<'_>) {
    // SAFETY: the stamina is embedded in its live player (base_player+6C0h).
    let value = unsafe {
        let Some(stamina) = raw::site::medical_sprint::stamina(ctx) else {
            return;
        };
        let player = Player::new(ctx.thread(), stamina.player());
        player.skills().sprint_cost(ctx.xmm_f32(0), player.painkiller())
    };
    ctx.set_xmm_f32(0, value);
}
/// Take Cover at medkit::tick's heal amount (after drug_amount, which
/// Medical Training raises): XMM0 the amount of this influence.
pub fn medicine_heal(ctx: &mut Context<'_>) {
    use raw::site::medical_medicine_heal as site;
    // SAFETY: medkit::tick's holder (EBP) is live for the tick.
    let value = unsafe {
        let Some(at) = site::player(ctx) else {
            return;
        };
        let player = Player::new(ctx.thread(), at);
        player.skills().medicine_heal(ctx.xmm_f32(0), player.stance())
    };
    ctx.set_xmm_f32(0, value);
}
/// Unflinching, from patch 54's stagger after Spring's factor: the sum of
/// the recent hits' stopping power, written back to its argument.
pub fn stopping(ctx: &mut Context<'_>) {
    use raw::site::medical_stopping as site;
    // SAFETY: the caller's frame holds the staggered live player and the sum.
    unsafe {
        let Some(frame) = site::frame(ctx) else {
            return;
        };
        let Some(at) = frame.player() else {
            return;
        };
        let player = Player::new(ctx.thread(), at);
        *frame.sum() = player.skills().stopping_power(*frame.sum(), player.painkiller());
    }
}
/// Concentration at breath_vibration_calculator::tick, after apply_modifier
/// of breath_vibration (61): XMM0 the dispersion that drives the aim's sway,
/// the tick's argument the dispersion before the modifier.
pub fn sway(ctx: &mut Context<'_>) {
    use raw::site::medical_sway as site;
    // SAFETY: the aimed weapon's breath calculator and its holder are live for
    // its tick; EBP is the tick's frame, its first argument unchanged.
    let value = unsafe {
        let (Some(calculator), Some(frame)) = (site::calculator(ctx), site::frame(ctx)) else {
            return;
        };
        let Some(at) = calculator.player() else {
            return;
        };
        let player = Player::new(ctx.thread(), at);
        player.skills().sway(ctx.xmm_f32(0), *frame.dispersion(), player.painkiller())
    };
    ctx.set_xmm_f32(0, value);
}
pub fn initialize(ctx: &mut Context<'_>) {
    // SAFETY: constructor bound the complete model, before first reset/load.
    unsafe {
        let Some(at) = raw::site::medical_initialize::player(ctx) else {
            return;
        };
        let player = Player::new(ctx.thread(), at);
        let skills = player.skills();
        for (i, name) in [
            c"body",
            c"head",
            c"pain",
            c"left_hand",
            c"right_hand",
            c"left_leg",
            c"right_leg",
        ]
        .into_iter()
        .enumerate()
        {
            let Some(pool) = player.pool(name) else {
                continue;
            };
            *pool.delay() = skills.regeneration_delay_ms(*pool.delay());
            if i >= 3 {
                *pool.maximum() = skills.limb_health(*pool.maximum());
                *pool.health() = skills.limb_health(*pool.health());
                *pool.previous_health() = skills.limb_health(*pool.previous_health());
            }
        }
    }
}
/// Kit modifiers the exe never reads (no getter call, no code reference, no
/// retail config), srp's regeneration ones, made from items' built-in
/// modifications as any modifier (docs/notes/regeneration-booster.md):
/// 10 engineer_success_chance, additive in the getter (table 0x927E08):
/// seconds added to the delay after a hit; 9 engineer_use_time,
/// multiplicative (the getter gives max(0, 1 + S)): the fraction of the
/// maximum added to the limit regeneration stops at.
const REGENERATION_DELAY: u32 = 10;
const REGENERATION_LIMIT: u32 = 9;

pub fn regenerate(ctx: &mut Context<'_>) {
    use raw::site::medical_regenerate as site;
    let delay = ctx.reg(Reg::Eax);
    // SAFETY: body_part::regenerate's pool, its model's owner (bound at the
    // constructor) and the frame's locals live for this hook.
    let delay = unsafe {
        let (Some(pool), Some(frame)) = (site::pool(ctx), site::frame(ctx)) else {
            return;
        };
        let Some(at) = pool.model().and_then(|m| m.owner()) else {
            return;
        };
        let modifiers = Player::new(ctx.thread(), at).modifiers();
        if modifiers.is_none() {
            return;
        }
        let regeneration = Regeneration::parse(
            raw::engine_apply_modifier(modifiers, REGENERATION_DELAY, 0.0, 1.0),
            raw::engine_apply_modifier(modifiers, REGENERATION_LIMIT, 1.0, 1.0) - 1.0,
        )
        .unwrap_or_default();
        let (delay, limit) = regeneration.apply(*frame.rate(), delay, *frame.limit());
        *frame.limit() = limit;
        delay
    };
    ctx.set_reg(Reg::Eax, delay);
}
pub fn movement(ctx: &mut Context<'_>) {
    // SAFETY: the movement epilogue retains the player and final speed.
    let speed = unsafe {
        let Some(at) = raw::site::medical_movement::player(ctx) else {
            return;
        };
        let player = Player::new(ctx.thread(), at);
        player
            .skills()
            .movement_speed(ctx.xmm_f32(0), player.health())
    };
    ctx.set_xmm_f32(0, speed);
}
fn refresh_pool(thread: &Thread, pool: Option<raw::MedicalPool>) {
    // SAFETY: hit/heal's pool lives for the hook. Construction has no owner yet.
    unsafe {
        let Some(at) = pool.and_then(|p| p.model()).and_then(|m| m.owner()) else {
            return;
        };
        let player = Player::new(thread, at);
        if player.skills().has_adrenalin() {
            player.refresh();
        }
    }
}
pub fn heal(ctx: &mut Context<'_>) {
    // SAFETY: generated typed register at this declared pool-heal site.
    refresh_pool(ctx.thread(), unsafe { raw::site::medical_heal::pool(ctx) });
}
pub fn wound(ctx: &mut Context<'_>) {
    // SAFETY: generated typed register at this declared pool-hit site.
    refresh_pool(ctx.thread(), unsafe { raw::site::medical_wound::pool(ctx) });
}
pub fn hit(ctx: &mut Context<'_>) {
    use raw::site::medical_hit as site;
    // SAFETY: native model, hit and stack-local amount live for this hook.
    unsafe {
        let Some(at) = site::model(ctx).and_then(|m| m.owner()) else {
            return;
        };
        let Some(owner) = site::hit(ctx).and_then(|h| h.owner()) else {
            return;
        };
        let Ok(place) = u8::try_from(*owner.place()) else {
            return;
        };
        let Some(frame) = site::frame(ctx) else {
            return;
        };
        let player = Player::new(ctx.thread(), at);
        if let Some(source) = player.at_place(ctx.thread(), place)
            && player.ally(source.0)
        {
            *frame.amount() = player.skills().allied_damage(*frame.amount());
        }
    }
}
pub fn protect(ctx: &mut Context<'_>) {
    use raw::site::medical_protection as site;
    // SAFETY: replacement of medkit::reduce_damage's complete body. Entry
    // arguments and protection records are borrowed, pierce stays untouched.
    unsafe {
        let (Some(item), Some(frame)) = (site::item(ctx), site::frame(ctx)) else {
            return;
        };
        let Some(hit) = frame.hit() else {
            return;
        };
        let damage = frame.damage();
        if damage.is_null() {
            return;
        }
        let Some(first) = item.protection() else {
            return;
        };
        for i in 0..usize::from(*item.protection_count()) {
            let p = first.at(i);
            if *p.hit_kind() == *hit.kind() && name_equal(p.name(), hit.name()) {
                let threshold = *p.threshold();
                if *damage < threshold {
                    *damage = 0.0;
                    return;
                }
                let Some(holder) = item.item().inventory().and_then(|v| v.holder()) else {
                    return;
                };
                let player = Player::new(ctx.thread(), holder.player());
                let coefficient =
                    raw::engine_apply_modifier(player.modifiers(), 58, *p.coefficient(), 1.0);
                let coefficient = if name_equal(hit.name(), c"pain".as_ptr().cast()) {
                    player.skills().pain_coefficient(coefficient)
                } else {
                    coefficient
                };
                *damage = (*damage - threshold) * coefficient;
                return;
            }
        }
    }
}
/// Cascade's share of an activation (Medic's, rev45, for Larkspur; every kind since the owner's
/// rev51 games, artefact-skills.md "Resonance or Cascade"): when a copy of a kind Cascade shares
/// becomes active and its holder has the share (Medic's bit, which skill 542 sets), each living
/// teammate within Cascade's reach plays the kind's own activation glow for the active's length (as
/// its holder does: Spring blue, Lifebone green, Onyx orange), and a Lifebone's also does to it what
/// its active does to its holder (the configured affects removed, body, head and pain full). What a
/// Spring's or an Onyx's active gives is read where it is used (patch 109's aura); this is its
/// activation's part.
pub fn share_active(ctx: &mut Context<'_>) {
    use raw::site::medical_artefact as site;
    // SAFETY: the actual activation owns this item and its inventory holder.
    // The visual effect helper is also called on restore and must not heal.
    unsafe {
        let Some(item) = site::item(ctx) else {
            return;
        };
        let Some(holder) = item.item().inventory().and_then(|v| v.holder()) else {
            return;
        };
        let at = holder.player();
        let Some((kind, length_ms)) = super::artefact_skills::activated(item.item().artefact()) else {
            return;
        };
        let source = Player::new(ctx.thread(), at);
        if !source.skills().is_medic() {
            return;
        }
        let time = ctx.reg(super::Reg::Edi);
        let reach = crate::artefact_skills::CASCADE_REACH * crate::artefact_skills::CASCADE_REACH;
        for place in 0..20 {
            let Some(target) = source.at_place(ctx.thread(), place) else {
                continue;
            };
            if !target.0.active()
                || !source.ally(target.0)
                || source.distance_squared(target.0) > reach
            {
                continue;
            }
            if kind == crate::artefact_skills::Kind::Lifebone {
                heal_as_lifebone(item, target.0, time);
            }
            if let Some(resource) = item.active_resource() {
                OwnedEffect::emit(ctx.thread(), resource, Some(length_ms))
                    .play_timed(ctx.thread(), target.0.at, time);
            }
        }
    }
}

/// A Lifebone's active on `target`: its configured affects removed, body, head and pain full.
///
/// # Safety
/// `item` is the activating Lifebone (its removed-affect records its own), `target` a live player
/// of its world, on the simulation thread.
unsafe fn heal_as_lifebone(item: raw::MedicalArtefact, target: Player<'_>, time: u32) {
    // SAFETY: the caller's contract.
    unsafe {
        if let (Some(first), Some(end)) = (item.removes(), item.removes_end()) {
            for i in 0..end.index_from(first).unwrap_or(0) {
                let remove = first.at(i);
                let name = remove.name() as *const u8;
                if let Some(model) = target.damage()
                    && let Some(pool) = raw::engine_find_pool(Some(model.pools()), &name)
                {
                    raw::engine_remove_affect(Some(pool), time, *remove.affect());
                }
            }
        }
        for name in [c"body", c"head", c"pain"] {
            if let Some(pool) = target.pool(name) {
                raw::engine_increase_health(Some(pool), time, *pool.maximum());
            }
        }
    }
}
