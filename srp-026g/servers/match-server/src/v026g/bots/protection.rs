//! [Gap 8] Spawn protection: when an enemy cannot be hurt (exactness 2026-11-14, PROVEN in the exe).
//!
//! - `damage_model::get_invulnerability` 0x2E5550, with `now` the damage model's last update (+328h), `end` its
//!   `m_invulnerability_time_in_ms` (+32Ch) and `fade` its `m_invulnerability_attenuation` (+330h): `end` none -> 0;
//!   `now <= end` (unsigned) -> 1; `now > end + fade` -> 0; else `1 - (now - end) / fade`.
//! - `hit_impl` 0x2D9B90 drops a hit while it is 1 or more (0x2D9BBE..0x2D9BCB); `hit_body_part` 0x2E5080 takes the
//!   hit's damage times `1 - it` (0x2E50A8..0x2E50B7).
//! - Two sources set it: a spawn at a respawn point (end = the spawn time + the point's `invulnerability_time`, no fade,
//!   0x316E72..0x316E96), and in the battery modes a team's base `effect_zone` while its own body is inside (end = now
//!   + the zone's `invulnerability_duration`, fade its `invulnerability_attenuation`, every tick, 0x31AD03..0x31AD15).
//! - Nothing draws it for a player: a player knows the rule (a body just spawned, or in his own base) and sees where
//!   an enemy is. So the bots take an enemy's protection by the rule a player can apply (his life's start and the
//!   points' time), and the world's own end and fade only with `cheats.enemy_protection_exact`.

/// A team's base: an `effect_zone` of the map with its team and its protection (`mapdata::project::Protection`, kept in
/// the bots' maps since #1880): while a body of that team is inside it, the zone sets his end to the time now plus
/// `duration_ms` every tick (`effect_zone_core::apply_invulnerability` 0x31ACA0, 0x31AD03..0x31AD15).
#[derive(Clone, Debug, PartialEq)]
pub struct Base {
    pub team: u8,
    pub duration_ms: u32,
    pub shapes: Vec<mapdata::project::Shape>,
}

impl Base {
    /// The bases of a map: its effect zones of a team (0 or 1) with a protection.
    pub fn of_map(map: &mapdata::project::MapData) -> Vec<Self> {
        map.zones
            .iter()
            .filter_map(|zone| {
                let protection = zone.protection?;
                let team = u8::try_from(zone.team).ok().filter(|&team| team < 2)?;
                Some(Self { team, duration_ms: protection.duration_ms, shapes: zone.shapes.clone() })
            })
            .collect()
    }

    pub fn contains(&self, at: mapdata::Vec3) -> bool {
        self.shapes.iter().any(|shape| shape.contains(at))
    }
}

/// Until when (exclusive, match ms) a body of `team` known at `at` at `time` (a player's own senses: seen, heard, told)
/// is fully protected by his team's base: inside one of its bases at `time`, his end is at least `time` plus the base's
/// duration (the zone sets it every tick he is inside), protected up to it inclusive. `None`: in none of his bases.
pub fn base_until(bases: &[Base], team: u8, at: mapdata::Vec3, time: u32) -> Option<u32> {
    bases
        .iter()
        .filter(|base| base.team == team && base.contains(at))
        .map(|base| time.saturating_add(base.duration_ms).saturating_add(1))
        .max()
}

/// `get_invulnerability` at `now` for an end `end` and a fade `fade` ms: 1 fully protected, 0 not.
pub fn factor(end: Option<u32>, fade: u32, now: u32) -> f32 {
    let Some(end) = end else {
        return 0.0;
    };
    if now <= end {
        return 1.0;
    }
    if fade == 0 || now > end.saturating_add(fade) {
        return 0.0;
    }
    1.0 - (now - end) as f32 / fade as f32
}

/// Until when (exclusive, match ms) an enemy is fully protected: with `exact` his world's end (`now <= end`), else his
/// life's start `spawned` plus his team's points' protection `point_ms` (a spawn's end, no fade). `None`: not protected
/// by that rule.
pub fn fully_until(exact: bool, world_end: Option<u32>, spawned: Option<u32>, point_ms: Option<u32>) -> Option<u32> {
    let end = if exact { world_end } else { spawned.zip(point_ms).map(|(spawned, ms)| spawned.saturating_add(ms)) };
    end.map(|end| end.saturating_add(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The client's factor: full to the end inclusive, then the fade's line, then nothing; no fade ends it at once.
    #[test]
    fn the_factor_is_the_clients() {
        assert_eq!(factor(None, 2_000, 5_000), 0.0);
        assert_eq!(factor(Some(5_000), 2_000, 5_000), 1.0);
        assert_eq!(factor(Some(5_000), 2_000, 5_500), 0.75);
        assert_eq!(factor(Some(5_000), 2_000, 7_000), 0.0);
        assert_eq!(factor(Some(5_000), 2_000, 7_001), 0.0);
        assert_eq!(factor(Some(5_000), 0, 5_001), 0.0);
    }

    /// [item 4] A body known inside his own team's base at 30 s is protected to 30 s + the duration inclusive; outside it,
    /// or inside the other team's base, not by this rule.
    #[test]
    fn a_body_known_in_his_base_is_protected_for_its_duration() {
        use mapdata::project::{Shape, ShapeKind};
        use mapdata::{Rotation, Vec3};
        let sphere = |x: f32| Shape { kind: ShapeKind::Sphere, center: Vec3::new(x, 0.0, 0.0), rotation: Rotation::IDENTITY, half: Vec3::new(5.0, 5.0, 5.0) };
        let bases = vec![Base { team: 0, duration_ms: 2_000, shapes: vec![sphere(0.0)] }, Base { team: 1, duration_ms: 2_000, shapes: vec![sphere(100.0)] }];
        assert_eq!(base_until(&bases, 0, Vec3::new(1.0, 0.0, 0.0), 30_000), Some(32_001));
        assert_eq!(base_until(&bases, 0, Vec3::new(50.0, 0.0, 0.0), 30_000), None);
        assert_eq!(base_until(&bases, 1, Vec3::new(1.0, 0.0, 0.0), 30_000), None, "the other team's base");
    }

    /// The fair rule: a spawn at 10 s with the points' 6 s is protected to 16 s inclusive; the exact rule the world's end.
    #[test]
    fn a_spawn_is_protected_for_the_points_time() {
        assert_eq!(fully_until(false, Some(20_000), Some(10_000), Some(6_000)), Some(16_001));
        assert_eq!(fully_until(true, Some(20_000), Some(10_000), Some(6_000)), Some(20_001));
        assert_eq!(fully_until(false, None, None, Some(6_000)), None);
        assert_eq!(fully_until(false, None, Some(10_000), None), None);
    }
}
