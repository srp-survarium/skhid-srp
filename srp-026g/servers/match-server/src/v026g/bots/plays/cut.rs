//! Play 26: cut the enemy's best way (the owner's idea, 2026-10-14:
//! "countering enemies best paths to reduce their win chances").
//!
//! The ways the enemy side's humans take out of their base, and what each
//! gave them, are play 17's table (`routes.rs`). The way most used and
//! best paid is the one to cut: a bot holds a spot off it where it comes
//! into our half, with a sight line onto it from the side, so whoever comes
//! that way walks into a held angle. The recordings of the players'
//! matches: when one of two saw the other first and was already looking
//! his way (by more than 5 degrees nearer), he won 64 % of the duels
//! decided (by the shots of a man who never fired back).
//!
//! - One candidate per enemy route (its slot), weighed by the route's
//!   share of the side's lives times what a life on it gave, so the bots
//!   spread over the best ways first. Only the best `most_cuts` ways are
//!   cut, and ways whose cut points lie within `merge_m` of a better one's
//!   are that one's cut: with the top players' routes in the table a side
//!   has up to 10 ways, several through the same place.
//! - The cut point: where the route first comes `cut_progress` of the way
//!   from their base to ours (past the middle: the owner's "never in the
//!   middle"), on the floor there, out of every spawn area; the end of that
//!   leg when no fine floor is there. A route point moved a little (a 3D
//!   point onto the players' floor) moves the cut a little: taken at the
//!   first point past `cut_progress`, a point at 0.600 moved 0.3 m sent the
//!   cut a leg on, 25 m, by their base's (level_01 day 4: 2 kills in 64
//!   holds there against 5 in 10).
//! - The spot: `spot_min_m`-`spot_max_m` from it with a sight line to it,
//!   off the route's line by at least `side_deg` (a side angle, not in
//!   their face), not on their side of it, higher ground preferred; one of
//!   the best few, drawn. With no such spot, straight down their way, then
//!   in a wider ring. The spots are looked for once and kept a while for
//!   the side, and a cut is offered only where one is fine for the bot:
//!   taken to find none, it ended within the second (2,992 times in the
//!   Team Deathmatch bench with the routes, 16 seeds; the audit's churn).
//! - Held crouched, looking where they come from. Team Deathmatch: a long
//!   hold; the battery modes: brief (the fights follow the batteries).
//!   Moved on after 1-2 kills.

use super::*;
use crate::v026g::bots::curve::{Consideration, Curve, Input, Score};
use crate::v026g::bots::tactics::{SpotQuery, Tactics};
use crate::v026g::bots::view::Mode;

pub const NUMBERS: &[(&str, f32)] = &[
    // The cut point: this share of the way from their base to ours.
    ("cut_progress", 0.6),
    ("spot_min_m", 12.0),
    ("spot_max_m", 28.0),
    // Off the route's line by at least this, degrees.
    ("side_deg", 35.0),
    ("prefer_high", 0.5),
    ("pick_of", 3.0),
    ("reach_m", 2.0),
    ("sneak_m", 10.0),
    ("hold_tdm_min_s", 30.0),
    ("hold_tdm_max_s", 60.0),
    ("hold_batteries_min_s", 10.0),
    ("hold_batteries_max_s", 20.0),
    ("weight_tdm", 1.0),
    ("weight_batteries", 0.6),
    ("weight_timed", 0.5),
    ("kills_min", 1.0),
    ("kills_max", 2.0),
    ("give_up_s", 60.0),
    ("samples", 64.0),
    // A cut's spots are looked for once and kept this long for the side,
    // seconds. GUESS, as HoldExit's and Crossfire's: the sight rays cast
    // once a quarter-minute a cut, a hazard on a spot still checked at each
    // poll.
    ("spots_keep_s", 15.0),
    // Cut points this near are one cut (the better way's), metres; and
    // at most this many ways are cut. GUESS: a cut's spot ring is 12-28 m,
    // so two points 10 m apart are held from the same spots; 4 is the old
    // table's most ways a side.
    ("merge_m", 10.0),
    ("most_cuts", 4.0),
];

pub fn tuning() -> PlayTuning {
    PlayTuning {
        rank: Rank::Fight,
        considerations: vec![
            considerations::personality(),
            considerations::holding(),
            considerations::not_behind_late(),
            considerations::near(250.0),
            Consideration::new(Input::FriendsNearTarget, 0.0, 3.0, Curve::Linear).inverted(),
            considerations::health_above(0.3),
            considerations::cooldown(30.0),
            considerations::runtime(90.0),
        ],
        numbers: Default::default(),
    }
}

/// The spawn centre of a side.
fn centre(tactics: &crate::v026g::bots::tactics::Tactics, side: i32) -> Option<Vec3> {
    let s: Vec<Vec3> = tactics
        .world
        .map
        .spawns
        .iter()
        .filter(|s| s.team == side)
        .map(|s| s.position)
        .collect();
    (!s.is_empty()).then(|| s.iter().fold(Vec3::ZERO, |a, b| a + *b) * (1.0 / s.len() as f32))
}

/// Share of the way from `from` to `to` that `at` stands at.
fn progress(from: Vec3, to: Vec3, at: Vec3) -> f32 {
    let (dx, dz) = (to.x - from.x, to.z - from.z);
    let l = dx * dx + dz * dz;
    if l <= 0.0 {
        return 0.0;
    }
    ((at.x - from.x) * dx + (at.z - from.z) * dz) / l
}

/// Where `route` first comes `min` of the way from `theirs` to `ours`, and
/// the point before it they come from: on the leg that crosses `min`, where
/// it crosses, on the floor there; else that leg's end. None when that is
/// in a spawn area or no fine goal for `side`.
fn cut_on(
    cx: &Ctx,
    route: &super::super::routes::HumanRoute,
    (theirs, ours): (Vec3, Vec3),
    min: f32,
    side: i32,
) -> Option<(Vec3, Vec3)> {
    let points = &route.points;
    let i = points
        .iter()
        .position(|&p| progress(theirs, ours, p) >= min)?;
    let before = |k: usize| if k > 0 { points[k - 1] } else { theirs };
    let fine =
        |p: Vec3| !cx.tactics.in_spawn_area(p, None) && cx.tactics.fine_goal(p, cx.now, Some(side));
    if i > 0 {
        let (a, b) = (points[i - 1], points[i]);
        let (pa, pb) = (progress(theirs, ours, a), progress(theirs, ours, b));
        let t = if pb > pa {
            ((min - pa) / (pb - pa)).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let grid = &cx.tactics.world.grid;
        if let Some(at) = grid
            .locate(a + (b - a) * t, ON_FLOOR_M)
            .map(|n| grid.position(n))
            && fine(at)
        {
            let from = if at.distance_xz(a) >= 1.0 {
                a
            } else {
                before(i - 1)
            };
            return Some((at, from));
        }
    }
    let at = points[i];
    fine(at).then(|| (at, before(i)))
}

/// How far from a point on a route's leg its floor may lie (the bots'
/// goals' reach, `Tactics::fine_goal`).
const ON_FLOOR_M: f32 = 3.0;

/// The enemy's best ways' cuts, best first: (route index, cut point, the
/// point before it they come from, weight in (0, 1]); one a place
/// (`merge_m`), `most_cuts` at most.
pub fn cuts(cx: &Ctx) -> Vec<(usize, Vec3, Vec3, f32)> {
    let Some(side) = cx.bot.side else {
        return Vec::new();
    };
    let Some(enemy) = cx.enemy_side(side) else {
        return Vec::new();
    };
    let (Some(theirs), Some(ours)) = (centre(cx.tactics, enemy), centre(cx.tactics, side)) else {
        return Vec::new();
    };
    let min = cx.num(PlayKind::Cut, "cut_progress");
    let routes: Vec<_> = cx.tactics.routes_of(enemy).collect();
    let worth = |r: &super::super::routes::HumanRoute| r.lives as f32 * r.reward.max(0.1);
    let best = routes.iter().map(|r| worth(r)).fold(0.0, f32::max);
    if best <= 0.0 {
        return Vec::new();
    }
    let mut all = routes
        .iter()
        .enumerate()
        .filter_map(|(index, route)| {
            let (at, from) = cut_on(cx, route, (theirs, ours), min, side)?;
            Some((index, at, from, worth(route) / best))
        })
        .collect::<Vec<_>>();
    all.sort_by(|a, b| b.3.total_cmp(&a.3).then(a.0.cmp(&b.0)));
    let merge = cx.num(PlayKind::Cut, "merge_m");
    let most = cx.num(PlayKind::Cut, "most_cuts").max(0.0) as usize;
    let mut out: Vec<(usize, Vec3, Vec3, f32)> = Vec::new();
    for cut in all {
        if out.len() >= most {
            break;
        }
        if out.iter().all(|kept| kept.1.distance_xz(cut.1) >= merge) {
            out.push(cut);
        }
    }
    out
}

pub fn candidates(cx: &Ctx, out: &mut Vec<Candidate>) {
    if cx.me.carrying.is_some() {
        return;
    }
    let weight = match cx.view.mode {
        Mode::TeamDeathmatch => cx.num(PlayKind::Cut, "weight_tdm"),
        Mode::Batteries => cx.num(PlayKind::Cut, "weight_batteries"),
        Mode::TimedBatteries => cx.num(PlayKind::Cut, "weight_timed"),
    };
    let running = match cx.bot.current {
        Some((PlayKind::Cut, TargetRef::Point(p), _)) => Some(p),
        _ => None,
    };
    for (index, at, from, worth) in cuts(cx) {
        // Offered only where a spot is fine for it now.
        let runs = running.is_some_and(|p| p.distance_xz(at) < 1.0);
        if !runs && fine_spots(cx, at, from).is_empty() {
            continue;
        }
        if let Some(mut c) = cx.candidate(PlayKind::Cut, TargetRef::Point(at), Some(index as u32)) {
            c.score = Score::new(c.score.get() * worth * weight);
            if !c.score.is_zero() {
                out.push(c);
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    NoSpot,
    Going {
        spot: Vec3,
        from: Vec3,
    },
    Hold {
        from: Vec3,
        until: u32,
        kills_at: u32,
        kills: u32,
    },
}

/// The spots of `pick`'s `try_` (0-2) to cut the way through `at` (coming
/// from `from`), best first: in sight of it, in the try's ring and angle,
/// outside every spawn area; found once for the side and kept
/// `spots_keep_s`.
fn spots_by(cx: &Ctx, at: Vec3, from: Vec3, try_: u32) -> std::sync::Arc<Vec<Vec3>> {
    let num = |name| cx.num(PlayKind::Cut, name);
    // Their way's direction at the cut.
    let (wx, wz) = (at.x - from.x, at.z - from.z);
    let wl = wx.hypot(wz).max(0.01);
    let side_cos = num("side_deg").to_radians().cos();
    // Along their way: 1 straight ahead of them (our side), -1 back where
    // they come from.
    let along = |spot: &Vec3| {
        let (sx, sz) = (spot.x - at.x, spot.z - at.z);
        (sx * wx + sz * wz) / (sx.hypot(sz).max(0.01) * wl)
    };
    // A side angle first; then straight down their way (still not on their
    // side); then a wider ring.
    let (min_m, max_m, most) = match try_ {
        0 => (num("spot_min_m"), num("spot_max_m"), side_cos),
        1 => (num("spot_min_m"), num("spot_max_m"), 1.0),
        _ => (num("spot_min_m") * 0.6, num("spot_max_m") * 1.5, 1.0),
    };
    let key = Tactics::spot_key(PlayKind::Cut as u32 + 64 * try_, cx.bot.side, at);
    let keep = (num("spots_keep_s") * 1000.0) as u32;
    cx.tactics.spots_kept(key, cx.now, keep, || {
        let query = SpotQuery {
            sight_to: Some(at),
            prefer_high: num("prefer_high"),
            side: cx.bot.side,
            samples: num("samples") as usize,
            ..SpotQuery::ring(at, min_m, max_m)
        };
        let mut rng = Rng::new(key ^ u64::from(cx.now));
        cx.tactics
            // More than a plain hold asks for: the side rule drops many.
            .spots(&query, cx.now, 40, &mut rng)
            .into_iter()
            .map(|s| s.at)
            .filter(|spot| (-0.2..=most).contains(&along(spot)))
            .filter(|spot| !cx.tactics.in_spawn_area(*spot, None))
            .collect()
    })
}

/// The spots this bot may take now to cut the way through `at`, best
/// first: those of the first of [`spots_by`]'s tries with a fine goal that
/// this bot has a way to ([`Ctx::reachable`], no search lately proved none):
/// the ring's high spots in sight of the cut point are often ledges no walk
/// gets onto, and a bot sent there stood with no way.
fn fine_spots(cx: &Ctx, at: Vec3, from: Vec3) -> Vec<Vec3> {
    (0..3)
        .map(|try_| {
            spots_by(cx, at, from, try_)
                .iter()
                .copied()
                .filter(|spot| {
                    cx.tactics.fine_goal(*spot, cx.now, cx.bot.side)
                        && cx.reachable(*spot, false)
                        && !cx.no_way_lately(*spot)
                })
                .collect::<Vec<_>>()
        })
        .find(|fine| !fine.is_empty())
        .unwrap_or_default()
}

/// A spot to cut the way through `at` (coming from `from`).
fn pick(cx: &Ctx, at: Vec3, from: Vec3, rng: &mut Rng) -> Option<Vec3> {
    let fine = fine_spots(cx, at, from);
    let n = fine.len().min(cx.num(PlayKind::Cut, "pick_of").max(1.0) as usize);
    (n > 0).then(|| fine[rng.below(n)])
}

pub fn start(target: TargetRef, cx: &Ctx, rng: &mut Rng) -> State {
    let Some(at) = cx.point_of(target) else {
        return State::NoSpot;
    };
    let from = cuts(cx)
        .into_iter()
        .find(|c| c.1.distance_xz(at) < 1.0)
        .map_or(at, |c| c.2);
    match pick(cx, at, from, rng) {
        Some(spot) => State::Going { spot, from },
        None => State::NoSpot,
    }
}

pub fn step(state: &mut State, target: TargetRef, cx: &Ctx, rng: &mut Rng) -> Step {
    let num = |name| cx.num(PlayKind::Cut, name);
    let Some(at) = cx.point_of(target) else {
        return Step::Failed("no cut point");
    };
    match state {
        State::NoSpot => Step::Failed("no spot by their way"),
        State::Going { spot, from } => {
            // Its spot proved no way to: another of the fine ones (none:
            // kept, and the think leaves the play, `goal_shut`).
            if cx.no_way_lately(*spot)
                && let Some(new) = pick(cx, at, *from, rng)
            {
                *spot = new;
            }
            let (spot, from) = (*spot, *from);
            if let Some((_, _, started)) = cx.bot.current
                && cx.now > started + (num("give_up_s") * 1000.0) as u32
            {
                return Step::Failed("could not reach its spot");
            }
            let d = cx.me.feet.distance_xz(spot);
            if d <= num("reach_m") {
                let (lo, hi) = if cx.view.mode == Mode::TeamDeathmatch {
                    (num("hold_tdm_min_s"), num("hold_tdm_max_s"))
                } else {
                    (num("hold_batteries_min_s"), num("hold_batteries_max_s"))
                };
                let kills = rng
                    .range(num("kills_min"), num("kills_max") + 1.0)
                    .floor()
                    .clamp(num("kills_min"), num("kills_max"))
                    .max(1.0) as u32;
                *state = State::Hold {
                    from,
                    until: cx.now + (rng.range(lo, hi) * 1000.0) as u32,
                    kills_at: cx.me.kills,
                    kills,
                };
                return Step::Continue(Intent::stay().crouched().looking(Look::At(from)));
            }
            let pace = if d < num("sneak_m") {
                Pace::Sneak
            } else {
                cx.pace_toward(spot)
            };
            Step::Continue(Intent::to(spot, pace).within(num("reach_m") * 0.7))
        }
        State::Hold {
            from,
            until,
            kills_at,
            kills,
        } => {
            if cx.me.kills >= *kills_at + *kills {
                return Step::Done("moved on after its kills");
            }
            if cx.now >= *until {
                return Step::Done("held long enough");
            }
            Step::Continue(Intent::stay().crouched().looking(Look::At(*from)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::routes::HumanRoute;
    use super::super::tests::Scenario;
    use super::*;

    /// The enemy side (1, spawns at x 75) has a busy, well paid way along
    /// z 30 and a poor one along z 5; bot 0 of side 0 at (20, 0, 20).
    fn field(mode: Mode) -> Scenario {
        let mut s = Scenario::field(mode);
        s.bot(0, 0, Vec3::new(20.0, 0.0, 20.0));
        let route = |lives: u32, reward: f32, z: f32| HumanRoute {
            team: 1,
            lives,
            reward,
            fight: reward,
            points: [65.0, 55.0, 45.0, 35.0, 25.0]
                .iter()
                .map(|&x| Vec3::new(x, 0.0, z))
                .collect(),
            ..Default::default()
        };
        s.tactics
            .set_routes(vec![route(30, 3.0, 30.0), route(10, 1.0, 5.0)]);
        s
    }

    fn cut_candidates(s: &Scenario) -> Vec<Candidate> {
        let mut out = Vec::new();
        candidates(&s.cx(0), &mut out);
        out
    }

    #[test]
    fn the_best_enemy_way_is_cut_first_and_past_the_middle() {
        let s = field(Mode::TeamDeathmatch);
        let cx = s.cx(0);
        let cuts = cuts(&cx);
        assert_eq!(cuts.len(), 2, "{cuts:?}");
        // 60 % of the way from x 75 to x 5 is x 33, on the leg from x 35
        // to x 25. (Points sit on the grid's floors, within a cell.)
        assert!(cuts[0].1.distance_xz(Vec3::new(33.0, 0.0, 30.0)) < 0.5);
        assert!(cuts[0].2.distance_xz(Vec3::new(35.0, 0.0, 30.0)) < 0.5);
        let out = cut_candidates(&s);
        assert_eq!(out.len(), 2);
        let busy = out
            .iter()
            .find(|c| c.slot.map(|k| k.key) == Some(0))
            .unwrap();
        let poor = out
            .iter()
            .find(|c| c.slot.map(|k| k.key) == Some(1))
            .unwrap();
        assert!(busy.score.get() > 3.0 * poor.score.get());
        // Less in the battery modes; none for a carrier or with no routes.
        let b = cut_candidates(&field(Mode::Batteries));
        assert!(b.iter().map(|c| c.score.get()).fold(0.0, f32::max) < busy.score.get());
        let mut s = field(Mode::TeamDeathmatch);
        s.view.places[0].carrying = Some(0);
        assert!(cut_candidates(&s).is_empty());
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(20.0, 0.0, 20.0));
        assert!(cut_candidates(&s).is_empty());
    }

    /// A route point moved a little across `cut_progress` (x 33 is 60 % of
    /// the way from x 75 to x 5) moves the cut a little, not a leg on.
    #[test]
    fn a_point_moved_a_little_moves_the_cut_a_little() {
        let cut_at = |x: f32| {
            let mut s = Scenario::field(Mode::TeamDeathmatch);
            s.bot(0, 0, Vec3::new(20.0, 0.0, 20.0));
            s.tactics.set_routes(vec![HumanRoute {
                team: 1,
                lives: 10,
                reward: 1.0,
                fight: 1.0,
                points: [65.0, 55.0, 45.0, x, 15.0]
                    .iter()
                    .map(|&x| Vec3::new(x, 0.0, 30.0))
                    .collect(),
                ..Default::default()
            }]);
            cuts(&s.cx(0))[0].1
        };
        let (short, past) = (cut_at(33.2), cut_at(32.8));
        assert!(short.distance_xz(past) < 1.5, "{short:?} {past:?}");
        assert!(
            past.distance_xz(Vec3::new(33.0, 0.0, 30.0)) < 1.0,
            "{past:?}"
        );
    }

    /// Ways through one place are one cut (the better way's), and only the
    /// best `most_cuts` ways are cut.
    #[test]
    fn ways_through_one_place_are_one_cut_and_only_the_best_are_cut() {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(20.0, 0.0, 20.0));
        let route = |lives: u32, z: f32| HumanRoute {
            team: 1,
            lives,
            reward: 1.0,
            fight: 1.0,
            points: [65.0, 55.0, 45.0, 35.0, 25.0]
                .iter()
                .map(|&x| Vec3::new(x, 0.0, z))
                .collect(),
            ..Default::default()
        };
        // Four ways: z 30 and 34 through one place (the second better), the
        // others 12 m apart and less used.
        s.tactics.set_routes(vec![
            route(20, 30.0),
            route(30, 34.0),
            route(12, 5.0),
            route(11, 17.0),
        ]);
        let ways = |s: &Scenario| cuts(&s.cx(0)).iter().map(|c| c.0).collect::<Vec<_>>();
        assert_eq!(ways(&s), vec![1, 2, 3]);
        s.tuning
            .plays
            .entry(PlayKind::Cut)
            .or_insert_with(|| crate::v026g::bots::plays::default_tuning(PlayKind::Cut))
            .numbers
            .insert("most_cuts".into(), 2.0);
        assert_eq!(ways(&s), vec![1, 2]);
        let cuts = cuts(&s.cx(0));
        assert!((cuts[0].3 - 1.0).abs() < 1e-6);
    }

    #[test]
    fn it_holds_a_side_spot_looking_where_they_come_from() {
        let mut s = field(Mode::TeamDeathmatch);
        let (_, at, from, _) = cuts(&s.cx(0))[0];
        let steps = s.run(0, PlayKind::Cut, TargetRef::Point(at), 60, 250, 5.0);
        let held = steps.iter().find_map(|st| match st {
            Step::Continue(i) if i.go == Go::Stay => Some(*i),
            _ => None,
        });
        let held = held.unwrap_or_else(|| panic!("{steps:?}"));
        assert_eq!(held.look, Look::At(from));
        let me = s.view.places[0].feet;
        let d = me.distance_xz(at);
        assert!((10.0..=30.0).contains(&d), "{me:?} {d}");
        // Off their line (z 30) to the side, not straight down it.
        let (sx, sz) = (me.x - at.x, me.z - at.z);
        let along = (sx * -10.0) / (sx.hypot(sz) * 10.0);
        assert!(
            (-0.2..=35f32.to_radians().cos() + 1e-3).contains(&along),
            "{me:?}"
        );
    }

    /// The real routes on the real maps (private files): each side with
    /// enemy routes gets cuts, and a bot at its spawns finds a spot for
    /// most of them. Run with `SRP_MAPDATA_DIR` and `SRP_BOT_ROUTES` set,
    /// in release.
    #[test]
    #[ignore = "needs the private map files and route table"]
    fn the_real_enemy_ways_get_cuts_with_spots_on_the_real_maps() {
        use super::super::super::routes::RouteTable;
        let (Some(dir), Some(table)) = (
            std::env::var_os("SRP_MAPDATA_DIR"),
            std::env::var_os("SRP_BOT_ROUTES"),
        ) else {
            panic!("set SRP_MAPDATA_DIR and SRP_BOT_ROUTES");
        };
        let table = RouteTable::load(table.as_ref()).unwrap();
        let (mut sides, mut with_cuts, mut cuts_n, mut spots) = (0, 0, 0, 0);
        for level in 1..=8u8 {
            for (mode, view_mode) in [
                (2u8, Mode::Batteries),
                (3, Mode::TimedBatteries),
                (4, Mode::TeamDeathmatch),
            ] {
                let path =
                    std::path::Path::new(&dir).join(format!("level_{level:02}_day_{mode}.mapdata"));
                let map = mapdata::MapData::read(&path).unwrap();
                let world = std::sync::Arc::new(mapdata::World::build(
                    map,
                    mapdata::CELL,
                    mapdata::Body::default(),
                ));
                for side in [0, 1] {
                    let mut s = Scenario::field(view_mode);
                    s.tactics = crate::v026g::bots::tactics::Tactics::new(world.clone(), 1.0);
                    s.tactics.set_routes(table.for_map(level, mode));
                    s.side = Some(side);
                    let Some(home) = centre(&s.tactics, side) else {
                        continue;
                    };
                    s.bot(0, side as u8, home);
                    if s.tactics.routes_of(1 - side).next().is_none() {
                        continue;
                    }
                    sides += 1;
                    let found = cuts(&s.cx(0));
                    if !found.is_empty() {
                        with_cuts += 1;
                    }
                    for (_, at, _, _) in &found {
                        cuts_n += 1;
                        let mut rng = Rng::new(3);
                        if matches!(
                            start(TargetRef::Point(*at), &s.cx(0), &mut rng),
                            State::Going { .. }
                        ) {
                            spots += 1;
                        }
                    }
                    eprintln!(
                        "level {level} mode {mode} side {side}: {} cuts",
                        found.len()
                    );
                }
            }
        }
        eprintln!(
            "{sides} sides with enemy routes, {with_cuts} with cuts; {cuts_n} cuts, {spots} with a spot"
        );
        assert!(with_cuts * 10 >= sides * 9, "{with_cuts} of {sides}");
        assert!(spots * 10 >= cuts_n * 8, "{spots} of {cuts_n}");
    }

    /// A spot its bot proved no way to is left for another fine one.
    #[test]
    fn a_spot_proved_no_way_to_is_left_for_another() {
        let mut s = field(Mode::TeamDeathmatch);
        let (_, at, _, _) = cuts(&s.cx(0))[0];
        let target = TargetRef::Point(at);
        let mut rng = Rng::new(4);
        let mut state = start(target, &s.cx(0), &mut rng);
        let State::Going { spot: first, .. } = state else {
            panic!("{state:?}")
        };
        s.no_way_to = vec![(first, s.view.clock + 5_000)];
        step(&mut state, target, &s.cx(0), &mut rng);
        let State::Going { spot, .. } = state else {
            panic!("{state:?}")
        };
        assert!(!s.cx(0).no_way_lately(spot), "{first:?} {spot:?}");
    }
}
