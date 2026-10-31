//! The owner counts of the sprint by the way ahead (A207, `feel.sprint_turns`): the two tables of the live census
//! (`traces-20261028/a199-sprint`, `a199_sprint_legs.py`) from the bench's record of every body, by the bot's team, so the bench
//! reads against the live humans' rows:
//!
//! - **by the straight way ahead:** from a tick, the distance along the path the body did walk to the first turn of more
//!   than 0.6 rad between 1 m chords, or to a stop (10 ticks under 1 m/s); bands 0-3.5 / 3.5-6 / 6-10 / 10-15 / 15-20 / 20-30 /
//!   30-50 / 50+ m. Live humans sprint 59 / 64 / 70 / 74 / 77 / 78 / 78 / 76 % of their calm ticks, the live bots 4 to 24 %.
//! - **by the sharpest turn within 3 m ahead** (between 1 m chords of the walked path): bands 0-30 / 30-60 / 60-90 / 90-135 /
//!   135-180 degrees. Live humans 71 / 65 / 57 / 45 / 33 %, the live bots 11 / 3 / 2 / 3 / 3 %.
//!
//! The ticks are the census's: every other tick (100 ms) of a living bot moving at 1.5 m/s or more (over the ticks either
//! side), forward held, not crouched, no fire or aim from 2 s before to 2 s after; the sprint is the sprint key. The bench's
//! paths are its own ticks, the live humans' were rebuilt from inputs between 5 s worlds (median 0.42 m): sharp turns
//! read a little low there.

use super::*;
use crate::v026g::bots::tests::standin_stamina;

/// The census's turn between chords that ends a straight stretch, radians.
const TURN: f32 = 0.6;
/// A stretch ends where the body is under 1 m/s for this many ticks.
const STILL_TICKS: u32 = 10;
/// A step this long in a tick is a respawn, not a walk, metres.
const JUMP_M: f32 = 3.0;
/// Fire or aim this near a tick (either side) takes it out, ticks: 2 s.
const QUIET_TICKS: usize = 40;
/// The straight way's bands, metres (upper edges; the last is open), and the turns', degrees.
const LEG_EDGES: [f32; 7] = [3.5, 6.0, 10.0, 15.0, 20.0, 30.0, 50.0];
const LEG_NAMES: [&str; 8] = ["0_3.5", "3.5_6", "6_10", "10_15", "15_20", "20_30", "30_50", "50_up"];
const TURN_EDGES: [f32; 4] = [30.0, 60.0, 90.0, 135.0];
const TURN_NAMES: [&str; 5] = ["0_30", "30_60", "60_90", "90_135", "135_180"];

/// By team: the census ticks and the sprinted ones, by the straight way's band and by the turn's.
#[derive(Default)]
pub(super) struct SprintTables {
    legs: [[[u64; 2]; 8]; 2],
    turns: [[[u64; 2]; 5]; 2],
}

impl Topic for SprintTables {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn parts(&mut self, run: &Run, game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        for place in 0..run.recs.len() {
            if run.is_bot[place] {
                let team = usize::from(game.bodies[place].team.min(1));
                self.count(&run.recs[place], team);
            }
        }
        vec![("sprint_tables", self.part(game))]
    }
}

impl SprintTables {
    /// The census ticks of one body's record, by stretches of living ticks with no respawn in them.
    fn count(&mut self, recs: &[Rec], team: usize) {
        let quiet = quiet_ticks(recs);
        let mut start = 0;
        while start < recs.len() {
            let mut end = start;
            while end < recs.len() && recs[end].alive && (end == start || recs[end].feet.distance_xz(recs[end - 1].feet) < JUMP_M) {
                end += 1;
            }
            if end > start + 2 {
                let path: Vec<(f32, f32)> = recs[start..end].iter().map(|r| (r.feet.x, r.feet.z)).collect();
                for i in (1..path.len() - 1).step_by(2) {
                    let rec = &recs[start + i];
                    let (a, b) = (path[i - 1], path[i + 1]);
                    let speed = (b.0 - a.0).hypot(b.1 - a.1) / (2.0 * TICK_S);
                    if speed < 1.5 || rec.keys & key::FORWARD == 0 || rec.keys & key::CROUCH != 0 || !quiet[start + i] {
                        continue;
                    }
                    let sprint = u64::from(rec.keys & key::SPRINT != 0);
                    let leg = LEG_EDGES.iter().take_while(|&&edge| straight_ahead(&path, i) >= edge).count();
                    let turn = TURN_EDGES.iter().take_while(|&&edge| turn_ahead(&path, i, 3.0).to_degrees() >= edge).count();
                    self.legs[team][leg][0] += 1;
                    self.legs[team][leg][1] += sprint;
                    self.turns[team][turn][0] += 1;
                    self.turns[team][turn][1] += sprint;
                }
            }
            start = end.max(start + 1);
        }
    }

    /// The two tables by team, and each team's stamina lockouts (the guard: more sprint spends more of the bar).
    fn part(&self, game: &Game) -> Part {
        let mut json = serde_json::Map::new();
        let mut text = Vec::new();
        for team in 0..2 {
            let bar = standin_stamina::counts_of(game.bodies.iter().filter(|b| usize::from(b.team.min(1)) == team).map(|b| &b.stamina));
            json.insert(format!("sprint_t{team}_stamina_lockouts"), bar.lockouts.into());
            json.insert(format!("sprint_t{team}_stamina_low_ticks"), bar.low_ticks.into());
            for (band, name) in LEG_NAMES.iter().enumerate() {
                let [n, s] = self.legs[team][band];
                json.insert(format!("sprint_legs_t{team}_{name}_ticks"), n.into());
                json.insert(format!("sprint_legs_t{team}_{name}_sprint"), s.into());
            }
            for (band, name) in TURN_NAMES.iter().enumerate() {
                let [n, s] = self.turns[team][band];
                json.insert(format!("sprint_turns_t{team}_{name}_ticks"), n.into());
                json.insert(format!("sprint_turns_t{team}_{name}_sprint"), s.into());
            }
            let shares = |rows: &[[u64; 2]], names: &[&str]| {
                rows.iter()
                    .zip(names)
                    .map(|([n, s], name)| format!("{name} {:.0} % of {n}", Scale::pct(*s as f64, *n as f64)))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            text.push(format!(
                "team {team}: by the straight way ahead (m) {}; by the turn within 3 m (deg) {}; stamina lockouts {}",
                shares(&self.legs[team], &LEG_NAMES),
                shares(&self.turns[team], &TURN_NAMES),
                bar.lockouts
            ));
        }
        Part::new(serde_json::Value::Object(json), format!("owner-test sprint tables: {}", text.join("; ")))
    }
}

/// Whether each tick has no fire or aim within [`QUIET_TICKS`] either side.
fn quiet_ticks(recs: &[Rec]) -> Vec<bool> {
    let loud: Vec<usize> = recs.iter().enumerate().filter(|(_, r)| r.keys & (key::FIRE | key::AIM) != 0).map(|(k, _)| k).collect();
    (0..recs.len())
        .map(|k| {
            let first = loud.partition_point(|&at| at + QUIET_TICKS < k);
            loud.get(first).is_none_or(|&at| at > k + QUIET_TICKS)
        })
        .collect()
}

/// The heading change between the chords `a`-`b` and `b`-`c`, radians.
fn chord_turn(a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> f32 {
    let (h1, h2) = ((b.0 - a.0).atan2(b.1 - a.1), (c.0 - b.0).atan2(c.1 - b.1));
    wrap(h2 - h1).abs()
}

/// The census's straight way ahead of tick `i` along `path`, metres: to the first turn over [`TURN`] between 1 m chords,
/// to a stop, or to the stretch's end (at most a little over 60 m).
fn straight_ahead(path: &[(f32, f32)], i: usize) -> f32 {
    let mut chords = vec![path[i]];
    let (mut dist, mut still) = (0.0f32, 0u32);
    for j in i + 1..path.len() {
        let (p, q) = (path[j - 1], path[j]);
        let step = (q.0 - p.0).hypot(q.1 - p.1);
        still = if step < 0.05 { still + 1 } else { 0 };
        if still >= STILL_TICKS {
            return dist;
        }
        let last = chords[chords.len() - 1];
        if (q.0 - last.0).hypot(q.1 - last.1) >= 1.0 {
            chords.push(q);
            let n = chords.len();
            if n >= 3 && chord_turn(chords[n - 3], chords[n - 2], chords[n - 1]) > TURN {
                return dist;
            }
        }
        dist += step;
        if dist > 60.0 {
            return dist;
        }
    }
    dist
}

/// The census's sharpest heading change between 1 m chords of `path` within `within` metres ahead of tick `i`, radians.
fn turn_ahead(path: &[(f32, f32)], i: usize, within: f32) -> f32 {
    let mut chords = vec![path[i]];
    let (mut dist, mut most) = (0.0f32, 0.0f32);
    let mut j = i + 1;
    while j < path.len() && dist < within + 1.0 {
        let (p, q) = (path[j - 1], path[j]);
        dist += (q.0 - p.0).hypot(q.1 - p.1);
        let last = chords[chords.len() - 1];
        if (q.0 - last.0).hypot(q.1 - last.1) >= 1.0 {
            chords.push(q);
            let n = chords.len();
            if n >= 3 {
                most = most.max(chord_turn(chords[n - 3], chords[n - 2], chords[n - 1]));
            }
        }
        j += 1;
    }
    most
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A path of 50 ms ticks at `speed` along straight legs between `corners`.
    fn walked(corners: &[(f32, f32)], speed: f32) -> Vec<(f32, f32)> {
        let step = speed * TICK_S;
        let mut path = vec![corners[0]];
        for w in corners.windows(2) {
            let (a, b) = (w[0], w[1]);
            let len = (b.0 - a.0).hypot(b.1 - a.1);
            let n = (len / step).round().max(1.0) as usize;
            for k in 1..=n {
                let t = k as f32 / n as f32;
                path.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
            }
        }
        path
    }

    /// The census's measures on a known path: 8 m to a right-angle corner, then a long leg.
    #[test]
    fn the_straight_way_and_the_turn_ahead_are_the_census_s() {
        let path = walked(&[(0.0, 0.0), (8.0, 0.0), (8.0, 30.0)], 4.0);
        let straight = straight_ahead(&path, 0);
        assert!((7.0..=9.5).contains(&straight), "{straight}");
        assert!(turn_ahead(&path, 0, 3.0) < 0.1, "no turn within 3 m of the start");
        let near = path.iter().position(|p| p.0 >= 6.5).unwrap();
        assert!(turn_ahead(&path, near, 3.0).to_degrees() > 60.0, "{}", turn_ahead(&path, near, 3.0).to_degrees());
        let after = path.iter().position(|p| p.1 >= 2.0).unwrap();
        assert!(straight_ahead(&path, after) > 25.0);
    }

    /// Fire or aim takes out the ticks within 2 s of it, and only those.
    #[test]
    fn fire_takes_out_the_ticks_within_two_seconds() {
        let mut recs = vec![Rec::default(); 200];
        recs[100].keys = key::FIRE;
        let quiet = quiet_ticks(&recs);
        assert!(quiet[59] && !quiet[60] && !quiet[140] && quiet[141]);
    }
}
