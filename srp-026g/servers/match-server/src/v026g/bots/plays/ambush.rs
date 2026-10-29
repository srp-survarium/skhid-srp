//! Play 1, battery ambush (`player_bots_behaviour.md` §3 play 1; §4 "an
//! enemy picking up at a spawner, seen"): an enemy is heading to a free
//! battery no friend covers; the bot hides 10-25 m from it, with a sight
//! line to the pickup point and out of sight of his approach, crouches,
//! stands still and looks at the pickup point. The reflex shoots once he
//! is seen and noticed (the play keeps fire allowed and never aims).
//!
//! - "Heading to it" is read from what the bot knows: a fresh track of
//!   the team's own senses (seen, heard or told, within `enemy_fresh_s`
//!   and since his spawn) within `enemy_max_m` of the battery, for whom
//!   it is the nearest free battery (tracks carry no heading).
//! - Only a battery the team could take itself (`Ctx::can_take`): one
//!   no way leads to is not worth waiting at (in the owner's match of
//!   2026-10-16, 56 ambushes at batteries nobody ever took).
//! - The bot goes only if it gets there first: its path to the battery
//!   is no longer than his (`first_ratio`); otherwise play 2 answers him
//!   once he carries it.
//! - One bot per battery (its slot). It gives up after a time drawn per
//!   run (30-40 s), so no fixed timer is learnt; once he has picked the
//!   battery up it watches a moment more (the reflex's window), then the
//!   commander picks again (interception, or the battery he drops).
//!
//! Left out: rule 1.4 of the design (hold fire during his pickup unless
//! his mates are close) is the reflex's business; this play keeps fire
//! allowed throughout.

use super::*;
use crate::v026g::bots::curve::{Consideration, Curve, Input};
use crate::v026g::bots::know::Source;
use crate::v026g::bots::tactics::SpotQuery;
use crate::v026g::bots::view::BatteryState;

pub const NUMBERS: &[(&str, f32)] = &[
    // An enemy this near a battery (it the nearest free one to him) heads
    // to it.
    ("enemy_max_m", 80.0),
    // Only an enemy the team sensed itself (seen, heard, told by a
    // teammate) this lately, and since his last spawn, heads anywhere: not
    // a radar's mark, a ghost of a dead one or an old place.
    ("enemy_fresh_s", 6.0),
    // A teammate this near the battery covers it.
    ("cover_m", 20.0),
    // The bot's path to the battery at most this times his.
    ("first_ratio", 1.0),
    // A walk is the ground distance times this (as `Ctx::path_dist`).
    ("path_factor", 1.25),
    ("hide_min_m", 10.0),
    ("hide_max_m", 25.0),
    ("spot_samples", 64.0),
    // Picked at random among this many of the best spots.
    ("spot_pick", 3.0),
    // The last metres to the spot are walked, not sprinted (a sprint is
    // heard).
    ("quiet_m", 15.0),
    ("reach_m", 1.5),
    ("give_up_min_s", 30.0),
    ("give_up_max_s", 40.0),
    // After his pickup, watched this long more.
    ("after_pickup_min_s", 2.0),
    ("after_pickup_max_s", 4.0),
];

pub fn tuning() -> PlayTuning {
    PlayTuning {
        rank: Rank::Objective,
        considerations: vec![
            considerations::near(150.0),
            considerations::personality(),
            considerations::holding(),
            considerations::not_behind_late(),
            considerations::health_above(0.3),
            considerations::modifier(Input::ObjectiveUrgency, 0.6),
            // One or two enemies by the battery are the point; a squad is
            // not.
            Consideration::new(
                Input::EnemiesNearTarget,
                0.0,
                4.0,
                Curve::Piecewise {
                    points: vec![(0.0, 1.0), (0.25, 1.0), (1.0, 0.0)],
                },
            ),
            considerations::cooldown(15.0),
        ],
        numbers: Default::default(),
    }
}

impl Ctx<'_> {
    /// The known enemy heading to the free battery at `at`: within
    /// `enemy_max_m` of it, it the nearest free battery to him; the
    /// nearest such.
    fn ambush_enemy(&self, at: Vec3) -> Option<Track> {
        let far = self.num(PlayKind::Ambush, "enemy_max_m");
        let fresh = (self.num(PlayKind::Ambush, "enemy_fresh_s") * 1000.0) as u32;
        let free: Vec<Vec3> = (0..self.view.batteries.len())
            .filter_map(|b| self.battery_lying(b))
            .collect();
        self.memory
            .tracks()
            .iter()
            .filter(|t| {
                matches!(t.source, Source::Seen | Source::Heard | Source::Told { .. })
                    && t.time + fresh >= self.now
                    && self
                        .view
                        .place(t.who)
                        .is_some_and(|p| p.alive && p.spawned <= t.time)
            })
            .filter(|t| {
                let d = t.at.distance_xz(at);
                d < far && free.iter().all(|other| t.at.distance_xz(*other) >= d)
            })
            .min_by(|a, b| a.at.distance_xz(at).total_cmp(&b.at.distance_xz(at)))
            .copied()
    }

    /// Whether an ambush at the free battery at `at` is worth it: an enemy
    /// heads to it, no friend covers it, and the bot is there first.
    fn ambush_worth(&self, at: Vec3) -> bool {
        let num = |name| self.num(PlayKind::Ambush, name);
        if self.friends_near(at, num("cover_m")) > 0 {
            return false;
        }
        let Some(enemy) = self.ambush_enemy(at) else {
            return false;
        };
        let his = enemy.at.distance_xz(at) * num("path_factor");
        self.path_dist(at) <= his * num("first_ratio")
    }
}

pub fn candidates(cx: &Ctx, out: &mut Vec<Candidate>) {
    if !cx.view.mode.has_batteries() || cx.me.carrying.is_some() {
        return;
    }
    for (battery, state) in cx.view.batteries.iter().enumerate() {
        let target = TargetRef::Battery(battery);
        // A running ambush keeps its candidate while the battery lies there
        // or an enemy has just taken it (the step ends it).
        let running = matches!(cx.bot.current, Some((PlayKind::Ambush, t, _)) if t == target);
        let worth = match state {
            BatteryState::Free(_) if running => true,
            BatteryState::Carried(carrier) if running => carrier
                .and_then(|p| cx.view.place(p))
                .is_none_or(|p| p.team != cx.me.team),
            _ if !running => cx
                .battery_lying(battery)
                .is_some_and(|at| cx.can_take(at) && cx.ambush_worth(at)),
            _ => false,
        };
        if worth {
            out.extend(cx.candidate(PlayKind::Ambush, target, Some(battery as u32)));
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    /// No spot hides it with a sight line: given up at the first step.
    NoSpot,
    Going {
        spot: Vec3,
        pickup: Vec3,
        until: u32,
    },
    Holding {
        spot: Vec3,
        pickup: Vec3,
        until: u32,
    },
    /// He took it: a moment more, for the reflex.
    Taken { pickup: Vec3, until: u32 },
}

pub fn start(target: TargetRef, cx: &Ctx, rng: &mut Rng) -> State {
    let num = |name| cx.num(PlayKind::Ambush, name);
    let Some(pickup) = cx.point_of(target) else {
        return State::NoSpot;
    };
    let enemy = cx.ambush_enemy(pickup);
    let query = SpotQuery {
        sight_to: Some(pickup),
        unseen_by: enemy.map(|t| t.at),
        side: cx.bot.side,
        samples: num("spot_samples") as usize,
        ..SpotQuery::ring(pickup, num("hide_min_m"), num("hide_max_m"))
    };
    let his = enemy.map_or(f32::INFINITY, |t| {
        t.at.distance_xz(pickup) * num("path_factor")
    });
    // Spots it reaches before he reaches the battery.
    let spots: Vec<Vec3> = cx
        .spots(&query, num("spot_pick") as usize, rng)
        .into_iter()
        .map(|spot| spot.at)
        .filter(|&at| cx.path_dist(at) <= his * num("first_ratio"))
        .collect();
    if spots.is_empty() {
        return State::NoSpot;
    }
    let spot = spots[rng.below(spots.len())];
    let give_up = rng.range(num("give_up_min_s"), num("give_up_max_s"));
    State::Going {
        spot,
        pickup,
        until: cx.now + (give_up * 1000.0) as u32,
    }
}

pub fn step(state: &mut State, target: TargetRef, cx: &Ctx, rng: &mut Rng) -> Step {
    let num = |name| cx.num(PlayKind::Ambush, name);
    let TargetRef::Battery(battery) = target else {
        return Step::Failed("no battery");
    };
    let watch = |pickup: Vec3| Intent::stay().crouched().looking(Look::At(pickup));
    let lying = cx.view.batteries.get(battery).copied();
    match state {
        State::NoSpot => Step::Failed("no hiding spot with a sight line"),
        State::Taken { pickup, until } => {
            if cx.now >= *until {
                return Step::Done("he took it: the others' turn");
            }
            if matches!(lying, Some(BatteryState::Free(_))) {
                return Step::Done("he dropped it");
            }
            Step::Continue(watch(*pickup))
        }
        State::Going { pickup, until, .. } | State::Holding { pickup, until, .. }
            if !matches!(lying, Some(BatteryState::Free(_))) =>
        {
            let (pickup, until) = (*pickup, *until);
            let ours = |p: Place| cx.view.place(p).is_some_and(|p| p.team == cx.me.team);
            match lying {
                Some(BatteryState::Carried(carrier)) if !carrier.is_some_and(ours) => {
                    let more = rng.range(num("after_pickup_min_s"), num("after_pickup_max_s"));
                    *state = State::Taken {
                        pickup,
                        until: (cx.now + (more * 1000.0) as u32).min(until),
                    };
                    Step::Continue(watch(pickup))
                }
                _ => Step::Done("the battery is ours or home"),
            }
        }
        State::Going { until, .. } | State::Holding { until, .. } if cx.now >= *until => {
            Step::Failed("nobody came")
        }
        State::Going {
            spot,
            pickup,
            until,
        } => {
            let (spot, pickup, until) = (*spot, *pickup, *until);
            let d = cx.me.feet.distance_xz(spot);
            if d <= num("reach_m") {
                *state = State::Holding {
                    spot,
                    pickup,
                    until,
                };
                return Step::Continue(watch(pickup));
            }
            let pace = if d < num("quiet_m") {
                Pace::Walk
            } else {
                cx.pace_toward(spot)
            };
            Step::Continue(Intent::to(spot, pace).within(num("reach_m") * 0.7))
        }
        State::Holding { spot, pickup, .. } => {
            // Pushed off the spot: back to it.
            if cx.me.feet.distance_xz(*spot) > num("reach_m") * 2.0 {
                return Step::Continue(
                    Intent::to(*spot, Pace::Walk)
                        .within(num("reach_m") * 0.7)
                        .looking(Look::At(*pickup)),
                );
            }
            Step::Continue(watch(*pickup))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::Scenario;
    use super::*;
    use crate::v026g::bots::know::Source;
    use crate::v026g::bots::team::{Asking, assign};
    use crate::v026g::bots::view::{Mode, still_eye};

    const BATTERY: Vec3 = Vec3::new(30.0, 0.0, 10.0);

    /// A battery at (30, 10) west of the wall, an enemy coming from the
    /// east at (60, 10), the bot at (8, 10).
    fn scene(known: bool) -> Scenario {
        let mut s = Scenario::field(Mode::Batteries);
        s.bot(0, 0, Vec3::new(8.0, 0.0, 10.0));
        s.battery(BatteryState::Free(BATTERY));
        s.enemy(
            1,
            1,
            Vec3::new(60.0, 0.0, 10.0),
            known.then_some(Source::Seen),
        );
        s
    }

    fn ambushes(s: &Scenario, place: Place) -> Vec<Candidate> {
        let mut out = Vec::new();
        candidates(&s.cx(place), &mut out);
        out
    }

    #[test]
    fn an_ambush_only_with_a_known_enemy_coming_no_friend_near_and_the_bot_first() {
        let s = scene(true);
        let found = ambushes(&s, 0);
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].target, TargetRef::Battery(0));
        assert_eq!(found[0].rank, Rank::Objective);
        // No enemy known: no ambush.
        assert!(ambushes(&scene(false), 0).is_empty());
        // He is nearer the battery than the bot: no ambush.
        let mut s = scene(false);
        s.enemy(1, 1, Vec3::new(36.0, 0.0, 30.0), Some(Source::Seen));
        assert!(ambushes(&s, 0).is_empty());
        // A friend covers it.
        let mut s = scene(true);
        s.human(2, 0, Vec3::new(28.0, 0.0, 14.0));
        assert!(ambushes(&s, 0).is_empty());
        // Carrying, or in Team Deathmatch: never.
        let mut s = scene(true);
        s.view.places[0].carrying = Some(0);
        assert!(ambushes(&s, 0).is_empty());
        let mut s = scene(true);
        s.view.mode = Mode::TeamDeathmatch;
        assert!(ambushes(&s, 0).is_empty());
    }

    #[test]
    fn it_hides_in_reach_of_the_pickup_point_out_of_his_sight_and_gives_up_in_30_to_40_s() {
        let mut s = scene(true);
        let start_clock = s.view.clock;
        let steps = s.run(0, PlayKind::Ambush, TargetRef::Battery(0), 400, 300, 3.5);
        let spot = s.view.places[0].feet;
        let d = spot.distance_xz(BATTERY);
        assert!((9.0..=26.0).contains(&d), "{spot:?} {d}");
        let eye = |feet| still_eye(feet, false);
        assert!(!s.tactics.sees(eye(Vec3::new(60.0, 0.0, 10.0)), eye(spot)));
        // Held crouched, still, looking at the pickup point.
        assert!(steps.iter().any(|step| matches!(
            step,
            Step::Continue(Intent { go: Go::Stay, crouch: true, look: Look::At(at), fire: true, .. })
                if *at == BATTERY
        )));
        assert_eq!(steps.last(), Some(&Step::Failed("nobody came")));
        let took = s.view.clock - start_clock;
        assert!((30_000..=40_600).contains(&took), "{took}");
    }

    #[test]
    fn his_pickup_ends_it_after_a_moment_and_a_friends_ends_it_at_once() {
        let mut s = scene(true);
        let mut rng = Rng::new(3);
        s.current = Some((PlayKind::Ambush, TargetRef::Battery(0), s.view.clock));
        let mut state = start(TargetRef::Battery(0), &s.cx(0), &mut rng);
        assert!(matches!(state, State::Going { .. }), "{state:?}");
        s.view.batteries[0] = BatteryState::Carried(Some(1));
        // Still a candidate while it runs.
        assert_eq!(ambushes(&s, 0).len(), 1);
        let first = step(&mut state, TargetRef::Battery(0), &s.cx(0), &mut rng);
        assert!(matches!(first, Step::Continue(_)), "{first:?}");
        assert!(matches!(state, State::Taken { .. }));
        s.view.clock += 5_000;
        assert!(matches!(
            step(&mut state, TargetRef::Battery(0), &s.cx(0), &mut rng),
            Step::Done(_)
        ));
        s.view.batteries[0] = BatteryState::Free(BATTERY);
        let mut state = start(TargetRef::Battery(0), &s.cx(0), &mut rng);
        assert!(matches!(state, State::Going { .. }), "{state:?}");
        s.human(2, 0, Vec3::new(28.0, 0.0, 14.0));
        s.view.batteries[0] = BatteryState::Carried(Some(2));
        assert!(matches!(
            step(&mut state, TargetRef::Battery(0), &s.cx(0), &mut rng),
            Step::Done(_)
        ));
    }

    /// Only a fresh track of the team's own senses heads anywhere: not
    /// the radar's, not one older than `enemy_fresh_s`, not one from
    /// before his last spawn (a ghost); and a battery no way leads to is
    /// not waited at.
    #[test]
    fn only_a_fresh_sensed_enemy_heads_to_a_battery_the_team_could_take() {
        use crate::v026g::bots::know::CheatKind;
        let s = scene(true);
        assert_eq!(ambushes(&s, 0).len(), 1);
        let mut s = scene(false);
        s.enemy(
            1,
            1,
            Vec3::new(60.0, 0.0, 10.0),
            Some(Source::Told { by: 2 }),
        );
        assert_eq!(ambushes(&s, 0).len(), 1, "a teammate's callout counts");
        let mut s = scene(false);
        s.enemy(
            1,
            1,
            Vec3::new(60.0, 0.0, 10.0),
            Some(Source::Cheat(CheatKind::Radar)),
        );
        assert!(ambushes(&s, 0).is_empty(), "the radar's mark");
        let mut s = scene(true);
        s.view.clock += 7_000;
        assert!(ambushes(&s, 0).is_empty(), "7 s old");
        let mut s = scene(true);
        s.view.clock += 1_000;
        s.view.places[1].spawned = s.view.clock;
        assert!(ambushes(&s, 0).is_empty(), "sensed before his spawn");
        let mut s = scene(true);
        s.no_way_to = vec![(BATTERY, s.view.clock + 5_000)];
        assert!(ambushes(&s, 0).is_empty(), "no way to it");
    }

    /// Far behind late in the match an ambush (a hold) is not worth it.
    #[test]
    fn a_team_far_behind_late_does_not_ambush() {
        let mut s = scene(true);
        s.time_left(100.0).points(0, 3);
        assert!(ambushes(&s, 0).is_empty());
        let mut s = scene(true);
        s.time_left(100.0).points(3, 3);
        assert_eq!(ambushes(&s, 0).len(), 1);
    }

    #[test]
    fn one_bot_per_ambush() {
        let mut s = scene(true);
        s.bot(2, 0, Vec3::new(8.0, 0.0, 14.0));
        let asking = vec![
            Asking {
                place: 0,
                candidates: ambushes(&s, 0),
                current: None,
                committed: false,
            },
            Asking {
                place: 2,
                candidates: ambushes(&s, 2),
                current: None,
                committed: false,
            },
        ];
        assert_eq!(asking[0].candidates.len(), 1);
        assert_eq!(asking[1].candidates.len(), 1);
        assert_eq!(asking[0].candidates[0].slot, asking[1].candidates[0].slot);
        let out = assign(asking, s.view.clock);
        assert_eq!(out.len(), 1, "{out:?}");
    }
}
