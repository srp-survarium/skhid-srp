//! Client-test steps of the grid package (PR #710: walk-off lips, the jump cap, the drops; A166 the press point; `an-gear-moves.md` 6.5 and 6.6;
//! `fx-grid-client-tests.md` G-V1, G-V2, G-V4, G-K1, G-C1, G-R1; `fx-grid-a166-landing-carry.md` section 6).
//!
//! The owner's jumps and walks are read from his body: a jump is the jump key going down, and the jump is *up* when his feet are the rise (less
//! 0.2 m) higher 1.5 s later. Where the press was made is the flat distance from the link's anchor. The bots' links are walked by mates
//! scripted along the link (`Script::Walk`); which way a bot crosses is its own route's, so the steps read only whether it got up.
//!
//! Not here: G-Z1 (a bot put into a zone: the runner places no bot) and G-V3 (the lap along each lip needs the route goals of the
//! walk-off edges, which a script cannot give); the map's falls are in the grid tally of the owner topic of the bench.

use super::*;
use crate::v026g::bots::view::Place;
use crate::v026g::drive::key;
use mapdata::Vec3;

const VECTOR_LABORATORY: u8 = 7;
const CHEMICAL_PLANT: u8 = 4;
const MAMAYEV_KURGAN: u8 = 6;
const COLOGNE: u8 = 8;
const RUDNYA: u8 = 1;

/// Vector Laboratory, the sprint link: take-off floor 25.1 m, landing 26.1 m, rise 1.04 (an-gear-moves 6.5).
const LINK_A: Spot = Spot::at(59.3, 25.1, -22.3);
const LINK_A_TOP: Spot = Spot::at(59.8, 26.1, -22.3);
/// Vector Laboratory, the walk step of rise 0.62 (no jump needed).
const STEP_FOOT: Spot = Spot::at(26.8, 21.3, -3.8);
const STEP_TOP: Spot = Spot::at(26.0, 21.9, -3.0);
/// Chemical Plant: the wall of A166 (flat place) with rise 1.08, and the ledge of rise 0.96 for the walking case.
const WALL_XZ: (f32, f32) = (94.9, 67.1);
const LEDGE: Spot = Spot::at(11.88, 2.47, 109.12);
/// Mamayev Kurgan: the rim of the spawn deck where the route's own drop is.
const RIM: Spot = Spot::at(97.6, 29.2, 136.3);
/// The stuck spots of the reports: Cologne (two) and Rudnya.
const COLOGNE_1: Spot = Spot::at(3.3, 35.4, 32.2);
const COLOGNE_2: Spot = Spot::at(-13.7, 35.0, 38.0);
const RUDNYA_1: Spot = Spot::at(-18.7, 12.7, -20.3);

/// A jump is up when the feet are this much below the rise higher than the press 1.5 s later: 0.2 m.
const SLACK_M: f32 = 0.2;
/// The wait after a press before the body is read, ms.
const AFTER_MS: u32 = 1_500;
/// A press at or above this speed is a sprint's, m/s.
const SPRINT_MPS: f32 = 4.5;
/// A press counts for a link when it is made within this flat distance of the anchor, m.
const NEAR_M: f32 = 6.0;

/// Which bodies a jump meter reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Jumpers {
    Owner,
    /// The bots of the owner's team.
    Mates,
}

/// The jumps made near an anchor and what became of them. Names: `presses`, `up`, `up_share`, `walk_presses` and `sprint_presses`
/// (by the speed at the press), `mean_dist_up_m` and `mean_dist_down_m` (the flat distance from the anchor at the press, of the ones that got
/// up and of the ones that did not), `max_dist_up_m` and `min_dist_down_m` (the edges of the two).
#[derive(Debug)]
struct JumpMeter {
    who: Jumpers,
    anchor: (f32, f32),
    rise_m: f32,
    /// By place: whether the jump key was down at the last frame.
    down: Vec<(Place, bool)>,
    /// Presses waiting for their 1.5 s: place, when, feet y, distance from the anchor, speed.
    pending: Vec<(Place, At, f32, f32, f32)>,
    up: Vec<f32>,
    fell: Vec<f32>,
    walk: u32,
    sprint: u32,
}

impl JumpMeter {
    fn new(who: Jumpers, anchor: (f32, f32), rise_m: f32) -> Self {
        Self { who, anchor, rise_m, down: Vec::new(), pending: Vec::new(), up: Vec::new(), fell: Vec::new(), walk: 0, sprint: 0 }
    }

    fn bodies<'a>(&self, frame: &'a Frame) -> Vec<&'a Body> {
        match self.who {
            Jumpers::Owner => frame.owner.iter().map(|o| &o.body).collect(),
            Jumpers::Mates => frame.side(Side::Mates).map(|b| &b.body).collect(),
        }
    }
}

impl CustomMeter for JumpMeter {
    fn sample(&mut self, frame: &Frame) {
        let bodies = self.bodies(frame);
        for body in &bodies {
            let pressed = body.alive && body.keys & key::JUMP != 0;
            let was = self.down.iter().find(|(p, _)| *p == body.place).is_some_and(|(_, d)| *d);
            if pressed && !was {
                let dist = (body.feet.x - self.anchor.0).hypot(body.feet.z - self.anchor.1);
                if dist <= NEAR_M {
                    self.pending.push((body.place, frame.clock, body.feet.y, dist, body.speed));
                    if body.speed >= SPRINT_MPS {
                        self.sprint += 1;
                    } else {
                        self.walk += 1;
                    }
                }
            }
            match self.down.iter_mut().find(|(p, _)| *p == body.place) {
                Some(entry) => entry.1 = pressed,
                None => self.down.push((body.place, pressed)),
            }
        }
        let (rise, clock) = (self.rise_m, frame.clock);
        let mut done = Vec::new();
        self.pending.retain(|&(place, at, y, dist, _)| {
            if clock.0.saturating_sub(at.0) < AFTER_MS {
                return true;
            }
            if let Some(body) = bodies.iter().find(|b| b.place == place) {
                done.push((body.feet.y - y >= rise - SLACK_M, dist));
            }
            false
        });
        for (up, dist) in done {
            if up { self.up.push(dist) } else { self.fell.push(dist) }
        }
    }

    fn value(&self, n: usize) -> Option<f64> {
        let mean = |v: &[f32]| (!v.is_empty()).then(|| f64::from(v.iter().sum::<f32>()) / v.len() as f64);
        let presses = self.up.len() + self.fell.len();
        match n {
            0 => Some(presses as f64),
            1 => Some(self.up.len() as f64),
            2 => (presses > 0).then(|| self.up.len() as f64 / presses as f64),
            3 => Some(f64::from(self.walk)),
            4 => Some(f64::from(self.sprint)),
            5 => mean(&self.up),
            6 => mean(&self.fell),
            7 => self.up.iter().copied().reduce(f32::max).map(f64::from),
            8 => self.fell.iter().copied().reduce(f32::min).map(f64::from),
            _ => None,
        }
    }
}

const JUMP_NAMES: &[&str] =
    &["presses", "up", "up_share", "walk_presses", "sprint_presses", "mean_dist_up_m", "mean_dist_down_m", "max_dist_up_m", "min_dist_down_m"];

fn link_a_owner(_: &TryStart) -> Box<dyn CustomMeter> {
    Box::new(JumpMeter::new(Jumpers::Owner, (LINK_A.0[0], LINK_A.0[2]), 1.0))
}

fn link_a_mates(_: &TryStart) -> Box<dyn CustomMeter> {
    Box::new(JumpMeter::new(Jumpers::Mates, (LINK_A.0[0], LINK_A.0[2]), 1.0))
}

fn step_owner(_: &TryStart) -> Box<dyn CustomMeter> {
    Box::new(JumpMeter::new(Jumpers::Owner, (STEP_FOOT.0[0], STEP_FOOT.0[2]), 0.6))
}

fn step_mates(_: &TryStart) -> Box<dyn CustomMeter> {
    Box::new(JumpMeter::new(Jumpers::Mates, (STEP_FOOT.0[0], STEP_FOOT.0[2]), 0.6))
}

fn wall_owner(_: &TryStart) -> Box<dyn CustomMeter> {
    Box::new(JumpMeter::new(Jumpers::Owner, WALL_XZ, 1.08))
}

fn ledge_owner(_: &TryStart) -> Box<dyn CustomMeter> {
    Box::new(JumpMeter::new(Jumpers::Owner, (LEDGE.0[0], LEDGE.0[2]), 0.96))
}

fn near_flat(frame: &Frame, at: Spot, r_m: f32) -> bool {
    frame.owner.as_ref().is_some_and(|o| o.body.alive && o.body.feet.distance_xz(at.vec()) <= r_m)
}

/// The owner is on the top of the link or the step (within 0.4 m of its height).
fn on_link_top(frame: &Frame, _since: Span) -> bool {
    frame.owner.as_ref().is_some_and(|o| (o.body.feet.y - LINK_A_TOP.0[1]).abs() < 0.4)
}

/// A walk up a step: whether he reached its top, and the jumps he pressed on the way. Names: `reached_top` and `jumps`.
#[derive(Debug, Default)]
struct StepWalk {
    reached: bool,
    jumps: u32,
    was_down: bool,
}

impl StepWalk {
    fn make(_: &TryStart) -> Box<dyn CustomMeter> {
        Box::new(Self::default())
    }
}

impl CustomMeter for StepWalk {
    fn sample(&mut self, frame: &Frame) {
        let Some(owner) = frame.owner.as_ref() else {
            return;
        };
        let down = owner.body.keys & key::JUMP != 0;
        if down && !self.was_down && owner.body.feet.distance_xz(STEP_FOOT.vec()) <= NEAR_M {
            self.jumps += 1;
        }
        self.was_down = down;
        if owner.body.feet.distance_xz(STEP_TOP.vec()) <= 1.5 && (owner.body.feet.y - STEP_TOP.0[1]).abs() < 0.3 {
            self.reached = true;
        }
    }

    fn value(&self, n: usize) -> Option<f64> {
        match n {
            0 => Some(f64::from(u8::from(self.reached))),
            1 => Some(f64::from(self.jumps)),
            _ => None,
        }
    }
}

/// Mamayev Kurgan's rim: where the owner's feet are 1 s after he left the rim (a fall of 0.4 m or more in 0.5 s near it). Names: `fell`
/// (1 if he went off), `landing_y` (his feet y 1 s later; the shelf is 28.19, the floor 27.53 per `fx-grid-client-tests.md` G-K1).
#[derive(Debug, Default)]
struct RimMeter {
    recent: Vec<(At, f32)>,
    left: Option<At>,
    landed: Option<f32>,
}

impl RimMeter {
    fn make(_: &TryStart) -> Box<dyn CustomMeter> {
        Box::new(Self::default())
    }
}

impl CustomMeter for RimMeter {
    fn sample(&mut self, frame: &Frame) {
        let Some(owner) = frame.owner.as_ref() else {
            return;
        };
        let (clock, y) = (frame.clock, owner.body.feet.y);
        if self.left.is_none() && owner.body.feet.distance_xz(RIM.vec()) <= 5.0 {
            self.recent.retain(|(at, _)| clock.0.saturating_sub(at.0) <= 500);
            if self.recent.iter().any(|(_, was)| was - y >= 0.4) {
                self.left = Some(clock);
            }
            self.recent.push((clock, y));
        }
        if let Some(left) = self.left
            && self.landed.is_none()
            && clock.0.saturating_sub(left.0) >= 1_000
        {
            self.landed = Some(y);
        }
    }

    fn value(&self, n: usize) -> Option<f64> {
        match n {
            0 => Some(f64::from(u8::from(self.left.is_some()))),
            1 => self.landed.map(f64::from),
            _ => None,
        }
    }
}

/// A walk past a stuck spot: the longest spell he stood (speed under 0.3 m/s) within 6 m of it with a walk key held, s, and the spells of
/// 1.5 s or more. Names: `blocked_spells`, `longest_blocked_s`, `closest_m` (the nearest he came).
#[derive(Debug)]
struct BlockMeter {
    at: Vec3,
    since: Option<At>,
    longest_ms: u32,
    spells: u32,
    counted: bool,
    closest: Option<f32>,
}

impl BlockMeter {
    fn new(at: Spot) -> Self {
        Self { at: at.vec(), since: None, longest_ms: 0, spells: 0, counted: false, closest: None }
    }
}

const WALK_KEYS: u32 = key::FORWARD | key::BACK | key::LEFT | key::RIGHT;

impl CustomMeter for BlockMeter {
    fn sample(&mut self, frame: &Frame) {
        let Some(owner) = frame.owner.as_ref() else {
            return;
        };
        let d = owner.body.feet.distance_xz(self.at);
        self.closest = Some(self.closest.map_or(d, |c| c.min(d)));
        let stuck = d <= NEAR_M && owner.body.alive && owner.body.speed < 0.3 && owner.body.keys & WALK_KEYS != 0;
        match (stuck, self.since) {
            (true, None) => {
                self.since = Some(frame.clock);
                self.counted = false;
            }
            (true, Some(since)) => {
                let ms = frame.clock.0.saturating_sub(since.0);
                self.longest_ms = self.longest_ms.max(ms);
                if ms >= 1_500 && !self.counted {
                    self.counted = true;
                    self.spells += 1;
                }
            }
            (false, _) => self.since = None,
        }
    }

    fn value(&self, n: usize) -> Option<f64> {
        let closest = self.closest?;
        match n {
            0 => Some(f64::from(self.spells)),
            1 => Some(f64::from(self.longest_ms) / 1_000.0),
            2 => Some(f64::from(closest)),
            _ => None,
        }
    }
}

fn block_measure(make: fn(&TryStart) -> Box<dyn CustomMeter>) -> Measure {
    Measure::Custom { names: &["blocked_spells", "longest_blocked_s", "closest_m"], make }
}

fn block_cologne_1(_: &TryStart) -> Box<dyn CustomMeter> {
    Box::new(BlockMeter::new(COLOGNE_1))
}

fn block_cologne_2(_: &TryStart) -> Box<dyn CustomMeter> {
    Box::new(BlockMeter::new(COLOGNE_2))
}

fn block_rudnya(_: &TryStart) -> Box<dyn CustomMeter> {
    Box::new(BlockMeter::new(RUDNYA_1))
}

/// A lane of the two ends of Vector Laboratory's link A, looped: the bot walks up it and back down.
const LINK_A_LANE: &[Spot] = &[LINK_A, LINK_A_TOP];
const STEP_LANE: &[Spot] = &[STEP_FOOT, STEP_TOP];

pub fn steps() -> Vec<Step> {
    let vector = || Group::new(VECTOR_LABORATORY, TEAM_DEATHMATCH);
    let walk_bot = |lane: &'static [Spot]| Script::Walk { lane, gait: Gait::Run, looped: true };
    vec![
        // an-gear-moves 6.5 / G-V4: the owner's own jumps at link A, from a walk and from a sprint; calibration of the standing top (0.998 or 1.08).
        Step::new(
            "grid-jump-walk",
            vector(),
            "Stand at the marked spot on the upper floor and jump to the marker above it from a walk (forward + jump), 3 times.",
        )
        .start(&[Cond::Custom(WaitFor { describe: "you are at the marked spot", holds: |f, _| near_flat(f, LINK_A, 3.0) })])
        .end(&[End::OwnerDied, End::OwnerLeft { at: LINK_A, r_m: 20.0 }])
        .measure(Measure::Custom { names: JUMP_NAMES, make: link_a_owner })
        .budget_s(90)
        .tries(1),
        Step::new(
            "grid-jump-sprint",
            vector(),
            "Same spot, same marker above: now from a sprint (sprint + jump), 3 times.",
        )
        .start(&[Cond::Custom(WaitFor { describe: "you are at the marked spot", holds: |f, _| near_flat(f, LINK_A, 3.0) })])
        .end(&[End::OwnerDied, End::OwnerLeft { at: LINK_A, r_m: 20.0 }])
        .measure(Measure::Custom { names: JUMP_NAMES, make: link_a_owner })
        .budget_s(90)
        .tries(1),
        Step::new(
            "grid-walk-step",
            vector(),
            "Walk up the low step at the marker without jumping.",
        )
        .start(&[Cond::Custom(WaitFor { describe: "you are at the foot of the step", holds: |f, _| near_flat(f, STEP_FOOT, 3.0) })])
        .end(&[End::OwnerDied, End::OwnerLeft { at: STEP_FOOT, r_m: 15.0 }])
        .measure(Measure::Custom { names: &["reached_top", "jumps"], make: StepWalk::make })
        .bar("reached_top", Test::AtLeast(1.0))
        .bar("jumps", Test::AtMost(0.0))
        .budget_s(60)
        .tries(1),
        // G-V1: bots across link A. Two mates are sent up and down it; the owner stands and watches.
        Step::new(
            "grid-bots-link",
            vector(),
            "Stand still at the marker and watch; two bots go across the upper link and back.",
        )
        .setup(Who::Nth(Side::Mates, 0), walk_bot(LINK_A_LANE))
        .setup(Who::Nth(Side::Mates, 1), walk_bot(LINK_A_LANE))
        .start(&[Cond::After(Span::s(20))])
        .end(&[End::OwnerDied])
        .measure(Measure::Custom { names: JUMP_NAMES, make: link_a_mates })
        .bar("up_share", Test::AtLeast(0.8))
        .budget_s(150)
        .arm_s(120)
        .tries(1),
        // G-V2: bots up the walk step: no jump on a rise of 0.62.
        Step::new(
            "grid-bots-step",
            vector(),
            "Stand still at the marker and watch; two bots walk up the low step and back.",
        )
        .setup(Who::Nth(Side::Mates, 0), walk_bot(STEP_LANE))
        .setup(Who::Nth(Side::Mates, 1), walk_bot(STEP_LANE))
        .start(&[Cond::After(Span::s(20))])
        .end(&[End::OwnerDied])
        .measure(Measure::Custom { names: JUMP_NAMES, make: step_mates })
        // The rise is 0.62: a bot presses no jump for it (the cap), so the presses near the step are 0 or the ones that got up.
        .bar("presses", Test::AtMost(0.0))
        .budget_s(90)
        .arm_s(120)
        .tries(1),
        // G-K1: Mamayev Kurgan's rim.
        Step::new(
            "grid-kurgan-rim",
            Group::new(MAMAYEV_KURGAN, TEAM_DEATHMATCH),
            "Walk off the rim of your spawn deck at the marker and stand where you land.",
        )
        .start(&[Cond::Custom(WaitFor { describe: "you are at the rim", holds: |f, _| near_flat(f, RIM, 3.0) })])
        .end(&[End::OwnerDied, End::OwnerLeft { at: RIM, r_m: 25.0 }])
        .measure(Measure::Custom { names: &["fell", "landing_y"], make: RimMeter::make })
        .budget_s(60)
        .tries(1),
        // A166: the press point. The wall (rise 1.08): pressed 0.5, 1.0, 1.5 and 2.0 m before its face, 5 times each.
        Step::new(
            "grid-press-wall",
            Group::new(CHEMICAL_PLANT, TEAM_DEATHMATCH),
            "At the marked wall, jump from a walk (forward + jump, no sprint) 0.5 m before its face, then 1.0, 1.5 and 2.0 m: 5 times each.",
        )
        .start(&[Cond::Custom(WaitFor { describe: "you are at the wall", holds: |f, _| near_flat(f, Spot::at(WALL_XZ.0, 0.0, WALL_XZ.1), 5.0) })])
        .end(&[End::OwnerDied, End::OwnerLeft { at: Spot::at(WALL_XZ.0, 0.0, WALL_XZ.1), r_m: 25.0 }])
        .measure(Measure::Custom { names: JUMP_NAMES, make: wall_owner })
        .budget_s(180)
        .tries(1),
        Step::new(
            "grid-press-ledge",
            Group::new(CHEMICAL_PLANT, TEAM_DEATHMATCH),
            "At the marked ledge (rise 0.96), jump from a walk 0.5 m before its face, then 1.0, 1.5 and 2.0 m: 5 times each.",
        )
        .start(&[Cond::Custom(WaitFor { describe: "you are at the ledge", holds: |f, _| near_flat(f, LEDGE, 5.0) })])
        .end(&[End::OwnerDied, End::OwnerLeft { at: LEDGE, r_m: 25.0 }])
        .measure(Measure::Custom { names: JUMP_NAMES, make: ledge_owner })
        .budget_s(180)
        .tries(1),
        // an-gear-moves 6.6: the stuck spots. The owner walks through the point and back; a spell he stands with a walk key held is the block.
        Step::new(
            "grid-stuck-cologne-1",
            Group::new(COLOGNE, TEAM_DEATHMATCH),
            "Walk to the first marker and a few metres past it, and back; say what stops you, if anything.",
        )
        .start(&[Cond::Custom(WaitFor { describe: "you are within 8 m of the marker", holds: |f, _| near_flat(f, COLOGNE_1, 8.0) })])
        .end(&[End::OwnerDied, End::OwnerLeft { at: COLOGNE_1, r_m: 25.0 }])
        .measure(block_measure(block_cologne_1))
        .budget_s(90)
        .tries(1),
        Step::new(
            "grid-stuck-cologne-2",
            Group::new(COLOGNE, TEAM_DEATHMATCH),
            "Now the second marker: walk to it and a few metres past it, and back; say what stops you, if anything.",
        )
        .start(&[Cond::Custom(WaitFor { describe: "you are within 8 m of the marker", holds: |f, _| near_flat(f, COLOGNE_2, 8.0) })])
        .end(&[End::OwnerDied, End::OwnerLeft { at: COLOGNE_2, r_m: 25.0 }])
        .measure(block_measure(block_cologne_2))
        .budget_s(90)
        .tries(1),
        Step::new(
            "grid-stuck-rudnya",
            Group::new(RUDNYA, TEAM_DEATHMATCH),
            "Walk to the marker and a few metres past it, and back; say what stops you, if anything.",
        )
        .start(&[Cond::Custom(WaitFor { describe: "you are within 8 m of the marker", holds: |f, _| near_flat(f, RUDNYA_1, 8.0) })])
        .end(&[End::OwnerDied, End::OwnerLeft { at: RUDNYA_1, r_m: 25.0 }])
        .measure(block_measure(block_rudnya))
        .budget_s(90)
        .tries(1),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::dry;

    fn owner(clock: u32, at: Vec3, keys: u32, speed: f32) -> Frame {
        let mut body = dry::body(0, 0, at);
        body.keys = keys;
        body.speed = speed;
        dry::frame_at(clock, body)
    }

    fn named(id: &str) -> Step {
        steps().into_iter().find(|s| s.id == id).unwrap_or_else(|| panic!("no step {id}")).tries(1)
    }

    /// Two jumps at the link: one pressed 1.0 m from the anchor that gets up (feet 1.04 m higher 1.5 s later), one at 3.0 m that does not.
    #[test]
    fn the_jump_meter_separates_the_presses_that_got_up() {
        let mut meter = JumpMeter::new(Jumpers::Owner, (0.0, 0.0), 1.0);
        let at = |x| Vec3::new(x, 25.0, 0.0);
        // Press at 1 m (a walk), up 1.0 m 1.5 s later.
        meter.sample(&owner(0, at(1.0), 0, 2.0));
        meter.sample(&owner(100, at(1.0), key::JUMP, 2.0));
        meter.sample(&owner(300, at(1.0), 0, 2.0));
        meter.sample(&owner(1_700, Vec3::new(0.5, 26.0, 0.0), 0, 0.0));
        // Press at 3 m (a sprint), feet 0.1 higher 1.5 s later: down.
        meter.sample(&owner(2_000, at(3.0), 0, 5.0));
        meter.sample(&owner(2_100, at(3.0), key::JUMP, 5.0));
        meter.sample(&owner(2_200, at(3.0), key::JUMP, 5.0));
        meter.sample(&owner(3_700, Vec3::new(3.0, 25.1, 0.0), 0, 0.0));
        // A press far from the anchor is not the link's.
        meter.sample(&owner(4_000, at(30.0), key::JUMP, 5.0));
        meter.sample(&owner(6_000, at(30.0), 0, 0.0));
        let v = |n| meter.value(n);
        assert_eq!((v(0), v(1)), (Some(2.0), Some(1.0)));
        assert_eq!(v(2), Some(0.5));
        assert_eq!((v(3), v(4)), (Some(1.0), Some(1.0)));
        assert_eq!((v(5), v(6), v(7), v(8)), (Some(1.0), Some(3.0), Some(1.0), Some(3.0)));
        assert_eq!(JUMP_NAMES.len(), 9);
    }

    #[test]
    fn the_block_meter_counts_a_spell_standing_with_a_walk_key_held() {
        let mut meter = BlockMeter::new(Spot::at(0.0, 0.0, 0.0));
        let frame = |ms, keys, speed| owner(ms, Vec3::new(2.0, 0.0, 0.0), keys, speed);
        meter.sample(&frame(0, key::FORWARD, 2.0));
        meter.sample(&frame(500, key::FORWARD, 0.0));
        meter.sample(&frame(1_000, key::FORWARD, 0.0));
        meter.sample(&frame(2_100, key::FORWARD, 0.0));
        meter.sample(&frame(2_600, key::FORWARD, 0.0));
        meter.sample(&frame(3_000, 0, 0.0));
        let v = |n| meter.value(n).unwrap();
        assert_eq!(v(0), 1.0);
        assert!((v(1) - 2.1).abs() < 1e-9, "from 500 ms to 2.6 s: {}", v(1));
        assert!((v(2) - 2.0).abs() < 1e-6);
    }

    #[test]
    fn the_rim_meter_reads_where_he_lands() {
        let mut meter = RimMeter::make(&TryStart { started: At(0), kills: 0, deaths: 0 });
        let on = |ms, y| owner(ms, Vec3::new(RIM.0[0], y, RIM.0[2]), 0, 0.0);
        assert_eq!(meter.value(0), Some(0.0));
        meter.sample(&on(0, 29.2));
        meter.sample(&on(200, 29.2));
        meter.sample(&on(400, 28.0));
        meter.sample(&on(1_000, 27.6));
        meter.sample(&on(1_600, 27.53));
        assert_eq!((meter.value(0), meter.value(1)), (Some(1.0), Some(27.53_f32 as f64)));
    }

    // ---- dry runs ----

    /// The owner walks up the step without a jump: PASS; with a jump press near it the second try fails the bar.
    #[test]
    fn dry_run_the_walk_step_passes_without_a_jump() {
        let (results, _) = dry::run(named("grid-walk-step"), 80, |ms| {
            let t = (ms as f32 / 10_000.0).min(1.0);
            let at = Vec3::new(STEP_FOOT.0[0] + (STEP_TOP.0[0] - STEP_FOOT.0[0]) * t, STEP_FOOT.0[1] + 0.6 * t, STEP_FOOT.0[2] + (STEP_TOP.0[2] - STEP_FOOT.0[2]) * t);
            owner(ms, at, key::FORWARD, 2.0)
        });
        assert!(results.contains("grid-walk-step") && results.contains("\tPASS\t") && results.contains("reached_top=1"), "{results}");
    }

    /// The owner stands at link A and jumps three times from a walk, each getting up; the row reads three presses, three up.
    #[test]
    fn dry_run_the_owner_jumps_read_the_presses() {
        let (results, _) = dry::run(named("grid-jump-walk"), 60, |ms| {
            let press = [10_000u32, 14_000, 18_000].iter().filter(|&&t| ms >= t).count();
            let down = [10_000u32, 14_000, 18_000].iter().any(|&t| (t..t + 100).contains(&ms));
            let y = if press > 0 && ms >= [10_000u32, 14_000, 18_000][press - 1] + 600 { 26.1 } else { 25.1 };
            let y = if ms >= 30_000 { 25.1 } else { y };
            let at = if ms >= 40_000 { Vec3::new(80.0, y, -22.3) } else { Vec3::new(LINK_A.0[0], y, LINK_A.0[2]) };
            owner(ms, at, if down { key::JUMP } else { 0 }, 2.0)
        });
        assert!(results.contains("grid-jump-walk") && results.contains("presses=3"), "{results}");
        assert!(results.contains("walk_presses=3") && results.contains("up=3"), "{results}");
    }

    /// The stuck step reads a stand of 3 s at the Cologne marker as one blocked spell.
    #[test]
    fn dry_run_a_stuck_spot_reads_the_block() {
        let (results, _) = dry::run(named("grid-stuck-cologne-1"), 40, |ms| {
            let stuck = (6_000..9_000).contains(&ms);
            let at = if ms >= 20_000 { Vec3::new(COLOGNE_1.0[0] + 40.0, 35.4, COLOGNE_1.0[2]) } else { COLOGNE_1.vec() };
            owner(ms, at, key::FORWARD, if stuck { 0.0 } else { 2.0 })
        });
        assert!(results.contains("grid-stuck-cologne-1") && results.contains("INFO") && results.contains("blocked_spells=1"), "{results}");
    }
}
