use super::*;

fn up(id: u32, a: u32, value: u64, param: u8) -> Upgrade {
    Upgrade {
        id,
        a,
        value,
        param,
    }
}

const MAGIC: u64 = (KIT_MAGIC as u64) << 56;
const DECAL0: Upgrade = Upgrade {
    id: 1603,
    a: 603,
    value: 0x0102_0304_0506_0708,
    param: 0,
};
const DECAL1: Upgrade = Upgrade {
    id: 1604,
    a: 604,
    value: 0x1112_1314_1516_1718,
    param: 1,
};
const CAMO: Upgrade = Upgrade {
    id: 1601,
    a: 601,
    value: 0,
    param: 2,
};
const SCOPE: Upgrade = Upgrade {
    id: 1605,
    a: 2002,
    value: 0,
    param: 3,
};

/// Every combination of a camouflage, a scope and two decals (as the
/// account holds them, in any order) goes in at most three records and
/// comes back in its logical slots: what step A sends.
#[test]
fn every_combination_of_camo_scope_and_two_decals_comes_back() {
    let all = [SCOPE, DECAL1, CAMO, DECAL0];
    for bits in 0u32..16 {
        let logical: Vec<Upgrade> = all
            .iter()
            .enumerate()
            .filter(|(i, _)| bits >> i & 1 == 1)
            .map(|(_, held)| *held)
            .collect();
        let wire = pack(&logical);
        assert!(wire.len() <= crate::v026g::lobby::limits::UPGRADES);
        // the decals first, as they are
        let decals: Vec<Upgrade> = [DECAL0, DECAL1]
            .into_iter()
            .filter(|decal| logical.contains(decal))
            .collect();
        assert_eq!(wire[..decals.len()], decals[..]);
        // the kit: the scope's record when there is one, else the camouflage's
        let own = [SCOPE, CAMO].into_iter().find(|own| logical.contains(own));
        let kit = wire.get(decals.len());
        assert_eq!(kit.map(|kit| (kit.id, kit.a, kit.param)), own.map(|own| (own.id, own.a, 3)));
        assert_eq!(kit.is_some_and(is_kit), own.is_some());
        // back in logical slots: the scope's own, the camouflage named by
        // its dict id when the scope is the kit's own
        let mut back = logical.clone();
        back.sort_by_key(|held| held.param);
        if own == Some(SCOPE) && logical.contains(&CAMO) {
            back.iter_mut()
                .filter(|held| held.param == 2)
                .for_each(|camo| camo.id = 0);
        }
        assert_eq!(unpack(&wire), Ok(back), "{logical:?} -> {wire:?}");
    }
}

#[test]
fn six_upgrades_go_in_three_records() {
    let muzzle = up(1701, 2201, 0, MUZZLE_SLOT);
    let grip = up(1702, 2301, 0, GRIP_SLOT);
    let wire = pack(&[CAMO, DECAL1, SCOPE, muzzle, DECAL0, grip]);
    assert_eq!(
        wire,
        [
            DECAL0,
            DECAL1,
            up(1605, 2002, MAGIC | 3 << 48 | 2301 << 32 | 2201 << 16 | 601, KIT_SLOT),
        ]
    );
    assert_eq!(
        unpack(&wire),
        Ok(vec![
            DECAL0,
            DECAL1,
            up(0, 601, 0, 2),
            SCOPE,
            up(0, 2201, 0, MUZZLE_SLOT),
            up(0, 2301, 0, GRIP_SLOT),
        ])
    );
}

#[test]
fn a_malformed_kit_is_refused() {
    let kit = |value: u64| up(1605, 2002, MAGIC | value, KIT_SLOT);
    for (wire, why) in [
        (vec![kit(7 << 48)], Malformed::OwnSlot(7)),
        (vec![kit(3 << 48 | 2002)], Malformed::DictTwice(2002)),
        (vec![kit(2 << 48 | 601)], Malformed::NamesItsOwn(weapon_kit::KitSlot::Camo)),
        (vec![CAMO, kit(3 << 48 | 609)], Malformed::SlotTwice(weapon_kit::Slot::Camo)),
        (vec![kit(3 << 48), kit(2 << 48)], Malformed::TwoKits),
        (vec![DECAL0, DECAL1, CAMO, SCOPE], Malformed::TooMany),
    ] {
        assert_eq!(unpack(&wire), Err(why), "{wire:?}");
    }
    assert!(!is_kit(&kit(7 << 48)));
    // an armour's camouflage in slot 3 (value 0) is no kit
    assert!(!is_kit(&up(1601, 601, 0, 3)));
}

#[test]
fn the_accounts_extra_records_are_not_sent() {
    // a second record of a slot, a record of no weapon slot
    let wire = pack(&[DECAL0, up(1606, 606, 0, 0), up(1607, 607, 0, 9)]);
    assert_eq!(wire, [DECAL0]);
    assert!(pack(&[]).is_empty());
}
