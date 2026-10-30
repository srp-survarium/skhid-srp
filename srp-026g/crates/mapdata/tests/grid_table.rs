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

use mapdata::grid::POCKET_EXIT_FLOORS;
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
        // Under the exits' look, or an exit past it goes unseen and the
        // pocket is not marked (`WalkGrid::mark_no_exit`): raise it.
        assert!(
            class != 3 || members.len() <= POCKET_EXIT_FLOORS,
            "{}: a pocket of {} floors with no exit seen",
            map.project,
            members.len()
        );
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

/// The jump cap per map (`GridTuning::jump_cap_m`): the grid of the defaults' shape (the clients' body, exact erosion, honest
/// drops, gap links, lips at the walk-offs) built with the standing jump's cap at 0.998 and at the exe's 1.05, side by side: the
/// Jump links by band (standing, the capped band to 1.05, sprint above), the links of the capped band with no run-up (kept, as
/// the base kept them) and the sprint links taken away, and what a body gets to from the spawns (`reached`, `pockets`,
/// `obj_unreached`): the cap must disconnect nothing.
#[test]
#[ignore = "needs SRP_MAPDATA_DIR"]
fn the_jump_cap_per_map() {
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
    let header = "project\tcap\tfloors\tjump_standing\tjump_capped\tjump_sprint\tcapped_no_run_up\tsprint_cut\tjump_06_08\treached\tpockets\tobj_unreached\tlips\tlips_without_walk_off_lips\tovershot_cut\tovershot_kept\tovershot_lost";
    eprintln!("{header}");
    let mut lines = vec![header.to_owned()];
    for project in &projects {
        let Ok(map) = MapData::read(&dir.join(format!("{project}.mapdata"))) else {
            eprintln!("{project}: no file");
            continue;
        };
        let spawns: Vec<Vec3> = map.spawns.iter().map(|s| s.position).collect();
        for cap in [mapdata::grid::STANDING_JUMP_M, 1.05] {
            let body = Body {
                max_jump: cap,
                erosion: mapdata::Erosion::Exact,
                drop_flight_mps: 3.5,
                walk_off_lips: true,
                // `SRP_GRID_CARRY=1`: the tops the jumps carry the body off are cut (`Body::landing_carry`).
                landing_carry: std::env::var_os("SRP_GRID_CARRY").is_some(),
                ..Body::client()
            };
            let (grid, stats) = WalkGrid::build_kept_with_links(
                &map.walker_triangles(),
                map.play_area(mapdata::AREA_MARGIN),
                mapdata::CELL,
                body,
                &map.grid_seeds(),
                &spawns,
                mapdata::OffMeshLinks::Gaps,
            );
            let row = table_row(&map, &grid);
            // `SRP_GRID_COST_OUT=<file>`: the grid's cost (metres of walk and link penalties) from every spawn to every seed
            // (spawns, batteries, containers, stations) on the first cap's grid, appended: project, spawn, seed, cost.
            if let Some(out) = std::env::var_os("SRP_GRID_COST_OUT")
                && cap == mapdata::grid::STANDING_JUMP_M
            {
                use std::io::Write;
                let mut file = std::fs::OpenOptions::new().create(true).append(true).open(out).unwrap();
                let targets: Vec<Option<NodeId>> = map.grid_seeds().iter().map(|&p| grid.locate(p, OBJECTIVE_M)).collect();
                for (i, &spawn) in spawns.iter().enumerate() {
                    let Some(from) = grid.locate(spawn, SPAWN_M) else { continue };
                    let costs = grid.distances_from(&[from]);
                    for (j, target) in targets.iter().enumerate() {
                        if let Some(target) = target {
                            writeln!(file, "{project}\t{i}\t{j}\t{:.1}", costs.get(*target)).unwrap();
                        }
                    }
                }
            }
            let (mut standing, mut capped, mut sprint, mut below_step) = (0usize, 0usize, 0usize, 0usize);
            // `SRP_GRID_SPOT=x,z`: the Jump links of a rise under the step within 1.5 m of this spot are listed.
            let spot = std::env::var("SRP_GRID_SPOT").ok().and_then(|v| {
                let mut it = v.split(',').filter_map(|x| x.parse::<f32>().ok());
                Some(Vec3::new(it.next()?, 0.0, it.next()?))
            });
            for a in 0..grid.len() as NodeId {
                for (b, kind) in grid.links(a) {
                    if kind == mapdata::LinkKind::Jump {
                        let rise = grid.position(b).y - grid.position(a).y;
                        // A Jump link of a rise between the exe's step and the clients' (0.6-0.8 m, startup.cfg): the body walks it; counted.
                        if rise > mapdata::grid::STEP_UP_M && rise <= body.step {
                            below_step += 1;
                            if let Some(spot) = spot.filter(|spot| grid.position(a).distance_xz(*spot) <= 1.5) {
                                eprintln!("{project}: Jump link {:?} -> {:?} rise {rise:.2} near {spot:?}", grid.position(a), grid.position(b));
                            }
                        }
                        if rise > 1.05 {
                            sprint += 1;
                        } else if rise > cap {
                            capped += 1;
                        } else {
                            standing += 1;
                        }
                    }
                }
            }
            // The lips with and without the walk-off lips (`Body::walk_off_lips`), on the first cap's grid only.
            let lips = |grid: &WalkGrid| (0..grid.len() as NodeId).filter(|&n| grid.is_lip(n)).count();
            let lips_column = if cap == mapdata::grid::STANDING_JUMP_M {
                let (without, _) = WalkGrid::build_kept_with_links(
                    &map.walker_triangles(),
                    map.play_area(mapdata::AREA_MARGIN),
                    mapdata::CELL,
                    Body { walk_off_lips: false, ..body },
                    &map.grid_seeds(),
                    &spawns,
                    mapdata::OffMeshLinks::Gaps,
                );
                format!("{}\t{}", lips(&grid), lips(&without))
            } else {
                String::from("-\t-")
            };
            let line = format!(
                "{project}\t{cap}\t{}\t{standing}\t{capped}\t{sprint}\t{}\t{}\t{below_step}\t{}\t{}\t{}\t{lips_column}\t{}\t{}",
                row.floors, stats.capped_no_run_up, stats.sprint_cut, row.reached, row.pockets, row.objectives_unreached, stats.overshot_cut, stats.overshot_kept
            );
            eprintln!("{line}");
            lines.push(line);
        }
    }
    if let Some(out) = std::env::var_os("SRP_GRID_TABLE_OUT") {
        std::fs::write(out, lines.join("\n") + "\n").unwrap();
    }
}

/// Vector Laboratory (day_4): the floor at (26.8, 21.3, -3.8) is reached by a rise of 0.62 m. With the exe's step (0.6) that is
/// a Jump link; with the clients' numbers (step 0.8, `startup.cfg`) it is a step, so no Jump link of a rise within the step
/// starts or ends within 1.5 m of it and the walk never presses a jump there. (The owner's local matches ran the exe's step.)
#[test]
#[ignore = "needs SRP_MAPDATA_DIR"]
fn the_vector_laboratory_step_of_0_62_m_is_a_jump_with_the_exes_step_and_a_walk_with_the_clients() {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let Ok(map) = MapData::read(&dir.join("level_07_day_4.mapdata")) else {
        eprintln!("skipping: no level_07_day_4.mapdata");
        return;
    };
    let spawns: Vec<Vec3> = map.spawns.iter().map(|s| s.position).collect();
    // The Jump links of a rise over the exe's step and within the clients' (0.6-0.8 m) that end on the floor at the spot.
    let jumps_to_the_step = |body: Body| {
        let (grid, _) = WalkGrid::build_kept_with_links(
            &map.walker_triangles(),
            map.play_area(mapdata::AREA_MARGIN),
            mapdata::CELL,
            body,
            &map.grid_seeds(),
            &spawns,
            mapdata::OffMeshLinks::Gaps,
        );
        let spot = Vec3::new(26.8, 21.3, -3.8);
        let near = |p: Vec3| p.distance_xz(spot) <= 1.5 && (p.y - spot.y).abs() < 1.0;
        let mut jumps = 0;
        for a in 0..grid.len() as NodeId {
            for (b, kind) in grid.links(a) {
                let rise = grid.position(b).y - grid.position(a).y;
                if kind == mapdata::LinkKind::Jump
                    && rise > mapdata::grid::STEP_UP_M
                    && rise <= 0.8
                    && (near(grid.position(a)) || near(grid.position(b)))
                {
                    jumps += 1;
                }
            }
        }
        jumps
    };
    let exe = jumps_to_the_step(Body::default());
    let client = jumps_to_the_step(Body { erosion: mapdata::Erosion::Exact, drop_flight_mps: 3.5, walk_off_lips: true, ..Body::client() });
    assert!(exe > 0, "the exe's step makes the rise a jump");
    assert_eq!(client, 0, "the clients' step walks it");
}

/// The leaping links (Jump, GapJump, GapDrop) with an end within `SRP_GRID_SPOT_R` (default 6) metres of `SRP_GRID_SPOT=x,z` on
/// `SRP_GRID_TABLE_MAPS`' first project, on the bots' body (`GridTuning::body` defaults): kind, both ends, rise, across, press.
#[test]
#[ignore = "needs SRP_MAPDATA_DIR"]
fn the_leaping_links_near_a_spot() {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let project = std::env::var("SRP_GRID_TABLE_MAPS").unwrap_or_else(|_| "level_04_day_2".to_owned());
    let project = project.split(',').next().unwrap().to_owned();
    let Some(spot) = std::env::var("SRP_GRID_SPOT").ok().and_then(|v| {
        let mut it = v.split(',').filter_map(|x| x.parse::<f32>().ok());
        Some(Vec3::new(it.next()?, 0.0, it.next()?))
    }) else {
        eprintln!("skipping: SRP_GRID_SPOT=x,z is not set");
        return;
    };
    let radius: f32 = std::env::var("SRP_GRID_SPOT_R").ok().and_then(|v| v.parse().ok()).unwrap_or(6.0);
    let map = MapData::read(&dir.join(format!("{project}.mapdata"))).unwrap();
    let spawns: Vec<Vec3> = map.spawns.iter().map(|s| s.position).collect();
    let body = Body {
        max_jump: mapdata::grid::STANDING_JUMP_M,
        erosion: mapdata::Erosion::Exact,
        drop_flight_mps: 3.5,
        walk_off_lips: true,
        // `SRP_GRID_CARRY=1`: the grid with the carried landings (`Body::landing_carry`).
        landing_carry: std::env::var_os("SRP_GRID_CARRY").is_some(),
        ..Body::client()
    };
    let (grid, _) = WalkGrid::build_kept_with_links(
        &map.walker_triangles(),
        map.play_area(mapdata::AREA_MARGIN),
        mapdata::CELL,
        body,
        &map.grid_seeds(),
        &spawns,
        mapdata::OffMeshLinks::Gaps,
    );
    eprintln!("{project}: {} floors, spot {spot:?} within {radius} m", grid.len());
    for a in 0..grid.len() as NodeId {
        for (b, kind) in grid.links(a) {
            if !kind.is_leap() {
                continue;
            }
            let (pa, pb) = (grid.position(a), grid.position(b));
            if pa.distance_xz(spot) > radius && pb.distance_xz(spot) > radius {
                continue;
            }
            let press = grid.off_link(a, b).map(|l| format!("{:?}", l.press)).unwrap_or_default();
            eprintln!(
                "{kind:?} {a}->{b} from ({:.2},{:.2},{:.2}) to ({:.2},{:.2},{:.2}) rise {:+.2} across {:.2} {press} stand {}/{} lead {:?}",
                pa.x, pa.y, pa.z, pb.x, pb.y, pb.z, pb.y - pa.y, pa.distance_xz(pb), grid.can_stand(a), grid.can_stand(b), grid.press_lead(a, b)
            );
        }
    }
}

/// The floors around `SRP_GRID_SPOT=x,z` within `SRP_GRID_SPOT_R` metres between feet heights `SRP_GRID_Y=lo,hi`, as a map of
/// the lowest height per half metre (tenths of a metre over `lo`, letters from 'a' for 0.0), z down, x across.
#[test]
#[ignore = "needs SRP_MAPDATA_DIR"]
fn the_floors_around_a_spot() {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let project = std::env::var("SRP_GRID_TABLE_MAPS").unwrap_or_else(|_| "level_04_day_2".to_owned());
    let project = project.split(',').next().unwrap().to_owned();
    let nums = |name: &str| -> Vec<f32> {
        std::env::var(name).unwrap_or_default().split(',').filter_map(|x| x.parse().ok()).collect()
    };
    let (spot, y) = (nums("SRP_GRID_SPOT"), nums("SRP_GRID_Y"));
    if spot.len() != 2 || y.len() != 2 {
        eprintln!("skipping: SRP_GRID_SPOT=x,z and SRP_GRID_Y=lo,hi");
        return;
    }
    let radius: f32 = std::env::var("SRP_GRID_SPOT_R").ok().and_then(|v| v.parse().ok()).unwrap_or(6.0);
    let map = MapData::read(&dir.join(format!("{project}.mapdata"))).unwrap();
    let spawns: Vec<Vec3> = map.spawns.iter().map(|s| s.position).collect();
    let body = Body {
        max_jump: mapdata::grid::STANDING_JUMP_M,
        erosion: mapdata::Erosion::Exact,
        drop_flight_mps: 3.5,
        walk_off_lips: true,
        ..Body::client()
    };
    let (grid, _) = WalkGrid::build_kept_with_links(
        &map.walker_triangles(),
        map.play_area(mapdata::AREA_MARGIN),
        mapdata::CELL,
        body,
        &map.grid_seeds(),
        &spawns,
        mapdata::OffMeshLinks::Gaps,
    );
    let n = (2.0 * radius / 0.5) as usize + 1;
    let mut cells = vec![vec![f32::INFINITY; n]; n];
    for node in grid.floors_in([spot[0] - radius, spot[1] - radius, spot[0] + radius, spot[1] + radius]) {
        let p = grid.position(node);
        if p.y < y[0] || p.y > y[1] {
            continue;
        }
        let (i, j) = (((p.x - spot[0] + radius) / 0.5) as usize, ((p.z - spot[1] + radius) / 0.5) as usize);
        if i < n && j < n {
            cells[j][i] = cells[j][i].min(p.y);
        }
    }
    eprintln!("{project}: floors {:.1}..{:.1} m high, 0.5 m cells from x {:.1}, z {:.1} (z down)", y[0], y[1], spot[0] - radius, spot[1] - radius);
    for (j, row) in cells.iter().enumerate() {
        let line: String = row
            .iter()
            .map(|&h| if h.is_finite() { (b'a' + (((h - y[0]) * 10.0) as u8).min(25)) as char } else { '.' })
            .collect();
        eprintln!("z {:6.1} {line}", spot[1] - radius + j as f32 * 0.5);
    }
}

/// Every Jump link of a rise of `SRP_GRID_RISE_MIN` (default 0.9) m or more, on the bots' body, with how far the top it lands
/// on goes on straight ahead of the landing (`depth`, metres of floor within 0.35 m of the landing's height, up to 6) and how
/// wide it is across the link's way (`width`, up to 6): a flight that goes on past the top comes down beyond it.
/// `LINK` lines, tab-separated: project, kind, take-off x y z, landing x y z, rise, depth, width, press lead (`-` for none).
#[test]
#[ignore = "needs SRP_MAPDATA_DIR"]
fn the_jump_links_top_depth() {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let projects: Vec<String> = std::env::var("SRP_GRID_TABLE_MAPS")
        .unwrap_or_else(|_| "level_04_day_2".to_owned())
        .split(',')
        .map(str::to_owned)
        .collect();
    let rise_min: f32 = std::env::var("SRP_GRID_RISE_MIN").ok().and_then(|v| v.parse().ok()).unwrap_or(0.9);
    for project in &projects {
        let Ok(map) = MapData::read(&dir.join(format!("{project}.mapdata"))) else {
            eprintln!("{project}: no file");
            continue;
        };
        let spawns: Vec<Vec3> = map.spawns.iter().map(|s| s.position).collect();
        let body = Body {
            max_jump: mapdata::grid::STANDING_JUMP_M,
            erosion: mapdata::Erosion::Exact,
            drop_flight_mps: 3.5,
            walk_off_lips: true,
            // `SRP_GRID_CARRY=1`: the grid with the carried landings (`Body::landing_carry`), a press lead a column.
            landing_carry: std::env::var_os("SRP_GRID_CARRY").is_some(),
            ..Body::client()
        };
        let (grid, _) = WalkGrid::build_kept_with_links(
            &map.walker_triangles(),
            map.play_area(mapdata::AREA_MARGIN),
            mapdata::CELL,
            body,
            &map.grid_seeds(),
            &spawns,
            mapdata::OffMeshLinks::Gaps,
        );
        // Whether a floor within 0.35 m of this height is at the point.
        let floor_at = |x: f32, z: f32, y: f32| {
            grid.floors_in([x - 0.12, z - 0.12, x + 0.12, z + 0.12]).any(|n| {
                let p = grid.position(n);
                (p.y - y).abs() <= 0.35 && (p.x - x).abs() <= 0.13 && (p.z - z).abs() <= 0.13
            })
        };
        let reach = |from: Vec3, dx: f32, dz: f32| {
            let mut t = 0.25;
            while t <= 6.0 && floor_at(from.x + dx * t, from.z + dz * t, from.y) {
                t += 0.25;
            }
            t - 0.25
        };
        let mut seen = std::collections::HashSet::new();
        for a in 0..grid.len() as NodeId {
            for (b, kind) in grid.links(a) {
                if kind != mapdata::LinkKind::Jump {
                    continue;
                }
                let (pa, pb) = (grid.position(a), grid.position(b));
                let rise = pb.y - pa.y;
                if rise < rise_min || !seen.insert((a, b)) {
                    continue;
                }
                let across = pa.distance_xz(pb).max(1e-3);
                let (dx, dz) = ((pb.x - pa.x) / across, (pb.z - pa.z) / across);
                let depth = reach(pb, dx, dz);
                let width = reach(pb, -dz, dx) + reach(pb, dz, -dx) + 0.25;
                eprintln!(
                    "LINK\t{project}\t{kind:?}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{rise:.2}\t{depth:.2}\t{width:.2}\t{}",
                    pa.x, pa.y, pa.z, pb.x, pb.y, pb.z,
                    grid.press_lead(a, b).map_or("-".to_owned(), |l| format!("{l:.2}"))
                );
            }
        }
    }
}

/// A neck of the exact erosion: the floors of the cells grid within `SRP_GRID_NECK_R` (default 1.2) m of `SRP_GRID_NECK=x,y,z` (the
/// server's body otherwise), with whether the exact grid kept each, its nearest wall (distance and triangle) and the clearance the exact
/// pass asks. `SRP_GRID_TABLE_MAPS` picks the project (default `level_06_day_4`).
#[test]
#[ignore = "needs SRP_MAPDATA_DIR"]
fn a_neck_of_the_exact_erosion() {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let project = std::env::var("SRP_GRID_TABLE_MAPS").unwrap_or_else(|_| "level_06_day_4".to_owned());
    let project = project.split(',').next().unwrap().to_owned();
    let Some(neck) = std::env::var("SRP_GRID_NECK").ok().and_then(|v| {
        let mut it = v.split(',').filter_map(|x| x.parse::<f32>().ok());
        Some(Vec3::new(it.next()?, it.next()?, it.next()?))
    }) else {
        eprintln!("skipping: SRP_GRID_NECK=x,y,z is not set");
        return;
    };
    let radius: f32 = std::env::var("SRP_GRID_NECK_R").ok().and_then(|v| v.parse().ok()).unwrap_or(1.2);
    let map = MapData::read(&dir.join(format!("{project}.mapdata"))).unwrap();
    let spawns: Vec<Vec3> = map.spawns.iter().map(|s| s.position).collect();
    let triangles = map.walker_triangles();
    let (x0, y0, z0) = (neck.x, neck.y, neck.z);
    let build = |erosion| {
        let body = Body {
            max_jump: mapdata::grid::STANDING_JUMP_M,
            erosion,
            drop_flight_mps: 3.5,
            walk_off_lips: true,
            landing_carry: true,
            ..Body::client()
        };
        WalkGrid::build_kept_with_links(&triangles, map.play_area(mapdata::AREA_MARGIN), mapdata::CELL, body, &map.grid_seeds(), &spawns, mapdata::OffMeshLinks::Gaps).0
    };
    let (cells, exact) = (build(mapdata::Erosion::Cells), build(mapdata::Erosion::Exact));
    eprintln!("{project}: neck ({x0}, {y0}, {z0}); the exact clearance is {:.4} m, the cell {} m", exact.exact_clearance(), mapdata::CELL);
    for n in cells.floors_in([x0 - radius, z0 - radius, x0 + radius, z0 + radius]) {
        let p = cells.position(n);
        if (p.y - y0).abs() > 0.6 || p.distance_xz(Vec3::new(x0, 0.0, z0)) > radius {
            continue;
        }
        let kept = exact.locate(p, 0.05).is_some_and(|m| exact.position(m).distance_xz(p) < 0.05 && (exact.position(m).y - p.y).abs() < 0.05);
        let wall = cells.nearest_wall(&triangles, p.x, p.z, p.y, 1.0);
        let walk_links = cells.links(n).filter(|l| l.1 == mapdata::LinkKind::Walk).count();
        eprintln!(
            "({:.2}, {:.2}, {:.2}) exact_kept={kept} walk_links_on_cells={walk_links} wall={}",
            p.x,
            p.y,
            p.z,
            wall.map_or("none within 1 m".to_owned(), |(d, v)| format!(
                "{d:.3} m  tri[({:.2},{:.2},{:.2}) ({:.2},{:.2},{:.2}) ({:.2},{:.2},{:.2})]",
                v[0].x, v[0].y, v[0].z, v[1].x, v[1].y, v[1].z, v[2].x, v[2].y, v[2].z
            ))
        );
    }
}

/// Every floor of the grid the server builds (`FLOOR` lines, tab-separated: project, x, y, z, 1 when a standing body fits) for the maps
/// in `SRP_GRID_TABLE_MAPS`: two builds of the code are told apart by their floors.
#[test]
#[ignore = "needs SRP_MAPDATA_DIR"]
fn the_floor_list() {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let projects: Vec<String> = std::env::var("SRP_GRID_TABLE_MAPS")
        .unwrap_or_else(|_| "level_04_day_2".to_owned())
        .split(',')
        .map(str::to_owned)
        .collect();
    for project in &projects {
        let Ok(map) = MapData::read(&dir.join(format!("{project}.mapdata"))) else {
            eprintln!("{project}: no file");
            continue;
        };
        let spawns: Vec<Vec3> = map.spawns.iter().map(|s| s.position).collect();
        let body = Body {
            max_jump: mapdata::grid::STANDING_JUMP_M,
            erosion: mapdata::Erosion::Exact,
            drop_flight_mps: 3.5,
            walk_off_lips: true,
            landing_carry: true,
            ..Body::client()
        };
        let (grid, _) = WalkGrid::build_kept_with_links(
            &map.walker_triangles(),
            map.play_area(mapdata::AREA_MARGIN),
            mapdata::CELL,
            body,
            &map.grid_seeds(),
            &spawns,
            mapdata::OffMeshLinks::Gaps,
        );
        for n in 0..grid.len() as NodeId {
            let p = grid.position(n);
            eprintln!("FLOOR\t{project}\t{:.2}\t{:.2}\t{:.2}\t{}", p.x, p.y, p.z, u8::from(grid.can_stand(n)));
        }
    }
}

/// The Jump links the grid keeps though no press keeps a body on their top (the only way to a floor, `drop_overshot_landings`), per map
/// of `SRP_GRID_TABLE_MAPS`: how many, and what the grid reaches from the spawns with all of them taken away (the floors, the objectives
/// that are out of reach then, and the gateways: the kept links from the floors still reached to those not). With
/// `SRP_HUMAN_DIR=<dir of the <project>.tsv samples>` the live players' samples are put on the floors: the lives that stand on the floors
/// out of reach, and where consecutive samples go from a floor still reached to one not (the way the grid is missing).
/// Lines: `LASTRESORT` (a link), `GATEWAY` (a link into the lost floors), `LOST` (a summary), `HUMANCROSS` (a crossing, 1 m cells).
#[test]
#[ignore = "needs SRP_MAPDATA_DIR"]
fn the_last_resort_links() {
    use mapdata::grid::{SPRINT_TOP_M, STANDING_JUMP_M, WALK_CLIP_END_M, walking_press_lead};
    use std::collections::{HashMap, HashSet, VecDeque};
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let projects: Vec<String> = std::env::var("SRP_GRID_TABLE_MAPS")
        .unwrap_or_else(|_| "level_04_day_2".to_owned())
        .split(',')
        .map(str::to_owned)
        .collect();
    for project in &projects {
        let Ok(map) = MapData::read(&dir.join(format!("{project}.mapdata"))) else {
            eprintln!("{project}: no file");
            continue;
        };
        let spawns: Vec<Vec3> = map.spawns.iter().map(|s| s.position).collect();
        let body = Body {
            max_jump: STANDING_JUMP_M,
            erosion: mapdata::Erosion::Exact,
            drop_flight_mps: 3.5,
            walk_off_lips: true,
            landing_carry: true,
            ..Body::client()
        };
        let seeds = map.grid_seeds();
        let (grid, _) = WalkGrid::build_kept_with_links(
            &map.walker_triangles(),
            map.play_area(mapdata::AREA_MARGIN),
            mapdata::CELL,
            body,
            &seeds,
            &spawns,
            mapdata::OffMeshLinks::Gaps,
        );
        // The floors walked on past a landing along the jump (the carried landing's top), as the grid's own `top_run`.
        let top_run = |from: Vec3, b: NodeId, want: f32| {
            let pb = grid.position(b);
            let across = from.distance_xz(pb);
            let (ux, uz) = ((pb.x - from.x) / across, (pb.z - from.z) / across);
            let (mut at, mut run) = (b, 0.0f32);
            while run < want {
                let here = grid.position(at);
                let next = grid.links(at).find(|&(n, kind)| {
                    let p = grid.position(n);
                    let len = here.distance_xz(p);
                    kind == mapdata::LinkKind::Walk
                        && len > 1e-3
                        && ((p.x - here.x) * ux + (p.z - here.z) * uz) / len > 0.99
                        && grid.can_stand(n)
                });
                let Some((n, _)) = next else { break };
                run += here.distance_xz(grid.position(n));
                at = n;
            }
            run
        };
        let mut unfit: HashSet<(NodeId, NodeId)> = HashSet::new();
        for a in 0..grid.len() as NodeId {
            for (b, kind) in grid.links(a) {
                if kind != mapdata::LinkKind::Jump {
                    continue;
                }
                let (pa, pb) = (grid.position(a), grid.position(b));
                let (rise, across) = (pb.y - pa.y, pa.distance_xz(pb));
                if across < 1e-3 {
                    continue;
                }
                let sprint = rise > STANDING_JUMP_M;
                let fits = if sprint {
                    let want = SPRINT_TOP_M - 0.5 * across;
                    top_run(pa, b, want) + 1e-3 >= want
                } else {
                    walking_press_lead(rise, across, top_run(pa, b, WALK_CLIP_END_M)).is_some()
                };
                if !fits {
                    unfit.insert((a, b));
                    eprintln!(
                        "LASTRESORT\t{project}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{rise:.2}\t{}",
                        pa.x,
                        pa.y,
                        pa.z,
                        pb.x,
                        pb.y,
                        pb.z,
                        if sprint { "sprint" } else { "walk" }
                    );
                }
            }
        }
        // What is reached from the spawns, with and without the unfit links.
        let bfs = |from: NodeId, skip: bool| {
            let mut seen = vec![false; grid.len()];
            let mut queue = VecDeque::from([from]);
            seen[from as usize] = true;
            while let Some(a) = queue.pop_front() {
                for (b, _) in grid.links(a) {
                    if (skip && unfit.contains(&(a, b))) || seen[b as usize] {
                        continue;
                    }
                    seen[b as usize] = true;
                    queue.push_back(b);
                }
            }
            seen
        };
        let mut starts: Vec<NodeId> = spawns.iter().filter_map(|&p| grid.locate(p, 1.5)).collect();
        starts.sort_unstable();
        starts.dedup();
        let (mut lost, mut cut_any) = (vec![false; grid.len()], vec![false; grid.len()]);
        for &s in &starts {
            let (full, cut) = (bfs(s, false), bfs(s, true));
            for n in 0..grid.len() {
                lost[n] |= full[n] && !cut[n];
                cut_any[n] |= cut[n];
            }
        }
        // The floors a body can walk to but not back from once the unfit links are away: the way back out of a pit.
        let mut rev: Vec<Vec<(NodeId, bool)>> = vec![Vec::new(); grid.len()];
        for a in 0..grid.len() as NodeId {
            for (b, _) in grid.links(a) {
                rev[b as usize].push((a, unfit.contains(&(a, b))));
            }
        }
        let back = |skip: bool| {
            let mut seen = vec![false; grid.len()];
            let mut queue = VecDeque::new();
            for &s in &starts {
                seen[s as usize] = true;
                queue.push_back(s);
            }
            while let Some(b) = queue.pop_front() {
                for &(a, is_unfit) in &rev[b as usize] {
                    if (skip && is_unfit) || seen[a as usize] {
                        continue;
                    }
                    seen[a as usize] = true;
                    queue.push_back(a);
                }
            }
            seen
        };
        let (back_full, back_cut) = (back(false), back(true));
        let stranded: Vec<NodeId> = (0..grid.len() as NodeId)
            .filter(|&n| cut_any[n as usize] && back_full[n as usize] && !back_cut[n as usize])
            .collect();
        eprintln!("STRANDED	{project}	floors walked to but not back from with the unfit links away {}", stranded.len());
        let stranded_set: HashSet<NodeId> = stranded.iter().copied().collect();
        for (i, &p) in seeds.iter().enumerate() {
            if let Some(n) = grid.locate(p, 1.5)
                && stranded_set.contains(&n)
            {
                eprintln!("STRANDEDOBJECTIVE	{project}	seed {i}	{:.2}	{:.2}	{:.2}", p.x, p.y, p.z);
            }
        }
        for &n in stranded.iter().step_by((stranded.len() / 6).max(1)).take(6) {
            let p = grid.position(n);
            eprintln!("STRANDEDAT	{project}	{:.2}	{:.2}	{:.2}", p.x, p.y, p.z);
        }
        let lost_n = lost.iter().filter(|&&l| l).count();
        let objectives: Vec<NodeId> = seeds.iter().filter_map(|&p| grid.locate(p, 1.5)).collect();
        let objectives_lost = objectives.iter().filter(|&&n| lost[n as usize]).count();
        eprintln!(
            "LOST\t{project}\tunfit links {}\tfloors {}\tspawns {}\tfloors lost with them away {lost_n}\tobjectives {} lost {objectives_lost}",
            unfit.len(),
            grid.len(),
            starts.len(),
            objectives.len()
        );
        for &n in &objectives {
            if lost[n as usize] {
                let p = grid.position(n);
                eprintln!("LOSTOBJECTIVE	{project}	{:.2}	{:.2}	{:.2}", p.x, p.y, p.z);
            }
        }
        for &(a, b) in &unfit {
            if cut_any[a as usize] && lost[b as usize] {
                let (pa, pb) = (grid.position(a), grid.position(b));
                eprintln!("GATEWAY\t{project}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}", pa.x, pa.y, pa.z, pb.x, pb.y, pb.z);
            }
        }
        // The live players on those floors.
        let Some(human_dir) = std::env::var_os("SRP_HUMAN_DIR").map(std::path::PathBuf::from) else { continue };
        let Ok(text) = std::fs::read_to_string(human_dir.join(format!("{project}.tsv"))) else {
            eprintln!("HUMAN\t{project}\tno samples");
            continue;
        };
        let class = |x: f32, y: f32, z: f32| -> u8 {
            match grid.locate(Vec3::new(x, y, z), 0.7) {
                Some(n) if lost[n as usize] => 2,
                Some(n) if cut_any[n as usize] => 1,
                _ => 0,
            }
        };
        type Sample = (u32, f32, f32, f32, bool);
        let mut lives: HashMap<(String, String, String), Vec<Sample>> = HashMap::new();
        for line in text.lines().skip(1) {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 11 {
                continue;
            }
            let (Ok(t), Ok(x), Ok(y), Ok(z)) = (f[2].parse::<u32>(), f[3].parse::<f32>(), f[4].parse::<f32>(), f[5].parse::<f32>()) else {
                continue;
            };
            lives.entry((f[0].to_owned(), f[1].to_owned(), f[10].to_owned())).or_default().push((t, x, y, z, f[8] == "1"));
        }
        let (mut on_lost, mut jumped_in) = (0usize, 0usize);
        let mut crossings = HashMap::<(i32, i32, i32, i32), u32>::new();
        for samples in lives.values_mut() {
            samples.sort_by_key(|s| s.0);
            let mut prev: Option<(u32, u8, bool, Vec3)> = None;
            let mut stood = false;
            for &(t, x, y, z, jumping) in samples.iter() {
                let c = class(x, y, z);
                stood |= c == 2;
                if let Some((pt, pc, pj, pp)) = prev
                    && pc == 1
                    && c == 2
                    && t - pt <= 3000
                {
                    eprintln!(
                        "CROSSING	{project}	{} ms	from ({:.2},{:.2},{:.2}) to ({x:.2},{y:.2},{z:.2})	jumping {pj}/{jumping}",
                        t - pt,
                        pp.x,
                        pp.y,
                        pp.z
                    );
                    if jumping || pj {
                        jumped_in += 1;
                    } else {
                        *crossings.entry((pp.x.round() as i32, pp.z.round() as i32, x.round() as i32, z.round() as i32)).or_default() += 1;
                    }
                }
                prev = Some((t, c, jumping, Vec3::new(x, y, z)));
            }
            on_lost += usize::from(stood);
        }
        eprintln!(
            "HUMAN\t{project}\tlives {}\tlives standing on the lost floors {on_lost}\tcrossings by a jump {jumped_in}\tcrossings on foot {}",
            lives.len(),
            crossings.values().sum::<u32>()
        );
        let mut top: Vec<_> = crossings.into_iter().collect();
        top.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
        for ((px, pz, x, z), n) in top.into_iter().take(12) {
            eprintln!("HUMANCROSS\t{project}\t{n}\tfrom ({px},{pz}) to ({x},{z})");
        }
    }
}

/// Whether the grid the server builds reaches given points from the spawns: `SRP_POINTS_FILE` lines `project<TAB>x<TAB>y<TAB>z`, one
/// `REACH` line each (project, x, y, z, 1 when a floor is within 1 m, 1 when the spawns reach it). To tell two builds of the code apart.
#[test]
#[ignore = "needs SRP_MAPDATA_DIR"]
fn the_reach_of_points() {
    use std::collections::{BTreeMap, VecDeque};
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let Some(file) = std::env::var_os("SRP_POINTS_FILE") else {
        eprintln!("skipping: SRP_POINTS_FILE is not set");
        return;
    };
    let mut by_project: BTreeMap<String, Vec<Vec3>> = BTreeMap::new();
    for line in std::fs::read_to_string(file).unwrap().lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() == 4
            && let (Ok(x), Ok(y), Ok(z)) = (f[1].parse::<f32>(), f[2].parse::<f32>(), f[3].parse::<f32>())
        {
            by_project.entry(f[0].to_owned()).or_default().push(Vec3::new(x, y, z));
        }
    }
    for (project, points) in &by_project {
        let Ok(map) = MapData::read(&dir.join(format!("{project}.mapdata"))) else {
            eprintln!("{project}: no file");
            continue;
        };
        let spawns: Vec<Vec3> = map.spawns.iter().map(|s| s.position).collect();
        let body = Body {
            max_jump: mapdata::grid::STANDING_JUMP_M,
            erosion: mapdata::Erosion::Exact,
            drop_flight_mps: 3.5,
            walk_off_lips: true,
            landing_carry: true,
            ..Body::client()
        };
        let (grid, _) = WalkGrid::build_kept_with_links(
            &map.walker_triangles(),
            map.play_area(mapdata::AREA_MARGIN),
            mapdata::CELL,
            body,
            &map.grid_seeds(),
            &spawns,
            mapdata::OffMeshLinks::Gaps,
        );
        let mut seen = vec![false; grid.len()];
        let mut queue = VecDeque::new();
        for n in spawns.iter().filter_map(|&p| grid.locate(p, 1.5)) {
            if !seen[n as usize] {
                seen[n as usize] = true;
                queue.push_back(n);
            }
        }
        while let Some(a) = queue.pop_front() {
            for (b, _) in grid.links(a) {
                if !seen[b as usize] {
                    seen[b as usize] = true;
                    queue.push_back(b);
                }
            }
        }
        for p in points {
            let node = grid.locate(*p, 1.0);
            eprintln!("REACH\t{project}\t{:.1}\t{:.1}\t{:.1}\t{}\t{}", p.x, p.y, p.z, u8::from(node.is_some()), u8::from(node.is_some_and(|n| seen[n as usize])));
        }
    }
}
