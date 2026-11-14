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

    /// The fair rule: a spawn at 10 s with the points' 6 s is protected to 16 s inclusive; the exact rule the world's end.
    #[test]
    fn a_spawn_is_protected_for_the_points_time() {
        assert_eq!(fully_until(false, Some(20_000), Some(10_000), Some(6_000)), Some(16_001));
        assert_eq!(fully_until(true, Some(20_000), Some(10_000), Some(6_000)), Some(20_001));
        assert_eq!(fully_until(false, None, None, Some(6_000)), None);
        assert_eq!(fully_until(false, None, Some(10_000), None), None);
    }
}
