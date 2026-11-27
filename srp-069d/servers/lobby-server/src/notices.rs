//! In-game notices of progress: character level-up (B1 type 0xA), "a friend reached level N" (B1 type 9) and
//! faction level-up (B1 type 4, text `#ru[...]`). Evidence, layouts and open points: docs/notes/notices.md.
//!
//! Where it hooks in: every change of the experience or of a faction's reputation goes through
//! `Store::update_profile`, which calls a hook after the commit ([`accounts::ProgressHook`]). The hook only queues
//! `(account, before, after)`; [`flush`] (called by the server after every handler and on every poll) turns the queue
//! into messages once the handler's own pushes (8B money, 8C level, 8E factions, ...) are out, because the client
//! reads its own level bar and reputation when it shows a notice.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use accounts::{public_id, AccountId, FriendState, ProgressMark};
use game_data::levels::faction_level;
use protocol::s2c::*;

use crate::build;
use crate::dispatch::Ctx;
use crate::peer::Cross;

/// B1 message type of the faction level-up text `#ru[<faction>:<level>][a][b]` (the client's `important_message_arrived`
/// 0x2001C2140 tests for type 4 at 0x1C3054; its Flash type is `ImportantMessage.FACTION_LEVEL_UP`).
pub const KIND_FACTION_LEVEL: u8 = 4;
/// B1 type 9: a player's level, shown to the receiver as `nickname` + "reached level N" (0x1C354C; Flash type 6).
pub const KIND_FRIEND_LEVEL: u8 = 9;
/// B1 type 0xA: the receiver's own level-up with its reward (0x1C370F; Flash `ImportantMessage.LEVEL_UP`).
pub const KIND_LEVEL_UP: u8 = 0x0A;
/// Text prefix of [`KIND_FACTION_LEVEL`] (exe string 0x20096A388).
pub const RU_PREFIX: &str = "#ru";
/// `Reward.MONEY` of the Flash constants (carried as `reward_type`; the level-up window itself always draws money).
const REWARD_MONEY: u32 = 2;
/// `reward_id` of a silver reward: the `Payment` enum, 0 = silver (`RewardRenderer`, progression-audit.md).
const PAYMENT_SILVER: u32 = 0;

/// What the hook queues: the account and its experience / reputation before and after one committed update.
pub type Pending = (AccountId, ProgressMark, ProgressMark);

/// The queue the store hook fills and [`flush`] empties.
pub type Queue = Arc<Mutex<Vec<Pending>>>;

/// Install the hook on the store; the returned queue belongs to the lobby.
pub fn install(store: &accounts::Store) -> Queue {
    let q: Queue = Arc::new(Mutex::new(Vec::new()));
    let hook_q = Arc::clone(&q);
    store.set_progress_hook(Box::new(move |id, before, after| {
        hook_q.lock().unwrap_or_else(|e| e.into_inner()).push((id, before, after));
    }));
    q
}

/// A message id the client has not seen: it de-duplicates the list by id (reader 0x200265550), so the ids of
/// different notices must differ; they start at a value derived from the start time so a restart does not reuse the
/// ids of messages a client still shows.
pub(crate) fn next_message_id() -> u32 {
    static BOOT: OnceLock<u32> = OnceLock::new();
    static COUNT: AtomicU32 = AtomicU32::new(0);
    let boot = *BOOT.get_or_init(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_secs() as u32));
    0x1000_0000 | (boot.wrapping_mul(4096).wrapping_add(COUNT.fetch_add(1, Ordering::Relaxed)) & 0x0FFF_FFFF)
}

/// Text of the type 0xA message, nine bracketed integers read by 0x1C4750 in this order (PROVEN, 0x1C3779..0x1C37FA):
/// levels gained, first new level (= old level + 1), experience before, experience now, reward type, reward id,
/// reward amount, and the two values of the premium proposal (0, 0 = none, see docs/notes/notices.md).
pub fn level_up_text(old_level: u32, new_level: u32, xp_before: u32, xp_now: u32, silver: u32) -> String {
    level_up_text_with(old_level, new_level, xp_before, xp_now, silver, (0, 0))
}

/// [`level_up_text`] with the premium proposal `(days without premium, days with premium)` ([`premium_days`]).
pub fn level_up_text_with(old_level: u32, new_level: u32, xp_before: u32, xp_now: u32, silver: u32, (a, b): (u32, u32)) -> String {
    let gained = new_level.saturating_sub(old_level);
    format!("[{gained}][{}][{xp_before}][{xp_now}][{REWARD_MONEY}][{PAYMENT_SILVER}][{silver}][{a}][{b}]", old_level + 1)
}

/// The premium proposal of a level-up: `(days to the next level without premium, days with premium)` from the player's own
/// pace (`earned` points over `days` days since the account was created; CHOICE, the original formula is in no source) and
/// the premium bonus [`crate::premium::MATCH_BONUS_PERCENT`] (OFFICIAL +50 %). The client shows the proposal only when the
/// first is larger (PROVEN 0x1C1F90), so `(0, 0)` = none: premium running, nothing left to earn or no pace yet.
pub fn premium_days(left: u32, earned: u32, days: u32, premium_running: bool) -> (u32, u32) {
    if premium_running || left == 0 || earned == 0 {
        return (0, 0);
    }
    let per_day = f64::from(earned) / f64::from(days.max(1));
    let with = per_day * (1.0 + f64::from(crate::premium::MATCH_BONUS_PERCENT) / 100.0);
    let a = (f64::from(left) / per_day).ceil().min(f64::from(u16::MAX)) as u32;
    let b = (f64::from(left) / with).ceil().min(f64::from(u16::MAX)) as u32;
    if a > b { (a, b) } else { (0, 0) }
}

/// Text of the type 9 message: `[level][reward][reward_type]` (0x1C3575..0x1C35AB); the window uses the level and
/// the name of the message, the other two are only handed to Flash.
pub fn friend_level_text(level: u32) -> String {
    format!("[{level}][0][0]")
}

/// Text of the type 4 message: `#ru[<faction>:<level>]` and the two premium-proposal values `[0][0]` (0x1C30F1..0x1C3188).
pub fn faction_rank_text(faction: u8, level: u32) -> String {
    faction_rank_text_with(faction, level, (0, 0))
}

/// [`faction_rank_text`] with the premium proposal pair ([`premium_days`] on that faction's reputation).
pub fn faction_rank_text_with(faction: u8, level: u32, (a, b): (u32, u32)) -> String {
    format!("{RU_PREFIX}[{faction}:{level}][{a}][{b}]")
}

/// The messages one queued change calls for: (receiver, message). The faction notices come with a fresh 8E first
/// (the client reads the points from its last 8E when it builds the window).
fn messages(cx: &Ctx, account: AccountId, before: ProgressMark, after: ProgressMark) -> Vec<(AccountId, S2c)> {
    let mut out = Vec::new();
    let gd = &cx.lobby.data;
    let (old, new) = (gd.levels.level_of(before.xp), gd.levels.level_of(after.xp));
    // the premium proposal needs the player's pace and premium state
    let now = crate::premium::unix_now();
    let days = cx.lobby.store.account(account).ok().flatten().map_or(1, |a| ((now - a.created_at).max(0) / 86_400 + 1) as u32);
    let premium = cx.lobby.store.load_profile(account).is_ok_and(|p| p.premium_left(now) > 0);
    if new > old {
        let silver = gd.levels.level_up_silver(before.xp, after.xp);
        let next = gd.levels.level_fields(after.xp).next;
        let proposal = premium_days(next.saturating_sub(after.xp), after.xp, days, premium);
        let text = level_up_text_with(old, new, before.xp, after.xp, silver, proposal);
        out.push((account, S2c::SystemMessage(SystemMessage { kind: KIND_LEVEL_UP, text, message_id: next_message_id(), ..Default::default() })));
        // the friends who are online see the new level
        let name = cx.lobby.store.account(account).ok().flatten().map(|a| a.name).unwrap_or_default();
        for f in cx.lobby.store.friends(account).unwrap_or_default() {
            if f.state == FriendState::Friend && cx.registry.is_online(f.id) {
                let m = SystemMessage {
                    kind: KIND_FRIEND_LEVEL,
                    sender: public_id(account),
                    name: name.clone(),
                    text: friend_level_text(new),
                    message_id: next_message_id(),
                    ..Default::default()
                };
                out.push((f.id, S2c::SystemMessage(m)));
            }
        }
    }
    let mut ranks = Vec::new();
    for i in 0..4 {
        let (lo, hi) = (faction_level(before.points[i]), faction_level(after.points[i]));
        for level in lo + 1..=hi {
            ranks.push((i as u8 + 1, level));
        }
    }
    if !ranks.is_empty() {
        if let Ok(p) = cx.lobby.store.load_profile(account) {
            out.push((account, S2c::Factions(build::factions_8e(&p))));
            // a Prestige rank (faction level 6..15) gives a skill point: the client reads the unlocked points from 8C
            if ranks.iter().any(|(_, l)| *l > game_data::levels::PRESTIGE_FROM_LEVEL) {
                out.push((account, S2c::Level(build::level_8c(gd, &p))));
            }
        }
        for (faction, level) in ranks {
            let points = after.points[usize::from(faction - 1)];
            let next = game_data::levels::FACTION_THRESHOLDS.get(level as usize + 1).copied().unwrap_or(0);
            let proposal = premium_days(next.saturating_sub(points), points, days, premium);
            let text = faction_rank_text_with(faction, level, proposal);
            out.push((account, S2c::SystemMessage(SystemMessage { kind: KIND_FACTION_LEVEL, text, message_id: next_message_id(), ..Default::default() })));
        }
    }
    out
}

/// Turn the queued changes into messages for the online peers. A receiver who is not signed in gets nothing: the
/// notices are not stored (docs/notes/notices.md, OPEN).
pub fn flush(cx: &mut Ctx) {
    let queued = std::mem::take(&mut *cx.lobby.notices.lock().unwrap_or_else(|e| e.into_inner()));
    for (account, before, after) in queued {
        for (to, msg) in messages(cx, account, before, after) {
            if cx.registry.is_online(to) {
                cx.fx.push(Cross { to, msg });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texts_have_the_bracket_layout_the_client_parses() {
        assert_eq!(level_up_text(1, 3, 100, 2200, 1000), "[2][2][100][2200][2][0][1000][0][0]");
        assert_eq!(friend_level_text(7), "[7][0][0]");
        assert_eq!(faction_rank_text(2, 3), "#ru[2:3][0][0]");
        assert_eq!(faction_rank_text_with(2, 3, (9, 6)), "#ru[2:3][9][6]");
        // the pace: 1000 a day, 3000 left -> 3 days, with +50 % -> 2 days; premium running or no pace -> none
        assert_eq!(premium_days(3000, 10_000, 10, false), (3, 2));
        assert_eq!(premium_days(3000, 10_000, 10, true), (0, 0));
        assert_eq!(premium_days(3000, 0, 10, false), (0, 0));
        assert_eq!(premium_days(0, 10_000, 10, false), (0, 0));
        assert_eq!(premium_days(100, 10_000, 10, false), (0, 0), "one day either way: no proposal");
        // every text fits the 256-byte field of B1
        assert!(level_up_text(99, 100, u32::MAX, u32::MAX, u32::MAX).len() < 256);
    }

    #[test]
    fn message_ids_are_distinct() {
        let a = next_message_id();
        let b = next_message_id();
        assert_ne!(a, b);
        assert_eq!(a & 0xF000_0000, 0x1000_0000);
    }
}
