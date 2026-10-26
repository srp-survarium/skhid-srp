//! `WalkGrid::search` (the 4-ary open list of 64-bit keys) against the search written out with a `std` `BinaryHeap` of
//! (cost, floor) entries ordered as `Entry` is (`total_cmp`, then the floor), on the game's own maps: the same floors in the
//! same order and the same cost to the bit. Pairs of floors drawn over each world of `SRP_MAPDATA_DIR`.
//!
//! ```text
//! SRP_MAPDATA_DIR=<mapdata-dump files> cargo test -p mapdata --release --test heap_searches -- --ignored --nocapture
//! ```

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use mapdata::{Body, MapData, NodeId, Search, WalkGrid, World};

#[derive(Clone, Copy, PartialEq)]
struct Entry(f32, NodeId);

impl Eq for Entry {}

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        other.0.total_cmp(&self.0).then(other.1.cmp(&self.1))
    }
}

fn reference(grid: &WalkGrid, from: NodeId, to: NodeId) -> Option<(Vec<NodeId>, f32)> {
    let n = grid.len();
    let goal = grid.position(to);
    let (mut cost, mut back) = (vec![f32::INFINITY; n], vec![NodeId::MAX; n]);
    let mut closed = vec![false; n];
    let mut open = BinaryHeap::new();
    cost[from as usize] = 0.0;
    open.push(Entry(grid.position(from).distance_xz(goal), from));
    while let Some(Entry(_, u)) = open.pop() {
        if closed[u as usize] {
            continue;
        }
        closed[u as usize] = true;
        if u == to {
            let mut nodes = vec![to];
            while *nodes.last().unwrap() != from {
                nodes.push(back[*nodes.last().unwrap() as usize]);
            }
            nodes.reverse();
            return Some((nodes, cost[to as usize]));
        }
        for (v, kind) in grid.links(u) {
            if closed[v as usize] {
                continue;
            }
            let c = cost[u as usize] + grid.link_cost(u, v, kind);
            if !c.is_finite() {
                continue;
            }
            if c < cost[v as usize] {
                cost[v as usize] = c;
                back[v as usize] = u;
                open.push(Entry(c + grid.position(v).distance_xz(goal), v));
            }
        }
    }
    None
}

#[test]
#[ignore = "needs SRP_MAPDATA_DIR"]
fn searches_on_every_world_find_the_ways_the_binary_heap_search_finds() {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "mapdata"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no map files in {}", dir.display());
    let (mut found, mut none, mut expanded) = (0usize, 0usize, 0usize);
    for path in &files {
        let world = World::build(MapData::read(path).unwrap(), mapdata::CELL, Body::default());
        let grid = &world.grid;
        let n = grid.len() as u64;
        let mut state = 0x9E37_79B9_7F4A_7C15u64 ^ n;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..12 {
            let (a, b) = ((next() % n) as NodeId, (next() % n) as NodeId);
            let want = reference(grid, a, b);
            match (grid.search(a, b, |_| true, usize::MAX), want) {
                (Search::Found(path), Some((nodes, cost))) => {
                    assert_eq!(path.nodes, nodes, "{}: {a} to {b}", world.map.project);
                    assert_eq!(path.cost.to_bits(), cost.to_bits(), "{}: {a} to {b}", world.map.project);
                    found += 1;
                    expanded += path.nodes.len();
                }
                (Search::NoWay, None) => none += 1,
                (got, want) => panic!("{}: {a} to {b}: {got:?} against {:?}", world.map.project, want.map(|w| w.1)),
            }
        }
    }
    eprintln!("{} worlds, {found} ways found ({expanded} floors on them) and {none} with none, all as the binary heap's", files.len());
    assert!(found > 100);
}
