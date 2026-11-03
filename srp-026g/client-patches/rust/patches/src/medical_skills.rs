//! Medical skill decisions. The existing profile modifier 43 transports an
//! exact 24-bit integer; no new world state or snapshot side table is needed.
#![forbid(unsafe_code)]

use crate::vostok::{Context, Flow, NoExit, medical};

/// A rank is parsed once from the profile, never an arbitrary float inside
/// the gameplay calculation. Seven five-rank skills and three unlocks.
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
}

impl Skills {
    pub fn parse(value: f32) -> Option<Self> {
        if !(0.0..=16_777_215.0).contains(&value) || value != (value as u32) as f32 {
            return None;
        }
        let bits = value as u32;
        let rank = |shift: u32| ((bits >> shift) & 7u32) as u8;
        let result = Self {
            painkiller: rank(0),
            bandaging: rank(3),
            metabolism: rank(6),
            training: rank(9),
            adrenalin: rank(12),
            medicine: bits & (1 << 15) != 0,
            supply: bits & (1 << 16) != 0,
            bones: rank(17),
            medic: bits & (1 << 20) != 0,
            brothers: rank(21),
        };
        [
            result.painkiller,
            result.bandaging,
            result.metabolism,
            result.training,
            result.adrenalin,
            result.bones,
            result.brothers,
        ]
        .iter()
        .all(|&rank| rank <= 5)
        .then_some(result)
    }

    pub fn is_medic(self) -> bool {
        self.medic
    }
    pub fn has_adrenalin(self) -> bool {
        self.adrenalin != 0
    }
    pub fn use_time(self, native: f32) -> f32 {
        native * (1.0 - f32::from(self.bandaging) * 0.02)
    }

    pub fn pain_coefficient(self, native: f32) -> f32 {
        if self.painkiller == 0 {
            return native;
        }
        (1.0 - (1.0 - native) * (1.0 + f32::from(self.painkiller) * 0.05)).max(0.0)
    }

    pub fn pain_duration_ms(self, native: f32) -> f32 {
        native + f32::from(self.painkiller) * 1000.0
    }

    pub fn regeneration_delay_ms(self, native: u32) -> u32 {
        native.saturating_sub(u32::from(self.metabolism) * 500)
    }

    pub fn movement_speed(self, native: f32, health: f32) -> f32 {
        if health > 0.0 && health < 0.2 {
            native * (1.0 + f32::from(self.adrenalin) * 0.03)
        } else {
            native
        }
    }

    pub fn limb_health(self, native: f32) -> f32 {
        native * (1.0 + f32::from(self.bones) * 0.1)
    }

    pub fn allied_damage(self, native: f32) -> f32 {
        native * (1.0 - f32::from(self.brothers) * 0.1)
    }
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

pub fn on_effect_capacity(ctx: &mut Context<'_>) -> Flow<NoExit> {
    medical::effect_capacity(ctx);
    Flow::Continue
}

pub fn on_effect_clock(ctx: &mut Context<'_>) -> Flow<crate::vostok::MedicalSkillsOnEffectClockExit> {
    if medical::effect_clock(ctx) {
        Flow::Exit(crate::vostok::MedicalSkillsOnEffectClockExit::Returned)
    } else {
        Flow::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ranks_are_exact_and_invalid_profiles_are_refused() {
        for shift in [0, 3, 6, 9, 12, 17, 21] {
            for rank in 0..=5 {
                assert!(Skills::parse((rank << shift) as f32).is_some());
            }
            assert!(Skills::parse((6 << shift) as f32).is_none());
        }
        for invalid in [-1.0, 0.5, f32::NAN, f32::INFINITY, 16_777_216.0] {
            assert!(Skills::parse(invalid).is_none());
        }
    }
    #[test]
    fn rank_five_effects_and_thresholds() {
        let s = Skills {
            painkiller: 5,
            metabolism: 5,
            adrenalin: 5,
            bones: 5,
            brothers: 5,
            ..Skills::default()
        };
        assert!((s.pain_coefficient(0.9) - 0.875).abs() < 0.000001);
        assert_eq!(s.pain_duration_ms(10_000.0), 15_000.0);
        assert_eq!(s.regeneration_delay_ms(8_000), 5_500);
        assert_eq!(s.regeneration_delay_ms(1_000), 0);
        assert_eq!(s.movement_speed(4.0, 0.2), 4.0);
        assert_eq!(s.movement_speed(4.0, 0.199), 4.6);
        assert_eq!(s.movement_speed(4.0, 0.0), 4.0);
        assert_eq!(s.limb_health(0.5), 0.75);
        assert_eq!(s.allied_damage(0.6), 0.3);
    }
}
