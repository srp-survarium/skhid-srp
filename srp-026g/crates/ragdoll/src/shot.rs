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

/// The push of a shot along its direction, N s: [`tuning::SHOT_IMPULSE`] for
/// a weapon of [`tuning::SHOT_STOPPING_POWER`], scaled by the weapon's own
/// `stopping_power` when the bullet has one (the configs' 0.1 to 0.8).
pub fn impulse(direction: Vec3, stopping_power: Option<f32>) -> Vec3 {
    let scale = match stopping_power {
        Some(p) if p.is_finite() && p > 0.0 => clamp(
            p / tuning::SHOT_STOPPING_POWER,
            tuning::SHOT_SCALE_MIN,
            tuning::SHOT_SCALE_MAX,
        ),
        _ => 1.0,
    };
    direction * (tuning::SHOT_IMPULSE * scale)
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
pub const SHOT_LOG: usize = 128;

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
    fn the_push_scales_with_stopping_power_within_bounds() {
        let d = Vec3::new(0.0, 0.0, 1.0);
        assert!((impulse(d, None).z - tuning::SHOT_IMPULSE).abs() < 1e-6);
        assert!(
            (impulse(d, Some(tuning::SHOT_STOPPING_POWER)).z - tuning::SHOT_IMPULSE).abs() < 1e-6
        );
        assert!(impulse(d, Some(0.8)).z > impulse(d, Some(0.4)).z);
        assert!(impulse(d, Some(100.0)).z <= tuning::SHOT_IMPULSE * tuning::SHOT_SCALE_MAX + 1e-5);
        assert!((impulse(d, Some(f32::NAN)).z - tuning::SHOT_IMPULSE).abs() < 1e-6);
    }
}
