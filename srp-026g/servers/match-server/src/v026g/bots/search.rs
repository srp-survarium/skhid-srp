use super::*;

/// A goal no way was found to is not planned to again for this long, ms.
pub(super) const NO_WAY_RETRY_MS: u32 = 500;

/// How long the plays leave a goal no way was found to (`Bot::no_way_to`):
/// a zone on a timer may open meanwhile, and the bot does something else.
pub const NO_WAY_KEEP_MS: u32 = 20_000;

/// How long the whole team leaves a goal one of its bots proved no way to
/// (`TeamState::no_way_to`): in the owner's match of 2026-10-16 all six
/// bots tried one battery 183 times, each learning on its own, for 20 s.
/// A pocket that zones on all match close is left for the match anyway
/// (`Ctx::reachable`); this is for the rest (a switching zone's, a spawn
/// area's).
pub const TEAM_NO_WAY_KEEP_MS: u32 = 60_000;

/// Grid searches a poll may make for all the match's bots: a search costs
/// up to tens of ms on the host, and the match loop is the server's one
/// thread. A bot left without one plans at its next tick, its old route
/// meanwhile. The floors they expand are bounded too
/// ([`tuning::ThinkTuning::search_floors_per_poll`], [`Searching`]).
pub(super) const SEARCHES_PER_POLL: u32 = 2;

/// A poll's route searches, as Detour's crowd runs its path queue: at most
/// [`SEARCHES_PER_POLL`] searches and `floors` floors expanded, all the
/// match's bots together. A search that runs past the floors is left under
/// way (`pending`, one a match) and goes on first at the next polls; no
/// other search starts meanwhile, and its bot keeps its old route. The
/// floors are counted, not timed, so the bots stay deterministic.
pub(super) struct Searching {
    pub(super) left: u32,
    pub(super) floors: usize,
    pub(super) pending: Option<Pending>,
}

/// A match's hold on its thread's search arrays (8 bytes a floor of the
/// largest grid searched, twice while a search is left under way): when
/// the thread's last match with bots ends, they are given back
/// ([`mapdata::free_search_memory`]), so the match loop keeps none between
/// matches with bots.
pub(super) struct SearchMemory;

impl SearchMemory {
    pub(super) fn hold() -> Self {
        let _ = MATCHES_WITH_BOTS.try_with(|n| n.set(n.get() + 1));
        Self
    }
}

impl Drop for SearchMemory {
    fn drop(&mut self) {
        let left = MATCHES_WITH_BOTS.try_with(|n| {
            n.set(n.get().saturating_sub(1));
            n.get()
        });
        if left == Ok(0) {
            let bytes = mapdata::search_memory_bytes();
            mapdata::free_search_memory();
            if bytes > 0 {
                log::info!(
                    "No match with bots left: their search arrays given back ({:.1} MB)",
                    bytes as f64 / 1e6
                );
            }
        }
    }
}

/// A route search left under way, and what it is for: it lets through the
/// floors it started with ([`Tactics::resume_route`]).
pub(super) struct Pending {
    pub(super) place: Place,
    pub(super) goal: Vec3,
    pub(super) time: u32,
    pub(super) side: Option<i32>,
    pub(super) objective: bool,
    /// From and to.
    pub(super) ends: (NodeId, NodeId),
    /// The spots it got stuck at, kept clear of: the first search of two
    /// (the second goes through them).
    pub(super) spots: Option<Vec<Vec3>>,
    /// The landings of jumps the body gave up, shut to this search and the
    /// second ([`Stuck::shut_landings`]).
    pub(super) closed: Vec<Vec3>,
    /// [`hazards::Hazards::version`] when it started.
    pub(super) hazards: u64,
    pub(super) planning: path::Planning,
    /// The bot's health when it started, for a crossing's search.
    pub(super) health: f32,
    /// A crossing's search, after the plain one (`crossing.rs`).
    pub(super) second: Option<crossing::Second>,
}

/// Whether `point` is on one of the shut `landings` ([`Stuck::shut_landings`]).
pub(super) fn shut_landing(landings: &[Vec3], point: Vec3) -> bool {
    landings.iter().any(|&landing| stuck::on_landing(landing, point))
}

/// Whether `point` is near one of `spots` ([`Stuck::near_spot`]).
pub(super) fn near_any(spots: &[Vec3], point: Vec3) -> bool {
    spots.iter().any(|&spot| stuck::near(spot, point))
}

impl Bots {
    /// The route search left under way goes on, first in the poll (as
    /// Detour's path queue goes on with its head): its answer to its bot,
    /// for its next tick. Dropped when its bot left or the hazards changed.
    pub(super) fn go_on_searching(&mut self) {
        let Some(mut pending) = self.searching.pending.take() else {
            return;
        };
        let Some(tactics) = self.tactics.as_ref() else {
            return;
        };
        if tactics.hazards.version() != pending.hazards
            || self.bots.get(pending.place).is_none_or(Option::is_none)
        {
            return;
        }
        if self.searching.left == 0 {
            self.searching.pending = Some(pending);
            return;
        }
        self.searching.left -= 1;
        let floors = &mut self.searching.floors;
        let (time, side, objective) = (pending.time, pending.side, pending.objective);
        let grid = &tactics.world.grid;
        let spots = pending.spots.take();
        let closed = std::mem::take(&mut pending.closed);
        let open = |node: NodeId| {
            spots
                .as_deref()
                .is_none_or(|spots| !near_any(spots, grid.position(node)))
                && !shut_landing(&closed, grid.position(node))
        };
        let start = pending.ends.0;
        let health = pending.health;
        let ended = if pending.second.is_some() {
            crossing::resume(
                tactics,
                &mut pending.planning,
                start,
                time,
                side,
                objective,
                health,
                &closed,
                floors,
            )
        } else {
            tactics.resume_route(
                &mut pending.planning,
                start,
                time,
                side,
                objective,
                open,
                floors,
            )
        };
        let Some(mut route) = ended else {
            pending.spots = spots;
            pending.closed = closed;
            self.searching.pending = Some(pending);
            return;
        };
        let mut crossed = false;
        let mut kept = false;
        let (from, to) = pending.ends;
        let long_kept = self
            .bots
            .get(pending.place)
            .and_then(|bot| bot.as_ref())
            .is_some_and(|bot| long_kept(bot.long_kept, pending.goal, time));
        if let Some(second) = pending.second.take() {
            (route, crossed, kept) = crossing::choose(
                tactics,
                from,
                second.plain,
                route,
                health,
                time,
                side,
                objective,
            );
        } else {
            // None round the spots it got stuck at: through them.
            if route.is_none() && spots.is_some() {
                let through_open =
                    |node: NodeId| !shut_landing(&closed, grid.position(node));
                match tactics.start_route(from, to, time, side, objective, through_open, floors) {
                    Ok(through) => route = through,
                    Err(planning) => {
                        pending.planning = planning;
                        pending.closed = closed;
                        self.searching.pending = Some(pending);
                        return;
                    }
                }
            }
            // No way, or a long one past zones on all match: a crossing's
            // search (`crossing.rs`).
            if !(route.is_some() && long_kept)
                && crossing::wanted(tactics, route.as_ref(), from, to, side, objective, health)
            {
                match crossing::start(
                    tactics, from, to, time, side, objective, health, &closed, floors,
                ) {
                    Ok(crossing_route) => {
                        (route, crossed, kept) = crossing::choose(
                            tactics,
                            from,
                            route,
                            crossing_route,
                            health,
                            time,
                            side,
                            objective,
                        );
                    }
                    Err(planning) => {
                        pending.planning = planning;
                        pending.second = Some(crossing::Second {
                            plain: route,
                            health,
                        });
                        self.searching.pending = Some(pending);
                        return;
                    }
                }
            }
        }
        if let Some(Some(bot)) = self.bots.get_mut(pending.place) {
            if kept {
                bot.long_kept = Some((pending.goal, time + crossing::LONG_RETRY_MS));
            }
            #[cfg(test)]
            {
                bot.bench_found_ring = crossed && crossing::last_ring();
            }
            bot.found = Some((pending.goal, route, crossed));
        }
    }
}

/// Whether a crossing's search for a long route to `goal` kept the plain
/// one lately (`Bot::long_kept`, `crossing::LONG_RETRY_MS`).
pub(super) fn long_kept(kept: Option<(Vec3, u32)>, goal: Vec3, now: u32) -> bool {
    kept.is_some_and(|(to, until)| now < until && to.distance_xz(goal) <= crossing::LONG_RETRY_M)
}
