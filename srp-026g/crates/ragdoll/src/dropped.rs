//! A thing lying loose in the level that the corpses push: a dropped
//! battery as this client draws it. The simulation has its own battery
//! elsewhere (its Bullet body, which knows no corpse and can fall through a
//! one-sided floor); this one falls by the ragdoll's rules, lies on the
//! corpses and on the level, and is only drawn.
//!
//! ```text
//!              at rest (the body alone's rules)
//!     Falling ----------------------------------> Lying { on }
//!        ^                                           |
//!        +--- a corpse under it moves or goes, ------+
//!             or one comes to lie on it
//! ```

use crate::body::{Body, Capsule};
use crate::corpse::{Corpses, DeathKey, MAX_PLACES};
use crate::math::{Pose, Vec3};
use crate::ragdoll::{Alone, Apart, Level, Motion, lies_on};

/// A dropped thing's shape: a capsule at `centre` in its model's space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DroppedShape {
    pub centre: Vec3,
    pub capsule: Capsule,
    /// kg.
    pub mass: f32,
}

/// The places whose corpses a thing lies on, a bit each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Places(u32);

const _: () = assert!(MAX_PLACES <= u32::BITS as usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lie {
    Falling,
    /// At rest, on these places' corpses (none: on the level alone).
    Lying { on: Places },
}

/// A corpse nearer than this to a thing at rest holds it, m.
const ON: f32 = 0.01;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dropped {
    alone: Alone,
    /// The model relative to the body.
    model: Pose,
    /// Each place's corpse as it pushes this: the death it is of, and the
    /// parts this has been apart from.
    corpses: [Option<(DeathKey, Apart)>; MAX_PLACES],
    lie: Lie,
}

impl Dropped {
    /// The thing let go at rest where its model is drawn, at `model`.
    pub fn new(model: Pose, shape: DroppedShape) -> Dropped {
        let centre = Pose::new(model.rotation, model.apply(shape.centre));
        Dropped {
            alone: Alone::new(Body::new(centre, shape.capsule, shape.mass), 0.0),
            model: centre.inverse().then(model),
            corpses: [None; MAX_PLACES],
            lie: Lie::Falling,
        }
    }

    /// Its model's world pose.
    pub fn model(&self) -> Pose {
        self.alone.body.pose.then(self.model)
    }

    /// One frame of `dt` s among `corpses` (`up` the level's).
    pub fn step(&mut self, dt: f32, up: Vec3, corpses: &Corpses, level: &mut impl Level) {
        // each place's corpse now: a new death's starts apart from what it
        // does not overlap, a body gone pushes no more
        let mut on = Places(0);
        let mut disturbed = false;
        for place in 0..MAX_PLACES {
            let Some((death, ragdoll, moving)) = corpses.body(place) else {
                self.corpses[place] = None;
                continue;
            };
            let body = &self.alone.body;
            if !matches!(self.corpses[place], Some((d, _)) if d == death) {
                self.corpses[place] = Some((death, Apart::at_release(body, ragdoll.parts())));
            }
            if lies_on(body, ragdoll.parts(), ON) {
                on.0 |= 1 << place;
                disturbed |= moving;
            }
        }
        if let Lie::Lying { on: before } = self.lie {
            if before == on && !disturbed {
                return;
            }
            self.alone.wake();
            self.lie = Lie::Falling;
        }
        let pushed = &mut self.corpses;
        let motion = self.alone.step(dt, up, level, |body| {
            for (place, entry) in pushed.iter_mut().enumerate() {
                if let (Some((_, apart)), Some((_, ragdoll, _))) = (entry, corpses.body(place)) {
                    apart.solve(body, ragdoll.parts(), up);
                }
            }
        });
        if motion == Motion::Settled {
            self.lie = Lie::Lying { on };
        }
    }
}

#[cfg(test)]
mod tests;
