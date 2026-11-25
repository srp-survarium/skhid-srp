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
//! and no other jump; the view stays on the jump's way, the way it was pressed along ([jump start]: a decision that fires, the
//! reflex's, may aim within [`super::motor::JUMP_FACING`] of it; the trigger and the aim keys stay theirs). One state, one transition ([`JumpCarry::next`]).

use super::encode::Decision;
use super::jump_arc::Clip;
use super::kin::{TICK_S, wrap, yaw_of};
use super::motor::{JUMP_FACING, RoutePress};
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
    /// The way the jump was pressed along, map (x, z) ([`Press::of_route`]).
    pub way: [f32; 2],
    /// The keys and the ground speed at the press (they pick the clip, [`Clip::of`]).
    pub keys: u32,
    pub speed: f32,
    /// Ticks the jump key is held after this one before the weapon lets the jump go (`Bot::jump_hold`).
    pub held_ticks: u32,
}

impl Press {
    /// [jump start] The route press `route` that went out at `time` with `keys`, the ground speed `speed` and `held_ticks`: its way is
    /// the one the press was flown along ([`RoutePress::way`]), not the take-off-to-landing line. For a press aimed off the link's
    /// line, or along a grid way off that line, they differ (level_06 TDM's (56.35, 28.13, 101.38) -> (53.85, 29.07, 97.88): flown
    /// along (0, -1) turned -0.2 rad, its landing 35 deg off; the carry's cone turned the view 0.22 rad off the flight at the press).
    pub fn of_route(time: u32, route: &RoutePress, keys: u32, speed: f32, held_ticks: u32) -> Self {
        Self { time, way: route.way, keys, speed, held_ticks }
    }
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
        // The view after the turn, kept on the way, or within the facing cone of it for a decision that fires ([`aim_cone`]).
        let off = Self::off(carried, yaw, decision);
        let turn = wrap(carried.way + off - yaw);
        Decision { keys, turn: (turn, decision.turn.1) }
    }

    /// [jump start] How far a decision that fires (the reflex's aim) turns the carried view off the way at `time`, within the cone
    /// ([`aim_cone`]): Some for a turn off it (the log's line), None for a view held on it or no carry.
    pub fn fired_off(&self, time: u32, yaw: f32, decision: Decision) -> Option<f32> {
        let Self::Carried(carried) = *self else {
            return None;
        };
        let off = Self::off(carried, yaw, decision);
        (time < carried.until && decision.keys & key::FIRE != 0 && off.abs() > 1e-3).then_some(off)
    }

    /// The carried view's offset from the way after the decision's turn, clamped to the decision's cone.
    fn off(carried: Carried, yaw: f32, decision: Decision) -> f32 {
        let cone = aim_cone(decision);
        wrap(yaw + decision.turn.0 - carried.way).clamp(-cone, cone)
    }
}

/// [jump start] How far off the carried way a decision may turn the view: the facing cone ([`JUMP_FACING`]) for one that fires (the
/// reflex aims inside it), none for any other. The client's air control follows the view (`get_air_control_vector` 0x2D7990), and
/// the grid flew the press with the heading held through the air: level_06 TDM's (56.35, 28.13, 101.38) -> (53.85, 29.07, 97.88),
/// its view turned 0.2 rad toward the route's next point in the air, came down 0.4 m aside, off the top's edge; held, it lands.
pub fn aim_cone(decision: Decision) -> f32 {
    if decision.keys & key::FIRE != 0 { JUMP_FACING } else { 0.0 }
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
        let small = carry.hold(1_300, way, Decision { keys: key::FORWARD | key::FIRE, turn: (-0.05, 0.0) });
        assert!((small.turn.0 + 0.05).abs() < 1e-5, "a firing turn inside the cone stays: {:?}", small.turn);
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

    /// [jump start] A route press's carry holds the view on the way the press was flown along, not the take-off-to-landing line: the
    /// level_06 TDM press flown along (0, -1) turned -0.2 rad to a landing 35 deg off keeps its view at the press; held to the
    /// landing's line, the cone turned it off the flight.
    #[test]
    fn a_route_press_is_carried_along_the_way_it_was_flown() {
        use super::super::motor::{PressState, RoutePress};
        let (sin, cos) = (-0.2f32).sin_cos();
        let flown = [sin, -cos];
        let route = RoutePress {
            take_off: mapdata::Vec3::new(56.35, 28.13, 101.38),
            landing: mapdata::Vec3::new(53.85, 29.07, 97.88),
            tries: 1,
            before: 0.1,
            facing: 0.0,
            sprint_gate: None,
            aim: super::super::path::PressAim::new(3, -4).expect("an aim"),
            way: flown,
            state: PressState { lead_m: 0.1, flown_lead_m: Some(0.1), side_m: 0.0, turn_rad: 0.0, above_m: 0.0, along_ms: 4.8, window: None },
        };
        let keys = key::FORWARD | key::SPRINT | key::JUMP;
        let p = Press::of_route(1_000, &route, keys, 4.8, 0);
        assert_eq!(p.way, flown, "the flown way, not the landing's line");
        let carry = JumpCarry::Free.next(1_000, Some(p));
        let view = yaw_of(flown);
        let held = carry.hold(1_000, view, Decision { keys, turn: (0.0, 0.0) });
        assert!(held.turn.0.abs() < 1e-5, "the view pressed along the flight stays: {:?}", held.turn);
        let landing_line = Press { way: [route.landing.x - route.take_off.x, route.landing.z - route.take_off.z], ..p };
        let old = JumpCarry::Free.next(1_000, Some(landing_line)).hold(1_000, view, Decision { keys, turn: (0.0, 0.0) });
        assert!(old.turn.0.abs() > 0.15, "held to the landing's line the view turns off the flight: {:?}", old.turn);
    }

    /// [jump start] In the air the carried view stays on the flown way: a turn of a decision that does not fire (the motor steering at
    /// the route's next point) is held to none, and one that fires (the reflex's aim) turns within the facing cone, which
    /// [`JumpCarry::fired_off`] reports for the log.
    #[test]
    fn the_carried_view_holds_the_way_unless_the_decision_fires() {
        let carry = JumpCarry::Free.next(1_000, Some(press(key::FORWARD | key::SPRINT | key::JUMP, 4.9)));
        let way = yaw_of([1.0, 0.0]);
        let steer = Decision { keys: key::FORWARD, turn: (0.15, 0.0) };
        let held = carry.hold(1_300, way, steer);
        assert!(held.turn.0.abs() < 1e-6, "a steering turn is held on the way: {:?}", held.turn);
        assert_eq!(carry.fired_off(1_300, way, steer), None);
        let fire = Decision { keys: key::FORWARD | key::FIRE, turn: (0.15, 0.0) };
        let aimed = carry.hold(1_300, way, fire);
        assert!((aimed.turn.0 - 0.15).abs() < 1e-5, "a firing turn inside the cone stays: {:?}", aimed.turn);
        assert!(carry.fired_off(1_300, way, fire).is_some_and(|off| (off - 0.15).abs() < 1e-5));
        let wide = carry.hold(1_300, way, Decision { keys: key::FORWARD | key::FIRE, turn: (0.5, 0.0) });
        assert!((wide.turn.0 - JUMP_FACING).abs() < 1e-5, "and within the cone only: {:?}", wide.turn);
        assert_eq!(carry.fired_off(5_000, way, fire), None, "after the clip, no carry");
    }
}
