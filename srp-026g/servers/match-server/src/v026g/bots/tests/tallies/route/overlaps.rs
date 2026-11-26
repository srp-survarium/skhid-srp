//! [route spreading] How much of teammates' routes ahead is the same ground, once a world (1 s): for each pair of living
//! teammates that both walk a route, the floors of the next [`AHEAD_M`] of each route (sampled every metre), and the share
//! of the shorter set that the other's holds too. The routing items (route reuse, the search's heuristic, the per-poll
//! budget) must not raise it: the team's spreading is drawn at the plays (the goals), and a search that picked a route a
//! mate already walks would show here.

use super::*;
use std::collections::{BTreeMap, BTreeSet};

/// How far ahead along each route the floors are taken (m).
const AHEAD_M: f32 = 60.0;
/// A route with fewer floors ahead than this is near its end: left out (every pair near a shared goal would count).
const MIN_FLOORS: usize = 5;

/// One pair's counts: pair-seconds, of them sharing half or more of the floors, four fifths or more, half or more with
/// the same goal (ends within 2 m), half or more starting together (feet within 5 m); and the shares' sum (thousandths).
#[derive(Default, Clone, Copy)]
struct Pairs {
    seconds: u64,
    half: u64,
    most: u64,
    half_same_goal: u64,
    half_near: u64,
    share_milli: u64,
}

impl Pairs {
    fn add(&mut self, o: &Pairs) {
        self.seconds += o.seconds;
        self.half += o.half;
        self.most += o.most;
        self.half_same_goal += o.half_same_goal;
        self.half_near += o.half_near;
        self.share_milli += o.share_milli;
    }

    fn words(&self) -> String {
        format!(
            "pair_s {} ge50 {} ge80 {} ge50_same_goal {} ge50_near_start {} mean {:.3}",
            self.seconds,
            self.half,
            self.most,
            self.half_same_goal,
            self.half_near,
            self.share_milli as f64 / 1000.0 / self.seconds.max(1) as f64
        )
    }
}

#[derive(Default)]
pub(super) struct Overlaps {
    /// By the pair's plays (sorted, `a+b`).
    by_play: BTreeMap<String, Pairs>,
    /// Bot-seconds with a route ahead, and of them with a mate sharing half or more of it.
    bot_seconds: u64,
    shared_seconds: u64,
}

struct Ahead {
    team: u8,
    play: String,
    floors: BTreeSet<u32>,
    end: Vec3,
    feet: Vec3,
}

impl RouteTopic for Overlaps {
    fn world(&mut self, _run: &RouteRun, game: &Game, bots: &Bots) {
        let Some(tactics) = bots.tactics.as_ref() else {
            return;
        };
        let grid = &tactics.world.grid;
        let mut ahead = Vec::new();
        for (place, body) in game.bodies.iter().enumerate() {
            let Some(bot) = bots.bots[place].as_ref() else { continue };
            if !body.alive {
                continue;
            }
            let Some((route, _)) = bot.route.as_ref() else { continue };
            let (from, _) = route.clone().progress(body.feet);
            let to = route.length().min(from + AHEAD_M);
            let mut floors = BTreeSet::new();
            let mut s = from;
            while s <= to {
                if let Some(node) = grid.locate(route.point_at(s), 1.0) {
                    floors.insert(node);
                }
                s += 1.0;
            }
            if floors.len() < MIN_FLOORS {
                continue;
            }
            let play = bot.assignment.as_ref().map_or("none".to_owned(), |a| format!("{:?}", a.kind));
            ahead.push(Ahead { team: body.team, play, floors, end: route.end(), feet: body.feet });
        }
        let mut shared = vec![false; ahead.len()];
        for i in 0..ahead.len() {
            for j in i + 1..ahead.len() {
                let (a, b) = (&ahead[i], &ahead[j]);
                if a.team != b.team {
                    continue;
                }
                let common = a.floors.intersection(&b.floors).count();
                let share = common as f32 / a.floors.len().min(b.floors.len()) as f32;
                let mut plays = [a.play.as_str(), b.play.as_str()];
                plays.sort_unstable();
                let row = self.by_play.entry(format!("{}+{}", plays[0], plays[1])).or_default();
                row.seconds += 1;
                row.share_milli += (share * 1000.0) as u64;
                if share >= 0.5 {
                    row.half += 1;
                    row.half_same_goal += u64::from(a.end.distance_xz(b.end) < 2.0);
                    row.half_near += u64::from(a.feet.distance_xz(b.feet) < 5.0);
                    shared[i] = true;
                    shared[j] = true;
                }
                row.most += u64::from(share >= 0.8);
            }
        }
        self.bot_seconds += ahead.len() as u64;
        self.shared_seconds += shared.iter().filter(|&&s| s).count() as u64;
    }

    fn line(&mut self, _run: &RouteRun, line: &mut Line) {
        let mut all = Pairs::default();
        for (play, row) in &self.by_play {
            all.add(row);
            eprintln!("route overlap by plays {play}: {}", row.words());
        }
        line.tail.push(format!(
            "route_overlap {} bot_s_route {} bot_s_shared50 {}",
            all.words(),
            self.bot_seconds,
            self.shared_seconds
        ));
    }
}
