//! The table's writer, for the porter (`vostok-port port-modules`): plain
//! owned descriptions of guns and modules, to the bytes [`Table::parse`]
//! reads. [`encode`] parses what it wrote, so it never gives bytes the game
//! or a server would refuse.

use alloc::string::String;
use alloc::vec::Vec;

use crate::math::Affine;
use crate::slot::SlotId;
use crate::table::{HAND_LOCATORS, MAGIC, Malformed, Table, VERSION};
use crate::{Point, Role};

/// A gun. Modules and other guns are named by their index in [`Source`].
#[derive(Clone, Debug, Default)]
pub struct GunSource {
    pub dict: u16,
    /// Its model's bone count.
    pub bones: u16,
    /// The bone its model's `aim` locator hangs on.
    pub aim_bone: u16,
    /// The gun whose fits it takes (a premium item's base gun); None: its own.
    pub fits_of: Option<usize>,
    /// Its bare body's model.
    pub bare: String,
    /// Its slots: the slot, its locator's bone and offset.
    pub slots: Vec<(SlotId, u16, Affine)>,
    /// Its aimed idle pose: a bone and its matrix in the model's space, for
    /// every bone a slot or the aim hangs on.
    pub pose: Vec<(u16, Affine)>,
    /// Its own parts: the slot and the module in it.
    pub stock: Vec<(SlotId, usize)>,
    /// Its carriers: the slot they bring, and each with the slot it goes in.
    pub carriers: Vec<(SlotId, Vec<(usize, SlotId)>)>,
    /// The modules a player may fit.
    pub fits: Vec<usize>,
}

#[derive(Clone, Debug)]
pub struct ModuleSource {
    pub dict: u16,
    pub role: Role,
    pub model: String,
    pub alt: Option<String>,
    pub aimed: Option<String>,
    pub attaches: Vec<AttachSource>,
    /// A grip's hand locators: fifteen fingers, then the hand.
    pub hand: Option<[Affine; HAND_LOCATORS]>,
}

#[derive(Clone, Debug)]
pub struct AttachSource {
    pub slot: SlotId,
    pub adds: Vec<(SlotId, Affine)>,
    pub eye: Option<Affine>,
    pub fire_point: Option<Affine>,
    pub alters: Vec<SlotId>,
    pub closes: Vec<SlotId>,
    pub shot_effect: Option<String>,
}

impl AttachSource {
    pub fn new(slot: SlotId) -> AttachSource {
        AttachSource { slot, adds: Vec::new(), eye: None, fire_point: None, alters: Vec::new(), closes: Vec::new(), shot_effect: None }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Source {
    pub guns: Vec<GunSource>,
    pub modules: Vec<ModuleSource>,
}

/// Why a source has no table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncodeError {
    /// More records than an index can name, or a text too long or with a
    /// NUL in it.
    TooLarge,
    /// The written table is refused: the source breaks one of its rules.
    Refused(Malformed),
}

#[derive(Default)]
struct Sections {
    guns: Vec<u8>,
    gun_slots: Vec<u8>,
    poses: Vec<u8>,
    stock: Vec<u8>,
    carriers: Vec<u8>,
    links: Vec<u8>,
    fits: Vec<u8>,
    modules: Vec<u8>,
    attaches: Vec<u8>,
    adds: Vec<u8>,
    points: Vec<u8>,
    slot_refs: Vec<u8>,
    hands: Vec<u8>,
    strings: Vec<u8>,
}

fn index(value: usize) -> Result<u16, EncodeError> {
    u16::try_from(value).map_err(|_| EncodeError::TooLarge)
}

fn affine(out: &mut Vec<u8>, matrix: &Affine) {
    for float in matrix.floats() {
        out.extend(float.to_le_bytes());
    }
}

/// A span's start (the records `section` holds so far) and `count`.
fn span(out: &mut Vec<u8>, section: &[u8], record: usize, count: usize) -> Result<(), EncodeError> {
    out.extend(index(section.len().checked_div(record).ok_or(EncodeError::TooLarge)?)?.to_le_bytes());
    out.extend(index(count)?.to_le_bytes());
    Ok(())
}

fn text(out: &mut Vec<u8>, strings: &mut Vec<u8>, value: Option<&str>) -> Result<(), EncodeError> {
    let value = value.unwrap_or("");
    if value.contains('\0') {
        return Err(EncodeError::TooLarge);
    }
    out.extend(u32::try_from(strings.len()).map_err(|_| EncodeError::TooLarge)?.to_le_bytes());
    out.extend(index(value.len())?.to_le_bytes());
    if !value.is_empty() {
        strings.extend(value.as_bytes());
        strings.push(0);
    }
    Ok(())
}

/// The table of `source`.
pub fn encode(source: &Source) -> Result<Vec<u8>, EncodeError> {
    use crate::table::{ADD, ATTACH, CARRIER, FIT, GUN_SLOT, HAND, LINK, POINT, POSE, SLOT_REF, STOCK};
    let mut s = Sections::default();
    for (own, gun) in source.guns.iter().enumerate() {
        s.guns.extend(gun.dict.to_le_bytes());
        s.guns.extend(gun.bones.to_le_bytes());
        s.guns.extend(gun.aim_bone.to_le_bytes());
        s.guns.extend(index(gun.fits_of.unwrap_or(own))?.to_le_bytes());
        span(&mut s.guns, &s.gun_slots, GUN_SLOT, gun.slots.len())?;
        span(&mut s.guns, &s.poses, POSE, gun.pose.len())?;
        span(&mut s.guns, &s.stock, STOCK, gun.stock.len())?;
        span(&mut s.guns, &s.carriers, CARRIER, gun.carriers.len())?;
        span(&mut s.guns, &s.fits, FIT, gun.fits.len())?;
        text(&mut s.guns, &mut s.strings, Some(&gun.bare))?;
        for (slot, bone, offset) in &gun.slots {
            s.gun_slots.push(slot.byte());
            s.gun_slots.extend(bone.to_le_bytes());
            affine(&mut s.gun_slots, offset);
        }
        for (bone, matrix) in &gun.pose {
            s.poses.extend(bone.to_le_bytes());
            affine(&mut s.poses, matrix);
        }
        for (slot, module) in &gun.stock {
            s.stock.push(slot.byte());
            s.stock.extend(index(*module)?.to_le_bytes());
        }
        for (brings, links) in &gun.carriers {
            s.carriers.push(brings.byte());
            span(&mut s.carriers, &s.links, LINK, links.len())?;
            for (module, slot) in links {
                s.links.extend(index(*module)?.to_le_bytes());
                s.links.push(slot.byte());
            }
        }
        for module in &gun.fits {
            s.fits.extend(index(*module)?.to_le_bytes());
        }
    }
    for module in &source.modules {
        s.modules.extend(module.dict.to_le_bytes());
        s.modules.push(module.role.byte());
        text(&mut s.modules, &mut s.strings, Some(&module.model))?;
        text(&mut s.modules, &mut s.strings, module.alt.as_deref())?;
        text(&mut s.modules, &mut s.strings, module.aimed.as_deref())?;
        span(&mut s.modules, &s.attaches, ATTACH, module.attaches.len())?;
        span(&mut s.modules, &s.hands, HAND, usize::from(module.hand.is_some()))?;
        for locator in module.hand.iter().flatten() {
            affine(&mut s.hands, locator);
        }
        for attach in &module.attaches {
            let points = [(Point::Eye, attach.eye), (Point::FirePoint, attach.fire_point)];
            s.attaches.push(attach.slot.byte());
            span(&mut s.attaches, &s.adds, ADD, attach.adds.len())?;
            span(&mut s.attaches, &s.points, POINT, points.iter().filter(|(_, locator)| locator.is_some()).count())?;
            span(&mut s.attaches, &s.slot_refs, SLOT_REF, attach.alters.len())?;
            s.slot_refs.extend(attach.alters.iter().map(|slot| slot.byte()));
            span(&mut s.attaches, &s.slot_refs, SLOT_REF, attach.closes.len())?;
            s.slot_refs.extend(attach.closes.iter().map(|slot| slot.byte()));
            text(&mut s.attaches, &mut s.strings, attach.shot_effect.as_deref())?;
            for (slot, locator) in &attach.adds {
                s.adds.push(slot.byte());
                affine(&mut s.adds, locator);
            }
            for (point, locator) in points {
                if let Some(locator) = locator {
                    s.points.push(point.byte());
                    affine(&mut s.points, &locator);
                }
            }
        }
    }
    use crate::table::{GUN, MODULE};
    let sections: [(&[u8], usize); crate::table::SECTIONS] = [
        (&s.guns, GUN),
        (&s.gun_slots, GUN_SLOT),
        (&s.poses, POSE),
        (&s.stock, STOCK),
        (&s.carriers, CARRIER),
        (&s.links, LINK),
        (&s.fits, FIT),
        (&s.modules, MODULE),
        (&s.attaches, ATTACH),
        (&s.adds, ADD),
        (&s.points, POINT),
        (&s.slot_refs, SLOT_REF),
        (&s.hands, HAND),
        (&s.strings, 1),
    ];
    let mut out = Vec::from(MAGIC);
    out.extend(VERSION.to_le_bytes());
    for (bytes, record) in sections {
        let count = bytes.len().checked_div(record).ok_or(EncodeError::TooLarge)?;
        out.extend(u32::try_from(count).map_err(|_| EncodeError::TooLarge)?.to_le_bytes());
    }
    for (bytes, _) in sections {
        out.extend(bytes);
    }
    Table::parse(&out).map_err(EncodeError::Refused)?;
    Ok(out)
}
