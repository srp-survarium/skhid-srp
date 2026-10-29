//! Conservative payment limits for repeated kills of a passive opponent.
//! Only confidently paired kills count. An opponent's combat activity raises
//! the allowance, so small, contested matches are not treated as feeding.

use std::collections::HashMap;

use survarium::v026g::match_msgs::snapshot::{Statistics, event};

use super::battery_cap::cut;

/// Base allowance against an opponent with no enemy kills.
pub(crate) const MOST_PAID: u32 = 10;
const PER_VICTIM_KILL: u32 = 2;

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
        let Some(&team @ (0 | 1)) = teams.get(killer) else {
            return;
        };
        if deaths
            .iter()
            .any(|&(victim, _)| teams.get(victim) != Some(&(1 - team)))
            || deaths.iter().map(|&(_, died)| died).sum::<u32>() != kills
        {
            return;
        }
        for (victim, died) in deaths {
            *self.0.entry((killer, victim)).or_default() += died;
        }
    }

    /// Only kills above the allowance of an opponent who is not fighting
    /// back. Use the final, unmodified totals for every victim: roster order
    /// and reductions to another player's pay must not change this decision.
    fn over(&self, killer: usize, victim_kills: &[u32]) -> u32 {
        self.0
            .iter()
            .filter(|((by, _), _)| *by == killer)
            .filter_map(|((_, victim), &kills)| {
                let fought = victim_kills.get(*victim)?;
                Some(kills.saturating_sub(MOST_PAID + PER_VICTIM_KILL * fought))
            })
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

/// Pay ten kills of each opponent, plus two per enemy kill that opponent made.
pub(crate) fn cap(statistics: &mut Statistics, pairs: &Pairs) -> Vec<Cut> {
    let victim_kills: Vec<_> = statistics
        .players
        .iter()
        .map(|p| p.total(event::KILL))
        .collect();
    let mut cuts = Vec::new();
    for (place, player) in statistics.players.iter_mut().enumerate() {
        let counted = player.total(event::KILL);
        let paid = counted.saturating_sub(pairs.over(place, &victim_kills));
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
