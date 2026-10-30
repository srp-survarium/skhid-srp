//! Pointer-free ownership in the shared simulation. The native adapter must
//! serialize this with the world and restore it before native inventories.
//! It must apply events in simulation order, never packet arrival order.
//!
//! An unclaimed weapon exists for the life of its corpse. A claimed weapon
//! belongs to its recipient's current life independently of donor respawn.
//! Carried ammunition stays in the native weapon's serialized state; this
//! module owns only ammunition captured in an unclaimed drop.

use crate::{DroppedAmmo, Handedness, Loadout, PickupGrant, Refused, WeaponSlot};

/// Native places fit the ragdoll's 32-place table. The network boundary must
/// additionally check membership in the actual match roster.
pub const MAX_PLACES: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place(u8);

impl Place {
    pub fn parse(raw: u8) -> Option<Self> {
        (usize::from(raw) < MAX_PLACES).then_some(Self(raw))
    }

    pub const fn get(self) -> u8 {
        self.0
    }

    pub(super) const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kit(u8);

impl Kit {
    pub fn parse(raw: u8) -> Option<Self> {
        (raw < 3).then_some(Self(raw))
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

/// The native spawn time distinguishes repeated deaths in one roster place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Life {
    pub place: Place,
    pub spawned: u32,
}

/// One held weapon may drop per death. No address or cosmetic pose is an ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DropId(pub Life);

/// A weapon's original match kit, including that item's attachments. Native
/// instances made from this template must not alias the original spawn item.
/// The source life is part of the key: a roster place can be reused, and its
/// current occupant/profile must never replace a carried weapon's template.
/// Capture the profile for this life before publishing any drop from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Template {
    pub source: Life,
    pub kit: Kit,
    pub slot: WeaponSlot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Borrowed {
    pub drop: DropId,
    pub template: Template,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Drop {
    pub id: DropId,
    pub template: Template,
    pub handedness: Handedness,
    pub ammo: DroppedAmmo,
}

/// Native facts captured once when the held weapon's owner dies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeldAtDeath {
    pub slot: WeaponSlot,
    pub handedness: Handedness,
    pub ammo: DroppedAmmo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Player {
    Absent,
    Alive {
        spawned: u32,
        kit: Kit,
        borrowed: [Option<Borrowed>; 2],
    },
    Dead {
        spawned: u32,
        drop: Option<Drop>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct World {
    pub(super) players: [Player; MAX_PLACES],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickupError {
    PickerNotAlive,
    DropUnavailable,
    Loadout(Refused),
}

/// Execute this change in the native inventory at the same simulation event.
/// Native resources must be ready before dispatching the pickup event: an
/// asynchronous cook cannot decide whether one simulator accepts the pickup.
#[must_use = "apply the accepted transfer to the native inventory"]
#[derive(Debug, PartialEq, Eq)]
pub struct Transfer {
    pub recipient: Life,
    pub acquired: Borrowed,
    pub grant: PickupGrant,
    pub replaced: Option<Borrowed>,
}

impl World {
    pub const fn new() -> Self {
        Self {
            players: [Player::Absent; MAX_PLACES],
        }
    }

    /// After native respawn setup from the ORIGINAL kit. Re-observing the
    /// same spawn does nothing: it must not remove a weapon already picked up.
    /// Returns whether this is a new life requiring native borrowed cleanup.
    pub fn spawn(&mut self, life: Life, kit: Kit) -> bool {
        let player = &mut self.players[life.place.index()];
        if matches!(player, Player::Alive { spawned, .. } | Player::Dead { spawned, .. }
            if *spawned == life.spawned)
        {
            return false;
        }
        *player = Player::Alive {
            spawned: life.spawned,
            kit,
            borrowed: [None; 2],
        };
        true
    }

    /// Called once per native alive -> dead transition. Duplicate or stale
    /// death observations cannot recreate an already claimed weapon.
    pub fn die(&mut self, life: Life, held: Option<HeldAtDeath>) -> bool {
        let player = &mut self.players[life.place.index()];
        let Player::Alive {
            spawned,
            kit,
            borrowed,
        } = *player
        else {
            return false;
        };
        if spawned != life.spawned {
            return false;
        }
        let drop = held.map(|held| Drop {
            id: DropId(life),
            template: borrowed[held.slot.index()].map_or(
                Template {
                    source: life,
                    kit,
                    slot: held.slot,
                },
                |item| item.template,
            ),
            handedness: held.handedness,
            ammo: held.ammo,
        });
        *player = Player::Dead { spawned, drop };
        true
    }

    pub fn leave(&mut self, place: Place) {
        self.players[place.index()] = Player::Absent;
    }

    pub fn drop(&self, id: DropId) -> Option<Drop> {
        match self.players[id.0.place.index()] {
            Player::Dead { spawned, drop } if spawned == id.0.spawned => drop,
            _ => None,
        }
    }

    pub fn borrowed(&self, life: Life, slot: WeaponSlot) -> Option<Borrowed> {
        match self.players[life.place.index()] {
            Player::Alive {
                spawned, borrowed, ..
            } if spawned == life.spawned => borrowed[slot.index()],
            _ => None,
        }
    }

    /// Resolve a targeted pickup. All refusals precede any state change.
    /// The caller supplies native inventory/skill facts for this exact event.
    /// It does not supply a position, ray, bounding box, or distance verdict.
    pub fn pickup(
        &mut self,
        recipient: Life,
        id: DropId,
        loadout: Loadout,
    ) -> Result<Transfer, PickupError> {
        if !matches!(self.players[recipient.place.index()], Player::Alive { spawned, .. }
            if spawned == recipient.spawned)
        {
            return Err(PickupError::PickerNotAlive);
        }
        let drop = self.drop(id).ok_or(PickupError::DropUnavailable)?;
        let grant = PickupGrant::check(loadout, drop.handedness, drop.ammo)
            .map_err(PickupError::Loadout)?;
        let acquired = Borrowed {
            drop: id,
            template: drop.template,
        };
        // A live recipient cannot also be this dead donor. Both entries were
        // validated above, so mutation cannot fail halfway through.
        self.players[id.0.place.index()] = Player::Dead {
            spawned: id.0.spawned,
            drop: None,
        };
        let Player::Alive { borrowed, .. } = &mut self.players[recipient.place.index()] else {
            unreachable!("a live recipient cannot be its dead donor");
        };
        let replaced = borrowed[grant.slot().index()].replace(acquired);
        Ok(Transfer {
            recipient,
            acquired,
            grant,
            replaced,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CarryRule, Chamber};

    fn life(place: u8, spawned: u32) -> Life {
        Life {
            place: Place::parse(place).unwrap(),
            spawned,
        }
    }

    fn kit() -> Kit {
        Kit::parse(0).unwrap()
    }

    fn held(magazine: u8) -> HeldAtDeath {
        HeldAtDeath {
            slot: WeaponSlot::First,
            handedness: Handedness::Two,
            ammo: DroppedAmmo::parse(
                30,
                magazine,
                Chamber::Loaded,
                crate::AmmoKind::parse(1).unwrap(),
            )
            .unwrap(),
        }
    }

    fn loadout() -> Loadout {
        Loadout::new(
            [Some(Handedness::Two), Some(Handedness::One)],
            Some(WeaponSlot::First),
            CarryRule::OneDoubleHanded,
        )
    }

    fn fixture() -> World {
        let mut world = World::new();
        for place in 0..3 {
            assert!(world.spawn(life(place, 100), kit()));
        }
        assert!(world.die(life(0, 100), Some(held(17))));
        world
    }

    #[test]
    fn two_simulators_resolve_contested_pickup_in_event_order() {
        let mut a = fixture();
        let mut b = a.clone();
        for world in [&mut a, &mut b] {
            let picked = world
                .pickup(life(1, 100), DropId(life(0, 100)), loadout())
                .unwrap();
            assert_eq!(picked.grant.ammo().magazine(), 17);
            assert_eq!(picked.grant.ammo().pickup_reserve(), 30);
            assert_eq!(
                world.pickup(life(2, 100), DropId(life(0, 100)), loadout()),
                Err(PickupError::DropUnavailable)
            );
        }
        assert_eq!(a, b);
    }

    #[test]
    fn donor_respawn_does_not_reclaim_a_borrowed_weapon() {
        let mut world = fixture();
        let transfer = world
            .pickup(life(1, 100), DropId(life(0, 100)), loadout())
            .unwrap();
        assert!(world.spawn(life(0, 200), kit()));
        assert_eq!(
            world.borrowed(life(1, 100), WeaponSlot::First),
            Some(transfer.acquired)
        );
        assert_eq!(world.borrowed(life(0, 200), WeaponSlot::First), None);
    }

    #[test]
    fn a_reused_roster_place_cannot_replace_a_carried_weapons_source_template() {
        let mut world = fixture();
        let carried = world
            .pickup(life(1, 100), DropId(life(0, 100)), loadout())
            .unwrap();
        world.leave(Place::parse(0).unwrap());
        // Same roster place, kit and held slot, but a new native life/profile.
        world.spawn(life(0, 300), kit());
        world.die(life(0, 300), Some(held(21)));
        let replacement = world.drop(DropId(life(0, 300))).unwrap();
        assert_ne!(replacement.template, carried.acquired.template);
        assert_eq!(replacement.template.source, life(0, 300));
        assert_eq!(carried.acquired.template.source, life(0, 100));
        assert_eq!(
            world.borrowed(life(1, 100), WeaponSlot::First),
            Some(carried.acquired)
        );
        // Passing the borrowed weapon on changes the drop identity, never
        // the source of its original attachments, appearance or allowance.
        world.die(life(1, 100), Some(held(2)));
        let passed = world
            .pickup(life(2, 100), DropId(life(1, 100)), loadout())
            .unwrap();
        assert_eq!(passed.acquired.template, carried.acquired.template);
        assert_ne!(passed.acquired.drop, carried.acquired.drop);
        let restored = World::from_snapshot(world.snapshot().bytes()).unwrap();
        assert_eq!(restored, world);
        assert_eq!(
            restored.drop(replacement.id).unwrap().template.source,
            life(0, 300)
        );
        assert_eq!(
            restored
                .borrowed(life(2, 100), WeaponSlot::First)
                .unwrap()
                .template
                .source,
            life(0, 100)
        );
    }

    #[test]
    fn recipient_death_redrops_the_actual_weapon_with_current_ammo_then_respawns_original() {
        let mut world = fixture();
        let transfer = world
            .pickup(life(1, 100), DropId(life(0, 100)), loadout())
            .unwrap();
        assert!(world.die(life(1, 100), Some(held(3))));
        let dropped = world.drop(DropId(life(1, 100))).unwrap();
        assert_eq!(dropped.template, transfer.acquired.template);
        assert_eq!(dropped.ammo.magazine(), 3);
        let next = world.pickup(life(2, 100), dropped.id, loadout()).unwrap();
        assert_eq!(next.grant.ammo().pickup_reserve(), 30);
        assert!(world.spawn(life(1, 200), kit()));
        assert_eq!(world.borrowed(life(1, 200), WeaponSlot::First), None);
        assert_eq!(
            world.borrowed(life(2, 100), WeaponSlot::First),
            Some(next.acquired)
        );
    }

    #[test]
    fn repeated_spawn_and_death_observations_do_not_reset_or_duplicate_pickups() {
        let mut world = fixture();
        let transfer = world
            .pickup(life(1, 100), DropId(life(0, 100)), loadout())
            .unwrap();
        assert!(!world.spawn(life(1, 100), kit()));
        assert_eq!(
            world.borrowed(life(1, 100), WeaponSlot::First),
            Some(transfer.acquired)
        );
        assert!(!world.die(life(0, 100), Some(held(30))));
        assert!(!world.spawn(life(0, 100), kit()));
        assert_eq!(world.drop(DropId(life(0, 100))), None);
    }

    #[test]
    fn a_second_pickup_replaces_the_borrowed_slot_without_adding_a_third_weapon() {
        let mut world = fixture();
        let first = world
            .pickup(life(1, 100), DropId(life(0, 100)), loadout())
            .unwrap();
        assert!(world.die(life(2, 100), Some(held(9))));
        let next = world
            .pickup(life(1, 100), DropId(life(2, 100)), loadout())
            .unwrap();
        assert_eq!(next.replaced, Some(first.acquired));
        assert_eq!(
            world.borrowed(life(1, 100), WeaponSlot::First),
            Some(next.acquired)
        );
        assert_eq!(world.borrowed(life(1, 100), WeaponSlot::Second), None);
    }

    #[test]
    fn stale_requests_and_skill_refusals_do_not_consume_the_drop() {
        let mut world = fixture();
        let original = world.clone();
        let pistol = Loadout::new(
            [Some(Handedness::Two), Some(Handedness::One)],
            Some(WeaponSlot::Second),
            CarryRule::OneDoubleHanded,
        );
        assert_eq!(
            world.pickup(life(1, 100), DropId(life(0, 100)), pistol),
            Err(PickupError::Loadout(Refused::TwoDoubleHandedWeapons))
        );
        assert_eq!(world, original);
        assert_eq!(
            world.pickup(life(1, 99), DropId(life(0, 100)), loadout()),
            Err(PickupError::PickerNotAlive)
        );
        assert_eq!(
            world.pickup(life(1, 100), DropId(life(0, 99)), loadout()),
            Err(PickupError::DropUnavailable)
        );
        assert_eq!(world, original);
    }

    #[test]
    fn an_unclaimed_drop_expires_with_the_corpse_and_old_ids_cannot_select_the_next_one() {
        let mut world = fixture();
        assert!(world.spawn(life(0, 200), kit()));
        assert!(world.die(life(0, 200), Some(held(4))));
        assert_eq!(world.drop(DropId(life(0, 100))), None);
        assert_eq!(world.drop(DropId(life(0, 200))).unwrap().ammo.magazine(), 4);
        world.leave(Place::parse(0).unwrap());
        assert_eq!(world.drop(DropId(life(0, 200))), None);
    }
}
