//! Rollback pacing (stage (a), client patch 74; `sync_core.md` sections 3.3,
//! 7 (a) and 9).
//!
//! A late input or roster change puts the world core's rollback point
//! (+18FC8h) before the world's time; the stock `move` (0x2D2E40) then
//! loads the history item at that point and steps again to the target, on
//! the very next call. On a busy match that is nearly every frame.
//!
//! For the render move only (game_world::tick's call, a target past the
//! world's time), a rollback deeper than the grid item every render move
//! loads anyway, due within [`MIN_GAP_MS`] of game time of the last such
//! one, is postponed: the world steps on from its newest item and the
//! rollback point is kept. It is never postponed once the target is
//! [`DEADLINE_MS`] past the point, so the rollback always comes before a
//! stale item can reach the event horizon (the one item the outside sees:
//! 0xDF hashes it, 0xF6 uploads it). Every grid item is still stepped from
//! a grid item through every change node, so the worlds are the stock
//! ones; only the frames in between show the prediction a little longer.
//!
//! The world comes in parsed ([`World`]): the client's glue reads the three
//! raw fields once and is the only code that knows their encodings.

use crate::{EVENT_HORIZON_MS, GameTime, TICK_MS};

/// The least game time between two rollbacks of render moves.
pub const MIN_GAP_MS: u32 = 50;

/// A rollback point this far behind the target is never postponed.
pub const DEADLINE_MS: u32 = 300;

// A postponed rollback must happen before the first stale item (the one
// after the point) is the event-horizon item: target - point < horizon - one
// tick, with a frame's worth to spare.
const _: () = assert!(DEADLINE_MS + 2 * TICK_MS <= EVENT_HORIZON_MS);

/// A world core, by its address: a new match has a new core.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CoreId(pub u32);

/// What the world core says before a render move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum World {
    /// It waits for its first world (+18FDFh): move does nothing.
    Frozen { core: CoreId },
    /// It runs: its time (+18FC0h), and the rollback point (+18FC8h) when an
    /// input landed before the newest history item.
    Running {
        core: CoreId,
        now: GameTime,
        rollback: Option<GameTime>,
    },
}

/// Whether a move to a target has a rollback due: the stock move's own
/// test (0x2D2E62..0x2D2E91).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// The world agrees with every known input up to its time.
    Current,
    /// An input landed before the world's time: the world must be loaded
    /// from the item at or before `to` and stepped again.
    Behind { to: GameTime },
}

impl Step {
    /// The step of a running world at `now` with `rollback`, moved to `target`.
    pub fn of(now: GameTime, rollback: Option<GameTime>, target: GameTime) -> Step {
        match rollback {
            Some(to) if to < now && to < target => Step::Behind { to },
            _ => Step::Current,
        }
    }
}

/// The pacing machine: what it remembers between render moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pacing {
    /// No rollback of a render move yet (or of this core).
    Free,
    /// The last rollback of a render move of `core` was for the target `at`.
    Paced { core: CoreId, at: GameTime },
}

/// What to do with one render move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plan {
    /// The stock move, as it is.
    Stock,
    /// The stock move with the rollback point hidden (no rollback: the world
    /// steps on from its newest item), then the point put back as
    /// [`restored_point`] of `keep` and what the move left there.
    Postpone { keep: GameTime },
}

/// The transition: a render move to `target` in `pacing`, the world as
/// `world` says. Total: every (state, world, target) has its answer.
pub fn render_move(pacing: Pacing, world: World, target: GameTime) -> (Pacing, Plan) {
    let (core, now, rollback) = match world {
        // move does nothing in a frozen world: no rollback happens.
        World::Frozen { .. } => return (pacing, Plan::Stock),
        World::Running {
            core,
            now,
            rollback,
        } => (core, now, rollback),
    };
    let to = match Step::of(now, rollback, target) {
        Step::Current => return (pacing, Plan::Stock),
        Step::Behind { to } => to,
    };
    // A shallow rollback loads the item a render move loads anyway (the
    // grid point at or before the world's time): it costs nothing more, so
    // it is stock and does not count as one.
    if to >= now.grid_floor() {
        return (pacing, Plan::Stock);
    }
    let rolled = Pacing::Paced { core, at: target };
    // Not a render advance (a target not past the world's time): stock,
    // which rolls back.
    if target <= now {
        return (rolled, Plan::Stock);
    }
    let recent = match pacing {
        Pacing::Paced { core: last, at } => last == core && target.since(at) < MIN_GAP_MS,
        Pacing::Free => false,
    };
    let due = target.since(to) >= DEADLINE_MS;
    if recent && !due {
        (pacing, Plan::Postpone { keep: to })
    } else {
        (rolled, Plan::Stock)
    }
}

/// The rollback point after a postponed move: the earliest of the kept one
/// and whatever the move lowered it to (its own local input snapping).
pub fn restored_point(keep: GameTime, after: Option<GameTime>) -> GameTime {
    after.map_or(keep, |after| keep.earliest(after))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CORE: CoreId = CoreId(0x1000);

    fn t(ms: u32) -> GameTime {
        GameTime(ms)
    }

    fn world(now: u32, rollback: Option<u32>) -> World {
        World::Running {
            core: CORE,
            now: t(now),
            rollback: rollback.map(t),
        }
    }

    fn paced(at: u32) -> Pacing {
        Pacing::Paced {
            core: CORE,
            at: t(at),
        }
    }

    #[test]
    fn a_world_with_no_rollback_due_moves_as_stock_and_keeps_the_state() {
        for w in [
            world(1000, None),
            world(1000, Some(1000)),
            world(1000, Some(1050)),
            world(1000, Some(1100)),
        ] {
            assert_eq!(
                render_move(paced(990), w, t(1010)),
                (paced(990), Plan::Stock),
                "{w:?}"
            );
        }
    }

    #[test]
    fn the_first_rollback_is_stock_and_starts_the_pacing() {
        assert_eq!(
            render_move(Pacing::Free, world(1000, Some(850)), t(1010)),
            (paced(1010), Plan::Stock)
        );
    }

    #[test]
    fn a_shallow_rollback_is_stock_and_does_not_count() {
        // the point is the grid item a render move loads anyway
        for pacing in [Pacing::Free, paced(1025)] {
            assert_eq!(
                render_move(pacing, world(1030, Some(1000)), t(1040)),
                (pacing, Plan::Stock)
            );
        }
        // one tick deeper is a rollback
        assert_eq!(
            render_move(paced(1025), world(1030, Some(950)), t(1040)),
            (paced(1025), Plan::Postpone { keep: t(950) })
        );
    }

    #[test]
    fn a_rollback_within_50_ms_of_the_last_is_postponed() {
        let postponed = (paced(1010), Plan::Postpone { keep: t(900) });
        assert_eq!(
            render_move(paced(1010), world(1010, Some(900)), t(1020)),
            postponed
        );
        assert_eq!(
            render_move(paced(1010), world(1050, Some(900)), t(1059)),
            postponed
        );
        assert_eq!(
            render_move(paced(1010), world(1059, Some(900)), t(1060)),
            (paced(1060), Plan::Stock)
        );
    }

    #[test]
    fn a_point_300_ms_behind_the_target_is_never_postponed() {
        assert_eq!(
            render_move(paced(1010), world(1010, Some(750)), t(1049)).1,
            Plan::Postpone { keep: t(750) }
        );
        assert_eq!(
            render_move(paced(1010), world(1010, Some(750)), t(1050)).1,
            Plan::Stock
        );
        assert_eq!(
            render_move(paced(1010), world(1010, Some(700)), t(1020)).1,
            Plan::Stock
        );
    }

    #[test]
    fn moves_that_are_no_render_advance_are_stock() {
        // the target at or before the world's time (the horizon move, a camera)
        assert_eq!(
            render_move(paced(1010), world(1020, Some(900)), t(1020)).1,
            Plan::Stock
        );
        assert_eq!(
            render_move(paced(1010), world(1020, Some(900)), t(950)).1,
            Plan::Stock
        );
        // a frozen world: nothing happens, nothing is counted
        let frozen = World::Frozen { core: CORE };
        assert_eq!(
            render_move(paced(1010), frozen, t(1020)),
            (paced(1010), Plan::Stock)
        );
    }

    #[test]
    fn another_core_or_a_clock_put_back_starts_afresh() {
        let other = Pacing::Paced {
            core: CoreId(0x2000),
            at: t(1010),
        };
        assert_eq!(
            render_move(other, world(1010, Some(900)), t(1020)).1,
            Plan::Stock
        );
        assert_eq!(
            render_move(paced(90_000), world(1010, Some(900)), t(1020)).1,
            Plan::Stock
        );
    }

    #[test]
    fn a_postponed_move_never_lets_the_point_near_the_horizon() {
        // every state, world and target on a grid of times: a postponement is
        // within the gap and the deadline, so the first stale item (point +
        // 50) is still 100 ms short of the event horizon (target - 500)
        for at in (0..1500).step_by(7) {
            for point in (0..1500).step_by(25) {
                for now in (point..1500).step_by(11) {
                    for target in (now..now + 120).step_by(3) {
                        if let (_, Plan::Postpone { keep }) =
                            render_move(paced(at), world(now, Some(point)), t(target))
                        {
                            assert_eq!(keep, t(point));
                            assert!(target - at < MIN_GAP_MS && target - point < DEADLINE_MS);
                            // the event horizon (target - 500) stays 100 ms short of
                            // the first stale item (point + 50)
                            assert!(target + 2 * TICK_MS <= point + TICK_MS + EVENT_HORIZON_MS);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn the_point_after_a_postponed_move_is_the_earliest() {
        assert_eq!(restored_point(t(900), None), t(900));
        assert_eq!(restored_point(t(900), Some(t(850))), t(850));
        assert_eq!(restored_point(t(900), Some(t(950))), t(900));
    }
}
