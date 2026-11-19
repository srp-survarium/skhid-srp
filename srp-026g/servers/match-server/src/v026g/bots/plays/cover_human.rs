//! Play 12: Cover a human (`player_bots_behaviour.md` §3): follow a human
//! teammate at 5-10 m and look where he is not looking: behind him or to a
//! side, turned now and then.
//!
//! One bot per human (the slot is his place). It is a quiet default of the
//! roam band, scored so a support personality takes it over roaming and the
//! others seldom do. A bot whose support weight is high (`support_band`)
//! offers it in the fight band instead, as `collect` raises a carrier's: a
//! fight-band play (holding an exit, a crossfire) would else always beat
//! it, and covering the humans is what support bots are for. A human still in a spawn area is not followed (bots do
//! not idle at base); nor is he followed back into one.
//!
//! What the bot reads of him is what a teammate knows: his place, his
//! facing and his stance (the HUD and plain sight of a friend).

use super::*;
use crate::v026g::bots::curve::{Consideration, Curve, Input};
use crate::v026g::bots::kin;

pub const NUMBERS: &[(&str, f32)] = &[
    // Kept this far from him, metres.
    ("follow_min_m", 5.0),
    ("follow_max_m", 10.0),
    // Off his back by this, either side, degrees.
    ("side_min_deg", 20.0),
    ("side_max_deg", 70.0),
    // Farther than this from him it hurries (runs where it may), metres.
    ("catch_up_m", 20.0),
    // A human this far is not followed, metres.
    ("max_m", 120.0),
    // The watched direction off his facing is drawn again this often.
    ("turn_min_s", 4.0),
    ("turn_max_s", 9.0),
    // A point this far out along the watched direction is looked at.
    ("look_m", 20.0),
    // Covered this long, then the commander picks again, seconds.
    ("cover_min_s", 60.0),
    ("cover_max_s", 120.0),
    // A support weight this high offers it in the fight band.
    ("support_band", 1.3),
];

pub fn tuning() -> PlayTuning {
    PlayTuning {
        rank: Rank::Roam,
        considerations: vec![
            // The support weight decides: support (1.6) is full, the rest
            // (1.0 and under) low, so they roam.
            Consideration::new(
                Input::Personality,
                0.0,
                2.0,
                Curve::Piecewise {
                    points: vec![(0.0, 0.0), (0.5, 0.15), (0.7, 0.9), (0.8, 1.0)],
                },
            ),
            // Full within 60 m, nothing past 200.
            Consideration::new(
                Input::PathDist,
                0.0,
                200.0,
                Curve::Piecewise {
                    points: vec![(0.0, 1.0), (0.3, 1.0), (0.75, 0.3), (1.0, 0.0)],
                },
            ),
            considerations::cooldown(15.0),
        ],
        numbers: Default::default(),
    }
}

/// The human teammate at `place`, alive and out of every spawn area.
fn human<'a>(cx: &Ctx<'a>, place: Place) -> Option<&'a PlaceView> {
    cx.view.place(place).filter(|p| {
        p.present
            && p.alive
            && !p.bot
            && p.team == cx.me.team
            && p.place != cx.me.place
            && !cx.tactics.in_spawn_area(p.feet, None)
    })
}

/// [fix-team push `mods.push`] The push's taker this bot escorts (`push::Role::Escort`), a live teammate.
fn push_taker<'a>(cx: &Ctx<'a>, place: Place) -> Option<&'a PlaceView> {
    match cx.team.push.role(cx.me.place) {
        Some(crate::v026g::bots::push::Role::Escort { taker }) if taker == place => {
            cx.view.place(place).filter(|p| p.alive && p.team == cx.me.team && p.place != cx.me.place)
        }
        _ => None,
    }
}

/// [fix-team push `mods.push`] The cover of the push's taker for one of its escorts (UT2003's squad formation centre on the
/// taker): this play's following and watching, in the objective band with the taker's take, by its full support weight.
pub fn push_candidate(cx: &Ctx, taker: Place) -> Option<Candidate> {
    push_taker(cx, taker)?;
    let mut candidate =
        cx.candidate_given(PlayKind::CoverHuman, TargetRef::Place(taker), Some(taker as u32), &[(Input::Personality, 2.0)])?;
    candidate.rank = candidate.rank.max(Rank::Objective);
    Some(candidate)
}

pub fn candidates(cx: &Ctx, out: &mut Vec<Candidate>) {
    let far = cx.num(PlayKind::CoverHuman, "max_m");
    let support = cx.bot.personality.support >= cx.num(PlayKind::CoverHuman, "support_band");
    for p in cx.view.team(cx.me.team) {
        if human(cx, p.place).is_some() && p.feet.distance_xz(cx.me.feet) <= far {
            let Some(mut candidate) = cx.candidate(
                PlayKind::CoverHuman,
                TargetRef::Place(p.place),
                Some(p.place as u32),
            ) else {
                continue;
            };
            if support {
                candidate.rank = candidate.rank.max(Rank::Fight);
            }
            out.push(candidate);
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Follow {
        /// Kept this far from him.
        keep_m: f32,
        /// Where it stands round him, off his back, degrees (signed).
        side_deg: f32,
        /// What it watches, off his facing, degrees.
        watch_deg: f32,
        turn_at: u32,
        until: u32,
    },
}

/// A watched direction off his facing: behind, or a side.
fn draw_watch(rng: &mut Rng) -> f32 {
    [180.0, 90.0, -90.0][rng.below(3)]
}

pub fn start(_target: TargetRef, cx: &Ctx, rng: &mut Rng) -> State {
    let num = |name| cx.num(PlayKind::CoverHuman, name);
    let side = rng.range(num("side_min_deg"), num("side_max_deg"));
    State::Follow {
        keep_m: rng.range(num("follow_min_m"), num("follow_max_m")),
        side_deg: if rng.unit() < 0.5 { -side } else { side },
        watch_deg: draw_watch(rng),
        turn_at: cx.now + (rng.range(num("turn_min_s"), num("turn_max_s")) * 1000.0) as u32,
        until: cx.now + (rng.range(num("cover_min_s"), num("cover_max_s")) * 1000.0) as u32,
    }
}

/// The ground point `m` from `from` along `yaw`.
fn along(from: Vec3, yaw: f32, m: f32) -> Vec3 {
    let f = kin::forward(yaw);
    Vec3::new(from.x + f[0] * m, from.y, from.z + f[1] * m)
}

pub fn step(state: &mut State, target: TargetRef, cx: &Ctx, rng: &mut Rng) -> Step {
    let num = |name| cx.num(PlayKind::CoverHuman, name);
    let TargetRef::Place(place) = target else {
        return Step::Failed("no human to cover");
    };
    let Some(him) = human(cx, place).or_else(|| push_taker(cx, place)) else {
        return Step::Done("the human is gone, dead or back at base");
    };
    let State::Follow {
        keep_m,
        side_deg,
        watch_deg,
        turn_at,
        until,
    } = state;
    if cx.now >= *until {
        return Step::Done("covered him a while");
    }
    if cx.now >= *turn_at {
        *watch_deg = draw_watch(rng);
        *turn_at = cx.now + (rng.range(num("turn_min_s"), num("turn_max_s")) * 1000.0) as u32;
    }
    let look = Look::At(along(
        cx.me.eye(),
        him.yaw + watch_deg.to_radians(),
        num("look_m"),
    ));
    let d = cx.me.feet.distance_xz(him.feet);
    // In the band: stay, matching his stance.
    if d >= num("follow_min_m") - 0.5 && d <= num("follow_max_m") {
        let mut intent = Intent::stay().looking(look);
        if him.crouched() {
            intent = intent.crouched();
        }
        return Step::Continue(intent);
    }
    // Behind him, off to a side; else beside him at least.
    let wanted = along(
        him.feet,
        him.yaw + (180.0 + *side_deg).to_radians(),
        *keep_m,
    );
    let goal = if cx.tactics.fine_goal(wanted, cx.now, cx.bot.side)
        && !cx.tactics.in_spawn_area(wanted, None)
    {
        wanted
    } else {
        him.feet
    };
    let pace = if d > num("catch_up_m") {
        cx.pace_toward(goal)
    } else if him.crouched() {
        Pace::Sneak
    } else {
        Pace::Walk
    };
    let within = if goal == him.feet { *keep_m } else { 1.5 };
    let mut intent = Intent::to(goal, pace).within(within);
    if d <= num("catch_up_m") {
        intent = intent.looking(look);
    }
    Step::Continue(intent)
}

#[cfg(test)]
mod tests {
    use super::super::tests::Scenario;
    use super::*;
    use crate::v026g::bots::view::Mode;

    const SUPPORT: usize = 1;
    const CAMPER: usize = 3;

    /// Bot 0 at (25, 0, 35); human teammate 2 at (40, 0, 33) facing +x.
    fn with_human() -> Scenario {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(25.0, 0.0, 35.0));
        s.human(2, 0, Vec3::new(40.0, 0.0, 33.0)).yaw = kin::yaw_of([1.0, 0.0]);
        s
    }

    #[test]
    fn a_support_bot_prefers_covering_a_human_to_roaming_and_others_do_not() {
        let mut s = with_human();
        s.personality = SUPPORT;
        let best = s.candidates(0);
        assert_eq!(best[0].kind, PlayKind::CoverHuman, "{best:#?}");
        assert_eq!(best[0].rank, Rank::Fight, "a support bot's band");
        assert_eq!(best[0].target, TargetRef::Place(2));
        assert_eq!(
            best[0].slot,
            Some(SlotKey {
                kind: PlayKind::CoverHuman,
                key: 2
            })
        );
        s.personality = CAMPER;
        let best = s.candidates(0);
        let roam = best.iter().find(|c| c.kind == PlayKind::Roam).unwrap();
        let cover = best
            .iter()
            .find(|c| c.kind == PlayKind::CoverHuman)
            .unwrap();
        assert_eq!(cover.rank, Rank::Roam);
        assert!(roam.score.get() > cover.score.get(), "{best:#?}");
    }

    #[test]
    fn no_cover_for_a_bot_a_human_in_spawn_or_an_enemy() {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.personality = SUPPORT;
        s.bot(0, 0, Vec3::new(25.0, 0.0, 35.0));
        s.bot(2, 0, Vec3::new(40.0, 0.0, 33.0));
        s.human(3, 0, Vec3::new(6.0, 0.0, 12.0));
        s.enemy(4, 1, Vec3::new(45.0, 0.0, 35.0), None);
        let mut out = Vec::new();
        candidates(&s.cx(0), &mut out);
        assert!(out.is_empty(), "{out:#?}");
    }

    #[test]
    fn it_follows_at_five_to_ten_metres_looking_away_from_his_facing() {
        let mut s = with_human();
        s.personality = SUPPORT;
        let human = Vec3::new(40.0, 0.0, 33.0);
        let steps = s.run(0, PlayKind::CoverHuman, TargetRef::Place(2), 40, 300, 3.5);
        let last = match steps.last() {
            Some(Step::Continue(i)) => *i,
            other => panic!("{other:?}"),
        };
        assert_eq!(last.go, Go::Stay, "{steps:?}");
        let me = s.view.places[0].feet;
        let d = me.distance_xz(human);
        assert!((4.5..=10.0).contains(&d), "{me:?} {d}");
        let Look::At(at) = last.look else {
            panic!("{last:?}")
        };
        // Not where he looks (+x): behind or a side.
        let dir = [at.x - me.x, at.z - me.z];
        let len = dir[0].hypot(dir[1]);
        assert!(dir[0] / len < 0.5, "{at:?}");
        // He dies: done.
        s.view.places[2].alive = false;
        let mut rng = Rng::new(3);
        let mut state = start(TargetRef::Place(2), &s.cx(0), &mut rng);
        assert!(matches!(
            step(&mut state, TargetRef::Place(2), &s.cx(0), &mut rng),
            Step::Done(_)
        ));
    }
}
