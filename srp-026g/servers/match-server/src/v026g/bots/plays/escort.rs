//! Play 3, escort (`player_bots_behaviour.md` §3 play 3; §4 "we killed an
//! enemy carrier: the nearest bot picks it up, one escorts"): our carrier,
//! bot or human, cannot shoot. One bot goes ahead of him on his way home
//! (toward our container), looking down that way; one stays behind him,
//! looking back.
//!
//! - Two slots per carrier: ahead and behind. The role is in the target:
//!   the carrier's place for the one ahead, the battery he carries for the
//!   one behind (both stand for where he is; a teammate's place is known,
//!   the HUD shows it).
//! - The one behind comes first (the owner's word after the first match
//!   against the real game, 2026-10-16: the bot that should have guarded
//!   his carry walked ahead to the base): any carrier of ours, human or
//!   bot, gets it at [`Rank::Alarm`], above every battery to collect, so
//!   one bot always takes it. It looks back at where the enemies are: the
//!   nearest the team knows within [`ENEMY_LOOK_M`] of him, else back
//!   along his way. The one ahead stays an extra of the objective band.
//! - The posts are drawn per run within their bands, so two escorts do not
//!   look alike; a post that is no goal (an enemy spawn area, off the
//!   floors) is pulled in toward him. When he is on floors that are no goal
//!   themselves (a zone's), it is pulled to the nearest dry floor that sees
//!   him ([`Ctx::escort_post`]), and a carrier no dry floor sees gets no
//!   escort.
//! - None for a carrier near home (`near_home_m`): it ends there, when he
//!   no longer carries, or after a time drawn per run.

use super::*;

/// An enemy the team knows this near the carrier is where the escort
/// behind him looks.
pub const ENEMY_LOOK_M: f32 = 40.0;

/// A post off the line is looked for this far from the carrier at most, metres (the look-back's near half: it still
/// sees him and his way).
const POST_SEARCH_M: f32 = 16.0;
/// ... at these distances from him, metres, in this many directions.
const POST_RINGS_M: [f32; 5] = [3.0, 5.0, 8.0, 12.0, POST_SEARCH_M];
const POST_DIRECTIONS: usize = 16;
/// The height of an eye over the feet for the post's sight line, metres (a standing body's).
const POST_EYE_M: f32 = 1.5;

pub const NUMBERS: &[(&str, f32)] = &[
    // The post ahead of him on his way home, metres, drawn per run.
    ("ahead_min_m", 10.0),
    ("ahead_max_m", 15.0),
    // The post behind him.
    ("behind_min_m", 6.0),
    ("behind_max_m", 10.0),
    // Where the escorts look: this far ahead of the post ahead, this far
    // behind him.
    ("look_ahead_m", 25.0),
    ("look_back_m", 25.0),
    // A carrier this near home needs no escort (the owner's word,
    // 2026-10-16: none for a battery picked up close to the base), and an
    // escort ends there.
    ("near_home_m", 25.0),
    // The post ahead stays this far short of home at least (half of it).
    ("home_m", 8.0),
    ("within_m", 3.0),
    // Farther than this from its post it hurries (`Ctx::pace_toward`);
    // nearer, it walks at his pace.
    ("catch_up_m", 15.0),
    ("give_up_min_s", 80.0),
    ("give_up_max_s", 110.0),
];

pub fn tuning() -> PlayTuning {
    PlayTuning {
        rank: Rank::Objective,
        considerations: vec![
            considerations::near(150.0),
            considerations::personality(),
            considerations::health_above(0.3),
            considerations::modifier(Input::ObjectiveUrgency, 0.6),
        ],
        numbers: Default::default(),
    }
}

impl Ctx<'_> {
    /// The teammates doing the scoring action now, as the HUD shows them: in the battery modes the carriers (none in
    /// deathmatch). The escort's post is asked about where such a body is ([`Self::escort_post`] takes a position), so a mode
    /// whose actors are others (bodies in a capture zone) answers here and the post code stays as it is.
    fn objective_actors(&self) -> impl Iterator<Item = &PlaceView> {
        self.view
            .team(self.me.team)
            .filter(|p| p.place != self.me.place && self.view.objective().is_actor(p))
    }

    /// Our carrier the target stands for, alive and carrying.
    fn escort_carrier(&self, target: TargetRef) -> Option<&PlaceView> {
        match target {
            TargetRef::Place(place) => self.objective_actors().find(|p| p.place == place),
            TargetRef::Battery(battery) => self.objective_actors().find(|p| p.carrying == Some(battery)),
            _ => None,
        }
    }

    /// `from` moved `m` metres along `dir` (a unit xz direction), pulled in
    /// toward `from` while that is no goal; when `from` itself is none (the
    /// carrier stands on a zone's floors), the dry floor that sees him
    /// nearest that post ([`ring_post`], kept by the carrier's floor,
    /// `HazardTuning::escort_dry_post`; off: his own feet, which no route leads
    /// to). `None`: no dry floor that a walk reaches sees him.
    fn escort_post(&self, from: Vec3, dir: (f32, f32), m: f32) -> Option<Vec3> {
        self.escort_post_by(self.tuning.hazard.escort_reach, from, dir, m)
    }

    /// [`Self::escort_post`] by the rule `reach` says (`HazardTuning::escort_reach`): a post on his line must be a floor the
    /// escort's walk reaches too ([`Self::reaches`]), else it is none and the next one in, then the carrier's own floor if
    /// that is one, then the dry floor on the zone's edge that sees him.
    fn escort_post_by(&self, reach: bool, from: Vec3, dir: (f32, f32), m: f32) -> Option<Vec3> {
        let wanted = |m: f32| Vec3::new(from.x + dir.0 * m, from.y, from.z + dir.1 * m);
        let goal = |p: Vec3| self.tactics.fine_goal(p, self.now, self.bot.side) && (!reach || self.reaches(p));
        let mut at = m;
        while at > 1.0 {
            if goal(wanted(at)) {
                return Some(wanted(at));
            }
            at *= 0.5;
        }
        if !self.tuning.hazard.escort_dry_post || goal(from) {
            return Some(from);
        }
        let target = wanted(m);
        self.memo.escort_post(self.tactics, from, target, self.now, self.bot.side, || {
            ring_post(self.tactics, from, target, self.now, self.bot.side)
        })
    }
}

impl Ctx<'_> {
    /// Whether a walk gets to `point` over floors no zone on all match marks (`Tactics::past_fixed_zones`): a dry island or a
    /// stand inside such zones is a floor, and no goal a route leads to.
    fn reaches(&self, point: Vec3) -> bool {
        let grid = &self.tactics.world.grid;
        grid.locate(point, 2.0).is_some_and(|node| self.tactics.past_fixed_zones(node))
    }
}

/// The dry floor nearest `target` within [`POST_SEARCH_M`] of `from` that a walk reaches (`walkable`, past the zones on
/// all match: a dry island in a swamp is none) and from which the carrier at `from` is seen.
fn ring_post(tactics: &Tactics, from: Vec3, target: Vec3, now: u32, side: Option<i32>) -> Option<Vec3> {
    let grid = &tactics.world.grid;
    let eye = Vec3::new(0.0, POST_EYE_M, 0.0);
    POST_RINGS_M
        .iter()
        .flat_map(|&r| {
            (0..POST_DIRECTIONS).map(move |k| {
                let a = k as f32 * std::f32::consts::TAU / POST_DIRECTIONS as f32;
                Vec3::new(from.x + r * a.cos(), from.y, from.z + r * a.sin())
            })
        })
        .filter_map(|p| grid.locate(p, 2.0))
        .filter(|&node| {
            let p = grid.position(node);
            tactics.fine_goal(p, now, side)
                && tactics.past_fixed_zones(node)
                && (p.y - from.y).abs() < 3.0
                && tactics.sees(p + eye, from + eye)
        })
        .map(|node| grid.position(node))
        .min_by(|a, b| a.distance_xz(target).total_cmp(&b.distance_xz(target)))
}

pub fn candidates(cx: &Ctx, out: &mut Vec<Candidate>) {
    if !cx.view.objective().has_homes() || cx.view.objective().is_actor(cx.me) {
        return;
    }
    let Some(home) = cx.view.container_of(cx.me.team).map(|c| c.at) else {
        return;
    };
    let home_m = cx.num(PlayKind::Escort, "near_home_m");
    // A bot that stands in a pocket of zones on all match walks to no post outside it.
    let reach = cx.tuning.hazard.escort_reach;
    let stuck_in_pocket = reach && !cx.reaches(cx.me.feet);
    for carrier in cx.objective_actors() {
        let Some(battery) = carrier.carrying else {
            continue;
        };
        if carrier.feet.distance_xz(home) < home_m {
            continue;
        }
        // A carrier who gave up getting out of his pocket goes nowhere: no
        // escort walks in to him (`escape.rs`).
        if cx.teammate_trapped(carrier.place).is_some_and(|t| t.gave_up) {
            continue;
        }
        // Nor to one on a zone's floors that no dry floor sees: it would end at once, "no way to its goal". Each
        // candidate asks its own post: ahead of him toward home, behind him away from it.
        let left = carrier.feet.distance_xz(home).max(1.0);
        let ahead = ((home.x - carrier.feet.x) / left, (home.z - carrier.feet.z) / left);
        let post_ok = |dir: (f32, f32), numbers: &str| {
            let m = cx.num(PlayKind::Escort, numbers);
            let ok = !stuck_in_pocket && cx.escort_post(carrier.feet, dir, m).is_some();
            // What the old rule would have offered and this one refuses, for the bench's tally.
            #[cfg(test)]
            if reach && !ok && cx.escort_post_by(false, carrier.feet, dir, m).is_some() {
                crate::v026g::bots::hazards::damage_zone_fixes::asked_by(cx.me.team, || {
                    crate::v026g::bots::hazards::damage_zone_fixes::count_once(
                        crate::v026g::bots::hazards::damage_zone_fixes::Ev::EscortUnreachable,
                        carrier.place as u32 * 64 + cx.me.place as u32,
                        cx.now,
                    );
                });
            }
            ok
        };
        let key = carrier.place as u32 * 2;
        if post_ok(ahead, "ahead_min_m") {
            out.extend(cx.candidate(PlayKind::Escort, TargetRef::Place(carrier.place), Some(key)));
        }
        // Behind him: above the batteries to collect.
        if post_ok((-ahead.0, -ahead.1), "behind_min_m")
            && let Some(mut behind) =
                cx.candidate(PlayKind::Escort, TargetRef::Battery(battery), Some(key + 1))
        {
            behind.rank = Rank::Alarm;
            out.push(behind);
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    /// Ahead of him on his way home.
    Ahead { m: f32, until: u32 },
    /// Behind him, looking back.
    Behind { m: f32, until: u32 },
}

pub fn start(target: TargetRef, cx: &Ctx, rng: &mut Rng) -> State {
    let num = |name| cx.num(PlayKind::Escort, name);
    #[cfg(test)]
    count_post(cx, target);
    let until = cx.now + (rng.range(num("give_up_min_s"), num("give_up_max_s")) * 1000.0) as u32;
    match target {
        TargetRef::Battery(_) => State::Behind {
            m: rng.range(num("behind_min_m"), num("behind_max_m")),
            until,
        },
        _ => State::Ahead {
            m: rng.range(num("ahead_min_m"), num("ahead_max_m")),
            until,
        },
    }
}

/// The post a play begun gets, for the bench's tally (`hazards::damage_zone_fixes`): on the line to the post, pulled to a dry
/// floor that sees the carrier, or none.
#[cfg(test)]
fn count_post(cx: &Ctx, target: TargetRef) {
    use crate::v026g::bots::hazards::damage_zone_fixes::{Ev, count};
    let (Some(carrier), Some(home)) = (cx.escort_carrier(target), cx.view.container_of(cx.me.team).map(|c| c.at)) else {
        return;
    };
    let him = carrier.feet;
    let left = him.distance_xz(home).max(1.0);
    let (dx, dz) = ((home.x - him.x) / left, (home.z - him.z) / left);
    let dir = if matches!(target, TargetRef::Battery(_)) { (-dx, -dz) } else { (dx, dz) };
    let m = cx.num(PlayKind::Escort, if matches!(target, TargetRef::Battery(_)) { "behind_min_m" } else { "ahead_min_m" });
    crate::v026g::bots::hazards::damage_zone_fixes::asked_by(cx.me.team, || match cx.escort_post(him, dir, m) {
        None => count(Ev::PostNone),
        Some(post) if cx.tactics.fine_goal(post, cx.now, cx.bot.side) && cx.tactics.fine_goal(him, cx.now, cx.bot.side) => {
            count(Ev::PostLine)
        }
        Some(post) if post.distance_xz(him) > 0.01 => count(Ev::PostPulled),
        Some(_) => count(Ev::PostLine),
    })
}

pub fn step(state: &mut State, target: TargetRef, cx: &Ctx, _rng: &mut Rng) -> Step {
    let num = |name| cx.num(PlayKind::Escort, name);
    let Some(carrier) = cx.escort_carrier(target) else {
        return Step::Done("he is home or down");
    };
    let Some(home) = cx.view.container_of(cx.me.team).map(|c| c.at) else {
        return Step::Failed("no container of ours");
    };
    let him = carrier.feet;
    let left = him.distance_xz(home);
    if left < num("near_home_m") {
        return Step::Done("he is home");
    }
    let (State::Ahead { until, .. } | State::Behind { until, .. }) = *state;
    if cx.now >= until {
        return Step::Done("escorted long enough");
    }
    let dir = ((home.x - him.x) / left, (home.z - him.z) / left);
    let (post, look) = match *state {
        State::Ahead { m, .. } => {
            let Some(post) = cx.escort_post(him, dir, m.min(left - num("home_m") * 0.5)) else {
                return Step::Failed("no dry floor sees him");
            };
            let ahead = num("look_ahead_m");
            (
                post,
                Vec3::new(post.x + dir.0 * ahead, post.y, post.z + dir.1 * ahead),
            )
        }
        State::Behind { m, .. } => {
            let back = (-dir.0, -dir.1);
            let behind = num("look_back_m");
            // At the nearest enemy the team knows near him, else back
            // along his way.
            let enemy = cx
                .memory
                .tracks()
                .iter()
                .filter(|t| t.at.distance_xz(him) < ENEMY_LOOK_M)
                .min_by(|a, b| a.at.distance_xz(him).total_cmp(&b.at.distance_xz(him)))
                .map(|t| t.at);
            let Some(post) = cx.escort_post(him, back, m) else {
                return Step::Failed("no dry floor sees him");
            };
            (
                post,
                enemy.unwrap_or(Vec3::new(
                    him.x + back.0 * behind,
                    him.y,
                    him.z + back.1 * behind,
                )),
            )
        }
    };
    let pace = if cx.me.feet.distance_xz(post) > num("catch_up_m") {
        cx.pace_toward(post)
    } else {
        Pace::Walk
    };
    Step::Continue(
        Intent::to(post, pace)
            .within(num("within_m"))
            .looking(Look::At(look)),
    )
}

#[cfg(test)]
mod tests {
    use super::super::tests::Scenario;
    use super::*;
    use crate::v026g::bots::team::{Asking, assign};
    use crate::v026g::bots::view::{BatteryState, Mode};

    const CARRIER: Vec3 = Vec3::new(45.0, 0.0, 33.0);

    /// Our human (place 1) carries battery 0 at (50, 32), home is (12, 30);
    /// bots 0 and 2 stand near him.
    fn scene() -> Scenario {
        let mut s = Scenario::field(Mode::Batteries);
        s.bot(0, 0, Vec3::new(50.0, 0.0, 35.0));
        s.human(1, 0, CARRIER).carrying = Some(0);
        s.bot(2, 0, Vec3::new(47.0, 0.0, 38.0));
        s.battery(BatteryState::Carried(Some(1)));
        s
    }

    fn escorts(s: &Scenario, place: Place) -> Vec<Candidate> {
        let mut out = Vec::new();
        candidates(&s.cx(place), &mut out);
        out
    }

    #[test]
    fn a_carrier_of_ours_away_from_home_gets_an_escort_ahead_and_one_behind() {
        let s = scene();
        let found = escorts(&s, 0);
        let targets: Vec<TargetRef> = found.iter().map(|c| c.target).collect();
        assert_eq!(targets, [TargetRef::Place(1), TargetRef::Battery(0)]);
        // Behind him first: above the batteries; ahead an extra.
        let ranks: Vec<Rank> = found.iter().map(|c| c.rank).collect();
        assert_eq!(ranks, [Rank::Objective, Rank::Alarm]);
        // Nobody carries: none.
        let mut s = scene();
        s.view.places[1].carrying = None;
        assert!(escorts(&s, 0).is_empty());
        // The carrier escorts nobody, himself least.
        let mut s = scene();
        s.view.places[1].carrying = None;
        s.view.places[2].carrying = Some(0);
        assert!(escorts(&s, 2).is_empty());
        assert_eq!(escorts(&s, 0).len(), 2);
        // Nearly home: none.
        let mut s = scene();
        s.view.places[1].feet = Vec3::new(16.0, 0.0, 30.0);
        assert!(escorts(&s, 0).is_empty());
        // An enemy carrier is not ours to escort.
        let mut s = scene();
        s.view.places[1].team = 1;
        assert!(escorts(&s, 0).is_empty());
    }

    #[test]
    fn two_bots_take_the_two_posts() {
        let s = scene();
        let asking = [0, 2]
            .into_iter()
            .map(|place| Asking {
                place,
                candidates: escorts(&s, place),
                current: None,
                committed: false,
            })
            .collect();
        let out = assign(asking, s.view.clock);
        let mut targets: Vec<String> = out
            .iter()
            .map(|(_, a, _)| format!("{:?}", a.target))
            .collect();
        targets.sort();
        assert_eq!(targets, ["Battery(0)", "Place(1)"]);
    }

    /// The one behind him comes before a battery lying near the bot: the
    /// best of all its plays.
    #[test]
    fn behind_a_carrier_comes_before_a_battery_to_collect() {
        let mut s = scene();
        s.battery(BatteryState::Free(Vec3::new(48.0, 0.0, 36.0)));
        let best = &s.candidates(0)[0];
        assert_eq!(
            (best.kind, best.target, best.rank),
            (PlayKind::Escort, TargetRef::Battery(0), Rank::Alarm)
        );
    }

    /// Behind him it looks at the nearest enemy the team knows near him,
    /// not along his way.
    #[test]
    fn behind_looks_at_a_known_enemy_near_him() {
        let enemy = Vec3::new(52.0, 0.0, 20.0);
        let mut s = scene();
        s.enemy(5, 1, enemy, Some(crate::v026g::bots::know::Source::Seen));
        let steps = s.run(2, PlayKind::Escort, TargetRef::Battery(0), 1, 300, 3.5);
        let Step::Continue(Intent {
            look: Look::At(look),
            ..
        }) = steps[0]
        else {
            panic!("{:?}", steps[0]);
        };
        assert!(look.distance_xz(enemy) < 0.5, "{look:?}");
    }

    #[test]
    fn ahead_goes_on_toward_home_and_behind_looks_back_and_both_end() {
        let mut s = scene();
        let steps = s.run(0, PlayKind::Escort, TargetRef::Place(1), 30, 300, 3.5);
        let Step::Continue(Intent {
            go: Go::To { at: post, .. },
            look: Look::At(look),
            ..
        }) = steps[0]
        else {
            panic!("{:?}", steps[0]);
        };
        assert!(
            post.x < CARRIER.x - 9.0 && look.x < post.x,
            "{post:?} {look:?}"
        );
        let mut s = scene();
        let steps = s.run(2, PlayKind::Escort, TargetRef::Battery(0), 30, 300, 3.5);
        let Step::Continue(Intent {
            go: Go::To { at: post, .. },
            look: Look::At(look),
            ..
        }) = steps[0]
        else {
            panic!("{:?}", steps[0]);
        };
        assert!(
            post.x > CARRIER.x + 5.0 && look.x > post.x,
            "{post:?} {look:?}"
        );
        assert!(steps.iter().all(|s| matches!(s, Step::Continue(_))));
        // It ends when he is home, after its drawn time otherwise.
        let mut s = scene();
        s.view.places[1].feet = Vec3::new(16.0, 0.0, 30.0);
        let steps = s.run(0, PlayKind::Escort, TargetRef::Place(1), 3, 300, 3.5);
        assert_eq!(steps.last(), Some(&Step::Done("he is home")));
        let mut s = scene();
        let start_clock = s.view.clock;
        let steps = s.run(2, PlayKind::Escort, TargetRef::Battery(0), 200, 1_000, 3.5);
        assert_eq!(steps.last(), Some(&Step::Done("escorted long enough")));
        let took = s.view.clock - start_clock;
        assert!((80_000..=111_000).contains(&took), "{took}");
    }

    /// The field with a harsh zone (always on) of radius `r` round the carrier's feet, a sphere on the floor.
    fn carrier_in_a_zone(r: f32) -> Scenario {
        use mapdata::{Rotation, Shape, ShapeKind, Zone, ZoneKind};
        let mut s = scene();
        let world = crate::v026g::bots::tactics::tests::field_with(|map| {
            map.zones.push(Zone {
                kind: ZoneKind::Damage,
                name: "toxic".into(),
                team: -1,
                position: CARRIER,
                damage_type: "toxic".into(),
                damage: None,
                shapes: vec![Shape {
                    kind: ShapeKind::Sphere,
                    center: Vec3::new(CARRIER.x, 1.0, CARRIER.z),
                    rotation: Rotation::IDENTITY,
                    half: Vec3::new(r, r, r),
                }],
            });
        });
        s.tactics = crate::v026g::bots::tactics::Tactics::new(world, 1.0);
        s.tuning.hazard.escort_dry_post = true;
        s
    }

    /// A carrier on the floors of a zone: the post is the dry floor that sees him nearest the post on his line
    /// (before, his own feet, which no route leads to: "no way to its goal" 18 and 54 times in a match); one a zone
    /// covers all round within the search gets no escort.
    #[test]
    fn a_post_for_a_carrier_in_a_zone_is_the_nearest_dry_floor_that_sees_him() {
        let s = carrier_in_a_zone(3.0);
        let cx = s.cx(0);
        assert!(!s.tactics.fine_goal(CARRIER, cx.now, cx.bot.side), "he stands in the zone");
        let back = (1.0, 0.0);
        // A post 2 m along his line is in the zone too: the nearest dry floor that sees him, near it.
        let post = cx.escort_post(CARRIER, back, 2.0).expect("a post");
        assert!(s.tactics.fine_goal(post, cx.now, cx.bot.side), "{post:?}");
        assert!(post.distance_xz(CARRIER) > 3.5 && post.distance_xz(CARRIER) < 7.0, "{post:?}");
        // One on the line outside the zone is the line's.
        let far = cx.escort_post(CARRIER, back, 8.0).expect("a post");
        assert!(far.distance_xz(Vec3::new(CARRIER.x + 8.0, 0.0, CARRIER.z)) < 0.01, "{far:?}");
        assert_eq!(escorts(&s, 0).len(), 2);
        // Off: the carrier's own feet, as it was.
        let mut off = carrier_in_a_zone(3.0);
        off.tuning.hazard.escort_dry_post = false;
        assert_eq!(off.cx(0).escort_post(CARRIER, back, 2.0), Some(CARRIER));
        // A zone wider than the search: no dry floor sees him, so no escort is offered.
        let mut wide = carrier_in_a_zone(POST_SEARCH_M + 6.0);
        assert_eq!(wide.cx(0).escort_post(CARRIER, back, 8.0), None);
        assert!(escorts(&wide, 0).is_empty());
        let steps = wide.run(0, PlayKind::Escort, TargetRef::Place(1), 1, 300, 3.5);
        assert_eq!(steps[0], Step::Failed("no dry floor sees him"));
    }

    /// A dry island ringed by zones on all match (4 x 4 m of floor in a ring of boxes): the carrier stands on it.
    fn carrier_on_an_island() -> Scenario {
        use mapdata::{Rotation, Shape, ShapeKind, Zone, ZoneKind};
        let island = Vec3::new(50.0, 0.0, 10.0);
        let mut s = scene();
        let world = crate::v026g::bots::tactics::tests::field_with(|map| {
            for (x, z, hx, hz) in [(46.0, 10.0, 1.0, 5.0), (54.0, 10.0, 1.0, 5.0), (50.0, 6.0, 5.0, 1.0), (50.0, 14.0, 5.0, 1.0)] {
                map.zones.push(Zone {
                    kind: ZoneKind::Damage,
                    name: format!("ring {x} {z}"),
                    team: -1,
                    position: Vec3::new(x, 0.0, z),
                    damage_type: "toxic".into(),
                    damage: None,
                    shapes: vec![Shape {
                        kind: ShapeKind::Box,
                        center: Vec3::new(x, 1.0, z),
                        rotation: Rotation::IDENTITY,
                        half: Vec3::new(hx, 1.0, hz),
                    }],
                });
            }
        });
        s.tactics = crate::v026g::bots::tactics::Tactics::new(world, 1.0);
        s.tuning.hazard.escort_dry_post = true;
        s.view.places[1].feet = island;
        s
    }

    /// A carrier on a dry island inside zones on all match: a post on his line that is on the island is a goal no walk reaches
    /// (the bench's "no way to its goal"), so with `escort_reach` the post is the dry floor outside the ring that sees him
    /// (the zone's edge); off, the island's, as it was. A bot standing on the island itself walks to no post outside it.
    #[test]
    fn a_post_for_a_carrier_on_a_dry_island_is_outside_the_zones_when_the_escort_must_reach_it() {
        let island = Vec3::new(50.0, 0.0, 10.0);
        let mut on = carrier_on_an_island();
        on.tuning.hazard.escort_reach = true;
        let mut off = carrier_on_an_island();
        off.tuning.hazard.escort_reach = false;
        // Behind him, 2 m east along the island: on it.
        let (cx_on, cx_off) = (on.cx(0), off.cx(0));
        let old = cx_off.escort_post(island, (1.0, 0.0), 1.5).expect("a post");
        assert!(old.distance_xz(island) < 3.0, "the island's: {old:?}");
        assert!(!cx_off.reaches(old), "and no walk reaches it");
        let new = cx_on.escort_post(island, (1.0, 0.0), 1.5).expect("a post outside");
        assert!(cx_on.reaches(new) && cx_on.tactics.fine_goal(new, cx_on.now, cx_on.bot.side), "{new:?}");
        assert!(new.distance_xz(island) > 4.0, "outside the ring: {new:?}");
        // The escort offered by the old rule (bot 0 stands outside) is offered still, with the new post; one standing on
        // the island is offered none.
        assert_eq!(escorts(&on, 0).len(), 2);
        let mut inside = carrier_on_an_island();
        inside.tuning.hazard.escort_reach = true;
        inside.view.places[0].feet = Vec3::new(49.0, 0.0, 9.0);
        assert!(escorts(&inside, 0).is_empty(), "a bot on the island walks to no post outside it");
        let mut inside_off = carrier_on_an_island();
        inside_off.tuning.hazard.escort_reach = false;
        inside_off.view.places[0].feet = Vec3::new(49.0, 0.0, 9.0);
        assert_eq!(escorts(&inside_off, 0).len(), 2);
    }
}
