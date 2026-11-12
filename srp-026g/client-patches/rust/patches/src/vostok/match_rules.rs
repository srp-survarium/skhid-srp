//! Scoped rule reads for FFA scoring and its exact match deadline.
use core::num::NonZeroU32;
use super::{Context, Thread};
use super::world::read;
use crate::raw;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchClock {
    Pregame,
    Playing { remaining_ms: NonZeroU32 },
    Finished,
}

impl MatchClock {
    fn at(state: u32, start: u32, duration: u32, now: u32) -> Option<Self> {
        match state {
            0..=2 => Some(Self::Pregame),
            3 if start != u32::MAX => {
                let left = duration.saturating_sub(now.wrapping_sub(start));
                Some(NonZeroU32::new(left).map_or(Self::Finished, |remaining_ms| Self::Playing { remaining_ms }))
            }
            _ => None,
        }
    }
}

/// The time rule in the core's owned vector, as rule_completed 0x318360
/// reads it: state +148h, start +12Ch, match duration +138h.
///
/// # Safety
/// `core` and its owned rules are alive and stable for the current hook.
pub(super) unsafe fn clock_of(core: raw::CoreWorld, time: u32) -> Option<MatchClock> {
    // SAFETY: the caller owns a synchronous native boundary; the vector's
    // concrete base type identifies the timelimit layout before reading it.
    unsafe {
        let first = core.rules_begin()?;
        let count = core.rules_end()?.index_from(first)?;
        if count > 64 { return None; }
        for index in 0..count {
            let rule = first.at(index).rule()?;
            if read(rule.rule_type()) == 2 {
                return MatchClock::at(read(rule.time_state()), read(rule.time_start()), read(rule.match_ms()), time);
            }
        }
        None
    }
}

/// A death notification at kd_stats_rule::on_event (0x3237E0). The rule
/// owns its serialized records; the growing-roster patch fixes its
/// record count at twenty, including zeroed places not installed yet.
pub struct KillEvent<'t> {
    rule: raw::KdStatsRule,
    time: u32,
    _thread: &'t Thread,
}

impl<'t> KillEvent<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<Self> {
        use raw::site::kd_stats_event as site;
        // SAFETY: this site is the handler's entry, ECX its rule and the
        // untouched stack its three arguments. Only event 1 changes KD.
        unsafe {
            let frame = site::frame(ctx)?;
            if read(frame.event()) != 1 { return None; }
            Some(Self { rule: site::rule(ctx)?, time: read(frame.time()), _thread: ctx.thread() })
        }
    }

    pub fn standing(&self) -> Option<(MatchClock, u16)> {
        // SAFETY: at() pins the live rule for this synchronous hook. Its
        // core and inline records are owned by the rule and match.
        unsafe {
            let places = usize::from(read(self.rule.places()));
            if places == 0 || places > super::types::Place::COUNT { return None; }
            let top = (0..places).map(|i| read(self.rule.records().at(i).kills())).max()?;
            Some((clock_of(self.rule.core()?, self.time)?, top))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_fractional_second_is_still_play_and_the_deadline_wraps() {
        for start in [20_000u32, u32::MAX - 400] {
            let end = start.wrapping_add(900_000);
            for left in [999, 50, 1] {
                assert_eq!(MatchClock::at(3, start, 900_000, end.wrapping_sub(left)),
                           Some(MatchClock::Playing { remaining_ms: NonZeroU32::new(left).unwrap() }));
            }
            assert_eq!(MatchClock::at(3, start, 900_000, end), Some(MatchClock::Finished));
            assert_eq!(MatchClock::at(3, start, 900_000, end.wrapping_add(50)), Some(MatchClock::Finished));
        }
        assert_eq!(MatchClock::at(2, 0, 900_000, 950_000), Some(MatchClock::Pregame));
        assert_eq!(MatchClock::at(3, u32::MAX, 900_000, 1), None);
        assert_eq!(MatchClock::at(4, 0, 900_000, 1), None);
    }
}
