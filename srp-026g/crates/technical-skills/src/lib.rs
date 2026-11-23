//! The technical tree's skills (docs/notes/technical-skills.md): the fourth skills tab, its
//! skills packed in kit modifier 47 (`spot_enemy_traps`, which the exe never reads and
//! no config sets), parsed once here for exe patch 180 and srp alike. Pure Rust, no game
//! bindings: each skill's numbers live here and nowhere else.
#![cfg_attr(not(test), no_std)]

pub mod dead_mans_switch;
pub mod smoke;
pub mod throw_back;

/// Kit modifier 47, `spot_enemy_traps`: the packed ranks.
pub const MODIFIER: usize = 47;

/// The ranks of a skill of `MOST` ranks, 0..=MOST.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Rank<const MOST: u8>(u8);

impl<const MOST: u8> Rank<MOST> {
    fn parse(bits: u32) -> Option<Self> {
        let rank = u8::try_from(bits).ok()?;
        (rank <= MOST).then_some(Self(rank))
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

impl Rank<2> {
    /// `table[rank]`.
    fn pick(self, table: [f32; 3]) -> f32 {
        table[usize::from(self.0)]
    }
}

impl Rank<3> {
    /// `table[rank]`.
    fn pick(self, table: [f32; 4]) -> f32 {
        table[usize::from(self.0)]
    }
}

/// A pocket stack a skill makes take more of a pocket ([`Skills::stack_units`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stack {
    Grenades,
    Traps,
}

/// What one unit of such a stack takes of a pocket's units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StackUnits {
    /// Its own (the item's config).
    Own,
    /// One and a half: two in three of the pocket's units, at least one (Toxic Smoke's grenade,
    /// Bear Trap's trap). The RGD-5's two units are its own (its config's `pocket_units` 2), not a
    /// skill's.
    OneAndHalf,
}

/// Row II's either/or pair: what an oxygen tank in the kit adds to its pockets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TankHarness {
    #[default]
    None,
    /// Tank Pocket (413): +1 pocket a rank.
    Pocket(Rank<2>),
    /// Tank Depth (414): +1 to every pocket's size a rank.
    Depth(Rank<2>),
}

impl TankHarness {
    /// From the two skills' digits; both at once is no tree's (they exclude each other).
    fn parse(pocket: u32, depth: u32) -> Option<Self> {
        Some(match (Rank::parse(pocket)?, Rank::parse(depth)?) {
            (Rank(0), Rank(0)) => Self::None,
            (pocket, Rank(0)) => Self::Pocket(pocket),
            (Rank(0), depth) => Self::Depth(depth),
            _ => return None,
        })
    }

    fn ranks(self) -> (u8, u8) {
        match self {
            Self::None => (0, 0),
            Self::Pocket(rank) => (rank.0, 0),
            Self::Depth(rank) => (0, rank.0),
        }
    }

    /// (pockets, pocket size) a kit with an oxygen tank gets.
    pub fn pockets(self) -> (u8, u8) {
        self.ranks()
    }
}

/// One skill's digit in modifier 47.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Field {
    pub skill: u16,
    /// The skill's ranks: its digit is 0..=ranks.
    pub ranks: u8,
}

/// Every skill's digit, in this order: modifier 47 is a mixed-radix number, the digit of skill n
/// worth [`weight`]`(n)` (`technical-skills-1.json` adds that much a rank).
#[rustfmt::skip]
pub const FIELDS: [Field; 17] = [
    Field { skill: 401, ranks: 3 }, Field { skill: 402, ranks: 3 }, Field { skill: 403, ranks: 2 },
    Field { skill: 404, ranks: 2 }, Field { skill: 411, ranks: 3 }, Field { skill: 412, ranks: 1 },
    Field { skill: 413, ranks: 2 }, Field { skill: 414, ranks: 2 }, Field { skill: 422, ranks: 1 },
    Field { skill: 423, ranks: 1 }, Field { skill: 431, ranks: 3 }, Field { skill: 432, ranks: 1 },
    Field { skill: 433, ranks: 2 }, Field { skill: 441, ranks: 3 }, Field { skill: 442, ranks: 1 },
    Field { skill: 443, ranks: 1 }, Field { skill: 444, ranks: 1 },
];

/// What one rank of `FIELDS[n]` adds to modifier 47, but the last (Bear Trap), whose rank takes
/// [`LOW`] away: a balanced top digit, so that the values span -LOW..LOW, under 2^24 either way.
pub const fn weight(n: usize) -> u32 {
    let mut weight = 1;
    let mut i = 0;
    while i < n {
        weight *= FIELDS[i].ranks as u32 + 1;
        i += 1;
    }
    weight
}

/// The weight of the last digit, and the bound of the others' sum: every integer of -LOW..LOW is
/// exact in an f32.
pub const LOW: u32 = weight(FIELDS.len() - 1);
const _: () = assert!(LOW <= 1 << 24 && FIELDS[FIELDS.len() - 1].ranks == 1);

/// Rearm (433): how long a trap that was shot stays sprung, in milliseconds, by rank.
const REARM_MS: [u32; 2] = [30_000, 20_000];

/// Oxygen Rush (442): how long its speed lasts, in milliseconds, and the speed it adds.
pub const OXYGEN_RUSH_MS: u32 = 20_000;
pub const OXYGEN_RUSH_SPEED: f32 = 0.10;
/// Oxygen Rush (442): this many milliseconds of a tank's use without a break start it.
pub const OXYGEN_RUSH_USE_MS: u16 = 20_000;

/// A player's stamina clock (`player_stamina` +70h, the simulation's ms, saved), wrapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clock(pub u32);

impl Clock {
    pub fn after(self, ms: u32) -> Clock {
        Clock(self.0.wrapping_add(ms))
    }

    /// Whether `self` is at or past `then` (two of a match's times, less than half the wrap apart).
    pub fn reached(self, then: Clock) -> bool {
        self.0.wrapping_sub(then.0) < 1 << 31
    }
}

/// A tank's use without a break toward its next Oxygen Rush, in milliseconds of air spent
/// (0..[`OXYGEN_RUSH_USE_MS`]; patch 180 keeps it on the tank and saves it with it as a u16). The
/// tank's tick runs only while it is on, and spends the elapsed ms (but at its last air), so the
/// air spent since the tank was switched on is its time of use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TankUse(u16);

/// What happens to an oxygen tank, as its use toward Oxygen Rush counts it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TankEvent {
    /// Switched on (`set_active(1)`): the use counts from 0. Off, it does not count at all.
    On,
    /// Filled again (a respawn's or an ammo crate's): the use counts from 0.
    Refill,
    /// Its tick while on: `spent` ms of air, `left` after it.
    Spent { spent: u32, left: u32 },
}

/// Why Oxygen Rush starts at a tank's tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RushCause {
    /// Another [`OXYGEN_RUSH_USE_MS`] of use without a break.
    Use,
    /// The tank ran out (its tick spent its last air).
    RanOut,
    /// Both at the same tick: one start.
    UseAtRunOut,
}

impl TankUse {
    /// The saved u16; none past the count's domain.
    pub fn parse(raw: u16) -> Option<Self> {
        (raw < OXYGEN_RUSH_USE_MS).then_some(Self(raw))
    }

    pub fn raw(self) -> u16 {
        self.0
    }

    /// The use after `event`, and why Oxygen Rush starts, if it does. A use mark keeps the
    /// milliseconds past it (the next mark is 20 s of air after this one, wherever the ticks fall);
    /// a tank that ran out counts from 0.
    pub fn after(self, event: TankEvent) -> (Self, Option<RushCause>) {
        let Self(used) = self;
        let (spent, left) = match event {
            TankEvent::On | TankEvent::Refill => return (Self(0), None),
            TankEvent::Spent { spent, left } => (spent, left),
        };
        let total = u32::from(used).saturating_add(spent);
        let every = u32::from(OXYGEN_RUSH_USE_MS);
        let mark = total >= every;
        let ran_out = spent > 0 && left == 0;
        let cause = match (mark, ran_out) {
            (true, true) => Some(RushCause::UseAtRunOut),
            (true, false) => Some(RushCause::Use),
            (false, true) => Some(RushCause::RanOut),
            (false, false) => None,
        };
        let rest = if ran_out { 0 } else { total % every };
        // `rest` is below OXYGEN_RUSH_USE_MS, a u16.
        (Self(rest as u16), cause)
    }
}

/// Oxygen Rush's boost, kept on the oxygen tank (patch 180 saves it with the tank as one u32: 0
/// none, else the clock it lasts until).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OxygenRush {
    #[default]
    Off,
    On { until: Clock },
}

impl OxygenRush {
    /// The saved word.
    pub fn parse(raw: u32) -> Self {
        match raw {
            0 => Self::Off,
            until => Self::On { until: Clock(until) },
        }
    }

    pub fn raw(self) -> u32 {
        match self {
            Self::Off => 0,
            Self::On { until } => until.0,
        }
    }

    /// A start at `now` ([`RushCause`]): the boost until [`OXYGEN_RUSH_MS`] later, whatever was left
    /// of a running one (the new start sets its end; the speed never stacks). A clock of 0, which
    /// the word cannot hold, is a millisecond later.
    pub fn start(now: Clock) -> Self {
        let until = now.after(OXYGEN_RUSH_MS);
        Self::On { until: Clock(until.0.max(1)) }
    }

    /// The boost at `now`: over once its end is reached.
    pub fn at(self, now: Clock) -> Self {
        match self {
            Self::On { until } if now.reached(until) => Self::Off,
            other => other,
        }
    }

    /// The movement speed's factor.
    pub fn speed(self) -> f32 {
        match self {
            Self::Off => 1.0,
            Self::On { .. } => 1.0 + OXYGEN_RUSH_SPEED,
        }
    }
}

/// A kit's technical skills.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Skills {
    dexterous_hands: Rank<3>,
    dense_smoke: Rank<3>,
    steady_breath: Rank<2>,
    blast_drill: Rank<2>,
    sapper: Rank<3>,
    sharp_ears: bool,
    tank_harness: TankHarness,
    return_to_sender: bool,
    trophy_trap: bool,
    deep_breaths: Rank<3>,
    dead_mans_switch: bool,
    rearm: Rank<2>,
    toxic_smoke: Rank<3>,
    oxygen_rush: bool,
    scavenger: bool,
    bear_trap: bool,
}

impl Skills {
    /// Modifier 47 as the skills tree summed it: a whole number below 2^24, each field within its
    /// skill's ranks. Anything else (a fraction, a negative, a NaN, a field past its ranks) is no
    /// skill at all.
    pub fn parse(value: f32) -> Option<Self> {
        if !(-(LOW as f32)..LOW as f32).contains(&value) || value != (value as i32) as f32 {
            return None;
        }
        let bear_trap = value < 0.0;
        let low = (value as i32 + if bear_trap { LOW as i32 } else { 0 }) as u32;
        let field = |n: usize| low / weight(n) % (u32::from(FIELDS[n].ranks) + 1);
        let flag = |n: usize| field(n) != 0;
        Some(Self {
            dexterous_hands: Rank::parse(field(0))?,
            dense_smoke: Rank::parse(field(1))?,
            steady_breath: Rank::parse(field(2))?,
            blast_drill: Rank::parse(field(3))?,
            sapper: Rank::parse(field(4))?,
            sharp_ears: flag(5),
            tank_harness: TankHarness::parse(field(6), field(7))?,
            return_to_sender: flag(8),
            trophy_trap: flag(9),
            deep_breaths: Rank::parse(field(10))?,
            dead_mans_switch: flag(11),
            rearm: Rank::parse(field(12))?,
            toxic_smoke: Rank::parse(field(13))?,
            oxygen_rush: flag(14),
            scavenger: flag(15),
            bear_trap,
        })
    }

    /// The packed value of these skills (the tree's sum for them).
    pub fn pack(self) -> f32 {
        let ranks = [
            self.dexterous_hands.0,
            self.dense_smoke.0,
            self.steady_breath.0,
            self.blast_drill.0,
            self.sapper.0,
            u8::from(self.sharp_ears),
            self.tank_harness.ranks().0,
            self.tank_harness.ranks().1,
            u8::from(self.return_to_sender),
            u8::from(self.trophy_trap),
            self.deep_breaths.0,
            u8::from(self.dead_mans_switch),
            self.rearm.0,
            self.toxic_smoke.0,
            u8::from(self.oxygen_rush),
            u8::from(self.scavenger),
        ];
        let low = (0..FIELDS.len() - 1).zip(ranks).map(|(n, r)| u32::from(r) * weight(n)).sum::<u32>();
        (low as i32 - if self.bear_trap { LOW as i32 } else { 0 }) as f32
    }

    /// Dexterous Hands (401): the battery's take animation plays this many times as fast.
    pub fn battery_take_speed(self) -> f32 {
        self.dexterous_hands.pick([1.0, 1.1, 1.2, 1.3])
    }

    /// Dexterous Hands (401): the milliseconds passed at an ammo crate count as this many (x (10 +
    /// rank) / 10, whole numbers: the same in every game), so its 3 s take 2728/2500/2308 ms.
    pub fn crate_ms(self, elapsed: u32) -> u32 {
        match u32::from(self.dexterous_hands.0) {
            0 => elapsed,
            rank => elapsed.saturating_mul(10 + rank) / 10,
        }
    }

    /// Dense Smoke (402): a thrown smoke's cloud lasts this many times as long.
    pub fn smoke_duration(self) -> f32 {
        self.dense_smoke.pick([1.0, 1.1, 1.2, 1.3])
    }

    /// Steady Breath (403): the weapon's sway while an oxygen tank is on, times this.
    pub fn tank_sway(self) -> f32 {
        self.steady_breath.pick([1.0, 0.9, 0.8])
    }

    /// Blast Drill (404): damage from its own or a teammate's grenade, times this.
    pub fn friendly_blast(self) -> f32 {
        self.blast_drill.pick([1.0, 0.75, 0.5])
    }

    /// Sapper (411): a trap's disarming runs this many times as fast.
    pub fn disarm_speed(self) -> f32 {
        self.sapper.pick([1.0, 1.15, 1.3, 1.45])
    }

    /// Sapper (411): a disarming of `ms` milliseconds, as fast as [`Self::disarm_speed`] says, in
    /// whole milliseconds (x 20 / (20 + 3 x rank): 32-bit integers, the same in every game; the arena
    /// has no 64-bit division).
    pub fn disarm_ms(self, ms: u32) -> u32 {
        match u32::from(self.sapper.0) {
            0 => ms,
            rank => ms.saturating_mul(20) / (20 + 3 * rank),
        }
    }

    /// Sharp Ears (412): this game plays an enemy's pin pull near its player.
    pub fn sharp_ears(self) -> bool {
        self.sharp_ears
    }

    /// Tank Pocket (413) or Tank Depth (414).
    pub fn tank_harness(self) -> TankHarness {
        self.tank_harness
    }

    /// Return to Sender (422): a live grenade nearby can be picked up and thrown back.
    pub fn return_to_sender(self) -> bool {
        self.return_to_sender
    }

    /// Trophy Trap (423): a trap the player disarms, its own or an enemy's, goes into its pocket.
    pub fn trophy_trap(self) -> bool {
        self.trophy_trap
    }

    /// Deep Breaths (431): the sprint's stamina cost while an oxygen tank is on, times this.
    pub fn tank_sprint_cost(self) -> f32 {
        self.deep_breaths.pick([1.0, 0.8, 0.7, 0.6])
    }

    /// Dead Man's Switch (432): after death the throw key drops a smoke where the body fell.
    pub fn dead_mans_switch(self) -> bool {
        self.dead_mans_switch
    }

    /// Rearm (433): a trap of this player's that was shot opens again this many milliseconds later.
    pub fn rearm_ms(self) -> Option<u32> {
        match self.rearm.0 {
            0 => None,
            rank => Some(REARM_MS[usize::from(rank) - 1]),
        }
    }

    /// Toxic Smoke (441): its rank (0 none), the damage a second its smoke does to enemies.
    pub fn toxic_smoke(self) -> Rank<3> {
        self.toxic_smoke
    }

    /// Oxygen Rush (442): after every [`OXYGEN_RUSH_USE_MS`] of oxygen tank use without a break
    /// and when the tank runs out ([`TankUse::after`]), [`OXYGEN_RUSH_SPEED`] for [`OXYGEN_RUSH_MS`]
    /// and the energy full.
    pub fn oxygen_rush(self) -> bool {
        self.oxygen_rush
    }

    /// Scavenger (443): a kill with its grenade gives one grenade back.
    pub fn scavenger(self) -> bool {
        self.scavenger
    }

    /// Bear Trap (444): its trap breaks the legs it catches.
    pub fn bear_trap(self) -> bool {
        self.bear_trap
    }

    /// Toxic Smoke (441): a grenade of such a kit takes one and a half of a pocket's units (the
    /// owner, 2026-11-23: "do the same with toxic smokes"); Bear Trap (444): a trap does (2026-11-22:
    /// "so you could take 3 traps").
    pub fn stack_units(self, stack: Stack) -> StackUnits {
        match stack {
            Stack::Grenades if self.toxic_smoke.0 > 0 => StackUnits::OneAndHalf,
            Stack::Traps if self.bear_trap => StackUnits::OneAndHalf,
            _ => StackUnits::Own,
        }
    }
}

/// Tank Container (445): a kit that wears an oxygen tank gets one more artefact container. No digit
/// of [`MODIFIER`]: the skill's data is the container (`artefact_slots` +1, as Artifact Container's),
/// which srp leaves out of the modifier block of a kit without a tank; the exe never knows it.
pub const TANK_CONTAINER: u16 = 445;

#[cfg(test)]
mod tests {
    use super::*;

    /// The value the tree sums for these `(skill, rank)`s.
    fn packed(ranks: &[(u16, u8)]) -> f32 {
        let n = |skill| FIELDS.iter().position(|f| f.skill == skill).unwrap();
        let low: u32 = ranks.iter().filter(|r| r.0 != 444).map(|&(skill, rank)| u32::from(rank) * weight(n(skill))).sum();
        let bear = ranks.iter().any(|&(skill, rank)| skill == 444 && rank > 0);
        (low as i32 - if bear { LOW as i32 } else { 0 }) as f32
    }

    #[test]
    fn the_digits_fit_an_f32_either_side_of_0() {
        assert_eq!(LOW, 15_925_248);
    }

    #[test]
    fn parses_what_the_tree_sums() {
        assert_eq!(Skills::parse(0.0), Some(Skills::default()));
        let value = packed(&[(401, 3), (404, 2), (414, 1), (444, 1), (441, 3), (443, 1), (433, 2)]);
        let s = Skills::parse(value).unwrap();
        assert_eq!(s.battery_take_speed(), 1.3);
        assert_eq!(s.friendly_blast(), 0.5);
        assert_eq!(s.tank_harness().pockets(), (0, 1));
        assert_eq!(Skills::parse(packed(&[(413, 1)])).unwrap().tank_harness().pockets(), (1, 0));
        assert_eq!(Skills::parse(packed(&[(413, 2)])).unwrap().tank_harness().pockets(), (2, 0));
        assert_eq!(Skills::parse(packed(&[(414, 2), (444, 1)])).unwrap().tank_harness().pockets(), (0, 2));
        assert_eq!(s.toxic_smoke().get(), 3);
        assert!(s.scavenger() && s.bear_trap());
        assert_eq!(s.rearm_ms(), Some(20_000));
        assert_eq!(Skills::parse(packed(&[(433, 1)])).unwrap().rearm_ms(), Some(30_000));
        assert_eq!(Skills::default().rearm_ms(), None);
        assert_eq!((s.stack_units(Stack::Grenades), s.stack_units(Stack::Traps)), (StackUnits::OneAndHalf, StackUnits::OneAndHalf));
        let none = Skills::default();
        assert_eq!((none.stack_units(Stack::Grenades), none.stack_units(Stack::Traps)), (StackUnits::Own, StackUnits::Own));
        assert_eq!(s.pack(), value);
        // Everything at its most, Tank Depth for the pair: 30 points.
        let most: Vec<_> = FIELDS.iter().filter(|f| f.skill != 413).map(|f| (f.skill, f.ranks)).collect();
        assert_eq!(Skills::parse(packed(&most)).unwrap().pack(), packed(&most));
        assert_eq!(most.iter().map(|&(_, r)| u32::from(r)).sum::<u32>(), 30);
    }

    #[test]
    fn oxygen_rush_lasts_twenty_seconds_across_the_wrap() {
        for now in [0u32, 5_000, u32::MAX - 5_000] {
            let rush = OxygenRush::start(Clock(now));
            assert_eq!(OxygenRush::parse(rush.raw()), rush);
            assert_eq!(rush.at(Clock(now.wrapping_add(19_999))).speed(), 1.1);
            assert_eq!(rush.at(Clock(now.wrapping_add(20_000))), OxygenRush::Off);
        }
        // A start whose end is clock 0 keeps a word that is not "none".
        assert_ne!(OxygenRush::start(Clock(0u32.wrapping_sub(20_000))).raw(), 0);
        assert_eq!(OxygenRush::parse(0), OxygenRush::Off);
    }

    /// A tank's use, tick by tick: `ticks` of `(spent, left)` from a use of 0.
    fn run(ticks: impl IntoIterator<Item = TankEvent>) -> (TankUse, Vec<Option<RushCause>>) {
        let mut causes = Vec::new();
        let used = ticks.into_iter().fold(TankUse::default(), |used, event| {
            let (next, cause) = used.after(event);
            causes.push(cause);
            next
        });
        (used, causes)
    }

    /// `ms` of use in 16 ms ticks of a tank holding `air`.
    fn on_for(air: u32, ms: u32) -> Vec<TankEvent> {
        (1..=ms / 16).map(|n| TankEvent::Spent { spent: 16, left: air - n * 16 }).collect()
    }

    #[test]
    fn oxygen_rush_after_twenty_seconds_without_a_break() {
        // 20 s on: one start, at the tick that reaches it.
        let (used, causes) = run(on_for(120_000, 20_000));
        assert_eq!(causes.iter().flatten().collect::<Vec<_>>(), [&RushCause::Use]);
        assert_eq!(causes.last(), Some(&Some(RushCause::Use)));
        assert_eq!(used, TankUse(0));
        // 19 s, off, on, 19 s: no start (switching on counts from 0).
        let mut ticks = on_for(120_000, 19_008);
        ticks.push(TankEvent::On);
        ticks.extend(on_for(100_992, 19_008));
        assert!(run(ticks).1.iter().all(Option::is_none));
        // A second 20 s: a second start; a tick past the mark keeps the rest.
        let (used, causes) = run(on_for(120_000, 40_000));
        assert_eq!(causes.iter().flatten().count(), 2);
        assert_eq!(used, TankUse(0));
        assert_eq!(TankUse(19_990).after(TankEvent::Spent { spent: 16, left: 50_000 }), (TankUse(6), Some(RushCause::Use)));
        // A refill counts from 0.
        assert_eq!(TankUse(19_000).after(TankEvent::Refill), (TankUse(0), None));
    }

    #[test]
    fn oxygen_rush_when_the_tank_runs_out() {
        // Out between marks: a start; out on a mark: one start, not two.
        assert_eq!(TankUse(5_000).after(TankEvent::Spent { spent: 16, left: 0 }), (TankUse(0), Some(RushCause::RanOut)));
        assert_eq!(TankUse(19_984).after(TankEvent::Spent { spent: 16, left: 0 }), (TankUse(0), Some(RushCause::UseAtRunOut)));
        // A tick that spends nothing is no run-out.
        assert_eq!(TankUse(5_000).after(TankEvent::Spent { spent: 0, left: 0 }), (TankUse(5_000), None));
        // Whole tanks kept on from full: 20 s (the defect's 30 - 10), 30, 50, 80, 120, 140 s give
        // 1, 2, 3, 4, 6, 7 starts.
        let starts = |air: u32| run(on_for(air, air)).1.iter().flatten().count();
        assert_eq!([20_000, 30_000, 50_000, 80_000, 120_000, 140_000].map(starts), [1, 2, 3, 4, 6, 7]);
    }

    /// The owner, 2026-11-22: "note that might mean one trigger while another is active". A start
    /// while a boost runs sets its end to 20 s from that start: one boost, x1.1 once.
    #[test]
    fn a_start_while_a_boost_runs_sets_its_end_again() {
        // A 30 s tank kept on: the use mark's boost from 20 s has 10 s left when the tank runs out
        // at 30 s; the run-out's start makes it end at 50 s, not 40 + 20.
        let running = OxygenRush::start(Clock(20_000));
        assert_eq!(running.at(Clock(30_000)), OxygenRush::On { until: Clock(40_000) });
        let again = OxygenRush::start(Clock(30_000));
        assert_eq!(again, OxygenRush::On { until: Clock(50_000) });
        assert_eq!(again.at(Clock(45_000)).speed(), 1.1);
        assert_eq!(again.at(Clock(50_000)), OxygenRush::Off);
    }

    #[test]
    fn a_tank_use_is_below_twenty_seconds() {
        assert_eq!(TankUse::parse(19_999), Some(TankUse(19_999)));
        assert_eq!(TankUse::parse(20_000), None);
        assert_eq!(TankUse::parse(u16::MAX), None);
    }

    #[test]
    fn sapper_shortens_the_five_second_disarm() {
        let ms = |rank: u8| Skills::parse(packed(&[(411, rank)])).unwrap().disarm_ms(5_000);
        assert_eq!([ms(0), ms(1), ms(2), ms(3)], [5_000, 4_347, 3_846, 3_448]);
        assert_eq!(Skills::default().disarm_ms(u32::MAX), u32::MAX);
    }

    #[test]
    fn dexterous_hands_shorten_the_three_second_crate() {
        let hands = |rank: u8| Skills::parse(packed(&[(401, rank)])).unwrap();
        // The first millisecond that counts as the crate's 3000.
        let done = |rank: u8| (0..=3_000).find(|&ms| hands(rank).crate_ms(ms) >= 3_000).unwrap();
        assert_eq!([done(0), done(1), done(2), done(3)], [3_000, 2_728, 2_500, 2_308]);
        assert_eq!(hands(3).crate_ms(u32::MAX), u32::MAX / 10);
    }

    #[test]
    fn refuses_what_no_tree_gives() {
        // Tank Pocket and Tank Depth together.
        let both = packed(&[(413, 1), (414, 1)]);
        for value in [-(LOW as f32) - 1.0, 0.5, f32::NAN, f32::INFINITY, LOW as f32, both] {
            assert_eq!(Skills::parse(value), None, "{value}");
        }
    }
}
