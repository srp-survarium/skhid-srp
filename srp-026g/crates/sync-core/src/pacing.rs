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

use crate::EVENT_HORIZON_MS;

/// The least game time between two rollbacks of render moves.
pub const MIN_GAP_MS: u32 = 50;

/// A rollback point this far behind the target is never postponed.
pub const DEADLINE_MS: u32 = 300;

/// The rollback point's "none" (`+18FC8h` = -1).
pub const NO_POINT: u32 = u32::MAX;

// A postponed rollback must happen before the first stale item (the one
// after the point) is the event-horizon item: target - point < horizon - one
// tick, with a frame's worth to spare.
const _: () = assert!(DEADLINE_MS + 2 * crate::TICK_MS <= EVENT_HORIZON_MS);

/// What the world core says before a render move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldView {
    /// The core's address: a new match has a new core.
    pub core: u32,
    /// The world's time (+18FC0h).
    pub now: u32,
    /// The rollback point (+18FC8h), [`NO_POINT`] for none.
    pub point: u32,
    /// The world waits for its first world (+18FDFh): move does nothing.
    pub frozen: bool,
}

/// Whether a move to a target has a rollback due: the stock move's own
/// test (0x2D2E62..0x2D2E91).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// The world agrees with every known input up to its time.
    Current,
    /// An input landed before the world's time: the world must be loaded
    /// from the item at or before `to` and stepped again.
    Behind { to: u32 },
}

impl Step {
    pub fn of(world: &WorldView, target: u32) -> Step {
        if world.point == NO_POINT || world.now <= world.point || world.point >= target {
            Step::Current
        } else {
            Step::Behind { to: world.point }
        }
    }
}

/// The pacing machine: what it remembers between render moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pacing {
    /// No rollback of a render move yet (or of this core).
    Free,
    /// The last rollback of a render move of `core` was for the target `at`.
    Paced { core: u32, at: u32 },
}

/// What to do with one render move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plan {
    /// The stock move, as it is.
    Stock,
    /// The stock move with the rollback point hidden (no rollback: the world
    /// steps on from its newest item), then the point put back as the least
    /// of `keep` and what the move left there.
    Postpone { keep: u32 },
}

/// The transition: a render move to `target` in `pacing`, the world as
/// `world` says. Total: every (state, world, target) has its answer.
pub fn render_move(pacing: Pacing, world: WorldView, target: u32) -> (Pacing, Plan) {
    let to = match Step::of(&world, target) {
        Step::Current => return (pacing, Plan::Stock),
        Step::Behind { to } => to,
    };
    // A shallow rollback loads the item a render move loads anyway (the
    // grid point at or before the world's time): it costs nothing more, so
    // it is stock and does not count as one.
    if to >= world.now - world.now % crate::TICK_MS {
        return (pacing, Plan::Stock);
    }
    let rolled = Pacing::Paced {
        core: world.core,
        at: target,
    };
    // Not a render advance (a frozen world, a target not past the world's
    // time): stock, which rolls back.
    if world.frozen || target <= world.now {
        return (rolled, Plan::Stock);
    }
    let recent = match pacing {
        Pacing::Paced { core, at } => core == world.core && target.wrapping_sub(at) < MIN_GAP_MS,
        Pacing::Free => false,
    };
    let due = target.wrapping_sub(to) >= DEADLINE_MS;
    if recent && !due {
        (pacing, Plan::Postpone { keep: to })
    } else {
        (rolled, Plan::Stock)
    }
}

/// The rollback point after a postponed move: the least of the kept one and
/// whatever the move lowered it to (its own local input snapping).
pub fn restored_point(keep: u32, after: u32) -> u32 {
    keep.min(after)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world(now: u32, point: u32) -> WorldView {
        WorldView {
            core: 0x1000,
            now,
            point,
            frozen: false,
        }
    }

    #[test]
    fn a_world_with_no_rollback_due_moves_as_stock_and_keeps_the_state() {
        let paced = Pacing::Paced {
            core: 0x1000,
            at: 990,
        };
        for w in [
            world(1000, NO_POINT),
            world(1000, 1000),
            world(1000, 1050),
            world(1000, 1100),
        ] {
            assert_eq!(render_move(paced, w, 1010), (paced, Plan::Stock), "{w:?}");
        }
    }

    #[test]
    fn the_first_rollback_is_stock_and_starts_the_pacing() {
        let (next, plan) = render_move(Pacing::Free, world(1000, 850), 1010);
        assert_eq!(plan, Plan::Stock);
        assert_eq!(
            next,
            Pacing::Paced {
                core: 0x1000,
                at: 1010
            }
        );
    }

    #[test]
    fn a_shallow_rollback_is_stock_and_does_not_count() {
        // the point is the grid item a render move loads anyway
        for pacing in [
            Pacing::Free,
            Pacing::Paced {
                core: 0x1000,
                at: 1025,
            },
        ] {
            assert_eq!(
                render_move(pacing, world(1030, 1000), 1040),
                (pacing, Plan::Stock)
            );
        }
        // one tick deeper is a rollback
        let paced = Pacing::Paced {
            core: 0x1000,
            at: 1025,
        };
        assert_eq!(
            render_move(paced, world(1030, 950), 1040),
            (paced, Plan::Postpone { keep: 950 })
        );
    }

    #[test]
    fn a_rollback_within_50_ms_of_the_last_is_postponed() {
        let paced = Pacing::Paced {
            core: 0x1000,
            at: 1010,
        };
        assert_eq!(
            render_move(paced, world(1010, 900), 1020),
            (paced, Plan::Postpone { keep: 900 })
        );
        assert_eq!(
            render_move(paced, world(1050, 900), 1059),
            (paced, Plan::Postpone { keep: 900 })
        );
        let (next, plan) = render_move(paced, world(1059, 900), 1060);
        assert_eq!(
            (next, plan),
            (
                Pacing::Paced {
                    core: 0x1000,
                    at: 1060
                },
                Plan::Stock
            )
        );
    }

    #[test]
    fn a_point_300_ms_behind_the_target_is_never_postponed() {
        let paced = Pacing::Paced {
            core: 0x1000,
            at: 1010,
        };
        assert_eq!(
            render_move(paced, world(1010, 750), 1049).1,
            Plan::Postpone { keep: 750 }
        );
        assert_eq!(render_move(paced, world(1010, 750), 1050).1, Plan::Stock);
        assert_eq!(render_move(paced, world(1010, 700), 1020).1, Plan::Stock);
    }

    #[test]
    fn moves_that_are_no_render_advance_are_stock() {
        let paced = Pacing::Paced {
            core: 0x1000,
            at: 1010,
        };
        // the target at or before the world's time (the horizon move, a camera)
        assert_eq!(render_move(paced, world(1020, 900), 1020).1, Plan::Stock);
        assert_eq!(render_move(paced, world(1020, 900), 950).1, Plan::Stock);
        // a frozen world
        let frozen = WorldView {
            frozen: true,
            ..world(1015, 900)
        };
        assert_eq!(render_move(paced, frozen, 1020).1, Plan::Stock);
    }

    #[test]
    fn another_core_or_a_clock_put_back_starts_afresh() {
        let paced = Pacing::Paced {
            core: 0x2000,
            at: 1010,
        };
        assert_eq!(render_move(paced, world(1010, 900), 1020).1, Plan::Stock);
        let later = Pacing::Paced {
            core: 0x1000,
            at: 90_000,
        };
        assert_eq!(render_move(later, world(1010, 900), 1020).1, Plan::Stock);
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
                        let w = world(now, point);
                        if let (_, Plan::Postpone { keep }) =
                            render_move(Pacing::Paced { core: 0x1000, at }, w, target)
                        {
                            assert_eq!(keep, point);
                            assert!(target - at < MIN_GAP_MS && target - point < DEADLINE_MS);
                            // the event horizon (target - 500) stays 100 ms short of
                            // the first stale item (point + 50)
                            assert!(
                                target + 2 * crate::TICK_MS
                                    <= point + crate::TICK_MS + EVENT_HORIZON_MS
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn the_point_after_a_postponed_move_is_the_least() {
        assert_eq!(restored_point(900, NO_POINT), 900);
        assert_eq!(restored_point(900, 850), 850);
        assert_eq!(restored_point(900, 950), 900);
    }
}
