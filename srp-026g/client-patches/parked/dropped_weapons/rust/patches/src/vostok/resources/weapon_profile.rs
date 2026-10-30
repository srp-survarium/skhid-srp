//! A query-owned equipment profile containing only one weapon and its ammo
//! slots. Never change the retained source or query unrelated donor equipment.

use super::{PrepareError, ProfileTemplate};
use crate::raw;
use crate::vostok::world::read;
use weapon_pickup::WeaponSlot;
use weapon_pickup::state::Kit;

// Native inventory cook's eight ammo slots, 928780h; the order determines
// compatible ammo ordinals and must survive filtering unchanged.
pub(super) const AMMO_SLOTS: [u32; 8] = [8, 9, 11, 12, 19, 20, 21, 22];

pub(super) struct WeaponProfile {
    pub profile: ProfileTemplate,
    pub kit: Kit,
    pub slot: WeaponSlot,
}

impl ProfileTemplate {
    pub(super) fn weapon_profile(
        &self,
        kit: Kit,
        slot: WeaponSlot,
    ) -> Result<WeaponProfile, PrepareError> {
        // SAFETY: this owner has a constructed native profile for the entire
        // copy. Validate all retained descriptors before allocation/query.
        unsafe {
            let kits = read(raw::player_profile::kit_count(self.this.ptr()));
            let items = read(raw::player_profile::item_count(self.this.ptr()));
            if kits > 3 || kit.get() >= kits || items == 0 || items > 69 {
                return Err(PrepareError::InvalidTemplate);
            }
            let mut selected = self.this.ptr();
            for _ in 0..kit.get() {
                selected = raw::profile_kit::end(selected).cast();
            }
            let descriptions = raw::player_profile::kit_slots(selected);
            let weapon = read(descriptions.add(slot.native() as usize));
            if weapon as u16 >= u16::from(items) {
                return Err(PrepareError::InvalidTemplate);
            }
            for ammo in AMMO_SLOTS {
                let description = read(descriptions.add(ammo as usize));
                let index = description as u16;
                if index != u16::MAX && index >= u16::from(items) {
                    return Err(PrepareError::InvalidTemplate);
                }
            }

            let profile = ProfileTemplate::copy_from(self.this.ptr(), self.allocator)
                .map_err(PrepareError::Cook)?;
            let mut base = profile.this.ptr();
            for k in 0..3 {
                let slots = raw::player_profile::kit_slots(base);
                for s in 0..23 {
                    if k != kit.get() || (s != slot.native() && !AMMO_SLOTS.contains(&s)) {
                        // Native empty descriptor: item index FFFF, count 0.
                        // Item records/inline buffers are left intact; deleting
                        // or renumbering them would change attachment identity.
                        slots.add(s as usize).write(u32::from(u16::MAX));
                    }
                }
                base = raw::profile_kit::end(base).cast();
            }
            Ok(WeaponProfile { profile, kit, slot })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vostok::resources::profiles::fixture::Source;
    use crate::vostok::world::Object;

    fn source() -> (Source, ProfileTemplate) {
        let mut source = Source::new(1, 0, 100, 17);
        unsafe {
            let profile = source.profile.as_mut_ptr().cast();
            raw::player_profile::kit_count(profile).write(3);
            raw::player_profile::item_count(profile).write(69);
            let mut base = profile;
            for kit in 0..3 {
                for slot in 0..23 {
                    raw::player_profile::kit_slots(base)
                        .add(slot as usize)
                        .write(((10 + slot) << 16) | (kit * 23 + slot));
                }
                base = raw::profile_kit::end(base).cast();
            }
            let template = ProfileTemplate::copy_from(profile, Object::at(7).unwrap()).unwrap();
            (source, template)
        }
    }

    #[test]
    fn keeps_one_weapon_and_ordered_ammo_without_mutating_retained_equipment() {
        for kit in 0..3 {
            for slot in [WeaponSlot::First, WeaponSlot::Second] {
                let (source, template) = source();
                let before = source.profile.clone();
                let selected = template
                    .weapon_profile(Kit::parse(kit).unwrap(), slot)
                    .unwrap();
                assert_eq!(selected.kit.get(), kit);
                assert_eq!(selected.slot, slot);
                drop(source);
                unsafe {
                    let mut original = template.this.ptr();
                    let mut result = selected.profile.this.ptr();
                    assert_ne!(original, result);
                    for k in 0..3 {
                        for s in 0..23 {
                            let description = ((10 + s) << 16) | (k * 23 + s);
                            assert_eq!(
                                read(raw::player_profile::kit_slots(original).add(s as usize)),
                                description
                            );
                            let keep = k == u32::from(kit)
                                && (s == slot.native() || AMMO_SLOTS.contains(&s));
                            assert_eq!(
                                read(raw::player_profile::kit_slots(result).add(s as usize)),
                                if keep {
                                    description
                                } else {
                                    u32::from(u16::MAX)
                                }
                            );
                        }
                        original = raw::profile_kit::end(original).cast();
                        result = raw::profile_kit::end(result).cast();
                    }
                    assert_eq!(
                        read(raw::player_profile::kit_count(selected.profile.this.ptr())),
                        3
                    );
                    assert_eq!(
                        read(raw::player_profile::item_count(selected.profile.this.ptr())),
                        69
                    );
                    assert_eq!(*template.this.ptr().cast::<u32>(), before[0]);
                }
                // Independent owners: the filtered recipe outlives the
                // retained template, and teardown frees each profile once.
                drop(template);
                assert_eq!(
                    super::super::profiles::fixture::equipment(&selected.profile),
                    17
                );
            }
        }
    }

    #[test]
    fn keeps_empty_ammo_slots_and_refuses_invalid_retained_descriptions() {
        let (_source, template) = source();
        let kit = Kit::parse(0).unwrap();
        unsafe {
            let base = template.this.ptr();
            let slots = raw::player_profile::kit_slots(base);
            slots.add(9).write(u32::from(u16::MAX));
            slots.add(11).write(11); // zero allowance preserved, not invented
            let selected = template.weapon_profile(kit, WeaponSlot::First).unwrap();
            let selected_slots = raw::player_profile::kit_slots(selected.profile.this.ptr());
            assert_eq!(read(selected_slots.add(9)), u32::from(u16::MAX));
            assert_eq!(read(selected_slots.add(11)), 11);
            for field in [7, 8, 22] {
                let saved = read(slots.add(field));
                slots.add(field).write((50 << 16) | 69);
                assert!(matches!(
                    template.weapon_profile(kit, WeaponSlot::First),
                    Err(PrepareError::InvalidTemplate)
                ));
                slots.add(field).write(saved);
            }
            for count in [0, 70, u8::MAX] {
                raw::player_profile::item_count(base).write(count);
                assert!(matches!(
                    template.weapon_profile(kit, WeaponSlot::First),
                    Err(PrepareError::InvalidTemplate)
                ));
            }
            raw::player_profile::item_count(base).write(69);
            for count in [0, 4, u8::MAX] {
                raw::player_profile::kit_count(base).write(count);
                assert!(matches!(
                    template.weapon_profile(kit, WeaponSlot::First),
                    Err(PrepareError::InvalidTemplate)
                ));
            }
            raw::player_profile::kit_count(base).write(1);
            assert!(matches!(
                template.weapon_profile(Kit::parse(1).unwrap(), WeaponSlot::First),
                Err(PrepareError::InvalidTemplate)
            ));
        }
    }
}
