//! Client-test steps of the capsule package (A198, PR #932 `grid.capsule_steps`; A216 Mamayev Kurgan day 4): does the client's capsule
//! walk up an edge higher than its step (0.80 m) and within its edge step (0.96 m: the hemisphere rides an edge it meets within the 55 deg
//! slope), with no jump pressed?
//!
//! - Vostok Radar Station's edge (`capsule-edge-owner`, `capsule-edge-bots`): the ramp meets the block's east face 0.86-0.90 m under its
//!   top (3.81 m). With `grid.capsule_steps` on, the grid walks it; this is the switch's first gate. The bots' step needs the switch on
//!   in the server's tuning: off, their route jumps it and the step reads presses, not walks.
//! - Vector Laboratory's link A (`capsule-control-owner`): a 1.04 m face, over the edge step: no walk gets up it (the control).
//! - Mamayev Kurgan day 4 (`capsule-kurgan-owner`): fx-bench's walkable route from the foot over the boulder onto the cliff top (A216,
//!   LIKELY): 36 / 47 deg faces, a 0.25-0.3 m gap the capsule spans, a 41 deg face up.
//!
//! A walk up is read from the body: it was under the top by [`BELOW_M`] near the edge, then its feet are within [`SLACK_M`] of the top's
//! height near the edge, with no jump pressed between.

use super::*;
use crate::v026g::bots::view::Place;
use crate::v026g::drive::key;
use mapdata::Vec3;

const VOSTOK_RADAR_STATION: u8 = 3;
const MAMAYEV_KURGAN: u8 = 6;
const VECTOR_LABORATORY: u8 = 7;

/// Vostok Radar Station: the take-off of the grid's jump (A198), the ramp 2 m east of it (the start), and the block's top 1 m west.
const VOSTOK_EDGE: Spot = Spot::at(8.41, 2.86, -16.51);
const VOSTOK_FOOT: Spot = Spot::at(10.4, 1.95, -16.51);
const VOSTOK_TOP: Spot = Spot::at(7.4, 3.81, -16.51);
/// Vector Laboratory, link A: the take-off floor (25.1 m) 1.5 m before the face, and the top (26.1 m) (an-gear-moves 6.5).
const VECTOR_EDGE: Spot = Spot::at(59.3, 25.1, -22.3);
const VECTOR_FOOT: Spot = Spot::at(57.8, 25.1, -22.3);
const VECTOR_TOP: Spot = Spot::at(59.8, 26.1, -22.3);
/// Mamayev Kurgan day 4: the foot (terrain at 27.9 m), the boulder (29.6 m) the walk goes over, the cliff top (30.3-30.4 m).
const KURGAN_FOOT: Spot = Spot::at(-2.9, 27.9, 106.4);
const KURGAN_BOULDER: Spot = Spot::at(-2.9, 29.6, 105.4);
const KURGAN_TOP: Spot = Spot::at(-2.9, 30.35, 104.9);

/// A body this far (flat) from the edge is at it.
const NEAR_M: f32 = 4.0;
/// Under the top by this much near the edge: below it.
const BELOW_M: f32 = 0.5;
/// Within this of the top's height near the edge: up.
const SLACK_M: f32 = 0.2;

/// Whose walks are read.
#[derive(Clone, Copy, Debug)]
enum Walkers {
    Owner,
    /// The bots of the owner's team.
    Mates,
}

/// Where a body is with respect to the edge, and whether it pressed jump since it was last below.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Track {
    Away,
    Below { pressed: bool },
    Up,
}

/// Walks up an edge. Names: `ups` (arrivals on the top from below it), `walked_ups` (of them, with no jump pressed since below), `presses`
/// (jump presses near the edge).
#[derive(Debug)]
struct EdgeWalk {
    who: Walkers,
    edge: (f32, f32),
    top_y: f32,
    bodies: Vec<(Place, Track, bool)>,
    ups: u32,
    walked: u32,
    presses: u32,
}

impl EdgeWalk {
    fn new(who: Walkers, edge: Spot, top: Spot) -> Self {
        Self { who, edge: (edge.0[0], edge.0[2]), top_y: top.0[1], bodies: Vec::new(), ups: 0, walked: 0, presses: 0 }
    }

    fn step(&mut self, place: Place, alive: bool, feet: Vec3, keys: u32) {
        let near = alive && (feet.x - self.edge.0).hypot(feet.z - self.edge.1) <= NEAR_M;
        let down = alive && keys & key::JUMP != 0;
        let i = match self.bodies.iter().position(|(p, ..)| *p == place) {
            Some(i) => i,
            None => {
                self.bodies.push((place, Track::Away, false));
                self.bodies.len() - 1
            }
        };
        let (_, track, was_down) = self.bodies[i];
        let pressed_now = down && !was_down && near;
        self.presses += u32::from(pressed_now);
        let track = match track {
            _ if !near => Track::Away,
            Track::Below { pressed } if feet.y >= self.top_y - SLACK_M => {
                self.ups += 1;
                self.walked += u32::from(!(pressed || pressed_now));
                Track::Up
            }
            Track::Below { pressed } => Track::Below { pressed: pressed || pressed_now },
            Track::Away | Track::Up if feet.y < self.top_y - BELOW_M => Track::Below { pressed: false },
            other => other,
        };
        self.bodies[i] = (place, track, down);
    }
}

impl CustomMeter for EdgeWalk {
    fn sample(&mut self, frame: &Frame) {
        let bodies: Vec<(Place, bool, Vec3, u32)> = match self.who {
            Walkers::Owner => frame.owner.iter().map(|o| (o.body.place, o.body.alive, o.body.feet, o.body.keys)).collect(),
            Walkers::Mates => frame.side(Side::Mates).map(|b| (b.body.place, b.body.alive, b.body.feet, b.body.keys)).collect(),
        };
        for (place, alive, feet, keys) in bodies {
            self.step(place, alive, feet, keys);
        }
    }

    fn value(&self, n: usize) -> Option<f64> {
        match n {
            0 => Some(f64::from(self.ups)),
            1 => Some(f64::from(self.walked)),
            2 => Some(f64::from(self.presses)),
            _ => None,
        }
    }
}

const WALK_NAMES: &[&str] = &["ups", "walked_ups", "presses"];

fn vostok_owner(_: &TryStart) -> Box<dyn CustomMeter> {
    Box::new(EdgeWalk::new(Walkers::Owner, VOSTOK_EDGE, VOSTOK_TOP))
}

fn vostok_mates(_: &TryStart) -> Box<dyn CustomMeter> {
    Box::new(EdgeWalk::new(Walkers::Mates, VOSTOK_EDGE, VOSTOK_TOP))
}

fn vector_owner(_: &TryStart) -> Box<dyn CustomMeter> {
    Box::new(EdgeWalk::new(Walkers::Owner, VECTOR_EDGE, VECTOR_TOP))
}

fn kurgan_owner(_: &TryStart) -> Box<dyn CustomMeter> {
    Box::new(EdgeWalk::new(Walkers::Owner, KURGAN_BOULDER, KURGAN_TOP))
}

fn owner_near(frame: &Frame, at: Spot, r_m: f32) -> bool {
    frame.owner.as_ref().is_some_and(|o| o.body.alive && o.body.feet.distance_xz(at.vec()) <= r_m)
}

/// The ramp to Vostok Radar Station's edge and the top past it, looped: a bot walks up and back down.
const VOSTOK_LANE: &[Spot] = &[VOSTOK_FOOT, VOSTOK_TOP];

pub fn steps() -> Vec<Step> {
    let walk_bot = |lane: &'static [Spot]| Script::Walk { lane, gait: Gait::Walk, looped: true };
    vec![
        // A198 gate 1: the owner walks into the edge, no jump.
        Step::new(
            "capsule-edge-owner",
            Group::new(VOSTOK_RADAR_STATION, TEAM_DEATHMATCH),
            "From the marker walk west up the ramp into the edge of the block, holding forward, never jumping; back down and again: 5 times.",
        )
        .start(&[Cond::Custom(WaitFor { describe: "you are at the marker on the ramp", holds: |f, _| owner_near(f, VOSTOK_FOOT, 3.0) })])
        .end(&[End::OwnerDied, End::OwnerLeft { at: VOSTOK_EDGE, r_m: 25.0 }])
        .measure(Measure::Custom { names: WALK_NAMES, make: vostok_owner })
        .bar("walked_ups", Test::AtLeast(1.0))
        .bar("presses", Test::AtMost(0.0))
        .budget_s(90)
        .tries(1),
        // A198 gate 1, the bots (needs `grid.capsule_steps` on in the server's tuning): two mates walk the lane over the edge.
        Step::new(
            "capsule-edge-bots",
            Group::new(VOSTOK_RADAR_STATION, TEAM_DEATHMATCH),
            "Stand still near the marker and watch; two bots walk up the ramp over the block's edge and back.",
        )
        .setup(Who::Nth(Side::Mates, 0), walk_bot(VOSTOK_LANE))
        .setup(Who::Nth(Side::Mates, 1), walk_bot(VOSTOK_LANE))
        .start(&[Cond::After(Span::s(20))])
        .end(&[End::OwnerDied])
        .measure(Measure::Custom { names: WALK_NAMES, make: vostok_mates })
        .bar("walked_ups", Test::AtLeast(1.0))
        .bar("presses", Test::AtMost(0.0))
        .budget_s(120)
        .arm_s(120)
        .tries(1),
        // The control: a 1.04 m face, over the capsule's edge step.
        Step::new(
            "capsule-control-owner",
            Group::new(VECTOR_LABORATORY, TEAM_DEATHMATCH),
            "From the marker walk east into the 1 m ledge, holding forward, never jumping, 5 times; you should not get up.",
        )
        .start(&[Cond::Custom(WaitFor { describe: "you are at the marker", holds: |f, _| owner_near(f, VECTOR_FOOT, 3.0) })])
        .end(&[End::OwnerDied, End::OwnerLeft { at: VECTOR_EDGE, r_m: 20.0 }])
        .measure(Measure::Custom { names: WALK_NAMES, make: vector_owner })
        .bar("walked_ups", Test::AtMost(0.0))
        .budget_s(60)
        .tries(1),
        // A216: Mamayev Kurgan day 4's walk from the foot over the boulder onto the cliff top.
        Step::new(
            "capsule-kurgan-owner",
            Group::new(MAMAYEV_KURGAN, TEAM_DEATHMATCH),
            "From the marker at the cliff's foot walk up over the boulder onto the top, never jumping; back down and again: 5 times.",
        )
        .start(&[Cond::Custom(WaitFor { describe: "you are at the marker at the cliff's foot", holds: |f, _| owner_near(f, KURGAN_FOOT, 3.0) })])
        .end(&[End::OwnerDied, End::OwnerLeft { at: KURGAN_BOULDER, r_m: 25.0 }])
        .measure(Measure::Custom { names: WALK_NAMES, make: kurgan_owner })
        .bar("walked_ups", Test::AtLeast(1.0))
        .bar("presses", Test::AtMost(0.0))
        .budget_s(120)
        .tries(1),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(spot: Spot, dy: f32) -> Vec3 {
        let v = spot.vec();
        Vec3::new(v.x, v.y + dy, v.z)
    }

    /// Below the top near the edge, then on it: a walk up with no press, a jump up with one, and nothing from away or from the top.
    #[test]
    fn a_walk_up_the_edge_is_counted_and_a_jump_up_is_not_a_walk() {
        let mut meter = EdgeWalk::new(Walkers::Owner, VOSTOK_EDGE, VOSTOK_TOP);
        // Away: nothing.
        meter.step(0, true, Vec3::new(30.0, 1.0, -16.5), 0);
        // Up the ramp and onto the top, forward only.
        meter.step(0, true, at(VOSTOK_EDGE, 0.0), key::FORWARD);
        meter.step(0, true, at(VOSTOK_TOP, 0.0), key::FORWARD);
        // Staying on the top is no new arrival.
        meter.step(0, true, at(VOSTOK_TOP, 0.05), key::FORWARD);
        assert_eq!((meter.ups, meter.walked, meter.presses), (1, 1, 0));
        // Back down, then a jump up.
        meter.step(0, true, at(VOSTOK_FOOT, 0.0), 0);
        meter.step(0, true, at(VOSTOK_EDGE, 0.0), key::FORWARD | key::JUMP);
        meter.step(0, true, at(VOSTOK_EDGE, 0.5), key::FORWARD | key::JUMP);
        meter.step(0, true, at(VOSTOK_TOP, 0.0), key::FORWARD);
        assert_eq!((meter.ups, meter.walked, meter.presses), (2, 1, 1));
        // A dead body near the edge counts nothing.
        meter.step(0, false, at(VOSTOK_EDGE, 0.0), key::JUMP);
        assert_eq!(meter.presses, 1);
    }
}
