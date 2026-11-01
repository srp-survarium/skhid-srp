//! The owner counts of the movement grid's switches (`grid.walk_off_lips`, `grid.jump_cap_m`, `feel.zone_lines`): how often each
//! one fires and the outcome it is for. This topic owns its drop counts (a fall of a metre or more in a tick, drops of 4 m or
//! more in a second) and reads the bot and the grid itself; it changes none of the shared tally.
//!
//! - **Falls** by whether the route drops there (planned), by the start floor (a lip, near a lip or a fall: flagged) and by
//!   the link taken (walked off a gap drop, or a Drop). The target: no unplanned fall from an unflagged floor (before: 63 of
//!   245). A fall counts as the route's own within [`PLANNED_LANDING_M`] of a corner of its drops: a sprint walk-off lands
//!   1-1.5 m past the planned landing. `planned_late` are those the shared tally's 1 m would have called unplanned.
//! - **The lip guard** on a walk-off's take-off: ticks it changed the keys (swapped, braked or doomed) on such a floor.
//! - **Drops of 4 m or more in a second** by whether a bot fell off its route in that second and by the link, and the rest
//!   (the route's own drops, humans, a loss with no 1 m tick).
//! - **Route jump presses** by the link's rise against the grid's cap: as a step, standing, in the capped band (the cap to the
//!   exe's 1.05 m), sprint, and over a gap link. (The sprint jump counters by band, pressed, got up and given up, are the
//!   route counters' line: they are process-wide.)
//! - **The grid's links** (once a grid): jump links standing, capped and sprint, walk links over the old step (the
//!   0.6-0.8 m rises taken as steps), walk-off links and the lips among their take-offs.
//! - **Zone lines** (A136): the walks that tested their lines over a zone's floors (entries, exits, ticks), the bot-ticks on
//!   floors a zone refuses, those standing still, and the stands of 2 s or more by play and whether the zone was its choice.

use std::collections::BTreeMap;

use mapdata::LinkKind;
use mapdata::grid::gap::Takeoff;

use super::*;

/// A body this slow stands still, m/s.
const STILL_MPS: f32 = 0.2;
/// A stand on refused floors counts from this many ticks in a row (2 s).
const STAND_TICKS: u32 = 2 * TPS as u32;
/// A fall is the route's own when its landing is within this of a corner of the route's drops, m (the shared tally's is 1).
const PLANNED_LANDING_M: f32 = 2.0;
const SHARED_LANDING_M: f32 = 1.0;
/// A fall from this much in a tick, and a drop in a second from this much, m.
const FALL_M: f32 = 1.0;
const DROP_M: f32 = 4.0;
/// The exe body's standing jump top, m: the capped band ends here.
/// The exe's standing top (the one number, `mapdata::Body::default().max_jump`).
fn exe_top_m() -> f32 {
    mapdata::Body::default().max_jump
}

/// What a grid's links are (counted once for the grid of the run).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct LinkCounts {
    jump_standing: u32,
    jump_capped: u32,
    jump_sprint: u32,
    walk_rise_over_exe_step: u32,
    gap_jump: u32,
    walk_off: u32,
    walk_off_lips: u32,
    press_gap_drop: u32,
}

impl LinkCounts {
    fn of(grid: &WalkGrid) -> Self {
        let body = grid.body();
        let mut counts = Self::default();
        for node in 0..grid.len() as mapdata::NodeId {
            let from = grid.position(node).y;
            for (to, kind) in grid.links(node) {
                let rise = grid.position(to).y - from;
                match kind {
                    LinkKind::Jump if rise > exe_top_m() => counts.jump_sprint += 1,
                    LinkKind::Jump if rise > body.max_jump => counts.jump_capped += 1,
                    LinkKind::Jump => counts.jump_standing += 1,
                    LinkKind::Walk if rise > mapdata::grid::STEP_UP_M && rise <= body.step => {
                        counts.walk_rise_over_exe_step += 1;
                    }
                    _ => {}
                }
            }
        }
        for link in grid.off_links() {
            match (link.kind, link.press) {
                (LinkKind::GapJump, _) => counts.gap_jump += 1,
                (LinkKind::GapDrop, Takeoff::WalkOff) => {
                    counts.walk_off += 1;
                    counts.walk_off_lips += u32::from(grid.is_lip(link.from));
                }
                (LinkKind::GapDrop, _) => counts.press_gap_drop += 1,
                _ => {}
            }
        }
        counts
    }
}

/// How a fall began and ended: the splits of the module's text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FallClass {
    planned: bool,
    flagged: bool,
    walk_off: bool,
}

impl FallClass {
    fn key(self) -> String {
        format!(
            "{}_{}_{}",
            if self.planned { "planned" } else { "unplanned" },
            if self.flagged { "flagged" } else { "unflagged" },
            if self.walk_off { "walk_off" } else { "drop" }
        )
    }
}

/// A body's second, for the drops of 4 m: the height at its start, every tick alive, and the unplanned fall in it, if any.
#[derive(Clone, Copy, Debug, Default)]
struct Second {
    from_y: f32,
    whole: bool,
    unplanned: Option<FallClass>,
}

#[derive(Default)]
pub(super) struct GridCounts {
    /// By place: the landings of the drops of its route (the lower corner of each leg that goes down a metre or more).
    route_drops: Vec<Vec<Vec3>>,
    /// By place: the lip guard's verdict of its last tick (`Bot::bench_guard`).
    guard: Vec<u8>,
    /// By place: the body stands on floors a zone refuses at this tick; its walk tests its lines over them; its play's
    /// intent puts it there by choice (`Intent::zone_ok`).
    on_zone: Vec<bool>,
    walking_out: Vec<bool>,
    was_walking_out: Vec<bool>,
    chosen: Vec<bool>,
    still_ticks: Vec<u32>,
    seconds: Vec<Second>,
    zone_ticks: u32,
    zone_still_ticks: u32,
    stands: u32,
    /// The stands of 2 s or more by the play and whether the zone was its choice.
    stands_by: BTreeMap<String, u32>,
    walk_entries: u32,
    walk_exits: u32,
    walk_ticks: u32,
    /// Falls of a metre or more in a tick: `<planned|unplanned>_<flagged|unflagged>_<walk_off|drop>`.
    falls: BTreeMap<String, u32>,
    planned_late: u32,
    /// Ticks the lip guard changed the keys (swapped, braked, doomed) on a walk-off's take-off, and on another lip's floor.
    guard_walk_off: u32,
    guard_other_lip: u32,
    /// The ticks a battery carrier is alive and carrying: on a lip's floors, and with the lip guard's verdict Safe / Swapped /
    /// Braked (`Bot::bench_guard` 1 / 2 / 3, whatever the floor): what the guard does to the runs that deliver.
    carry_ticks: u32,
    carry_lip_ticks: u32,
    carry_guard: [u32; 3],
    /// Sprint next to lips and drops (the motor cluster's check that a lip's, a drop's and a jump's release of the sprint still win
    /// over a sprint through a turn): bot ticks alive on a lip's floors, those with the sprint held, ticks on a drop's take-off floor
    /// with the sprint held, and the falls off the route whose take-off tick held the sprint.
    lip_ticks: u32,
    lip_sprint_ticks: u32,
    drop_sprint_ticks: u32,
    falls_sprinting: u32,
    /// Drops of 4 m or more in a second: with an unplanned fall (by link), and the rest.
    drops4_unplanned: BTreeMap<&'static str, u32>,
    drops4_rest: u32,
    /// The grid's links, for the grid of the first tick the topic saw (a run has one).
    links: Option<LinkCounts>,
}

impl GridCounts {
    pub(super) fn new(n: usize) -> Self {
        Self {
            route_drops: vec![Vec::new(); n],
            guard: vec![0; n],
            on_zone: vec![false; n],
            walking_out: vec![false; n],
            was_walking_out: vec![false; n],
            chosen: vec![false; n],
            still_ticks: vec![0; n],
            seconds: vec![Second::default(); n],
            ..Self::default()
        }
    }

    /// What `at`'s body fell from and by, if it fell a metre this tick.
    fn fall_of(&self, at: &BodyAt<'_>) -> Option<FallClass> {
        if !(at.prev.alive && at.rec.alive && at.prev.feet.y - at.rec.feet.y >= FALL_M) {
            return None;
        }
        let grid = &at.game.world.grid;
        let (from, to) = (grid.locate(at.prev.feet, 1.0), grid.locate(at.rec.feet, 1.0));
        let near = |metres: f32| self.route_drops[at.place].iter().any(|landing| landing.distance_xz(at.rec.feet) <= metres);
        Some(FallClass {
            planned: near(PLANNED_LANDING_M),
            flagged: from.is_some_and(|n| grid.is_lip(n) || grid.near_lip(n) || grid.near_fall(n)),
            walk_off: from.zip(to).is_some_and(|(a, b)| grid.off_link(a, b).is_some_and(|l| l.press == Takeoff::WalkOff)),
        })
    }

    fn fall_total(&self, planned: &str, flagged: &str) -> u32 {
        self.falls.iter().filter(|(k, _)| k.starts_with(&format!("{planned}_{flagged}"))).map(|(_, n)| n).sum()
    }

    /// The take-off of a walk-off, whose floor the guard watched.
    fn walk_off_floor(grid: &WalkGrid, node: mapdata::NodeId) -> bool {
        grid.links(node).any(|(to, kind)| kind == LinkKind::GapDrop && grid.off_link(node, to).is_some_and(|l| l.press == Takeoff::WalkOff))
    }

    /// The route's drops, for a test with no bot to read them from.
    #[cfg(test)]
    pub(super) fn set_route_drops(&mut self, place: Place, landings: Vec<Vec3>) {
        self.route_drops[place] = landings;
    }
}

impl Topic for GridCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn read(&mut self, place: Place, bot: Option<&Bot>, _tick: u32) {
        let Some(bot) = bot else {
            return;
        };
        self.route_drops[place] = bot.route.as_ref().map_or_else(Vec::new, |(route, _)| {
            route.points().windows(2).filter(|w| w[0].at.y - w[1].at.y >= FALL_M).map(|w| w[1].at).collect()
        });
        self.guard[place] = bot.bench_guard;
        self.walking_out[place] = bot.zone_walk.is_some();
        self.chosen[place] = bot.intent.zone_ok;
    }

    fn bots(&mut self, bots: &Bots, game: &Game) {
        let Some(tactics) = bots.tactics() else {
            return;
        };
        let grid = &game.world.grid;
        for (place, body) in game.bodies.iter().enumerate() {
            if let Some(on) = self.on_zone.get_mut(place) {
                *on = body.alive && grid.locate(body.feet, 1.0).is_some_and(|node| !tactics.hazards.allowed(node, game.tick));
            }
        }
    }

    fn body(&mut self, run: &Run, at: &BodyAt<'_>, _rays: &mut u64) {
        if !run.is_bot[at.place] {
            return;
        }
        let grid = &at.game.world.grid;
        if self.links.is_none() {
            self.links = Some(LinkCounts::of(grid));
        }
        let place = at.place;
        // The falls, and the lip guard on a walk-off's take-off.
        let fall = self.fall_of(at);
        if let Some(class) = fall {
            *self.falls.entry(class.key()).or_default() += 1;
            let shared = self.route_drops[place].iter().any(|landing| landing.distance_xz(at.rec.feet) <= SHARED_LANDING_M);
            self.planned_late += u32::from(class.planned && !shared);
            self.falls_sprinting += u32::from(!class.planned && at.prev.keys & key::SPRINT != 0);
        }
        if at.rec.alive
            && let Some(node) = grid.locate(at.rec.feet, 1.0)
        {
            let sprint = at.rec.keys & key::SPRINT != 0;
            if grid.is_lip(node) || grid.near_lip(node) {
                self.lip_ticks += 1;
                self.lip_sprint_ticks += u32::from(sprint);
            }
            self.drop_sprint_ticks +=
                u32::from(sprint && grid.links(node).any(|(_, kind)| matches!(kind, LinkKind::Drop | LinkKind::GapDrop)));
        }
        if at.rec.alive && at.rec.carrying {
            self.carry_ticks += 1;
            self.carry_lip_ticks += u32::from(grid.locate(at.rec.feet, 1.0).is_some_and(|n| grid.is_lip(n) || grid.near_lip(n)));
            if let Some(slot) = usize::from(self.guard[place]).checked_sub(1).filter(|&k| k < 3) {
                self.carry_guard[slot] += 1;
            }
        }
        if matches!(self.guard[place], 2 | 3)
            && let Some(node) = grid.locate(at.rec.feet, 1.0)
        {
            if Self::walk_off_floor(grid, node) {
                self.guard_walk_off += 1;
            } else if grid.is_lip(node) || grid.near_lip(node) {
                self.guard_other_lip += 1;
            }
        }
        // The drops of 4 m or more in a second (a whole second alive, as the shared moving count reads it).
        let second = &mut self.seconds[place];
        second.whole &= at.rec.alive;
        if let Some(class) = fall.filter(|class| !class.planned) {
            second.unplanned = Some(class);
        }
        if run.recs[place].len().is_multiple_of(TPS) {
            if second.whole && second.from_y - at.rec.feet.y >= DROP_M {
                match second.unplanned {
                    Some(class) => *self.drops4_unplanned.entry(if class.walk_off { "walk_off" } else { "drop" }).or_default() += 1,
                    None => self.drops4_rest += 1,
                }
            }
            *second = Second { from_y: at.rec.feet.y, whole: at.rec.alive, unplanned: None };
        }
        // The zone lines.
        let walking = self.walking_out[place];
        self.walk_entries += u32::from(walking && !self.was_walking_out[place]);
        self.walk_exits += u32::from(!walking && self.was_walking_out[place]);
        self.was_walking_out[place] = walking;
        let run_ticks = &mut self.still_ticks[place];
        if at.rec.alive && self.on_zone[place] {
            self.zone_ticks += 1;
            self.walk_ticks += u32::from(walking);
            if at.rec.speed < STILL_MPS {
                self.zone_still_ticks += 1;
                *run_ticks += 1;
                if *run_ticks == STAND_TICKS {
                    self.stands += 1;
                    let play = run.plays.get(usize::from(at.rec.play)).map_or("?", String::as_str);
                    let key = format!("{play}/{}", if self.chosen[place] { "chosen" } else { "not_chosen" });
                    *self.stands_by.entry(key).or_default() += 1;
                }
                return;
            }
        }
        *run_ticks = 0;
    }

    fn parts(&mut self, run: &Run, game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        let grid = &game.world.grid;
        let body = grid.body();
        // Route jump presses by the link's rise against the cap: step, standing, capped band, sprint, over a gap.
        let mut presses = [0u32; 5];
        for press in run.presses.iter().filter(|p| p.by == "route") {
            if let Some((take_off, landing)) = press.route {
                let rise = landing.y - take_off.y;
                let gap = grid.locate(take_off, 0.5).zip(grid.locate(landing, 0.5)).is_some_and(|(a, b)| grid.off_link(a, b).is_some());
                let band = if gap {
                    4
                } else if rise <= body.step {
                    0
                } else if rise <= body.max_jump {
                    1
                } else if rise <= exe_top_m() {
                    2
                } else {
                    3
                };
                presses[band] += 1;
            }
        }
        let links = self.links.unwrap_or_else(|| LinkCounts::of(grid));
        let unplanned = self.fall_total("unplanned", "");
        let planned = self.fall_total("planned", "");
        let drops4_unplanned: u32 = self.drops4_unplanned.values().sum();
        let json = serde_json::json!({
            "grid_falls": self.falls,
            "grid_falls_unplanned": unplanned,
            "grid_falls_unplanned_unflagged": self.fall_total("unplanned", "unflagged"),
            "grid_falls_planned": planned,
            "grid_falls_planned_late": self.planned_late,
            "grid_guard_walk_off_ticks": self.guard_walk_off,
            "grid_guard_other_lip_ticks": self.guard_other_lip,
            "grid_carry_ticks": self.carry_ticks,
            "grid_carry_lip_ticks": self.carry_lip_ticks,
            "grid_carry_guard_ticks": self.carry_guard,
            "grid_lip_ticks": self.lip_ticks,
            "grid_lip_sprint_ticks": self.lip_sprint_ticks,
            "grid_drop_sprint_ticks": self.drop_sprint_ticks,
            "grid_falls_unplanned_sprinting": self.falls_sprinting,
            "grid_drops4_unplanned": self.drops4_unplanned,
            "grid_drops4_rest": self.drops4_rest,
            "grid_route_jump_presses_step": presses[0],
            "grid_route_jump_presses_standing": presses[1],
            "grid_route_jump_presses_capped": presses[2],
            "grid_route_jump_presses_sprint": presses[3],
            "grid_route_jump_presses_gap": presses[4],
            "grid_links_jump_standing": links.jump_standing,
            "grid_links_jump_capped": links.jump_capped,
            "grid_links_jump_sprint": links.jump_sprint,
            "grid_links_walk_rises_over_exe_step": links.walk_rise_over_exe_step,
            "grid_links_gap_jump": links.gap_jump,
            "grid_links_walk_off": links.walk_off,
            "grid_links_walk_off_lips": links.walk_off_lips,
            "grid_links_gap_drop_pressed": links.press_gap_drop,
            "grid_zone_bot_ticks": self.zone_ticks,
            "grid_zone_still_ticks": self.zone_still_ticks,
            "grid_zone_stands_2s": self.stands,
            "grid_zone_stands_by": self.stands_by,
            "grid_zone_walk_entries": self.walk_entries,
            "grid_zone_walk_exits": self.walk_exits,
            "grid_zone_walk_ticks": self.walk_ticks,
        });
        let text = format!(
            "owner-test grid: falls off the route {unplanned} (from a flagged floor {}, an unflagged one {}; by class {:?}), planned {planned} ({} only within {PLANNED_LANDING_M} m); lip guard on walk-off take-offs {} ticks, other lips {}; drops of 4 m or more in a second: off the route {drops4_unplanned} {:?}, the rest {}; route jump presses by rise: step {}, standing {}, capped band {}, sprint {}, over a gap {}; links: jump {} standing {} capped {} sprint, {} walk rises of 0.6 m to the step, {} gap jumps, {} walk-offs ({} take-offs are lips); zone lines: {} entries {} exits, {} ticks; zone floors: {} bot-ticks ({} standing still, {} stands of 2 s or more {:?}); carriers: {} ticks, {} on lip floors, lip guard safe / swapped / braked {:?}; sprint: {} of {} lip-floor ticks, {} on drop take-offs, {} falls off the route sprinting",
            self.fall_total("unplanned", "flagged"),
            self.fall_total("unplanned", "unflagged"),
            self.falls,
            self.planned_late,
            self.guard_walk_off,
            self.guard_other_lip,
            self.drops4_unplanned,
            self.drops4_rest,
            presses[0],
            presses[1],
            presses[2],
            presses[3],
            presses[4],
            links.jump_standing,
            links.jump_capped,
            links.jump_sprint,
            links.walk_rise_over_exe_step,
            links.gap_jump,
            links.walk_off,
            links.walk_off_lips,
            self.walk_entries,
            self.walk_exits,
            self.walk_ticks,
            self.zone_ticks,
            self.zone_still_ticks,
            self.stands,
            self.stands_by,
            self.carry_ticks,
            self.carry_lip_ticks,
            self.carry_guard,
            self.lip_sprint_ticks,
            self.lip_ticks,
            self.drop_sprint_ticks,
            self.falls_sprinting,
        );
        vec![("grid", Part::new(json, text))]
    }
}

/// A bot's fall of 5 m in a tick is counted as the route's own (a corner of its drops within [`PLANNED_LANDING_M`]) or off it,
/// by the start floor and the link taken, and its 4 m drop in a second by the same; a body that is no bot's is not counted.
#[test]
pub(crate) fn falls_and_drops_are_counted_by_plan_start_floor_and_link() {
    let (mut game, mut tally) = made_up(&[0, 0, 0]);
    // Place 0's route drops to where it lands, place 1's lands 1.5 m past its drop (a sprint walk-off), place 2 is no bot's.
    tally.topic::<GridCounts>().set_route_drops(0, vec![Vec3::new(10.0, 0.0, 10.0)]);
    tally.topic::<GridCounts>().set_route_drops(1, vec![Vec3::new(10.0, 0.0, 18.5)]);
    play(&mut game, &mut tally, 60, |k, game| {
        for place in 0..3 {
            put(game, place, 10.0, 10.0 + 10.0 * place as f32, 0);
            game.bodies[place].feet.y = if k < 30 { 6.0 } else { 1.0 };
        }
        vec![bot_now(), bot_now(), None]
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["grid_falls_planned"], 2, "{json}");
    assert_eq!(json["grid_falls_planned_late"], 1, "the 1.5 m landing is the route's only within 2 m: {json}");
    assert_eq!(json["grid_falls_unplanned"], 0, "{json}");
    assert_eq!(json["grid_drops4_rest"], 2, "{json}");
    assert_eq!(json["grid_drops4_unplanned"], serde_json::json!({}), "{json}");
    // No route drop: the same fall is off the route, from an unflagged floor, by a Drop, and so is the 4 m drop.
    let (mut game, mut tally) = made_up(&[0]);
    play(&mut game, &mut tally, 60, |k, game| {
        put(game, 0, 10.0, 10.0, 0);
        game.bodies[0].feet.y = if k < 30 { 6.0 } else { 1.0 };
        vec![bot_now()]
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["grid_falls"]["unplanned_unflagged_drop"], 1, "{json}");
    assert_eq!(json["grid_falls_unplanned_unflagged"], 1, "{json}");
    assert_eq!(json["grid_drops4_unplanned"]["drop"], 1, "{json}");
}
