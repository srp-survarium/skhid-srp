//! The account as lobby messages (`survarium::v026g::lobby`): what each
//! push carries, and the pushes of a sign-in in the order the client needs
//! (`docs/notes/lobby.md` §3).

use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::lobby::{
    self, AccountBody, ItemDescr, Kit, KitSlot, MINUTES_TO_MAINTENANCE, Modification,
    NO_PROTECTION, OwnAccount, QuestRecord, ServerMessage, Upgrade, WireError, limits,
};
use survarium::v026g::modifications::Modifications;
use survarium::v026g::shop_offers;
use survarium::v026g::skills::SkillsTree;

use super::account::{Account, Item};
use super::{items_rules, rules};
use super::{premium, tutorial};

/// The team byte of an account body in the lobby [as 0.21d sent it there:
/// 0; a match's static info sets the real one].
pub const LOBBY_TEAM: u8 = 0;

/// An item as the wire describes it, by the clock.
pub fn item_descr(item: &Item) -> ItemDescr {
    item_descr_at(item, super::quests::now())
}

/// An item as the wire describes it at `now` (Unix seconds): its two
/// timers are the seconds left. A modification or defect the client's
/// tables lack is left out: the client would read the row it did not find
/// (`Account::repair` takes them off a loaded account; this is the last
/// guard).
pub fn item_descr_at(item: &Item, now: u64) -> ItemDescr {
    let table = Modifications::shared();
    ItemDescr {
        item_id: item.item_id,
        dict_id: item.dict_id,
        condition_or_stack: item.condition_or_stack,
        repair_time: items_rules::seconds_left(item.repair_time, now),
        reroll_time: items_rules::seconds_left(item.reroll_time, now),
        modifications: item
            .modifications
            .iter()
            .filter(|&&(id, state)| table.is_known(id, state))
            .map(|&(id, state)| Modification { id, state })
            .collect(),
        // A record whose dict id the data lacks (a withdrawn scope variant)
        // would have the client build the item with a modification it does
        // not have; the load takes such records off, this is the last guard.
        upgrades: item
            .upgrades
            .iter()
            .filter(|upgrade| {
                u16::try_from(upgrade.a)
                    .ok()
                    .is_some_and(|dict_id| Dictionary::shared().item(dict_id).is_some())
            })
            .map(|upgrade| Upgrade {
                id: upgrade.id,
                a: upgrade.a,
                value: upgrade.value,
                param: upgrade.param,
            })
            .collect(),
    }
}

/// The account body: its kits and the table of the items they hold, each
/// item once however many kits wear it. A slot whose item the inventory
/// lacks is left out (the client would look it up and crash).
pub fn account_body(account: &Account, now: u64) -> AccountBody {
    let modifiers = SkillsTree::shared().modifiers(&account.skills);
    let mut items: Vec<ItemDescr> = Vec::new();
    let mut kits = Vec::with_capacity(account.kits.len());
    for (index, kit) in account.kits.iter().enumerate().take(limits::KITS) {
        let mut slots = [None; limits::KIT_SLOTS];
        for (slot, held) in kit.slots.iter().enumerate() {
            let Some(held) = held else {
                continue;
            };
            let Some(owned) = account.item(held.item_id) else {
                continue;
            };
            let item_index = match items.iter().position(|item| item.item_id == owned.item_id) {
                Some(position) => position,
                None => {
                    items.push(item_descr_at(owned, now));
                    items.len() - 1
                }
            };
            slots[slot] = Some(KitSlot {
                item_index: item_index as u16,
                count: held.count.min(u32::from(u16::MAX)) as u16,
            });
        }
        kits.push(Kit {
            state: account.kit_state(index, now),
            id: kit.id,
            name: kit.name.clone(),
            slots,
            modifiers,
        });
    }
    AccountBody {
        team: LOBBY_TEAM,
        name: account.name.clone(),
        clan_tag: account.clan_tag.clone(),
        protection: NO_PROTECTION,
        kits,
        items,
    }
}

/// `89`: everything the account owns, by the clock.
pub fn inventory(account: &Account) -> Result<ServerMessage, WireError> {
    inventory_at(account, super::quests::now())
}

/// `89`: everything the account owns, at `now` (Unix seconds).
pub fn inventory_at(account: &Account, now: u64) -> Result<ServerMessage, WireError> {
    let items: Vec<ItemDescr> = account
        .inventory
        .iter()
        .map(|item| item_descr_at(item, now))
        .collect();
    lobby::inventory(&items)
}

/// `89` with what the server spent for the account after a match
/// ([`super::autobuy`]): the results screen lists it.
pub fn inventory_with_spent(
    account: &Account,
    spent: &lobby::SpentMoney,
) -> Result<ServerMessage, WireError> {
    let items: Vec<ItemDescr> = account.inventory.iter().map(item_descr).collect();
    lobby::inventory_with_spent(&items, spent)
}

/// `87`: the account. Send [`inventory`] before it.
pub fn own_account(account: &Account, now: u64) -> Result<ServerMessage, WireError> {
    let (level, start, next) = account.level_progress();
    lobby::own_account(&OwnAccount {
        body: account_body(account, now),
        revisions: account
            .kits
            .iter()
            .take(limits::KITS)
            .map(|kit| kit.revision)
            .collect(),
        active_kit: account.active().map_or(0, |kit| kit.id),
        autobuy: account.autobuy,
        auto_repair: account.extras.auto_repair,
        level,
        exp_total: account.exp_total.max(start),
        exp_level_start: start,
        exp_next_level: next,
    })
}

/// `88`: the account as a squad mate sees it, with the revision of the kit
/// it plays.
pub fn squad_member_account(account: &Account, now: u64) -> Result<ServerMessage, WireError> {
    let revision = account.active().map_or(0, |kit| kit.revision);
    lobby::squad_member_account(&account_body(account, now), revision)
}

/// `93`: the account's kits as a results screen's row shows them (the
/// game's "Information about the player" window), from the account as it
/// is now; `id` goes in the full result the client discards.
pub fn player_gear(account: &Account, id: u32, now: u64) -> Result<ServerMessage, WireError> {
    lobby::player_details(&lobby::FullResult::empty(id), &account_body(account, now))
}

/// `8B`.
pub fn money(account: &Account) -> ServerMessage {
    lobby::money(account.generic_money, account.premium_money)
}

/// `8C`.
pub fn level_and_skills(account: &Account) -> Result<ServerMessage, WireError> {
    let (level, start, next) = account.level_progress();
    lobby::level_and_skills(
        level,
        account.exp_total.max(start),
        start,
        next,
        &account.skills,
    )
}

/// `8E`: the reputations and the faction whose quests the account takes.
pub fn reputations(account: &Account) -> Result<ServerMessage, WireError> {
    let list: Vec<(u8, u16)> = account
        .reputations
        .iter()
        .map(|&(faction, points)| (faction, points.min(u32::from(u16::MAX)) as u16))
        .collect();
    lobby::reputations(&list, account.quest_faction)
}

/// `92`: the account's quests at `now` (Unix seconds).
pub fn quests(account: &Account, now: u64) -> ServerMessage {
    let records: Vec<QuestRecord> = account
        .quests
        .iter()
        .map(|quest| QuestRecord {
            instance_id: quest.instance_id,
            quest_id: quest.quest_id,
            games: quest.games,
            minutes_left: quest
                .expires_at
                .saturating_sub(now)
                .div_ceil(60)
                .min(i32::MAX as u64) as u32,
            active: true,
        })
        .collect();
    lobby::quests(&records)
}

/// `94`: the premium account, if it holds.
pub fn services(account: &Account, now: u64) -> ServerMessage {
    lobby::services(account.premium_hours(now))
}

/// `96`.
pub fn unlocked_factions(account: &Account) -> ServerMessage {
    lobby::unlocked_factions(rules::unlocked_factions(account))
}

/// `90`: the account's row of the ranking, at `place` (from 1).
pub fn player_elo(account: &Account, place: u32) -> ServerMessage {
    lobby::player_elo(place, account.rating, account.rating_before)
}

/// `9B`: the chances and prices of an item's upgrade ("craft"), from the
/// game's `craft.options`. The lobby's upgrade window shows them and works
/// its price out from them (`MessageCraftItem.updateCraftChance`).
pub fn craft_tables() -> ServerMessage {
    let (chances, general, premium) = Modifications::shared().craft.tables();
    lobby::craft_tables(&chances, &general, &premium)
}

/// What the client must be sent after an action changed the account: it
/// never asks for its inventory, account or money (`lobby_requests.md` §3).
/// The inventory comes first: the account's items are looked up in it.
pub fn after_change(account: &Account, now: u64) -> Result<Vec<ServerMessage>, WireError> {
    Ok(vec![
        inventory_at(account, now)?,
        own_account(account, now)?,
        money(account),
    ])
}

/// What a sign-in's pushes need besides the account.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SignInInfo {
    /// Unix seconds.
    pub now: u64,
    /// The client's `2E` flag: it has no shop records yet.
    pub need_shop_items: bool,
    /// How many players the ranking has, and the account's place (from 1).
    pub ranked_players: u32,
    pub place: u32,
    pub players_online: u32,
    pub games_online: u32,
    pub minutes_to_maintenance: u32,
}

impl SignInInfo {
    /// A lone player with no maintenance planned.
    pub fn at(now: u64) -> Self {
        Self {
            now,
            need_shop_items: true,
            ranked_players: 1,
            place: 1,
            players_online: 1,
            games_online: 0,
            minutes_to_maintenance: MINUTES_TO_MAINTENANCE,
        }
    }
}

/// Everything the client needs once its sign-in (`2E`) is accepted, in the
/// order of `lobby.md` §3: the sign-in answer, then the pushes, with the
/// inventory before the account. The first message is the only one the
/// client takes before it is signed in.
///
/// Not here, because they need other accounts: the friend and ignore lists
/// (0xA3 07/08, which the client never asks for), the squad (0xA1) and the
/// clan (0x9A).
pub fn sign_in_push(account: &Account, info: &SignInInfo) -> Result<Vec<ServerMessage>, WireError> {
    let now = info.now;
    let mut push = vec![
        lobby::signed_in(0, false),
        lobby::base_info(info.ranked_players, &account.name),
        player_elo(account, info.place),
        money(account),
        unlocked_factions(account),
        reputations(account)?,
        lobby::service_prices(&rules::service_prices()),
        services(account, now),
        // The shop's list and the premium items sold for gold.
        lobby::prices(shop_offers::price_list())?,
    ];
    let settings = premium::settings();
    if info.need_shop_items {
        // Premium time and the bundles (`shop_offers`).
        push.push(lobby::shop(&settings.shop_records())?);
    }
    push.extend([
        inventory_at(account, now)?,
        own_account(account, now)?,
        level_and_skills(account)?,
        quests(account, now),
        // The numbers of the clan rules (`clans::Config`).
        super::clans::limits(super::clans::Config::shared()),
        tutorial::message(true),
        lobby::online_stats(
            info.players_online,
            info.games_online,
            info.minutes_to_maintenance,
        ),
        // The upgrade window reads its chances and prices from these; it
        // has none of its own.
        craft_tables(),
        lobby::client_state(&lobby::ClientState::Lobby, None),
    ]);
    Ok(push)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::account::{KIT_SLOTS, KitSlot as HeldSlot};
    use crate::v026g::{quests as quest_rules, rules};
    use survarium::v026g::dictionary::Dictionary;
    use survarium::v026g::drops::Rng;
    use survarium::v026g::loadouts::Loadouts;
    use survarium::v026g::lobby::client_model::{Client, Decoded, ModelError};
    use survarium::v026g::lobby::requests::{INVENTORY_SLOT, Relocation};
    use survarium::v026g::lobby::{Delivery, FULL_CONDITION, KIT_READY, ServerId};
    use survarium::v026g::prices;

    const NOW: u64 = 1_000;

    fn bytes(message: &ServerMessage) -> Vec<u8> {
        let mut out = vec![message.id_byte()];
        out.extend_from_slice(message.body());
        out
    }

    #[test]
    fn a_new_account_goes_out_byte_for_byte() {
        let account = Account::new("Alice");
        // The inventory: the TT-33 (dict 18, no stack) and 50 rounds of its
        // ammo (dict 50, a stack), which the first kit wears; then the
        // Mosin carbine (107), 10 rounds of its ammo (104) and the PPSh-41
        // (106).
        #[rustfmt::skip]
        let inventory_bytes: Vec<u8> = vec![
            0x89, 5, 0, 0, 0,
            1, 0, 0, 0,  18, 0,  0x40, 0x42, 0x0F, 0,  0, 0, 0, 0,  0, 0, 0, 0,  0,  0,
            2, 0, 0, 0,  50, 0,  50, 0, 0, 0,  0,  0,
            3, 0, 0, 0,  107, 0,  0x40, 0x42, 0x0F, 0,  0, 0, 0, 0,  0, 0, 0, 0,  0,  0,
            4, 0, 0, 0,  104, 0,  10, 0, 0, 0,  0,  0,
            5, 0, 0, 0,  106, 0,  0x40, 0x42, 0x0F, 0,  0, 0, 0, 0,  0, 0, 0, 0,  0,  0,
            0, 0, 0,
        ];
        assert_eq!(bytes(&inventory(&account).unwrap()), inventory_bytes);

        #[rustfmt::skip]
        let mut expected: Vec<u8> = vec![
            0x87,
            0,                              // team
            5, b'A', b'l', b'i', b'c', b'e',
            0,                              // no clan
            10, 0, 0, 0,                    // no new-player protection
            3,                              // kits
            2,                              // items
            // Kit 1: the pistol in slot 7, its ammo in slot 8.
            0,  1, 0, 0, 0,  0,  0x80, 0x01, 0, 0,
            0, 0, 1, 0,
            1, 0, 50, 0,
            0, 0, 0, 0, 0, 0, 0, 0,  0, 0, 0, 0, 0, 0, 0, 0,
            // Kit 2: empty.
            0,  2, 0, 0, 0,  0,  0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0,  0, 0, 0, 0, 0, 0, 0, 0,
            // Kit 3: empty, and usable without premium.
            0,  3, 0, 0, 0,  0,  0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0,  0, 0, 0, 0, 0, 0, 0, 0,
        ];
        // The item table: the two items the kits wear, as in the inventory
        // (20 and 12 bytes).
        expected.extend_from_slice(&inventory_bytes[5..5 + 20 + 12]);
        #[rustfmt::skip]
        expected.extend_from_slice(&[
            1, 0, 0, 0,  1, 0, 0, 0,  1, 0, 0, 0,   // the kits' revisions
            1, 0, 0, 0,                             // the active kit
            0,                                      // autobuy
            0,                                      // level
            0, 0, 0, 0,  0, 0, 0, 0,  0xF4, 0x01, 0, 0,
        ]);
        assert_eq!(bytes(&own_account(&account, NOW).unwrap()), expected);
        // A new account's 2,000 silver and no gold.
        assert_eq!(
            bytes(&money(&account)),
            [0x8B, 0xD0, 0x07, 0, 0, 0, 0, 0, 0]
        );
        // And with gold: the second u32.
        let mut rich = account.clone();
        rich.premium_money = 1_000;
        assert_eq!(
            bytes(&money(&rich)),
            [0x8B, 0xD0, 0x07, 0, 0, 0xE8, 0x03, 0, 0]
        );
        assert_eq!(
            bytes(&level_and_skills(&account).unwrap()),
            [0x8C, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xF4, 0x01, 0, 0, 0]
        );
        #[rustfmt::skip]
        assert_eq!(
            bytes(&reputations(&account).unwrap()),
            [
                0x8E, 4,
                1, 0, 0, 0, 0, 0, 0, 0,  2, 0, 0, 0, 0, 0, 0, 0,
                3, 0, 0, 0, 0, 0, 0, 0,  4, 0, 0, 0, 0, 0, 0, 0,
                0,
            ]
        );
        assert_eq!(bytes(&quests(&account, NOW)), [0x92, 0, 0, 0, 0]);
        assert_eq!(bytes(&services(&account, NOW)), [0x94, 0]);
        assert_eq!(bytes(&unlocked_factions(&account)), [0x96, 0x0F]);
    }

    #[test]
    fn the_sign_in_pushes_are_in_the_clients_order() {
        let account = Account::new("Alice");
        let push = sign_in_push(&account, &SignInInfo::at(NOW)).unwrap();
        let ids: Vec<u8> = push.iter().map(ServerMessage::id_byte).collect();
        assert_eq!(
            ids,
            [
                0x80, 0x8F, 0x90, 0x8B, 0x96, 0x8E, 0x8D, 0x94, 0x8A, 0x95, 0x89, 0x87, 0x8C, 0x92,
                0x9C, 0x9D, 0x9F, 0x9B, 0x85
            ]
        );
        // The craft tables, byte for byte: the twelve chances (for an
        // item with 0, 1 and 2 more modifications than its donor: success
        // and failure in silver, then in gold), the sixteen prices in
        // silver and the sixteen in gold.
        #[rustfmt::skip]
        let mut tables: Vec<u8> = vec![
            0x9B,
            0, 0, 0x48, 0x42,  0, 0, 0, 0,        0, 0, 0xC8, 0x42,  0, 0, 0, 0,
            0, 0, 0xC8, 0x41,  0, 0, 0xC8, 0x41,  0, 0, 0x48, 0x42,  0, 0, 0, 0,
            0, 0, 0x48, 0x41,  0, 0, 0x87, 0x42,  0, 0, 0xC8, 0x41,  0, 0, 0xC8, 0x41,
        ];
        for price in [
            400_u16, 400, 800, 1600, 2400, 3400, 4400, 5400, 6600, 8000, 10000, 12000, 14000,
            16000, 18000, 20000, 2, 2, 4, 8, 12, 17, 22, 27, 33, 40, 50, 60, 70, 80, 90, 100,
        ] {
            tables.extend(price.to_le_bytes());
        }
        assert_eq!(bytes(&push[17]), tables);
        assert_eq!(tables.len(), 1 + 112);
        assert_eq!(bytes(&push[0]), [0x80, 0, 0]);
        assert_eq!(bytes(push.last().unwrap()), [0x85, 0]);
        // The inventory before the account; the mask before the
        // reputations.
        let at = |id: u8| ids.iter().position(|&other| other == id).unwrap();
        assert!(at(0x89) < at(0x87));
        assert!(at(0x96) < at(0x8E));
        // The order holds on the wire only among the ordered ids: all but
        // the first and the online stats.
        for message in &push[1..] {
            let expected = if message.id() == ServerId::OnlineStats {
                Delivery::Plain
            } else {
                Delivery::Ordered
            };
            assert_eq!(message.delivery(), expected, "{:?}", message.id());
        }
        // A real client takes every one of them.
        let mut client = Client::new();
        let read = client.receive_all(&push).unwrap();
        assert_eq!(read.len(), push.len());
        assert_eq!(client.inventory().len(), 5);
        assert!(matches!(
            &read[11],
            Decoded::Account(own) if own.body.kits.len() == 3 && own.active_kit == 1
        ));
        // Without the shop when the client has it.
        let info = SignInInfo {
            need_shop_items: false,
            ..SignInInfo::at(NOW)
        };
        let push = sign_in_push(&account, &info).unwrap();
        assert!(push.iter().all(|message| message.id() != ServerId::Shop));
        assert!(Client::new().receive_all(&push).is_ok());
        // The maintenance minutes leave Play enabled (over 30) with no
        // warning (60 or more).
        assert!(matches!(
            read[16],
            Decoded::OnlineStats {
                minutes_to_maintenance: 1440,
                ..
            }
        ));
    }

    #[test]
    fn the_account_before_the_inventory_would_crash_the_client() {
        let account = Account::new("Alice");
        let mut client = Client::signed_in();
        assert_eq!(
            client.receive(&own_account(&account, NOW).unwrap()),
            Err(ModelError::NotInInventory(1))
        );
        let mut client = Client::signed_in();
        assert!(
            client
                .receive_all(&after_change(&account, NOW).unwrap())
                .is_ok()
        );
    }

    #[test]
    fn every_change_goes_out_in_a_form_the_client_takes() {
        let dictionary = Dictionary::shared();
        let mut account = Account::new("Alice");
        account.generic_money = 500_000_000;
        account.premium_until = NOW + 86_400;
        account.add_experience(50_000);
        for faction in 1..=4 {
            account.add_reputation(faction, 40_000);
        }
        let mut client = Client::new();
        client
            .receive_all(&sign_in_push(&account, &SignInInfo::at(NOW)).unwrap())
            .unwrap();

        // Buy one of everything the shop lists.
        for record in prices::price_list() {
            let bought = rules::buy(
                &mut account,
                record.dict_id,
                1,
                record.faction_id,
                INVENTORY_SLOT,
                1,
                NOW,
            )
            .unwrap();
            client
                .receive(&lobby::permitted_bought(&bought).unwrap())
                .unwrap();
        }
        assert!(account.inventory.len() > 100);
        client
            .receive_all(&after_change(&account, NOW).unwrap())
            .unwrap();
        assert_eq!(client.inventory().len(), account.inventory.len());

        // Dress all three kits with whatever fits each slot.
        for kit in 1..=3 {
            let mut moves = Vec::new();
            for slot in 0..KIT_SLOTS as u8 {
                let Some(item) = account
                    .inventory
                    .iter()
                    .find(|item| dictionary.fits(item.dict_id, slot))
                else {
                    continue;
                };
                moves.push(Relocation {
                    item_id: item.item_id,
                    dict_id: item.dict_id,
                    source_slot: INVENTORY_SLOT,
                    target_slot: slot,
                    count: 1,
                    count_in_inventory: 0,
                });
            }
            rules::move_items(&mut account, kit, &moves, NOW).unwrap();
        }
        let body = account_body(&account, NOW);
        assert!(body.items.len() <= limits::ACCOUNT_ITEMS);
        // The three kits wear the same items, and a stack may fill several
        // slots: each item is listed once.
        let filled = account.kits[0].slots.iter().flatten().count();
        assert!(filled > 10);
        let mut worn: Vec<u32> = account.kits[0]
            .slots
            .iter()
            .flatten()
            .map(|slot| slot.item_id)
            .collect();
        worn.sort_unstable();
        worn.dedup();
        assert_eq!(body.items.len(), worn.len());
        for kit in &body.kits {
            assert_eq!(kit.slots.iter().flatten().count(), filled);
        }
        assert!(body.kits.iter().all(|kit| kit.state == KIT_READY));
        client
            .receive_all(&after_change(&account, NOW).unwrap())
            .unwrap();

        // Skills, a quest faction and a sale.
        rules::set_skills(&mut account, &[(101, 5), (102, 5), (201, 1)]).unwrap();
        quest_rules::choose_faction(&mut account, 3).unwrap();
        quest_rules::refresh(&mut account, NOW, &mut Rng::new(9));
        let sold_id = account.inventory[0].item_id;
        let sold = rules::sell(&mut account, sold_id, 1).unwrap();
        client
            .receive(&lobby::permitted_sold(sold.item_id, sold.amount))
            .unwrap();
        let mut push = after_change(&account, NOW).unwrap();
        push.extend([
            level_and_skills(&account).unwrap(),
            reputations(&account).unwrap(),
            quests(&account, NOW),
            squad_member_account(&account, NOW).unwrap(),
        ]);
        let read = client.receive_all(&push).unwrap();
        // The kits carry the skills' modifiers.
        let Decoded::Account(own) = &read[1] else {
            panic!("{:?}", read[1]);
        };
        assert!(own.body.kits[0].modifiers.iter().any(|&value| value != 0.0));
        assert_eq!(own.body.kits[0].modifiers, own.body.kits[2].modifiers);
        assert_eq!(own.level, account.level());
        assert!(matches!(
            &read[4],
            Decoded::Reputations { quest_active_faction: 3, reputations } if reputations[0] == (1, 40_000)
        ));
        assert!(matches!(&read[5], Decoded::Quests(list) if list.len() == 3 && list[0].active));
    }

    #[test]
    fn no_kit_is_marked_premium() {
        let mut account = Account::new("Alice");
        let states = |account: &Account| -> Vec<u8> {
            account_body(account, NOW)
                .kits
                .iter()
                .map(|kit| kit.state)
                .collect()
        };
        assert_eq!(states(&account), [KIT_READY, KIT_READY, KIT_READY]);
        account.premium_until = NOW + 1;
        assert_eq!(states(&account), [KIT_READY, KIT_READY, KIT_READY]);
        assert_eq!(
            bytes(&services(&account, NOW)),
            [0x94, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0]
        );
    }

    #[test]
    fn a_slot_whose_item_is_gone_is_not_sent() {
        let mut account = Account::new("Alice");
        // Not what `sync_kits` leaves, but what a damaged row could.
        account.kits[1].slots[7] = Some(HeldSlot {
            item_id: 999,
            count: 1,
        });
        let body = account_body(&account, NOW);
        assert_eq!(body.kits[1].slots[7], None);
        assert_eq!(body.items.len(), 2);
        let mut client = Client::signed_in();
        assert!(
            client
                .receive_all(&after_change(&account, NOW).unwrap())
                .is_ok()
        );
    }

    #[test]
    fn the_test_loadouts_go_out_whole() {
        for loadout in Loadouts::shared().usable() {
            let account = Account::with_loadout("Bob", loadout);
            let body = account_body(&account, NOW);
            assert_eq!(body.items.len(), loadout.items.len());
            for item in &body.items {
                let stack = Dictionary::shared().is_stack(item.dict_id);
                assert!(stack || item.condition_or_stack == FULL_CONDITION);
            }
            let mut client = Client::new();
            client
                .receive_all(&sign_in_push(&account, &SignInInfo::at(NOW)).unwrap())
                .unwrap();
        }
    }
}
