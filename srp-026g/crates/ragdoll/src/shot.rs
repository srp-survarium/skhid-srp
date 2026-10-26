//! Bullets that cross a corpse (the owner, 2026-10-26: "bodies continue on
//! being physics objects so that bullets even in ragdoll state affected
//! them"): the segment test against a ragdoll's capsules, the push's size,
//! and the log that applies each shot's push once.
//!
//! Strictly visual. The game's bullet is not changed: a corpse's hit model
//! has left the simulation's world at the death, so the game's own trace
//! goes through it, as before. The arena reads each bullet's segment as the
//! game traces it (`bullet::check_collision`), tests it here, and pushes the
//! body it hits. The simulation re-traces the same bullet when a rollback
//! simulates its ticks again, so [`ShotLog`] keeps the shots already applied:
//! the same shot (the same bullet, segment and corpse) pushes once.

use crate::math::{Vec3, abs, clamp, max, min, sqrt};
use crate::skeleton::Part;
use crate::tuning;

/// One trace of a bullet: from `from` along the unit `direction`, `length` m
/// (to the first thing the game's own trace hit, or its full reach).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub from: Vec3,
    pub direction: Vec3,
    pub length: f32,
}

impl Shot {
    /// The shot, if its numbers make one (finite, a unit direction, a
    /// length of at most a kilometre).
    pub fn new(from: Vec3, direction: Vec3, length: f32) -> Option<Shot> {
        let unit = direction.direction()?;
        (from.is_finite() && length.is_finite() && length > 0.0 && length < 1000.0).then_some(
            Shot {
                from,
                direction: unit,
                length,
            },
        )
    }

    pub fn end(&self) -> Vec3 {
        self.from + self.direction * self.length
    }

    /// Whether the segment passes through the box `lo`..`hi` (the slab test).
    pub fn crosses_box(&self, lo: Vec3, hi: Vec3) -> bool {
        let mut enter = 0.0f32;
        let mut leave = self.length;
        for (o, d, l, h) in [
            (self.from.x, self.direction.x, lo.x, hi.x),
            (self.from.y, self.direction.y, lo.y, hi.y),
            (self.from.z, self.direction.z, lo.z, hi.z),
        ] {
            if abs(d) < 1e-12 {
                if o < l || o > h {
                    return false;
                }
                continue;
            }
            let (a, b) = ((l - o) / d, (h - o) / d);
            enter = max(enter, min(a, b));
            leave = min(leave, max(a, b));
            if enter > leave {
                return false;
            }
        }
        true
    }
}

/// Where a shot meets a body: its part, the distance along the shot, the
/// point on the capsule's surface and the surface's outward normal there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShotHit {
    pub part: Part,
    pub distance: f32,
    pub point: Vec3,
    pub normal: Vec3,
}

/// The distance along the shot to the capsule `a`..`b` of `radius`, if the
/// segment meets it (0 when it starts inside).
pub fn ray_capsule(shot: &Shot, a: Vec3, b: Vec3, radius: f32) -> Option<f32> {
    let (o, d) = (shot.from, shot.direction);
    let r2 = radius * radius;
    let ba = b - a;
    let oa = o - a;
    let baba = ba.dot(ba);
    // inside already: the closest point of the axis within the radius
    let t_axis = if baba > 0.0 {
        clamp(oa.dot(ba) / baba, 0.0, 1.0)
    } else {
        0.0
    };
    if (o - (a + ba * t_axis)).length_squared() <= r2 {
        return Some(0.0);
    }
    let mut best: Option<f32> = None;
    let mut take = |t: f32| {
        if t >= 0.0 && t <= shot.length && best.is_none_or(|b| t < b) {
            best = Some(t);
        }
    };
    // the side: the infinite cylinder, between the ends
    let bard = ba.dot(d);
    let baoa = ba.dot(oa);
    let k2 = baba - bard * bard;
    if baba > 0.0 && k2 > 1e-9 * baba {
        let k1 = baba * d.dot(oa) - baoa * bard;
        let k0 = baba * oa.dot(oa) - baoa * baoa - r2 * baba;
        let h = k1 * k1 - k2 * k0;
        if h >= 0.0 {
            let t = (-k1 - sqrt(h)) / k2;
            let y = baoa + t * bard;
            if y > 0.0 && y < baba {
                take(t);
            }
        }
    }
    // the two caps
    for centre in [a, b] {
        let oc = o - centre;
        let half_b = d.dot(oc);
        let c = oc.dot(oc) - r2;
        let h = half_b * half_b - c;
        if h >= 0.0 {
            take(-half_b - sqrt(h));
        }
    }
    best
}

/// The hit's point and outward normal on the capsule `a`..`b` at `distance`
/// along the shot.
pub fn hit_on_capsule(shot: &Shot, a: Vec3, b: Vec3, distance: f32, part: Part) -> ShotHit {
    let point = shot.from + shot.direction * distance;
    let ba = b - a;
    let baba = ba.dot(ba);
    let t = if baba > 0.0 {
        clamp((point - a).dot(ba) / baba, 0.0, 1.0)
    } else {
        0.0
    };
    let normal = (point - (a + ba * t)).direction_or(-shot.direction);
    ShotHit {
        part,
        distance,
        point,
        normal,
    }
}

/// A bullet's push along its direction, N s: its momentum at the hit, the
/// ammo's `mass` (kg, the config's) times its speed then (m/s), times
/// [`tuning::SHOT_MOMENTUM`]; [`tuning::SHOT_DEFAULT`] when the bullet gives
/// no sane mass or speed. So a pistol round pushes less than a rifle's, a
/// sniper's most, and each buckshot pellet a little.
pub fn impulse(direction: Vec3, mass: f32, speed: f32) -> Vec3 {
    let momentum = mass * speed;
    let size = if mass.is_finite()
        && speed.is_finite()
        && mass > 0.0
        && mass < 1.0
        && speed > 0.0
        && speed < 3000.0
    {
        momentum * tuning::SHOT_MOMENTUM
    } else {
        tuning::SHOT_DEFAULT
    };
    direction * size
}

/// A bullet that hit a living player: where, and its push (N s).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub point: Vec3,
    pub impulse: Vec3,
}

/// The hits a place took lately (a ring of [`RECENT`] a place, the time of
/// each), for the push of the death that follows.
pub const RECENT: usize = 4;
/// The hits of a death: the killing one (the last) and those within this
/// time before it, s.
pub const HIT_WINDOW: f32 = 0.15;
/// A hit older than this is no death's.
pub const HIT_KEPT: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Recent {
    hits: [Option<(f32, Hit)>; RECENT],
}

impl Recent {
    pub const fn new() -> Recent {
        Recent {
            hits: [None; RECENT],
        }
    }

    /// A hit at `time` (s, any clock that only goes on), the oldest dropped.
    pub fn record(&mut self, time: f32, hit: Hit) {
        let slot = match self.hits.iter().position(Option::is_none) {
            Some(i) => i,
            None => (0..RECENT)
                .min_by(|&a, &b| {
                    let t = |i: usize| self.hits[i].map_or(f32::MIN, |h| h.0);
                    t(a).total_cmp(&t(b))
                })
                .unwrap_or(0),
        };
        self.hits[slot] = Some((time, hit));
    }

    /// The hits of a death seen at `now`: the latest (the killing one,
    /// kept at most HIT_KEPT) and those within HIT_WINDOW before it, oldest
    /// first; their count.
    pub fn of_death(&self, now: f32) -> ([Hit; RECENT], usize) {
        let mut out = [Hit {
            point: Vec3::ZERO,
            impulse: Vec3::ZERO,
        }; RECENT];
        let latest = self
            .hits
            .iter()
            .flatten()
            .map(|h| h.0)
            .filter(|&t| now - t <= HIT_KEPT)
            .fold(None, |m: Option<f32>, t| Some(m.map_or(t, |m| m.max(t))));
        let Some(latest) = latest else {
            return (out, 0);
        };
        let mut chosen: [Option<(f32, Hit)>; RECENT] = [None; RECENT];
        let mut n = 0;
        for h in self.hits.iter().flatten() {
            if h.0 <= latest && latest - h.0 <= HIT_WINDOW {
                chosen[n] = Some(*h);
                n += 1;
            }
        }
        let chosen = &mut chosen[..n];
        chosen.sort_unstable_by(|a, b| {
            let t = |x: &Option<(f32, Hit)>| x.map_or(0.0, |h| h.0);
            t(a).total_cmp(&t(b))
        });
        for (o, c) in out.iter_mut().zip(chosen.iter()) {
            *o = c.map_or(*o, |h| h.1);
        }
        (out, n)
    }

    pub fn clear(&mut self) {
        self.hits = [None; RECENT];
    }
}

impl Default for Recent {
    fn default() -> Recent {
        Recent::new()
    }
}

/// One shot through one corpse: the bullet (its shooter's place, its id,
/// its trajectory's segment) and the corpse (its place, its death). The game
/// keeps the bullet's three in its state, so a re-simulated tick gives the
/// same key; its own hit effects are deduplicated by the same three
/// (`player_shootmarks_storage::add_shootmark` 0x32E460).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShotKey {
    pub shooter: u8,
    pub bullet: u32,
    pub segment: u32,
    pub place: u8,
    pub death: u32,
}

/// The shots applied lately: a ring of [`SHOT_LOG`] keys. A shot is pushed
/// only the first time it is seen; a rollback re-simulates at most a few
/// seconds, far fewer hits on corpses than the ring holds.
pub const SHOT_LOG: usize = 256;

pub struct ShotLog {
    keys: [Option<ShotKey>; SHOT_LOG],
    next: usize,
}

impl ShotLog {
    pub const fn new() -> ShotLog {
        ShotLog {
            keys: [None; SHOT_LOG],
            next: 0,
        }
    }

    /// True the first time `key` is seen (it is kept then), false again.
    pub fn first(&mut self, key: ShotKey) -> bool {
        if self.keys.contains(&Some(key)) {
            return false;
        }
        self.keys[self.next] = Some(key);
        self.next = (self.next + 1) % SHOT_LOG;
        true
    }
}

impl Default for ShotLog {
    fn default() -> ShotLog {
        ShotLog::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shot(from: Vec3, to: Vec3) -> Shot {
        Shot::new(from, to - from, (to - from).length()).unwrap()
    }

    #[test]
    fn a_segment_meets_a_capsule_at_its_side_its_caps_or_not_at_all() {
        let (a, b) = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
        // the side, from the front
        let s = shot(Vec3::new(0.0, 0.5, -2.0), Vec3::new(0.0, 0.5, 2.0));
        assert!((ray_capsule(&s, a, b, 0.1).unwrap() - 1.9).abs() < 1e-5);
        // the top cap, from above
        let s = shot(Vec3::new(0.0, 3.0, 0.0), Vec3::new(0.0, -3.0, 0.0));
        assert!((ray_capsule(&s, a, b, 0.1).unwrap() - 1.9).abs() < 1e-5);
        // the cap's rounded edge, past the end
        let s = shot(Vec3::new(0.05, 1.05, -2.0), Vec3::new(0.05, 1.05, 2.0));
        assert!(ray_capsule(&s, a, b, 0.1).is_some());
        // beside it, or short of it
        let s = shot(Vec3::new(0.2, 0.5, -2.0), Vec3::new(0.2, 0.5, 2.0));
        assert_eq!(ray_capsule(&s, a, b, 0.1), None);
        let s = shot(Vec3::new(0.0, 0.5, -2.0), Vec3::new(0.0, 0.5, -0.5));
        assert_eq!(ray_capsule(&s, a, b, 0.1), None);
        // from inside
        let s = shot(Vec3::new(0.0, 0.5, 0.0), Vec3::new(0.0, 0.5, 2.0));
        assert_eq!(ray_capsule(&s, a, b, 0.1), Some(0.0));
        // along the axis (no side), into the bottom cap
        let s = shot(Vec3::new(0.0, -2.0, 0.0), Vec3::new(0.0, 2.0, 0.0));
        assert!((ray_capsule(&s, a, b, 0.1).unwrap() - 1.9).abs() < 1e-5);
        let hit = hit_on_capsule(&s, a, b, 1.9, Part::Torso);
        assert!((hit.normal - Vec3::new(0.0, -1.0, 0.0)).length() < 1e-4);
    }

    #[test]
    fn the_box_test_passes_a_crossing_segment_only() {
        let (lo, hi) = (Vec3::new(-1.0, 0.0, -1.0), Vec3::new(1.0, 2.0, 1.0));
        assert!(shot(Vec3::new(-5.0, 1.0, 0.0), Vec3::new(5.0, 1.0, 0.0)).crosses_box(lo, hi));
        assert!(!shot(Vec3::new(-5.0, 3.0, 0.0), Vec3::new(5.0, 3.0, 0.0)).crosses_box(lo, hi));
        assert!(!shot(Vec3::new(-5.0, 1.0, 0.0), Vec3::new(-2.0, 1.0, 0.0)).crosses_box(lo, hi));
        assert!(shot(Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 1.0, 9.0)).crosses_box(lo, hi));
    }

    #[test]
    fn the_log_takes_a_shot_once() {
        let mut log = ShotLog::new();
        let key = ShotKey {
            shooter: 3,
            bullet: 77,
            segment: 0,
            place: 5,
            death: 9000,
        };
        assert!(log.first(key));
        // the same shot traced again (a rollback's re-simulated tick)
        assert!(!log.first(key));
        // the bullet's next segment (after a pierce), another corpse, another shooter
        assert!(log.first(ShotKey { segment: 1, ..key }));
        assert!(log.first(ShotKey { place: 6, ..key }));
        assert!(log.first(ShotKey { shooter: 4, ..key }));
        assert!(!log.first(key));
        // the ring forgets the oldest only after SHOT_LOG others
        for i in 0..SHOT_LOG as u32 {
            log.first(ShotKey {
                bullet: 1000 + i,
                ..key
            });
        }
        assert!(log.first(key));
    }

    #[test]
    fn the_push_is_the_rounds_momentum() {
        let d = Vec3::new(0.0, 0.0, 1.0);
        // (the configs' mass and muzzle speed)
        let pistol = impulse(d, 0.02, 315.0).z; // 9x18 Makarov
        let rifle = impulse(d, 0.02, 800.0).z; // 5.45x39
        let sniper = impulse(d, 0.1, 800.0).z; // 7.62x54
        let pellet = impulse(d, 0.01, 250.0).z; // 12 mm buckshot, one of ten
        assert!(
            pistol < rifle && rifle < sniper,
            "{pistol} {rifle} {sniper}"
        );
        assert!(pellet > 0.0 && pellet < pistol, "{pellet}");
        assert!((impulse(d, f32::NAN, 800.0).z - tuning::SHOT_DEFAULT).abs() < 1e-6);
        assert!((impulse(d, 0.02, 0.0).z - tuning::SHOT_DEFAULT).abs() < 1e-6);
    }

    #[test]
    fn a_death_takes_its_killing_hit_and_those_just_before() {
        let hit = |z: f32| Hit {
            point: Vec3::new(0.0, 1.0, z),
            impulse: Vec3::Z,
        };
        let mut recent = Recent::new();
        assert_eq!(recent.of_death(5.0).1, 0, "no hit: no push");
        recent.record(1.0, hit(1.0)); // long before
        recent.record(2.0, hit(2.0));
        recent.record(2.1, hit(3.0));
        recent.record(2.2, hit(4.0)); // the killing one
        let (hits, n) = recent.of_death(2.3);
        assert_eq!(n, 2, "within 0.15 s of the last: {hits:?}");
        assert_eq!(
            (hits[0].point.z, hits[1].point.z),
            (3.0, 4.0),
            "oldest first, the killing one last"
        );
        assert_eq!(recent.of_death(9.0).1, 0, "a hit long ago is no death's");
        // the ring drops the oldest
        recent.record(2.25, hit(5.0));
        assert!(recent.hits.iter().flatten().all(|h| h.0 > 1.5));
    }
}
