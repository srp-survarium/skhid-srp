//! Dead Man's Switch (432, docs/notes/technical-skills.md): a dead player with the skill drops one
//! of its smokes at its body with the throw key, during the respawn countdown. The press travels as
//! a profile input (0xDE, relayed as 0xF3), the one input a dead player's game sends, of a value no
//! kit choice has; every game's place tick plays it from the input and the saved world alone. Pure
//! Rust, for exe patch 180 and srp.

/// The profile input's value. A kit choice is 0..=2, and the place's tick (0x321790) keeps the
/// next kit for any value of 3 or more, so this one changes no kit in any game.
pub const INPUT: u8 = 0x80;

/// How far above the body's feet the smoke is let go, metres (my pick: clear of the ground; it
/// falls and settles with no speed of its own).
pub const DROP_HEIGHT: f32 = 0.15;

/// The kits of a place (a profile's three).
const KITS: u8 = 3;

/// What a profile input asks for, parsed once from its byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileInput {
    /// The kit (0..=2) of the place's next life.
    Kit(u8),
    /// Dead Man's Switch.
    Switch,
    /// Anything else, which no game acts on.
    Other,
}

impl ProfileInput {
    pub fn parse(value: u8) -> Self {
        match value {
            kit if kit < KITS => Self::Kit(kit),
            INPUT => Self::Switch,
            _ => Self::Other,
        }
    }
}

/// The grid of 50 ms the exe divides by whatever the tick's length: pull_pin's fuse (0x30A153:
/// / 50, x 32h) and a profile input's time on the wire (serialize_local_fixed_time 0x3185A0 writes
/// time / 50 alone; the reader 0x318540 makes it a tick x 50).
const GRID: u32 = 50;

/// A profile input's wire tick (time / 50): what every other game gets of its time. This game's own
/// input keeps the ms it was made at (change_input<profile_input> 0x1FB730), the others' are on the
/// grid, so two inputs of a place in one wire tick are one time apart here and the same time there.
pub fn wire_tick(time: u32) -> u32 {
    time / GRID
}

/// The first time on the grid at or after `time`: the time this game gives its switch, so it is
/// the same here and in the others.
pub fn on_grid(time: u32) -> u32 {
    time.div_ceil(GRID).wrapping_mul(GRID)
}

/// A tick of the simulation: its time and the tick's length (core +18FBCh, 50), ms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tick {
    pub now: u32,
    pub length: u32,
}

impl Tick {
    /// Whether a switch of `time` is played at this tick: the tick of its wire tick (the switch is
    /// on the grid, [`on_grid`]; a time off it is taken as the other games get it), so once in a run
    /// of ticks, and again, the same, in a replay of them. The ticks are on the grid (the core's
    /// first is 50, 0x2D0550).
    pub fn takes(self, time: u32) -> bool {
        wire_tick(time).wrapping_mul(GRID) == self.now
    }

    /// When a pin pulled at this tick goes off: grenade_set_core::pull_pin (0x30A134..0x30A16A),
    /// `now + ttl` rounded up to the grid past the tick's length.
    pub fn fuse_end(self, ttl_ms: u16) -> u32 {
        let end = u32::from(ttl_ms).wrapping_add(self.now);
        let past = if end > self.length { end - self.length - 1 } else { 0 };
        (past / GRID + 1).wrapping_mul(GRID).wrapping_add(self.length)
    }

    /// Whether a smoke pulled now goes off before the place's respawn, which takes its kit's
    /// grenades out of the world unexploded: no respawn is due (the place's time is this tick's or
    /// past: a spawn comes only at its very tick, before the places' ticks), or the fuse ends first.
    pub fn before_respawn(self, ttl_ms: u16, respawn: u32) -> bool {
        respawn <= self.now || self.fuse_end(ttl_ms) < respawn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_input_is_no_kit() {
        assert_eq!(ProfileInput::parse(0), ProfileInput::Kit(0));
        assert_eq!(ProfileInput::parse(2), ProfileInput::Kit(2));
        assert_eq!(ProfileInput::parse(3), ProfileInput::Other);
        assert_eq!(ProfileInput::parse(INPUT), ProfileInput::Switch);
        assert_eq!(ProfileInput::parse(0xFF), ProfileInput::Other);
    }

    #[test]
    fn a_switch_is_played_at_its_wire_tick() {
        let tick = |now| Tick { now, length: 50 };
        let played: Vec<u32> = (0..10).map(|n| n * 50).filter(|&now| tick(now).takes(150)).collect();
        assert_eq!(played, [150]);
        assert!(tick(150).takes(199) && !tick(200).takes(199), "off the grid: as the others get it");
        assert_eq!(on_grid(1234), 1250);
        assert_eq!(on_grid(1250), 1250);
        assert_eq!(wire_tick(1249), wire_tick(1200));
        assert!(tick(1250).takes(on_grid(1201)));
    }

    #[test]
    fn the_fuse_ends_as_pull_pin_rounds() {
        let tick = |now| Tick { now, length: 50 };
        assert_eq!(tick(1000).fuse_end(4000), 5000);
        assert_eq!(tick(1010).fuse_end(4000), 5050);
        assert_eq!(tick(0).fuse_end(0), 100, "a tick past the length at the least");
        assert_eq!(tick(1000).fuse_end(4001), 5050);
    }

    #[test]
    fn only_a_smoke_that_goes_off_before_the_respawn() {
        let tick = Tick { now: 10_000, length: 50 };
        assert!(tick.before_respawn(4000, 14_050), "goes off a tick before");
        assert!(!tick.before_respawn(4000, 14_000), "the spawn's tick comes first");
        assert!(!tick.before_respawn(4000, 12_000));
        assert!(tick.before_respawn(4000, 10_000), "no respawn due");
        assert!(tick.before_respawn(4000, 0), "none set");
    }
}
