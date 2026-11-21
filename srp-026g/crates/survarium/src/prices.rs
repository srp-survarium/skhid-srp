//! The shop for 0.21d. 0.21d's data has no prices, but each item's config
//! names its `faction`; where it does, that faction sells the item.
//!
//! Ammo and consumables cost what the 2013 Survarium wiki says (as srp's
//! 0.100b shop, `../srp-021d/docs/notes/prices.md`). Weapons and armour follow
//! srp's ladder (`../srp-021d/docs/notes/shop_ranks.md`): a base price (a
//! rifle, shotgun or SMG 1000, as the PPSh-41 a new player can buy; a pistol
//! 400; an armour piece 300) times the tier of the rank that unlocks it
//! (`RANKS`): ×1, 1.5, 2, 3, 4, 6 for Scavenger and Black Market ranks 0..5,
//! and ×6, 8 for Renaissance and Edge ranks 1 and 2, whose points are those
//! of the others' top ranks.
//!
//! srp then raises what a high rank unlocks [its own choice]: weapons and
//! armour needing rank 2 cost 1.15 times the ladder, rank 3 1.25 times, rank 4 1.5 times, rank
//! 5 1.75 times, and the Renaissance and Edge gear (items of level 6 and 7)
//! twice, so that the best gear is far out of the reach of a new player.

/// 0.21d faction ids (`factions_dict`).
pub mod faction {
    pub const SCAVENGERS: u8 = 1;
    pub const BLACK_MARKET: u8 = 2;
    pub const RENAISSANCE: u8 = 3;
    pub const EDGE: u8 = 4;
}

use faction::{BLACK_MARKET, EDGE, RENAISSANCE, SCAVENGERS};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Price {
    pub dict_id: u16,
    /// For one item; for ammo, one round. The client is shown and charged
    /// a clip: this × `item_params::shop_unit`.
    pub cost: u16,
    pub sellers: &'static [u8],
}

const BOTH: &[u8] = &[SCAVENGERS, BLACK_MARKET];

/// Sellers marked "cfg" are the item config's `faction` in 0.21d; the others
/// are the wiki's, for items whose config names none (ammo, drugs, the trap,
/// the TT-33, and the Vityaz, `faction: 0`).
#[rustfmt::skip]
pub const PRICES: &[Price] = &[
    // Weapons: the ladder.
    Price { dict_id: 13, cost: 10500, sellers: &[BLACK_MARKET] }, // AKS-74U, cfg
    Price { dict_id: 14, cost: 10500, sellers: &[SCAVENGERS] },   // Remington 700, cfg
    Price { dict_id: 15, cost: 6000, sellers: &[BLACK_MARKET] }, // Remington 870, cfg
    Price { dict_id: 19, cost: 10500, sellers: &[SCAVENGERS] },   // Vityaz
    Price { dict_id: 16, cost: 1500, sellers: &[SCAVENGERS] },   // TOZ-34, cfg
    Price { dict_id: 55, cost: 2300, sellers: &[BLACK_MARKET] }, // Uzi, cfg
    Price { dict_id: 56, cost: 1500, sellers: &[BLACK_MARKET] }, // Colt Python, cfg
    Price { dict_id: 12, cost: 3750, sellers: &[SCAVENGERS] },   // TOZ-122, cfg
    Price { dict_id: 17, cost: 1500, sellers: &[BLACK_MARKET] }, // TOZ-66, cfg
    Price { dict_id: 64, cost: 6400, sellers: &[RENAISSANCE] },  // Fort 17, cfg (the wiki: Scavengers)
    Price { dict_id: 18, cost: 400,  sellers: BOTH },            // TT-33, a starting weapon
    // Ammo, December 2013: 1 a round. Sold where its weapons are. The
    // better rounds of a calibre (`arp` and `damage` in their configs above
    // 1: AP 1.2, 7N21 and PBM 1.25, buck 3/0 over 2/0, the slug's 1 against
    // the buck's 0.25) cost more than the common one, which the wiki's
    // single price didn't say [srp's choice].
    Price { dict_id: 7,  cost: 1, sellers: &[BLACK_MARKET] }, // 5.45x39
    Price { dict_id: 20, cost: 1, sellers: &[BLACK_MARKET] }, // .357 Magnum
    Price { dict_id: 22, cost: 1, sellers: BOTH },            // 12x70 2/0
    Price { dict_id: 72, cost: 2, sellers: BOTH },            // 12x70 slug: penetration 1 against the buck's 0.25 (srp)
    Price { dict_id: 73, cost: 2, sellers: BOTH },            // 12x70 3/0: better than 2/0 (srp)
    Price { dict_id: 50, cost: 1, sellers: BOTH },            // 7.62x25
    Price { dict_id: 51, cost: 1, sellers: &[SCAVENGERS] },   // 7.62x51
    Price { dict_id: 52, cost: 2, sellers: &[SCAVENGERS] },   // 7.62x51 AP: 20% more penetration (srp)
    Price { dict_id: 70, cost: 1, sellers: &[SCAVENGERS] },   // 9x18
    Price { dict_id: 53, cost: 1, sellers: BOTH },            // 9x19 FMJ
    Price { dict_id: 71, cost: 3, sellers: BOTH },            // 9x19 7N21: as the 9x18 PBM, 25% more damage and penetration (srp)
    // Scavenger armour, on the ladder; every seller from the cfg.
    Price { dict_id: 43, cost: 700,  sellers: &[SCAVENGERS] }, // "Amber" respirator
    Price { dict_id: 31, cost: 450,  sellers: &[SCAVENGERS] }, // quilted jacket
    Price { dict_id: 34, cost: 700, sellers: &[SCAVENGERS] }, // chemical protection jacket
    Price { dict_id: 48, cost: 1800, sellers: &[SCAVENGERS] }, // light armoured vest
    Price { dict_id: 49, cost: 450, sellers: &[SCAVENGERS] }, // backpack with container
    Price { dict_id: 9,  cost: 1200, sellers: &[SCAVENGERS] }, // oxygen tank: a guess, as the backpack
    Price { dict_id: 45, cost: 450,  sellers: &[SCAVENGERS] }, // linen pants
    Price { dict_id: 46, cost: 700, sellers: &[SCAVENGERS] }, // chemical protection pants
    Price { dict_id: 47, cost: 1800, sellers: &[SCAVENGERS] }, // field pants
    Price { dict_id: 41, cost: 450,  sellers: &[SCAVENGERS] }, // cut gloves
    Price { dict_id: 42, cost: 700, sellers: &[SCAVENGERS] }, // chemical protection gloves
    Price { dict_id: 37, cost: 450,  sellers: &[SCAVENGERS] }, // leather boots
    Price { dict_id: 38, cost: 700, sellers: &[SCAVENGERS] }, // chemical protection boots
    Price { dict_id: 39, cost: 1800, sellers: &[SCAVENGERS] }, // military boots
    // Black Market armour, on the ladder; every seller from the cfg.
    Price { dict_id: 27, cost: 1200,  sellers: &[BLACK_MARKET] }, // biker's helmet
    Price { dict_id: 29, cost: 450,  sellers: &[BLACK_MARKET] }, // leather sleeveless jacket
    Price { dict_id: 32, cost: 1200, sellers: &[BLACK_MARKET] }, // "Bruiser" jacket
    Price { dict_id: 33, cost: 3150, sellers: &[BLACK_MARKET] }, // "Corsair" jacket
    Price { dict_id: 28, cost: 450,  sellers: &[BLACK_MARKET] }, // jeans
    Price { dict_id: 44, cost: 1200, sellers: &[BLACK_MARKET] }, // reinforced jeans
    Price { dict_id: 25, cost: 450,  sellers: &[BLACK_MARKET] }, // biker gloves
    Price { dict_id: 40, cost: 3150, sellers: &[BLACK_MARKET] }, // spiked gloves
    Price { dict_id: 24, cost: 450,  sellers: &[BLACK_MARKET] }, // cossacks
    Price { dict_id: 35, cost: 1800, sellers: &[BLACK_MARKET] }, // ropers
    Price { dict_id: 36, cost: 3150, sellers: &[BLACK_MARKET] }, // choppers
    // Consumables: the trap May 2013, the drugs December 2013.
    Price { dict_id: 68, cost: 5, sellers: &[BLACK_MARKET] }, // trap
    Price { dict_id: 67, cost: 5, sellers: BOTH },            // medkit
    Price { dict_id: 66, cost: 5, sellers: &[SCAVENGERS] },   // set of bandages
    Price { dict_id: 65, cost: 5, sellers: &[BLACK_MARKET] }, // anesthetic
    // Not in the 2013 lists: weapons and armour on the ladder, the grenade
    // and antitoxin at 3 × the nearest priced item of their kind. Sellers
    // from the cfg, or as noted where it names none.
    Price { dict_id: 58,  cost: 3600,  sellers: &[RENAISSANCE] }, // Renaissance torso, cfg
    Price { dict_id: 59,  cost: 3600,  sellers: &[RENAISSANCE] }, // Renaissance legs, cfg
    Price { dict_id: 60,  cost: 3600,  sellers: &[RENAISSANCE] }, // Renaissance helmet, cfg
    Price { dict_id: 62,  cost: 3600,  sellers: &[RENAISSANCE] }, // Renaissance boots, cfg
    Price { dict_id: 83,  cost: 3600,  sellers: &[EDGE] },        // Edge torso, cfg
    Price { dict_id: 84,  cost: 3600,  sellers: &[EDGE] },        // Edge legs, cfg
    Price { dict_id: 85,  cost: 3600,  sellers: &[EDGE] },        // Edge boots, cfg
    Price { dict_id: 86,  cost: 3600,  sellers: &[EDGE] },        // Edge helmet, cfg
    Price { dict_id: 87,  cost: 3600,  sellers: &[EDGE] },        // Edge back, cfg
    Price { dict_id: 102, cost: 3600,  sellers: &[EDGE] },        // Edge respirator, cfg
    Price { dict_id: 80,  cost: 12000, sellers: &[RENAISSANCE] }, // AKMN, cfg
    Price { dict_id: 109, cost: 12000, sellers: &[EDGE] },        // SKS, cfg
    Price { dict_id: 88,  cost: 2300,  sellers: &[SCAVENGERS] },  // Kiparis, cfg
    Price { dict_id: 106, cost: 1000,  sellers: &[SCAVENGERS] },  // PPSh
    Price { dict_id: 89,  cost: 900,  sellers: &[SCAVENGERS] },  // PM, cfg
    Price { dict_id: 126, cost: 6400,  sellers: &[EDGE] },        // APS, cfg
    Price { dict_id: 90,  cost: 3750,  sellers: &[BLACK_MARKET] }, // Winchester 1894, cfg
    Price { dict_id: 107, cost: 1000,  sellers: &[SCAVENGERS] },  // Mosin-Nagant
    Price { dict_id: 108, cost: 2300,  sellers: &[SCAVENGERS] },  // IZh-27
    Price { dict_id: 110, cost: 6000,  sellers: &[SCAVENGERS] },  // MP-153, cfg
    Price { dict_id: 124, cost: 16000,  sellers: &[RENAISSANCE] }, // SPAS-12, cfg
    Price { dict_id: 125, cost: 16000,  sellers: &[EDGE] },        // Benelli M4, cfg
    Price { dict_id: 81,  cost: 3, sellers: &[RENAISSANCE, EDGE] },  // 7.62x39, for the AKMN and SKS
    Price { dict_id: 91,  cost: 3, sellers: &[BLACK_MARKET] },       // .30-30, for the Winchester
    Price { dict_id: 103, cost: 3, sellers: &[SCAVENGERS, EDGE] },   // 9x18 PBM, for the PM and APS
    Price { dict_id: 104, cost: 3, sellers: &[SCAVENGERS] },         // 7.62x54R, for the Mosin
    Price { dict_id: 5,   cost: 15, sellers: BOTH },  // RGD-5 grenade; no grenade priced: 3 × the trap
    Price { dict_id: 82,  cost: 15, sellers: BOTH },  // antitoxin; 3 × the drugs
];

/// The reputation rank (level) with the seller that buying an item needs,
/// `(dict_id, seller, rank)`; anything not listed needs none. The client
/// locks such an item with the rank's name and marks it new once reached
/// (`GangShop.setPlayerProgress`). No list from 0.21d survives: the ranks are
/// the Survarium wiki's faction pages of January 2015 (0.25-0.26) for the
/// items 0.21d has, except as noted (`../srp-021d/docs/notes/shop_ranks.md`).
#[rustfmt::skip]
pub const RANKS: &[(u16, u8, u8)] = &[
    // Scavengers: 1 Rookie, 2 Determined, 3 Experienced, 4 Die-Hard, 5 Veteran.
    (16, SCAVENGERS, 1),  // TOZ-34
    (31, SCAVENGERS, 1),  // quilted jacket
    (45, SCAVENGERS, 1),  // linen pants
    (41, SCAVENGERS, 1),  // cut gloves
    (37, SCAVENGERS, 1),  // leather boots
    (49, SCAVENGERS, 1),  // backpack with container
    (66, SCAVENGERS, 1),  // bandages
    (88, SCAVENGERS, 2),  // Kiparis
    (89, SCAVENGERS, 2),  // PM
    (43, SCAVENGERS, 2),  // "Amber" respirator
    (34, SCAVENGERS, 2),  // chemical protection jacket
    (46, SCAVENGERS, 2),  // chemical protection pants
    (42, SCAVENGERS, 2),  // chemical protection gloves
    (38, SCAVENGERS, 2),  // chemical protection boots
    (108, SCAVENGERS, 2), // IZh-27: not listed; its item level, as the list's rule
    (12, SCAVENGERS, 3),  // TOZ-122
    (9,  SCAVENGERS, 3),  // oxygen tank
    (67, SCAVENGERS, 3),  // medkit
    (82, SCAVENGERS, 3),  // antitoxin
    (5,  SCAVENGERS, 3),  // RGD-5
    (110, SCAVENGERS, 4), // MP-153
    (48, SCAVENGERS, 4),  // light armoured vest
    (47, SCAVENGERS, 4),  // field pants
    (39, SCAVENGERS, 4),  // military boots
    (14, SCAVENGERS, 5),  // Remington 700
    (19, SCAVENGERS, 5),  // Vityaz: not listed (no longer sold); its item level
    (72, SCAVENGERS, 5),  // 12x70 slug: "Veteran" in the 0.21a notes
    // Black Market: 1 Recruit, 2 Fighter, 3 Foreman, 4 Centurion, 5 Elite.
    (17, BLACK_MARKET, 1), // TOZ-66
    (29, BLACK_MARKET, 1), // leather sleeveless jacket
    (28, BLACK_MARKET, 1), // jeans
    (25, BLACK_MARKET, 1), // biker gloves
    (24, BLACK_MARKET, 1), // cossacks
    (68, BLACK_MARKET, 1), // trap
    (55, BLACK_MARKET, 2), // Uzi
    (90, BLACK_MARKET, 3), // Winchester 1894
    (56, BLACK_MARKET, 3), // Colt Python
    (27, BLACK_MARKET, 3), // biker's helmet
    (32, BLACK_MARKET, 3), // thug's jacket
    (44, BLACK_MARKET, 3), // reinforced jeans
    (65, BLACK_MARKET, 3), // anesthetic
    (5,  BLACK_MARKET, 3), // RGD-5
    (15, BLACK_MARKET, 4), // Remington 870
    (35, BLACK_MARKET, 4), // ropers
    (13, BLACK_MARKET, 5), // AKS-74U
    (33, BLACK_MARKET, 5), // "Corsair" jacket
    (40, BLACK_MARKET, 5), // spiked gloves
    (36, BLACK_MARKET, 5), // choppers
    // Renaissance: 1 Soldier, 2 Lance Corporal.
    (80, RENAISSANCE, 1),  // AKMN (the list's AKM)
    (58, RENAISSANCE, 1),  // army vest
    (59, RENAISSANCE, 1),  // army pants
    (60, RENAISSANCE, 1),  // paratrooper's helmet
    (62, RENAISSANCE, 1),  // army boots
    (124, RENAISSANCE, 2), // SPAS-12
    (64, RENAISSANCE, 2),  // Fort-17
    // Edge (Fringe Settlers): 1 Hunter, 2 Huntsman.
    (109, EDGE, 1),        // SKS
    (83, EDGE, 1),         // hunter's jacket
    (84, EDGE, 1),         // hunter's pants
    (85, EDGE, 1),         // hunter's boots
    (86, EDGE, 1),         // hunter's hood
    (87, EDGE, 1),         // Dar 2M container
    (102, EDGE, 1),        // "Malachite" respirator
    (125, EDGE, 2),        // Benelli M4
    (126, EDGE, 2),        // APS
];

/// The rank with `seller` that buying `dict_id` from it needs.
pub fn rank(dict_id: u16, seller: u8) -> u8 {
    RANKS
        .iter()
        .find(|&&(id, by, _)| id == dict_id && by == seller)
        .map_or(0, |&(_, _, rank)| rank)
}

/// What a faction sells.
pub fn shop(faction: u8) -> impl Iterator<Item = &'static Price> {
    PRICES
        .iter()
        .filter(move |price| price.sellers.contains(&faction))
}

pub fn price(dict_id: u16) -> Option<&'static Price> {
    PRICES.iter().find(|price| price.dict_id == dict_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::Dictionary;

    #[test]
    fn every_price_is_a_021d_item() {
        let dictionary = Dictionary::v021d();
        let mut ids: Vec<u16> = PRICES.iter().map(|price| price.dict_id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), PRICES.len(), "each item once");
        for price in PRICES {
            // The client caps a purchase at the money divided by the price.
            assert!(price.cost > 0);
            assert!(
                dictionary.item(price.dict_id).is_some(),
                "{}",
                price.dict_id
            );
        }
        // The 0.100b ids still name the same items.
        assert_eq!(dictionary.item(13).unwrap().name, "ak_74u");
        assert_eq!(dictionary.item(68).unwrap().category, 11);
        assert_eq!(dictionary.item(66).unwrap().name, "bandages");
    }

    #[test]
    fn every_rank_is_for_a_seller_of_the_item() {
        for &(dict_id, seller, rank) in RANKS {
            let price = price(dict_id).unwrap();
            assert!(price.sellers.contains(&seller), "{dict_id} {seller}");
            let top = if seller <= BLACK_MARKET { 5 } else { 10 };
            assert!((1..=top).contains(&rank), "{dict_id}");
            assert_eq!(
                RANKS
                    .iter()
                    .filter(|&&(id, by, _)| id == dict_id && by == seller)
                    .count(),
                1
            );
        }
        assert_eq!(rank(13, BLACK_MARKET), 5);
        assert_eq!(rank(5, SCAVENGERS), 3);
        assert_eq!(rank(18, SCAVENGERS), 0); // the TT-33
        assert_eq!(rank(50, SCAVENGERS), 0); // ammo
    }

    #[test]
    fn better_ammo_of_a_calibre_costs_more_than_the_common_round() {
        // (the better round, the common one) by dict id: 7.62x51 AP and
        // 7.62x51; 9x19 7N21 and 9x19 para; 12x70 3/0 and 2/0; the slug and
        // the 2/0 buck; and the 9x18 PBM and the Makarov.
        for (better, common) in [(52, 51), (71, 53), (73, 22), (72, 22), (103, 70)] {
            let (better_price, common_price) = (price(better).unwrap(), price(common).unwrap());
            assert!(
                better_price.cost > common_price.cost,
                "{better} costs {}, {common} {}",
                better_price.cost,
                common_price.cost
            );
        }
        // Equal rounds, equal prices: the 7N21 and the PBM have the same
        // damage and penetration.
        assert_eq!(price(71).unwrap().cost, price(103).unwrap().cost);
    }

    /// Not sold, at the user's request: no slot takes them in 0.21d, and
    /// their configs never let them drop (`drop_weight` 0), so no player can
    /// own one.
    const NOT_SOLD: [u16; 4] = [
        69, // Leupold scope
        54, // "Lifebone" artefact
        74, // "Spring" artefact
        76, // "Onyx" artefact
    ];

    #[test]
    fn every_021d_item_but_the_unusable_has_a_price() {
        let dictionary = Dictionary::v021d();
        for item in &dictionary.items {
            assert_eq!(
                price(item.dict_id).is_some(),
                !NOT_SOLD.contains(&item.dict_id),
                "{} {}",
                item.dict_id,
                item.name
            );
        }
    }
}
