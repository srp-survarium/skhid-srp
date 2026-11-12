//! [2d, owner-14 row 13] A route jump pressed is carried through the client's clip, whatever the reckoning says
//! ([`super::motor::Feel::carry_jumps`]).
//!
//! The client (PROVEN, `jump_arc.rs`'s header): a jump goes the way of the keys at the press, the clip moves the body by its
//! root for 24 frames (0.80 s; the sprint jump at time scale 0.909 after its prepare, 0.955 s), and in the air the body follows
//! the keys held after the press and the view's yaw (`get_air_control_vector` 0x2D7990, 1 m/s). A body still at the face when
//! its keys let go of forward, press back or a side key, or its view turns away, is pushed off the face.
//!
//! The bots' fault (owner-14 report row 13, `jumps-rootcause.md` 1c, PROVEN): the reckoning put the body on the landing ~100 ms
//! after the press (`kin.jump_arc` OFF: `body.rs` `jumped_to`), the motor's `jump_ahead` then saw it up and steered at the
//! route's next leg from the reckoned top: in 92 of 118 broken carries the keys left the jump's way at or after that false "up";
//! 0.7-0.95 m presses that kept forward and the way got up 72 % against 44 % for those that did not. The reflex's and the
//! spacing's keys, the landing brake and an unstick press broke the rest.
//!
//! So the carry is keyed on the press (the keys the bot sent), not on any reckoned place, and is applied to the tick's final keys
//! (`drive_tick`): from the press to the clip's end the keys are forward (and the sprint as pressed), no back, side or crouch key
//! and no other jump; the view stays within [`super::motor::JUMP_FACING`] of the jump's way (the reflex may aim inside that
//! cone, the trigger and the aim keys stay theirs). One state, one transition ([`JumpCarry::next`]).

use super::encode::Decision;
use super::jump_arc::Clip;
use super::kin::{TICK_S, wrap, yaw_of};
use super::motor::JUMP_FACING;
use crate::v026g::drive::key;

/// The keys a carry takes off the tick's keys: every way but forward, the crouch, the sprint (put back as it was at the press).
const TAKEN: u32 = key::BACK | key::LEFT | key::RIGHT | key::CROUCH | key::SPRINT;

/// A route jump's carry: free, or carried until the clip's end.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum JumpCarry {
    #[default]
    Free,
    Carried(Carried),
}

/// A jump carried: its way, the keys it holds, until when its press's own jump key goes out and until when the clip lasts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Carried {
    /// The jump's way (yaw of take-off to landing), radians.
    pub way: f32,
    /// Forward, and the sprint if the press had it.
    pub keys: u32,
    /// The press's jump key is held up to this time, ms (the trigger let go first, `drive::JUMP_HOLD_MS`).
    pub press_until: u32,
    /// The clip ends at this time, ms.
    pub until: u32,
}

/// A route jump that went out this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Press {
    pub time: u32,
    /// Take-off to landing, map (x, z).
    pub way: [f32; 2],
    /// The keys and the ground speed at the press (they pick the clip, [`Clip::of`]).
    pub keys: u32,
    pub speed: f32,
    /// Ticks the jump key is held after this one before the weapon lets the jump go (`Bot::jump_hold`).
    pub held_ticks: u32,
}

/// The jump's length from the press to the end of its air phase, ms: its last air step ([`Clip::air_end_ms`]: the take-off frame at
/// the clip's time scale, then the game's twelve air steps, A364 P6), after the sprint jump's prepare; and a step for that last step.
pub fn clip_ms(clip: Clip) -> u32 {
    let prepare = match clip {
        Clip::Sprint(_) => super::jump_arc::PREPARE_MS,
        Clip::Short(_) => 0.0,
    };
    (clip.air_end_ms() + prepare + TICK_S * 1000.0).round() as u32
}

impl JumpCarry {
    /// The carry after a tick at `time`: a route press this tick starts one (a new press replaces an old one); a carry whose clip
    /// is over is free.
    pub fn next(self, time: u32, press: Option<Press>) -> Self {
        match press {
            Some(press) => {
                let tick_ms = (TICK_S * 1000.0).round() as u32;
                let press_until = press.time + press.held_ticks * tick_ms;
                let clip = Clip::of(press.keys, press.speed);
                Self::Carried(Carried {
                    way: yaw_of(press.way),
                    keys: key::FORWARD | (press.keys & key::SPRINT),
                    press_until,
                    until: press_until + clip_ms(clip),
                })
            }
            None => match self {
                Self::Carried(carried) if time < carried.until => self,
                _ => Self::Free,
            },
        }
    }

    /// Whether a carry holds the keys at `time` (no other jump is pressed then).
    pub fn holds(&self, time: u32) -> bool {
        matches!(self, Self::Carried(carried) if time < carried.until)
    }

    /// The tick's decision under the carry, the view's yaw at `yaw`: forward and the way. Free, the decision as it is.
    pub fn hold(&self, time: u32, yaw: f32, decision: Decision) -> Decision {
        let Self::Carried(carried) = *self else {
            return decision;
        };
        if time >= carried.until {
            return decision;
        }
        let jump = if time <= carried.press_until { decision.keys & key::JUMP } else { 0 };
        let keys = (decision.keys & !(TAKEN | key::JUMP)) | carried.keys | jump;
        // The view after the turn, kept within the facing cone of the way.
        let off = wrap(yaw + decision.turn.0 - carried.way).clamp(-JUMP_FACING, JUMP_FACING);
        let turn = wrap(carried.way + off - yaw);
        Decision { keys, turn: (turn, decision.turn.1) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(keys: u32, speed: f32) -> Press {
        Press { time: 1_000, way: [1.0, 0.0], keys, speed, held_ticks: 0 }
    }

    #[test]
    fn the_clips_last_as_the_client_plays_them() {
        // The short jump's twelfth air step at frame 22.5 (750 ms after the press), held through that step.
        assert_eq!(clip_ms(Clip::of(key::FORWARD, 3.0)), 800);
        let sprint = clip_ms(Clip::of(key::FORWARD | key::SPRINT, 4.9));
        assert!((894..=896).contains(&sprint), "{sprint}");
    }

    /// The reckoning's false "up" (the motor steering at the next leg: back and a side key, the view turned away) and the
    /// reflex's side step are taken off from the press to the clip's end; the trigger and the aim stay.
    #[test]
    fn a_pressed_jump_holds_forward_and_its_way_to_the_clips_end() {
        let way = yaw_of([1.0, 0.0]);
        let carry = JumpCarry::Free.next(1_000, Some(press(key::FORWARD | key::JUMP, 3.0)));
        assert!(carry.holds(1_000) && carry.holds(1_799) && !carry.holds(1_800));
        let broken = Decision { keys: key::BACK | key::LEFT | key::CROUCH | key::FIRE | key::AIM | key::JUMP, turn: (1.2, 0.1) };
        let held = carry.hold(1_300, way, broken);
        assert_eq!(held.keys, key::FORWARD | key::FIRE | key::AIM);
        assert!((held.turn.0 - JUMP_FACING).abs() < 1e-5 && held.turn.1 == 0.1, "{:?}", held.turn);
        let small = carry.hold(1_300, way, Decision { keys: key::FORWARD, turn: (-0.05, 0.0) });
        assert!((small.turn.0 + 0.05).abs() < 1e-5, "a turn inside the cone stays: {:?}", small.turn);
        assert_eq!(carry.next(1_850, None), JumpCarry::Free);
        assert_eq!(carry.hold(1_850, way, broken), broken, "after the clip the keys are the walk's");
    }

    /// The press's own jump key goes out while it is held for the weapon; no other jump is pressed in the carry.
    #[test]
    fn only_the_carried_press_jumps() {
        let mut p = press(key::FORWARD | key::JUMP, 3.0);
        p.held_ticks = 5;
        let carry = JumpCarry::Free.next(1_000, Some(p));
        let jump = Decision { keys: key::FORWARD | key::JUMP, turn: (0.0, 0.0) };
        assert_ne!(carry.hold(1_250, 0.0, jump).keys & key::JUMP, 0);
        assert_eq!(carry.hold(1_300, 0.0, jump).keys & key::JUMP, 0);
        assert!(carry.holds(1_250 + 799) && !carry.holds(1_250 + 800));
    }

    /// A sprint press keeps the sprint for its clip; a walking press gets none.
    #[test]
    fn the_sprint_is_held_as_pressed() {
        let sprint = JumpCarry::Free.next(0, Some(Press { time: 0, ..press(key::FORWARD | key::SPRINT | key::JUMP, 4.9) }));
        let walk = JumpCarry::Free.next(0, Some(Press { time: 0, ..press(key::FORWARD | key::JUMP, 3.0) }));
        let d = Decision { keys: key::SPRINT | key::RIGHT, turn: (0.0, 0.0) };
        assert_eq!(sprint.hold(100, 0.0, d).keys, key::FORWARD | key::SPRINT);
        assert_eq!(walk.hold(100, 0.0, d).keys, key::FORWARD);
    }
}
