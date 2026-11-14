use std::collections::BTreeMap;

use survarium::v026g::item_params::item_params;
use survarium::v026g::loadouts::bot_gear::{bot_carry_limit, kit_problems};

use super::*;

const NOW: u64 = 1_000;

fn mm(dict_id: u16) -> u8 {
    item_params(dict_id).unwrap().match_making_level
}

/// The first ammo of the dictionary that fits `weapon`.
fn ammo_of(weapon: u16) -> u16 {
    let dictionary = Dictionary::shared();
    dictionary
        .items
        .iter()
        .find(|item| {
            item_params(item.dict_id).is_some_and(|params| params.kind == "ammo")
                && dictionary.compatible(weapon, item.dict_id)
        })
        .unwrap()
        .dict_id
}

/// A player's kit: `weapon` in slot 7 with its ammo in slot 8 (enough for
/// the Play check), and `armour`.
fn players_kit(weapon: u16, armour: &[(u8, u16)]) -> Loadout {
    let mut items = vec![
        SlotItem {
            slot: 7,
            dict_id: weapon,
            count: 1,
        },
        SlotItem {
            slot: 8,
            dict_id: ammo_of(weapon),
            count: 150,
        },
    ];
    items.extend(armour.iter().map(|&(slot, dict_id)| SlotItem {
        slot,
        dict_id,
        count: 1,
    }));
    Loadout { items }
}

/// An account that played `played` matches, of `rating`, playing `kit`
/// (its first kit, active).
fn account(name: &str, rating: u16, played: u32, kit: &Loadout) -> Account {
    let mut account = Account::with_loadout(name, kit);
    account.rating = rating;
    account.map_plays = vec![(1, played)];
    assert_eq!(
        rules::check_play(&account, account.active_kit, NOW),
        Ok(()),
        "{name}"
    );
    account
}

/// The table of `accounts`, ids 1 on.
fn table(accounts: &[Account]) -> BotKits {
    let mut gathering = Gathering::default();
    for (id, account) in accounts.iter().enumerate() {
        gathering.add(id as u32 + 1, account, NOW);
    }
    gathering.finish()
}

/// One armour piece of each level the data has one of: `(slot, dict id)`.
fn armour_by_level() -> BTreeMap<u8, (u8, u16)> {
    let dictionary = Dictionary::shared();
    let mut by_level = BTreeMap::new();
    for item in &dictionary.items {
        let Some(slot) = (0..=6).find(|&slot| dictionary.fits(item.dict_id, slot)) else {
            continue;
        };
        by_level
            .entry(mm(item.dict_id))
            .or_insert((slot, item.dict_id));
    }
    by_level
}

/// For each role and level, a player's kit whose bot kit is of that level:
/// each weapon of the role bare and in each level's armour piece.
fn kits_by_role_and_level() -> BTreeMap<(BotRole, u8), Loadout> {
    let armour = armour_by_level();
    let mut kits = BTreeMap::new();
    for item in &Dictionary::shared().items {
        let Some(role) = BotRole::of_weapon(item.dict_id) else {
            continue;
        };
        let pieces = std::iter::once(None).chain(armour.values().map(Some));
        for piece in pieces {
            let worn: Vec<(u8, u16)> = piece.into_iter().copied().collect();
            let kit = players_kit(item.dict_id, &worn);
            let Some(parts) = KitParts::of_kit(&kit).pop() else {
                continue;
            };
            let Ok(bot) = bot_kit_of(&parts.armour, parts.weapon, parts.ammo, role) else {
                continue;
            };
            kits.entry((role, bot.gear_level())).or_insert(kit);
        }
    }
    kits
}

/// The owner's ask: every level 0 to 15 gets each role a valid kit, from
/// made-up accounts at those levels; of the level when an account has a
/// kit of the role there, else of the nearest level that has one. Each
/// passes the bot kit rules and the lobby's Play check, its weapon and ammo
/// fit each other, and its second kit is of its level or lower.
#[test]
fn every_level_gets_a_valid_kit_for_each_role_from_accounts_at_it() {
    let kits = kits_by_role_and_level();
    let accounts: Vec<Account> = kits
        .values()
        .enumerate()
        .map(|(n, kit)| account(&format!("P{n}"), 100 + n as u16, 10, kit))
        .collect();
    let table = table(&accounts);
    let dictionary = Dictionary::shared();
    let mut exact = 0;
    for level in 0..LEVELS {
        for role in BotRole::ALL {
            let levels: Vec<u8> = kits
                .keys()
                .filter(|(each, _)| *each == role)
                .map(|&(_, level)| level)
                .collect();
            let nearest = *levels
                .iter()
                .min_by_key(|&&each| (each.abs_diff(level), each))
                .unwrap();
            for k in 0..3 {
                let kit = table.bot_kit(level, role, k);
                assert_eq!(
                    kit.source,
                    KitSource::Players {
                        level: nearest,
                        players: 1,
                        kits: 1
                    },
                    "{role} at {level}"
                );
                assert_eq!(kit.first.gear_level(), nearest, "{role} at {level}");
                assert_eq!(kit.level(), nearest, "{role} at {level}");
                for each in kit.kits() {
                    assert_eq!(kit_problems(each, bot_carry_limit()), Vec::<String>::new());
                    let weapon = each.slot(7).unwrap().dict_id;
                    for slot in loadouts::BOT_AMMO_SLOTS {
                        assert!(dictionary.compatible(weapon, each.slot(slot).unwrap().dict_id));
                    }
                    // The lobby's Play check, on an account wearing it.
                    let wearing = Account::with_loadout("Bot", each);
                    assert_eq!(
                        rules::check_play(&wearing, 1, NOW),
                        Ok(()),
                        "{role} at {level}"
                    );
                }
                assert_eq!(
                    BotRole::of_weapon(kit.first.slot(7).unwrap().dict_id),
                    Some(role)
                );
                if let Some(second) = &kit.second {
                    assert_eq!(
                        BotRole::of_weapon(second.slot(7).unwrap().dict_id),
                        Some(role.backup())
                    );
                    assert!(second.gear_level() <= kit.first.gear_level());
                }
            }
            exact += usize::from(nearest == level);
        }
    }
    // Most of the 64 are of their own level in the test data (it lacks a
    // few: no SMG of level 7, no assault rifle under 5, ...).
    assert!(exact >= 40, "{exact}");
}

/// No account at all (a new server): each role's fixed kits. Accounts at
/// one level only: every level takes theirs, and a role none has keeps its
/// fixed kits.
#[test]
fn the_fallback_with_no_players_at_a_level() {
    let empty = table(&[]);
    for level in [0, 7, 15] {
        for role in BotRole::ALL {
            let kit = empty.bot_kit(level, role, 0);
            assert_eq!(kit.source, KitSource::Fixed);
            let [first, second] = loadouts::bot_kits(role);
            assert_eq!(kit.first, *first);
            assert_eq!(kit.second.as_ref(), Some(second));
        }
    }
    // Flankers' kits of level 3 only: an Uzi in scavengers' level 3 torso.
    let dictionary = Dictionary::shared();
    let uzi = dictionary.item_named("uzi").unwrap().dict_id;
    let torso = dictionary.item_named("scav_torso_2").unwrap().dict_id;
    assert_eq!(mm(torso), 3);
    let only = table(&[account("Alice", 120, 10, &players_kit(uzi, &[(2, torso)]))]);
    for level in [0, 3, 15] {
        let kit = only.bot_kit(level, BotRole::Flanker, 0);
        assert_eq!(
            kit.source,
            KitSource::Players {
                level: 3,
                players: 1,
                kits: 1
            }
        );
        assert_eq!(kit.first.slot(7).unwrap().dict_id, uzi);
        assert_eq!(kit.first.slot(2).unwrap().dict_id, torso);
        // No support's kit of level 3 or lower: the fixed AK-74U is of
        // level 5, so no second kit.
        assert_eq!(kit.second, None);
        assert_eq!(
            only.bot_kit(level, BotRole::Camper, 0).source,
            KitSource::Fixed
        );
    }
}

/// Whose kits: an account of fewer than [`ACTIVE_MATCHES`] matches counts
/// not; of more, the [`DONORS`] highest-rated (ties by id) at the level;
/// each kit weighted by its use, picked in turn by those weights.
#[test]
fn the_top_players_kits_by_their_use() {
    let dictionary = Dictionary::shared();
    let dict = |name: &str| dictionary.item_named(name).unwrap().dict_id;
    let ak = dict("ak_74u");
    // Twelve torsos of level 3: a distinct kit each.
    let torsos: Vec<u16> = dictionary
        .items
        .iter()
        .filter(|item| dictionary.fits(item.dict_id, 2) && mm(item.dict_id) <= 5)
        .map(|item| item.dict_id)
        .take(12)
        .collect();
    assert_eq!(torsos.len(), 12);
    let accounts: Vec<Account> = torsos
        .iter()
        .enumerate()
        .map(|(n, &torso)| {
            account(
                &format!("P{n}"),
                100 + n as u16,
                ACTIVE_MATCHES,
                &players_kit(ak, &[(2, torso)]),
            )
        })
        .collect();
    let picked = table(&accounts);
    let kits = picked.role_kits(5, BotRole::Support).unwrap();
    assert_eq!((kits.players, kits.candidates.len()), (DONORS, DONORS));
    // The two lowest-rated are left out.
    for (n, &torso) in torsos.iter().enumerate() {
        let used = kits
            .candidates
            .iter()
            .any(|each| each.parts.armour == [(2, torso)]);
        assert_eq!(used, n >= 2, "P{n}");
    }
    // One match fewer: nobody counts.
    let mut inactive = accounts.clone();
    for account in &mut inactive {
        account.map_plays = vec![(1, ACTIVE_MATCHES - 1)];
    }
    assert_eq!(
        table(&inactive).bot_kit(5, BotRole::Support, 0).source,
        KitSource::Fixed
    );

    // Kit A played by two, kit B by one and kept in a preset of a fourth:
    // weights 4 and 3 (2 for a kit played, 1 for a preset's).
    let a = players_kit(ak, &[(2, torsos[0])]);
    let b = players_kit(ak, &[(2, torsos[1])]);
    let mut keeper = account("Keeper", 50, 10, &players_kit(ak, &[(2, torsos[2])]));
    keeper.presets[1] = super::super::presets::snapshot(&account("Copy", 50, 10, &b));
    let weighted = table(&[
        account("A1", 150, 10, &a),
        account("A2", 140, 10, &a),
        account("B1", 130, 10, &b),
        keeper,
    ]);
    let kits = weighted.role_kits(5, BotRole::Support).unwrap();
    let weights: Vec<(u16, u32)> = kits
        .candidates
        .iter()
        .map(|each| (each.parts.armour[0].1, each.weight))
        .collect();
    assert_eq!(weights, [(torsos[0], 4), (torsos[1], 3), (torsos[2], 2)]);
    // In turn by the weights: A, B, A, C, B, A, ...
    let torso = |k: usize| {
        weighted
            .bot_kit(5, BotRole::Support, k)
            .first
            .slot(2)
            .unwrap()
            .dict_id
    };
    let picks: Vec<u16> = (0..9).map(torso).collect();
    assert_eq!(picks.iter().filter(|&&each| each == torsos[0]).count(), 4);
    assert_eq!(picks.iter().filter(|&&each| each == torsos[1]).count(), 3);
    assert_eq!(picks.iter().filter(|&&each| each == torsos[2]).count(), 2);
    assert_eq!(picks[0], torsos[0]);
    // The same table, the same picks.
    assert_eq!((0..9).map(torso).collect::<Vec<_>>(), picks);
}

#[test]
fn a_smooth_weighted_round_robin_is_near_the_weights() {
    assert_eq!(
        (0..6).map(|k| nth_weighted(&[2, 1], k)).collect::<Vec<_>>(),
        [0, 1, 0, 0, 1, 0]
    );
    assert_eq!(
        (0..4)
            .map(|k| nth_weighted(&[1, 1, 1, 1], k))
            .collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    assert_eq!(nth_weighted(&[5], 7), 0);
}
