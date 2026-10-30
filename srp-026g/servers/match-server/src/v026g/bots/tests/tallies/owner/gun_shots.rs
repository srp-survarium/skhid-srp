//! The owner counts of a gun's shots, by the gun and by who fires it (the stand-in camper, A182, or a bot), to read the bench's
//! hits per shot against the live humans' (`hit_marks.py`: the statistics counters' fire and hit per 5 s interval whose fire ticks
//! all aim within 10 degrees of an enemy), and to find why they differ: where the shot goes (the distance of the target, the
//! aim's state, the cone the spread gives, how far the line is off his chest), whether the shooter or the target moves, and the
//! rhythm (the length of a burst, the pause between two). A shot is counted when it has a target ([`ShotFact::target`]) and its
//! line is within 10 degrees of his chest (the humans' view test); `shots_all` counts every shot at a target.

use std::any::Any;
use std::collections::BTreeMap;

use super::*;
use crate::v026g::bots::tests::tallies::fight_tally::stance;

/// The live humans' distance bands, metres (upper edges; the last takes the rest): 0-20, 20-40, 40-65, 65-80, 80-105, 105+.
const BANDS_M: [f32; 5] = [20.0, 40.0, 65.0, 80.0, 105.0];
/// The shot's line is on the target when within this of his chest, degrees (the humans' test on the view).
const ON_DEG: f32 = 10.0;
/// A shot follows the one before in the same burst when within this many of the gun's cycles (and at least [`MIN_GAP_MS`]).
const CYCLES: u32 = 2;
const MIN_GAP_MS: u32 = 150;
/// The burst lengths binned: 1, 2-3, 4-6, 7-10, 11 and more shots.
const BURST_BINS: [u32; 4] = [1, 3, 6, 10];
/// The pauses between bursts binned, ms (upper edges): 0.3 s, 1 s, 3 s, more.
const PAUSE_BINS_MS: [u32; 3] = [300, 1_000, 3_000];

#[derive(Default, Clone)]
struct Acc {
    shots_all: u32,
    /// On the target's chest line (within [`ON_DEG`]): shots and hits (a bullet landed on an enemy).
    shots: u32,
    hits: u32,
    by_band: [[u32; 2]; 6],
    /// Hip, aiming in, aimed in.
    by_aim: [[u32; 2]; 3],
    /// The shooter holding a move key / not; the target holding one / not.
    by_shooter_moving: [[u32; 2]; 2],
    by_target_moving: [[u32; 2]; 2],
    by_crouch: [[u32; 2]; 2],
    /// The cone of the spread, degrees, summed over the shots, and how far the line was off the chest.
    cone_sum: f64,
    off_sum: f64,
    /// By the burst the shot was in (its length): bursts, shots, hits.
    by_burst: [[u32; 3]; 5],
    /// Pauses between bursts, by [`PAUSE_BINS_MS`].
    pauses: [u32; 4],
}

/// A shooter's burst so far: its gun, the tick of its last shot, shots and hits in it, and the bin keys.
#[derive(Clone)]
struct Burst {
    key: String,
    last: u32,
    shots: u32,
    hits: u32,
}

pub(super) struct GunShots {
    groups: BTreeMap<String, Acc>,
    open: Vec<Option<Burst>>,
}

impl GunShots {
    pub(super) fn new(n: usize) -> Self {
        Self { groups: BTreeMap::new(), open: vec![None; n] }
    }

    fn close(&mut self, place: Place) {
        let Some(b) = self.open[place].take() else { return };
        let bin = BURST_BINS.iter().position(|&edge| b.shots <= edge).unwrap_or(BURST_BINS.len());
        let cell = &mut self.groups.entry(b.key).or_default().by_burst[bin];
        cell[0] += 1;
        cell[1] += b.shots;
        cell[2] += b.hits;
    }
}

impl Topic for GunShots {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn begin(&mut self, run: &Run, game: &mut Game) {
        let facts: Vec<_> = game.shot_facts.clone();
        for fact in facts {
            let Some(body) = game.bodies.get(fact.shooter) else { continue };
            let who = if run.camper == Some(fact.shooter) { "camper" } else { "bots" };
            let key = format!("{}/{who}", body.gun.item);
            let hit = fact.landed.on_enemy > 0;
            // The burst: this shot continues the shooter's last when it came soon enough.
            let gap_ms = (CYCLES * 60_000 / body.gun.rpm.max(1)).max(MIN_GAP_MS);
            let (shooter, tick) = (fact.shooter, fact.tick);
            if let Some(b) = &self.open[shooter] {
                let gap = tick.saturating_sub(b.last);
                if b.key != key || gap > gap_ms {
                    if b.key == key {
                        let bin = PAUSE_BINS_MS.iter().position(|&edge| gap <= edge).unwrap_or(PAUSE_BINS_MS.len());
                        self.groups.entry(key.clone()).or_default().pauses[bin] += 1;
                    }
                    self.close(shooter);
                }
            }
            let b = self.open[shooter].get_or_insert(Burst { key: key.clone(), last: tick, shots: 0, hits: 0 });
            b.last = tick;
            b.shots += 1;
            b.hits += u32::from(hit);
            let Some((target, distance, off)) = fact.target else { continue };
            let a = self.groups.entry(key).or_default();
            a.shots_all += 1;
            if off > ON_DEG {
                continue;
            }
            let h = usize::from(hit);
            let tally = |cell: &mut [u32; 2]| {
                cell[0] += 1;
                cell[1] += h as u32;
            };
            a.shots += 1;
            a.hits += h as u32;
            tally(&mut a.by_band[BANDS_M.iter().position(|&edge| distance < edge).unwrap_or(BANDS_M.len())]);
            tally(&mut a.by_aim[fact.aim_state as usize]);
            tally(&mut a.by_shooter_moving[usize::from(fact.moving)]);
            tally(&mut a.by_crouch[usize::from(fact.crouched)]);
            let target_moving = game.bodies.get(target).is_some_and(|t| stance(t.keys).0);
            tally(&mut a.by_target_moving[usize::from(target_moving)]);
            a.cone_sum += f64::from(fact.cone);
            a.off_sum += f64::from(off);
        }
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        for place in 0..self.open.len() {
            self.close(place);
        }
        let mut json = serde_json::Map::new();
        let mut lines = Vec::new();
        for (key, a) in &self.groups {
            if a.shots_all == 0 {
                continue;
            }
            json.insert(
                key.clone(),
                serde_json::json!({
                    "shots_all": a.shots_all,
                    "shots": a.shots,
                    "hits": a.hits,
                    "by_band": a.by_band,
                    "by_aim": a.by_aim,
                    "by_shooter_moving": a.by_shooter_moving,
                    "by_target_moving": a.by_target_moving,
                    "by_crouch": a.by_crouch,
                    "cone_deg_sum": a.cone_sum,
                    "off_deg_sum": a.off_sum,
                    "by_burst": a.by_burst,
                    "pauses": a.pauses,
                }),
            );
            let pct = |h: u32, s: u32| if s == 0 { 0.0 } else { 100.0 * f64::from(h) / f64::from(s) };
            lines.push(format!(
                "{key}: {} shots at a target, {} on his line, {:.1} % hit (mean cone {:.2} deg, off the chest {:.2} deg)",
                a.shots_all,
                a.shots,
                pct(a.hits, a.shots),
                if a.shots == 0 { 0.0 } else { a.cone_sum / f64::from(a.shots) },
                if a.shots == 0 { 0.0 } else { a.off_sum / f64::from(a.shots) },
            ));
        }
        vec![("gun_shots", Part::new(serde_json::json!({ "gun_shots": json }), format!("owner-test gun shots: {}", lines.join("; "))))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::guns::gun;
    use crate::v026g::bots::tests::tallies::fight_tally::{Landed, ShotFact};

    fn shot(game: &Game, hit: bool) -> ShotFact {
        ShotFact {
            shooter: 0,
            tick: game.tick,
            hit: hit.then_some(1),
            target: Some((1, 30.0, 2.0)),
            moving: false,
            aimed: true,
            crouched: false,
            aim_state: Default::default(),
            cone: 0.4,
            offset: None,
            landed: Landed { fired: 1, on_enemy: u8::from(hit), parts: [0; 4] },
        }
    }

    /// Three shots of a 600 rpm rifle 100 ms apart (two hit), 2 s of quiet, then a single shot: a burst of 3 with 2 hits, a
    /// burst of 1 with none and one pause of 1-3 s, all in the group of the gun and the bots.
    #[test]
    fn a_burst_is_shots_close_together_and_a_pause_is_the_gap_between_two() {
        let (mut game, mut tally) = made_up(&[0, 1]);
        play(&mut game, &mut tally, 100, |k, game| {
            put(game, 0, 10.0, 10.0, 0);
            put(game, 1, 10.0, 40.0, 0);
            game.bodies[0].gun = gun("vsk_94").expect("a gun");
            game.shot_facts.clear();
            match k {
                20 | 24 => game.shot_facts.push(shot(game, true)),
                22 | 64 => game.shot_facts.push(shot(game, false)),
                _ => {}
            }
            vec![bot_now(); 2]
        });
        let (_, json) = tally.finish(&game);
        let g = &json["gun_shots"]["vsk_94/bots"];
        assert_eq!(g["shots"], 4, "{json}");
        assert_eq!(g["hits"], 2, "{json}");
        assert_eq!(g["by_burst"][1], serde_json::json!([1, 3, 2]), "{json}");
        assert_eq!(g["by_burst"][0], serde_json::json!([1, 1, 0]), "{json}");
        assert_eq!(g["pauses"], serde_json::json!([0, 0, 1, 0]), "{json}");
    }
}
