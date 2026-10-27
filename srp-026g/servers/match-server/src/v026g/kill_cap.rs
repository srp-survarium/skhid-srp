//! A player's kills of one enemy pay at most [`MOST_PAID`] times in one
//! match (the owner's rule against farming a willing victim, 2026-10-27,
//! docs/notes/kill_cap.md). The server sees each player's running totals
//! only, not who killed whom: so a kill is paired with a death when a
//! trusted snapshot shows them alone, one player's kills gone up and as
//! many deaths of his enemies, no teammate of his dead. Kills that came
//! together with another player's are not paired, and pay.

use std::collections::HashMap;

use survarium::v026g::match_msgs::snapshot::{Statistics, event};

use super::battery_cap::cut;

/// Kills of one enemy past this many pay nothing.
pub(crate) const MOST_PAID: u32 = 10;

/// The kill and the kinds of kill that score besides it: cut together, in
/// the same proportion.
const KILL_KINDS: [usize; 5] = [
    event::KILL,
    event::HEADSHOT_KILL,
    event::MELEE_KILL,
    event::SILENT_KILL,
    event::GRENADE_KILL,
];

/// By (killer's place, victim's place): the kills paired so far.
#[derive(Debug, Default)]
pub(crate) struct Pairs(HashMap<(usize, usize), u32>);

impl Pairs {
    /// Pair what changed from `before` (none: the match's start) to
    /// `after`, the places on `teams`.
    pub(crate) fn observe(
        &mut self,
        before: Option<&Statistics>,
        after: &Statistics,
        teams: &[u8],
    ) {
        let gained = |event: usize| -> Vec<(usize, u32)> {
            after
                .players
                .iter()
                .enumerate()
                .map(|(place, player)| {
                    let was = before
                        .and_then(|before| before.players.get(place))
                        .map_or(0, |player| player.total(event));
                    (place, player.total(event).saturating_sub(was))
                })
                .filter(|&(_, gained)| gained > 0)
                .collect()
        };
        let [(killer, kills)] = gained(event::KILL)[..] else {
            return;
        };
        let deaths = gained(event::DEATH);
        let team = teams.get(killer);
        if deaths.iter().any(|&(victim, _)| teams.get(victim) == team)
            || deaths.iter().map(|&(_, died)| died).sum::<u32>() != kills
        {
            return;
        }
        for (victim, died) in deaths {
            *self.0.entry((killer, victim)).or_default() += died;
        }
    }

    /// How many of `killer`'s kills were of an enemy he had already killed
    /// [`MOST_PAID`] times.
    fn over(&self, killer: usize) -> u32 {
        self.0
            .iter()
            .filter(|((by, _), _)| *by == killer)
            .map(|(_, &kills)| kills.saturating_sub(MOST_PAID))
            .sum()
    }
}

/// A player's kills cut: the place, its kills and the kills it is paid.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Cut {
    pub place: usize,
    pub counted: u32,
    pub paid: u32,
}

/// Cut each player's kills by those over [`MOST_PAID`] of one enemy.
pub(crate) fn cap(statistics: &mut Statistics, pairs: &Pairs) -> Vec<Cut> {
    let mut cuts = Vec::new();
    for (place, player) in statistics.players.iter_mut().enumerate() {
        let counted = player.total(event::KILL);
        let paid = counted.saturating_sub(pairs.over(place));
        if paid == counted {
            continue;
        }
        for kind in KILL_KINDS {
            let had = player.total(kind);
            if had > 0 {
                let keep = u64::from(had) * u64::from(paid) / u64::from(counted);
                cut(&mut player.events[kind], keep as u32);
            }
        }
        cuts.push(Cut {
            place,
            counted,
            paid,
        });
    }
    cuts
}

#[cfg(test)]
mod tests;
