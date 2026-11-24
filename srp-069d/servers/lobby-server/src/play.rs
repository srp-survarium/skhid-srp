//! Play gate and local matches: 5B 01 (Training), later 5B 02 (tutorial), 53 event flush after the match.
//! The content comes from the [`LocalMatches`](crate::local::LocalMatches) trait object of the lobby.

use accounts::Currency;
use protocol::c2s::Event53;
use protocol::s2c::*;

use crate::build;
use crate::dispatch::Ctx;
use crate::local::{MatchCtx, MatchOutcome};
use crate::login;
use crate::peer::{LocalMatchState, Peer};
use crate::social;

/// Run the match builder of the trait with the account's current profile.
fn with_ctx<T>(cx: &Ctx, peer: &Peer, f: impl FnOnce(&MatchCtx) -> T) -> Option<T> {
    let id = peer.account?;
    let profile = cx.lobby.store.load_profile(id).ok()?;
    Some(f(&MatchCtx { account_id: id, account_name: &peer.name, profile: &profile, data: &cx.lobby.data }))
}

/// [`with_ctx`] for the end of a match: in the tutorial the profile the builder sees counts the first-game bonus as
/// already paid (the tutorial is not a game of the account; the copy is never saved).
fn with_ctx_for<T>(cx: &Ctx, peer: &Peer, tutorial: bool, f: impl FnOnce(&MatchCtx) -> T) -> Option<T> {
    let id = peer.account?;
    let mut profile = cx.lobby.store.load_profile(id).ok()?;
    if tutorial {
        profile.claim(accounts::CLAIM_FIRST_GAME);
    }
    Some(f(&MatchCtx { account_id: id, account_name: &peer.name, profile: &profile, data: &cx.lobby.data }))
}

/// A local match needs an account that is in no room, search or online match (busy.rs); else the Play button comes back.
/// A local match still recorded for the peer does not refuse a new one: the client may leave a match without the end
/// flush (CHOICE), the new start replaces the stale state.
fn free_to_play(cx: &Ctx, peer: &mut Peer) -> bool {
    let Some(id) = peer.account else { return false };
    if let Some(b) = crate::busy::reason(cx, id, false) {
        log::info!("{}: local Play refused: {b:?}", peer.addr);
        peer.push(&S2c::QueueState(QueueState { state: 0 }));
        return false;
    }
    true
}

/// A premium profile that ended falls back to profile 0 before a local match is built from it (profiles.rs).
fn local_profile_check(cx: &mut Ctx, peer: &mut Peer) {
    let Some(id) = peer.account else { return };
    if crate::profiles::ensure_playable(&cx.lobby.store, id) {
        if let Ok(p) = cx.lobby.store.load_profile(id) {
            peer.push(&S2c::Account(build::account_87(&cx.lobby.data, &p, &peer.name)));
        }
    }
}

/// 5B 01 `u32 map, u32 mode, u32 weather`: queue state 2 first (else the client's load-complete epilogue puts the
/// lobby back), then `83 5B 01` with the options of the trait.
pub fn training(cx: &mut Ctx, peer: &mut Peer, map: u32, mode: u32, weather: u32) {
    if with_ctx(cx, peer, |_| ()).is_none() {
        return;
    }
    if !free_to_play(cx, peer) {
        return;
    }
    local_profile_check(cx, peer);
    peer.push(&protocol::s2c::local_match_state());
    peer.local_match = Some(LocalMatchState { started: cx.now, map, mode, range: false });
    // no booster charge: a training match with bots pays nothing (game_data::rewards::LOCAL_MATCH_PAYS),
    // so its per-match boosters are not used up either; they stay selected for the next online match (which charges them)
    let Some(blob) = with_ctx(cx, peer, |c| cx.lobby.matches.training(c, map, mode, weather)) else { return };
    presence(cx, peer, social::ST_INGAME);
    log::info!("{}: 5B 01 map {map} mode {mode} weather {weather}: 85 02 and {} bytes of options", peer.addr, blob.len());
    peer.reply(Reply::Play(PlayBlob { sub: 1, blob }));
}

/// Map and mode the tutorial match is recorded with ([`load`]); a training match never uses map 0.
const TUTORIAL_MAP_MODE: (u32, u32) = (0, 1);

/// 5B 02: the first per connection is the lobby-load handshake, a later one starts the tutorial (match options of
/// `tutorial.options`, map 0 mode 1, LIKELY, not run).
pub fn load(cx: &mut Ctx, peer: &mut Peer) {
    if !peer.handshake_5b {
        login::handshake(cx, peer);
        return;
    }
    let Some(blob) = with_ctx(cx, peer, |c| cx.lobby.matches.tutorial(c)) else { return };
    if !free_to_play(cx, peer) {
        return;
    }
    peer.push(&protocol::s2c::local_match_state());
    peer.local_match = Some(LocalMatchState { started: cx.now, map: TUTORIAL_MAP_MODE.0, mode: TUTORIAL_MAP_MODE.1, range: false });
    presence(cx, peer, social::ST_INGAME);
    log::info!("{}: 5B 02 tutorial: 85 02 and {} bytes of options", peer.addr, blob.len());
    peer.reply(Reply::Play(PlayBlob { sub: 2, blob }));
}

/// 5B 03 `item record (the weapon to test), u8 profile index` (the "test the weapon" button of the inventory,
/// `goto_shooting_range`, exe 0x2CAD20): queue state 2 first (the loader of every local match checks it when the load
/// completes), then `83 5B 03` with the range options of the trait. The range is a local match
/// that pays nothing: no boosters are charged, no results are built (docs/notes/shooting-range.md).
pub fn shooting_range(cx: &mut Ctx, peer: &mut Peer, weapon: &protocol::items::ItemRecord, profile: u8) {
    let Some(blob) = with_ctx(cx, peer, |c| cx.lobby.matches.shooting_range(c, weapon, profile)) else { return };
    if !free_to_play(cx, peer) {
        return;
    }
    peer.push(&protocol::s2c::local_match_state());
    peer.local_match = Some(LocalMatchState { started: cx.now, map: TUTORIAL_MAP_MODE.0, mode: TUTORIAL_MAP_MODE.1, range: true });
    presence(cx, peer, social::ST_INGAME);
    log::info!("{}: 5B 03 shooting range, weapon dict {} uid {}: 85 02 and {} bytes of options", peer.addr, weapon.dict, weapon.uid, blob.len());
    peer.reply(Reply::Play(PlayBlob { sub: 3, blob }));
}

/// 53 event flush (`u32 n, n x {u16 id, u16, u32 value, u32 time}`): the first one more than `match_min_time` after a
/// local match started ends it. Events 5 / 0x3D = won, 6 / 0x3E = lost (enabled by B8), none = left early.
pub fn events(cx: &mut Ctx, peer: &mut Peer, ev: &[Event53]) {
    // a player of an online match: the flush is its win / loss report for the match (online.rs)
    if let Some(a) = peer.account {
        if crate::online::on_events(cx, a, ev) {
            return;
        }
    }
    let Some(m) = peer.local_match else { return };
    if cx.now.saturating_duration_since(m.started) <= cx.lobby.cfg.match_min_time {
        // an early flush (loading markers) does not end the match; logged so an early exit can be told apart
        let ids: Vec<u16> = ev.iter().map(|e| e.id).collect();
        log::info!("{}: 53 flush {:?} within the first {:?} of the local match: not its end", peer.addr, ids, cx.lobby.cfg.match_min_time);
        return;
    }
    peer.local_match = None;
    if m.range {
        // the shooting range has no result: the Play button and the friends' status come back, nothing is paid
        peer.push(&S2c::QueueState(QueueState { state: 0 }));
        presence(cx, peer, social::ST_LOBBY);
        log::info!("{}: shooting range left", peer.addr);
        return;
    }
    let crate::match_report::LocalReport { won, finished, ids } = crate::match_report::local_report(ev);
    let outcome = with_ctx_for(cx, peer, (m.map, m.mode) == TUTORIAL_MAP_MODE, |c| cx.lobby.matches.finished(c, m.map, m.mode, won, finished, &ids)).unwrap_or_default();
    // spare parts and chests of drop_after_combat (leavers get nothing; a premium account gets its factor)
    let premium = with_ctx(cx, peer, |c| c.profile.premium_left(crate::premium::unix_now()) > 0).unwrap_or(false);
    // no drops either: a local match pays no rewards (game_data::rewards::LOCAL_MATCH_PAYS)
    let drops = if game_data::rewards::LOCAL_MATCH_PAYS {
        let parts_pct = with_ctx(cx, peer, |c| crate::boosters::parts_percent(c.data, c.profile)).unwrap_or(0);
        cx.lobby.roll(|r| cx.lobby.data.drops.roll_with_parts_bonus(game_data::rewards::LOCAL_MATCH_LEVEL, won, finished, premium, parts_pct, r))
    } else {
        Default::default()
    };
    let reputation = outcome.reputation;
    finish_with(cx, peer, outcome, drops);
    if finished && (m.map, m.mode) == TUTORIAL_MAP_MODE {
        // the tutorial was played to its end: onboarding flag "tutorial complete" (9D bit 5)
        crate::rename::tutorial_done(cx, peer);
        // the tutorial is not a game of the account (it does not use up the first-game bonus, see `with_ctx_for`); its own
        // one-time awards are paid here
        crate::awards::tutorial_match_over(cx, peer);
    }
    if let Some(id) = peer.account {
        // what the local path really knows; kills, score and the rest stay None until a match observer exists
        let faction = with_ctx(cx, peer, |c| c.profile.faction).unwrap_or(1);
        let mut facts = game_data::MatchFacts::local(m.map, m.mode, won, finished, faction);
        facts.local_bots = true;
        facts.reputation = Some(reputation);
        crate::stats::record_match(cx, id, &facts);
    }
    log::info!("{}: local match over (events {ids:?}, won {won})", peer.addr);
}

/// 97 results, the rewards (8B money, 8C level, 8E reputation) and `85 00` so the Play button comes back. The results
/// screen is display only: the account changes only through the pushes.
pub fn finish(cx: &mut Ctx, peer: &mut Peer, o: MatchOutcome) {
    finish_with(cx, peer, o, game_data::rewards::MatchDrops::default());
}

/// [`finish`] plus the item drops: spare parts go to the wallet, chests into the inventory; one `83 46 00` with flag 1
/// shows the received-item popup for each (the client does not add the spare parts row to its inventory, PROVEN
/// 0x2001937D2..0x19392D: dict 925 is one of the currency-like ids), then 8B.
fn finish_with(cx: &mut Ctx, peer: &mut Peer, o: MatchOutcome, drops: game_data::rewards::MatchDrops) {
    if let Some(id) = peer.account {
        for m in reward_messages(cx, id, o, drops) {
            peer.push(&m);
        }
    }
    peer.push(&S2c::QueueState(QueueState { state: 0 }));
    presence(cx, peer, social::ST_LOBBY);
}

/// The 97 results, the rewards and the pushes that show them, in the order the client gets them. With a results screen
/// (`results97`): `83 45 0A` (the drops for the victory screen's extra rewards panel, [`Inventory45::MatchDrops`]; it
/// must be there when the 97 builds the screen: LIKELY order), `97`, `8B`, `8C`, `8E`. Without one (awards): `83 46 00`
/// (the received-item popup), `8B`, `8C`, `8E`. Both drop messages add the records to the client's inventory, so only
/// one of them is sent. The profile is changed here (a peer is not needed: an online match can end while the player is
/// away).
///
/// A dropped chest is opened at once ([`open_dropped_chests`]): the drop message lists the chest (the victory panel's
/// "?" item) and the client adds it, then one `83 46 06` per unit takes it out of the client's inventory again, adds the
/// contents and shows the client's own "chest opened" notification (0x200248F50, type 25 `CHEST_OPENED`). The 0.69d lobby
/// lists no stack and no category 25..31 item in any inventory or shop tab (PROVEN in the lobby movie), so a stored chest
/// could never be opened (docs/notes/items.md section 3).
pub fn reward_messages(cx: &Ctx, account: accounts::AccountId, o: MatchOutcome, drops: game_data::rewards::MatchDrops) -> Vec<S2c> {
    let mut out = vec![];
    let screen = o.results97.is_some();
    let mut tail = vec![];
    let has_drops = drops.parts > 0 || !drops.items.is_empty() || !drops.once.is_empty() || drops.event.iter().any(|&n| n > 0);
    if o.exp > 0 || o.silver > 0 || o.reputation > 0 || has_drops {
        let gd = &cx.lobby.data;
        // the contents of every dropped chest unit, rolled before the transaction (the roll of a one-time item whose claim
        // is old stays unused)
        let mut rolls = roll_dropped_chests(cx, &drops);
        let mut opened: Vec<OpenedChest> = vec![];
        let popups = crate::dispatch::update_account(cx, account, |p| {
            if o.first_game {
                p.claim(accounts::CLAIM_FIRST_GAME);
            }
            crate::progress::add_xp(gd, p, o.exp);
            p.credit(Currency::Silver, o.silver as u64);
            p.credit(Currency::Parts, drops.parts as u64);
            for (slot, &n) in drops.event.iter().enumerate() {
                p.credit(crate::events::currency(slot), u64::from(n));
            }
            let f = p.faction;
            p.add_points(f, o.reputation);
            // (record, bonus part) of every drop
            let mut records = vec![];
            if drops.parts > 0 {
                let rec = protocol::items::ItemRecord { dict: game_data::rewards::DETAILS_DICT, value: drops.parts, ..Default::default() };
                records.push((rec, drops.parts_bonus));
            }
            // event currency: records for the received-item popup only (the client skips 951 / 1138 / 1139 when it adds
            // records; the wallet is 8B)
            for (slot, &n) in drops.event.iter().enumerate().filter(|(_, n)| **n > 0) {
                let rec = protocol::items::ItemRecord { dict: game_data::events::CURRENCIES[slot], value: n, ..Default::default() };
                records.push((rec, drops.event_bonus[slot]));
            }
            // one-time items: only when their claim is new (recorded now, in this transaction)
            let mut items = drops.items.clone();
            for (claim, dict, n) in &drops.once {
                if p.claim(claim) {
                    items.push((*dict, *n));
                }
            }
            for &(dict, n) in &items {
                let existing = if gd.items.is_stackable(dict) { p.items.iter().find(|i| i.dict_id == dict).map(|i| i.uid) } else { None };
                let uid = match existing {
                    Some(uid) => {
                        p.item_mut(uid).unwrap().count += n;
                        uid
                    }
                    None => p.add_item(dict, if gd.items.is_stackable(dict) { n } else { gd.items.amount_field(dict, 100) }),
                };
                // a dropped module (none in 0.69d match drops) is green, never grey; its record carries the pairs
                crate::chests::give_rarity(gd, p, uid, &mut crate::fresh_roll());
                let row = *p.item(uid).unwrap();
                records.push((build::record_value(gd, p, &row, if gd.items.is_stackable(dict) { n } else { row.count }), 0));
                opened.extend(open_dropped_chests(gd, p, uid, dict, n, &mut rolls));
            }
            records
        })
        .unwrap_or_default();
        if !popups.is_empty() {
            if screen {
                // amount = the record's +0xC (the count of a stack; the own value of another item, which the screen
                // ignores), so the inventory add keeps the right value
                let items = popups.into_iter().map(|(item, bonus)| MatchDrop { amount: item.value, bonus: bonus.min(item.value), item }).collect();
                out.push(S2c::Reply(Reply::Inventory(Inventory45::MatchDrops { flag: 0, items })));
            } else {
                tail.push(S2c::Reply(Reply::Shop(Shop46::Buy { flag: 1, items: popups.into_iter().map(|(r, _)| r).collect() })));
            }
        }
        // after the drop message that listed the chests (the client adds them first, then takes them out again), before
        // 8B / 8C / 8E (they carry what the contents added)
        for c in opened {
            log::info!("account {account}: dropped chest {} (dict {}) opened: {:?}", c.item, c.chest_dict, c.rewards);
            tail.push(S2c::Reply(Reply::Shop(Shop46::Chest { item: c.item, chest_dict: c.chest_dict, rewards: c.records })));
        }
        if let Ok(p) = cx.lobby.store.load_profile(account) {
            tail.push(S2c::Money(build::money_8b(&p)));
            tail.push(S2c::Level(build::level_8c(&cx.lobby.data, &p)));
            tail.push(S2c::Factions(build::factions_8e(&p)));
        }
    }
    if let Some(payload) = o.results97 {
        out.push(S2c::Raw { id: 0x97, payload });
    }
    out.extend(tail);
    out
}

/// One dropped chest unit the server opened: what its `83 46 06` carries.
#[derive(Debug)]
struct OpenedChest {
    item: u32,
    chest_dict: u16,
    rewards: Vec<game_data::chests::Reward>,
    records: Vec<protocol::items::ItemRecord>,
}

/// One rolled chest unit: chest dict, its rewards, the modifier lists of the modules among them.
type DroppedRoll = (u16, Vec<game_data::chests::Reward>, Vec<Vec<(u16, u16)>>);

/// The rolled contents of every unit of the dropped chests, `(chest dict, rewards, module modifiers)` in drop order. A chest the data cannot
/// open (no table, unresolved groups) gets no roll and stays in the inventory as before (logged).
fn roll_dropped_chests(cx: &Ctx, drops: &game_data::rewards::MatchDrops) -> Vec<DroppedRoll> {
    let gd = &cx.lobby.data;
    let units = drops.items.iter().copied().chain(drops.once.iter().map(|(_, d, n)| (*d, *n)));
    let mut out = vec![];
    for (dict, n) in units {
        if !gd.items.is_stackable(dict) {
            continue;
        }
        for _ in 0..n {
            match cx.lobby.roll(|r| gd.chests.open(dict, r)) {
                Ok(v) => {
                    // the modules inside get their rarity and drop modifiers now too (docs/notes/workshop.md 7)
                    let mods = cx.lobby.roll(|r| crate::chests::roll_module_mods(gd, dict, &v, r));
                    out.push((dict, v, mods))
                }
                Err(game_data::chests::OpenError::NotAChest) => break,
                Err(e) => {
                    log::warn!("dropped chest dict {dict} stays closed: {e}");
                    break;
                }
            }
        }
    }
    out
}

/// Open `n` units of the chest stack `uid` (dict `dict`) a drop has just added: per unit one taken from the stack and the
/// next roll of that dict granted, the way `46 06` does ([`crate::chests::grant`]: money to the wallet, items into the
/// inventory). The rolls used are taken out of `rolls`; units without a roll stay in the stack.
fn open_dropped_chests(
    gd: &game_data::GameData,
    p: &mut accounts::Profile,
    uid: u32,
    dict: u16,
    n: u32,
    rolls: &mut Vec<DroppedRoll>,
) -> Vec<OpenedChest> {
    let mut out = vec![];
    if !gd.items.is_stackable(dict) {
        return out;
    }
    for _ in 0..n {
        let Some(at) = rolls.iter().position(|(d, _, _)| *d == dict) else { break };
        if p.consume(uid, 1).is_none() {
            break;
        }
        let (_, rewards, mods) = rolls.remove(at);
        let (records, _) = crate::chests::grant_with_mods(gd, p, &rewards, mods);
        out.push(OpenedChest { item: uid, chest_dict: dict, rewards, records });
    }
    out
}

/// Tell the friends: in a match / back in the lobby (B0 07).
fn presence(cx: &mut Ctx, peer: &Peer, status: u8) {
    if let Some(id) = peer.account {
        social::announce(cx, id, status);
    }
}