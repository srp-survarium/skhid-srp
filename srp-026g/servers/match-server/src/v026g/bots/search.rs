use super::*;
use super::tuning::SearchQueue;

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

/// A poll's route searches for the bots' drive, as Detour's crowd runs its
/// path queue: at most [`SEARCHES_PER_POLL`] searches and `floors` floors
/// expanded a poll. A search that runs past its floors is left under way
/// and goes on first at the next polls; no other search of its queue starts
/// meanwhile, and its bot walks the way so far. The floors are counted, not
/// timed, so the bots stay deterministic. (The plays' own route questions,
/// such as `Tactics::route`, are not drawn from it.)
///
/// The one owner of the drive's budget: a search starts only through
/// [`Self::grant`], goes on only through [`Self::resume`], and the floors it
/// spends are booked when its [`Grant`] drops. How the match's bots share it
/// is [`SearchQueue`]:
/// - `FirstCome` (as it was): one pool and one search under way for the
///   whole match, taken by place order, so the first places (team 0 on the
///   bench, the first to join live) claim first, and one bot's long search
///   holds every other bot's;
/// - `Fair`: the poll's searches are the match's, but the bots go in turn
///   by team, one of each, the team that did not get the last search first,
///   and each team round robin from the bot after its last served one (a
///   bot that keeps asking is served within its team's size of grants); the
///   floors are each team's by its living bots (a team's costly searches
///   starve only its own bots), a team whose bots have all gone lending the
///   rest to the other; each team has its own search under way, which after
///   the poll's bots goes on with what both teams left.
pub(super) struct Searching {
    queue: Queue,
}

/// [`Searching`]'s state by [`SearchQueue`].
enum Queue {
    /// One pool: the poll's searches and floors left, the match's search
    /// under way.
    FirstCome { left: u32, floors: usize, pending: Option<Box<Pending>> },
    /// The poll's searches left (the match's), each team's part, and the
    /// team that goes first (the one that did not get the last search).
    Fair { left: u32, teams: [Share; 2], first: u8 },
}

/// A team's part of a fair queue.
struct Share {
    /// Its floors left this poll.
    floors: usize,
    /// Its search under way (boxed: it is large, and most polls have none).
    pending: Option<Box<Pending>>,
    /// The place after its last served bot: its round robin goes on there.
    cursor: usize,
    /// Whether its bots have all gone this poll (its rest is the other's).
    going: Going,
    /// The bench's: a bot of it wanted a search this poll and had none.
    #[cfg(test)]
    starved: bool,
}

impl Share {
    fn new() -> Self {
        Self {
            floors: 0,
            pending: None,
            cursor: 0,
            going: Going::Going,
            #[cfg(test)]
            starved: false,
        }
    }
}

/// Whether a team's bots still go this poll.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Going {
    Going,
    Done,
}

/// The bench's counts of the queue, by team (`tests/tallies/owner/search_queue.rs`):
/// - searches granted (started on a grant), bot ticks starved (a search wanted and none to be had), floors spent
///   (by the team of the search's bot, each slice), the polls a bot waited before its search (their sum, and the
///   grants after a wait);
/// - floors handed over (drawn from the other team's rest once its bots had all gone); floors left idle at a poll's
///   end while the team had a starved bot or a search under way;
/// - searches under way dropped: their goal moved, the hazards changed, their bot left;
/// - drive ticks walked on a search's way so far (a partial route); unstick jumps, and those within 5 s of walking a
///   partial route.
#[cfg(test)]
pub mod queue {
    use std::cell::Cell;

    pub const GRANTED: usize = 0;
    pub const STARVED: usize = 1;
    pub const FLOORS: usize = 2;
    pub const WAITED_POLLS: usize = 3;
    pub const WAITED: usize = 4;
    pub const HANDED_OVER: usize = 5;
    pub const IDLE_WITH_DEMAND: usize = 6;
    pub const DROPPED_MOVED: usize = 7;
    pub const DROPPED_HAZARDS: usize = 8;
    pub const DROPPED_GONE: usize = 9;
    pub const PARTIAL_TICKS: usize = 10;
    pub const UNSTICKS: usize = 11;
    pub const UNSTICKS_PARTIAL: usize = 12;
    pub const NAMES: [&str; 13] = [
        "granted",
        "starved",
        "floors",
        "waited_polls",
        "waited",
        "handed_over",
        "idle_with_demand",
        "dropped_moved",
        "dropped_hazards",
        "dropped_gone",
        "partial_ticks",
        "unsticks",
        "unsticks_partial",
    ];
    /// An unstick jump within this of walking a partial route counts as on one, ms.
    pub const PARTIAL_RECENT_MS: u32 = 5_000;

    thread_local! {
        static COUNTS: Cell<[[u64; 13]; 2]> = const { Cell::new([[0; 13]; 2]) };
    }

    /// `n` of count `what` for team `team`.
    pub fn add(team: u8, what: usize, n: u64) {
        COUNTS.with(|c| {
            let mut all = c.get();
            all[usize::from(team & 1)][what] += n;
            c.set(all);
        });
    }

    /// The counts so far, by team.
    pub fn counted() -> [[u64; 13]; 2] {
        COUNTS.with(Cell::get)
    }
}

impl Default for Searching {
    fn default() -> Self {
        Self { queue: Queue::FirstCome { left: SEARCHES_PER_POLL, floors: 0, pending: None } }
    }
}

/// A search granted on a [`Searching`]: the floors it may expand
/// ([`Self::floors`], spent as the search runs); left under way with
/// [`Self::leave_under_way`]. When it drops, what it spent is taken off its
/// team's floors first, then the lending team's.
pub(super) struct Grant<'a> {
    searching: &'a mut Searching,
    team: u8,
    lent: usize,
    pub(super) floors: usize,
}

impl Grant<'_> {
    /// Its search goes on at the next polls, in its team's slot.
    pub(super) fn leave_under_way(&mut self, pending: Pending) {
        *self.searching.slot_mut(self.team) = Some(Box::new(pending));
    }
}

impl Drop for Grant<'_> {
    fn drop(&mut self) {
        let spent = self.lent - self.floors.min(self.lent);
        #[cfg(test)]
        queue::add(self.team, queue::FLOORS, spent as u64);
        match &mut self.searching.queue {
            Queue::FirstCome { floors, .. } => *floors -= spent.min(*floors),
            Queue::Fair { teams, .. } => {
                let own = usize::from(self.team & 1);
                let from_own = spent.min(teams[own].floors);
                teams[own].floors -= from_own;
                let rest = spent - from_own;
                teams[1 - own].floors -= rest.min(teams[1 - own].floors);
                #[cfg(test)]
                queue::add(self.team, queue::HANDED_OVER, rest as u64);
            }
        }
    }
}

impl Searching {
    /// A new poll's searches and `floors` by `queue`, for the living bots of
    /// each team `living` (`Fair` shares the floors by them; a team with none,
    /// such as the humans' side of a PvB match, has none and is done).
    pub(super) fn new_poll(&mut self, queue: SearchQueue, floors: usize, living: [usize; 2]) {
        let left = SEARCHES_PER_POLL;
        match (queue, &mut self.queue) {
            (SearchQueue::FirstCome, Queue::FirstCome { left: l, floors: f, .. }) => (*l, *f) = (left, floors),
            (SearchQueue::FirstCome, _) => self.queue = Queue::FirstCome { left, floors, pending: None },
            (SearchQueue::Fair, Queue::FirstCome { .. }) => {
                self.queue = Queue::Fair { left, teams: [Share::new(), Share::new()], first: 0 };
                self.new_poll(queue, floors, living);
            }
            (SearchQueue::Fair, Queue::Fair { left: l, teams, .. }) => {
                *l = left;
                let all = living[0] + living[1];
                let zero = (floors * living[0]).checked_div(all).unwrap_or(0);
                let shares = [zero, if all == 0 { 0 } else { floors - zero }];
                for (t, share) in teams.iter_mut().enumerate() {
                    share.floors = shares[t];
                    share.going = if living[t] == 0 { Going::Done } else { Going::Going };
                    #[cfg(test)]
                    {
                        share.starved = false;
                    }
                }
            }
        }
    }

    /// The places of `bots` in the order the poll takes them: by place
    /// (`FirstCome`); in turn by team, one of each, the team that goes first
    /// first, each team from its round robin's place on (`Fair`).
    pub(super) fn order(&self, bots: &[Option<Bot>]) -> Vec<usize> {
        let teams: Vec<Option<u8>> = bots.iter().map(|b| b.as_ref().map(|b| b.team & 1)).collect();
        self.order_of(&teams)
    }

    /// [`Self::order`] for each place's team (`None`: no bot there).
    fn order_of(&self, place_teams: &[Option<u8>]) -> Vec<usize> {
        let Queue::Fair { teams, first, .. } = &self.queue else {
            return (0..place_teams.len()).collect();
        };
        let by_team = [0u8, 1].map(|team| {
            let mut places: Vec<usize> = (0..place_teams.len()).filter(|&p| place_teams[p] == Some(team)).collect();
            let cursor = teams[usize::from(team)].cursor;
            let k = places.iter().position(|&p| p >= cursor).unwrap_or(0);
            places.rotate_left(k);
            places
        });
        let first = usize::from(*first & 1);
        let (a, b) = (&by_team[first], &by_team[1 - first]);
        let mut order = Vec::with_capacity(place_teams.len());
        for i in 0..a.len().max(b.len()) {
            order.extend(a.get(i));
            order.extend(b.get(i));
        }
        order
    }

    /// Team `team`'s bots have all gone this poll: its rest is the other's to draw on (`Fair`).
    pub(super) fn team_done(&mut self, team: u8) {
        if let Queue::Fair { teams, .. } = &mut self.queue {
            teams[usize::from(team & 1)].going = Going::Done;
        }
    }

    /// The searches under way's slots: one (the match's) or one a team.
    pub(super) fn slots(&self) -> u8 {
        match self.queue {
            Queue::FirstCome { .. } => 1,
            Queue::Fair { .. } => 2,
        }
    }

    /// The search under way of team `team`'s queue.
    fn slot_mut(&mut self, team: u8) -> &mut Option<Box<Pending>> {
        match &mut self.queue {
            Queue::FirstCome { pending, .. } => pending,
            Queue::Fair { teams, .. } => &mut teams[usize::from(team & 1)].pending,
        }
    }

    fn slot(&self, team: u8) -> Option<&Pending> {
        match &self.queue {
            Queue::FirstCome { pending, .. } => pending.as_deref(),
            Queue::Fair { teams, .. } => teams[usize::from(team & 1)].pending.as_deref(),
        }
    }

    /// The floors a search of team `team` may draw now: the match's
    /// (`FirstCome`); its team's, with the other's rest once that team's bots
    /// have all gone (`Fair`).
    fn available(&self, team: u8) -> usize {
        match &self.queue {
            Queue::FirstCome { floors, .. } => *floors,
            Queue::Fair { teams, .. } => {
                let (own, other) = (&teams[usize::from(team & 1)], &teams[usize::from(!team & 1)]);
                own.floors + if other.going == Going::Done { other.floors } else { 0 }
            }
        }
    }

    fn left_mut(&mut self) -> &mut u32 {
        match &mut self.queue {
            Queue::FirstCome { left, .. } | Queue::Fair { left, .. } => left,
        }
    }

    /// Whether a search of team `team` may start now: a search and floors
    /// left to it, and no search of its queue under way.
    pub(super) fn may_start(&self, team: u8) -> bool {
        let left = match self.queue {
            Queue::FirstCome { left, .. } | Queue::Fair { left, .. } => left,
        };
        left > 0 && self.available(team) > 0 && self.slot(team).is_none()
    }

    /// A search for team `team`'s bot at `place`, if one may start
    /// ([`Self::may_start`]): one of the poll's searches is taken, and in a
    /// fair queue its team's round robin moves past `place` and the other
    /// team goes first next.
    pub(super) fn grant(&mut self, team: u8, place: Place) -> Option<Grant<'_>> {
        if !self.may_start(team) {
            return None;
        }
        *self.left_mut() -= 1;
        if let Queue::Fair { teams, first, .. } = &mut self.queue {
            teams[usize::from(team & 1)].cursor = place + 1;
            *first = !team & 1;
        }
        let lent = self.available(team);
        Some(Grant { searching: self, team, lent, floors: lent })
    }

    /// The search under way in slot `slot`, taken out to go on (`None`: none).
    pub(super) fn take_under_way(&mut self, slot: u8) -> Option<Pending> {
        self.slot_mut(slot).take().map(|pending| *pending)
    }

    /// The search under way of team `team` (or the match's), taken back
    /// when there is no search left to this poll, else given one of the
    /// poll's searches and its team's floors to go on with (the bot's
    /// `team`, for the books).
    pub(super) fn resume(&mut self, slot: u8, team: u8, pending: Pending) -> Option<(Pending, Grant<'_>)> {
        if *self.left_mut() == 0 {
            *self.slot_mut(slot) = Some(Box::new(pending));
            return None;
        }
        *self.left_mut() -= 1;
        let lent = self.available(team);
        Some((pending, Grant { searching: self, team, lent, floors: lent }))
    }

    /// Team `team`'s search under way for the bot at `place`, if that bot's: its goal.
    pub(super) fn own_under_way(&self, team: u8, place: Place) -> Option<Vec3> {
        self.slot(team).filter(|p| p.place == place).map(|p| p.goal)
    }

    /// Team `team`'s search under way is dropped (its goal moved on).
    pub(super) fn drop_own(&mut self, team: u8) {
        *self.slot_mut(team) = None;
        #[cfg(test)]
        queue::add(team, queue::DROPPED_MOVED, 1);
    }

    /// The bench's: a bot of team `team` wanted a search and had none.
    #[cfg(test)]
    pub(super) fn starved(&mut self, team: u8) {
        if let Queue::Fair { teams, .. } = &mut self.queue {
            teams[usize::from(team & 1)].starved = true;
        }
    }

    /// The bench's count at a poll's end: floors left while a team had demand.
    #[cfg(test)]
    pub(super) fn count_idle(&self) {
        if let Queue::Fair { teams, .. } = &self.queue {
            for (t, share) in teams.iter().enumerate() {
                if share.floors > 0 && (share.starved || share.pending.is_some()) {
                    queue::add(t as u8, queue::IDLE_WITH_DEMAND, share.floors as u64);
                }
            }
        }
    }

    /// Whether the poll's end gives the searches under way another go on
    /// what both teams left (`Fair`).
    pub(super) fn goes_on_at_poll_end(&self) -> bool {
        matches!(self.queue, Queue::Fair { .. })
    }
}

/// The living bots of each team (entered, alive in `view`), for the poll's fair shares.
pub(super) fn living(bots: &[Option<Bot>], view: &MatchView) -> [usize; 2] {
    let mut n = [0usize; 2];
    for bot in bots.iter().flatten() {
        if bot.entered.is_some() && view.place(bot.place).is_some_and(|p| p.alive) {
            n[usize::from(bot.team & 1)] += 1;
        }
    }
    n
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
    /// What it keeps out: with the spots the body got stuck at as its soft
    /// part for the first search of two (the second goes through them,
    /// [`RouteFilter::without_soft`]), and the landings of jumps it gave up
    /// ([`Stuck::shut_landings`]) as its hard part for both.
    pub(super) filter: RouteFilter,
    /// [`hazards::Hazards::version`] when it started.
    pub(super) hazards: u64,
    pub(super) planning: path::Planning,
    /// The bot's health when it started, for a crossing's search.
    pub(super) health: f32,
    /// A crossing's search, after the plain one (`crossing.rs`).
    pub(super) second: Option<crossing::Second>,
}

impl Bots {
    /// The route search left under way goes on, first in the poll (as
    /// Detour's path queue goes on with its head): its answer to its bot,
    /// for its next tick. Dropped when its bot left or the hazards changed.
    pub(super) fn go_on_searching(&mut self) {
        for slot in 0..self.features.searching.slots() {
            let Some(pending) = self.features.searching.take_under_way(slot) else {
                continue;
            };
            let team = self.bots.get(pending.place).and_then(Option::as_ref).map(|bot| bot.team);
            // TEST ONLY: the work counters count a pending search for the team of the bot it is for.
            #[cfg(test)]
            if let Some(team) = team {
                super::bench_work::set(super::bench_work::Who::Team(team));
            }
            self.go_on_searching_with(slot, team, pending);
            #[cfg(test)]
            super::bench_work::set(super::bench_work::Who::Shared);
        }
    }

    /// The search under way of slot `slot` (its bot's `team`, none when the
    /// bot left) goes on, on a search and the floors its grant lends.
    fn go_on_searching_with(&mut self, slot: u8, team: Option<u8>, pending: Pending) {
        let Some(tactics) = self.tactics.as_ref() else {
            return;
        };
        let Some(team) = team else {
            #[cfg(test)]
            queue::add(slot, queue::DROPPED_GONE, 1);
            return;
        };
        if tactics.hazards.version() != pending.hazards {
            #[cfg(test)]
            queue::add(team, queue::DROPPED_HAZARDS, 1);
            return;
        }
        let Some((mut pending, mut grant)) = self.features.searching.resume(slot, team, pending) else {
            return;
        };
        let floors = &mut grant.floors;
        let (time, side, objective) = (pending.time, pending.side, pending.objective);
        let filter = pending.filter.clone();
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
                &filter,
                floors,
            )
        } else {
            tactics.resume_route(
                &mut pending.planning,
                start,
                time,
                side,
                objective,
                &filter,
                floors,
            )
        };
        let Some(mut route) = ended else {
            grant.leave_under_way(pending);
            return;
        };
        let mut crossed = false;
        let mut kept = false;
        let (from, to) = pending.ends;
        let long_kept = self
            .bots
            .get(pending.place)
            .and_then(|bot| bot.as_ref())
            .is_some_and(|bot| long_kept(bot.features.crossing.long_kept, pending.goal, time));
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
            if route.is_none() && filter.has_soft() {
                let through = filter.without_soft();
                match tactics.start_route(from, to, time, side, objective, &through, floors) {
                    Ok(found) => route = found,
                    Err(planning) => {
                        pending.planning = planning;
                        pending.filter = through;
                        grant.leave_under_way(pending);
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
                    tactics, from, to, time, side, objective, health, &filter, floors,
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
                        grant.leave_under_way(pending);
                        return;
                    }
                }
            }
        }
        if let Some(Some(bot)) = self.bots.get_mut(pending.place) {
            if kept {
                bot.features.crossing.long_kept = Some((pending.goal, time + crossing::LONG_RETRY_MS));
            }
            #[cfg(test)]
            {
                bot.bench_found_ring = crossed && crossing::last_ring();
            }
            bot.features.crossing.found = Some((pending.goal, route, crossed));
        }
    }
}

/// Whether a crossing's search for a long route to `goal` kept the plain
/// one lately (`Bot::long_kept`, `crossing::LONG_RETRY_MS`).
pub(super) fn long_kept(kept: Option<(Vec3, u32)>, goal: Vec3, now: u32) -> bool {
    kept.is_some_and(|(to, until)| now < until && to.distance_xz(goal) <= crossing::LONG_RETRY_M)
}

/// The bench's counts of the route searches a drive makes ([`asks::searched`]):
/// all, those from a start floor in a pocket, those within a second of the
/// bot's last, and the asks a bot given up in its pocket was spared
/// ([`Ask::Skipped`], in drive ticks) and the episodes of a bot shut in ([`Ask::Episodes`]).
#[cfg(test)]
pub(super) mod asks {
    use std::sync::atomic::{AtomicU64, Ordering};

    #[derive(Clone, Copy)]
    pub(crate) enum Ask {
        All,
        Pocket,
        Again,
        Skipped,
        Episodes,
    }

    /// One bot's own: when its last search was, and how many it made.
    #[derive(Clone, Copy, Debug, Default)]
    pub(crate) struct Asked {
        pub(crate) last: u32,
        pub(crate) total: u32,
    }

    static COUNTS: [AtomicU64; 5] = [const { AtomicU64::new(0) }; 5];

    pub(crate) fn count(kind: Ask) {
        COUNTS[kind as usize].fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn counts() -> [u64; 5] {
        std::array::from_fn(|k| COUNTS[k].load(Ordering::Relaxed))
    }

    /// A search at `time` by a bot (`asked`); from a pocket's floor or not.
    pub(crate) fn searched(asked: &mut Asked, time: u32, in_pocket: bool) {
        count(Ask::All);
        if in_pocket {
            count(Ask::Pocket);
        }
        if asked.total != 0 && time.saturating_sub(asked.last) <= 1_000 {
            count(Ask::Again);
        }
        asked.last = time;
        asked.total += 1;
    }
}

#[cfg(test)]
mod queue_tests {
    use super::*;

    /// As it was: one pool of the poll's searches and floors for both teams, one search under way, places in order.
    #[test]
    fn first_come_is_one_pool_one_search_under_way_and_the_place_order() {
        let mut s = Searching::default();
        s.new_poll(SearchQueue::FirstCome, 10_000, [3, 3]);
        assert_eq!(s.slots(), 1);
        assert_eq!(s.order_of(&[Some(1), None, Some(0), Some(1)]), [0, 1, 2, 3]);
        let g = s.grant(1, 0).expect("a search");
        assert_eq!(g.floors, 10_000);
        drop(g);
        let mut g = s.grant(0, 2).expect("the second");
        g.floors -= 4_000;
        drop(g);
        assert!(!s.may_start(0) && !s.may_start(1), "one pool: both searches spent for both teams");
        s.new_poll(SearchQueue::FirstCome, 10_000, [3, 3]);
        assert_eq!(s.order_of(&[Some(1), Some(0), Some(0)]), [0, 1, 2], "no rotation");
    }

    /// Fair: the floors by living bots; a team's rest lent once its bots are done; a botless team none; a grant books
    /// its floors off its own first.
    #[test]
    fn fair_shares_by_the_living_and_lends_a_done_teams_rest() {
        let mut s = Searching::default();
        s.new_poll(SearchQueue::Fair, 10_000, [3, 7]);
        assert_eq!(s.slots(), 2);
        assert_eq!((s.available(0), s.available(1)), (3_000, 7_000));
        let mut g = s.grant(0, 0).expect("team 0");
        assert_eq!(g.floors, 3_000);
        g.floors = 0;
        drop(g);
        assert_eq!(s.available(0), 0);
        s.team_done(1);
        assert_eq!(s.available(0), 7_000, "team 1 done: its rest is team 0's to draw");
        let mut g = s.grant(0, 1).expect("team 0 again, on team 1's rest");
        g.floors -= 500;
        drop(g);
        assert_eq!(s.available(1), 6_500, "taken off team 1's rest");
        assert!(!s.may_start(0) && !s.may_start(1), "both searches spent");
        // A botless team (the humans' side of a PvB match): the other's floors are the whole, and it is done.
        s.new_poll(SearchQueue::Fair, 10_000, [4, 0]);
        assert_eq!((s.available(0), s.available(1)), (10_000, 0), "a botless team never draws");
    }

    /// Fair, round robin by grant: with every bot asking at every drive poll, each team's bots are served in turn (a
    /// bot within its team's size of grants), and the teams alternate first, for team sizes 1 to 10 each.
    #[test]
    fn fair_serves_every_bot_within_its_teams_size_of_grants() {
        for n0 in 1..=10usize {
            for n1 in 1..=10usize {
                let mut place_teams = vec![Some(0u8); n0];
                place_teams.extend(vec![Some(1u8); n1]);
                let mut s = Searching::default();
                let mut served: [Vec<usize>; 2] = [Vec::new(), Vec::new()];
                for _ in 0..2 * (n0 + n1) {
                    s.new_poll(SearchQueue::Fair, 10_000, [n0, n1]);
                    for place in s.order_of(&place_teams) {
                        let team = place_teams[place].unwrap();
                        if let Some(mut g) = s.grant(team, place) {
                            g.floors -= 1;
                            served[usize::from(team)].push(place);
                        }
                    }
                }
                for (t, n) in [(0usize, n0), (1, n1)] {
                    for window in served[t].windows(n) {
                        let mut seen = window.to_vec();
                        seen.sort_unstable();
                        seen.dedup();
                        assert_eq!(seen.len(), n, "team {t} of {n} (other {}): {:?}", if t == 0 { n1 } else { n0 }, served[t]);
                    }
                }
                // Equal claims: each team about half of the grants.
                let (g0, g1) = (served[0].len() as i64, served[1].len() as i64);
                assert!((g0 - g1).abs() <= 1, "{n0} v {n1}: {g0} vs {g1}");
                // Each grant's floor is booked to its team when it drops.
                let floors = queue::counted();
                assert!(floors[0][queue::FLOORS] >= g0 as u64 && floors[1][queue::FLOORS] >= g1 as u64);
            }
        }
    }

    /// The pending slot of team t holds only team t's search; its search under way holds only its own team.
    #[test]
    fn fair_starts_with_each_teams_slot_free() {
        let mut s = Searching::default();
        s.new_poll(SearchQueue::Fair, 10_000, [2, 2]);
        assert!(s.own_under_way(0, 0).is_none() && s.may_start(0) && s.may_start(1));
    }
}
