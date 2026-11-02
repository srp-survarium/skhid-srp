//! A match's time rule as it is made: survarium::timelimit_rule_core, its
//! constructor (0x317DA0) at the site `timelimit.made` (0x317EA7: the mode's
//! times stored; ESI the rule, EBP the constructor's frame, whose argument
//! is the rule cook's data, the match's options at its +0). A match's world
//! and a replay's make it once, from the options their static info gave.

use super::context::Context;
use super::hook::Thread;
use super::options::{self, Countdown};
use crate::raw;

/// The time rule being made, and the options it is made from.
pub struct MadeRule<'t> {
    rule: raw::TimelimitRuleCore,
    options: raw::MatchOptions,
    _thread: &'t Thread,
}

impl<'t> MadeRule<'t> {
    /// At `timelimit.made`.
    pub fn at_made(ctx: &'t Context<'_>) -> Option<MadeRule<'t>> {
        use raw::site::timelimit_made as site;
        // SAFETY: the context is this site's: the rule being constructed and
        // the constructor's frame, its argument the cook's data the
        // constructor read N through (0x317E0A), alive while it runs on the
        // game's thread.
        let (rule, options) = unsafe { (site::rule(ctx)?, site::frame(ctx)?.cook()?.options()?) };
        Some(MadeRule { rule, options, _thread: ctx.thread() })
    }

    /// The countdown the static info gave (patch 95's, kept in the options),
    /// if any.
    pub fn options_countdown(&self) -> Option<Countdown> {
        // SAFETY: the options are alive for the hook.
        unsafe { options::countdown(self.options) }
    }

    /// The rule counts `countdown` down before play, in place of the mode's.
    pub fn set_countdown(&mut self, countdown: Countdown) {
        // SAFETY: the rule is alive for the hook; nothing has read its
        // countdown yet (the constructor stored it, logic_tick reads it).
        unsafe { *self.rule.countdown() = countdown.ms() }
    }
}
