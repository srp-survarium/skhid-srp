//! What a sponge is and its one transition (AGENTS.md, types and states).
//!
//! - [`State::Held`]: an artefact as artefact_base has it (its game state
//!   +280h 0..5: in an anomaly, held, dead); no fuse, no thrower.
//! - [`State::InHand`]: armed at the pin's pull, in its holder's hand (+280h 6).
//! - [`State::InAir`]: armed and thrown (or dropped from the hand): its body
//!   in a physics world (+280h 6), its contacts reported while it explodes
//!   on contact.
//! - [`State::Cloud`]: exploded, its damage zone on (+280h 7).
//! - [`State::Spent`]: its cloud over while a throw state still holds it
//!   (+280h 7): it leaves its slot when the state lets it go.
//!
//! The first contact (retail's rule, [`transition`]) is waiting from the
//! pull, seen by a contact before the tick that rules on it, or spent. From
//! the pull on, a throw state holds it (its +17Ch): while one does it stays
//! in its slot (a world saved then names it by its slot, and its load reads
//! the slot back). The patch lets it go at the first tick its holder's
//! weapon's current state is not that throw state holding it.
//!
//! The layout is C's (`#[repr(C, u32)]`): the patch keeps the state in the
//! sponge object at +41Ch, 28 bytes, where check_sponge_026g.py reads it.

use core::num::NonZeroU32;

use crate::{GameTime, Place, TICK};

/// Who threw it: a place of the match, or nobody (armed with no holder).
#[repr(C, u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Thrower {
    Nobody,
    Place(Place),
}

impl Thrower {
    pub const fn place(self) -> Option<Place> {
        match self {
            Thrower::Nobody => None,
            Thrower::Place(place) => Some(place),
        }
    }
}

/// The first contact since the pull (0.69d0's
/// grenade_core_state_transition_by_contact, its handler 0x2003ECC00).
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Contact {
    /// None yet; the next one counts.
    Waiting,
    /// One came since the last tick, which rules on it.
    Seen,
    /// Ruled on, or the config has no explode_on_contact: the fuse decides.
    Spent,
}

/// Whether a throw state holds it (its +17Ch names it).
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hold {
    Free,
    ThrowState,
}

/// The physics world its body is in (an id: the patch's handle to it).
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Physics(pub NonZeroU32);

/// Armed: in the hand or the air.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Armed {
    /// The fuse's end, on the tick grid.
    pub explode_at: GameTime,
    pub thrower: Thrower,
    pub contact: Contact,
    pub hold: Hold,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Air {
    pub armed: Armed,
    pub physics: Physics,
}

/// Exploded, its zone on from `exploded_at` for the config's duration.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cloud {
    pub exploded_at: GameTime,
    /// Whom the zone's hits name.
    pub thrower: Thrower,
    pub hold: Hold,
}

#[repr(C, u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Held,
    InHand(Armed),
    InAir(Air),
    Cloud(Cloud),
    Spent,
}

const _: () = assert!(core::mem::size_of::<State>() == 28 && core::mem::align_of::<State>() == 4);

impl State {
    /// Armed (in the hand or the air): its fuse, thrower and first contact.
    pub const fn armed(&self) -> Option<&Armed> {
        match self {
            State::InHand(armed) | State::InAir(Air { armed, .. }) => Some(armed),
            State::Held | State::Cloud(_) | State::Spent => None,
        }
    }

    pub const fn thrower(&self) -> Thrower {
        match self {
            State::InHand(Armed { thrower, .. })
            | State::InAir(Air { armed: Armed { thrower, .. }, .. })
            | State::Cloud(Cloud { thrower, .. }) => *thrower,
            State::Held | State::Spent => Thrower::Nobody,
        }
    }

    pub const fn hold(&self) -> Hold {
        match self {
            State::InHand(Armed { hold, .. })
            | State::InAir(Air { armed: Armed { hold, .. }, .. })
            | State::Cloud(Cloud { hold, .. }) => *hold,
            State::Spent => Hold::ThrowState,
            State::Held => Hold::Free,
        }
    }

    /// Ready to be thrown (0.28's grenade_set_base[6] 0x515150): held or in
    /// the hand; not in the air, not exploded.
    pub const fn can_throw(&self) -> bool {
        matches!(self, State::Held | State::InHand(_))
    }
}

/// What the rules need: the config's numbers and the world's tick grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rules {
    /// data/ttl_ms: the fuse from the pin's pull.
    pub ttl_ms: u32,
    /// data/safe_time: a first contact with more than ttl_ms - safe_time of
    /// the fuse left is spent.
    pub safe_time_ms: u32,
    /// active/duration_s x 1000: the cloud's length.
    pub duration_ms: u32,
    /// data/explode_on_contact.
    pub explode_on_contact: bool,
    /// The world's tick grid's origin (+18FBCh).
    pub origin: GameTime,
}

impl Rules {
    /// The fuse's end: ttl_ms after the pull, rounded up to the world's tick
    /// grid (as 0.26g's grenade pull_pin 0x30A120).
    pub const fn fuse_end(&self, pull: GameTime) -> GameTime {
        let due = pull.0.wrapping_add(self.ttl_ms);
        let due = if due > self.origin.0 { due - self.origin.0 - 1 } else { 0 };
        GameTime((due / TICK + 1).wrapping_mul(TICK).wrapping_add(self.origin.0))
    }
}

/// What happens to a sponge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// The pin's pull by its holder (the throw state pulling it holds it).
    Pull { time: GameTime, thrower: Thrower },
    /// Thrown, or dropped from the hand: its body went into `physics`.
    Launch { physics: Physics },
    /// A contact of its body, or a player in its way (the sensor).
    Contact,
    Tick { time: GameTime },
    /// No throw state holds it now.
    LetGo,
}

/// When an exploded sponge leaves its slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Consumed {
    /// Now: no throw state holds it.
    Now,
    /// When its throw state lets it go.
    AtLetGo,
}

/// What a transition asks of the game (its caller does it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    None,
    /// Its game state 6 (armed).
    Arm,
    /// The splash and the zone where it is, out of the physics world, game state 7.
    Explode(Consumed),
    /// Its zone off; an artefact again (its game state 7 back to 0) or spent.
    CloudOff,
    /// Out of its slot.
    Consume,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    pub state: State,
    pub effect: Effect,
}

impl Step {
    const fn same(state: State) -> Step {
        Step { state, effect: Effect::None }
    }
}

/// An armed sponge's tick: retail's first-contact rule (0.69d0's handler
/// 0x2003ECC00: the first contact since the pull with at most ttl_ms -
/// safe_time of the fuse left explodes it now, this tick; earlier it is
/// spent and the fuse decides; 0.46c2's 0x7AD5E0 the same), then the fuse
/// (0.69d0's timeout transition 0x2003EFF60).
fn armed_tick(rules: &Rules, mut armed: Armed, time: GameTime, keep: impl FnOnce(Armed) -> State) -> Step {
    if armed.contact == Contact::Seen {
        armed.contact = Contact::Spent;
        if armed.explode_at.since(time) <= rules.ttl_ms.wrapping_sub(rules.safe_time_ms) as i32 {
            armed.explode_at = time;
        }
    }
    if time == armed.explode_at {
        let consumed = match armed.hold {
            Hold::Free => Consumed::Now,
            Hold::ThrowState => Consumed::AtLetGo,
        };
        let cloud = Cloud { exploded_at: time, thrower: armed.thrower, hold: armed.hold };
        return Step { state: State::Cloud(cloud), effect: Effect::Explode(consumed) };
    }
    Step::same(keep(armed))
}

/// The one transition: (state, event) to the new state and its effect.
pub fn transition(rules: &Rules, state: State, event: Event) -> Step {
    match (state, event) {
        // Armed at the pull (retail's start_timer_on_throw false), the first
        // contact waiting (as grenade_core::pull_pin 0x303B90 sets +156h),
        // the thrower its holder now; the pull is its throw state's.
        (State::Held, Event::Pull { time, thrower }) => {
            let contact = if rules.explode_on_contact { Contact::Waiting } else { Contact::Spent };
            let armed = Armed { explode_at: rules.fuse_end(time), thrower, contact, hold: Hold::ThrowState };
            Step { state: State::InHand(armed), effect: Effect::Arm }
        }
        (State::Held, Event::Launch { .. } | Event::Contact | Event::Tick { .. } | Event::LetGo) => Step::same(state),

        (State::InHand(armed), Event::Launch { physics }) => Step::same(State::InAir(Air { armed, physics })),
        (State::InHand(armed), Event::Tick { time }) => armed_tick(rules, armed, time, State::InHand),
        (State::InHand(armed), Event::LetGo) => Step::same(State::InHand(Armed { hold: Hold::Free, ..armed })),
        // armed once; in the hand its body is in no physics world
        (State::InHand(_), Event::Pull { .. } | Event::Contact) => Step::same(state),

        (State::InAir(mut air), Event::Contact) => {
            if air.armed.contact == Contact::Waiting {
                air.armed.contact = Contact::Seen;
            }
            Step::same(State::InAir(air))
        }
        (State::InAir(air), Event::Tick { time }) => {
            armed_tick(rules, air.armed, time, |armed| State::InAir(Air { armed, ..air }))
        }
        (State::InAir(air), Event::LetGo) => {
            Step::same(State::InAir(Air { armed: Armed { hold: Hold::Free, ..air.armed }, ..air }))
        }
        (State::InAir(_), Event::Pull { .. } | Event::Launch { .. }) => Step::same(state),

        // duration_ms after the explosion the cloud ends: spent until a throw
        // state still holding it lets it go, else an artefact again
        (State::Cloud(cloud), Event::Tick { time }) => {
            if time == cloud.exploded_at.after(rules.duration_ms) {
                let state = match cloud.hold {
                    Hold::Free => State::Held,
                    Hold::ThrowState => State::Spent,
                };
                Step { state, effect: Effect::CloudOff }
            } else {
                Step::same(state)
            }
        }
        (State::Cloud(cloud), Event::LetGo) => match cloud.hold {
            Hold::Free => Step::same(state),
            Hold::ThrowState => Step { state: State::Cloud(Cloud { hold: Hold::Free, ..cloud }), effect: Effect::Consume },
        },
        (State::Cloud(_), Event::Pull { .. } | Event::Launch { .. } | Event::Contact) => Step::same(state),

        (State::Spent, Event::LetGo) => Step { state: State::Held, effect: Effect::Consume },
        (State::Spent, Event::Pull { .. } | Event::Launch { .. } | Event::Contact | Event::Tick { .. }) => {
            Step::same(state)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RULES: Rules =
        Rules { ttl_ms: 3000, safe_time_ms: 1500, duration_ms: 15000, explode_on_contact: true, origin: GameTime(50) };

    fn physics() -> Physics {
        Physics(NonZeroU32::new(0x1000).unwrap())
    }

    fn pulled(at: u32) -> State {
        let thrower = Thrower::Place(Place::new(3).unwrap());
        let step = transition(&RULES, State::Held, Event::Pull { time: GameTime(at), thrower });
        assert_eq!(step.effect, Effect::Arm);
        step.state
    }

    #[test]
    fn layout_is_c() {
        assert_eq!(core::mem::size_of::<Thrower>(), 8);
        assert_eq!(core::mem::size_of::<Armed>(), 20);
        assert_eq!(core::mem::size_of::<Air>(), 24);
        assert_eq!(core::mem::size_of::<Cloud>(), 16);
    }

    #[test]
    fn fuse_on_the_grid() {
        // 100100 + 3000 = 103100, past the origin 50: the grid's next point
        assert_eq!(RULES.fuse_end(GameTime(100100)), GameTime(103100));
        assert_eq!(RULES.fuse_end(GameTime(100101)), GameTime(103150));
        assert_eq!(Rules { origin: GameTime(200000), ..RULES }.fuse_end(GameTime(0)), GameTime(200050));
    }

    #[test]
    fn early_contact_is_spent_late_one_explodes() {
        let hand = pulled(100000);
        let air = transition(&RULES, hand, Event::Launch { physics: physics() }).state;
        // 1000 ms after the pull: 2000 left, more than 1500: spent
        let seen = transition(&RULES, air, Event::Contact).state;
        let step = transition(&RULES, seen, Event::Tick { time: GameTime(101000) });
        assert_eq!(step.effect, Effect::None);
        assert_eq!(step.state.armed().unwrap().contact, Contact::Spent);
        // a second contact is not the first
        let again = transition(&RULES, step.state, Event::Contact).state;
        assert_eq!(again, step.state);
        // the fuse
        let end = transition(&RULES, again, Event::Tick { time: GameTime(103000) });
        assert_eq!(end.effect, Effect::Explode(Consumed::AtLetGo));

        // 1500 ms after the pull: exactly safe: explodes now
        let air = transition(&RULES, pulled(100000), Event::Launch { physics: physics() }).state;
        let seen = transition(&RULES, air, Event::Contact).state;
        let step = transition(&RULES, seen, Event::Tick { time: GameTime(101500) });
        assert_eq!(step.effect, Effect::Explode(Consumed::AtLetGo));
        assert!(matches!(step.state, State::Cloud(Cloud { exploded_at: GameTime(101500), .. })));
    }

    #[test]
    fn cloud_ends_and_let_go_consumes() {
        let air = transition(&RULES, pulled(100000), Event::Launch { physics: physics() }).state;
        let free = transition(&RULES, air, Event::LetGo).state;
        let cloud = transition(&RULES, free, Event::Tick { time: GameTime(103000) });
        assert_eq!(cloud.effect, Effect::Explode(Consumed::Now));
        let off = transition(&RULES, cloud.state, Event::Tick { time: GameTime(118000) });
        assert_eq!(off, Step { state: State::Held, effect: Effect::CloudOff });

        let held = transition(&RULES, pulled(100000), Event::Tick { time: GameTime(103000) });
        assert_eq!(held.effect, Effect::Explode(Consumed::AtLetGo));
        let spent = transition(&RULES, held.state, Event::Tick { time: GameTime(118000) });
        assert_eq!(spent, Step { state: State::Spent, effect: Effect::CloudOff });
        assert_eq!(transition(&RULES, spent.state, Event::LetGo), Step { state: State::Held, effect: Effect::Consume });
        let consumed = transition(&RULES, held.state, Event::LetGo);
        assert_eq!(consumed.effect, Effect::Consume);
        assert_eq!(consumed.state.hold(), Hold::Free);
    }

    #[test]
    fn no_contact_rule_without_the_key() {
        let rules = Rules { explode_on_contact: false, ..RULES };
        let hand = transition(&rules, State::Held, Event::Pull { time: GameTime(0), thrower: Thrower::Nobody }).state;
        assert_eq!(hand.armed().unwrap().contact, Contact::Spent);
    }
}
