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
use std::sync::{Mutex, PoisonError};

use mapdata::Vec3;

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

    /// Crossings kept.
    pub fn crossings_kept(&self) -> usize {
        self.crossings.kept()
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
}
