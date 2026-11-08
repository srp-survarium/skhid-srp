//! A match's events (`session::v026g::persist::match_events`,
//! `docs/notes/match_history.md`, "Events") from the worlds the server
//! reads: the trusted ones as [`super::game::Game`] observes them (about
//! one every 5 s, `Timing::refresh`).
//!
//! No message names a kill, a battery or a capture (`kill_weapon.md` 1):
//! each event is what changed between two worlds' shared statistics and
//! rules, which every world carries whole (`snapshot_rules.md` 3). So:
//!
//! - an event's time is the window between those worlds
//!   ([`Timing::Between`]), save a kill whose killer made no other kill in
//!   the window: the killer's `last_killed_enemy_ms` is its tick
//!   ([`Timing::Exact`]);
//! - a kill's victim is a death of the window on the other team, when only
//!   one killer of the window could have made it, or only one of them hurt
//!   the victim in the life it lost (its `received_damage`); else the kill
//!   has no victim and the death is a death of its own;
//! - suicides, falls, anomalies and teammates' shots are deaths with no
//!   kill: the worlds do not say which;
//! - no weapon (the game never sends it) and no position (a dead player's
//!   block has no body, and the worlds are seconds from the event).

use session::v026g::persist::match_events::{EventKind, MatchEvent, Player, Timing};
use survarium::v026g::maps::RuleMode;
use survarium::v026g::match_msgs::rules::DRAW;
use survarium::v026g::match_msgs::snapshot::{
    PlayerStatistics, Research, Standing, Statistics, event,
};

/// Who is in a roster place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seat {
    /// 0: the server's own bot.
    pub account_id: u32,
    /// 0 or 1.
    pub team: u8,
}

/// What a world says that events are read from.
pub struct World<'a> {
    pub time: u32,
    pub statistics: &'a Statistics,
    pub standing: Option<Standing>,
    /// Mode 5's rule.
    pub research: Option<&'a Research>,
}

/// A place's counts the events are the changes of.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Tally {
    kills: u32,
    deaths: u32,
    found: u32,
    delivered: u32,
}

impl Tally {
    fn of(player: &PlayerStatistics) -> Self {
        Self {
            kills: player.total(event::KILL),
            deaths: player.total(event::DEATH),
            found: player.total(event::VICTORY_ITEM_FOUND),
            delivered: player.total(event::PUT_VICTORY_ITEM_IN_CONTAINER),
        }
    }

    /// What it gained since `before`; `None` when a count went down (not
    /// a later state of the same play).
    fn since(self, before: Self) -> Option<Self> {
        Some(Self {
            kills: self.kills.checked_sub(before.kills)?,
            deaths: self.deaths.checked_sub(before.deaths)?,
            found: self.found.checked_sub(before.found)?,
            delivered: self.delivered.checked_sub(before.delivered)?,
        })
    }
}

/// The player in roster place `place`, if there is one.
fn player(seats: &[Seat], place: usize) -> Option<Player> {
    Some(Player {
        place: u8::try_from(place).ok()?,
        account_id: seats.get(place)?.account_id,
    })
}

/// The last world read.
struct Seen {
    time: u32,
    tallies: Vec<Tally>,
    points: Option<[u16; 2]>,
    /// Research's units' owners.
    owners: Vec<u8>,
}

/// The events of one match, world by world.
pub struct Extractor {
    mode: RuleMode,
    seen: Option<Seen>,
    events: Vec<MatchEvent>,
}

impl Extractor {
    pub fn new(mode: RuleMode) -> Self {
        Self {
            mode,
            seen: None,
            events: Vec::new(),
        }
    }

    /// Reads the events between the last world and `world`, whose roster
    /// places are `seats`. A world no later than the last, or whose counts
    /// went down (not a later state of the same play), is passed over:
    /// whether it was read.
    pub fn observe(&mut self, world: &World, seats: &[Seat]) -> bool {
        let tallies: Vec<Tally> = world.statistics.players.iter().map(Tally::of).collect();
        let owners: Vec<u8> = world
            .research
            .map(|research| research.units.iter().map(|unit| unit.owner).collect())
            .unwrap_or_default();
        let points = world.standing.map(|standing| standing.points);
        let (after, old_points, old_owners) = match &self.seen {
            Some(seen) => (seen.time, seen.points, seen.owners.clone()),
            None => (0, Some([0, 0]), Vec::new()),
        };
        if world.time <= after {
            return false;
        }
        let before = |place: usize| {
            self.seen
                .as_ref()
                .and_then(|seen| seen.tallies.get(place).copied())
                .unwrap_or_default()
        };
        let Some(gains) = tallies
            .iter()
            .enumerate()
            .map(|(place, now)| now.since(before(place)))
            .collect::<Option<Vec<Tally>>>()
        else {
            return false;
        };
        let window = Timing::Between {
            after,
            by: world.time,
        };
        self.kills(world, seats, after, &gains);
        for (place, gain) in gains.iter().enumerate() {
            let Some(by) = player(seats, place) else {
                continue;
            };
            for _ in 0..gain.found {
                self.push(window, EventKind::BatteryFound { by });
            }
            for _ in 0..gain.delivered {
                self.push(window, EventKind::BatteryDelivered { by });
            }
        }
        for (unit, &owner) in owners.iter().enumerate() {
            let was = old_owners.get(unit).copied().unwrap_or(DRAW);
            if owner != was && owner < 2 {
                self.push(
                    window,
                    EventKind::Capture {
                        team: owner,
                        unit: unit as u8,
                    },
                );
            }
        }
        if self.mode.has_batteries()
            && let (Some(points), Some(was)) = (points, old_points)
            && points != was
        {
            self.push(window, EventKind::Score { points });
        }
        self.seen = Some(Seen {
            time: world.time,
            tallies,
            points: points.or(old_points),
            owners: if owners.is_empty() {
                old_owners
            } else {
                owners
            },
        });
        true
    }

    /// The kills and deaths since the world at `after`, `gains` by place.
    fn kills(&mut self, world: &World, seats: &[Seat], after: u32, gains: &[Tally]) {
        let players = &world.statistics.players;
        let team = |place: usize| seats.get(place).map(|seat| seat.team).filter(|&t| t < 2);
        let enemies =
            |a: usize, b: usize| matches!((team(a), team(b)), (Some(x), Some(y)) if x != y);
        // When each killer's kills of the window were.
        let timing = |place: usize| {
            let last = players[place].last_killed_enemy_ms;
            match (last > after && last <= world.time, gains[place].kills) {
                (true, 1) => Timing::Exact(last),
                (true, _) => Timing::Between { after, by: last },
                (false, _) => Timing::Between {
                    after,
                    by: world.time,
                },
            }
        };
        // Each killer of the window, with its kills not given a victim yet.
        let mut killers: Vec<(usize, Player, u32)> = gains
            .iter()
            .enumerate()
            .filter(|(_, gain)| gain.kills > 0)
            .filter_map(|(place, gain)| Some((place, player(seats, place)?, gain.kills)))
            .collect();
        let deaths: Vec<usize> = gains
            .iter()
            .enumerate()
            .flat_map(|(place, gain)| std::iter::repeat_n(place, gain.deaths as usize))
            .collect();
        for &victim in &deaths {
            let Some(dead) = player(seats, victim) else {
                continue;
            };
            let candidates: Vec<usize> = killers
                .iter()
                .filter(|&&(killer, _, left)| left > 0 && enemies(killer, victim))
                .map(|&(killer, _, _)| killer)
                .collect();
            let only = match candidates[..] {
                // One killer, and no more of its enemies died than it killed.
                [killer] => {
                    let its_victims = deaths.iter().filter(|&&dead| enemies(killer, dead)).count();
                    (its_victims as u32 <= gains[killer].kills).then_some(killer)
                }
                _ => None,
            };
            // Else the one of them who hurt the victim in the life it lost.
            let hurt: Vec<usize> = candidates
                .iter()
                .copied()
                .filter(|&killer| {
                    players[victim]
                        .received_damage
                        .iter()
                        .any(|kit| kit.get(killer).is_some_and(|&damage| damage > 0.0))
                })
                .collect();
            let paired = only.or(match hurt[..] {
                [killer] => Some(killer),
                _ => None,
            });
            let Some(entry) = paired.and_then(|place| killers.iter_mut().find(|k| k.0 == place))
            else {
                self.push(
                    Timing::Between {
                        after,
                        by: world.time,
                    },
                    EventKind::Death { victim: dead },
                );
                continue;
            };
            entry.2 -= 1;
            let (place, killer) = (entry.0, entry.1);
            self.push(
                timing(place),
                EventKind::Kill {
                    killer,
                    victim: Some(dead),
                },
            );
        }
        for (place, killer, left) in killers {
            for _ in 0..left {
                self.push(
                    timing(place),
                    EventKind::Kill {
                        killer,
                        victim: None,
                    },
                );
            }
        }
    }

    fn push(&mut self, timing: Timing, kind: EventKind) {
        self.events.push(MatchEvent { timing, kind });
    }

    /// The events so far and the end, at `end_ms` with `points`.
    pub fn finish(&self, end_ms: u32, points: [u16; 2]) -> Vec<MatchEvent> {
        let mut events = self.events.clone();
        events.push(MatchEvent {
            timing: Timing::Exact(end_ms),
            kind: EventKind::End { points },
        });
        events
    }
}

#[cfg(test)]
mod tests {
    use survarium::v026g::match_msgs::snapshot::{ResearchUnit, StatisticsTail};

    use super::*;

    /// Four places: 0 and 1 on team 0, 2 and 3 on team 1.
    const SEATS: [Seat; 4] = [
        Seat {
            account_id: 10,
            team: 0,
        },
        Seat {
            account_id: 11,
            team: 0,
        },
        Seat {
            account_id: 12,
            team: 1,
        },
        Seat {
            account_id: 0,
            team: 1,
        },
    ];

    fn who(place: u8) -> Player {
        Player {
            place,
            account_id: SEATS[usize::from(place)].account_id,
        }
    }

    /// Statistics of the four places with these events, by place.
    fn statistics(events: &[(usize, usize, u16)]) -> Statistics {
        let mut players = vec![PlayerStatistics::default(); SEATS.len()];
        for &(place, event, count) in events {
            players[place].events[event][0] = count;
        }
        Statistics {
            players,
            victory_items: Vec::new(),
            tail: StatisticsTail::default(),
        }
    }

    fn observe(extractor: &mut Extractor, time: u32, statistics: &Statistics) -> bool {
        extractor.observe(
            &World {
                time,
                statistics,
                standing: None,
                research: None,
            },
            &SEATS,
        )
    }

    #[test]
    fn kills_are_paired_by_the_one_killer_or_the_one_who_hurt() {
        let mut extractor = Extractor::new(RuleMode::TeamDeathmatch);
        // 0 kills 2 alone: paired, at its tick.
        let mut first = statistics(&[(0, event::KILL, 1), (2, event::DEATH, 1)]);
        first.players[0].last_killed_enemy_ms = 3_250;
        assert!(observe(&mut extractor, 5_000, &first));
        // Then 0 and 1 kill once each, 2 and 3 die, each hurt by 1 only:
        // 2 is 1's; 3 is left open (0 did not hurt it), so is 0's kill.
        let mut second = statistics(&[
            (0, event::KILL, 2),
            (1, event::KILL, 1),
            (2, event::DEATH, 2),
            (3, event::DEATH, 1),
        ]);
        second.players[0].last_killed_enemy_ms = 7_000;
        second.players[1].last_killed_enemy_ms = 8_100;
        second.players[2].received_damage[0][1] = 40.0;
        second.players[3].received_damage[1][1] = 20.0;
        assert!(observe(&mut extractor, 10_000, &second));
        // A world whose counts went down is not a later one.
        assert!(!observe(&mut extractor, 15_000, &first));
        // Nor one of the same time.
        assert!(!observe(&mut extractor, 10_000, &second));
        let window = Timing::Between {
            after: 5_000,
            by: 10_000,
        };
        assert_eq!(
            extractor.finish(20_000, [90, 98]),
            [
                MatchEvent {
                    timing: Timing::Exact(3_250),
                    kind: EventKind::Kill {
                        killer: who(0),
                        victim: Some(who(2)),
                    },
                },
                MatchEvent {
                    timing: Timing::Exact(8_100),
                    kind: EventKind::Kill {
                        killer: who(1),
                        victim: Some(who(2)),
                    },
                },
                MatchEvent {
                    timing: window,
                    kind: EventKind::Death { victim: who(3) },
                },
                MatchEvent {
                    timing: Timing::Exact(7_000),
                    kind: EventKind::Kill {
                        killer: who(0),
                        victim: None,
                    },
                },
                MatchEvent {
                    timing: Timing::Exact(20_000),
                    kind: EventKind::End { points: [90, 98] },
                },
            ]
        );
    }

    #[test]
    fn a_killer_of_two_in_a_window_has_the_window_up_to_its_last_kill() {
        let mut extractor = Extractor::new(RuleMode::TeamDeathmatch);
        let mut world = statistics(&[
            (2, event::KILL, 2),
            (0, event::DEATH, 1),
            (1, event::DEATH, 1),
            // A suicide: no enemy's kill.
            (3, event::DEATH, 1),
        ]);
        world.players[2].last_killed_enemy_ms = 4_000;
        observe(&mut extractor, 5_000, &world);
        let events = extractor.finish(6_000, [0, 0]);
        let killed = |victim| MatchEvent {
            timing: Timing::Between {
                after: 0,
                by: 4_000,
            },
            kind: EventKind::Kill {
                killer: who(2),
                victim: Some(who(victim)),
            },
        };
        assert_eq!(events[..2], [killed(0), killed(1)]);
        assert_eq!(
            events[2],
            MatchEvent {
                timing: Timing::Between {
                    after: 0,
                    by: 5_000
                },
                kind: EventKind::Death { victim: who(3) },
            }
        );
    }

    #[test]
    fn batteries_score_and_captures_are_read_from_the_window() {
        let mut batteries = Extractor::new(RuleMode::Batteries);
        let world = statistics(&[
            (1, event::VICTORY_ITEM_FOUND, 1),
            (1, event::PUT_VICTORY_ITEM_IN_CONTAINER, 1),
        ]);
        batteries.observe(
            &World {
                time: 5_000,
                statistics: &world,
                standing: Some(Standing {
                    points: [1, 0],
                    winner: 0,
                }),
                research: None,
            },
            &SEATS,
        );
        let window = Timing::Between {
            after: 0,
            by: 5_000,
        };
        let kinds: Vec<(Timing, EventKind)> = batteries
            .finish(9_000, [1, 0])
            .iter()
            .map(|event| (event.timing, event.kind))
            .collect();
        assert_eq!(
            kinds[..3],
            [
                (window, EventKind::BatteryFound { by: who(1) }),
                (window, EventKind::BatteryDelivered { by: who(1) }),
                (window, EventKind::Score { points: [1, 0] }),
            ]
        );

        let mut research = Extractor::new(RuleMode::Research);
        let units = |owners: [u8; 3]| Research {
            points: [0, 0],
            scores: [0, 0],
            end: None,
            entered: 0,
            units: owners
                .map(|owner| ResearchUnit {
                    owner,
                    ..ResearchUnit::default()
                })
                .to_vec(),
        };
        let quiet = statistics(&[]);
        for (time, owners) in [(5_000, [DRAW, DRAW, DRAW]), (10_000, [DRAW, 1, DRAW])] {
            let rule = units(owners);
            research.observe(
                &World {
                    time,
                    statistics: &quiet,
                    standing: None,
                    research: Some(&rule),
                },
                &SEATS,
            );
        }
        assert_eq!(
            research.finish(12_000, [0, 1])[0],
            MatchEvent {
                timing: Timing::Between {
                    after: 5_000,
                    by: 10_000
                },
                kind: EventKind::Capture { team: 1, unit: 1 },
            }
        );
    }
}
