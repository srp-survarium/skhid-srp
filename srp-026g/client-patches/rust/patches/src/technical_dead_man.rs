//! Patch 180: Dead Man's Switch (432, technical-skills.md). A dead player whose kit has the skill
//! presses the throw key (G) during the respawn countdown: one of its smokes drops at its body with
//! no speed and goes off as any smoke, once a life, while a smoke is left and it would go off before
//! the respawn. The press travels as a profile input of its own value
//! (`technical_skills::dead_mans_switch::INPUT`), the one input a dead player's game sends, and
//! every game's place tick plays it (`vostok::technical_dead_man`).
//!
//! - every game (`technical.place_tick`): a switch the tick takes drops the smoke;
//! - this game: the throw key on the death screen asks for the switch (`technical.game_key`), the
//!   frame's end queues it (`game.frame_end`), at a time on the grid. This game's profile inputs
//!   (the hud's kit picks too, `technical.select_profile`) never share a wire tick: the others get
//!   two such at one time, ordered by their relays, where this game has them by their ms, and the
//!   kit the place's tick takes would differ; a pick made in the switch's wire tick is held and
//!   queued at a frame's end of the next.
#![forbid(unsafe_code)]

use technical_skills::dead_mans_switch::ProfileInput;

use crate::vostok::log::{self, Piece::{Dec, Text}};
use crate::vostok::technical_dead_man::{self as native, FrameEnd, Life, Played, Queued, Refusal};
use crate::vostok::{Context, Flow, GameCell, NoExit, TechnicalDeadManOnSelectProfileExit};

/// Every game's place tick, before its kit choice: a switch of this tick is played.
pub fn on_place_tick(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(played) = native::place_tick(ctx) {
        let (place, now, what) = match played {
            Played::Dropped { place, now } => (place, now, "dropped its smoke"),
            Played::Refused { place, now, why } => (place, now, match why {
                Refusal::NotDead => "refused: not dead in the world",
                Refusal::NoSkill => "refused: no Dead Man's Switch in its kit",
                Refusal::NoSmoke => "refused: no smoke ready",
                Refusal::TooLate => "refused: the smoke would go off after the respawn",
            }),
        };
        log::line(&[Text("dead_mans_switch: place "), Dec(u32::from(place)), Text(" at "), Dec(now), Text(" ms "),
                    Text(what)]);
    }
    Flow::Continue
}

/// This game's switch, one a life of the local player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Switch {
    /// Not asked for in this life.
    Ready,
    /// The throw key asked; a frame's end queues it.
    Pending(Life),
    /// Queued for this life.
    Sent(Life),
}

/// This game's profile inputs: the switch, and a kit pick held out of its wire tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Local {
    switch: Switch,
    held: Option<u8>,
}

/// What happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    /// The throw key went down; `ready` the local player's life when the switch may go.
    Key { ready: Option<Life> },
    /// A profile input is about to be queued; `busy` when it could share the wire tick of the one
    /// before.
    Queue { input: ProfileInput, busy: bool },
    /// The frame's end; `ready` as for the key, `busy` as for a queue now.
    FrameEnd { ready: Option<Life>, busy: bool },
}

/// What to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    None,
    /// Hold the input being queued (select_profile returns at once).
    Hold,
    SendKit(u8),
    SendSwitch,
}

/// The one transition.
fn step(state: Local, event: Event) -> (Local, Action) {
    let Local { switch, held } = state;
    match event {
        Event::Key { ready: None } => (state, Action::None),
        Event::Key { ready: Some(life) } => match switch {
            Switch::Pending(asked) | Switch::Sent(asked) if asked == life => (state, Action::None),
            _ => (Local { switch: Switch::Pending(life), held }, Action::None),
        },
        Event::Queue { input: ProfileInput::Kit(kit), busy: true } => (Local { switch, held: Some(kit) }, Action::Hold),
        Event::Queue { .. } => (state, Action::None),
        Event::FrameEnd { ready, busy } => {
            let switch = match switch {
                // Respawned, or the smoke went: nothing to queue.
                Switch::Pending(asked) if ready != Some(asked) => Switch::Ready,
                other => other,
            };
            match (busy, held, switch) {
                (true, _, _) => (Local { switch, held }, Action::None),
                (false, Some(kit), _) => (Local { switch, held: None }, Action::SendKit(kit)),
                (false, None, Switch::Pending(life)) => (Local { switch: Switch::Sent(life), held }, Action::SendSwitch),
                (false, None, _) => (Local { switch, held }, Action::None),
            }
        }
    }
}

static LOCAL: GameCell<Local> = GameCell::new(Local { switch: Switch::Ready, held: None });

/// The throw key on the game world's handler (before the player's, also on the death screen).
pub fn on_game_key(ctx: &mut Context<'_>) -> Flow<NoExit> {
    // Only the throw key reads the local player.
    if let Some(ready) = native::throw_key(ctx) {
        let thread = ctx.thread();
        LOCAL.set(thread, step(LOCAL.get(thread), Event::Key { ready }).0);
    }
    Flow::Continue
}

/// Every profile input this game queues: one in the wire tick of the one before is held.
pub fn on_select_profile(ctx: &mut Context<'_>) -> Flow<TechnicalDeadManOnSelectProfileExit> {
    let Some(Queued { input, busy }) = Queued::at(ctx) else { return Flow::Continue };
    let thread = ctx.thread();
    let (next, action) = step(LOCAL.get(thread), Event::Queue { input, busy });
    LOCAL.set(thread, next);
    match action {
        Action::Hold => Flow::Exit(TechnicalDeadManOnSelectProfileExit::Held),
        Action::None | Action::SendKit(_) | Action::SendSwitch => Flow::Continue,
    }
}

/// The frame's end, before the game's time moves on: a held pick, else the pending switch, when no
/// profile input of this game has the wire tick yet.
pub fn on_frame_end(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let state = LOCAL.get(thread);
    if state.held.is_none() && !matches!(state.switch, Switch::Pending(_)) {
        return Flow::Continue;
    }
    let Some(frame) = FrameEnd::at(ctx) else { return Flow::Continue };
    let (next, action) = step(state, Event::FrameEnd { ready: frame.ready(), busy: frame.busy() });
    // Set before queueing: the queue passes select_profile's hook, which reads it.
    LOCAL.set(thread, next);
    match action {
        // No match to queue it in: the pick is of no use any more.
        Action::SendKit(kit) => frame.send_kit(kit),
        // Asked again at the next frame's end.
        Action::SendSwitch if !frame.send_switch() => LOCAL.set(thread, state),
        Action::SendSwitch | Action::None | Action::Hold => {}
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIFE: Life = Life { place: 3, kit: 1, spawned: 40_000 };
    const NEXT: Life = Life { place: 3, kit: 1, spawned: 70_000 };
    const IDLE: Local = Local { switch: Switch::Ready, held: None };

    fn key(ready: Option<Life>) -> Event {
        Event::Key { ready }
    }
    fn end(ready: Option<Life>, busy: bool) -> Event {
        Event::FrameEnd { ready, busy }
    }
    fn pick(kit: u8, busy: bool) -> Event {
        Event::Queue { input: ProfileInput::Kit(kit), busy }
    }

    #[test]
    fn one_switch_a_life() {
        let (state, action) = step(IDLE, key(Some(LIFE)));
        assert_eq!((state.switch, action), (Switch::Pending(LIFE), Action::None));
        let (state, action) = step(state, end(Some(LIFE), false));
        assert_eq!((state.switch, action), (Switch::Sent(LIFE), Action::SendSwitch));
        // Its own queue passes.
        assert_eq!(step(state, Event::Queue { input: ProfileInput::Switch, busy: false }), (state, Action::None));
        // Pressed again in the same life: nothing.
        let (state, action) = step(state, key(Some(LIFE)));
        assert_eq!((state.switch, action), (Switch::Sent(LIFE), Action::None));
        assert_eq!(step(state, end(Some(LIFE), false)).1, Action::None);
        // The next life's death: again.
        let (state, _) = step(state, key(Some(NEXT)));
        assert_eq!(step(state, end(Some(NEXT), false)).1, Action::SendSwitch);
    }

    #[test]
    fn a_pick_and_the_switch_never_share_a_wire_tick() {
        // A pick in the switch's wire tick is held, queued at the next free frame's end.
        let (state, _) = step(IDLE, key(Some(LIFE)));
        let (state, _) = step(state, end(Some(LIFE), false));
        let (state, action) = step(state, pick(2, true));
        assert_eq!((state.held, action), (Some(2), Action::Hold));
        let (state, action) = step(state, pick(0, true));
        assert_eq!((state.held, action), (Some(0), Action::Hold), "the newest pick wins");
        assert_eq!(step(state, end(None, true)).1, Action::None, "still the switch's wire tick");
        let (state, action) = step(state, end(None, false));
        assert_eq!((state.held, action), (None, Action::SendKit(0)));
        assert_eq!(state.switch, Switch::Sent(LIFE));
        // A pick before the switch: the switch waits for the next wire tick.
        let (state, _) = step(IDLE, key(Some(LIFE)));
        assert_eq!(step(state, pick(1, false)), (state, Action::None));
        assert_eq!(step(state, end(Some(LIFE), true)), (state, Action::None));
        assert_eq!(step(state, end(Some(LIFE), false)).1, Action::SendSwitch);
    }

    #[test]
    fn nothing_when_it_may_not_go() {
        assert_eq!(step(IDLE, key(None)), (IDLE, Action::None));
        let (state, _) = step(IDLE, key(Some(LIFE)));
        // Respawned (or the smoke went) before a frame's end.
        assert_eq!(step(state, end(None, false)), (IDLE, Action::None));
        assert_eq!(step(state, end(Some(NEXT), true)), (IDLE, Action::None));
        assert_eq!(step(IDLE, end(Some(LIFE), false)), (IDLE, Action::None));
        // Picks with nothing in their wire tick go as they came.
        assert_eq!(step(IDLE, pick(2, false)), (IDLE, Action::None));
    }
}
