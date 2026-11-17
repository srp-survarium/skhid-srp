//! A player's step at the guard of
//! `player_equipment_sound_effect::on_player_sound_event` (0x2A6470; patch
//! 101, docs/notes/footsteps.md).
//!
//! A step is an animation event of the body's `player_sound_events`
//! channel: domain 0 or 1, one foot or the other. The body animates in the
//! simulation, so a re-simulated tick (a late input of another player, a
//! resync) fires its steps again. The game plays an event only when its
//! time is past the newest time already played once (core_world
//! `maximum_time`), so a step that first exists in a re-simulation (the
//! other player's real input put it there, the prediction did not) is
//! never heard. The site's view: the step's player, foot, time, and whether
//! its time is played for the first time.

use super::context::Context;
use super::types::GameTime;
use super::world::{read, Player};
use crate::raw;

/// A step's foot: the event's domain value 0 or 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Foot {
    Left,
    Right,
}

impl Foot {
    /// The domain value; 2..4 are the rustles, no step.
    fn parse(domain: u8) -> Option<Foot> {
        match domain {
            0 => Some(Foot::Left),
            1 => Some(Foot::Right),
            _ => None,
        }
    }

    pub fn index(self) -> usize {
        match self {
            Foot::Left => 0,
            Foot::Right => 1,
        }
    }
}

/// Simulation milliseconds (the event's time, the core's times).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SimTime(u32);

impl SimTime {
    #[cfg(test)]
    pub const fn at(ms: u32) -> SimTime {
        SimTime(ms)
    }

    /// Milliseconds from `earlier` to `self`, negative when `self` is
    /// earlier (wraps as the game's u32 clock does).
    pub fn since(self, earlier: SimTime) -> i32 {
        self.0.wrapping_sub(earlier.0) as i32
    }
}

/// Whose steps: the sound effect (the held item's +884h, one an item), or a
/// player for a patch's own steps ([`StepSource::player`]), as an identity
/// only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StepSource(u32);

impl StepSource {
    #[cfg(test)]
    pub const fn at(address: u32) -> StepSource {
        StepSource(address)
    }

    /// A player's steps that a patch plays itself (a ladder's rungs): the
    /// player's address, no effect's.
    pub fn player(player: &Player<'_>) -> StepSource {
        StepSource(player.this.address())
    }
}

/// When the step's time is played.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pass {
    /// Past the newest time played: the game plays it.
    First,
    /// At or before it: a re-simulation; the game drops it. `newest` is the
    /// newest time played (core_world `maximum_time`).
    Again { newest: SimTime },
}

/// A step the game would play but for its time: its own player's event,
/// the feet's positions read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    pub source: StepSource,
    pub foot: Foot,
    pub time: SimTime,
    pub pass: Pass,
}

impl Step {
    /// A step a patch plays itself (a ladder's rung: `source` the player's,
    /// `foot` a limb), made in the simulation's tick at `time`, the world
    /// core's newest played time `newest` (its `maximum_time`): played for
    /// the first time past it, by the game's rule (on_player_sound_event's
    /// unsigned compare, 0x2A6488).
    pub fn new(source: StepSource, foot: Foot, time: GameTime, newest: GameTime) -> Step {
        let pass = if time.0 > newest.0 { Pass::First } else { Pass::Again { newest: SimTime(newest.0) } };
        Step { source, foot, time: SimTime(time.0), pass }
    }

    /// The event at `player_sound.step_guard`; None for a rustle, another
    /// object's event or feet not read (the game's own rule decides those).
    pub fn at_guard(ctx: &Context<'_>) -> Option<Step> {
        use raw::site::player_sound_step_guard as site;
        // SAFETY: the context is this site's: the sound effect (ECX of the
        // handler, kept in EAX), the animation's callback parameters (the
        // handler's argument) and the effect's player's world core (tested
        // not 0 at 0x2A6483), all alive for the callback on the simulation's
        // thread.
        unsafe {
            let (effect, event, core) = (site::effect(ctx)?, site::event(ctx)?, site::core(ctx)?);
            let foot = Foot::parse(read(event.domain()))?;
            let player = effect.player()?;
            if read(event.object()) != player.address() || read(effect.feet()) == 0 {
                return None;
            }
            let time = read(event.time());
            let newest = read(core.maximum_time());
            let pass = if time > newest { Pass::First } else { Pass::Again { newest: SimTime(newest) } };
            Some(Step { source: StepSource(effect.address()), foot, time: SimTime(time), pass })
        }
    }
}


/// The step log's view (SKHID_STEP_LOG) of a body sound event as the guard
/// (`player_sound.step_guard`, 0x2A6485, before the time test) sees it,
/// whatever the guard then does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventLine {
    /// The event's domain: 0 and 1 a foot, 2..4 the rustles.
    pub domain: u8,
    pub time: u32,
    /// The core's `maximum_time`.
    pub newest: u32,
    /// The event's object is the effect's player.
    pub ours: bool,
    /// The effect's feet were read (+44h).
    pub feet: bool,
    /// The local player's.
    pub own: bool,
    /// The logic state's movement id now.
    pub movement: Option<u32>,
    /// The effect's address (the held item's +884h): two lines of one event
    /// with two addresses are two effects.
    pub effect: u32,
}

impl EventLine {
    pub fn at_guard(ctx: &Context<'_>) -> Option<EventLine> {
        use raw::site::player_sound_step_guard as site;
        // SAFETY: the site's effect, event and core, alive for the call; the
        // player's world, game and network client alive for a match.
        unsafe {
            let (effect, event, core) = (site::effect(ctx)?, site::event(ctx)?, site::core(ctx)?);
            let player = effect.player()?;
            let local = player.game_world().and_then(|world| world.game()?.network_client()?.local_player());
            Some(EventLine {
                domain: read(event.domain()),
                time: read(event.time()),
                newest: read(core.maximum_time()),
                ours: read(event.object()) == player.address(),
                feet: read(effect.feet()) != 0,
                own: local == Some(player),
                movement: effect.logic().and_then(|logic| logic.state()).map(|state| read(state.movement())),
                effect: effect.address(),
            })
        }
    }
}
