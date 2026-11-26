//! A round's restart: how a round ends and the next one starts in every mode
//! played in rounds, Artifact Hunt (0.46c2's states 6 to 8) and Freeze Tag
//! (srp-validate's `world::freeze`), by this one machine (the owner,
//! 2026-11-23: "I expect the round to end exactly in the same manner in
//! both"):
//!
//! 1. [`Restart::Decided`], `time_before_new_round`: the round is decided,
//!    play goes on, nobody respawns;
//! 2. the mode says whether the match ends there;
//! 3. [`Restart::Fade`], [`FADE`]: the screen fades; at its end every
//!    player, alive or dead, respawns at his team's spawns;
//! 4. [`Restart::Countdown`], `round_countdown_time`: everyone at the
//!    spawns, his input held (the match start's countdown: the warmup
//!    camera, patch 31's kits, each switch a life);
//! 5. the next round.

use crate::roster::PLAYERS;
use crate::time::{Ms, Time};

/// 0.46c2's transition to a new round: the screen fades for a constant
/// second (0x740000), in no config.
pub const FADE: Ms = seconds(1);

const fn seconds(seconds: u32) -> Ms {
    match Ms::from_seconds(seconds) {
        Some(ms) => ms,
        None => Ms::ZERO,
    }
}

/// How long a restart's parts last: the mode's `time_before_new_round` and
/// `round_countdown_time` (the fade is [`FADE`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timing {
    decided: Ms,
    countdown: Ms,
}

impl Timing {
    /// 0.46c2's own, 3 s and 5 s, which every Artifact Hunt map configures:
    /// Freeze Tag's.
    pub const STOCK: Timing = Timing::new(seconds(3), seconds(5));

    pub const fn new(decided: Ms, countdown: Ms) -> Timing {
        Timing { decided, countdown }
    }

    pub const fn decided(self) -> Ms {
        self.decided
    }

    pub const fn countdown(self) -> Ms {
        self.countdown
    }

    /// The whole restart, from the round's end to the next round.
    pub const fn whole(self) -> Ms {
        self.decided.plus(FADE).plus(self.countdown)
    }
}

/// A round's restart, from the step its round was decided to the next
/// round's first. `F` is what the fade carries to the next round, `C` what
/// the countdown does (Artifact Hunt: sudden death's start, then its clock;
/// Freeze Tag: nothing).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Restart<F, C> {
    /// Until `until`: play goes on, nobody respawns.
    Decided { until: Time },
    /// Until `until`: the screen fades; at its end everyone respawns.
    Fade { until: Time, next: F },
    /// Until `until`: everyone stands at his team's spawns, his input held.
    Countdown { until: Time, next: C },
}

/// What a step makes of a restart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step<F, C> {
    /// Nothing ends at this step.
    Stays(Restart<F, C>),
    /// The time after the round ended (its deadline `until`): the mode ends
    /// the match, or begins the fade ([`Restart::fade`]).
    Decided { until: Time },
    /// Everyone respawned at this step ([`Spawning::Everyone`]): a round
    /// start, counted, and the countdown begun.
    Started(Restart<F, C>),
    /// The countdown ended: this step is the next round's first.
    Next(C),
}

/// What the respawn rule does with the places due at a step: asked at its
/// tick, before the mode's rule steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spawning {
    /// As the respawn rule has them.
    Native,
    /// Nobody spawns: a place due now is due no more (0.46c2's respawns
    /// off, rule+0x220, in states 6 and 7).
    Held,
    /// The round starts: every place of the match spawns now, alive or dead
    /// (0x785C40).
    Everyone,
}

impl<F: Copy, C: Copy> Restart<F, C> {
    /// The round was decided at `now`.
    pub const fn decided(now: Time, timing: Timing) -> Restart<F, C> {
        Restart::Decided {
            until: now.after(timing.decided),
        }
    }

    /// The fade from `now`, towards a round that starts with `next`.
    pub const fn fade(now: Time, next: F) -> Restart<F, C> {
        Restart::Fade {
            until: now.after(FADE),
            next,
        }
    }

    pub const fn until(&self) -> Time {
        match *self {
            Restart::Decided { until }
            | Restart::Fade { until, .. }
            | Restart::Countdown { until, .. } => until,
        }
    }

    /// The part's whole length.
    pub const fn length(&self, timing: Timing) -> Ms {
        match self {
            Restart::Decided { .. } => timing.decided,
            Restart::Fade { .. } => FADE,
            Restart::Countdown { .. } => timing.countdown,
        }
    }

    /// One step at `now`: at most one transition, each part timed from the
    /// step that entered it. A round start is counted in `lives`; `start`
    /// is what the countdown carries of what the fade did.
    pub fn step(
        self,
        now: Time,
        timing: Timing,
        lives: &mut Lives,
        start: impl FnOnce(F) -> C,
    ) -> Step<F, C> {
        match self {
            Restart::Decided { until } if now.reached(until) => Step::Decided { until },
            Restart::Fade { until, next } if now.reached(until) => {
                lives.round_starts = lives.round_starts.saturating_add(1);
                Step::Started(Restart::Countdown {
                    until: now.after(timing.countdown),
                    next: start(next),
                })
            }
            Restart::Countdown { until, next } if now.reached(until) => Step::Next(next),
            other => Step::Stays(other),
        }
    }

    /// What the respawn rule does at `now`, a step whose rule step is still
    /// to come: the same test the step makes ends the fade.
    pub fn spawning(&self, now: Time) -> Spawning {
        match self {
            Restart::Fade { until, .. } if now.reached(*until) => Spawning::Everyone,
            Restart::Decided { .. } | Restart::Fade { .. } => Spawning::Held,
            Restart::Countdown { .. } => Spawning::Native,
        }
    }

    /// Whether the game plays the match start's countdown: the fade and the
    /// countdown (the core's time state 4: the warmup camera, whose input
    /// holds, and patch 31's kits).
    pub fn round_countdown(&self) -> bool {
        matches!(self, Restart::Fade { .. } | Restart::Countdown { .. })
    }

    /// Whether everyone stands at the spawns: the countdown (0.46c2's flag 4
    /// of state 8, 0x7B0BA0).
    pub fn frozen(&self) -> bool {
        matches!(self, Restart::Countdown { .. })
    }
}

/// The lives a mode's rounds give besides its deaths, by which srp charges a
/// match's supplies: every round start (the fade's end respawns everyone)
/// and every kit switch in a round's countdown (patch 31's, where the player
/// stands).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lives {
    round_starts: u16,
    /// By place.
    kit_switches: [u16; PLAYERS],
}

impl Lives {
    pub const NONE: Lives = Lives::new(0, [0; PLAYERS]);

    pub const fn new(round_starts: u16, kit_switches: [u16; PLAYERS]) -> Lives {
        Lives {
            round_starts,
            kit_switches,
        }
    }

    /// The rounds started after the match's first: each a life of every
    /// player in the match then.
    pub const fn round_starts(&self) -> u16 {
        self.round_starts
    }

    /// The kit switches of the place at `index`.
    pub fn kit_switches(&self, index: usize) -> u16 {
        self.kit_switches.get(index).copied().unwrap_or(0)
    }

    pub const fn every_kit_switch(&self) -> [u16; PLAYERS] {
        self.kit_switches
    }

    /// The place at `index` was switched into its chosen kit; a life when
    /// it was in a round's countdown.
    pub fn kit_switched<F: Copy, C: Copy>(&mut self, restart: &Restart<F, C>, index: usize) {
        if restart.round_countdown()
            && let Some(count) = self.kit_switches.get_mut(index)
        {
            *count = count.saturating_add(1);
        }
    }
}
