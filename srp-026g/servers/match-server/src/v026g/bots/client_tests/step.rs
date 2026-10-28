//! The step: one client test, as data (see the module docs for the API).

use mapdata::Vec3;

use super::frame::{Body, Frame};

/// The longest a step may run once started, ms (the owner's rule: at most 3 min a step).
pub const MAX_BUDGET_MS: u32 = 180_000;
/// How long the owner has to start a step (reach its start condition) before the try is void, ms.
pub const DEFAULT_ARM_MS: u32 = 120_000;
/// How many bots of a side a script may name at most.
const MAX_NTH: u8 = 32;

/// The names the owner uses (`docs/notes/modes_maps.md`).
pub fn map_name(level: u8) -> &'static str {
    match level {
        1 => "Rudnya",
        2 => "School",
        3 => "Vostok Radar Station",
        4 => "Chemical Plant",
        5 => "Tarakanovsky Fort",
        6 => "Mamayev Kurgan",
        7 => "Vector Laboratory",
        8 => "Cologne",
        _ => "an unknown map",
    }
}

/// The mode ids of the game data.
pub const BATTERIES: u8 = 2;
pub const TIMED_BATTERIES: u8 = 3;
pub const TEAM_DEATHMATCH: u8 = 4;

pub fn mode_name(mode: u8) -> &'static str {
    match mode {
        BATTERIES => "batteries",
        TIMED_BATTERIES => "timed batteries",
        TEAM_DEATHMATCH => "team deathmatch",
        _ => "an unknown mode",
    }
}

/// How many places a team has in the match.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Size {
    /// A step that runs in a match of any size.
    Any,
    /// Places in the larger team.
    PerTeam(u8),
}

/// Where a step runs: a map, a mode and a size. A match change costs the owner a trip to the lobby, so steps are run
/// grouped by this (map first, then mode, then size).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Group {
    pub level: u8,
    pub mode: u8,
    pub size: Size,
}

impl Group {
    pub const fn new(level: u8, mode: u8) -> Self {
        Self { level, mode, size: Size::Any }
    }

    pub const fn of_size(self, per_team: u8) -> Self {
        Self { size: Size::PerTeam(per_team), ..self }
    }

    /// Whether a step of this group runs in `played` (the match's own group, whose size is always a number).
    pub fn fits(self, played: Group) -> bool {
        self.level == played.level
            && self.mode == played.mode
            && match self.size {
                Size::Any => true,
                Size::PerTeam(_) => self.size == played.size,
            }
    }

    /// For the owner: "School, batteries, 6 a side".
    pub fn describe(self) -> String {
        let size = match self.size {
            Size::Any => String::new(),
            Size::PerTeam(n) => format!(", {n} a side"),
        };
        format!("{}, {}{size}", map_name(self.level), mode_name(self.mode))
    }
}

/// A point on the map, x y z.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot(pub [f32; 3]);

impl Spot {
    pub const fn at(x: f32, y: f32, z: f32) -> Self {
        Self([x, y, z])
    }

    pub fn vec(self) -> Vec3 {
        Vec3::from_array(self.0)
    }
}

/// Which side a bot is on, as the owner sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// The owner's team (the bots in it).
    Mates,
    /// The other team.
    Enemies,
}

impl Side {
    pub fn name(self) -> &'static str {
        match self {
            Side::Mates => "mate",
            Side::Enemies => "enemy",
        }
    }
}

/// Which bots a setup is for. Bots of a side are numbered by their place, from 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Who {
    Nth(Side, u8),
    All(Side),
}

/// How fast a scripted bot goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gait {
    Walk,
    Run,
}

/// What the server makes a bot do while a step lasts. A scripted bot plays no ordinary play; its reflex still aims and
/// fires on sight, except for [`Script::Idle`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Script {
    /// Stands where it is and never fires: a target dummy.
    Idle,
    /// Stands where it is, crouched or not; fires on sight.
    Stand { crouch: bool },
    /// Goes to `at` and holds it (within `within_m`), crouched once there if told; fires on sight and never leaves.
    Hold { at: Spot, within_m: f32, crouch: bool },
    /// Walks the points one after another (`looped`: round again, else stops at the last); fires on sight.
    Walk { lane: &'static [Spot], gait: Gait, looped: bool },
    /// Keeps within `within_m` of the owner.
    Follow { within_m: f32, gait: Gait },
    /// Looks at `at` with the throw key held for `wind_ms`, then lets go: one grenade, if it carries one. A flat lob
    /// (GUESS: the throw's speed is the client's); read where it landed with a measure.
    Throw { at: Spot, wind_ms: u32 },
}

/// Setup the server does for a step: `who` follows `script`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Setup {
    pub who: Who,
    pub script: Script,
}

impl Setup {
    pub const fn new(who: Who, script: Script) -> Self {
        Self { who, script }
    }
}

/// A condition of the start; the step starts when all of them hold at once.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cond {
    /// The owner, alive, within `r_m` (flat) of `at`.
    OwnerNear { at: Spot, r_m: f32 },
    /// Every bot with a [`Script::Hold`] is at its spot.
    BotsInPlace,
    /// This long since the step was armed (its instruction told), ms.
    After { ms: u32 },
}

/// A condition of the end; the step ends when the first of them holds, or at its budget.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum End {
    OwnerDied,
    /// The owner has made this many kills since the start.
    OwnerKills(u32),
    /// The owner is further than `r_m` (flat) from `at`.
    OwnerLeft { at: Spot, r_m: f32 },
    /// Any bot of `who` that is scripted died since the start.
    ScriptedDied(Who),
}

/// What the server measures of a try, from its own view: the world's positions, health and statistics and the bots' notes.
/// A measure has a name; a [`Bar`] tests it.
#[derive(Clone, Copy, Debug)]
pub enum Measure {
    ElapsedMs,
    OwnerKills,
    OwnerDeaths,
    /// Time from the start to the owner's first kill, ms (none: no kill).
    FirstKillMs,
    /// Time from the start to the owner's death, ms (none: alive at the end).
    DiedAtMs,
    /// The lowest health of the owner, [0, 1].
    MinHealth,
    /// Metres the owner walked (flat).
    TravelM,
    /// Kills the bots of a side made.
    SideKills(Side),
    /// The nearest the owner came to a living bot of a side, m.
    NearestBotM(Side),
    /// Bot notes (the server's log lines about a bot) that contain `contains`.
    Notes { name: &'static str, contains: &'static str },
    /// Seconds with at least one living bot of `side` within `r_m` (flat) of `at`.
    SecondsInArea { name: &'static str, side: Side, at: Spot, r_m: f32 },
    /// A measure of the package's own: `sample` sees every frame of the try and keeps what it needs in the scratch,
    /// `value` reads the result.
    Custom { name: &'static str, sample: fn(&Frame, &mut Scratch), value: fn(&Scratch) -> Option<f64> },
}

impl Measure {
    pub fn name(&self) -> &'static str {
        match self {
            Measure::ElapsedMs => "elapsed_ms",
            Measure::OwnerKills => "owner_kills",
            Measure::OwnerDeaths => "owner_deaths",
            Measure::FirstKillMs => "first_kill_ms",
            Measure::DiedAtMs => "died_at_ms",
            Measure::MinHealth => "min_health",
            Measure::TravelM => "travel_m",
            Measure::SideKills(Side::Mates) => "mate_kills",
            Measure::SideKills(Side::Enemies) => "enemy_kills",
            Measure::NearestBotM(Side::Mates) => "nearest_mate_m",
            Measure::NearestBotM(Side::Enemies) => "nearest_enemy_m",
            Measure::Notes { name, .. } | Measure::SecondsInArea { name, .. } | Measure::Custom { name, .. } => name,
        }
    }
}

/// What a [`Measure::Custom`] keeps between frames.
#[derive(Clone, Debug, Default)]
pub struct Scratch {
    pub numbers: Vec<f64>,
    /// The try's start on the match clock, ms.
    pub started: u32,
    /// The owner's kills and deaths at the start.
    pub base: (u32, u32),
}

/// What a bar asks of a measure's value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Test {
    AtLeast(f64),
    AtMost(f64),
    Within(f64, f64),
}

impl Test {
    pub fn holds(self, value: f64) -> bool {
        match self {
            Test::AtLeast(low) => value >= low,
            Test::AtMost(high) => value <= high,
            Test::Within(low, high) => (low..=high).contains(&value),
        }
    }

    pub fn describe(self) -> String {
        match self {
            Test::AtLeast(low) => format!(">= {low}"),
            Test::AtMost(high) => format!("<= {high}"),
            Test::Within(low, high) => format!("{low}..{high}"),
        }
    }
}

/// The pass bar: the measure `measure` (by name) must hold `test`; a measure with no value fails it.
#[derive(Clone, Copy, Debug)]
pub struct Bar {
    pub measure: &'static str,
    pub test: Test,
}

impl Bar {
    pub const fn new(measure: &'static str, test: Test) -> Self {
        Self { measure, test }
    }
}

/// One client test.
#[derive(Clone, Debug)]
pub struct Step {
    /// Unique across all packages: `<package>-<what>`.
    pub id: &'static str,
    /// Set by the registry from the file the step lives in.
    pub package: &'static str,
    pub group: Group,
    /// One line, said to the owner when the step is armed.
    pub instruction: &'static str,
    pub setup: Vec<Setup>,
    pub start: Vec<Cond>,
    pub end: Vec<End>,
    pub measures: Vec<Measure>,
    pub bars: Vec<Bar>,
    /// How many tries are run (a row each).
    pub tries: u8,
    /// The longest the step runs once started, ms.
    pub budget_ms: u32,
    /// The longest the owner has to start it, ms.
    pub arm_ms: u32,
}

impl Step {
    /// A step with no setup, starting at once, ending at its budget (60 s), one try.
    pub fn new(id: &'static str, group: Group, instruction: &'static str) -> Self {
        Self {
            id,
            package: "",
            group,
            instruction,
            setup: Vec::new(),
            start: vec![Cond::After { ms: 0 }],
            end: Vec::new(),
            measures: Vec::new(),
            bars: Vec::new(),
            tries: 1,
            budget_ms: 60_000,
            arm_ms: DEFAULT_ARM_MS,
        }
    }

    pub fn setup(mut self, who: Who, script: Script) -> Self {
        self.setup.push(Setup::new(who, script));
        self
    }

    /// Replaces the start condition.
    pub fn start(mut self, conds: &[Cond]) -> Self {
        self.start = conds.to_vec();
        self
    }

    pub fn end(mut self, ends: &[End]) -> Self {
        self.end = ends.to_vec();
        self
    }

    pub fn measure(mut self, measure: Measure) -> Self {
        self.measures.push(measure);
        self
    }

    pub fn bar(mut self, measure: &'static str, test: Test) -> Self {
        self.bars.push(Bar::new(measure, test));
        self
    }

    pub fn tries(mut self, tries: u8) -> Self {
        self.tries = tries;
        self
    }

    pub fn budget_s(mut self, seconds: u32) -> Self {
        self.budget_ms = seconds * 1_000;
        self
    }

    pub fn arm_s(mut self, seconds: u32) -> Self {
        self.arm_ms = seconds * 1_000;
        self
    }

    /// Why this step cannot be run, if so. The registry's test checks every registered step with it.
    pub fn check(&self) -> Result<(), String> {
        let bad = |why: &str| Err(format!("step {}: {why}", self.id));
        if self.id.is_empty() || !self.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
            return bad("the id is lowercase letters, digits and dashes");
        }
        if self.instruction.is_empty() || self.instruction.contains('\n') || self.instruction.len() > 160 {
            return bad("the instruction is one line of at most 160 characters");
        }
        if self.budget_ms == 0 || self.budget_ms > MAX_BUDGET_MS {
            return bad("the budget is 1 s to 3 min");
        }
        if self.tries == 0 {
            return bad("it runs at least one try");
        }
        if !(1..=8).contains(&self.group.level) || !matches!(self.group.mode, BATTERIES | TIMED_BATTERIES | TEAM_DEATHMATCH) {
            return bad("the group is a map 1-8 and a mode 2, 3 or 4");
        }
        if self.start.is_empty() {
            return bad("it needs a start condition (`Cond::After { ms: 0 }` for none)");
        }
        let names: Vec<&str> = self.measures.iter().map(Measure::name).collect();
        if let Some(dup) = names.iter().enumerate().find(|(i, n)| names[..*i].contains(n)) {
            return bad(&format!("the measure {} is declared twice", dup.1));
        }
        if let Some(bar) = self.bars.iter().find(|bar| !names.contains(&bar.measure)) {
            return bad(&format!("a bar names the measure {}, which the step does not declare", bar.measure));
        }
        let nth = |who: &Who| matches!(who, Who::Nth(_, n) if *n >= MAX_NTH);
        if self.setup.iter().any(|s| nth(&s.who)) || self.end.iter().any(|e| matches!(e, End::ScriptedDied(w) if nth(w))) {
            return bad("a bot number is out of range");
        }
        if self.start.iter().any(|c| matches!(c, Cond::BotsInPlace)) && !self.setup.iter().any(|s| matches!(s.script, Script::Hold { .. })) {
            return bad("`BotsInPlace` needs a bot with a `Hold` script");
        }
        Ok(())
    }
}

/// The distance in the ground plane between a body and a point.
pub fn flat(body: &Body, at: Spot) -> f32 {
    body.feet.distance_xz(at.vec())
}
