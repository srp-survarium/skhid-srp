//! The walk grid's table per map: what a body gets to from the spawns and
//! back, the pockets, the objectives out of reach and the jumps by height.
//! The before and after of every change to the grid's build
//! (`docs/notes/player_bots_mapdata.md`, "Grid: pockets and jumps").
//!
//! ```text
//! SRP_MAPDATA_DIR=<mapdata-dump files> cargo test -p mapdata --release --test grid_table -- --ignored --nocapture
//! ```
//!
//! `SRP_GRID_TABLE_OUT=<file>` also writes the table there (tab-separated);
//! `SRP_GRID_TABLE_MAPS=level_01_day_2,...` picks the projects (default:
//! every level's `day_2` and `day_4`).
//!
//! The pockets are found here from the grid's links alone, so the table
//! reads the same on a grid of any build; the grid's own marks
//! ([`WalkGrid::in_pocket`]) must agree.

use mapdata::{Body, MapData, NodeId, Vec3, WalkGrid};

/// A spawn's floor, as the bots' tactics find it (`walkable_from_spawns`).
const SPAWN_M: f32 = 1.5;
/// An objective's floor (the grid's seeds).
const OBJECTIVE_M: f32 = 3.0;
/// The client's jump (PROVEN, the jump study) and its sprint jump.
const JUMP_M: f32 = 0.998;
const SPRINT_JUMP_M: f32 = 1.35;
/// A pocket's way out: a floor that returns, within this across.
const EXIT_ACROSS_M: f32 = 1.0;
const EXIT_LOW_M: f32 = 0.2;

#[derive(Default)]
struct Row {
    floors: usize,
    reached: usize,
    returns: usize,
    pocket_floors: usize,
    pockets: usize,
    /// Pockets of 4 floors or more, by their lowest way out: within the
    /// jump, within the sprint jump, none.
    big_pockets: [usize; 3],
    objectives: usize,
    objectives_unreached: usize,
    objectives_in_pocket: usize,
    jumps: usize,
    /// Jump links by rise: up to 0.95, 0.95-0.998, 0.998-1.05 m.
    jumps_by_rise: [usize; 3],
    /// Jump links landing 3 or 4 cells along their direction.
    far_jumps: usize,
}

/// The largest strongly connected set of the `reached` floors, by
/// Tarjan's algorithm (iterative).
fn largest_component(grid: &WalkGrid, reached: &[bool]) -> Vec<NodeId> {
    let n = grid.len();
    let mut index = vec![u32::MAX; n];
    let mut low = vec![0u32; n];
    let mut on_stack = vec![false; n];
    let mut stack: Vec<NodeId> = Vec::new();
    let mut next = 0u32;
    let mut best: Vec<NodeId> = Vec::new();
    for root in 0..n as NodeId {
        if !reached[root as usize] || index[root as usize] != u32::MAX {
            continue;
        }
        // (floor, its links, the next one to look at)
        let mut calls: Vec<(NodeId, Vec<NodeId>, usize)> = Vec::new();
        let open = |a: NodeId, index: &mut [u32], low: &mut [u32], next: &mut u32| {
            index[a as usize] = *next;
            low[a as usize] = *next;
            *next += 1;
            let links: Vec<NodeId> = grid
                .links(a)
                .map(|(b, _)| b)
                .filter(|&b| reached[b as usize])
                .collect();
            (a, links, 0)
        };
        calls.push(open(root, &mut index, &mut low, &mut next));
        stack.push(root);
        on_stack[root as usize] = true;
        while let Some((a, links, i)) = calls.last_mut() {
            let a = *a;
            if *i < links.len() {
                let b = links[*i];
                *i += 1;
                if index[b as usize] == u32::MAX {
                    calls.push(open(b, &mut index, &mut low, &mut next));
                    stack.push(b);
                    on_stack[b as usize] = true;
                } else if on_stack[b as usize] {
                    low[a as usize] = low[a as usize].min(index[b as usize]);
                }
                continue;
            }
            calls.pop();
            if let Some((parent, _, _)) = calls.last() {
                let p = *parent as usize;
                low[p] = low[p].min(low[a as usize]);
            }
            if low[a as usize] == index[a as usize] {
                let mut component = Vec::new();
                loop {
                    let b = stack.pop().unwrap();
                    on_stack[b as usize] = false;
                    component.push(b);
                    if b == a {
                        break;
                    }
                }
                if component.len() > best.len() {
                    best = component;
                }
            }
        }
    }
    best
}

fn table_row(map: &MapData, grid: &WalkGrid) -> Row {
    let n = grid.len();
    let mut row = Row {
        floors: n,
        ..Row::default()
    };
    // Forward from the spawns' floors, and backward to them.
    let spawns: Vec<NodeId> = map
        .spawns
        .iter()
        .filter_map(|s| grid.locate(s.position, SPAWN_M))
        .collect();
    let mut into: Vec<Vec<NodeId>> = vec![Vec::new(); n];
    for a in 0..n as NodeId {
        for (b, kind) in grid.links(a) {
            into[b as usize].push(a);
            if kind == mapdata::LinkKind::Jump {
                let (pa, pb) = (grid.position(a), grid.position(b));
                let rise = pb.y - pa.y;
                row.jumps += 1;
                row.jumps_by_rise[if rise <= 0.95 {
                    0
                } else if rise <= JUMP_M {
                    1
                } else {
                    2
                }] += 1;
                let cells = (pa.x - pb.x).abs().max((pa.z - pb.z).abs()) / grid.cell_size();
                if cells > 2.5 {
                    row.far_jumps += 1;
                }
            }
        }
    }
    let flood = |next: &dyn Fn(NodeId, &mut Vec<NodeId>)| {
        let mut seen = vec![spawns.is_empty(); n];
        let mut queue = Vec::new();
        for &s in &spawns {
            if !seen[s as usize] {
                seen[s as usize] = true;
                queue.push(s);
            }
        }
        let mut out = Vec::new();
        while let Some(a) = queue.pop() {
            out.clear();
            next(a, &mut out);
            for &b in &out {
                if !seen[b as usize] {
                    seen[b as usize] = true;
                    queue.push(b);
                }
            }
        }
        seen
    };
    let reached = flood(&|a, out| out.extend(grid.links(a).map(|(b, _)| b)));
    // The main area: the largest strongly connected set of reached floors
    // (Tarjan's, the grid uses Kosaraju's); returns: walks back into it.
    let main = largest_component(grid, &reached);
    let mut returns = vec![spawns.is_empty(); n];
    let mut queue: Vec<NodeId> = main.clone();
    for &a in &main {
        returns[a as usize] = true;
    }
    while let Some(a) = queue.pop() {
        for &b in &into[a as usize] {
            if !returns[b as usize] {
                returns[b as usize] = true;
                queue.push(b);
            }
        }
    }
    let pocket = |a: NodeId| reached[a as usize] && !returns[a as usize];
    row.reached = reached.iter().filter(|&&r| r).count();
    row.returns = returns.iter().filter(|&&r| r).count();
    // The pockets: pocket floors joined by links either way.
    let mut label = vec![u32::MAX; n];
    for start in 0..n as NodeId {
        if !pocket(start) || label[start as usize] != u32::MAX {
            continue;
        }
        let id = row.pockets as u32;
        row.pockets += 1;
        let mut members = vec![start];
        label[start as usize] = id;
        let mut i = 0;
        while i < members.len() {
            let a = members[i];
            i += 1;
            let near: Vec<NodeId> = grid
                .links(a)
                .map(|(b, _)| b)
                .chain(into[a as usize].iter().copied())
                .collect();
            for b in near {
                if pocket(b) && label[b as usize] == u32::MAX {
                    label[b as usize] = id;
                    members.push(b);
                }
            }
        }
        row.pocket_floors += members.len();
        if members.len() < 4 {
            continue;
        }
        // Its lowest way out by a jump to a floor that returns.
        let mut lowest = f32::INFINITY;
        for &a in &members {
            let at = grid.position(a);
            let r = EXIT_ACROSS_M;
            for b in grid.floors_in([at.x - r, at.z - r, at.x + r, at.z + r]) {
                let to = grid.position(b);
                let rise = to.y - at.y;
                if returns[b as usize]
                    && grid.can_stand(b)
                    && to.distance_xz(at) <= EXIT_ACROSS_M
                    && rise >= EXIT_LOW_M
                {
                    lowest = lowest.min(rise);
                }
            }
        }
        row.big_pockets[if lowest <= JUMP_M {
            0
        } else if lowest <= SPRINT_JUMP_M {
            1
        } else {
            2
        }] += 1;
    }
    // The objectives: batteries, containers, stations.
    let objectives: Vec<Vec3> = map
        .battery_spawners
        .iter()
        .map(|b| b.item)
        .chain(map.containers.iter().map(|c| c.position))
        .chain(map.stations.iter().copied())
        .collect();
    row.objectives = objectives.len();
    for at in objectives {
        match grid.locate(at, OBJECTIVE_M) {
            Some(f) if pocket(f) => row.objectives_in_pocket += 1,
            Some(f) if reached[f as usize] => {}
            _ => row.objectives_unreached += 1,
        }
    }
    // The grid's own marks agree.
    for a in 0..n as NodeId {
        assert_eq!(
            grid.in_pocket(a),
            pocket(a),
            "{}: floor {a} at {:?}",
            map.project,
            grid.position(a)
        );
        assert_eq!(grid.reached(a), reached[a as usize], "{}", map.project);
        assert_eq!(grid.returns(a), returns[a as usize], "{}", map.project);
        assert_eq!(
            grid.pocket_of(a).is_some(),
            pocket(a),
            "{}: floor {a}",
            map.project
        );
    }
    row
}

#[test]
#[ignore = "needs SRP_MAPDATA_DIR"]
fn the_grid_table() {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let projects: Vec<String> = match std::env::var("SRP_GRID_TABLE_MAPS") {
        Ok(list) => list.split(',').map(str::to_owned).collect(),
        Err(_) => (1..=8)
            .flat_map(|level| [2, 4].map(|mode| format!("level_{level:02}_day_{mode}")))
            .collect(),
    };
    let mut lines = vec![
        [
            "project",
            "floors",
            "reached",
            "returns",
            "pocket_floors",
            "pockets",
            "pockets4_out_jump",
            "pockets4_out_sprint",
            "pockets4_no_out",
            "objectives",
            "obj_unreached",
            "obj_in_pocket",
            "jumps",
            "jumps_le_0.95",
            "jumps_0.95_0.998",
            "jumps_0.998_1.05",
            "far_jumps",
        ]
        .join("\t"),
    ];
    for project in &projects {
        let path = dir.join(format!("{project}.mapdata"));
        let Ok(map) = MapData::read(&path) else {
            eprintln!("{project}: no file at {}", path.display());
            continue;
        };
        let grid = mapdata::World::build(map.clone(), mapdata::CELL, Body::default())
            .grid
            .clone();
        let row = table_row(&map, &grid);
        let numbers = [
            row.floors,
            row.reached,
            row.returns,
            row.pocket_floors,
            row.pockets,
            row.big_pockets[0],
            row.big_pockets[1],
            row.big_pockets[2],
            row.objectives,
            row.objectives_unreached,
            row.objectives_in_pocket,
            row.jumps,
            row.jumps_by_rise[0],
            row.jumps_by_rise[1],
            row.jumps_by_rise[2],
            row.far_jumps,
        ];
        let line = std::iter::once(project.clone())
            .chain(numbers.iter().map(usize::to_string))
            .collect::<Vec<_>>()
            .join("\t");
        eprintln!("{line}");
        lines.push(line);
    }
    if let Some(out) = std::env::var_os("SRP_GRID_TABLE_OUT") {
        std::fs::write(out, lines.join("\n") + "\n").unwrap();
    }
}
