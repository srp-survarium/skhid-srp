//! [route spreading] Whether bots take the same way again and again: each route a bot takes is one choice, kept with the corridor it
//! passes, and at the match end the choices toward the same place are counted by corridor: how many distinct corridors, the most used
//! one's share, and the entropy of the choices. Two corridors are counted: the ordered map regions the route's floors lie in
//! (`mapdata::regions`, the corridor the search is kept in), and the coarser ordered 15 m cells of its points, which only a really
//! other way changes. Logging only. The routing items (route reuse, the search's heuristic, the per-poll budget) must not lower it.

use super::*;
use std::collections::{BTreeMap, BTreeSet};

/// A cell of the coarse corridor and of the choices' keys (m).
const CELL_M: f32 = 15.0;
/// The routes' points are taken every metre.
const STEP_M: f32 = 1.0;
/// A route whose end moved less than this is the same choice (a replan toward the same place).
const SAME_END_M: f32 = 2.0;
/// A key with fewer choices than this says nothing of a spread: left out.
const MIN_CHOICES: usize = 3;
/// Two region corridors whose region sets share this much (Jaccard) are one way in the merged count.
const MERGE_JACCARD: f64 = 0.8;

type Cell = (i32, i32);

fn cell(p: Vec3) -> Cell {
    ((p.x / CELL_M).floor() as i32, (p.z / CELL_M).floor() as i32)
}

/// One route choice.
struct Choice {
    team: u8,
    /// The first route of a life (from the spawn).
    first: bool,
    start: Cell,
    end: Cell,
    regions: Vec<u32>,
    cells: Vec<Cell>,
}

/// What the topic last saw of a place: its route's end, and whether its body died since.
#[derive(Clone, Copy)]
struct Seen {
    end: Vec3,
    died: bool,
}

#[derive(Default)]
pub(super) struct Corridors {
    choices: Vec<Choice>,
    seen: Vec<Option<Seen>>,
}

/// The counts over the keys with [`MIN_CHOICES`] or more, weighted by choices.
#[derive(Default)]
struct Spread {
    keys: usize,
    choices: usize,
    distinct: usize,
    top: usize,
    entropy_bits: f64,
    entropy_norm: f64,
}

impl Spread {
    fn add<T: Ord>(&mut self, list: &[T]) {
        let n = list.len();
        let mut counts: BTreeMap<&T, usize> = BTreeMap::new();
        for c in list {
            *counts.entry(c).or_default() += 1;
        }
        let counts: Vec<usize> = counts.into_values().collect();
        self.add_counts(&counts, n);
    }

    fn add_counts(&mut self, counts: &[usize], n: usize) {
        let e = entropy(counts, n);
        self.keys += 1;
        self.choices += n;
        self.distinct += counts.len();
        self.top += counts.iter().copied().max().unwrap_or(0);
        self.entropy_bits += e * n as f64;
        self.entropy_norm += e / (n as f64).log2() * n as f64;
    }

    fn words(&self, name: &str) -> String {
        let w = self.choices.max(1) as f64;
        format!(
            "{name}_keys {} {name}_choices {} {name}_distinct {} {name}_top {:.3} {name}_bits {:.3} {name}_norm {:.3}",
            self.keys,
            self.choices,
            self.distinct,
            self.top as f64 / w,
            self.entropy_bits / w,
            self.entropy_norm / w
        )
    }
}

fn entropy(counts: &[usize], n: usize) -> f64 {
    counts
        .iter()
        .map(|&c| {
            let p = c as f64 / n as f64;
            -p * p.log2()
        })
        .sum()
}

/// The region corridors joined greedily, most used first, where their region sets share [`MERGE_JACCARD`] or more: the counts.
fn merged(list: &[&Vec<u32>]) -> Vec<usize> {
    let mut counts: BTreeMap<&Vec<u32>, usize> = BTreeMap::new();
    for c in list {
        *counts.entry(*c).or_default() += 1;
    }
    let mut by: Vec<(&Vec<u32>, usize)> = counts.into_iter().collect();
    by.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    let mut heads: Vec<(BTreeSet<u32>, usize)> = Vec::new();
    for (c, k) in by {
        let set: BTreeSet<u32> = c.iter().copied().collect();
        match heads.iter_mut().find(|(h, _)| h.intersection(&set).count() as f64 / h.union(&set).count() as f64 >= MERGE_JACCARD) {
            Some(h) => h.1 += k,
            None => heads.push((set, k)),
        }
    }
    heads.into_iter().map(|h| h.1).collect()
}

impl RouteTopic for Corridors {
    fn world(&mut self, _run: &RouteRun, game: &Game, bots: &Bots) {
        let Some(tactics) = bots.tactics.as_ref() else {
            return;
        };
        let (grid, regions) = (&tactics.world.grid, tactics.regions());
        if self.seen.len() < game.bodies.len() {
            self.seen.resize(game.bodies.len(), None);
        }
        for (place, body) in game.bodies.iter().enumerate() {
            if bots.bots[place].is_none() {
                continue;
            }
            if !body.alive {
                if let Some(seen) = self.seen[place].as_mut() {
                    seen.died = true;
                }
                continue;
            }
            let Some((route, _)) = bots.bots[place].as_ref().and_then(|bot| bot.route.as_ref()) else { continue };
            let end = route.end();
            let first = self.seen[place].is_none_or(|seen| seen.died);
            if !first && self.seen[place].is_some_and(|seen| seen.end.distance_xz(end) <= SAME_END_M) {
                continue;
            }
            self.seen[place] = Some(Seen { end, died: false });
            let (mut by_region, mut by_cell): (Vec<u32>, Vec<Cell>) = (Vec::new(), Vec::new());
            let mut s = 0.0;
            while s <= route.length() {
                let p = route.point_at(s);
                if let Some(node) = grid.locate(p, 1.0) {
                    let r = regions.of(node);
                    if by_region.last() != Some(&r) {
                        by_region.push(r);
                    }
                }
                let c = cell(p);
                if by_cell.last() != Some(&c) {
                    by_cell.push(c);
                }
                s += STEP_M;
            }
            if by_region.len() < 2 {
                continue;
            }
            self.choices.push(Choice {
                team: body.team,
                first,
                start: cell(route.point_at(0.0)),
                end: cell(end),
                regions: by_region,
                cells: by_cell,
            });
        }
    }

    fn line(&mut self, _run: &RouteRun, line: &mut Line) {
        // The keys: a life's first route by (team, end cell); every route by (team, start cell, end cell).
        let mut spawn: BTreeMap<(u8, Cell), Vec<&Choice>> = BTreeMap::new();
        let mut all: BTreeMap<(u8, Cell, Cell), Vec<&Choice>> = BTreeMap::new();
        for c in &self.choices {
            if c.first {
                spawn.entry((c.team, c.end)).or_default().push(c);
            }
            all.entry((c.team, c.start, c.end)).or_default().push(c);
        }
        let mut words = vec![format!(
            "choices {} firsts {}",
            self.choices.len(),
            self.choices.iter().filter(|c| c.first).count()
        )];
        for (name, groups) in [("spawn", spawn.into_values().collect::<Vec<_>>()), ("all", all.into_values().collect())] {
            let (mut regions, mut merged_regions, mut cells) = (Spread::default(), Spread::default(), Spread::default());
            for list in groups.iter().filter(|l| l.len() >= MIN_CHOICES) {
                let rs: Vec<&Vec<u32>> = list.iter().map(|c| &c.regions).collect();
                regions.add(&rs);
                merged_regions.add_counts(&merged(&rs), list.len());
                cells.add(&list.iter().map(|c| &c.cells).collect::<Vec<_>>());
            }
            words.push(regions.words(&format!("{name}_reg")));
            words.push(merged_regions.words(&format!("{name}_merged")));
            words.push(cells.words(&format!("{name}_cell")));
        }
        let text = words.join(" ");
        eprintln!("route corridor since the last match end: {text}");
        line.tail.push(format!("route_corridor {text}"));
        self.choices.clear();
        self.seen.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spread_counts_each_corridor_once() {
        let mut s = Spread::default();
        s.add(&["a", "a", "b", "c"]);
        assert_eq!((s.keys, s.choices, s.distinct, s.top), (1, 4, 3, 2));
        assert!((s.entropy_bits / 4.0 - 1.5).abs() < 1e-9);
        let one = vec![1u32, 2, 3, 4, 5];
        let near = vec![1u32, 2, 3, 4, 5, 6];
        let other = vec![7u32, 8];
        assert_eq!(merged(&[&one, &near, &other, &one]), vec![3, 1]);
    }
}
