//! A ragdoll's loose things (the dropped gun, the headwear off the head)
//! against the corpse's own parts.
//!
//! A loose body is light against a corpse: it is pushed out of a part as
//! out of the ground and held on it by the same static friction, and the
//! part is not moved by it. A part it overlaps when it is let go (the hand
//! that held the gun, the head the hat sat on) does not push it until the
//! two have come apart once: the release throws nothing.

use super::{RECOVERY, STATIC_FRICTION, SUBSTEP, apart, carry_previous, closest_points, separation};
use crate::body::{Body, CONTACTS, Contact, Pair, close_gap};
use crate::math::{Pose, Vec3, max, min};
use crate::skeleton::{PARTS, Part};
use crate::tuning;

/// The corpse's parts a loose body has been apart from since it was let
/// go, a bit each ([`Part::index`]): those push it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Apart(u16);

const _: () = assert!(PARTS <= u16::BITS as usize);

impl Apart {
    fn at_release(body: &Body, parts: &[Body; PARTS]) -> Apart {
        Apart(
            Part::ALL
                .into_iter()
                .filter(|p| separation(&parts[p.index()], body).is_none())
                .fold(0, |bits, p| bits | 1 << p.index()),
        )
    }

    fn from(self, part: Part) -> bool {
        self.0 & 1 << part.index() != 0
    }

    fn now(&mut self, part: Part) {
        self.0 |= 1 << part.index();
    }
}

/// A loose body of a ragdoll and the parts that push it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Loose {
    pub(super) body: Body,
    apart: Apart,
}

impl Loose {
    /// `body` let go among `parts`.
    pub(super) fn released(body: Body, parts: &[Body; PARTS]) -> Loose {
        Loose {
            body,
            apart: Apart::at_release(&body, parts),
        }
    }

    /// Whether `part` pushes it (it has been apart from it).
    #[cfg(test)]
    pub(super) fn meets(&self, part: Part) -> bool {
        self.apart.from(part)
    }

    /// One substep's contacts with `parts`, before the level's.
    pub(super) fn solve(&mut self, parts: &[Body; PARTS], up: Vec3) {
        let item = &mut self.body;
        let start = *item;
        for part in Part::ALL {
            let body = &parts[part.index()];
            let touch = if apart(body, item) {
                None
            } else {
                separation(body, item)
            };
            let Some((on_part, on_item, distance)) = touch else {
                self.apart.now(part);
                continue;
            };
            if !self.apart.from(part) {
                continue;
            }
            let n = (on_item - on_part).direction_or(up);
            let reach = body.shape.radius + item.shape.radius;
            let depth = reach - distance;
            // as between two parts: the substep's depth goes out at once, an
            // older one a little a substep and without a speed
            let (b0, b1) = body.shape.ends(body.previous);
            let (i0, i1) = item.shape.ends(item.previous);
            let (was_part, was_item) = closest_points(b0, b1, i0, i1);
            let before = max(0.0, reach - (was_item - was_part).length());
            let push = max(0.0, depth - before);
            let recovery = min(min(before, depth), RECOVERY);
            let point = on_item - n * item.shape.radius;
            let r = point - item.pose.position;
            if push > 0.0 {
                close_gap(Pair::One(item), r, r, n * push, 0.0);
            }
            if recovery > 0.0 {
                let was = item.pose;
                close_gap(Pair::One(item), r, r, n * recovery, 0.0);
                carry_previous(item, was);
            }
            // static friction: the touching point's slide over the part's
            // surface in this substep (the part's own motion there taken off)
            let moved = |b: &Body| point - b.previous.apply(b.pose.inverse().apply(point));
            let slide = (moved(item) - moved(body)).reject(n);
            if slide.length() < STATIC_FRICTION * depth {
                let r = point - item.pose.position;
                close_gap(Pair::One(item), r, r, -slide, 0.0);
            }
        }
        // a part moves it no faster than this (wedged between a limb and the
        // chest, or a limb and the ground, it shot out at their closing
        // speed over the wedge's angle); what is left of the depth goes out
        // in the next substeps, without a speed
        let moved = item.pose.position - start.pose.position;
        let most = tuning::LOOSE_PUSH_SPEED * SUBSTEP;
        if moved.length() > most {
            item.pose.position = start.pose.position + moved * (most / moved.length());
        }
        // the level holds: parts that press it into a plane it lies on do not
        // move it this substep (a gun under a falling body stays on the
        // floor, the body lies over it)
        if pressed_in(&start, item) {
            *item = start;
        }
    }
}

/// A depth the parts may press a body into the level in a substep, m: the
/// level's own contact takes it out at once.
const PRESS: f32 = 0.001;

/// Whether an end of `now` is under one of its planes, deeper than it was
/// at `start` (the same body before the parts moved it).
fn pressed_in(start: &Body, now: &Body) -> bool {
    let heights = |body: &Body, pose: Pose| -> [Option<f32>; CONTACTS] {
        let (a, b) = body.shape.ends(pose);
        core::array::from_fn(|i| match body.contacts[i] {
            Contact::Plane { point, normal } => {
                let end = if i % 2 == 0 { a } else { b };
                Some((end - point).dot(normal) - body.shape.radius)
            }
            Contact::Free => None,
        })
    };
    let deeper = |was: [Option<f32>; CONTACTS], is: [Option<f32>; CONTACTS]| {
        was.into_iter()
            .zip(is)
            .any(|pair| matches!(pair, (Some(was), Some(is)) if is < -PRESS && is < was - PRESS))
    };
    deeper(heights(start, start.pose), heights(now, now.pose))
        || deeper(heights(start, start.previous), heights(now, now.previous))
}
