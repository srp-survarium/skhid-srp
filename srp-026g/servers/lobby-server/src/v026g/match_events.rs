//! A match's events (`session::v026g::persist::match_events`) as rows of
//! srp's side channel: the names of a `91`'s rows, ASCII fields with `;`
//! between them, at most [`limits::NAME`] bytes each and 255 rows to a
//! `91`, as the rooms' and the replay poll's (`survarium::v026g::lobby::
//! rooms`). For the replay's timeline and the History's rows; the request
//! that asks for them is theirs to add.
//!
//! A page is [`match_events_page`]: its first row is the head,
//! `E1;<status>;<match id 8 hex>;<events>;<page>`, `status` `ok`, `none`
//! (no such match kept, or no history) or `error` (the history could not be
//! read), `events` how many the match has in all. Then the page's events,
//! [`PAGE_ROWS`] a page, oldest first, one a row:
//!
//! | row | event |
//! |---|---|
//! | `K;<at>;<after>;<killer place>;<killer account>;<victim place>;<victim account>` | a kill; the victim's fields empty when not known |
//! | `D;<at>;<after>;<place>;<account>` | a death no kill was paired with |
//! | `F;<at>;<after>;<place>;<account>` | a battery found (first picked up) |
//! | `B;<at>;<after>;<place>;<account>` | a battery put into a container |
//! | `C;<at>;<after>;<team>;<unit>` | Research: a team took a unit |
//! | `S;<at>;<after>;<team 0 points>;<team 1 points>` | the batteries in the containers changed |
//! | `E;<at>;<after>;<team 0 points>;<team 1 points>` | the end |
//!
//! `at` is match ms (the replay's time): when it was, or with `after` (not
//! empty) the latest it can have been, `after` the earliest (excluded).
//! Places are the roster's, 0 to 19; account 0 is the server's own bot.

use session::v026g::persist::history::MatchHistory;
use session::v026g::persist::match_events::{EventKind, MatchEvent, Player, Timing};
use survarium::v026g::lobby::limits;

/// The rows' version, first in the head.
pub const EVENTS_VERSION: &str = "E1";
/// Events a page: a `91`'s 255 rows less the head.
pub const PAGE_ROWS: usize = u8::MAX as usize - 1;

/// Page `page` of the events of the newest match the match server
/// numbered `match_id`: the head and its rows. Of any kind of match: who
/// may see them is the caller's to say.
pub fn match_events_page(history: Option<&MatchHistory>, match_id: u32, page: u8) -> Vec<String> {
    let found = history.map(|history| history.events(match_id));
    let (status, events) = match found {
        Some(Ok(Some(found))) => ("ok", found.events),
        Some(Ok(None)) | None => ("none", Vec::new()),
        Some(Err(error)) => {
            log::warn!("Match {match_id:#x}'s events not read: {error}");
            ("error", Vec::new())
        }
    };
    let mut rows = vec![format!(
        "{EVENTS_VERSION};{status};{match_id:08x};{};{page}",
        events.len()
    )];
    rows.extend(
        events
            .iter()
            .skip(usize::from(page) * PAGE_ROWS)
            .take(PAGE_ROWS)
            .map(event_row),
    );
    debug_assert!(rows.iter().all(|row| row.len() <= limits::NAME));
    rows
}

/// One event's row.
fn event_row(event: &MatchEvent) -> String {
    let (at, after) = match event.timing {
        Timing::Exact(at) => (at, String::new()),
        Timing::Between { after, by } => (by, after.to_string()),
    };
    let player = |player: Player| format!("{};{}", player.place, player.account_id);
    let (kind, fields) = match event.kind {
        EventKind::Kill { killer, victim } => (
            'K',
            format!(
                "{};{}",
                player(killer),
                victim.map_or_else(|| ";".to_owned(), player)
            ),
        ),
        EventKind::Death { victim } => ('D', player(victim)),
        EventKind::BatteryFound { by } => ('F', player(by)),
        EventKind::BatteryDelivered { by } => ('B', player(by)),
        EventKind::Capture { team, unit } => ('C', format!("{team};{unit}")),
        EventKind::Score { points } => ('S', format!("{};{}", points[0], points[1])),
        EventKind::End { points } => ('E', format!("{};{}", points[0], points[1])),
    };
    format!("{kind};{at};{after};{fields}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(place: u8, account_id: u32) -> Player {
        Player { place, account_id }
    }

    #[test]
    fn each_kind_has_its_row_and_the_longest_fits() {
        let between = Timing::Between {
            after: 5_000,
            by: 10_000,
        };
        let row = |timing, kind| event_row(&MatchEvent { timing, kind });
        assert_eq!(
            row(
                Timing::Exact(99_100),
                EventKind::Kill {
                    killer: player(0, 2),
                    victim: Some(player(1, 1)),
                }
            ),
            "K;99100;;0;2;1;1"
        );
        assert_eq!(
            row(
                between,
                EventKind::Kill {
                    killer: player(3, 0),
                    victim: None,
                }
            ),
            "K;10000;5000;3;0;;"
        );
        assert_eq!(
            row(
                between,
                EventKind::Death {
                    victim: player(4, 77)
                }
            ),
            "D;10000;5000;4;77"
        );
        assert_eq!(
            row(between, EventKind::BatteryFound { by: player(1, 5) }),
            "F;10000;5000;1;5"
        );
        assert_eq!(
            row(between, EventKind::BatteryDelivered { by: player(1, 5) }),
            "B;10000;5000;1;5"
        );
        assert_eq!(
            row(between, EventKind::Capture { team: 1, unit: 2 }),
            "C;10000;5000;1;2"
        );
        assert_eq!(
            row(between, EventKind::Score { points: [3, 1] }),
            "S;10000;5000;3;1"
        );
        assert_eq!(
            row(Timing::Exact(845_000), EventKind::End { points: [3, 1] }),
            "E;845000;;3;1"
        );
        let longest = row(
            Timing::Between {
                after: u32::MAX - 1,
                by: u32::MAX,
            },
            EventKind::Kill {
                killer: player(19, u32::MAX),
                victim: Some(player(19, u32::MAX)),
            },
        );
        assert!(longest.len() <= limits::NAME, "{longest}");
    }

    #[test]
    fn a_long_match_comes_in_pages_from_the_history() {
        use session::v026g::persist::history::{
            Eligibility, MatchInfo, MatchKind, MatchRecord, Winner,
        };
        use session::v026g::persist::{GameRepository, SqliteGameRepository};
        use session::v026g::replays::ReplayId;
        use survarium::v026g::maps::GameMode;

        let dir = std::env::temp_dir().join(format!("srp-match-events-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut repository = SqliteGameRepository::open_in(&dir).unwrap();
        let events: Vec<MatchEvent> = (1..=300u32)
            .map(|second| MatchEvent {
                timing: Timing::Between {
                    after: second * 1_000 - 1_000,
                    by: second * 1_000,
                },
                kind: EventKind::Death {
                    victim: player(1, 7),
                },
            })
            .collect();
        let record = MatchRecord {
            replay_id: ReplayId::new(0xAB, std::time::UNIX_EPOCH),
            kind: MatchKind::Public(Eligibility::Counted),
            info: MatchInfo {
                ended_at: 1,
                duration_ms: 300_000,
                map_id: 3,
                game_mode: GameMode::TeamDeathmatch,
                match_level: 1,
                winner: Winner::Unknown,
                team_points: [0, 0],
            },
            players: Vec::new(),
            events,
        };
        assert!(repository.record_match(&record).unwrap());
        let history = repository.history().unwrap();
        let page = |page| match_events_page(history.as_ref(), 0xAB, page);
        let first = page(0);
        assert_eq!(first.len(), 255);
        assert_eq!(first[0], "E1;ok;000000ab;300;0");
        assert_eq!(first[1], "D;1000;0;1;7");
        let second = page(1);
        assert_eq!(second.len(), 1 + 300 - PAGE_ROWS);
        assert_eq!(second.last().unwrap(), "D;300000;299000;1;7");
        assert_eq!(page(2), ["E1;ok;000000ab;300;2"]);
        assert_eq!(
            match_events_page(history.as_ref(), 0xCD, 0),
            ["E1;none;000000cd;0;0"]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn with_no_history_a_page_is_its_head() {
        assert_eq!(
            match_events_page(None, 0x1f07_c8fb, 0),
            ["E1;none;1f07c8fb;0;0"]
        );
    }
}
