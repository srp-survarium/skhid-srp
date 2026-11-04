//! A294: a bot's drift (its reckoned place against the client's world) as log lines with a likely cause, and a match-end table.
//!
//! Between two worlds a bot walks on its own reckoning (`body.rs`: its keys on the walk grid); each world puts it right. The only
//! log of the gap was "was N m off its reckoning; plans again" at the replan threshold, so a bot that walked into things on a
//! wrong idea of where it was (A289, fx-grid: Vector Laboratory bot 13, 2.4-2.8 m off during a crouch-walk) left no trace of how
//! or why. Now, at each world of a bot that differs from its reckoning for that tick by [`DRIFT_M`] or more (flat or in height):
//!
//! `nav: drift D m (flat F, height H) at (x, y, z), reckoned (x, y, z): CAUSE; play P, speed S m/s, STANCE, moved M of N m meant[,
//! in a jump][, body B m][, world after G ms]`
//!
//! The cause is the first of these that holds ([`Cause::of`]; each reads only what the bot holds at that world):
//!
//! | word | when | grade |
//! |---|---|---|
//! | `landing` | a jump the bot pressed is still watched, or the feet dropped [`super::nav_log::FALL_M`]+ since the last world | the jump or drop is PROVEN; that it made the error is LIKELY |
//! | `late world` | the world came more than [`LATE_MS`] after the last one of this life (the reckoning ran on alone that long) | the gap is PROVEN; the cause LIKELY |
//! | `body` | another live body stands within [`BODY_M`] on the same floor (`MatchView::neighbour_of`): pushed or held | LIKELY |
//! | `blocked` | the keys meant [`BLOCKED_MEANT_M`]+ of walking and the world moved the body under [`BLOCKED_SHARE`] of it: held by something the model walks through (an object the map data lacks, a lip, a railing) | heuristic |
//! | `floor` | the error is mostly height ([`FLOOR_RISE_M`]+, flat under [`DRIFT_M`]): the model put the body on another floor (a step or lip taken differently) | heuristic |
//! | `faster` | the world moved the body [`FASTER_M`] or more further than the keys meant (a slide, a push, a speed the model lacks) | heuristic |
//! | `crouch` | the world shows crouch held (the crouch-walk speed or its toggles against the model) | heuristic |
//! | `other` | none of these | - |
//!
//! Decision-neutral: built from what the bot already holds, written to the notes only (the bench's decision hash skips `nav: ` lines).
//! One line per bot per [`super::nav_log::EVERY_MS`], the held ones counted. Every drifting world also goes into the bot's
//! [`Tally`]; [`report`] gives the match-end lines.

use std::collections::HashMap;

use mapdata::Vec3;

/// A reckoning this far off a world's feet (flat or in height) is a drift, m (the replan threshold is 1.0 too).
pub const DRIFT_M: f32 = 1.0;
/// A world this long after the last one of the same life let the reckoning run alone too long, ms (one client: 2 s).
pub const LATE_MS: u32 = 2_500;
/// Another body this near, flat, on the same floor, m: two capsules (0.38 m) touch at 0.76.
pub const BODY_M: f32 = 0.9;
/// The keys meant at least this much walking for `blocked`, m.
pub const BLOCKED_MEANT_M: f32 = 1.0;
/// ... and the body moved under this share of it.
pub const BLOCKED_SHARE: f32 = 0.4;
/// The height error for `floor`, m (more than a stair's rise of noise).
pub const FLOOR_RISE_M: f32 = 0.5;
/// The world moved the body this much further than the keys meant for `faster`, m.
pub const FASTER_M: f32 = 1.0;
/// The map cells of the table, m.
pub const CELL_M: f32 = 4.0;
/// The table's worst cells.
const WORST_CELLS: usize = 10;

/// The likely cause of a drift.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cause {
    Landing,
    LateWorld,
    Body,
    Blocked,
    Floor,
    Faster,
    Crouch,
    Other,
}

impl Cause {
    pub const ALL: [Cause; 8] =
        [Cause::Landing, Cause::LateWorld, Cause::Body, Cause::Blocked, Cause::Floor, Cause::Faster, Cause::Crouch, Cause::Other];

    pub fn word(self) -> &'static str {
        match self {
            Cause::Landing => "landing",
            Cause::LateWorld => "late world",
            Cause::Body => "body",
            Cause::Blocked => "blocked",
            Cause::Floor => "floor",
            Cause::Faster => "faster",
            Cause::Crouch => "crouch",
            Cause::Other => "other",
        }
    }

    /// The first cause that holds, in the order of the module's table.
    pub fn of(f: &Facts) -> Self {
        let (flat, rise) = f.error();
        if f.in_jump || f.dropped_m >= super::nav_log::FALL_M {
            Cause::Landing
        } else if f.gap_ms.is_some_and(|gap| gap > LATE_MS) {
            Cause::LateWorld
        } else if f.body_m.is_some_and(|m| m <= BODY_M) {
            Cause::Body
        } else if f.meant_m.zip(f.moved_m).is_some_and(|(meant, moved)| meant >= BLOCKED_MEANT_M && moved < BLOCKED_SHARE * meant) {
            Cause::Blocked
        } else if rise.abs() >= FLOOR_RISE_M && flat < DRIFT_M {
            Cause::Floor
        } else if f.meant_m.zip(f.moved_m).is_some_and(|(meant, moved)| moved >= meant + FASTER_M) {
            Cause::Faster
        } else if f.crouch {
            Cause::Crouch
        } else {
            Cause::Other
        }
    }
}

/// What the bot holds at a world, for one drift.
#[derive(Clone, Debug, PartialEq)]
pub struct Facts {
    pub world: Vec3,
    pub reckoned: Vec3,
    /// The world's interval since the last one of this life, ms.
    pub gap_ms: Option<u32>,
    /// How far the feet dropped since the last world, m (0 when they rose).
    pub dropped_m: f32,
    /// A jump the bot pressed is still watched.
    pub in_jump: bool,
    /// The nearest other live body on its floor, flat, m.
    pub body_m: Option<f32>,
    /// The way the keys meant since the last world (the reckoning's), and how far the world moved the body, flat, m.
    pub meant_m: Option<f32>,
    pub moved_m: Option<f32>,
    pub crouch: bool,
    pub sprint: bool,
    pub speed: Option<f32>,
    /// The bot's play then (its kind), if any.
    pub play: Option<String>,
}

impl Facts {
    /// (flat, height) of the error; height is the reckoning's above the world's.
    pub fn error(&self) -> (f32, f32) {
        (self.reckoned.distance_xz(self.world), self.reckoned.y - self.world.y)
    }

    pub fn drifts(&self) -> bool {
        let (flat, rise) = self.error();
        flat >= DRIFT_M || rise.abs() >= DRIFT_M
    }

    pub fn line(&self, cause: Cause) -> String {
        let (flat, rise) = self.error();
        let size = flat.hypot(rise);
        let stance = if self.crouch {
            "crouched"
        } else if self.sprint {
            "sprinting"
        } else {
            "standing"
        };
        let mut text = format!(
            "{}drift {size:.1} m (flat {flat:.1}, height {rise:+.1}) at {}, reckoned {}: {}; play {}, speed {}, {stance}",
            super::nav_log::PREFIX,
            at(self.world),
            at(self.reckoned),
            cause.word(),
            self.play.as_deref().unwrap_or("none"),
            self.speed.map_or("?".to_string(), |s| format!("{s:.1} m/s")),
        );
        if let (Some(meant), Some(moved)) = (self.meant_m, self.moved_m) {
            text.push_str(&format!(", moved {moved:.1} of {meant:.1} m meant"));
        }
        if self.in_jump {
            text.push_str(", in a jump");
        }
        if let Some(m) = self.body_m.filter(|m| *m <= 2.0 * BODY_M) {
            text.push_str(&format!(", body {m:.1} m"));
        }
        if let Some(gap) = self.gap_ms.filter(|g| *g > LATE_MS) {
            text.push_str(&format!(", world after {gap} ms"));
        }
        text
    }
}

fn at(p: Vec3) -> String {
    format!("({:.1}, {:.1}, {:.1})", p.x, p.y, p.z)
}

/// A bot's drift over the match: time drifting, the largest error, the worlds by cause, and where drifts began.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tally {
    pub drift_ms: u32,
    pub max_m: f32,
    pub by_cause: [u32; 8],
    /// Drifts begun (a drifting world after one that was not, or the life's first) by map cell of [`CELL_M`].
    pub began: HashMap<(i32, i32), u32>,
    /// Whether its last world drifted.
    drifting: bool,
}

impl Tally {
    /// A world of this bot: `facts` when it drifted (with its cause), none when it did not.
    pub fn world(&mut self, drift: Option<(&Facts, Cause)>) {
        match drift {
            None => self.drifting = false,
            Some((f, cause)) => {
                let (flat, rise) = f.error();
                self.max_m = self.max_m.max(flat.hypot(rise));
                self.by_cause[cause as usize] += 1;
                if self.drifting {
                    self.drift_ms += f.gap_ms.unwrap_or(0);
                } else {
                    *self.began.entry(cell(f.world)).or_default() += 1;
                }
                self.drifting = true;
            }
        }
    }

    /// A new life: the last world's drift does not run on.
    pub fn new_life(&mut self) {
        self.drifting = false;
    }

    fn worlds(&self) -> u32 {
        self.by_cause.iter().sum()
    }
}

fn cell(p: Vec3) -> (i32, i32) {
    ((p.x / CELL_M).floor() as i32, (p.z / CELL_M).floor() as i32)
}

/// The match-end lines: one per bot that drifted (worlds drifting, time over [`DRIFT_M`], the largest, by cause), and the cells where
/// most drifts began (their centre, the drifts begun there, by how many bots).
pub fn report(tallies: &[(usize, &Tally)]) -> Vec<String> {
    let mut lines = Vec::new();
    let mut bots: Vec<&(usize, &Tally)> = tallies.iter().filter(|(_, t)| t.worlds() > 0).collect();
    bots.sort_by(|a, b| b.1.drift_ms.cmp(&a.1.drift_ms).then(a.0.cmp(&b.0)));
    for (place, t) in &bots {
        let causes: Vec<String> = Cause::ALL
            .iter()
            .filter(|c| t.by_cause[**c as usize] > 0)
            .map(|c| format!("{} {}", c.word(), t.by_cause[*c as usize]))
            .collect();
        lines.push(format!(
            "drift of player {place}: {} worlds, {:.0} s over {DRIFT_M} m, largest {:.1} m; {}",
            t.worlds(),
            t.drift_ms as f32 / 1000.0,
            t.max_m,
            causes.join(", ")
        ));
    }
    let mut cells: HashMap<(i32, i32), (u32, u32)> = HashMap::new();
    for (_, t) in tallies {
        for (c, n) in &t.began {
            let e = cells.entry(*c).or_default();
            e.0 += n;
            e.1 += 1;
        }
    }
    let mut worst: Vec<((i32, i32), (u32, u32))> = cells.into_iter().collect();
    worst.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(a.0.cmp(&b.0)));
    if !worst.is_empty() {
        let text: Vec<String> = worst
            .iter()
            .take(WORST_CELLS)
            .map(|((x, z), (n, bots))| {
                format!("({:.0}, {:.0}) {n} by {bots}", (*x as f32 + 0.5) * CELL_M, (*z as f32 + 0.5) * CELL_M)
            })
            .collect();
        lines.push(format!("drift starts by {CELL_M} m cell (x, z), most first: {}", text.join("; ")));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> Facts {
        Facts {
            world: Vec3::new(11.35, 16.68, -20.02),
            reckoned: Vec3::new(11.0, 17.81, -22.5),
            gap_ms: Some(2_000),
            dropped_m: 0.0,
            in_jump: false,
            body_m: None,
            meant_m: Some(2.0),
            moved_m: Some(1.5),
            crouch: false,
            sprint: false,
            speed: Some(2.6),
            play: Some("Collect".into()),
        }
    }

    #[test]
    fn a_drift_is_a_metre_off_flat_or_in_height() {
        let mut f = facts();
        assert!(f.drifts());
        f.reckoned = Vec3::new(11.8, 16.9, -20.02);
        assert!(!f.drifts(), "0.45 m flat, 0.22 m up");
        f.reckoned = Vec3::new(11.35, 17.7, -20.02);
        assert!(f.drifts(), "1.02 m up, none flat");
    }

    #[test]
    fn the_cause_is_the_first_rule_that_holds() {
        let f = facts();
        assert_eq!(Cause::of(&Facts { in_jump: true, body_m: Some(0.5), ..f.clone() }), Cause::Landing);
        assert_eq!(Cause::of(&Facts { dropped_m: 1.6, ..f.clone() }), Cause::Landing);
        assert_eq!(Cause::of(&Facts { gap_ms: Some(4_000), body_m: Some(0.5), ..f.clone() }), Cause::LateWorld);
        assert_eq!(Cause::of(&Facts { body_m: Some(0.8), ..f.clone() }), Cause::Body);
        assert_eq!(Cause::of(&Facts { body_m: Some(1.2), moved_m: Some(0.3), ..f.clone() }), Cause::Blocked);
        let floor = Facts { reckoned: Vec3::new(11.6, 17.81, -20.02), ..f.clone() };
        assert_eq!(Cause::of(&floor), Cause::Floor);
        assert_eq!(Cause::of(&Facts { moved_m: Some(3.2), ..f.clone() }), Cause::Faster);
        assert_eq!(Cause::of(&Facts { crouch: true, ..f.clone() }), Cause::Crouch);
        assert_eq!(Cause::of(&f), Cause::Other);
        // No keys' way known: neither blocked nor faster.
        assert_eq!(Cause::of(&Facts { meant_m: None, moved_m: Some(0.0), crouch: true, ..f }), Cause::Crouch);
    }

    #[test]
    fn the_line_says_where_how_much_and_why() {
        let f = Facts { crouch: true, body_m: Some(1.5), ..facts() };
        assert_eq!(
            f.line(Cause::of(&f)),
            "nav: drift 2.7 m (flat 2.5, height +1.1) at (11.3, 16.7, -20.0), reckoned (11.0, 17.8, -22.5): crouch; play Collect, \
             speed 2.6 m/s, crouched, moved 1.5 of 2.0 m meant, body 1.5 m"
        );
        let late = Facts { gap_ms: Some(3_100), in_jump: true, play: None, speed: None, meant_m: None, ..facts() };
        assert!(late.line(Cause::of(&late)).ends_with(": landing; play none, speed ?, standing, in a jump, world after 3100 ms"));
    }

    #[test]
    fn the_table_counts_time_causes_and_the_cells_drifts_began_in() {
        let f = facts();
        let mut a = Tally::default();
        a.world(Some((&f, Cause::Crouch)));
        a.world(Some((&f, Cause::Crouch)));
        a.world(Some((&Facts { reckoned: Vec3::new(14.0, 16.68, -20.02), ..f.clone() }, Cause::Body)));
        a.world(None);
        a.world(Some((&f, Cause::Blocked)));
        // Two drifting worlds after the first: 2 x 2 s; three starts, all in the cell (8..12, -24..-20).
        assert_eq!(a.drift_ms, 4_000);
        assert_eq!(a.by_cause[Cause::Crouch as usize], 2);
        assert_eq!(a.began.get(&(2, -6)), Some(&2));
        let mut b = Tally::default();
        b.world(Some((&Facts { world: Vec3::new(57.1, 20.9, -3.8), ..f.clone() }, Cause::Floor)));
        b.new_life();
        b.world(Some((&Facts { world: Vec3::new(11.0, 16.68, -21.0), ..f }, Cause::Other)));
        let quiet = Tally::default();
        let lines = report(&[(13, &a), (4, &b), (2, &quiet)]);
        assert_eq!(lines.len(), 3, "{lines:?}");
        assert_eq!(lines[0], "drift of player 13: 4 worlds, 4 s over 1 m, largest 2.7 m; body 1, blocked 1, crouch 2");
        assert!(lines[1].starts_with("drift of player 4: 2 worlds, 0 s over 1 m"), "{lines:?}");
        assert_eq!(lines[2], "drift starts by 4 m cell (x, z), most first: (10, -22) 3 by 2; (58, -2) 1 by 1");
    }
}
