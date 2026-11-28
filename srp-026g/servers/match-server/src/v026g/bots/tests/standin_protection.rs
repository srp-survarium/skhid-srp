//! [Gap 8, patch 179] The stand-in's spawn protection, as the game's damage model keeps it (`protection.rs`, PROVEN in
//! the exe):
//!
//! - **The end and the fade:** the damage model's `m_invulnerability_time_in_ms` (+32Ch, -1 none) and
//!   `m_invulnerability_attenuation` (+330h) ([`Shield`]); `damage_model::get_invulnerability` 0x2E5550 the factor
//!   ([`super::super::protection::factor`]).
//! - **A spawn** at a respawn point: the end the spawn time plus the point's `invulnerability_time`, no fade
//!   (0x316E72..0x316E96; [`survarium::v026g::maps::MapMode::spawn_protection_ms`], 6 s on the shipped maps).
//! - **A team's base** (`effect_zone_core::apply_invulnerability` 0x31ACA0): every tick its own team's living bodies
//!   inside it get the end now plus `invulnerability_duration` and the fade `invulnerability_attenuation`
//!   (0x31AD03..0x31AD15) - unless the end is -1: exe patch 179's `on_zone_protect` skips them
//!   (`client-patches/rust/patches/src/shot_protection.rs:24`, the skip at `effect_zone.protect` 0x31AD0F).
//! - **A gun's shot** (exe patch 179, `shot_protection.rs:16` `on_fired`, at `weapon_stats.fired` 0x2D7843 in
//!   `base_player::on_fire` 0x2D7800, every shot of `weapon_core::instant_fire` 0x2E23A0): the shooter's end -1 until his
//!   next spawn (no grenade, no melee: the stand-in fires only guns).
//! - **A hit** (`hit_impl` 0x2D9B90): dropped while the factor is 1 or more (0x2D9BBE..0x2D9BCB), so it does nothing at all;
//!   otherwise `hit_body_part` 0x2E5080 takes its damage times `1 - factor` (0x2E50A8..0x2E50B7), before the zone's armour
//!   and the pools. The stand-in applies it to its bullets and its grenades' blasts ([`Shield::scale`]).

use super::super::protection::{self, Base};
use mapdata::Vec3;

/// A body's protection as its damage model holds it: `end` `None` for the game's -1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Shield {
    end: Option<u32>,
    fade_ms: u32,
}

impl Shield {
    /// A spawn at `at` ms on a point of `point_ms` protection (a missing key is 0, `MapMode::spawn_protection_ms`).
    pub(crate) fn spawned(at: u32, point_ms: u32) -> Self {
        Self { end: Some(at.saturating_add(point_ms)), fade_ms: 0 }
    }

    /// A tick at `now` inside a base of its own team: the zone moves the end on, unless a shot ended it.
    pub(crate) fn in_base(&mut self, base: &Base, now: u32) {
        if self.end.is_some() {
            *self = Self { end: Some(now.saturating_add(base.duration_ms)), fade_ms: base.fade_ms };
        }
    }

    /// It fired a gun: the protection ends, whole, until the next spawn.
    pub(crate) fn fired(&mut self) {
        self.end = None;
    }

    /// `get_invulnerability` at `now`: 1 fully protected, 0 not.
    pub(crate) fn factor(&self, now: u32) -> f32 {
        protection::factor(self.end, self.fade_ms, now)
    }

    /// What a hit at `now` leaves of its damage: `None` for a hit the game drops (fully protected), else `1 - factor`.
    pub(crate) fn scale(&self, now: u32) -> Option<f32> {
        let factor = self.factor(now);
        (factor < 1.0).then_some(1.0 - factor)
    }
}

/// The match's protection rule for the stand-in: the points' time by team and the bases.
#[derive(Clone, Debug, Default)]
pub(crate) struct Rule {
    pub(crate) spawn_ms: [Option<u32>; 2],
    pub(crate) bases: Vec<Base>,
}

impl Rule {
    /// The shield of a body of `team` that spawned at `at`.
    pub(crate) fn spawn(&self, team: u8, at: u32) -> Shield {
        Shield::spawned(at, self.spawn_ms.get(usize::from(team & 1)).copied().flatten().unwrap_or(0))
    }

    /// A tick of a living body of `team` with its feet at `feet`: each of its own team's bases it is inside.
    pub(crate) fn tick(&self, shield: &mut Shield, team: u8, feet: Vec3, now: u32) {
        for base in self.bases.iter().filter(|base| base.team == team && base.contains(feet)) {
            shield.in_base(base, now);
        }
    }
}

/// [measure] The match's protection in the stand-in: hits dropped by it, hits it scaled, protections a shot ended while
/// they still protected (a factor over 0).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Counts {
    pub(crate) dropped: u32,
    pub(crate) scaled: u32,
    pub(crate) ended_by_shot: u32,
}

impl Counts {
    pub(crate) fn line(&self) -> String {
        format!(
            "stand-in protection: hits dropped {} hits faded {} protections ended by a shot {}",
            self.dropped, self.scaled, self.ended_by_shot
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mapdata::Rotation;
    use mapdata::project::{Shape, ShapeKind};

    fn base(team: u8) -> Base {
        let shape = Shape { kind: ShapeKind::Sphere, center: Vec3::ZERO, rotation: Rotation::IDENTITY, half: Vec3::new(5.0, 5.0, 5.0) };
        Base { team, duration_ms: 2_000, fade_ms: 2_000, shapes: vec![shape] }
    }

    /// A spawn protects for the points' time, whole, then not at all; a hit then is dropped, after it taken whole.
    #[test]
    fn a_spawn_protects_for_the_points_time() {
        let shield = Rule { spawn_ms: [Some(6_000), Some(500)], bases: vec![] }.spawn(0, 10_000);
        assert_eq!(shield.scale(16_000), None, "to the end inclusive");
        assert_eq!(shield.scale(16_001), Some(1.0), "no fade");
        assert_eq!(Rule { spawn_ms: [Some(6_000), Some(500)], bases: vec![] }.spawn(1, 10_000).scale(10_501), Some(1.0));
    }

    /// The base moves the end on every tick inside it, with its fade after; a shot ends it and no base gives it back
    /// until the next spawn.
    #[test]
    fn a_shot_ends_it_and_the_base_never_gives_it_back() {
        let rule = Rule { spawn_ms: [Some(6_000); 2], bases: vec![base(0), base(1)] };
        let mut shield = rule.spawn(0, 10_000);
        rule.tick(&mut shield, 0, Vec3::new(1.0, 0.0, 0.0), 20_000);
        assert_eq!(shield.scale(22_000), None);
        assert_eq!(shield.scale(23_000), Some(0.5), "half way through the fade");
        let mut other = shield;
        rule.tick(&mut other, 0, Vec3::new(50.0, 0.0, 0.0), 21_000);
        assert_eq!(other, shield, "outside the base: unchanged");
        shield.fired();
        assert_eq!(shield.scale(20_000), Some(1.0), "ended at once, whole");
        rule.tick(&mut shield, 0, Vec3::new(1.0, 0.0, 0.0), 20_050);
        assert_eq!(shield.scale(20_100), Some(1.0), "the base gives none back");
        assert_eq!(rule.spawn(0, 30_000).scale(30_100), None, "the next spawn protects again");
    }
}
