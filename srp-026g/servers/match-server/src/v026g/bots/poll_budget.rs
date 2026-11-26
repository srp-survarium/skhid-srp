//! [A2] The per-poll budget's counts (`Bots::commanders_this_poll`): the commander rounds that waited a poll and those
//! kept, the bots' new searches held a poll, and the full-rate rule that kept each bot's work, every match on this server
//! since the last match end (the match-end line).

use std::sync::atomic::{AtomicU64, Ordering};

/// The first rule that keeps a bot's work at the full rate (`Bots::full_rate`, in its order).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FullRate {
    Hit,
    Fight,
    MatesEngaged,
    Known,
    PlayerNear,
    FightNear,
    ThreatNear,
}

impl FullRate {
    const ALL: [FullRate; 7] = [
        FullRate::Hit,
        FullRate::Fight,
        FullRate::MatesEngaged,
        FullRate::Known,
        FullRate::PlayerNear,
        FullRate::FightNear,
        FullRate::ThreatNear,
    ];

    fn word(self) -> &'static str {
        match self {
            FullRate::Hit => "hit lately",
            FullRate::Fight => "in a fight",
            FullRate::MatesEngaged => "mates engaged",
            FullRate::Known => "an enemy known",
            FullRate::PlayerNear => "a player in sight range",
            FullRate::FightNear => "a fight in sight range",
            FullRate::ThreatNear => "a threat in sight range",
        }
    }
}

/// A commander round moved to the next poll (a second round due in one poll, its team under no rule).
pub(crate) const ROUND_MOVED: usize = 0;
/// A bot's new search held to the next poll.
pub(crate) const SEARCH_HELD: usize = 1;
/// Bots looked at in polls with a round: under no rule.
const CALM: usize = 2;
/// Then per rule: the bots it kept ([`count_bot`]), and the second rounds it kept ([`count_round_kept`]).
const BOTS_KEPT: usize = 3;
const ROUNDS_KEPT: usize = BOTS_KEPT + FullRate::ALL.len();

static COUNTS: [AtomicU64; ROUNDS_KEPT + FullRate::ALL.len()] = [const { AtomicU64::new(0) }; ROUNDS_KEPT + FullRate::ALL.len()];

pub(crate) fn count(at: usize) {
    COUNTS[at].fetch_add(1, Ordering::Relaxed);
}

/// A bot looked at in a poll with a round: the rule it is under, or none (calm).
pub(crate) fn count_bot(why: Option<FullRate>) {
    count(why.map_or(CALM, |why| BOTS_KEPT + why as usize));
}

/// A second round in one poll kept by a bot of its team under `why`.
pub(crate) fn count_round_kept(why: FullRate) {
    count(ROUNDS_KEPT + why as usize);
}

/// The line's words, and the counts zeroed.
pub(crate) fn take_line() -> String {
    let c: Vec<u64> = COUNTS.iter().map(|c| c.swap(0, Ordering::Relaxed)).collect();
    let by = |from: usize| {
        FullRate::ALL.iter().map(|&why| format!("{} {}", why.word(), c[from + why as usize])).collect::<Vec<_>>().join(", ")
    };
    format!(
        "per-poll budget since the last match end (every match on this server): commander rounds moved a poll {}, new searches held a poll {}; bots in polls with a round: calm {}, at full rate by {}; second rounds kept by {}",
        c[ROUND_MOVED],
        c[SEARCH_HELD],
        c[CALM],
        by(BOTS_KEPT),
        by(ROUNDS_KEPT)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The line names every rule twice (bots, rounds), in the rules' order. (Counts are shared by every match on the
    /// thread's server, other tests' too: only the words are checked.)
    #[test]
    fn the_line_names_each_choice() {
        count_bot(Some(FullRate::PlayerNear));
        let line = take_line();
        for why in FullRate::ALL {
            assert_eq!(line.matches(why.word()).count(), 2, "{line}");
        }
        assert!(line.contains("commander rounds moved a poll ") && line.contains("new searches held a poll "), "{line}");
    }
}
