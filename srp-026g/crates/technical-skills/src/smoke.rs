//! The smoke grenade's cloud (docs/notes/technical-skills.md, "Dense Smoke and Toxic Smoke"): Dense
//! Smoke (402) holds the cloud longer, Toxic Smoke (441) poisons the enemies inside it. The client
//! has no cloud object, only the particle system `fx/explosions/smoke_grenade` the blast plays; its
//! timeline and shape are read off that system (the stock resources.db) and kept here, once, for
//! exe patch 180 and the match server's bots alike. Pure Rust.
//!
//! The particle system: 55 puffs placed at 0 and 0.2 s (a cylinder of radius 8 m and height 2 m,
//! 1.5 m up, and a sphere of 1 m), no velocity, nearly no spin; each lives 29 s, full from 1.9 s,
//! thinning from 17.5 s. Since the puffs are still, pausing the instance from [`HOLD_FROM_MS`]
//! holds the full cloud as it is: Dense Smoke pauses it for [`Skills::smoke_extra_ms`].

use crate::{Clock, Skills};

/// A puff's life in the particle system (its `LifeTime`), ms.
pub const PARTICLE_LIFE_MS: u32 = 29_000;
/// When, after the blast, Dense Smoke's pause starts: the cloud is full (1.9 s) and still.
pub const HOLD_FROM_MS: u32 = 2_000;
/// When, after the blast, the cloud is full and starts to poison (its puffs' fade-in, 0.0646 of
/// 29 s).
pub const TOXIC_FROM_MS: u32 = 1_900;
/// When, after the blast, a cloud no Dense Smoke held is gone (the bots' reading of the particle
/// system: shrinking from 17.5 s, gone at 26 s).
pub const GONE_MS: u32 = 26_000;

/// The cloud's shape around the grenade's resting point (the blast's centre): radius, metres.
pub const RADIUS: f32 = 8.0;
/// How far below and above the centre the cloud reaches, metres (the bots' model: its top 5 m
/// above the rest point, 1.3 m below).
pub const BELOW: f32 = 1.3;
pub const ABOVE: f32 = 5.0;
/// A player's eyes above its feet, metres: what is inside the cloud (the gas zones hit the face).
pub const EYE_HEIGHT: f32 = 1.6;

/// Toxic Smoke's damage a second to an enemy's face, by rank (the stock gas zones: 0.07 to 0.25;
/// fruit cloud level 1 is 0.07): 0.05, 0.06, 0.07 (the owner, 2026-11-21).
/// The face's health is 1.
const TOXIC_RATE: [f32; 4] = [0.0, 0.05, 0.06, 0.07];

impl Skills {
    /// Dense Smoke (402): how much longer its cloud lasts, ms: its rank's tenth of a puff's life
    /// ((smoke_duration - 1) x 29 s, in whole ms).
    pub fn smoke_extra_ms(self) -> u32 {
        u32::from(self.dense_smoke.get()) * (PARTICLE_LIFE_MS / 10)
    }

    /// Toxic Smoke (441): its smoke's toxic damage a second to an enemy's face (0 without it).
    pub fn toxic_rate(self) -> f32 {
        self.toxic_smoke.pick(TOXIC_RATE)
    }
}

/// A toxic cloud: the blast's simulation time and its centre (the grenade where it went off). What
/// exe patch 180 keeps on the grenade object (saved with it) from the blast of a smoke grenade
/// whose thrower has Toxic Smoke until that grenade object goes off again.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cloud {
    pub blast: Clock,
    pub centre: [f32; 3],
}

impl Cloud {
    /// Its saved bytes: u32 the blast's ms, f32 x3 the centre (little-endian).
    pub const BYTES: usize = 16;

    pub fn to_bytes(self) -> [u8; Self::BYTES] {
        let mut out = [0; Self::BYTES];
        out[..4].copy_from_slice(&self.blast.0.to_le_bytes());
        for (at, value) in self.centre.iter().enumerate() {
            out[4 + 4 * at..8 + 4 * at].copy_from_slice(&value.to_le_bytes());
        }
        out
    }

    /// The saved bytes; none when the centre is not three finite numbers.
    pub fn parse(bytes: [u8; Self::BYTES]) -> Option<Self> {
        let word = |at: usize| [bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]];
        let centre = [4, 8, 12].map(|at| f32::from_le_bytes(word(at)));
        centre.iter().all(|v| v.is_finite()).then_some(Self { blast: Clock(u32::from_le_bytes(word(0))), centre })
    }

    /// The milliseconds of `from..to` (a tick's) in which the cloud of a thrower with `skills`
    /// poisons: from [`TOXIC_FROM_MS`] after the blast to [`GONE_MS`] plus Dense Smoke's extra.
    /// Integers on the match's wrapping clock (two times less than half the wrap apart).
    pub fn toxic_ms(self, skills: Skills, from: Clock, to: Clock) -> u32 {
        let since = |t: Clock| i64::from(t.0.wrapping_sub(self.blast.0) as i32);
        let (start, end) = (i64::from(TOXIC_FROM_MS), i64::from(GONE_MS) + i64::from(skills.smoke_extra_ms()));
        let clamp = |t: i64| t.clamp(start, end);
        (clamp(since(to)) - clamp(since(from))).max(0) as u32
    }

    /// The toxic damage to an enemy's face in `from..to` (0 without Toxic Smoke or outside its
    /// time).
    pub fn toxic_dose(self, skills: Skills, from: Clock, to: Clock) -> f32 {
        skills.toxic_rate() * self.toxic_ms(skills, from, to) as f32 / 1000.0
    }

    /// Whether the cloud of a thrower with `skills` is toxic at `now`: the time [`Self::toxic_ms`]
    /// counts.
    pub fn toxic_at(self, skills: Skills, now: Clock) -> bool {
        self.toxic_ms(skills, now, now.after(1)) > 0
    }

    /// Whether a player standing at `feet` has its eyes in the cloud.
    pub fn inside(self, feet: [f32; 3]) -> bool {
        let (dx, dz) = (feet[0] - self.centre[0], feet[2] - self.centre[2]);
        let up = feet[1] + EYE_HEIGHT - self.centre[1];
        dx * dx + dz * dz <= RADIUS * RADIUS && (-BELOW..=ABOVE).contains(&up)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skills(dense: u32, toxic: u32) -> Skills {
        Skills::parse((dense * crate::weight(1) + toxic * crate::weight(13)) as f32).unwrap()
    }

    #[test]
    fn dense_smoke_adds_a_tenth_of_the_life_a_rank() {
        assert_eq!([0, 1, 2, 3].map(|r| skills(r, 0).smoke_extra_ms()), [0, 2_900, 5_800, 8_700]);
        // the crate's factor, in ms
        for r in 0..=3 {
            let s = skills(r, 0);
            let factor = (s.smoke_duration() - 1.0) * PARTICLE_LIFE_MS as f32;
            assert!((s.smoke_extra_ms() as f32 - factor).abs() < 0.01);
        }
    }

    #[test]
    fn the_toxic_window_follows_the_visible_cloud() {
        let cloud = Cloud { blast: Clock(10_000), centre: [0.0; 3] };
        let s = skills(0, 2);
        let at = |ms: u32| Clock(10_000 + ms);
        assert_eq!(cloud.toxic_ms(s, at(0), at(1_900)), 0);
        assert_eq!(cloud.toxic_ms(s, at(1_850), at(1_950)), 50);
        assert_eq!(cloud.toxic_ms(s, at(2_000), at(2_050)), 50);
        assert_eq!(cloud.toxic_ms(s, at(25_980), at(26_030)), 20);
        assert_eq!(cloud.toxic_ms(s, at(26_000), at(26_050)), 0);
        // Dense Smoke 3 holds it 8.7 s longer
        assert_eq!(cloud.toxic_ms(skills(3, 2), at(30_000), at(30_050)), 50);
        assert_eq!(cloud.toxic_ms(skills(3, 2), at(34_700), at(34_750)), 0);
        // a whole second at rank 2: 0.06
        assert!((cloud.toxic_dose(s, at(5_000), at(6_000)) - 0.06).abs() < 1e-6);
        assert_eq!(cloud.toxic_dose(skills(3, 0), at(5_000), at(6_000)), 0.0);
        // a tick before the blast (a rollback's) and across the clock's wrap
        assert_eq!(cloud.toxic_ms(s, Clock(9_000), Clock(9_050)), 0);
        let wrapped = Cloud { blast: Clock(u32::MAX - 999), centre: [0.0; 3] };
        assert_eq!(wrapped.toxic_ms(s, Clock(1_000), Clock(1_050)), 50);
    }

    #[test]
    fn inside_is_the_eyes_in_the_cylinder() {
        let cloud = Cloud { blast: Clock(0), centre: [10.0, 2.0, -4.0] };
        assert!(cloud.inside([10.0, 2.0, -4.0]));
        assert!(cloud.inside([17.9, 2.0, -4.0]));
        assert!(!cloud.inside([18.1, 2.0, -4.0]));
        assert!(!cloud.inside([10.0, 2.0 - 3.0, -4.0]));
        assert!(cloud.inside([10.0, 2.0 + 3.3, -4.0]));
        assert!(!cloud.inside([10.0, 2.0 + 3.5, -4.0]));
    }

    #[test]
    fn saved_bytes_read_back() {
        let cloud = Cloud { blast: Clock(123_456), centre: [1.5, -2.25, 1e6] };
        assert_eq!(Cloud::parse(cloud.to_bytes()), Some(cloud));
        let mut bad = cloud.to_bytes();
        bad[8..12].copy_from_slice(&f32::NAN.to_le_bytes());
        assert_eq!(Cloud::parse(bad), None);
    }

    #[test]
    fn toxic_rates_by_rank() {
        assert_eq!([0, 1, 2, 3].map(|r| skills(0, r).toxic_rate()), [0.0, 0.05, 0.06, 0.07]);
    }
}
