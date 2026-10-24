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
//!   floors) is pulled in toward him.
//! - None for a carrier near home (`near_home_m`): it ends there, when he
//!   no longer carries, or after a time drawn per run.

use super::*;

/// An enemy the team knows this near the carrier is where the escort
/// behind him looks.
pub const ENEMY_LOOK_M: f32 = 40.0;

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
    /// Our carrier the target stands for, alive and carrying.
    fn escort_carrier(&self, target: TargetRef) -> Option<&PlaceView> {
        let mate = |p: &&PlaceView| p.place != self.me.place && p.carrying.is_some();
        match target {
            TargetRef::Place(place) => self
                .view
                .team(self.me.team)
                .filter(mate)
                .find(|p| p.place == place),
            TargetRef::Battery(battery) => self
                .view
                .team(self.me.team)
                .filter(mate)
                .find(|p| p.carrying == Some(battery)),
            _ => None,
        }
    }

    /// `from` moved `m` metres along `dir` (a unit xz direction), pulled in
    /// toward `from` while that is no goal.
    fn escort_post(&self, from: Vec3, dir: (f32, f32), m: f32) -> Vec3 {
        let mut m = m;
        while m > 1.0 {
            let at = Vec3::new(from.x + dir.0 * m, from.y, from.z + dir.1 * m);
            if self.tactics.fine_goal(at, self.now, self.bot.side) {
                return at;
            }
            m *= 0.5;
        }
        from
    }
}

pub fn candidates(cx: &Ctx, out: &mut Vec<Candidate>) {
    if !cx.view.mode.has_batteries() || cx.me.carrying.is_some() {
        return;
    }
    let Some(home) = cx.view.container_of(cx.me.team).map(|c| c.at) else {
        return;
    };
    let home_m = cx.num(PlayKind::Escort, "near_home_m");
    for carrier in cx.view.team(cx.me.team) {
        let Some(battery) = carrier.carrying else {
            continue;
        };
        if carrier.place == cx.me.place || carrier.feet.distance_xz(home) < home_m {
            continue;
        }
        // A carrier who gave up getting out of his pocket goes nowhere: no
        // escort walks in to him (`escape.rs`).
        if cx.teammate_trapped(carrier.place).is_some_and(|t| t.gave_up) {
            continue;
        }
        let key = carrier.place as u32 * 2;
        out.extend(cx.candidate(PlayKind::Escort, TargetRef::Place(carrier.place), Some(key)));
        // Behind him: above the batteries to collect.
        if let Some(mut behind) =
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
            let post = cx.escort_post(him, dir, m.min(left - num("home_m") * 0.5));
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
            (
                cx.escort_post(him, back, m),
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
}
