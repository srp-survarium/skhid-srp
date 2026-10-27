//! A bot that is stuck: it walks, and its body does not move.
//!
//! The walk grid is near the game's walking but not the same: a body may
//! catch on a ledge the grid thinks is a step, a door narrower than the
//! grid's, a crate the walker mesh lacks, or another player. The bot
//! cannot feel it: it sees its body only in the worlds it is given (one a
//! second or two while a bot needs one). So it is stuck when two worlds
//! [`STUCK_AFTER_MS`] apart show its body moved less than
//! [`STUCK_MOVED_M`] while its own keys, reckoned on the walk grid
//! ([`super::body::BodyTrack::last_meant`]), would have moved it at least
//! [`MEANT_M`]: 1.5 to 3 s after it caught, about when a player would
//! notice. A bot that stood to aim, crept, or waited for its search with a
//! route in hand meant to go nowhere, and is not stuck (the live logs of
//! 2026-10-17..23 had a third of their stuck lines with nothing within
//! reach of the body: `player_bots_stuck.md`).
//!
//! The reckoning walks the same grid, so where the grid has a wall too (a
//! body fallen to a floor under its route, steering at the route above)
//! it does not move either and means nothing: the second sign needs no
//! grid. The worlds show the walk keys held ([`HELD_KEYS`]) in every one
//! of them over [`HELD_AFTER_MS`] while the body stayed within
//! [`HELD_MOVED_M`] of where they started: stuck too (0x4da3b51d: a
//! carrier held W against a wall 273 s, its reckoning as still as its
//! body, `player_bots_stuck.md` §8).
//!
//! What it does then, more each time at the same spot (within
//! [`SPOT_M`] in [`AGAIN_MS`]), as a player would:
//! 1. [`Recovery::BackOff`]: a jump, a short step back and to a side, and
//!    the same route planned again from there;
//! 2. [`Recovery::Sidestep`]: a jump and a longer step to a side, and
//!    another goal;
//! 3. [`Recovery::GiveUp`]: a step back and to a side (each time again
//!    there the other way round: [`Stuck::escape`]), it stands and looks
//!    about, then goes anywhere else.
//!
//! A spot it got stuck at twice is kept for [`SPOT_KEPT_MS`]: a new route
//! that passes near it is not taken ([`Stuck::clear`]), so it does not
//! walk into the same trap again. Once is not enough: the spot may be the
//! only door to the other side, and a body caught there once by another
//! player gets through the next time; and where every route passes a spot,
//! the bot takes one anyway (`Bot::choose`).
//!
//! A fall off its route is such a spot too, from the first fall
//! ([`Stuck::fell`]): a bot that fell off a narrow way goes another way
//! for a while if there is one, as a player would (`player_bots_stuck.md`
//! §8.6). A spot is a place on one
//! floor: a floor [`SPOT_HEIGHT_M`] over or under it is not near it.
//!
//! The team learns them too ([`TeamSpots`]): a spot that caught its bots
//! [`TEAM_AVOID_AFTER`] times, whichever bots, or one a bot fell off its
//! route from, every bot of the team keeps clear of for the match, as
//! players who saw a teammate stick or fall there would (0x60aeff4f,
//! Cologne: 14 blocks by 6 bots at one strip; two carriers 4 s apart off
//! one stair's edge into a pit they never left). Round them only where
//! the way is not much longer ([`worth_round`]), and never at the goal
//! itself ([`GOAL_FREE_M`]).

use mapdata::Vec3;

use super::hazards::ZoneSet;
use super::path::Route;
use super::tuning::UnstickTuning;
use crate::v026g::drive::key;

/// Worlds this far apart in time…
pub const STUCK_AFTER_MS: u32 = 1_500;
/// … that show the body moved less than this while walking: stuck.
pub const STUCK_MOVED_M: f32 = 0.4;
/// … while its keys meant to move it at least this far, metres (a walk
/// of 1.5 s is 5 m, a crouched one 2.5 m).
pub const MEANT_M: f32 = 1.2;
/// The walk keys: a world that shows one held means the body was walked.
pub const HELD_KEYS: u32 = key::FORWARD | key::BACK | key::LEFT | key::RIGHT;
/// Walk keys held in every world this long, ms… GUESS: twice the reckoned
/// watch's time, as no reckoning vouches that the keys meant a walk (a
/// side step in a fight goes and comes back within it); the stuck carrier
/// of 0x4da3b51d was found after one such span instead of never.
pub const HELD_AFTER_MS: u32 = 3_000;
/// … while every world kept the body this near the first of them: stuck,
/// metres. GUESS: a walk of 3 s is 10 m, a crouched one 5 m; a body
/// nudged by a wall it slides along moves more.
pub const HELD_MOVED_M: f32 = 0.5;
/// The progress watch ([`Stuck::no_progress`]): the distance along its route a walking body must gain within the window, metres.
/// A walk of the window (6 s) is 20 m, a crouched one 15 m; a loop round a post gains none.
pub const PROGRESS_M: f32 = 1.5;
/// … and the route it is still short of its end by at least this, metres (a body at the end of its route holds keys and gains none).
pub const PROGRESS_END_M: f32 = 1.5;
/// … and only a body this near its route counts (off it, the motor's own replan acts), metres.
pub const PROGRESS_OFF_M: f32 = 3.0;
/// … the leg kept clear of is the point this far ahead of where the body got to along the route, metres.
pub const PROGRESS_LEG_M: f32 = 1.0;
/// A world this long after the one before it, ms, is noted with the gap: a note after it (a stuck body, a body off
/// its reckoning) may come of the worlds that did not reach the bot, not of the bot. The longest gap between two
/// uploads of the J0 matches' healthy streams was 2.85 s; one missed regular world is 4 s.
pub const FIX_GAP_MS: u32 = 3_000;
/// The keys a bot stands still of when it is blind (`UnstickTuning::blind_stop_ms`).
pub const BLIND_KEYS: u32 = HELD_KEYS | key::SPRINT;
/// A world after a blind stop that shows the body this near its reckoning, metres, shows the stop was needless.
pub const BLIND_NEEDLESS_M: f32 = 1.0;
/// A stuck spot this near an earlier one is the same spot, metres.
pub const SPOT_M: f32 = 2.0;
/// … if the earlier one was within this, ms.
pub const AGAIN_MS: u32 = 20_000;
/// A spot this far over or under a point is on another floor, not near it,
/// metres. GUESS: over a storey's step of the grid's walk (0.6 m) and a
/// ramp's rise within [`SPOT_M`], under a walkway's height over the floor
/// under it (the falls off a route in a bench came down 2.5 m at the
/// median, a third of them less than 1.5 m).
pub const SPOT_HEIGHT_M: f32 = 2.0;
/// A spot is avoided this long, ms.
pub const SPOT_KEPT_MS: u32 = 45_000;
/// The landing of a jump the motor gave up shuts to the body's searches for
/// this long, ms: the motor refuses that jump for as long
/// (`motor::TRY_KEPT_S`), so a route over it would end on its first tick,
/// every tick (the Rudnya probe: 146 times in 8 s).
pub const JUMP_SHUT_MS: u32 = (super::motor::TRY_KEPT_S * 1000.0) as u32;
/// A floor within this of a shut landing is shut too (across), metres: the
/// motor takes a jump 0.3 m from the one it gave up as the same one.
pub const LANDING_SHUT_M: f32 = 0.35;
/// … and within this up or down, metres.
const LANDING_SHUT_UP_M: f32 = 0.3;
/// A spot is avoided once it caught the body this many times.
const AVOID_AFTER: u32 = 2;
/// A route passing this near a kept spot is not taken (its first metres,
/// where the body is now, are not looked at), metres.
const ROUTE_CLEAR_M: f32 = 1.2;
const ROUTE_FROM_M: f32 = SPOT_M + 0.5;
/// A team spot is kept clear of once its bots were caught there this many
/// times, whichever bots ([`TeamSpots`]). GUESS: as a bot's own
/// ([`AVOID_AFTER`]).
pub const TEAM_AVOID_AFTER: u32 = 2;
/// The spots a team keeps at most; the least lately hit go first. GUESS:
/// the Cologne match had 9 spots hit twice or more; a route's check costs
/// a test a spot a floor.
pub const TEAM_SPOTS_MAX: usize = 64;
/// A team spot this near the goal is not kept clear of: a block at a
/// container's put-in stand or a battery's take stand never shuts the goal,
/// metres. GUESS: past [`SPOT_M`] and a use's 2 m reach.
pub const GOAL_FREE_M: f32 = 3.0;
/// A way round the spots is taken when it is at most this many times the
/// way through them…
pub const ROUND_FACTOR: f32 = 1.5;
/// … plus this, metres; else the bot goes through. GUESS: on Cologne's
/// grid the way to team 0's container round its pit's stair edge was 0.1-2.7
/// m longer; a way round a whole building is not worth a spot.
pub const ROUND_EXTRA_M: f32 = 10.0;
/// A second way out of the same zones within this, toward the same goal,
/// gives that goal up ([`Stuck::zone_escape`]), ms. GUESS: two of
/// level_03's electric pairs' on spans (6 s on, 6 s off) and the walk back.
pub const ZONE_AGAIN_MS: u32 = 30_000;
/// The goals of two ways out are the same within this, metres (a goal that
/// moved less is planned to again, `mod.rs` `REPLAN_MOVED_M`).
const ZONE_GOAL_M: f32 = 2.0;
/// A held route is looked over for the zones ahead at most this often, ms
/// (`Tactics::route_shut_ahead`). GUESS: a tenth of a zone's on or off span;
/// a route dropped there is searched again at once.
pub const ZONE_CHECK_MS: u32 = 500;
/// A way out of a zone a bot with no goal walks is kept this long, ms,
/// before it is looked for again. GUESS: a few metres at a walk.
pub const ZONE_OUT_KEPT_MS: u32 = 2_000;

/// The ladder's last step ([`Recovery::GiveUp`]) taken this many times at one
/// spot is nothing left to try ([`Stuck::hopeless`]): BackOff and Sidestep,
/// then three GiveUps.
pub const HOPELESS_GIVE_UPS: u32 = 3;
const HOPELESS_TIMES: u32 = 2 + HOPELESS_GIVE_UPS;
/// A fall of this much or more, metres (a landing at 10.8 m/s: the first that hurts a leg), leaves the body unable to
/// move for [`FALL_LOCK_MS`]: in the J0 worlds the bodies that landed from 6.2-6.9 m stood 6.5-8.0 s at speed 0 with
/// the walk keys held and the stuck unsticking keys too (match 2a2e5b76 bots 5 and 11 at the foot of the g2 ledge,
/// 8.0 and 8.5 s; 12939af6 bot 3, 7.6 s), then walked on; the falls of 5.2 m and less stood 0-2 s.
pub const HARD_FALL_M: f32 = 6.0;
/// The lock of a hard fall, ms (a broken leg is 8 s, `motor::Hurt`).
pub const FALL_LOCK_MS: u32 = 8_500;
/// The heights of the worlds are kept this long to see a fall, ms.
const FALL_WINDOW_MS: u32 = 2_000;

/// A bot with nothing left to try (`Stuck::hopeless`): since when, and why.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hopeless {
    pub since: u32,
    pub why: HopelessWhy,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HopelessWhy {
    /// Every exit of its pocket tried (`escape.rs`).
    Pocket,
    /// Caught again and again at this spot.
    Ladder(Vec3),
}

/// What a stuck bot does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recovery {
    BackOff,
    Sidestep,
    GiveUp,
}

#[derive(Clone, Debug, Default)]
pub struct Stuck {
    /// The last world's feet and time while walking, and how far its keys
    /// meant to move it since (none: not known).
    walked_from: Option<(u32, Vec3, Option<f32>)>,
    /// How far its keys meant to move it over the last window the reckoned watch checked (none: not known).
    meant_checked: Option<f32>,
    /// The first of the worlds since that showed the walk keys held and
    /// the body near where it was then: time and feet ([`HELD_AFTER_MS`]).
    held_from: Option<(u32, Vec3)>,
    /// Where it got stuck, the last time, and how often there.
    spots: Vec<Spot>,
    /// How often at the spot of the last time.
    last_times: u32,
    /// Its stucks and falls since the team last heard ([`Self::tell`]):
    /// where, and whether a fall.
    told: Vec<(Vec3, bool)>,
    /// The team's spots to keep clear of, as last shared ([`Self::share`]).
    team: Vec<Vec3>,
    /// Its ways out of zones lately: the zones' set, the goal, the time,
    /// and whether it was off those zones' floors since
    /// ([`Self::zone_escape`], [`Self::zone_on`]).
    zone_escapes: Vec<(ZoneSet, Vec3, u32, bool)>,
    /// When its held route was last looked over for zones
    /// ([`ZONE_CHECK_MS`]).
    pub zone_checked: u32,
    /// The way out of a zone it walks with no goal of its own: where to,
    /// and till when it is kept ([`ZONE_OUT_KEPT_MS`]).
    pub zone_out: Option<(Vec3, u32)>,
    /// Its escape from the pocket it is in (`escape.rs`).
    pub escape: Option<super::escape::Escape>,
    /// The landings of the jumps the motor gave up (`motor::JUMP_TRIES`), and
    /// till when every search of this body keeps off them
    /// ([`JUMP_SHUT_MS`]).
    shut_landings: Vec<(Vec3, u32)>,
    /// The worlds' heights of the last [`FALL_WINDOW_MS`] (time, y), for [`Self::track_fall`].
    heights: Vec<(u32, f32)>,
    /// A hard fall locks the body till this time ([`HARD_FALL_M`]): it is not stuck while it cannot move.
    fall_lock_until: u32,
    /// The progress watch's window ([`Self::no_progress`]).
    progress: Option<ProgressWindow>,
}

/// The window of the progress watch: since when, how far along the route the body was then, and which route (its goal and length).
#[derive(Clone, Copy, Debug)]
struct ProgressWindow {
    since: u32,
    from_s: f32,
    goal: Vec3,
    length: f32,
}

#[derive(Clone, Copy, Debug)]
struct Spot {
    at: Vec3,
    /// The first time the body was caught here in this run of stucks
    /// ([`Stuck::hopeless`]).
    first: u32,
    last: u32,
    times: u32,
    /// A body fell off its route from here ([`Stuck::fell`]).
    fell: bool,
}

impl Stuck {
    /// A world showed the body at `feet`: after a fall of [`HARD_FALL_M`] or more within [`FALL_WINDOW_MS`] that has
    /// landed (the height is no longer dropping), the body is locked for [`FALL_LOCK_MS`] ([`Self::walking`] says
    /// nothing meanwhile). `on`: the switch (`think.fall_lock`); off, nothing is kept.
    pub fn track_fall(&mut self, time: u32, feet: Vec3, on: bool) {
        if !on {
            return;
        }
        self.heights.retain(|&(at, _)| at + FALL_WINDOW_MS > time);
        let dropped = self.heights.iter().map(|&(_, y)| y).fold(f32::MIN, f32::max) - feet.y;
        let landed = self.heights.last().is_some_and(|&(_, y)| feet.y > y - 0.05);
        if dropped >= HARD_FALL_M && landed {
            self.fall_lock_until = time + FALL_LOCK_MS;
            self.heights.clear();
            #[cfg(test)]
            fall_locks::locked();
        }
        self.heights.push((time, feet.y));
    }

    /// It stopped walking (a new route, a pause, a death): the next world
    /// starts the watch again.
    pub fn reset(&mut self) {
        self.walked_from = None;
        self.held_from = None;
        self.progress = None;
    }

    /// The progress watch: a world showed the body `s` metres along its route (to `goal`, `length` long) with `keys` held. True when
    /// it held a walk key for `window_ms` without gaining [`PROGRESS_M`] and is not at the route's end; then the leg ahead (`leg`,
    /// the route's point [`PROGRESS_LEG_M`] on) is kept clear of ([`Self::avoided`]) and the watch starts again. A new route (another
    /// goal or length), keys let go, or a gain of [`PROGRESS_M`] starts the window again.
    #[allow(clippy::too_many_arguments)]
    pub fn no_progress(&mut self, time: u32, s: f32, keys: u32, goal: Vec3, length: f32, window_ms: u32, leg: Vec3) -> bool {
        let fresh = ProgressWindow { since: time, from_s: s, goal, length };
        let Some(window) = self.progress.filter(|w| w.goal == goal && (w.length - length).abs() < 0.05) else {
            self.progress = Some(fresh);
            return false;
        };
        if keys & HELD_KEYS == 0 || s >= window.from_s + PROGRESS_M {
            self.progress = Some(fresh);
            return false;
        }
        if time < window.since + window_ms {
            return false;
        }
        self.progress = Some(fresh);
        if length - window.from_s < PROGRESS_END_M {
            return false;
        }
        #[cfg(test)]
        {
            let again = self.spots.iter().any(|spot| spot.last + AGAIN_MS > time && near(spot.at, leg));
            progress_watch::count(usize::from(again));
        }
        match self.spots.iter_mut().find(|spot| spot.last + SPOT_KEPT_MS > time && near(spot.at, leg)) {
            Some(spot) => {
                spot.last = time;
                spot.times = spot.times.max(AVOID_AFTER);
            }
            None => self.spots.push(Spot { at: leg, first: time, last: time, times: AVOID_AFTER, fell: false }),
        }
        self.told.push((leg, false));
        true
    }

    /// A world showed the body at `feet` while it had a route since the
    /// last one, and its keys meant to move it `meant` metres between the
    /// two worlds (none: not known, as at a life's first world; then it is
    /// taken as walking); `keys`: the keys the world shows held. `Some`
    /// when it is stuck: what to do about it, and whether the held keys
    /// alone told ([`HELD_AFTER_MS`]; the reckoning meant no walk).
    pub fn walking(
        &mut self,
        time: u32,
        feet: Vec3,
        meant: Option<f32>,
        keys: u32,
    ) -> Option<(Recovery, bool)> {
        if time < self.fall_lock_until {
            // Landed hard: the body cannot move yet, whatever the keys; not stuck, nothing to back off from.
            #[cfg(test)]
            fall_locks::paused();
            self.reset();
            return None;
        }
        let by_keys = self.held(time, feet, keys);
        let by_reckoning = self.reckoned(time, feet, meant);
        if !(by_keys || by_reckoning) {
            return None;
        }
        self.spots.retain(|spot| spot.last + SPOT_KEPT_MS > time);
        self.walked_from = None;
        self.held_from = None;
        let times = match self
            .spots
            .iter_mut()
            .find(|spot| spot.last + AGAIN_MS > time && near(spot.at, feet))
        {
            Some(spot) => {
                spot.last = time;
                spot.times += 1;
                spot.times
            }
            None => {
                self.spots.push(Spot {
                    at: feet,
                    first: time,
                    last: time,
                    times: 1,
                    fell: false,
                });
                1
            }
        };
        self.last_times = times;
        self.told.push((feet, false));
        let recovery = match times {
            1 => Recovery::BackOff,
            2 => Recovery::Sidestep,
            _ => Recovery::GiveUp,
        };
        Some((recovery, by_keys && !by_reckoning))
    }

    /// For a note of a stuck body the reckoned watch told of: how far its keys meant to move it over the window
    /// (none: not known, taken as walking).
    pub fn meant_checked(&self) -> Option<f32> {
        self.meant_checked
    }

    /// The reckoned watch: worlds [`STUCK_AFTER_MS`] apart, the body moved
    /// under [`STUCK_MOVED_M`] while the keys meant [`MEANT_M`].
    fn reckoned(&mut self, time: u32, feet: Vec3, meant: Option<f32>) -> bool {
        let Some((at, from, meant_since)) = self.walked_from else {
            self.walked_from = Some((time, feet, Some(0.0)));
            return false;
        };
        let meant_since = meant_since.zip(meant).map(|(a, b)| a + b);
        if time < at + STUCK_AFTER_MS {
            self.walked_from = Some((at, from, meant_since));
            return false;
        }
        self.walked_from = Some((time, feet, Some(0.0)));
        self.meant_checked = meant_since;
        from.distance_xz(feet) < STUCK_MOVED_M && meant_since.is_none_or(|m| m >= MEANT_M)
    }

    /// The held-keys watch: every world for [`HELD_AFTER_MS`] with a walk
    /// key held, the body within [`HELD_MOVED_M`] of the first (in all
    /// three axes: a fall is a move).
    fn held(&mut self, time: u32, feet: Vec3, keys: u32) -> bool {
        if keys & HELD_KEYS == 0 {
            self.held_from = None;
            return false;
        }
        match self.held_from {
            Some((at, from)) if (feet - from).length() < HELD_MOVED_M => time >= at + HELD_AFTER_MS,
            _ => {
                self.held_from = Some((time, feet));
                false
            }
        }
    }

    /// The keys that take it off the spot it is stuck at the `times`th
    /// time ([`Self::walking`] just said so): back and to `side` first,
    /// then the other side, then straight to each side (a body caught in a
    /// corner between two props gets out the way it did not try).
    pub fn escape(&self, side: u32, other: u32) -> u32 {
        self.escapes(side, other)[0]
    }

    /// [`Self::escape`]'s keys first, then the others in their turn: for a
    /// way that does not step into a zone (`Tactics::zone_free_step`).
    pub fn escapes(&self, side: u32, other: u32) -> [u32; 4] {
        let order = [side, key::BACK | side, key::BACK | other, other];
        std::array::from_fn(|k| order[(self.last_times as usize + k) % 4])
    }

    /// A way out of the zones of set `set` (`Hazards::set_of`) toward
    /// `goal`, at `time`: whether it is the second out of those zones
    /// toward that goal within [`ZONE_AGAIN_MS`], with the body off their
    /// floors in between (another way out in one stay is not counted).
    /// Then the way there goes in and out again (a zone between it and its
    /// goal that keeps coming on as it gets there), and the goal is given
    /// up for a while (`Bot::no_way_to`) for the play to choose again. A
    /// zone the body left is not held as on meanwhile: the Cologne anomaly
    /// bench's 15 s memory shut the only way's open windows (run 8: 11
    /// back-outs, 79.5 s to the goal against 41.7 s).
    pub fn zone_escape(&mut self, time: u32, set: ZoneSet, goal: Vec3) -> bool {
        self.zone_escapes
            .retain(|&(_, _, at, _)| time < at.saturating_add(ZONE_AGAIN_MS));
        let same =
            |&(s, g, _, _): &(ZoneSet, Vec3, u32, bool)| s == set && g.distance_xz(goal) <= ZONE_GOAL_M;
        let again = self.zone_escapes.iter().any(|e| same(e) && e.3);
        match self.zone_escapes.iter_mut().find(|e| same(e) && !e.3) {
            Some(stay) => stay.2 = time,
            None => self.zone_escapes.push((set, goal, time, false)),
        }
        again
    }

    /// The body is on a floor of the zones of set `set` (0: none): the
    /// ways out of other zones' floors are left ([`Self::zone_escape`]).
    pub fn zone_on(&mut self, set: ZoneSet) {
        for escape in self.zone_escapes.iter_mut().filter(|e| e.0 != set) {
            escape.3 = true;
        }
    }

    /// A fall off its route from `at` (the route's floor it fell from,
    /// [`Route::fell_from`]): the spot there is avoided from the first
    /// fall (where it is the only way, the bot takes it anyway), and is the
    /// same spot while it is kept (a way back up to it takes a while).
    pub fn fell(&mut self, time: u32, at: Vec3) {
        self.told.push((at, true));
        self.spots.retain(|spot| spot.last + SPOT_KEPT_MS > time);
        match self.spots.iter_mut().find(|spot| near(spot.at, at)) {
            Some(spot) => {
                spot.last = time;
                spot.times += 1;
                spot.fell = true;
            }
            None => self.spots.push(Spot {
                at,
                first: time,
                last: time,
                times: 1,
                fell: true,
            }),
        }
    }

    /// Whether it has nothing left to try, and since when: in a pocket
    /// whose every exit it tried ([`super::escape::Trapped::gave_up`]), or
    /// caught at one spot [`HOPELESS_GIVE_UPS`] times in a row (the ladder
    /// at its last step each time, each within [`AGAIN_MS`] of the one
    /// before). `since` is the first stuck of the run, or when it found itself
    /// in the pocket; a pocket of an earlier life (before `life_from`) is not
    /// this one's.
    pub fn hopeless(&self, now: u32, life_from: u32) -> Option<Hopeless> {
        let pocket = self
            .escape
            .as_ref()
            .map(super::escape::Escape::trapped)
            .filter(|trapped| trapped.gave_up && trapped.since >= life_from)
            .map(|trapped| Hopeless {
                since: trapped.since,
                why: HopelessWhy::Pocket,
            });
        let ladder = self
            .spots
            .iter()
            .filter(|spot| {
                !spot.fell
                    && spot.first >= life_from
                    && spot.times >= HOPELESS_TIMES
                    && spot.last + AGAIN_MS > now
            })
            .map(|spot| Hopeless {
                since: spot.first,
                why: HopelessWhy::Ladder(spot.at),
            })
            .min_by_key(|hopeless| hopeless.since);
        [pocket, ladder].into_iter().flatten().min_by_key(|h| h.since)
    }

    /// A jump of its route from `at` (the take-off) that did not get the
    /// body up in its tries (`motor::JUMP_TRIES`): the spot is avoided from
    /// the first time, as a fall's ([`Self::fell`]; where it is the only
    /// way, the bot takes it anyway). The grid's jumps of 0.9-1.05 m are at
    /// the client's reach: some it never makes (`player_bots_stuck.md`
    /// §8.9).
    ///
    /// Its `landing` (when the route says) is also shut to every search of
    /// this body for [`JUMP_SHUT_MS`] ([`Self::shut_landings`]): the motor
    /// will not make that jump again before then, and a search that found
    /// no other way was handed the same route every tick.
    pub fn failed_jump(&mut self, time: u32, at: Vec3, landing: Option<Vec3>) {
        self.fell(time, at);
        if let Some(landing) = landing {
            self.shut_landings.retain(|&(_, until)| until > time);
            match self
                .shut_landings
                .iter_mut()
                .find(|(shut, _)| on_landing(*shut, landing))
            {
                Some((_, until)) => *until = time + JUMP_SHUT_MS,
                None => self.shut_landings.push((landing, time + JUMP_SHUT_MS)),
            }
        }
    }

    /// The landings of the jumps given up that every search keeps off at
    /// `time`.
    pub fn shut_landings(&self, time: u32) -> impl Iterator<Item = Vec3> + '_ {
        self.shut_landings
            .iter()
            .filter(move |&&(_, until)| until > time)
            .map(|&(landing, _)| landing)
    }

    /// Its stucks and falls since the team last heard, for [`TeamSpots::learn`].
    pub fn tell(&mut self) -> Vec<(Vec3, bool)> {
        std::mem::take(&mut self.told)
    }

    /// The team's spots to keep clear of ([`TeamSpots::avoided`]).
    pub fn share(&mut self, team: Vec<Vec3>) {
        self.team = team;
    }

    /// The spots to keep clear of at `time` on the way to `goal`: its own
    /// ([`Self::avoided`]) and the team's, but those within [`GOAL_FREE_M`]
    /// of the goal.
    pub fn avoided_for(&self, time: u32, goal: Vec3) -> impl Iterator<Item = Vec3> + '_ {
        self.avoided(time)
            .chain(self.team.iter().copied().filter(move |spot| {
                !(spot.distance_xz(goal) < GOAL_FREE_M && (spot.y - goal.y).abs() < SPOT_HEIGHT_M)
            }))
    }

    /// The spots to keep clear of at `time`.
    pub fn avoided(&self, time: u32) -> impl Iterator<Item = Vec3> + '_ {
        self.spots
            .iter()
            .filter(move |spot| {
                (spot.fell || spot.times >= AVOID_AFTER) && spot.last + SPOT_KEPT_MS > time
            })
            .map(|spot| spot.at)
    }

    /// Whether there is any spot to keep clear of at `time`.
    pub fn avoids_any(&self, time: u32) -> bool {
        self.avoided(time).next().is_some()
    }

    /// Whether `point` is near a spot to keep clear of.
    pub fn near_spot(&self, time: u32, point: Vec3) -> bool {
        self.avoided(time).any(|spot| near(spot, point))
    }

    /// Whether a route keeps clear of the spots (past its first metres).
    pub fn clear(&self, time: u32, route: &Route) -> bool {
        let spots: Vec<Vec3> = self.avoided(time).collect();
        if spots.is_empty() {
            return true;
        }
        let mut s = ROUTE_FROM_M;
        while s < route.length() {
            let point = route.floor_point(s);
            if spots.iter().any(|spot| {
                spot.distance_xz(point) < ROUTE_CLEAR_M && (spot.y - point.y).abs() < SPOT_HEIGHT_M
            }) {
                return false;
            }
            s += 0.5;
        }
        true
    }
}

/// The keys of one stuck recovery: a forward jump up the ledge the body is
/// held by (`Tactics::ledge_ahead`), else the escape keys. The first stop at
/// a spot backs off with no jump, the second keeps the old back-diagonal
/// jump (`jump` is `key::JUMP` when the body may jump, else 0), the third
/// none.
pub fn unstick_keys(recovery: Recovery, ledge: bool, escape: u32, jump: u32) -> u32 {
    match (recovery, ledge) {
        (Recovery::BackOff | Recovery::Sidestep, true) => key::FORWARD | key::JUMP,
        (Recovery::BackOff | Recovery::GiveUp, _) => escape,
        (Recovery::Sidestep, false) => escape | jump,
    }
}

/// Whether `point` is on the shut landing `landing`.
pub fn on_landing(landing: Vec3, point: Vec3) -> bool {
    landing.distance_xz(point) < LANDING_SHUT_M && (landing.y - point.y).abs() < LANDING_SHUT_UP_M
}

/// Whether `point` is near the spot `spot`: within [`SPOT_M`] across and
/// [`SPOT_HEIGHT_M`] up or down.
pub fn near(spot: Vec3, point: Vec3) -> bool {
    spot.distance_xz(point) < SPOT_M && (spot.y - point.y).abs() < SPOT_HEIGHT_M
}

/// Whether a way round the spots (`round`, of `round_m`) may be longer than
/// [`ROUND_FACTOR`] × the way through plus [`ROUND_EXTRA_M`]: only then is
/// the way through searched to compare ([`worth_round`]); the way through
/// is no shorter than the straight line from `from` to `to`.
pub fn may_be_long(round_m: f32, from: Vec3, to: Vec3) -> bool {
    round_m > (to - from).length() * ROUND_FACTOR + ROUND_EXTRA_M
}

/// Whether the way round the spots (`round_m`) is worth it against the way
/// through them (`through_m`).
pub fn worth_round(round_m: f32, through_m: f32) -> bool {
    round_m <= through_m * ROUND_FACTOR + ROUND_EXTRA_M
}

/// A team's spots ([`Stuck`]'s, told to it at its commander's ticks): kept
/// for the match.
#[derive(Clone, Debug, Default)]
pub struct TeamSpots {
    spots: Vec<TeamSpot>,
}

#[derive(Clone, Copy, Debug)]
struct TeamSpot {
    at: Vec3,
    last: u32,
    /// Its bots' stucks there, and whether one fell off its route from it.
    caught: u32,
    fell: bool,
}

impl TeamSpot {
    fn avoided(&self) -> bool {
        self.fell || self.caught >= TEAM_AVOID_AFTER
    }
}

impl TeamSpots {
    /// A bot's stucks and falls ([`Stuck::tell`]) at `time`.
    pub fn learn(&mut self, time: u32, told: &[(Vec3, bool)]) {
        for &(at, fell) in told {
            match self.spots.iter_mut().find(|spot| near(spot.at, at)) {
                Some(spot) => {
                    spot.last = time;
                    spot.caught += u32::from(!fell);
                    spot.fell |= fell;
                }
                None => {
                    if self.spots.len() >= TEAM_SPOTS_MAX
                        && let Some(oldest) = self
                            .spots
                            .iter()
                            .enumerate()
                            .min_by_key(|(_, spot)| (spot.avoided(), spot.last))
                            .map(|(i, _)| i)
                    {
                        self.spots.swap_remove(oldest);
                    }
                    self.spots.push(TeamSpot {
                        at,
                        last: time,
                        caught: u32::from(!fell),
                        fell,
                    });
                }
            }
        }
    }

    /// The spots its bots keep clear of.
    pub fn avoided(&self) -> Vec<Vec3> {
        self.spots
            .iter()
            .filter(|spot| spot.avoided())
            .map(|spot| spot.at)
            .collect()
    }
}

/// The last resort: a bot with nothing left to try ([`Stuck::hopeless`])
/// kills itself with the game's own suicide input (bit 24, the hit on one's
/// own head every game applies from the relayed input, PROVEN by 8 deaths of
/// 8 in the jump test's run), so it respawns instead of standing in its
/// pocket until the match ends. The owner's rule (2026-10-24, 26): only after
/// every fix and every try, 60 s, never in a fight, never carrying.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelfKill {
    /// Nothing sent in this life.
    #[default]
    Ready,
    /// The bit was sent at `at` (the `tries`th time, 1 or 2): the world must
    /// show the body dead by `at` + [`UnstickTuning::suicide_wait_ms`].
    Sent { at: u32, tries: u8 },
    /// Sent twice and the body lived: no more in this life.
    Barred,
}

/// What a bot's think knows, read once from the match (`think.rs`
/// `self_kill_facts`): nothing in the transition looks at anything else.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Facts {
    pub now: u32,
    /// Nothing left to try since, and what ([`Stuck::hopeless`]).
    pub hopeless: Option<Hopeless>,
    /// Since the bot last sensed an enemy or took a hit, ms (`u32::MAX`:
    /// not in this life).
    pub quiet_ms: u32,
    /// The life's and the match's age, ms.
    pub life_ms: u32,
    pub match_ms: u32,
    /// Whether it carries a battery or an artefact (it would drop it where
    /// it dies, in the pocket).
    pub carrying: bool,
    /// Its team's tickets in Team Deathmatch (a suicide costs one).
    pub tickets: Option<u32>,
    /// Since its last self-kill (any life), ms.
    pub since_last_ms: Option<u32>,
    /// Self-kills its team made in the last minute.
    pub team_last_minute: u32,
}

/// What the transition asks the bot to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelfKillAct {
    /// Send the suicide bit; `again`: the second time in the life (the
    /// first one's death was not shown).
    Send { again: bool },
}

impl Facts {
    /// Whether every rule lets it, [`UnstickTuning`]'s numbers. `again`
    /// (a second try in the life) is not counted against the bot's and the
    /// team's caps a second time.
    fn allowed(&self, rule: &UnstickTuning, again: bool) -> bool {
        rule.suicide_after_ms.is_some_and(|after| {
            self.hopeless
                .is_some_and(|h| self.now.saturating_sub(h.since) >= after)
        })
            && self.quiet_ms >= rule.suicide_quiet_ms
            && !self.carrying
            && self.life_ms >= rule.suicide_life_ms
            && self.match_ms >= rule.suicide_match_ms
            && self.tickets.is_none_or(|t| t > rule.suicide_min_tickets)
            && (again
                || (self.since_last_ms.is_none_or(|s| s >= rule.suicide_bot_gap_ms)
                    && self.team_last_minute < rule.suicide_team_per_min))
    }
}

/// One tick of a bot's self-kill: from the state and the facts to the new
/// state and what to do. Total over both.
pub fn self_kill_step(
    state: SelfKill,
    facts: &Facts,
    rule: &UnstickTuning,
) -> (SelfKill, Option<SelfKillAct>) {
    match state {
        SelfKill::Ready if facts.allowed(rule, false) => (
            SelfKill::Sent { at: facts.now, tries: 1 },
            Some(SelfKillAct::Send { again: false }),
        ),
        SelfKill::Ready | SelfKill::Barred => (state, None),
        SelfKill::Sent { at, .. } if facts.now < at.saturating_add(rule.suicide_wait_ms) => (state, None),
        SelfKill::Sent { tries: 1, .. } if facts.allowed(rule, true) => (
            SelfKill::Sent { at: facts.now, tries: 2 },
            Some(SelfKillAct::Send { again: true }),
        ),
        // The first did not show, and a fight or a carried battery has come
        // since: it may be asked again later.
        SelfKill::Sent { tries: 1, .. } => (SelfKill::Ready, None),
        SelfKill::Sent { .. } => (SelfKill::Barred, None),
    }
}

/// What a note's text says without the evidence tags the notes above end with (they are for the reader of the log,
/// not decisions: the bench's decision digest leaves them out).
pub fn without_evidence(text: &str) -> &str {
    [" [meant ", " [fix gap ", " [beside ", " [route "]
        .iter()
        .filter_map(|tag| text.find(tag))
        .min()
        .map_or(text, |at| &text[..at])
}

/// The note of how far a stuck body's keys meant to move it, when the reckoned watch told (not the held keys alone).
pub fn meant_note(by_keys: bool, meant: Option<f32>) -> String {
    if by_keys {
        return String::new();
    }
    meant.map_or_else(|| " [meant unknown]".to_owned(), |m| format!(" [meant {m:.1} m]"))
}

/// A body this near a stuck one, metres, is beside it: enemy bodies block each other in the game (the J0 matches'
/// enemies head-on on one route line, or at one start); friends pass through each other.
pub const BESIDE_M: f32 = 1.2;

/// The note of the body beside a stuck one, if one is within [`BESIDE_M`].
pub fn beside_note(beside: Option<super::view::Beside>) -> String {
    beside.filter(|b| b.distance_m <= BESIDE_M).map_or_else(String::new, |b| {
        format!(" [beside {} place {}, {:.1} m]", if b.enemy { "enemy" } else { "friend" }, b.place, b.distance_m)
    })
}

/// The note of where a stuck body was steering (A81 item 2: Chemical Plant's back and forth, where the bot
/// let go of its keys by its own decision): the route's leg and its next waypoint, how far the body's heading is
/// from it in degrees (positive to the left), and how many spots the bot keeps clear of. `[route none]` without a
/// route. For the reader of the log; a decision never reads it.
pub fn route_note(route: Option<&Route>, feet: Vec3, yaw: Option<f32>, kept: usize) -> String {
    let Some(route) = route else {
        return " [route none]".to_owned();
    };
    let leg = route.segment();
    let Some(next) = route.points().get(leg + 1).map(|point| point.at) else {
        return format!(" [route leg {leg}/{} at the end, {kept} kept]", route.points().len());
    };
    let off = yaw.map_or_else(
        || "?".to_owned(),
        |yaw| {
            let want = super::kin::yaw_of([next.x - feet.x, next.z - feet.z]);
            format!("{:+.0}", super::kin::wrap(want - yaw).to_degrees())
        },
    );
    format!(
        " [route leg {leg}/{} next {:.1},{:.1},{:.1} {:.1} m, heading off {off} deg, {kept} kept]",
        route.points().len(),
        next.x,
        next.y,
        next.z,
        feet.distance_xz(next)
    )
}

/// The note for a world that came `gap` ms after the one before it (none when the gap is not one).
pub fn gap_note(gap: Option<u32>) -> String {
    gap.filter(|&gap| gap >= FIX_GAP_MS)
        .map_or_else(String::new, |gap| format!(" [fix gap {gap} ms]"))
}

/// The bench's counts of the worlds that came [`FIX_GAP_MS`] or more after the one before: all, the stuck notes
/// after one, the off-reckoning notes after one, the stuck checks a tuned gap ended, and the stuck notes with an enemy, and with a friend, beside.
#[cfg(test)]
pub mod fix_gaps {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTS: [AtomicU64; 6] = [const { AtomicU64::new(0) }; 6];

    pub fn count(kind: usize) {
        COUNTS[kind].fetch_add(1, Ordering::Relaxed);
    }

    pub fn counts() -> [u64; 6] {
        std::array::from_fn(|k| COUNTS[k].load(Ordering::Relaxed))
    }
}

/// The bench's counts of the hard falls ([`Stuck::track_fall`]): bodies locked, and worlds the stuck watch kept quiet
/// for them.
#[cfg(test)]
pub mod fall_locks {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTS: [AtomicU64; 2] = [const { AtomicU64::new(0) }; 2];

    pub fn locked() {
        COUNTS[0].fetch_add(1, Ordering::Relaxed);
    }

    pub fn paused() {
        COUNTS[1].fetch_add(1, Ordering::Relaxed);
    }

    pub fn counts() -> [u64; 2] {
        std::array::from_fn(|k| COUNTS[k].load(Ordering::Relaxed))
    }
}

/// The bench's counts of the progress watch: bodies it stopped (activity), and those that came to the same leg again within
/// [`AGAIN_MS`] (harm: the watch did not take them off it).
#[cfg(test)]
pub mod progress_watch {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTS: [AtomicU64; 2] = [const { AtomicU64::new(0) }; 2];

    /// `0`: stopped one; `1`: stopped one that was at the same leg lately (also counted as stopped).
    pub fn count(again: usize) {
        COUNTS[0].fetch_add(1, Ordering::Relaxed);
        if again == 1 {
            COUNTS[1].fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn counts() -> [u64; 2] {
        std::array::from_fn(|k| COUNTS[k].load(Ordering::Relaxed))
    }
}

/// The bench's counts of the blind stops: ticks a bot stood for want of a world, stops a world ended, and those worlds
/// that showed the body within [`BLIND_NEEDLESS_M`] of its reckoning (the stop was needless).
#[cfg(test)]
pub mod blind {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTS: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];

    pub fn count(kind: usize) {
        COUNTS[kind].fetch_add(1, Ordering::Relaxed);
    }

    pub fn counts() -> [u64; 3] {
        std::array::from_fn(|k| COUNTS[k].load(Ordering::Relaxed))
    }
}

/// The bench's counts of the self-kills ([`SelfKillAct`]): sent, sent again,
/// barred after two.
#[cfg(test)]
pub mod self_kills {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTS: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];

    pub fn count(kind: usize) {
        COUNTS[kind].fetch_add(1, Ordering::Relaxed);
    }

    pub fn counts() -> [u64; 3] {
        std::array::from_fn(|k| COUNTS[k].load(Ordering::Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::path::Waypoint;

    /// A body that landed from a fall of 6 m or more cannot move for about 8 s (the J0 worlds): with `think.fall_lock`
    /// the walk-keys-held watch keeps quiet that long and says stuck only after it; off, it says so 3 s after the
    /// landing, as it did. A fall of 3 m locks nothing.
    #[test]
    fn a_body_that_landed_hard_is_not_stuck_while_it_cannot_move() {
        let first_stuck = |on: bool, from: f32| {
            let mut stuck = Stuck::default();
            for i in 0..100u32 {
                let time = 1_000 + i * 150;
                let y = match i {
                    0 => from,
                    1 => from - 2.5,
                    2 => from - 5.0,
                    _ => 3.0,
                };
                let feet = Vec3::new(0.0, y.max(3.0), 0.0);
                stuck.track_fall(time, feet, on);
                if stuck.walking(time, feet, Some(0.0), key::FORWARD).is_some() {
                    return Some(time - 1_000);
                }
            }
            None
        };
        let off = first_stuck(false, 9.5).expect("stuck after the landing");
        let on = first_stuck(true, 9.5).expect("stuck once the lock ends");
        assert!(off < 4_500 && on >= off + 5_000, "{off} {on}");
        let short = first_stuck(true, 6.0).expect("a 3 m fall locks nothing");
        assert!(short < 4_500, "{short}");
    }

    /// (iii): the landing of a jump the motor gave up is shut to the body's
    /// searches for as long as the motor refuses the jump, and a floor within
    /// the motor's "same jump" of it is shut with it; no other floor, nor
    /// after the lockout.
    /// A body that walks without gaining on its route is stopped after the window and the leg is kept clear of; a gain, keys let go,
    /// the route's end and a new route start the window again.
    #[test]
    fn a_body_that_walks_without_gaining_on_its_route_is_stopped() {
        let goal = Vec3::new(40.0, 0.0, 0.0);
        let leg = Vec3::new(11.0, 0.0, 0.0);
        let walk = |stuck: &mut Stuck, time: u32, s: f32, keys: u32, length: f32| stuck.no_progress(time, s, keys, goal, length, 6_000, leg);
        let mut stuck = Stuck::default();
        // Round a post: 10 to 10.6 m along, back and forth, keys held.
        let mut fired = None;
        for k in 0..20u32 {
            let s = 10.0 + 0.6 * (k % 2) as f32;
            if walk(&mut stuck, 1_000 + k * 500, s, key::FORWARD, 40.0) {
                fired = Some(1_000 + k * 500);
                break;
            }
        }
        assert_eq!(fired, Some(7_000), "six seconds after the window began");
        assert!(stuck.near_spot(7_000, leg), "the leg is kept clear of");
        // The window began again: nothing for six seconds more.
        assert!(!walk(&mut stuck, 7_500, 10.0, key::FORWARD, 40.0));
        // A gain of PROGRESS_M starts it again.
        let mut stuck = Stuck::default();
        assert!(!walk(&mut stuck, 1_000, 10.0, key::FORWARD, 40.0));
        assert!(!walk(&mut stuck, 5_000, 11.6, key::FORWARD, 40.0));
        assert!(!walk(&mut stuck, 7_000, 11.7, key::FORWARD, 40.0), "the window is 2 s old");
        assert!(walk(&mut stuck, 11_000, 11.8, key::FORWARD, 40.0));
        // Keys let go start it again.
        let mut stuck = Stuck::default();
        assert!(!walk(&mut stuck, 1_000, 10.0, key::FORWARD, 40.0));
        assert!(!walk(&mut stuck, 5_000, 10.0, 0, 40.0));
        assert!(!walk(&mut stuck, 7_500, 10.0, key::FORWARD, 40.0));
        // At the route's end holding keys is no stall.
        let mut stuck = Stuck::default();
        assert!(!walk(&mut stuck, 1_000, 39.0, key::FORWARD, 40.0));
        assert!(!walk(&mut stuck, 8_000, 39.0, key::FORWARD, 40.0));
        // Another route (its length) starts it again.
        let mut stuck = Stuck::default();
        assert!(!walk(&mut stuck, 1_000, 10.0, key::FORWARD, 40.0));
        assert!(!walk(&mut stuck, 8_000, 10.0, key::FORWARD, 52.0));
    }

    #[test]
    fn the_evidence_tags_of_a_note_are_cut_off() {
        let plain = "is stuck at [1.0, 0.0, 2.0]: BackOff";
        assert_eq!(without_evidence(plain), plain);
        let tagged = format!("{plain} [meant 3.2 m] [fix gap 7000 ms] [beside enemy place 1, 0.8 m] [route none]");
        assert_eq!(without_evidence(&tagged), plain);
        let off = "was 13.9 m off its reckoning; plans again to [2.2, 16.3, 105.7] [fix gap 4000 ms]";
        assert_eq!(without_evidence(off), "was 13.9 m off its reckoning; plans again to [2.2, 16.3, 105.7]");
    }

    #[test]
    fn a_failed_jumps_landing_is_shut_for_the_motors_lockout() {
        let (take_off, landing) = (Vec3::new(1.0, 0.0, 1.0), Vec3::new(1.0, 1.0, 1.5));
        let mut stuck = Stuck::default();
        assert_eq!(stuck.shut_landings(1_000).count(), 0);
        stuck.failed_jump(1_000, take_off, Some(landing));
        let shut: Vec<Vec3> = stuck.shut_landings(1_000 + JUMP_SHUT_MS - 1).collect();
        assert_eq!(shut, [landing]);
        assert!(on_landing(landing, landing));
        assert!(on_landing(landing, landing + Vec3::new(0.25, 0.1, 0.0)));
        assert!(!on_landing(landing, landing + Vec3::new(0.5, 0.0, 0.0)), "another cell");
        assert!(!on_landing(landing, landing + Vec3::new(0.0, 1.0, 0.0)), "another floor");
        assert_eq!(stuck.shut_landings(1_000 + JUMP_SHUT_MS).count(), 0, "the lockout ends");
        // The motor's lockout is the same number of seconds.
        assert_eq!(JUMP_SHUT_MS as f32 / 1000.0, super::super::motor::TRY_KEPT_S);
        // The spot is avoided as before; a jump with no landing on the route
        // shuts nothing.
        let mut other = Stuck::default();
        other.failed_jump(1_000, take_off, None);
        assert_eq!(other.shut_landings(1_000).count(), 0);
        assert_eq!(other.tell(), [(take_off, true)]);
    }

    /// F4: a forward jump up the ledge a stuck body is held by, else the
    /// escape keys: no jump on the first stop at a spot, the old
    /// back-diagonal jump on the second, none on the third.
    #[test]
    fn a_stop_at_a_ledge_jumps_forward_and_any_other_backs_off() {
        let (escape, jump) = (key::BACK | key::LEFT, key::JUMP);
        let forward_jump = key::FORWARD | key::JUMP;
        for recovery in [Recovery::BackOff, Recovery::Sidestep] {
            assert_eq!(unstick_keys(recovery, true, escape, jump), forward_jump);
        }
        assert_eq!(unstick_keys(Recovery::GiveUp, true, escape, jump), escape);
        assert_eq!(unstick_keys(Recovery::BackOff, false, escape, jump), escape);
        assert_eq!(
            unstick_keys(Recovery::Sidestep, false, escape, jump),
            escape | jump
        );
        assert_eq!(unstick_keys(Recovery::GiveUp, false, escape, jump), escape);
        // A body that may not jump (a hit's stagger) gets no jump at all.
        assert_eq!(unstick_keys(Recovery::Sidestep, false, escape, 0), escape);
    }

    /// [`Stuck::walking`] with no walk key shown held: the reckoned watch.
    fn walk(stuck: &mut Stuck, time: u32, feet: Vec3, meant: Option<f32>) -> Option<Recovery> {
        stuck
            .walking(time, feet, meant, 0)
            .map(|(recovery, _)| recovery)
    }

    /// A body whose walk keys the worlds show held, and that stays put,
    /// is stuck after [`HELD_AFTER_MS`] though its reckoning meant no
    /// walk (it walked into the same wall on the grid); not if a world
    /// shows it moved, or one shows no walk key.
    #[test]
    fn walk_keys_held_and_no_move_is_stuck_without_the_reckoning() {
        let spot = Vec3::new(10.0, 0.0, 10.0);
        let walking =
            |stuck: &mut Stuck, time, feet, keys| stuck.walking(time, feet, Some(0.0), keys);
        let mut stuck = Stuck::default();
        assert_eq!(walking(&mut stuck, 0, spot, key::FORWARD), None);
        assert_eq!(walking(&mut stuck, 1_000, spot, key::FORWARD), None);
        assert_eq!(walking(&mut stuck, 2_000, spot, key::FORWARD), None);
        assert_eq!(
            walking(
                &mut stuck,
                3_000,
                spot + Vec3::new(0.3, 0.0, 0.0),
                key::FORWARD | key::SPRINT
            ),
            Some((Recovery::BackOff, true))
        );
        // A world with no walk key starts it again.
        let mut stuck = Stuck::default();
        walking(&mut stuck, 0, spot, key::FORWARD);
        walking(&mut stuck, 1_000, spot, key::FIRE);
        assert_eq!(walking(&mut stuck, 3_000, spot, key::LEFT), None);
        assert_eq!(walking(&mut stuck, 4_000, spot, key::LEFT), None);
        assert!(walking(&mut stuck, 6_000, spot, key::LEFT).is_some());
        // A side step out and back between the worlds' ends is a move.
        let mut stuck = Stuck::default();
        walking(&mut stuck, 0, spot, key::LEFT);
        walking(
            &mut stuck,
            1_500,
            spot + Vec3::new(1.0, 0.0, 0.0),
            key::RIGHT,
        );
        assert_eq!(walking(&mut stuck, 3_000, spot, key::LEFT), None);
        // So is a fall.
        let mut stuck = Stuck::default();
        walking(&mut stuck, 0, spot + Vec3::new(0.0, 3.0, 0.0), key::FORWARD);
        assert_eq!(walking(&mut stuck, 3_000, spot, key::FORWARD), None);
        // 0x6da44bbb bot 4, 285.8-288.8 s: in a 1 m trench, forward held
        // (sprint in some worlds) and a jump up its side every second that
        // never got it up, the feet 0.08 m off at most; it stood 88 s with
        // no stuck note (`player_bots_stuck.md` §8.8).
        let mut stuck = Stuck::default();
        let worlds = [
            (285_800, Vec3::new(-48.95, 21.44, -82.03), key::FORWARD),
            (
                286_800,
                Vec3::new(-48.97, 21.44, -82.02),
                key::FORWARD | key::SPRINT,
            ),
            (287_800, Vec3::new(-48.97, 21.51, -82.02), key::FORWARD),
            (
                288_800,
                Vec3::new(-48.97, 21.44, -82.03),
                key::FORWARD | key::SPRINT,
            ),
        ];
        let told: Vec<_> = worlds
            .iter()
            .map(|&(time, feet, keys)| walking(&mut stuck, time, feet, keys))
            .collect();
        assert_eq!(told[..3], [None, None, None]);
        assert_eq!(told[3], Some((Recovery::BackOff, true)));
    }

    #[test]
    fn a_body_that_does_not_move_is_stuck_and_each_time_more_is_done() {
        let mut stuck = Stuck::default();
        let spot = Vec3::new(10.0, 0.0, 10.0);
        // Walking along: not stuck.
        assert_eq!(
            walk(&mut stuck, 1_000, Vec3::new(0.0, 0.0, 0.0), None),
            None
        );
        assert_eq!(
            walk(&mut stuck, 2_000, Vec3::new(3.0, 0.0, 0.0), None),
            None,
            "too soon to tell"
        );
        assert_eq!(
            walk(&mut stuck, 2_600, Vec3::new(5.0, 0.0, 0.0), None),
            None
        );
        // Caught: two worlds 1.5 s apart, 0.2 m between them.
        stuck.reset();
        assert_eq!(walk(&mut stuck, 10_000, spot, None), None);
        assert_eq!(
            walk(&mut stuck, 11_600, Vec3::new(10.2, 0.0, 10.0), None),
            Some(Recovery::BackOff)
        );
        // Again there: more each time.
        assert_eq!(walk(&mut stuck, 14_000, spot, None), None);
        assert_eq!(
            walk(&mut stuck, 15_500, spot, None),
            Some(Recovery::Sidestep)
        );
        assert_eq!(walk(&mut stuck, 17_000, spot, None), None);
        assert_eq!(walk(&mut stuck, 18_500, spot, None), Some(Recovery::GiveUp));
        // Elsewhere, or long after: a first time again.
        assert_eq!(walk(&mut stuck, 19_000, Vec3::ZERO, None), None);
        assert_eq!(
            walk(&mut stuck, 20_500, Vec3::ZERO, None),
            Some(Recovery::BackOff)
        );
        assert!(stuck.near_spot(30_000, spot), "caught there three times");
        assert!(!stuck.near_spot(30_000, Vec3::ZERO), "caught there once");
        assert!(!stuck.near_spot(18_500 + SPOT_KEPT_MS, spot));
    }

    /// The stuck note says where the body was steering: the leg, the next
    /// waypoint, how far the heading is from it, and how many spots are kept.
    #[test]
    fn the_route_tail_of_a_stuck_note_says_where_the_body_steered() {
        let route = Route::new(
            [(0.0, 0.0), (20.0, 0.0)]
                .iter()
                .map(|&(x, z)| Waypoint {
                    at: Vec3::new(x, 0.0, z),
                    node: 0,
                    jump: None,
                    low: false,
                })
                .collect(),
        );
        let feet = Vec3::ZERO;
        let towards = crate::v026g::bots::kin::yaw_of([1.0, 0.0]);
        assert_eq!(
            route_note(Some(&route), feet, Some(towards), 2),
            " [route leg 0/2 next 20.0,0.0,0.0 20.0 m, heading off +0 deg, 2 kept]"
        );
        assert!(route_note(Some(&route), feet, Some(0.0), 0).contains("heading off -90 deg"));
        assert!(route_note(Some(&route), feet, None, 0).contains("heading off ? deg"));
        assert_eq!(route_note(None, feet, None, 0), " [route none]");
        assert_eq!(without_evidence("is stuck at [1.0]: BackOff [route none]"), "is stuck at [1.0]: BackOff");
    }

    #[test]
    fn a_route_through_a_spot_is_not_clear() {
        let mut stuck = Stuck::default();
        let spot = Vec3::new(10.0, 0.0, 0.3);
        for time in [0, 1_500, 3_000, 4_500] {
            walk(&mut stuck, time, spot, None);
        }
        let route = |to: (f32, f32)| {
            Route::new(
                [(0.0, 0.0), to]
                    .iter()
                    .map(|&(x, z)| Waypoint {
                        at: Vec3::new(x, 0.0, z),
                        node: 0,
                        jump: None,
                        low: false,
                    })
                    .collect(),
            )
        };
        assert!(!stuck.clear(5_000, &route((20.0, 0.0))));
        assert!(stuck.clear(5_000, &route((0.0, 20.0))));
        assert!(stuck.clear(4_500 + SPOT_KEPT_MS, &route((20.0, 0.0))));
    }

    /// A fall from a spot of a walkway: it is avoided there, not on the
    /// floor under it, and a route along the walkway is not clear; a
    /// second fall there keeps it longer.
    #[test]
    fn a_spot_fallen_from_is_avoided_on_its_floor() {
        let mut stuck = Stuck::default();
        let spot = Vec3::new(10.0, 3.0, 0.3);
        stuck.fell(0, spot);
        assert!(stuck.near_spot(1_000, spot));
        stuck.fell(30_000, spot + Vec3::new(0.5, 0.0, 0.0));
        assert!(stuck.near_spot(SPOT_KEPT_MS + 1_000, spot));
        assert!(
            !stuck.near_spot(31_000, Vec3::new(10.0, 0.0, 0.3)),
            "the floor under"
        );
        let route = |y: f32| {
            Route::new(
                [0.0, 20.0]
                    .iter()
                    .map(|&x| Waypoint {
                        at: Vec3::new(x, y, 0.0),
                        node: 0,
                        jump: None,
                        low: false,
                    })
                    .collect(),
            )
        };
        assert!(!stuck.clear(31_000, &route(3.0)));
        assert!(stuck.clear(31_000, &route(0.0)));
        assert!(!stuck.near_spot(30_000 + SPOT_KEPT_MS, spot));
    }

    /// A body that did not move while its keys meant to move it nowhere
    /// (standing to aim, waiting with a route in hand) is not stuck; one
    /// whose keys meant a walk is.
    #[test]
    fn only_a_body_that_meant_to_walk_is_stuck() {
        let mut stuck = Stuck::default();
        let spot = Vec3::new(10.0, 0.0, 10.0);
        assert_eq!(walk(&mut stuck, 0, spot, None), None);
        assert_eq!(walk(&mut stuck, 800, spot, Some(0.2)), None);
        assert_eq!(
            walk(&mut stuck, 1_600, spot, Some(0.3)),
            None,
            "meant 0.5 m"
        );
        assert_eq!(walk(&mut stuck, 2_400, spot, Some(1.5)), None, "too soon");
        assert_eq!(
            walk(&mut stuck, 3_200, spot, Some(1.5)),
            Some(Recovery::BackOff),
            "meant 3 m"
        );
    }

    /// Each time again at the spot it backs off another way.
    #[test]
    fn each_time_at_a_spot_it_gets_off_another_way() {
        let mut stuck = Stuck::default();
        let spot = Vec3::new(10.0, 0.0, 10.0);
        let mut ways = Vec::new();
        let mut time = 0;
        for _ in 0..4 {
            walk(&mut stuck, time, spot, None);
            time += STUCK_AFTER_MS;
            assert!(walk(&mut stuck, time, spot, None).is_some());
            ways.push(stuck.escape(key::LEFT, key::RIGHT));
            time += 100;
        }
        assert_eq!(
            ways,
            [
                key::BACK | key::LEFT,
                key::BACK | key::RIGHT,
                key::RIGHT,
                key::LEFT
            ]
        );
    }

    /// Spot B of 0x60aeff4f (Cologne, (-15.0, 35.1, 37.5)): bots 1, 5, 11,
    /// 12, 13 and 14 caught there 14 times, each learning it alone. Two
    /// bots' stucks there make every bot of the team keep clear of it for
    /// the match; one does not.
    #[test]
    fn a_spot_two_bots_stuck_at_is_kept_clear_of_by_the_team() {
        let spot_b = Vec3::new(-15.0, 35.1, 37.5);
        let goal = Vec3::new(40.0, 33.0, 30.0);
        let mut team = TeamSpots::default();
        let mut first = Stuck::default();
        first.told.push((spot_b, false));
        team.learn(724_000, &first.tell());
        assert!(first.tell().is_empty(), "told once");
        let mut third = Stuck::default();
        third.share(team.avoided());
        assert_eq!(third.avoided_for(730_000, goal).count(), 0, "caught once");
        let mut second = Stuck::default();
        second
            .told
            .push((spot_b + Vec3::new(0.6, 0.0, -0.4), false));
        team.learn(740_000, &second.tell());
        third.share(team.avoided());
        let spots: Vec<Vec3> = third.avoided_for(860_000, goal).collect();
        assert_eq!(spots, [spot_b]);
        // Kept for the match, past a bot's own keep.
        assert!(third.avoided(860_000 + SPOT_KEPT_MS).next().is_none());
        assert_eq!(third.avoided_for(860_000 + SPOT_KEPT_MS, goal).count(), 1);
        // On another floor, or as the goal itself: not kept clear of.
        assert!(!near(spot_b, spot_b + Vec3::new(0.0, -4.0, 0.0)));
        assert_eq!(
            third
                .avoided_for(860_000, spot_b + Vec3::new(1.5, 0.0, 1.0))
                .count(),
            0
        );
    }

    /// 0x60aeff4f: bots 9 and 5 (team 0) walked off pit A's stair 4 s
    /// apart, at (-53.0, 34.9, 16.2): one fall makes the spot the team's.
    #[test]
    fn a_fall_is_kept_clear_of_by_the_team_at_once() {
        let edge = Vec3::new(-53.0, 34.9, 16.2);
        let mut team = TeamSpots::default();
        let mut nine = Stuck::default();
        nine.fell(116_400, edge);
        team.learn(116_500, &nine.tell());
        let mut five = Stuck::default();
        five.share(team.avoided());
        assert_eq!(
            five.avoided_for(117_000, Vec3::new(-53.0, 37.1, 25.8))
                .collect::<Vec<_>>(),
            [edge]
        );
        // A failed jump is a fall's spot too.
        let mut team = TeamSpots::default();
        let mut jumper = Stuck::default();
        jumper.failed_jump(1_000, edge, None);
        team.learn(1_000, &jumper.tell());
        assert_eq!(team.avoided(), [edge]);
    }

    /// The team keeps [`TEAM_SPOTS_MAX`] spots; a new one pushes out the
    /// least lately hit that is not kept clear of first.
    #[test]
    fn the_team_keeps_its_spots_to_a_bound() {
        let mut team = TeamSpots::default();
        let kept = Vec3::new(500.0, 0.0, 0.0);
        team.learn(0, &[(kept, true)]);
        for i in 0..TEAM_SPOTS_MAX as u32 {
            team.learn(1 + i, &[(Vec3::new(10.0 * i as f32, 0.0, 50.0), false)]);
        }
        assert_eq!(team.spots.len(), TEAM_SPOTS_MAX);
        assert_eq!(team.avoided(), [kept]);
    }

    /// A way round is taken when at most 1.5 times the way through plus
    /// 10 m; the way through is searched only when the way round may be
    /// longer than that against the straight line.
    #[test]
    fn a_way_round_is_taken_when_not_much_longer() {
        assert!(worth_round(13.7, 13.6));
        assert!(worth_round(28.4, 25.7));
        assert!(!worth_round(60.0, 30.0));
        let (from, to) = (Vec3::ZERO, Vec3::new(20.0, 0.0, 0.0));
        assert!(!may_be_long(40.0, from, to));
        assert!(may_be_long(41.0, from, to));
    }

    /// Nothing left to try on the ladder: the last step ([`Recovery::GiveUp`])
    /// taken [`HOPELESS_GIVE_UPS`] times at one spot, each within
    /// [`AGAIN_MS`] of the one before; since the first stuck there. Not at
    /// another spot, not after a break in the run, not before the life.
    #[test]
    fn three_give_ups_at_one_spot_are_nothing_left_to_try() {
        let mut stuck = Stuck::default();
        let spot = Vec3::new(10.0, 0.0, 10.0);
        let t0 = 10_000;
        assert_eq!(stuck.hopeless(t0, 0), None);
        for k in 0..5 {
            let at = t0 + k * 3_000;
            assert_eq!(walk(&mut stuck, at, spot, None), None);
            assert!(walk(&mut stuck, at + 1_600, spot, None).is_some());
            let hopeless = stuck.hopeless(at + 1_600, 0);
            if k < 4 {
                assert_eq!(hopeless, None, "stuck {} times: BackOff, Sidestep, GiveUp, GiveUp", k + 1);
            } else {
                assert_eq!(
                    hopeless,
                    Some(Hopeless { since: t0 + 1_600, why: HopelessWhy::Ladder(spot) })
                );
            }
        }
        let last = t0 + 4 * 3_000 + 1_600;
        // The run ends when it is not caught again within AGAIN_MS.
        assert!(stuck.hopeless(last + AGAIN_MS - 1, 0).is_some());
        assert_eq!(stuck.hopeless(last + AGAIN_MS, 0), None);
        // A run of an earlier life is not this one's.
        assert_eq!(stuck.hopeless(last, last), None);
        // A spot a fall marked is no ladder.
        let mut fallen = Stuck::default();
        for k in 0..5 {
            fallen.fell(t0 + k * 3_000, spot);
        }
        assert_eq!(fallen.hopeless(t0 + 12_000, 0), None);
    }

    /// The owner's rule: the defaults with the 60 s on.
    fn rule() -> UnstickTuning {
        UnstickTuning { suicide_after_ms: Some(crate::v026g::bots::tuning::SUICIDE_AFTER_OWNER_MS), ..UnstickTuning::default() }
    }

    fn facts() -> Facts {
        Facts {
            now: 100_000,
            hopeless: Some(Hopeless { since: 30_000, why: HopelessWhy::Pocket }),
            quiet_ms: u32::MAX,
            life_ms: 90_000,
            match_ms: 200_000,
            carrying: false,
            tickets: None,
            since_last_ms: None,
            team_last_minute: 0,
        }
    }

    fn sends(state: SelfKill, facts: &Facts) -> bool {
        matches!(
            self_kill_step(state, facts, &rule()),
            (SelfKill::Sent { tries: 1, .. }, Some(SelfKillAct::Send { again: false }))
        )
    }

    /// The owner's rule: 60 s of nothing left to try, no enemy or hit in
    /// 15 s, nothing carried, the life 20 s and the match 120 s old, in
    /// Team Deathmatch above 5 tickets, one a bot in 3 minutes, two a team a
    /// minute: each one alone stops it, and all of them together let it.
    #[test]
    fn a_bot_kills_itself_only_when_every_rule_lets_it() {
        let ok = facts();
        assert_eq!(
            self_kill_step(SelfKill::Ready, &ok, &rule()),
            (SelfKill::Sent { at: 100_000, tries: 1 }, Some(SelfKillAct::Send { again: false }))
        );
        let hopeless = |since| Facts { hopeless: Some(Hopeless { since, why: HopelessWhy::Pocket }), ..ok };
        let stops: Vec<(&str, Facts)> = vec![
            ("no run", Facts { hopeless: None, ..ok }),
            ("59.9 s", hopeless(40_001)),
            ("a hit or an enemy 14.9 s ago", Facts { quiet_ms: 14_999, ..ok }),
            ("carrying", Facts { carrying: true, ..ok }),
            ("a life of 19.9 s", Facts { life_ms: 19_999, ..ok }),
            ("a match of 119.9 s", Facts { match_ms: 119_999, ..ok }),
            ("5 tickets", Facts { tickets: Some(5), ..ok }),
            ("the last one 179.9 s ago", Facts { since_last_ms: Some(179_999), ..ok }),
            ("two a minute already", Facts { team_last_minute: 2, ..ok }),
        ];
        for (what, facts) in &stops {
            assert!(!sends(SelfKill::Ready, facts), "{what}");
        }
        for (what, facts) in [
            ("60 s", hopeless(40_000)),
            ("an enemy 15 s ago", Facts { quiet_ms: 15_000, ..ok }),
            ("a life of 20 s", Facts { life_ms: 20_000, ..ok }),
            ("a match of 120 s", Facts { match_ms: 120_000, ..ok }),
            ("6 tickets", Facts { tickets: Some(6), ..ok }),
            ("the last one 180 s ago", Facts { since_last_ms: Some(180_000), ..ok }),
            ("one a minute", Facts { team_last_minute: 1, ..ok }),
        ] {
            assert!(sends(SelfKill::Ready, &facts), "{what}");
        }
        // The setting at 0 is never.
        let off = UnstickTuning { suicide_after_ms: None, ..rule() };
        assert_eq!(self_kill_step(SelfKill::Ready, &ok, &off), (SelfKill::Ready, None));
    }

    /// After the bit the world has 4 s to show the body dead; if it does not,
    /// the bit goes once more (the caps are not counted twice), and then the
    /// bot is barred for the life. A fight or a battery since the first try
    /// gives the second up: it may ask again later.
    #[test]
    fn the_bit_is_sent_again_once_and_then_the_bot_is_barred() {
        let rule = rule();
        let sent = SelfKill::Sent { at: 100_000, tries: 1 };
        let at = |now| Facts { now, since_last_ms: Some(now - 100_000), team_last_minute: 2, ..facts() };
        assert_eq!(self_kill_step(sent, &at(103_999), &rule), (sent, None), "the world is not due");
        assert_eq!(
            self_kill_step(sent, &at(104_000), &rule),
            (SelfKill::Sent { at: 104_000, tries: 2 }, Some(SelfKillAct::Send { again: true }))
        );
        let second = SelfKill::Sent { at: 104_000, tries: 2 };
        assert_eq!(self_kill_step(second, &at(107_999), &rule), (second, None));
        assert_eq!(self_kill_step(second, &at(108_000), &rule), (SelfKill::Barred, None));
        assert_eq!(self_kill_step(SelfKill::Barred, &at(900_000), &rule), (SelfKill::Barred, None));
        // An enemy came in view meanwhile: no second try.
        let fight = Facts { quiet_ms: 1_000, ..at(104_000) };
        assert_eq!(self_kill_step(sent, &fight, &rule), (SelfKill::Ready, None));
        let carrying = Facts { carrying: true, ..at(104_000) };
        assert_eq!(self_kill_step(sent, &carrying, &rule), (SelfKill::Ready, None));
    }
}

/// A team's stuck state, by feature ([`super::features`]).
#[derive(Default)]
pub struct TeamStuck {
    /// Goals a bot of the team proved no way to, and until when the team's plays leave them
    /// (`TEAM_NO_WAY_KEEP_MS`).
    pub no_way_to: Vec<(mapdata::Vec3, u32)>,
    /// Where its bots got stuck or fell, for the match ([`TeamSpots`]).
    pub spots: TeamSpots,
    /// When its bots killed themselves in the last minute (`SelfKill`).
    pub self_kills: Vec<u32>,
}
