//! Answers to the plays' costly questions of the map, kept for the match
//! ([`Memo`], owned by `Bots`, reached as `Ctx::memo`): a question whose
//! answer depends only on the map and on what each zone is (its control
//! and its hit, set at `Hazards::attach`) is worked out once and read
//! after.
//!
//! The first is a battery's crossing ([`collect::crossing_to`]): about
//! 0.47 ms a call, asked about 70 times in a slow commander pass (Collect's
//! and Ambush's candidates and the notes' `take_blocks`, each for every
//! bot and battery in zones on all match), 93 % of that pass.
//!
//! [`collect::crossing_to`]: super::collect::crossing_to

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Mutex, OnceLock, PoisonError};

use mapdata::{UsePose, UseStand, UseStands, Vec3};

use super::Ctx;
use super::collect::{Crossing, crossing_to};
use crate::v026g::bots::hazards::Hazards;
use crate::v026g::bots::tactics::Tactics;

/// Answers one question keeps at most; past them it starts afresh (a
/// match asks a few dozen: the stands of batteries in zones on all match,
/// by side).
const MEMO_MOST: usize = 4096;

/// What each zone is, as the answers depend on it: whether it switches
/// (its control, `Hazard::switches`) and its hit a second
/// (`Hazard::hit_per_s`). It changes only at `Hazards::attach` (and in
/// tests that set a zone's control).
fn zones_stamp(hazards: &Hazards) -> impl Iterator<Item = u64> + Clone + '_ {
    hazards
        .zones
        .iter()
        .map(|zone| u64::from(zone.switches()) << 32 | u64::from(zone.hit_per_s().to_bits()))
}

/// One question's answers by key, and the stamp ([`zones_stamp`]) they
/// were made for: another stamp drops them all.
pub struct QueryMemo<K, V> {
    kept: Mutex<Kept<K, V>>,
}

struct Kept<K, V> {
    made_for: Vec<u64>,
    answers: HashMap<K, V>,
}

impl<K, V> Default for QueryMemo<K, V> {
    fn default() -> Self {
        Self {
            kept: Mutex::new(Kept {
                made_for: Vec::new(),
                answers: HashMap::new(),
            }),
        }
    }
}

impl<K: Eq + Hash, V: Clone> QueryMemo<K, V> {
    /// The answer for `key` under `stamp`: the kept one, else `ask`'s,
    /// kept. `ask` runs with nothing locked.
    pub fn get(
        &self,
        stamp: impl Iterator<Item = u64> + Clone,
        key: K,
        ask: impl FnOnce() -> V,
    ) -> V {
        {
            let mut kept = self.kept.lock().unwrap_or_else(PoisonError::into_inner);
            if !kept.made_for.iter().copied().eq(stamp.clone()) {
                kept.answers.clear();
                kept.made_for = stamp.collect();
            } else if let Some(answer) = kept.answers.get(&key) {
                return answer.clone();
            }
        }
        let answer = ask();
        let mut kept = self.kept.lock().unwrap_or_else(PoisonError::into_inner);
        if kept.answers.len() >= MEMO_MOST {
            kept.answers.clear();
        }
        kept.answers.insert(key, answer.clone());
        answer
    }

    /// Answers kept.
    pub fn kept(&self) -> usize {
        self.kept
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .answers
            .len()
    }
}

/// A crossing's question: the stand's x, y, z bits, the side, the stop's
/// bits.
type CrossingKey = (u32, u32, u32, Option<i32>, u32);

/// The plays' kept answers for one match on one map (`Bots` makes it anew
/// with the tactics).
#[derive(Default)]
pub struct Memo {
    crossings: QueryMemo<CrossingKey, Option<Crossing>>,
    /// The containers' stands: they depend on the map alone, so they are
    /// made at the first ask and kept for the match.
    stands: OnceLock<UseStands>,
}

impl Memo {
    /// [`crossing_to`], kept: the same answer, worked out once a match for
    /// each stand, side and stop while the zones stay what they are.
    pub fn crossing(
        &self,
        tactics: &Tactics,
        to: Vec3,
        side: Option<i32>,
        stop_m: f32,
    ) -> Option<Crossing> {
        let key = (
            to.x.to_bits(),
            to.y.to_bits(),
            to.z.to_bits(),
            side,
            stop_m.to_bits(),
        );
        self.crossings.get(zones_stamp(&tactics.hazards), key, || {
            crossing_to(tactics, to, side, stop_m)
        })
    }

    /// The stands of every container of the map, worked out at the first
    /// ask ([`UseStands::build`]: the game's use test, a few milliseconds a
    /// map).
    pub fn use_stands(&self, tactics: &Tactics) -> &UseStands {
        self.stands.get_or_init(|| UseStands::build(&tactics.world))
    }

    /// Crossings kept.
    pub fn crossings_kept(&self) -> usize {
        self.crossings.kept()
    }
}

impl Ctx<'_> {
    /// The stands of container `container` (its id): floors from which the
    /// game's use ray meets its box, nearest the box first. One list for a
    /// put-in, a theft and the holder's sight.
    pub fn use_stands(&self, container: u8) -> &[UseStand] {
        self.memo.use_stands(self.tactics).of(container)
    }

    /// The stand of `container` in `pose` the bot reaches soonest
    /// (`path_dist`), if there is one, leaving out the floors in `passed`
    /// (stands it stood on and could not press from).
    pub fn nearest_use_stand(
        &self,
        container: u8,
        pose: UsePose,
        passed: &[Vec3],
    ) -> Option<UseStand> {
        self.use_stands(container)
            .iter()
            .filter(|stand| stand.fits(pose) && !passed.contains(&stand.feet))
            .min_by(|a, b| {
                self.path_dist(a.feet)
                    .total_cmp(&self.path_dist(b.feet))
                    .then(a.feet.x.total_cmp(&b.feet.x))
                    .then(a.feet.z.total_cmp(&b.feet.z))
            })
            .copied()
    }

    /// Where on the box of `container` a use ray from `eye` can meet it by
    /// the game's rule (a point of the box), if one can: the press test for
    /// the moment of the press. Out of reach of the box (its farthest
    /// corner and the ray) no ray is cast.
    pub fn use_press(&self, eye: Vec3, container: u8) -> Option<Vec3> {
        let world = &self.tactics.world;
        let shape = world
            .map
            .containers
            .iter()
            .find(|c| c.id == i32::from(container))?
            .use_shapes
            .first()?;
        if (shape.center - eye).length() > shape.half.length() + mapdata::stands::USE_RAY_M {
            return None;
        }
        mapdata::stands::press(&world.sight, eye, shape).map(|(aim, _)| aim)
    }

    /// Whether the game's use ray from `eye` can meet the box of
    /// `container` ([`Ctx::use_press`]).
    pub fn use_press_ok(&self, eye: Vec3, container: u8) -> bool {
        self.use_press(eye, container).is_some()
    }
}

/// A storey, metres: floors this far apart are on different levels (the
/// grid's `locate` window; a ramp or a ledge is under it).
pub const STOREY_GAP_M: f32 = 2.5;

impl Ctx<'_> {
    /// The walk to `at` when the bot and `at` are a storey or more apart
    /// ([`STOREY_GAP_M`]): the regions' way between the floors
    /// ([`Tactics::walk_estimate`]) if longer than [`Ctx::path_dist`]'s
    /// straight guess, which ignores the floor (a battery a storey up
    /// looked near to everyone under it: 105 plays on School's battery 3,
    /// 58 % of the Collect time in the match). `None` on one level, or
    /// without a floor under either end: the straight guess stands.
    pub fn storey_walk_dist(&self, at: Vec3) -> Option<f32> {
        let grid = &self.tactics.world.grid;
        let from = grid.locate(self.me.feet, 1.5)?;
        let to = crate::v026g::bots::path::goal_floor(grid, at)?;
        if (grid.position(from).y - grid.position(to).y).abs() < STOREY_GAP_M {
            return None;
        }
        let walk = self.tactics.walk_estimate(from, to)?;
        Some(walk.max(self.path_dist(at)))
    }
}

#[cfg(test)]
mod tests {
    use mapdata::{Rotation, Shape, ShapeKind, Vec3, Zone, ZoneKind};

    use super::*;
    use crate::v026g::bots::anomalies::Control;

    /// The field with a zone of radius 4 m at (25, 32) that hurts `max_hit`
    /// a second, on all match.
    fn hurting(max_hit: f32) -> Tactics {
        let world = crate::v026g::bots::tactics::tests::field_with(|map| {
            map.zones.push(Zone {
                kind: ZoneKind::Damage,
                name: "toxic".into(),
                team: -1,
                position: Vec3::new(25.0, 0.0, 32.0),
                damage_type: "toxic".into(),
                damage: Some(mapdata::Damage {
                    max_hit,
                    interval_ms: 1000,
                    parts: 2,
                    ..mapdata::Damage::default()
                }),
                shapes: vec![Shape {
                    kind: ShapeKind::Sphere,
                    center: Vec3::new(25.0, 1.0, 32.0),
                    rotation: Rotation::IDENTITY,
                    half: Vec3::new(4.0, 4.0, 4.0),
                }],
            });
        });
        Tactics::new(world, 1.0)
    }

    /// A solid box (12 triangles) for the sight tree.
    fn solid_box(center: Vec3, half: Vec3) -> Vec<mapdata::Triangle> {
        let corner = |i: usize| {
            let pick = |bit: usize, h: f32| if i >> bit & 1 == 1 { h } else { -h };
            center + Vec3::new(pick(0, half.x), pick(1, half.y), pick(2, half.z))
        };
        [
            [0, 1, 3, 2],
            [4, 5, 7, 6],
            [0, 1, 5, 4],
            [2, 3, 7, 6],
            [0, 2, 6, 4],
            [1, 3, 7, 5],
        ]
        .iter()
        .flat_map(|&[a, b, c, d]| {
            [[a, b, c], [a, c, d]].map(|v| mapdata::Triangle {
                v: v.map(corner),
                walker: mapdata::WalkerBehaviour::BySlope,
            })
        })
        .collect()
    }

    /// The stands of a container come from the game's use test: the list is
    /// made once, nearest the box first, a bot asks for its nearest in a
    /// pose, and the press test agrees with the list.
    #[test]
    fn a_containers_stands_are_one_list_for_every_play() {
        use super::super::tests::Scenario;
        use crate::v026g::bots::view::Mode;
        let ours = Vec3::new(12.0, 0.0, 30.0);
        let middle = ours + Vec3::new(0.0, 1.0, 0.0);
        let half = Vec3::new(1.0, 0.85, 0.65);
        let body = Shape {
            kind: ShapeKind::Box,
            center: middle,
            rotation: Rotation::IDENTITY,
            half,
        };
        let world = crate::v026g::bots::tactics::tests::field_with_solids(
            &solid_box(middle, half),
            |map| {
                map.containers.push(mapdata::Container {
                    position: ours,
                    team: 0,
                    id: 0,
                    use_shapes: vec![body],
                });
            },
        );
        let mut s = Scenario::field(Mode::Batteries);
        s.bot(0, 0, Vec3::new(20.0, 0.0, 30.0));
        s.tactics = Tactics::new(world, 1.0);
        let cx = s.cx(0);
        let list = cx.use_stands(0);
        assert!(!list.is_empty(), "a box on the field has stands");
        assert!(
            std::ptr::eq(list.as_ptr(), cx.use_stands(0).as_ptr()),
            "made once"
        );
        assert!(cx.use_stands(7).is_empty(), "no such container");
        let d: Vec<f32> = list.iter().map(|st| st.feet.distance_xz(middle)).collect();
        assert!(d.windows(2).all(|w| w[0] <= w[1]), "nearest first");
        let near = cx
            .nearest_use_stand(0, UsePose::Standing, &[])
            .expect("a stand to walk to");
        assert!(near.standing);
        assert!(
            near.feet.distance_xz(cx.me.feet)
                <= list
                    .iter()
                    .map(|st| st.feet.distance_xz(cx.me.feet))
                    .fold(f32::MAX, f32::min)
                    + 1.0,
            "the bot's nearest side"
        );
        assert!(cx.use_press_ok(near.feet + Vec3::new(0.0, 1.6, 0.0), 0));
        assert!(!cx.use_press_ok(Vec3::new(40.0, 1.6, 5.0), 0), "far away");
        assert!(
            !cx.use_press_ok(near.feet + Vec3::new(0.0, 1.6, 0.0), 9),
            "no such container"
        );
    }

    const IN_ZONE: Vec3 = Vec3::new(25.0, 0.0, 32.0);

    /// Kept answers are the question's own, for each stand, side and stop
    /// (a crossing and none alike), and asked again they are read, not
    /// worked out.
    #[test]
    fn a_kept_crossing_is_the_one_worked_out() {
        let tactics = hurting(0.01);
        let memo = Memo::default();
        let asks = [
            (IN_ZONE, Some(0), 1.0),
            (IN_ZONE, Some(1), 1.0),
            (IN_ZONE, None, 0.0),
            (Vec3::new(26.0, 0.0, 33.0), Some(0), 1.0),
            // Dry floor far from the zone: still a question with an answer.
            (Vec3::new(10.0, 0.0, 10.0), Some(0), 0.0),
        ];
        assert!(crossing_to(&tactics, IN_ZONE, Some(0), 1.0).is_some());
        for round in 0..2 {
            for &(to, side, stop) in &asks {
                assert_eq!(
                    memo.crossing(&tactics, to, side, stop),
                    crossing_to(&tactics, to, side, stop),
                    "round {round}: {to:?} {side:?} {stop}"
                );
            }
            assert_eq!(memo.crossings_kept(), asks.len());
        }
    }

    /// A zone that becomes another (its control at `Hazards::attach`, its
    /// hit) drops what was kept: the answers are worked out again for it.
    #[test]
    fn a_zone_changed_drops_the_kept_crossings() {
        let mut tactics = hurting(0.01);
        let memo = Memo::default();
        let mild = memo
            .crossing(&tactics, IN_ZONE, Some(0), 1.0)
            .expect("a crossing");
        // Harsher.
        tactics.hazards.zones[0].damage = Some(mapdata::Damage {
            max_hit: 0.05,
            interval_ms: 1000,
            parts: 2,
            ..mapdata::Damage::default()
        });
        let harsh = memo.crossing(&tactics, IN_ZONE, Some(0), 1.0);
        assert_eq!(harsh, crossing_to(&tactics, IN_ZONE, Some(0), 1.0));
        assert!(
            harsh.is_some_and(|h| h.leg > mild.leg),
            "{harsh:?} {mild:?}"
        );
        assert_eq!(memo.crossings_kept(), 1);
        // Switching (a timer's, as `Hazards::attach` makes it): its floor is
        // past the zones on all match now, and so a crossing's edge may be.
        tactics.hazards.zones[0].control = Control::Timer;
        assert_eq!(
            memo.crossing(&tactics, IN_ZONE, Some(0), 1.0),
            crossing_to(&tactics, IN_ZONE, Some(0), 1.0)
        );
        assert_eq!(memo.crossings_kept(), 1);
    }

    /// The notes' `take_blocks` given the bot's candidates says what it says
    /// asking each battery afresh: offered, in a zone, too hurt for it.
    #[test]
    fn take_blocks_given_the_candidates_is_take_blocks_asked() {
        use crate::v026g::bots::plays::collect::{note_take_blocks, take_blocks};
        use crate::v026g::bots::plays::tests::Scenario;
        use crate::v026g::bots::view::{BatteryState, Mode};
        let mut s = Scenario::field(Mode::Batteries);
        s.tactics = hurting(0.01);
        s.battery(BatteryState::Free(IN_ZONE));
        s.battery(BatteryState::Free(Vec3::new(36.6, 0.0, 10.0)));
        s.battery(BatteryState::Free(Vec3::new(70.0, 0.0, 35.0)));
        for health in [1.0, 0.3, 0.05] {
            s.bot(0, 0, Vec3::new(10.0, 0.0, 10.0)).health = health;
            let cx = s.cx(0);
            let mut offered = Vec::new();
            crate::v026g::bots::plays::candidates(&cx, &mut offered);
            let (mut given, mut asked) = (s.team.clone(), s.team.clone());
            assert_eq!(
                note_take_blocks(&mut given, 0, &take_blocks(&cx, &offered)),
                note_take_blocks(&mut asked, 0, &take_blocks(&cx, &[])),
                "health {health}"
            );
            assert_eq!(given.take_blocked, asked.take_blocked, "health {health}");
            assert!(!given.take_blocked.is_empty());
        }
    }

    /// Past [`MEMO_MOST`] answers it starts afresh, and still answers right.
    #[test]
    fn a_full_memo_starts_afresh() {
        let memo: QueryMemo<usize, usize> = QueryMemo::default();
        let asked = std::cell::Cell::new(0);
        let get = |stamp: u64, k: usize| {
            memo.get([stamp].into_iter(), k, || {
                asked.set(asked.get() + 1);
                k * 2
            })
        };
        for k in 0..MEMO_MOST + 3 {
            assert_eq!(get(7, k), k * 2);
        }
        assert_eq!((memo.kept(), asked.get()), (3, MEMO_MOST + 3));
        // Kept: read.
        assert_eq!(get(7, MEMO_MOST + 1), 2 * (MEMO_MOST + 1));
        assert_eq!(asked.get(), MEMO_MOST + 3);
        // Dropped: worked out again.
        assert_eq!(get(7, 1), 2);
        assert_eq!(asked.get(), MEMO_MOST + 4);
        // Another stamp: all dropped.
        assert_eq!(get(8, MEMO_MOST + 1), 2 * (MEMO_MOST + 1));
        assert_eq!((memo.kept(), asked.get()), (1, MEMO_MOST + 5));
    }

    /// A platform a storey up (3.4 m) over the middle of the field, its
    /// only way up a ramp at the far end (10 m wide): a battery on it, a bot on the
    /// ground right under it is a long walk from it, and the straight
    /// guess said ten metres (School's battery 3).
    #[test]
    fn a_battery_a_storey_up_is_as_far_as_the_way_up() {
        use super::super::tests::Scenario;
        use crate::v026g::bots::view::Mode;
        use mapdata::{Triangle, WalkerBehaviour};
        let tri = |v: [Vec3; 3]| Triangle {
            v,
            walker: WalkerBehaviour::BySlope,
        };
        let mut solids: Vec<Triangle> = crate::v026g::bots::body::tests::floor(40.0, 0.0, 60.0, 40.0, 3.4)
            .into_iter()
            .collect();
        let (a, b, c, d) = (
            Vec3::new(78.0, 0.0, 0.0),
            Vec3::new(78.0, 0.0, 10.0),
            Vec3::new(60.0, 3.4, 10.0),
            Vec3::new(60.0, 3.4, 0.0),
        );
        solids.extend([tri([a, b, c]), tri([a, c, d])]);
        let world = crate::v026g::bots::tactics::tests::field_with_solids(&solids, |_| {});
        let mut s = Scenario::field(Mode::Batteries);
        s.bot(0, 0, Vec3::new(45.0, 0.0, 30.0));
        s.tactics = Tactics::new(world, 1.0);
        let battery = Vec3::new(50.0, 3.4, 30.0);
        let cx = s.cx(0);
        let walk = cx.storey_walk_dist(battery).expect("a storey apart");
        assert!(walk > 40.0, "the way up is the ramp's: {walk}");
        assert!(cx.path_dist(battery) < 10.0);
        // On the platform, one level: the straight guess stands.
        s.view.places[0].feet = Vec3::new(45.0, 3.4, 30.0);
        assert_eq!(s.cx(0).storey_walk_dist(battery), None);
        // On the ground away from the platform, a battery on the ground.
        s.view.places[0].feet = Vec3::new(10.0, 0.0, 10.0);
        assert_eq!(s.cx(0).storey_walk_dist(Vec3::new(20.0, 0.0, 10.0)), None);
    }
}
