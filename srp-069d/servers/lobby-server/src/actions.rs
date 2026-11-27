//! The store's action tabs (95 tabs, `5C` -> `BC`), their gold offers, the event currency exchange (`48 28`), the
//! favourite-weapon discount click (`46 08`) and the pocket "specifics" opened with gold (`46 09`, push 91).
//! Layouts, what is proven and what stays open: `docs/notes/actions.md`.
//!
//! No game file has the offers (prices, bundles, durations: they were real-server data, docs/notes/store.md section 3).
//! The tabs and offers are a server setting ([`LobbyConfig::store_tabs`], [`LobbyConfig::store_offers`]) whose default is
//! one tab with two cards (CHOICE): 7 days of premium (300 gold) and the contract (100 gold, contract.rs).
//! The client does NOT localize card texts or tab names: they are client keys whose values in the player's language
//! (the `5C 00` language byte) come from the client's own files (`game_data::texts`). With no tab, 95 carries no tab
//! and the store window shows its own built-in tabs; a request for a tab is always answered (a tab with no offer gets an empty `shop_items`) so the window never waits. A `5C buy` pays the gold
//! price, applies the offer ([`OfferGrant`]) and answers `BC sub 1`.

use game_data::chests::Reward;
use game_data::texts::{ClientTexts, ENGLISH};
use protocol::binconfig::{self, Value};
use protocol::s2c::hero::StoreActions;
use protocol::s2c::*;

use crate::dispatch::{update, Ctx};
use crate::peer::Peer;
use crate::premium::unix_now;
use crate::shop::send_money;

/// A tab of the store window (an entry of the 95 tab list: `root.set_shop_action_tabs {id, name, default_gold}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreTab {
    pub id: u32,
    /// shown as the tab's title: `StoreScreen.setTabData` shows `String(name).toUpperCase()` (plain text, not localized):
    /// a client key of `game_data::texts` is sent as its English text (95 comes at login; no language is known then),
    /// any other string as it is
    pub name: String,
    /// the tab opens on the gold currency
    pub default_gold: bool,
}

/// What buying an offer gives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OfferGrant {
    /// the bundle's items (money items 214 silver, 215 gold, 925 spare parts are credited)
    Items,
    /// premium account time, like the vouchers (premium.rs)
    Premium { hours: u32 },
    /// the Winner's Contract (contract.rs): one at a time
    WinnersContract,
}

/// A gold offer of a tab: an actions-tab card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreOffer {
    /// the `data id` of `5C buy` and `BC sub 1`
    pub id: u32,
    pub tab: u32,
    /// the card title (upper-cased by the client) and the full view's header: a client key of `game_data::texts` (sent
    /// in the player's language) or an operator's own text (sent as it is)
    pub label: String,
    /// the full view's text (same rule)
    pub description: String,
    /// price in gold (`cost.gold.price`)
    pub gold: u32,
    /// the bundle: (dictionary id, amount), shown in the full view's "Contains" block after the grant's own entry
    /// ([`shown_items`]); granted for [`OfferGrant::Items`]
    pub items: Vec<(u16, u32)>,
    /// seconds the offer is shown as running (the client's `ttl`); None = no timer (`ttl` 0)
    pub ttl_secs: Option<u32>,
    /// the card picture per slot size: `s_img` (small 600 x 210 slots: items 1..4 and later pages), `m_img` (item 5,
    /// 600 x 450), `l_img` (item 0, 580 x 687): file names in the client's `flash_movies/textures/ui_actions/`
    pub images: [String; 3],
    pub grant: OfferGrant,
}

/// `data_id` / chest item the client expects when no chest is part of the offer (0x200196985: -1 = none).
const NO_CHEST: u32 = u32::MAX;

/// The tab of the default offers.
pub const DEFAULT_TAB: u32 = 1;
/// Card ids of the default offers (CHOICE; only sent back in `5C 01`).
pub const PREMIUM_7_DAYS_CARD: u32 = 1;
pub const CONTRACT_CARD: u32 = 2;
/// `currency_type` / `currency_sign` of the blob: the NAME of the real-money price table inside every item's `cost` and its
/// sign (strings, PROVEN types; the values are CHOICE: no real-money sale exists, every real-money price is 0, which hides
/// the real-money button).
pub const CURRENCY_TYPE: &str = "usd";
pub const CURRENCY_SIGN: &str = "$";

/// The default tab (CHOICE: the client shows the server tabs INSTEAD of its built-in five; one tab for the two cards).
pub fn default_tabs() -> Vec<StoreTab> {
    vec![StoreTab { id: DEFAULT_TAB, name: "st_special_offer".into(), default_gold: true }]
}

/// The default cards (CHOICE). ORDER matters: item 0 is drawn in the big slot (`l_img`), items 1..4 in
/// the small ones (`s_img`). Item 0 = 7 days of premium for 300 gold (the window's week price, 0.6x web shop) with the
/// client's golden crown `673_*_1563455165.png` (every size exists); item 1 = the contract for 100 gold with
/// `image_10.png` (the client's only contract art, 774 x 238: it fits only a small slot). Texts: the client's own keys:
/// "Premium account for 7 days" + the premium advert; the contract under the client's own name of
/// its window, "Contract with Huckster", + "Win 1 match" (no key states the 20 000 silver).
pub fn default_offers() -> Vec<StoreOffer> {
    let crown = |s: &str| format!("673_{s}_1563455165.png");
    vec![
        StoreOffer {
            id: PREMIUM_7_DAYS_CARD,
            tab: DEFAULT_TAB,
            label: "st_premium_access_7d_voucher_league_desc".into(),
            description: "st_gold_advert_1_hint".into(),
            gold: 300,
            items: vec![],
            ttl_secs: None,
            images: [crown("s"), crown("m"), crown("l")],
            grant: OfferGrant::Premium { hours: 7 * 24 },
        },
        StoreOffer {
            id: CONTRACT_CARD,
            tab: DEFAULT_TAB,
            label: "st_contract_with_profiteer".into(),
            description: "st_contract_complete_after_win_descr".into(),
            gold: crate::contract::PRICE_GOLD,
            items: vec![],
            ttl_secs: None,
            images: ["image_10.png".into(), "image_10.png".into(), "image_10.png".into()],
            grant: OfferGrant::WinnersContract,
        },
    ]
}

/// 95: the premium-window elements (premium.rs) and the store tabs. With neither it is the smallest 95 the client accepts.
pub fn shop_push(cx: &Ctx) -> S2c {
    let tabs: Vec<ShopTab> =
        cx.lobby.cfg.store_tabs.iter().take(255).map(|t| ShopTab { id: t.id, name: cx.lobby.data.texts.text(&t.name, ENGLISH), flag: u8::from(t.default_gold) }).collect();
    let elems: Vec<ShopElem> = cx.lobby.cfg.premium_offers.iter().map(crate::premium::shop_elem).collect();
    if tabs.is_empty() && elems.is_empty() {
        return S2c::Raw { id: 0x95, payload: vec![0; 5] };
    }
    S2c::Shop(Shop95 { elems, tabs })
}

fn int(v: u32) -> Value {
    Value::Int(v.min(i32::MAX as u32) as i32)
}

/// `{price, discount_price, discount_value}` of one currency (ints): no discount (`discount_value` 0; the card shows the
/// discount only when `discount_value > 0` and `ttl_discount > -1`, ActionStoreRenderer, PROVEN).
fn price(p: u32) -> Value {
    Value::Table(vec![("price".to_string(), int(p)), ("discount_price".to_string(), int(p)), ("discount_value".to_string(), Value::Int(0))])
}

/// `dictId` 0 in `bundle.items` = premium time, `amount` in HOURS (PROVEN, lobby movie): `ItemFactory._rewardsId` =
/// `[214, 215, 925, 0]` with `_rewardsIdData[3] = {reward: Reward.PREMIUM}`; `ItemFactory.filterRewards` turns that entry
/// into `{type PREMIUM, amount Util.getTimeLeftStringHours(amount)}` ("7" + `days_short` for 168) and the full view's
/// "Contains" renderer (`blockInsideSilverAll` = `RewardRendererBig`) draws it with the `premium` icon in gold.
pub const PREMIUM_TIME_DICT: u16 = 0;

/// What the full view's "Contains" block (`st_inside`) lists: the grant's own value first, then the operator's items.
/// The client builds that block only from `bundle.items` (builder 0x20028E8A0, 0x28F1D4..0x28F3E1; dicts 214 / 215 /
/// 925 / 0 are drawn as rewards, others as item tiles), so an empty list leaves the block empty. Premium time =
/// [`PREMIUM_TIME_DICT`] x hours; the Winner's Contract = its daily reward, 20 000 silver (the contract config's `amount` / `dictId`, contract.rs). Only
/// [`OfferGrant::Items`] grants these entries; for the other grants they are what the grant gives.
pub fn shown_items(o: &StoreOffer) -> Vec<(u16, u32)> {
    let mut v = match o.grant {
        OfferGrant::Items => vec![],
        OfferGrant::Premium { hours } => vec![(PREMIUM_TIME_DICT, hours)],
        OfferGrant::WinnersContract => vec![(crate::contract::SILVER_DICT as u16, crate::contract::DAILY_SILVER)],
    };
    v.extend(o.items.iter().copied());
    v
}

/// Whether the offer is shown to a player: the Winner's Contract card is left out while the player's contract runs (one
/// contract at a time). The client cannot show a card as bought or disabled (PROVEN: builder 0x20028E8A0 never copies
/// `complete`, a refused `BC sub 1` does nothing), so the card goes; it is back in the first tab request after the
/// contract ends (the client keeps a tab 300 s, 0x2001967B5, f32 at 0x2009E60A4).
pub fn shown_to(o: &StoreOffer, p: Option<&accounts::Profile>, now: i64) -> bool {
    match (&o.grant, p) {
        (OfferGrant::WinnersContract, Some(p)) => !crate::contract::running(&p.contract, now),
        _ => true,
    }
}

/// The offers the player at `account` is shown (all of them when the profile cannot be read).
fn offers_for(cx: &Ctx, account: Option<accounts::AccountId>, now: i64) -> Vec<StoreOffer> {
    let p = account.and_then(|a| cx.lobby.store.load_profile(a).map_err(|e| log::error!("loading profile of {a}: {e}")).ok());
    cx.lobby.cfg.store_offers.iter().filter(|o| shown_to(o, p.as_ref(), now)).cloned().collect()
}

/// One card of the blob. Every key the builder 0x20028E8A0 reads with `operator[]` is present (a missing one hits the
/// engine's fatal assertion, PROVEN); only `bundle.items`, `bundle.bonuses` and
/// `chest.items` are optional (`value_exists`). The full view has NO picture (its `addElement` has no `Background`
/// case and it covers only the left panel, x 0..~600: the lobby room shows on the right);
/// what it draws besides the texts is the "Contains" block of [`shown_items`].
fn card(o: &StoreOffer, texts: &ClientTexts, language: u8) -> Value {
    let mut bundle = vec![];
    let shown = shown_items(o);
    if !shown.is_empty() {
        let items = shown.iter().map(|(d, n)| Value::Table(vec![("dictId".to_string(), Value::Int(i32::from(*d))), ("amount".to_string(), int(*n))])).collect();
        bundle.push(("items".to_string(), Value::List(items)));
    }
    Value::Table(vec![
        ("id".to_string(), int(o.id)),
        // 0 = items / bonus card (full view PRESET_EQUIPMENT: header, text, the "inside" list, the gold button)
        ("type".to_string(), Value::Int(0)),
        // seconds the card is shown; 0 = no timer
        ("ttl".to_string(), int(o.ttl_secs.unwrap_or(0))),
        // no discount timer (only read together with discount_value > 0)
        ("ttl_discount".to_string(), Value::Int(0)),
        ("label".to_string(), Value::Str(texts.text(&o.label, language))),
        ("description".to_string(), Value::Str(texts.text(&o.description, language))),
        ("s_img".to_string(), Value::Str(o.images[0].clone())),
        ("m_img".to_string(), Value::Str(o.images[1].clone())),
        ("l_img".to_string(), Value::Str(o.images[2].clone())),
        // gold price, and the real-money table named by currency_type with price 0 (hides the real-money button)
        ("cost".to_string(), Value::Table(vec![("gold".to_string(), price(o.gold)), (CURRENCY_TYPE.to_string(), price(0))])),
        ("bundle".to_string(), Value::Table(bundle)),
    ])
}

/// The blob of one tab: a binary config whose root holds `currency_type` (a STRING: the name of the real-money table in
/// every item's `cost`), `currency_sign` and `shop_items` (keys of 0x20028E8A0, PROVEN). Card
/// texts in the client language `language` (`5C 00`).
pub fn tab_blob(texts: &ClientTexts, offers: &[StoreOffer], tab: u32, language: u8) -> Vec<u8> {
    let items: Vec<Value> = offers.iter().filter(|o| o.tab == tab).map(|o| card(o, texts, language)).collect();
    binconfig::encode(&[
        ("currency_type".to_string(), Value::Str(CURRENCY_TYPE.to_string())),
        ("currency_sign".to_string(), Value::Str(CURRENCY_SIGN.to_string())),
        ("shop_items".to_string(), Value::List(items)),
    ])
}

/// `5C 00 u32 tab, u8 language, u8 steam` (`get_actions_data`): answer `BC sub 0` with the tab's blob, card texts in that
/// language, the offers this player is shown ([`shown_to`]). A tab the server does not list gets an empty `shop_items`
/// (the client asked, so something must come back).
pub fn get_data(cx: &mut Ctx, peer: &mut Peer, tab: u32, language: u8) {
    let n = push_tab(cx, peer, tab, language);
    log::info!("{}: 5C tab {tab} language {language}: {n} bytes", peer.addr);
}

/// `BC sub 0` of `tab` for this player; returns the blob length. The reader 0x200196540 has no request guard: an
/// unasked `BC sub 0` is cached (now + 300 s) and drawn like an answer (`root.set_actions_store_data` ->
/// `StoreActions.setData(value, tab)`, which rebuilds the card list of the open tab).
fn push_tab(cx: &Ctx, peer: &mut Peer, tab: u32, language: u8) -> usize {
    let offers = offers_for(cx, peer.account, unix_now());
    let blob = tab_blob(&cx.lobby.data.texts, &offers, tab, language);
    let n = blob.len();
    peer.push(&StoreActions::Tab { tab, blob, more: None }.push());
    n
}

/// `5C 01 u32 data id, u8 mode` (`buy_action`; mode 1 = gold): pay the offer's gold and apply it ([`OfferGrant`]), answer
/// `BC sub 1 {data id, no chest, result}`. An unknown offer, another mode, too little gold or a refused grant (a contract
/// already running: reachable only from a store view drawn before the purchase) answers result 0 and changes nothing.
/// A bought contract is followed by the offer's tab without the contract card ([`shown_to`]): sending `5C 01` expired
/// every cached tab (0x200196F48..0x200196F6B), and the new `BC sub 0` redraws the open list at once.
pub fn buy(cx: &mut Ctx, peer: &mut Peer, id: u32, mode: u8) {
    let offer = cx.lobby.cfg.store_offers.iter().find(|o| o.id == id).cloned();
    let result = match (&offer, mode) {
        (Some(o), 1) => match o.grant {
            OfferGrant::Items => buy_items(cx, peer, o),
            OfferGrant::Premium { hours } => crate::premium::buy_card(cx, peer, o.id, o.gold, hours),
            OfferGrant::WinnersContract => crate::contract::buy(cx, peer, o.id, o.gold),
        },
        (Some(_), m) => {
            log::info!("{}: 5C buy {id}: payment mode {m} is not served", peer.addr);
            false
        }
        (None, _) => {
            log::info!("{}: 5C buy {id}: no such offer", peer.addr);
            false
        }
    };
    peer.push(&StoreActions::Bought { data_id: id, chest_item: NO_CHEST, ok: result }.push());
    if let (true, Some(o @ StoreOffer { grant: OfferGrant::WinnersContract, .. })) = (result, &offer) {
        let language = peer.account.map_or(0, |a| cx.registry.language(a));
        push_tab(cx, peer, o.tab, language);
    }
}

/// An item bundle: one transaction spends the gold and grants the items (records `83 45 0F`, 8B, level / factions when
/// experience or reputation items were in it).
fn buy_items(cx: &mut Ctx, peer: &mut Peer, o: &StoreOffer) -> bool {
    let gd = cx.lobby.data.clone();
    let id = o.id;
    let rewards: Vec<Reward> = o.items.iter().map(|(d, n)| Reward { dict: *d, amount: *n }).collect();
    let paid = update(cx, peer, |p| {
        if p.spend(accounts::Currency::Gold, u64::from(o.gold)).is_err() {
            return None;
        }
        Some(crate::chests::grant(&gd, p, &rewards))
    });
    match paid {
        Some(Some((records, granted))) => {
            log::info!("{}: 5C buy {id}: paid {} gold", peer.addr, o.gold);
            let records: Vec<_> = records.into_iter().filter(|r| r.uid != 0).collect();
            if !records.is_empty() {
                peer.reply(Reply::Inventory(Inventory45::Records { x: 0, items: records }));
            }
            send_money(cx, peer);
            if granted.level || granted.reputation {
                // experience / reputation items of an operator offer: the level and faction pushes
                if let Some(acc) = peer.account {
                    if let Ok(p) = cx.lobby.store.load_profile(acc) {
                        peer.push(&S2c::Level(crate::build::level_8c(&gd, &p)));
                        peer.push(&S2c::Factions(crate::build::factions_8e(&p)));
                    }
                }
            }
            true
        }
        _ => {
            log::info!("{}: 5C buy {id}: not enough gold", peer.addr);
            false
        }
    }
}

/// `48 28 u16 source, u16 target` (`action_item_exchanged`, the event window `ItemsExchangeEventWindow`): see
/// [`crate::events::exchange`] (the wallet, the table and the reply layout are documented there and in docs/notes/events.md).
pub fn exchange(cx: &mut Ctx, peer: &mut Peer, source: u16, target: u16) {
    crate::events::exchange(cx, peer, source, target)
}

/// `46 08 u32 weapon, u8 tab` (`favorite_weapon_discount_clicked`): the player clicked the favourite-weapon discount card
/// of the store (a card the tab's `BC sub 0` tail announces: `is_favorite_weapon`, weapon dict id, web shop id). The client
/// reads no answer (no `83 46` sub 8, PROVEN) and opens the weapon itself; there is nothing to store. The server makes no
/// such card (it would need per-player weapon usage statistics, OPEN), so a click is only logged.
pub fn favorite_clicked(peer: &Peer, weapon: u32, tab: u8) {
    log::info!("{}: 46 08 favourite weapon {weapon} clicked on tab {tab}: no discount card is offered by this server", peer.addr);
}

/// The 19 pocket consumables of the client's static table (0x200976A90, PROVEN): bit `i` of the `91` mask opens entry `i`.
pub const SPECIFICS: [u16; 19] = [5, 65, 66, 67, 68, 82, 219, 733, 734, 736, 759, 760, 1089, 1090, 1091, 1101, 1106, 1133, 1196];

/// Is pocket consumable `dict` open for the player (`opened` = his `specifics_opened`, the `46 09` buys)? The client's rule
/// (PROVEN, 91 handler 0x200247030, AS `SlotsSelector.filterItemsByCategories`): open when its 91 mask bit is set or the
/// faction level reaches the item's level; an item outside the table of 19 (Flash grenade 2072, Nut 1355, Snowball 1884)
/// opens by faction level only ([`crate::progress::faction_allows`]).
pub fn pocket_open(gd: &game_data::GameData, p: &accounts::Profile, opened: &[u16], dict: u16) -> bool {
    crate::progress::faction_allows(gd, p, dict) || (SPECIFICS.contains(&dict) && opened.contains(&dict))
}

/// The `91` mask of an account: the bits of the specifics bought with `46 09` (the client also opens an item whose
/// faction level is reached), as the client gates them.
pub fn specifics_mask(cx: &Ctx, account: accounts::AccountId) -> u32 {
    let opened = cx.lobby.store.specifics_opened(account).unwrap_or_default();
    SPECIFICS.iter().enumerate().filter(|(_, d)| opened.contains(d)).fold(0, |m, (i, _)| m | (1 << i))
}

/// Gold price of opening `dict`: the client shows `dataPrices[clamp(sellerLevel - 2, 0, len - 1)]` of the `buy_specific_cost`
/// row the 8A `idx` names (AS `SlotsSelector.getItemPrice`, PROVEN; `idx` is 0). `sellerLevel` = the item's level (LIKELY).
pub fn specific_price(gd: &game_data::GameData, dict: u16) -> Option<u32> {
    let row = gd.buy_specific_cost.first()?;
    let level = gd.items.get(dict)?.level_min1() as usize;
    row.get(level.saturating_sub(2).min(row.len().checked_sub(1)?)).copied()
}

/// `46 09 u16 dict` (`buy_usage_of_specific`, paid in GOLD: the client's buy window sets `usePremiumMoney`): open the pocket
/// consumable permanently. Refused (logged) for an item outside the 19, one already open (bought, or its faction level is
/// reached: the client then shows no price), or too little gold. The answer is the new 91 mask (the client's `opened` flag
/// comes from it) and 8B.
pub fn buy_specific(cx: &mut Ctx, peer: &mut Peer, dict: u16) {
    let Some(me) = peer.account else { return };
    let gd = &cx.lobby.data;
    if !SPECIFICS.contains(&dict) {
        log::info!("{}: 46 09 dict {dict} is not one of the 19 specifics", peer.addr);
        return;
    }
    let Some(price) = specific_price(gd, dict) else {
        log::info!("{}: 46 09 dict {dict}: no price", peer.addr);
        return;
    };
    match cx.lobby.store.load_profile(me) {
        Ok(p) if crate::progress::faction_allows(gd, &p, dict) => {
            log::info!("{}: 46 09 dict {dict}: already open by faction level", peer.addr);
            return;
        }
        Ok(_) => {}
        Err(e) => {
            log::error!("{}: loading profile of {me}: {e}", peer.addr);
            return;
        }
    }
    match cx.lobby.store.buy_specific(me, dict, price, unix_now()) {
        Ok(()) => log::info!("{}: 46 09 opened specific {dict} for {price} gold", peer.addr),
        Err(e) => {
            log::info!("{}: 46 09 dict {dict} refused: {e}", peer.addr);
            return;
        }
    }
    peer.push(&S2c::SpecificsOpened(SpecificsOpened { mask: specifics_mask(cx, me) }));
    send_money(cx, peer);
}
