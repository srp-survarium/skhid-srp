//! Play 27: Resupply. A bot that is running short of rounds walks to its
//! team's resupply box when it is calm and near, and holds USE on it.
//!
//! The client's rule (`fighting-record/fact-b-resupply.md`, PROVEN in the
//! exe: `use_update` 0x2F8120, `inventory::resupply` 0x2FDC10): USE pressed
//! on an edge (up the tick before) while the 2 m ray from the eye along the
//! view meets the zone's box, and held until `resupply_time_in_ms` (3000 ms
//! in every project) have passed with the ray still on the box. Then each
//! ammo stack is topped up to the kit's count from the profile's storage,
//! the magazine counted in; the magazine itself and the health are not
//! touched. A full kit or an empty storage allows no use at all.
//!
//! - **When:** the rounds it holds (magazine and stack) are at or under
//!   `rounds_share` of the kit's count for a life (dry always counts), its
//!   kit's storage has something to give, it carries no battery, no enemy
//!   is seen now or known within `calm_m`, and its team's box is within
//!   `zone_m` by the way. Humans do the same: of 92 holds near a box in 39
//!   recordings, 89 had walked at most 47 m in the 15 s before, the nearest
//!   living enemy was 58 m away at the median and within 30 m in 10 of 89.
//! - **Where:** a stand of the box's own list (`UseStands::of_zone`, the
//!   floors from which the game's ray meets the box), the nearest by the
//!   way. Retreat walked to the zone's pivot, 1-28 m from the box, and held
//!   USE only with its chest inside the box: it never resupplied.
//! - **How:** at the stand the view turns onto the box, then USE is held
//!   for `hold_ms` (the client's 3000 ms and a margin), standing, fire held.
//!   The ray lost (a push, a turn) starts the hold again. An enemy seen ends
//!   it. At the end the bot's kit gives its stack the rounds
//!   (`Intent::resupplied`); the reload habit loads the magazine.
//! - **Not again:** `again_s` after a run; a stand it stood at for
//!   `stand_still_s` with no press possible is left out.

use super::*;
use crate::v026g::bots::curve::{Consideration, Curve, Input};
use crate::v026g::drive::key;
use mapdata::{UseStand, ZoneKind};

pub const NUMBERS: &[(&str, f32)] = &[
    // The play's switch, **off by default** (merged switched off; a train
    // turns it on): 1 on, 0 off. Off, nothing of the play is offered and
    // Retreat's wounded fallback walks and holds as it did before the play
    // (the zone's pivot, USE held with the chest inside the box).
    ("on", 0.0),
    // Offered at or under this share of the kit's count for a life; a dry
    // bot always. GUESS (no human number: recordings have no ammo); swept
    // 0.25 / 0.40 / 0.60 on the bench.
    ("rounds_share", 0.4),
    // At or under this share it is nearly dry: the play then ranks with the
    // objectives, so a bot on its way to one goes to the box first (the owner:
    // bots keep doing their task, but not without a gun). Above it the play
    // ranks with the fights and a bot with an objective in hand keeps it.
    ("dry_share", 0.1),
    // The way to the box at most, metres (humans: p90 47 m in the 15 s
    // before a hold).
    ("zone_m", 45.0),
    // ... for a nearly dry bot. The cost model said more (a dry bot cannot
    // fight, a respawn is 8 s in batteries and 6.4 s in TDM, so the walk
    // pays up to 3.5 x 5 m/s plus how far it has come from its spawn, about
    // 80 m: a GUESS, no human cap, humans are never dry;
    // `batteries-suicide-vs-resupply.md`), but the bench did not bear it
    // out: 600 s, 16 maps, tier 9-10, 3 seeds, 7,250 lives a variant: dry
    // seconds 4155 off, 2993 at 45 m (-28 %), 4008 at 80 m (-4 %); deaths
    // while dry 70, 62, 73. 80 sent more bots (140 runs against 118) and
    // saved no dry time, so the default is `zone_m`; the number stays the
    // knob.
    ("zone_m_dry", 45.0),
    // No enemy known this near, metres (humans: 10 of 89 holds had one).
    ("calm_m", 30.0),
    // USE held this long once the view is on the box, ms: the client's 3000
    // plus a margin of 4 ticks of 50 ms.
    ("hold_ms", 3200.0),
    // Within this of the stand's floor it counts as there, metres.
    ("stand_at_m", 0.6),
    // At a stand this long with no press possible, the next stand, s.
    ("stand_still_s", 3.0),
    // The view not on the box this long after arriving: pressed anyway, ms.
    ("settle_ms", 1500.0),
    ("give_up_s", 20.0),
    // Not offered again this soon after a run, s.
    ("again_s", 60.0),
];

pub fn tuning() -> PlayTuning {
    PlayTuning {
        rank: Rank::Fight,
        considerations: vec![
            // The fewer rounds, the sooner; dry first.
            Consideration::new(
                Input::Ammo,
                0.0,
                1.0,
                Curve::Piecewise {
                    points: vec![(0.0, 1.0), (0.5, 0.6), (1.0, 0.05)],
                },
            ),
            considerations::near(90.0),
            considerations::cooldown(60.0),
        ],
        numbers: Default::default(),
    }
}

fn num(cx: &Ctx, name: &str) -> f32 {
    cx.num(PlayKind::Resupply, name)
}

/// Whether its rounds are low enough for the play to be offered.
fn short(cx: &Ctx) -> bool {
    let ammo = cx.bot.ammo;
    ammo.missing() > 0 && (ammo.have == 0 || ammo.share() <= num(cx, "rounds_share"))
}

/// An enemy seen now, or one it knows of within `calm_m`.
fn unsafe_here(cx: &Ctx) -> bool {
    let near = num(cx, "calm_m");
    !cx.memory.all_seen(cx.now).is_empty()
        || cx
            .memory
            .tracks()
            .iter()
            .any(|t| t.at.distance_xz(cx.me.feet) < near)
}

/// The bot's team's resupply zones that have a stand, nearest stand first:
/// the zone's index, the stand and the way to it, metres.
fn zones(cx: &Ctx) -> Vec<(usize, UseStand, f32)> {
    let Some(side) = cx.bot.side else {
        return Vec::new();
    };
    let mut out: Vec<(usize, UseStand, f32)> = cx
        .tactics
        .world
        .map
        .zones
        .iter()
        .enumerate()
        .filter(|(_, z)| z.kind == ZoneKind::Resupply && z.team == side)
        .filter_map(|(index, _)| {
            let stand = cx.nearest_zone_stand(index, &[])?;
            Some((index, stand, cx.path_dist(stand.feet)))
        })
        .collect();
    out.sort_by(|a, b| a.2.total_cmp(&b.2));
    out
}

pub fn candidates(cx: &Ctx, out: &mut Vec<Candidate>) {
    if num(cx, "on") <= 0.0 || cx.me.carrying.is_some() {
        return;
    }
    let running = matches!(cx.bot.current, Some((PlayKind::Resupply, _, _)));
    if !running && (!short(cx) || unsafe_here(cx)) {
        return;
    }
    if running && cx.bot.ammo.missing() == 0 {
        return;
    }
    let Some((index, _, way)) = zones(cx).into_iter().next() else {
        return;
    };
    let nearly_dry = cx.bot.ammo.share() <= num(cx, "dry_share");
    let reach = if nearly_dry { num(cx, "zone_m_dry") } else { num(cx, "zone_m") };
    if !running && way > reach {
        return;
    }
    if let Some(shape) = cx.tactics.world.map.zones[index].shapes.first() {
        out.extend(
            cx.candidate(PlayKind::Resupply, TargetRef::Point(shape.center), None)
                .map(|c| Candidate {
                    rank: if nearly_dry { Rank::Objective } else { c.rank },
                    ..c
                }),
        );
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    /// Walking to a stand of the zone's box.
    Going {
        zone: usize,
        stand: UseStand,
        /// Since when it stands at the stand with no press possible.
        still_since: Option<u32>,
        /// Stands it left (no press from them).
        passed: Vec<Vec3>,
    },
    /// At the stand: the view turns onto the box, then USE is held from
    /// `since`.
    Holding {
        zone: usize,
        stand: UseStand,
        arrived: u32,
        since: Option<u32>,
        passed: Vec<Vec3>,
    },
    /// The top-up was given; one step to let go of the key.
    Finished,
}

pub fn start(target: TargetRef, cx: &Ctx, _rng: &mut Rng) -> State {
    let zone = zones(cx)
        .into_iter()
        .find(|(index, ..)| {
            cx.point_of(target).is_some_and(|at| {
                cx.tactics.world.map.zones[*index]
                    .shapes
                    .first()
                    .is_some_and(|shape| (shape.center - at).length() < 0.01)
            })
        })
        .or_else(|| zones(cx).into_iter().next());
    match zone {
        Some((zone, stand, _)) => State::Going {
            zone,
            stand,
            still_since: None,
            passed: Vec::new(),
        },
        None => State::Finished,
    }
}

pub fn step(state: &mut State, _target: TargetRef, cx: &Ctx, _rng: &mut Rng) -> Step {
    if matches!(state, State::Finished) {
        return Step::Done("topped up");
    }
    if cx.me.carrying.is_some() {
        return Step::Done("carrying a battery");
    }
    if cx.bot.ammo.missing() == 0 {
        return Step::Done("nothing to refill");
    }
    if !cx.memory.all_seen(cx.now).is_empty() {
        return Step::Done("an enemy came");
    }
    if let Some((_, _, started)) = cx.bot.current
        && cx.now > started + (num(cx, "give_up_s") * 1000.0) as u32
    {
        return Step::Failed("took too long");
    }
    match state {
        State::Going {
            zone,
            stand,
            still_since,
            passed,
        } => {
            let at = stand.feet;
            // The game's ray decides, wherever the bot is (a stand has a
            // margin in height only): on the box, it holds.
            if cx.zone_press(cx.me.eye(), *zone).is_some() {
                let (zone, stand, aim) = (*zone, *stand, stand.aim);
                *state = State::Holding {
                    zone,
                    stand,
                    arrived: cx.now,
                    since: None,
                    passed: std::mem::take(passed),
                };
                return Step::Continue(Intent::stay().hold_fire().looking(Look::Use(aim)));
            }
            if cx.me.feet.distance_xz(at) > num(cx, "stand_at_m") {
                *still_since = None;
                return Step::Continue(
                    Intent::to(at, cx.pace_toward(at))
                        .within(0.15)
                        .hold_fire()
                        .looking(Look::Use(stand.aim)),
                );
            }
            // At the stand and no press from it (the eye is not where the
            // list put it): the next stand after a while.
            let since = *still_since.get_or_insert(cx.now);
            if cx.now >= since + (num(cx, "stand_still_s") * 1000.0) as u32 {
                passed.push(at);
                match cx.nearest_zone_stand(*zone, passed) {
                    Some(next) => {
                        *stand = next;
                        *still_since = None;
                    }
                    None => return Step::Failed("no stand to press from"),
                }
            }
            Step::Continue(Intent::stay().hold_fire().looking(Look::Use(stand.aim)))
        }
        State::Holding {
            zone,
            stand,
            arrived,
            since,
            passed,
        } => {
            let Some(aim) = cx.zone_press(cx.me.eye(), *zone) else {
                // The ray left the box (pushed, turned): hold again from
                // the edge.
                let (zone, stand, aim) = (*zone, *stand, stand.aim);
                *state = State::Going {
                    zone,
                    stand,
                    still_since: None,
                    passed: std::mem::take(passed),
                };
                return Step::Continue(Intent::stay().hold_fire().looking(Look::Use(aim)));
            };
            let mut intent = Intent::stay().hold_fire().looking(Look::Use(aim));
            // Held once the view is on the box (or after a while there
            // anyway): a use pressed while still turning missed it.
            let begun = match *since {
                Some(begun) => begun,
                None if collect::aimed_at(cx, aim)
                    || cx.now >= *arrived + num(cx, "settle_ms") as u32 =>
                {
                    *since = Some(cx.now);
                    cx.now
                }
                None => return Step::Continue(intent),
            };
            if cx.now >= begun + num(cx, "hold_ms") as u32 {
                intent.resupplied = Some(*zone);
                *state = State::Finished;
                return Step::Continue(intent);
            }
            intent.hold = key::USE;
            Step::Continue(intent)
        }
        State::Finished => Step::Done("topped up"),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::v026g::bots::guns::Ammo;
    use crate::v026g::bots::know::Source;
    use crate::v026g::bots::plays::tests::Scenario;
    use crate::v026g::bots::tactics::Tactics;
    use crate::v026g::bots::view::Mode;

    /// The field with a resupply box for side 0 at (20, .., 20) and one for
    /// side 1 at (60, .., 20): 1.7 x 1.0 x 1.3 m, standing on the floor.
    fn with_boxes(s: &mut Scenario) {
        let mut world = Arc::try_unwrap(crate::v026g::bots::tactics::tests::field())
            .ok()
            .unwrap();
        for (team, x) in [(0, 20.0), (1, 60.0)] {
            let center = Vec3::new(x, 0.5, 20.0);
            world.map.zones.push(mapdata::Zone {
                kind: ZoneKind::Resupply,
                name: format!("resupply_{team}"),
                team,
                position: Vec3::new(x + 6.0, 0.0, 20.0),
                damage_type: String::new(),
                damage: None,
                shapes: vec![mapdata::Shape {
                    kind: mapdata::ShapeKind::Box,
                    center,
                    rotation: mapdata::Rotation::IDENTITY,
                    half: Vec3::new(0.85, 0.5, 0.65),
                }],
            });
        }
        s.tactics = Tactics::new(Arc::new(world), 1.0);
        switch_on(s);
    }

    /// The play on (it ships off).
    pub(super) fn switch_on(s: &mut Scenario) {
        s.tuning
            .plays
            .entry(PlayKind::Resupply)
            .or_insert_with(|| crate::v026g::bots::plays::default_tuning(PlayKind::Resupply))
            .numbers
            .insert("on".into(), 1.0);
    }

    fn ammo(have: u32) -> Ammo {
        Ammo {
            have,
            whole: 90,
            storage: 300,
        }
    }

    fn offered(s: &Scenario) -> bool {
        s.candidates(0).iter().any(|c| c.kind == PlayKind::Resupply)
    }

    #[test]
    fn a_bot_short_of_rounds_calm_and_near_its_box_is_offered_the_play() {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        with_boxes(&mut s);
        s.bot(0, 0, Vec3::new(35.0, 0.0, 22.0));
        s.ammo = ammo(30);
        assert!(offered(&s), "30 of 90 is under 40 %");
        s.ammo = ammo(60);
        assert!(!offered(&s), "two thirds is not short");
        s.ammo = ammo(0);
        assert!(offered(&s), "dry always");
        let rank = |s: &Scenario| {
            s.candidates(0)
                .iter()
                .find(|c| c.kind == PlayKind::Resupply)
                .map(|c| c.rank)
        };
        assert_eq!(rank(&s), Some(Rank::Objective), "nearly dry ranks with the objectives");
        s.ammo = ammo(30);
        assert_eq!(rank(&s), Some(Rank::Fight), "short ranks with the fights: an objective in hand keeps it");
        s.ammo = ammo(0);
        s.ammo = Ammo { storage: 0, ..ammo(0) };
        assert!(!offered(&s), "nothing in the kit's storage to give");
        s.ammo = ammo(30);
        s.view.places[0].carrying = Some(0);
        assert!(!offered(&s), "a carrier brings its battery");
        s.view.places[0].carrying = None;
        s.enemy(1, 1, Vec3::new(35.0, 0.0, 45.0), Some(Source::Heard));
        assert!(!offered(&s), "an enemy known within 30 m");
    }

    #[test]
    fn a_box_far_by_the_way_or_the_switch_off_is_no_offer() {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        with_boxes(&mut s);
        s.bot(0, 0, Vec3::new(75.0, 0.0, 22.0));
        s.ammo = ammo(10);
        assert!(!offered(&s), "55 m from its box");
        s.ammo = ammo(5);
        assert!(!offered(&s), "a nearly dry bot reaches no farther than zone_m by default");
        s.tuning
            .plays
            .entry(PlayKind::Resupply)
            .or_insert_with(|| crate::v026g::bots::plays::default_tuning(PlayKind::Resupply))
            .numbers
            .insert("zone_m_dry".into(), 80.0);
        assert!(offered(&s), "a nearly dry bot reaches 80 m when zone_m_dry says so");
        s.ammo = ammo(10);
        s.view.places[0].feet = Vec3::new(35.0, 0.0, 22.0);
        assert!(offered(&s));
        s.tuning
            .plays
            .entry(PlayKind::Resupply)
            .or_insert_with(|| crate::v026g::bots::plays::default_tuning(PlayKind::Resupply))
            .numbers
            .insert("on".into(), 0.0);
        assert!(!offered(&s), "on = 0");
    }

    #[test]
    fn it_walks_to_a_stand_turns_on_the_box_and_holds_use_to_the_end() {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        with_boxes(&mut s);
        s.bot(0, 0, Vec3::new(35.0, 0.0, 22.0));
        s.ammo = ammo(10);
        let target = TargetRef::Point(Vec3::new(20.0, 0.5, 20.0));
        let steps = s.run(0, PlayKind::Resupply, target, 200, 100, 4.0);
        let held: Vec<&Intent> = steps
            .iter()
            .filter_map(|s| match s {
                Step::Continue(i) if i.hold & key::USE != 0 => Some(i),
                _ => None,
            })
            .collect();
        // 3200 ms at 100 ms a step, and never firing or walking.
        assert!((30..=34).contains(&held.len()), "{}", held.len());
        assert!(held
            .iter()
            .all(|i| !i.fire && i.go == Go::Stay && matches!(i.look, Look::Use(_))));
        // It stood where the game's ray meets the box.
        let me = s.view.places[0].eye();
        assert!(s.cx(0).zone_press(me, 0).is_some());
        let given: Vec<_> = steps
            .iter()
            .filter(|s| matches!(s, Step::Continue(i) if i.resupplied == Some(0)))
            .collect();
        assert_eq!(given.len(), 1, "{steps:?}");
        assert_eq!(steps.last(), Some(&Step::Done("topped up")));
    }

    #[test]
    fn the_ray_off_the_box_lets_go_of_the_key_and_an_enemy_seen_ends_the_play() {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        with_boxes(&mut s);
        s.bot(0, 0, Vec3::new(35.0, 0.0, 22.0));
        s.ammo = ammo(10);
        let stand = s.cx(0).nearest_zone_stand(0, &[]).expect("a stand");
        let mut state = State::Holding {
            zone: 0,
            stand,
            arrived: 0,
            since: Some(s.view.clock - 1_000),
            passed: Vec::new(),
        };
        let target = TargetRef::Point(Vec3::new(20.0, 0.5, 20.0));
        s.current = Some((PlayKind::Resupply, target, s.view.clock - 1_000));
        let mut rng = Rng::new(1);
        // Far from the box: the ray does not meet it.
        let out = step(&mut state, target, &s.cx(0), &mut rng);
        assert!(matches!(&out, Step::Continue(i) if i.hold == 0), "{out:?}");
        assert!(matches!(state, State::Going { .. }), "{state:?}");
        // At the stand an enemy comes into view.
        s.view.places[0].feet = stand.feet;
        s.enemy(1, 1, Vec3::new(45.0, 0.0, 22.0), Some(Source::Seen));
        let out = step(&mut state, target, &s.cx(0), &mut rng);
        assert_eq!(out, Step::Done("an enemy came"));
    }
}
