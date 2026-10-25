//! Play 25: keep with the partner. Bots go in pairs (play 21's
//! `BotFacts::partner`); the follower of a pair (the higher place) keeps
//! within trading reach of its leader instead of roaming on its own, and the
//! leader waits a little for a follower left behind (`plays::roam`).
//!
//! Why (the players' matches of 6+ players, 2026-10-14, private study): a
//! death with a teammate within 30 m was answered by a kill of the team
//! within seconds 78 % of the time on the winning side, 61 % with no one
//! near (chance alone gave about 55 %); winners died near a teammate more
//! often, and in the battery modes they stood nearer each other (nearest
//! teammate 31 m against 35 m) and were alone (no one within 50 m) less.
//! Being forward, crouching or sprinting did not separate winners from
//! losers once the sides were of equal size. The tick-level recordings say
//! how near: the killer was killed within 5 s after 20 % of the deaths
//! with a teammate within 20 m, after 3 % of those with none.
//!
//! The follower goes to a point `close_m` short of the leader on its own
//! side of him (a second angle, not his back), runs while farther than
//! `keep_m`, walks nearer. Fights, objectives and alarms outrank it (it is
//! in the roam band), so it is the default of a follower and nothing more;
//! when the leader is down the follower roams by itself.
//!
//! Near him it covers him, as a teammate a player would want: it watches
//! where he does not (a known enemy off his facing, else behind him or a
//! side, drawn and turned now and then, as `cover_human`) and matches his
//! stance. The audit of three real 7v7 matches (2026-10-22) had followers
//! standing beside their leader, doing nothing, for minutes. Stepping out
//! of his line of fire too was tried and left out: it covered more (58 %
//! of the time near him looking more than 60 degrees off his facing,
//! against 47 % looking only and 36 % before) but its moves churned the
//! plays (A->B->A flips +8 %, beyond the noise).

use super::*;
use crate::v026g::bots::curve::{Consideration, Curve, Input};
use crate::v026g::bots::kin;

pub const NUMBERS: &[(&str, f32)] = &[
    // Stand this far from the leader, metres.
    ("close_m", 9.0),
    // Farther than this, run to him, metres.
    ("keep_m", 18.0),
    // The leader waits for a follower farther than this, metres...
    ("wait_m", 25.0),
    // ...but not for one this far (just respawned at home), metres.
    ("wait_max_m", 90.0),
    // The leader waits at most this long a run, seconds.
    ("wait_s", 6.0),
    ("give_up_s", 90.0),
    // Near him: the direction it watches, off his facing, is drawn again
    // this often, seconds. GUESS, cover_human's.
    ("turn_min_s", 4.0),
    ("turn_max_s", 9.0),
    // A point this far out along the watched direction is looked at,
    // metres (cover_human's).
    ("look_m", 20.0),
    // A known enemy this near him and more than `front_deg` off his facing
    // is watched instead, metres. GUESS: the fights' ranges (median kill at
    // 19 m, few past 60).
    ("threat_m", 60.0),
    // What he covers himself: within this of his facing, degrees. GUESS.
    ("front_deg", 60.0),
];

pub fn tuning() -> PlayTuning {
    PlayTuning {
        rank: Rank::Roam,
        considerations: vec![
            // Every personality keeps with its partner; support most.
            Consideration::new(
                Input::Personality,
                0.0,
                2.0,
                Curve::Piecewise {
                    points: vec![(0.0, 0.7), (0.5, 0.9), (1.0, 1.0)],
                },
            ),
        ],
        numbers: Default::default(),
    }
}

/// The bot's partner, alive, when the bot follows him (the higher place
/// of the pair follows).
pub fn leader<'a>(cx: &Ctx<'a>) -> Option<&'a PlaceView> {
    let partner = cx.bot.partner?;
    if partner > cx.me.place || !cx.bot.paired {
        return None;
    }
    cx.view
        .place(partner)
        .filter(|p| p.present && p.alive && p.team == cx.me.team)
}

/// The follower to wait for, when the bot leads its pair and the follower
/// is left behind (farther than `wait_m`, nearer than `wait_max_m`).
pub fn lagging<'a>(cx: &Ctx<'a>) -> Option<&'a PlaceView> {
    let partner = cx.bot.partner?;
    if partner < cx.me.place || !cx.bot.paired {
        return None;
    }
    let num = |name| cx.num(PlayKind::Buddy, name);
    cx.view
        .place(partner)
        .filter(|p| p.present && p.alive && p.team == cx.me.team)
        .filter(|p| {
            let d = p.feet.distance_xz(cx.me.feet);
            d > num("wait_m") && d < num("wait_max_m")
        })
}

pub fn candidates(cx: &Ctx, out: &mut Vec<Candidate>) {
    if cx.me.carrying.is_some() {
        return;
    }
    if let Some(leader) = leader(cx) {
        out.extend(cx.candidate(PlayKind::Buddy, TargetRef::Place(leader.place), None));
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Follow {
        /// What it watches near him, off his facing, degrees.
        watch_deg: f32,
        turn_at: u32,
        /// Till when it keeps going round him ([`round_him`]): a point
        /// beside him proved no way to lately ([`ROUND_HOLD_MS`]).
        round_until: u32,
    },
}

/// A watched direction off his facing: behind, or a side.
fn draw_watch(rng: &mut Rng) -> f32 {
    [180.0, 90.0, -90.0][rng.below(3)]
}

/// When the watched direction is drawn again.
fn next_turn(cx: &Ctx, rng: &mut Rng) -> u32 {
    let num = |name| cx.num(PlayKind::Buddy, name);
    cx.now + (rng.range(num("turn_min_s"), num("turn_max_s")) * 1000.0) as u32
}

pub fn start(_target: TargetRef, cx: &Ctx, rng: &mut Rng) -> State {
    State::Follow {
        watch_deg: draw_watch(rng),
        turn_at: next_turn(cx, rng),
        round_until: 0,
    }
}

/// The ground point `m` from `from` along `yaw`.
fn along(from: Vec3, yaw: f32, m: f32) -> Vec3 {
    let f = kin::forward(yaw);
    Vec3::new(from.x + f[0] * m, from.y, from.z + f[1] * m)
}

/// The degrees between `him`'s facing and the way from him to `at`.
fn off_facing(him: &PlaceView, at: Vec3) -> f32 {
    let (dx, dz) = (at.x - him.feet.x, at.z - him.feet.z);
    let len = dx.hypot(dz);
    if len < 1e-3 {
        return 180.0;
    }
    let f = kin::forward(him.yaw);
    ((f[0] * dx + f[1] * dz) / len).clamp(-1.0, 1.0).acos().to_degrees()
}

/// Where it looks near him: the known enemy nearest him off his facing,
/// else the drawn direction.
fn watch(cx: &Ctx, him: &PlaceView, watch_deg: f32) -> Look {
    let num = |name| cx.num(PlayKind::Buddy, name);
    let threat = cx
        .memory
        .tracks()
        .iter()
        .filter(|t| {
            t.at.distance_xz(him.feet) <= num("threat_m")
                && off_facing(him, t.at) > num("front_deg")
        })
        .min_by(|a, b| {
            a.at.distance_xz(him.feet)
                .total_cmp(&b.at.distance_xz(him.feet))
        })
        .map(|t| t.at);
    Look::At(threat.unwrap_or_else(|| {
        along(cx.me.eye(), him.yaw + watch_deg.to_radians(), num("look_m"))
    }))
}

pub fn step(state: &mut State, target: TargetRef, cx: &Ctx, rng: &mut Rng) -> Step {
    let num = |name| cx.num(PlayKind::Buddy, name);
    if let Some((_, _, started)) = cx.bot.current
        && cx.now > started + (num("give_up_s") * 1000.0) as u32
    {
        return Step::Done("kept with him long enough");
    }
    let TargetRef::Place(place) = target else {
        return Step::Failed("no partner");
    };
    let Some(him) = cx
        .view
        .place(place)
        .filter(|p| p.present && p.alive && p.team == cx.me.team)
    else {
        return Step::Done("the partner is down");
    };
    let d = cx.me.feet.distance_xz(him.feet);
    let close = num("close_m");
    if d <= close * 1.4 {
        let State::Follow { watch_deg, turn_at, .. } = state;
        if cx.now >= *turn_at {
            *watch_deg = draw_watch(rng);
            *turn_at = next_turn(cx, rng);
        }
        let mut intent = Intent::stay().looking(watch(cx, him, *watch_deg));
        if him.crouched() {
            intent = intent.crouched();
        }
        return Step::Continue(intent);
    }
    // Short of him on the bot's own side: beside, not in his back.
    let k = close / d;
    let at = Vec3::new(
        him.feet.x + (cx.me.feet.x - him.feet.x) * k,
        him.feet.y,
        him.feet.z + (cx.me.feet.z - him.feet.z) * k,
    );
    // That point proved no way to (beyond a drop, on a ledge): the nearest
    // of those round him it may have a way to, kept up [`ROUND_HOLD_MS`]
    // past the proof (not back to the shut side the think the point slides
    // out of it); none: kept, and the think leaves the play, `goal_shut`.
    let State::Follow { round_until, .. } = state;
    if cx.no_way_lately(at) {
        *round_until = cx.now + ROUND_HOLD_MS;
    }
    let at = if cx.now < *round_until {
        round_him(cx, him.feet).unwrap_or(at)
    } else {
        at
    };
    let pace = if d > num("keep_m") {
        cx.pace_toward(at)
    } else {
        Pace::Walk
    };
    Step::Continue(Intent::to(at, pace).within(close * 0.3))
}

/// Of the points [`ROUND_HIM_M`] round `feet` (not his own cell: two bodies
/// there block each other), the nearest this bot may have a way to
/// ([`Ctx::reachable`], no search lately proved none).
fn round_him(cx: &Ctx, feet: Vec3) -> Option<Vec3> {
    ROUND_HIM_M
        .iter()
        .flat_map(|&m| {
            (0..8).map(move |k| {
                let yaw = k as f32 * std::f32::consts::FRAC_PI_4;
                along(feet, yaw, m)
            })
        })
        .filter(|&p| cx.can_go(p))
        .min_by(|a, b| {
            a.distance_xz(cx.me.feet)
                .total_cmp(&b.distance_xz(cx.me.feet))
        })
}

/// How far round the partner [`round_him`] looks, metres.
const ROUND_HIM_M: [f32; 2] = [1.5, 2.0];

/// How long it keeps going round him once the point beside him is no
/// longer proved no way to, ms.
const ROUND_HOLD_MS: u32 = 3_000;

#[cfg(test)]
mod tests {
    use super::super::tests::Scenario;
    use super::*;
    use crate::v026g::bots::know::Source;
    use crate::v026g::bots::view::Mode;
    use crate::v026g::drive::key;

    /// Bot 2 follows its partner bot 0, `d` metres ahead on the x axis.
    fn pair(d: f32) -> Scenario {
        let mut s = Scenario::field(Mode::Batteries);
        s.bot(0, 0, Vec3::new(20.0 + d, 0.0, 30.0));
        s.bot(2, 0, Vec3::new(20.0, 0.0, 30.0));
        s.partner = Some(0);
        s
    }

    #[test]
    fn the_follower_keeps_with_its_leader_instead_of_roaming_alone() {
        let s = pair(40.0);
        let best = s.candidates(2);
        // Its default: the roam band holds only this (fights outrank it).
        let roam_band: Vec<_> = best.iter().filter(|c| c.rank == Rank::Roam).collect();
        assert_eq!(roam_band[0].kind, PlayKind::Buddy, "{best:#?}");
        assert!(best.iter().all(|c| c.kind != PlayKind::Roam), "{best:#?}");
        // The leader (lower place) roams; it follows no one.
        let mut s = pair(40.0);
        s.partner = Some(2);
        let lead = s.candidates(0);
        assert!(lead.iter().all(|c| c.kind != PlayKind::Buddy));
        assert!(lead.iter().any(|c| c.kind == PlayKind::Roam));
        // A dead leader: the follower roams by itself.
        let mut s = pair(40.0);
        s.view.places[0].alive = false;
        let alone = s.candidates(2);
        assert!(alone.iter().all(|c| c.kind != PlayKind::Buddy));
        assert!(alone.iter().any(|c| c.kind == PlayKind::Roam));
        // A team's odd bot partners one way, for its trades: it follows
        // no one, and no one waits for it.
        let mut s = pair(40.0);
        s.paired = false;
        let odd = s.candidates(2);
        assert!(odd.iter().all(|c| c.kind != PlayKind::Buddy), "{odd:#?}");
        let mut s = pair(0.0);
        s.partner = Some(2);
        s.view.places[0].feet = Vec3::new(60.0, 0.0, 30.0);
        assert!(lagging(&s.cx(0)).is_some());
        s.paired = false;
        assert!(lagging(&s.cx(0)).is_none());
    }

    #[test]
    fn it_closes_to_beside_him_and_stands_there() {
        let mut s = pair(40.0);
        let steps = s.run(2, PlayKind::Buddy, TargetRef::Place(0), 40, 250, 5.0);
        let d = s.view.places[2].feet.distance_xz(s.view.places[0].feet);
        assert!((5.0..=13.0).contains(&d), "{d}");
        assert_eq!(
            steps
                .last()
                .map(|s| matches!(s, Step::Continue(i) if i.go == Go::Stay)),
            Some(true)
        );
        // The leader down: the play ends.
        s.view.places[0].alive = false;
        let steps = s.run(2, PlayKind::Buddy, TargetRef::Place(0), 2, 250, 5.0);
        assert_eq!(steps[0], Step::Done("the partner is down"));
    }

    /// Near him it watches where he does not look, in his stance.
    #[test]
    fn near_him_it_covers_his_back_in_his_stance() {
        // The cosine between his facing and the way from the bot to what
        // it looks at.
        let looks_his_way = |s: &Scenario, look: Look| {
            let Look::At(at) = look else {
                panic!("{look:?}")
            };
            let me = s.view.places[2].feet;
            let f = kin::forward(s.view.places[0].yaw);
            let (dx, dz) = (at.x - me.x, at.z - me.z);
            (f[0] * dx + f[1] * dz) / dx.hypot(dz)
        };
        let first = |s: &Scenario, seed: u64| {
            let mut rng = Rng::new(seed);
            let mut state = start(TargetRef::Place(0), &s.cx(2), &mut rng);
            match step(&mut state, TargetRef::Place(0), &s.cx(2), &mut rng) {
                Step::Continue(i) => i,
                other => panic!("{other:?}"),
            }
        };
        // He faces away from it (+x), it stands 8 m behind: it stays and
        // watches behind him or a side, never his way.
        let mut s = pair(8.0);
        s.view.places[0].yaw = kin::yaw_of([1.0, 0.0]);
        for seed in 0..6 {
            let i = first(&s, seed);
            assert_eq!(i.go, Go::Stay, "{i:?}");
            assert!(looks_his_way(&s, i.look) < 0.5, "{i:?}");
        }
        // A known enemy off his facing: watched.
        let enemy = Vec3::new(20.0, 0.0, 10.0);
        s.enemy(5, 1, enemy, Some(Source::Heard));
        assert_eq!(first(&s, 1).look, Look::At(enemy));
        // A known enemy he faces: his own, so it keeps to its watch.
        let ahead = Vec3::new(40.0, 0.0, 30.0);
        let mut s = pair(8.0);
        s.view.places[0].yaw = kin::yaw_of([1.0, 0.0]);
        s.enemy(5, 1, ahead, Some(Source::Heard));
        assert_ne!(first(&s, 1).look, Look::At(ahead));
        // He crouches: so does it, staying.
        s.view.places[0].keys |= key::CROUCH;
        let i = first(&s, 1);
        assert!(i.go == Go::Stay && i.crouch, "{i:?}");
    }

    #[test]
    fn a_leader_waits_for_a_follower_left_behind_but_not_for_one_at_home() {
        let mut s = pair(0.0);
        s.partner = Some(2);
        s.view.places[0].feet = Vec3::new(60.0, 0.0, 30.0);
        assert!(lagging(&s.cx(0)).is_some(), "40 m behind");
        s.view.places[0].feet = Vec3::new(40.0, 0.0, 30.0);
        assert!(lagging(&s.cx(0)).is_none(), "20 m: near enough");
        s.view.places[0].feet = Vec3::new(20.0, 0.0, 130.0);
        assert!(lagging(&s.cx(0)).is_none(), "100 m: respawned at home");
    }

    /// The point beside him proved no way to: the nearest point 1.5-2 m round
    /// him it may have a way to, not his own spot.
    #[test]
    fn beside_him_proved_no_way_to_it_goes_round_him_instead() {
        let mut s = pair(40.0);
        let mut rng = Rng::new(5);
        let him = s.view.places[0].feet;
        let mut state = start(TargetRef::Place(0), &s.cx(2), &mut rng);
        let to = |st: Step| match st {
            Step::Continue(Intent { go: Go::To { at, .. }, .. }) => at,
            other => panic!("{other:?}"),
        };
        let beside = to(step(&mut state, TargetRef::Place(0), &s.cx(2), &mut rng));
        assert!(beside.distance_xz(him) > 5.0, "{beside:?}");
        s.no_way_to = vec![(beside, s.view.clock + 5_000)];
        let round = to(step(&mut state, TargetRef::Place(0), &s.cx(2), &mut rng));
        let d = round.distance_xz(him);
        assert!((1.4..=2.1).contains(&d), "{round:?} {d}");
        // The nearest of them: on its side of him.
        assert!(round.x < him.x, "{round:?}");
        // Kept round him for ROUND_HOLD_MS past the proof, then beside again.
        s.no_way_to.clear();
        s.view.clock += ROUND_HOLD_MS - 50;
        let kept = to(step(&mut state, TargetRef::Place(0), &s.cx(2), &mut rng));
        assert_eq!(kept, round);
        s.view.clock += 50;
        let back = to(step(&mut state, TargetRef::Place(0), &s.cx(2), &mut rng));
        assert_eq!(back, beside);
    }
}
