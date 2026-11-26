//! A weapon's second sight, and the switch between it and its main sight,
//! simulated as 0.69d0 simulates its aim-slot switch (client patch 206;
//! `docs/notes/second-sights.md`, "The simulation (0.26g)").
//!
//! 0.69d0's weapon keeps a current aim slot and, while a switch runs, the
//! next one and the switch's end (`request_aim_slot_switch` 0x2006cd490,
//! `start_aim_slot_switch` 0x2006d87d0, `update_timed_switches`
//! 0x2006d8950, `reset_switches` 0x2006cf9e0). Here a weapon has two sights
//! at most, the main one and a second one beside it, and the machine is
//! [`State`]: settled on one, or switching to the other since a press, with
//! one transition, [`State::step`], total over the [`Event`]s.
//!
//! What it changes in a match (the shot's dispersion: [`State::aimed`],
//! [`State::effective_aim`]) and what the view shows ([`State::view`]) are
//! read here, at a [`GameTime`]; its bytes in a world are a [`Record`]'s.
//! No pointer into the game, no static, no allocation: the client patch and
//! srp-validate (the world's check, in the match server and in each game)
//! use this same code, and every game computes the same states.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

pub use weapon_pickup::WeaponSlot;

/// A switch's time from the press to its end: 0.69d0's `scope_switch_delay`
/// (a console integer, 500 by default: 0x200086630 over 0x200a9000c).
pub const SWITCH_MS: u32 = 500;
/// The simulation's tick: a switch's blend starts at the first tick after its
/// press, as 0.69d0's interpolator does (`start_aim_slot_switch`).
pub const TICK_MS: u32 = 50;

/// Simulation milliseconds: the game's u32 clock (a player's +324h, a
/// weapon's last tick +5A4h, an input's time), wrapping. Its arithmetic is
/// here alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct GameTime(pub u32);

impl GameTime {
    /// `ms` later.
    pub const fn after(self, ms: u32) -> GameTime {
        GameTime(self.0.wrapping_add(ms))
    }

    /// Milliseconds from `earlier` to `self`, negative when `self` is before
    /// it (two times within 24 days of each other, as a match's are).
    pub const fn since(self, earlier: GameTime) -> i32 {
        self.0.wrapping_sub(earlier.0) as i32
    }

    /// The first tick boundary after `self` (the ticks are the multiples of
    /// [`TICK_MS`]).
    pub const fn next_tick(self) -> GameTime {
        GameTime((self.0 / TICK_MS).wrapping_add(1).wrapping_mul(TICK_MS))
    }
}

/// One of a weapon's two sights.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sight {
    /// The gun's own sight or its scope: the only one of a weapon without a
    /// second sight.
    Main,
    /// The sight on the offset rail beside it (`data/srp_second_sight`).
    Second,
}

impl Sight {
    pub const fn other(self) -> Sight {
        match self {
            Sight::Main => Sight::Second,
            Sight::Second => Sight::Main,
        }
    }
}

/// What a weapon can switch between: 0.69d0's aim slots of two owners (its
/// `request_aim_slot_switch` does nothing with one).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sights {
    /// No second sight: always [`State::MAIN`].
    One,
    /// A second sight beside the main one.
    Two,
}

/// A weapon's sights in the simulation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum State {
    /// On one sight.
    Settled(Sight),
    /// Switching from the other sight to `to` since the press at `pressed`:
    /// the blend runs from the first tick after it ([`State::start`]) and the
    /// switch ends [`SWITCH_MS`] after it ([`State::end`]).
    Switching { to: Sight, pressed: GameTime },
}

/// What happens to a weapon's sights.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// The switch key's press (its input applied at that time).
    Press(GameTime),
    /// The weapon's tick at that time.
    Tick(GameTime),
    /// The weapon put away (deactivated): a switch going is undone, as
    /// 0.69d0's `reset_switches` sets the next slot to the current one.
    Deactivate,
}

/// What the view shows of a weapon's sights at a time: the switch from
/// `from` to `to` at `s` (0 settled; the eye, the field of view and which
/// sight draws its reticle blend by it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub from: Sight,
    pub to: Sight,
    /// The switch's progress, 0..=1: 0 settled and before the first tick
    /// after the press.
    pub s: f32,
}

impl View {
    /// The sight that draws its reticle and hides its parts: the one switched
    /// to once past half way (0.69d0's `current_aim_slot_index`).
    pub fn shown(&self) -> Sight {
        if self.s > 0.5 { self.to } else { self.from }
    }

    /// `aim`, the weapon's aim progress (0 at the hip, 1 aimed), as the
    /// switch lowers it: down by min(s, 1 - s), half way through by 0.5
    /// (0.69d0's `effective_aim_progress` 0x2006cc8a0).
    pub fn effective_aim(&self, aim: f32) -> f32 {
        (aim - self.s.min(1.0 - self.s)).clamp(0.0, 1.0)
    }
}

impl State {
    /// On the main sight: every weapon's state when it is made, and always a
    /// weapon's without a second sight.
    pub const MAIN: State = State::Settled(Sight::Main);

    /// The sight it switches from, or is on.
    pub const fn from(self) -> Sight {
        match self {
            State::Settled(sight) => sight,
            State::Switching { to, .. } => to.other(),
        }
    }

    /// The new state after `event`, for a weapon of `sights`.
    pub fn step(self, sights: Sights, event: Event) -> State {
        if sights == Sights::One {
            return State::MAIN;
        }
        match (self, event) {
            (State::Settled(sight), Event::Press(now)) => State::Switching { to: sight.other(), pressed: now },
            // a press while switching is nothing (`request_aim_slot_switch`: a switch pending)
            (State::Switching { .. }, Event::Press(_)) => self,
            (State::Switching { to, pressed }, Event::Tick(now)) if now.since(end(pressed)) >= 0 => State::Settled(to),
            (State::Switching { .. }, Event::Deactivate) => State::Settled(self.from()),
            (_, Event::Tick(_) | Event::Deactivate) => self,
        }
    }

    /// When the switch's blend starts: the first tick after the press.
    pub const fn start(self) -> Option<GameTime> {
        match self {
            State::Switching { pressed, .. } => Some(pressed.next_tick()),
            State::Settled(_) => None,
        }
    }

    /// When the switch ends: [`SWITCH_MS`] after the press.
    pub const fn end(self) -> Option<GameTime> {
        match self {
            State::Switching { pressed, .. } => Some(end(pressed)),
            State::Settled(_) => None,
        }
    }

    /// The switch's milliseconds into its blend at `now`, 0..=[`SWITCH_MS`]:
    /// 0 settled and before its start.
    fn blended_ms(self, now: GameTime) -> u32 {
        match self.start() {
            Some(start) => now.since(start).clamp(0, SWITCH_MS as i32) as u32,
            None => 0,
        }
    }

    /// What the view shows at `now`.
    pub fn view(self, now: GameTime) -> View {
        View { from: self.from(), to: self.to(), s: self.blended_ms(now) as f32 / SWITCH_MS as f32 }
    }

    /// The sight switched to, or the one it is on.
    pub const fn to(self) -> Sight {
        match self {
            State::Settled(sight) => sight,
            State::Switching { to, .. } => to,
        }
    }

    /// Whether a weapon aimed (its m_aimed set) counts as aimed at `now`: not
    /// while a switch lowers its aim (min(s, 1 - s) over 0), as 0.69d0's
    /// recoil takes the aimed values only at an effective aim of 1.
    pub fn aimed(self, now: GameTime) -> bool {
        let ms = self.blended_ms(now);
        ms.min(SWITCH_MS - ms) == 0
    }

    /// `aim` (the weapon's aim progress) as the switch lowers it at `now`.
    pub fn effective_aim(self, aim: f32, now: GameTime) -> f32 {
        self.view(now).effective_aim(aim)
    }
}

const fn end(pressed: GameTime) -> GameTime {
    pressed.after(SWITCH_MS)
}

/// The sights of a player's two weapon slots as a world holds them: client
/// patch 206 appends one to a player's state, after patch 78's carry
/// snapshot (`base_player::serialize` 0x2DAF60's epilogue).
///
/// ```text
/// for slot 7, then slot 10:
///   u8  0 settled on the main sight, 1 on the second,
///       2 switching to the second, 3 to the main
///   u32 the press's time when switching, else 0
/// u8  version (1), the record's last byte
/// ```
///
/// The version is last so that a state can be read from its end
/// ([`Record::split_end`]): an older state ends with the carry snapshot,
/// whose last byte is 0 or the high byte of a carry generation (one in 2^24
/// generations), and only a record decodes as one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Record {
    first: State,
    second: State,
}

impl Record {
    /// Its bytes in a world.
    pub const BYTES: usize = 11;
    /// Its version, its last byte.
    pub const VERSION: u8 = 1;
    /// Both slots on their main sight.
    pub const MAIN: Record = Record { first: State::MAIN, second: State::MAIN };

    pub const fn new(first: State, second: State) -> Record {
        Record { first, second }
    }

    /// The state of the weapon in `slot`.
    pub const fn get(&self, slot: WeaponSlot) -> State {
        match slot {
            WeaponSlot::First => self.first,
            WeaponSlot::Second => self.second,
        }
    }

    pub fn encode(&self) -> [u8; Record::BYTES] {
        let mut out = [0; Record::BYTES];
        for (at, state) in [(0, self.first), (5, self.second)] {
            let (tag, pressed) = match state {
                State::Settled(Sight::Main) => (0, 0),
                State::Settled(Sight::Second) => (1, 0),
                State::Switching { to: Sight::Second, pressed } => (2, pressed.0),
                State::Switching { to: Sight::Main, pressed } => (3, pressed.0),
            };
            out[at] = tag;
            out[at + 1..at + 5].copy_from_slice(&pressed.to_le_bytes());
        }
        out[Record::BYTES - 1] = Record::VERSION;
        out
    }

    /// The record of `bytes`, `None` for any bytes [`Record::encode`] does
    /// not make (another version, a tag past 3, a time on a settled sight).
    pub fn decode(bytes: &[u8; Record::BYTES]) -> Option<Record> {
        let state = |at: usize| {
            let pressed = u32::from_le_bytes([bytes[at + 1], bytes[at + 2], bytes[at + 3], bytes[at + 4]]);
            match (bytes[at], pressed) {
                (0, 0) => Some(State::Settled(Sight::Main)),
                (1, 0) => Some(State::Settled(Sight::Second)),
                (2, pressed) => Some(State::Switching { to: Sight::Second, pressed: GameTime(pressed) }),
                (3, pressed) => Some(State::Switching { to: Sight::Main, pressed: GameTime(pressed) }),
                _ => None,
            }
        };
        if bytes[Record::BYTES - 1] != Record::VERSION {
            return None;
        }
        Some(Record { first: state(0)?, second: state(5)? })
    }

    /// `state` split before the record it ends with, `None` when its last
    /// [`Record::BYTES`] are no record.
    pub fn split_end(state: &[u8]) -> Option<(&[u8], Record)> {
        let (before, bytes) = state.split_at_checked(state.len().checked_sub(Record::BYTES)?)?;
        Some((before, Record::decode(bytes.try_into().ok()?)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: GameTime = GameTime(104_520);

    fn press(state: State, now: u32) -> State {
        state.step(Sights::Two, Event::Press(GameTime(now)))
    }

    fn tick(state: State, now: u32) -> State {
        state.step(Sights::Two, Event::Tick(GameTime(now)))
    }

    #[test]
    fn a_press_switches_to_the_other_sight_until_its_end() {
        let switching = press(State::MAIN, T.0);
        assert_eq!(switching, State::Switching { to: Sight::Second, pressed: T });
        assert_eq!(switching.start(), Some(GameTime(104_550)), "the first tick after the press");
        assert_eq!(switching.end(), Some(GameTime(105_020)));
        assert_eq!(tick(switching, 105_000), switching);
        assert_eq!(tick(switching, 105_020), State::Settled(Sight::Second));
        assert_eq!(tick(switching, 105_050), State::Settled(Sight::Second), "a tick past the end");
        let back = press(State::Settled(Sight::Second), 200_000);
        assert_eq!(back, State::Switching { to: Sight::Main, pressed: GameTime(200_000) });
        assert_eq!(back.start(), Some(GameTime(200_050)), "a press on a tick: the next one");
        assert_eq!(tick(back, 200_500), State::MAIN);
    }

    #[test]
    fn a_press_while_switching_is_nothing() {
        let switching = press(State::MAIN, T.0);
        assert_eq!(press(switching, T.0 + 100), switching);
        assert_eq!(tick(switching, T.0 + 100), switching);
    }

    #[test]
    fn putting_the_weapon_away_undoes_a_switch_and_keeps_a_sight() {
        let switching = press(State::Settled(Sight::Second), T.0);
        assert_eq!(switching.step(Sights::Two, Event::Deactivate), State::Settled(Sight::Second));
        assert_eq!(State::Settled(Sight::Second).step(Sights::Two, Event::Deactivate), State::Settled(Sight::Second));
        assert_eq!(State::MAIN.step(Sights::Two, Event::Deactivate), State::MAIN);
    }

    #[test]
    fn a_weapon_without_a_second_sight_is_always_on_its_main() {
        for state in [State::MAIN, State::Settled(Sight::Second), press(State::MAIN, T.0)] {
            for event in [Event::Press(T), Event::Tick(T), Event::Deactivate] {
                assert_eq!(state.step(Sights::One, event), State::MAIN, "{state:?} {event:?}");
            }
        }
    }

    #[test]
    fn the_blend_runs_from_the_next_tick_and_the_sight_shown_changes_half_way() {
        let switching = press(State::MAIN, T.0);
        let at = |ms: u32| switching.view(GameTime(ms));
        assert_eq!(at(T.0).s, 0.0);
        assert_eq!(at(104_550).s, 0.0, "the blend's start");
        assert_eq!(at(104_800).s, 0.5);
        assert_eq!(at(104_800).shown(), Sight::Main);
        assert_eq!(at(104_850).s, 0.6);
        assert_eq!(at(104_850).shown(), Sight::Second);
        assert_eq!(at(105_000).s, 0.9, "the last tick before the end");
        assert_eq!(State::Settled(Sight::Second).view(T), View { from: Sight::Second, to: Sight::Second, s: 0.0 });
        assert_eq!(State::Settled(Sight::Second).view(T).shown(), Sight::Second);
    }

    #[test]
    fn the_aim_dips_by_the_switch_and_counts_only_outside_it() {
        let switching = press(State::MAIN, T.0);
        assert_eq!(switching.effective_aim(1.0, GameTime(104_550)), 1.0);
        assert!(switching.aimed(GameTime(104_550)) && switching.aimed(T), "before the blend: aimed");
        assert_eq!(switching.effective_aim(1.0, GameTime(104_800)), 0.5, "half way: down by 0.5");
        assert_eq!(switching.effective_aim(0.3, GameTime(104_800)), 0.0, "clamped");
        assert!((switching.effective_aim(1.0, GameTime(105_000)) - 0.9).abs() < 1e-6);
        assert!(!switching.aimed(GameTime(104_600)) && !switching.aimed(GameTime(105_000)));
        assert!(State::Settled(Sight::Second).aimed(T));
        assert_eq!(State::Settled(Sight::Second).effective_aim(0.75, T), 0.75);
    }

    #[test]
    fn the_clock_wraps() {
        let late = GameTime(u32::MAX - 100);
        let switching = press(State::MAIN, late.0);
        assert_eq!(switching.end(), Some(GameTime(399)));
        assert_eq!(tick(switching, 398), switching);
        assert_eq!(tick(switching, 399), State::Settled(Sight::Second));
        assert_eq!(GameTime(10).since(GameTime(u32::MAX)), 11);
        assert_eq!(GameTime(u32::MAX).since(GameTime(10)), -11);
    }

    #[test]
    fn a_record_is_its_bytes_and_only_those() {
        let record = Record::new(State::Settled(Sight::Second), State::Switching { to: Sight::Main, pressed: T });
        let bytes = record.encode();
        assert_eq!(bytes, [1, 0, 0, 0, 0, 3, 0x48, 0x98, 0x01, 0x00, 1]);
        assert_eq!(Record::decode(&bytes), Some(record));
        assert_eq!(record.get(WeaponSlot::First), State::Settled(Sight::Second));
        assert_eq!(Record::decode(&Record::MAIN.encode()), Some(Record::MAIN));
        // another version, a tag past 3, a time on a settled sight (slot 7's second, slot 10's main)
        for (at, byte) in [(10, 2), (10, 0), (0, 4), (1, 1), (5, 0)] {
            let mut bad = bytes;
            bad[at] = byte;
            assert_eq!(Record::decode(&bad), None, "byte {at} = {byte}");
        }
    }

    #[test]
    fn a_state_is_split_before_its_record() {
        let record = Record::new(State::MAIN, State::Switching { to: Sight::Second, pressed: T });
        let mut state = vec![9u8; 40];
        state.extend(record.encode());
        let (before, read) = Record::split_end(&state).unwrap();
        assert_eq!((before.len(), read), (40, record));
        // an older state: a carry snapshot's end
        let mut older = vec![9u8; 40];
        older.extend([2, 1, 0, 0, 0]);
        older.extend([0; 27]);
        assert_eq!(Record::split_end(&older), None);
        assert_eq!(Record::split_end(&[1; 5]), None);
    }
}
