use super::*;

const DONOR: Place = Place(0);
const FIRST: Place = Place(1);
const SECOND: Place = Place(2);
const ADAPTER: ActiveObjectId = ActiveObjectId(120);
const WEAPON: ActiveObjectId = ActiveObjectId(9);
const ITEM: Artefact = Artefact {
    id: ArtefactId(7),
    slot: InventorySlot(20),
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Inventories {
    items: [Option<ArtefactId>; 3],
    containers: [bool; 3],
    calls: usize,
}

impl Inventories {
    fn new() -> Self {
        Self {
            items: [Some(ITEM.id), None, None],
            containers: [true; 3],
            calls: 0,
        }
    }
}

impl Transfer for Inventories {
    fn take(&mut self, donor: Place, recipient: Place, artefact: Artefact) -> Result<(), Refused> {
        self.calls += 1;
        let from = donor.get() as usize;
        let to = recipient.get() as usize;
        if self.items[from] != Some(artefact.id) {
            return Err(Refused::OwnershipChanged);
        }
        if !self.containers[to] || self.items[to].is_some() {
            return Err(Refused::NoFreeContainer);
        }
        self.items[to] = self.items[from].take();
        Ok(())
    }
}

fn equip(adapter: &mut Adapter, inventories: &mut Inventories) {
    assert_eq!(
        adapter.step(
            Event::Equip {
                artefact: ITEM,
                previous: WEAPON
            },
            inventories
        ),
        Outcome::action(Action::EquipAdapter)
    );
}

fn offer(adapter: &mut Adapter, inventories: &mut Inventories) -> Offer {
    equip(adapter, inventories);
    assert_eq!(
        adapter.step(Event::Equipped, inventories),
        Outcome::action(Action::Present(ITEM))
    );
    adapter.offer().unwrap()
}

#[test]
fn offer_only_appears_after_weapon_hide_finishes() {
    let mut adapter = Adapter::registered(DONOR, ADAPTER);
    let mut inventories = Inventories::new();
    equip(&mut adapter, &mut inventories);
    assert_eq!(adapter.offer(), None);
    adapter.step(Event::Equipped, &mut inventories);
    assert!(adapter.offer().is_some());
    assert_eq!(inventories.items, [Some(ITEM.id), None, None]);
    assert_eq!(inventories.calls, 0);
}

#[test]
fn two_takers_cannot_duplicate_one_artefact() {
    let mut adapter = Adapter::registered(DONOR, ADAPTER);
    let mut inventories = Inventories::new();
    let token = offer(&mut adapter, &mut inventories);
    assert_eq!(
        adapter.step(
            Event::Take {
                offer: token,
                recipient: FIRST
            },
            &mut inventories
        ),
        Outcome::action(Action::Restore(WEAPON))
    );
    assert_eq!(adapter.offer(), None);
    assert_eq!(
        adapter.step(
            Event::Take {
                offer: token,
                recipient: SECOND
            },
            &mut inventories
        ),
        Outcome::refused(Refused::NotOffering)
    );
    assert_eq!(inventories.items, [None, Some(ITEM.id), None]);
    assert_eq!(inventories.calls, 1);
}

#[test]
fn full_or_missing_container_leaves_ownership_and_offer_unchanged() {
    for occupied in [true, false] {
        let mut adapter = Adapter::registered(DONOR, ADAPTER);
        let mut inventories = Inventories::new();
        let token = offer(&mut adapter, &mut inventories);
        if occupied {
            inventories.items[1] = Some(ArtefactId(8));
        } else {
            inventories.containers[1] = false;
        }
        let before = inventories.items;
        assert_eq!(
            adapter.step(
                Event::Take {
                    offer: token,
                    recipient: FIRST
                },
                &mut inventories
            ),
            Outcome::refused(Refused::NoFreeContainer)
        );
        assert_eq!(inventories.items, before);
        assert_eq!(adapter.offer(), Some(token));
    }
}

#[test]
fn cancelled_offer_token_cannot_take_a_later_offer() {
    let mut adapter = Adapter::registered(DONOR, ADAPTER);
    let mut inventories = Inventories::new();
    let old = offer(&mut adapter, &mut inventories);
    adapter.step(Event::Cancel, &mut inventories);
    adapter.step(Event::Unequipped, &mut inventories);
    let new = offer(&mut adapter, &mut inventories);
    assert_ne!(new, old);
    assert_eq!(
        adapter.step(
            Event::Take {
                offer: old,
                recipient: FIRST
            },
            &mut inventories
        ),
        Outcome::refused(Refused::StaleOffer)
    );
    assert_eq!(adapter.offer(), Some(new));
    assert_eq!(inventories.calls, 0);
}

#[test]
fn cancellation_during_equip_never_presents_or_transfers() {
    let mut adapter = Adapter::registered(DONOR, ADAPTER);
    let mut inventories = Inventories::new();
    equip(&mut adapter, &mut inventories);
    assert_eq!(
        adapter.step(Event::Cancel, &mut inventories),
        Outcome::action(Action::Restore(WEAPON))
    );
    assert_eq!(
        adapter.step(Event::Equipped, &mut inventories),
        Outcome::action(Action::None)
    );
    assert_eq!(adapter.offer(), None);
    adapter.step(Event::Unequipped, &mut inventories);
    assert_eq!(adapter.state(), State::Stowed);
    assert_eq!(inventories.items[0], Some(ITEM.id));
}

#[test]
fn owner_death_or_disconnect_clears_each_phase_without_transferring() {
    let mut adapter = Adapter::registered(DONOR, ADAPTER);
    let mut inventories = Inventories::new();
    let stowed = adapter;
    equip(&mut adapter, &mut inventories);
    let equipping = adapter;
    adapter.step(Event::Equipped, &mut inventories);
    let offering = adapter;
    adapter.step(Event::Cancel, &mut inventories);
    for mut phase in [stowed, equipping, offering, adapter] {
        phase.step(Event::OwnerGone, &mut inventories);
        assert_eq!(phase.state(), State::Stowed);
        assert_eq!(phase.offer(), None);
    }
    assert_eq!(inventories.calls, 0);
}

#[test]
fn stale_inventory_and_self_take_are_refused() {
    let mut adapter = Adapter::registered(DONOR, ADAPTER);
    let mut inventories = Inventories::new();
    let token = offer(&mut adapter, &mut inventories);
    assert_eq!(
        adapter.step(
            Event::Take {
                offer: token,
                recipient: DONOR
            },
            &mut inventories
        ),
        Outcome::refused(Refused::SamePlayer)
    );
    assert_eq!(inventories.calls, 0);
    inventories.items[0] = None;
    assert_eq!(
        adapter.step(
            Event::Take {
                offer: token,
                recipient: FIRST
            },
            &mut inventories
        ),
        Outcome::refused(Refused::OwnershipChanged)
    );
    assert_eq!(inventories.items, [None; 3]);
}

#[test]
fn snapshot_roundtrip_each_phase_and_replay_after_rollback() {
    let mut adapter = Adapter::registered(DONOR, ADAPTER);
    let mut inventories = Inventories::new();
    let stowed = adapter;
    equip(&mut adapter, &mut inventories);
    let equipping = adapter;
    adapter.step(Event::Equipped, &mut inventories);
    let offering = adapter;
    let token = adapter.offer().unwrap();
    adapter.step(
        Event::Take {
            offer: token,
            recipient: FIRST,
        },
        &mut inventories,
    );
    for expected in [stowed, equipping, offering, adapter] {
        let mut restored = Adapter::registered(DONOR, ADAPTER);
        restored.restore(&expected.snapshot()).unwrap();
        assert_eq!(restored, expected);
    }
    let first_result = adapter;
    let first_inventory = inventories.clone();
    adapter.restore(&offering.snapshot()).unwrap();
    inventories = Inventories::new();
    adapter.step(
        Event::Take {
            offer: token,
            recipient: FIRST,
        },
        &mut inventories,
    );
    assert_eq!(adapter, first_result);
    assert_eq!(inventories, first_inventory);
}

#[test]
fn malformed_snapshot_is_rejected_without_mutation() {
    let mut adapter = Adapter::registered(DONOR, ADAPTER);
    let mut inventories = Inventories::new();
    offer(&mut adapter, &mut inventories);
    let before = adapter;
    let good = adapter.snapshot();
    for size in 0..9 {
        assert_eq!(adapter.restore(&good[..size]), Err(InvalidSnapshot));
        assert_eq!(adapter, before);
    }
    for (offset, value) in [
        (0, 2),
        (5, 9),
        (6, 255),
        (7, 23),
        (8, 255),
        (8, ADAPTER.get()),
    ] {
        let mut bad = good;
        bad[offset] = value;
        assert_eq!(adapter.restore(&bad), Err(InvalidSnapshot));
        assert_eq!(adapter, before);
    }
    let mut bad = Adapter::registered(DONOR, ADAPTER).snapshot();
    bad[6] = 1;
    assert_eq!(adapter.restore(&bad), Err(InvalidSnapshot));
}

#[test]
fn adapter_cannot_select_itself_as_the_previous_weapon() {
    let mut adapter = Adapter::registered(DONOR, ADAPTER);
    let mut inventories = Inventories::new();
    assert_eq!(
        adapter.step(
            Event::Equip {
                artefact: ITEM,
                previous: ADAPTER
            },
            &mut inventories
        ),
        Outcome::action(Action::None)
    );
    assert_eq!(adapter.state(), State::Stowed);
}
