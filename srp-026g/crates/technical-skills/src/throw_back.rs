//! Return to Sender (422, docs/notes/technical-skills.md): a player with the skill throws a live
//! enemy grenade near it back along its view, the fuse running on. The command an input carries,
//! the server's check of it and the client's twin, and the rule of which grenade it takes: pure
//! Rust, shared by exe patch 180 and the match server.
//!
//! - the command is patch 85's pickup input (Use's press, `weapon_pickup`): its marker bit 29 with
//!   all six quick bits ([`command`], [`encode`]), a pattern neither patch 85's pickups (the first
//!   20 patterns of two or more bits, 3..=25) nor patch 78's carry commands (one bit) use;
//! - the server relays it only from a player one of whose kits has the skill ([`sane`]); the
//!   client sends it only then, and every game plays it only for a player whose current kit has it;
//! - the grenade: the nearest live one within [`REACH`] of the player's eye, in sight, whose
//!   stack's holder is an enemy and whose fuse has more than [`MIN_FUSE_MS`] left ([`Grenade`],
//!   [`Nearest`]).

use weapon_pickup::{CHORD, QUICK, USE};

use crate::{Clock, Skills};

/// The quick bits' pattern of the command: all six.
const PATTERN: u32 = 0b11_1111;

/// Whether an input's actions carry a throw-back: the marker and the pattern.
pub fn command(actions: u32) -> bool {
    actions & CHORD != 0 && (actions & QUICK) >> 14 == PATTERN
}

/// The input builder's mask asking for a throw-back: the marker and the pattern; Use and the quick
/// bits of the press are consumed (as patch 85's pickup).
pub fn encode(actions: u32) -> u32 {
    (actions & !(QUICK | USE)) | CHORD | PATTERN << 14
}

/// Whether the server relays an input with these actions from a player whose kits' modifier 47
/// values are `kits`: no throw-back, or one from a player with Return to Sender in a kit. The
/// client checks the same before it sends ([`encode`] only for a current kit with the skill).
pub fn sane(actions: u32, kits: impl IntoIterator<Item = f32>) -> bool {
    !command(actions) || kits.into_iter().any(|value| Skills::parse(value).is_some_and(Skills::return_to_sender))
}

/// How far from the thrower's eye a grenade can be taken, metres (my pick).
pub const REACH: f32 = 2.5;

/// The fuse a grenade must have left, ms: two of the fuse's 50 ms steps, so it never explodes in
/// the tick it is thrown back in, whichever of the input and the grenade's tick runs first.
pub const MIN_FUSE_MS: u32 = 100;

/// One grenade object of a stack, as its saved fields say (grenade_core +148h in the physics world,
/// +14Ch the explode time, -1 not armed, +154h spent).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grenade {
    /// Not armed, in no world: the next units to pull.
    Unused,
    /// Its pin pulled, in a hand (its transform the holder's hand).
    Held { explodes: Clock },
    /// Thrown: flying or on the ground, exploding at `explodes`.
    Thrown { explodes: Clock },
    /// It exploded (out of the world, disarmed).
    Spent,
}

impl Grenade {
    /// The grenade of these fields; none for a combination the game never makes (in the world and
    /// not armed, or spent and still there).
    pub fn parse(in_world: bool, explode_time: u32, spent: bool) -> Option<Self> {
        let armed = (explode_time != u32::MAX).then_some(Clock(explode_time));
        match (spent, in_world, armed) {
            (true, false, None) => Some(Self::Spent),
            (false, false, None) => Some(Self::Unused),
            (false, false, Some(explodes)) => Some(Self::Held { explodes }),
            (false, true, Some(explodes)) => Some(Self::Thrown { explodes }),
            _ => None,
        }
    }

    /// Whether it can be thrown back at `now`: thrown, with more than [`MIN_FUSE_MS`] to go.
    pub fn returnable(self, now: Clock) -> bool {
        match self {
            Self::Thrown { explodes } => !now.after(MIN_FUSE_MS).reached(explodes),
            Self::Unused | Self::Held { .. } | Self::Spent => false,
        }
    }
}

/// The squared distance from `eye` to `at`, when within [`REACH`].
pub fn within_reach(eye: [f32; 3], at: [f32; 3]) -> Option<f32> {
    let d = [at[0] - eye[0], at[1] - eye[1], at[2] - eye[2]];
    let squared = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
    (squared <= REACH * REACH).then_some(squared)
}

/// The nearest of the candidates offered in the game's order (places, then slots, then the
/// stack's objects): a strictly nearer one replaces the kept one, so a tie keeps the first, the
/// same in every game.
#[derive(Clone, Copy, Debug)]
pub struct Nearest<T> {
    best: Option<(f32, T)>,
}

impl<T: Copy> Nearest<T> {
    pub const fn new() -> Self {
        Self { best: None }
    }

    /// The candidate `item` at `squared` distance.
    pub fn offer(&mut self, squared: f32, item: T) {
        if self.best.is_none_or(|(kept, _)| squared < kept) {
            self.best = Some((squared, item));
        }
    }

    /// Whether `squared` would replace the kept one (a farther one needs no sight test).
    pub fn nearer(&self, squared: f32) -> bool {
        self.best.is_none_or(|(kept, _)| squared < kept)
    }

    pub fn get(self) -> Option<T> {
        self.best.map(|(_, item)| item)
    }
}

impl<T: Copy> Default for Nearest<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_is_no_pickup_and_no_carry() {
        let sent = encode(USE | 1 << 14 | 0x21);
        assert!(command(sent));
        assert_eq!(sent & USE, 0, "Use consumed");
        assert_eq!(sent & 0x21, 0x21, "the other actions kept");
        assert_eq!(weapon_pickup::command(sent), None, "not one of patch 85's pickups");
        assert!(weapon_pickup::sane(sent, 1), "patch 85's check passes it");
        for p in 0..weapon_pickup::PLACES {
            let pickup = weapon_pickup::encode(0, weapon_pickup::Place::new(p).unwrap());
            assert!(!command(pickup), "place {p}'s pickup");
        }
        for bit in 0..6 {
            assert!(!command(CHORD | 1 << (14 + bit)), "a carry command");
        }
        assert!(!command(QUICK), "no marker");
    }

    #[test]
    fn the_server_and_the_client_check_alike() {
        let skill = crate::weight(8) as f32;
        let sent = encode(0);
        assert!(sane(sent, [0.0, skill]), "a kit has it");
        assert!(!sane(sent, [0.0, 0.0, 0.0]));
        assert!(!sane(sent, []));
        assert!(!sane(sent, [f32::NAN]), "no skills at all");
        assert!(sane(0x21, []), "no throw-back");
    }

    #[test]
    fn a_grenade_is_parsed_once() {
        assert_eq!(Grenade::parse(false, u32::MAX, false), Some(Grenade::Unused));
        assert_eq!(Grenade::parse(false, 500, false), Some(Grenade::Held { explodes: Clock(500) }));
        assert_eq!(Grenade::parse(true, 500, false), Some(Grenade::Thrown { explodes: Clock(500) }));
        assert_eq!(Grenade::parse(false, u32::MAX, true), Some(Grenade::Spent));
        assert_eq!(Grenade::parse(true, u32::MAX, false), None);
        assert_eq!(Grenade::parse(true, 500, true), None);
    }

    #[test]
    fn the_fuse_must_have_two_steps_left() {
        for now in [0u32, 10_000, u32::MAX - 40] {
            let thrown = |left: u32| Grenade::Thrown { explodes: Clock(now.wrapping_add(left)) };
            assert!(thrown(101).returnable(Clock(now)));
            assert!(!thrown(100).returnable(Clock(now)));
            assert!(!thrown(0).returnable(Clock(now)));
            assert!(!Grenade::Thrown { explodes: Clock(now.wrapping_sub(50)) }.returnable(Clock(now)), "past");
            assert!(!Grenade::Held { explodes: Clock(now.wrapping_add(3_000)) }.returnable(Clock(now)));
        }
    }

    #[test]
    fn the_nearest_in_reach_and_the_first_of_a_tie() {
        let eye = [0.0, 1.6, 0.0];
        assert_eq!(within_reach(eye, [0.0, 1.6, 2.5]), Some(6.25));
        assert_eq!(within_reach(eye, [0.0, 1.6, 2.51]), None);
        let mut nearest = Nearest::new();
        for (n, at) in [[2.0, 1.6, 0.0], [0.0, 0.1, 0.0], [1.5, 1.6, 0.0], [-1.5, 1.6, 0.0]].into_iter().enumerate() {
            if let Some(squared) = within_reach(eye, at) {
                nearest.offer(squared, n);
            }
        }
        assert_eq!(nearest.get(), Some(1), "1.5 m, the first of the three");
        assert_eq!(Nearest::<u8>::new().get(), None);
    }
}
