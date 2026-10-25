//! Pick up a battery lying about and bring it home: the battery modes'
//! point, and §4's "our carrier killed: the nearest bot goes for the
//! dropped battery" (a dropped battery lies about like any).
//!
//! One bot per battery (its slot). Taking it is a use on the key going
//! down (bit 21, PROVEN instant with a ~0.6 s animation); carrying, the
//! bot cannot shoot (`Intent::hold_fire`) and is slower; at its own
//! container, another use puts it in. The worlds say whether it carries
//! (`PlaceView::carrying`, up to two seconds late), so each use is tried a
//! few times at most: twice where it stopped, then at other stands round
//! the battery (`take`), as a player steps round a battery he cannot get.
//!
//! With the battery cheat off (`CheatTuning::batteries`) a bot goes only
//! for batteries its team knows of (`Ctx::battery_lying`), and while some
//! batteries are not accounted for it looks for them at the spawners' spots
//! as a human does (`TargetRef::Spawner`): a spot is checked once a bot of
//! the team sees it (`sense::sense_batteries`), and no more bots look than
//! batteries are missing. A spot weighs the chance a look there finds a
//! battery it can take (spots no way leads to all match count as where the
//! missing ones may lie; the enemy clears his side as the match runs),
//! less where the team keeps dying and deep in the enemy's third. Where an
//! enemy carrier the team knew of died, the team looks first
//! (`LookSpot`, `TargetRef::Point`).

use super::*;
use crate::v026g::bots::curve::{Consideration, Curve, Input};
use crate::v026g::bots::view;
use crate::v026g::drive::key;

/// How far over a lying battery's position a bot aims to take it: inside
/// its box whether the position is the box's foot or its middle (the box
/// is about 0.25 m high).
pub const BATTERY_AIM_M: f32 = 0.1;
/// How near the eye comes to what it uses: the client's use is a ray of
/// 2 m from the eye (`base_player::detect_usable_object`, PROVEN), so a
/// bot that stopped farther (the walk's arrival is loose, the spot to stand
/// can be off the battery) steps in closer while it presses.
pub const USE_NEAR_M: f32 = 1.6;
/// The steepest look down a take is tried at, radians: the view's pitch
/// stops at 1 rad (`kin`, PROVEN), so from a crouch (the eye 0.95 m up)
/// over a battery on the floor no ray passes within the use's 0.2 m of
/// its middle closer than about 0.3 m on the ground (`bots::tests`): that
/// is a look down of about 1.2 rad. Past this the bot steps off first.
pub const TAKE_PITCH_MAX_RAD: f32 = 1.15;
/// The look down a take stands for, radians: the eye about 1.2 m from a
/// battery on the floor, the use's ray 1.5 m long. A take walks in to
/// there, and of the stands round the battery prefers one near it.
pub const TAKE_PITCH_RAD: f32 = 0.6;

/// How far off a battery `below` metres under the eye the look down at
/// it is `pitch`, on the ground.
fn take_flat_m(below: f32, pitch: f32) -> f32 {
    below.max(0.0) / pitch.tan()
}

/// A stand round a battery is this near another to be the same, metres.
const SAME_STAND_M: f32 = 0.35;
/// The ring of stands round a battery tried after a stand failed: eight
/// ways at these distances off it, metres.
const RING_M: [f32; 3] = [0.6, 0.9, 1.2];
/// From a ring's stand the look down at the battery is at most this,
/// radians (the view's pitch stops at 1 rad, `kin`)...
const RING_PITCH_MAX_RAD: f32 = 0.95;
/// ... and the eye at most this far from it (the use's ray is 2 m; the
/// body stops up to [`AT_STAND_M`] off the stand).
const RING_USE_M: f32 = 1.7;
/// Where it stopped, a use is pressed if the look down is at most this,
/// radians: the view stops at 1 rad, and in the soaks uses at the stop
/// over a steeper look passed 0.33-0.36 m from the battery's middle.
const FIRST_PITCH_MAX_RAD: f32 = 0.98;
/// A stand is reached within this up or down, metres (one on a crate is
/// not reached from the floor beside it).
const AT_STAND_UP_M: f32 = 0.4;
/// A stand is reached within this, metres.
const AT_STAND_M: f32 = 0.25;
/// A bot this far below the floor it would stand on for a take is on
/// another floor (it fell off the ledge the battery lies by, or stands
/// under an overhang), metres. Once a stand there failed, the take goes up
/// by the route, not round the battery by straight steps: in 0x1b8ee975
/// battery 4 lay 236 s on a walk 2.8 m over a floor that runs under it, and
/// bots tried their stands under it (one for 36 s). A take from below that
/// works is left alone (on the bench, gating every take from 1.2 m below
/// cost 3.5 pickups a seed of 66).
pub const BELOW_STAND_M: f32 = 1.2;
/// A stand this near in a clear line is walked to straight, by keys
/// (`Intent::straight`); the route's arrival (the motor's `arrive_m`,
/// 0.6 m) is too loose for a stand.
const STRAIGHT_M: f32 = 2.5;
/// The line from the eye to the battery is clear if nothing is hit short
/// of it by more than this, metres (what it lies on or against).
const USE_LINE_SLACK_M: f32 = 0.15;
/// The way it came: where it last stood at least this far off the
/// battery, metres; a step back out of a nook goes that way...
const FROM_M: f32 = 0.8;
/// ... until this far off the battery, metres, or for at most this, ms.
const BACKED_M: f32 = 0.6;
const BACKING_MS: u32 = 2_000;
/// A stand failed is kept this long, ms.
const FAILED_KEEP_MS: u32 = 90_000;
/// At a stand, a use is pressed once the view is on the battery within
/// this, radians, or after this long there anyway, ms.
const SETTLED_RAD: f32 = 0.06;
const SETTLE_MS: u32 = 1_500;
/// A use (a take, a put-in) is judged by a world of this long after it, ms,
/// before it counts as failed or the next is pressed: the world showing
/// it comes a second or two after the press (a world a second, behind the
/// clock), and the bench's put-ins "failed" at the second press while the
/// first had put the battery in, to be taken again and end "put it in" a
/// second later (291 of 442 put-in plays, 8 seeds; GUESS: a press is in
/// the next server frame).
const USE_SEEN_MS: u32 = 300;
/// ... unless no such world came for this long (the worlds stopped), ms.
const USE_SEEN_WAIT_MS: u32 = 3_000;

/// No fire this long after a use pressed at a battery: the take may have
/// worked, and `PlaceView::carrying` comes with the worlds, up to 2 s
/// after it; a fire press while carrying drops the battery in the game
/// (0x2D8E73..0x2D8EC8, PROVEN). In the bench's traces every fire drop
/// left was 0.45-0.85 s after a take. GUESS: the worlds' 2 s.
const TAKE_HOLD_FIRE_MS: u32 = 2_000;
/// A carrier letting go of its battery presses again (fire, which drops it
/// as the drop key does: 0x2D8E73..0x2D8EC8, PROVEN) if the view still
/// shows it carried this long after: past the worlds' lag. GUESS.
const LET_GO_AGAIN_MS: u32 = 2_500;
/// A put-in takes this long before the battery is in, ms: the battery's put
/// state lasts `states[2].put_time`, 0.6 s (`docs/notes/battery_pickup.md`;
/// a take's battery is in hand at once). PROVEN that the state lasts it;
/// LIKELY that the battery goes in at its end: in the owner's Cologne match
/// of 2026-10-24 all 6 put-ins judged failed after [`USE_SEEN_MS`] went in
/// 0.3-3.3 s later, by the same carrier.
const PUT_TIME_MS: u32 = 600;
/// A put-in's use whose ray (the client's, 2 m from the eye) passes the
/// use box's middle farther than this, metres, missed it: no put began, and
/// the next use need not wait for one. GUESS: the box's size is not known
/// (the bench's stand-in takes 0.6 m); 1 m is well outside it.
const PUT_MISS_M: f32 = 1.0;
/// The client's use ray, metres (`base_player::detect_usable_object`).
const USE_RAY_M: f32 = 2.0;

/// How far the bot's use ray, from its eye along its view, passes `point`.
fn use_ray_miss(cx: &Ctx, point: Vec3) -> f32 {
    let eye = cx.me.eye();
    let [fx, fz] = crate::v026g::bots::kin::forward(cx.me.yaw);
    let (sin, cos) = cx.me.pitch.sin_cos();
    let dir = Vec3::new(fx * cos, sin, fz * cos);
    let along = (point - eye).dot(dir).clamp(0.0, USE_RAY_M);
    (eye + dir * along - point).length()
}

/// Whether a use pressed at `pressed` (0: none yet), whose effect shows
/// `lasts` ms after the press, has been judged by a world
/// ([`USE_SEEN_MS`] after that).
fn use_seen(cx: &Ctx, pressed: u32, lasts: u32) -> bool {
    pressed == 0
        || cx.view.world_time >= pressed + lasts + USE_SEEN_MS
        || cx.now >= pressed + lasts + USE_SEEN_WAIT_MS
}
/// Each use at a stand aims at the next of these heights over the
/// battery's position (as the eye's height is a GUESS, `view`).
const AIM_LIFTS_M: [f32; 2] = [BATTERY_AIM_M, 0.0];

/// A stand a take of a battery gave up at (its uses spent, the battery
/// still lying there): the bot keeps it for a while, so its next take of
/// that battery stands elsewhere (in the owner's match of 2026-10-16 a
/// bot took the same play and stand 9 times).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FailedStand {
    pub battery: usize,
    /// Where the battery lay.
    pub at: Vec3,
    pub feet: Vec3,
    pub crouch: bool,
    pub until: u32,
    /// The last use's ray, for the log: from the eye to the battery's
    /// point aimed at (metres; the client's ray is 2 m), how far down
    /// (radians), and how far the view was off it (radians).
    pub ray_m: f32,
    pub down: f32,
    pub off: f32,
}

impl FailedStand {
    /// Its line in the log, with where the bot believed the battery lay
    /// (to hold against the worlds: a belief left behind by a battery
    /// that moved is aimed at and missed).
    pub fn note(&self) -> String {
        format!(
            "took no battery {} from a stand {:.1} m off it ({}): its use's ray {:.2} m to it, {:.0} deg down, the view {:.1} deg off it; it was believed at ({:.2}, {:.2}, {:.2})",
            self.battery,
            self.feet.distance_xz(self.at),
            if self.crouch { "crouched" } else { "standing" },
            self.ray_m,
            self.down.to_degrees(),
            self.off.to_degrees(),
            self.at.x,
            self.at.y,
            self.at.z,
        )
    }
}

/// Where and how a body stands to take a battery.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TakeStand {
    pub feet: Vec3,
    pub crouch: bool,
}

impl TakeStand {
    fn same(&self, feet: Vec3, crouch: bool) -> bool {
        self.crouch == crouch && self.feet.distance_xz(feet) < SAME_STAND_M
    }

    fn eye(&self) -> Vec3 {
        let up = if self.crouch {
            view::EYE_CROUCHED
        } else {
            view::EYE_STANDING
        };
        self.feet + Vec3::new(0.0, up, 0.0)
    }
}

/// How a use of a battery (its point `aim`) goes from a stand.
#[derive(Clone, Copy, Debug, PartialEq)]
enum UseFrom {
    /// The ray reaches it in a clear line; the look down, radians.
    Clear(f32),
    /// Right over it: the view does not look down that far.
    Steep,
    /// Farther than `max_m` from the eye.
    Far,
    /// Something lies between the eye and it (on the bullet mesh).
    Blocked,
}

fn use_from(cx: &Ctx, stand: TakeStand, aim: Vec3, max_m: f32, max_pitch: f32) -> UseFrom {
    let eye = stand.eye();
    let d = aim - eye;
    let length = d.length();
    let pitch = (-d.y).atan2(d.x.hypot(d.z));
    if pitch > max_pitch {
        return UseFrom::Steep;
    }
    if length > max_m {
        return UseFrom::Far;
    }
    if length > USE_LINE_SLACK_M
        && cx
            .tactics
            .world
            .sight
            .ray_cast(eye, d * (1.0 / length), length - USE_LINE_SLACK_M)
            .is_some()
    {
        return UseFrom::Blocked;
    }
    UseFrom::Clear(pitch)
}

/// The stands round a battery lying at `at` (its point to use `aim`): a
/// ring of eight ways at [`RING_M`], on floors a walk may get to, crouched
/// and standing, from which the eye is within the use's reach in a clear
/// line, with their look down. On a crossing (`crossing`), a zone's
/// floors are stands too, and no way need lead to them: they are walked to
/// in a line.
fn ring_stands(cx: &Ctx, at: Vec3, aim: Vec3, crossing: bool) -> Vec<(TakeStand, f32)> {
    let grid = &cx.tactics.world.grid;
    let mut out: Vec<(TakeStand, f32)> = Vec::new();
    for k in 0..8 {
        let a = k as f32 * std::f32::consts::FRAC_PI_4;
        for r in RING_M {
            let p = Vec3::new(at.x + r * a.cos(), at.y, at.z + r * a.sin());
            let Some(node) = grid.locate(p, 0.5) else {
                continue;
            };
            let feet = grid.position(node);
            if feet.distance_xz(p) > 0.3
                || (feet.y - at.y).abs() > 2.5
                || !cx.tactics.walkable(node)
                || !if crossing {
                    cx.tactics.spawn_ok(node, cx.bot.side, false)
                        && grid
                            .locate(at, 1.5)
                            .is_none_or(|home| !traps(cx.tactics, home, node))
                } else {
                    cx.tactics.allowed(node, cx.now, cx.bot.side, false)
                }
            {
                continue;
            }
            for crouch in [true, false] {
                if out.iter().any(|(s, _)| s.same(feet, crouch)) {
                    continue;
                }
                let stand = TakeStand { feet, crouch };
                if let UseFrom::Clear(pitch) =
                    use_from(cx, stand, aim, RING_USE_M, RING_PITCH_MAX_RAD)
                    && (crossing || cx.reachable(feet, false))
                {
                    out.push((stand, pitch));
                }
            }
        }
    }
    out
}

/// Whether the feet are over the floor a walk to `at` ends on
/// (`path::goal_floor`) by more than `motor::ARRIVE_ABOVE_M` (an overhang, a
/// box top), the motor's rule for a route's end (`motor::arrived`). Under
/// it is not asked: Take goes up by the route itself once a stand there
/// failed (`BELOW_STAND_M`), and refusing the take from under it cost the
/// batt bench stuck +9.1 (5.4 SE) and quick ends +17.9 (2.3 SE) at 16 seeds.
/// No floor: false.
fn over_goal(cx: &Ctx, at: Vec3) -> bool {
    use crate::v026g::bots::motor::ARRIVE_ABOVE_M;
    let grid = &cx.tactics.world.grid;
    crate::v026g::bots::path::goal_floor(grid, at)
        .is_some_and(|node| cx.me.feet.y - grid.position(node).y > ARRIVE_ABOVE_M)
}

/// Where a body right over a battery lying at `at` steps back to: a metre
/// off it the way it came (`from`), else behind its back.
fn back_spot(cx: &Ctx, at: Vec3, from: Option<Vec3>) -> Vec3 {
    let (dx, dz) = match from {
        Some(from) if from.distance_xz(at) > 0.05 => (from.x - at.x, from.z - at.z),
        _ => {
            let [fx, fz] = crate::v026g::bots::kin::forward(cx.me.yaw);
            (-fx, -fz)
        }
    };
    let n = dx.hypot(dz).max(1e-3);
    Vec3::new(at.x + dx / n, cx.me.feet.y, at.z + dz / n)
}

pub const NUMBERS: &[(&str, f32)] = &[
    ("reach_m", 1.5),
    // A bot going for a battery that is over 2 m off its reckoning within
    // `fall_near_m` of it fell there (a ledge by it); after `falls_aside`
    // such falls of the team within `falls_window_s`, the battery is not
    // offered for `falls_aside_s` after the last (0: never).
    ("fall_near_m", 8.0),
    ("falls_aside", 3.0),
    ("falls_window_s", 120.0),
    ("falls_aside_s", 60.0),
    // Within `hold_near_m` of the battery, a take waits while an enemy
    // within `exposed_m` sees the bot, not reloading (a reload pressed
    // within `reload_cover_s`) and kept busy by no teammate with a clear
    // line to him from within `cover_m` (`Ctx::exposed_to`): it fights
    // first. GUESSES; measured on the battery soak and the real-map bench
    // (docs/notes/player_bots_behaviour.md §2.3).
    ("hold_near_m", 3.0),
    ("exposed_m", 25.0),
    ("reload_cover_s", 1.5),
    ("cover_m", 40.0),
    ("use_every_s", 1.2),
    // Uses pressed at one stand; stands tried a take (the first where it
    // stopped, then round the battery, `ring_stands`); how long it may
    // take to reach a stand, seconds.
    ("tries", 2.0),
    ("stands", 4.0),
    ("stand_walk_s", 5.0),
    ("give_up_s", 120.0),
    // A spawner's spot weighs this times the chance a missing battery lies
    // in sight of it (missing batteries x spots worth a look within
    // `cluster_m` of it / all spots worth a look), at least `search_min`.
    ("search_weight", 0.9),
    ("search_min", 0.25),
    ("cluster_m", 30.0),
    // A spot within this of one a teammate is going to look at is his.
    ("spread_m", 20.0),
    // A spot whose chance times its risk (below) is under this is not
    // looked at: a search that may find nothing takeable stops.
    ("search_cut", 0.1),
    // The enemy clears his side: a spot's prior falls by up to this share,
    // the more the nearer his container (from the map's middle on) and
    // the longer the match has run (fully after `clear_s`).
    ("clear_share", 0.8),
    ("clear_s", 120.0),
    // Each loss of the team within `danger_m` of a spot, or of a bot on
    // its way to it, takes this share off (fading over `LOSS_KEEP_MS`).
    ("danger_cut", 0.4),
    ("danger_m", 25.0),
    // A spot in the enemy's third loses up to this share, the deeper the
    // more (at his container all of it).
    ("deep_cut", 0.5),
    // A look where an enemy carrier died with a battery (`LookSpot`):
    // this times its score, above any spawner's spot.
    ("look_weight", 1.0),
    // A look round our container after a theft whose thief dropped the
    // battery alive (`LookSpot::theft`): this times its score.
    ("theft_look_weight", 1.0),
    // A carrier's way home (`way_home`): by its side's human route that
    // passes within `home_route_m` of it, when the straight way has known
    // danger (an enemy seen, a teammate killed lately) within
    // `home_danger_m` of it and the route's way has less, at most
    // `home_detour` times as long. GUESSES; measured on the real-map bench.
    ("home_route_m", 20.0),
    ("home_danger_m", 20.0),
    ("home_detour", 1.4),
];

/// Spawners' slots come after the batteries'.
const SPAWNER_SLOT: u32 = 0x1_0000;
/// Look spots' slots (by battery) after the spawners'.
const LOOK_SLOT: u32 = 0x2_0000;
/// The one slot of all the team's looks after a theft dropped alive.
const THEFT_LOOK_SLOT: u32 = 0x3_0000;

/// Where `at` lies between the team's container (0) and the enemy's (1),
/// along the line between them; none without both containers.
fn toward_enemy(cx: &Ctx, at: Vec3) -> Option<f32> {
    let ours = cx.view.container_of(cx.me.team)?.at;
    let theirs = cx.view.containers.iter().find(|c| c.team != cx.me.team)?.at;
    let (dx, dz) = (theirs.x - ours.x, theirs.z - ours.z);
    let length = dx * dx + dz * dz;
    if length < 1.0 {
        return None;
    }
    Some((((at.x - ours.x) * dx + (at.z - ours.z) * dz) / length).clamp(0.0, 1.0))
}

/// How likely a spawner's spot not yet seen still holds a missing battery,
/// against the others: a spot no way leads to all match (`shut`) keeps
/// what lies there (1); else the enemy may have taken it, the more likely
/// the nearer his container and the longer the match has run.
fn search_prior(cx: &Ctx, at: Vec3, shut: bool) -> f32 {
    if shut {
        return 1.0;
    }
    let num = |name| cx.num(PlayKind::Collect, name);
    let theirs = toward_enemy(cx, at).map_or(0.0, |s| ((s - 0.5) * 2.0).clamp(0.0, 1.0));
    let ran = (cx.now as f32 / 1000.0 / num("clear_s").max(1.0)).min(1.0);
    1.0 - num("clear_share") * theirs * ran
}

/// A spot's worth for the team's deaths lately: each loss within
/// `danger_m` of it, or of a bot sent to look at it, takes `danger_cut`
/// off, fading over [`LOSS_KEEP_MS`] (the kill feed; what a team sees).
fn search_danger(cx: &Ctx, index: usize, at: Vec3) -> f32 {
    use crate::v026g::bots::know::LOSS_KEEP_MS;
    let num = |name| cx.num(PlayKind::Collect, name);
    let (cut, near) = (num("danger_cut"), num("danger_m"));
    cx.team
        .losses
        .iter()
        .filter(|loss| loss.spawner == Some(index) || loss.at.distance_xz(at) <= near)
        .map(|loss| {
            let fresh = 1.0 - cx.now.saturating_sub(loss.time) as f32 / LOSS_KEEP_MS as f32;
            1.0 - cut * fresh.clamp(0.0, 1.0)
        })
        .product()
}

/// A spot's worth for how deep it lies in the enemy's third.
fn search_depth(cx: &Ctx, at: Vec3) -> f32 {
    let deep = toward_enemy(cx, at).map_or(0.0, |s| ((s - 2.0 / 3.0) * 3.0).clamp(0.0, 1.0));
    1.0 - cx.num(PlayKind::Collect, "deep_cut") * deep
}

pub fn tuning() -> PlayTuning {
    PlayTuning {
        rank: Rank::Objective,
        considerations: vec![
            considerations::near(200.0),
            considerations::health_above(0.25),
            considerations::modifier(Input::ObjectiveUrgency, 0.6),
            Consideration::new(Input::EnemiesNearTarget, 0.0, ENEMIES_OUT, Curve::Linear)
                .inverted(),
        ],
        numbers: Default::default(),
    }
}

/// The enemies known by a battery (within 25 m, [`Input::EnemiesNearTarget`])
/// at which it scores out of Collect: the consideration's top in [`tuning`].
const ENEMIES_OUT: f32 = 4.0;

/// How many enemies over [`ENEMIES_OUT`] a bot already on a battery's
/// Collect stands before the battery scores out for it: while
/// [`ENEMIES_OUT`] or more are known by it, the bot reads this many fewer. A
/// battery out comes back only below [`ENEMIES_OUT`] (hysteresis).
///
/// GUESS (2026-10-25): one enemy. Why: the count is of the bot's remembered
/// enemies (`Memory::tracks`), whose spots move with sight, hearing and
/// callouts and which drop when not sensed for a while, so the 4th one's
/// spot crossing 25 m, or his track dropping, flipped the battery in and
/// out. level_02 day 2 10v10, 16 seeds (a182512a): 1,135 times out for
/// "enemies by it", a median 2.5 s out, 47% back within 2 s; the bot left
/// its Collect each time. A bot that is not on the battery yet reads the
/// count as it is.
///
/// Only while the bot has no enemy in sight (noticed): one in sight with the
/// count at the cut-off is a fight by a crowded battery, not a track's spot
/// moving. Held at all times, the deaths on a Collect with 4 or more enemies
/// within 25 m doubled (batt 4-5/9-10/13-15 x 16 seeds: 19 -> 40, 31 -> 48,
/// 23 -> 40); of the deaths the bot was held through (28, 34, 24), 25, 28
/// and 23 had an enemy in sight. A hold only near the battery would have
/// missed the flicker: of 1,267 drop-outs for the enemies, 898 came with the
/// bot over 25 m off it and 70 within 10 m.
pub const COLLECT_ENEMY_MARGIN: f32 = 1.0;

/// Bot's Collect candidate for lying battery `battery` (`None`: scored out
/// or set aside), with [`COLLECT_ENEMY_MARGIN`] for the one it is on while
/// it has no enemy in sight.
fn battery_candidate(cx: &Ctx, battery: usize) -> Option<Candidate> {
    let target = TargetRef::Battery(battery);
    let slot = Some(battery as u32);
    let on_it = matches!(cx.bot.current, Some((PlayKind::Collect, t, _)) if t == target);
    let near = cx.input(Input::EnemiesNearTarget, PlayKind::Collect, target);
    // A crossing's damage weighs against it (`Crossing::cost`): a dry
    // battery as good wins.
    let dry = cx
        .battery_lying(battery)
        .and_then(|at| cx.crossing(at))
        .map_or(1.0, |c| (1.0 - c.cost() / cx.me.health.max(1e-3)).max(0.0));
    battery_candidate_given(cx, target, slot, on_it, near).map(|mut candidate| {
        if dry < 1.0 {
            candidate.score = Score::new(candidate.score.get() * dry);
        }
        candidate
    })
}

/// [`battery_candidate`] before a crossing's cost.
fn battery_candidate_given(
    cx: &Ctx,
    target: TargetRef,
    slot: Option<u32>,
    on_it: bool,
    near: f32,
) -> Option<Candidate> {
    if on_it && near >= ENEMIES_OUT && cx.memory.all_seen(cx.now).is_empty() {
        let read = near - COLLECT_ENEMY_MARGIN;
        cx.candidate_given(
            PlayKind::Collect,
            target,
            slot,
            &[(Input::EnemiesNearTarget, read)],
        )
    } else {
        cx.candidate(PlayKind::Collect, target, slot)
    }
}

/// A crossing's walk across a zone's floor, m/s: the walk is 3.5 m/s
/// forward (`kin`, PROVEN); less for side steps, the 0.3 s blend and a
/// carrier's load. GUESS.
pub const CROSS_MPS: f32 = 2.5;
/// A crossing's dry edge is looked for this far off what it crosses to at
/// most, metres, along [`CROSS_WAYS`] ways; the line's damage is summed
/// every [`CROSS_STEP_M`]. GUESS: of the real maps' batteries in zones on
/// all match, those any dry floor sees in a walkable line lie within
/// about 20 m of one (the survey in `docs/notes/player_bots_batteries.md`).
const CROSS_MAX_M: f32 = 25.0;
const CROSS_WAYS: usize = 16;
pub const CROSS_STEP_M: f32 = 0.5;
/// A take's walk in stops this short of a battery on the floor, metres:
/// it stops where the look down is [`TAKE_PITCH_RAD`], about 1.2 m off
/// (`take_flat_m`), and uses from there; a little less, to be safe. GUESS.
const CROSS_STOP_M: f32 = 1.0;
/// A take stands this long at the battery, s: the last metre, the aim, up
/// to two uses `use_every_s` apart. GUESS (the bench's takes, 2-4 s).
const CROSS_TAKE_S: f32 = 4.0;
/// A crossing is made only if, in and out and the take, it costs at most
/// this share of the bot's health...
const CROSS_SHARE: f32 = 0.5;
/// ... and leaves it at least this. GUESSES: never lethal, the damage is
/// the zones' raw hit (before armour) and the marks' 1 m margin counts.
const CROSS_LEFT: f32 = 0.4;
/// In a zone, a take gives up once the walk back out would leave less
/// than this. GUESS.
pub const CROSS_ABORT_LEFT: f32 = 0.25;

/// A walk in a line across zones on all match to a battery no way leads
/// to (`Ctx::needs_crossing`), from a dry floor a way does lead to; or a
/// carrier's walk back out. The zones' damage is the game data's
/// (`Hazards::hit_per_s`, a zone with none counts as lethal).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crossing {
    /// The dry floor it starts from (or, out, ends at).
    pub edge: Vec3,
    /// Health lost on the walk one way, at [`CROSS_MPS`].
    pub leg: f32,
    /// Health lost each second standing where the walk in stops (short of
    /// a battery, [`CROSS_STOP_M`]).
    pub at_per_s: f32,
}

impl Crossing {
    /// Health it costs in and out and a take.
    pub fn cost(&self) -> f32 {
        2.0 * self.leg + CROSS_TAKE_S * self.at_per_s
    }

    /// Whether a bot of `health` makes it ([`bears`]).
    pub fn fits(&self, health: f32) -> bool {
        bears(self.cost(), health)
    }
}

/// Whether a bot of `health` bears a walk across zones that costs it `cost`
/// of its health: at most [`CROSS_SHARE`] of it, leaving at least
/// [`CROSS_LEFT`]. The one rule for walks across zones on all match: a
/// battery's crossing ([`Crossing::fits`]) and a route's
/// (`crossing::choose`).
pub fn bears(cost: f32, health: f32) -> bool {
    cost <= CROSS_SHARE * health && health - cost >= CROSS_LEFT
}

/// How far short of its stand a take's walk in stops ([`CROSS_STOP_M`]):
/// on the floor by the battery; none for a stand beside a shelf.
fn stop_short(stand: Vec3, at: Vec3) -> f32 {
    if (stand - at).length() < 0.01 {
        CROSS_STOP_M
    } else {
        0.0
    }
}

/// Whether a walk from floor `from` to floor `node` ends in a pocket of
/// its own (not `from`'s) with no way out by a bot's own jump
/// (`escape::standing_exit`): a pit inside a zone on all match, where a
/// crossing would leave the bot to die.
fn traps(tactics: &Tactics, from: mapdata::NodeId, node: mapdata::NodeId) -> bool {
    let grid = &tactics.world.grid;
    grid.pocket_of(node).is_some_and(|pocket| {
        Some(pocket) != grid.pocket_of(from)
            && !crate::v026g::bots::escape::standing_exit(grid, &tactics.escapes, pocket)
    })
}

/// Health lost walking from `a` to `b` in a line, at [`CROSS_MPS`].
fn line_hurt(tactics: &Tactics, a: Vec3, b: Vec3) -> f32 {
    let grid = &tactics.world.grid;
    let d = a.distance_xz(b);
    let n = (d / CROSS_STEP_M).ceil().max(1.0) as usize;
    let ds = d / n as f32;
    (0..n)
        .map(|i| {
            let p = a + (b - a) * ((i as f32 + 0.5) / n as f32);
            grid.locate(p, 1.5)
                .map_or(0.0, |node| tactics.hazards.hit_per_s(node))
                * ds
                / CROSS_MPS
        })
        .sum()
}

/// The cheapest crossing to `to` for a body of map side `side`, its walk
/// in stopping `stop_m` short of it: of the dry floors a way leads to (past
/// zones on all match, out of the zones' marks, not the other side's
/// spawn) nearest `to` along each of [`CROSS_WAYS`] ways within
/// [`CROSS_MAX_M`], in a line walked to it.
pub fn crossing_to(
    tactics: &Tactics,
    to: Vec3,
    side: Option<i32>,
    stop_m: f32,
) -> Option<Crossing> {
    let grid = &tactics.world.grid;
    let mut best: Option<Crossing> = None;
    for k in 0..CROSS_WAYS {
        let a = k as f32 * std::f32::consts::TAU / CROSS_WAYS as f32;
        let (sin, cos) = a.sin_cos();
        for r in 1..=CROSS_MAX_M as usize {
            let p = Vec3::new(to.x + r as f32 * cos, to.y, to.z + r as f32 * sin);
            let Some(node) = grid.locate(p, 2.0) else {
                continue;
            };
            let edge = grid.position(node);
            if (edge.y - to.y).abs() > 3.0
                || !tactics.hazards.hazard_free(node)
                || !tactics.past_fixed_zones(node)
                || !tactics.spawn_ok(node, side, false)
            {
                continue;
            }
            // The nearest dry floor this way: past it, the line is longer.
            // Not into a pit it cannot jump out of.
            let into_pit = grid
                .locate(to, 1.5)
                .is_some_and(|goal| traps(tactics, node, goal));
            if !into_pit && tactics.clear_line_through_zones(edge, to, side, false) {
                let d = edge.distance_xz(to);
                let stop = if d > stop_m {
                    to + (edge - to) * (stop_m / d)
                } else {
                    edge
                };
                let crossing = Crossing {
                    edge,
                    leg: line_hurt(tactics, edge, stop),
                    at_per_s: grid
                        .locate(stop, 1.5)
                        .map_or(0.0, |node| tactics.hazards.hit_per_s(node)),
                };
                if crossing.cost().is_finite() && best.is_none_or(|b| crossing.cost() < b.cost()) {
                    best = Some(crossing);
                }
            }
            break;
        }
    }
    best
}

impl Ctx<'_> {
    /// Whether no way leads to the stand at `stand` all match, though a
    /// walk gets there with the zones off: zones on all match lie on it or
    /// round it (level_01's swamp, level_06's acid). Such a battery is
    /// taken by a crossing ([`crossing_to`]) or not at all.
    pub fn needs_crossing(&self, stand: Vec3) -> bool {
        let grid = &self.tactics.world.grid;
        let Some(goal) = grid.locate(stand, 3.0) else {
            return false;
        };
        let past = |b| self.tactics.past_fixed_zones(b);
        self.tactics.walkable(goal) && !(past(goal) || grid.entered_from(goal, past))
    }

    /// The crossing to take a battery lying at `at` by (its stand needs
    /// one, [`Ctx::needs_crossing`]), if one is found.
    pub fn crossing(&self, at: Vec3) -> Option<Crossing> {
        let stand = self.tactics.stand_to_take(at)?;
        if !self.needs_crossing(stand) {
            return None;
        }
        // Kept for the match (`memo::Memo::crossing`): the same answer.
        self.memo
            .crossing(self.tactics, stand, self.bot.side, stop_short(stand, at))
    }

    /// The bot's way out of zones on all match when no way leads from where
    /// it stands ([`Ctx::needs_crossing`]): its cheapest crossing.
    fn way_out(&self) -> Option<Crossing> {
        let me = self.me.feet;
        if !self.needs_crossing(me) {
            return None;
        }
        crossing_to(self.tactics, me, self.bot.side, 0.0)
    }
}

/// Why a battery is no Collect candidate when its stand is in a pocket
/// with no way out for a carrier (no standing jump, and no teammate got out
/// by one): whoever took it could not bring it home.
pub const POCKET_BLOCK: &str = "its stand is in a pocket a carrier cannot leave";

impl Ctx<'_> {
    /// Whether a battery lying at `at` may be taken: a floor to stand on
    /// for it (one on a shelf is taken from the floor beside it) that a way
    /// may lead to ([`Ctx::reachable`]), and no search of this bot or its
    /// team proved no way to it lately ([`Ctx::no_way_lately`]). A battery
    /// no way leads to now (inside a hazard) is left for later. Ambush
    /// asks it too: a battery nobody can take is not waited at. Nor is
    /// one the team keeps falling by ([`TeamKnowledge::fallen_at`]), nor
    /// one whose stand is in a pocket no carrier gets out of
    /// ([`POCKET_BLOCK`]).
    pub fn can_take(&self, at: Vec3) -> bool {
        self.take_block(at).is_none()
    }

    /// Why a battery lying at `at` may not be taken ([`Ctx::can_take`]),
    /// the first reason found; `None`: it may.
    pub fn take_block(&self, at: Vec3) -> Option<&'static str> {
        let num = |name| self.num(PlayKind::Collect, name);
        let Some(stand) = self.tactics.stand_to_take(at) else {
            // No way is planned into a pocket the grid knows no way out of,
            // so none of its floors is a stand; a battery there is its.
            let grid = &self.tactics.world.grid;
            if grid
                .locate(at, 1.5)
                .is_some_and(|node| grid.in_shut_pocket(node))
            {
                return Some(POCKET_BLOCK);
            }
            return Some("no stand to take it from");
        };
        // Its stand in zones on all match: by a crossing's dry edge, if the
        // bot's health bears it.
        let stand = if self.needs_crossing(stand) {
            let Some(crossing) = self.crossing(at) else {
                return Some("no crossing to its zone");
            };
            if !crossing.fits(self.me.health) {
                return Some("its zone hurts too much");
            }
            crossing.edge
        } else {
            stand
        };
        if !self.reachable(stand, false) {
            return Some("no way to its stand");
        }
        if self.no_way_lately(stand) {
            return Some("no way to its stand lately");
        }
        // Its taker carries it out: a sprint jump is no way (Cologne CG-11,
        // pit A). No expiry: the pocket's ways out are the grid's, and a
        // teammate's escape there counts once made (`escape::way_out`).
        if self.in_pocket(stand) && !self.escape_known(stand, true) {
            return Some(POCKET_BLOCK);
        }
        self.team
            .fallen_at(
                at,
                self.now,
                num("falls_aside") as usize,
                (num("falls_window_s") * 1000.0) as u32,
                (num("falls_aside_s") * 1000.0) as u32,
            )
            .then_some("the team keeps falling by it")
    }
}

/// A bot's known lying batteries and why each is no Collect candidate
/// ([`take_blocks`]), for the notes.
pub struct TakeBlocks {
    now: u32,
    /// Battery, why it is not offered (`None`: it is), the bot's ground
    /// distance to it (m).
    list: Vec<(usize, Option<&'static str>, f32)>,
}

/// Each battery the bot believes lies about, and why it is not offered a
/// Collect of it (`None`: it is): [`Ctx::take_block`], else set aside or
/// scored out. `offered`: the bot's candidates already made this pass; a
/// battery with a Collect of it is read, not asked again (empty: all
/// asked). Empty while it carries one.
pub fn take_blocks(cx: &Ctx, offered: &[Candidate]) -> TakeBlocks {
    let mut list = Vec::new();
    if cx.view.mode.has_batteries() && cx.me.carrying.is_none() {
        for battery in 0..cx.view.batteries.len() {
            let Some(at) = cx.battery_lying(battery) else {
                continue;
            };
            let target = TargetRef::Battery(battery);
            // Offered already this pass (`candidates`): no block.
            let offered = offered
                .iter()
                .any(|c| c.kind == PlayKind::Collect && c.target == target);
            let why = (!offered)
                .then(|| {
                    cx.take_block(at).or_else(|| {
                        let aside = cx.bot.set_aside.iter().any(|&(k, t, until)| {
                            k == PlayKind::Collect && t == target && cx.now < until
                        });
                        battery_candidate(cx, battery).is_none().then(|| {
                            if aside {
                                "set aside"
                            } else {
                                scored_out(cx, target)
                            }
                        })
                    })
                })
                .flatten();
            list.push((battery, why, cx.me.feet.distance_xz(at)));
        }
    }
    TakeBlocks { now: cx.now, list }
}

/// Which of Collect's considerations scored `target` out (its first 0), for
/// the note: the way's length, the bot's health, or the enemies by it.
fn scored_out(cx: &Ctx, target: TargetRef) -> &'static str {
    let play = cx.play(PlayKind::Collect);
    let (_, why) = crate::v026g::bots::curve::score(&play.considerations, |input| {
        cx.input(input, PlayKind::Collect, target)
    });
    match why.parts.last().map(|&(input, _, _)| input) {
        Some(Input::PathDist) => "scored out: its way",
        Some(Input::Health) => "scored out: hurt",
        Some(Input::EnemiesNearTarget) => "scored out: enemies by it",
        _ => "scored out",
    }
}

/// The notes for bot `place`'s [`take_blocks`] where they changed since
/// the last (kept in [`TeamKnowledge::take_blocked`]): a battery that left
/// its candidates and why, or came back, with the bot's distance to it and
/// how long it was a candidate (or out) before. A battery no longer believed
/// to lie about is dropped without a note (the knowledge's own note says
/// why).
pub fn note_take_blocks(
    team: &mut TeamKnowledge,
    place: Place,
    blocks: &TakeBlocks,
) -> Vec<String> {
    let now = blocks.now;
    let was = |battery: usize| {
        team.take_blocked
            .iter()
            .find(|&&(p, b, _, _)| p == place && b == battery)
            .map(|&(_, _, why, since)| (why, since))
    };
    let held_s = |since: u32| now.saturating_sub(since) as f32 / 1000.0;
    let mut notes = Vec::new();
    let mut rows = Vec::new();
    for &(battery, why, dist) in &blocks.list {
        let before = was(battery);
        let since = match (before, why) {
            (Some((before, since)), why) if before == why => since,
            (before, Some(why)) => {
                let for_s = match before {
                    Some((None, since)) => format!(", a candidate for {:.1} s", held_s(since)),
                    Some((Some(_), since)) => format!(", out for {:.1} s", held_s(since)),
                    None => String::new(),
                };
                notes.push(format!(
                    "battery {battery} is no Collect candidate: {why} ({dist:.0} m off{for_s})"
                ));
                // Out for another reason: still out since then.
                match before {
                    Some((Some(_), since)) => since,
                    _ => now,
                }
            }
            (Some((Some(_), since)), None) => {
                notes.push(format!(
                    "battery {battery} is a Collect candidate again ({dist:.0} m off, out for {:.1} s)",
                    held_s(since)
                ));
                now
            }
            (_, None) => now,
        };
        rows.push((place, battery, why, since));
    }
    team.take_blocked.retain(|&(p, _, _, _)| p != place);
    team.take_blocked.extend(rows);
    notes
}

pub fn candidates(cx: &Ctx, out: &mut Vec<Candidate>) {
    if !cx.view.mode.has_batteries() {
        return;
    }
    // Carrying already: bring it home, at the top of the objective band.
    // Hurt or among enemies too: what scores a battery to fetch does not
    // score one in hand, and a carrier given another play fires, which in
    // the game drops it. In 919 human carries under fire, those who kept
    // carrying did no worse than those who dropped.
    if let Some(battery) = cx.me.carrying {
        // Its escape from a pocket given up, the commander gives it no
        // slotted play (`team::assign_trapped`): no slot, so its Collect
        // stays to let go of the battery (`State::LetGo`). Nobody else can
        // take a battery in its hands anyway.
        let gave_up = cx.trapped().is_some_and(|t| t.gave_up);
        if let Some(mut candidate) = cx.candidate_given(
            PlayKind::Collect,
            TargetRef::Battery(battery),
            (!gave_up).then_some(battery as u32),
            &[(Input::Health, 1.0), (Input::EnemiesNearTarget, 0.0)],
        ) {
            candidate.rank = Rank::Alarm;
            out.push(candidate);
        }
        return;
    }
    for battery in 0..cx.view.batteries.len() {
        if cx.battery_lying(battery).is_some_and(|at| cx.can_take(at)) {
            out.extend(battery_candidate(cx, battery));
        }
    }
    if cx.tuning.cheats.batteries {
        return;
    }
    // A battery an enemy carrier dropped where he died, which the team
    // cannot see: a look where the team last sensed him, near the top of
    // the band (one bot a spot).
    // A battery of ours a thief dropped alive: the spots round our
    // container and where the team last sensed him, all one bot's (one
    // slot), each spot once.
    for (index, look) in cx.team.looks.iter().enumerate() {
        if cx.team.battery(look.battery).is_some()
            || cx.team.looks[..index].iter().any(|l| l.at == look.at)
            || !cx.can_take(look.at)
        {
            continue;
        }
        let (slot, weight) = if look.theft {
            (THEFT_LOOK_SLOT, "theft_look_weight")
        } else {
            (LOOK_SLOT + look.battery as u32, "look_weight")
        };
        if let Some(mut candidate) =
            cx.candidate(PlayKind::Collect, TargetRef::Point(look.at), Some(slot))
        {
            candidate.score = Score::new(candidate.score.get() * cx.num(PlayKind::Collect, weight));
            if !candidate.score.is_zero() {
                out.push(candidate);
            }
        }
    }
    // The batteries the team cannot place are looked for at the spawners'
    // spots, by no more bots than are missing, apart from each other, the
    // spots with more spots in sight of them first (one look checks them
    // all, as a human looks round a yard).
    let missing = cx.batteries_unaccounted();
    let searching = matches!(
        cx.bot.current,
        Some((PlayKind::Collect, TargetRef::Spawner(_), _))
    );
    let others: Vec<usize> = cx
        .team
        .searching
        .iter()
        .filter(|(place, _)| *place != cx.me.place)
        .map(|(_, index)| *index)
        .collect();
    if missing == 0 || (!searching && others.len() >= missing) {
        return;
    }
    let spawners = &cx.tactics.world.map.battery_spawners;
    let worth: Vec<usize> = (0..spawners.len())
        .filter(|&i| cx.spawner_worth_checking(i))
        .collect();
    if worth.is_empty() {
        return;
    }
    let num = |name| cx.num(PlayKind::Collect, name);
    let (cluster, spread) = (num("cluster_m"), num("spread_m"));
    // How likely each spot not yet seen still holds a missing battery
    // (`search_prior`): a spot no way leads to all match surely keeps
    // what lies there; one far on the enemy's side was likely cleared by
    // the enemy since the start.
    let shut: Vec<bool> = worth
        .iter()
        .map(|&i| cx.shut_all_match(spawners[i].item))
        .collect();
    let prior: Vec<f32> = worth
        .iter()
        .zip(&shut)
        .map(|(&i, &shut)| search_prior(cx, spawners[i].item, shut))
        .collect();
    let total: f32 = prior.iter().sum();
    if total <= 0.0 {
        return;
    }
    for (k, &i) in worth.iter().enumerate() {
        let item = spawners[i].item;
        // Its own spot it keeps (else bots near each other trade spots
        // every tick); a new one is not one near a teammate's.
        let own = matches!(cx.bot.current, Some((_, TargetRef::Spawner(j), _)) if j == i);
        let taken = others.iter().any(|&j| {
            spawners
                .get(j)
                .is_some_and(|s| s.item.distance_xz(item) < spread)
        });
        if shut[k] || (taken && !own) || !cx.can_take(item) {
            continue;
        }
        // The missing batteries a look round here may find and take: the
        // reachable spots in sight of it, by their share of all spots
        // not seen yet (the shut ones too, so the missing ones may well be
        // there).
        let near: f32 = (0..worth.len())
            .filter(|&m| !shut[m] && spawners[worth[m]].item.distance_xz(item) <= cluster)
            .map(|m| prior[m])
            .sum();
        let chance = (missing as f32 * near / total).min(1.0);
        // Worth less where the team keeps dying lately, and deep in the
        // enemy's third.
        let risk = search_danger(cx, i, item) * search_depth(cx, item);
        if chance * risk < num("search_cut") {
            continue;
        }
        let weight = (num("search_weight") * chance).max(num("search_min")) * risk;
        if let Some(mut candidate) = cx.candidate(
            PlayKind::Collect,
            TargetRef::Spawner(i),
            Some(SPAWNER_SLOT + i as u32),
        ) {
            candidate.score = Score::new(candidate.score.get() * weight);
            if !candidate.score.is_zero() {
                out.push(candidate);
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    /// Going to look at a spawner's spot.
    Search,
    Fetch,
    Take {
        /// Uses pressed at this stand.
        tries: u8,
        last_use: u32,
        /// The stand it goes to or uses from; `None` until the first: where
        /// it stopped, if a use reaches from there.
        stand: Option<TakeStand>,
        /// Since when it heads for this stand, and since when it stands
        /// there.
        since: u32,
        arrived: Option<u32>,
        /// The stands tried this play.
        tried: Vec<TakeStand>,
        /// Where it last stood a step off the battery: the way it came.
        from: Option<Vec3>,
        /// Stepping back out of a nook since (no straight line to a stand).
        backing: Option<u32>,
        /// A crossing's dry edge (`Crossing`): the battery lies in zones on
        /// all match, walked into in lines.
        crossing: Option<Vec3>,
    },
    /// Home with the battery; `via`: the points of a safer way home along
    /// a human route (`way_home`), chosen at the first step.
    Bring {
        via: Option<Vec<Vec3>>,
    },
    /// Trapped in a pocket, its escape given up (`escape.rs`): lets go of
    /// the battery (pressed at `pressed`) so it may fight from where it
    /// stands; then the play ends.
    LetGo {
        pressed: u32,
    },
    PutIn {
        tries: u8,
        last_use: u32,
        /// Since when it stands within a use of the box (`None`: walking up).
        arrived: Option<u32>,
        /// Its last use's ray provably passed the box ([`PUT_MISS_M`]): no
        /// put began, so no put is waited for.
        missed: bool,
    },
}

pub fn start(target: TargetRef, cx: &Ctx, _rng: &mut Rng) -> State {
    if cx.me.carrying.is_some() {
        State::Bring { via: None }
    } else if matches!(target, TargetRef::Spawner(_) | TargetRef::Point(_)) {
        State::Search
    } else {
        State::Fetch
    }
}

pub fn step(state: &mut State, target: TargetRef, cx: &Ctx, _rng: &mut Rng) -> Step {
    let num = |name| cx.num(PlayKind::Collect, name);
    let every = (num("use_every_s") * 1000.0) as u32;
    let tries = num("tries") as u8;
    // A carrier trapped in a pocket is not given up on: the escape has its
    // own clock (`escape::GIVE_UP_MS`), and then it lets go (`LetGo`).
    let escaping = (cx.me.carrying.is_some() || matches!(state, State::LetGo { .. }))
        && cx.trapped().is_some();
    if let Some((_, _, started)) = cx.bot.current
        && cx.now > started + (num("give_up_s") * 1000.0) as u32
        && !escaping
    {
        return Step::Failed("took too long");
    }
    let home = cx.view.container_of(cx.me.team).map(|c| c.at);
    match state {
        State::Search | State::Fetch | State::Take { .. } if cx.me.carrying.is_some() => {
            *state = State::Bring { via: None };
            step(state, target, cx, _rng)
        }
        State::Search => {
            let Some(at) = cx.point_of(target) else {
                return Step::Failed("no spawner");
            };
            match target {
                TargetRef::Spawner(index) => {
                    let started = cx.bot.current.map_or(0, |(_, _, since)| since);
                    if cx
                        .team
                        .spawner_seen(index)
                        .is_some_and(|seen| seen >= started)
                    {
                        // Whatever lay there is known now: the commander
                        // picks again.
                        return Step::Done("looked at the spot");
                    }
                    if !cx.spawner_worth_checking(index) {
                        return Step::Done("the spot needs no look");
                    }
                }
                // Where an enemy carrier died: looked at once a bot of the
                // team stood by it, or the battery is known (seen there).
                TargetRef::Point(_) => {
                    if !cx.team.looks.iter().any(|look| look.at == at) {
                        return Step::Done("looked where the carrier died");
                    }
                }
                _ => return Step::Failed("no spawner"),
            }
            let Some(stand) = cx.tactics.stand_to_take(at) else {
                return Step::Failed("no way to it");
            };
            if !cx.can_go(stand) {
                return Step::Failed("no way to it");
            }
            Step::Continue(Intent::to(stand, cx.pace_toward(stand)).within(num("reach_m") * 0.6))
        }
        State::Fetch => {
            let Some(at) = cx.point_of(target) else {
                return Step::Failed("the battery is gone");
            };
            if !matches!(target, TargetRef::Battery(b) if cx.battery_lying(b).is_some()) {
                return Step::Failed("someone took it");
            }
            // A hazard switched on round it (the cloud's energy rose).
            let Some(stand) = cx.tactics.stand_to_take(at) else {
                return Step::Failed("no way to it");
            };
            // In zones on all match: to its crossing's dry edge first
            // (`Crossing`), while the bot's health bears it.
            let crossing = if cx.needs_crossing(stand) {
                match cx.crossing(at) {
                    Some(crossing) if crossing.fits(cx.me.health) => Some(crossing.edge),
                    _ => return Step::Failed("no way to it"),
                }
            } else {
                None
            };
            let stand = crossing.unwrap_or(stand);
            // Or every way in crosses a hazard (a battery ringed by fire):
            // the search said so, and the bot does something else.
            if !cx.can_go(stand) {
                return Step::Failed("no way to it");
            }
            // Within reach across and not over the stand's floor
            // (`over_goal`, the motor's rule for a route's end).
            let near = cx.me.feet.distance_xz(stand) <= num("reach_m");
            if near && !over_goal(cx, stand) {
                *state = State::Take {
                    tries: 0,
                    last_use: 0,
                    stand: None,
                    since: cx.now,
                    arrived: None,
                    tried: Vec::new(),
                    from: (cx.me.feet.distance_xz(at) >= FROM_M).then_some(cx.me.feet),
                    backing: None,
                    crossing,
                };
                return Step::Continue(Intent::stay().looking(Look::At(at)));
            }
            // Near across but over its floor (an overhang, a box top): a
            // goal within reach across is no goal, so the walk
            // asks for the stand itself, and the route ends there only at its
            // floor's height (`motor::arrived`). Cologne CG-17.
            let within = if near { 0.0 } else { num("reach_m") * 0.6 };
            Step::Continue(Intent::to(stand, cx.pace_toward(stand)).within(within))
        }
        State::Take {
            tried, crossing, ..
        } => {
            // On a crossing, the zone's floor is the play's choice
            // (`Intent::zone_ok`): no way out of it is walked meanwhile.
            let crossing = crossing.is_some();
            // Under its floor (fallen, or under an overhang), a stand there
            // failed: up by the route, however near across (a goal within
            // reach across is no goal); the take goes on once up.
            if !crossing
                && !tried.is_empty()
                && let TargetRef::Battery(battery) = target
                && let Some(stand) = cx
                    .battery_lying(battery)
                    .and_then(|at| cx.tactics.stand_to_take(at))
                && cx.me.feet.y < stand.y - BELOW_STAND_M
            {
                return Step::Continue(Intent::to(stand, cx.pace_toward(stand)).within(0.0));
            }
            let step = match take(state, target, cx) {
                Step::Continue(intent) if crossing => Step::Continue(intent.zone_ok()),
                step => step,
            };
            // A use lately: it may carry already without knowing it.
            match (step, &*state) {
                (Step::Continue(intent), State::Take { last_use, .. })
                    if *last_use > 0 && cx.now < *last_use + TAKE_HOLD_FIRE_MS =>
                {
                    Step::Continue(intent.hold_fire())
                }
                (step, _) => step,
            }
        }
        State::Bring { via } => {
            if cx.me.carrying.is_none() {
                return Step::Done(carry_ended(cx, target, "brought home"));
            }
            let Some(home) = home else {
                return Step::Failed("no container of ours");
            };
            // In zones on all match with no way out (a crossing's take):
            // out first by the cheapest crossing, in a line. Not while its
            // motor walks a crossing or a ring's walk out home (`crossing.rs`):
            // one way at a time. On Mamayev Kurgan's acid the two took turns,
            // a poll each (the line to a floor 0.5 m up it never stepped
            // onto, the route 8 m on toward home), and a carrier stood 60 s
            // in it, from 0.66 health to 0.38.
            if cx.bot.crossing.is_none()
                && let Some(out) = cx.way_out()
            {
                return Step::Continue(Intent::to(out.edge, Pace::Walk).within(0.5).straight());
            }
            // Trapped for good: no way home with it. Let go, and fight
            // from here: a carrier cannot fire. A pocket some player got
            // out of leaves it takeable (`Ctx::take_block`).
            if cx.trapped().is_some_and(|t| t.gave_up) {
                *state = State::LetGo { pressed: cx.now };
                return Step::Continue(Intent::stay().pressing(key::DROP).let_go());
            }
            if cx.me.feet.distance_xz(home) <= num("reach_m") + 0.5 {
                *state = State::PutIn {
                    tries: 0,
                    last_use: 0,
                    arrived: None,
                    missed: false,
                };
            } else {
                let points = via.get_or_insert_with(|| way_home(cx, home));
                // A point reached, one its search found no way to lately
                // (`Ctx::no_way_lately`: else retried every replan), or one
                // a teammate was killed by since: on to the next (the
                // straight way home when none is left).
                let reach = num("reach_m") * 3.0;
                let near = num("home_danger_m") * 0.5;
                while points.first().is_some_and(|&p| {
                    cx.me.feet.distance_xz(p) < reach
                        || cx.no_way_lately(p)
                        || cx
                            .team
                            .danger_within(cx.now, 10_000)
                            .any(|d| d.at.distance_xz(p) < near)
                }) {
                    points.remove(0);
                }
                if let Some(&at) = points.first() {
                    return Step::Continue(
                        Intent::to(at, Pace::Walk).within(reach * 0.6).hold_fire(),
                    );
                }
            }
            // A carrier cannot shoot, and does not sprint (LIKELY).
            Step::Continue(
                Intent::to(home, Pace::Walk)
                    .within(num("reach_m"))
                    .hold_fire(),
            )
        }
        State::LetGo { pressed } => {
            if cx.me.carrying.is_none() {
                return Step::Done("trapped: let go of it");
            }
            // The view's lag (up to 2 s), or a lost press: again, by fire,
            // which drops it in the game as the drop key does.
            let mut intent = Intent::stay().let_go();
            if cx.now >= *pressed + LET_GO_AGAIN_MS {
                *pressed = cx.now;
                intent = intent.pressing(key::FIRE);
            }
            Step::Continue(intent)
        }
        State::PutIn {
            tries: n,
            last_use,
            arrived,
            missed,
        } => {
            if cx.me.carrying.is_none() {
                return Step::Done(carry_ended(cx, target, "put it in"));
            }
            // The last use not seen yet: it may have put it in; one whose
            // ray passed the box began no put.
            let seen = *missed || use_seen(cx, *last_use, PUT_TIME_MS);
            if *n >= tries && seen {
                return Step::Failed("could not put it in");
            }
            let mut intent = Intent::stay().hold_fire();
            let mut aimed = home.is_none();
            if let Some(home) = home {
                // Its use box's middle is a metre over `home`
                // (`Container::use_point`).
                let aim = home + Vec3::new(0.0, 1.0, 0.0);
                let eye = cx.me.feet + Vec3::new(0.0, view::EYE_STANDING, 0.0);
                if (aim - eye).length() > USE_NEAR_M {
                    intent = Intent::to(home, Pace::Walk).within(0.5).hold_fire();
                    *arrived = None;
                } else {
                    // Pressed once the view is on the box, as a take's
                    // (or after a while there anyway): a use pressed while
                    // still turning to it missed it.
                    let since = *arrived.get_or_insert(cx.now);
                    aimed = aimed_at(cx, aim) || cx.now >= since + SETTLE_MS;
                }
                intent = intent.looking(Look::use_box(home));
            }
            if cx.now >= *last_use + every && seen && *n < tries && aimed {
                *n += 1;
                *last_use = cx.now;
                *missed = home.is_some_and(|home| {
                    use_ray_miss(cx, home + Vec3::new(0.0, 1.0, 0.0)) > PUT_MISS_M
                });
                intent = intent.pressing(key::USE);
            }
            Step::Continue(intent)
        }
    }
}

/// Why a carry ended with the bot no longer carrying: `home` if the battery
/// is in a container; else the view believes another carries it (its uses
/// at the container put nothing in, `view::Carry`), or it lies dropped.
fn carry_ended(cx: &Ctx, target: TargetRef, home: &'static str) -> &'static str {
    match target {
        TargetRef::Battery(b) => match cx.view.batteries.get(b) {
            Some(view::BatteryState::Carried(_)) => "someone else carries it",
            Some(view::BatteryState::Free(_)) => "dropped it",
            _ => home,
        },
        _ => home,
    }
}

/// A carrier's safer way home: the points, nearest the carrier first, of
/// its side's human route (`routes.rs`, walked back toward its base) that
/// passes within `home_route_m` of it, when the straight way home has known
/// danger near it (enemies the bot knows of, teammates killed in the last
/// 20 s, within `home_danger_m`) and that way less, at most `home_detour`
/// times as long; of those the least dangerous, then the shortest. None
/// (the straight way) otherwise. Points out of bounds for the side (a
/// hazard, the enemy's spawn area), and those no way leads to now or the
/// bot's search found none to lately, are left out.
fn way_home(cx: &Ctx, home: Vec3) -> Vec<Vec3> {
    let num = |name| cx.num(PlayKind::Collect, name);
    let Some(side) = cx.bot.side else {
        return Vec::new();
    };
    let me = cx.me.feet;
    let near = num("home_danger_m");
    let dangers: Vec<Vec3> = cx
        .memory
        .tracks()
        .iter()
        .map(|t| t.at)
        .chain(cx.team.danger_within(cx.now, 20_000).map(|d| d.at))
        .collect();
    // Known dangers within `near` of a way through these points.
    let danger_of = |way: &[Vec3]| {
        dangers
            .iter()
            .filter(|&&d| {
                way.windows(2).any(|leg| {
                    let (a, b) = (leg[0], leg[1]);
                    let (dx, dz) = (b.x - a.x, b.z - a.z);
                    let l = dx * dx + dz * dz;
                    let t = if l > 0.0 {
                        (((d.x - a.x) * dx + (d.z - a.z) * dz) / l).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    d.distance_xz(Vec3::new(a.x + dx * t, d.y, a.z + dz * t)) < near
                })
            })
            .count()
    };
    let length = |way: &[Vec3]| {
        way.windows(2)
            .map(|leg| leg[0].distance_xz(leg[1]))
            .sum::<f32>()
    };
    let direct = danger_of(&[me, home]);
    if direct == 0 {
        return Vec::new();
    }
    let to_home = me.distance_xz(home);
    let most = num("home_detour") * to_home;
    cx.tactics
        .routes_of(side)
        .filter_map(|route| {
            let points = &route.points;
            let i = (0..points.len()).min_by(|&a, &b| {
                points[a]
                    .distance_xz(me)
                    .total_cmp(&points[b].distance_xz(me))
            })?;
            if points[i].distance_xz(me) > num("home_route_m") {
                return None;
            }
            // Back toward the base: the points nearer home than the bot.
            let back: Vec<Vec3> = points[..=i]
                .iter()
                .rev()
                .copied()
                .filter(|&p| p.distance_xz(home) < to_home)
                .filter(|&p| cx.tactics.fine_goal(p, cx.now, Some(side)))
                .filter(|&p| cx.can_go(p))
                .collect();
            if back.is_empty() {
                return None;
            }
            let way: Vec<Vec3> = std::iter::once(me)
                .chain(back.iter().copied())
                .chain(std::iter::once(home))
                .collect();
            let (danger, long) = (danger_of(&way), length(&way));
            (danger < direct && long <= most).then_some((danger, long, back))
        })
        .min_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)))
        .map(|(_, _, back)| back)
        .unwrap_or_default()
}

/// A take: uses at the stand where it stopped; when they fail (or no use
/// reaches from there: the battery under a ledge, right under its feet),
/// at other stands round the battery, crouched and standing in turn, none
/// it gave up at lately (`BotFacts::failed_stands`); wedged right over
/// it with no line to walk off by, a step back the way it came.
fn take(state: &mut State, target: TargetRef, cx: &Ctx) -> Step {
    let num = |name| cx.num(PlayKind::Collect, name);
    let every = (num("use_every_s") * 1000.0) as u32;
    let per_stand = num("tries") as u8;
    let stands = num("stands") as usize;
    let walk_ms = (num("stand_walk_s") * 1000.0) as u32;
    let State::Take {
        tries,
        last_use,
        stand,
        since,
        arrived,
        tried,
        from,
        backing,
        crossing,
    } = state
    else {
        unreachable!("a take's state");
    };
    let crossing = *crossing;
    // Seen gone at last (it was taken since the team saw it), or taken.
    let TargetRef::Battery(battery) = target else {
        return Step::Failed("the battery is gone");
    };
    let Some(at) = cx.battery_lying(battery) else {
        return Step::Failed(if cx.tuning.cheats.batteries {
            "someone took it"
        } else {
            "the battery is gone"
        });
    };
    let me = cx.me.feet;
    // On a crossing, out once the way out would leave too little health
    // ([`CROSS_ABORT_LEFT`]), at the current health (enemy fire counts):
    // the crossing a carrier walks out by from here (`Ctx::way_out`), else
    // the line back to its edge. The motor walks it out of the zone once
    // the play ends (no `Intent::zone_ok`).
    if let Some(edge) = crossing
        && cx.me.health
            - crossing_to(cx.tactics, me, cx.bot.side, 0.0)
                .map_or_else(|| line_hurt(cx.tactics, me, edge), |out| out.leg)
            < CROSS_ABORT_LEFT
    {
        return Step::Failed("its zone hurts too much");
    }
    // In an enemy's sight, the fight first (`Ctx::exposed_to`): the play
    // stays, its look gives way to the reflex's (no use within reach, so
    // it aims, moves and fires), and its timers wait.
    if me.distance_xz(at) <= num("hold_near_m")
        && cx
            .exposed_to(
                num("exposed_m"),
                (num("reload_cover_s") * 1000.0) as u32,
                num("cover_m"),
            )
            .is_some()
    {
        *since = cx.now;
        *arrived = None;
        return Step::Continue(Intent::stay());
    }
    // The use's ray from the eye must hit the battery: at it, not a metre
    // over it as at a place (`Look::At`), which from a crouch passes over
    // a battery on the floor.
    let aim = at + Vec3::new(0.0, BATTERY_AIM_M, 0.0);
    if me.distance_xz(at) >= FROM_M {
        *from = Some(me);
    }
    let gave_up = |s: TakeStand| {
        cx.bot.failed_stands.iter().any(|f| {
            f.until > cx.now
                && f.battery == battery
                && (f.at - at).length() < 0.5
                && TakeStand {
                    feet: f.feet,
                    crouch: f.crouch,
                }
                .same(s.feet, s.crouch)
        })
    };
    // Stepping back out of a nook: straight, no line looked at.
    if let Some(started) = *backing {
        if me.distance_xz(at) < BACKED_M && cx.now < started + BACKING_MS {
            return Step::Continue(back_off(cx, at, aim, *from));
        }
        *backing = None;
        *since = cx.now;
    }
    let mut failed = None;
    // The first stand: where it stopped, if a use reaches from there.
    if stand.is_none() && tried.is_empty() {
        let here = TakeStand {
            feet: me,
            crouch: true,
        };
        match use_from(cx, here, aim, USE_NEAR_M, FIRST_PITCH_MAX_RAD) {
            UseFrom::Clear(_) if !gave_up(here) => {
                *stand = Some(here);
                *since = cx.now;
            }
            UseFrom::Far
                if cx.now
                    < *since
                        + walk_ms
                        + crossing.map_or(0, |edge| {
                            (edge.distance_xz(at) / CROSS_MPS * 1000.0) as u32
                        }) =>
            {
                // On the floor: stopped short of it, where the look down
                // is easy; on a shelf: at the floor beside it.
                let Some(to) = cx.tactics.stand_to_take(at) else {
                    return Step::Failed("no way to it");
                };
                let below = me.y + view::EYE_CROUCHED - aim.y;
                let within = if (to - at).length() < 0.01 {
                    take_flat_m(below, TAKE_PITCH_RAD).max(0.25)
                } else {
                    0.25
                };
                let mut intent = Intent::to(to, Pace::Walk).within(within);
                // Across a zone, upright till near (a crouch walks slower).
                if crossing.is_none() || me.distance_xz(to) <= STRAIGHT_M {
                    intent = intent.crouched();
                }
                let intent = straight_if_clear(cx, intent, to, crossing.is_some());
                return Step::Continue(intent.looking(Look::Use(aim)));
            }
            _ => {
                // Steep, blocked, too far still, or given up before.
                tried.push(here);
            }
        }
    }
    if let Some(s) = *stand {
        let off = me.distance_xz(s.feet);
        let level = (me.y - s.feet.y).abs() <= AT_STAND_UP_M;
        if level && (off <= AT_STAND_M || (arrived.is_some() && off <= AT_STAND_M * 2.0)) {
            let arrived_at = *arrived.get_or_insert(cx.now);
            let lift = AIM_LIFTS_M[usize::from(*tries) % AIM_LIFTS_M.len()];
            let aim_now = at + Vec3::new(0.0, lift, 0.0);
            let mut intent = Intent::stay().looking(Look::Use(aim_now));
            if s.crouch {
                intent = intent.crouched();
            }
            // The last use not seen yet: it may have taken it.
            if cx.now < *last_use + every || !use_seen(cx, *last_use, 0) {
                return Step::Continue(intent);
            }
            if *tries < per_stand {
                // Off the stand where no use reaches (it stopped past it,
                // or was pushed): back onto it first, a while.
                let here = TakeStand {
                    feet: me,
                    crouch: s.crouch,
                };
                if off > 0.05
                    && cx.now < arrived_at + SETTLE_MS
                    && !matches!(
                        use_from(cx, here, aim_now, RING_USE_M, FIRST_PITCH_MAX_RAD),
                        UseFrom::Clear(_)
                    )
                {
                    return Step::Continue(to_stand(cx, s, aim, crossing.is_some()).within(0.05));
                }
                if settled(cx, s, aim_now) || cx.now >= arrived_at + SETTLE_MS {
                    *tries += 1;
                    *last_use = cx.now;
                    intent = intent.pressing(key::USE);
                }
                return Step::Continue(intent);
            }
            // Its uses spent there: kept, and another stand.
            let eye = cx.me.eye();
            let to = aim_now - eye;
            let [fx, fz] = crate::v026g::bots::kin::forward(cx.me.yaw);
            let (sin, cos) = cx.me.pitch.sin_cos();
            let looking = Vec3::new(fx * cos, sin, fz * cos);
            failed = Some(FailedStand {
                battery,
                at,
                feet: s.feet,
                crouch: s.crouch,
                until: cx.now + FAILED_KEEP_MS,
                ray_m: to.length(),
                down: (-to.y).atan2(to.x.hypot(to.z)),
                off: (looking.dot(to) / to.length().max(1e-3))
                    .clamp(-1.0, 1.0)
                    .acos(),
            });
            tried.push(s);
            *stand = None;
        } else if cx.now < *since + walk_ms {
            return Step::Continue(to_stand(cx, s, aim, crossing.is_some()));
        } else {
            // Not reached in time: another.
            tried.push(s);
            *stand = None;
        }
    }
    // Another stand round it.
    if tried.len() >= stands {
        return match failed {
            // Kept first; the play ends at the next step.
            Some(_) => Step::Continue(Intent {
                failed_stand: failed,
                ..Intent::stay()
            }),
            None => Step::Failed("could not take it"),
        };
    }
    let steep = matches!(
        use_from(
            cx,
            TakeStand {
                feet: me,
                crouch: true
            },
            aim,
            f32::INFINITY,
            TAKE_PITCH_MAX_RAD
        ),
        UseFrom::Steep
    );
    let through = crossing.is_some();
    let intent = match next_stand(cx, at, aim, tried, steep, &gave_up, through) {
        Some(s) => {
            *stand = Some(s);
            *since = cx.now;
            *arrived = None;
            *tries = 0;
            *last_use = 0;
            to_stand(cx, s, aim, through)
        }
        None if steep && backing.is_none() => {
            // Wedged right over it: a step back the way it came, with no
            // line looked at (none is found from where it stands).
            *backing = Some(cx.now);
            back_off(cx, at, aim, *from)
        }
        None if failed.is_some() => Intent::stay(),
        None => return Step::Failed("could not take it"),
    };
    Step::Continue(Intent {
        failed_stand: failed,
        ..intent
    })
}

/// The next stand round a battery lying at `at`: of the ring's
/// ([`ring_stands`]) not tried this play nor given up lately, the nearest
/// in the stance not tried last, with an easy look down, on a side not
/// tried; right over the battery (`steep`), only one it walks off to in a
/// line.
fn next_stand(
    cx: &Ctx,
    at: Vec3,
    aim: Vec3,
    tried: &[TakeStand],
    steep: bool,
    gave_up: &dyn Fn(TakeStand) -> bool,
    crossing: bool,
) -> Option<TakeStand> {
    let me = cx.me.feet;
    let prefer_crouch = tried.last().is_none_or(|t| !t.crouch);
    let away = |feet: Vec3| (feet.z - at.z).atan2(feet.x - at.x);
    ring_stands(cx, at, aim, crossing)
        .into_iter()
        .filter(|(s, _)| !gave_up(*s) && !tried.iter().any(|t| t.same(s.feet, s.crouch)))
        .filter(|(s, _)| {
            !steep || (me.distance_xz(s.feet) <= STRAIGHT_M && clear_line(cx, me, s.feet, crossing))
        })
        .map(|(s, pitch)| {
            let side_tried = tried.iter().any(|t| {
                t.feet.distance_xz(at) > 0.3
                    && crate::v026g::bots::kin::wrap(away(t.feet) - away(s.feet)).abs() < 0.9
            });
            let cost = me.distance_xz(s.feet)
                + if s.crouch == prefer_crouch { 0.0 } else { 0.6 }
                + (pitch - TAKE_PITCH_RAD).abs() * 0.5
                + if side_tried { 1.0 } else { 0.0 };
            (s, cost)
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(s, _)| s)
}

/// To a stand, looking at the battery: straight if near in a clear line.
fn to_stand(cx: &Ctx, s: TakeStand, aim: Vec3, crossing: bool) -> Intent {
    let mut intent = Intent::to(s.feet, Pace::Walk).within(AT_STAND_M * 0.5);
    if s.crouch {
        intent = intent.crouched();
    }
    straight_if_clear(cx, intent, s.feet, crossing).looking(Look::Use(aim))
}

/// The intent walked straight ([`Intent::straight`]) if `to` is within
/// [`STRAIGHT_M`] in a line the walk grid clears; on a crossing, however
/// far, in a line over the zones' floors too.
fn straight_if_clear(cx: &Ctx, intent: Intent, to: Vec3, crossing: bool) -> Intent {
    let me = cx.me.feet;
    if (crossing || me.distance_xz(to) <= STRAIGHT_M) && clear_line(cx, me, to, crossing) {
        intent.straight()
    } else {
        intent
    }
}

/// Whether a body walks from `a` to `b` in a line on allowed floors; on a
/// crossing, over the zones' floors too.
fn clear_line(cx: &Ctx, a: Vec3, b: Vec3, crossing: bool) -> bool {
    if crossing {
        cx.tactics
            .clear_line_through_zones(a, b, cx.bot.side, false)
    } else {
        cx.tactics.clear_line_for(a, b, cx.now, cx.bot.side, false)
    }
}

/// A step back off a battery the way it came ([`back_spot`]), straight
/// with no line looked at, facing the battery from that side.
fn back_off(cx: &Ctx, at: Vec3, aim: Vec3, from: Option<Vec3>) -> Intent {
    let back = back_spot(cx, at, from);
    let (dx, dz) = (at.x - back.x, at.z - back.z);
    let n = dx.hypot(dz).max(1e-3);
    let ahead = Vec3::new(at.x + dx / n, aim.y, at.z + dz / n);
    Intent::to(back, Pace::Walk)
        .within(0.1)
        .crouched()
        .straight()
        .looking(Look::Use(ahead))
}

/// Whether the view looks at `aim` from the stand's eye (its own estimate
/// of its view, `PlaceView`), within [`SETTLED_RAD`].
fn settled(cx: &Ctx, stand: TakeStand, aim: Vec3) -> bool {
    cx.me.crouched() == stand.crouch && aimed_at(cx, aim)
}

/// Whether the bot's view is on `aim` within [`SETTLED_RAD`].
fn aimed_at(cx: &Ctx, aim: Vec3) -> bool {
    use crate::v026g::bots::kin::{wrap, yaw_of};
    let eye = cx.me.eye();
    let (dx, dz) = (aim.x - eye.x, aim.z - eye.z);
    let yaw = yaw_of([dx, dz]);
    let pitch = (aim.y - eye.y).atan2(dx.hypot(dz)).clamp(-1.0, 1.0);
    wrap(cx.me.yaw - yaw).abs() < SETTLED_RAD && (cx.me.pitch - pitch).abs() < SETTLED_RAD
}

#[cfg(test)]
mod tests {
    use mapdata::{Rotation, Shape, ShapeKind, Vec3, Zone, ZoneKind};

    use super::*;
    use crate::v026g::bots::plays::tests::Scenario;
    use crate::v026g::bots::tactics::Tactics;
    use crate::v026g::bots::view::{BatteryState, Mode};

    /// The field with a toxic zone (always on) of radius `r` round
    /// (25, 0, 32), of no known damage (lethal to the bots' reckoning).
    fn with_toxic(r: f32) -> Scenario {
        with_zone(r, None)
    }

    /// [`with_toxic`] hitting each of 2 parts `max_hit` a second.
    fn with_hurting(r: f32, max_hit: f32) -> Scenario {
        with_zone(
            r,
            Some(mapdata::Damage {
                max_hit,
                interval_ms: 1000,
                parts: 2,
                ..mapdata::Damage::default()
            }),
        )
    }

    fn with_zone(r: f32, damage: Option<mapdata::Damage>) -> Scenario {
        let mut s = Scenario::field(Mode::Batteries);
        let world = crate::v026g::bots::tactics::tests::field_with(|map| {
            map.zones.push(Zone {
                kind: ZoneKind::Damage,
                name: "toxic".into(),
                team: -1,
                position: Vec3::new(25.0, 0.0, 32.0),
                damage_type: "toxic".into(),
                damage,
                shapes: vec![Shape {
                    kind: ShapeKind::Sphere,
                    center: Vec3::new(25.0, 1.0, 32.0),
                    rotation: Rotation::IDENTITY,
                    half: Vec3::new(r, r, r),
                }],
            });
        });
        s.tactics = Tactics::new(world, 1.0);
        s.bot(0, 0, Vec3::new(10.0, 0.0, 10.0));
        s
    }

    /// A battery in a zone on all match that hurts 0.02 a second (level_01's
    /// swamp hurts 0.06): offered, fetched by walking to the crossing's dry
    /// edge, then in by a line, the zone's floor the play's choice; given
    /// up inside once the way back would leave too little health; not
    /// offered to a bot too hurt to bear it, nor where it hurts more.
    #[test]
    fn a_battery_in_a_zone_on_all_match_is_taken_by_a_crossing_health_bears() {
        let at = Vec3::new(25.0, 0.0, 32.0);
        let target = TargetRef::Battery(0);
        let mut s = with_hurting(4.0, 0.01);
        s.battery(BatteryState::Free(at));
        assert_eq!(collects(&s), vec![target]);
        let crossing = s.cx(0).crossing(at).expect("a crossing");
        let off = crossing.edge.distance_xz(at);
        // The zone's 4 m and the marks' margin of 1 m; the nearest such floor.
        assert!((5.0..7.0).contains(&off), "{crossing:?}");
        assert!(crossing.fits(1.0) && !crossing.fits(0.3), "{crossing:?}");
        // To the edge first.
        s.current = Some((PlayKind::Collect, target, s.view.clock));
        let mut rng = Rng::new(1);
        let mut state = start(target, &s.cx(0), &mut rng);
        let intent = intent_of(step(&mut state, target, &s.cx(0), &mut rng));
        assert!(
            matches!(intent.go, Go::To { at: to, .. } if to.distance_xz(crossing.edge) < 0.01),
            "{intent:?}"
        );
        assert!(!intent.zone_ok);
        // At it: a take by the crossing, walked in by a line.
        s.bot(0, 0, crossing.edge);
        step(&mut state, target, &s.cx(0), &mut rng);
        assert!(
            matches!(
                state,
                State::Take {
                    crossing: Some(_),
                    ..
                }
            ),
            "{state:?}"
        );
        let intent = intent_of(step(&mut state, target, &s.cx(0), &mut rng));
        assert!(intent.straight && intent.zone_ok, "{intent:?}");
        assert!(matches!(intent.go, Go::To { at: to, .. } if to.distance_xz(at) < 0.01));
        // Hurt inside: given up, the way back would leave under 0.25.
        s.bot(0, 0, Vec3::new(26.0, 0.0, 32.0)).health = 0.27;
        assert_eq!(
            step(&mut state, target, &s.cx(0), &mut rng),
            Step::Failed("its zone hurts too much")
        );
        // Too hurt to begin it.
        s.bot(0, 0, Vec3::new(10.0, 0.0, 10.0)).health = 0.3;
        assert_eq!(s.cx(0).take_block(at), Some("its zone hurts too much"));
        assert!(collects(&s).is_empty());
        // A zone ten times as harsh: not offered at full health.
        let mut s = with_hurting(4.0, 0.1);
        s.battery(BatteryState::Free(at));
        assert_eq!(s.cx(0).take_block(at), Some("its zone hurts too much"));
        // A dry battery as near scores above it.
        let mut s = with_hurting(4.0, 0.01);
        s.battery(BatteryState::Free(at));
        let dry = Vec3::new(36.6, 0.0, 10.0);
        s.battery(BatteryState::Free(dry));
        let mut out = Vec::new();
        candidates(&s.cx(0), &mut out);
        let score = |t| {
            out.iter()
                .find(|c| c.target == t)
                .map(|c| c.score.get())
                .unwrap()
        };
        assert!(
            score(TargetRef::Battery(1)) > score(TargetRef::Battery(0)),
            "{out:?}"
        );
    }

    /// A carrier in a zone on all match with no way out walks out by its
    /// cheapest crossing, in a line.
    #[test]
    fn a_carrier_in_a_zone_on_all_match_walks_out_by_a_crossing() {
        let mut s = with_hurting(4.0, 0.01);
        s.battery(BatteryState::Carried(Some(0)));
        s.bot(0, 0, Vec3::new(25.0, 0.0, 32.0)).carrying = Some(0);
        let target = TargetRef::Battery(0);
        s.current = Some((PlayKind::Collect, target, s.view.clock));
        let mut rng = Rng::new(1);
        let mut state = start(target, &s.cx(0), &mut rng);
        assert!(matches!(state, State::Bring { .. }));
        let intent = intent_of(step(&mut state, target, &s.cx(0), &mut rng));
        let edge = s.cx(0).way_out().map(|c| c.edge).expect("a crossing out");
        // Walked straight: no way out of the zone is asked for.
        assert!(intent.straight && !intent.zone_ok, "{intent:?}");
        assert!(matches!(intent.go, Go::To { at, .. } if at.distance_xz(edge) < 0.01));
        // Its motor walking a crossing home already: that way, not the line.
        s.crossing = Some(Vec3::new(10.0, 0.0, 10.0));
        let intent = intent_of(step(&mut state, target, &s.cx(0), &mut rng));
        assert!(!intent.straight, "{intent:?}");
        assert!(matches!(intent.go, Go::To { at, .. } if at.distance_xz(edge) > 1.0), "{intent:?}");
        s.crossing = None;
        // Out: home by the route as ever.
        s.bot(0, 0, edge).carrying = Some(0);
        let intent = intent_of(step(&mut state, target, &s.cx(0), &mut rng));
        assert!(!intent.straight, "{intent:?}");
    }

    fn collects(s: &Scenario) -> Vec<TargetRef> {
        let mut out = Vec::new();
        candidates(&s.cx(0), &mut out);
        out.iter()
            .filter(|c| c.kind == PlayKind::Collect)
            .map(|c| c.target)
            .collect()
    }

    /// A battery inside a zone the bots keep out of is no candidate (it
    /// stood still on it before); one beside it is.
    #[test]
    fn a_battery_inside_a_hazard_is_left_and_one_outside_is_taken() {
        let mut s = with_toxic(4.0);
        s.battery(BatteryState::Free(Vec3::new(25.0, 0.0, 32.0)));
        s.battery(BatteryState::Free(Vec3::new(25.0, 0.0, 15.0)));
        assert_eq!(collects(&s), vec![TargetRef::Battery(1)]);
        assert!(
            s.tactics
                .route(
                    Vec3::new(10.0, 0.0, 10.0),
                    Vec3::new(25.0, 0.0, 32.0),
                    s.view.clock,
                    Some(0)
                )
                .is_none(),
            "no route to the one left"
        );
        // A battery on the marked edge of a zone (1 m, the margin 1 m): a
        // floor let through leads to it, so it is taken.
        let mut s = with_toxic(1.0);
        s.battery(BatteryState::Free(Vec3::new(25.0, 0.0, 30.1)));
        let edge = s
            .tactics
            .world
            .grid
            .locate(Vec3::new(25.0, 0.0, 30.1), 3.0)
            .unwrap();
        assert!(!s.tactics.allowed(edge, s.view.clock, Some(0), false));
        assert_eq!(collects(&s), vec![TargetRef::Battery(0)]);
    }

    /// A battery on a shelf 2 m up (past the jump): its own floor is no
    /// walk's, but from the ground beside it a crouched eye is within a
    /// use's reach in a clear line, so it is taken from there; one in the
    /// shelf's middle, out of reach from every side, is left.
    #[test]
    fn a_battery_on_a_shelf_is_taken_from_the_floor_beside_it() {
        use crate::v026g::bots::body::tests::floor;
        use mapdata::{RayTree, WalkGrid, World};

        let mut s = Scenario::field(Mode::Batteries);
        let mut triangles = floor(0.0, 0.0, 80.0, 40.0, 0.0).to_vec();
        triangles.extend(floor(47.0, 13.0, 51.0, 17.0, 2.0));
        let grid = WalkGrid::build(
            &triangles,
            [0.0, 0.0, 80.0, 40.0],
            0.25,
            mapdata::Body::default(),
        );
        let map = s.tactics.world.map.clone();
        let world = std::sync::Arc::new(World::assemble(
            map,
            grid.into(),
            RayTree::build(&triangles).into(),
        ));
        s.tactics = Tactics::new(world, 1.0);
        s.bot(0, 0, Vec3::new(10.0, 0.0, 10.0));
        let edge = Vec3::new(51.0, 2.2, 15.0);
        let middle = Vec3::new(49.0, 2.2, 15.0);
        s.battery(BatteryState::Free(edge));
        s.battery(BatteryState::Free(middle));
        let stand = s.tactics.stand_to_take(edge).unwrap();
        assert!(stand.y.abs() < 0.01 && stand.x > 51.0, "{stand:?}");
        assert_eq!(s.tactics.stand_to_take(middle), None);
        assert_eq!(collects(&s), vec![TargetRef::Battery(0)]);
        // The fetch walks to the floor beside it, then takes it there.
        let target = TargetRef::Battery(0);
        s.current = Some((PlayKind::Collect, target, s.view.clock));
        let mut rng = Rng::new(1);
        let mut state = start(target, &s.cx(0), &mut rng);
        let Step::Continue(intent) = step(&mut state, target, &s.cx(0), &mut rng) else {
            panic!("goes for it");
        };
        assert!(matches!(intent.go, Go::To { at, .. } if at == stand));
        s.bot(0, 0, stand);
        step(&mut state, target, &s.cx(0), &mut rng);
        assert!(matches!(state, State::Take { .. }));
    }

    /// A bot under the floor the battery is taken from (fallen off the
    /// ledge, or under an overhang: 0x1b8ee975's battery 4, 236 s) whose
    /// stand there failed walks up to the stand by the route, however near
    /// across; before a failure, and once up, the take goes on as ever.
    #[test]
    fn a_take_failed_under_its_floor_walks_up_to_it() {
        let mut s = take_field(&[], Vec3::new(25.6, -2.0, 12.0));
        let target = TargetRef::Battery(0);
        let mut rng = Rng::new(1);
        let mut state = fresh_take(&s, None);
        let first = intent_of(step(&mut state, target, &s.cx(0), &mut rng));
        assert!(
            !matches!(first.go, Go::To { within_m, .. } if within_m == 0.0),
            "{first:?}"
        );
        let State::Take { tried, .. } = &mut state else {
            panic!("{state:?}");
        };
        tried.push(TakeStand {
            feet: Vec3::new(25.6, -2.0, 12.0),
            crouch: false,
        });
        let up = intent_of(step(&mut state, target, &s.cx(0), &mut rng));
        assert!(
            matches!(up.go, Go::To { at, within_m } if at == TAKE_AT && within_m == 0.0),
            "{up:?}"
        );
        assert!(matches!(state, State::Take { .. }), "{state:?}");
        s.bot(0, 0, Vec3::new(25.6, 0.0, 12.0));
        let on = intent_of(step(&mut state, target, &s.cx(0), &mut rng));
        assert!(
            !matches!(on.go, Go::To { within_m, .. } if within_m == 0.0),
            "{on:?}"
        );
    }

    /// The enemies known by a battery hovering at the cut-off (4) do not
    /// flip a bot's Collect of it: on it, it stays a candidate at 4 and
    /// leaves at 5; out, it comes back at 3, not at 4. With an enemy in
    /// sight there is no hold.
    #[test]
    fn enemies_hovering_at_the_cut_off_do_not_flip_a_collect() {
        use crate::v026g::bots::know::Source;
        let at = Vec3::new(25.0, 0.0, 15.0);
        let target = TargetRef::Battery(0);
        let scene = |enemies: usize, on_it: bool| {
            let mut s = with_toxic(0.3);
            s.battery(BatteryState::Free(at));
            for e in 0..enemies {
                let off = Vec3::new(4.0 + e as f32, 0.0, 6.0);
                s.enemy(1 + e as Place, 1, at + off, Some(Source::Heard));
            }
            if on_it {
                s.current = Some((PlayKind::Collect, target, s.view.clock));
            }
            s
        };
        let offered = |enemies, on_it| collects(&scene(enemies, on_it)) == vec![target];
        // Not on it: out at the cut-off.
        assert!(offered(3, false));
        assert!(!offered(4, false));
        // On it: 3, 4, 3 keep it; 5 is out.
        assert!(offered(3, true));
        assert!(offered(4, true));
        assert!(!offered(5, true));
        // The score on it at 4 is the score at 3: one enemy fewer read.
        let score = |enemies, on_it| {
            let mut out = Vec::new();
            candidates(&scene(enemies, on_it).cx(0), &mut out);
            out.iter().find(|c| c.target == target).map(|c| c.score)
        };
        assert_eq!(score(4, true), score(3, false));
        // An enemy in sight (30 m off the battery, not one of the 4): no
        // hold, out at 4 on it too.
        let mut fight = scene(4, true);
        fight.enemy(9, 1, at + Vec3::new(30.0, 0.0, 0.0), Some(Source::Seen));
        assert!(!fight.cx(0).memory.all_seen(fight.view.clock).is_empty());
        assert!(collects(&fight).is_empty());
        // The note agrees: out for the enemies only when not on it.
        let whys = |s: &Scenario| {
            take_blocks(&s.cx(0), &[])
                .list
                .iter()
                .map(|&(_, why, _)| why)
                .collect::<Vec<_>>()
        };
        assert_eq!(whys(&scene(4, true)), [None]);
        assert_eq!(whys(&scene(4, false)), [Some("scored out: enemies by it")]);
    }

    #[test]
    fn a_battery_no_way_was_found_to_lately_is_left_for_a_while() {
        let mut s = with_toxic(0.3);
        let at = Vec3::new(45.0, 0.0, 32.0);
        s.battery(BatteryState::Free(at));
        assert_eq!(collects(&s), vec![TargetRef::Battery(0)]);
        let target = TargetRef::Battery(0);
        s.current = Some((PlayKind::Collect, target, s.view.clock));
        let mut rng = Rng::new(1);
        let mut state = start(target, &s.cx(0), &mut rng);
        let noted = |s: &mut Scenario| {
            let blocks = take_blocks(&s.cx(0), &[]);
            note_take_blocks(&mut s.team, 0, &blocks)
        };
        assert!(
            noted(&mut s).is_empty(),
            "a candidate from the start: no note"
        );
        s.no_way_to = vec![(at + Vec3::new(1.0, 0.0, 0.0), s.view.clock + 5_000)];
        assert!(collects(&s).is_empty());
        assert_eq!(
            step(&mut state, target, &s.cx(0), &mut rng),
            Step::Failed("no way to it")
        );
        // Why it left the candidates is noted once.
        assert_eq!(
            noted(&mut s),
            [concat!(
                "battery 0 is no Collect candidate: no way to its stand lately ",
                "(41 m off, a candidate for 0.0 s)"
            )]
        );
        assert!(noted(&mut s).is_empty());
        // Forgotten: fetched again.
        s.view.clock += 5_000;
        assert_eq!(
            noted(&mut s),
            ["battery 0 is a Collect candidate again (41 m off, out for 5.0 s)"]
        );
        assert_eq!(collects(&s), vec![TargetRef::Battery(0)]);
    }

    /// The field with three battery spawners (as the soaks' map), the
    /// battery cheat off, and a bot of team 0 at (10, 0, 10).
    fn fair() -> (Scenario, [Vec3; 3]) {
        use mapdata::BatterySpawner;
        let mut s = Scenario::field(Mode::Batteries);
        s.tuning.cheats.batteries = false;
        let items = [(40.0, 33.0), (25.0, 12.0), (55.0, 12.0)].map(|(x, z)| Vec3::new(x, 0.3, z));
        let world = crate::v026g::bots::tactics::tests::field_with(|map| {
            map.battery_spawners = items
                .iter()
                .map(|&item| BatterySpawner {
                    position: Vec3::new(item.x, 0.0, item.z),
                    item,
                    layer: "default".into(),
                })
                .collect();
        });
        s.tactics = Tactics::new(world, 1.0);
        s.bot(0, 0, Vec3::new(10.0, 0.0, 10.0));
        (s, items)
    }

    /// With the battery cheat off a bot goes only for the batteries its
    /// team knows of; while some are missing it looks at the spawners'
    /// spots (no more bots than batteries missing), a known battery first;
    /// a look ends once a bot of the team has seen the spot, and a fetch
    /// gives up when the battery is seen gone.
    #[test]
    fn without_the_cheat_a_bot_takes_known_batteries_and_looks_for_the_rest() {
        use crate::v026g::bots::know::KnownBattery;
        let (mut s, items) = fair();
        // Two batteries, at spawners 1 and 2; the team knows neither.
        s.battery(BatteryState::Free(items[1]));
        s.battery(BatteryState::Free(items[2]));
        let spawners = (0..3).map(TargetRef::Spawner).collect::<Vec<_>>();
        assert_eq!(collects(&s), spawners);
        // Two bots looking already: no more.
        s.team.searching = vec![(1, 0), (2, 1)];
        assert!(collects(&s).is_empty());
        // One looking at spawner 2: the others are left to this bot.
        s.team.searching = vec![(1, 2)];
        assert_eq!(collects(&s), spawners[..2].to_vec());
        // A bot saw battery 0 at spawner 1: it comes first, spawner 1
        // needs no look, and one battery is missing.
        s.team.set_battery(
            0,
            Some(KnownBattery {
                at: items[1],
                time: s.view.clock,
                hud: false,
            }),
        );
        s.team.searching.clear();
        let mut out = Vec::new();
        candidates(&s.cx(0), &mut out);
        out.sort_by(|a, b| b.score.get().total_cmp(&a.score.get()));
        let targets: Vec<TargetRef> = out.iter().map(|c| c.target).collect();
        assert_eq!(targets[0], TargetRef::Battery(0), "{out:?}");
        assert!(!targets.contains(&TargetRef::Spawner(1)));
        assert_eq!(s.cx(0).batteries_unaccounted(), 1);
        s.team.searching = vec![(1, 2)];
        assert_eq!(collects(&s), vec![TargetRef::Battery(0)]);
        s.team.searching.clear();
        // Looking at spawner 0: it walks there until the spot is seen.
        let target = TargetRef::Spawner(0);
        s.current = Some((PlayKind::Collect, target, s.view.clock));
        let mut rng = Rng::new(1);
        let mut state = start(target, &s.cx(0), &mut rng);
        assert_eq!(state, State::Search);
        let Step::Continue(intent) = step(&mut state, target, &s.cx(0), &mut rng) else {
            panic!("goes to look");
        };
        assert!(matches!(intent.go, Go::To { at, .. } if at.distance_xz(items[0]) < 2.0));
        s.view.clock += 1_000;
        s.team.saw_spawner(0, s.view.clock);
        assert_eq!(
            step(&mut state, target, &s.cx(0), &mut rng),
            Step::Done("looked at the spot")
        );
        // Fetching battery 0: once its spot is seen empty (an enemy took
        // it), the fetch gives up.
        let target = TargetRef::Battery(0);
        s.current = Some((PlayKind::Collect, target, s.view.clock));
        let mut state = start(target, &s.cx(0), &mut rng);
        assert!(matches!(
            step(&mut state, target, &s.cx(0), &mut rng),
            Step::Continue(_)
        ));
        s.view.batteries[0] = BatteryState::Carried(Some(5));
        assert!(
            matches!(
                step(&mut state, target, &s.cx(0), &mut rng),
                Step::Continue(_)
            ),
            "not seen gone yet: still believed there"
        );
        s.team.set_battery(0, None);
        assert_eq!(
            step(&mut state, target, &s.cx(0), &mut rng),
            Step::Failed("the battery is gone")
        );
    }

    /// The field with toxic ground (on all match) ringing a pocket round
    /// (20, 0, 30) with four battery spawners in it (with `pocket`), and
    /// one spawner at (55, 0, 12) on the enemy's side; the battery cheat
    /// off, three batteries lying unseen in the pocket (level_01's swamp
    /// in small).
    fn swamp(pocket_spawners: bool) -> Scenario {
        use mapdata::BatterySpawner;
        let pocket = [(18.0, 28.0), (22.0, 28.0), (18.0, 32.0), (22.0, 32.0)];
        let world = crate::v026g::bots::tactics::tests::field_with(|map| {
            let wall = |x: f32, z: f32, hx: f32, hz: f32| Shape {
                kind: ShapeKind::Box,
                center: Vec3::new(x, 1.0, z),
                rotation: Rotation::IDENTITY,
                half: Vec3::new(hx, 1.0, hz),
            };
            {
                map.zones.push(Zone {
                    kind: ZoneKind::Damage,
                    name: "toxic".into(),
                    team: -1,
                    position: Vec3::new(20.0, 0.0, 30.0),
                    damage_type: "toxic".into(),
                    damage: None,
                    shapes: vec![
                        wall(20.0, 25.0, 5.5, 0.5),
                        wall(20.0, 35.0, 5.5, 0.5),
                        wall(15.0, 30.0, 0.5, 5.5),
                        wall(25.0, 30.0, 0.5, 5.5),
                    ],
                });
            }
            let inside = if pocket_spawners { &pocket[..] } else { &[] };
            map.battery_spawners = inside
                .iter()
                .chain(&[(55.0, 12.0)])
                .map(|&(x, z)| BatterySpawner {
                    position: Vec3::new(x, 0.0, z),
                    item: Vec3::new(x, 0.3, z),
                    layer: "default".into(),
                })
                .collect();
        });
        let mut s = Scenario::field(Mode::Batteries);
        s.tuning.cheats.batteries = false;
        s.tactics = Tactics::new(world, 1.0);
        s.bot(0, 0, Vec3::new(10.0, 0.0, 10.0));
        for &(x, z) in &pocket[..3] {
            s.battery(BatteryState::Free(Vec3::new(x, 0.3, z)));
        }
        s
    }

    fn score_of(s: &Scenario, target: TargetRef) -> Option<f32> {
        let mut out = Vec::new();
        candidates(&s.cx(0), &mut out);
        out.iter()
            .find(|c| c.target == target)
            .map(|c| c.score.get())
    }

    /// Match 0x7db2c16a: three batteries lay in level_01's swamp (no way
    /// in all match) and team 0 searched spawners' spots 12.4 bot-minutes
    /// for them, 30 deaths. The shut spots count among the spots the
    /// missing batteries may lie at (and surely keep them), so a far spot
    /// on the enemy's side, likely cleared by him, weighs little; the
    /// team's deaths there lately take it off the list.
    #[test]
    fn a_search_counts_the_spots_no_way_leads_to_and_the_teams_losses() {
        use crate::v026g::bots::know::Loss;
        // Without the pocket's spots the missing three must be at the far
        // one: offered at full chance.
        let open_far = score_of(&swamp(false), TargetRef::Spawner(0)).expect("offered");
        // The pocket's spots, shut all match: never offered themselves,
        // and the far spot weighs less (the three may well lie there).
        let far = TargetRef::Spawner(4);
        let mut s = swamp(true);
        assert!(s.cx(0).shut_all_match(Vec3::new(18.0, 0.3, 28.0)));
        assert!(!s.cx(0).shut_all_match(Vec3::new(55.0, 0.3, 12.0)));
        for i in 0..4 {
            assert_eq!(score_of(&s, TargetRef::Spawner(i)), None);
        }
        let shut_far = score_of(&s, far).expect("still a chance");
        assert!(shut_far < open_far * 0.5, "{shut_far} against {open_far}");
        // Early in the match the enemy has had no time to clear his side:
        // more likely there.
        s.view.clock = 30_000;
        assert!(score_of(&s, far).unwrap() > shut_far);
        s.view.clock = 300_000;
        // A teammate died on the way there a moment ago: less; a few
        // more by it, none.
        let now = s.view.clock;
        s.team.losses.push(Loss {
            at: Vec3::new(45.0, 0.0, 15.0),
            time: now - 1_000,
            spawner: Some(4),
        });
        let one = score_of(&s, far).expect("one loss");
        assert!(one < shut_far * 0.7, "{one} against {shut_far}");
        for _ in 0..2 {
            s.team.losses.push(Loss {
                at: Vec3::new(53.0, 0.0, 14.0),
                time: now - 2_000,
                spawner: None,
            });
        }
        assert_eq!(score_of(&s, far), None);
        // Forgotten after LOSS_KEEP_MS: as before.
        s.view.clock += crate::v026g::bots::know::LOSS_KEEP_MS + 1;
        s.team.forget_old(s.view.clock);
        assert!(s.team.losses.is_empty());
        assert!(score_of(&s, far).is_some());
    }

    /// An enemy carrier the team knew of died with a battery its sight
    /// cannot find: the look spot is offered above the spawners' spots,
    /// one bot walks there and ends when the spot is looked at.
    #[test]
    fn a_look_where_a_carrier_died_comes_first_and_ends_once_looked_at() {
        use crate::v026g::bots::know::LookSpot;
        let (mut s, items) = fair();
        // Battery 0 dropped at (14, 0, 33), unseen; two more at spawners.
        let dropped = Vec3::new(14.0, 0.0, 33.0);
        s.battery(BatteryState::Free(dropped));
        s.battery(BatteryState::Free(items[1]));
        s.battery(BatteryState::Free(items[2]));
        let look = Vec3::new(13.5, 0.0, 33.5);
        s.team.add_look(LookSpot {
            battery: 0,
            at: look,
            time: s.view.clock,
            until: s.view.clock + crate::v026g::bots::know::LOOK_KEEP_MS,
            theft: false,
        });
        let mut out = Vec::new();
        candidates(&s.cx(0), &mut out);
        out.sort_by(|a, b| b.score.get().total_cmp(&a.score.get()));
        assert_eq!(out[0].target, TargetRef::Point(look), "{out:?}");
        assert!(
            out.iter()
                .any(|c| matches!(c.target, TargetRef::Spawner(_)))
        );
        // The play walks there.
        let target = TargetRef::Point(look);
        s.current = Some((PlayKind::Collect, target, s.view.clock));
        let mut rng = Rng::new(1);
        let mut state = start(target, &s.cx(0), &mut rng);
        assert_eq!(state, State::Search);
        let Step::Continue(intent) = step(&mut state, target, &s.cx(0), &mut rng) else {
            panic!("goes to look");
        };
        assert!(matches!(intent.go, Go::To { at, .. } if at.distance_xz(look) < 1.0));
        // Seen there (a bot stood by it): no more look; the battery is
        // known and fetched instead.
        s.team.set_battery(
            0,
            Some(crate::v026g::bots::know::KnownBattery {
                at: dropped,
                time: s.view.clock,
                hud: false,
            }),
        );
        s.team.settle_looks(0, &s.view);
        assert!(s.team.looks.is_empty());
        assert_eq!(
            step(&mut state, target, &s.cx(0), &mut rng),
            Step::Done("looked where the carrier died")
        );
        let mut out = Vec::new();
        candidates(&s.cx(0), &mut out);
        out.sort_by(|a, b| b.score.get().total_cmp(&a.score.get()));
        assert_eq!(out[0].target, TargetRef::Battery(0));
    }

    /// Two batteries of ours a thief dropped alive: the spots round our
    /// container are offered once each (both batteries' rings are the same
    /// spots), all under one slot (one bot looks), and go at their time.
    #[test]
    fn looks_after_a_dropped_theft_share_one_slot_and_end_in_time() {
        let (mut s, items) = fair();
        s.battery(BatteryState::Free(Vec3::new(14.0, 0.0, 33.0)));
        s.battery(BatteryState::Free(Vec3::new(10.0, 0.0, 33.0)));
        s.battery(BatteryState::Free(items[2]));
        let now = s.view.clock;
        let ring = [
            Vec3::new(16.0, 0.0, 30.0),
            Vec3::new(12.0, 0.0, 34.0),
            Vec3::new(8.0, 0.0, 30.0),
        ];
        s.team.add_theft_looks(0, &ring, now, now + 30_000);
        s.team.add_theft_looks(1, &ring, now, now + 30_000);
        let mut out = Vec::new();
        candidates(&s.cx(0), &mut out);
        let looks: Vec<&Candidate> = out
            .iter()
            .filter(|c| matches!(c.target, TargetRef::Point(_)))
            .collect();
        assert_eq!(looks.len(), ring.len(), "{looks:?}");
        assert!(looks.iter().all(|c| c.slot == looks[0].slot), "{looks:?}");
        for at in ring {
            assert!(looks.iter().any(|c| c.target == TargetRef::Point(at)));
        }
        // Their weight 0: none offered.
        s.tuning
            .plays
            .entry(PlayKind::Collect)
            .or_insert_with(|| crate::v026g::bots::plays::default_tuning(PlayKind::Collect))
            .numbers
            .insert("theft_look_weight".into(), 0.0);
        let mut out = Vec::new();
        candidates(&s.cx(0), &mut out);
        assert!(!out.iter().any(|c| matches!(c.target, TargetRef::Point(_))));
        // Gone after their time.
        s.team.forget_old(now + 30_000);
        assert_eq!(s.team.looks.len(), 2 * ring.len());
        s.team.forget_old(now + 30_001);
        assert!(s.team.looks.is_empty());
    }

    /// A battery in a pocket that toxic ground (on all match) rings: the
    /// floor it lies on is entered from free floors (the pocket's), so the
    /// look at its next floors said yes, while the search says no way; now
    /// the filter says no too, and a fetch of it gives up at once. One in
    /// a ring of a switching zone is still tried (the zone opens).
    #[test]
    fn a_battery_in_a_pocket_ringed_by_toxic_ground_is_left() {
        let ring = |kind_timer: bool| {
            let world = crate::v026g::bots::tactics::tests::field_with(|map| {
                let wall = |x: f32, z: f32, hx: f32, hz: f32| Shape {
                    kind: ShapeKind::Box,
                    center: Vec3::new(x, 1.0, z),
                    rotation: Rotation::IDENTITY,
                    half: Vec3::new(hx, 1.0, hz),
                };
                map.zones.push(Zone {
                    kind: ZoneKind::Damage,
                    name: "toxic".into(),
                    team: -1,
                    position: Vec3::new(20.0, 0.0, 30.0),
                    damage_type: "toxic".into(),
                    damage: None,
                    shapes: vec![
                        wall(20.0, 25.0, 5.5, 0.5),
                        wall(20.0, 35.0, 5.5, 0.5),
                        wall(15.0, 30.0, 0.5, 5.5),
                        wall(25.0, 30.0, 0.5, 5.5),
                    ],
                });
            });
            let mut s = Scenario::field(Mode::Batteries);
            s.tactics = Tactics::new(world, 1.0);
            if kind_timer {
                // As `Hazards::attach` makes a timer group's zone.
                s.tactics.hazards.zones[0].control = crate::v026g::bots::anomalies::Control::Timer;
            }
            s.bot(0, 0, Vec3::new(10.0, 0.0, 10.0));
            s.battery(BatteryState::Free(Vec3::new(20.0, 0.0, 30.0)));
            s
        };
        let s = ring(false);
        let at = Vec3::new(20.0, 0.0, 30.0);
        assert!(
            s.tactics
                .route(Vec3::new(10.0, 0.0, 10.0), at, s.view.clock, Some(0))
                .is_none(),
            "no way in"
        );
        let goal = s.tactics.world.grid.locate(at, 3.0).unwrap();
        assert!(s.tactics.allowed(goal, s.view.clock, Some(0), false));
        assert!(collects(&s).is_empty());
        let mut s = s;
        let target = TargetRef::Battery(0);
        s.current = Some((PlayKind::Collect, target, s.view.clock));
        let mut rng = Rng::new(1);
        let mut state = start(target, &s.cx(0), &mut rng);
        assert_eq!(
            step(&mut state, target, &s.cx(0), &mut rng),
            Step::Failed("no way to it")
        );
        // A switching ring: the filter leaves it to the search.
        assert_eq!(collects(&ring(true)), vec![TargetRef::Battery(0)]);
    }

    /// The field's floor with a walled pit at x 30-34, z 18-22, `depth` m
    /// deep. The grid links no jump over 0.6 m, so the pit is a pocket; a
    /// 0.9 m rim is a standing jump out (as pit A's), 3 m none.
    fn pit_world(depth: f32) -> std::sync::Arc<mapdata::World> {
        use mapdata::{MapData, RayTree, Spawn, Triangle, WalkGrid, WalkerBehaviour};

        use crate::v026g::bots::body::tests::floor;
        let y = -depth;
        let mut tris: Vec<Triangle> = Vec::new();
        tris.extend(floor(0.0, 0.0, 30.0, 40.0, 0.0));
        tris.extend(floor(34.0, 0.0, 80.0, 40.0, 0.0));
        tris.extend(floor(30.0, 0.0, 34.0, 18.0, 0.0));
        tris.extend(floor(30.0, 22.0, 34.0, 40.0, 0.0));
        tris.extend(floor(30.0, 18.0, 34.0, 22.0, y));
        let walker = WalkerBehaviour::BySlope;
        for (a, b) in [
            ((30.0, 18.0), (34.0, 18.0)),
            ((30.0, 22.0), (34.0, 22.0)),
            ((30.0, 18.0), (30.0, 22.0)),
            ((34.0, 18.0), (34.0, 22.0)),
        ] {
            let (p, q) = (Vec3::new(a.0, y, a.1), Vec3::new(b.0, y, b.1));
            let (r, s) = (Vec3::new(b.0, 0.0, b.1), Vec3::new(a.0, 0.0, a.1));
            tris.push(Triangle {
                v: [p, q, r],
                walker,
            });
            tris.push(Triangle {
                v: [p, r, s],
                walker,
            });
        }
        let spawns = [Vec3::new(5.0, 0.0, 10.0), Vec3::new(75.0, 0.0, 10.0)];
        let grid = WalkGrid::build_kept(
            &tris,
            [0.0, 0.0, 80.0, 40.0],
            0.25,
            mapdata::Body {
                max_jump: 0.6,
                ..mapdata::Body::default()
            },
            &spawns,
            &spawns,
        );
        let spawn = |team: i32, at: Vec3| Spawn {
            position: at,
            rotation: Vec3::ZERO,
            team,
            priority: 0,
            point_id: 0,
            layer: "default".into(),
        };
        let map = MapData {
            spawns: vec![spawn(0, spawns[0]), spawn(1, spawns[1])],
            ..MapData::default()
        };
        let sight = RayTree::build(&tris);
        std::sync::Arc::new(mapdata::World::assemble(map, grid.into(), sight.into()))
    }

    /// Cologne CG-11: a battery in a pit no carrier gets out of is no
    /// Collect candidate, and says why; one in a pit with a standing jump
    /// out (pit A's 0.9 m rim) still is.
    #[test]
    fn a_battery_in_a_pocket_no_carrier_leaves_is_not_taken() {
        let scene = |depth: f32| {
            let mut s = Scenario::field(Mode::Batteries);
            s.tactics = Tactics::new(pit_world(depth), 1.0);
            s.bot(0, 0, Vec3::new(10.0, 0.0, 10.0));
            s.battery(BatteryState::Free(Vec3::new(32.0, -depth, 20.0)));
            s
        };
        // The deep pit has no way out on the grid: no way is planned in, so
        // it holds no stand, and the block is still the pocket's.
        let deep = scene(3.0);
        let at = Vec3::new(32.0, -3.0, 20.0);
        assert_eq!(deep.tactics.stand_to_take(at), None);
        assert!(deep.cx(0).in_pocket(at));
        assert!(!deep.cx(0).escape_known(at, true));
        assert_eq!(deep.cx(0).take_block(at), Some(POCKET_BLOCK));
        assert!(collects(&deep).is_empty());
        let blocks = take_blocks(&deep.cx(0), &[]);
        let whys: Vec<_> = blocks.list.iter().map(|&(b, why, _)| (b, why)).collect();
        assert_eq!(whys, vec![(0, Some(POCKET_BLOCK))]);
        let rim = scene(0.9);
        let at = Vec3::new(32.0, -0.9, 20.0);
        let stand = rim.tactics.stand_to_take(at).expect("a stand");
        assert!(rim.cx(0).in_pocket(stand));
        assert!(rim.cx(0).escape_known(stand, true));
        assert_eq!(rim.cx(0).take_block(at), None);
        assert_eq!(collects(&rim), vec![TargetRef::Battery(0)]);
    }

    /// The real maps (private files; `SRP_MAPDATA_DIR`, in release): for
    /// every battery spawner of the battery modes and a bot at each side's
    /// first spawn, Collect's filter (`Ctx::reachable`) says what the
    /// route search says, every zone on (as before the anomalies' attach).
    /// At 8c1ce49 they disagreed on 11 spawners (22 of 982 pairs, on
    /// level_01, 02, 03 and 07): pockets ringed by toxic ground, where the
    /// filter's look at the goal's next floors said yes and the search no
    /// (battery 4 of the owner's match of 2026-10-16: 183 takes).
    #[test]
    #[ignore = "needs the private map files"]
    fn collects_filter_agrees_with_the_search_on_the_real_maps() {
        let dir = std::env::var_os("SRP_MAPDATA_DIR").expect("set SRP_MAPDATA_DIR");
        let mut wrong = Vec::new();
        let mut pairs = 0;
        for level in 1..=8u8 {
            for mode in [2u8, 3] {
                let path =
                    std::path::Path::new(&dir).join(format!("level_{level:02}_day_{mode}.mapdata"));
                let map = mapdata::MapData::read(&path).unwrap();
                let world = std::sync::Arc::new(mapdata::World::build(
                    map,
                    mapdata::CELL,
                    mapdata::Body::default(),
                ));
                let mut s = Scenario::field(Mode::Batteries);
                s.tactics = Tactics::new(world.clone(), 1.0);
                let started = std::time::Instant::now();
                let spawn = world
                    .grid
                    .locate(world.map.spawns[0].position, 1.5)
                    .unwrap();
                assert!(s.tactics.past_fixed_zones(spawn));
                eprintln!(
                    "level {level} mode {mode}: past the fixed zones in {:.2} ms",
                    started.elapsed().as_secs_f64() * 1e3
                );
                for (i, spawner) in world.map.battery_spawners.iter().enumerate() {
                    let Some(stand) = s.tactics.stand_to_take(spawner.item) else {
                        continue;
                    };
                    for side in [0, 1] {
                        let Some(from) = world
                            .map
                            .spawns
                            .iter()
                            .find(|spawn| spawn.team == side)
                            .map(|spawn| spawn.position)
                        else {
                            continue;
                        };
                        s.side = Some(side);
                        s.bot(0, side as u8, from);
                        let filter = s.cx(0).reachable(stand, false);
                        let search = s
                            .tactics
                            .route(from, stand, s.view.clock, Some(side))
                            .is_some();
                        pairs += 1;
                        if filter != search {
                            wrong.push((level, mode, i, side, filter, search));
                        }
                    }
                }
            }
        }
        eprintln!("{} of {pairs} disagree", wrong.len());
        assert!(wrong.is_empty(), "{wrong:?}");
    }

    /// The real maps (private files; `SRP_MAPDATA_DIR`, in release): each
    /// battery spawner's take stand, whether it is in a pocket and has a
    /// way out for a carrier or anyone, and the first reason a bot at side
    /// 0's spawn may not take it (CG-11). Prints only.
    #[test]
    #[ignore = "needs the private map files"]
    fn zone_crossings_on_the_real_maps() {
        let dir = std::env::var_os("SRP_MAPDATA_DIR").expect("set SRP_MAPDATA_DIR");
        let (mut zoned, mut found, mut fit) = (0, 0, 0);
        for level in 1..=8u8 {
            for mode in [2u8, 3] {
                let path =
                    std::path::Path::new(&dir).join(format!("level_{level:02}_day_{mode}.mapdata"));
                let Ok(map) = mapdata::MapData::read(&path) else {
                    continue;
                };
                let world = std::sync::Arc::new(mapdata::World::build(
                    map,
                    mapdata::CELL,
                    mapdata::Body::default(),
                ));
                let mut s = Scenario::field(Mode::Batteries);
                s.tactics = Tactics::new(world.clone(), 1.0);
                for team in [0u8, 1] {
                    let Some(spawn) = world.map.spawns.iter().find(|p| p.team == i32::from(team))
                    else {
                        continue;
                    };
                    s.bot(0, team, spawn.position);
                    for (i, spawner) in world.map.battery_spawners.iter().enumerate() {
                        let cx = s.cx(0);
                        let at = spawner.item;
                        let Some(stand) = s.tactics.stand_to_take(at) else {
                            continue;
                        };
                        if !cx.needs_crossing(stand) {
                            continue;
                        }
                        zoned += 1;
                        let crossing = cx.crossing(at);
                        found += usize::from(crossing.is_some());
                        fit += usize::from(crossing.is_some_and(|c| c.fits(1.0)));
                        eprintln!(
                            "level {level} mode {mode} team {team} spawner {i}: {:?} cost {:?} block {:?}",
                            crossing.map(|c| (c.edge.distance_xz(stand), c.leg, c.at_per_s)),
                            crossing.map(|c| c.cost()),
                            cx.take_block(at),
                        );
                    }
                }
            }
        }
        eprintln!("zoned (per team) {zoned}, a crossing {found}, within budget {fit}");
    }

    #[test]
    #[ignore = "needs the private map files"]
    fn pocket_stands_on_the_real_maps() {
        let dir = std::env::var_os("SRP_MAPDATA_DIR").expect("set SRP_MAPDATA_DIR");
        for level in 1..=8u8 {
            for mode in [2u8, 3] {
                let path =
                    std::path::Path::new(&dir).join(format!("level_{level:02}_day_{mode}.mapdata"));
                let Ok(map) = mapdata::MapData::read(&path) else {
                    continue;
                };
                let world = std::sync::Arc::new(mapdata::World::build(
                    map,
                    mapdata::CELL,
                    mapdata::Body::default(),
                ));
                let mut s = Scenario::field(Mode::Batteries);
                s.tactics = Tactics::new(world.clone(), 1.0);
                let from = world.map.spawns[0].position;
                s.bot(0, world.map.spawns[0].team as u8, from);
                let pockets = (0..world.grid.len())
                    .filter(|&n| world.grid.in_pocket(n as mapdata::NodeId))
                    .count();
                let mut reasons = std::collections::BTreeMap::<&str, usize>::new();
                for (i, spawner) in world.map.battery_spawners.iter().enumerate() {
                    let cx = s.cx(0);
                    let at = spawner.item;
                    let why = cx.take_block(at).unwrap_or("takeable");
                    *reasons.entry(why).or_default() += 1;
                    if s.tactics.stand_to_take(at).is_none() {
                        eprintln!(
                            "level {level} mode {mode} spawner {i} at {at:?}: no crouched stand"
                        );
                    }
                    let Some(stand) = s.tactics.stand_to_take(at) else {
                        continue;
                    };
                    if cx.in_pocket(stand) {
                        eprintln!(
                            "level {level} mode {mode} spawner {i} at {at:?}: stand in a pocket,                              carrier way out {}, anyone's {}, first block: {why}",
                            cx.escape_known(stand, true),
                            cx.escape_known(stand, false),
                        );
                    }
                }
                eprintln!(
                    "level {level} mode {mode}: {} spawners, {pockets} pocket floors; {reasons:?}",
                    world.map.battery_spawners.len()
                );
            }
        }
    }

    /// Fetching a battery a hazard closes round: the play gives up at once
    /// instead of standing till `give_up_s`.
    #[test]
    fn a_fetch_gives_up_when_no_way_leads_to_the_battery() {
        let mut s = with_toxic(4.0);
        s.battery(BatteryState::Free(Vec3::new(25.0, 0.0, 32.0)));
        let target = TargetRef::Battery(0);
        s.current = Some((PlayKind::Collect, target, s.view.clock));
        let mut rng = Rng::new(1);
        let mut state = start(target, &s.cx(0), &mut rng);
        assert_eq!(
            step(&mut state, target, &s.cx(0), &mut rng),
            Step::Failed("no way to it")
        );
    }

    /// The field with `solids` (walls 3 m high, `body::tests::wall_x`),
    /// a battery at (25, 0, 12) (the battery cheat on) and the bot of team
    /// 0 at `feet`.
    fn take_field(solids: &[mapdata::Triangle], feet: Vec3) -> Scenario {
        let mut s = Scenario::field(Mode::Batteries);
        let world = crate::v026g::bots::tactics::tests::field_with_solids(solids, |_| {});
        s.tactics = Tactics::new(world, 1.0);
        s.bot(0, 0, feet);
        s.battery(BatteryState::Free(TAKE_AT));
        s.current = Some((PlayKind::Collect, TargetRef::Battery(0), s.view.clock));
        s
    }

    const TAKE_AT: Vec3 = Vec3::new(25.0, 0.0, 12.0);

    /// A take's state as the fetch leaves it, the bot having come `from`.
    fn fresh_take(s: &Scenario, from: Option<Vec3>) -> State {
        State::Take {
            tries: 0,
            last_use: 0,
            stand: None,
            since: s.view.clock,
            arrived: None,
            tried: Vec::new(),
            from,
            backing: None,
            crossing: None,
        }
    }

    fn intent_of(step: Step) -> Intent {
        match step {
            Step::Continue(intent) => intent,
            other => panic!("{other:?}"),
        }
    }

    /// The bot stands at `feet`, crouched, looking at `aim` from there.
    fn stand_at(s: &mut Scenario, feet: Vec3, aim: Vec3) {
        use crate::v026g::bots::kin::yaw_of;
        let me = s.bot(0, 0, feet);
        me.keys = key::CROUCH;
        let eye = me.eye();
        me.yaw = yaw_of([aim.x - eye.x, aim.z - eye.z]);
        me.pitch = (aim.y - eye.y).atan2(eye.distance_xz(aim));
    }

    /// Whether the crouched eye at `feet` sees the battery's point to use.
    fn clear_from(s: &Scenario, feet: Vec3) -> bool {
        let aim = TAKE_AT + Vec3::new(0.0, BATTERY_AIM_M, 0.0);
        let stand = TakeStand { feet, crouch: true };
        matches!(
            use_from(&s.cx(0), stand, aim, RING_USE_M, TAKE_PITCH_MAX_RAD),
            UseFrom::Clear(_)
        )
    }

    /// A wall between where the bot stopped and the battery (as in the
    /// owner's match of 2026-10-16, b9 under a ledge: 9 uses aimed right
    /// at it from where the bullet mesh blocks the line): no use is
    /// pressed there; it goes round to a stand with a clear line, uses it
    /// twice, and when that fails too keeps the stand and goes to another.
    #[test]
    fn a_take_blocked_where_it_stopped_goes_round_the_battery() {
        use crate::v026g::bots::body::tests::wall_x;
        let east = Vec3::new(26.3, 0.0, 12.0);
        let mut s = take_field(&wall_x(25.6, 11.5, 12.5), east);
        assert!(!clear_from(&s, east), "the wall blocks the line");
        let target = TargetRef::Battery(0);
        let mut rng = Rng::new(1);
        let mut state = start(target, &s.cx(0), &mut rng);
        step(&mut state, target, &s.cx(0), &mut rng);
        assert!(matches!(state, State::Take { .. }), "{state:?}");
        let intent = intent_of(step(&mut state, target, &s.cx(0), &mut rng));
        assert_eq!(
            intent.press & key::USE,
            0,
            "no use where the line is blocked"
        );
        let State::Take {
            stand: Some(first),
            ref tried,
            ..
        } = state
        else {
            panic!("{state:?}");
        };
        assert_eq!(tried.len(), 1);
        let off = first.feet.distance_xz(TAKE_AT);
        assert!((0.5..1.3).contains(&off), "{first:?}");
        assert!(clear_from(&s, first.feet), "{first:?}");
        assert!(matches!(intent.go, Go::To { at, .. } if at == first.feet));
        assert!(matches!(intent.look, Look::Use(_)));
        // There, the view on the battery: two uses, 1.2 s apart.
        let aim = TAKE_AT + Vec3::new(0.0, BATTERY_AIM_M, 0.0);
        stand_at(&mut s, first.feet, aim);
        let intent = intent_of(step(&mut state, target, &s.cx(0), &mut rng));
        assert_ne!(intent.press & key::USE, 0, "a use at the stand");
        assert!(!intent.fire, "no fire as it uses: it may carry");
        s.view.clock += 1_200;
        stand_at(&mut s, first.feet, TAKE_AT);
        // No world since the use: it waits, the use may have taken it.
        let intent = intent_of(step(&mut state, target, &s.cx(0), &mut rng));
        assert_eq!(intent.press & key::USE, 0, "the first use not seen yet");
        assert!(!intent.fire, "no fire within 2 s of the use");
        s.view.world_time = s.view.clock - 300;
        let intent = intent_of(step(&mut state, target, &s.cx(0), &mut rng));
        assert_ne!(intent.press & key::USE, 0, "the second use, aimed lower");
        // Both failed: the stand is kept, and another one taken.
        s.view.clock += 1_200;
        s.view.world_time = s.view.clock - 300;
        let intent = intent_of(step(&mut state, target, &s.cx(0), &mut rng));
        let failed = intent.failed_stand.expect("the stand is kept");
        assert_eq!(
            (failed.battery, failed.feet, failed.crouch),
            (0, first.feet, true)
        );
        let State::Take {
            stand: Some(second),
            ..
        } = state
        else {
            panic!("{state:?}");
        };
        assert!(second.feet.distance_xz(first.feet) >= SAME_STAND_M);
        assert!(clear_from(&s, second.feet) || !second.crouch);
        // The next take (the commander gives the play again) does not use
        // the kept stand where it stands: it goes elsewhere at once.
        s.failed_stands = vec![failed];
        stand_at(&mut s, first.feet, aim);
        let mut state = fresh_take(&s, Some(east));
        let intent = intent_of(step(&mut state, target, &s.cx(0), &mut rng));
        assert_eq!(intent.press & key::USE, 0);
        assert!(
            matches!(intent.go, Go::To { at, .. } if at.distance_xz(first.feet) >= SAME_STAND_M),
            "{intent:?}"
        );
        // Once it is forgotten, the stand is used again.
        s.view.clock += FAILED_KEEP_MS;
        let mut state = fresh_take(&s, Some(east));
        let intent = intent_of(step(&mut state, target, &s.cx(0), &mut rng));
        assert_ne!(intent.press & key::USE, 0);
    }

    /// Right over the battery (the view does not look down that far) and
    /// wedged: walls round it, no stand off it in a line. The bot steps
    /// back the way it came, straight, with no line looked at (bot 10 of
    /// 0x7db2c16a stood 25 s at 0.2 m from b9, the step back never taken);
    /// out of its reach the play ends.
    #[test]
    fn a_take_wedged_right_over_the_battery_steps_back_the_way_it_came() {
        use crate::v026g::bots::body::tests::wall_x;
        let mut walls = Vec::new();
        walls.extend(wall_x(24.6, 11.5, 12.5));
        walls.extend(wall_x(25.4, 11.5, 12.5));
        // Across z (the walls' x and z swapped).
        for z in [11.6f32, 12.4] {
            for mut t in wall_x(0.0, 24.5, 25.5) {
                for v in &mut t.v {
                    *v = Vec3::new(v.z, v.y, z);
                }
                walls.push(t);
            }
        }
        let over = Vec3::new(25.05, 0.0, 12.0);
        let mut s = take_field(&walls, over);
        let target = TargetRef::Battery(0);
        let mut rng = Rng::new(1);
        let came = Vec3::new(24.0, 0.0, 12.0);
        let mut state = fresh_take(&s, Some(came));
        let intent = intent_of(step(&mut state, target, &s.cx(0), &mut rng));
        assert!(intent.straight, "{intent:?}");
        assert!(
            matches!(intent.go, Go::To { at, .. } if at.distance_xz(Vec3::new(24.0, 0.0, 12.0)) < 0.01),
            "back the way it came: {intent:?}"
        );
        assert!(matches!(
            state,
            State::Take {
                backing: Some(_),
                ..
            }
        ));
        // Not yet out: on back.
        s.bot(0, 0, Vec3::new(24.8, 0.0, 12.0));
        let intent = intent_of(step(&mut state, target, &s.cx(0), &mut rng));
        assert!(intent.straight && matches!(intent.go, Go::To { .. }));
        // Out, and no stand round it: given up.
        s.bot(0, 0, Vec3::new(24.3, 0.0, 12.0));
        assert_eq!(
            step(&mut state, target, &s.cx(0), &mut rng),
            Step::Failed("could not take it")
        );
        // With no way it came known, behind its back.
        let mut s = take_field(&walls, over);
        s.view.places[0].yaw = std::f32::consts::FRAC_PI_2;
        let [fx, fz] = crate::v026g::bots::kin::forward(s.view.places[0].yaw);
        let mut state = fresh_take(&s, None);
        let intent = intent_of(step(&mut state, target, &s.cx(0), &mut rng));
        let behind = Vec3::new(TAKE_AT.x - fx, 0.0, TAKE_AT.z - fz);
        assert!(
            matches!(intent.go, Go::To { at, .. } if at.distance_xz(behind) < 0.01),
            "{intent:?}"
        );
    }

    /// Right over the battery with room round it: a stand off it in a
    /// line, walked to straight (no route: the route's arrival is 0.6 m).
    #[test]
    fn a_take_right_over_the_battery_steps_to_a_stand_in_a_line() {
        let over = Vec3::new(25.05, 0.0, 12.0);
        let s = take_field(&[], over);
        let target = TargetRef::Battery(0);
        let mut rng = Rng::new(1);
        let mut state = fresh_take(&s, Some(Vec3::new(24.0, 0.0, 12.0)));
        let intent = intent_of(step(&mut state, target, &s.cx(0), &mut rng));
        let State::Take {
            stand: Some(stand),
            backing: None,
            ..
        } = state
        else {
            panic!("{state:?}");
        };
        assert!(intent.straight);
        assert!(matches!(intent.go, Go::To { at, .. } if at == stand.feet));
        assert!(clear_from(&s, stand.feet));
    }

    /// A carrier with a known enemy on its straight way home goes by its
    /// side's human route that keeps clear of him; with none known, straight.
    /// A point of the route its search found no way to lately is left out
    /// (straight home here), and one chosen before the search failed is
    /// dropped at the next step: it is not tried again every replan.
    #[test]
    fn a_carrier_goes_home_by_a_human_route_clear_of_known_danger() {
        use crate::v026g::bots::know::Source;
        use crate::v026g::bots::routes::HumanRoute;
        let setup = |enemy: bool| {
            let mut s = Scenario::field(Mode::Batteries);
            s.tuning
                .plays
                .entry(PlayKind::Collect)
                .or_insert_with(|| crate::v026g::bots::plays::default_tuning(PlayKind::Collect))
                .numbers
                .insert("home_danger_m".into(), 6.0);
            s.tactics.set_routes(vec![HumanRoute {
                team: 0,
                lives: 20,
                reward: 1.0,
                fight: 1.0,
                points: [(20.0, 8.0), (30.0, 5.0), (45.0, 5.0)]
                    .iter()
                    .map(|&(x, z)| Vec3::new(x, 0.0, z))
                    .collect(),
                ..Default::default()
            }]);
            s.battery(BatteryState::Carried(Some(0)));
            s.bot(0, 0, Vec3::new(30.0, 0.0, 12.0)).carrying = Some(0);
            if enemy {
                // By the straight way to the container at (12, 30).
                s.enemy(1, 1, Vec3::new(24.0, 0.0, 19.0), Some(Source::Seen));
            }
            s
        };
        let goal = |s: &Scenario| {
            let target = TargetRef::Battery(0);
            let mut rng = Rng::new(1);
            let mut state = start(target, &s.cx(0), &mut rng);
            match intent_of(step(&mut state, target, &s.cx(0), &mut rng)).go {
                Go::To { at, .. } => at,
                other => panic!("{other:?}"),
            }
        };
        let by_route = goal(&setup(true));
        assert!(
            by_route.distance_xz(Vec3::new(20.0, 0.0, 8.0)) < 1.0,
            "{by_route:?}"
        );
        let straight = goal(&setup(false));
        assert!(
            straight.distance_xz(Vec3::new(12.0, 0.0, 30.0)) < 1.0,
            "{straight:?}"
        );
        let point = Vec3::new(20.0, 0.0, 8.0);
        let mut s = setup(true);
        s.no_way_to = vec![(point, s.view.clock + 5_000)];
        let left = goal(&s);
        assert!(
            left.distance_xz(Vec3::new(12.0, 0.0, 30.0)) < 1.0,
            "{left:?}"
        );
        let mut s = setup(true);
        let target = TargetRef::Battery(0);
        let mut rng = Rng::new(1);
        let mut state = start(target, &s.cx(0), &mut rng);
        let first = intent_of(step(&mut state, target, &s.cx(0), &mut rng));
        assert!(matches!(first.go, Go::To { at, .. } if at.distance_xz(point) < 1.0));
        s.no_way_to = vec![(point, s.view.clock + 5_000)];
        let next = intent_of(step(&mut state, target, &s.cx(0), &mut rng));
        assert!(
            matches!(next.go, Go::To { at, .. } if at.distance_xz(Vec3::new(12.0, 0.0, 30.0)) < 1.0),
            "{next:?}"
        );
    }

    /// A hurt carrier among 4 or more known enemies keeps bringing its
    /// battery home: its Bring is not scored as a battery to fetch, so the
    /// commander never gives it a fight play or Roam (which would fire and,
    /// in the game, drop the battery).
    #[test]
    fn a_hurt_carrier_among_enemies_keeps_bringing_it_home() {
        use crate::v026g::bots::know::Source;
        use crate::v026g::bots::team::{Asking, assign};
        let mut s = Scenario::field(Mode::Batteries);
        s.battery(BatteryState::Carried(Some(0)));
        let me = s.bot(0, 0, Vec3::new(40.0, 0.0, 25.0));
        me.carrying = Some(0);
        me.health = 0.1;
        s.bot(2, 0, Vec3::new(36.0, 0.0, 22.0));
        let around = [
            (8.0, 3.0),
            (10.0, -4.0),
            (6.0, 8.0),
            (12.0, 1.0),
            (9.0, -9.0),
        ];
        for (i, (dx, dz)) in around.into_iter().enumerate() {
            let at = Vec3::new(40.0 + dx, 0.0, 25.0 + dz);
            s.enemy(1 + 2 * i, 1, at, Some(Source::Seen));
        }
        let found = s.candidates(0);
        let bring = found
            .iter()
            .find(|c| c.kind == PlayKind::Collect && c.target == TargetRef::Battery(0))
            .unwrap_or_else(|| panic!("no Bring: {found:#?}"));
        assert_eq!(bring.rank, Rank::Alarm);
        let given = assign(
            vec![Asking {
                place: 0,
                candidates: found.clone(),
                current: None,
            }],
            s.view.clock,
        );
        assert_eq!(
            (given[0].1.kind, given[0].1.target),
            (PlayKind::Collect, TargetRef::Battery(0)),
            "{found:#?}"
        );
    }

    /// A put-in counts its use failed only once a world after it shows
    /// the battery still carried (the bench's put-ins "failed" while the
    /// first use had put it in); it ends as the battery did.
    /// (Facing the box: [`a_put_in_presses_once_the_view_is_on_the_box`].)
    #[test]
    fn a_put_in_waits_for_a_world_after_its_use_and_ends_as_the_battery_did() {
        let mut s = Scenario::field(Mode::Batteries);
        s.battery(BatteryState::Carried(Some(0)));
        s.bot(0, 0, Vec3::new(12.0, 0.0, 31.0)).carrying = Some(0);
        let target = TargetRef::Battery(0);
        let mut rng = Rng::new(1);
        let mut state = start(target, &s.cx(0), &mut rng);
        assert_eq!(state, State::Bring { via: None });
        step(&mut state, target, &s.cx(0), &mut rng);
        assert!(matches!(state, State::PutIn { .. }), "{state:?}");
        let used = |s: &Scenario, state: &mut State, rng: &mut Rng| {
            intent_of(step(state, target, &s.cx(0), rng)).press & key::USE != 0
        };
        face_use_box(&mut s);
        assert!(used(&s, &mut state, &mut rng), "the first use");
        s.view.clock += 1_200;
        assert!(!used(&s, &mut state, &mut rng), "no world since");
        // A world 0.6 s after the press: the put (0.6 s) may not be in it.
        s.view.world_time = s.view.clock - 600;
        assert!(!used(&s, &mut state, &mut rng), "the put not judged yet");
        s.view.world_time = s.view.clock - 300;
        assert!(used(&s, &mut state, &mut rng), "the second use");
        s.view.clock += 1_200;
        assert!(
            matches!(
                step(&mut state, target, &s.cx(0), &mut rng),
                Step::Continue(_)
            ),
            "the second use not judged yet"
        );
        s.view.world_time = s.view.clock - 300;
        assert_eq!(
            step(&mut state, target, &s.cx(0), &mut rng),
            Step::Failed("could not put it in")
        );
        // Not carrying any more: in, or another believed to carry it.
        s.view.places[0].carrying = None;
        for (battery, why) in [
            (BatteryState::Home(0), "put it in"),
            (BatteryState::Carried(Some(3)), "someone else carries it"),
        ] {
            s.view.batteries[0] = battery;
            let mut state = State::PutIn {
                tries: 1,
                last_use: s.view.clock,
                arrived: None,
                missed: false,
            };
            assert_eq!(
                step(&mut state, target, &s.cx(0), &mut rng),
                Step::Done(why)
            );
        }
    }

    /// Cologne CG-17: a bot within reach across of its take stand but over
    /// it (an overhang) by more than `motor::ARRIVE_ABOVE_M` does not start
    /// the take: it walks on to the stand. At the stand's height it does.
    #[test]
    fn a_fetch_over_its_stand_walks_on() {
        let at = Vec3::new(20.0, 0.0, 20.0);
        let run = |feet: Vec3| {
            let mut s = Scenario::field(Mode::Batteries);
            s.bot(0, 0, feet);
            s.battery(BatteryState::Free(at));
            let target = TargetRef::Battery(0);
            s.current = Some((PlayKind::Collect, target, s.view.clock));
            let mut rng = Rng::new(1);
            let mut state = State::Fetch;
            let step = step(&mut state, target, &s.cx(0), &mut rng);
            (state, step)
        };
        let (state, step) = run(Vec3::new(20.0, 1.5, 20.5));
        assert_eq!(state, State::Fetch);
        assert!(
            matches!(&step, Step::Continue(i) if matches!(i.go, Go::To { at: to, within_m } if to.distance_xz(at) < 0.01 && within_m == 0.0)),
            "{step:?}"
        );
        let (state, _) = run(Vec3::new(20.0, 0.0, 20.5));
        assert!(matches!(state, State::Take { .. }), "{state:?}");
    }

    /// Bot 0's view on its container's use box (a metre over its use
    /// point), from where it stands.
    fn face_use_box(s: &mut Scenario) {
        use crate::v026g::bots::kin::yaw_of;
        let home = s.view.container_of(0).expect("a container").at;
        let me = &mut s.view.places[0];
        let eye = me.eye();
        let aim = home + Vec3::new(0.0, 1.0, 0.0);
        let (dx, dz) = (aim.x - eye.x, aim.z - eye.z);
        me.yaw = yaw_of([dx, dz]);
        me.pitch = (aim.y - eye.y).atan2(dx.hypot(dz));
    }

    /// A put-in's use is pressed once the view is on the box (a use
    /// pressed while still turning missed it, and since the put's wait each
    /// miss cost 0.6 s more), or after [`SETTLE_MS`] there anyway; a use
    /// whose ray passed the box by over [`PUT_MISS_M`] began no put, so the
    /// next waits for none.
    #[test]
    fn a_put_in_presses_once_the_view_is_on_the_box() {
        let mut s = Scenario::field(Mode::Batteries);
        s.battery(BatteryState::Carried(Some(0)));
        s.bot(0, 0, Vec3::new(12.0, 0.0, 31.0)).carrying = Some(0);
        let target = TargetRef::Battery(0);
        let mut rng = Rng::new(1);
        let mut state = start(target, &s.cx(0), &mut rng);
        step(&mut state, target, &s.cx(0), &mut rng);
        assert!(matches!(state, State::PutIn { .. }), "{state:?}");
        let used = |s: &Scenario, state: &mut State, rng: &mut Rng| {
            intent_of(step(state, target, &s.cx(0), rng)).press & key::USE != 0
        };
        // Turned away (its back to the box, 1.2 m off): no use yet, and
        // none while it turns.
        face_use_box(&mut s);
        s.view.places[0].yaw += std::f32::consts::PI;
        assert!(!used(&s, &mut state, &mut rng), "turned away");
        s.view.clock += 1_000;
        assert!(!used(&s, &mut state, &mut rng), "still turned away");
        // After SETTLE_MS there it presses anyway: a miss, judged at once.
        s.view.clock += SETTLE_MS;
        assert!(
            used(&s, &mut state, &mut rng),
            "pressed after the settle time"
        );
        assert!(
            matches!(state, State::PutIn { missed: true, .. }),
            "{state:?}"
        );
        // The next, on the box, after the use's spacing only: no world
        // came since, and no put is waited for.
        face_use_box(&mut s);
        s.view.clock += 1_200;
        assert!(
            used(&s, &mut state, &mut rng),
            "the next use, no put waited for"
        );
        assert!(
            matches!(state, State::PutIn { missed: false, .. }),
            "{state:?}"
        );
        // That one may have begun a put: the next waits for it.
        s.view.clock += 1_200;
        assert!(!used(&s, &mut state, &mut rng), "the put waited for");
    }

    /// A carrier trapped in a pocket, its escape given up: its Collect has
    /// no slot (the commander gives a given-up bot none), it lets go of the
    /// battery with the fire gate open (`let_go`), and once the view shows
    /// it down the play ends and its next play may fire. While it still
    /// escapes, Collect does not end as "took too long".
    #[test]
    fn a_carrier_whose_escape_gave_up_lets_go_and_may_fire() {
        use crate::v026g::bots::escape::Trapped;
        let mut s = Scenario::field(Mode::Batteries);
        s.battery(BatteryState::Carried(Some(0)));
        s.bot(0, 0, Vec3::new(50.0, 0.0, 32.0)).carrying = Some(0);
        let target = TargetRef::Battery(0);
        let mut rng = Rng::new(1);
        let since = s.view.clock;
        s.tactics.escapes.set_trapped_for_test(
            0,
            Some(Trapped {
                since,
                gave_up: false,
            }),
        );
        s.current = Some((PlayKind::Collect, target, since));
        s.view.clock += 130_000;
        let mut state = start(target, &s.cx(0), &mut rng);
        let intent = intent_of(step(&mut state, target, &s.cx(0), &mut rng));
        assert_eq!(intent.press & key::DROP, 0, "escaping: on with it");
        assert!(matches!(state, State::Bring { .. }), "{state:?}");
        s.tactics.escapes.set_trapped_for_test(
            0,
            Some(Trapped {
                since,
                gave_up: true,
            }),
        );
        let bring = s
            .candidates(0)
            .into_iter()
            .find(|c| c.kind == PlayKind::Collect && c.target == target)
            .expect("its Collect");
        assert_eq!(bring.slot, None);
        let intent = intent_of(step(&mut state, target, &s.cx(0), &mut rng));
        assert_ne!(intent.press & key::DROP, 0, "{intent:?}");
        assert!(intent.let_go && intent.fire);
        assert!(matches!(state, State::LetGo { .. }), "{state:?}");
        // The view still shows it carried (the worlds' lag): the gate stays
        // open, no second press yet.
        s.view.clock += 500;
        let intent = intent_of(step(&mut state, target, &s.cx(0), &mut rng));
        assert!(
            intent.let_go && intent.fire && intent.press == 0,
            "{intent:?}"
        );
        s.view.batteries[0] = BatteryState::Free(Vec3::new(50.0, 0.0, 32.0));
        s.view.places[0].carrying = None;
        assert_eq!(
            step(&mut state, target, &s.cx(0), &mut rng),
            Step::Done("trapped: let go of it")
        );
        s.tactics.escapes.set_trapped_for_test(0, None);
    }
}
