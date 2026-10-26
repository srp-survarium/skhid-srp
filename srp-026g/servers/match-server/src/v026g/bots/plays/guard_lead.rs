//! Play 18, guard the lead (`player_bots_behaviour.md` §3 play 18; §4
//! "little time left: the winning team defends"): in the battery modes, in
//! the last ~2 minutes, with our side ahead, 1-2 bots hold a spot 12-20 m
//! from our container with a sight line to it, crouched, looking at the
//! container and at the way the enemy comes to it, to stop a late denial
//! (play 16). The only time bots guard their base, and never on top of the
//! container (players within 12 m of their own container fared worst).
//!
//! - Two slots per container, so at most two guards.
//! - The spot: `SpotQuery` round our container with a sight line to it
//!   (one that ends on its box: the box is solid in the sight tree),
//!   off every enemy spawn area, picked at random among the best few and
//!   away from a teammate's. The look turns between the container and its
//!   approach every few seconds (drawn); every 35-60 s (drawn) it moves
//!   to another spot, never the same one (humans learn a spot fast, play
//!   15's rule).
//! - It holds till the end, or until a theft (play 2's alarm answers it),
//!   or the lead is gone.
//!
//! **The container holder** (Cologne CG-1, CG-12): at any time, one more
//! slot ([`HOLDER_SLOT`], [`is_holder`]) for a scored need to hold our
//! container: it holds batteries, and a battery of ours was taken this
//! match (`TeamKnowledge::thefts_match`, by anyone), or one in the last
//! 60 s, or an enemy is known within `holder_near_m` of it. The score is
//! the guard's own (near, personality, health) times the need: the
//! batteries at stake ([`HOLDER_STAKES`]) times the thefts
//! ([`HOLDER_THEFTS`]). Its spot is a guard's, kept within
//! `holder_face_deg` of the way a thief comes: the best of the thieves'
//! lanes near the container (`HumanLanes::thief_lane`), else toward the
//! enemy container. It stays `Rank::Objective`, above every fight-band
//! play (a Crossfire elsewhere does not take it); a theft does not end it
//! (the alarm takes others), and it ends when the need has been off for `holder_off_s`. With
//! 2 or fewer of the team alive and a battery lying loose, no holder (until
//! the commander's role budget, CG-13, decides it).

use super::*;
use crate::v026g::bots::curve::{Consideration, Curve, Input, Score};
use crate::v026g::bots::tactics::SpotQuery;

pub const NUMBERS: &[(&str, f32)] = &[
    ("spot_min_m", 12.0),
    ("spot_max_m", 20.0),
    ("spot_samples", 64.0),
    // Picked at random among this many of the best spots.
    ("spot_pick", 4.0),
    // Spots higher than the container weigh this much more per metre.
    ("prefer_high", 0.3),
    // Spots on the storeys over the anchor are looked at up to this many metres
    // over it (0: only the floor the grid finds, at most 2 m over it). GUESS: 8 m,
    // a storey or two (the players' heights span 9-18 m a map); V-S2a.
    ("up_m", 8.0),
    // Not within this of a teammate.
    ("apart_m", 5.0),
    // The approach watched: this far from the container toward theirs.
    ("approach_m", 15.0),
    // The look turns between the container and the approach this often.
    ("glance_min_s", 3.0),
    ("glance_max_s", 7.0),
    // Another spot this often, at least `move_apart_m` from the last.
    ("move_min_s", 35.0),
    ("move_max_s", 60.0),
    ("move_apart_m", 6.0),
    ("reach_m", 1.5),
    // A theft this recent keeps guards off (the alarm goes first).
    ("theft_recent_s", 30.0),
    // The container holder (CG-1, CG-12). GUESS (2026-10-25) each:
    // an enemy known this near our container turns the need on;
    ("holder_near_m", 20.0),
    // the thieves' lane cells looked at, within this of the container;
    ("holder_lane_m", 30.0),
    // its spots within this of the way a thief comes, seen from the box;
    ("holder_face_deg", 60.0),
    // the need off this long ends the hold.
    ("holder_off_s", 20.0),
];

/// The container holder's slot key: `HOLDER_SLOT` plus the container's id
/// (the end-game guards' keys are twice the id and the next).
pub const HOLDER_SLOT: u32 = 0x100;

/// The holder's need by the batteries at stake in our container (0, 1, 2,
/// 3 or more). GUESS (2026-10-25).
pub const HOLDER_STAKES: [f32; 4] = [0.0, 0.5, 0.75, 1.0];

/// The holder's need by the batteries of ours taken this match (0, 1, 2 or
/// more): with none, only an enemy by the container or a fresh theft turns
/// it on. GUESS (2026-10-25).
pub const HOLDER_THEFTS: [f32; 3] = [0.6, 0.85, 1.0];

/// Whether a slot is the container holder's (not one of the end-game
/// guards').
pub fn is_holder_slot(slot: SlotKey) -> bool {
    slot.kind == PlayKind::GuardLead && slot.key >= HOLDER_SLOT
}

/// Whether a play is the container holder's: GuardLead on a container in
/// a [`HOLDER_SLOT`] key (the end-game guards' keys are below it). Pure,
/// for the commander's role budget (CG-13, at most one holder), which sees
/// only candidates and assignments.
pub fn is_holder(kind: PlayKind, target: TargetRef, slot: Option<SlotKey>) -> bool {
    kind == PlayKind::GuardLead
        && matches!(target, TargetRef::Container(_))
        && slot.is_some_and(is_holder_slot)
}

pub fn tuning() -> PlayTuning {
    PlayTuning {
        rank: Rank::Objective,
        considerations: vec![
            // Full in the last two minutes, 0 above three.
            Consideration::new(
                Input::TimeLeft,
                0.0,
                180.0,
                Curve::Piecewise {
                    points: vec![(0.0, 1.0), (0.67, 1.0), (0.83, 0.3), (1.0, 0.0)],
                },
            ),
            // Ahead by one or more; never level or behind.
            Consideration::new(Input::ScoreGap, 0.0, 1.0, Curve::Step { at: 1.0 }),
            considerations::near(150.0),
            considerations::personality(),
            considerations::health_above(0.2),
        ],
        numbers: Default::default(),
    }
}

pub fn candidates(cx: &Ctx, out: &mut Vec<Candidate>) {
    if !cx.view.mode.has_batteries() || cx.me.carrying.is_some() {
        return;
    }
    let Some(ours) = cx.view.container_of(cx.me.team) else {
        return;
    };
    out.extend(holder_candidate(cx, ours.id, ours.at));
    if cx.sig.score_gap <= 0.0
        || cx.sig.since_theft_s < cx.num(PlayKind::GuardLead, "theft_recent_s")
    {
        return;
    }
    let key = u32::from(ours.id) * 2;
    for slot in [key, key + 1] {
        out.extend(cx.candidate(
            PlayKind::GuardLead,
            TargetRef::Container(ours.id),
            Some(slot),
        ));
    }
}

/// The need to hold our container (at `container`), and how much (`None`:
/// none): [`HOLDER_STAKES`] times [`HOLDER_THEFTS`].
fn holder_need(cx: &Ctx, container: Vec3) -> Option<f32> {
    let ours = cx.view.container_of(cx.me.team)?;
    if ours.batteries == 0 {
        return None;
    }
    let thefts = cx.team.thefts_match as usize;
    // `TeamKnowledge::thefts` keeps the last 60 s.
    let fresh = !cx.team.thefts.is_empty();
    let near = cx.enemies_near(container, cx.num(PlayKind::GuardLead, "holder_near_m")) > 0;
    (thefts > 0 || fresh || near)
        .then(|| HOLDER_STAKES[ours.batteries.min(3)] * HOLDER_THEFTS[thefts.min(2)])
}

/// The container holder's candidate for our container `id` at `at`.
fn holder_candidate(cx: &Ctx, id: u8, at: Vec3) -> Option<Candidate> {
    let need = holder_need(cx, at)?;
    // Temporary, until the role budget (CG-13) gives the same and a test
    // pins it there: the last two never hold while a battery lies loose.
    let alive = cx.view.team(cx.me.team).count();
    let loose = (0..cx.view.batteries.len()).any(|b| cx.battery_lying(b).is_some());
    if alive <= 2 && loose {
        return None;
    }
    // The guard's considerations, its time and lead taken as met.
    let mut candidate = cx.candidate_given(
        PlayKind::GuardLead,
        TargetRef::Container(id),
        Some(HOLDER_SLOT + u32::from(id)),
        &[(Input::TimeLeft, 0.0), (Input::ScoreGap, 1.0)],
    )?;
    candidate.score = Score::new(candidate.score.get() * need);
    (!candidate.score.is_zero()).then_some(candidate)
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    NoSpot,
    Going {
        spot: Vec3,
        move_at: u32,
        hold: Hold,
    },
    Holding {
        spot: Vec3,
        move_at: u32,
        at_container: bool,
        glance_at: u32,
        hold: Hold,
    },
}

/// Whose rules a guard keeps: the end-game guard's, or the container
/// holder's (decided at its start: the need on then), and since when the
/// holder's need is off.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hold {
    EndGame,
    Holder { off_since: Option<u32> },
}

impl Ctx<'_> {
    /// A guard spot round `container`, not within `move_apart_m` of `last`.
    fn guard_spot(
        &self,
        container: Vec3,
        last: Option<Vec3>,
        face: Option<Vec3>,
        rng: &mut Rng,
    ) -> Option<Vec3> {
        let num = |name| self.num(PlayKind::GuardLead, name);
        let query = SpotQuery {
            sight_to: Some(container),
            // The container's box is solid in the sight tree: a sight line
            // to it ends on its faces.
            sight_body: self.tactics.container_body(container),
            prefer_high: num("prefer_high"),
            up_m: num("up_m"),
            side: self.bot.side,
            samples: num("spot_samples") as usize,
            ..SpotQuery::ring(container, num("spot_min_m"), num("spot_max_m"))
        };
        let pick = num("spot_pick") as usize;
        // Facing keeps about a third of a ring's spots: ask for three times
        // as many.
        let ask = if face.is_some() { pick * 6 } else { pick * 2 };
        let found: Vec<Vec3> = self
            .tactics
            .spots(&query, self.now, ask, rng)
            .into_iter()
            .map(|spot| spot.at)
            .filter(|&at| self.friends_near(at, num("apart_m")) == 0)
            .filter(|&at| last.is_none_or(|last| last.distance_xz(at) >= num("move_apart_m")))
            .collect();
        // The holder's: those facing the way a thief comes. None facing: any
        // spot for the first, else the one it has (`None`).
        let facing = |at: Vec3| {
            face.is_none_or(|face| {
                let yaw = |p: Vec3| (p.z - container.z).atan2(p.x - container.x);
                let off = (yaw(at) - yaw(face)).rem_euclid(std::f32::consts::TAU);
                off.min(std::f32::consts::TAU - off) <= num("holder_face_deg").to_radians()
            })
        };
        let spots: Vec<Vec3> = if found.iter().any(|&at| facing(at)) {
            found
                .into_iter()
                .filter(|&at| facing(at))
                .take(pick)
                .collect()
        } else if face.is_some() && last.is_some() {
            Vec::new()
        } else {
            found.into_iter().take(pick).collect()
        };
        (!spots.is_empty()).then(|| spots[rng.below(spots.len())])
    }

    /// The way a thief comes to our container: the best of the thieves'
    /// lanes near it, else toward the enemy container.
    fn holder_approach(&self, container: Vec3) -> Vec3 {
        let within = self.num(PlayKind::GuardLead, "holder_lane_m");
        self.team
            .humans
            .thief_lane(container, within, self.now)
            .first()
            .map_or_else(|| self.guard_approach(container), |&(at, _)| at)
    }

    /// The way the enemy comes to our container: toward theirs.
    fn guard_approach(&self, container: Vec3) -> Vec3 {
        let theirs = self
            .view
            .containers
            .iter()
            .find(|c| c.team != self.me.team)
            .map(|c| c.at);
        let Some(theirs) = theirs else {
            return container;
        };
        let d = container.distance_xz(theirs).max(0.1);
        let m = self.num(PlayKind::GuardLead, "approach_m").min(d);
        container + (theirs - container) * (m / d)
    }
}

fn draw_ms(cx: &Ctx, rng: &mut Rng, lo: &str, hi: &str) -> u32 {
    let num = |name| cx.num(PlayKind::GuardLead, name);
    (rng.range(num(lo), num(hi)) * 1000.0) as u32
}

/// A guard's start. Its rules are the slot's: a holder's slot is a holder
/// whatever the need is at this instant (its own end is the need off for
/// `holder_off_s`), and an end-game guard's slot is never one (the need
/// turning on after it was given the play does not change its end).
pub fn start(target: TargetRef, slot: Option<SlotKey>, cx: &Ctx, rng: &mut Rng) -> State {
    let Some(container) = cx.point_of(target) else {
        return State::NoSpot;
    };
    let hold = if slot.is_some_and(is_holder_slot) {
        Hold::Holder { off_since: None }
    } else {
        Hold::EndGame
    };
    match cx.guard_spot(container, None, face(cx, hold, container), rng) {
        Some(spot) => State::Going {
            spot,
            move_at: cx.now + draw_ms(cx, rng, "move_min_s", "move_max_s"),
            hold,
        },
        None => State::NoSpot,
    }
}

/// The way a guard of `hold`'s rules faces: the holder's spots and look
/// face a thief's way; an end-game guard's spots face anywhere.
fn face(cx: &Ctx, hold: Hold, container: Vec3) -> Option<Vec3> {
    matches!(hold, Hold::Holder { .. }).then(|| cx.holder_approach(container))
}

pub fn step(state: &mut State, target: TargetRef, cx: &Ctx, rng: &mut Rng) -> Step {
    let num = |name| cx.num(PlayKind::GuardLead, name);
    let Some(container) = cx.point_of(target) else {
        return Step::Failed("no container of ours");
    };
    let hold = match state {
        State::NoSpot => return Step::Failed("no spot with a sight line"),
        State::Going { hold, .. } | State::Holding { hold, .. } => hold,
    };
    match hold {
        Hold::EndGame => {
            let started = cx.bot.current.map_or(cx.now, |(_, _, since)| since);
            if cx.team.thefts.iter().any(|t| t.time >= started) {
                return Step::Done("a theft: the alarm answers it");
            }
            if cx.sig.score_gap <= 0.0 {
                return Step::Done("the lead is gone");
            }
        }
        Hold::Holder { off_since } => {
            if holder_need(cx, container).is_some() {
                *off_since = None;
            } else {
                let since = *off_since.get_or_insert(cx.now);
                if cx.now >= since + (num("holder_off_s") * 1000.0) as u32 {
                    return Step::Done("no need to hold the container");
                }
            }
        }
    }
    let hold = *hold;
    // Time to move on: another spot (none found: this one a while more).
    if let State::Going { spot, move_at, .. } | State::Holding { spot, move_at, .. } = *state
        && cx.now >= move_at
    {
        let next = cx.guard_spot(container, Some(spot), face(cx, hold, container), rng);
        let move_at = cx.now + draw_ms(cx, rng, "move_min_s", "move_max_s");
        *state = State::Going {
            spot: next.unwrap_or(spot),
            move_at,
            hold,
        };
    }
    match *state {
        State::NoSpot => Step::Failed("no spot with a sight line"),
        State::Going { spot, move_at, .. } => {
            if cx.me.feet.distance_xz(spot) <= num("reach_m") {
                *state = State::Holding {
                    spot,
                    move_at,
                    at_container: true,
                    glance_at: cx.now + draw_ms(cx, rng, "glance_min_s", "glance_max_s"),
                    hold,
                };
                return Step::Continue(Intent::stay().crouched().looking(Look::At(container)));
            }
            Step::Continue(Intent::to(spot, cx.pace_toward(spot)).within(num("reach_m") * 0.7))
        }
        State::Holding {
            spot,
            move_at,
            mut at_container,
            mut glance_at,
            ..
        } => {
            if cx.now >= glance_at {
                at_container = !at_container;
                glance_at = cx.now + draw_ms(cx, rng, "glance_min_s", "glance_max_s");
                *state = State::Holding {
                    spot,
                    move_at,
                    at_container,
                    glance_at,
                    hold,
                };
            }
            let look = if at_container {
                container
            } else {
                face(cx, hold, container).unwrap_or_else(|| cx.guard_approach(container))
            };
            let intent = if cx.me.feet.distance_xz(spot) > num("reach_m") * 2.0 {
                Intent::to(spot, Pace::Walk).within(num("reach_m") * 0.7)
            } else {
                Intent::stay()
            };
            Step::Continue(intent.crouched().looking(Look::At(look)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::Scenario;
    use super::*;
    use crate::v026g::bots::know::{Source, Theft};
    use crate::v026g::bots::team::{Asking, assign};
    use crate::v026g::bots::view::{BatteryState, EYE_CROUCHED, Mode};

    const OURS: Vec3 = Vec3::new(12.0, 0.0, 30.0);

    /// Ahead 3 to 2 with 90 s left; bots 0, 2 and 4 of team 0 out in the
    /// field.
    fn scene() -> Scenario {
        let mut s = Scenario::field(Mode::Batteries);
        s.bot(0, 0, Vec3::new(30.0, 0.0, 35.0));
        s.bot(2, 0, Vec3::new(32.0, 0.0, 30.0));
        s.bot(4, 0, Vec3::new(34.0, 0.0, 36.0));
        s.time_left(90.0).points(3, 2);
        s
    }

    fn guards(s: &Scenario, place: Place) -> Vec<Candidate> {
        let mut out = Vec::new();
        candidates(&s.cx(place), &mut out);
        out
    }

    #[test]
    fn guards_only_late_ahead_and_with_no_fresh_theft() {
        let found = guards(&scene(), 0);
        assert_eq!(found.len(), 2, "{found:#?}");
        assert!(found.iter().all(|c| c.target == TargetRef::Container(0)));
        let mut s = scene();
        s.points(2, 3);
        assert!(guards(&s, 0).is_empty(), "behind");
        s.points(2, 2);
        assert!(guards(&s, 0).is_empty(), "level");
        let mut s = scene();
        s.time_left(400.0);
        assert!(guards(&s, 0).is_empty(), "too early");
        let mut s = scene();
        s.sig.since_theft_s = 5.0;
        assert!(guards(&s, 0).is_empty(), "a theft first");
        let mut s = scene();
        s.view.mode = Mode::TeamDeathmatch;
        assert!(guards(&s, 0).is_empty());
    }

    #[test]
    fn at_most_two_guards() {
        let s = scene();
        let asking = [0, 2, 4]
            .into_iter()
            .map(|place| Asking {
                place,
                candidates: guards(&s, place),
                current: None,
            })
            .collect();
        assert_eq!(assign(asking, s.view.clock).len(), 2);
    }

    #[test]
    fn it_holds_a_spot_in_sight_of_the_container_looks_about_and_moves_on() {
        let mut s = scene();
        let steps = s.run(
            0,
            PlayKind::GuardLead,
            TargetRef::Container(0),
            300,
            300,
            3.5,
        );
        assert!(
            steps.iter().all(|s| matches!(s, Step::Continue(_))),
            "{:?}",
            steps.last()
        );
        // The spots it went to: 12-20 m off, seeing the container.
        let mut spots: Vec<Vec3> = Vec::new();
        for step in &steps {
            if let Step::Continue(Intent {
                go: Go::To { at, .. },
                ..
            }) = step
                && spots.last().is_none_or(|last| last.distance_xz(*at) > 0.1)
            {
                spots.push(*at);
            }
        }
        assert!(spots.len() >= 2, "it moved on: {spots:?}");
        for spot in &spots {
            let d = spot.distance_xz(OURS);
            assert!((11.5..=20.5).contains(&d), "{spot:?}");
            let eye = *spot + Vec3::new(0.0, EYE_CROUCHED, 0.0);
            assert!(s.tactics.sees(eye, OURS + Vec3::new(0.0, 1.0, 0.0)));
        }
        assert!(spots[1].distance_xz(spots[0]) >= 6.0);
        // Crouched, still, looking at the container and at its approach.
        let looks: Vec<Vec3> = steps
            .iter()
            .filter_map(|s| match s {
                Step::Continue(Intent {
                    go: Go::Stay,
                    crouch: true,
                    look: Look::At(at),
                    ..
                }) => Some(*at),
                _ => None,
            })
            .collect();
        assert!(looks.contains(&OURS));
        assert!(looks.iter().any(|at| at.x > OURS.x + 10.0), "{looks:?}");
    }

    /// The 12 triangles of a solid box.
    fn solid_box(center: Vec3, half: Vec3) -> Vec<mapdata::Triangle> {
        let corner = |i: usize| {
            let pick = |bit: usize, h: f32| if i >> bit & 1 == 1 { h } else { -h };
            center + Vec3::new(pick(0, half.x), pick(1, half.y), pick(2, half.z))
        };
        let faces = [
            [0, 1, 3, 2],
            [4, 5, 7, 6],
            [0, 1, 5, 4],
            [2, 3, 7, 6],
            [0, 2, 6, 4],
            [1, 3, 7, 5],
        ];
        faces
            .iter()
            .flat_map(|&[a, b, c, d]| {
                [[a, b, c], [a, c, d]].map(|v| mapdata::Triangle {
                    v: v.map(corner),
                    walker: mapdata::WalkerBehaviour::BySlope,
                })
            })
            .collect()
    }

    /// On the real maps the container's box is solid in the sight tree,
    /// and the sight line goes to its middle: a line that ends on the box
    /// sees it. (Before, no spot on any real map ever did; the play failed
    /// at once and was taken again at the next think, ~670 times in a
    /// 180 s bench match.)
    #[test]
    fn a_solid_container_box_is_seen_on_its_faces() {
        use crate::v026g::bots::tactics::{SpotQuery, Tactics, tests::field_with_solids};
        use mapdata::{Container, Rotation, Shape, ShapeKind};
        let middle = OURS + Vec3::new(0.0, 1.0, 0.0);
        let half = Vec3::new(1.0, 0.85, 0.65);
        let body = Shape {
            kind: ShapeKind::Box,
            center: middle,
            rotation: Rotation::IDENTITY,
            half,
        };
        let world = field_with_solids(&solid_box(middle, half), |map| {
            map.containers.push(Container {
                position: OURS,
                team: 0,
                id: 0,
                use_shapes: vec![body],
            });
        });
        let mut s = scene();
        s.tactics = Tactics::new(world, 1.0);
        assert_eq!(s.tactics.container_body(OURS), Some(body));
        let eye = Vec3::new(28.0, EYE_CROUCHED, 30.0);
        assert!(!s.tactics.sees(eye, middle), "its own faces block the line");
        assert!(s.tactics.sees_body(eye, middle, &body));
        // Behind the wall across x = 40: not seen.
        let behind = Vec3::new(50.0, EYE_CROUCHED, 10.0);
        assert!(!s.tactics.sees_body(behind, middle, &body));
        // A plain sight line finds no spot round it.
        let plain = SpotQuery {
            sight_to: Some(OURS),
            ..SpotQuery::ring(OURS, 12.0, 20.0)
        };
        let mut rng = Rng::new(3);
        assert!(
            s.tactics
                .spots(&plain, s.view.clock, 8, &mut rng)
                .is_empty()
        );
        // The play finds one and holds it.
        let steps = s.run(
            0,
            PlayKind::GuardLead,
            TargetRef::Container(0),
            100,
            300,
            3.5,
        );
        assert!(
            steps.iter().all(|s| matches!(s, Step::Continue(_))),
            "{:?}",
            steps.last()
        );
        assert!(steps.iter().any(|s| matches!(
            s,
            Step::Continue(Intent {
                go: Go::Stay,
                crouch: true,
                ..
            })
        )));
    }

    /// Level, mid-match, our container holding 2 batteries; bots 0, 2 and
    /// 4 of team 0 out in the field.
    fn holder_scene() -> Scenario {
        let mut s = scene();
        s.time_left(500.0).points(2, 2);
        s.container_count(0, 2);
        s.slot = Some(SlotKey {
            kind: PlayKind::GuardLead,
            key: HOLDER_SLOT,
        });
        s
    }

    fn holders(s: &Scenario, place: Place) -> Vec<Candidate> {
        guards(s, place)
            .into_iter()
            .filter(|c| is_holder(c.kind, c.target, c.slot))
            .collect()
    }

    #[test]
    fn a_holder_only_with_a_need() {
        let mut s = holder_scene();
        assert!(guards(&s, 0).is_empty(), "no theft, no enemy by it");
        s.team.thefts_match = 1;
        let found = holders(&s, 0);
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].target, TargetRef::Container(0));
        assert!(is_holder_slot(found[0].slot.unwrap()));
        assert!(is_holder(
            PlayKind::GuardLead,
            TargetRef::Container(0),
            found[0].slot
        ));
        assert_eq!(found[0].rank, Rank::Objective);
        // Nothing at stake: no need.
        s.container_count(0, 0);
        assert!(holders(&s, 0).is_empty());
        // An enemy known by the container turns it on with no theft.
        let mut s = holder_scene();
        s.enemy(1, 1, OURS + Vec3::new(8.0, 0.0, 0.0), Some(Source::Seen));
        assert_eq!(holders(&s, 0).len(), 1);
        let mut s = holder_scene();
        s.view.mode = Mode::TeamDeathmatch;
        s.team.thefts_match = 1;
        assert!(holders(&s, 0).is_empty());
    }

    #[test]
    fn the_need_grows_with_the_stakes_and_the_thefts() {
        let score = |batteries: usize, thefts: u32| {
            let mut s = holder_scene();
            s.container_count(0, batteries);
            s.team.thefts_match = thefts;
            holders(&s, 0)[0].score.get()
        };
        assert!(score(1, 1) < score(3, 1));
        assert!(score(2, 1) < score(2, 2));
        assert_eq!(score(3, 2), score(5, 9));
    }

    #[test]
    fn one_holder_beside_the_end_game_guards() {
        let mut s = holder_scene();
        s.team.thefts_match = 2;
        let asking = [0, 2, 4]
            .into_iter()
            .map(|place| Asking {
                place,
                candidates: guards(&s, place),
                current: None,
            })
            .collect();
        let given = assign(asking, s.view.clock);
        let holding = given
            .iter()
            .filter(|(_, a, _)| is_holder(a.kind, a.target, a.slot));
        assert_eq!(holding.count(), 1, "{given:#?}");
        // Late and ahead: the two end-game guards and the holder.
        s.time_left(90.0).points(3, 2);
        let found = guards(&s, 0);
        assert_eq!(found.len(), 3, "{found:#?}");
        let holding = |c: &&Candidate| is_holder(c.kind, c.target, c.slot);
        assert_eq!(found.iter().filter(holding).count(), 1);
        // The end-game guards' slots are not the holder's.
        for c in found.iter().filter(|c| !holding(c)) {
            assert!(!is_holder(c.kind, c.target, c.slot), "{c:?}");
            assert!(c.slot.is_some_and(|k| k.key < HOLDER_SLOT));
        }
    }

    #[test]
    fn the_last_two_do_not_hold_while_a_battery_lies_loose() {
        let mut s = Scenario::field(Mode::Batteries);
        s.bot(0, 0, Vec3::new(30.0, 0.0, 35.0));
        s.bot(2, 0, Vec3::new(32.0, 0.0, 30.0));
        s.time_left(500.0).container_count(0, 2);
        s.team.thefts_match = 1;
        assert_eq!(holders(&s, 0).len(), 1, "nothing loose");
        s.battery(BatteryState::Free(Vec3::new(45.0, 0.0, 20.0)));
        assert!(holders(&s, 0).is_empty());
        s.bot(4, 0, Vec3::new(34.0, 0.0, 36.0));
        assert_eq!(holders(&s, 0).len(), 1, "three alive");
    }

    #[test]
    fn the_holders_spot_faces_the_thiefs_lane() {
        let mut s = holder_scene();
        s.team.thefts_match = 1;
        // A human thief came to our container from the north (+z); the
        // enemy container is east (+x).
        s.human(7, 1, Vec3::new(60.0, 0.0, 50.0));
        let lane = OURS + Vec3::new(0.0, 0.0, 16.0);
        for i in 0..6u32 {
            s.team
                .humans
                .knew(7, lane, s.view.clock - 60_000 + i * 1_000);
        }
        s.team.humans.stole(7, 0, s.view.clock - 50_000);
        let steps = s.run(
            0,
            PlayKind::GuardLead,
            TargetRef::Container(0),
            300,
            300,
            3.5,
        );
        assert!(
            steps.iter().all(|s| matches!(s, Step::Continue(_))),
            "{:?}",
            steps.last()
        );
        let spots: Vec<Vec3> = steps
            .iter()
            .filter_map(|s| match s {
                Step::Continue(Intent {
                    go: Go::To { at, .. },
                    ..
                }) => Some(*at),
                _ => None,
            })
            .collect();
        assert!(!spots.is_empty());
        for spot in &spots {
            let off = (spot.x - OURS.x).atan2(spot.z - OURS.z).abs();
            assert!(off <= 61f32.to_radians(), "{spot:?} faces the lane");
        }
        // It looks at the lane, not toward their container.
        let looks: Vec<Vec3> = steps
            .iter()
            .filter_map(|s| match s {
                Step::Continue(Intent {
                    go: Go::Stay,
                    look: Look::At(at),
                    ..
                }) => Some(*at),
                _ => None,
            })
            .collect();
        assert!(
            looks.iter().any(|at| at.distance_xz(lane) < 3.0),
            "{looks:?}"
        );
    }

    #[test]
    fn a_theft_does_not_end_the_holder_and_the_need_gone_does() {
        let mut s = holder_scene();
        s.team.thefts_match = 1;
        let mut rng = Rng::new(2);
        let target = TargetRef::Container(0);
        s.current = Some((PlayKind::GuardLead, target, s.view.clock));
        let slot = Some(SlotKey {
            kind: PlayKind::GuardLead,
            key: HOLDER_SLOT,
        });
        let mut state = start(target, slot, &s.cx(0), &mut rng);
        assert!(matches!(
            state,
            State::Going {
                hold: Hold::Holder { .. },
                ..
            }
        ));
        s.team.thefts.push(Theft {
            battery: 1,
            at: OURS,
            time: s.view.clock + 1,
            thief: None,
        });
        s.team.thefts_match = 2;
        s.view.clock += 300;
        assert!(matches!(
            step(&mut state, target, &s.cx(0), &mut rng),
            Step::Continue(_)
        ));
        // The container emptied: the need is off; 20 s later it ends.
        s.container_count(0, 0);
        s.view.clock += 300;
        assert!(matches!(
            step(&mut state, target, &s.cx(0), &mut rng),
            Step::Continue(_)
        ));
        s.view.clock += 19_000;
        assert!(matches!(
            step(&mut state, target, &s.cx(0), &mut rng),
            Step::Continue(_)
        ));
        s.view.clock += 1_500;
        assert_eq!(
            step(&mut state, target, &s.cx(0), &mut rng),
            Step::Done("no need to hold the container")
        );
    }

    #[test]
    fn the_slot_not_the_need_at_the_start_gives_the_hold() {
        let target = TargetRef::Container(0);
        let hold_of = |state: &State| match state {
            State::Going { hold, .. } | State::Holding { hold, .. } => *hold,
            State::NoSpot => panic!("no spot"),
        };
        // An end-game guard's slot with the holder's need on is still an
        // end-game guard: a theft after it started ends it.
        let mut s = holder_scene();
        s.team.thefts_match = 1;
        let mut rng = Rng::new(2);
        s.current = Some((PlayKind::GuardLead, target, s.view.clock));
        let guard = Some(SlotKey {
            kind: PlayKind::GuardLead,
            key: 0,
        });
        let mut state = start(target, guard, &s.cx(0), &mut rng);
        assert_eq!(hold_of(&state), Hold::EndGame);
        s.team.thefts.push(Theft {
            battery: 1,
            at: OURS,
            time: s.view.clock + 1,
            thief: None,
        });
        s.view.clock += 300;
        assert_eq!(
            step(&mut state, target, &s.cx(0), &mut rng),
            Step::Done("a theft: the alarm answers it")
        );
        // A holder's slot with the need off at the start is still a holder
        // (its end is the need off for `holder_off_s`).
        let mut s = holder_scene();
        s.container_count(0, 0);
        let holder = Some(SlotKey {
            kind: PlayKind::GuardLead,
            key: HOLDER_SLOT,
        });
        let state = start(target, holder, &s.cx(0), &mut rng);
        assert!(matches!(hold_of(&state), Hold::Holder { .. }));
    }

    #[test]
    fn a_theft_or_a_lost_lead_ends_it() {
        let mut s = scene();
        let mut rng = Rng::new(2);
        s.current = Some((PlayKind::GuardLead, TargetRef::Container(0), s.view.clock));
        let mut state = start(
            TargetRef::Container(0),
            Some(SlotKey {
                kind: PlayKind::GuardLead,
                key: 0,
            }),
            &s.cx(0),
            &mut rng,
        );
        assert!(matches!(state, State::Going { .. }));
        assert!(matches!(
            step(&mut state, TargetRef::Container(0), &s.cx(0), &mut rng),
            Step::Continue(_)
        ));
        s.team.thefts.push(Theft {
            battery: 0,
            at: OURS,
            time: s.view.clock + 1,
            thief: None,
        });
        s.view.clock += 300;
        assert_eq!(
            step(&mut state, TargetRef::Container(0), &s.cx(0), &mut rng),
            Step::Done("a theft: the alarm answers it")
        );
        s.team.thefts.clear();
        s.points(3, 3);
        assert_eq!(
            step(&mut state, TargetRef::Container(0), &s.cx(0), &mut rng),
            Step::Done("the lead is gone")
        );
    }
}
