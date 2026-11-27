//! Complaints and compliments about other players (`48 06` `comment_about_user`), kept for a moderator.

use rusqlite::{params, OptionalExtension};

use crate::{now, AccountId, Error, Store};

/// The two kinds. The wire byte is 1 for a complaint (the reply `83 48 06 01` shows `st_claim_added`), anything
/// else is a compliment (`st_compliment_added`); PROVEN for the reply, LIKELY that the request's kind byte is the same.
pub const KIND_COMPLAINT: u8 = 1;
pub const KIND_COMPLIMENT: u8 = 0;

/// What [`Store::add_comment`] decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentResult {
    Added,
    /// The sender filed a comment of this kind about this player already today (`st_claims_count_per_user_owerflow` /
    /// `st_compliment_count_per_user_owerflow`: the 0.69d texts state the rule for both kinds).
    SameUserToday,
    /// The sender reached the day's maximum of this kind (`st_claims_count_per_day_owerflow` /
    /// `st_compliment_count_per_day_owerflow`).
    DayLimit,
    /// The target is not an account, or is the sender.
    NoTarget,
}

/// One stored comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentRow {
    pub id: i64,
    pub at: i64,
    pub from: AccountId,
    pub from_name: String,
    pub target: AccountId,
    pub target_name: String,
    pub kind: u8,
    /// The reason (`comment_about_user` subIndex, PROVEN): complaint 1 bot / cheater, 2 swearing, 3 team kill,
    /// 4 account trading; compliment 1 friendly, 2 great player, 3 born commander.
    pub a: u8,
    /// The match id the comment is about (0 from the lobby chat), PROVEN.
    pub b: u32,
}

/// Length of the "day" the limits count in (UTC days; CHOICE: the client's text only says "for one day").
const DAY: i64 = 86_400;

impl Store {
    /// Store a complaint / compliment of `from` about `target`, unless a rule refuses it: no self comments, a complaint
    /// of a kind about the same player once a day, and the optional daily maximum of `kind`.
    pub fn add_comment(
        &self,
        from: AccountId,
        target: AccountId,
        kind: u8,
        a: u8,
        b: u32,
        day_max: Option<u32>,
    ) -> Result<CommentResult, Error> {
        let kind = if kind == KIND_COMPLAINT { KIND_COMPLAINT } else { KIND_COMPLIMENT };
        let at = now();
        let day_start = at - at.rem_euclid(DAY);
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let exists: Option<i64> = tx.query_row("SELECT id FROM accounts WHERE id = ?1", [target], |r| r.get(0)).optional()?;
        if exists.is_none() || target == from {
            return Ok(CommentResult::NoTarget);
        }
        let twice: i64 = tx.query_row(
            "SELECT COUNT(*) FROM user_comments WHERE from_account = ?1 AND target_account = ?2 AND kind = ?3 AND at >= ?4",
            params![from, target, i64::from(kind), day_start],
            |r| r.get(0),
        )?;
        if twice > 0 {
            return Ok(CommentResult::SameUserToday);
        }
        if let Some(max) = day_max {
            let n: i64 = tx.query_row(
                "SELECT COUNT(*) FROM user_comments WHERE from_account = ?1 AND kind = ?2 AND at >= ?3",
                params![from, i64::from(kind), day_start],
                |r| r.get(0),
            )?;
            if n >= i64::from(max) {
                return Ok(CommentResult::DayLimit);
            }
        }
        tx.execute(
            "INSERT INTO user_comments (at, from_account, target_account, kind, a, b) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![at, from, target, i64::from(kind), i64::from(a), i64::from(b)],
        )?;
        tx.commit()?;
        Ok(CommentResult::Added)
    }

    /// The newest comments first; `target` limits the list to one player.
    pub fn list_comments(&self, target: Option<AccountId>, limit: u32) -> Result<Vec<CommentRow>, Error> {
        let conn = self.conn();
        let mut st = conn.prepare(
            "SELECT c.id, c.at, c.from_account, COALESCE(f.nick, 'Player' || f.id), c.target_account, COALESCE(t.nick, 'Player' || t.id), c.kind, c.a, c.b
             FROM user_comments c
             LEFT JOIN accounts f ON f.id = c.from_account
             LEFT JOIN accounts t ON t.id = c.target_account
             WHERE (?1 IS NULL OR c.target_account = ?1)
             ORDER BY c.id DESC LIMIT ?2",
        )?;
        let rows = st
            .query_map(params![target, i64::from(limit)], |r| {
                Ok(CommentRow {
                    id: r.get(0)?,
                    at: r.get(1)?,
                    from: r.get(2)?,
                    from_name: r.get(3)?,
                    target: r.get(4)?,
                    target_name: r.get(5)?,
                    kind: r.get::<_, i64>(6)? as u8,
                    a: r.get::<_, i64>(7)? as u8,
                    b: r.get::<_, i64>(8)? as u32,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two() -> (Store, AccountId, AccountId) {
        let s = Store::open_in_memory().unwrap();
        let a = s.create_account("alpha", "password1").unwrap();
        let b = s.create_account("bravo", "password1").unwrap();
        (s, a, b)
    }

    #[test]
    fn complaint_once_per_user_per_day_and_not_self() {
        let (s, a, b) = two();
        assert_eq!(s.add_comment(a, b, 1, 0, 7, None).unwrap(), CommentResult::Added);
        assert_eq!(s.add_comment(a, b, 1, 0, 7, None).unwrap(), CommentResult::SameUserToday);
        assert_eq!(s.add_comment(b, b, 1, 0, 0, None).unwrap(), CommentResult::NoTarget);
        assert_eq!(s.add_comment(a, 9999, 0, 0, 0, None).unwrap(), CommentResult::NoTarget);
        // a compliment counts on its own, also once per user per day
        assert_eq!(s.add_comment(a, b, 0, 0, 0, None).unwrap(), CommentResult::Added);
        assert_eq!(s.add_comment(a, b, 0, 0, 0, None).unwrap(), CommentResult::SameUserToday);
    }

    #[test]
    fn day_maximum_counts_per_kind() {
        let (s, a, b) = two();
        let c = s.create_account("charlie", "password1").unwrap();
        let d = s.create_account("delta", "password1").unwrap();
        assert_eq!(s.add_comment(a, b, 0, 0, 0, Some(2)).unwrap(), CommentResult::Added);
        assert_eq!(s.add_comment(a, c, 0, 0, 0, Some(2)).unwrap(), CommentResult::Added);
        assert_eq!(s.add_comment(a, d, 0, 0, 0, Some(2)).unwrap(), CommentResult::DayLimit);
        // complaints have their own count
        assert_eq!(s.add_comment(a, b, 1, 0, 0, Some(2)).unwrap(), CommentResult::Added);
    }

    #[test]
    fn list_newest_first_with_names() {
        let (s, a, b) = two();
        s.add_comment(a, b, 1, 2, 3, None).unwrap();
        s.add_comment(b, a, 0, 0, 0, None).unwrap();
        let all = s.list_comments(None, 10).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!((all[0].from_name.as_str(), all[0].target_name.as_str(), all[0].kind), ("bravo", "alpha", 0));
        assert_eq!((all[1].a, all[1].b), (2, 3));
        assert_eq!(s.list_comments(Some(b), 10).unwrap().len(), 1);
    }
}
