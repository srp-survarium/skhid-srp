//! Carry presentation over the player's existing native weapon. The weapon
//! remains owned and serialized by the engine; this machine controls only
//! the offer, action suppression and the hand/weapon presentation.

use crate::{Artefact, ArtefactId, ContainerIndex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Time(pub u32);
impl Time {
    fn elapsed(self, start: Self) -> u32 { self.0.wrapping_sub(start.0) }
}

pub const LOWER_WEAPON_MS: u32 = 200;
pub const RAISE_MS: u32 = 600;
pub const LOWER_HAND_MS: u32 = 500;
pub const RESTORE_MS: u32 = 200;
/// A container's artefact shown in the hand (the owner, 2026-11-02): the
/// weapon goes down (LOWER_WEAPON_MS), the artefact comes up, is held, goes
/// back down, and the weapon comes back (RESTORE_MS): 2 s in all, the two
/// seconds the containers' search time lost (content-edit artefact-hold-1).
pub const SHOW_RAISE_MS: u32 = 300;
pub const SHOW_HOLD_MS: u32 = 1000;
pub const SHOW_LOWER_MS: u32 = 300;
pub const SHOW_MS: u32 = LOWER_WEAPON_MS + SHOW_RAISE_MS + SHOW_HOLD_MS + SHOW_LOWER_MS + RESTORE_MS;
/// The carry chord's marker (bit 29, unused natively): with exactly one
/// QUICK bit it offers that slot's artefact. Bits 30 and 31 stay clear: the
/// relay readers (0x31847B, 0x27C509) and replays (0x1EE2A1) strip bit 31
/// as the input's own wire flag.
pub const CHORD: u32 = 1 << 29;
pub const USE: u32 = 1 << 21;
pub const QUICK: u32 = 63 << 14;
pub const MELEE: u32 = 1 << 27;
/// Fire, reload, aim, sprint, weapon selection, quick use, breath, drop,
/// grenade and melee. Looking, movement, crouch, jump and suicide survive.
pub const BLOCKED: u32 = 0xe0 | (1 << 9) | (3 << 12) | QUICK | (1 << 20)
    | (3 << 22) | (3 << 25) | MELEE;
/// Presses that put the artefact away: fire, reload, aim, weapon selection,
/// the wheel, grenade, melee, and a quick use without the chord. Each
/// stays blocked until the hand is stowed, then reaches the game.
const CANCEL: u32 = 0xe0 | (3 << 12) | (3 << 25) | (1 << 23) | MELEE;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuickSlot(u8);
impl QuickSlot {
    pub const fn parse(n: u8) -> Option<Self> { if n < 6 { Some(Self(n)) } else { None } }
    pub const fn get(self) -> u8 { self.0 }
    pub const fn command(self) -> u32 { CHORD | 1 << (14 + self.0 as u32) }
    /// The marker with exactly one QUICK bit; anything else (two inputs
    /// OR-merged in one node, 0x2C5B60) is no command.
    pub const fn from_actions(actions: u32) -> Option<Self> {
        let quick = (actions & QUICK) >> 14;
        if actions & CHORD != 0 && quick.count_ones() == 1 { Some(Self(quick.trailing_zeros() as u8)) } else { None }
    }
}

/// Input builder conversion. One selected quick slot only; ambiguous chords
/// are consumed without offering or activating an artefact.
pub fn encode_chord(actions: u32, modifier_down: bool) -> u32 {
    let quick = (actions & QUICK) >> 14;
    let clean = actions & !CHORD;
    if !modifier_down || quick == 0 { return clean; }
    if quick.count_ones() != 1 { return clean & !QUICK; }
    clean | CHORD
}

/// One of the carry key's own events in an input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyPress {
    Pressed,
    Released,
}

/// The carry key (H, action 23h), kept from its down event to its up event
/// as player_input_handler::on_keyboard_action gets them (patches: site
/// carry.key), since the input builder's action query sees the down and not
/// always the up: H held, then a quick key, is the chord as much as the other
/// order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CarryKey {
    #[default]
    Up,
    Down,
}
impl CarryKey {
    pub const fn step(self, press: Option<KeyPress>) -> Self {
        match press {
            Some(KeyPress::Pressed) => Self::Down,
            Some(KeyPress::Released) => Self::Up,
            None => self,
        }
    }
    pub const fn is_down(self) -> bool { matches!(self, Self::Down) }
}

/// Client input latch: releasing the modifier before the quick key must
/// not turn the same press into native artefact activation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chord {
    Idle,
    Held { keys: u8, command: Option<QuickSlot> },
}
impl Chord {
    pub fn step(&mut self, actions: u32, modifier_down: bool) -> u32 {
        let keys = ((actions & QUICK) >> 14) as u8;
        if let Self::Held { keys: held, command } = *self {
            if keys & held != 0 {
                return (actions & !(CHORD | (u32::from(held) << 14)))
                    | command.map_or(0, QuickSlot::command);
            }
            *self = Self::Idle;
        }
        let encoded = encode_chord(actions, modifier_down);
        if modifier_down && keys != 0 {
            *self = Self::Held { keys, command: QuickSlot::from_actions(encoded) };
        }
        encoded
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    pub artefact: Artefact,
    pub generation: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Stowed,
    Switching { current: Option<Token>, next: Token, start: Time, from: u16 },
    LoweringWeapon { token: Token, start: Time, from: u16 },
    Raising { token: Token, start: Time, from: u16 },
    Offering(Token),
    LoweringHand { token: Token, start: Time, from: u16 },
    LoweringEmptyHand { start: Time, from: u16 },
    RestoringWeapon { start: Time, from: u16 },
    /// A container's artefact just taken, shown from `start` for SHOW_MS:
    /// nothing the player presses ends it, no one can take it, and it stays
    /// in its slot (its passive on from the take).
    Showing { token: Token, start: Time },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Carry {
    phase: Phase,
    generation: u32,
    last_raw: u32,
    last_passed: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Tick { now: Time, owned: bool },
    Input { now: Time, actions: u32, artefact: Option<Artefact> },
    Taken { now: Time, token: Token },
    /// A container gave this artefact (artefact_container_core::use_execute).
    PickedUp { now: Time, artefact: Artefact },
    Gone,
    PendingLost,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Presentation {
    pub artefact: Option<Artefact>,
    /// Blend the original weapon pose into the lowered hand pose.
    pub arms: f32,
    /// Authored raise curve, 0 lowered, 1 palm up.
    pub raised: f32,
    pub hide_weapon: bool,
}

/// A presentation fraction (0..=1) as the snapshot keeps it.
fn unit(value: f32) -> u16 { (value * 65535.0) as u16 }

impl Default for Carry { fn default() -> Self { Self::NEW } }
impl Carry {
    pub const NEW: Self = Self { phase: Phase::Stowed, generation: 0, last_raw: 0, last_passed: 0 };
    pub const fn phase(self) -> Phase { self.phase }
    pub const fn active(self) -> bool { !matches!(self.phase, Phase::Stowed) }
    pub const fn artefact(self) -> Option<Artefact> {
        match self.phase {
            Phase::LoweringWeapon { token, .. } | Phase::Raising { token, .. }
            | Phase::Offering(token) | Phase::LoweringHand { token, .. }
            | Phase::Showing { token, .. } => Some(token.artefact),
            Phase::Switching { current: Some(token), .. } => Some(token.artefact),
            _ => None,
        }
    }
    /// The artefact a death drops: one the key brought out. A pickup's
    /// show has none to drop: its artefact is in its slot.
    pub const fn dropped_at_death(self) -> Option<Artefact> {
        if let Phase::Showing { .. } = self.phase { None } else { self.artefact() }
    }
    pub const fn pending(self) -> Option<Artefact> {
        if let Phase::Switching { next, .. } = self.phase { Some(next.artefact) } else { None }
    }
    pub const fn presentation_token(self) -> Option<Token> {
        match self.phase {
            Phase::Raising { token, .. } | Phase::Offering(token) | Phase::LoweringHand { token, .. }
            | Phase::Showing { token, .. } => Some(token),
            Phase::Switching { current, .. } => current,
            _ => None,
        }
    }
    pub const fn offer(self) -> Option<Token> {
        if let Phase::Offering(token) = self.phase { Some(token) } else { None }
    }
    pub const fn use_pressed(self, raw: u32) -> bool { raw & USE != 0 && self.last_raw & USE == 0 }
    pub const fn filtered(self) -> u32 { self.last_passed }

    /// Suppress the entire press after the take handler consumes it. A held
    /// Use must not fall through to a native pickup on the following tick.
    pub fn consume_use(&mut self) { self.last_passed &= !USE; }

    pub fn step(&mut self, event: Event) {
        match event {
            // A held-item change ends presentation, not the physical button
            // press. Keep its edge/consumption until release; respawn gets a
            // fresh owner through the native spawn identity.
            Event::Gone => { self.phase = Phase::Stowed; }
            Event::PendingLost => {
                if let Phase::Switching { current, start, from, .. } = self.phase {
                    self.phase = match current {
                        Some(token) => Phase::LoweringHand { token, start, from },
                        None => Phase::LoweringEmptyHand { start, from },
                    };
                }
            }
            Event::Taken { now, token } => {
                if self.offer() == Some(token) {
                    // No recipient pickup animation. Donor lowers an empty hand.
                    self.phase = Phase::LoweringEmptyHand { start: now, from: u16::MAX };
                }
            }
            // Only from an empty hand: a pickup while the key's carry is on
            // is not shown (its hand is busy), the artefact goes to its slot.
            Event::PickedUp { now, artefact } => {
                if let Phase::Stowed = self.phase {
                    self.generation = self.generation.wrapping_add(1);
                    self.phase = Phase::Showing { token: Token { artefact, generation: self.generation }, start: now };
                }
            }
            Event::Tick { now, owned } => {
                if self.artefact().is_some() && !owned {
                    self.phase = match self.phase {
                        Phase::Switching { next, start, from, .. } => Phase::Switching { current: None, next, start, from },
                        _ => Phase::LoweringEmptyHand { start: now, from: self.height(now) },
                    };
                }
                // A late tick can cross multiple boundaries. Times are absolute
                // and wrapping, so the result does not depend on tick splitting.
                for _ in 0..4 {
                    self.phase = match self.phase {
                        Phase::Switching { next, start, .. } if now.elapsed(start) >= LOWER_HAND_MS =>
                            Phase::Raising { token: next, start: Time(start.0.wrapping_add(LOWER_HAND_MS)), from: 0 },
                        Phase::LoweringWeapon { token, start, .. } if now.elapsed(start) >= LOWER_WEAPON_MS =>
                            Phase::Raising { token, start: Time(start.0.wrapping_add(LOWER_WEAPON_MS)), from: 0 },
                        Phase::Raising { token, start, .. } if now.elapsed(start) >= RAISE_MS => Phase::Offering(token),
                        Phase::LoweringHand { start, .. } | Phase::LoweringEmptyHand { start, .. } if now.elapsed(start) >= LOWER_HAND_MS =>
                            Phase::RestoringWeapon { start: Time(start.0.wrapping_add(LOWER_HAND_MS)), from: u16::MAX },
                        Phase::RestoringWeapon { start, .. } if now.elapsed(start) >= RESTORE_MS => Phase::Stowed,
                        Phase::Showing { start, .. } if now.elapsed(start) >= SHOW_MS => Phase::Stowed,
                        other => other,
                    };
                }
            }
            Event::Input { now, actions: raw, artefact } => {
                let command = QuickSlot::from_actions(raw);
                if let Phase::Showing { .. } = self.phase {
                    // The show runs to its end: no command, no cancel.
                } else if command.is_some() && command != QuickSlot::from_actions(self.last_raw) {
                    if let Some(artefact) = artefact {
                        if let Phase::LoweringHand { token, .. } = self.phase && token.artefact == artefact {
                            // Pressed again while it goes down: back up from here.
                            self.phase = Phase::Raising { token, start: now, from: self.height(now) };
                        } else if self.pending().or(self.artefact()) == Some(artefact) {
                            self.cancel(now);
                        } else {
                            self.generation = self.generation.wrapping_add(1);
                            let token = Token { artefact, generation: self.generation };
                            self.phase = match self.phase {
                                Phase::LoweringWeapon { start, from, .. } => Phase::LoweringWeapon { token, start, from },
                                Phase::Raising { token: old, .. } | Phase::Offering(old) => Phase::Switching {
                                    current: Some(old), next: token, start: now, from: self.height(now),
                                },
                                Phase::LoweringHand { token: old, start, from } => Phase::Switching { current: Some(old), next: token, start, from },
                                Phase::LoweringEmptyHand { start, from } => Phase::Switching { current: None, next: token, start, from },
                                Phase::Switching { current, start, from, .. } => Phase::Switching { current, next: token, start, from },
                                Phase::RestoringWeapon { .. } | Phase::Stowed =>
                                    Phase::LoweringWeapon { token, start: now, from: unit(self.presentation(now).arms) },
                                Phase::Showing { .. } => self.phase,
                            };
                        }
                    }
                } else if self.active() && (raw & CANCEL != 0 || (raw & CHORD == 0 && raw & QUICK != 0)) {
                    self.cancel(now);
                }
                let consumed_use = self.last_raw & USE != 0 && self.last_passed & USE == 0;
                self.last_raw = raw;
                self.last_passed = raw & !CHORD;
                if self.active() { self.last_passed &= !BLOCKED; }
                // A chord's quick keys never reach the game, even an ambiguous one.
                if raw & CHORD != 0 { self.last_passed &= !QUICK; }
                if consumed_use { self.last_passed &= !USE; }
            }
        }
    }

    fn cancel(&mut self, now: Time) {
        self.phase = match self.phase {
            Phase::Switching { current, start, from, .. } => match current {
                Some(token) => Phase::LoweringHand { token, start, from },
                None => Phase::LoweringEmptyHand { start, from },
            },
            Phase::LoweringWeapon { .. } => Phase::RestoringWeapon { start: now, from: unit(self.presentation(now).arms) },
            Phase::Raising { token, .. } | Phase::Offering(token) => Phase::LoweringHand {
                token, start: now, from: self.height(now),
            },
            other => other,
        };
    }

    fn height(self, now: Time) -> u16 { unit(self.presentation(now).raised) }

    pub fn presentation(self, now: Time) -> Presentation {
        let blend = |start, duration| {
            let t = (now.elapsed(start) as f32 / duration as f32).min(1.0);
            t * t * (3.0 - 2.0 * t)
        };
        let rise = |from: u16, start, duration| {
            let from = f32::from(from) / 65535.0;
            from + (1.0 - from) * blend(start, duration)
        };
        match self.phase {
            Phase::Switching { current, start, from, .. } => Presentation {
                artefact: current.map(|t| t.artefact), arms: 1.0,
                raised: f32::from(from) / 65535.0 * (1.0 - blend(start, LOWER_HAND_MS)), hide_weapon: true,
            },
            Phase::Stowed => Presentation { artefact: None, arms: 0.0, raised: 0.0, hide_weapon: false },
            Phase::LoweringWeapon { start, from, .. } => Presentation { artefact: None, arms: rise(from, start, LOWER_WEAPON_MS), raised: 0.0, hide_weapon: false },
            Phase::Raising { token, start, from } => Presentation { artefact: Some(token.artefact), arms: 1.0, raised: rise(from, start, RAISE_MS), hide_weapon: true },
            Phase::Offering(token) => Presentation { artefact: Some(token.artefact), arms: 1.0, raised: 1.0, hide_weapon: true },
            Phase::LoweringHand { token, start, from } => Presentation { artefact: Some(token.artefact), arms: 1.0, raised: f32::from(from) / 65535.0 * (1.0 - blend(start, LOWER_HAND_MS)), hide_weapon: true },
            Phase::LoweringEmptyHand { start, from } => Presentation { artefact: None, arms: 1.0, raised: f32::from(from) / 65535.0 * (1.0 - blend(start, LOWER_HAND_MS)), hide_weapon: true },
            Phase::RestoringWeapon { start, from } => Presentation { artefact: None, arms: f32::from(from) / 65535.0 * (1.0 - blend(start, RESTORE_MS)), raised: 0.0, hide_weapon: false },
            Phase::Showing { token, start } => {
                let at = |offset: u32| Time(start.0.wrapping_add(offset));
                let up = LOWER_WEAPON_MS;
                let held = up + SHOW_RAISE_MS;
                let down = held + SHOW_HOLD_MS;
                let back = down + SHOW_LOWER_MS;
                match now.elapsed(start) {
                    e if e < up => Presentation { artefact: None, arms: blend(start, LOWER_WEAPON_MS), raised: 0.0, hide_weapon: false },
                    e if e < back => Presentation {
                        artefact: Some(token.artefact), arms: 1.0, hide_weapon: true,
                        raised: if e < down { blend(at(up), SHOW_RAISE_MS) } else { 1.0 - blend(at(down), SHOW_LOWER_MS) },
                    },
                    _ => Presentation { artefact: None, arms: 1.0 - blend(at(back), RESTORE_MS), raised: 0.0, hide_weapon: false },
                }
            }
        }
    }

    pub const SNAPSHOT_SIZE: usize = 32;
    pub fn snapshot(self) -> [u8; Self::SNAPSHOT_SIZE] {
        let mut out = [0; Self::SNAPSHOT_SIZE];
        out[0] = 2;
        out[1..5].copy_from_slice(&self.generation.to_le_bytes());
        out[5..9].copy_from_slice(&self.last_raw.to_le_bytes());
        out[9..13].copy_from_slice(&self.last_passed.to_le_bytes());
        let (tag, token, start, from) = match self.phase {
            Phase::Switching { current, next, start, from } => {
                out[26] = next.artefact.id.get(); out[27] = next.artefact.slot.get();
                out[28..32].copy_from_slice(&next.generation.to_le_bytes());
                (if current.is_some() { 7 } else { 8 }, current, start, from)
            },
            Phase::Stowed => (0, None, Time(0), 0),
            Phase::LoweringWeapon { token, start, from } => (1, Some(token), start, from),
            Phase::Raising { token, start, from } => (2, Some(token), start, from),
            Phase::Offering(token) => (3, Some(token), Time(0), 0),
            Phase::LoweringHand { token, start, from } => (4, Some(token), start, from),
            Phase::RestoringWeapon { start, from } => (5, None, start, from),
            Phase::LoweringEmptyHand { start, from } => (6, None, start, from),
            Phase::Showing { token, start } => (9, Some(token), start, 0),
        };
        out[13] = tag;
        if let Some(token) = token {
            out[14] = token.artefact.id.get(); out[15] = token.artefact.slot.get();
            out[16..20].copy_from_slice(&token.generation.to_le_bytes());
        }
        out[20..24].copy_from_slice(&start.0.to_le_bytes());
        out[24..26].copy_from_slice(&from.to_le_bytes());
        out
    }

    pub fn restore(bytes: [u8; Self::SNAPSHOT_SIZE]) -> Option<Self> {
        if bytes[0] != 2 { return None; }
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let generation = word(1);
        let start = Time(word(20));
        let from = u16::from_le_bytes([bytes[24], bytes[25]]);
        let token = || Some(Token { artefact: Artefact {
            id: ArtefactId::parse(bytes[14])?, slot: ContainerIndex::parse(bytes[15])?,
        }, generation: word(16) });
        let phase = match bytes[13] {
            7 | 8 => Phase::Switching {
                current: if bytes[13] == 7 { Some(token()?) } else { None },
                next: Token { artefact: Artefact { id: ArtefactId::parse(bytes[26])?, slot: ContainerIndex::parse(bytes[27])? }, generation: word(28) },
                start, from,
            },
            0 => Phase::Stowed,
            1 => Phase::LoweringWeapon { token: token()?, start, from },
            2 => Phase::Raising { token: token()?, start, from },
            3 => Phase::Offering(token()?),
            4 => Phase::LoweringHand { token: token()?, start, from },
            5 => Phase::RestoringWeapon { start, from },
            6 => Phase::LoweringEmptyHand { start, from },
            9 => Phase::Showing { token: token()?, start },
            _ => return None,
        };
        let carry = Self { phase, generation, last_raw: word(5), last_passed: word(9) };
        // Canonical encoding rejects junk fields and a mismatched queued generation.
        // Cancelling a switch retains the older visible token.
        let valid_generation = match phase {
            Phase::Switching { current, next, .. } => next.generation == generation && current.is_none_or(|c| c.artefact != next.artefact || c.generation != next.generation),
            _ => true,
        };
        if carry.snapshot() != bytes || !valid_generation { return None; }
        Some(carry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn artefact() -> Artefact { Artefact { id: ArtefactId::parse(7).unwrap(), slot: ContainerIndex::parse(1).unwrap() } }
    fn input(c: &mut Carry, now: u32, actions: u32) { c.step(Event::Input { now: Time(now), actions, artefact: Some(artefact()) }); }
    fn tick(c: &mut Carry, now: u32) { c.step(Event::Tick { now: Time(now), owned: true }); }
    fn equip(at: u32) -> Carry {
        let mut c = Carry::NEW;
        input(&mut c, at, QuickSlot(1).command());
        c
    }
    fn roundtrip(c: Carry) { assert_eq!(Carry::restore(c.snapshot()), Some(c)); }

    #[test]
    fn switching_lowers_old_then_raises_new_and_snapshots_each_half() {
        let second = Artefact { id: ArtefactId::parse(8).unwrap(), slot: ContainerIndex::parse(2).unwrap() };
        for now in [400, 900, u32::MAX - 300] {
            let start = now.wrapping_sub(900);
            let mut c = equip(start); tick(&mut c, now); input(&mut c, now, 0);
            let old = c.offer().unwrap();
            c.step(Event::Input { now: Time(now), actions: QuickSlot(2).command(), artefact: Some(second) });
            assert_eq!(c.offer(), None);
            assert_eq!(c.artefact(), Some(artefact()));
            assert_eq!(c.pending(), Some(second));
            roundtrip(c);
            let mut direct = c;
            tick(&mut c, now.wrapping_add(250));
            assert_eq!(c.presentation(Time(now.wrapping_add(250))).artefact, Some(artefact()));
            assert!((c.presentation(Time(now.wrapping_add(250))).raised - 0.5).abs() < 0.001);
            c.step(Event::Taken { now: Time(now.wrapping_add(250)), token: old });
            assert_eq!(c.pending(), Some(second));
            tick(&mut c, now.wrapping_add(500));
            assert_eq!(c.artefact(), Some(second));
            assert_eq!(c.presentation(Time(now.wrapping_add(500))).raised, 0.0);
            roundtrip(c);
            tick(&mut c, now.wrapping_add(1100)); tick(&mut direct, now.wrapping_add(1100));
            assert_eq!(c, direct);
            assert_eq!(c.offer().unwrap().artefact, second);
        }
    }

    #[test]
    fn switching_during_raise_and_lower_and_lost_pending_do_not_pop() {
        let second = Artefact { id: ArtefactId::parse(8).unwrap(), slot: ContainerIndex::parse(2).unwrap() };
        for now in [300, 500, 850] {
            let mut c = equip(0); tick(&mut c, now); input(&mut c, now, 0);
            let before = c.presentation(Time(now));
            c.step(Event::Input { now: Time(now), actions: QuickSlot(2).command(), artefact: Some(second) });
            assert!((before.raised - c.presentation(Time(now)).raised).abs() < 0.0001);
            assert_eq!(before.artefact, c.presentation(Time(now)).artefact);
            roundtrip(c);
            c.step(Event::PendingLost);
            assert_eq!(c.pending(), None); roundtrip(c);
            assert!((before.raised - c.presentation(Time(now)).raised).abs() < 0.0001);
            assert_eq!(before.artefact, c.presentation(Time(now)).artefact);
            tick(&mut c, now+1000); assert!(!c.active());
        }
        let mut c = equip(0); tick(&mut c, 800); input(&mut c, 810, 0);
        c.step(Event::Input { now: Time(820), actions: QuickSlot(2).command(), artefact: Some(second) });
        c.step(Event::Tick { now: Time(900), owned: false });
        assert_eq!(c.artefact(), None); assert_eq!(c.pending(), Some(second)); roundtrip(c);
        tick(&mut c, 1920); assert_eq!(c.offer().unwrap().artefact, second);
        // Repeated same quick-key press cancels the queued switch.
        input(&mut c, 1930, 0);
        c.step(Event::Input { now: Time(1940), actions: QuickSlot(1).command(), artefact: Some(artefact()) });
        input(&mut c, 1950, 0);
        input(&mut c, 1960, QuickSlot(1).command());
        assert_eq!(c.pending(), None); assert!(matches!(c.phase(), Phase::LoweringHand { .. }));
    }

    #[test]
    fn weapon_lowers_before_hand_raises_and_offer_is_not_early() {
        let mut c = equip(100);
        assert_eq!(c.offer(), None);
        assert_eq!(c.presentation(Time(100)).artefact, None);
        tick(&mut c, 300);
        assert_eq!(c.presentation(Time(300)).raised, 0.0);
        assert!(c.presentation(Time(300)).hide_weapon);
        assert!(c.presentation(Time(600)).raised > 0.4);
        assert_eq!(c.offer(), None);
        tick(&mut c, 900);
        assert_eq!(c.offer().unwrap().artefact, artefact());
    }

    #[test]
    fn take_consumes_offer_once_and_lowers_empty_hand_before_weapon_returns() {
        let mut c = equip(0);
        tick(&mut c, 800);
        let token = c.offer().unwrap();
        c.step(Event::Taken { now: Time(900), token });
        assert_eq!(c.offer(), None);
        let first = c;
        c.step(Event::Taken { now: Time(950), token });
        assert_eq!(c, first);
        let drawn = c.presentation(Time(900));
        assert_eq!(drawn.artefact, None);
        assert_eq!(drawn.raised, 1.0);
        assert!(drawn.hide_weapon);
        tick(&mut c, 1400);
        assert!(!c.presentation(Time(1400)).hide_weapon);
        tick(&mut c, 1600);
        assert!(!c.active());
        input(&mut c, 1700, 0);
        input(&mut c, 1800, QuickSlot(1).command());
        tick(&mut c, 2600);
        c.step(Event::Taken { now: Time(2600), token });
        assert!(c.offer().is_some());
        assert_ne!(c.offer(), Some(token));
    }

    #[test]
    fn cancelling_halfway_does_not_pop_or_restart_on_held_fire() {
        let mut c = equip(0);
        tick(&mut c, 200);
        input(&mut c, 400, 0);
        let before = c.presentation(Time(400));
        input(&mut c, 400, 0x20);
        let after = c.presentation(Time(400));
        assert!((before.raised - after.raised).abs() < 0.0001);
        let phase = c.phase();
        input(&mut c, 450, 0x20);
        assert_eq!(c.phase(), phase);
        assert_eq!(c.filtered() & 0x20, 0);
        tick(&mut c, 1100);
        input(&mut c, 1100, 0x20);
        assert_eq!(c.filtered() & 0x20, 0x20);
    }

    #[test]
    fn consumed_use_stays_consumed_until_release_and_survives_restore() {
        let mut c = Carry::NEW;
        assert!(c.use_pressed(USE));
        input(&mut c, 0, USE);
        c.consume_use();
        c = Carry::restore(c.snapshot()).unwrap();
        c.step(Event::Gone); // changing held item must not repeat a take
        assert!(!c.use_pressed(USE));
        input(&mut c, 10, USE);
        assert_eq!(c.filtered() & USE, 0);
        input(&mut c, 20, 0);
        assert!(c.use_pressed(USE));
        input(&mut c, 30, USE);
        assert_ne!(c.filtered() & USE, 0);
    }

    #[test]
    fn native_actions_survive_stowed_and_chords_never_activate_the_item() {
        let mut c = Carry::NEW;
        let actions = 0x20 | 0x100 | (1 << 24) | (1 << 28);
        input(&mut c, 0, actions);
        assert_eq!(c.filtered(), actions);
        for slot in 0..6 {
            let actions = (1 << 9) | (1 << (14 + slot)) | 1;
            let encoded = encode_chord(actions, true);
            assert_eq!(QuickSlot::from_actions(encoded), Some(QuickSlot(slot)));
            assert_eq!(encoded & QUICK, 1 << (14 + slot));
            assert_ne!(encoded & (1 << 9), 0);
            assert_eq!(encoded & 1, 1);
        }
        assert_eq!(encode_chord(QUICK, true), 0);
        assert_eq!(encode_chord(QUICK, false), QUICK);
        assert_eq!(encode_chord(QUICK | (1 << 9), false), QUICK | (1 << 9));
        let mut chord = Chord::Idle;
        let encoded = chord.step(1 << 14, true);
        assert_eq!(chord.step(1 << 14, false), encoded);
        assert_eq!(chord.step(0, false), 0);
        assert_eq!(chord.step(1 << 14, false), 1 << 14);
        let mut c = equip(0);
        input(&mut c, 1, BLOCKED | 0x11 | (1 << 24));
        assert_eq!(c.filtered() & BLOCKED, 0);
        assert_eq!(c.filtered() & (0x11 | (1 << 24)), 0x11 | (1 << 24));
    }

    /// The relays and replays keep 31 bits of an input (0x31847B, 0x27C509,
    /// 0x1EE2A1), and a node may OR two inputs together (0x2C5B60).
    #[test]
    fn commands_survive_the_wire_and_merged_inputs_are_no_command() {
        for slot in 0..6 {
            let command = QuickSlot(slot).command();
            assert_eq!(command & 0x7FFF_FFFF, command);
            assert_eq!(QuickSlot::from_actions(command & 0x7FFF_FFFF), Some(QuickSlot(slot)));
            for other in (0..6).filter(|&o| o != slot) {
                assert_eq!(QuickSlot::from_actions(command | QuickSlot(other).command()), None);
                assert_eq!(QuickSlot::from_actions(command | 1 << (14 + other)), None);
            }
        }
        assert_eq!(QuickSlot::from_actions(QUICK & (1 << 15)), None);
        assert_eq!(QuickSlot::from_actions(CHORD), None);
        // A merged chord never reaches the game as quick use.
        let mut c = Carry::NEW;
        input(&mut c, 0, QuickSlot(0).command() | QuickSlot(3).command());
        assert!(!c.active());
        assert_eq!(c.filtered() & (QUICK | CHORD), 0);
    }

    #[test]
    fn quick_use_and_melee_put_the_artefact_away_then_reach_the_game() {
        for press in [1 << 16, MELEE] {
            let mut c = equip(0);
            tick(&mut c, 800); input(&mut c, 800, 0);
            input(&mut c, 810, press);
            assert!(matches!(c.phase(), Phase::LoweringHand { .. }), "{press:#x}");
            assert_eq!(c.filtered() & press, 0);
            tick(&mut c, 1600);
            assert!(!c.active());
            input(&mut c, 1600, press);
            assert_eq!(c.filtered() & press, press);
        }
    }

    /// The owner, 2026-10-29: no sprint while carrying, and sprint does not
    /// put the artefact away.
    #[test]
    fn sprint_does_nothing_while_carrying() {
        let sprint = 1 << 9;
        let mut c = equip(0);
        tick(&mut c, 800); input(&mut c, 800, sprint);
        assert!(c.offer().is_some());
        assert_eq!(c.filtered() & sprint, 0);
    }

    #[test]
    fn pressing_again_while_lowering_or_restoring_does_not_pop() {
        let mut c = equip(0);
        tick(&mut c, 800); input(&mut c, 800, 0);
        input(&mut c, 810, QuickSlot(1).command());
        input(&mut c, 900, 0);
        tick(&mut c, 1000);
        let before = c.presentation(Time(1000));
        assert!(before.raised > 0.1 && before.raised < 0.9);
        input(&mut c, 1000, QuickSlot(1).command());
        assert!(matches!(c.phase(), Phase::Raising { .. }));
        assert!((c.presentation(Time(1000)).raised - before.raised).abs() < 0.0001);
        roundtrip(c);
        tick(&mut c, 1600);
        assert_eq!(c.offer().unwrap().artefact, artefact());
        // Put away, and pressed again while the weapon comes back.
        input(&mut c, 1600, 0); input(&mut c, 1610, QuickSlot(1).command());
        input(&mut c, 1620, 0);
        tick(&mut c, 2210);
        assert!(matches!(c.phase(), Phase::RestoringWeapon { .. }));
        let before = c.presentation(Time(2210));
        assert!(before.arms > 0.1 && before.arms < 0.9);
        input(&mut c, 2210, QuickSlot(1).command());
        assert!(matches!(c.phase(), Phase::LoweringWeapon { .. }));
        assert!((c.presentation(Time(2210)).arms - before.arms).abs() < 0.0001);
        roundtrip(c);
    }

    #[test]
    fn snapshots_cover_all_phases_and_time_wrap_is_split_independent() {
        let start = u32::MAX - 100;
        let mut split = equip(start);
        roundtrip(split);
        tick(&mut split, start.wrapping_add(200));
        roundtrip(split);
        tick(&mut split, start.wrapping_add(800));
        roundtrip(split);
        let mut direct = equip(start);
        tick(&mut direct, start.wrapping_add(800));
        assert_eq!(split, direct);
        input(&mut split, 800, 0);
        input(&mut split, 810, QuickSlot(1).command());
        roundtrip(split);
        tick(&mut split, 1310);
        roundtrip(split);
        tick(&mut split, 1510);
        roundtrip(split);
        assert!(!split.active());
        let mut invalid = split.snapshot(); invalid[13] = 99;
        assert!(Carry::restore(invalid).is_none());
        let mut invalid = split.snapshot(); invalid[14] = 1;
        assert!(Carry::restore(invalid).is_none());
        let mut offering = equip(0); tick(&mut offering, 800);
        let mut invalid = offering.snapshot(); invalid[26] = 1;
        assert!(Carry::restore(invalid).is_none());
        offering.step(Event::Tick { now: Time(850), owned: false });
        roundtrip(offering);
        assert!(offering.offer().is_none());
        offering.step(Event::Gone);
        assert!(!offering.active());
        roundtrip(offering);
    }

    fn pick_up(c: &mut Carry, now: u32) { c.step(Event::PickedUp { now: Time(now), artefact: artefact() }); }

    /// The owner, 2026-11-02: a container's artefact comes into the hand,
    /// stays a second and goes back; weapon away, no shooting meanwhile.
    #[test]
    fn a_pickup_shows_the_artefact_then_gives_the_weapon_back() {
        for start in [100, u32::MAX - 700] {
            let at = |ms: u32| start.wrapping_add(ms);
            let mut c = Carry::NEW;
            pick_up(&mut c, start);
            assert!(matches!(c.phase(), Phase::Showing { .. }));
            assert_eq!(c.artefact(), Some(artefact()));
            assert_eq!(c.dropped_at_death(), None, "a death drops a shown pickup");
            roundtrip(c);
            let shown = |c: &Carry, ms| c.presentation(Time(at(ms)));
            assert_eq!(shown(&c, 0), Presentation { artefact: None, arms: 0.0, raised: 0.0, hide_weapon: false });
            assert!(shown(&c, 100).arms > 0.4 && !shown(&c, 100).hide_weapon);
            assert_eq!(shown(&c, LOWER_WEAPON_MS).artefact, Some(artefact()));
            assert!(shown(&c, LOWER_WEAPON_MS).hide_weapon && shown(&c, LOWER_WEAPON_MS).raised == 0.0);
            let up = LOWER_WEAPON_MS + SHOW_RAISE_MS;
            for ms in [up, up + SHOW_HOLD_MS - 1] { assert_eq!(shown(&c, ms).raised, 1.0); }
            let back = up + SHOW_HOLD_MS + SHOW_LOWER_MS;
            assert!(shown(&c, back - 1).raised < 0.01 && shown(&c, back - 1).artefact.is_some());
            assert_eq!(shown(&c, back).artefact, None);
            assert!(!shown(&c, back).hide_weapon && shown(&c, back).arms == 1.0);
            assert_eq!(SHOW_MS, 2000);
            // Fire, a weapon key, a quick use, the carry chord: nothing ends
            // it early, and none reaches the game while it runs.
            for (ms, press) in [(250, 0x20), (400, 1 << 12), (600, 1 << 16), (800, QuickSlot(1).command()), (900, 0)] {
                let phase = c.phase();
                input(&mut c, at(ms), press);
                assert_eq!(c.phase(), phase);
                assert_eq!(c.filtered() & BLOCKED, 0);
                assert_eq!(c.offer(), None);
                tick(&mut c, at(ms));
            }
            let mut split = c;
            tick(&mut c, at(SHOW_MS - 1));
            assert!(c.active());
            roundtrip(c);
            tick(&mut c, at(SHOW_MS));
            tick(&mut split, at(SHOW_MS + 5));
            assert_eq!(c, split);
            assert!(!c.active());
            input(&mut c, at(SHOW_MS), 0x20);
            assert_eq!(c.filtered() & 0x20, 0x20);
        }
    }

    #[test]
    fn a_pickup_is_not_shown_over_a_carry_and_ends_with_its_artefact() {
        let mut c = equip(0);
        tick(&mut c, 800);
        let offering = c;
        pick_up(&mut c, 900);
        assert_eq!(c, offering);
        // The artefact gone (its slot lost): the hand goes down empty.
        let mut c = Carry::NEW;
        pick_up(&mut c, 0);
        c.step(Event::Tick { now: Time(700), owned: false });
        assert!(matches!(c.phase(), Phase::LoweringEmptyHand { .. }));
        tick(&mut c, 1500);
        assert!(!c.active());
        // A battery, a grenade in hand or a death: stowed at once.
        let mut c = Carry::NEW;
        pick_up(&mut c, 0);
        c.step(Event::Gone);
        assert!(!c.active());
    }

    /// The input of one tick: the carry key's own event (if any) and the
    /// actions, through the key state and the chord as the input builder runs them.
    fn key_tick(key: &mut CarryKey, chord: &mut Chord, press: Option<KeyPress>, actions: u32) -> u32 {
        *key = key.step(press);
        chord.step(actions, key.is_down())
    }

    #[test]
    fn carry_key_then_quick_key_is_the_chord_as_the_other_order() {
        let four = 1 << (14 + 3);
        let offer = QuickSlot::parse(3).unwrap().command();
        // H down, held with no event of its own, then 4: the chord, never 4's quick use.
        let (mut key, mut chord) = (CarryKey::Up, Chord::Idle);
        assert_eq!(key_tick(&mut key, &mut chord, Some(KeyPress::Pressed), 0), 0);
        assert_eq!(key_tick(&mut key, &mut chord, None, 0), 0);
        assert_eq!(key_tick(&mut key, &mut chord, None, four), offer);
        assert_eq!(key_tick(&mut key, &mut chord, None, four), offer);
        // H up while 4 is still held: the press stays the chord's.
        assert_eq!(key_tick(&mut key, &mut chord, Some(KeyPress::Released), four), offer);
        assert_eq!(key_tick(&mut key, &mut chord, None, 0), 0);
        // 4 alone afterwards is the game's quick use again.
        assert_eq!(key_tick(&mut key, &mut chord, None, four), four);
        // 4 down, then H: the chord too.
        let (mut key, mut chord) = (CarryKey::Up, Chord::Idle);
        assert_eq!(key_tick(&mut key, &mut chord, None, 0), 0);
        assert_eq!(key_tick(&mut key, &mut chord, Some(KeyPress::Pressed), four), offer);
        // A repeat of H keeps it down.
        assert_eq!(CarryKey::Down.step(None), CarryKey::Down);
        assert_eq!(CarryKey::Up.step(None), CarryKey::Up);
    }
}
