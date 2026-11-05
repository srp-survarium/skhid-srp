//! Medical skill decisions. Two unused profile modifiers transport exact
//! integers: 43 the tree's original ranks, 45 the combo skills' (Numb Legs,
//! Take Cover, Unflinching or Concentration). Two more, 9 and 10, are summed
//! from the kit's items like any modifier: their regeneration (the
//! Regeneration booster). No new world state or snapshot side table:
//! Concentration is the local player's hearing only.
#![forbid(unsafe_code)]

use core::sync::atomic::{AtomicU32, Ordering};

use crate::vostok::sound::{self, OnceCaller, OncePoint, Source, VoiceVolume, WeaponSound, WeaponSoundKind};
use crate::vostok::types::Place;
use crate::vostok::effect;
use crate::vostok::{Context, Flow, NoExit, medical};

/// What the kit's items add to passive regeneration (the Regeneration
/// booster's built-in modifications 1290, 1300; regeneration-booster.md),
/// parsed once from the two sums of the kit's modifier lists.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Regeneration {
    /// Milliseconds off the delay after a hit, on top of Metabolism's.
    sooner_ms: u32,
    /// Fraction of the maximum added to the limit regeneration stops at.
    raise: f32,
}

impl Regeneration {
    /// `delay_s`: seconds added to the delay (only sooner, at most 60 s);
    /// `limit`: fraction added to the limit (0 or more). None otherwise.
    pub fn parse(delay_s: f32, limit: f32) -> Option<Self> {
        ((-60.0..=0.0).contains(&delay_s) && limit.is_finite() && limit >= 0.0).then(|| Self {
            sooner_ms: (-delay_s * 1000.0 + 0.5) as u32,
            raise: limit,
        })
    }

    /// The delay (ms) and limit one tick of a pool's regeneration uses. A
    /// pool that does not regenerate (rate 0) keeps its own: raising its
    /// limit would only make the native heal it by nothing.
    pub fn apply(self, rate: f32, delay_ms: u32, limit: f32) -> (u32, f32) {
        if rate > 0.0 {
            (delay_ms.saturating_sub(self.sooner_ms), limit.max((limit + self.raise).min(1.0)))
        } else {
            (delay_ms, limit)
        }
    }
}

/// Whether one of the player's painkillers is active: native set_active's
/// flag (+2B0h), the whole time its pain protection is linked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Painkiller {
    Inactive,
    Active,
}

/// The player's effective crouch (the controller's, which stays crouched
/// under a low ceiling), not the crouch key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stance {
    Standing,
    Crouched,
}

/// A packed field: its first bit, its width and the most ranks it takes.
struct Field {
    shift: u32,
    bits: u32,
    most: u8,
}
impl Field {
    const fn new(shift: u32, bits: u32, most: u8) -> Self {
        Self { shift, bits, most }
    }
    fn rank(&self, packed: u32) -> Option<u8> {
        let rank = ((packed >> self.shift) & ((1 << self.bits) - 1)) as u8;
        (rank <= self.most).then_some(rank)
    }
    fn unlock(&self, packed: u32) -> Option<bool> {
        self.rank(packed).map(|rank| rank != 0)
    }
}

/// Modifier 43's layout, unchanged since the tree shipped (the five-rank
/// fields kept three bits when their skills went down to two or three ranks).
const PAINKILLER: Field = Field::new(0, 3, 3);
const BANDAGING: Field = Field::new(3, 3, 2);
const METABOLISM: Field = Field::new(6, 3, 5);
const TRAINING: Field = Field::new(9, 3, 3);
const ADRENALIN: Field = Field::new(12, 3, 3);
const MEDICINE: Field = Field::new(15, 1, 1);
const SUPPLY: Field = Field::new(16, 1, 1);
const BONES: Field = Field::new(17, 3, 5);
/// Medic left the tree (it returns in another one): its bit stays decoded.
const MEDIC: Field = Field::new(20, 1, 1);
const BROTHERS: Field = Field::new(21, 3, 2);
/// Modifier 45: Numb Legs and Take Cover two bits each, Unflinching and
/// Concentration three each (four ranks); every combination below 1024.
const NUMB_LEGS: Field = Field::new(0, 2, 2);
const TAKE_COVER: Field = Field::new(2, 2, 2);
const UNFLINCHING: Field = Field::new(4, 3, 4);
const CONCENTRATION: Field = Field::new(7, 3, 4);

/// Tier 4's pair that excludes each other (the tree's antiskills): at most
/// one of them, 1..=4 ranks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Focus {
    #[default]
    Neither,
    Unflinching(u8),
    Concentration(u8),
}

impl Focus {
    /// Both held is no set the tree makes: refused.
    fn parse(unflinching: u8, concentration: u8) -> Option<Focus> {
        match (unflinching, concentration) {
            (0, 0) => Some(Focus::Neither),
            (rank, 0) => Some(Focus::Unflinching(rank)),
            (0, rank) => Some(Focus::Concentration(rank)),
            _ => None,
        }
    }
}

/// A packed modifier: an exact integer of `bits` bits, nothing else.
fn packed(value: f32, bits: u32) -> Option<u32> {
    let most = ((1u32 << bits) - 1) as f32;
    ((0.0..=most).contains(&value) && value == (value as u32) as f32).then_some(value as u32)
}

/// The ranks, parsed once from the profile, never an arbitrary float inside
/// the gameplay calculation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Skills {
    painkiller: u8,
    bandaging: u8,
    metabolism: u8,
    training: u8,
    adrenalin: u8,
    medicine: bool,
    supply: bool,
    bones: u8,
    medic: bool,
    brothers: u8,
    numb_legs: u8,
    take_cover: u8,
    focus: Focus,
}

impl Skills {
    /// Modifiers 43 and 45 of the kit; None when either is not a valid set.
    pub fn parse(medical: f32, combo: f32) -> Option<Self> {
        let medical = packed(medical, 24)?;
        let combo = packed(combo, 10)?;
        Some(Self {
            painkiller: PAINKILLER.rank(medical)?,
            bandaging: BANDAGING.rank(medical)?,
            metabolism: METABOLISM.rank(medical)?,
            training: TRAINING.rank(medical)?,
            adrenalin: ADRENALIN.rank(medical)?,
            medicine: MEDICINE.unlock(medical)?,
            supply: SUPPLY.unlock(medical)?,
            bones: BONES.rank(medical)?,
            medic: MEDIC.unlock(medical)?,
            brothers: BROTHERS.rank(medical)?,
            numb_legs: NUMB_LEGS.rank(combo)?,
            take_cover: TAKE_COVER.rank(combo)?,
            focus: Focus::parse(UNFLINCHING.rank(combo)?, CONCENTRATION.rank(combo)?)?,
        })
    }

    pub fn is_medic(self) -> bool {
        self.medic
    }
    pub fn has_adrenalin(self) -> bool {
        self.adrenalin != 0
    }
    pub fn use_time(self, native: f32) -> f32 {
        native * (1.0 - f32::from(self.bandaging) * 0.05)
    }

    /// Potent Painkiller: one percentage point of pain protection a rank,
    /// added (the hit's coefficient 0.9, 10%, becomes 0.87 at rank 3).
    pub fn pain_coefficient(self, native: f32) -> f32 {
        (native - f32::from(self.painkiller) * 0.01).max(0.0)
    }

    pub fn pain_duration_ms(self, native: f32) -> f32 {
        native + f32::from(self.painkiller) * 1500.0
    }

    pub fn regeneration_delay_ms(self, native: u32) -> u32 {
        native.saturating_sub(u32::from(self.metabolism) * 500)
    }

    pub fn movement_speed(self, native: f32, health: f32) -> f32 {
        if health > 0.0 && health < 0.3 {
            native * (1.0 + f32::from(self.adrenalin) * 0.05)
        } else {
            native
        }
    }

    pub fn limb_health(self, native: f32) -> f32 {
        native * (1.0 + f32::from(self.bones) * 0.1)
    }

    pub fn allied_damage(self, native: f32) -> f32 {
        native * (1.0 - f32::from(self.brothers) * 0.25)
    }

    /// Numb Legs: the sprint's stamina a second.
    pub fn sprint_cost(self, native: f32, painkiller: Painkiller) -> f32 {
        match painkiller {
            Painkiller::Active => native * (1.0 - f32::from(self.numb_legs) * 0.2),
            Painkiller::Inactive => native,
        }
    }

    /// Take Cover: what a first-aid kit or bandage heals at this tick.
    pub fn medicine_heal(self, native: f32, stance: Stance) -> f32 {
        match stance {
            Stance::Crouched => native * (1.0 + f32::from(self.take_cover) * 0.1),
            Stance::Standing => native,
        }
    }

    /// Unflinching: the recent hits' stopping power (the slow).
    pub fn stopping_power(self, native: f32, painkiller: Painkiller) -> f32 {
        match (painkiller, self.focus) {
            (Painkiller::Active, Focus::Unflinching(rank)) => native * (1.0 - f32::from(rank) * 0.1),
            _ => native,
        }
    }
}

/// Concentration's ranks, 1..=4 (made only from a parsed set).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rank(u8);

/// What the local player hears: Concentration while its painkiller is
/// active, else the game's volumes. The game's thread publishes it once a
/// frame ([`publish`]); the sound thread reads it for each voice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hearing {
    Plain,
    Concentrated(Rank),
}

impl Hearing {
    pub fn of(skills: Skills, painkiller: Painkiller) -> Hearing {
        match (painkiller, skills.focus) {
            (Painkiller::Active, Focus::Concentration(rank)) => Hearing::Concentrated(Rank(rank)),
            _ => Hearing::Plain,
        }
    }

    /// A voice's volume: the environment 10% quieter a rank, an opponent's
    /// sounds (not his shots) 10% louder a rank. set_volume_impl still clamps
    /// the result to [0, 1].
    pub fn volume(self, source: Source, native: f32) -> f32 {
        match (self, source) {
            (Hearing::Concentrated(Rank(rank)), Source::Environment) => native * (1.0 - f32::from(rank) * 0.1),
            (Hearing::Concentrated(Rank(rank)), Source::Opponent) => native * (1.0 + f32::from(rank) * 0.1),
            (Hearing::Plain, _) | (_, Source::Untagged) => native,
        }
    }

    fn code(self) -> u32 {
        match self {
            Hearing::Plain => 0,
            Hearing::Concentrated(Rank(rank)) => u32::from(rank),
        }
    }

    fn parse(code: u32) -> Hearing {
        match code {
            1..=4 => Hearing::Concentrated(Rank(code as u8)),
            _ => Hearing::Plain,
        }
    }
}

/// The places whose sounds are the local player's opponents': every other
/// place not on its team (vostok::medical::listener).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Opponents(u32);

impl Opponents {
    pub const NONE: Opponents = Opponents(0);

    pub fn insert(&mut self, place: Place) {
        self.0 |= 1 << place.get();
    }

    pub fn has(self, place: Place) -> bool {
        self.0 & 1 << place.get() != 0
    }
}

/// The frame's [`Hearing`] (its code) for the sound thread, and the
/// frame's [`Opponents`] for the sites that tag a sound as it is made.
static HEARING: AtomicU32 = AtomicU32::new(0);
static OPPONENTS: AtomicU32 = AtomicU32::new(0);

fn publish(hearing: Hearing, opponents: Opponents) {
    HEARING.store(hearing.code(), Ordering::Relaxed);
    OPPONENTS.store(opponents.0, Ordering::Relaxed);
}

fn hearing() -> Hearing {
    Hearing::parse(HEARING.load(Ordering::Relaxed))
}

fn opponent(place: Option<Place>) -> bool {
    place.is_some_and(|place| Opponents(OPPONENTS.load(Ordering::Relaxed)).has(place))
}
pub fn on_duration_start(ctx: &mut Context<'_>) -> Flow<NoExit> {
    medical::duration_start(ctx);
    Flow::Continue
}
pub fn on_duration_tick(ctx: &mut Context<'_>) -> Flow<NoExit> {
    medical::duration_tick(ctx);
    Flow::Continue
}
pub fn on_duration_restore(ctx: &mut Context<'_>) -> Flow<NoExit> {
    medical::duration_restore(ctx);
    Flow::Continue
}
pub fn on_use_time(ctx: &mut Context<'_>) -> Flow<NoExit> {
    medical::use_time(ctx);
    Flow::Continue
}
pub fn on_initialize(ctx: &mut Context<'_>) -> Flow<NoExit> {
    medical::initialize(ctx);
    Flow::Continue
}
/// One pool's regeneration, past its own regeneration modifiers: the kit's
/// Regeneration (on top of Metabolism's delay, set at initialization).
pub fn on_regenerate(ctx: &mut Context<'_>) -> Flow<NoExit> {
    medical::regenerate(ctx);
    Flow::Continue
}
pub fn on_wound(ctx: &mut Context<'_>) -> Flow<NoExit> {
    medical::wound(ctx);
    Flow::Continue
}
pub fn on_movement(ctx: &mut Context<'_>) -> Flow<NoExit> {
    medical::movement(ctx);
    Flow::Continue
}
pub fn on_hit(ctx: &mut Context<'_>) -> Flow<NoExit> {
    medical::hit(ctx);
    Flow::Continue
}
pub fn on_heal(ctx: &mut Context<'_>) -> Flow<NoExit> {
    medical::heal(ctx);
    Flow::Continue
}
pub fn on_artefact(ctx: &mut Context<'_>) -> Flow<NoExit> {
    medical::share_larkspur(ctx);
    Flow::Continue
}
pub fn on_protection(ctx: &mut Context<'_>) -> Flow<crate::vostok::MedicalSkillsOnProtectionExit> {
    medical::protect(ctx);
    Flow::Exit(crate::vostok::MedicalSkillsOnProtectionExit::Returned)
}

/// The native profile getter has already added base, skills and equipment.
/// Cap both the number of pockets (15) and their capacity (17) at return.
pub fn on_pocket_count(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let maximum = match ctx.reg(crate::vostok::Reg::Esi) {
        15 => 6.0,
        17 => 5.0,
        _ => return Flow::Continue,
    };
    ctx.set_xmm_f32(0, ctx.xmm_f32(0).clamp(0.0, maximum));
    Flow::Continue
}

/// Larkspur's effect: one activation reaches at most the 20 places of the
/// roster, the source among them, each with an effect of its own.
pub fn on_effect_capacity(ctx: &mut Context<'_>) -> Flow<NoExit> {
    effect::effect_capacity(ctx, c"artefact_lifebone", 20);
    Flow::Continue
}

/// Medic's shared actives end at their activation's length.
pub fn on_effect_clock(ctx: &mut Context<'_>) -> Flow<crate::vostok::MedicalSkillsOnEffectClockExit> {
    if effect::timed_clock(ctx) {
        Flow::Exit(crate::vostok::MedicalSkillsOnEffectClockExit::Returned)
    } else {
        Flow::Continue
    }
}

pub fn on_sprint(ctx: &mut Context<'_>) -> Flow<NoExit> {
    medical::sprint(ctx);
    Flow::Continue
}
pub fn on_medicine_heal(ctx: &mut Context<'_>) -> Flow<NoExit> {
    medical::medicine_heal(ctx);
    Flow::Continue
}
/// Patch 54's stagger calls this entry after Spring's factor: the one site.
pub fn on_stopping(ctx: &mut Context<'_>) -> Flow<NoExit> {
    medical::stopping(ctx);
    Flow::Continue
}

/// Concentration, once a frame after the simulation's move: the local
/// player's hearing and opponents (none without a local player).
pub fn on_listener(ctx: &mut Context<'_>) -> Flow<NoExit> {
    match medical::listener(ctx) {
        Some(local) => publish(Hearing::of(local.skills, local.painkiller), local.opponents),
        None => publish(Hearing::Plain, Opponents::NONE),
    }
    Flow::Continue
}
/// The lobby again after a match: no painkiller, no opponents.
pub fn on_lobby(_: &mut Context<'_>) -> Flow<NoExit> {
    publish(Hearing::Plain, Opponents::NONE);
    Flow::Continue
}
/// The sound thread at a voice's volume.
pub fn on_voice_gain(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(voice) = VoiceVolume::at(ctx) {
        let volume = hearing().volume(voice.source, voice.volume);
        if volume != voice.volume {
            VoiceVolume::set(ctx, volume);
        }
    }
    Flow::Continue
}
pub fn on_proxy_allocated(ctx: &mut Context<'_>) -> Flow<NoExit> {
    sound::clear_allocated(ctx);
    Flow::Continue
}
pub fn on_object_sound(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(proxy) = sound::object_sound(ctx) {
        proxy.tag(Source::Environment);
    }
    Flow::Continue
}
pub fn on_zone_sound(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(proxy) = sound::zone_sound(ctx) {
        proxy.tag(Source::Environment);
    }
    Flow::Continue
}
pub fn on_weapon_sound(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(sound) = WeaponSound::at(ctx)
        && sound.kind == WeaponSoundKind::Other
        && opponent(sound.place)
    {
        sound.proxy.tag(Source::Opponent);
    }
    Flow::Continue
}
pub fn on_foot_step(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if opponent(sound::foot_step_place(ctx)) {
        sound::begin_opponent_step(ctx.thread());
    }
    Flow::Continue
}
pub fn on_landing_step(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if opponent(sound::landing_place(ctx)) {
        sound::begin_opponent_step(ctx.thread());
    }
    Flow::Continue
}
/// After either step call, also when it played nothing.
pub fn on_foot_stepped(ctx: &mut Context<'_>) -> Flow<NoExit> {
    sound::end_step(ctx.thread());
    Flow::Continue
}
pub fn on_landing_stepped(ctx: &mut Context<'_>) -> Flow<NoExit> {
    sound::end_step(ctx.thread());
    Flow::Continue
}
pub fn on_once_point(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(point) = OncePoint::at(ctx) {
        match point.caller {
            OnceCaller::OpponentStep => point.proxy.tag(Source::Opponent),
            OnceCaller::DamageZone => point.proxy.tag(Source::Environment),
            OnceCaller::Other => {}
        }
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;
    fn medical(ranks: &[(u32, u32)]) -> f32 {
        ranks.iter().map(|&(rank, shift)| rank << shift).sum::<u32>() as f32
    }
    #[test]
    fn ranks_are_exact_and_invalid_profiles_are_refused() {
        // (shift, most) of modifier 43's ranked fields, then modifier 45's.
        for (shift, most) in [(0, 3), (3, 2), (6, 5), (9, 3), (12, 3), (17, 5), (21, 2)] {
            for rank in 0..=most {
                assert!(Skills::parse(medical(&[(rank, shift)]), 0.0).is_some());
            }
            for rank in most + 1..=7 {
                assert!(Skills::parse(medical(&[(rank, shift)]), 0.0).is_none(), "{shift} {rank}");
            }
        }
        // (shift, most) of modifier 45's fields; past the most is no set,
        // Unflinching's and Concentration's 5..7 (their third bit) too.
        for (shift, most, width) in [(0, 2, 2), (2, 2, 2), (4, 4, 3), (7, 4, 3)] {
            for rank in 0..=most {
                assert!(Skills::parse(0.0, (rank << shift) as f32).is_some());
            }
            for rank in most + 1..1 << width {
                assert!(Skills::parse(0.0, (rank << shift) as f32).is_none(), "{shift} {rank}");
            }
        }
        for invalid in [-1.0, 0.5, f32::NAN, f32::INFINITY, 16_777_216.0] {
            assert!(Skills::parse(invalid, 0.0).is_none());
        }
        for invalid in [-1.0, 0.5, f32::NAN, f32::INFINITY, 1024.0] {
            assert!(Skills::parse(0.0, invalid).is_none());
        }
        let ranks = medical(&[(3, 0), (2, 3), (5, 6), (3, 9), (3, 12), (1, 15), (1, 16), (5, 17), (2, 21)]);
        let full = Skills::parse(ranks, 74.0).unwrap();
        assert_eq!((full.numb_legs, full.take_cover, full.focus), (2, 2, Focus::Unflinching(4)));
        assert!(!full.is_medic());
        // Concentration instead: 2 + 8 + 4 x 128.
        let concentrating = Skills::parse(ranks, 522.0).unwrap();
        assert_eq!(concentrating.focus, Focus::Concentration(4));
        // Unflinching and Concentration both held is no set: refused.
        for (unflinching, concentration) in [(1, 1), (4, 4), (2, 3)] {
            let combo = (unflinching << 4) | (concentration << 7);
            assert!(Skills::parse(0.0, combo as f32).is_none(), "{combo}");
        }
    }
    #[test]
    fn top_rank_effects_and_thresholds() {
        let s = Skills {
            painkiller: 3,
            bandaging: 2,
            metabolism: 5,
            adrenalin: 3,
            bones: 5,
            brothers: 2,
            ..Skills::default()
        };
        assert!((s.pain_coefficient(0.9) - 0.87).abs() < 0.000001);
        for (rank, want) in [(0, 0.9), (1, 0.89), (2, 0.88), (3, 0.87)] {
            let s = Skills { painkiller: rank, ..Skills::default() };
            assert!((s.pain_coefficient(0.9) - want).abs() < 1e-6, "{rank}");
        }
        // Never below no pain at all.
        assert_eq!(s.pain_coefficient(0.02), 0.0);
        assert_eq!(Skills::default().pain_coefficient(0.9), 0.9);
        assert_eq!(s.pain_duration_ms(10_000.0), 14_500.0);
        assert!((s.use_time(1000.0) - 900.0).abs() < 0.001);
        assert_eq!(s.regeneration_delay_ms(8_000), 5_500);
        assert_eq!(s.regeneration_delay_ms(1_000), 0);
        assert_eq!(s.movement_speed(4.0, 0.3), 4.0);
        assert!((s.movement_speed(4.0, 0.299) - 4.6).abs() < 0.00001);
        assert!((s.movement_speed(4.0, 0.199) - 4.6).abs() < 0.00001);
        assert_eq!(s.movement_speed(4.0, 0.0), 4.0);
        assert_eq!(s.limb_health(0.5), 0.75);
        assert_eq!(s.allied_damage(0.6), 0.3);
    }
    #[test]
    fn regeneration_starts_sooner_and_goes_to_full_health() {
        // The booster: -2 s, +100 % of the limit; Metabolism's 5 ranks are
        // already in the pool's delay (8 s - 2.5 s).
        let booster = Regeneration::parse(-2.0, 1.0).unwrap();
        assert_eq!(booster.apply(0.03, 8_000, 0.5), (6_000, 1.0));
        assert_eq!(booster.apply(0.03, 5_500, 0.2), (3_500, 1.0));
        assert_eq!(booster.apply(0.1, 8_000, 1.0), (6_000, 1.0));
        assert_eq!(booster.apply(0.03, 1_000, 0.5), (0, 1.0));
        // A pool without regeneration keeps its own.
        assert_eq!(booster.apply(0.0, 0, 0.0), (0, 0.0));
        // No items: the native's.
        assert_eq!(Regeneration::parse(0.0, 0.0).unwrap().apply(0.03, 8_000, 0.2), (8_000, 0.2));
        assert_eq!(Regeneration::default().apply(0.03, 8_000, 0.5), (8_000, 0.5));
        // Two of them never pass full health; part of the way stays part.
        assert_eq!(Regeneration::parse(-4.0, 2.0).unwrap().apply(0.03, 8_000, 0.5), (4_000, 1.0));
        assert_eq!(Regeneration::parse(0.0, 0.25).unwrap().apply(0.03, 8_000, 0.5), (8_000, 0.75));
        for (delay, limit) in [(0.5, 0.0), (-61.0, 0.0), (f32::NAN, 0.0), (0.0, -0.1), (0.0, f32::INFINITY)] {
            assert!(Regeneration::parse(delay, limit).is_none(), "{delay} {limit}");
        }
    }
    #[test]
    fn combo_skills_need_their_condition() {
        for rank in 0..=2u8 {
            let s = Skills { numb_legs: rank, take_cover: rank, ..Skills::default() };
            let r = f32::from(rank);
            assert_eq!(s.sprint_cost(10.0, Painkiller::Inactive), 10.0);
            assert!((s.sprint_cost(10.0, Painkiller::Active) - 10.0 * (1.0 - 0.2 * r)).abs() < 1e-5);
            assert_eq!(s.medicine_heal(0.5, Stance::Standing), 0.5);
            assert!((s.medicine_heal(0.5, Stance::Crouched) - 0.5 * (1.0 + 0.1 * r)).abs() < 1e-6);
        }
        for rank in 0..=4u8 {
            let s = Skills { focus: Focus::parse(rank, 0).unwrap(), ..Skills::default() };
            let r = f32::from(rank);
            assert_eq!(s.stopping_power(0.4, Painkiller::Inactive), 0.4);
            assert!((s.stopping_power(0.4, Painkiller::Active) - 0.4 * (1.0 - 0.1 * r)).abs() < 1e-6);
        }
        // Four ranks of 10%: the same -40% the two ranks of 20% gave.
        let top = Skills { focus: Focus::Unflinching(4), ..Skills::default() };
        assert!((top.stopping_power(1.0, Painkiller::Active) - 0.6).abs() < 1e-6);
        // Concentration is no Unflinching.
        let ears = Skills { focus: Focus::Concentration(4), ..Skills::default() };
        assert_eq!(ears.stopping_power(0.4, Painkiller::Active), 0.4);
    }
    #[test]
    fn concentration_needs_the_painkiller_and_changes_only_tagged_sounds() {
        for rank in 0..=4u8 {
            let s = Skills { focus: Focus::parse(0, rank).unwrap(), ..Skills::default() };
            let r = f32::from(rank);
            assert_eq!(Hearing::of(s, Painkiller::Inactive), Hearing::Plain);
            let hearing = Hearing::of(s, Painkiller::Active);
            assert_eq!(hearing == Hearing::Plain, rank == 0);
            assert_eq!(Hearing::parse(hearing.code()), hearing);
            assert!((hearing.volume(Source::Environment, 0.5) - 0.5 * (1.0 - 0.1 * r)).abs() < 1e-6);
            assert!((hearing.volume(Source::Opponent, 0.5) - 0.5 * (1.0 + 0.1 * r)).abs() < 1e-6);
            assert_eq!(hearing.volume(Source::Untagged, 0.5), 0.5);
            for painkiller in [Painkiller::Inactive, Painkiller::Active] {
                let other = Skills { focus: Focus::parse(rank, 0).unwrap(), ..Skills::default() };
                assert_eq!(Hearing::of(other, painkiller), Hearing::Plain);
            }
        }
        for code in [0, 5, 7, u32::MAX] {
            assert_eq!(Hearing::parse(code), Hearing::Plain);
        }
        let mut opponents = Opponents::NONE;
        let places: std::vec::Vec<Place> = Place::all().collect();
        opponents.insert(places[3]);
        opponents.insert(places[19]);
        assert!(opponents.has(places[3]) && opponents.has(places[19]));
        assert!(!opponents.has(places[0]) && !opponents.has(places[4]));
    }
}
