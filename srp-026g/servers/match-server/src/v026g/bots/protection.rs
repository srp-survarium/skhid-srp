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

use super::view::Place;

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

/// [patch 179] What one bot knows of its enemies' gun shots, and the lives whose protection a known shot ended.
///
/// Exe patch 179 (`client-patches/rust/patches/src/shot_protection.rs:16` `on_fired`, PROVEN in its code, Unicorn-checked,
/// `docs/notes/shot-protection.md`): a gun's shot (`weapon_stats.fired` 0x2D7843 in `base_player::on_fire` 0x2D7800, which
/// `weapon_core::instant_fire` 0x2E23A0 calls for every shot at 0x2E258D) sets the shooter's end to -1: his protection
/// ends at once and whole, with no fade, until his next spawn; a base's zone never starts an ended one
/// (`shot_protection.rs:24` `on_zone_protect`, the skip at `effect_zone.protect` 0x31AD0F). A melee strike
/// (`weapon_core_melee_state_base::on_shoot` 0x33AB50 emits its bullet without on_fire), a thrown grenade and a battery's
/// strike are not shots. So the fair rule counts only an enemy's fire (the FIRE key, bit 0x20, `player_bots.md`'s input
/// table; melee is bit 0x8000000 and a throw 0x800000) that the bot itself heard or saw: never the team's word or a
/// cheat.
#[derive(Clone, Debug, Default)]
pub struct ShotsKnown {
    /// Each enemy's latest gun shot the bot heard or saw (match ms).
    last: Vec<(Place, u32)>,
    /// The lives (enemy, his life's start) whose protection a known shot ended, each counted once.
    ended: Vec<(Place, u32)>,
}

impl ShotsKnown {
    /// The bot heard or saw `who` fire a gun at `time`.
    pub fn fired(&mut self, who: Place, time: u32) {
        match self.last.iter_mut().find(|(place, _)| *place == who) {
            Some((_, last)) => *last = (*last).max(time),
            None => self.last.push((who, time)),
        }
    }

    /// Whether a known shot ended the protection of the life that started at `spawned`.
    fn ends(&self, who: Place, spawned: Option<u32>) -> bool {
        let shot = self.last.iter().find(|(place, _)| *place == who).map(|&(_, time)| time);
        matches!((spawned, shot), (Some(spawned), Some(shot)) if shot >= spawned)
    }

    /// Takes off `list` (enemy, until) each enemy whose present life (its start by `spawned`) has a known shot: the
    /// spawn's entries and the base's alike, since no base gives an ended one back. Returns the lives ended for the
    /// first time.
    pub fn end_protection(
        &mut self,
        list: &mut Vec<(Place, u32)>,
        spawned: impl Fn(Place) -> Option<u32>,
    ) -> u32 {
        self.ended.retain(|&(who, life)| spawned(who) == Some(life));
        let mut new = 0;
        list.retain(|&(who, _)| {
            let life = spawned(who);
            if !self.ends(who, life) {
                return true;
            }
            if let Some(life) = life
                && !self.ended.contains(&(who, life))
            {
                self.ended.push((who, life));
                new += 1;
            }
            false
        });
        new
    }
}

/// [patch 179] [measure] Ticks a bot's shot was held off a protected target, all and those on a target that truly fired
/// in his life (the bench's true state, read for the count only), and the "before" switch of the bench's A/B.
#[cfg(test)]
pub mod held {
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};

    static TARGETED: AtomicU64 = AtomicU64::new(0);
    static HELD: AtomicU64 = AtomicU64::new(0);
    static ON_SHOOTER: AtomicU64 = AtomicU64::new(0);
    /// The bench's "before" arm: the shot rule off (`SRP_BENCH_SHOT_RULE=off`).
    pub static SHOT_RULE_OFF: AtomicBool = AtomicBool::new(false);

    /// A reflex tick whose target was protected, its trigger pressed or not.
    pub fn targeted() {
        TARGETED.fetch_add(1, Relaxed);
    }

    pub fn count(on_shooter: bool) {
        HELD.fetch_add(1, Relaxed);
        if on_shooter {
            ON_SHOOTER.fetch_add(1, Relaxed);
        }
    }

    pub fn line() -> String {
        format!(
            "ticks on a protected target {} held ticks {} on a shooter {} shot rule {}",
            TARGETED.load(Relaxed),
            HELD.load(Relaxed),
            ON_SHOOTER.load(Relaxed),
            if SHOT_RULE_OFF.load(Relaxed) { "off" } else { "on" }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [patch 179] An enemy heard or seen firing in his present life is no longer protected (the spawn's entry and the
    /// base's), each life counted once; a shot before his present life ends nothing; an enemy who has not fired keeps his.
    #[test]
    fn a_known_shot_ends_the_shooters_protection_for_his_life() {
        let mut shots = ShotsKnown::default();
        // Enemy 3 spawned at 10 s, enemy 4 at 12 s, enemy 5 at 40 s (his earlier life fired at 20 s).
        let spawned = |who| match who {
            3 => Some(10_000),
            4 => Some(12_000),
            5 => Some(40_000),
            _ => None,
        };
        shots.fired(5, 20_000);
        let mut list = vec![(3, 16_001), (4, 18_001), (5, 46_001)];
        assert_eq!(shots.end_protection(&mut list, spawned), 0);
        assert_eq!(list, vec![(3, 16_001), (4, 18_001), (5, 46_001)], "no shot in a present life");
        shots.fired(3, 11_500);
        shots.fired(3, 11_000);
        assert_eq!(shots.end_protection(&mut list, spawned), 1);
        assert_eq!(list, vec![(4, 18_001), (5, 46_001)], "3 fired after his spawn");
        // The next world gives 3 a base entry again: still ended, not counted twice.
        let mut list = vec![(3, 14_001), (4, 18_001), (5, 46_001)];
        assert_eq!(shots.end_protection(&mut list, spawned), 0);
        assert_eq!(list, vec![(4, 18_001), (5, 46_001)], "no base gives an ended protection back");
        // 3's next life is protected again until he fires in it.
        let respawned = |who| if who == 3 { Some(30_000) } else { spawned(who) };
        let mut list = vec![(3, 36_001)];
        assert_eq!(shots.end_protection(&mut list, respawned), 0);
        assert_eq!(list, vec![(3, 36_001)], "a new life");
        shots.fired(3, 31_000);
        assert_eq!(shots.end_protection(&mut list, respawned), 1, "the new life counted once");
        assert!(list.is_empty());
    }

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
