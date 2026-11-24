//! Request dispatch: one decoded client message -> the lobby's answers.

use std::net::SocketAddr;
use std::time::Instant;

use accounts::Profile;
use protocol::c2s::{Account48, Inventory45 as Req45, Play5B, Quest4A, Request, Roulette5A, Shop46 as Req46, Workshop60};
use protocol::s2c::*;

use crate::equip;
use crate::login;
use crate::peer::{Cross, Peer, Registry};
use crate::play;
use crate::shop;
use crate::social;
use crate::workshop;
use crate::Lobby;

/// What a handler may use besides its own peer.
pub struct Ctx<'a> {
    pub lobby: &'a Lobby,
    pub registry: &'a mut Registry,
    pub now: Instant,
    /// Messages for other accounts' peers (delivered by the server after the handler returned).
    pub fx: Vec<Cross>,
    /// Peers that must be dropped (a new sign-in of the same account replaced them).
    pub kick: Vec<SocketAddr>,
}

/// Atomic change of the peer's profile; a store failure is logged and gives None.
pub fn update<T>(cx: &Ctx, peer: &Peer, f: impl FnOnce(&mut Profile) -> T) -> Option<T> {
    update_account(cx, peer.account?, f)
}

/// [`update`] for an account that may have no peer at hand (an online match ends while the player is away).
pub fn update_account<T>(cx: &Ctx, id: accounts::AccountId, f: impl FnOnce(&mut Profile) -> T) -> Option<T> {
    match cx.lobby.store.update_profile(id, |p| Ok::<T, ()>(f(p))) {
        Ok(v) => Some(v),
        Err(e) => {
            log::error!("profile update of account {id} failed: {e:?}");
            None
        }
    }
}

/// Fixed answers (the real client accepts them): the data after `83 x <request id>`.
pub fn fixed_reply(mid: u8, pl: &[u8]) -> Option<Vec<u8>> {
    let sub = *pl.first()?;
    Some(match (mid, sub) {
        // (48 04 nickname validation is answered for real: rename::validate)
        // flag-clearing replies with an unknown sub
        (0x62, 0x23) => vec![0xFF],
        // equip: plain ack (sub 0 is ignored by the client but clears the 45 00 lock)
        (0x45, 0) => vec![0],
        // 60 00: the workshop opened (ignored by the client, clears the wait)
        (0x60, 0) => vec![0],
        _ => return None,
    })
}

/// Handle one message of a peer. `x` is not needed: the transport already dropped duplicates.
pub fn handle(cx: &mut Ctx, peer: &mut Peer, mid: u8, payload: &[u8]) {
    if peer.phase == crate::peer::Phase::Refused {
        return;
    }
    if mid == 0x2E {
        login::sign_in(cx, peer, payload);
        return;
    }
    if peer.account.is_none() {
        log::debug!("{}: request {mid:#04x} before sign-in ignored", peer.addr);
        return;
    }
    if std::mem::take(&mut peer.rejoin_check) {
        if let Some(a) = peer.account {
            crate::online::offer_rejoin_after_login(cx, a);
        }
    }
    crate::quests::tick(cx, peer);
    crate::hero::tick(cx);
    if mid == 0x55 && payload.first() == Some(&4) {
        // achievements window opened: 83 55 02 progress + 83 55 03 category scores (achievements.rs)
        crate::achievements::window(cx, peer);
        return;
    }
    if let Some(data) = fixed_reply(mid, payload) {
        // 5B 03 and the others are plain answers; 45 00 and 60 00 continue below
        peer.reply_raw(mid, &data);
    }
    let req = match Request::decode(mid, payload) {
        Ok(r) => r,
        Err(e) => {
            log::debug!("{}: request {mid:#04x} {} not decoded: {e}", peer.addr, hex(payload));
            // squad actions (49 xx) are answered with the current (empty) squad whatever their layout
            if mid == 0x49 {
                peer.push(&S2c::Squad(SquadUpdate { squad: None }));
            }
            return;
        }
    };
    match &req {
        Request::Inventory(Req45::Equip { profile, entries, .. }) => equip::equip(cx, peer, *profile, entries),
        // boosters (boosters.rs)
        Request::Inventory(Req45::BoosterEnable { dict }) => crate::boosters::enable(cx, peer, *dict),
        Request::Inventory(Req45::BoosterDisable { dict }) => crate::boosters::disable(cx, peer, *dict),
        Request::Inventory(Req45::BoosterReplace { old, new }) => crate::boosters::replace(cx, peer, *old, *new),
        Request::Account(Account48::BoosterAutobuy { dict, on }) => crate::boosters::autobuy(cx, peer, *dict, *on != 0),
        // rename voucher (rename.rs)
        Request::Account(Account48::NicknameChange { name, a, b }) => crate::rename::change(cx, peer, name, *a, *b),
        // onboarding (docs/notes/onboarding.md): nickname validation and the skipped tutorial
        Request::Account(Account48::NicknameCheck { name, .. }) => crate::rename::validate(cx, peer, name),
        Request::Quest(Quest4A::SkipTutorial) => crate::rename::tutorial_done(cx, peer),
        // premium vouchers (premium.rs); other item uses belong to the consumables
        Request::Inventory(Req45::Use { item }) if crate::premium::use_voucher(cx, peer, *item) => {}
        Request::Inventory(Req45::Use { item }) => crate::items::use_item(cx, peer, *item),
        Request::Inventory(Req45::RelocateUpgrade { a, b, c, d }) => crate::decals::relocate(cx, peer, *a, *b, *c, *d),
        Request::Inventory(Req45::DropUpgrade { a, b, c }) => crate::decals::drop(cx, peer, *a, *b, *c),
        Request::Inventory(Req45::Decal { a, b, c, d }) => crate::decals::save(cx, peer, *a, *b, *c, *d),
        Request::Inventory(Req45::Skin { item, skin }) => crate::skins::change(cx, peer, *item, *skin),
        Request::Inventory(Req45::SkinPreview { item, skin }) => crate::skins::preview(cx, peer, *item, *skin),
        Request::Inventory(r) => {
            workshop::inventory(cx, peer, r);
        }
        Request::Shop(Req46::OpenChest { item }) => crate::chests::open(cx, peer, *item),
        Request::Shop(Req46::ItemSearch { a }) => crate::hunt::search_request(cx, peer, *a),
        Request::Roulette(Roulette5A::Sub2 { .. }) => crate::hunt::opened(cx, peer),
        Request::Roulette(Roulette5A::Sub3 { a, b }) => crate::hunt::level_request(cx, peer, *a, *b),
        // equipment profiles (profiles.rs) and weapon skins (skins.rs)
        Request::Account(Account48::ProfileChanged { profile }) => crate::profiles::changed(cx, peer, *profile),
        Request::Account(Account48::ProfileRename { profile, name }) => crate::profiles::rename(cx, peer, *profile, name),
        Request::Account(Account48::EnableProfile { profile, enable }) => crate::profiles::enable(cx, peer, *profile, *enable != 0),
        Request::Account(Account48::BuyPreset { a }) => crate::profiles::buy(cx, peer, *a),
        Request::VictoryData { player, b, .. } => crate::details::player(cx, peer, *player, *b),
        Request::Presets(p) => crate::presets::handle(cx, peer, p),
        // league window (league.rs)
        Request::League(l) => crate::league::handle(cx, peer, l),
        Request::Shop(Req46::Buy { dict, count, flag, .. }) => shop::buy(cx, peer, *dict, *count, *flag),
        Request::Shop(Req46::Sell { item, amount }) => shop::sell(cx, peer, *item, *amount),
        Request::Shop(Req46::Exchange { amount }) => shop::exchange(cx, peer, *amount),
        Request::Workshop(Workshop60::Opened { dict }) => workshop::open(cx, peer, *dict),
        Request::Workshop(Workshop60::Save { weapon, actions, .. }) => workshop::modules(cx, peer, *weapon, actions),
        Request::Quest(Quest4A::Sub01 { a: 1 }) => crate::awards::tutorial_completed(cx, peer),
        Request::Quest(Quest4A::SetFaction { faction }) => set_faction(cx, peer, *faction),
        Request::Quest(Quest4A::DailyReroll { quest }) => crate::quests::reroll_request(cx, peer, *quest),
        Request::Quest(Quest4A::ChestOpened { .. }) => crate::quests::chest_request(cx, peer),
        Request::Quest(Quest4A::FactionBattleRewards { a }) => crate::faction_battle::claim_request(cx, peer, *a),
        Request::Generic { id: 0x35, .. } => crate::faction_battle::push_status(cx, peer),
        // 39 ask_new_contract_info: the contract lives in the 94 record of type 0x16; 3A: the 96 faction mask
        Request::Generic { id: 0x39, .. } => crate::login::refresh_notices(cx, peer),
        Request::Generic { id: 0x3A, .. } => peer.push(&crate::login::faction_mask_push()),
        // 34 follows the client's own ack (51) of a level-up (B1 type 0xA) or friend-level (type 9) message, sent by
        // 0x1C3700 / 0x1C3B05; no S->C handler waits for an answer (docs/notes/notices.md)
        Request::Generic { id: 0x34, .. } => log::debug!("{}: 34 00 after an important message: no reply", peer.addr),
        Request::DailyReward { a } => crate::login_calendar::collect_request(cx, peer, *a),
        Request::Skills { skills } => crate::skills::apply(cx, peer, skills),
        Request::SkillReset { branch, payment } => crate::skills::reset(cx, peer, *branch, *payment),
        // 31 of a player whose seat is kept for a rejoin: he declines it (online.rs)
        Request::QueueCancel { .. } if peer.account.is_some_and(|a| crate::online::decline_rejoin(cx, a)) => {}
        // 31 while a PvE search runs (pve.rs); 5F PvE requests
        Request::QueueCancel { .. } if crate::pve::cancel_if_queued(cx, peer) => {}
        Request::Pve(p) => crate::pve::handle(cx, peer, p),
        Request::Queue { .. } | Request::QueueCancel { .. } if crate::squads::queue(cx, peer, &req) => {}
        Request::People { id, kind: kind @ (2 | 3 | 0x17 | 0x18), text } => crate::squads::people(cx, peer, *id, *kind, text),
        Request::MemberProfile { member, .. } => crate::squads::member_profile(cx, peer, *member),
        Request::Queue { arg, mode, .. } => crate::queue::press(cx, peer, *arg, *mode),
        Request::QueueCancel { .. } => crate::queue::cancel(cx, peer),
        Request::Squad(sq) => crate::squads::squad_request(cx, peer, sq),
        Request::Play(p) => match p {
            Play5B::Play { map, mode, weather } => play::training(cx, peer, *map, *mode, *weather),
            Play5B::Load => play::load(cx, peer),
            Play5B::ShootingRange { weapon, tail } => play::shooting_range(cx, peer, weapon, *tail),
        },
        Request::Events(ev) => play::events(cx, peer, ev),
        // hero of the day (hero.rs) and the store's action tabs, exchange and specifics (actions.rs)
        // game-server list: selection (57) and ping results (56) of the 9E list (servers.rs)
        Request::ServerList { a, b } => crate::servers::selected(cx, peer, *a, *b),
        Request::Retry56 { pings } => crate::servers::pinged(peer, pings),
        // complaint / compliment about a player (comments.rs)
        Request::Account(Account48::Comment { player, kind, a, b }) => crate::comments::file(cx, peer, *player, *kind, *a, *b),
        Request::Account(Account48::HeroOfTheDay { a, b }) => crate::hero::window(cx, peer, *a, *b),
        Request::Account(Account48::DayHeroData { a, b }) => crate::hero::friends(cx, peer, *a, *b),
        Request::Account(Account48::ActionItemExchanged { a, b }) => crate::actions::exchange(cx, peer, *a, *b),
        Request::ActionsData { a: 0, b, c, .. } => {
            if let Some(id) = peer.account {
                cx.registry.set_language(id, *c);
            }
            crate::actions::get_data(cx, peer, *b, *c)
        }
        // 32 after the 80 (builder 0x200185BBD..0x200185C6F): `u8 language` (0x2000915E0, the index 5C 00 carries too),
        // `u8 flag`. No answer; the language names a bot in the 97 result window (bots' names are shown raw)
        Request::Info32 { a, b } => {
            if let Some(id) = peer.account {
                log::info!("{}: 32 language {a} (flag {b})", peer.addr);
                cx.registry.set_language(id, *a);
            }
        }
        Request::ActionBuy { id, mode } => crate::actions::buy(cx, peer, *id, *mode),
        Request::Shop(Req46::FavoriteDiscount { a, b }) => crate::actions::favorite_clicked(peer, *a, *b),
        Request::Shop(Req46::BuySpecific { dict }) => crate::actions::buy_specific(cx, peer, *dict),
        // 46 02: a premium-window offer (premium.rs)
        Request::Shop(Req46::BuyOffer { offer, a, .. }) => crate::premium::buy_offer(cx, peer, *offer, *a),
        Request::Account(a) if crate::clans::handle(cx, peer, a) => {}
        Request::People { id, kind: 7, .. } => crate::clans::invite_relay(cx, peer, *id),
        Request::Other { id: 0x62, payload } => crate::rooms::handle(cx, peer, payload),
        Request::Chat { .. } | Request::People { .. } | Request::Friends(_) | Request::ReadMessage { .. } => {
            social::handle(cx, peer, &req)
        }
        _ => {}
    }
}

/// 4A 00 `u8 faction` (1..4): the active faction, answered with 8E.
fn set_faction(cx: &mut Ctx, peer: &mut Peer, f: u8) {
    let gd = cx.lobby.data.clone();
    let ok = crate::dispatch::update(cx, peer, |p| {
        // the first choice (nothing earned yet) also decides the faction armour of the kit
        crate::progress::first_faction_choice(&gd, p, f);
        let ok = p.set_faction(f).is_ok();
        if ok {
            // the choose-faction step of the onboarding is done (9D bit 6)
            p.onboarding |= accounts::Profile::FACTION_CHOSEN;
        }
        ok
    });
    if ok != Some(true) {
        log::info!("{}: 4A 00 faction {f} refused", peer.addr);
        return;
    }
    log::info!("{}: 4A 00 active faction {f}: 8E", peer.addr);
    if let Some(id) = peer.account {
        if let Ok(p) = cx.lobby.store.load_profile(id) {
            peer.push(&S2c::Factions(crate::build::factions_8e(&p)));
        }
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
