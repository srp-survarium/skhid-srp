//! [patch 143] An enemy battery carrier strikes with the battery: the bots keep out of his strike's reach.
//!
//! Exe patch 143 (`docs/notes/battery-hit.md`, `client-patches/rust/patches/src/battery_hit.rs`, PROVEN in its code):
//! - **The input:** the melee key (input bit 27, 0x8000000, `player_bots.md`'s input table) held while carrying, not in
//!   the air (`battery_hit::on_wants_swing`, `vostok/battery.rs` `melee_down`: the actions +330h bit 27, as
//!   `weapon_core::melee_pred` 0x2E4100 reads it). Held, it swings again once a strike ends.
//! - **The timing:** press to impact 0.2 s (`battery_hit::TIMES.swing`, 6 frames at 30 a second), 0.55 s a strike.
//! - **The reach:** one melee bullet from the eye at the swing's end, the bullet's length the carrier's primary gun's
//!   melee distance (`vostok/battery.rs` `strike`, `gun.melee_distance()`, the gun's +578h; `melee.data.distance` in the
//!   game data, 1.7 m for every gun of the stock archive), along the first of 15 rays (a 0.6 m wide grid on the plane
//!   0.7 m ahead of the look) whose first hit is a living body.
//! - **The damage:** the gun's melee data, blunt with an attacker: patch 108 deals 1000 (`docs/notes/hand-kill.md`), a
//!   kill on any zone.
//!
//! So a body whose hit volume is within the melee distance of a carrier's eye dies at his next strike. The bots keep
//! their centre farther from a carrier they see than that distance, plus the body's radius (the stand capsule's half
//! width, `mapdata::grid::Body::radius`), plus the carrier's walk in the swing's 0.2 s (the run's 3.5 m/s times the
//! carrier's 0.95, [`super::switch_take::CARRIER_SHARE`]): what he closes before his strike lands.

use super::view::Place;
use mapdata::Vec3;

/// Press to impact, seconds (`battery_hit::TIMES.swing`: 6 frames of 30 a second).
pub const SWING_S: f32 = 6.0 / 30.0;
/// The keep-off's goal is this far past the reach, metres: out of it with a step to spare.
pub const STEP_OUT_M: f32 = 0.5;

/// The largest melee distance of the game data's guns, metres (`item_params::ItemParams::melee_distance`): a carrier's
/// strike reaches as far as his primary gun's.
pub fn melee_distance_m() -> f32 {
    static LARGEST: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *LARGEST.get_or_init(|| {
        survarium::v026g::item_params::all()
            .iter()
            .filter(|item| item.gun.is_some())
            .map(|item| item.melee_distance)
            .fold(0.0, f32::max)
    })
}

/// How near a body's centre may be to a carrier's feet (across, metres) before his next strike can reach it: the melee
/// distance, the body's radius and the carrier's walk in the swing.
pub fn reach_m(melee_m: f32, body_radius_m: f32) -> f32 {
    melee_m + body_radius_m + super::switch_take::RUN_MPS * super::switch_take::CARRIER_SHARE * SWING_S
}

/// The nearest carrier within `reach` of `me` among `carriers` (each where the bot sees him), and where to step to be
/// out of it: on the line from him through `me`, [`STEP_OUT_M`] past the reach. A carrier on its very spot gives no
/// line: none.
pub fn keep_off(me: Vec3, carriers: impl IntoIterator<Item = (Place, Vec3)>, reach: f32) -> Option<(Place, Vec3)> {
    let (who, at, d) = carriers
        .into_iter()
        .map(|(who, at)| (who, at, me.distance_xz(at)))
        .filter(|&(_, _, d)| d < reach)
        .min_by(|a, b| a.2.total_cmp(&b.2))?;
    if d < 1e-3 {
        return None;
    }
    let out = reach + STEP_OUT_M;
    let (dx, dz) = ((me.x - at.x) / d, (me.z - at.z) / d);
    Some((who, Vec3::new(at.x + dx * out, me.y, at.z + dz * out)))
}

/// [measure] The keep-off over a match, a bot's: ticks it took the legs, the times it began, and those it could not (its
/// way out not a fine goal).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeepCounts {
    pub ticks: u32,
    pub began: u32,
    pub no_way: u32,
}

impl KeepCounts {
    pub fn add(&mut self, other: &Self) {
        self.ticks += other.ticks;
        self.began += other.began;
        self.no_way += other.no_way;
    }

    pub fn line(&self) -> String {
        format!(
            "carrier strike keep-off this match: bot ticks {} (seconds {:.1}), times begun {}, no way out {}",
            self.ticks,
            f64::from(self.ticks) * f64::from(survarium::v026g::match_msgs::TICK_MS) / 1000.0,
            self.began,
            self.no_way
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reach from the exe's numbers: 1.7 m of melee, the 0.38 m radius, 3.325 m/s for 0.2 s.
    #[test]
    fn the_reach_is_the_melee_the_body_and_the_lunge() {
        assert_eq!(melee_distance_m(), 1.7, "every gun of the stock archive");
        let reach = reach_m(1.7, 0.38);
        assert!((reach - (1.7 + 0.38 + 0.665)).abs() < 1e-4, "{reach}");
    }

    /// Inside the reach the nearest carrier gives a step out on the line from him; outside it, none.
    #[test]
    fn a_carrier_within_the_reach_is_stepped_away_from() {
        let me = Vec3::new(0.0, 1.0, 0.0);
        let reach = 2.745;
        assert_eq!(keep_off(me, [(3, Vec3::new(3.0, 1.0, 0.0))], reach), None, "3 m: out of it");
        let (who, to) = keep_off(me, [(3, Vec3::new(2.5, 1.0, 0.0)), (4, Vec3::new(0.0, 1.0, 1.5))], reach).unwrap();
        assert_eq!(who, 4, "the nearer");
        assert!((to.x).abs() < 1e-5 && (to.z - (1.5 - reach - STEP_OUT_M)).abs() < 1e-5, "{to:?}");
        assert!(to.distance_xz(Vec3::new(0.0, 1.0, 1.5)) > reach);
    }
}
