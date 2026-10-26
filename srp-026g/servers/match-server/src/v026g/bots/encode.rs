//! A bot's decisions as the inputs a client would send (the tick rules of
//! `docs/notes/player_bots.md` §5, as the stage 0 script keeps them in
//! `crate::v026g::drive`):
//! - one input a tick at most, on the 50 ms grid, 50 to 100 ms ahead of
//!   the match clock, never for a tick the clock has passed;
//! - only when the keys change or there is a turn;
//! - a turn is the view's change at that tick, yaw then pitch, as half
//!   floats; what the halves say is what the game turns, so that (and not
//!   what was asked for) is handed back to keep the bot's own yaw exact;
//! - never a bit of `key::NEVER`.

use survarium::v026g::match_msgs::{PlayerInput, TICK_MS, WireTime};

use crate::v026g::drive::{half_bits, half_value, key};

/// How far past the clock's present tick inputs are made.
pub const LEAD_TICKS: u32 = 2;
/// A turn smaller than this is not sent (radians; a half float near 0
/// holds far less, but such a turn moves nothing anyone sees).
const MIN_TURN: f32 = 2e-4;
/// The largest turn one input may carry (radians, under π).
pub const MAX_TURN: f32 = 3.0;

/// What a bot does at one tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Decision {
    pub keys: u32,
    /// Yaw and pitch change at this tick, radians.
    pub turn: (f32, f32),
}

/// One bot's encoder: the ticks it has decided, and its last keys.
#[derive(Clone, Debug, Default)]
pub struct Encoder {
    handled: Option<u32>,
    keys: u32,
}

impl Encoder {
    /// The ticks to decide with the clock at `clock`, in order: those up to
    /// [`LEAD_TICKS`] past the present one that are not decided yet, none
    /// at or behind the clock and none before `from`.
    pub fn ticks(&self, clock: u32, from: u32) -> Vec<u32> {
        let until = (clock / TICK_MS + LEAD_TICKS) * TICK_MS;
        let first = self
            .handled
            .map_or(0, |handled| handled + TICK_MS)
            .max((clock / TICK_MS + 1) * TICK_MS)
            .max(from.div_ceil(TICK_MS) * TICK_MS);
        (first..=until).step_by(TICK_MS as usize).collect()
    }

    /// The input for the decision at the tick `time` (from [`Self::ticks`]),
    /// if one is needed, and the turn as the game will apply it.
    pub fn encode(&mut self, time: u32, decision: Decision) -> (Option<PlayerInput>, (f32, f32)) {
        debug_assert!(self.handled.is_none_or(|handled| time > handled));
        debug_assert_eq!(time % TICK_MS, 0);
        self.handled = Some(time);
        let never = key::NEVER;
        // TEST ONLY: the jump test's bots may kill themselves to respawn.
        #[cfg(feature = "jump-test")]
        let never = never & !key::SUICIDE;
        let keys = decision.keys & !never;
        let clamp = |turn: f32| {
            if turn.abs() < MIN_TURN || !turn.is_finite() {
                0
            } else {
                half_bits(turn.clamp(-MAX_TURN, MAX_TURN))
            }
        };
        let (yaw, pitch) = (clamp(decision.turn.0), clamp(decision.turn.1));
        let turn = (half_value(yaw), half_value(pitch));
        if keys == self.keys && yaw == 0 && pitch == 0 {
            return (None, (0.0, 0.0));
        }
        self.keys = keys;
        let mut rotation = [0; 4];
        rotation[..2].copy_from_slice(&yaw.to_le_bytes());
        rotation[2..].copy_from_slice(&pitch.to_le_bytes());
        let input = PlayerInput {
            time: WireTime::tick(time),
            rotation,
            actions: keys,
        };
        (Some(input), turn)
    }

    /// The keys of the last input sent.
    pub fn keys(&self) -> u32 {
        self.keys
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The jump test's bots may send the suicide (bit 24) and nothing else of the held-back bits; any
    /// other build strips all of them.
    #[test]
    fn only_the_jump_test_lets_the_suicide_through() {
        let mut encoder = Encoder::default();
        let all = Decision {
            keys: key::NEVER | key::FORWARD,
            turn: (0.0, 0.0),
        };
        let (input, _) = encoder.encode(1_050, all);
        let sent = input.map_or(0, |input| input.actions);
        #[cfg(feature = "jump-test")]
        assert_eq!(sent, key::FORWARD | key::SUICIDE);
        #[cfg(not(feature = "jump-test"))]
        assert_eq!(sent, key::FORWARD);
    }

    #[test]
    fn ticks_lead_the_clock_once_each_and_only_changes_are_sent() {
        let mut encoder = Encoder::default();
        assert_eq!(encoder.ticks(1_013, 0), vec![1_050, 1_100]);
        assert_eq!(encoder.ticks(1_013, 1_080), vec![1_100]);
        let walk = Decision {
            keys: key::FORWARD,
            turn: (0.0, 0.0),
        };
        let (input, _) = encoder.encode(1_050, walk);
        assert_eq!(input.map(|input| input.actions), Some(key::FORWARD));
        assert!(encoder.encode(1_100, walk).0.is_none(), "no change");
        assert_eq!(encoder.ticks(1_020, 0), Vec::<u32>::new());
        assert_eq!(encoder.ticks(1_060, 0), vec![1_150]);
        // A stalled clock skips the ticks it passed.
        assert_eq!(encoder.ticks(2_000, 0), vec![2_050, 2_100]);
        // A turn alone is sent, as the halves say it.
        let (input, turn) = encoder.encode(
            2_050,
            Decision {
                keys: key::FORWARD,
                turn: (0.1, -0.02),
            },
        );
        let input = input.unwrap();
        assert!(input.rotation_is_sane());
        assert!((turn.0 - 0.1).abs() < 1e-4 && (turn.1 + 0.02).abs() < 1e-5);
        assert_eq!(
            half_value(u16::from_le_bytes([input.rotation[0], input.rotation[1]])),
            turn.0
        );
        // Never a bit no bot sends (the wheel's, bit 25; the suicide is the jump test's); a tiny turn is none.
        let (input, turn) = encoder.encode(
            2_100,
            Decision {
                keys: key::FORWARD | 0x0200_0000,
                turn: (1e-5, 0.0),
            },
        );
        assert!(input.is_none() && turn == (0.0, 0.0));
    }
}
