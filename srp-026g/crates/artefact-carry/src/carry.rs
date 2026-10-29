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
pub const COMMAND_MASK: u32 = 7 << 29;
pub const USE: u32 = 1 << 21;
pub const QUICK: u32 = 63 << 14;
/// Fire, reload, aim, sprint, weapon selection, quick use, breath, drop,
/// grenade and melee. Looking, movement, crouch, jump and suicide survive.
pub const BLOCKED: u32 = 0xe0 | (1 << 9) | (3 << 12) | QUICK | (1 << 20)
    | (3 << 22) | (3 << 25) | (1 << 27);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuickSlot(u8);
impl QuickSlot {
    pub const fn parse(n: u8) -> Option<Self> { if n < 6 { Some(Self(n)) } else { None } }
    pub const fn get(self) -> u8 { self.0 }
    pub const fn command(self) -> u32 { (self.0 as u32 + 1) << 29 }
    pub const fn from_actions(actions: u32) -> Option<Self> {
        let n = actions >> 29;
        if n >= 1 && n <= 6 { Some(Self(n as u8 - 1)) } else { None }
    }
}

/// Input builder conversion. One selected quick slot only; ambiguous chords
/// are consumed without offering or activating an artefact.
pub fn encode_chord(actions: u32, modifier_down: bool) -> u32 {
    let quick = (actions & QUICK) >> 14;
    let clean = actions & !COMMAND_MASK;
    if !modifier_down || quick == 0 { return clean; }
    let clean = clean & !QUICK;
    if quick.count_ones() != 1 { return clean; }
    clean | QuickSlot(quick.trailing_zeros() as u8).command()
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
                return (actions & !(COMMAND_MASK | (u32::from(held) << 14)))
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
    LoweringWeapon { token: Token, start: Time },
    Raising { token: Token, start: Time },
    Offering(Token),
    LoweringHand { token: Token, start: Time, from: u16 },
    LoweringEmptyHand { start: Time, from: u16 },
    RestoringWeapon { start: Time, from: u16 },
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

impl Default for Carry { fn default() -> Self { Self::NEW } }
impl Carry {
    pub const NEW: Self = Self { phase: Phase::Stowed, generation: 0, last_raw: 0, last_passed: 0 };
    pub const fn phase(self) -> Phase { self.phase }
    pub const fn active(self) -> bool { !matches!(self.phase, Phase::Stowed) }
    pub const fn artefact(self) -> Option<Artefact> {
        match self.phase {
            Phase::LoweringWeapon { token, .. } | Phase::Raising { token, .. }
            | Phase::Offering(token) | Phase::LoweringHand { token, .. } => Some(token.artefact),
            Phase::Switching { current: Some(token), .. } => Some(token.artefact),
            _ => None,
        }
    }
    pub const fn pending(self) -> Option<Artefact> {
        if let Phase::Switching { next, .. } = self.phase { Some(next.artefact) } else { None }
    }
    pub const fn presentation_token(self) -> Option<Token> {
        match self.phase {
            Phase::Raising { token, .. } | Phase::Offering(token) | Phase::LoweringHand { token, .. } => Some(token),
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
            Event::Tick { now, owned } => {
                if self.artefact().is_some() && !owned {
                    self.phase = match self.phase {
                        Phase::Switching { next, start, from, .. } => Phase::Switching { current: None, next, start, from },
                        _ => Phase::LoweringEmptyHand { start: now, from: (self.presentation(now).raised * 65535.0) as u16 },
                    };
                }
                // A late tick can cross multiple boundaries. Times are absolute
                // and wrapping, so the result does not depend on tick splitting.
                for _ in 0..4 {
                    self.phase = match self.phase {
                        Phase::Switching { next, start, .. } if now.elapsed(start) >= LOWER_HAND_MS =>
                            Phase::Raising { token: next, start: Time(start.0.wrapping_add(LOWER_HAND_MS)) },
                        Phase::LoweringWeapon { token, start } if now.elapsed(start) >= LOWER_WEAPON_MS =>
                            Phase::Raising { token, start: Time(start.0.wrapping_add(LOWER_WEAPON_MS)) },
                        Phase::Raising { token, start } if now.elapsed(start) >= RAISE_MS => Phase::Offering(token),
                        Phase::LoweringHand { start, .. } | Phase::LoweringEmptyHand { start, .. } if now.elapsed(start) >= LOWER_HAND_MS =>
                            Phase::RestoringWeapon { start: Time(start.0.wrapping_add(LOWER_HAND_MS)), from: u16::MAX },
                        Phase::RestoringWeapon { start, .. } if now.elapsed(start) >= RESTORE_MS => Phase::Stowed,
                        other => other,
                    };
                }
            }
            Event::Input { now, actions: raw, artefact } => {
                let command = QuickSlot::from_actions(raw);
                if command.is_some() && command != QuickSlot::from_actions(self.last_raw) {
                    if let Some(artefact) = artefact {
                        if self.pending().or(self.artefact()) == Some(artefact) {
                            self.cancel(now);
                        } else {
                            self.generation = self.generation.wrapping_add(1);
                            let token = Token { artefact, generation: self.generation };
                            self.phase = match self.phase {
                                Phase::LoweringWeapon { start, .. } => Phase::LoweringWeapon { token, start },
                                Phase::Raising { token: old, .. } | Phase::Offering(old) => Phase::Switching {
                                    current: Some(old), next: token, start: now, from: (self.presentation(now).raised * 65535.0) as u16,
                                },
                                Phase::LoweringHand { token: old, start, from } => Phase::Switching { current: Some(old), next: token, start, from },
                                Phase::LoweringEmptyHand { start, from } => Phase::Switching { current: None, next: token, start, from },
                                Phase::Switching { current, start, from, .. } => Phase::Switching { current, next: token, start, from },
                                Phase::RestoringWeapon { .. } | Phase::Stowed => Phase::LoweringWeapon { token, start: now },
                            };
                        }
                    }
                } else if self.active() && raw & (0xe0 | (3 << 12) | (3 << 25) | (1 << 23)) != 0 {
                    self.cancel(now);
                }
                let consumed_use = self.last_raw & USE != 0 && self.last_passed & USE == 0;
                self.last_raw = raw;
                self.last_passed = raw & !COMMAND_MASK;
                if self.active() { self.last_passed &= !BLOCKED; }
                if command.is_some() { self.last_passed &= !QUICK; }
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
            Phase::LoweringWeapon { .. } => Phase::RestoringWeapon { start: now, from: (self.presentation(now).arms * 65535.0) as u16 },
            Phase::Raising { token, .. } | Phase::Offering(token) => Phase::LoweringHand {
                token, start: now, from: (self.presentation(now).raised * 65535.0) as u16,
            },
            other => other,
        };
    }

    pub fn presentation(self, now: Time) -> Presentation {
        let blend = |start, duration| {
            let t = (now.elapsed(start) as f32 / duration as f32).min(1.0);
            t * t * (3.0 - 2.0 * t)
        };
        match self.phase {
            Phase::Switching { current, start, from, .. } => Presentation {
                artefact: current.map(|t| t.artefact), arms: 1.0,
                raised: f32::from(from) / 65535.0 * (1.0 - blend(start, LOWER_HAND_MS)), hide_weapon: true,
            },
            Phase::Stowed => Presentation { artefact: None, arms: 0.0, raised: 0.0, hide_weapon: false },
            Phase::LoweringWeapon { start, .. } => Presentation { artefact: None, arms: blend(start, LOWER_WEAPON_MS), raised: 0.0, hide_weapon: false },
            Phase::Raising { token, start } => Presentation { artefact: Some(token.artefact), arms: 1.0, raised: blend(start, RAISE_MS), hide_weapon: true },
            Phase::Offering(token) => Presentation { artefact: Some(token.artefact), arms: 1.0, raised: 1.0, hide_weapon: true },
            Phase::LoweringHand { token, start, from } => Presentation { artefact: Some(token.artefact), arms: 1.0, raised: f32::from(from) / 65535.0 * (1.0 - blend(start, LOWER_HAND_MS)), hide_weapon: true },
            Phase::LoweringEmptyHand { start, from } => Presentation { artefact: None, arms: 1.0, raised: f32::from(from) / 65535.0 * (1.0 - blend(start, LOWER_HAND_MS)), hide_weapon: true },
            Phase::RestoringWeapon { start, from } => Presentation { artefact: None, arms: f32::from(from) / 65535.0 * (1.0 - blend(start, RESTORE_MS)), raised: 0.0, hide_weapon: false },
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
            Phase::LoweringWeapon { token, start } => (1, Some(token), start, 0),
            Phase::Raising { token, start } => (2, Some(token), start, 0),
            Phase::Offering(token) => (3, Some(token), Time(0), 0),
            Phase::LoweringHand { token, start, from } => (4, Some(token), start, from),
            Phase::RestoringWeapon { start, from } => (5, None, start, from),
            Phase::LoweringEmptyHand { start, from } => (6, None, start, from),
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
        let token = || Some(Token { artefact: Artefact {
            id: ArtefactId::parse(bytes[14])?, slot: ContainerIndex::parse(bytes[15])?,
        }, generation: word(16) });
        let phase = match bytes[13] {
            7 | 8 => Phase::Switching {
                current: if bytes[13] == 7 { Some(token()?) } else { None },
                next: Token { artefact: Artefact { id: ArtefactId::parse(bytes[26])?, slot: ContainerIndex::parse(bytes[27])? }, generation: word(28) },
                start, from: u16::from_le_bytes([bytes[24], bytes[25]]),
            },
            0 => Phase::Stowed,
            1 => Phase::LoweringWeapon { token: token()?, start },
            2 => Phase::Raising { token: token()?, start },
            3 => Phase::Offering(token()?),
            4 => Phase::LoweringHand { token: token()?, start, from: u16::from_le_bytes([bytes[24], bytes[25]]) },
            5 => Phase::RestoringWeapon { start, from: u16::from_le_bytes([bytes[24], bytes[25]]) },
            6 => Phase::LoweringEmptyHand { start, from: u16::from_le_bytes([bytes[24], bytes[25]]) },
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
            assert_eq!(encoded & QUICK, 0);
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
}
