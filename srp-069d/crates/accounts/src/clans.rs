//! Clans: storage and rules (social area: push 9A, requests 48 07..17 / 26).
//!
//! Layouts and limits from the client: at most 10 ranks and 107 members (the client's tables, 0x18BD60), clan name
//! `str(100)`, abbreviation `str(32)`, rank name `str(32)`, daily message `str(512)`. Rank 0 is the commander
//! (LIKELY: `rank_up_to_commander`). A rank carries a mask, bit 4 invite, bit 3 kick, bit 2 change rank,
//! bit 1 daily message, bit 0 battle commander (the order the client unpacks it, 0x2800A3). Rules without client
//! evidence (name length, who may do what) are the server's choices, named in `docs/notes/social.md`.

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::{AccountId, Error, Store, now};

pub const MAX_MEMBERS: usize = 107;
pub const MAX_RANKS: usize = 10;
pub const PERM_BATTLE: u8 = 1;
pub const PERM_DAILY: u8 = 2;
pub const PERM_RANK: u8 = 4;
pub const PERM_KICK: u8 = 8;
pub const PERM_INVITE: u8 = 16;
pub const ALL_PERMS: u8 = 31;
/// The standard ranks of a new clan (PROVEN display rule): a rank sent with the wire flag
/// `default` = 1 is shown by the clan window as the client's own text `clan_rank_<name>` in the player's language
/// (`ManageClanWindow.setRanksData`, `ClanListRenderer`; the lobby text table has exactly commander / assistant / warlord
/// / soldier = "Commander" / "Deputy commander" / "Sergeant" / "Soldier", all 11 languages), so the names are exactly
/// these lowercase suffixes; the client also refuses to delete them. Order (LIKELY: the table and the hierarchy) and the
/// permissions (not in the client): CHOICE.
pub const DEFAULT_RANKS: [(&str, u8); 4] = [
    ("commander", ALL_PERMS),
    ("assistant", PERM_INVITE | PERM_KICK | PERM_DAILY | PERM_BATTLE),
    ("warlord", PERM_INVITE | PERM_BATTLE),
    ("soldier", 0),
];
/// The standard rank new members join at (server-side only, nothing in the client says; CHOICE).
pub const JOIN_RANK: &str = "soldier";
/// Choice: invites expire after this many seconds.
pub const INVITE_TTL: i64 = 3 * 24 * 3600;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClanRank {
    pub name: String,
    pub perms: u8,
    /// a standard rank (wire `default` = 1: the client shows its own text for the name and cannot delete it); stored in
    /// the `is_default` column (schema v21 gave it this meaning)
    pub standard: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClanMember {
    pub id: AccountId,
    pub name: String,
    pub rank: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clan {
    pub id: u32,
    pub name: String,
    pub abbr: String,
    pub daily_message: String,
    pub xp: u32,
    /// Index = rank number.
    pub ranks: Vec<ClanRank>,
    pub members: Vec<ClanMember>,
}

impl Clan {
    pub fn member(&self, a: AccountId) -> Option<&ClanMember> {
        self.members.iter().find(|m| m.id == a)
    }
    pub fn perms_of(&self, a: AccountId) -> u8 {
        self.member(a).and_then(|m| self.ranks.get(m.rank as usize)).map(|r| r.perms).unwrap_or(0)
    }
    /// The rank new members join at: the standard [`JOIN_RANK`], else the last standard rank, else the last rank.
    pub fn default_rank(&self) -> u8 {
        let last = self.ranks.len().saturating_sub(1);
        self.ranks
            .iter()
            .position(|r| r.standard && r.name == JOIN_RANK)
            .or_else(|| self.ranks.iter().rposition(|r| r.standard))
            .unwrap_or(last) as u8
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ClanError {
    #[error(transparent)]
    Store(#[from] Error),
    #[error("not in a clan")]
    NotInClan,
    #[error("already in a clan")]
    AlreadyInClan,
    #[error("no such clan")]
    NoSuchClan,
    #[error("name or abbreviation not valid (reason {0})")]
    InvalidName(u8),
    #[error("name or abbreviation already used")]
    Taken,
    #[error("nothing reserved")]
    NoReservation,
    #[error("registration not paid")]
    NotPaid,
    #[error("not allowed")]
    Denied,
    #[error("the clan is full")]
    Full,
    #[error("no such member")]
    NoSuchMember,
    #[error("no such invite")]
    NoInvite,
    #[error("{0}")]
    Rule(&'static str),
}

impl From<rusqlite::Error> for ClanError {
    fn from(e: rusqlite::Error) -> Self {
        ClanError::Store(Error::Sqlite(e))
    }
}

/// Client reason codes of `83 48 09/0A` (jump table 0x192DAC): 0 short, 1 few letters, 2 exists, 3 space symbol,
/// 4 wrong symbol, 6 long.
pub const R_SHORT: u8 = 0;
pub const R_FEW_ALPHA: u8 = 1;
pub const R_EXISTS: u8 = 2;
pub const R_SPACE: u8 = 3;
pub const R_WRONG: u8 = 4;
pub const R_LONG: u8 = 6;

/// Choice: clan name 4..=32 characters, letters / digits / single inner spaces / `-` `_`, at least 3 letters.
pub fn check_clan_name(name: &str) -> Result<(), u8> {
    let n = name.chars().count();
    if n < 4 {
        return Err(R_SHORT);
    }
    if n > 32 {
        return Err(R_LONG);
    }
    if name.starts_with(' ') || name.ends_with(' ') || name.contains("  ") {
        return Err(R_SPACE);
    }
    if !name.chars().all(|c| c.is_alphanumeric() || c == ' ' || c == '-' || c == '_') {
        return Err(R_WRONG);
    }
    if name.chars().filter(|c| c.is_alphabetic()).count() < 3 {
        return Err(R_FEW_ALPHA);
    }
    Ok(())
}

/// Choice: abbreviation 2..=5 letters or digits.
pub fn check_clan_abbr(abbr: &str) -> Result<(), u8> {
    let n = abbr.chars().count();
    if n < 2 {
        return Err(R_SHORT);
    }
    if n > 5 {
        return Err(R_LONG);
    }
    if !abbr.chars().all(|c| c.is_alphanumeric()) {
        return Err(R_WRONG);
    }
    Ok(())
}

fn load(conn: &Connection, id: u32) -> Result<Option<Clan>, ClanError> {
    let head: Option<(String, String, String, i64)> = conn
        .query_row("SELECT name, abbr, daily_message, xp FROM clans WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .optional()?;
    let Some((name, abbr, daily_message, xp)) = head else { return Ok(None) };
    let mut st = conn.prepare("SELECT name, perms, is_default FROM clan_ranks WHERE clan_id = ?1 ORDER BY idx")?;
    let ranks = st
        .query_map([id], |r| Ok(ClanRank { name: r.get(0)?, perms: r.get::<_, i64>(1)? as u8, standard: r.get::<_, i64>(2)? != 0 }))?
        .collect::<Result<Vec<_>, _>>()?;
    let mut st = conn.prepare(
        "SELECT m.account_id, COALESCE(a.nick, 'Player' || a.id), m.rank FROM clan_members m JOIN accounts a ON a.id = m.account_id WHERE m.clan_id = ?1 ORDER BY m.rank, m.joined_at, m.account_id",
    )?;
    let members = st
        .query_map([id], |r| Ok(ClanMember { id: r.get(0)?, name: r.get(1)?, rank: r.get::<_, i64>(2)? as u8 }))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(Clan { id, name, abbr, daily_message, xp: xp.max(0) as u32, ranks, members }))
}

fn clan_id_of(conn: &Connection, a: AccountId) -> Result<Option<u32>, ClanError> {
    Ok(conn.query_row("SELECT clan_id FROM clan_members WHERE account_id = ?1", [a], |r| r.get::<_, i64>(0)).optional()?.map(|v| v as u32))
}

fn name_used(conn: &Connection, name: &str, abbr: &str, except: Option<AccountId>) -> Result<bool, ClanError> {
    let a: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM clans WHERE name = ?1 OR abbr = ?2)", params![name, abbr], |r| r.get(0))?;
    let b: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM clan_reservations WHERE (name = ?1 COLLATE NOCASE OR abbr = ?2 COLLATE NOCASE) AND account_id <> ?3)",
        params![name, abbr, except.unwrap_or(-1)],
        |r| r.get(0),
    )?;
    Ok(a || b)
}

impl Store {
    /// The clan an account is in.
    pub fn clan_id_of(&self, a: AccountId) -> Result<Option<u32>, ClanError> {
        clan_id_of(&self.conn(), a)
    }

    pub fn load_clan(&self, id: u32) -> Result<Option<Clan>, ClanError> {
        load(&self.conn(), id)
    }

    pub fn clan_of(&self, a: AccountId) -> Result<Option<Clan>, ClanError> {
        let conn = self.conn();
        match clan_id_of(&conn, a)? {
            Some(id) => load(&conn, id),
            None => Ok(None),
        }
    }

    /// Does the name or abbreviation collide with a clan or somebody else's reservation (checked by 48 09 / 0A).
    pub fn clan_text_taken(&self, name: Option<&str>, abbr: Option<&str>, except: AccountId) -> Result<bool, ClanError> {
        name_used(&self.conn(), name.unwrap_or("\u{0}"), abbr.unwrap_or("\u{0}"), Some(except))
    }

    /// 48 07: reserve a name + abbreviation for the account (replaces its earlier reservation unless paid).
    pub fn reserve_clan(&self, a: AccountId, name: &str, abbr: &str) -> Result<(), ClanError> {
        check_clan_name(name).map_err(ClanError::InvalidName)?;
        check_clan_abbr(abbr).map_err(ClanError::InvalidName)?;
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if clan_id_of(&tx, a)?.is_some() {
            return Err(ClanError::AlreadyInClan);
        }
        if name_used(&tx, name, abbr, Some(a))? {
            return Err(ClanError::Taken);
        }
        let paid: i64 = tx.query_row("SELECT paid FROM clan_reservations WHERE account_id = ?1", [a], |r| r.get(0)).optional()?.unwrap_or(0);
        tx.execute(
            "INSERT OR REPLACE INTO clan_reservations (account_id, name, abbr, paid, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![a, name, abbr, paid, now()],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// `(name, abbr, paid)`.
    pub fn clan_reservation(&self, a: AccountId) -> Result<Option<(String, String, bool)>, ClanError> {
        Ok(self
            .conn()
            .query_row("SELECT name, abbr, paid FROM clan_reservations WHERE account_id = ?1", [a], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? != 0)))
            .optional()?)
    }

    pub fn mark_clan_registration_paid(&self, a: AccountId) -> Result<(), ClanError> {
        let n = self.conn().execute("UPDATE clan_reservations SET paid = 1 WHERE account_id = ?1", [a])?;
        if n == 0 { Err(ClanError::NoReservation) } else { Ok(()) }
    }

    /// Silver deposited on the account's reservation so far (0 without one).
    pub fn clan_registration_collected(&self, a: AccountId) -> Result<u32, ClanError> {
        let v: Option<i64> = self.conn().query_row("SELECT collected FROM clan_reservations WHERE account_id = ?1", [a], |r| r.get(0)).optional()?;
        Ok(v.map_or(0, |v| v.clamp(0, i64::from(u32::MAX)) as u32))
    }

    /// 48 0D `amount`: deposit silver on the reservation: `min(amount, cost - collected)` is taken from the profile and
    /// added to the running total in ONE transaction; the reservation is paid when the total reaches `cost`. Returns
    /// `(taken, collected, paid)`. Nothing to take (already paid, amount 0) or too little silver: an error, nothing changes.
    pub fn deposit_clan_registration(&self, a: AccountId, amount: u32, cost: u32) -> Result<(u32, u32, bool), ClanError> {
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (collected, paid): (i64, i64) = tx
            .query_row("SELECT collected, paid FROM clan_reservations WHERE account_id = ?1", [a], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?
            .ok_or(ClanError::NoReservation)?;
        let collected = collected.clamp(0, i64::from(cost)) as u32;
        let take = amount.min(cost.saturating_sub(collected));
        if paid != 0 || take == 0 {
            return Err(ClanError::Rule("st_cant_buy_clan_registration"));
        }
        let mut p = crate::load_profile_conn(&tx, a)?;
        p.spend(crate::Currency::Silver, u64::from(take)).map_err(|_| ClanError::Rule("st_not_enough_money"))?;
        crate::save_profile_tx(&tx, a, &p)?;
        let now_collected = collected + take;
        let now_paid = now_collected >= cost;
        tx.execute("UPDATE clan_reservations SET collected = ?1, paid = ?2 WHERE account_id = ?3", params![i64::from(now_collected), i64::from(now_paid), a])?;
        tx.commit()?;
        Ok((take, now_collected, now_paid))
    }

    /// 48 0D: pay the registration of the reserved clan: the silver is taken and the reservation marked paid in ONE
    /// transaction (a failure of either leaves both untouched).
    pub fn pay_clan_registration(&self, a: AccountId, cost: u64) -> Result<(), ClanError> {
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let paid: i64 = tx
            .query_row("SELECT paid FROM clan_reservations WHERE account_id = ?1", [a], |r| r.get(0))
            .optional()?
            .ok_or(ClanError::NoReservation)?;
        if paid != 0 {
            return Err(ClanError::Rule("st_cant_buy_clan_registration"));
        }
        let mut p = crate::load_profile_conn(&tx, a)?;
        p.spend(crate::Currency::Silver, cost).map_err(|_| ClanError::Rule("st_not_enough_money"))?;
        crate::save_profile_tx(&tx, a, &p)?;
        tx.execute("UPDATE clan_reservations SET paid = 1 WHERE account_id = ?1", [a])?;
        tx.commit()?;
        Ok(())
    }

    pub fn cancel_clan_reservation(&self, a: AccountId) -> Result<(), ClanError> {
        self.conn().execute("DELETE FROM clan_reservations WHERE account_id = ?1", [a])?;
        Ok(())
    }

    /// 48 08: found the clan from the paid reservation; the founder is the commander (rank 0).
    pub fn create_clan(&self, a: AccountId) -> Result<u32, ClanError> {
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if clan_id_of(&tx, a)?.is_some() {
            return Err(ClanError::AlreadyInClan);
        }
        let (name, abbr, paid): (String, String, i64) = tx
            .query_row("SELECT name, abbr, paid FROM clan_reservations WHERE account_id = ?1", [a], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()?
            .ok_or(ClanError::NoReservation)?;
        if paid == 0 {
            return Err(ClanError::NotPaid);
        }
        let exists: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM clans WHERE name = ?1 OR abbr = ?2)", params![name, abbr], |r| r.get(0))?;
        if exists {
            return Err(ClanError::Taken);
        }
        tx.execute("INSERT INTO clans (name, abbr, created_at) VALUES (?1, ?2, ?3)", params![name, abbr, now()])?;
        let id = tx.last_insert_rowid();
        for (i, (rn, perms)) in DEFAULT_RANKS.iter().enumerate() {
            tx.execute(
                "INSERT INTO clan_ranks (clan_id, idx, name, perms, is_default) VALUES (?1, ?2, ?3, ?4, 1)",
                params![id, i as i64, rn, i64::from(*perms)],
            )?;
        }
        tx.execute("INSERT INTO clan_members (account_id, clan_id, rank, joined_at) VALUES (?1, ?2, 0, ?3)", params![a, id, now()])?;
        tx.execute("DELETE FROM clan_reservations WHERE account_id = ?1", [a])?;
        tx.execute("DELETE FROM clan_invites WHERE account_id = ?1", [a])?;
        tx.commit()?;
        Ok(id as u32)
    }

    /// 48 10: the inviter (needs the invite permission) invites `target`.
    pub fn clan_invite(&self, inviter: AccountId, target: AccountId) -> Result<u32, ClanError> {
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let cid = clan_id_of(&tx, inviter)?.ok_or(ClanError::NotInClan)?;
        let clan = load(&tx, cid)?.ok_or(ClanError::NoSuchClan)?;
        if clan.perms_of(inviter) & PERM_INVITE == 0 {
            return Err(ClanError::Denied);
        }
        if clan_id_of(&tx, target)?.is_some() {
            return Err(ClanError::AlreadyInClan);
        }
        let exists: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ?1)", [target], |r| r.get(0))?;
        if !exists {
            return Err(Error::NoSuchAccount.into());
        }
        if clan.members.len() >= MAX_MEMBERS {
            return Err(ClanError::Full);
        }
        tx.execute(
            "INSERT OR REPLACE INTO clan_invites (clan_id, account_id, inviter_id, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![cid, target, inviter, now()],
        )?;
        tx.commit()?;
        Ok(cid)
    }

    /// Open invites of an account: `(clan id, inviter)`, newest first; expired ones are skipped.
    pub fn clan_invites_of(&self, target: AccountId) -> Result<Vec<(u32, AccountId)>, ClanError> {
        let conn = self.conn();
        let mut st = conn.prepare("SELECT clan_id, inviter_id FROM clan_invites WHERE account_id = ?1 AND created_at > ?2 ORDER BY created_at DESC")?;
        let rows = st.query_map(params![target, now() - INVITE_TTL], |r| Ok((r.get::<_, i64>(0)? as u32, r.get::<_, i64>(1)?)))?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// 48 0B accept: join the clan with the default rank.
    pub fn clan_accept(&self, target: AccountId, clan: u32) -> Result<(), ClanError> {
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let live: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM clan_invites WHERE clan_id = ?1 AND account_id = ?2 AND created_at > ?3)",
            params![clan, target, now() - INVITE_TTL],
            |r| r.get(0),
        )?;
        if !live {
            return Err(ClanError::NoInvite);
        }
        if clan_id_of(&tx, target)?.is_some() {
            return Err(ClanError::AlreadyInClan);
        }
        let c = load(&tx, clan)?.ok_or(ClanError::NoSuchClan)?;
        if c.members.len() >= MAX_MEMBERS {
            return Err(ClanError::Full);
        }
        tx.execute("INSERT INTO clan_members (account_id, clan_id, rank, joined_at) VALUES (?1, ?2, ?3, ?4)", params![target, clan, i64::from(c.default_rank()), now()])?;
        tx.execute("DELETE FROM clan_invites WHERE account_id = ?1", [target])?;
        tx.execute("DELETE FROM clan_reservations WHERE account_id = ?1", [target])?;
        tx.commit()?;
        Ok(())
    }

    pub fn clan_decline(&self, target: AccountId, clan: u32) -> Result<(), ClanError> {
        self.conn().execute("DELETE FROM clan_invites WHERE clan_id = ?1 AND account_id = ?2", params![clan, target])?;
        Ok(())
    }

    /// Leave the clan. The commander may leave only when he is alone (the clan is then disbanded); otherwise he
    /// must hand over first (choice). Returns `(clan id, remaining members, disbanded)`.
    pub fn clan_leave(&self, a: AccountId) -> Result<(u32, Vec<AccountId>, bool), ClanError> {
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let cid = clan_id_of(&tx, a)?.ok_or(ClanError::NotInClan)?;
        let clan = load(&tx, cid)?.ok_or(ClanError::NoSuchClan)?;
        let me = clan.member(a).ok_or(ClanError::NotInClan)?;
        if me.rank == 0 {
            if clan.members.len() > 1 {
                return Err(ClanError::Rule("st_cant_oust_commander"));
            }
            tx.execute("DELETE FROM clans WHERE id = ?1", [cid])?;
            tx.commit()?;
            return Ok((cid, vec![], true));
        }
        tx.execute("DELETE FROM clan_members WHERE account_id = ?1", [a])?;
        tx.commit()?;
        Ok((cid, clan.members.iter().map(|m| m.id).filter(|x| *x != a).collect(), false))
    }

    /// Disband (commander only). Returns the former members.
    pub fn clan_disband(&self, by: AccountId) -> Result<(u32, Vec<AccountId>), ClanError> {
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let cid = clan_id_of(&tx, by)?.ok_or(ClanError::NotInClan)?;
        let clan = load(&tx, cid)?.ok_or(ClanError::NoSuchClan)?;
        if clan.member(by).map(|m| m.rank) != Some(0) {
            return Err(ClanError::Denied);
        }
        tx.execute("DELETE FROM clans WHERE id = ?1", [cid])?;
        tx.commit()?;
        Ok((cid, clan.members.iter().map(|m| m.id).collect()))
    }

    /// Kick (needs the kick permission and a strictly higher rank than the target's).
    pub fn clan_kick(&self, by: AccountId, target: AccountId) -> Result<u32, ClanError> {
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let cid = clan_id_of(&tx, by)?.ok_or(ClanError::NotInClan)?;
        let clan = load(&tx, cid)?.ok_or(ClanError::NoSuchClan)?;
        let (me, t) = (clan.member(by).ok_or(ClanError::NotInClan)?, clan.member(target).ok_or(ClanError::NoSuchMember)?);
        if clan.perms_of(by) & PERM_KICK == 0 || t.rank <= me.rank {
            return Err(ClanError::Denied);
        }
        tx.execute("DELETE FROM clan_members WHERE account_id = ?1", [target])?;
        tx.commit()?;
        Ok(cid)
    }

    /// 48 13 (up) / 48 12 a=0 (down). Needs the change-rank permission; only members ranked below the actor
    /// move, and not up to or past the actor's own rank, except that the commander can promote one member to
    /// commander: they swap places (hand over).
    pub fn clan_move_rank(&self, by: AccountId, target: AccountId, up: bool) -> Result<(u32, bool), ClanError> {
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let cid = clan_id_of(&tx, by)?.ok_or(ClanError::NotInClan)?;
        let clan = load(&tx, cid)?.ok_or(ClanError::NotInClan)?;
        let me = clan.member(by).ok_or(ClanError::NotInClan)?.rank;
        let t = clan.member(target).ok_or(ClanError::NoSuchMember)?.rank;
        if clan.perms_of(by) & PERM_RANK == 0 || t <= me || by == target {
            return Err(ClanError::Denied);
        }
        let last = clan.ranks.len() as u8 - 1;
        let new = if up { t - 1 } else { t.checked_add(1).filter(|n| *n <= last).ok_or(ClanError::Rule("st_cant_change_rank_clan_member"))? };
        let mut handover = false;
        if new == me {
            if me != 0 {
                return Err(ClanError::Denied);
            }
            // hand over: the old commander drops to rank 1
            tx.execute("UPDATE clan_members SET rank = 1 WHERE account_id = ?1", [by])?;
            handover = true;
        }
        tx.execute("UPDATE clan_members SET rank = ?1 WHERE account_id = ?2", params![i64::from(new), target])?;
        tx.commit()?;
        Ok((cid, handover))
    }

    /// 48 11: set one permission bit (`bit` = the mask bit, 0..4) of a rank. Commander only; rank 0 is fixed.
    pub fn clan_set_perm(&self, by: AccountId, rank: u8, bit: u8, value: bool) -> Result<u32, ClanError> {
        if bit > 4 {
            return Err(ClanError::Rule("st_cant_change_permission"));
        }
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let cid = clan_id_of(&tx, by)?.ok_or(ClanError::NotInClan)?;
        let clan = load(&tx, cid)?.ok_or(ClanError::NotInClan)?;
        if clan.member(by).map(|m| m.rank) != Some(0) || rank == 0 || rank as usize >= clan.ranks.len() {
            return Err(ClanError::Denied);
        }
        let old = clan.ranks[rank as usize].perms;
        let new = if value { old | (1 << bit) } else { old & !(1 << bit) };
        tx.execute("UPDATE clan_ranks SET perms = ?1 WHERE clan_id = ?2 AND idx = ?3", params![i64::from(new), cid, i64::from(rank)])?;
        tx.commit()?;
        Ok(cid)
    }

    /// 48 15: append a rank without permissions (commander only, at most 10 ranks).
    pub fn clan_create_rank(&self, by: AccountId, name: &str) -> Result<u32, ClanError> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 31 {
            return Err(ClanError::Rule("st_string_pattern_error"));
        }
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let cid = clan_id_of(&tx, by)?.ok_or(ClanError::NotInClan)?;
        let clan = load(&tx, cid)?.ok_or(ClanError::NotInClan)?;
        if clan.member(by).map(|m| m.rank) != Some(0) {
            return Err(ClanError::Denied);
        }
        if clan.ranks.len() >= MAX_RANKS {
            return Err(ClanError::Rule("st_cant_create_clan_rank"));
        }
        tx.execute("INSERT INTO clan_ranks (clan_id, idx, name, perms, is_default) VALUES (?1, ?2, ?3, 0, 0)", params![cid, clan.ranks.len() as i64, name])?;
        tx.commit()?;
        Ok(cid)
    }

    /// 48 16: remove a rank (commander only; never a standard rank, `st_cant_delete_default_rank`, like the client's
    /// disabled Delete button). Its members fall to the join rank; ranks behind it move up one place.
    pub fn clan_remove_rank(&self, by: AccountId, rank: u8) -> Result<u32, ClanError> {
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let cid = clan_id_of(&tx, by)?.ok_or(ClanError::NotInClan)?;
        let clan = load(&tx, cid)?.ok_or(ClanError::NotInClan)?;
        if clan.member(by).map(|m| m.rank) != Some(0) {
            return Err(ClanError::Denied);
        }
        let n = clan.ranks.len();
        if rank == 0 || rank as usize >= n || clan.ranks[rank as usize].standard {
            return Err(ClanError::Rule("st_cant_delete_default_rank"));
        }
        let def = clan.default_rank();
        tx.execute("UPDATE clan_members SET rank = ?1 WHERE clan_id = ?2 AND rank = ?3", params![i64::from(def), cid, i64::from(rank)])?;
        tx.execute("DELETE FROM clan_ranks WHERE clan_id = ?1 AND idx = ?2", params![cid, i64::from(rank)])?;
        tx.execute("UPDATE clan_ranks SET idx = idx - 1 WHERE clan_id = ?1 AND idx > ?2", params![cid, i64::from(rank)])?;
        tx.execute("UPDATE clan_members SET rank = rank - 1 WHERE clan_id = ?1 AND rank > ?2", params![cid, i64::from(rank)])?;
        tx.commit()?;
        Ok(cid)
    }

    /// 48 17.
    pub fn clan_set_daily_message(&self, by: AccountId, text: &str) -> Result<u32, ClanError> {
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let cid = clan_id_of(&tx, by)?.ok_or(ClanError::NotInClan)?;
        let clan = load(&tx, cid)?.ok_or(ClanError::NotInClan)?;
        if clan.perms_of(by) & PERM_DAILY == 0 {
            return Err(ClanError::Denied);
        }
        let text: String = text.chars().take(500).collect();
        tx.execute("UPDATE clans SET daily_message = ?1 WHERE id = ?2", params![text, cid])?;
        tx.commit()?;
        Ok(cid)
    }

    /// Add experience to a clan (saturating); returns `(old, new)` experience, None when the clan does not exist.
    pub fn clan_add_xp(&self, clan: u32, amount: u32) -> Result<Option<(u32, u32)>, ClanError> {
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let old: Option<i64> = tx.query_row("SELECT xp FROM clans WHERE id = ?1", [clan], |r| r.get(0)).optional()?;
        let Some(old) = old else { return Ok(None) };
        let old = old.max(0) as u32;
        let new = old.saturating_add(amount);
        tx.execute("UPDATE clans SET xp = ?1 WHERE id = ?2", params![new, clan])?;
        tx.commit()?;
        Ok(Some((old, new)))
    }

    /// Clan abbreviation of an account (empty without a clan): the tag shown in lists and chat.
    pub fn clan_abbr_of(&self, a: AccountId) -> String {
        self.conn()
            .query_row("SELECT c.abbr FROM clan_members m JOIN clans c ON c.id = m.clan_id WHERE m.account_id = ?1", [a], |r| r.get(0))
            .unwrap_or_default()
    }

    /// Clans by name or abbreviation containing `query` (case-insensitive), at most `limit`.
    pub fn search_clans(&self, query: &str, limit: usize) -> Result<Vec<Clan>, ClanError> {
        if query.is_empty() {
            return Ok(vec![]);
        }
        let pat = format!("%{}%", query.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
        let conn = self.conn();
        let ids: Vec<i64> = {
            let mut st = conn.prepare("SELECT id FROM clans WHERE name LIKE ?1 ESCAPE '\\' OR abbr LIKE ?1 ESCAPE '\\' ORDER BY name LIMIT ?2")?;
            st.query_map(params![pat, limit as i64], |r| r.get(0))?.collect::<Result<_, _>>()?
        };
        let mut out = vec![];
        for id in ids {
            if let Some(c) = load(&conn, id as u32)? {
                out.push(c);
            }
        }
        Ok(out)
    }
}
