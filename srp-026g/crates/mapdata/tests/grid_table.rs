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
//! every level's `day_2` and `day_4`); `SRP_GRID_POCKETS_OUT=<file>` writes
//! every pocket (its lowest floor, floors, surest way out), to compare two
//! builds pocket by pocket; `SRP_GRID_OBJECTIVES_OUT=<file>` lists the
//! objectives in a shut pocket or reached by no planned way.
//!
//! The pockets are found here from the grid's links alone, so the table
//! reads the same on a grid of any build; the grid's own marks
//! ([`WalkGrid::in_pocket`]) must agree.

use mapdata::{Body, ExitJump, MapData, NodeId, Vec3, WalkGrid};

/// A spawn's floor, as the bots' tactics find it (`walkable_from_spawns`).
const SPAWN_M: f32 = 1.5;
/// An objective's floor (the grid's seeds).
const OBJECTIVE_M: f32 = 3.0;
/// The client's jump (PROVEN, the jump study).
const JUMP_M: f32 = 0.998;

#[derive(Default)]
struct Row {
    floors: usize,
    reached: usize,
    returns: usize,
    pocket_floors: usize,
    pockets: usize,
    /// Pockets by their surest way out ([`WalkGrid::pocket_exits`] from
    /// the pocket's lowest floor): the jump, only the sprint jump, only a
    /// climb onto steep ground, none.
    pockets_by_exit: [usize; 4],
    objectives: usize,
    objectives_unreached: usize,
    objectives_in_pocket: usize,
    jumps: usize,
    /// Jump links by rise: up to 0.95, 0.95-0.998, 0.998-1.05 m.
    jumps_by_rise: [usize; 3],
    /// Jump links landing 3 or 4 cells along their direction.
    far_jumps: usize,
    /// Each pocket: its lowest floor, its floors, its surest way out.
    pocket_list: Vec<(Vec3, usize, &'static str)>,
    /// Pockets with no way out ([`WalkGrid::in_shut_pocket`]), and those a
    /// planned way from a spawn gets into (none: [`WalkGrid::plans`]).
    shut_pockets: usize,
    shut_planned: usize,
    /// Floors reached over the links but by no planned way.
    unplanned: usize,
    objectives_in_shut: usize,
    objectives_unplanned: usize,
    /// Those objectives: kind, place, why.
    objective_list: Vec<(&'static str, Vec3, &'static str)>,
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
    // Where a planned way from a spawn gets.
    let planned = grid.distances_from(&spawns);
    let plans = |a: NodeId| spawns.is_empty() || planned.reaches(a);
    row.unplanned = (0..n as NodeId)
        .filter(|&a| reached[a as usize] && !plans(a))
        .count();
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
        let lowest = members.iter().copied().min().unwrap();
        let surest = grid.pocket_exits(lowest).iter().map(|e| e.jump).min();
        let (class, name) = match surest {
            Some(ExitJump::Standing) => (0, "standing"),
            Some(ExitJump::Sprint) => (1, "sprint"),
            Some(ExitJump::Steep) => (2, "steep"),
            None => (3, "none"),
        };
        row.pockets_by_exit[class] += 1;
        row.pocket_list
            .push((grid.position(lowest), members.len(), name));
        for &a in &members {
            assert_eq!(
                grid.in_shut_pocket(a),
                class == 3,
                "{}: floor {a}",
                map.project
            );
        }
        if class == 3 {
            row.shut_pockets += 1;
            row.shut_planned += usize::from(members.iter().any(|&a| plans(a)));
        }
    }
    // The objectives: batteries, containers, stations.
    let objectives: Vec<(&'static str, Vec3)> = map
        .battery_spawners
        .iter()
        .map(|b| ("battery", b.item))
        .chain(map.containers.iter().map(|c| ("container", c.position)))
        .chain(map.stations.iter().map(|&s| ("station", s)))
        .collect();
    row.objectives = objectives.len();
    for (kind, at) in objectives {
        let floor = grid.locate(at, OBJECTIVE_M);
        if let Some(f) = floor {
            if grid.in_shut_pocket(f) {
                row.objectives_in_shut += 1;
                row.objective_list.push((kind, at, "in a shut pocket"));
            } else if reached[f as usize] && !plans(f) {
                row.objectives_unplanned += 1;
                row.objective_list.push((kind, at, "no planned way"));
            }
        }
        match floor {
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
            "pockets_out_jump",
            "pockets_sprint_only",
            "pockets_steep_only",
            "pockets_no_out",
            "objectives",
            "obj_unreached",
            "obj_in_pocket",
            "jumps",
            "jumps_le_0.95",
            "jumps_0.95_0.998",
            "jumps_0.998_1.05",
            "far_jumps",
            "shut_pockets",
            "shut_planned",
            "unplanned",
            "obj_in_shut",
            "obj_unplanned",
        ]
        .join("\t"),
    ];
    let mut shut_planned = Vec::new();
    let mut objective_lines = vec!["project\tkind\tx\ty\tz\twhy".to_owned()];
    let mut pocket_lines = vec!["project\tx\ty\tz\tfloors\texit".to_owned()];
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
        if row.shut_planned > 0 {
            shut_planned.push(project.clone());
        }
        for (kind, at, why) in &row.objective_list {
            let line = format!(
                "{project}\t{kind}\t{:.2}\t{:.2}\t{:.2}\t{why}",
                at.x, at.y, at.z
            );
            eprintln!("{line}");
            objective_lines.push(line);
        }
        for (at, floors, exit) in &row.pocket_list {
            pocket_lines.push(format!(
                "{project}\t{:.2}\t{:.2}\t{:.2}\t{floors}\t{exit}",
                at.x, at.y, at.z
            ));
        }
        let numbers = [
            row.floors,
            row.reached,
            row.returns,
            row.pocket_floors,
            row.pockets,
            row.pockets_by_exit[0],
            row.pockets_by_exit[1],
            row.pockets_by_exit[2],
            row.pockets_by_exit[3],
            row.objectives,
            row.objectives_unreached,
            row.objectives_in_pocket,
            row.jumps,
            row.jumps_by_rise[0],
            row.jumps_by_rise[1],
            row.jumps_by_rise[2],
            row.far_jumps,
            row.shut_pockets,
            row.shut_planned,
            row.unplanned,
            row.objectives_in_shut,
            row.objectives_unplanned,
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
    if let Some(out) = std::env::var_os("SRP_GRID_OBJECTIVES_OUT") {
        std::fs::write(out, objective_lines.join("\n") + "\n").unwrap();
    }
    if let Some(out) = std::env::var_os("SRP_GRID_POCKETS_OUT") {
        std::fs::write(out, pocket_lines.join("\n") + "\n").unwrap();
    }
    // No way is planned into a pocket with no way out.
    assert!(
        shut_planned.is_empty(),
        "planned into a shut pocket: {shut_planned:?}"
    );
}
