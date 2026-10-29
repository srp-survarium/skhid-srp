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
const DECAL0: (u32, u32, u64) = (1603, 603, 0x0102_0304_0506_0708);
const DECAL1: (u32, u32, u64) = (1604, 604, 0x1112_1314_1516_1718);

fn decal(d: (u32, u32, u64), slot: u8) -> Upgrade {
    up(d.0, d.1, d.2, slot)
}

#[test]
fn six_attachments_go_in_three_records() {
    let logical = [
        up(1601, 601, 0, 2),
        decal(DECAL1, 1),
        up(1605, 2002, 0, 3),
        up(1701, 2201, 0, MUZZLE_SLOT),
        decal(DECAL0, 0),
        up(1702, 2301, 0, GRIP_SLOT),
    ];
    let wire = pack(&logical);
    assert_eq!(
        wire,
        [
            decal(DECAL0, 0),
            decal(DECAL1, 1),
            up(
                1605,
                2002,
                MAGIC | 3 << 48 | 2301 << 32 | 2201 << 16 | 601,
                KIT_SLOT
            ),
        ]
    );
    assert!(wire.len() <= crate::v026g::lobby::limits::UPGRADES);
    assert!(is_kit(&wire[2]) && !is_kit(&wire[0]));
    // The same as check_weapon_kit_026g.py's literal case and the C pack.
    let back = unpack(&wire);
    assert_eq!(
        back,
        [
            decal(DECAL0, 0),
            decal(DECAL1, 1),
            up(1605, 2002, 0, 3),
            up(0, 601, 0, 2),
            up(0, 2201, 0, MUZZLE_SLOT),
            up(0, 2301, 0, GRIP_SLOT),
        ]
    );
}

#[test]
fn without_a_scope_the_camouflage_is_the_kits_own() {
    let wire = pack(&[decal(DECAL0, 0), up(1601, 601, 0, 2)]);
    assert_eq!(
        wire,
        [decal(DECAL0, 0), up(1601, 601, MAGIC | 2 << 48, KIT_SLOT)]
    );
    assert_eq!(unpack(&wire), [decal(DECAL0, 0), up(1601, 601, 0, 2)]);
    // a scope alone: its record, marked
    assert_eq!(
        pack(&[up(1605, 2002, 0, 3)]),
        [up(1605, 2002, MAGIC | 3 << 48, KIT_SLOT)]
    );
    // a muzzle alone is the kit's own too
    assert_eq!(
        pack(&[up(1701, 2201, 0, MUZZLE_SLOT)]),
        [up(1701, 2201, MAGIC | 4 << 48, KIT_SLOT)]
    );
    assert!(pack(&[]).is_empty());
    assert_eq!(pack(&[decal(DECAL1, 1)]), [decal(DECAL1, 1)]);
}

#[test]
fn records_that_are_no_kit_unpack_as_they_are() {
    // an armour's camouflage in slot 3 (value 0), a value with another top byte
    let plain = [
        up(1601, 601, 0, 3),
        up(1605, 2002, 0x5A07 << 48, 3),
        up(1601, 601, MAGIC | 2 << 48, 2),
    ];
    assert_eq!(unpack(&plain), plain);
    assert!(plain.iter().all(|record| !is_kit(record)));
    // a kit naming its own slot's field: the field is not a record of its own
    let own_named = up(1601, 601, MAGIC | 2 << 48 | 602, KIT_SLOT);
    assert_eq!(unpack(&[own_named]), [up(1601, 601, 0, 2)]);
}
