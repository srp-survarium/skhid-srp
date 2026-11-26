//! A gun and what a player fitted to it, to what stands where. 0.69d's own
//! attach rules, total over their inputs:
//!
//! 1. Nothing fitted: the gun is drawn [`Loadout::Whole`], as it always was.
//! 2. Else the gun's own parts stand in their slots, and each fitted module
//!    goes in the first slot it names that the gun with its own parts has;
//!    if it has none, in the first it names that the gun has carriers for,
//!    and those carriers come with it. Each kind is placed against the gun
//!    as it is with nothing fitted, so where a scope sits never depends on
//!    the grip.
//! 3. A module or carrier takes the place of the gun's own part in its
//!    slot. Two kinds that need one carrier share it.
//! 4. Every part's rules then apply: the gun's own part in a slot a part
//!    closes goes (and whatever own part stood on it), a part in a slot a
//!    part alters shows its alternative shape, and the eye, the fire point
//!    and the shot effect are the last part's that sets them, a fitted
//!    module's over a carrier's over the gun's own.
//! 5. A second sight's eye is never the eye: it is the second one, which the
//!    player switches to (0.69d's `aim_3`, `aim_4` beside `aim`), at the
//!    plane of the gun's own eye as 0.69d's `plane_slot` puts it.
//!
//! What cannot be is a [`Refusal`], never a guess.
//!
//! A close removes the gun's own part only. 0.69d's data shows why: an
//! optic closes every scope slot, its own and the one its rail is in, and
//! still stands on that rail; the parts a close does remove there are the
//! guns' own sights and scopes in another scope slot (L85A2, SV-98).

use core::ffi::CStr;

use crate::math::Affine;
use crate::math::dot;
use crate::pose::{Eye, Hand, Placement};
use crate::slot::SlotId;
use crate::table::{Attach, Gun, MAX_CHAIN, MAX_STOCK, Module};
use crate::{AttachEye, DictId, Kind, Role};

/// How far behind an eye, along its line, the gun's own eye plane may be for
/// the eye to move to it (metres; 0.69d's own bound).
const PLANE_BEHIND: f32 = -2.0;

/// The step that takes the frame `line` along its own k (its line of sight)
/// to the plane through `plane`'s origin normal to `plane`'s k, as a
/// translation in the frames' space; None when it stays: its line runs along
/// the plane, or the plane is more than 2 m behind it.
pub(crate) fn slide_to_plane(line: &Affine, plane: &Affine) -> Option<Affine> {
    let gap = [plane.t[0] - line.t[0], plane.t[1] - line.t[1], plane.t[2] - line.t[2]];
    let along = dot(gap, plane.z) / dot(line.z, plane.z);
    (along.is_finite() && along >= PLANE_BEHIND)
        .then(|| Affine::translation(along * line.z[0], along * line.z[1], along * line.z[2]))
}

/// The parts of one gun at most: its own, a chain of carriers a kind, the
/// fitted modules.
const MAX_PARTS: usize = MAX_STOCK + Kind::ALL.len() * MAX_CHAIN + Kind::ALL.len();

/// What a player fitted, by dict id (the weapon kit's slots 3, 4, 5 and 6).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Wanted {
    pub scope: Option<DictId>,
    pub muzzle: Option<DictId>,
    pub grip: Option<DictId>,
    pub second_sight: Option<DictId>,
}

impl Wanted {
    pub fn of(&self, kind: Kind) -> Option<DictId> {
        match kind {
            Kind::Scope => self.scope,
            Kind::Muzzle => self.muzzle,
            Kind::Grip => self.grip,
            Kind::SecondSight => self.second_sight,
        }
    }
}

/// Why a gun cannot be fitted so.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The gun takes no module of that dict id as that kind.
    NoFit(Kind),
    /// The gun has none of the slots the module names and no carrier that
    /// brings one.
    NoSlot(Kind),
    /// The slot is another fitted module's, or its carrier's.
    Taken(SlotId),
    /// The slot's part was displaced by another fitted module's carrier.
    Unseated(SlotId),
    /// The table contradicts itself. [`crate::Table::parse`] refuses every
    /// such table; the variant keeps `resolve` total without a panic.
    Broken,
}

/// The shape a part shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Main,
    /// Its alternative one (a sight folded under an optic).
    Alt,
}

/// A part to draw at a placement on the gun.
#[derive(Clone, Copy)]
pub struct Part<'t> {
    pub module: Module<'t>,
    pub shape: Shape,
    /// Its model's frame.
    pub at: Placement,
}

impl<'t> Part<'t> {
    /// The model to draw.
    pub fn model(&self) -> &'t CStr {
        match self.shape {
            Shape::Main => self.module.model(),
            Shape::Alt => self.module.alt().unwrap_or(self.module.model()),
        }
    }
}

/// The fitted scope: the game's own scope object draws it (its idle and
/// aimed models) at the scope's frame.
#[derive(Clone, Copy)]
pub struct Optic<'t> {
    pub module: Module<'t>,
    /// The scope locator record's content.
    pub frame: Placement,
}

/// The fitted second sight: drawn as a part ([`Bare::parts`]), its own eye
/// the second one.
#[derive(Clone, Copy)]
pub struct SecondSight<'t> {
    pub module: Module<'t>,
    /// Its model's frame, as its part's.
    pub at: Placement,
    /// The eye through it.
    pub eye: Eye,
}

/// A gun with something fitted: its bare body and every part on it.
#[derive(Clone, Copy)]
pub struct Bare<'t> {
    parts: [Option<Part<'t>>; MAX_PARTS],
    pub optic: Option<Optic<'t>>,
    /// The eye, when a part moves it; None: the gun's own.
    pub eye: Option<Eye>,
    /// The second sight and the second eye, when one is fitted.
    pub second: Option<SecondSight<'t>>,
    /// The fire point, when a part moves it; None: the gun's own.
    pub fire_point: Option<Placement>,
    /// The left hand, on a grip; None: the gun's own.
    pub left_hand: Option<Hand>,
    /// The shot effect, when a part sets it; None: the gun's own.
    pub shot_effect: Option<&'t CStr>,
}

impl<'t> Bare<'t> {
    /// The parts to draw on the bare body, the gun's own that stay, the
    /// carriers, the muzzle device, the grip and the second sight; not the
    /// scope.
    pub fn parts(&self) -> impl Iterator<Item = &Part<'t>> + Clone {
        self.parts.iter().flatten()
    }
}

/// What to draw for a gun.
#[derive(Clone, Copy)]
pub enum Loadout<'t> {
    /// The whole body, its own parts in its mesh: nothing is fitted.
    Whole,
    Bare(Bare<'t>),
}

/// What a slot's frame belongs to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Seat {
    Gun,
    Part(usize),
}

#[derive(Clone, Copy)]
struct Placed<'t> {
    module: Module<'t>,
    attach: Attach<'t>,
    role: Role,
    seat: Seat,
    at: Placement,
    shape: Shape,
}

/// A gun being fitted: every part put on it so far, the displaced ones None.
struct Build<'t> {
    gun: Gun<'t>,
    parts: [Option<Placed<'t>>; MAX_PARTS],
    used: usize,
}

impl<'t> Build<'t> {
    fn new(gun: Gun<'t>) -> Build<'t> {
        Build { gun, parts: [None; MAX_PARTS], used: 0 }
    }

    fn live(&self) -> impl Iterator<Item = (usize, &Placed<'t>)> {
        self.parts.iter().enumerate().filter_map(|(index, part)| Some((index, part.as_ref()?)))
    }

    fn part(&self, index: usize) -> Option<&Placed<'t>> {
        self.parts.get(index)?.as_ref()
    }

    fn remove(&mut self, index: usize) {
        if let Some(part) = self.parts.get_mut(index) {
            *part = None;
        }
    }

    /// A slot's frame: the gun's own, else the first part's that brings it.
    fn frame(&self, slot: SlotId) -> Option<(Seat, Placement)> {
        if let Some(at) = self.gun.slot(slot) {
            return Some((Seat::Gun, at));
        }
        self.live().find_map(|(index, part)| Some((Seat::Part(index), part.at.child(&part.attach.add(slot)?))))
    }

    fn occupant(&self, slot: SlotId) -> Option<(usize, Role, u16)> {
        self.live().find(|(_, part)| part.attach.slot() == slot).map(|(index, part)| (index, part.role, part.module.index()))
    }

    /// Puts `module` in its attach's slot. A carrier already there is shared.
    fn seat(&mut self, module: Module<'t>, attach: Attach<'t>, role: Role) -> Result<(), Refusal> {
        let slot = attach.slot();
        let (seat, at) = self.frame(slot).ok_or(Refusal::Unseated(slot))?;
        match (self.occupant(slot), role) {
            (None, _) => {}
            (Some((_, Role::Stock, _)), Role::Stock) => return Err(Refusal::Broken),
            (Some((index, Role::Stock, _)), _) => self.remove(index),
            (Some((_, Role::Carrier, there)), Role::Carrier) if there == module.index() => return Ok(()),
            (Some(_), _) => return Err(Refusal::Taken(slot)),
        }
        let place = self.parts.get_mut(self.used).ok_or(Refusal::Broken)?;
        *place = Some(Placed { module, attach, role, seat, at, shape: Shape::Main });
        self.used = self.used.saturating_add(1);
        Ok(())
    }

    /// The gun's own parts, each in its slot. One may stand on a slot
    /// another brings, so as many rounds as there are parts.
    fn stock(gun: Gun<'t>) -> Result<Build<'t>, Refusal> {
        let mut build = Build::new(gun);
        for _ in 0..MAX_STOCK {
            for (slot, module) in gun.stock() {
                if build.occupant(slot).is_none() && build.frame(slot).is_some() {
                    build.seat(module, module.attach(slot).ok_or(Refusal::Broken)?, Role::Stock)?;
                }
            }
        }
        if build.live().count() == gun.stock().count() { Ok(build) } else { Err(Refusal::Broken) }
    }

    /// The slot `module` goes in on the gun as it is now, and whether
    /// carriers bring it.
    fn way(&self, module: Module<'t>) -> Option<(Attach<'t>, bool)> {
        let stands = module.attaches().find(|attach| self.frame(attach.slot()).is_some());
        let carried = || module.attaches().find(|attach| self.gun.carrier(attach.slot()).is_some());
        stands.map(|attach| (attach, false)).or_else(|| carried().map(|attach| (attach, true)))
    }

    /// The closes, and what they leave standing on nothing, until nothing
    /// changes.
    fn settle(&mut self) -> Result<(), Refusal> {
        for _ in 0..=MAX_PARTS {
            let mut changed = false;
            for index in 0..MAX_PARTS {
                let Some(part) = self.part(index).copied() else { continue };
                if let Seat::Part(on) = part.seat
                    && self.part(on).is_none()
                {
                    if part.role != Role::Stock {
                        return Err(Refusal::Unseated(part.attach.slot()));
                    }
                    self.remove(index);
                    changed = true;
                    continue;
                }
                for slot in part.attach.closes() {
                    if let Some((there, Role::Stock, _)) = self.occupant(slot)
                        && there != index
                    {
                        self.remove(there);
                        changed = true;
                    }
                }
            }
            if !changed {
                return Ok(());
            }
        }
        Err(Refusal::Broken)
    }

    fn alter(&mut self) {
        for index in 0..MAX_PARTS {
            let Some(part) = self.part(index).copied() else { continue };
            for slot in part.attach.alters() {
                let Some((there, _, _)) = self.occupant(slot) else { continue };
                if let Some(Some(altered)) = self.parts.get_mut(there)
                    && altered.module.alt().is_some()
                {
                    altered.shape = Shape::Alt;
                }
            }
        }
    }

    /// The last part's value, a fitted module's over a carrier's over the
    /// gun's own.
    fn last<T>(&self, of: impl Fn(&Placed<'t>) -> Option<T>) -> Option<T> {
        let rank = |role: Role| match role {
            Role::Stock => 0,
            Role::Carrier => 1,
            Role::Fitted(_) => 2,
        };
        self.live().filter_map(|(_, part)| Some((rank(part.role), of(part)?))).max_by_key(|(rank, _)| *rank).map(|(_, value)| value)
    }

    /// The eye a part's attach gives, in the gun's terms.
    fn eye(&self, at: Placement, eye: AttachEye) -> Result<Eye, Refusal> {
        let eye = match eye {
            AttachEye::Own(locator) => at.child(&locator),
            AttachEye::InGunPlane(locator) => self.in_gun_plane(at.child(&locator))?,
        };
        let aim = self.gun.aim_bone();
        let aim_pose = self.gun.pose(aim).ok_or(Refusal::Broken)?;
        Eye::new(eye, &self.bone(&eye)?, (aim, &aim_pose)).ok_or(Refusal::Broken)
    }

    fn bone(&self, at: &Placement) -> Result<Affine, Refusal> {
        self.gun.pose(at.bone).ok_or(Refusal::Broken)
    }

    /// `eye` moved along its own line of sight to the plane of the gun's own
    /// eye, in the gun's aimed idle pose. 0.69d's rule for a slot with a
    /// `plane_slot` (`get_slot_transform` 0x2006CF320 of its exe, read by the
    /// scopes lane: `docs/notes/attachments-scopes.md`): with M the slot's
    /// frame and P the plane slot's, `s = (P.c - M.c)·P.k / M.k·P.k` and
    /// `M.c += s·M.k`; the axes stay. A line along the plane, or a plane
    /// more than 2 m behind the eye, leaves the eye where it is.
    fn in_gun_plane(&self, eye: Placement) -> Result<Placement, Refusal> {
        let frame = |at: &Placement, bone: &Affine| Affine::HALF_TURN.then(&at.offset).then(bone);
        let (bone, own) = (self.bone(&eye)?, self.gun.body_aim());
        let Some(slide) = slide_to_plane(&frame(&eye, &bone), &frame(&own, &self.bone(&own)?)) else {
            return Ok(eye);
        };
        let offset = eye.offset.then(&bone).then(&slide).then(&bone.inverse().ok_or(Refusal::Broken)?);
        Ok(Placement { bone: eye.bone, offset })
    }

    fn loadout(&self) -> Result<Bare<'t>, Refusal> {
        let mut bare =
            Bare { parts: [None; MAX_PARTS], optic: None, eye: None, second: None, fire_point: None, left_hand: None, shot_effect: None };
        let mut drawn = bare.parts.iter_mut();
        for (_, part) in self.live() {
            if part.role == Role::Fitted(Kind::Scope) {
                bare.optic = Some(Optic { module: part.module, frame: part.at });
            } else if let Some(place) = drawn.next() {
                *place = Some(Part { module: part.module, shape: part.shape, at: part.at });
            }
            if part.role == Role::Fitted(Kind::Grip) {
                let locators = part.module.hand().ok_or(Refusal::Broken)?;
                bare.left_hand = Some(Hand::new(&part.at, &locators).ok_or(Refusal::Broken)?);
            }
            if part.role == Role::Fitted(Kind::SecondSight) {
                let eye = self.eye(part.at, part.attach.eye().ok_or(Refusal::Broken)?)?;
                bare.second = Some(SecondSight { module: part.module, at: part.at, eye });
            }
        }
        let main = |part: &Placed<'t>| part.role != Role::Fitted(Kind::SecondSight);
        if let Some((at, eye)) = self.last(|part| Some((part.at, part.attach.eye().filter(|_| main(part))?))) {
            bare.eye = Some(self.eye(at, eye)?);
        }
        bare.fire_point = self.last(|part| Some(part.at.child(&part.attach.fire_point()?)));
        bare.shot_effect = self.last(|part| part.attach.shot_effect());
        Ok(bare)
    }
}

/// Whether the gun's own parts all stand: [`crate::Table::parse`]'s check.
pub(crate) fn stock_stands(gun: Gun<'_>) -> bool {
    Build::stock(gun).is_ok()
}

/// What to draw for `gun` with `wanted` fitted, or why it cannot be.
pub fn resolve<'t>(gun: Gun<'t>, wanted: Wanted) -> Result<Loadout<'t>, Refusal> {
    if wanted == Wanted::default() {
        return Ok(Loadout::Whole);
    }
    let mut build = Build::stock(gun)?;
    let mut fitted = [None; Kind::ALL.len()];
    for (kind, place) in Kind::ALL.into_iter().zip(&mut fitted) {
        let Some(dict) = wanted.of(kind) else { continue };
        let module = gun.fit(dict, kind).ok_or(Refusal::NoFit(kind))?;
        let (attach, carried) = build.way(module).ok_or(Refusal::NoSlot(kind))?;
        *place = Some((kind, module, attach, carried));
    }
    for (_, _, attach, carried) in fitted.into_iter().flatten() {
        let carriers = carried.then(|| gun.carrier(attach.slot())).flatten();
        for (carrier, slot) in carriers.into_iter().flatten() {
            build.seat(carrier, carrier.attach(slot).ok_or(Refusal::Broken)?, Role::Carrier)?;
        }
    }
    for (kind, module, attach, _) in fitted.into_iter().flatten() {
        build.seat(module, attach, Role::Fitted(kind))?;
    }
    build.settle()?;
    build.alter();
    build.loadout().map(Loadout::Bare)
}
