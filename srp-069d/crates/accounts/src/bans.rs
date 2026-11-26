//! Sign-in bans and suspensions (login refusal `0C 02` / `0C 03`).

use rusqlite::{OptionalExtension, params};

use crate::{AccountId, Error, Store, now};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BanKind {
    /// `0C 02` (reason 15 shows the "soft ban" text).
    Banned,
    /// `0C 03`.
    Suspended,
}

impl BanKind {
    /// The refusal code the client reads.
    pub fn code(self) -> u8 {
        match self {
            BanKind::Banned => 2,
            BanKind::Suspended => 3,
        }
    }
}

/// An active ban.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ban {
    pub kind: BanKind,
    /// 0..15 (`st_ban_reason_*`, 0x2009584C0 table in the client).
    pub reason: u8,
    /// Unix seconds; 0 = permanent.
    pub until: i64,
}

/// The largest ban value the client shows correctly: `(v + 1) * 60 <= u32::MAX` (about 136 years).
pub const MAX_BAN_MINUTES: u32 = 71_582_787;

impl Ban {
    /// The `value` of the refusal: minutes left. PROVEN (0x2001298F1..0x2001298F6): the client shows
    /// `time() + (value + 1) * 60` computed in 32 bits, so it adds one minute itself and any value above
    /// [`MAX_BAN_MINUTES`] wraps (`u32::MAX` shows a permanent ban as ending now). A permanent ban sends the maximum.
    pub fn minutes_left(&self, at: i64) -> u32 {
        if self.until == 0 {
            return MAX_BAN_MINUTES;
        }
        let secs = (self.until - at).max(0);
        u32::try_from(secs / 60).unwrap_or(MAX_BAN_MINUTES).min(MAX_BAN_MINUTES)
    }
}

impl Store {
    /// Ban or suspend an account. `minutes` 0 = permanent. `reason` must be 0..=15.
    pub fn set_ban(&self, id: AccountId, kind: BanKind, reason: u8, minutes: u32) -> Result<(), Error> {
        if reason > 15 {
            return Err(Error::Invalid("ban reason must be 0..15"));
        }
        let until = if minutes == 0 { 0 } else { now() + i64::from(minutes) * 60 };
        let n = self.conn().execute(
            "UPDATE accounts SET ban_kind = ?1, ban_reason = ?2, ban_until = ?3 WHERE id = ?4",
            params![i64::from(kind.code()), i64::from(reason), until, id],
        )?;
        if n == 0 { Err(Error::NoSuchAccount) } else { Ok(()) }
    }

    pub fn clear_ban(&self, id: AccountId) -> Result<(), Error> {
        let n = self.conn().execute("UPDATE accounts SET ban_kind = 0, ban_reason = 0, ban_until = 0 WHERE id = ?1", [id])?;
        if n == 0 { Err(Error::NoSuchAccount) } else { Ok(()) }
    }

    /// The ban in force at unix time `at` (None: none, or it ran out).
    pub fn ban_at(&self, id: AccountId, at: i64) -> Result<Option<Ban>, Error> {
        let row: Option<(i64, i64, i64)> = self
            .conn()
            .query_row("SELECT ban_kind, ban_reason, ban_until FROM accounts WHERE id = ?1", [id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .optional()?;
        Ok(row.and_then(|(k, reason, until)| {
            let kind = match k {
                2 => BanKind::Banned,
                3 => BanKind::Suspended,
                _ => return None,
            };
            if until != 0 && until <= at {
                return None;
            }
            Some(Ban { kind, reason: reason.clamp(0, 15) as u8, until })
        }))
    }

    pub fn ban_of(&self, id: AccountId) -> Result<Option<Ban>, Error> {
        self.ban_at(id, now())
    }
}
