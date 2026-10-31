use super::*;
use crate::v026g::bots::tests::standin_jump::{LETHAL_FALL_DAMAGE, fall_damage};

/// The jumps counts: counted as the match runs, kept for [`OwnerTally::finish`].
#[derive(Default)]
pub(super) struct JumpCounts {
    pub(super) landings: Vec<Landing>,
}

/// The wrong-jump detector (`jumps/detect.py`, `jumps.md` §9, final): its
/// flags, the second numbers (counted, but alone they make no press
/// wrong), and the outcome flags (a failed jump).
pub(crate) const FLAGS: [&str; 16] = [
    "TOO_LOW",
    "NO_SPRINT",
    "PRESSED_LOW",
    "STAND_TURN",
    "SHORT_RUNUP",
    "FWD_RELEASED",
    "FWD_RELEASED_085",
    "BACK_SIDE",
    "IN_PLACE",
    "REPEAT",
    "LINK_AGAIN",
    "NOT_UP",
    "UP_THEN_DOWN",
    "PRESSED_UP",
    "NOT_FREED",
    "DROPPED",
];

pub(crate) const SECOND_NUMBERS: [&str; 2] = ["FWD_RELEASED_085", "LINK_AGAIN"];

pub(crate) const OUTCOMES: [&str; 7] = [
    "REPEAT",
    "LINK_AGAIN",
    "NOT_UP",
    "UP_THEN_DOWN",
    "PRESSED_UP",
    "NOT_FREED",
    "DROPPED",
];

/// A link's rise a jump makes, m: with sprint (not carrying), without.
pub(crate) const REACH_M: f32 = 1.30;

pub(crate) const SPRINTLESS_M: f32 = 1.00;

/// The flag of a press for a link of `rise` m by what the press could make (`jumps.md` 9.2): TOO_LOW if the link is over
/// the sprint jump's reach, or over the plain jump's with a battery carried (which cannot sprint); NO_SPRINT if it is over
/// the plain jump's and the press was made with no sprint held. A press with the sprint held (not carrying) for a link
/// within the sprint jump's reach is no flag.
pub(crate) fn form_flag(rise: f32, sprint: bool, carrying: bool) -> Option<&'static str> {
    if rise > REACH_M || (rise > SPRINTLESS_M && carrying) {
        Some("TOO_LOW")
    } else if rise > SPRINTLESS_M && !sprint {
        Some("NO_SPRINT")
    } else {
        None
    }
}

#[test]
fn the_form_flag_asks_for_the_sprint_only_over_the_plain_jump() {
    // Within the plain jump: no flag. Over it, within the sprint jump's: NO_SPRINT with no sprint held, none with it.
    assert_eq!(form_flag(0.9, false, false), None);
    assert_eq!(form_flag(1.2, false, false), Some("NO_SPRINT"));
    assert_eq!(form_flag(1.2, true, false), None);
    // A carried battery cannot sprint: TOO_LOW over the plain jump, held or not.
    assert_eq!(form_flag(1.2, true, true), Some("TOO_LOW"));
    assert_eq!(form_flag(1.2, false, true), Some("TOO_LOW"));
    assert_eq!(form_flag(0.9, false, true), None);
    // Over the sprint jump's reach: TOO_LOW whatever the keys.
    assert_eq!(form_flag(1.4, true, false), Some("TOO_LOW"));
    assert_eq!(form_flag(1.4, false, false), Some("TOO_LOW"));
}

/// Pressed this far under the take-off, m.
pub(crate) const PRESSED_LOW_M: f32 = 0.30;

/// The run-up: a forward run under this many ticks; with this many still
/// ticks right before it; gaps of up to this many ticks with no walk key
/// bridged; looked for this many ticks back.
pub(crate) const RUN_TICKS: usize = 7;

pub(crate) const STILL_TICKS: usize = 4;

pub(crate) const RUN_GAP_TICKS: usize = 2;

pub(crate) const RUN_LOOK_TICKS: usize = 30;

/// Forward let go this many ticks after the press, at most (and the second
/// number's `CARRY_S` 0.85 s).
pub(crate) const RELEASED_TICKS: usize = 5;

pub(crate) const CARRY_TICKS: usize = 17;

/// A press again within this, ms, and this near the last, m.
pub(crate) const REPEAT_MS: u32 = 3_000;

pub(crate) const REPEAT_M: f32 = 2.0;

/// A route link pressed again within this, ms (its take-off and landing
/// both within [`LINK_M`]).
pub(crate) const LINK_AGAIN_MS: u32 = 10_000;

pub(crate) const LINK_M: f32 = 0.3;

/// Up a link: the feet this close under the landing, m; looked for this
/// many ticks after the press (0.2-2.2 s), and needing a record this long.
pub(crate) const UP_SLACK_M: f32 = 0.35;

pub(crate) const UP_FROM_TICKS: usize = 4;

pub(crate) const UP_TO_TICKS: usize = 44;

pub(crate) const UP_SEEN_TICKS: usize = 64;

/// Up, then down: links rising more than this, m; back on the take-off
/// within this many ticks, within this of it, m; or the link pressed
/// again within this, ms.
pub(crate) const RISE_MIN_M: f32 = 0.4;

pub(crate) const DOWN_TICKS: usize = 50;

pub(crate) const DOWN_FAR_M: f32 = 3.5;

pub(crate) const AGAIN_MS: u32 = 6_000;

/// Pressed while already up: within this of the landing, m.
pub(crate) const PRESSED_UP_NEAR_M: f32 = 1.5;

/// Not freed: the jump watch's 3 s; moved under this when it ended by a
/// stuck watch or another press, or under [`WENT_ON_M`] when it went on;
/// or stood at the press and every half second 1.0-3.2 s after.
pub(crate) const FREED_MS: u32 = 3_000;

pub(crate) const FREED_M: f32 = 1.0;

pub(crate) const WENT_ON_M: f32 = 0.5;

pub(crate) const STOOD_MPS: f32 = 0.6;

pub(crate) const STOOD_TICKS: (usize, usize) = (20, 64);

/// Stalls (§9.4): walking forward under this, m/s, moving under this
/// between seconds, m, for this long, s; a ledge ahead this high (missed
/// jump; step), this far ahead and at most this to the side, m; missed
/// when no press came from this before it, ms, or the first came this
/// late into it, ms.
pub(crate) const STALL_MPS: f32 = 0.6;

pub(crate) const STALL_MOVE_M: f32 = 0.5;

pub(crate) const STALL_S: f32 = 1.8;

pub(crate) const LEDGES: [(f32, f32); 2] = [(0.6, 1.0), (0.3, 0.6)];

pub(crate) const LEDGE_AHEAD_M: (f32, f32) = (0.25, 1.1);

pub(crate) const LEDGE_SIDE_M: f32 = 0.4;

pub(crate) const MISSED_BEFORE_MS: u32 = 500;

pub(crate) const MISSED_LATE_MS: u32 = 2_000;

/// A jump press.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Press {
    pub(crate) place: Place,
    pub(crate) t: u32,
    pub(crate) feet: Vec3,
    pub(crate) by: &'static str,
    /// The route's take-off and landing, for a route jump.
    pub(crate) route: Option<(Vec3, Vec3)>,
    pub(crate) carrying: bool,
    /// The reflex's hide at the press (`open`, `hiding`, `peeking`).
    pub(crate) cover: &'static str,
    /// The route's length left past the landing and the goal's distance from it (the FWDREL probe).
    pub(crate) after_landing: Option<(f32, f32)>,
}

/// The run-up before a press at record `ri` (§9.2): the forward run's
/// ticks (no back key; gaps of up to [`RUN_GAP_TICKS`] with no walk key
/// bridged; a strafe or back key ends it), and the ticks with no walk key
/// right before it, within [`RUN_LOOK_TICKS`] of the press.
pub(crate) fn run_up(recs: &[Rec], ri: usize) -> (usize, usize) {
    let lo = ri.saturating_sub(RUN_LOOK_TICKS);
    let (mut k, mut start, mut gap) = (ri, ri, 0);
    while k > lo {
        let keys = recs[k - 1].keys;
        if keys & key::FORWARD != 0 && keys & key::BACK == 0 {
            k -= 1;
            start = k;
            gap = 0;
        } else if keys & WALK_KEYS == 0 && gap < RUN_GAP_TICKS {
            k -= 1;
            gap += 1;
        } else {
            break;
        }
    }
    let (mut k, mut still) = (start, 0);
    while k > lo && recs[k - 1].keys & WALK_KEYS == 0 {
        k -= 1;
        still += 1;
    }
    (ri - start, still)
}

/// A floor `lo`-`hi` m above `feet`, [`LEDGE_AHEAD_M`] ahead along the
/// facing `yaw` and at most [`LEDGE_SIDE_M`] to its side (`gridprobe`).
pub(crate) fn ledge_ahead(grid: &WalkGrid, feet: Vec3, yaw: f32, lo: f32, hi: f32) -> bool {
    let (dx, dz) = (-yaw.sin(), yaw.cos());
    let r = LEDGE_AHEAD_M.1 + 0.1;
    grid.floors_in([feet.x - r, feet.z - r, feet.x + r, feet.z + r])
        .any(|node| {
            let p = grid.position(node);
            let (x, z) = (p.x - feet.x, p.z - feet.z);
            let along = x * dx + z * dz;
            let side = (x * dz - z * dx).abs();
            let up = p.y - feet.y;
            (LEDGE_AHEAD_M.0..=LEDGE_AHEAD_M.1).contains(&along)
                && side <= LEDGE_SIDE_M
                && (lo..=hi).contains(&up)
        })
}

/// Up a link to `to` at a record `dt` s after the press (§9.3 `NOT_UP`):
/// alive, the feet within [`UP_SLACK_M`] under the landing (at or over it
/// in the air), near enough the landing for the time.
pub(crate) fn is_up(r: &Rec, to: Vec3, dt: f32) -> bool {
    let feet_up = if r.air {
        r.feet.y >= to.y
    } else {
        r.feet.y >= to.y - UP_SLACK_M
    };
    r.alive && feet_up && r.feet.distance_xz(to) <= (1.5 + 3.5 * (dt - 0.6)).min(4.0)
}

impl Run {
    /// The flags of press `i` of `presses` (sorted by place and time),
    /// `jumps.md` §9.2-9.3.
    pub(crate) fn press_flags(&self, presses: &[Press], i: usize) -> Vec<&'static str> {
        let p = &presses[i];
        let recs = &self.recs[p.place];
        let mut flags = Vec::new();
        if recs.is_empty() {
            return flags;
        }
        let ri = self.index(p.t).min(recs.len() - 1);
        let keys = recs[ri].keys;
        let forward = keys & key::FORWARD != 0;
        let walk = keys & WALK_KEYS;
        let sprint = forward && keys & key::SPRINT != 0;
        // The same bot's presses before and after this one.
        let first = presses[..i]
            .iter()
            .rposition(|q| q.place != p.place)
            .map_or(0, |k| k + 1);
        let before = &presses[first..i];
        let after = presses[i + 1..].iter().take_while(|q| q.place == p.place);
        let same_link = |q: &Press| match (p.route, q.route) {
            (Some((a, b)), Some((c, d))) => (a - c).length() <= LINK_M && (b - d).length() <= LINK_M,
            _ => false,
        };

        // Form.
        if let Some((from, to)) = p.route {
            let rise = to.y - from.y;
            if let Some(flag) = form_flag(rise, sprint, p.carrying) {
                flags.push(flag);
            }
            if from.y - p.feet.y > PRESSED_LOW_M {
                flags.push("PRESSED_LOW");
            }
        }
        if forward && matches!(p.by, "route" | "hop") {
            let (run, still) = run_up(recs, ri);
            if run < RUN_TICKS {
                flags.push(if still >= STILL_TICKS {
                    "STAND_TURN"
                } else {
                    "SHORT_RUNUP"
                });
            }
        }
        if forward && p.by == "route" {
            let off = (ri + 1..recs.len())
                .position(|k| recs[k].keys & key::FORWARD == 0)
                .map(|o| o + 1);
            match off {
                Some(o) if o <= RELEASED_TICKS => flags.push("FWD_RELEASED"),
                Some(o) if o <= CARRY_TICKS => flags.push("FWD_RELEASED_085"),
                _ => {}
            }
        }
        if !forward && walk & (key::BACK | key::LEFT | key::RIGHT) != 0 {
            flags.push("BACK_SIDE");
        }
        if walk == 0 {
            flags.push("IN_PLACE");
        }

        // Outcome.
        if before
            .last()
            .is_some_and(|q| p.t <= q.t + REPEAT_MS && p.feet.distance_xz(q.feet) <= REPEAT_M)
        {
            flags.push("REPEAT");
        }
        if p.route.is_some() && before.iter().any(|q| p.t <= q.t + LINK_AGAIN_MS && same_link(q)) {
            flags.push("LINK_AGAIN");
        }
        if let Some((from, to)) = p.route {
            let rise = to.y - from.y;
            let last = recs.len() - 1;
            let died = (ri..=(ri + UP_TO_TICKS).min(last)).any(|k| !recs[k].alive);
            let up_at = (ri + UP_FROM_TICKS..=(ri + UP_TO_TICKS).min(last))
                .find(|&k| is_up(&recs[k], to, (k - ri) as f32 * TICK_S));
            if up_at.is_none() && !died && ri + UP_SEEN_TICKS <= last {
                flags.push("NOT_UP");
            }
            if rise > RISE_MIN_M {
                if let Some(u) = up_at {
                    let link = (to.x - from.x).hypot(to.z - from.z).max(1e-3);
                    let on_take_off = |r: &Rec| {
                        let along = ((r.feet.x - from.x) * (to.x - from.x)
                            + (r.feet.z - from.z) * (to.z - from.z))
                            / link;
                        r.alive
                            && !r.air
                            && (r.feet.y - from.y).abs() <= UP_SLACK_M
                            && r.feet.distance_xz(from) <= DOWN_FAR_M
                            && along <= link
                    };
                    let back = (u + 1..=(ri + DOWN_TICKS).min(last)).any(|k| on_take_off(&recs[k]));
                    let again = after.clone().any(|q| {
                        q.t <= p.t + AGAIN_MS && same_link(q) && q.feet.y < to.y - UP_SLACK_M
                    });
                    if back || again {
                        flags.push("UP_THEN_DOWN");
                    }
                }
                let r = &recs[ri];
                if !r.air
                    && r.feet.y >= to.y - UP_SLACK_M.min(rise / 4.0)
                    && r.feet.distance_xz(to) <= PRESSED_UP_NEAR_M
                {
                    flags.push("PRESSED_UP");
                }
            }
        }
        if matches!(p.by, "hop" | "unstick") {
            let moved_at = |t: u32| {
                recs.get(self.index(t))
                    .filter(|r| r.alive)
                    .map(|r| r.feet.distance_xz(p.feet))
            };
            let next = after.clone().next().map(|q| q.t).filter(|&t| t <= p.t + FREED_MS);
            let stuck = self.stuck_notes[p.place]
                .iter()
                .copied()
                .find(|&t| t > p.t && t <= p.t + FREED_MS);
            let watched = match next.into_iter().chain(stuck).min() {
                Some(t) => moved_at(t).is_some_and(|m| m < FREED_M),
                None => moved_at(p.t + FREED_MS).is_some_and(|m| m < WENT_ON_M),
            };
            let stood = recs[ri].speed < STOOD_MPS
                && ri + STOOD_TICKS.1 < recs.len()
                && (ri + STOOD_TICKS.0..=ri + STOOD_TICKS.1).step_by(TPS / 2).all(|k| {
                    let r = &recs[k];
                    r.alive && r.feet.distance_xz(p.feet) <= FREED_M && r.speed < STOOD_MPS
                });
            if watched || stood {
                flags.push("NOT_FREED");
            }
        }
        // No jump came of it: not in the air 100-400 ms after, on the
        // ground at 400 ms, alive a second on.
        if ri + TPS < recs.len()
            && !(ri + 2..ri + 8).any(|k| recs[k].air)
            && recs[ri + 8].alive
            && !recs[ri + 8].air
            && recs[ri + TPS].alive
        {
            flags.push("DROPPED");
        }
        flags
    }

    /// One place's stalls at a ledge (§9.4): [missed jumps, stalls at a
    /// step, stalls]. From a sample a second.
    pub(crate) fn stalls(&self, place: Place, grid: &WalkGrid, presses: &[Press]) -> [u32; 3] {
        self.stall_scan(place, grid, presses).0
    }

    /// [`Self::stalls`] and the record indices at which the missed jumps
    /// start.
    pub(crate) fn stall_scan(&self, place: Place, grid: &WalkGrid, presses: &[Press]) -> ([u32; 3], Vec<usize>) {
        let recs = &self.recs[place];
        let samples: Vec<usize> = (0..recs.len() / TPS).map(|s| s * TPS).collect();
        let ok = |k: usize| {
            let r = &recs[k];
            r.alive
                && !r.air
                && r.keys & key::FORWARD != 0
                && r.keys & key::BACK == 0
                && r.speed < STALL_MPS
        };
        let mut out = [0u32; 3];
        let mut missed = Vec::new();
        let mut i = 0;
        while i < samples.len() {
            if !ok(samples[i]) {
                i += 1;
                continue;
            }
            let mut j = i;
            while j + 1 < samples.len()
                && ok(samples[j + 1])
                && recs[samples[j]].feet.distance_xz(recs[samples[j + 1]].feet) < STALL_MOVE_M
            {
                j += 1;
            }
            let (k0, k1) = (samples[i], samples[j]);
            if (k1 - k0) as f32 * TICK_S >= STALL_S {
                out[2] += 1;
                let (t0, t1) = (self.time_of(k0), self.time_of(k1));
                let pressed = presses
                    .iter()
                    .filter(|p| p.place == place && p.t + MISSED_BEFORE_MS >= t0 && p.t <= t1)
                    .map(|p| p.t)
                    .min();
                if pressed.is_none_or(|t| t > t0 + MISSED_LATE_MS) {
                    let r = &recs[k0];
                    if let Some(c) = LEDGES
                        .iter()
                        .position(|&(lo, hi)| ledge_ahead(grid, r.feet, r.yaw, lo, hi))
                    {
                        out[c] += 1;
                        if c == 0 {
                            missed.push(k0);
                        }
                    }
                }
            }
            i = j + 1;
        }
        (out, missed)
    }
}

/// The bots' jumps' flags, by name.
pub(crate) fn flags_of(json: &serde_json::Value, flag: &str) -> u64 {
    json["jump_flags"][flag].as_u64().unwrap_or(0)
}

#[test]
pub(crate) fn a_route_press_from_a_standstill_too_high_with_forward_let_go_is_flagged() {
    let (mut game, mut tally) = made_up(&[0, 0]);
    play(&mut game, &mut tally, 200, |k, game| {
        // Bot 0: walks, stands 7 ticks, 3 ticks of forward, presses at
        // tick 100 for a 1.4 m rise, lets forward go 3 ticks on; its body
        // never gets up.
        let keys0 = match k {
            0..90 => key::FORWARD,
            90..97 => 0,
            97..100 => key::FORWARD,
            100 => key::FORWARD | key::JUMP,
            101..103 => key::FORWARD,
            _ => 0,
        };
        // Bot 1 (the near miss): runs into its press for a 0.9 m rise,
        // holds forward on, and is up half a second later.
        let keys1 = match k {
            100 => key::FORWARD | key::JUMP,
            0..121 => key::FORWARD,
            _ => 0,
        };
        put(game, 0, 10.0, 10.0, keys0);
        put(game, 1, 10.0, 20.0, keys1);
        if k >= 110 {
            game.bodies[1].feet = Vec3::new(10.8, 0.9, 20.0);
        }
        for body in &mut game.bodies {
            body.air_until = if (100..116).contains(&k) { game.tick + TICK_MS } else { 0 };
        }
        let mut nows = vec![bot_now(), bot_now()];
        if k == 100 {
            let low = Vec3::new(10.0, 0.0, 10.0);
            nows[0].as_mut().unwrap().jump = Some(("route", Some((low, Vec3::new(11.0, 1.4, 10.0)))));
            let low = Vec3::new(10.0, 0.0, 20.0);
            nows[1].as_mut().unwrap().jump = Some(("route", Some((low, Vec3::new(11.0, 0.9, 20.0)))));
        }
        nows
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["presses"], 2);
    for (flag, count) in [
        ("TOO_LOW", 1),
        ("NO_SPRINT", 0),
        ("STAND_TURN", 1),
        ("SHORT_RUNUP", 0),
        ("FWD_RELEASED", 1),
        ("FWD_RELEASED_085", 0),
        ("NOT_UP", 1),
        ("PRESSED_LOW", 0),
        ("DROPPED", 0),
        ("IN_PLACE", 0),
    ] {
        assert_eq!(flags_of(&json, flag), count, "{flag}: {json}");
    }
    assert_eq!(json["jumps_wrong"], 1);
    assert_eq!(json["jumps_failed"], 1);
}

#[test]
pub(crate) fn presses_again_at_one_place_repeat_and_one_that_never_leaves_the_ground_dropped() {
    let (mut game, mut tally) = made_up(&[0, 0]);
    play(&mut game, &mut tally, 120, |k, game| {
        // Bot 0: three presses 1.5 s apart where it stands, each in the
        // air; bot 1: two presses 4 s apart (no repeat), with no key and
        // never in the air.
        let k0 = if [10, 40, 70].contains(&k) { key::JUMP | key::FORWARD } else { 0 };
        let k1 = if [10, 90].contains(&k) { key::JUMP } else { 0 };
        put(game, 0, 10.0, 10.0, k0);
        put(game, 1, 10.0, 20.0, k1);
        if [10, 40, 70].contains(&k) {
            game.bodies[0].air_until = game.tick + 600;
        }
        vec![bot_now(), bot_now()]
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["presses"], 5);
    assert_eq!(flags_of(&json, "REPEAT"), 2, "{json}");
    assert_eq!(flags_of(&json, "IN_PLACE"), 2);
    assert_eq!(flags_of(&json, "DROPPED"), 2);
}

#[test]
pub(crate) fn a_route_link_pressed_again_and_a_hop_that_frees_nothing_are_flagged() {
    let (mut game, mut tally) = made_up(&[0, 0, 0, 0]);
    play(&mut game, &mut tally, 200, |k, game| {
        let press = if [10, 110].contains(&k) { key::JUMP } else { 0 };
        put(game, 0, 10.0, 10.0, press);
        put(game, 1, 10.0, 20.0, press);
        put(game, 2, 30.0, 10.0, if k == 10 { key::JUMP } else { 0 });
        // Bot 3 (the near miss) walks off after its hop.
        let x3 = if k <= 10 { 30.0 } else { 30.0 + 0.1 * (k - 10) as f32 };
        put(game, 3, x3, 30.0, if k == 10 { key::JUMP } else { key::FORWARD });
        game.bodies[3].step_m = if k > 10 { 0.1 } else { 0.0 };
        let mut nows = vec![bot_now(); 4];
        let link = |z: f32, dx: f32| Some((Vec3::new(11.0 + dx, 0.0, z), Vec3::new(12.0 + dx, 1.0, z)));
        if k == 10 || k == 110 {
            // Bot 0 the same link twice, 5 s apart; bot 1 another 1 m on.
            nows[0].as_mut().unwrap().jump = Some(("route", link(10.0, 0.0)));
            let dx = if k == 10 { 0.0 } else { 1.0 };
            nows[1].as_mut().unwrap().jump = Some(("route", link(20.0, dx)));
        }
        if k == 10 {
            nows[2].as_mut().unwrap().jump = Some(("hop", None));
            nows[3].as_mut().unwrap().jump = Some(("hop", None));
        }
        nows
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(flags_of(&json, "LINK_AGAIN"), 1, "{json}");
    assert_eq!(flags_of(&json, "REPEAT"), 0);
    assert_eq!(flags_of(&json, "NOT_FREED"), 1);
}

#[test]
pub(crate) fn a_stall_at_a_ledge_with_no_press_is_a_missed_jump() {
    let grid = |rise: f32| {
        let triangles = super::super::ledge_triangles(rise, mapdata::WalkerBehaviour::BySlope);
        WalkGrid::build(&triangles, [0.0, 0.0, 20.0, 10.0], 0.25, mapdata::Body::default())
    };
    let (mut game, mut tally) = made_up(&[0, 0]);
    play(&mut game, &mut tally, 100, |k, game| {
        // Both push forward east at the ledge's foot; bot 1 (the near
        // miss) presses a jump 1.5 s in.
        put(game, 0, 9.6, 3.0, key::FORWARD);
        put(game, 1, 9.6, 7.0, if k == 30 { key::FORWARD | key::JUMP } else { key::FORWARD });
        for body in &mut game.bodies {
            body.yaw = -std::f32::consts::FRAC_PI_2;
        }
        vec![bot_now(), bot_now()]
    });
    let presses = tally.run.presses.clone();
    let high = grid(0.8);
    assert_eq!(tally.run.stalls(0, &high, &presses), [1, 0, 1]);
    assert_eq!(tally.run.stalls(1, &high, &presses), [0, 0, 1]);
    assert_eq!(tally.run.stalls(0, &grid(0.45), &presses), [0, 1, 1]);
    assert_eq!(tally.run.stalls(0, &grid(1.5), &presses), [0, 0, 1]);
}

impl Topic for JumpCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    /// The game's landings this tick are taken.
    fn begin(&mut self, _run: &Run, game: &mut Game) {
        self.landings.append(&mut game.landings);
    }

    fn parts(&mut self, run: &Run, game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        let presses = run.sorted_presses();
        vec![("jumps", self.part(run, game, &presses))]
    }
}

impl JumpCounts {

    /// The jumps counts at the end: their keys of the `owner` object and their lines of the owner-test text.
    fn part(&self, run: &Run, game: &Game, presses: &[Press]) -> Part {
        let n = run.recs.len();
        // Jumps: the flags of every press, the stalls.
        let mut flag_n: BTreeMap<&str, u32> = FLAGS.iter().map(|&f| (f, 0)).collect();
        let mut released_by: BTreeMap<String, u32> = BTreeMap::new();
        let (mut wrong, mut failed) = (0u32, 0u32);
        let mut by_kind: BTreeMap<&str, [u32; 2]> = BTreeMap::new();
        let mut route_landed = [0u32; 2];
        // The route jumps of a rise over the walk jump's top ([`crate::v026g::bots::motor::WALK_JUMP_TOP_M`]), by the honest
        // stand-in's own landing: presses, landed up, landed down, of those that were sprint jumps, and with no landing.
        let mut band = [0u32; 5];
        let (mut missed_pre, mut not_freed_pre) = (PreCounts::default(), PreCounts::default());
        for (i, p) in presses.iter().enumerate() {
            let flags = run.press_flags(presses, i);
            for &f in &flags {
                *flag_n.entry(f).or_default() += 1;
            }
            // Who was running the route jump whose forward key went up right after (A142's guard): the play and the hide.
            if flags.contains(&"FWD_RELEASED") {
                let recs = &run.recs[p.place];
                let play = recs.get(run.index(p.t).min(recs.len().saturating_sub(1))).map_or("none", |r| run.plays[usize::from(r.play)].as_str());
                *released_by.entry(format!("{play}/{}", p.cover)).or_default() += 1;
                if std::env::var_os("SRP_BENCH_FWD").is_some() {
                    let ri = run.index(p.t).min(recs.len().saturating_sub(1));
                    let name = |k: u32| {
                        let mut n = String::new();
                        for (bit, c) in [(key::FORWARD, 'F'), (key::BACK, 'B'), (key::LEFT, 'L'), (key::RIGHT, 'R'), (key::SPRINT, 'S'), (key::JUMP, 'J'), (key::CROUCH, 'C'), (key::FIRE, 'X'), (key::AIM, 'A')] {
                            if k & bit != 0 {
                                n.push(c);
                            }
                        }
                        if n.is_empty() { n.push('-') }
                        n
                    };
                    let after: Vec<String> = (ri..(ri + 10).min(recs.len())).map(|k| name(recs[k].keys)).collect();
                    let end = (ri + 20).min(recs.len().saturating_sub(1));
                    let moved = recs[ri].feet.distance_xz(recs[end].feet);
                    let rise = p.route.map_or(f32::NAN, |(a, b)| b.y - a.y);
                    let travel = recs[ri].travel;
                    let hold = recs[ri].hold;
                    eprintln!(
                        "FWDREL place={} t={} play={play} cover={} rise={rise:.2} speed={:.2} hold={hold} travel={travel} moved1s={moved:.2} landed_dy={:.2} left={:.1} goal_d={:.1} diag={} yaw={} keys={}",
                        p.place, p.t, p.cover, recs[ri].speed, recs[end].feet.y - recs[ri].feet.y, p.after_landing.map_or(-1.0, |l| l.0), p.after_landing.map_or(-1.0, |l| l.1), (ri..(ri + 10).min(recs.len())).map(|k| format!("{}.{:x}.{}", recs[k].walk, recs[k].why, u8::from(recs[k].has_route))).collect::<Vec<_>>().join(" "), (ri..(ri + 10).min(recs.len())).map(|k| format!("{:.0}", (recs[k].yaw - recs[ri].yaw).rem_euclid(std::f32::consts::TAU).to_degrees().min(360.0 - (recs[k].yaw - recs[ri].yaw).rem_euclid(std::f32::consts::TAU).to_degrees()))).collect::<Vec<_>>().join(","), after.join(" ")
                    );
                }
            }
            if flags.contains(&"NOT_FREED") {
                not_freed_pre.add(run, p.by, p.place, run.index(p.t));
            }
            let is_wrong = flags.iter().any(|f| !SECOND_NUMBERS.contains(f));
            wrong += u32::from(is_wrong);
            failed += u32::from(flags.iter().any(|f| OUTCOMES.contains(f) && !SECOND_NUMBERS.contains(f)));
            let entry = by_kind.entry(p.by).or_default();
            entry[0] += 1;
            entry[1] += u32::from(is_wrong);
            // The honest stand-in's own landing of a route jump: up or not.
            if let Some((_, to)) = p.route
                && let Some(l) = self.landings.iter().find(|l| l.place == p.place && l.at == p.t)
            {
                route_landed[usize::from(l.to.y < to.y - UP_SLACK_M)] += 1;
            }
            // `SRP_BENCH_JUMP_ROWS` (A187's residual check, local): one row per route jump: the link's rise, sprint held and the
            // body's speed at the press, a battery carried, where the honest stand-in came down and with which clip, the flags.
            if std::env::var_os("SRP_BENCH_JUMP_ROWS").is_some()
                && let Some((from, to)) = p.route
            {
                let recs = &run.recs[p.place];
                let ri = run.index(p.t).min(recs.len().saturating_sub(1));
                let (keys, speed) = recs.get(ri).map_or((0, 0.0), |r| (r.keys, r.speed));
                let landing = self.landings.iter().find(|l| l.place == p.place && l.at == p.t);
                eprintln!(
                    "jumprow\t{}\t{}\t{:.2}\t{:.2}\t{:.2}\t{:.3}\t{}\t{}\t{:.2}\t{}\t{}\t{}",
                    p.place,
                    p.t,
                    from.x,
                    from.y,
                    from.z,
                    to.y - from.y,
                    u8::from(keys & key::SPRINT != 0),
                    u8::from(p.carrying),
                    speed,
                    landing.map_or("none", |l| if l.to.y < to.y - UP_SLACK_M { "down" } else { "up" }),
                    landing.map_or("-", |l| if l.sprint { "sprint" } else { "short" }),
                    flags.join("+")
                );
            }
            if let Some((from, to)) = p.route
                && to.y - from.y > crate::v026g::bots::motor::WALK_JUMP_TOP_M
            {
                band[0] += 1;
                let landing = self.landings.iter().find(|l| l.place == p.place && l.at == p.t);
                match landing {
                    Some(l) => {
                        band[1 + usize::from(l.to.y < to.y - UP_SLACK_M)] += 1;
                        band[3] += u32::from(l.sprint);
                    }
                    None => band[4] += 1,
                }
                // The first 80 of a run, to the log (the trace of #673): where the press was against the take-off along the
                // link's way and across it, the link's direction, and where the body came down.
                static SEEN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
                if SEEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 80 {
                    let (dx, dz) = (to.x - from.x, to.z - from.z);
                    let len = dx.hypot(dz).max(1e-3);
                    let (ux, uz) = (dx / len, dz / len);
                    let (px, pz) = (p.feet.x - from.x, p.feet.z - from.z);
                    eprintln!(
                        "band-landing by {} take-off ({:.2}, {:.2}, {:.2}) landing ({:.2}, {:.2}, {:.2}) rise {:.2} link {:.0} deg; pressed {:.2} m along, {:.2} m across, {:.2} high; came down {}",
                        p.by,
                        from.x, from.y, from.z, to.x, to.y, to.z, to.y - from.y,
                        uz.atan2(ux).to_degrees(),
                        px * ux + pz * uz,
                        px * uz - pz * ux,
                        p.feet.y - from.y,
                        landing.map_or("nowhere".to_string(), |l| format!(
                            "{} at ({:.2}, {:.2}, {:.2}) {}",
                            if l.to.y < to.y - UP_SLACK_M { "DOWN" } else { "UP" },
                            l.to.x, l.to.y, l.to.z,
                            if l.sprint { "sprint clip" } else { "short clip" }
                        ))
                    );
                }
            }
        }
        // The route jumps up a plain ledge (a rise the walk jump makes), to the log: where the press was, how the body
        // faced and moved, the ground under it, the floor ahead, and where the stand-in's body came down (the trace of
        // the ledge press, test only).
        for p in presses {
            let Some((from, to)) = p.route else { continue };
            let rise = to.y - from.y;
            if !(0.5..=crate::v026g::bots::motor::WALK_JUMP_TOP_M).contains(&rise) {
                continue;
            }
            static SEEN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            if SEEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed) >= 2000 {
                continue;
            }
            let place_recs = &run.recs[p.place];
            let rec = &place_recs[run.index(p.t).min(place_recs.len() - 1)];
            let (dx, dz) = (to.x - from.x, to.z - from.z);
            let len = dx.hypot(dz).max(1e-3);
            let (ux, uz) = (dx / len, dz / len);
            let (px, pz) = (p.feet.x - from.x, p.feet.z - from.z);
            let (hx, hz) = (-rec.yaw.sin(), rec.yaw.cos());
            let off = (hx * uz - hz * ux).atan2(hx * ux + hz * uz).to_degrees();
            // The floors within 0.15 m of the feet: the lowest and highest ground, and the nearest higher floor ahead
            // along the link's way (a ledge's top, its cell centre).
            let grid = &game.world.grid;
            let r = 0.15;
            let near: Vec<f32> = grid
                .floors_in([p.feet.x - r, p.feet.z - r, p.feet.x + r, p.feet.z + r])
                .map(|n| grid.position(n).y)
                .filter(|y| (y - p.feet.y).abs() < 0.5)
                .collect();
            let low = near.iter().copied().fold(f32::INFINITY, f32::min);
            let high = near.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            let face = grid
                .floors_in([p.feet.x - 1.2, p.feet.z - 1.2, p.feet.x + 1.2, p.feet.z + 1.2])
                .filter_map(|n| {
                    let q = grid.position(n);
                    let (ex, ez) = (q.x - p.feet.x, q.z - p.feet.z);
                    let (along, across) = (ex * ux + ez * uz, (ex * uz - ez * ux).abs());
                    (q.y - p.feet.y >= 0.5 && q.y - p.feet.y <= 1.3 && across <= 0.15 && along > 0.0).then_some(along)
                })
                .fold(f32::INFINITY, f32::min);
            let landing = self.landings.iter().find(|l| l.place == p.place && l.at == p.t);
            eprintln!(
                "ledge-landing by {} take-off ({:.2}, {:.2}, {:.2}) landing ({:.2}, {:.2}, {:.2}) rise {:.2} link {:.0} deg; pressed {:.2} m along, {:.2} m across, feet {:.3} over the take-off, need {:.3}, ground {:.3}..{:.3}, higher floor {:.2} m ahead; faced {:+.0} deg off the link at {:.2} m/s keys {:#x}; came down {}",
                p.by,
                from.x, from.y, from.z, to.x, to.y, to.z, rise,
                uz.atan2(ux).to_degrees(),
                px * ux + pz * uz,
                px * uz - pz * ux,
                p.feet.y - from.y,
                to.y - p.feet.y,
                low - from.y,
                high - from.y,
                face,
                off,
                rec.speed,
                rec.keys,
                landing.map_or("nowhere".to_string(), |l| format!(
                    "{} at ({:.2}, {:.2}, {:.2}) {}",
                    if l.to.y < to.y - UP_SLACK_M { "DOWN" } else { "UP" },
                    l.to.x, l.to.y, l.to.z,
                    if l.sprint { "sprint clip" } else { "short clip" }
                ))
            );
        }
        let sprint_jumps = self.landings.iter().filter(|l| l.sprint).count() as u32;
        // The jumps' free falls by the client's fall damage (C14: the bench applies none; this says how often it would matter).
        let falls = self.landings.iter().filter_map(|l| l.fall_v).fold([0u32; 3], |mut c, v| {
            let d = fall_damage(v);
            c[0] += 1;
            c[1] += u32::from(d > 0.0);
            c[2] += u32::from(d >= LETHAL_FALL_DAMAGE);
            c
        });
        let gaps = self.gap_counts(run, &game.world.grid, presses);
        let mut stalls = [0u32; 3];
        for place in (0..n).filter(|&p| run.is_bot[p]) {
            let (counts, missed) = run.stall_scan(place, &game.world.grid, presses);
            for (c, s) in stalls.iter_mut().zip(counts) {
                *c += s;
            }
            for k0 in missed {
                missed_pre.add(run, "missed", place, k0);
            }
        }

        let mut json = serde_json::json!({
            "presses": presses.len(),
            "presses_by": by_kind,
            "jump_flags": flag_n,
            "jumps_wrong": wrong,
            "jumps_failed": failed,
            "missed_jump": stalls[0],
            "missed_jump_pre_5s": missed_pre.by_json(),
            "missed_jump_play": missed_pre.plays,
            "not_freed_pre_5s": not_freed_pre.by_json(),
            "not_freed_play": not_freed_pre.plays,
            "stall_at_step": stalls[1],
            "stalls": stalls[2],
            "route_landed_up": route_landed[0],
            "route_landed_down": route_landed[1],
            "sprint_jumps": sprint_jumps,
            "falls_free_hurt_lethal": falls,
            "band_jumps": band,
            "fwd_released_by_play_and_hide": released_by,
        });
        if let Some(g) = &gaps {
            json["gaps"] = g.json();
        }
        let flags_set: Vec<String> = flag_n
            .iter()
            .filter(|(_, c)| **c > 0)
            .map(|(f, c)| format!("{f} {c}"))
            .collect();
        let released_text = if released_by.is_empty() {
            String::new()
        } else {
            format!("; FWD_RELEASED by play/hide {released_by:?}")
        };
        let mut text = format!(
            "owner-test jumps: presses {} by [presses, wrong] {}; wrong {} failed {}; flags {}; missed jumps {} stalls at a step {} (stalls {}); route landings up {} down {}; sprint jumps {}; the 5 s before a missed jump: {}; before a press that freed nothing: {}",
            presses.len(),
            json["presses_by"],
            wrong,
            failed,
            flags_set.join(", "),
            stalls[0],
            stalls[1],
            stalls[2],
            route_landed[0],
            route_landed[1],
            sprint_jumps,
            missed_pre.text(),
            not_freed_pre.text(),
        );
        text.push_str(&released_text);
        if let Some(g) = &gaps {
            text.push_str(&g.text());
        }
        Part::new(json, text)
    }

    /// The route presses of gap links (`mapdata::grid::gap`), when the
    /// grid has any: how many, of which kind, how the stand-in landed
    /// them, which failed and why (the outcome flags), and how many came
    /// down more than [`HARMLESS_FALL_M`] under the take-off (hurt).
    pub(super) fn gap_counts(&self, run: &Run, grid: &WalkGrid, presses: &[Press]) -> Option<GapCounts> {
        if grid.off_links().is_empty() {
            return None;
        }
        let mut g = GapCounts {
            sprint: SprintGapCounts {
                built: grid.off_links().iter().filter(|l| matches!(l.press, mapdata::grid::gap::Takeoff::Sprint(_))).count() as u32,
                ..SprintGapCounts::default()
            },
            ..GapCounts::default()
        };
        for (i, p) in presses.iter().enumerate() {
            let Some((take_off, landing)) = p.route else {
                continue;
            };
            let (Some(a), Some(b)) = (grid.locate(take_off, LINK_M), grid.locate(landing, LINK_M)) else {
                continue;
            };
            let Some(link) = grid.off_link(a, b) else {
                continue;
            };
            let sprint = matches!(link.press, mapdata::grid::gap::Takeoff::Sprint(_));
            g.takes += 1;
            g.sprint.takes += u32::from(sprint);
            *g.by_kind.entry(format!("{:?}", link.kind)).or_default() += 1;
            if let Some(l) = self.landings.iter().find(|l| l.place == p.place && l.at == p.t) {
                let short = l.to.y < landing.y - UP_SLACK_M;
                let hurt = take_off.y - l.to.y > HARMLESS_FALL_M;
                g.landed[usize::from(short)] += 1;
                g.hurt += u32::from(hurt);
                g.sprint.landed_short += u32::from(sprint && short);
                g.sprint.hurt += u32::from(sprint && hurt);
            }
            let outcomes: Vec<&str> = run
                .press_flags(presses, i)
                .into_iter()
                .filter(|f| OUTCOMES.contains(f) && !SECOND_NUMBERS.contains(f))
                .collect();
            g.failed += u32::from(!outcomes.is_empty());
            g.sprint.failed += u32::from(sprint && !outcomes.is_empty());
            for f in outcomes {
                *g.failed_by.entry(f).or_default() += 1;
            }
        }
        Some(g)
    }
}

/// A fall from no higher than this hurts no one, m (`fall_params.harmless_height`,
/// PROVEN; `mapdata::Body::max_drop`).
pub(crate) const HARMLESS_FALL_M: f32 = 5.0;

/// The gap links' counts ([`OwnerTally::gap_counts`]).
#[derive(Default)]
pub(crate) struct GapCounts {
    pub(crate) takes: u32,
    pub(crate) by_kind: BTreeMap<String, u32>,
    /// Up on the landing's level, and not.
    pub(crate) landed: [u32; 2],
    pub(crate) failed: u32,
    pub(crate) failed_by: BTreeMap<&'static str, u32>,
    pub(crate) hurt: u32,
    /// Of them, the sprint jumps (`mapdata::grid::gap::Takeoff::Sprint`).
    pub(crate) sprint: SprintGapCounts,
}

/// The sprint gap links: built in the grid (activity), taken, and of those
/// taken the harm: landed under the landing's level, failed, hurt.
#[derive(Default)]
pub(crate) struct SprintGapCounts {
    pub(crate) built: u32,
    pub(crate) takes: u32,
    pub(crate) landed_short: u32,
    pub(crate) failed: u32,
    pub(crate) hurt: u32,
}

impl GapCounts {
    fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "gap_takes": self.takes,
            "gap_takes_by": self.by_kind,
            "gap_landed_up": self.landed[0],
            "gap_landed_down": self.landed[1],
            "gap_failed": self.failed,
            "gap_failed_by": self.failed_by,
            "gap_hurt": self.hurt,
            "sprint_gap_built": self.sprint.built,
            "sprint_gap_takes": self.sprint.takes,
            "sprint_gap_landed_short": self.sprint.landed_short,
            "sprint_gap_failed": self.sprint.failed,
            "sprint_gap_hurt": self.sprint.hurt,
        })
    }

    fn text(&self) -> String {
        format!(
            "; gap links: takes {} by {:?}, landed up {} down {}, failed {} by {:?}, hurt {}; sprint gap links: built {} takes {} landed short {} failed {} hurt {}",
            self.takes,
            self.by_kind,
            self.landed[0],
            self.landed[1],
            self.failed,
            self.failed_by,
            self.hurt,
            self.sprint.built,
            self.sprint.takes,
            self.sprint.landed_short,
            self.sprint.failed,
            self.sprint.hurt
        )
    }
}

