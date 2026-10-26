//! Play 10: Health (`player_bots_behaviour.md` §3): a wounded bot that
//! cannot heal falls back to resupply. (Its other half, pushing a wounded
//! enemy, is the reflex's: a wounded target counts nearer.)
//!
//! - **When:** health under `below` and either no medkit left
//!   (`heal::medkits_left`) or under fire (an enemy seen now, or sensed by
//!   sight or hearing within `under_fire_s`). Kept once taken until done.
//! - **Where:** the team's resupply zone (`ZoneKind::Resupply` of its map
//!   side, the nearest if a map has more), else the edge of its own spawn
//!   area on the side it is on (`SPAWN_AREA_M` + `edge_m` from the nearest
//!   own spawn): back toward the base, not into the spawn.
//! - **The way:** an enemy it knows of within `clear_m` of the straight
//!   way puts a waypoint `detour_m` off it, on the side away from him
//!   (the walk itself keeps off hazards and their spawn area).
//! - **There:** `stay_min_s` to `stay_max_s`; in a resupply zone the use
//!   key is held while in it (resupply is use held in the zone); then
//!   done. It runs, and holds fire only if it carries a battery.

use super::*;
use crate::v026g::bots::curve::{Consideration, Curve, Input};
use crate::v026g::bots::know::Source;
use crate::v026g::bots::tactics::SPAWN_AREA_M;
use crate::v026g::drive::key;
use mapdata::ZoneKind;

pub const NUMBERS: &[(&str, f32)] = &[
    ("below", 0.35),
    ("under_fire_s", 2.0),
    ("edge_m", 3.0),
    ("clear_m", 15.0),
    ("detour_m", 15.0),
    ("arrive_m", 2.0),
    ("zone_m", 3.0),
    ("stay_min_s", 5.0),
    ("stay_max_s", 8.0),
    ("give_up_s", 45.0),
];

pub fn tuning() -> PlayTuning {
    PlayTuning {
        rank: Rank::Fight,
        considerations: vec![Consideration::new(
            Input::Health,
            0.0,
            1.0,
            Curve::Piecewise {
                points: vec![(0.0, 1.0), (1.0, 0.3)],
            },
        )],
        numbers: Default::default(),
    }
}

fn num(cx: &Ctx, name: &str) -> f32 {
    cx.num(PlayKind::Retreat, name)
}

/// Under fire: an enemy seen now, or seen or heard within `under_fire_s`.
pub fn under_fire(cx: &Ctx) -> bool {
    let within = (num(cx, "under_fire_s") * 1000.0) as u32;
    !cx.memory.all_seen(cx.now).is_empty()
        || cx
            .memory
            .tracks()
            .iter()
            .any(|t| matches!(t.source, Source::Seen | Source::Heard) && t.time + within >= cx.now)
}

pub fn candidates(cx: &Ctx, out: &mut Vec<Candidate>) {
    let running = matches!(cx.bot.current, Some((PlayKind::Retreat, _, _)));
    if !running
        && (cx.me.health >= num(cx, "below")
            || (super::heal::medkits_left(cx) > 0 && !under_fire(cx)))
    {
        return;
    }
    if goal(cx).is_none() {
        return;
    }
    out.extend(cx.candidate(PlayKind::Retreat, TargetRef::None, None));
}

/// Where to fall back to, and the resupply zone's index if it is one.
pub fn goal(cx: &Ctx) -> Option<(Vec3, Option<usize>)> {
    let side = cx.bot.side?;
    let map = &cx.tactics.world.map;
    let zone = map
        .zones
        .iter()
        .enumerate()
        .filter(|(_, z)| z.kind == ZoneKind::Resupply && z.team == side)
        .filter(|(_, z)| cx.tactics.fine_goal(z.position, cx.now, Some(side)))
        .min_by(|a, b| {
            a.1.position
                .distance_xz(cx.me.feet)
                .total_cmp(&b.1.position.distance_xz(cx.me.feet))
        });
    if let Some((index, zone)) = zone {
        return Some((zone.position, Some(index)));
    }
    // The edge of its own spawn area, on its side of it.
    let spawn = map
        .spawns
        .iter()
        .filter(|s| s.team == side)
        .min_by(|a, b| {
            a.position
                .distance_xz(cx.me.feet)
                .total_cmp(&b.position.distance_xz(cx.me.feet))
        })?
        .position;
    let edge = SPAWN_AREA_M + num(cx, "edge_m");
    let (dx, dz) = (cx.me.feet.x - spawn.x, cx.me.feet.z - spawn.z);
    let d = dx.hypot(dz);
    let at = if d <= edge {
        cx.me.feet
    } else {
        Vec3::new(
            spawn.x + dx / d * edge,
            cx.me.feet.y,
            spawn.z + dz / d * edge,
        )
    };
    cx.tactics
        .fine_goal(at, cx.now, Some(side))
        .then_some((at, None))
}

/// A waypoint off the straight way to `goal`, away from the known enemy
/// nearest that way if he is within `clear_m` of it.
fn detour(cx: &Ctx, goal: Vec3) -> Option<Vec3> {
    let me = cx.me.feet;
    let (dx, dz) = (goal.x - me.x, goal.z - me.z);
    let length = dx.hypot(dz);
    if length < 5.0 {
        return None;
    }
    let (ux, uz) = (dx / length, dz / length);
    // The nearest known enemy beside the way: (distance, which side).
    let near = cx
        .memory
        .tracks()
        .iter()
        .filter_map(|t| {
            let (ex, ez) = (t.at.x - me.x, t.at.z - me.z);
            let along = ex * ux + ez * uz;
            let across = ex * -uz + ez * ux;
            (0.0..=length)
                .contains(&along)
                .then_some((across.abs(), across.signum(), along))
        })
        .filter(|(off, _, _)| *off < num(cx, "clear_m"))
        .min_by(|a, b| a.0.total_cmp(&b.0))?;
    let (_, side, along) = near;
    let mut offset = num(cx, "detour_m");
    while offset >= 3.0 {
        // Beside him on the way, off to the other side.
        let via = Vec3::new(
            me.x + ux * along - (-uz) * side * offset,
            me.y,
            me.z + uz * along - ux * side * offset,
        );
        if cx.tactics.fine_goal(via, cx.now, cx.bot.side) {
            return Some(via);
        }
        offset *= 0.5;
    }
    None
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Via {
        via: Vec3,
        goal: Vec3,
        zone: Option<usize>,
    },
    Going {
        goal: Vec3,
        zone: Option<usize>,
    },
    Staying {
        until: u32,
        zone: Option<usize>,
    },
    /// No place to fall back to.
    Nowhere,
}

pub fn start(_target: TargetRef, cx: &Ctx, _rng: &mut Rng) -> State {
    match goal(cx) {
        Some((goal, zone)) => match detour(cx, goal) {
            Some(via) => State::Via { via, goal, zone },
            None => State::Going { goal, zone },
        },
        None => State::Nowhere,
    }
}

/// Whether the bot stands in the resupply zone `index`.
fn in_zone(cx: &Ctx, index: usize) -> bool {
    let Some(zone) = cx.tactics.world.map.zones.get(index) else {
        return false;
    };
    let chest = cx.me.feet + Vec3::new(0.0, 0.9, 0.0);
    if zone.shapes.is_empty() {
        cx.me.feet.distance_xz(zone.position) <= num(cx, "zone_m")
    } else {
        zone.shapes.iter().any(|shape| shape.contains(chest))
    }
}

pub fn step(state: &mut State, _target: TargetRef, cx: &Ctx, rng: &mut Rng) -> Step {
    if let Some((_, _, started)) = cx.bot.current
        && cx.now > started + (num(cx, "give_up_s") * 1000.0) as u32
    {
        return Step::Failed("took too long");
    }
    let carrying = cx.me.carrying.is_some();
    let going = |at: Vec3| {
        let intent = Intent::to(at, Pace::Run).within(num(cx, "arrive_m") * 0.75);
        if carrying {
            Intent {
                pace: Pace::Walk,
                ..intent.hold_fire()
            }
        } else {
            intent
        }
    };
    match state {
        State::Nowhere => Step::Failed("nowhere to fall back to"),
        State::Via { via, goal, zone } => {
            let (via, goal, zone) = (*via, *goal, *zone);
            if cx.me.feet.distance_xz(via) < 4.0 {
                *state = State::Going { goal, zone };
                return step(state, _target, cx, rng);
            }
            Step::Continue(going(via).within(3.0))
        }
        State::Going { goal, zone } => {
            let (goal, zone) = (*goal, *zone);
            let there = cx.me.feet.distance_xz(goal) <= num(cx, "arrive_m")
                || zone.is_some_and(|zone| in_zone(cx, zone));
            if there {
                let stay = rng.range(num(cx, "stay_min_s"), num(cx, "stay_max_s"));
                *state = State::Staying {
                    until: cx.now + (stay * 1000.0) as u32,
                    zone,
                };
                return step(state, _target, cx, rng);
            }
            Step::Continue(going(goal))
        }
        State::Staying { until, zone } => {
            if cx.now >= *until {
                return Step::Done("fell back and resupplied");
            }
            let mut intent = Intent::stay();
            if carrying {
                intent = intent.hold_fire();
            }
            if zone.is_some_and(|zone| in_zone(cx, zone)) {
                intent.hold |= key::USE;
            }
            Step::Continue(intent)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::v026g::bots::plays::tests::Scenario;
    use crate::v026g::bots::tactics::Tactics;
    use crate::v026g::bots::view::Mode;

    /// The field with a resupply zone of side 0 at (20, 0, 35) and one of
    /// side 1 at (60, 0, 35).
    fn with_zones(s: &mut Scenario) {
        let mut world = Arc::try_unwrap(crate::v026g::bots::tactics::tests::field())
            .ok()
            .unwrap();
        for (team, x) in [(0, 20.0), (1, 60.0)] {
            world.map.zones.push(mapdata::Zone {
                kind: ZoneKind::Resupply,
                name: format!("resupply_{team}"),
                team,
                position: Vec3::new(x, 0.0, 35.0),
                damage_type: String::new(),
                damage: None,
                shapes: Vec::new(),
            });
        }
        s.tactics = Tactics::new(Arc::new(world), 1.0);
    }

    #[test]
    fn under_fire_and_badly_hurt_it_falls_back_to_its_resupply_and_holds_use() {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        with_zones(&mut s);
        s.bot(0, 0, Vec3::new(45.0, 0.0, 32.0)).health = 0.3;
        s.enemy(1, 1, Vec3::new(65.0, 0.0, 33.0), Some(Source::Seen));
        // With a medkit it heals first, in the alarm band, under fire too.
        let best = s.candidates(0);
        assert_eq!(best[0].kind, PlayKind::Heal, "{best:#?}");
        // Both medkits of the kit used: nothing to heal with, so it falls back.
        s.pantry.pressed(0, 0);
        s.pantry.pressed(0, 0);
        let best = s.candidates(0);
        assert_eq!(best[0].kind, PlayKind::Retreat, "{best:#?}");
        assert!(best.iter().all(|c| c.kind != PlayKind::Heal));
        let steps = s.run(0, PlayKind::Retreat, TargetRef::None, 60, 300, 4.0);
        // To its own zone (side 0), not the other's.
        let goals: Vec<Vec3> = steps
            .iter()
            .filter_map(|s| match s {
                Step::Continue(Intent {
                    go: Go::To { at, .. },
                    ..
                }) => Some(*at),
                _ => None,
            })
            .collect();
        assert!(
            goals
                .last()
                .unwrap()
                .distance_xz(Vec3::new(20.0, 0.0, 35.0))
                < 0.5
        );
        let held = steps
            .iter()
            .filter(|s| matches!(s, Step::Continue(i) if i.hold & key::USE != 0))
            .count();
        // 5-8 s of 0.3 s steps holding use in the zone.
        assert!((15..=28).contains(&held), "{held}");
        assert_eq!(steps.last(), Some(&Step::Done("fell back and resupplied")));
        assert!(
            steps
                .iter()
                .all(|s| !matches!(s, Step::Continue(i) if !i.fire)),
            "fire held without a battery"
        );
    }

    #[test]
    fn with_a_medkit_and_not_under_fire_it_does_not_retreat() {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        with_zones(&mut s);
        s.bot(0, 0, Vec3::new(45.0, 0.0, 32.0)).health = 0.3;
        assert!(s.candidates(0).iter().all(|c| c.kind != PlayKind::Retreat));
        // Both medkits of the kit used: it falls back.
        s.pantry.pressed(0, 0);
        s.pantry.pressed(0, 0);
        assert!(s.candidates(0).iter().any(|c| c.kind == PlayKind::Retreat));
        // Healthy enough: not.
        s.view.places[0].health = 0.5;
        assert!(s.candidates(0).iter().all(|c| c.kind != PlayKind::Retreat));
    }

    #[test]
    fn with_no_zone_it_goes_to_the_edge_of_its_spawn_area_round_the_enemy() {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(60.0, 0.0, 35.0)).health = 0.2;
        // An enemy heard right on the straight way back.
        s.enemy(1, 1, Vec3::new(45.0, 0.0, 30.0), Some(Source::Heard));
        let (goal, zone) = goal(&s.cx(0)).unwrap();
        assert_eq!(zone, None);
        let spawn = Vec3::new(5.0, 0.0, 20.0);
        let d = goal.distance_xz(spawn);
        assert!(
            (SPAWN_AREA_M..SPAWN_AREA_M + 4.0).contains(&d),
            "{goal:?} {d}"
        );
        assert!(!s.tactics.in_spawn_area(goal, Some(1)));
        let mut rng = Rng::new(3);
        let state = start(TargetRef::None, &s.cx(0), &mut rng);
        let State::Via { via, .. } = state else {
            panic!("{state:?}");
        };
        // The waypoint is off the way, on the far side from him.
        let enemy = Vec3::new(45.0, 0.0, 30.0);
        assert!(via.distance_xz(enemy) >= 7.0, "{via:?}");
        assert!(via.z > enemy.z, "{via:?}");
    }
}
