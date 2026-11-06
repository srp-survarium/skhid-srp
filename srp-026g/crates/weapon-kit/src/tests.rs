use super::*;
use std::vec::Vec;

const MAGIC: u64 = (KIT_MAGIC as u64) << 56;

/// What each slot holds in the tests: an item of the account.
fn item(slot: Slot) -> Held {
    let n = u32::from(slot.byte());
    Held::Item {
        id: 1600 + n,
        dict: 600 + n,
        // a decal's place; the others' value is 0
        value: if n < 2 {
            0x0102_0304_0506_0700 | u64::from(n)
        } else {
            0
        },
    }
}

fn record(id: u32, dict: u32, value: u64, slot: u8) -> Record {
    Record {
        id,
        dict,
        value,
        slot,
    }
}

/// Every combination of the six slots (a bit a slot).
fn combinations() -> impl Iterator<Item = Logical> {
    (0u32..64).map(|bits| {
        let mut logical = Logical::new();
        for slot in Slot::ALL {
            if bits >> slot.byte() & 1 == 1 {
                logical.set(slot, Some(item(slot)));
            }
        }
        logical
    })
}

/// What unpack gives back for what pack sent: the kit's own an item of
/// value 0 (its record's value is the kit), the others it names by dict id.
fn as_received(logical: &Logical) -> Logical {
    let own = KitSlot::ALL
        .into_iter()
        .find(|kit| logical.get(kit.slot()).is_some());
    let mut back = Logical::new();
    for (slot, held) in logical.iter() {
        let kit = KitSlot::ALL.into_iter().find(|kit| kit.slot() == slot);
        back.set(
            slot,
            Some(match (kit, own) {
                (None, _) => held,
                (Some(kit), Some(own)) if kit == own => match held {
                    Held::Item { id, dict, .. } => Held::Item { id, dict, value: 0 },
                    Held::Named(_) => unreachable!(),
                },
                _ => Held::Named(DictId::new(held.dict() as u16).unwrap()),
            }),
        );
    }
    back
}

#[test]
fn every_combination_goes_in_three_records_and_comes_back() {
    for logical in combinations() {
        let wire = logical.pack();
        let records = wire.as_slice();
        assert!(records.len() <= RECORDS, "{logical:?}: {records:?}");
        // the decals first, in slot order, as they are
        let decals: Vec<Record> = [Slot::Decal0, Slot::Decal1]
            .into_iter()
            .filter_map(|slot| logical.get(slot).map(|held| record_of(held, slot.byte())))
            .collect();
        assert_eq!(&records[..decals.len()], &decals[..]);
        // a kit whenever a camouflage, scope, muzzle or grip is held
        let kits = records
            .iter()
            .filter(|r| Kit::of(r).unwrap().is_some())
            .count();
        let covered = KitSlot::ALL
            .iter()
            .any(|kit| logical.get(kit.slot()).is_some());
        assert_eq!(kits, usize::from(covered));
        assert_eq!(
            Logical::unpack(records),
            Ok(as_received(&logical)),
            "{logical:?}"
        );
        // packed again (the preview's apply packs what it unpacked): the same records
        assert_eq!(Logical::unpack(records).unwrap().pack().as_slice(), records);
    }
}

#[test]
fn the_kit_is_the_scope_naming_the_others() {
    let mut logical = Logical::new();
    for slot in Slot::ALL {
        logical.set(slot, Some(item(slot)));
    }
    assert_eq!(
        logical.pack().as_slice(),
        [
            record(1600, 600, 0x0102_0304_0506_0700, 0),
            record(1601, 601, 0x0102_0304_0506_0701, 1),
            record(1603, 603, MAGIC | 3 << 48 | 605 << 32 | 604 << 16 | 602, 3),
        ]
    );
    // without a scope the camouflage is the kit's own
    logical.set(Slot::Scope, None);
    assert_eq!(
        logical.pack().as_slice()[2],
        record(1602, 602, MAGIC | 2 << 48 | 605 << 32 | 604 << 16, 3)
    );
    // the camouflage alone: a kit naming nothing
    let mut camo = Logical::new();
    camo.set(Slot::Camo, Some(item(Slot::Camo)));
    assert_eq!(
        camo.pack().as_slice(),
        [record(1602, 602, MAGIC | 2 << 48, 3)]
    );
}

#[test]
fn a_malformed_kit_is_refused() {
    let kit = |value: u64| record(1603, 603, MAGIC | value, 3);
    let refused = [
        (std::vec![kit(0)], Malformed::OwnSlot(0)),
        (std::vec![kit(1 << 48)], Malformed::OwnSlot(1)),
        (std::vec![kit(6 << 48)], Malformed::OwnSlot(6)),
        (std::vec![kit(0xFF << 48)], Malformed::OwnSlot(0xFF)),
        // naming its own slot
        (
            std::vec![kit(2 << 48 | 602)],
            Malformed::NamesItsOwn(KitSlot::Camo),
        ),
        (
            std::vec![kit(4 << 48 | 604 << 16)],
            Malformed::NamesItsOwn(KitSlot::Muzzle),
        ),
        (
            std::vec![kit(5 << 48 | 605 << 32)],
            Malformed::NamesItsOwn(KitSlot::Grip),
        ),
        // a plain record in a slot the kit names, or the own's
        (
            std::vec![record(1602, 602, 0, 2), kit(3 << 48 | 609)],
            Malformed::SlotTwice(Slot::Camo),
        ),
        (
            std::vec![kit(2 << 48), record(1602, 602, 0, 2)],
            Malformed::SlotTwice(Slot::Camo),
        ),
        // one dict id twice: the config's duplicate modification
        (std::vec![kit(3 << 48 | 603)], Malformed::DictTwice(603)),
        (
            std::vec![record(1600, 609, 0, 0), kit(3 << 48 | 609)],
            Malformed::DictTwice(609),
        ),
        // the same decal twice is no kit's concern: taken
        (
            std::vec![
                record(1600, 600, 1, 0),
                record(1601, 600, 2, 1),
                record(1602, 602, 1 << 60, 7)
            ],
            Malformed::NoSlot(7),
        ),
        // a record of no weapon slot; more than three
        (std::vec![record(1607, 607, 0, 7)], Malformed::NoSlot(7)),
        (
            std::vec![
                record(1, 1, 0, 0),
                record(2, 2, 0, 1),
                record(3, 3, 0, 2),
                record(4, 4, 0, 4)
            ],
            Malformed::TooMany,
        ),
    ];
    for (records, why) in refused {
        assert_eq!(Logical::unpack(&records), Err(why), "{records:?}");
        assert_eq!(
            expand(&records, |_| true).map(|_| ()),
            if matches!(why, Malformed::NoSlot(_)) {
                Ok(())
            } else {
                Err(why)
            },
            "{records:?}"
        );
    }
    // two kits: two records of slot 3 with the magic
    let two = [kit(3 << 48), kit(2 << 48)];
    assert_eq!(expand(&two, |_| true), Err(Malformed::TwoKits));
    assert_eq!(Logical::unpack(&two), Err(Malformed::TwoKits));
}

#[test]
fn only_slot_three_with_the_magic_is_a_kit() {
    // a decal's place whose top byte is the magic is a plain record
    let decal = record(1600, 600, MAGIC | 0xFF << 48, 0);
    assert_eq!(Kit::of(&decal), Ok(None));
    // armour's camouflage in slot 3, value 0: plain
    assert_eq!(Kit::of(&record(1, 2, 0, 3)), Ok(None));
    assert_eq!(expand(&[decal, record(1, 2, 0, 3)], |_| true), Ok(None));
}

#[test]
fn the_cook_gets_the_records_and_what_the_kit_names() {
    let mut logical = Logical::new();
    for slot in [Slot::Decal0, Slot::Decal1, Slot::Camo, Slot::Scope] {
        logical.set(slot, Some(item(slot)));
    }
    let wire = logical.pack();
    let expanded = expand(wire.as_slice(), |_| true).unwrap().unwrap();
    // as the records were before the kit: the scope valued 0, the camouflage a record of its own
    assert_eq!(
        expanded.as_slice(),
        [
            record(1600, 600, 0x0102_0304_0506_0700, 0),
            record(1601, 601, 0x0102_0304_0506_0701, 1),
            record(1603, 603, 0, 3),
            record(0, 602, 0, 2),
        ]
    );
    // a dict id the game data lacks is left out
    let lacking = expand(wire.as_slice(), |dict| dict != 602)
        .unwrap()
        .unwrap();
    assert_eq!(lacking.as_slice().len(), 3);
    // every combination: at most EXPANDED, each held dict id once
    for logical in combinations() {
        let wire = logical.pack();
        match expand(wire.as_slice(), |_| true).unwrap() {
            None => assert!(
                KitSlot::ALL
                    .iter()
                    .all(|kit| logical.get(kit.slot()).is_none())
            ),
            Some(expanded) => {
                let mut dicts: Vec<u32> = expanded.as_slice().iter().map(|r| r.dict).collect();
                let mut held: Vec<u32> = logical.iter().map(|(_, held)| held.dict()).collect();
                dicts.sort();
                held.sort();
                assert_eq!(dicts, held);
                assert!(expanded.as_slice().iter().all(|r| Kit::of(r) == Ok(None)));
            }
        }
    }
}

#[test]
fn put_and_take_off_as_the_server() {
    let mut logical = Logical::unpack(Logical::default().pack().as_slice()).unwrap();
    // a camouflage, then a scope: the scope becomes the kit's own, the camouflage named
    logical.put(Slot::Camo, 1602, 602);
    logical.put(Slot::Scope, 1603, 603);
    let wire = logical.pack();
    assert_eq!(
        wire.as_slice(),
        [record(1603, 603, MAGIC | 3 << 48 | 602, 3)]
    );
    // received: the camouflage is a name; putting the same camouflage there again changes nothing
    let mut received = Logical::unpack(wire.as_slice()).unwrap();
    let before = received;
    received.put(Slot::Camo, 1602, 602);
    assert_eq!(received, before);
    // another camouflage takes its place
    received.put(Slot::Camo, 1612, 612);
    assert_eq!(
        received.pack().as_slice(),
        [record(1603, 603, MAGIC | 3 << 48 | 612, 3)]
    );
    // a decal moved from 0 to 1 keeps its place
    received.put(Slot::Decal0, 1600, 600);
    received.set(
        Slot::Decal0,
        Some(Held::Item {
            id: 1600,
            dict: 600,
            value: 0x55,
        }),
    );
    received.put(Slot::Decal1, 1600, 600);
    assert_eq!(received.get(Slot::Decal0), None);
    assert_eq!(
        received.get(Slot::Decal1),
        Some(Held::Item {
            id: 1600,
            dict: 600,
            value: 0x55
        })
    );
    // the scope off: the camouflage put since (an item now) is the kit's own
    received.take_off(1603, 603);
    assert_eq!(
        received.pack().as_slice(),
        [
            record(1600, 600, 0x55, 1),
            record(1612, 612, MAGIC | 2 << 48, 3)
        ]
    );
    // a name comes off by its dict id
    received.take_off(1612, 612);
    assert_eq!(received.pack().as_slice(), [record(1600, 600, 0x55, 1)]);
}

#[test]
fn the_same_decal_twice_is_a_weapons() {
    let records = [
        record(1600, 600, 1, 0),
        record(1601, 600, 2, 1),
        record(1603, 603, MAGIC | 3 << 48 | 602, 3),
    ];
    let logical = Logical::unpack(&records).unwrap();
    assert_eq!(logical.pack().as_slice(), records);
    let expanded = expand(&records, |_| true).unwrap().unwrap();
    assert_eq!(expanded.as_slice().len(), 4);
}
