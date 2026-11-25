//! Badges (in-match medals) and their rewards: `gameplay/quests/badges.options` (volume 00), extracted by
//! `crates/data-extract` into `data/badges_069d.json`.
//!
//! The world keeps per player 27 badge slots (statistics block `B+0x534`, slot t = badge type t); after its match end
//! (`S.vt[0x110]`) bytes `+1..+3` of a slot count how many times state 1 / 2 / 3 was earned. Types 23..26 are the four
//! post-match medals (best player, shooter, killer, hero). PROVEN.
//!
//! Every state of a badge carries silver, experience and reputation (`awards` type 2 / 0 / 1). The results screen
//! computes the shown "for awards in battle" amounts itself (0x200332080): for every earned
//! instance (badge t, state s, count times), every award a and every equipment profile p,
//! `trunc(trunc(amount_a x weight_p) x factor_a(level_p))`, weight = the profile's share of the play time (`97` R
//! `+0x1C`), factor = `match_levels_factors[min(level, 5)]` (1.0 in gunrace, mode 3); no premium or booster term. With
//! one profile (weight 1) that is `trunc(amount x factor)` per instance. [`Badges::reward`] is that formula: the lobby
//! pays exactly what the screen shows.

use serde::Deserialize;

use crate::progression::Reward;
use crate::LevelFactor;

pub const FILE: &str = "badges_069d.json";
/// Badge slots of a player (`B+0x534`, 27 x 12 bytes).
pub const SLOTS: usize = 27;
/// The four post-match medals: best player, best shooter, best killer, hero (badge types 23..26).
pub const MEDALS: [u8; 4] = [23, 24, 25, 26];
/// Game mode id of gunrace: the screen's factor is 1.0 there (0x200332080).
pub const MODE_GUNRACE: u32 = 3;

#[derive(Debug, Clone, Deserialize)]
pub struct BadgeRow {
    #[serde(rename = "type")]
    pub kind: u8,
    /// per state (1-based order of the file): `[condition, silver, exp, rep]`
    pub states: Vec<[u32; 4]>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Badges {
    pub badges: Vec<BadgeRow>,
}

impl Badges {
    pub fn parse(json: &str) -> Result<Badges, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// The badge reward of one match with one equipment profile: `counts[t][s]` = times state s + 1 of badge t was
    /// earned; `factors` = `match_levels_factors`, `match_level` 0..=5 (capped), `mode` (gunrace: factor 1).
    pub fn reward(&self, counts: &[[u8; 3]; SLOTS], factors: &[LevelFactor], match_level: u32, mode: u32) -> Reward {
        let lf = if mode == MODE_GUNRACE {
            LevelFactor { reputation_factor: 1.0, experience_factor: 1.0, silver_factor: 1.0 }
        } else {
            crate::progression::level_factor(factors, match_level.min(5))
        };
        // the screen multiplies in f32 (the factors are f32 in the data: 1.4 = 1.39999997...): 10 x 1.4 is 14 in f32
        let part = |amount: u32, k: f64| (amount as f32 * k as f32).trunc() as u32;
        let mut r = Reward::default();
        for (t, c) in counts.iter().enumerate() {
            let Some(row) = self.badges.get(t) else { continue };
            for (s, &n) in c.iter().enumerate() {
                let Some(st) = row.states.get(s) else { continue };
                for _ in 0..n {
                    r.silver += part(st[1], lf.silver_factor);
                    r.exp += part(st[2], lf.experience_factor);
                    r.rep += part(st[3], lf.reputation_factor);
                }
            }
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_has_27_types_and_the_medals_pay_30_8_4() {
        crate::require_tables!();
        let gd = crate::testing::tables();
        assert_eq!(gd.badges.badges.len(), SLOTS);
        assert!(gd.badges.badges.iter().enumerate().all(|(i, b)| usize::from(b.kind) == i));
        for t in MEDALS {
            assert_eq!(gd.badges.badges[usize::from(t)].states, vec![[1, 30, 8, 4]]);
        }
        // kill badge (type 5): 10 / 20 / 30 kills
        assert_eq!(gd.badges.badges[5].states, vec![[10, 10, 4, 2], [20, 15, 6, 3], [30, 20, 8, 4]]);
    }

    #[test]
    fn the_reward_is_the_screens_formula() {
        crate::require_tables!();
        let gd = crate::testing::tables();
        // 30 kills (kill badge state 3 once) at level 3 = silver trunc(20 x 3) = 60, experience
        // trunc(8 x 1.25) = 10, reputation trunc(4 x 4) = 16
        let mut c = [[0u8; 3]; SLOTS];
        c[5][2] = 1;
        let r = gd.badges.reward(&c, &gd.match_level_factors, 3, 1);
        assert_eq!((r.silver, r.exp, r.rep), (60, 10, 16));
        // gunrace: factor 1
        let r = gd.badges.reward(&c, &gd.match_level_factors, 3, MODE_GUNRACE);
        assert_eq!((r.silver, r.exp, r.rep), (20, 8, 4));
        // counts multiply, the level is capped at 5, a medal adds its own row
        c[5][2] = 2;
        c[23][0] = 1;
        let r5 = gd.badges.reward(&c, &gd.match_level_factors, 5, 1);
        let r9 = gd.badges.reward(&c, &gd.match_level_factors, 9, 1);
        assert_eq!(r5, r9);
        let lf = crate::progression::level_factor(&gd.match_level_factors, 5);
        let p = |a: u32, k: f64| (a as f32 * k as f32).trunc() as u32;
        assert_eq!(r5.silver, 2 * p(20, lf.silver_factor) + p(30, lf.silver_factor));
        assert_eq!(r5.exp, 2 * p(8, lf.experience_factor) + p(8, lf.experience_factor));
        // nothing earned: nothing paid
        assert_eq!(gd.badges.reward(&[[0; 3]; SLOTS], &gd.match_level_factors, 3, 1), Reward::default());
    }
}
