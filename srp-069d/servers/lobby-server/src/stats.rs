//! The single entry point for the facts of a finished match: [`record_match`].
//!
//! The local match path (`play::events`) calls it with what it really knows (map, mode, win / loss, faction; every
//! other field of [`MatchFacts`] is `None`). A future server-side match observer calls the same function with kills,
//! deaths, score and so on. Achievements (`achievements::record`) and daily quests (`quests::record`) both count from
//! the facts: a counter whose fact is absent does not move. The resulting messages go to the account's peer when it
//! is online.

use accounts::AccountId;
use game_data::MatchFacts;

use crate::dispatch::Ctx;
use crate::peer::Cross;

/// Count a finished match of `account` for achievements and daily quests and tell the player.
pub fn record_match(cx: &mut Ctx, account: AccountId, facts: &MatchFacts) {
    let mut msgs = vec![];
    // a local training match with bots counts for nothing (CHOICE after the client's "Rewards in this mode
    // are not awarded"): no achievement, quest, hunt card, hero row, league or clan progress
    if facts.local_bots {
        log::info!("account {account}: local bots match, nothing counted");
        return;
    }
    // every finished match counts for the client's newbie protection (match options +0x2D8)
    if facts.finished {
        if let Err(e) = cx.lobby.store.update_profile(account, |p| {
            p.matches = p.matches.saturating_add(1);
            Ok::<_, ()>(())
        }) {
            log::warn!("account {account}: match count: {e:?}");
        }
    }
    // a clan battle is a custom match: nothing 0.69d says counts it for achievements, quests, the hunt or the league
    // (docs/notes/clan-battles.md); its reputation still counts for the faction battle, its result for the clan
    // an inactive player (the world's flag B+0x9F0, PlayerDetail.inactive) earns no achievement and no daily-quest
    // progress from the match (CHOICE; he also loses the reward, the drops and the Winner's Contract day, online.rs
    // `pay`)
    let inactive = facts.inactive == Some(true);
    if inactive {
        log::info!("account {account}: inactive in the world: no achievements, no daily quests from this match");
    }
    if !facts.clan_battle {
        if !inactive {
            msgs.extend(crate::achievements::record(cx, account, facts));
            msgs.extend(crate::quests::record(cx, account, facts));
        }
        msgs.extend(crate::hunt::record(cx, account, facts));
        // an inactive player stores no Hero of the Day result either (inactive = no reward, no fine)
        if !inactive {
            crate::hero::record(cx, account, facts);
        }
    }
    msgs.extend(crate::faction_battle::record(cx, account, facts));
    if !facts.clan_battle {
        msgs.extend(crate::league::record(cx, account, facts));
    }
    crate::clans::record(cx, account, facts);
    for msg in msgs {
        cx.fx.push(Cross { to: account, msg });
    }
}
