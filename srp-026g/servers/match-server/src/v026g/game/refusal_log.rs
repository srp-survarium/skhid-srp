//! Every refusal of a player's bytes reaches the log: at most one line a
//! second for each player and kind, and a line counts the refusals since the
//! one before it, so a flood cannot fill the disk and none goes unseen.
use std::time::{Duration, Instant};

const LINE_EVERY: Duration = Duration::from_secs(1);

#[derive(Default)]
pub(super) enum RefusalLog {
    #[default]
    Quiet,
    /// A line was written at `at`; `unlogged` refusals came after it.
    Logged { at: Instant, unlogged: u32 },
}

impl RefusalLog {
    /// A refusal at `now`: `Some(n)` when it is to be logged, `n` being the
    /// refusals since the last line that were not.
    pub(super) fn refuse(&mut self, now: Instant) -> Option<u32> {
        let (logged, next) = match *self {
            Self::Logged { at, unlogged } if now.saturating_duration_since(at) < LINE_EVERY => {
                (None, Self::Logged { at, unlogged: unlogged.saturating_add(1) })
            }
            Self::Logged { unlogged, .. } => (Some(unlogged), Self::Logged { at: now, unlogged: 0 }),
            Self::Quiet => (Some(0), Self::Logged { at: now, unlogged: 0 }),
        };
        *self = next;
        logged
    }
}

/// `""`, or `"; n more refused since the last line"`.
pub(super) fn since(unlogged: u32) -> String {
    if unlogged == 0 { String::new() } else { format!("; {unlogged} more refused since the last line") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_a_second_counts_the_refusals_between() {
        let now = Instant::now();
        let mut log = RefusalLog::default();
        assert_eq!(log.refuse(now), Some(0));
        for n in 1..=5 {
            assert_eq!(log.refuse(now + Duration::from_millis(100 * n)), None);
        }
        assert_eq!(log.refuse(now + LINE_EVERY), Some(5));
        assert_eq!(log.refuse(now + LINE_EVERY * 3), Some(0));
    }
}
