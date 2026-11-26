//! The modules table: its bytes and their typed views. The format's one
//! definition (`docs/notes/attachments.md`, "The table"); [`crate::encode`]
//! writes what [`Table::parse`] reads.
//!
//! ```text
//! "SRPM"  u16 version  14 x u32 count   then the sections, in this order,
//! each `count` records of a fixed size (little-endian):
//!
//! guns       84  dict u16, bones u16, aim bone u16, fits-of gun u16,
//!                spans: slots, poses, stock, carriers, fits,
//!                text: the bare body's model,
//!                the composed model's own `aim` (its iron sights' line): bone
//!                u16, offset 12 x f32
//! gun slots  51  slot u8, bone u16, offset 12 x f32
//! poses      50  bone u16, the bone's matrix in the gun's aimed idle pose
//! stock       3  slot u8, module u16
//! carriers    5  the slot it brings u8, span of links
//! links       3  module u16, the slot it goes in u8
//! fits        2  module u16
//! modules    29  dict u16, role u8, texts: model, alternative model, aimed
//!                model, spans: attaches, hand (0 or 1)
//! attaches   23  slot u8, spans: adds, points, alters, closes, text: effect
//! adds       49  slot u8, locator 12 x f32
//! points     49  point u8 (0 eye, 1 fire point, 2 eye at the gun's own
//!                eye plane), locator 12 x f32; one eye an attach at most
//! slot refs   1  slot u8
//! hands     768  16 locators: 15 fingers (thumb 1..3, index, middle, ring,
//!                pinky), then the hand
//! strings     1  bytes; a text is u32 start, u16 length (0: none), its
//!                bytes followed by a NUL
//! ```
//!
//! A span is u16 start, u16 count into its section. `fits-of` is the gun's
//! own index, or the gun whose fits it takes (a premium or camouflage item
//! takes its base gun's).

use core::ffi::CStr;
use core::slice::ChunksExact;

use crate::math::Affine;
use crate::pose::Placement;
use crate::slot::SlotId;
use crate::{AttachEye, BoneIndex, DictId, Kind, Point, Role, resolve};

pub const MAGIC: [u8; 4] = *b"SRPM";
/// 2: a gun names its bare body's model. 3: a gun has its own eye, and an
/// attach's eye may keep that eye's plane (0.69d's `plane_slot`). 4: a
/// module may be a second sight (role 5, [`Kind::SecondSight`]); a version 3
/// table is read as one without any.
pub const VERSION: u16 = 4;
/// The oldest version read.
pub const OLDEST: u16 = 3;
/// A gun's own parts that are modules, at most.
pub const MAX_STOCK: usize = 8;
/// The carriers one fit brings, at most (a suppressor's clamp is one; no
/// 0.69d0 chain of our guns is longer than two).
pub const MAX_CHAIN: usize = 3;
/// A grip's hand locators: fifteen fingers, then the hand.
pub const HAND_LOCATORS: usize = 16;

pub(crate) const GUN: usize = 84;
pub(crate) const GUN_SLOT: usize = 51;
pub(crate) const POSE: usize = 50;
pub(crate) const STOCK: usize = 3;
pub(crate) const CARRIER: usize = 5;
pub(crate) const LINK: usize = 3;
pub(crate) const FIT: usize = 2;
pub(crate) const MODULE: usize = 29;
pub(crate) const ATTACH: usize = 23;
pub(crate) const ADD: usize = 49;
pub(crate) const POINT: usize = 49;
pub(crate) const SLOT_REF: usize = 1;
pub(crate) const HAND: usize = 768;
pub(crate) const SECTIONS: usize = 14;

/// Why bytes are no table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Malformed {
    Magic,
    /// Another version's table.
    Version(u16),
    /// The sections do not fill the bytes exactly, or one has more records
    /// than an index can name.
    Size,
    Gun { gun: u16, fault: GunFault },
    Module { module: u16, fault: ModuleFault },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GunFault {
    /// The record itself: a dict id of 0, no bones, an aim bone or a base
    /// gun that is not there, a span outside its section, no bare body.
    Record,
    /// Two guns of one dict id.
    DictTwice,
    /// A slot's record: no slot id, a bone the gun lacks, a float not finite.
    Slot,
    SlotTwice(SlotId),
    /// A pose's record: a bone the gun lacks, a matrix with no inverse.
    Pose,
    PoseTwice(u16),
    /// A slot's bone, or the aim bone, without its pose.
    PoseMissing(u16),
    /// The gun whose fits it takes takes another's.
    Base,
    /// A stock part: no such module, not a stock module, not one for that slot.
    Stock,
    StockTwice(SlotId),
    TooManyStock,
    /// A stock part's slot is on neither the gun nor another stock part.
    StockAdrift,
    /// A carrier chain: empty or too long, a link that is no carrier or not
    /// one for its slot, or the last link does not bring the slot.
    Carrier(SlotId),
    CarrierTwice(SlotId),
    /// A fit: no such module, or one nobody fits (a carrier, a stock part).
    Fit,
    /// Two fits of one dict id as one kind.
    FitTwice(DictId),
    /// A fit or a stock part keeps the eye in the gun's eye plane
    /// ([`AttachEye::InGunPlane`]) and the gun has no `body_aim` slot.
    NoEyePlane,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModuleFault {
    /// The record itself: a dict id of 0, no role, no model, a text or a
    /// span outside its section.
    Record,
    NoAttach,
    /// An attach's record, or one of its adds, points or slot lists.
    Attach,
    AttachTwice(SlotId),
    AddTwice(SlotId),
    /// One point twice, or both kinds of eye.
    PointTwice(Point),
    /// A scope's or a second sight's attach without its eye.
    NoEye(SlotId),
    /// A grip without its hand locators, another module with them, or a
    /// hand locator with no inverse.
    Hand,
}

pub(crate) struct Reader<'a>(pub(crate) &'a [u8]);

impl<'a> Reader<'a> {
    fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
        let (head, rest) = self.0.split_first_chunk::<N>()?;
        self.0 = rest;
        Some(*head)
    }
    fn u8(&mut self) -> Option<u8> {
        self.array().map(|[byte]| byte)
    }
    fn u16(&mut self) -> Option<u16> {
        self.array().map(u16::from_le_bytes)
    }
    fn u32(&mut self) -> Option<u32> {
        self.array().map(u32::from_le_bytes)
    }
    fn slot(&mut self) -> Option<SlotId> {
        SlotId::from_byte(self.u8()?)
    }
    fn affine(&mut self) -> Option<Affine> {
        let mut floats = [0.0f32; 12];
        for float in &mut floats {
            *float = f32::from_le_bytes(self.array()?);
        }
        let affine = Affine::from_floats(floats);
        affine.is_finite().then_some(affine)
    }
    fn span(&mut self) -> Option<Span> {
        Some(Span { start: self.u16()?, count: self.u16()? })
    }
    fn text(&mut self) -> Option<Text> {
        Some(Text { start: self.u32()?, len: self.u16()? })
    }
}

#[derive(Clone, Copy)]
struct Span {
    start: u16,
    count: u16,
}

#[derive(Clone, Copy)]
struct Text {
    start: u32,
    len: u16,
}

/// A section: records of `N` bytes.
#[derive(Clone, Copy)]
pub(crate) struct Section<'a, const N: usize>(&'a [u8]);

impl<'a, const N: usize> Section<'a, N> {
    fn records(self) -> ChunksExact<'a, u8> {
        self.0.chunks_exact(N)
    }
    fn len(self) -> usize {
        self.records().len()
    }
    fn get(self, index: u16) -> Option<&'a [u8]> {
        self.records().nth(usize::from(index))
    }
    fn span(self, span: Span) -> Option<Section<'a, N>> {
        let start = usize::from(span.start);
        let end = start.checked_add(usize::from(span.count))?;
        self.0.get(start.checked_mul(N)?..end.checked_mul(N)?).map(Section)
    }
}

/// A parsed table: every record read and checked once, by [`Table::parse`].
#[derive(Clone, Copy)]
pub struct Table<'a> {
    guns: Section<'a, GUN>,
    gun_slots: Section<'a, GUN_SLOT>,
    poses: Section<'a, POSE>,
    stock: Section<'a, STOCK>,
    carriers: Section<'a, CARRIER>,
    links: Section<'a, LINK>,
    fits: Section<'a, FIT>,
    modules: Section<'a, MODULE>,
    attaches: Section<'a, ATTACH>,
    adds: Section<'a, ADD>,
    points: Section<'a, POINT>,
    slot_refs: Section<'a, SLOT_REF>,
    hands: Section<'a, HAND>,
    strings: &'a [u8],
}

fn carve<'a, const N: usize>(rest: &mut &'a [u8], count: u32) -> Result<Section<'a, N>, Malformed> {
    let count = u16::try_from(count).map_err(|_| Malformed::Size)?;
    let size = usize::from(count).checked_mul(N).ok_or(Malformed::Size)?;
    let (head, tail) = rest.split_at_checked(size).ok_or(Malformed::Size)?;
    *rest = tail;
    Ok(Section(head))
}

impl<'a> Table<'a> {
    /// The table in `bytes`, or why they are none. Everything a view gives
    /// afterwards was checked here.
    pub fn parse(bytes: &'a [u8]) -> Result<Table<'a>, Malformed> {
        let mut header = Reader(bytes);
        if header.array() != Some(MAGIC) {
            return Err(Malformed::Magic);
        }
        let version = match header.u16() {
            Some(version @ OLDEST..=VERSION) => version,
            Some(other) => return Err(Malformed::Version(other)),
            None => return Err(Malformed::Size),
        };
        let mut counts = [0u32; SECTIONS];
        for count in &mut counts {
            *count = header.u32().ok_or(Malformed::Size)?;
        }
        let [guns, gun_slots, poses, stock, carriers, links, fits, modules, attaches, adds, points, slot_refs, hands, strings] =
            counts;
        let mut rest = header.0;
        let table = Table {
            guns: carve(&mut rest, guns)?,
            gun_slots: carve(&mut rest, gun_slots)?,
            poses: carve(&mut rest, poses)?,
            stock: carve(&mut rest, stock)?,
            carriers: carve(&mut rest, carriers)?,
            links: carve(&mut rest, links)?,
            fits: carve(&mut rest, fits)?,
            modules: carve(&mut rest, modules)?,
            attaches: carve(&mut rest, attaches)?,
            adds: carve(&mut rest, adds)?,
            points: carve(&mut rest, points)?,
            slot_refs: carve(&mut rest, slot_refs)?,
            hands: carve(&mut rest, hands)?,
            strings: rest,
        };
        if usize::try_from(strings) != Ok(rest.len()) {
            return Err(Malformed::Size);
        }
        table.check(version)?;
        Ok(table)
    }

    fn text(&self, text: Text) -> Option<Option<&'a CStr>> {
        if text.len == 0 {
            return Some(None);
        }
        let start = usize::try_from(text.start).ok()?;
        let end = start.checked_add(usize::from(text.len))?.checked_add(1)?;
        CStr::from_bytes_with_nul(self.strings.get(start..end)?).ok().map(Some)
    }

    fn gun_at(&'a self, index: u16) -> Option<Gun<'a>> {
        let mut r = Reader(self.guns.get(index)?);
        let dict = DictId::new(r.u16()?)?;
        let bones = r.u16()?;
        let aim_bone = r.u16()?;
        let fits_of = r.u16()?;
        let gun = Gun {
            table: self,
            index,
            dict,
            bones,
            aim_bone: BoneIndex(aim_bone),
            fits_of,
            slots: self.gun_slots.span(r.span()?)?,
            poses: self.poses.span(r.span()?)?,
            stock: self.stock.span(r.span()?)?,
            carriers: self.carriers.span(r.span()?)?,
            fits: self.fits.span(r.span()?)?,
            bare: self.text(r.text()?)??,
            own_aim: Placement { bone: BoneIndex(r.u16()?), offset: r.affine()? },
        };
        (aim_bone < bones && gun.own_aim.bone.0 < bones && usize::from(fits_of) < self.guns.len()).then_some(gun)
    }

    fn module_at(&'a self, index: u16) -> Option<Module<'a>> {
        let mut r = Reader(self.modules.get(index)?);
        let dict = DictId::new(r.u16()?)?;
        let role = Role::from_byte(r.u8()?)?;
        let model = self.text(r.text()?)??;
        let alt = self.text(r.text()?)?;
        let aimed = self.text(r.text()?)?;
        let attaches = self.attaches.span(r.span()?)?;
        let hand = self.hands.span(r.span()?)?;
        Some(Module { table: self, index, dict, role, model, alt, aimed, attaches, hand })
    }

    fn attach(&'a self, bytes: &'a [u8]) -> Option<Attach<'a>> {
        let mut r = Reader(bytes);
        Some(Attach {
            slot: r.slot()?,
            adds: self.adds.span(r.span()?)?,
            points: self.points.span(r.span()?)?,
            alters: self.slot_refs.span(r.span()?)?,
            closes: self.slot_refs.span(r.span()?)?,
            effect: self.text(r.text()?)?,
        })
    }

    /// Every gun.
    pub fn guns(&'a self) -> impl Iterator<Item = Gun<'a>> {
        (0..=u16::MAX).map_while(move |index| self.gun_at(index))
    }

    /// The gun of that dict id (a weapon item's).
    pub fn gun(&'a self, dict: DictId) -> Option<Gun<'a>> {
        self.guns().find(|gun| gun.dict == dict)
    }

    fn check(&self, version: u16) -> Result<(), Malformed> {
        for index in 0..=u16::MAX {
            if usize::from(index) >= self.modules.len() {
                break;
            }
            self.check_module(index).map_err(|fault| Malformed::Module { module: index, fault })?;
            let second = self.module_at(index).is_some_and(|module| module.role == Role::Fitted(Kind::SecondSight));
            if second && version < 4 {
                return Err(Malformed::Version(version));
            }
        }
        for index in 0..=u16::MAX {
            if usize::from(index) >= self.guns.len() {
                break;
            }
            self.check_gun(index).map_err(|fault| Malformed::Gun { gun: index, fault })?;
        }
        Ok(())
    }

    fn check_module(&self, index: u16) -> Result<(), ModuleFault> {
        use ModuleFault as F;
        let module = self.module_at(index).ok_or(F::Record)?;
        if module.attaches.len() == 0 {
            return Err(F::NoAttach);
        }
        for (nth, bytes) in module.attaches.records().enumerate() {
            let attach = self.attach(bytes).ok_or(F::Attach)?;
            if module.attaches().take(nth).any(|earlier| earlier.slot == attach.slot) {
                return Err(F::AttachTwice(attach.slot));
            }
            for (nth, bytes) in attach.adds.records().enumerate() {
                let (slot, _) = add(bytes).ok_or(F::Attach)?;
                if attach.adds().take(nth).any(|(earlier, _)| earlier == slot) {
                    return Err(F::AddTwice(slot));
                }
            }
            for (nth, bytes) in attach.points.records().enumerate() {
                let (point, _) = self::point(bytes).ok_or(F::Attach)?;
                let eye = |point| point != Point::FirePoint;
                if attach.points().take(nth).any(|(earlier, _)| earlier == point || (eye(earlier) && eye(point))) {
                    return Err(F::PointTwice(point));
                }
            }
            let slots = attach.alters.records().chain(attach.closes.records());
            if !slots.clone().all(|bytes| slot_ref(bytes).is_some()) {
                return Err(F::Attach);
            }
            if matches!(module.role, Role::Fitted(kind) if kind.sees()) && attach.eye().is_none() {
                return Err(F::NoEye(attach.slot));
            }
        }
        let hand = match (module.role, module.hand.len()) {
            (Role::Fitted(Kind::Grip), 1) => module.hand().is_some_and(|hand| crate::pose::fingers(&hand).is_some()),
            (Role::Fitted(Kind::Grip), _) => false,
            (_, count) => count == 0,
        };
        if !hand {
            return Err(F::Hand);
        }
        Ok(())
    }

    fn check_gun(&self, index: u16) -> Result<(), GunFault> {
        use GunFault as F;
        let gun = self.gun_at(index).ok_or(F::Record)?;
        if self.guns().take(usize::from(index)).any(|earlier| earlier.dict == gun.dict) {
            return Err(F::DictTwice);
        }
        for (nth, bytes) in gun.slots.records().enumerate() {
            let (slot, at) = gun_slot(bytes).filter(|(_, at)| at.bone.0 < gun.bones).ok_or(F::Slot)?;
            if gun.slots().take(nth).any(|(earlier, _)| earlier == slot) {
                return Err(F::SlotTwice(slot));
            }
            if gun.pose(at.bone).is_none() {
                return Err(F::PoseMissing(at.bone.0));
            }
        }
        for (nth, bytes) in gun.poses.records().enumerate() {
            let (bone, _) = pose(bytes).filter(|(bone, m)| bone.0 < gun.bones && m.inverse().is_some()).ok_or(F::Pose)?;
            if gun.poses.records().take(nth).filter_map(pose).any(|(earlier, _)| earlier == bone) {
                return Err(F::PoseTwice(bone.0));
            }
        }
        for bone in [gun.aim_bone, gun.own_aim.bone] {
            if gun.pose(bone).is_none() {
                return Err(F::PoseMissing(bone.0));
            }
        }
        if self.gun_at(gun.fits_of).is_none_or(|base| base.fits_of != base.index) {
            return Err(F::Base);
        }
        if gun.stock.len() > MAX_STOCK {
            return Err(F::TooManyStock);
        }
        for (nth, bytes) in gun.stock.records().enumerate() {
            let (slot, module) = self.stock_part(bytes).ok_or(F::Stock)?;
            if module.role != Role::Stock || module.attach(slot).is_none() {
                return Err(F::Stock);
            }
            if gun.stock().take(nth).any(|(earlier, _)| earlier == slot) {
                return Err(F::StockTwice(slot));
            }
        }
        if !resolve::stock_stands(gun) {
            return Err(F::StockAdrift);
        }
        for (nth, bytes) in gun.carriers.records().enumerate() {
            let (brings, links) = self.carrier(bytes).ok_or(F::Record)?;
            if gun.carriers.records().take(nth).filter_map(|bytes| self.carrier(bytes)).any(|(earlier, _)| earlier == brings)
            {
                return Err(F::CarrierTwice(brings));
            }
            let mut last = None;
            for bytes in links.records() {
                last = Some(
                    self.link(bytes)
                        .and_then(|(module, slot)| module.attach(slot).filter(|_| module.role == Role::Carrier))
                        .ok_or(F::Carrier(brings))?,
                );
            }
            let brought = last.is_some_and(|attach| attach.add(brings).is_some());
            if !brought || links.len() > MAX_CHAIN {
                return Err(F::Carrier(brings));
            }
        }
        for (nth, bytes) in gun.fits.records().enumerate() {
            let module = self.fit(bytes).filter(|module| matches!(module.role, Role::Fitted(_))).ok_or(F::Fit)?;
            // One optic may be a gun's scope and its second sight: one fit a dict id and kind.
            let same = |earlier: &Module<'_>| earlier.dict == module.dict && earlier.role == module.role;
            if gun.fits.records().take(nth).filter_map(|bytes| self.fit(bytes)).any(|earlier| same(&earlier)) {
                return Err(F::FitTwice(module.dict));
            }
        }
        let in_plane = |module: Module<'_>| module.attaches().any(|attach| matches!(attach.eye(), Some(AttachEye::InGunPlane(_))));
        if gun.eye_plane().is_none() && (gun.fits().any(in_plane) || gun.stock().any(|(_, module)| in_plane(module))) {
            return Err(F::NoEyePlane);
        }
        Ok(())
    }

    fn stock_part(&'a self, bytes: &[u8]) -> Option<(SlotId, Module<'a>)> {
        let mut r = Reader(bytes);
        Some((r.slot()?, self.module_at(r.u16()?)?))
    }

    fn carrier(&'a self, bytes: &[u8]) -> Option<(SlotId, Section<'a, LINK>)> {
        let mut r = Reader(bytes);
        Some((r.slot()?, self.links.span(r.span()?)?))
    }

    fn link(&'a self, bytes: &[u8]) -> Option<(Module<'a>, SlotId)> {
        let mut r = Reader(bytes);
        Some((self.module_at(r.u16()?)?, r.slot()?))
    }

    fn fit(&'a self, bytes: &[u8]) -> Option<Module<'a>> {
        self.module_at(Reader(bytes).u16()?)
    }
}

fn gun_slot(bytes: &[u8]) -> Option<(SlotId, Placement)> {
    let mut r = Reader(bytes);
    Some((r.slot()?, Placement { bone: BoneIndex(r.u16()?), offset: r.affine()? }))
}

fn pose(bytes: &[u8]) -> Option<(BoneIndex, Affine)> {
    let mut r = Reader(bytes);
    Some((BoneIndex(r.u16()?), r.affine()?))
}

fn add(bytes: &[u8]) -> Option<(SlotId, Affine)> {
    let mut r = Reader(bytes);
    Some((r.slot()?, r.affine()?))
}

fn point(bytes: &[u8]) -> Option<(Point, Affine)> {
    let mut r = Reader(bytes);
    Some((Point::from_byte(r.u8()?)?, r.affine()?))
}

fn slot_ref(bytes: &[u8]) -> Option<SlotId> {
    Reader(bytes).slot()
}

/// A gun: a weapon item's base mesh with its slots, its own parts and what
/// it takes.
#[derive(Clone, Copy)]
pub struct Gun<'t> {
    table: &'t Table<'t>,
    index: u16,
    dict: DictId,
    bones: u16,
    aim_bone: BoneIndex,
    fits_of: u16,
    slots: Section<'t, GUN_SLOT>,
    poses: Section<'t, POSE>,
    stock: Section<'t, STOCK>,
    carriers: Section<'t, CARRIER>,
    fits: Section<'t, FIT>,
    bare: &'t CStr,
    own_aim: Placement,
}

impl<'t> Gun<'t> {
    /// The weapon item's dict id.
    pub fn dict(&self) -> DictId {
        self.dict
    }

    /// The bones of its model: every bone a placement of this gun names is
    /// below it.
    pub fn bones(&self) -> u16 {
        self.bones
    }

    /// Its bare body's model, drawn with the parts once anything is fitted
    /// (a skeleton model on the bones of the gun's own, [`Gun::bones`] of
    /// them): the gun without the parts a module can displace.
    pub fn bare(&self) -> &'t CStr {
        self.bare
    }

    /// The bone its model's own `aim` locator hangs on.
    pub fn aim_bone(&self) -> BoneIndex {
        self.aim_bone
    }

    /// Its whole body's own `aim` locator: the eye the game reads from the
    /// model with no part moving it, on the line of the gun's own iron
    /// sights ([`crate::iron_sights`]).
    pub fn own_aim(&self) -> Placement {
        self.own_aim
    }

    /// 0.69d's `body_aim` slot: the plane an eye that keeps the gun's eye
    /// relief moves to ([`AttachEye::InGunPlane`]). Every gun with a fit or
    /// a stock part that keeps it has one ([`GunFault::NoEyePlane`]).
    pub fn eye_plane(&self) -> Option<Placement> {
        self.slot(SlotId::BodyAim)
    }

    /// Its own slots, each where its locator is.
    pub fn slots(&self) -> impl Iterator<Item = (SlotId, Placement)> + use<'t> {
        self.slots.records().filter_map(gun_slot)
    }

    pub fn slot(&self, slot: SlotId) -> Option<Placement> {
        self.slots().find(|(id, _)| *id == slot).map(|(_, at)| at)
    }

    /// A bone's matrix in the gun's aimed idle pose (the model's space).
    pub fn pose(&self, bone: BoneIndex) -> Option<Affine> {
        self.poses.records().filter_map(pose).find(|(id, _)| *id == bone).map(|(_, matrix)| matrix)
    }

    /// Its own parts that are modules (0.69d's `default_modules` a module
    /// can displace), each in its slot.
    pub fn stock(&self) -> impl Iterator<Item = (SlotId, Module<'t>)> + use<'t> {
        let table = self.table;
        self.stock.records().filter_map(move |bytes| table.stock_part(bytes))
    }

    /// The carriers that bring `slot` to this gun, each with the slot it
    /// goes in, in the order they go on. None: the gun has none for it.
    pub fn carrier(&self, slot: SlotId) -> Option<impl Iterator<Item = (Module<'t>, SlotId)> + use<'t>> {
        let table = self.table;
        let (_, links) = self.carriers.records().filter_map(|bytes| table.carrier(bytes)).find(|(brings, _)| *brings == slot)?;
        Some(links.records().filter_map(move |bytes| table.link(bytes)))
    }

    /// What it may be fitted with: its own fits, or its base gun's.
    pub fn fits(&self) -> impl Iterator<Item = Module<'t>> + use<'t> {
        let table = self.table;
        let owner = if self.fits_of == self.index { Some(*self) } else { table.gun_at(self.fits_of) };
        owner.into_iter().flat_map(move |gun| gun.fits.records().filter_map(move |bytes| table.fit(bytes)))
    }

    /// The module of that dict id it takes as a `kind`.
    pub fn fit(&self, dict: DictId, kind: Kind) -> Option<Module<'t>> {
        self.fits().find(|module| module.dict == dict && module.role == Role::Fitted(kind))
    }
}

/// A module: a part in a slot, with the rules of each slot it goes in.
#[derive(Clone, Copy)]
pub struct Module<'t> {
    table: &'t Table<'t>,
    index: u16,
    dict: DictId,
    role: Role,
    model: &'t CStr,
    alt: Option<&'t CStr>,
    aimed: Option<&'t CStr>,
    attaches: Section<'t, ATTACH>,
    hand: Section<'t, HAND>,
}

impl<'t> Module<'t> {
    /// Its row: two modules are the same one when their rows are.
    pub fn index(&self) -> u16 {
        self.index
    }

    /// Its item's dict id. A module whose rules differ on some gun (0.69d's
    /// `exceptions`) is a row of its own under the same dict id.
    pub fn dict(&self) -> DictId {
        self.dict
    }

    pub fn role(&self) -> Role {
        self.role
    }

    /// Its model's path.
    pub fn model(&self) -> &'t CStr {
        self.model
    }

    /// Its alternative shape (0.69d's `model_param1`: a folded sight).
    pub fn alt(&self) -> Option<&'t CStr> {
        self.alt
    }

    /// Its model in the aimed view (0.69d's `aimed_model`).
    pub fn aimed(&self) -> Option<&'t CStr> {
        self.aimed
    }

    /// The slots it goes in, in its own order of preference.
    pub fn attaches(&self) -> impl Iterator<Item = Attach<'t>> + use<'t> {
        let table = self.table;
        self.attaches.records().filter_map(move |bytes| table.attach(bytes))
    }

    /// Its rules in `slot`.
    pub fn attach(&self, slot: SlotId) -> Option<Attach<'t>> {
        self.attaches().find(|attach| attach.slot == slot)
    }

    /// A grip's hand locators, in its model's space: fifteen fingers, then
    /// the hand.
    pub fn hand(&self) -> Option<[Affine; HAND_LOCATORS]> {
        let mut r = Reader(self.hand.records().next()?);
        let mut hand = [Affine::IDENTITY; HAND_LOCATORS];
        for locator in &mut hand {
            *locator = r.affine()?;
        }
        Some(hand)
    }
}

/// A module's rules in one slot (0.69d's `attach_params`).
#[derive(Clone, Copy)]
pub struct Attach<'t> {
    slot: SlotId,
    adds: Section<'t, ADD>,
    points: Section<'t, POINT>,
    alters: Section<'t, SLOT_REF>,
    closes: Section<'t, SLOT_REF>,
    effect: Option<&'t CStr>,
}

impl<'t> Attach<'t> {
    pub fn slot(&self) -> SlotId {
        self.slot
    }

    /// The slots the module brings, each at its locator in the module's
    /// model (`slots_to_add`).
    pub fn adds(&self) -> impl Iterator<Item = (SlotId, Affine)> + use<'t> {
        self.adds.records().filter_map(add)
    }

    pub fn add(&self, slot: SlotId) -> Option<Affine> {
        self.adds().find(|(id, _)| *id == slot).map(|(_, locator)| locator)
    }

    fn points(&self) -> impl Iterator<Item = (Point, Affine)> + use<'t> {
        self.points.records().filter_map(point)
    }

    /// Where the module puts a point of the gun: its own locator, in its
    /// model (`slots_to_override`).
    pub fn point(&self, point: Point) -> Option<Affine> {
        self.points().find(|(id, _)| *id == point).map(|(_, locator)| locator)
    }

    /// Where the module puts the eye, if it does.
    pub fn eye(&self) -> Option<AttachEye> {
        self.points().find_map(|(point, locator)| match point {
            Point::Eye => Some(AttachEye::Own(locator)),
            Point::EyeInGunPlane => Some(AttachEye::InGunPlane(locator)),
            Point::FirePoint => None,
        })
    }

    pub fn fire_point(&self) -> Option<Affine> {
        self.point(Point::FirePoint)
    }

    /// The slots whose part shows its alternative shape
    /// (`slots_to_change_param`).
    pub fn alters(&self) -> impl Iterator<Item = SlotId> + use<'t> {
        self.alters.records().filter_map(slot_ref)
    }

    /// The slots whose stock part goes (`slots_to_disable`).
    pub fn closes(&self) -> impl Iterator<Item = SlotId> + use<'t> {
        self.closes.records().filter_map(slot_ref)
    }

    /// The shot effect the module sets (`effects_to_override`'s
    /// `particles.shoot`).
    pub fn shot_effect(&self) -> Option<&'t CStr> {
        self.effect
    }
}
