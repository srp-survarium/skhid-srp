//! Where an update lands: the server's rule for a place's inputs, the
//! one rule for every game (`netcode_port_069.md` 5.2). The server is the
//! clock of every input: it closes the ticks one by one, and an update that
//! comes after its tick was closed is **moved** to the first tick still
//! open, where every game can still take it, or **discarded**. Never into
//! a closed tick: no game ever gets an input for a tick the server has
//! already given every game.
//!
//! The rule, for an update of the tick `t` of one place, with inputs, the
//! first open tick being `F`:
//! - it goes to `X = max(F, t)`: its own tick if that is open
//!   ([`crate::wire::Outcome::Confirmed`]), else `F` (`Moved`);
//! - it is **stale**, and discarded, when a later update of the place
//!   already lands at or before `X`: moved past it, it would bring older
//!   keys back after newer ones;
//! - moved, it is discarded when its game could no longer follow the move
//!   (`X - t` past its reach: the game's history must still hold `t` when
//!   the answer comes) or when its game will not have reached `X` when the
//!   answer comes (it would make inputs on top of the moved ones): the
//!   server has that from the game's newest update (its tick, the time
//!   since it came, and the way the answer takes);
//! - an earlier update of the place that was to land at `X` too is
//!   **superseded** by it, and discarded: one update a tick for a place,
//!   the newest.
//!
//! So the landings of a place are in the order of its updates, at most one
//! a tick, and a moved update never lands on a tick its own game has an
//! update of in place (the client takes its own update of `X` out of its
//! lists before it puts the moved one there: `confirm`).

use crate::time::{GameTime, Grid};

/// The landings a [`Lane`] keeps: a place's latest updates with inputs.
pub const LANDINGS_KEPT: usize = 32;

/// Who made an update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Maker {
    /// A client: its own game follows the answer. It can move an input of
    /// its own by `reach_ms` at most, and its clock will be at `reached` at
    /// least when the answer comes.
    Client { reach_ms: u32, reached: GameTime },
    /// The server (its own bots): nobody to answer.
    Server,
}

/// Why an update lands nowhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// A later update of the place lands at or before its tick.
    Stale,
    /// Past its game's reach.
    TooLate,
    /// Its game has not reached the tick it would go to.
    Ahead,
}

/// The rule's answer for one update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// At `at`: the update's own tick (confirmed) or a later one (moved).
    /// `supersedes`: the earlier update of the place that was to land at
    /// `at`, discarded by this one.
    Land {
        at: GameTime,
        supersedes: Option<GameTime>,
    },
    Discard(Why),
    /// This update was decided before (a copy): no second answer.
    Known,
}

/// What became of one update, as a lane keeps it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fate {
    At(GameTime),
    Nowhere,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Landing {
    update: GameTime,
    fate: Fate,
}

/// One place's landings, the latest [`LANDINGS_KEPT`] of its updates with
/// inputs.
#[derive(Clone, Copy, Debug)]
pub struct Lane {
    landings: [Option<Landing>; LANDINGS_KEPT],
}

impl Default for Lane {
    fn default() -> Self {
        Self::new()
    }
}

impl Lane {
    pub const fn new() -> Lane {
        Lane {
            landings: [None; LANDINGS_KEPT],
        }
    }

    fn landed(&self) -> impl Iterator<Item = (GameTime, GameTime)> + '_ {
        self.landings.iter().flatten().filter_map(|each| match each.fate {
            Fate::At(at) => Some((each.update, at)),
            Fate::Nowhere => None,
        })
    }

    /// The rule for the update of the tick `update`, the first open tick
    /// being `first_open`. Pure: [`Lane::land`] decides and keeps.
    pub fn decide(&self, update: GameTime, first_open: GameTime, maker: Maker) -> Decision {
        if self.landings.iter().flatten().any(|each| each.update == update) {
            return Decision::Known;
        }
        let at = update.max(first_open);
        if self.landed().any(|(later, landed)| later > update && landed <= at) {
            return Decision::Discard(Why::Stale);
        }
        if at > update
            && let Maker::Client { reach_ms, reached } = maker
        {
            if at.since(update) > reach_ms {
                return Decision::Discard(Why::TooLate);
            }
            if at > reached {
                return Decision::Discard(Why::Ahead);
            }
        }
        let supersedes = self
            .landed()
            .find(|(earlier, landed)| *earlier < update && *landed == at)
            .map(|(earlier, _)| earlier);
        Decision::Land { at, supersedes }
    }

    /// Decides and keeps the decision.
    pub fn land(&mut self, update: GameTime, first_open: GameTime, maker: Maker) -> Decision {
        let decision = self.decide(update, first_open, maker);
        let fate = match decision {
            Decision::Known => return decision,
            Decision::Discard(_) => Fate::Nowhere,
            Decision::Land { at, supersedes } => {
                if let Some(earlier) = supersedes {
                    for each in self.landings.iter_mut().flatten() {
                        if each.update == earlier {
                            each.fate = Fate::Nowhere;
                        }
                    }
                }
                Fate::At(at)
            }
        };
        let landing = Landing { update, fate };
        // A free slot, else the oldest update's.
        let slot = match self.landings.iter().position(Option::is_none) {
            Some(free) => free,
            None => {
                let mut oldest = 0;
                for (index, each) in self.landings.iter().enumerate() {
                    if let (Some(each), Some(old)) = (each, &self.landings[oldest])
                        && each.update < old.update
                    {
                        oldest = index;
                    }
                }
                oldest
            }
        };
        self.landings[slot] = Some(landing);
        decision
    }

    /// A new game at the place (it connected again): its updates start
    /// anew.
    pub fn forget(&mut self) {
        self.landings = [None; LANDINGS_KEPT];
    }
}

/// How far a client's game can move an input of its own: its history holds
/// [`crate::EVENT_HORIZON_MS`]; the answer reaches it a round trip and a
/// tick after the server's close, and `margin` more is kept for the line.
pub const fn reach_ms(round_trip_ms: u32, grid: Grid, margin_ms: u32) -> u32 {
    crate::EVENT_HORIZON_MS.saturating_sub(round_trip_ms + grid.tick_ms() + margin_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(ms: u32) -> GameTime {
        GameTime(ms)
    }

    fn client(reached: u32) -> Maker {
        Maker::Client {
            reach_ms: 300,
            reached: t(reached),
        }
    }

    fn at(ms: u32) -> Decision {
        Decision::Land {
            at: t(ms),
            supersedes: None,
        }
    }

    #[test]
    fn an_update_in_time_lands_at_its_tick_and_a_late_one_at_the_first_open() {
        let mut lane = Lane::new();
        assert_eq!(lane.land(t(1000), t(950), client(1000)), at(1000));
        assert_eq!(lane.land(t(1050), t(1100), client(1150)), at(1100));
        // A copy: decided already.
        assert_eq!(lane.land(t(1050), t(1100), client(1150)), Decision::Known);
    }

    #[test]
    fn a_late_update_lands_where_its_game_can_follow_or_nowhere() {
        let lane = Lane::new();
        // 350 ms late, a reach of 300.
        assert_eq!(
            lane.decide(t(1000), t(1350), client(1400)),
            Decision::Discard(Why::TooLate)
        );
        // Its game has made no update of 1150 or later yet.
        assert_eq!(
            lane.decide(t(1000), t(1150), client(1100)),
            Decision::Discard(Why::Ahead)
        );
        assert_eq!(lane.decide(t(1000), t(1150), client(1150)), at(1150));
        // The server's bots: no reach, nobody behind.
        assert_eq!(lane.decide(t(1000), t(2000), Maker::Server), at(2000));
    }

    #[test]
    fn a_later_update_supersedes_one_at_the_same_tick_and_stales_an_earlier_one() {
        let mut lane = Lane::new();
        // 1000 and 1050 both late, both for 1100: the newer wins.
        assert_eq!(lane.land(t(1000), t(1100), client(1100)), at(1100));
        assert_eq!(
            lane.land(t(1050), t(1100), client(1100)),
            Decision::Land {
                at: t(1100),
                supersedes: Some(t(1000)),
            }
        );
        // The update of 1100 in time: it supersedes the moved one.
        assert_eq!(
            lane.land(t(1100), t(1100), client(1100)),
            Decision::Land {
                at: t(1100),
                supersedes: Some(t(1050)),
            }
        );
        // An older one coming after it would bring old keys back.
        assert_eq!(
            lane.land(t(950), t(1100), client(1100)),
            Decision::Discard(Why::Stale)
        );
        // One between landings in order goes in between.
        let mut lane = Lane::new();
        assert_eq!(lane.land(t(1200), t(1100), client(1200)), at(1200));
        assert_eq!(lane.land(t(1050), t(1150), client(1200)), at(1150));
        assert_eq!(
            lane.land(t(1100), t(1200), client(1200)),
            Decision::Discard(Why::Stale)
        );
    }

    #[test]
    fn landings_stay_in_the_order_of_their_updates_whatever_the_arrival_order() {
        // Every order of four updates of one place, two of them late:
        // whatever lands is strictly increasing in both the update's tick
        // and its landing, and never in a closed tick.
        let updates = [1000, 1050, 1100, 1150];
        let mut order = [0, 1, 2, 3];
        let mut permutations = 0;
        loop {
            permutations += 1;
            let mut lane = Lane::new();
            let mut first_open = 1100;
            let mut kept: std::vec::Vec<(u32, u32)> = std::vec::Vec::new();
            for (step, &index) in order.iter().enumerate() {
                let update = updates[index];
                if step == 2 {
                    first_open = 1150;
                }
                match lane.land(t(update), t(first_open), client(1150)) {
                    Decision::Land { at, supersedes } => {
                        assert!(at.0 >= first_open && at.0 >= update);
                        kept.retain(|(each, _)| Some(t(*each)) != supersedes);
                        kept.push((update, at.0));
                    }
                    Decision::Discard(_) => {}
                    Decision::Known => panic!("decided once"),
                }
            }
            kept.sort();
            assert!(
                kept.windows(2).all(|pair| pair[0].1 < pair[1].1),
                "{order:?}: {kept:?}"
            );
            // Next permutation.
            let Some(i) = (0..3).rev().find(|&i| order[i] < order[i + 1]) else {
                break;
            };
            let j = (i + 1..4).rev().find(|&j| order[j] > order[i]).unwrap();
            order.swap(i, j);
            order[i + 1..].reverse();
        }
        assert_eq!(permutations, 24);
    }

    #[test]
    fn a_full_lane_forgets_its_oldest() {
        let mut lane = Lane::new();
        for n in 0..LANDINGS_KEPT as u32 + 5 {
            let tick = 1000 + 50 * n;
            assert_eq!(lane.land(t(tick), t(tick), client(tick)), at(tick));
        }
        assert_eq!(
            lane.decide(t(1000 + 50 * 5), t(0), client(0)),
            Decision::Known
        );
        assert_ne!(lane.decide(t(1000), t(1000), client(5000)), Decision::Known);
        lane.forget();
        assert_eq!(lane.decide(t(1050), t(1050), client(1050)), at(1050));
    }

    #[test]
    fn the_reach_is_what_the_history_holds_past_the_way_back() {
        assert_eq!(reach_ms(100, Grid::STOCK, 50), 300);
        assert_eq!(reach_ms(1000, Grid::STOCK, 50), 0);
    }
}
