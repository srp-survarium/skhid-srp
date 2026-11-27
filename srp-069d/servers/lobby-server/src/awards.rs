//! One-time awards of `unique_awards.options` (tutorial) and the award type enum. See `docs/notes/progression-audit.md`.
//!
//! The `type` of an award row is the client's `survarium.constants.Reward` enum (PROVEN: lobby movie class
//! `Reward`, read by `ItemFactory.fillRewardItems`, which receives the rows through `root.set_tutorial_rewards`, exe
//! 0x2001C8330, key names `type`, `param`, `amount`):
//!
//! | type | name | what `param` / `amount` mean |
//! |---|---|---|
//! | 0 | EXPERIENCE | amount = experience |
//! | 1 | REPUTATION | param = faction (`RewardRenderer`: icon `faction_<param>`), amount = reputation |
//! | 2 | MONEY | param = `Payment`: 0 silver, 1 gold, 3 event coins, 4 spare parts (`CRAFT_DETAILS`), 9 league tokens |
//! | 3 | ITEM | param = dictionary id, amount = count |
//! | 4 | RATING | (rating points) |
//! | 5 | POINT | (level points) |
//! | 6 | DETAILS | amount = spare parts |
//! | 7 | PREMIUM | (premium time) |
//! | 8..10 | EVENTCURR1..3 | event currencies |
//!
//! Paid here: 0, 1 (credited to the faction `param`), 2 with payment 0 / 1 / 4 / 9, 3, 6. Everything else is returned as
//! unpaid and logged (no account field for it). Rows with a type the client enum names differently from the data's use
//! (`game_world_tutorial_completed` row 2: type 4 with an item id) are unpaid too, see the notes.

use accounts::{Currency, CLAIM_GAME_WORLD_TUTORIAL, CLAIM_TUTORIAL};
use game_data::progression::Award;
use game_data::rewards::MatchDrops;
use game_data::GameData;
use protocol::s2c::S2c;

use crate::dispatch::{update_account, Ctx};
use crate::local::MatchOutcome;
use crate::peer::Peer;

pub const EXPERIENCE: u32 = 0;
pub const REPUTATION: u32 = 1;
pub const MONEY: u32 = 2;
pub const ITEM: u32 = 3;
pub const DETAILS: u32 = 6;
pub const PAY_SILVER: u32 = 0;
pub const PAY_GOLD: u32 = 1;
pub const PAY_DETAILS: u32 = 4;
pub const PAY_LEAGUE: u32 = 9;

/// What a set of award rows pays, by wallet.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Payout {
    pub exp: u32,
    pub silver: u32,
    pub gold: u32,
    pub parts: u32,
    pub league: u32,
    /// (faction, reputation)
    pub reputation: Vec<(u8, u32)>,
    /// (dict id, count), only dictionary items that exist
    pub items: Vec<(u16, u32)>,
    /// rows this server has no account field for
    pub unpaid: Vec<Award>,
}

/// Resolve award rows through the type enum.
pub fn resolve(gd: &GameData, awards: &[Award]) -> Payout {
    let mut o = Payout::default();
    for a in awards {
        match (a.kind, a.param) {
            (EXPERIENCE, _) => o.exp += a.amount,
            (REPUTATION, f @ 1..=4) => o.reputation.push((f as u8, a.amount)),
            (MONEY, PAY_SILVER) => o.silver += a.amount,
            (MONEY, PAY_GOLD) => o.gold += a.amount,
            (MONEY, PAY_DETAILS) | (DETAILS, _) => o.parts += a.amount,
            (MONEY, PAY_LEAGUE) => o.league += a.amount,
            (ITEM, d) if u16::try_from(d).is_ok_and(|d| gd.items.contains(d)) => o.items.push((d as u16, a.amount)),
            _ => o.unpaid.push(*a),
        }
    }
    o
}

/// Pay the rows named `awards_name` of `unique_awards.options` to the account once: the claim `claim` is stored with the
/// payment. Returns false when it was already paid (or the account is unknown). The client gets the usual pushes
/// (8B money, 8C level, 8E factions, the received-item popup).
pub fn pay_once(cx: &mut Ctx, peer: &mut Peer, claim: &str, awards_name: &str) -> bool {
    let Some(account) = peer.account else { return false };
    let gd = &cx.lobby.data;
    let rows = gd.progression.tutorial_awards.get(awards_name).cloned().unwrap_or_default();
    let pay = resolve(gd, &rows);
    let first = update_account(cx, account, |p| {
        if !p.claim(claim) {
            return false;
        }
        p.credit(Currency::Gold, u64::from(pay.gold));
        p.credit(Currency::League, u64::from(pay.league));
        for &(f, n) in &pay.reputation {
            p.add_points(f, n);
        }
        true
    });
    if first != Some(true) {
        return false;
    }
    for a in &pay.unpaid {
        log::warn!("{awards_name}: award type {} param {} amount {} has no account field here: not paid", a.kind, a.param, a.amount);
    }
    log::info!("{awards_name}: paid exp {} silver {} gold {} parts {} league {} items {:?}", pay.exp, pay.silver, pay.gold, pay.parts, pay.league, pay.items);
    let outcome = MatchOutcome { exp: pay.exp, silver: pay.silver, ..Default::default() };
    for m in crate::play::reward_messages(cx, account, outcome, MatchDrops { parts: pay.parts, items: pay.items.clone(), ..Default::default() }) {
        peer.push(&m);
    }
    // gold, league tokens and reputation went in before: show what changed even when nothing else was paid
    let (money, factions) = extra_pushes(&pay);
    if money {
        crate::shop::send_money(cx, peer);
    }
    if factions {
        if let Ok(p) = cx.lobby.store.load_profile(account) {
            peer.push(&S2c::Factions(crate::build::factions_8e(&p)));
        }
    }
    true
}

/// Which pushes the paid rows still need after `reward_messages` (which sends 8B / 8C / 8E when experience, silver, spare
/// parts or items were paid): (8B for gold / league tokens, 8E for reputation).
fn extra_pushes(pay: &Payout) -> (bool, bool) {
    if pay.exp > 0 || pay.silver > 0 || pay.parts > 0 || !pay.items.is_empty() {
        return (false, false);
    }
    (pay.gold > 0 || pay.league > 0, !pay.reputation.is_empty())
}

/// `4A 01 01`: the client's tutorial finished while the account's "tutorial complete" flag was clear (the sole caller of
/// the request, exe 0x2002CA247; it shows the `tutorial_completed` rewards in the same step). Pays them once.
pub fn tutorial_completed(cx: &mut Ctx, peer: &mut Peer) {
    if !pay_once(cx, peer, CLAIM_TUTORIAL, "tutorial_completed") {
        log::info!("{}: tutorial awards already paid", peer.addr);
    }
    // the client just finished its onboarding steps and showed the "tutorial complete" window: 9D bit 5 from now on,
    // so the window (and this request) does not come back at the next sign-in (docs/notes/onboarding.md)
    crate::rename::tutorial_done(cx, peer);
}

/// The tutorial match ended: `game_world_tutorial_completed` once (CHOICE: the data names it for the "game world" tutorial,
/// the 5B 02 match; no client string names it).
pub fn tutorial_match_over(cx: &mut Ctx, peer: &mut Peer) {
    pay_once(cx, peer, CLAIM_GAME_WORLD_TUTORIAL, "game_world_tutorial_completed");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reputation_only_and_gold_only_awards_push_what_changed() {
        let rep = Payout { reputation: vec![(1, 50)], ..Default::default() };
        assert_eq!(extra_pushes(&rep), (false, true), "8E for a reputation-only award");
        let gold = Payout { gold: 5, ..Default::default() };
        assert_eq!(extra_pushes(&gold), (true, false));
        let both = Payout { gold: 5, reputation: vec![(2, 10)], ..Default::default() };
        assert_eq!(extra_pushes(&both), (true, true));
        // experience / silver / parts / items go through reward_messages, which sends 8B, 8C and 8E itself
        let exp = Payout { exp: 10, reputation: vec![(2, 10)], ..Default::default() };
        assert_eq!(extra_pushes(&exp), (false, false));
    }

    #[test]
    fn tutorial_rows_resolve_through_the_enum() {
        game_data::require_tables!();
        let gd = game_data::testing::tables();
        let t = resolve(&gd, &gd.progression.tutorial_awards["tutorial_completed"]);
        assert_eq!((t.exp, t.silver, t.parts), (1000, 2500, 200), "type 0 xp, type 2 payment 0 silver, type 2 payment 4 spare parts");
        assert!(t.unpaid.is_empty());
        let g = resolve(&gd, &gd.progression.tutorial_awards["game_world_tutorial_completed"]);
        assert_eq!(g.items, vec![(1132, 1)], "type 3 = item");
        assert_eq!(g.unpaid.len(), 1, "type 4 (RATING in the client enum) with an item id is not decodable");
        let n = resolve(&gd, &gd.progression.tutorial_awards["survival_prepare_to_battle_notes_completed"]);
        assert_eq!(n.silver, 1000);
    }
}
