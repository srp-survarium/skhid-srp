//! The stand-in game's spread of a shot, as 0.26g's
//! `get_dispersed_bullet_direction` (0x2E1D50) spreads it
//! (`player_bots_weapons.md` §9): a cone of the weapon's base, plus until
//! the aim-in is complete the character part (stance and movement, by the
//! keys) and the bloom; each from the weapon's own `dispersion` block and
//! the player's `character_dispersion_params`. On with `SRP_BENCH_SPREAD`;
//! the shot facts count the aim's state either way.

use mapdata::Vec3;
use survarium::v026g::weapons::{Stance, character_dispersion};

use super::super::guns::Gun;
use super::super::rng::Rng;
use crate::v026g::drive::key;

/// Where the aim was at a shot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum AimState {
    /// No aim pressed.
    #[default]
    Hip,
    /// Pressed, not yet `aim_time` ago: it spreads as from the hip.
    AimingIn,
    /// Pressed at least `aim_time` ago: the base alone.
    AimedIn,
}

/// One body's spread: the character part reached, the bloom, since when
/// the aim is pressed (ms).
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Spread {
    character: Option<f32>,
    bloom: f32,
    aim_since: Option<u32>,
}

/// A move key held as the game reads it (forward xor back, left xor right).
pub(super) fn moving(keys: u32) -> bool {
    let fb = keys & (key::FORWARD | key::BACK);
    let lr = keys & (key::LEFT | key::RIGHT);
    (fb == key::FORWARD || fb == key::BACK) || (lr == key::LEFT || lr == key::RIGHT)
}

fn stance(keys: u32) -> Stance {
    Stance {
        crouched: keys & key::CROUCH != 0,
        moving: moving(keys),
        sprinting: keys & key::SPRINT != 0,
        jumping: keys & key::JUMP != 0,
        aimed: keys & key::AIM != 0,
    }
}

impl Spread {
    /// Afresh (a spawn, another weapon taken): the character part snaps to
    /// its target at the next tick (`dispersion_calculator::initialize`).
    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }

    /// A tick of `dt_s` with `keys` (`pressed` those new this tick) at
    /// `tick` ms: the aim's start, the character part toward its target
    /// (falling at `speed_of_aiming`, rising at `growth_speed`), the bloom
    /// falling at `speed_of_aiming`.
    pub(super) fn tick(&mut self, gun: &Gun, keys: u32, pressed: u32, tick: u32, dt_s: f32) {
        if keys & key::AIM == 0 {
            self.aim_since = None;
        } else if pressed & key::AIM != 0 || self.aim_since.is_none() {
            self.aim_since = Some(tick);
        }
        let d = &gun.dispersion;
        let target = character_dispersion().of(stance(keys), d);
        let c = self.character.unwrap_or(target);
        self.character = Some(if c > target {
            (c - d.speed_of_aiming * dt_s).max(target)
        } else {
            (c + d.growth_speed * dt_s).min(target)
        });
        self.bloom = (self.bloom - d.speed_of_aiming * dt_s).max(0.0);
    }

    /// A shot at `at` ms with `keys`: the aim's state and the cone,
    /// degrees (full angle); the shot's bloom added after.
    pub(super) fn shot(&mut self, gun: &Gun, keys: u32, at: f32) -> (AimState, f32) {
        let state = match self.aim_since {
            Some(since) if keys & key::AIM != 0 && at >= (since + gun.aim_ms) as f32 => {
                AimState::AimedIn
            }
            Some(_) if keys & key::AIM != 0 => AimState::AimingIn,
            _ => AimState::Hip,
        };
        let d = &gun.dispersion;
        let character = self
            .character
            .unwrap_or_else(|| character_dispersion().of(stance(keys), d));
        let cone = d.cone_deg(character, self.bloom, state == AimState::AimedIn);
        self.bloom = (self.bloom + d.one_shot).min(d.max);
        (state, cone)
    }
}

/// `dir` turned by a draw of the game's: `frac(N(0, 0.5)) × cone / 2`
/// degrees off it, about a random axis across it.
pub(super) fn deflect(dir: Vec3, cone_deg: f32, rng: &mut Rng) -> Vec3 {
    let u1 = rng.unit().max(1e-9);
    let u2 = rng.unit();
    let n = (-2.0 * u1.ln()).sqrt() * (std::f32::consts::TAU * u2).cos() * 0.5;
    let theta = (n.fract() * cone_deg * 0.5).to_radians();
    let phi = std::f32::consts::TAU * rng.unit();
    let side = dir.cross(Vec3::Y);
    let side = if side.length() < 1e-4 {
        Vec3::new(1.0, 0.0, 0.0)
    } else {
        side * (1.0 / side.length())
    };
    let up = side.cross(dir);
    let across = side * phi.cos() + up * phi.sin();
    dir * theta.cos() + across * theta.sin()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::guns::Arms;

    /// A strafing hip shot of the fallback weapon spreads by the walk
    /// value and the base; aimed in, by the base alone, moving or not;
    /// the cone's deflections stay within half of it.
    #[test]
    fn the_spread_follows_the_aim_and_the_keys() {
        let gun = Arms::fallback().primary.0;
        let d = gun.dispersion;
        let walk = character_dispersion().walk * d.from_the_hip;
        let mut s = Spread::default();
        s.tick(gun, key::LEFT, key::LEFT, 100, 0.05);
        let (state, cone) = s.shot(gun, key::LEFT, 100.0);
        assert_eq!(state, AimState::Hip);
        assert!((cone - (d.base + walk)).abs() < 1e-4, "{cone}");
        let mut s = Spread::default();
        let aim = key::LEFT | key::AIM;
        s.tick(gun, aim, aim, 100, 0.05);
        assert_eq!(s.shot(gun, aim, (100 + gun.aim_ms - 1) as f32).0, AimState::AimingIn);
        s.tick(gun, aim, 0, 150, 0.05);
        let (state, cone) = s.shot(gun, aim, (100 + gun.aim_ms) as f32);
        assert_eq!(state, AimState::AimedIn);
        assert_eq!(cone, d.base);
        let mut rng = Rng::new(4);
        let dir = Vec3::new(0.0, 0.0, 1.0);
        for _ in 0..1000 {
            let out = deflect(dir, 10.0, &mut rng);
            assert!((out.length() - 1.0).abs() < 1e-3);
            assert!(out.dot(dir).clamp(-1.0, 1.0).acos().to_degrees() <= 5.0 + 1e-3);
        }
    }
}
