//! Daily AFK removals belong to the account, across matches and restarts.
//! Only the match server's AFK removal event calls this owner; ordinary
//! leaves, timeouts and exempt matches do not contribute.

use std::num::NonZeroU32;

use crate::{Ban, LockExt, SessionStore, unix_now};

const DAY_SECONDS: u64 = 86_400;

/// A UTC calendar day, counted from the Unix epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct UtcDay(u64);

/// A nonempty day's history. Absence is `None` on the account.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DailyKicks {
    day: UtcDay,
    count: NonZeroU32,
}

impl DailyKicks {
    /// Parse the database's day and positive count once at its boundary.
    pub(crate) fn parse(day: u64, count: u32) -> Option<Self> {
        Some(Self {
            day: UtcDay(day),
            count: NonZeroU32::new(count)?,
        })
    }

    pub(crate) fn stored(self) -> (u64, u32) {
        (self.day.0, self.count.get())
    }

    /// One removal at `now` (Unix seconds): reset at UTC midnight, then
    /// count it and return the penalty at exactly kicks three and six.
    fn removed(previous: Option<Self>, now: u64) -> (Self, Option<Ban>) {
        let day = UtcDay(now / DAY_SECONDS);
        let count = match previous.filter(|previous| previous.day == day) {
            Some(previous) => previous.count.saturating_add(1),
            None => NonZeroU32::MIN,
        };
        let seconds = match count.get() {
            3 => Some(30 * 60),
            6 => Some(6 * 60 * 60),
            _ => None,
        };
        let ban = seconds.map(|seconds| Ban {
            until: Some(now.saturating_add(seconds)),
            reason: format!("AFK: {count} kicks in one UTC day"),
        });
        (Self { day, count }, ban)
    }
}

impl SessionStore {
    /// Record an actual AFK kick and enforce any daily penalty. Counts and
    /// bans are saved together; a ban ends existing sessions and prevents
    /// signing in through the ordinary account-ban path. An existing
    /// longer or permanent ban is never shortened. Returns the ban applied
    /// at a threshold (including a stronger ban already in force).
    pub fn record_afk_kick(&self, account_id: u32) -> Option<Ban> {
        self.record_afk_kick_at(account_id, unix_now())
    }

    pub(crate) fn record_afk_kick_at(&self, account_id: u32, now: u64) -> Option<Ban> {
        let account = self.account(account_id)?;
        let ban = {
            let mut account = account.locked();
            let (kicks, penalty) = DailyKicks::removed(account.afk_kicks, now);
            account.afk_kicks = Some(kicks);
            penalty.map(|penalty| {
                let ban = match account.ban_at(now) {
                    Some(existing)
                        if existing.until.is_none() || existing.until > penalty.until =>
                    {
                        existing.clone()
                    }
                    _ => penalty,
                };
                account.ban = Some(ban.clone());
                ban
            })
        };
        self.mark_dirty(account_id);
        if let Some(ban) = &ban {
            self.match_results.locked().remove(&account_id);
            self.kick(|session| session.account_id == account_id);
            log::info!(
                "Account {account_id}: {}; banned until {:?}",
                ban.reason,
                ban.until
            );
        }
        // Also persist ended sessions before returning. Failed writes stay
        // dirty for the existing saver to retry.
        self.flush();
        ban
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn penalties_are_on_kicks_three_and_six_only() {
        let mut history = None;
        for kick in 1..=9 {
            let now = DAY_SECONDS * 100 + u64::from(kick) * 60;
            let (next, ban) = DailyKicks::removed(history, now);
            assert_eq!(next.stored(), (100, kick));
            let expected = match kick {
                3 => Some(now + 1800),
                6 => Some(now + 21_600),
                _ => None,
            };
            assert_eq!(ban.map(|ban| ban.until.unwrap()), expected);
            history = Some(next);
        }
    }

    #[test]
    fn midnight_starts_a_new_count() {
        let mut history = None;
        for now in [DAY_SECONDS - 2, DAY_SECONDS - 1, DAY_SECONDS] {
            let (next, ban) = DailyKicks::removed(history, now);
            assert!(ban.is_none());
            history = Some(next);
        }
        assert_eq!(history.unwrap().stored(), (1, 1));
        let (_, ban) = DailyKicks::removed(DailyKicks::parse(0, 2), DAY_SECONDS - 1);
        let ban = ban.unwrap();
        assert!(ban.holds(DAY_SECONDS));
        assert!(!ban.holds(DAY_SECONDS - 1 + 1800));
    }

    #[test]
    fn empty_history_is_not_a_zero_count_row() {
        assert!(DailyKicks::parse(1, 0).is_none());
    }

    #[test]
    fn an_afk_penalty_never_shortens_an_existing_ban() {
        for until in [None, Some(100_000)] {
            let store = SessionStore::new();
            let id = store.sign_in("banned", b"pw").unwrap();
            let existing = Ban {
                until,
                reason: "admin".into(),
            };
            store.ban_account(id, existing.clone());
            assert!(store.record_afk_kick_at(id, 1).is_none());
            assert!(store.record_afk_kick_at(id, 2).is_none());
            assert_eq!(store.record_afk_kick_at(id, 3), Some(existing.clone()));
            assert_eq!(store.account(id).unwrap().locked().ban, Some(existing));
        }
    }

    #[test]
    fn a_sign_in_already_checked_cannot_escape_an_afk_ban() {
        for during_insert in [false, true] {
            let store = std::sync::Arc::new(SessionStore::new());
            let signed_in = store
                .sign_in_from(
                    "idle",
                    b"pw",
                    "127.0.0.1".parse().unwrap(),
                    std::time::Instant::now(),
                )
                .unwrap();
            let id = signed_in.account_id();
            store.record_afk_kick(id);
            store.record_afk_kick(id);
            if during_insert {
                let banning = store.clone();
                *store.test_hook.locked() = Some((
                    crate::HookPoint::SessionInserted,
                    Box::new(move || {
                        banning.record_afk_kick(id);
                    }),
                ));
            } else {
                store.record_afk_kick(id);
            }
            assert_eq!(store.create_signed_in_session(signed_in, 0), None);
            assert_eq!(store.session_count(), 0);
            assert!(store.test_hook.locked().is_none());
        }
    }
}
