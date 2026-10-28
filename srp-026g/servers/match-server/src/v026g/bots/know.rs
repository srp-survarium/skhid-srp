//! What a bot and its team know (`docs/notes/player_bots_research.md` B1,
//! "Knowledge (Brewer)"): every enemy fact carries the sense it came by
//! ([`Source`]), so the fairness rules of `player_bots_behaviour.md` §2 are
//! rules on types:
//! - the motor aims only at a [`SeenTrack`], which only [`Memory`] makes,
//!   and only from a sight older than the noticing delay: "never aim at
//!   what no sense gave" and "never react faster than noticing" cannot be
//!   broken by a play;
//! - a teammate's word ([`Source::Told`]) and the cheats
//!   ([`Source::Cheat`]) steer routes and where to look, never a shot;
//! - an enemy not sensed for a while becomes a last known position that
//!   fades, then is dropped.

use mapdata::Vec3;

use super::plays::{PlayKind, TargetRef};
use super::tuning::{CheatTuning, SenseTuning};
use super::view::{BatteryState, MatchView, Place};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CheatKind {
    Thief,
    Radar,
}

/// How a fact came to the bot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Source {
    Seen,
    Heard,
    /// A bot teammate's callout.
    Told {
        by: Place,
    },
    /// What a human's HUD shows (the kill feed, markers).
    Hud,
    Cheat(CheatKind),
}

impl Source {
    /// How much the bot trusts a report of this kind: its own sight over
    /// its own hearing over a teammate's word (and the HUD) over a cheat.
    pub fn strength(self) -> u8 {
        match self {
            Self::Seen => 3,
            Self::Heard => 2,
            Self::Told { .. } | Self::Hud => 1,
            Self::Cheat(_) => 0,
        }
    }

    /// The team's own senses: a bot's sight or hearing, a teammate's word.
    pub fn own(self) -> bool {
        matches!(self, Self::Seen | Self::Heard | Self::Told { .. })
    }

    /// A track of this kind sensed within this, ms, is fresh: a weaker
    /// report does not replace it. A sight for a moment (as before: a heard
    /// shot from where he went is better than a sight half a second old),
    /// the rest for [`FRESH_MS`].
    pub fn fresh_ms(self) -> u32 {
        match self {
            Self::Seen => 500,
            _ => FRESH_MS,
        }
    }
}

/// A heard or told track sensed within this, ms, is fresh: a weaker report
/// does not replace it (a teammate's callout does not wipe what the bot
/// heard itself a moment ago; play 6's heard shot is as long).
pub const FRESH_MS: u32 = 1_500;

/// The sight windows' defaults (`SenseTuning::sight_untested_ms` and
/// `sight_grace_ms`), ms, on the information clock. `MatchView::clock` is
/// the server's clock: a player's ping changes only how accurate the
/// estimate of his body is, never these.
///
/// A sight that no look has tested goes on this long, ms on the information
/// clock (`MatchView::clock`, what every sight is stamped with): the
/// longest think gap (`ThinkTuning::think_ms`, 400) and a tick (50). A
/// look not made (a starved ray budget, a think not yet due) is no loss of
/// sight, but only this long: after it the next think's test must have its
/// rays, or a starved budget would be sight through a wall.
pub const SIGHT_UNTESTED_MS: u32 = 450;

/// A sight whose look failed (out of range, out of view, blocked) goes on
/// this long, ms on the information clock, from that look: a look away
/// from a corner, a body behind a post. A look that finds him again inside
/// it takes the failure back. At least the target's look gap
/// (`aimpoint::LOOK_AGAIN_MS`, 100) and a tick (50), or one look that
/// found him again could not come in time. 250 ms is the human measurement
/// (247 live recordings, 7,694 breaks of the line of sight on the mesh: a
/// player keeps the trigger down with no change until about 200 ms after
/// a break, and after an occlusion of 500 ms or less 75 % keep it held and
/// fire again at 200 ms; the range is 150 to 400, resolution 100 to 150
/// ms); `SenseTuning::sight_grace_ms` tunes it for the bench.
pub const SIGHT_GRACE_MS: u32 = 250;

// The default grace covers the look gap and a tick.
const _: () = assert!(SIGHT_GRACE_MS >= super::aimpoint::LOOK_AGAIN_MS + 50);

/// The default of `CheatTuning::sight_hold_ms`, ms: the most a sight that is
/// not the target's goes on with no test (the target is looked at every
/// tick). Not shorter than the longest think gap and a tick: a visible
/// enemy is tested at a think, every 200 to 400 ms, and a hold that ends
/// before the next test blanks him for the rest of the gap (the cadence
/// fault the sight refresh is for).
pub const SIGHT_HOLD_MS: u32 = 450;

/// A tick, ms: the hold to the next think reaches one tick past it.
const TICK_MS: u32 = 50;

/// How long a sight goes on without a new sensing when the sight refresh is off (the tip's rule), ms
/// on the tick clock of the read: the last sensing within this.
pub const LEGACY_SIGHT_MS: u32 = 400;

/// The sight windows a [`Memory`] reads ([`SenseTuning`]'s and
/// [`CheatTuning`]'s numbers).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SightWindow {
    /// The sight refresh (`SenseTuning::sight_refresh`). Off: a sight is read as it was before it, on the
    /// tick clock of the read, up to [`LEGACY_SIGHT_MS`] after its last sensing, by a think's sensing alone;
    /// no look, grace or hold is used, and none changes a track.
    pub refresh: bool,
    pub grace_ms: u32,
    pub untested_ms: u32,
    /// A sight that is not the target's goes on without a test at most this
    /// long (`CheatTuning::sight_hold_ms`), and at most `untested_ms`.
    pub hold_ms: u32,
    /// ... and only until the bot's own next think and a tick
    /// (`CheatTuning::sight_hold_to_next_think`), the test that comes next.
    pub hold_to_next_think: bool,
}

impl Default for SightWindow {
    /// The refresh off, as the tuning's default is.
    fn default() -> Self {
        Self {
            refresh: false,
            grace_ms: SIGHT_GRACE_MS,
            untested_ms: SIGHT_UNTESTED_MS,
            hold_ms: 0,
            hold_to_next_think: true,
        }
    }
}

impl SightWindow {
    /// The refresh on with the windows of the alias that turns it all on (`SRP_BENCH_SIGHT_REFRESH`): the
    /// grace of 250 ms, the untested bound of 450 and the hold of 450 to the next think.
    pub fn refreshed() -> Self {
        Self {
            refresh: true,
            hold_ms: SIGHT_HOLD_MS,
            ..Self::default()
        }
    }

    /// The tuning's windows.
    pub fn of(sense: &SenseTuning, cheats: &CheatTuning) -> Self {
        Self {
            refresh: sense.sight_refresh,
            grace_ms: sense.sight_grace_ms,
            untested_ms: sense.sight_untested_ms,
            hold_ms: cheats.sight_hold_ms,
            hold_to_next_think: cheats.sight_hold_to_next_think,
        }
    }

    /// Until when a sight not the target's, tested at `time` by a bot whose
    /// next think is at `next_think` (if it knows), is held: to that think
    /// and a tick, or `hold_ms` from the test, whichever is first, and
    /// never past `untested_ms`.
    fn held_until(self, time: u32, next_think: Option<u32>) -> u32 {
        let by_think = next_think
            .filter(|_| self.hold_to_next_think)
            .map_or(u32::MAX, |next| next.saturating_add(TICK_MS));
        by_think
            .min(time + self.hold_ms)
            .min(time + self.untested_ms)
            .max(time)
    }
}

/// Why a look at a live sight found nobody.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Miss {
    /// Farther than `SenseTuning::sight_m`.
    Range,
    /// Outside the view's width (`SenseTuning::view_deg`) at the bot's yaw.
    Cone,
    /// No part of him shows on the bullet mesh.
    Blocked,
    /// Dead, gone, or another life than the one seen (`PlaceView::spawned`).
    Dead,
}

/// What a look at one live sight came to (`sense::look`). A sight ends only
/// by a failed look: one not made is not a negative.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LookOutcome {
    Seen,
    Failed(Miss),
    /// Not tested: the rays ran out. The sight keeps its state, up to
    /// [`SIGHT_UNTESTED_MS`].
    Untested,
}

/// A sense's report: who, where (with its error), when.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Percept {
    pub who: Place,
    pub at: Vec3,
    pub error_m: f32,
    pub time: u32,
    pub source: Source,
    /// Seen moving or firing: noticed sooner.
    pub busy: bool,
}

/// What a bot knows of one enemy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Track {
    pub who: Place,
    pub at: Vec3,
    pub error_m: f32,
    /// When it was last sensed.
    pub time: u32,
    pub source: Source,
    /// Since when it has been in sight without a break (`Seen` only).
    pub sight_since: Option<u32>,
    /// The noticing delay for this sight, ms.
    pub notice_ms: u32,
    /// The last place the bot's or its team's own senses gave (seen,
    /// heard, told) and when: a cheat's place does not count.
    pub own: Option<(Vec3, u32)>,
    /// The first look since a sight that found him gone, when and why: the
    /// sight goes on [`SIGHT_GRACE_MS`] from it (`Seen` only).
    pub missed: Option<(u32, Miss)>,
    /// Until when the sight is held with no test if he is not the target
    /// ([`SightWindow::held_until`]; `Seen` only).
    pub held_until: u32,
}

impl Track {
    /// Whether a sight begun and not failed longer ago than
    /// [`SIGHT_GRACE_MS`] at `sight_now` (the information clock) can go on
    /// from a look that finds him: however long since the last one (the
    /// server stalled; a look that finds him is a test, which a stall does
    /// not make a negative).
    fn unbroken(&self, sight_now: u32, window: SightWindow) -> bool {
        if !window.refresh {
            return self.sight_since.is_some() && self.time + LEGACY_SIGHT_MS >= sight_now;
        }
        self.sight_since.is_some()
            && self
                .missed
                .is_none_or(|(at, _)| at + window.grace_ms >= sight_now)
    }

    /// Whether the sight is up at `sight_now`: [`Self::unbroken`] and last
    /// tested within the untested bound ([`SIGHT_UNTESTED_MS`]), or for an
    /// enemy that is not `target` within the hold (`held_until`). What is
    /// read as a sight.
    fn sighted(&self, sight_now: u32, window: SightWindow, target: Option<Place>) -> bool {
        if !window.refresh {
            return self.time + LEGACY_SIGHT_MS >= sight_now;
        }
        let up_to = if target == Some(self.who) {
            self.time + window.untested_ms
        } else {
            self.held_until.min(self.time + window.untested_ms)
        };
        self.unbroken(sight_now, window) && up_to >= sight_now
    }
}

/// An enemy seen, and noticed: the only thing the motor may aim at. Made
/// only by [`Memory::seen`], and by [`Memory::in_sight`] for its one use:
/// the reflex taking back its own last target, lost a moment ago, before
/// he is noticed again.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeenTrack(Track);

impl SeenTrack {
    pub fn track(&self) -> &Track {
        &self.0
    }

    pub fn who(&self) -> Place {
        self.0.who
    }

    pub fn at(&self) -> Vec3 {
        self.0.at
    }
}

/// One bot's memory of the enemies.
#[derive(Clone, Debug, Default)]
pub struct Memory {
    tracks: Vec<Track>,
    window: SightWindow,
    /// The owner's next think, if it is told ([`Self::set_next_think`]).
    next_think: Option<u32>,
    /// The reflex's target, which is looked at every tick and so is never
    /// held ([`SightWindow::hold_ms`]).
    target: Option<Place>,
    /// [measure] The sight refresh's activity, cumulative (nothing reads it to decide): the sights a look
    /// kept ([`Self::refresh_sight`]), and the times a think found a sight that is not the target's still up
    /// by the hold alone (no sensing of him at that think).
    refreshes: u32,
    hold_uses: u32,
}

impl Memory {
    /// [measure] The sights kept by a look so far.
    pub fn refreshes(&self) -> u32 {
        self.refreshes
    }

    /// [measure] The thinks that found a sight of a non-target up by the hold alone, so far.
    pub fn hold_uses(&self) -> u32 {
        self.hold_uses
    }

    /// The sight windows to read from now on (the tuning's).
    pub fn set_sight_window(&mut self, window: SightWindow) {
        self.window = window;
    }

    /// The bot's next think: a sight tested at a think is held to it.
    pub fn set_next_think(&mut self, next_think: u32) {
        self.next_think = Some(next_think);
    }

    /// The reflex's target: the one sight no hold applies to.
    pub fn set_sight_target(&mut self, target: Option<Place>) {
        self.target = target;
    }

    /// The sights begun and not ended (one failed past the grace is still
    /// listed until a think's sweep or the next sensing of it ends it): who,
    /// and since when (the enemies a think looks at first).
    pub fn live_sights(&self) -> impl Iterator<Item = (Place, u32)> + '_ {
        self.tracks.iter().filter_map(|track| {
            let since = track.sight_since?;
            (track.source == Source::Seen).then_some((track.who, since))
        })
    }

    /// Takes in a think's percepts. `notice_ms(percept)` is the noticing
    /// delay for a new sight, asked only when one begins (or a track is
    /// made); `forget_ms` and `drop_ms` the fading.
    pub fn update(
        &mut self,
        percepts: &[Percept],
        now: u32,
        notice_ms: impl Fn(&Percept) -> u32,
        forget_ms: u32,
        drop_ms: u32,
    ) {
        let (window, target, next_think) = (self.window, self.target, self.next_think);
        for percept in percepts {
            match self.tracks.iter_mut().find(|t| t.who == percept.who) {
                Some(track) => {
                    let seen = percept.source == Source::Seen;
                    // A report replaces the place only if it is no older
                    // than the track and not weaker than a fresh one: a
                    // teammate's callout (sensed a second or more ago) does
                    // not wipe the bot's own hearing, a cheat not a
                    // callout, and nothing a sight.
                    let fresh = track.time + track.source.fresh_ms() >= now;
                    let weaker = percept.source.strength() < track.source.strength();
                    let replaces = percept.time >= track.time && !(fresh && weaker);
                    if replaces {
                        track.at = percept.at;
                        track.error_m = percept.error_m;
                        track.source = percept.source;
                        if seen {
                            track.held_until = window.held_until(percept.time, next_think);
                        }
                    }
                    if percept.source.own()
                        && track.own.is_none_or(|(_, time)| percept.time >= time)
                    {
                        track.own = Some((percept.at, percept.time));
                    }
                    if seen {
                        // A sight goes on if the last one was a moment ago.
                        if !track.unbroken(percept.time, window) {
                            track.sight_since = Some(percept.time);
                            track.notice_ms = notice_ms(percept);
                            track.missed = None;
                        }
                        // A test that found him takes a failed look back.
                        if replaces {
                            track.missed = None;
                        }
                    } else if !track.sighted(percept.time, window, target) {
                        track.sight_since = None;
                    }
                    // When it was last sensed by a report it took: a weaker
                    // one it did not take keeps no sight alive (a cheat's
                    // place of a thief is no sight of him).
                    if replaces {
                        track.time = percept.time;
                    }
                }
                None => self.tracks.push(Track {
                    who: percept.who,
                    at: percept.at,
                    error_m: percept.error_m,
                    time: percept.time,
                    source: percept.source,
                    sight_since: (percept.source == Source::Seen).then_some(percept.time),
                    notice_ms: notice_ms(percept),
                    own: percept.source.own().then_some((percept.at, percept.time)),
                    missed: None,
                    held_until: window.held_until(percept.time, next_think),
                }),
            }
        }
        for track in &mut self.tracks {
            // Out of sight now.
            if !track.sighted(now, window, target) {
                track.sight_since = None;
            } else if window.refresh
                && track.sight_since.is_some()
                && track.source == Source::Seen
                && target != Some(track.who)
                && !percepts.iter().any(|p| p.who == track.who && p.source == Source::Seen)
            {
                self.hold_uses += 1;
            }
            // Not sensed for a while: a last known position, fading.
            if track.time + forget_ms < now {
                track.error_m += 0.5;
            }
        }
        self.tracks.retain(|track| track.time + drop_ms >= now);
    }

    /// The enemy at `who` seen now and noticed. `now` is the clock the
    /// noticing delay and the shot run on (the tick's); `sight_now` the
    /// information clock (`MatchView::clock`), which every sight is stamped
    /// with and its age is read on.
    ///
    /// The two differ by the input lead: since 0a3a5589 a tick's input is
    /// made 50 to 100 ms ahead of the match clock, and since 8e5b39db the
    /// reflex read the sight's age on that tick time against a stamp of
    /// the poll clock, so the 400 ms window was 300 ms in effect and a
    /// think gap over 300 ms lost a target in plain view. The lead is the
    /// time a decision takes effect, not the age of information.
    ///
    /// With the refresh off the sight is read on `now`, as it was before it.
    pub fn seen(&self, who: Place, now: u32, sight_now: u32) -> Option<SeenTrack> {
        let track = self.tracks.iter().find(|t| t.who == who)?;
        let since = track.sight_since?;
        (track.source == Source::Seen
            && now >= since + track.notice_ms
            && track.sighted(self.read_clock(now, sight_now), self.window, self.target))
        .then_some(SeenTrack(*track))
    }

    /// The clock a sight is read on: the information clock with the refresh, the tick's without.
    fn read_clock(&self, now: u32, sight_now: u32) -> u32 {
        if self.window.refresh { sight_now } else { now }
    }

    /// The enemy at `who` in sight now, noticed or not: [`Self::seen`]
    /// without the noticing delay only, so still a live sight (seen,
    /// `sight_since` set, tested within [`SIGHT_UNTESTED_MS`], not failed
    /// longer ago than [`SIGHT_GRACE_MS`] at `sight_now`). Its one allowed
    /// use: the reflex taking back its own last target, lost under
    /// `CombatTuning::aim_carry_ms` ago (`CombatTuning::reacquire_keep`),
    /// which noticed him already.
    pub(super) fn in_sight(&self, who: Place, now: u32, sight_now: u32) -> Option<SeenTrack> {
        let track = self.tracks.iter().find(|t| t.who == who)?;
        track.sight_since?;
        (track.source == Source::Seen
            && track.sighted(self.read_clock(now, sight_now), self.window, self.target))
        .then_some(SeenTrack(*track))
    }

    /// Every enemy seen now and noticed.
    pub fn all_seen(&self, now: u32, sight_now: u32) -> Vec<SeenTrack> {
        self.tracks
            .iter()
            .filter_map(|track| self.seen(track.who, now, sight_now))
            .collect()
    }

    /// A look at the live sight of `who` found him at `at` at `sight_clock`
    /// (`MatchView::clock`): the sight goes on from there. Not a sensing
    /// ([`Self::update`]): it touches only the track's time, place and own
    /// place, so nothing fades or is called out to the team, and a sight
    /// not begun or ended, or a look older than the track, is left as it
    /// is.
    pub fn refresh_sight(&mut self, who: Place, at: Vec3, sight_clock: u32) {
        let (window, next_think) = (self.window, self.next_think);
        if !window.refresh {
            return;
        }
        let Some(track) = self.tracks.iter_mut().find(|t| t.who == who) else {
            return;
        };
        if track.source != Source::Seen || !track.unbroken(sight_clock, window)
            || sight_clock < track.time
        {
            return;
        }
        self.refreshes += 1;
        track.time = sight_clock;
        track.at = at;
        track.own = Some((at, sight_clock));
        track.missed = None;
        track.held_until = window.held_until(sight_clock, next_think);
    }

    /// A look at the live sight of `who` at `sight_clock` found him gone,
    /// for `miss`. The first failure starts the grace ([`SIGHT_GRACE_MS`]),
    /// later ones do not extend it; his death ends the sight at once. The
    /// track keeps its last place.
    pub fn fail_sight(&mut self, who: Place, miss: Miss, sight_clock: u32) {
        let window = self.window;
        if !window.refresh {
            return;
        }
        let Some(track) = self.tracks.iter_mut().find(|t| t.who == who) else {
            return;
        };
        if track.source != Source::Seen || !track.unbroken(sight_clock, window) {
            return;
        }
        if miss == Miss::Dead {
            track.sight_since = None;
            track.missed = None;
        } else {
            track.missed.get_or_insert((sight_clock, miss));
        }
    }

    /// A look's outcome for the sight of `who`, whom it found at `at`: the
    /// sight kept, ended by a failure, or (untested) left as it is.
    pub fn looked(&mut self, who: Place, at: Vec3, outcome: LookOutcome, sight_clock: u32) {
        match outcome {
            LookOutcome::Seen => self.refresh_sight(who, at, sight_clock),
            LookOutcome::Failed(miss) => self.fail_sight(who, miss, sight_clock),
            LookOutcome::Untested => {}
        }
    }

    /// Everything known, however it came and however old.
    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    /// An enemy died: it is not where it was.
    pub fn forget(&mut self, who: Place) {
        self.tracks.retain(|track| track.who != who);
    }
}

/// A teammate's death at a spot: danger there for a while (and a trade to
/// make).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DangerSpot {
    pub at: Vec3,
    pub time: u32,
    pub victim: Place,
}

/// A teammate's callout "shot from there": hit with no shooter it sensed,
/// it gives the direction its HUD's hit marker showed (no distance, no
/// shooter), from where it stood.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShotFrom {
    pub by: Place,
    pub at: Vec3,
    /// The marker's direction (a yaw, radians, with the marker's error).
    pub bearing: f32,
    /// When the team knows it (the callout's delay after the hit).
    pub time: u32,
}

/// A battery of ours taken.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theft {
    pub battery: usize,
    /// Where it was last known.
    pub at: Vec3,
    pub time: u32,
    pub thief: Option<Place>,
}

/// A lane cell's side (CG-12's memory of each enemy human), m.
pub const LANE_CELL_M: f32 = 6.0;
/// A lane cell's count halves in this, ms: what he did minutes ago still
/// counts, what he did at the start fades.
pub const LANE_HALF_LIFE_MS: u32 = 180_000;
/// At most one place of a human a this, ms, goes into his lane: the count
/// is the time he was known there, not how many bots knew it.
pub const LANE_SAMPLE_MS: u32 = 1_000;

/// A 6 m cell of an enemy human's lanes: where he was known in it (the
/// mean of his places), how much (decayed by [`LANE_HALF_LIFE_MS`]) and
/// when last.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LaneCell {
    pub cell: (i32, i32),
    pub at: Vec3,
    pub count: f32,
    pub time: u32,
}

impl LaneCell {
    /// Its count at `now`.
    pub fn count_at(&self, now: u32) -> f32 {
        decayed(self.count, now.saturating_sub(self.time))
    }
}

fn decayed(count: f32, ms: u32) -> f32 {
    count * 0.5f32.powf(ms as f32 / LANE_HALF_LIFE_MS as f32)
}

/// What the team knows of one enemy human for the whole match (Cologne
/// CG-12), only from its own senses and its HUD: his lanes (the cells of
/// his places the team saw, heard or was told), the batteries he stole
/// from us, and where he killed us (the kill feed: his kills went up as
/// one of us died).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EnemyHuman {
    pub who: Place,
    pub cells: Vec<LaneCell>,
    /// When a place of his last went into his lanes.
    pub last: Option<u32>,
    /// The batteries he stole from us: (battery, when).
    pub thefts: Vec<(usize, u32)>,
    /// Where he killed one of us, and when.
    pub kills: Vec<(Vec3, u32)>,
    /// His kills as the scoreboard last showed them.
    kills_shown: u32,
}

/// The team's memory of each enemy human ([`EnemyHuman`]), kept for the
/// match.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HumanLanes {
    pub humans: Vec<EnemyHuman>,
}

impl HumanLanes {
    pub fn get(&self, who: Place) -> Option<&EnemyHuman> {
        self.humans.iter().find(|h| h.who == who)
    }

    fn human_mut(&mut self, who: Place) -> &mut EnemyHuman {
        let i = match self.humans.iter().position(|h| h.who == who) {
            Some(i) => i,
            None => {
                self.humans.push(EnemyHuman {
                    who,
                    ..Default::default()
                });
                self.humans.len() - 1
            }
        };
        &mut self.humans[i]
    }

    /// The team knew `who` at `at` (sensed at `time`): one place a
    /// [`LANE_SAMPLE_MS`] goes into his lanes.
    pub fn knew(&mut self, who: Place, at: Vec3, time: u32) {
        let human = self.human_mut(who);
        if human.last.is_some_and(|last| time < last + LANE_SAMPLE_MS) {
            return;
        }
        human.last = Some(time);
        let cell = (
            (at.x / LANE_CELL_M).floor() as i32,
            (at.z / LANE_CELL_M).floor() as i32,
        );
        match human.cells.iter_mut().find(|c| c.cell == cell) {
            Some(c) => {
                let count = c.count_at(time);
                c.at = (c.at * count + at) * (1.0 / (count + 1.0));
                c.count = count + 1.0;
                c.time = time;
            }
            None => human.cells.push(LaneCell {
                cell,
                at,
                count: 1.0,
                time,
            }),
        }
    }

    /// He stole `battery` from us at `time` (the HUD's theft); a theft is
    /// counted once.
    pub fn stole(&mut self, who: Place, battery: usize, time: u32) {
        let human = self.human_mut(who);
        if !human.thefts.contains(&(battery, time)) {
            human.thefts.push((battery, time));
        }
    }

    /// The scoreboard shows `kills` for him: the kills since the last
    /// showing were at `at`, our latest death, if the team knows one.
    pub fn shown_kills(&mut self, who: Place, kills: u32, at: Option<Vec3>, time: u32) {
        let human = self.human_mut(who);
        if kills > human.kills_shown
            && let Some(at) = at
        {
            human.kills.push((at, time));
        }
        human.kills_shown = kills;
    }

    /// The lanes near `near` (within `within_m`) of the humans who stole
    /// from us, best first: each cell's count at `now` times its human's
    /// thefts, the same cell of two thieves added. For a holder of our
    /// container (CG-1, CG-12): where a thief comes from.
    pub fn thief_lane(&self, near: Vec3, within_m: f32, now: u32) -> Vec<(Vec3, f32)> {
        let mut out: Vec<((i32, i32), Vec3, f32)> = Vec::new();
        for human in self.humans.iter().filter(|h| !h.thefts.is_empty()) {
            let thefts = human.thefts.len() as f32;
            for c in &human.cells {
                if c.at.distance_xz(near) > within_m {
                    continue;
                }
                let weight = c.count_at(now) * thefts;
                match out.iter_mut().find(|o| o.0 == c.cell) {
                    Some(o) => o.2 += weight,
                    None => out.push((c.cell, c.at, weight)),
                }
            }
        }
        out.sort_by(|a, b| b.2.total_cmp(&a.2));
        out.into_iter().map(|(_, at, w)| (at, w)).collect()
    }
}

/// Where a team believes a battery lies (the fair battery knowledge,
/// `player_bots_behaviour.md` §2), used with the battery cheat off.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KnownBattery {
    pub at: Vec3,
    /// When it was last put right.
    pub time: u32,
    /// Shown on the team's HUD: it follows the battery while it lies about
    /// and goes when the game takes it off the HUD. Else seen by a bot of
    /// the team, and left where it was seen until a bot sees that spot
    /// empty.
    pub hud: bool,
}

/// A known battery's spot for a note: where, and whether the HUD shows it.
fn spot(known: KnownBattery) -> String {
    let at = known.at;
    let how = if known.hud { "on the HUD" } else { "seen" };
    format!("[{:.1}, {:.1}, {:.1}] {how}", at.x, at.y, at.z)
}

/// A teammate's death (the kill feed), kept longer than a danger spot for
/// the spawners' search: a spot where the team keeps dying is worth less.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Loss {
    pub at: Vec3,
    pub time: u32,
    /// The spawner the bot was going to look at, if it was.
    pub spawner: Option<usize>,
}

/// Losses are kept this long, ms.
pub const LOSS_KEEP_MS: u32 = 120_000;

/// An enemy a bot of the team saw carrying a battery (a carrier holds it in
/// his hands, in sight).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarrierSeen {
    pub who: Place,
    pub battery: usize,
    pub at: Vec3,
    pub time: u32,
}

/// Seen carriers are kept this long, ms.
pub const CARRIER_SEEN_KEEP_MS: u32 = 60_000;

/// An enemy carrier's place as a bot of the team sensed it (seen, heard,
/// or the thief cheat), with its error: whether he stays put
/// ([`TeamKnowledge::carrier_still`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarrierPlace {
    pub who: Place,
    pub at: Vec3,
    pub error_m: f32,
    pub time: u32,
}

/// Carriers' places are kept this long, ms.
pub const CARRIER_PLACES_KEEP_MS: u32 = 30_000;

/// A bot of the team fell near a battery it went for: it was far off its
/// reckoning there (a ledge by the battery; [`TeamKnowledge::fell_near`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fall {
    pub who: Place,
    pub at: Vec3,
    pub time: u32,
}

/// Falls are kept this long, ms; a bot's falls this near in time are one.
pub const FALLS_KEEP_MS: u32 = 120_000;
const FALL_SAME_MS: u32 = 5_000;
/// Falls this near a spot are at it, metres across and up or down.
const FALL_SPOT_M: f32 = 1.5;
const FALL_SPOT_UP_M: f32 = 2.0;

/// A battery an enemy carrier dropped where he died, unseen: the kill feed
/// says he died, the team knew he carried it (a theft of ours, or seen
/// carrying), and the game's HUD shows the dropped battery only to his
/// team. A human walks to where he last saw him; the bots look there.
///
/// Or a battery of ours a thief dropped alive (F2 of the analysis of the
/// owner's matches 0x652d/0x1a09 of 2026-10-16): the HUD told the theft and
/// the container's count, the carrier no longer has it, and the battery is
/// on his team's HUD only. The team looks round its container and where it
/// last sensed him (`theft`: one bot for all these spots).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LookSpot {
    pub battery: usize,
    /// His last place the team itself sensed (the killer's sight, else
    /// its last own sight, hearing or callout of him); or a spot of the
    /// ring round our container.
    pub at: Vec3,
    pub time: u32,
    /// Dropped after this (match ms) if nobody has looked.
    pub until: u32,
    /// A spot after a theft whose thief dropped the battery alive.
    pub theft: bool,
}

/// A look spot nobody has looked at is dropped after this, ms.
pub const LOOK_KEEP_MS: u32 = 120_000;

/// A bot's claim on a spot: where its play means it to be (its goal, or
/// its feet when it holds), stamped when it was made. A bot makes it again
/// at every think; one not renewed for [`CLAIM_KEEP_MS`] (the bot died or
/// left) is dropped.
///
/// Root cause (`push-record.md` S1): two teammates chose the same spot in
/// one pass, each seeing only the other's feet, and a standing body is not
/// steered off another. A shared list of goals lets every spot choice skip
/// a spot a teammate already means to take (Jack, GAP ch. 26.6.2, "Hidespot
/// Contention").
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Claim {
    pub who: Place,
    pub at: Vec3,
    pub time: u32,
}

/// A claim is dropped when not renewed for this long, ms.
pub const CLAIM_KEEP_MS: u32 = 2_000;

/// The human route a bot's Roam run was drawn on (`plays::roam`, `fresh_claims`): the side's route
/// number and when it was drawn. Renewed at every think of the run, dropped with it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RouteClaim {
    pub who: Place,
    pub side: i32,
    pub route: usize,
    pub drawn: u32,
    seen: u32,
}

/// A bot's claim on a throw spot: the place it decided to throw a grenade
/// at, stamped when it decided. A teammate's roll skips places within
/// `CombatTuning::grenade_claim_m` of one for `grenade_claim_ms`.
///
/// Root cause (`grenades-record.md` 2.4): two bots of a team throwing at
/// one spot waste the second grenade (the blast is 10 m; humans' same-team
/// throws land within 8 m of a teammate's during the next 5 s in 1.7 % of
/// throws, twice the chance level, so they do not avoid it by design). The
/// owner: do not make several bots throw at the same spots. A spot claim
/// is a place where a bot stands, 1.5 m apart ([`Claim`]); a throw claim
/// a blast, metres apart, kept for the fuse and what follows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThrowClaim {
    pub who: Place,
    pub at: Vec3,
    pub time: u32,
}

/// A throw claim is dropped after this, ms, whatever the roll's own window
/// asks (the fuse is 4 s; `grenade_claim_ms` is 6 s).
pub const THROW_CLAIM_KEEP_MS: u32 = 30_000;

/// What a team's bots share: callouts (one bot's sense, the others' a
/// moment later), danger spots, thefts, and with the battery cheat off
/// what they know of the batteries and the spawners they have looked at.
#[derive(Clone, Debug, Default)]
pub struct TeamKnowledge {
    /// Callouts: (when the others know it, the percept, the places that
    /// took it already as a bit mask).
    callouts: Vec<(u32, Percept, u64)>,
    pub danger: Vec<DangerSpot>,
    /// "Shot from there" callouts of the last 20 s.
    pub shots_from: Vec<ShotFrom>,
    pub thefts: Vec<Theft>,
    /// Batteries of ours taken from home this match, by anyone (the
    /// container holder's need, `guard_lead`; [`Self::thefts`] keeps 60 s).
    pub thefts_match: u32,
    /// Each enemy human's lanes, thefts and kills, for the match (CG-12).
    pub humans: HumanLanes,
    /// The bear traps the team's bots put down (`trap_know.rs`).
    pub traps: super::trap_know::TrapList,
    /// By battery number.
    pub batteries: Vec<Option<KnownBattery>>,
    /// By battery spawner: when a bot of the team last saw its spot.
    pub spawners_seen: Vec<Option<u32>>,
    /// The team's bots checking spawners now and the spawner each looks
    /// at (the commander's, from the plays it handed out).
    pub searching: Vec<(Place, usize)>,
    /// The team's deaths of the last [`LOSS_KEEP_MS`].
    pub losses: Vec<Loss>,
    /// Enemies seen carrying lately.
    pub carriers_seen: Vec<CarrierSeen>,
    /// Where enemy carriers died with a battery the team cannot see.
    pub looks: Vec<LookSpot>,
    /// Enemy carriers' places of the last [`CARRIER_PLACES_KEEP_MS`].
    pub carrier_places: Vec<CarrierPlace>,
    /// The team's falls by batteries of the last [`FALLS_KEEP_MS`].
    pub falls: Vec<Fall>,
    /// What changed in [`Self::batteries`] and why, for the log (taken by
    /// the bots' notes; [`Self::change_battery`]).
    pub battery_notes: Vec<String>,
    /// The known lying batteries of each bot of the team: why it may not
    /// take one now (`None`: it may), and since when (match ms), for the
    /// notes (`collect::take_blocks`): a note goes out when it changes.
    pub take_blocked: Vec<(Place, usize, Option<&'static str>, u32)>,
    /// Plays a bot proved no way to, at a target whose goal lies on a floor of a zone, and until when (match ms) none
    /// of the team is offered them (`HazardTuning::team_zone_aside`): the proof is the zone's, not the bot's.
    aside: Vec<(PlayKind, TargetRef, u32)>,
    /// The team's bots' claims on spots, one a bot ([`Self::claim`]).
    claims: Vec<Claim>,
    /// The routes the team's Roam runs were drawn on, one a bot ([`Self::claim_route`]).
    route_claims: Vec<RouteClaim>,
    /// The places the team's bots decided to throw at, each throw a claim
    /// ([`Self::claim_throw`]).
    throws: Vec<ThrowClaim>,
}

impl TeamKnowledge {
    /// A bot of the team sensed something: its teammates know it after
    /// `delay_ms`. Only first-hand senses are called out.
    pub fn call_out(&mut self, by: Place, percept: &Percept, delay_ms: u32) {
        if !matches!(percept.source, Source::Seen | Source::Heard) {
            return;
        }
        self.callouts.push((
            percept.time + delay_ms,
            Percept {
                source: Source::Told { by },
                ..*percept
            },
            0,
        ));
    }

    /// The callouts the bot at `me` hears by `now` (not its own), each
    /// once (handed back at every think, a callout came again with its old
    /// time and place over what the bot sensed since), and the old ones
    /// dropped.
    pub fn heard_by(&mut self, me: Place, now: u32) -> Vec<Percept> {
        self.callouts.retain(|(at, _, _)| *at + 3_000 >= now);
        // Places past the mask's 64 take a callout at every think, as
        // before (the memory takes none older than what it knows).
        let bit = (me < 64).then(|| 1u64 << me);
        let mut out = Vec::new();
        for (at, percept, taken) in &mut self.callouts {
            if *at > now
                || percept.source == (Source::Told { by: me })
                || bit.is_some_and(|bit| *taken & bit != 0)
            {
                continue;
            }
            *taken |= bit.unwrap_or(0);
            out.push(*percept);
        }
        out
    }

    /// Danger spots of the last `ms`.
    pub fn danger_within(&self, now: u32, ms: u32) -> impl Iterator<Item = &DangerSpot> {
        self.danger.iter().filter(move |spot| spot.time + ms >= now)
    }

    /// `who` means to be at `at` (made again at every think).
    pub fn claim(&mut self, who: Place, at: Vec3, now: u32) {
        match self.claims.iter_mut().find(|c| c.who == who) {
            Some(c) => (c.at, c.time) = (at, now),
            None => self.claims.push(Claim { who, at, time: now }),
        }
    }

    /// `who` runs along route `route` of `side`'s, drawn at `drawn`, and is on it at `now`.
    pub fn claim_route(&mut self, who: Place, side: i32, route: usize, drawn: u32, now: u32) {
        let claim = RouteClaim { who, side, route, drawn, seen: now };
        match self.route_claims.iter_mut().find(|c| c.who == who) {
            Some(c) => *c = claim,
            None => self.route_claims.push(claim),
        }
    }

    /// `who` runs along no route any more.
    pub fn release_route(&mut self, who: Place) {
        self.route_claims.retain(|c| c.who != who);
    }

    /// The runs of bots other than `except` on `side`'s route `route`, drawn within `fresh_ms` of `now`.
    pub fn route_claims_of_others(
        &self,
        except: Place,
        side: i32,
        route: usize,
        now: u32,
        fresh_ms: u32,
    ) -> impl Iterator<Item = &RouteClaim> {
        self.route_claims.iter().filter(move |c| {
            c.who != except
                && c.side == side
                && c.route == route
                && c.seen + CLAIM_KEEP_MS >= now
                && c.drawn <= now
                && now - c.drawn <= fresh_ms
        })
    }

    /// The claims of bots other than `except` that are still kept at `now`.
    pub fn claims_of_others(&self, except: Place, now: u32) -> impl Iterator<Item = &Claim> {
        self.claims
            .iter()
            .filter(move |c| c.who != except && c.time + CLAIM_KEEP_MS >= now)
    }

    /// `who` decided to throw a grenade at `at` (one claim a throw: a bot's
    /// own volley is not held against it, [`Self::throws_of_others`]).
    pub fn claim_throw(&mut self, who: Place, at: Vec3, now: u32) {
        self.throws.push(ThrowClaim { who, at, time: now });
    }

    /// The throw claims of bots other than `except` made within `ms` of
    /// `now`.
    pub fn throws_of_others(
        &self,
        except: Place,
        now: u32,
        ms: u32,
    ) -> impl Iterator<Item = &ThrowClaim> {
        self.throws
            .iter()
            .filter(move |c| c.who != except && c.time + ms >= now)
    }

    /// The player at `who` left the match: no callout about it is heard,
    /// and no theft names it. The battery it took is still where it was
    /// last known.
    pub fn forget(&mut self, who: Place) {
        self.forget_callouts(who);
        self.claims.retain(|c| c.who != who);
        self.release_route(who);
        for theft in &mut self.thefts {
            if theft.thief == Some(who) {
                theft.thief = None;
            }
        }
    }

    /// The enemy at `who` died: no callout about him is heard any more (he
    /// is not where he was; a callout of the moment before his death would
    /// bring him back to the memories that forgot him).
    pub fn forget_callouts(&mut self, who: Place) {
        self.callouts.retain(|(_, percept, _)| percept.who != who);
    }

    /// Where the team believes battery `battery` lies.
    pub fn battery(&self, battery: usize) -> Option<&KnownBattery> {
        self.batteries.get(battery)?.as_ref()
    }

    /// Battery `battery` lies at `at` as far as the team knows (`None`:
    /// it does not know).
    pub fn set_battery(&mut self, battery: usize, known: Option<KnownBattery>) {
        if self.batteries.len() <= battery {
            if known.is_none() {
                return;
            }
            self.batteries.resize(battery + 1, None);
        }
        self.batteries[battery] = known;
    }

    /// [`Self::set_battery`], with a note of the change and `why` when the
    /// team comes to know the battery, forgets it, sees it moved over 1 m,
    /// or it comes onto or goes off the HUD; a spot only put right again
    /// writes nothing.
    pub fn change_battery(
        &mut self,
        battery: usize,
        known: Option<KnownBattery>,
        why: impl FnOnce() -> String,
    ) {
        let was = self.battery(battery).copied();
        let text = match (was, known) {
            (None, Some(now)) => Some(format!(
                "battery {battery} known at {} ({})",
                spot(now),
                why()
            )),
            (Some(then), None) => Some(format!(
                "battery {battery} no longer known at {} ({})",
                spot(then),
                why()
            )),
            (Some(then), Some(now)) if (then.at - now.at).length() > 1.0 || then.hud != now.hud => {
                Some(format!(
                    "battery {battery} known at {}, was {} ({})",
                    spot(now),
                    spot(then),
                    why()
                ))
            }
            _ => None,
        };
        self.battery_notes.extend(text);
        self.set_battery(battery, known);
    }

    /// The batteries on the HUD of team `team`, from two worlds' states
    /// (`player_bots_behaviour.md` §2). The client (PROVEN in 0.21d's
    /// `gather_victory_items_rule::fill_match_hud_state` 0x1FBDF4 and
    /// `victory_item_core::initialize`/`deactivate`, the same in 0.26g's
    /// 0x3140C0, 0x300BEA, 0x30104E, 0x3010A4) shows a lying battery to a
    /// team whose bit is set in the battery's mask; the mask is empty when
    /// the battery appears, set to the picker's team alone on a pickup, to
    /// the carrier's team alone on a drop (a death too), emptied when it is
    /// put in, and a player's spot adds his team. The world carries that
    /// mask (`m_spotted_mask`, `MatchView::marks`; `1 << team`: 1 is team
    /// 0's, 2 team 1's, as the matches of 2026-10-16 show: every drop gave
    /// the carrier's team's alone, never both), and a lying battery is on
    /// our HUD exactly when our bit is set. It stays set when the game puts back a
    /// battery that fell through the floor about 2 m away (level_01's hole
    /// at about (-43.5, 3.8), matches 0x1a09…/0x7298…: team 1 lost its
    /// battery for 125 s and 32 s when the HUD was reckoned from moves).
    /// So:
    /// - a battery dropped by our carrier is on our HUD where it lies,
    ///   wherever the game moves it while it lies;
    /// - one our HUD showed goes from it when anybody picks it up, or when
    ///   it is put in;
    /// - one our teammate carries is known by him (the HUD shows him);
    /// - one only seen by a bot stays where it was seen.
    ///
    /// A view without the masks (a tool or test that gives none) reckons
    /// the HUD from the states: our carrier's drop is ours, and a battery
    /// that moved more than 1 m between two worlds left the HUD.
    /// The commander's poll: what team `team`'s bots know of each enemy
    /// human goes into [`Self::humans`] (CG-12). His places from their
    /// own senses and callouts (`tracks`: the bots' memories; the latest of
    /// each human, within [`LANE_CELL_M`] of error), the thefts the HUD
    /// told, and his kills on the scoreboard placed at our latest death of
    /// the last 2 s (the kill feed).
    pub fn learn_humans<'a>(
        &mut self,
        team: u8,
        view: &MatchView,
        tracks: impl Iterator<Item = &'a Track>,
        now: u32,
    ) {
        self.learn_humans_as(team, view, tracks, now, false);
    }

    /// [`Self::learn_humans`], with every body of the other team taken for a human if `enemy_all` (the bench's model of
    /// Player vs Bots, where every enemy of a bot is one: `SRP_BENCH_ENEMY_HUMAN_LANES`; only the lanes read it).
    pub fn learn_humans_as<'a>(
        &mut self,
        team: u8,
        view: &MatchView,
        tracks: impl Iterator<Item = &'a Track>,
        now: u32,
        enemy_all: bool,
    ) {
        let human = |who: Place| {
            view.place(who)
                .is_some_and(|p| (enemy_all || !p.bot) && p.team & 1 != team & 1)
        };
        let mut latest: Vec<(Place, Vec3, u32, f32)> = Vec::new();
        for t in tracks {
            if !t.source.own() || t.error_m > LANE_CELL_M || !human(t.who) {
                continue;
            }
            match latest.iter_mut().find(|l| l.0 == t.who) {
                Some(l) if (t.time, -t.error_m) > (l.2, -l.3) => {
                    *l = (t.who, t.at, t.time, t.error_m)
                }
                Some(_) => {}
                None => latest.push((t.who, t.at, t.time, t.error_m)),
            }
        }
        latest.sort_by_key(|l| l.0);
        for (who, at, time, _) in latest {
            self.humans.knew(who, at, time);
        }
        for theft in &self.thefts {
            if let Some(thief) = theft.thief.filter(|&p| human(p)) {
                self.humans.stole(thief, theft.battery, theft.time);
            }
        }
        let death = self
            .losses
            .iter()
            .filter(|l| l.time + 2_000 >= now)
            .max_by_key(|l| l.time)
            .map(|l| l.at);
        for p in view.places.iter().filter(|p| p.present && human(p.place)) {
            self.humans.shown_kills(p.place, p.kills, death, now);
        }
    }

    pub fn hud_update(&mut self, team: u8, before: &MatchView, after: &MatchView, time: u32) {
        let team_of = |carrier: Option<Place>| carrier.and_then(|p| after.place(p)).map(|p| p.team);
        for (battery, now) in after.batteries.iter().enumerate() {
            let was = before.batteries.get(battery).copied();
            let known = self.battery(battery).copied();
            match *now {
                BatteryState::Free(at) => {
                    let hud = match (after.marks.get(battery), was) {
                        // The client's own HUD.
                        (Some(&mask), _) => mask & (1 << (team & 1)) != 0,
                        // Lying still: as it was.
                        (None, Some(BatteryState::Free(then))) if then.distance_xz(at) <= 1.0 => {
                            known.is_some_and(|k| k.hud)
                        }
                        (None, Some(BatteryState::Carried(carrier))) => {
                            team_of(carrier) == Some(team)
                        }
                        // New, moved unseen, or out of a container.
                        _ => false,
                    };
                    if hud {
                        self.change_battery(battery, Some(KnownBattery { at, time, hud }), || {
                            "the HUD shows it".into()
                        });
                    } else if known.is_some_and(|k| k.hud) {
                        self.change_battery(battery, None, || "the HUD no longer shows it".into());
                    }
                }
                BatteryState::Carried(carrier) => {
                    if known.is_some_and(|k| k.hud) || team_of(carrier) == Some(team) {
                        self.change_battery(battery, None, || match carrier {
                            Some(p) => format!("carried by player {p}"),
                            None => "carried".into(),
                        });
                    }
                }
                BatteryState::Home(id) => {
                    if known.is_some_and(|k| k.hud) {
                        self.change_battery(battery, None, || format!("put in container {id}"));
                    }
                }
            }
        }
    }

    /// When a bot of the team last saw spawner `spawner`'s spot.
    pub fn spawner_seen(&self, spawner: usize) -> Option<u32> {
        self.spawners_seen.get(spawner).copied().flatten()
    }

    pub fn saw_spawner(&mut self, spawner: usize, now: u32) {
        if self.spawners_seen.len() <= spawner {
            self.spawners_seen.resize(spawner + 1, None);
        }
        self.spawners_seen[spawner] = Some(now);
    }

    pub fn forget_old(&mut self, now: u32) {
        self.danger.retain(|spot| spot.time + 30_000 >= now);
        self.shots_from.retain(|shot| shot.time + 20_000 >= now);
        self.thefts.retain(|theft| theft.time + 60_000 >= now);
        self.losses.retain(|loss| loss.time + LOSS_KEEP_MS >= now);
        self.carriers_seen
            .retain(|seen| seen.time + CARRIER_SEEN_KEEP_MS >= now);
        self.looks.retain(|look| look.until >= now);
        self.carrier_places
            .retain(|p| p.time + CARRIER_PLACES_KEEP_MS >= now);
        self.falls.retain(|f| f.time + FALLS_KEEP_MS >= now);
        self.aside.retain(|&(_, _, until)| until > now);
        self.claims.retain(|c| c.time + CLAIM_KEEP_MS >= now);
        self.route_claims.retain(|c| c.seen + CLAIM_KEEP_MS >= now);
        self.throws.retain(|c| c.time + THROW_CLAIM_KEEP_MS >= now);
    }

    /// The whole team leaves play `kind` at `target` alone until `until` (match ms): a bot's search proved no way to
    /// its goal on a floor of a zone.
    pub fn set_aside(&mut self, kind: PlayKind, target: TargetRef, until: u32) {
        match self.aside.iter_mut().find(|(k, t, _)| *k == kind && *t == target) {
            Some(old) => old.2 = old.2.max(until),
            None => self.aside.push((kind, target, until)),
        }
    }

    /// Whether the team leaves play `kind` at `target` alone at `now` ([`Self::set_aside`]).
    pub fn is_aside(&self, kind: PlayKind, target: TargetRef, now: u32) -> bool {
        self.aside
            .iter()
            .any(|&(k, t, until)| k == kind && t == target && now < until)
    }

    /// `who`, going for a battery lying at `at`, fell near it (once in
    /// [`FALL_SAME_MS`] a bot).
    pub fn fell_near(&mut self, who: Place, at: Vec3, time: u32) {
        if self
            .falls
            .iter()
            .any(|f| f.who == who && f.time + FALL_SAME_MS > time)
        {
            return;
        }
        self.falls.push(Fall { who, at, time });
    }

    /// Whether a battery lying at `at` is set aside for the team: `count`
    /// falls at it within `window_ms`, the last less than `aside_ms` ago
    /// (0x1b8ee975: battery 6 lay 455 s by a ledge, 166 Collect plays, 17
    /// falls, 7 of them deaths).
    pub fn fallen_at(
        &self,
        at: Vec3,
        now: u32,
        count: usize,
        window_ms: u32,
        aside_ms: u32,
    ) -> bool {
        let falls: Vec<u32> = self
            .falls
            .iter()
            .filter(|f| {
                f.at.distance_xz(at) <= FALL_SPOT_M
                    && (f.at.y - at.y).abs() <= FALL_SPOT_UP_M
                    && f.time + window_ms >= now
            })
            .map(|f| f.time)
            .collect();
        count > 0
            && falls.len() >= count
            && falls
                .iter()
                .max()
                .is_some_and(|&last| now < last + aside_ms)
    }

    /// A bot of the team sensed the enemy carrier at `who` at `at`, within
    /// `error_m`. `carrying(place)`: whether a place carries now; the
    /// places of one who no longer does are dropped (a new carry starts
    /// afresh).
    pub fn carrier_at(
        &mut self,
        who: Place,
        at: Vec3,
        error_m: f32,
        time: u32,
        carrying: impl Fn(Place) -> bool,
    ) {
        self.carrier_places.retain(|p| carrying(p.who));
        self.carrier_places.push(CarrierPlace {
            who,
            at,
            error_m,
            time,
        });
    }

    /// Where the enemy carrier at `who` has stayed: his places of the last
    /// `window_ms` and more (the first at least that old) all lie within
    /// their error and `still_m` of their middle, three of them at least;
    /// that middle. A thief cheat's place (10 m off) is drawn every 3 s,
    /// so a carrier who walks home on it is seen to move within seconds.
    pub fn carrier_still(&self, who: Place, now: u32, window_ms: u32, still_m: f32) -> Option<Vec3> {
        let places: Vec<&CarrierPlace> = self
            .carrier_places
            .iter()
            .filter(|p| p.who == who)
            .collect();
        let first = places.iter().map(|p| p.time).min()?;
        if places.len() < 3 || first + window_ms > now {
            return None;
        }
        let n = places.len() as f32;
        let sum = places.iter().fold(Vec3::ZERO, |sum, p| sum + p.at);
        let middle = sum * (1.0 / n);
        places
            .iter()
            .all(|p| p.at.distance_xz(middle) <= p.error_m + still_m)
            .then_some(middle)
    }

    /// A bot of the team saw the enemy at `who` carrying `battery`.
    pub fn saw_carrying(&mut self, who: Place, battery: usize, at: Vec3, time: u32) {
        self.carriers_seen.retain(|seen| seen.who != who);
        self.carriers_seen.push(CarrierSeen {
            who,
            battery,
            at,
            time,
        });
    }

    /// Whether the team knows the enemy at `who` carried `battery`: he
    /// stole it from us (the HUD's theft), or a bot saw him carrying it.
    pub fn knew_carrying(&self, who: Place, battery: usize) -> bool {
        self.thefts
            .iter()
            .any(|t| t.battery == battery && t.thief == Some(who))
            || self
                .carriers_seen
                .iter()
                .any(|s| s.who == who && s.battery == battery)
    }

    /// Where the team looks for `battery`, dropped by a carrier it lost.
    pub fn add_look(&mut self, look: LookSpot) {
        self.looks.retain(|l| l.battery != look.battery);
        self.looks.push(look);
    }

    /// Where the team looks for `battery`, dropped by a thief alive: the
    /// spots `at`, until `until`.
    pub fn add_theft_looks(&mut self, battery: usize, at: &[Vec3], time: u32, until: u32) {
        self.looks.retain(|l| l.battery != battery);
        self.looks.extend(at.iter().map(|&at| LookSpot {
            battery,
            at,
            time,
            until,
            theft: true,
        }));
    }

    /// The look spots still worth a look for team `team`: not the ones
    /// whose battery the team knows of now (seen lying, on its HUD, home,
    /// or carried by a teammate).
    pub fn settle_looks(&mut self, team: u8, view: &MatchView) {
        if self.looks.is_empty() {
            return;
        }
        let known: Vec<bool> = (0..view.batteries.len())
            .map(|b| {
                self.battery(b).is_some()
                    || match view.batteries[b] {
                        BatteryState::Home(_) => true,
                        BatteryState::Carried(Some(p)) => {
                            view.place(p).is_some_and(|p| p.team == team)
                        }
                        _ => false,
                    }
            })
            .collect();
        self.looks
            .retain(|look| !known.get(look.battery).copied().unwrap_or(true));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A throw claim holds against the teammates' rolls, not the thrower's
    /// own, for the window the roll asks, and is dropped much later.
    #[test]
    fn a_throw_claim_holds_against_others_for_its_window() {
        let mut team = TeamKnowledge::default();
        let at = Vec3::new(10.0, 0.0, 20.0);
        team.claim_throw(3, at, 1_000);
        assert_eq!(team.throws_of_others(4, 5_000, 6_000).count(), 1);
        assert_eq!(team.throws_of_others(3, 5_000, 6_000).count(), 0, "his own volley");
        assert_eq!(team.throws_of_others(4, 7_001, 6_000).count(), 0, "after the window");
        team.forget_old(31_001);
        assert_eq!(team.throws_of_others(4, 31_001, 60_000).count(), 0, "dropped");
    }

    fn seen(who: Place, time: u32) -> Percept {
        Percept {
            who,
            at: Vec3::new(10.0, 0.0, 0.0),
            error_m: 0.0,
            time,
            source: Source::Seen,
            busy: false,
        }
    }

    /// A carrier the thief cheat places within its 10 m every 3 s is still
    /// once 20 s of places lie within their error and 3 m of their middle;
    /// one who walks home is not, nor one known for less than 20 s, and a
    /// new carry starts afresh.
    #[test]
    fn a_carrier_is_still_when_his_places_keep_within_their_error() {
        let mut team = TeamKnowledge::default();
        let at = Vec3::new(50.0, 0.0, 30.0);
        let offsets = [(9.0f32, 0.0f32), (-6.0, 6.0), (0.0, -9.0), (5.0, 5.0)];
        for k in 0..8u32 {
            let (dx, dz) = offsets[k as usize % 4];
            team.carrier_at(1, at + Vec3::new(dx, 0.0, dz), 10.0, k * 3_000, |_| true);
        }
        let still = team.carrier_still(1, 21_000, 20_000, 3.0).expect("still");
        assert!(still.distance_xz(at) < 3.0, "{still:?}");
        assert_eq!(team.carrier_still(1, 19_000, 20_000, 3.0), None, "too short");
        assert_eq!(team.carrier_still(2, 21_000, 20_000, 3.0), None);
        // Walking home at 1.5 m/s on the same cheat: not still.
        let mut walking = TeamKnowledge::default();
        for k in 0..8u32 {
            let (dx, dz) = offsets[k as usize % 4];
            let on = Vec3::new(4.5 * k as f32, 0.0, 0.0);
            walking.carrier_at(1, at + on + Vec3::new(dx, 0.0, dz), 10.0, k * 3_000, |_| true);
        }
        assert_eq!(walking.carrier_still(1, 21_000, 20_000, 3.0), None);
        // He no longer carries: the next place starts a new carry.
        team.carrier_at(3, at, 0.0, 22_000, |p| p != 1);
        assert_eq!(team.carrier_still(1, 22_000, 20_000, 3.0), None);
    }

    /// [`Memory::in_sight`] skips only the noticing delay: never a track
    /// not seen, one only heard, or one out of sight over 450 ms.
    #[test]
    fn in_sight_is_a_live_sight_before_its_noticing() {
        let mut memory = Memory::default();
        assert!(memory.in_sight(3, 1_000, 1_000).is_none(), "never seen");
        memory.update(&[seen(3, 1_000)], 1_000, |_| 300, 3_000, 20_000);
        assert!(memory.seen(3, 1_100, 1_100).is_none());
        assert_eq!(memory.in_sight(3, 1_100, 1_100).map(|t| t.who()), Some(3));
        let heard = Percept {
            source: Source::Heard,
            ..seen(4, 1_000)
        };
        memory.update(&[heard], 1_000, |_| 300, 3_000, 20_000);
        assert!(memory.in_sight(4, 1_100, 1_100).is_none(), "only heard");
        memory.update(&[], 1_500, |_| 300, 3_000, 20_000);
        assert!(memory.in_sight(3, 1_500, 1_500).is_none(), "out of sight over 400 ms");
    }

    /// The noticing delay is asked when a sight begins, not at every report
    /// of it (a delay that costs, [`crate::v026g::bots::foliage`], is worked out once).
    #[test]
    fn the_noticing_delay_is_asked_once_for_a_sight() {
        let asked = std::cell::Cell::new(0);
        let notice = |_: &Percept| {
            asked.set(asked.get() + 1);
            300
        };
        let mut memory = Memory::default();
        memory.update(&[seen(3, 1_000)], 1_000, notice, 3_000, 20_000);
        for k in 1..5u32 {
            memory.update(&[seen(3, 1_000 + 200 * k)], 1_000 + 200 * k, notice, 3_000, 20_000);
        }
        assert_eq!(asked.get(), 1, "one sight, one question");
        // Out of sight for a while, then seen again: a new sight asks again.
        memory.update(&[seen(3, 3_000)], 3_000, notice, 3_000, 20_000);
        assert_eq!(asked.get(), 2);
    }

    #[test]
    fn a_sight_is_a_seen_track_only_after_the_noticing_delay() {
        let mut memory = Memory::default();
        memory.update(&[seen(3, 1_000)], 1_000, |_| 300, 3_000, 20_000);
        assert!(memory.seen(3, 1_000, 1_000).is_none());
        assert!(memory.seen(3, 1_200, 1_200).is_none());
        memory.update(&[seen(3, 1_300)], 1_300, |_| 300, 3_000, 20_000);
        assert_eq!(memory.seen(3, 1_300, 1_300).map(|t| t.who()), Some(3));
        // Out of sight: no seen track, but it is still known.
        memory.update(&[], 2_000, |_| 300, 3_000, 20_000);
        assert!(memory.seen(3, 2_000, 2_000).is_none());
        assert_eq!(memory.tracks().len(), 1);
        // Seen again: a new noticing delay.
        memory.update(&[seen(3, 2_500)], 2_500, |_| 300, 3_000, 20_000);
        assert!(memory.seen(3, 2_600, 2_600).is_none());
        // A heard report does not make a seen track, and old ones go.
        let heard = Percept {
            source: Source::Heard,
            ..seen(4, 3_000)
        };
        memory.update(&[heard], 3_000, |_| 300, 3_000, 20_000);
        assert!(memory.seen(4, 9_000, 9_000).is_none());
        memory.update(&[], 30_000, |_| 300, 3_000, 20_000);
        assert!(memory.tracks().is_empty());
    }

    fn battery_view(batteries: Vec<BatteryState>) -> MatchView {
        use crate::v026g::bots::view::{Mode, PlaceView};
        let place = |place: Place, team: u8| PlaceView {
            place,
            team,
            bot: true,
            present: true,
            alive: true,
            feet: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            velocity: [0.0, 0.0],
            keys: 0,
            health: 1.0,
            spawned: 0,
            kills: 0,
            deaths: 0,
            streak: 0,
            carrying: None,
            seen_at: 0,
            late_ms: 0,
            known_at: 0,
        };
        MatchView {
            clock: 10_000,
            mode: Mode::Batteries,
            time_left_s: 600.0,
            total_s: 900.0,
            points: [0, 0],
            places: vec![place(0, 0), place(1, 1)],
            batteries,
            marks: vec![],
            containers: vec![],
            sounds: vec![],
            deaths: vec![],
            world_time: 10_000,
            can_respawn: [true, true],
            next_battery: None,
            grenades: Vec::new(),
        }
    }

    /// The client's HUD mask (`hud_update`): a battery that appears is on
    /// no HUD; one our carrier dropped is on ours where it lies, not on
    /// theirs; picked up by them it leaves our HUD, and put in it is on
    /// none. A battery only seen stays where it was seen.
    #[test]
    fn the_hud_shows_what_the_team_last_carried_and_drops_it_when_taken() {
        let (a, b) = (Vec3::new(10.0, 0.0, 10.0), Vec3::new(20.0, 0.0, 5.0));
        let mut ours = TeamKnowledge::default();
        let mut theirs = TeamKnowledge::default();
        let step = |ours: &mut TeamKnowledge, theirs: &mut TeamKnowledge, from, to| {
            let (before, after) = (battery_view(from), battery_view(to));
            ours.hud_update(0, &before, &after, 1);
            theirs.hud_update(1, &before, &after, 1);
        };
        // It appears: nobody's HUD.
        step(&mut ours, &mut theirs, vec![], vec![BatteryState::Free(a)]);
        assert!(ours.battery(0).is_none() && theirs.battery(0).is_none());
        // Our carrier (place 0) takes it and drops it at b (he died).
        step(
            &mut ours,
            &mut theirs,
            vec![BatteryState::Free(a)],
            vec![BatteryState::Carried(Some(0))],
        );
        step(
            &mut ours,
            &mut theirs,
            vec![BatteryState::Carried(Some(0))],
            vec![BatteryState::Free(b)],
        );
        assert_eq!(ours.battery(0).map(|k| (k.at, k.hud)), Some((b, true)));
        assert!(theirs.battery(0).is_none());
        // Lying still: still on our HUD.
        step(
            &mut ours,
            &mut theirs,
            vec![BatteryState::Free(b)],
            vec![BatteryState::Free(b)],
        );
        assert!(ours.battery(0).is_some_and(|k| k.hud));
        // Their player takes it: off our HUD; dropped, on theirs.
        step(
            &mut ours,
            &mut theirs,
            vec![BatteryState::Free(b)],
            vec![BatteryState::Carried(Some(1))],
        );
        assert!(ours.battery(0).is_none());
        step(
            &mut ours,
            &mut theirs,
            vec![BatteryState::Carried(Some(1))],
            vec![BatteryState::Free(a)],
        );
        assert!(ours.battery(0).is_none());
        assert!(theirs.battery(0).is_some_and(|k| k.hud && k.at == a));
        // Put in: on no HUD.
        step(
            &mut ours,
            &mut theirs,
            vec![BatteryState::Free(a)],
            vec![BatteryState::Home(1)],
        );
        assert!(theirs.battery(0).is_none());
        // Seen by one of our bots, then taken by them: still believed
        // where it was seen (no HUD said otherwise); taken by our own
        // teammate, known to be his.
        ours.set_battery(
            1,
            Some(KnownBattery {
                at: a,
                time: 1,
                hud: false,
            }),
        );
        let lying = vec![BatteryState::Home(1), BatteryState::Free(a)];
        let taken = vec![BatteryState::Home(1), BatteryState::Carried(Some(1))];
        step(&mut ours, &mut theirs, lying.clone(), taken);
        assert_eq!(ours.battery(1).map(|k| k.at), Some(a));
        let ours_taken = vec![BatteryState::Home(1), BatteryState::Carried(Some(0))];
        step(&mut ours, &mut theirs, lying, ours_taken);
        assert!(ours.battery(1).is_none());
    }

    /// Matches 0x1a09…/0x7298… (level_01's floor hole at about (-43.5,
    /// 3.8)): team 1's carrier died, the battery fell (y 19.6 -> -86) and
    /// the game put it back 2 m away; the world's mask stayed 2 (team 1's
    /// HUD) all the while, and so does the team's knowledge. Team 0's HUD
    /// never had it. An enemy who picks it up and drops it between two
    /// worlds within a metre takes it off our HUD (the mask says his team).
    #[test]
    fn the_hud_follows_the_worlds_mask_through_a_fall_and_a_reset() {
        let marked = |batteries: Vec<BatteryState>, marks: Vec<u32>| MatchView {
            marks,
            ..battery_view(batteries)
        };
        let mut teams = [TeamKnowledge::default(), TeamKnowledge::default()];
        let mut step = |from: &MatchView, to: &MatchView, time| {
            for (team, knowledge) in teams.iter_mut().enumerate() {
                knowledge.hud_update(team as u8, from, to, time);
            }
            teams.clone().map(|k| k.battery(0).copied())
        };
        let dropped = Vec3::new(-43.58, 19.55, 3.80);
        let fallen = Vec3::new(-43.58, -86.0, 3.80);
        let reset = Vec3::new(-41.6, 21.6, 3.5);
        let spawned = marked(vec![BatteryState::Free(Vec3::ZERO)], vec![0]);
        let carried = marked(vec![BatteryState::Carried(Some(1))], vec![0]);
        let lying = |at| marked(vec![BatteryState::Free(at)], vec![2]);
        assert_eq!(step(&spawned, &spawned, 1), [None, None]);
        step(&spawned, &carried, 2);
        let [ours, theirs] = step(&carried, &lying(dropped), 3);
        assert!(ours.is_none());
        assert_eq!(theirs.map(|k| (k.at, k.hud)), Some((dropped, true)));
        // It falls, and the game puts it back 2 m away: still on team 1's
        // HUD, where it lies now.
        step(&lying(dropped), &lying(fallen), 4);
        let [ours, theirs] = step(&lying(fallen), &lying(reset), 5);
        assert!(ours.is_none());
        assert!(dropped.distance_xz(reset) > 1.9);
        assert_eq!(
            theirs.map(|k| (k.at, k.time, k.hud)),
            Some((reset, 5, true))
        );
        let [_, theirs] = step(&lying(reset), &lying(reset), 6);
        assert_eq!(theirs.map(|k| k.at), Some(reset));
        // Team 0's player picks it up and drops it within a metre between
        // two worlds: the mask is team 0's now.
        let near = reset + Vec3::new(0.5, 0.0, 0.0);
        let theirs_now = marked(vec![BatteryState::Free(near)], vec![1]);
        let [ours, theirs] = step(&lying(reset), &theirs_now, 7);
        assert_eq!(ours.map(|k| (k.at, k.hud)), Some((near, true)));
        assert!(theirs.is_none());
        // Put in: on no HUD.
        let home = marked(vec![BatteryState::Home(0)], vec![0]);
        assert_eq!(step(&theirs_now, &home, 8), [None, None]);
    }

    #[test]
    fn callouts_reach_teammates_later_and_never_cheats() {
        let mut team = TeamKnowledge::default();
        team.call_out(1, &seen(5, 1_000), 1_000);
        let cheat = Percept {
            source: Source::Cheat(CheatKind::Radar),
            ..seen(6, 1_000)
        };
        team.call_out(1, &cheat, 1_000);
        assert!(team.heard_by(2, 1_500).is_empty());
        let heard = team.heard_by(2, 2_000);
        assert_eq!(heard.len(), 1);
        assert_eq!(heard[0].source, Source::Told { by: 1 });
        assert!(team.heard_by(1, 2_000).is_empty(), "not its own");
    }

    /// Match 0x5402a730: the human's track in the enemy bots' memories was
    /// told 252 times, heard 3. A teammate's callout (older, or weaker
    /// than a fresh report) does not replace what the bot heard itself; a
    /// cheat does not replace a fresh callout; a stale report gives way to
    /// a newer one of any kind.
    #[test]
    fn a_callout_does_not_wipe_the_bots_own_hearing() {
        let at = |x: f32| Vec3::new(x, 0.0, 0.0);
        let report = |source, x, time| Percept {
            source,
            at: at(x),
            ..seen(7, time)
        };
        let told = Source::Told { by: 2 };
        let mut memory = Memory::default();
        let update = |memory: &mut Memory, p: Percept, now| {
            memory.update(&[p], now, |_| 300, 3_000, 20_000);
            let t = memory.tracks()[0];
            (t.source, t.at.x, t.time)
        };
        assert_eq!(
            update(&mut memory, report(Source::Heard, 1.0, 5_000), 5_000),
            (Source::Heard, 1.0, 5_000)
        );
        // A callout of a moment before (its old time and place): kept.
        assert_eq!(
            update(&mut memory, report(told, 2.0, 4_200), 5_200),
            (Source::Heard, 1.0, 5_000)
        );
        // A callout newer but weaker while the hearing is fresh: kept.
        assert_eq!(
            update(&mut memory, report(told, 3.0, 5_100), 5_400),
            (Source::Heard, 1.0, 5_000)
        );
        // Heard again: the newer hearing.
        assert_eq!(
            update(&mut memory, report(Source::Heard, 4.0, 5_500), 5_500),
            (Source::Heard, 4.0, 5_500)
        );
        // The hearing stale (1.5 s): a newer callout takes it.
        assert_eq!(
            update(&mut memory, report(told, 5.0, 6_100), 7_100),
            (told, 5.0, 6_100)
        );
        // A cheat does not replace a fresh callout, then does.
        let radar = Source::Cheat(CheatKind::Radar);
        assert_eq!(
            update(&mut memory, report(radar, 6.0, 7_200), 7_200),
            (told, 5.0, 6_100)
        );
        assert_eq!(
            update(&mut memory, report(radar, 6.0, 7_700), 7_700),
            (radar, 6.0, 7_700)
        );
        // A sight replaces anything; a cheat's newer place keeps no sight
        // alive.
        assert_eq!(
            update(&mut memory, report(Source::Seen, 8.0, 7_800), 7_800),
            (Source::Seen, 8.0, 7_800)
        );
        update(&mut memory, report(Source::Seen, 8.0, 8_100), 8_100);
        assert!(memory.seen(7, 8_150, 8_150).is_some());
        let thief = Source::Cheat(CheatKind::Thief);
        update(&mut memory, report(thief, 9.0, 8_300), 8_300);
        assert!(memory.seen(7, 8_560, 8_560).is_none(), "no sight since 8.1 s");
    }

    #[test]
    fn a_callout_is_heard_once_and_a_dead_enemys_not_at_all() {
        let mut team = TeamKnowledge::default();
        team.call_out(1, &seen(5, 1_000), 1_000);
        team.call_out(1, &seen(6, 1_000), 1_000);
        assert_eq!(team.heard_by(2, 2_000).len(), 2);
        assert!(team.heard_by(2, 2_200).is_empty(), "taken already");
        assert_eq!(team.heard_by(3, 2_200).len(), 2, "another bot's own");
        team.forget_callouts(5);
        let left: Vec<Place> = team.heard_by(4, 2_300).iter().map(|p| p.who).collect();
        assert_eq!(left, vec![6]);
    }

    /// Three falls of the team at a battery within 120 s set it aside for
    /// 60 s after the last; one bot's fall is counted once in 5 s, falls at
    /// another battery or long ago do not count, and `count` 0 is off.
    #[test]
    fn falls_at_a_battery_set_it_aside_for_a_while() {
        let mut team = TeamKnowledge::default();
        let at = Vec3::new(5.0, 35.5, 60.0);
        let aside = |team: &TeamKnowledge, now| team.fallen_at(at, now, 3, 120_000, 60_000);
        team.fell_near(1, at, 10_000);
        team.fell_near(1, at, 12_000);
        team.fell_near(2, Vec3::new(20.0, 35.5, 60.0), 13_000);
        team.fell_near(3, at + Vec3::new(0.0, 3.0, 0.0), 14_000);
        team.fell_near(2, at + Vec3::new(1.0, 0.0, 0.0), 20_000);
        assert!(!aside(&team, 21_000), "two falls here: {:?}", team.falls);
        team.fell_near(1, at, 30_000);
        assert!(aside(&team, 31_000));
        assert!(aside(&team, 89_000));
        assert!(!aside(&team, 91_000), "60 s after the last");
        assert!(!team.fallen_at(at, 31_000, 0, 120_000, 60_000), "off");
        // The first fall out of the window: two left.
        assert!(!team.fallen_at(at, 31_000, 3, 15_000, 60_000));
    }

    /// CG-12: an enemy human's places go into his lanes once a second, in
    /// 6 m cells that fade with a 3 minute half-life; a bot's, a
    /// teammate's or a sense with too large an error do not.
    #[test]
    fn an_enemy_humans_places_make_his_lanes_and_fade() {
        let mut view = battery_view(vec![]);
        // Place 2: an enemy human; place 3: a human of ours; place 1: an
        // enemy bot.
        for (place, team) in [(2, 1), (3, 0)] {
            let mut p = view.places[1].clone();
            p.place = place;
            p.team = team;
            p.bot = false;
            view.places.push(p);
        }
        let track = |who: Place, x: f32, time: u32, source: Source, error_m: f32| Track {
            who,
            at: Vec3::new(x, 0.0, 1.0),
            error_m,
            time,
            source,
            sight_since: None,
            notice_ms: 0,
            own: None,
            missed: None,
            held_until: 0,
        };
        let mut team = TeamKnowledge::default();
        let tracks = [
            track(2, 1.0, 1_000, Source::Seen, 0.5),
            track(1, 1.0, 1_000, Source::Seen, 0.5),
            track(3, 1.0, 1_000, Source::Seen, 0.5),
            track(2, 30.0, 1_000, Source::Heard, 10.0),
        ];
        team.learn_humans(0, &view, tracks.iter(), 1_000);
        // Another bot's sight 0.4 s later is the same second: not counted.
        let later = [track(2, 3.0, 1_400, Source::Told { by: 4 }, 2.0)];
        team.learn_humans(0, &view, later.iter(), 1_400);
        assert_eq!(team.humans.humans.len(), 1, "{:?}", team.humans);
        let him = team.humans.get(2).unwrap();
        assert_eq!(him.cells.len(), 1);
        assert_eq!(him.cells[0].count, 1.0);
        // A second later in the same cell: counted, at the mean.
        let again = [track(2, 3.0, 2_000, Source::Told { by: 4 }, 2.0)];
        team.learn_humans(0, &view, again.iter(), 2_000);
        let cell = team.humans.get(2).unwrap().cells[0];
        assert!((cell.count - 2.0).abs() < 0.01, "{cell:?}");
        assert!((cell.at.x - 2.0).abs() < 0.01, "{cell:?}");
        // A cheat's place is not the team's knowledge.
        let cheat = [track(2, 50.0, 4_000, Source::Cheat(CheatKind::Radar), 0.0)];
        team.learn_humans(0, &view, cheat.iter(), 4_000);
        assert_eq!(team.humans.get(2).unwrap().cells.len(), 1);
        // Half of it 3 minutes on.
        let half = cell.count_at(2_000 + LANE_HALF_LIFE_MS);
        assert!((half - 1.0).abs() < 0.01, "{half}");
    }

    /// CG-12: the thief's lane near our container, best first; a human
    /// who never stole gives none, and his kills are placed at our latest
    /// death.
    #[test]
    fn the_thief_lane_is_his_cells_near_the_container_by_count_and_thefts() {
        let mut lanes = HumanLanes::default();
        for s in 0..5u32 {
            lanes.knew(2, Vec3::new(10.0, 0.0, 1.0), s * 1_000);
        }
        lanes.knew(2, Vec3::new(20.0, 0.0, 1.0), 6_000);
        lanes.knew(2, Vec3::new(200.0, 0.0, 1.0), 7_000);
        lanes.knew(5, Vec3::new(30.0, 0.0, 1.0), 6_000);
        let near = Vec3::new(0.0, 0.0, 0.0);
        assert!(lanes.thief_lane(near, 60.0, 8_000).is_empty());
        lanes.stole(2, 4, 7_500);
        lanes.stole(2, 4, 7_500);
        let lane = lanes.thief_lane(near, 60.0, 8_000);
        assert_eq!(lane.len(), 2, "{lane:?}");
        assert!(
            (lane[0].0.x - 10.0).abs() < 0.01 && lane[1].0.x == 20.0,
            "{lane:?}"
        );
        assert_eq!(lanes.get(2).unwrap().thefts, vec![(4, 7_500)]);
        // Kills: the first showing with none, then one at our death.
        lanes.shown_kills(2, 0, None, 1_000);
        lanes.shown_kills(2, 1, Some(Vec3::new(5.0, 0.0, 5.0)), 9_000);
        lanes.shown_kills(2, 1, Some(Vec3::new(6.0, 0.0, 5.0)), 9_500);
        assert_eq!(lanes.get(2).unwrap().kills, vec![(Vec3::new(5.0, 0.0, 5.0), 9_000)]);
    }

    /// A memory with `who` seen at `time`, noticed at once, the reflex's
    /// target (a target is looked at, never held).
    fn sighted(who: Place, time: u32) -> Memory {
        let mut memory = Memory::default();
        memory.set_sight_window(SightWindow::refreshed());
        memory.set_sight_target(Some(who));
        memory.update(&[seen(who, time)], time, |_| 0, 3_000, 20_000);
        memory
    }

    /// 8.13: a target in plain view is never lost to the think cadence:
    /// thinks 200 to 400 ms apart, ticks read 50 or 100 ms ahead of the
    /// information, with the ticks' looks and without.
    #[test]
    fn a_visible_target_never_lapses_with_think_gaps_of_200_to_400_ms() {
        for lead in [50, 100] {
            for gap in [200, 250, 300, 350, 400] {
                for looks in [false, true] {
                    let mut memory = sighted(3, 1_000);
                    let mut next_think = 1_000 + gap;
                    for clock in (1_000..6_000).step_by(50) {
                        if clock >= next_think {
                            memory.update(&[seen(3, clock)], clock, |_| 0, 3_000, 20_000);
                            next_think += gap;
                        } else if looks && clock % 100 == 0 {
                            memory.refresh_sight(3, Vec3::new(10.0, 0.0, 0.0), clock);
                        }
                        assert!(
                            memory.seen(3, clock + lead, clock).is_some(),
                            "gap {gap}, lead {lead}, looks {looks}, clock {clock}"
                        );
                    }
                    // The noticing delay ran once, at the first sight.
                    assert_eq!(memory.tracks()[0].sight_since, Some(1_000));
                }
            }
        }
    }

    /// A failed look ends the sight after exactly the grace on the
    /// information clock; later failures do not extend it; a look that
    /// finds him inside it takes the failure back.
    #[test]
    fn a_failed_look_ends_the_sight_after_the_grace() {
        let at = Vec3::new(10.0, 0.0, 0.0);
        let mut memory = sighted(3, 1_000);
        memory.refresh_sight(3, at, 1_100);
        memory.fail_sight(3, Miss::Blocked, 1_150);
        memory.fail_sight(3, Miss::Cone, 1_200);
        let last = 1_150 + SIGHT_GRACE_MS;
        assert!(memory.seen(3, last, last).is_some());
        assert!(memory.seen(3, last + 1, last + 1).is_none());
        // The place stays known.
        assert_eq!(memory.tracks()[0].at, at);
        assert_eq!(memory.tracks()[0].missed, Some((1_150, Miss::Blocked)));
        // Found again inside the grace: no failure, the sight goes on.
        memory.refresh_sight(3, at, 1_250);
        assert_eq!(memory.tracks()[0].missed, None);
        assert!(memory.seen(3, 1_600, 1_600).is_some());
        assert!(memory.seen(3, 1_701, 1_701).is_none(), "450 ms untested");
        // Found again after the grace: a new sight, noticed anew.
        let mut memory = sighted(3, 1_000);
        memory.fail_sight(3, Miss::Range, 1_100);
        memory.update(&[seen(3, 1_400)], 1_400, |_| 300, 3_000, 20_000);
        assert_eq!(memory.tracks()[0].sight_since, Some(1_400));
        assert!(memory.seen(3, 1_500, 1_500).is_none());
        assert!(memory.seen(3, 1_700, 1_700).is_some());
    }

    /// A look not made keeps the sight up to the untested bound and no
    /// longer; only a look that finds him takes the bound further.
    #[test]
    fn an_untested_sight_is_kept_to_the_bound_and_no_longer() {
        let mut memory = sighted(3, 1_000);
        let last = 1_000 + SIGHT_UNTESTED_MS;
        assert!(memory.seen(3, last + 100, last).is_some());
        assert!(memory.seen(3, last + 100, last + 1).is_none());
        assert!(memory.in_sight(3, last + 1, last + 1).is_none());
        // A look that found him is a test, a stall of the server or not:
        // the sight goes on (a second's stall, the target clear).
        memory.refresh_sight(3, Vec3::new(10.0, 0.0, 0.0), 2_000);
        assert!(memory.seen(3, 2_100, 2_000).is_some());
        assert_eq!(memory.tracks()[0].sight_since, Some(1_000));
        // A failed look at the same stall: the grace, not the bound.
        let mut memory = sighted(3, 1_000);
        memory.fail_sight(3, Miss::Blocked, 2_000);
        assert!(memory.seen(3, 2_000, 2_000).is_none(), "past the bound already");
    }

    /// A think 50 ms after a tick's look still replaces the place: both
    /// are stamped on the information clock.
    #[test]
    fn a_think_after_a_tick_look_still_replaces_the_place() {
        let mut memory = sighted(3, 1_000);
        memory.refresh_sight(3, Vec3::new(11.0, 0.0, 0.0), 1_100);
        let moved = Percept {
            at: Vec3::new(12.0, 0.0, 0.0),
            ..seen(3, 1_150)
        };
        memory.update(&[moved], 1_150, |_| 0, 3_000, 20_000);
        assert_eq!(memory.tracks()[0].at, Vec3::new(12.0, 0.0, 0.0));
        // A look older than the track leaves it alone.
        memory.refresh_sight(3, Vec3::new(13.0, 0.0, 0.0), 1_120);
        assert_eq!(memory.tracks()[0].at, Vec3::new(12.0, 0.0, 0.0));
        assert_eq!(memory.tracks()[0].time, 1_150);
    }

    /// A tick's look touches the time and places of its track, nothing
    /// else: no fading of the others, no new track, no sight begun.
    #[test]
    fn a_look_touches_only_its_tracks_time_place_and_own() {
        let mut memory = sighted(3, 1_000);
        memory.update(&[seen(4, 1_000)], 1_000, |_| 0, 3_000, 20_000);
        let before = memory.tracks().to_vec();
        memory.refresh_sight(3, Vec3::new(11.0, 0.0, 0.0), 5_000);
        memory.refresh_sight(9, Vec3::new(11.0, 0.0, 0.0), 5_000);
        let after = memory.tracks();
        assert_eq!(after.len(), 2);
        assert_eq!(after[1], before[1], "the other track is as it was");
        let track = after[0];
        assert_eq!(
            Track {
                time: before[0].time,
                at: before[0].at,
                own: before[0].own,
                held_until: before[0].held_until,
                ..track
            },
            before[0]
        );
        assert_eq!((track.time, track.own), (5_000, Some((track.at, 5_000))));
        // A sight not begun (heard only) is not made by a look.
        let heard = Percept {
            source: Source::Heard,
            ..seen(7, 1_000)
        };
        memory.update(&[heard], 1_000, |_| 0, 3_000, 20_000);
        memory.refresh_sight(7, Vec3::ZERO, 1_100);
        let heard = memory.tracks().iter().find(|t| t.who == 7).unwrap();
        assert_eq!((heard.time, heard.sight_since), (1_000, None));
    }

    /// A dead enemy ends the sight at once, not after the grace; a new life
    /// is noticed anew.
    #[test]
    fn a_dead_enemy_ends_the_sight_and_the_next_life_begins_another() {
        let mut memory = sighted(3, 1_000);
        memory.fail_sight(3, Miss::Dead, 1_050);
        assert!(memory.seen(3, 1_050, 1_050).is_none());
        assert!(memory.in_sight(3, 1_050, 1_050).is_none());
        memory.update(&[seen(3, 1_100)], 1_100, |_| 300, 3_000, 20_000);
        assert_eq!(memory.tracks()[0].sight_since, Some(1_100));
        assert!(memory.seen(3, 1_200, 1_200).is_none(), "noticed anew");
    }

    /// First sight is as it was: the sight begins at the sensing, and the
    /// noticing delay runs on the tick clock (the clock the shot's reaction
    /// runs on), whatever the lead of the tick over the information.
    #[test]
    fn first_sight_is_noticed_on_the_tick_clock_as_before() {
        for lead in [50, 100] {
            let memory = sighted_with(3, 1_000, 300);
            let track = memory.tracks()[0];
            assert_eq!((track.sight_since, track.notice_ms), (Some(1_000), 300));
            let noticed = |tick: u32| memory.seen(3, tick, tick - lead).is_some();
            assert!(!noticed(1_000 + 299));
            assert!(noticed(1_000 + 300));
            assert!(noticed(1_000 + 400));
        }
    }

    fn sighted_with(who: Place, time: u32, notice: u32) -> Memory {
        let mut memory = Memory::default();
        memory.set_sight_window(SightWindow::refreshed());
        memory.set_sight_target(Some(who));
        memory.update(&[seen(who, time)], time, |_| notice, 3_000, 20_000);
        memory
    }

    /// A memory of a bot that is not looking at `3`: its window, its next
    /// think, `3` seen at 1 s.
    fn held(window: SightWindow, next_think: Option<u32>) -> Memory {
        let mut memory = Memory::default();
        memory.set_sight_window(window);
        if let Some(next) = next_think {
            memory.set_next_think(next);
        }
        memory.update(&[seen(3, 1_000)], 1_000, |_| 0, 3_000, 20_000);
        memory
    }

    /// A sight that is not the target's is held, with no test, to the bot's
    /// next think and a tick (the test that comes next), never past the hold
    /// or the untested bound; at the last tested place only; the target's is
    /// not held, it goes to the bound (it is looked at every tick).
    #[test]
    fn a_sight_that_is_not_the_targets_is_held_to_the_next_think_at_its_last_tested_place() {
        let up_to = |memory: &Memory, last: u32| {
            memory.seen(3, last, last).is_some() && memory.seen(3, last + 1, last + 1).is_none()
        };
        // Think gaps of 200 to 400 ms: held to the next think and a tick.
        for gap in [200, 300, 400] {
            let memory = held(SightWindow::refreshed(), Some(1_000 + gap));
            assert!(up_to(&memory, 1_000 + gap + 50), "gap {gap}");
        }
        // ... never past the hold or the untested bound, however far the
        // think is.
        let memory = held(SightWindow::refreshed(), Some(9_000));
        assert!(up_to(&memory, 1_000 + SIGHT_UNTESTED_MS));
        let short = SightWindow {
            hold_ms: 250,
            ..SightWindow::refreshed()
        };
        assert!(up_to(&held(short, Some(1_400)), 1_250));
        // Fixed from the test (the other end of the A/B), and when the bot's
        // next think is not told.
        let fixed = SightWindow {
            hold_ms: 250,
            hold_to_next_think: false,
            ..SightWindow::refreshed()
        };
        assert!(up_to(&held(fixed, Some(1_400)), 1_250));
        assert!(up_to(&held(SightWindow::refreshed(), None), 1_450));
        let off = SightWindow {
            hold_ms: 0,
            ..fixed
        };
        assert!(up_to(&held(off, None), 1_000), "up at the test only");
        // The target's goes to the bound whatever the hold.
        let mut memory = held(fixed, Some(1_100));
        memory.set_sight_target(Some(3));
        assert!(up_to(&memory, 1_450));
        // The place is the last tested one however long it is held.
        let mut memory = held(SightWindow::refreshed(), Some(1_300));
        memory.refresh_sight(3, Vec3::new(11.0, 0.0, 0.0), 1_100);
        assert_eq!(memory.tracks()[0].at, Vec3::new(11.0, 0.0, 0.0));
        assert!(up_to(&memory, 1_350));
    }

    /// With the refresh off (the default) a sight is the tip's: read on the tick clock, 400 ms after the
    /// think that sensed it, whatever the information clock says, and a look changes nothing.
    #[test]
    fn with_the_refresh_off_a_sight_is_the_tips_and_a_look_changes_nothing() {
        let mut memory = Memory::default();
        memory.update(&[seen(3, 1_000)], 1_000, |_| 0, 3_000, 20_000);
        assert!(memory.seen(3, 1_400, 9_999).is_some(), "the clock of the read is the tick's");
        assert!(memory.seen(3, 1_401, 1_000).is_none());
        assert!(memory.in_sight(3, 1_400, 9_999).is_some());
        assert!(memory.in_sight(3, 1_401, 1_000).is_none());
        let before = memory.tracks().to_vec();
        memory.refresh_sight(3, Vec3::new(11.0, 0.0, 0.0), 1_300);
        memory.fail_sight(3, Miss::Dead, 1_300);
        memory.looked(3, Vec3::ZERO, LookOutcome::Failed(Miss::Blocked), 1_300);
        assert_eq!(memory.tracks(), &before[..]);
        assert_eq!((memory.refreshes(), memory.hold_uses()), (0, 0));
        // A sensing 399 ms on goes on the sight; one 401 ms on is a new one.
        memory.update(&[seen(3, 1_399)], 1_399, |_| 300, 3_000, 20_000);
        assert_eq!(memory.tracks()[0].sight_since, Some(1_000));
        memory.update(&[seen(3, 1_800)], 1_800, |_| 300, 3_000, 20_000);
        assert_eq!(memory.tracks()[0].sight_since, Some(1_800));
    }

    /// The live sights are the begun ones of `Seen` tracks.
    #[test]
    fn the_live_sights_are_those_begun_and_seen() {
        let mut memory = sighted(3, 1_000);
        let heard = Percept {
            source: Source::Heard,
            ..seen(4, 1_000)
        };
        memory.update(&[heard], 1_000, |_| 0, 3_000, 20_000);
        assert_eq!(memory.live_sights().collect::<Vec<_>>(), vec![(3, 1_000)]);
        memory.fail_sight(3, Miss::Dead, 1_010);
        assert_eq!(memory.live_sights().count(), 0);
    }
}
