//! Patch 109: the artefact tree's skills the exe computes (docs/notes/artefact-skills.md).
//! The data skills (Quick Hands, Forest Protection, Knowledge of Artifacts) are native
//! modifiers 7, 41 and 39; the others travel packed in modifier 40 (`artefact_power`), parsed
//! once here.
#![forbid(unsafe_code)]

use crate::vostok::{
    ArtefactSkillsOnLandingExit, Context, Flow, GameCell, NoExit, artefact_skills,
    types::{Place, Tick},
};

/// A hit's type (the hit +1Ch; 0.26g's seven, `blunt` gone): 4..7 are the anomalous ones
/// (table 0x92075C), against which armour is the card's isolation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitKind {
    Ranged,
    Blunt,
    Slash,
    Explosive,
    Toxic,
    Radiation,
    Fire,
    Shock,
}

impl HitKind {
    pub fn parse(value: u32) -> Option<Self> {
        Some(match value {
            0 => Self::Ranged,
            1 => Self::Blunt,
            2 => Self::Slash,
            3 => Self::Explosive,
            4 => Self::Toxic,
            5 => Self::Radiation,
            6 => Self::Fire,
            7 => Self::Shock,
            _ => return None,
        })
    }

    fn anomalous(self) -> bool {
        matches!(self, Self::Toxic | Self::Radiation | Self::Fire | Self::Shock)
    }
}

/// The match's Keeper of the Artifacts draw, 1..=6, as srp rolls it once a match and writes it
/// into every kit of every player (modifier 40 bits 9..11), with the skill or not: every game
/// knows the kind at the map's load.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Draw(u8);

impl Draw {
    pub const MOST: u8 = 6;

    pub fn parse(draw: u8) -> Option<Self> {
        (1..=Self::MOST).contains(&draw).then_some(Self(draw))
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

/// Resistance's rank, 0..=5.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Resistance(u8);

impl Resistance {
    pub const MOST: u8 = 5;
    /// Armour a rank against the anomalous types: +5 isolation points (the card x100).
    pub const ARMOUR: f32 = 0.05;
}

/// The packed skills of a kit's modifier 40.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Skills {
    resistance: Resistance,
    keeper: bool,
    draw: Option<Draw>,
    resonance: bool,
    spring: bool,
    onyx: bool,
    larkspur: bool,
    cascade: bool,
    sight: bool,
    late: bool,
}

impl Skills {
    /// Bits: 0..2 Resistance, 3 Keeper, 4 Resonance, 5 Spring II, 6 Onyx II, 7 Larkspur II,
    /// 8 Cascade, 9..11 the match's Keeper draw 1..6 (0 none; with the skill or not), 12 Artifact
    /// Sight, 13 a late joiner's kit (srp's mark, survarium::v026g::keeper::LATE). Anything else (a
    /// fraction, a negative, a rank past 5, a draw of 7, a bit past 13) is no skill at all.
    pub fn parse(value: f32) -> Option<Self> {
        if !(0.0..16384.0).contains(&value) || value != (value as u32) as f32 {
            return None;
        }
        let bits = value as u32;
        let resistance = (bits & 7) as u8;
        if resistance > Resistance::MOST {
            return None;
        }
        let bit = |n: u32| bits & (1 << n) != 0;
        let draw = match ((bits >> 9) & 7) as u8 {
            0 => None,
            draw => Some(Draw::parse(draw)?),
        };
        Some(Self {
            resistance: Resistance(resistance),
            keeper: bit(3),
            draw,
            resonance: bit(4),
            spring: bit(5),
            onyx: bit(6),
            larkspur: bit(7),
            cascade: bit(8),
            sight: bit(12),
            late: bit(13),
        })
    }

    /// The kit is a late joiner's (srp took it into the match once in process): Keeper's gift
    /// comes after its first death, not at the match's start.
    pub fn late(self) -> bool {
        self.late
    }

    /// Artifact Sight (the owner, 2026-11-07): this game shows the leg quarters of passive
    /// artefacts (patch 111's glow), its own player's too.
    pub fn sight(self) -> bool {
        self.sight
    }

    /// Keeper of the Artifacts taken.
    pub fn keeper(self) -> bool {
        self.keeper
    }

    /// The match's Keeper draw (srp writes it into every kit of the match), none in the lobby.
    pub fn draw(self) -> Option<Draw> {
        self.draw
    }

    /// Onyx II: no fall damage while one of the player's Onyx is active.
    pub fn onyx(self) -> bool {
        self.onyx
    }

    /// Spring II (the owner, 2026-11-05): each carried Spring's reload speed (modifier 13),
    /// +5% while its passive is on and +10% more while its active is, as its other nodes:
    /// (passive, active) node values, 0 without the skill.
    pub fn spring_reload(self) -> (f32, f32) {
        if self.spring { (0.05, 0.10) } else { (0.0, 0.0) }
    }

    /// Larkspur II: a Larkspur's activation restores all energy.
    pub fn larkspur(self) -> bool {
        self.larkspur
    }

    /// How far (squared metres) this carrier's artefacts give their `phase` to its teammates:
    /// Resonance its passives within 8 m, Cascade its actives within 5 m.
    pub fn reach(self, phase: Phase) -> Option<f32> {
        match phase {
            Phase::Passive => self.resonance.then_some(8.0 * 8.0),
            Phase::Active => self.cascade.then_some(5.0 * 5.0),
        }
    }

    /// Resistance: the part's armour `native` against a hit of `kind`, +0.05 a rank for the
    /// anomalous types, before the piercing's subtraction and the native clamp.
    pub fn armour(self, native: f32, kind: HitKind) -> f32 {
        if kind.anomalous() {
            native + f32::from(self.resistance.0) * Resistance::ARMOUR
        } else {
            native
        }
    }
}

/// An artefact kind Resonance and Cascade share (the Rattle has no passive nor active to share).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Spring,
    Lifebone,
    Onyx,
}

/// An artefact's state (+280h) as the tree reads it: 3 its passive (ready or cooling), 4 its
/// active; any other (being collected, its holder dead) gives nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Passive,
    Active,
    Other,
}

impl State {
    pub fn parse(value: u32) -> Self {
        match value {
            3 => Self::Passive,
            4 => Self::Active,
            _ => Self::Other,
        }
    }
}

/// What a carried copy shares: its passive (Resonance) or its active (Cascade).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Passive,
    Active,
}

impl Kind {
    /// Whether a copy in `state` gives its `phase` as it does to its holder: Spring's and
    /// Lifebone's passives are linked from pickup to death (3 and 4), Onyx's protect takes its
    /// passive's share in 3 only (its active's in 4); every active in 4.
    pub fn gives(self, phase: Phase, state: State) -> bool {
        matches!(
            (self, phase, state),
            (_, Phase::Active, State::Active)
                | (_, Phase::Passive, State::Passive)
                | (Self::Spring | Self::Lifebone, Phase::Passive, State::Active)
        )
    }
}

/// A player's passive copies of each kind that count for it (artefact-skills.md, "Leg quarters"):
/// its own carried copies giving their passive and the copies Resonance shares with it, at most
/// [`Room::MOST`] of a kind (the sharing's own cap; own copies past it count as 4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Copies {
    spring: u8,
    lifebone: u8,
    onyx: u8,
}

impl Copies {
    /// From each kind's (own, shared) count.
    pub fn of(mut count: impl FnMut(Kind) -> (usize, usize)) -> Self {
        let mut of = |kind| {
            let (own, shared) = count(kind);
            own.saturating_add(shared).min(usize::from(Room::MOST)) as u8
        };
        Self { spring: of(Kind::Spring), lifebone: of(Kind::Lifebone), onyx: of(Kind::Onyx) }
    }

    /// The copies of `kind`: 0 to 4.
    pub fn get(self, kind: Kind) -> u8 {
        match kind {
            Kind::Spring => self.spring,
            Kind::Lifebone => self.lifebone,
            Kind::Onyx => self.onyx,
        }
    }
}

/// The shared copies of one kind and phase a recipient still takes (the owner, 2026-11-05: at
/// most 4 effects of one kind in all, its own included): 4 less its own carried copies of the
/// kind; own copies past 4 stay (native), none shared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Room(u8);

impl Room {
    pub const MOST: u8 = 4;
    /// A recipient that takes none (a dead one).
    pub const NONE: Room = Room(0);

    pub fn after(own: usize) -> Self {
        Self(usize::from(Self::MOST).saturating_sub(own) as u8)
    }

    /// One copy more, if there is room for it.
    pub fn take(&mut self) -> bool {
        let room = self.0 > 0;
        self.0 = self.0.saturating_sub(1);
        room
    }
}

/// The kit lists a Spring's nodes are in (the loader 0x33ACF0 and patch 54): movement 4, aim
/// time 49, stamina regeneration 3 and spending 19, stopping power 42, reload 13. The getters'
/// other indices have nothing shared.
pub const SPRING_LISTS: [u32; 6] = [4, 49, 3, 19, 42, 13];

/// What an Onyx does to a hit it protects against (its protect 0x312BF0), read once from a
/// carried Onyx: its passive's or its active's numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OnyxProtection {
    pub damage_add: f32,
    pub damage_mul: f32,
    pub piercing_add: f32,
    pub piercing_mul: f32,
    /// The active's most off a hit (absorb_total); none for the passive.
    pub absorb: Option<f32>,
}

impl OnyxProtection {
    /// A hit's (damage, piercing) after it: the passive (D + a) x m; the active
    /// D - min(absorb, D - (D + a) x m); the piercing (P + a) x m either way.
    pub fn protect(self, damage: f32, piercing: f32) -> (f32, f32) {
        let reduced = (damage + self.damage_add) * self.damage_mul;
        let damage = match self.absorb {
            None => reduced,
            Some(absorb) => {
                let taken = damage - reduced;
                damage - if absorb > taken { taken } else { absorb }
            }
        };
        (damage, (piercing + self.piercing_add) * self.piercing_mul)
    }
}

/// One part's regeneration tick: its rate a second, the delay after a hit (ms) and the limit
/// (a fraction of the maximum), as body_part::regenerate has them after its own modifiers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Regeneration {
    pub rate: f32,
    pub delay_ms: u32,
    pub limit: f32,
}

/// What a Lifebone's passive does to one part's regeneration tick (its callback 0x312590),
/// read once from a carried Lifebone's record for that part.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LifeboneRegeneration {
    pub rate_add: f32,
    pub rate_mul: f32,
    /// Seconds.
    pub delay_add: f32,
    pub delay_mul: f32,
    pub limit_add: f32,
    pub limit_mul: f32,
}

impl LifeboneRegeneration {
    pub fn apply(self, tick: Regeneration) -> Regeneration {
        // As the callback: the delay as an unsigned float, each step a float, truncated to 64
        // bits (fistp, rounding toward 0) of which the low 32 are kept (a negative wraps).
        let delay = (tick.delay_ms as f32 + self.delay_add * 1000.0) * self.delay_mul;
        Regeneration {
            rate: (tick.rate + self.rate_add) * self.rate_mul,
            delay_ms: delay as i64 as u32,
            limit: (tick.limit + self.limit_add) * self.limit_mul,
        }
    }
}

/// The kit list a Spring's movement nodes are in: movement_speed (0x2DBB50) reads it when the
/// player's animations are selected (select_animations 0x2D9690), not every tick.
pub const MOVEMENT_LIST: u32 = 4;

/// The ticks of a place's shared movement kept (3.2 s): a re-simulated tick finds the tick
/// before it.
const HISTORY: usize = 64;

/// One place's shared movement (what the shared Springs add to its list 4) at its recent ticks,
/// each in the slot of its tick.
#[derive(Clone, Copy)]
pub struct MovementHistory {
    slots: [Option<(Tick, f32)>; HISTORY],
}

impl MovementHistory {
    pub const EMPTY: Self = Self { slots: [None; HISTORY] };

    fn slot(tick: Tick) -> usize {
        tick.0 as usize % HISTORY
    }

    /// Records `shared` at `tick`; whether it differs from the tick before's (nothing shared, 0,
    /// when that tick is not kept: the place's first tick). A function of the two ticks' worlds
    /// alone: a re-simulated tick answers as its first simulation did.
    pub fn changed(&mut self, tick: Tick, shared: f32) -> bool {
        let before = Tick(tick.0.wrapping_sub(1));
        let was = match self.slots[Self::slot(before)] {
            Some((at, value)) if at == before => value,
            _ => 0.0,
        };
        self.slots[Self::slot(tick)] = Some((tick, shared));
        was != shared
    }
}

/// Every place's shared movement history (the game's thread alone runs base_player::tick).
static MOVEMENTS: [GameCell<MovementHistory>; Place::COUNT] =
    [const { GameCell::new(MovementHistory::EMPTY) }; Place::COUNT];

/// Resonance and Cascade at each player's tick before its animations (battery.carrier_tick
/// 0x2DAAD3, right before tick_animations 0x2D8EE0): when the shared Springs' movement changed
/// since the tick before (a carrier came into reach or left it, died, a Spring's state changed),
/// the player's animations are selected again this tick, which takes the new movement speed
/// in, as patch 94 does for Adrenalin. No change, no re-selection.
pub fn on_movement_tick(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(tick) = artefact_skills::MovementTick::at(ctx) else {
        return Flow::Continue;
    };
    let history = &MOVEMENTS[tick.place().index()];
    let mut kept = history.get(ctx.thread());
    let changed = kept.changed(tick.tick(), tick.shared());
    history.set(ctx.thread(), kept);
    if changed {
        tick.reselect();
    }
    Flow::Continue
}

/// Resonance and Cascade at apply_modifier (0x2D5AE6): XMM0 the kit's own float of an index,
/// to which the shared Springs' nodes of that list are added.
pub fn on_shared_modifier(ctx: &mut Context<'_>) -> Flow<NoExit> {
    artefact_skills::shared_modifier(ctx);
    Flow::Continue
}

/// The same in apply_modifiers' loop (0x2D5C18), for each of its indices.
pub fn on_shared_modifiers(ctx: &mut Context<'_>) -> Flow<NoExit> {
    artefact_skills::shared_modifiers(ctx);
    Flow::Continue
}

/// Resonance and Cascade before a part's protectors (0x32A020): the shared Onyx.
pub fn on_shared_protection(ctx: &mut Context<'_>) -> Flow<NoExit> {
    artefact_skills::shared_protection(ctx);
    Flow::Continue
}

/// Resonance at a part's regeneration tick (0x32A53F, before patch 94's): the shared Lifebones.
pub fn on_shared_regeneration(ctx: &mut Context<'_>) -> Flow<NoExit> {
    artefact_skills::shared_regeneration(ctx);
    Flow::Continue
}

/// Resistance at body_part_parameters::hit_by_type (0x32A083): XMM0 the part's armour.
pub fn on_isolation(ctx: &mut Context<'_>) -> Flow<NoExit> {
    artefact_skills::isolation(ctx);
    Flow::Continue
}

/// Onyx II at the landing's start: the whole landing is its fall damage, skipped when the player
/// has the skill and an active Onyx.
pub fn on_landing(ctx: &mut Context<'_>) -> Flow<ArtefactSkillsOnLandingExit> {
    if artefact_skills::lands_unhurt(ctx) {
        Flow::Exit(ArtefactSkillsOnLandingExit::Done)
    } else {
        Flow::Continue
    }
}

/// Larkspur II at an artefact's actual activation (artefact_base::action 0x313333).
pub fn on_larkspur(ctx: &mut Context<'_>) -> Flow<NoExit> {
    artefact_skills::larkspur(ctx);
    Flow::Continue
}

/// Spring II, patch 54's entry at a Spring's passive link: its reload nodes' values.
pub fn on_spring_reload(ctx: &mut Context<'_>) -> Flow<NoExit> {
    artefact_skills::spring_reload(ctx);
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shared_movement_reselects_on_a_change_only() {
        let mut history = MovementHistory::EMPTY;
        // The first tick against nothing shared; then each tick against the one before.
        assert!(!history.changed(Tick(20), 0.0));
        assert!(history.changed(Tick(21), 0.05), "a carrier came into reach");
        assert!(!history.changed(Tick(22), 0.05), "no change");
        assert!(history.changed(Tick(23), 0.0), "the carrier left or died");
        assert!(!history.changed(Tick(24), 0.0));
        // A re-simulation from tick 21: each tick against its own tick before, as at first.
        assert!(history.changed(Tick(21), 0.05));
        assert!(!history.changed(Tick(22), 0.05));
        assert!(history.changed(Tick(23), 0.0));
        // ...and a re-simulated world where the carrier stayed: tick 23 no longer changes.
        assert!(!history.changed(Tick(23), 0.05));
        assert!(history.changed(Tick(24), 0.0), "tick 24 now sees tick 23's 0.05");
        // A gap (a place's first tick, or one past the history): against nothing shared.
        assert!(history.changed(Tick(24 + HISTORY as u32), 0.15));
        assert!(!history.changed(Tick(1000), 0.0));
    }

    #[test]
    fn parses_the_packed_ranks_only_exactly() {
        for bad in [-1.0, 0.5, 6.0, 7.0, 510.0, 7.0 * 512.0, 8.0 + 7.0 * 512.0, 16384.0, 16384.0 + 508.0, f32::INFINITY, f32::NAN] {
            assert_eq!(Skills::parse(bad), None, "{bad}");
        }
        assert_eq!(Skills::parse(0.0), Some(Skills::default()));
        let full = Skills::parse(509.0).unwrap();
        assert_eq!(full.resistance, Resistance(5));
        assert!(full.keeper && full.draw.is_none());
        assert!(full.resonance && full.spring && full.onyx && full.larkspur && full.cascade);
        for draw in 1..=6u8 {
            let skills = Skills::parse(8.0 + f32::from(draw) * 512.0).unwrap();
            assert!(skills.keeper && skills.draw == Draw::parse(draw));
            // Every kit of the match has the draw, a player without the skill too.
            let skills = Skills::parse(f32::from(draw) * 512.0).unwrap();
            assert!(!skills.keeper && skills.draw.map(Draw::get) == Some(draw));
        }
        assert!(Skills::parse(508.0 + 6.0 * 512.0).is_some());
        // Artifact Sight, bit 12: alone, with every other bit and a draw.
        assert!(Skills::parse(4096.0).unwrap().sight());
        let all = Skills::parse(4096.0 + 508.0 + 6.0 * 512.0).unwrap();
        assert!(all.sight() && all.keeper && all.cascade && all.draw.map(Draw::get) == Some(6));
        assert!(!full.sight());
        // A late joiner's kit, bit 13.
        assert!(Skills::parse(8192.0 + 8.0 + 512.0).unwrap().late());
        assert!(!all.late());
    }

    #[test]
    fn resistance_adds_isolation_to_the_anomalous_types_only() {
        for rank in 0..=5u8 {
            let skills = Skills::parse(f32::from(rank)).unwrap();
            for value in 0..8 {
                let kind = HitKind::parse(value).unwrap();
                let want = if value >= 4 { 0.3 + 0.05 * f32::from(rank) } else { 0.3 };
                assert!((skills.armour(0.3, kind) - want).abs() < 1e-6, "{rank} {kind:?}");
            }
        }
        assert_eq!(HitKind::parse(8), None);
    }

    #[test]
    fn resonance_reaches_8_m_with_passives_and_cascade_5_m_with_actives() {
        let none = Skills::parse(0.0).unwrap();
        let resonance = Skills::parse(16.0).unwrap();
        let cascade = Skills::parse(256.0).unwrap();
        let both = Skills::parse(16.0 + 256.0).unwrap();
        for phase in [Phase::Passive, Phase::Active] {
            assert_eq!(none.reach(phase), None);
        }
        assert_eq!((resonance.reach(Phase::Passive), resonance.reach(Phase::Active)), (Some(64.0), None));
        assert_eq!((cascade.reach(Phase::Passive), cascade.reach(Phase::Active)), (None, Some(25.0)));
        assert_eq!((both.reach(Phase::Passive), both.reach(Phase::Active)), (Some(64.0), Some(25.0)));
    }

    #[test]
    fn copies_count_own_and_shared_up_to_four() {
        let copies = Copies::of(|kind| match kind {
            Kind::Spring => (1, 0),
            Kind::Lifebone => (2, 3),
            Kind::Onyx => (6, 0),
        });
        assert_eq!((copies.get(Kind::Spring), copies.get(Kind::Lifebone), copies.get(Kind::Onyx)), (1, 4, 4));
        assert_eq!(Copies::default().get(Kind::Spring), 0);
    }

    #[test]
    fn a_recipient_takes_at_most_four_copies_its_own_first() {
        // Two own Springs and three shared: two of them count.
        let mut room = Room::after(2);
        assert_eq!([room.take(), room.take(), room.take()], [true, true, false]);
        for own in 0..=6 {
            let mut room = Room::after(own);
            let taken = (0..8).filter(|_| room.take()).count();
            assert_eq!(taken, 4usize.saturating_sub(own), "{own}");
        }
    }

    #[test]
    fn a_copy_gives_its_phase_in_the_states_it_does_for_its_holder() {
        use {Kind::*, Phase::*};
        for state in [3, 4, 0, 1, 2, 5] {
            let state = State::parse(state);
            for kind in [Spring, Lifebone] {
                assert_eq!(kind.gives(Passive, state), state != State::Other, "{kind:?} {state:?}");
            }
            assert_eq!(Onyx.gives(Passive, state), state == State::Passive);
            for kind in [Spring, Lifebone, Onyx] {
                assert_eq!(kind.gives(Active, state), state == State::Active);
            }
        }
    }

    #[test]
    fn onyx_and_lifebone_shares_do_what_their_native_code_does() {
        // The release's Onyx: passive x0.95, active x0.25 taking at most 5.
        let passive = OnyxProtection { damage_add: 0.0, damage_mul: 0.95, piercing_add: 0.0, piercing_mul: 0.95, absorb: None };
        let active = OnyxProtection { damage_mul: 0.25, piercing_mul: 0.25, absorb: Some(5.0), ..passive };
        assert_eq!(passive.protect(100.0, 0.4), (95.0, 0.4 * 0.95));
        assert_eq!(active.protect(4.0, 0.4), (1.0, 0.1));
        assert_eq!(active.protect(100.0, 0.4), (95.0, 0.1));
        let lifebone = LifeboneRegeneration { rate_add: 0.1, rate_mul: 1.0, delay_add: 0.0, delay_mul: 1.0, limit_add: 1.0, limit_mul: 1.0 };
        let tick = Regeneration { rate: 0.03, delay_ms: 8000, limit: 0.5 };
        let once = lifebone.apply(tick);
        assert_eq!((once.rate, once.delay_ms, once.limit), (0.03 + 0.1, 8000, 1.5));
        let sooner = LifeboneRegeneration { delay_add: -2.0, delay_mul: 0.5, ..lifebone };
        assert_eq!(sooner.apply(tick).delay_ms, 3000);
        // Below 0 wraps, as the native's (the delay then never ends).
        assert_eq!(LifeboneRegeneration { delay_add: -9.0, ..sooner }.apply(tick).delay_ms, u32::MAX - 499);
    }
}
