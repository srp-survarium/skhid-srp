//! A weapon's fitted modules, for srp's servers: the one owner of the
//! modules table and the one fit check (`docs/notes/attachments.md`).
//!
//! The table is game data (`gameplay/weapon_modules.srpm`, the
//! `weapon-modules` crate's format): which guns there are, which modules
//! each takes as a scope, a muzzle device or a grip, and how they stand on
//! it. The game draws by it; the servers ask it one thing, whether a weapon
//! may be fitted so, with the same `resolve` the game runs:
//!
//! - [`Modules::check`]: a weapon and what it would hold in its kit's slots
//!   3, 4, 5 and 6 ([`Fitted`]), to `Ok` or why not ([`Unfit`]). Every fit
//!   check (a scope's, a muzzle device's, a grip's, a second sight's; an
//!   attach, an account's load, a shop's list) calls this and nothing else;
//! - [`Modules::kinds`], [`Modules::takes`]: the kinds a module is fitted as
//!   on a weapon (an optic may be its scope and its second sight), each the
//!   slot its record goes in ([`slot_of`]);
//! - [`Modules::fits`]: what a weapon takes, for a list.
//!
//! Game data without the table has no gun: nothing is fitted to anything
//! through it, whatever the server's code.

use std::sync::OnceLock;

use weapon_modules::{DictId, Malformed, Table, Wanted, resolve};
pub use weapon_modules::{Kind, Mount, Refusal, Side};

use super::game_data::{self, GameData};
use super::scopes::SCOPE_SLOT;
use super::weapon_kit::{self, GRIP_SLOT, MUZZLE_SLOT, SECOND_SIGHT_SLOT};

/// The table's file in the game data (the game asks for it under
/// `resources/`).
pub const TABLE_PATH: &str = "gameplay/weapon_modules.srpm";

/// The kit slot a module of `kind` is held in on a weapon
/// ([`super::weapon_kit`]).
pub fn slot_of(kind: Kind) -> u8 {
    match kind {
        Kind::Scope => SCOPE_SLOT,
        Kind::Muzzle => MUZZLE_SLOT,
        Kind::Grip => GRIP_SLOT,
        Kind::SecondSight => SECOND_SIGHT_SLOT,
    }
}

/// A second sight's side by its record's value (the kit's [`weapon_kit::Side`]);
/// None for a value that is no side.
pub fn side_of(value: u64) -> Option<Side> {
    weapon_kit::Side::of_value(value).map(|side| match side {
        weapon_kit::Side::Left => Side::Left,
        weapon_kit::Side::Right => Side::Right,
    })
}

/// A second sight's record value for `side`.
pub fn side_value(side: Side) -> u64 {
    match side {
        Side::Left => weapon_kit::Side::Left.value(),
        Side::Right => weapon_kit::Side::Right.value(),
    }
}

/// A scope's way onto its gun by its record's value (the kit's
/// [`weapon_kit::Mount`]); None for a value that is no way.
pub fn mount_of(value: u64) -> Option<Mount> {
    weapon_kit::Mount::of_value(value).and_then(|mount| Mount::new(mount.index()))
}

/// A scope's record value for `mount`.
pub fn mount_value(mount: Mount) -> u64 {
    u64::from(mount.index())
}

/// The kind a weapon's kit slot holds, for the four that hold a module.
pub fn kind_of_slot(slot: u8) -> Option<Kind> {
    Kind::ALL.into_iter().find(|&kind| slot_of(kind) == slot)
}

/// What a weapon holds, or would hold, in its kit's slots 3, 4, 5 and 6:
/// each a module's dict id (the record's own, what its item's modification
/// sets `srp_scope`, `srp_muzzle` or `srp_grip` to), the scope's way and
/// the second sight's side.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fitted {
    pub scope: Option<(u16, Mount)>,
    pub muzzle: Option<u16>,
    pub grip: Option<u16>,
    pub second_sight: Option<(u16, Side)>,
}

impl Fitted {
    pub fn of(&self, kind: Kind) -> Option<u16> {
        match kind {
            Kind::Scope => self.scope.map(|(module, _)| module),
            Kind::Muzzle => self.muzzle,
            Kind::Grip => self.grip,
            Kind::SecondSight => self.second_sight.map(|(module, _)| module),
        }
    }

    /// This, with `module` (or nothing) as its `kind`; the same scope keeps
    /// the way it had (another takes the first), a second sight the side it
    /// had (the left one for a new one).
    pub fn with(mut self, kind: Kind, module: Option<u16>) -> Fitted {
        match kind {
            Kind::Scope => {
                let mount = self.scope.filter(|(had, _)| Some(*had) == module).map_or(Mount::default(), |(_, mount)| mount);
                self.scope = module.map(|module| (module, mount));
            }
            Kind::Muzzle => self.muzzle = module,
            Kind::Grip => self.grip = module,
            Kind::SecondSight => {
                let side = self.second_sight.map_or(Side::Left, |(_, side)| side);
                self.second_sight = module.map(|module| (module, side));
            }
        }
        self
    }

    /// This, with its scope (if any) on its way `mount`.
    pub fn on_mount(mut self, mount: Mount) -> Fitted {
        self.scope = self.scope.map(|(module, _)| (module, mount));
        self
    }

    /// This, with its second sight (if any) on `side`.
    pub fn on_side(mut self, side: Side) -> Fitted {
        self.second_sight = self.second_sight.map(|(module, _)| (module, side));
        self
    }

    pub fn is_empty(&self) -> bool {
        *self == Fitted::default()
    }
}

/// Why a weapon may not be fitted so.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unfit {
    /// The weapon is no gun of the table (or the game data has no table):
    /// it takes nothing.
    NoGun,
    /// The gun's own rules refuse it (`weapon_modules::resolve`): no such
    /// fit of that kind, no slot for it, a slot another fitted module or
    /// its carrier takes.
    Refused(Refusal),
}

/// The modules table of a game data, or none.
#[derive(Clone, Copy)]
pub struct Modules {
    table: Option<Table<'static>>,
}

impl Modules {
    /// Game data without the table.
    pub const NONE: Modules = Modules { table: None };

    /// The players' game data's ([`game_data::shared`]), read once. A table
    /// that does not parse stops the server: the game would refuse it too,
    /// and every fitted weapon would be drawn stock.
    pub fn shared() -> &'static Modules {
        static SHARED: OnceLock<Modules> = OnceLock::new();
        SHARED.get_or_init(|| {
            let modules = Modules::of(game_data::shared())
                .unwrap_or_else(|why| panic!("{TABLE_PATH} does not parse: {why:?}"));
            match &modules.table {
                Some(table) => log::info!(
                    "weapon modules: {} gun(s) of {TABLE_PATH}",
                    table.guns().count()
                ),
                None => log::info!(
                    "weapon modules: the game data has no {TABLE_PATH}: nothing is fitted to any weapon"
                ),
            }
            modules
        })
    }

    /// The table of `data`; none when it has no such file.
    pub fn of(data: &GameData) -> Result<Modules, Malformed> {
        match data.file(TABLE_PATH) {
            Some(bytes) => Modules::parse(bytes.to_vec()),
            None => Ok(Modules::NONE),
        }
    }

    /// The table in `bytes`, which are kept for the process's life (a table
    /// is read once a game data).
    pub fn parse(bytes: Vec<u8>) -> Result<Modules, Malformed> {
        let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        Ok(Modules {
            table: Some(Table::parse(bytes)?),
        })
    }

    /// Whether `weapon` is a gun of the table: one that may take anything.
    pub fn is_gun(&self, weapon: u16) -> bool {
        self.gun_fits(weapon).is_some()
    }

    /// The kinds `module` is fitted as on `weapon`, in [`Kind::ALL`]'s
    /// order; empty when the weapon takes no such module.
    pub fn kinds(&self, weapon: u16, module: u16) -> Vec<Kind> {
        let fits = self.gun_fits(weapon).unwrap_or_default();
        Kind::ALL.into_iter().filter(|kind| fits.contains(&(module, *kind))).collect()
    }

    /// Whether `weapon` takes `module` as a `kind`.
    pub fn takes(&self, weapon: u16, module: u16, kind: Kind) -> bool {
        self.gun_fits(weapon).is_some_and(|fits| fits.contains(&(module, kind)))
    }

    /// Whether some gun takes `module` (as any kind): an item of the
    /// modules, which the shop sells as one ([`super::prices`]).
    pub fn is_module(&self, module: u16) -> bool {
        self.table.as_ref().is_some_and(|table| {
            table
                .guns()
                .any(|gun| gun.fits().any(|fit| fit.dict().get() == module))
        })
    }

    /// Every module `weapon` takes, each with the kind it is fitted as, in
    /// the table's order; empty for a weapon that is no gun of it.
    pub fn fits(&self, weapon: u16) -> Vec<(u16, Kind)> {
        self.gun_fits(weapon).unwrap_or_default()
    }

    fn gun_fits(&self, weapon: u16) -> Option<Vec<(u16, Kind)>> {
        let gun = self.table.as_ref()?.gun(DictId::new(weapon)?)?;
        Some(
            gun.fits()
                .filter_map(|module| match module.role() {
                    weapon_modules::Role::Fitted(kind) => Some((module.dict().get(), kind)),
                    _ => None,
                })
                .collect(),
        )
    }

    /// The one fit check: whether `weapon` may hold `fitted`, by the gun's
    /// own rules. Nothing fitted is always fine, on any weapon.
    pub fn check(&self, weapon: u16, fitted: Fitted) -> Result<(), Unfit> {
        if fitted.is_empty() {
            return Ok(());
        }
        let gun = self
            .table
            .as_ref()
            .and_then(|table| table.gun(DictId::new(weapon)?))
            .ok_or(Unfit::NoGun)?;
        // a dict id of 0 is no item's: no fit of its kind
        let mut wanted = Wanted::default();
        for kind in Kind::ALL {
            let Some(module) = fitted.of(kind) else { continue };
            let dict = DictId::new(module).ok_or(Unfit::Refused(Refusal::NoFit(kind)))?;
            match kind {
                Kind::Scope => wanted.scope = fitted.scope.map(|(_, mount)| (dict, mount)),
                Kind::Muzzle => wanted.muzzle = Some(dict),
                Kind::Grip => wanted.grip = Some(dict),
                Kind::SecondSight => wanted.second_sight = fitted.second_sight.map(|(_, side)| (dict, side)),
            }
        }
        resolve(gun, wanted).map(|_| ()).map_err(Unfit::Refused)
    }
}

/// Tables made up for tests, this crate's and the crates' that use it.
#[cfg(any(test, feature = "fixtures"))]
pub mod fixtures {
    use weapon_modules::encode::{AttachSource, GunSource, ModuleSource, Source, encode};
    use weapon_modules::{Affine, AttachEye, HAND_LOCATORS, Role, SlotId};

    use super::{Kind, Modules};

    /// A table in which each `(weapon, module, kind)` is a fit: every gun
    /// has one slot a kind and nothing of its own, every module goes in its
    /// kind's slot. No pair is refused in it.
    pub fn modules(fits: &[(u16, u16, Kind)]) -> Modules {
        // a second sight on either side
        let slots = |kind| match kind {
            Kind::Scope => vec![SlotId::ScopeDovetail],
            Kind::Muzzle => vec![SlotId::M24x15Muzzle],
            Kind::Grip => vec![SlotId::PicatinnyGrip],
            Kind::SecondSight => vec![SlotId::AngledCollimShortL, SlotId::AngledCollimShortR],
        };
        let mut source = Source::default();
        for &(weapon, module, kind) in fits {
            let at = match source.modules.iter().position(|made| made.dict == module && made.role == Role::Fitted(kind)) {
                Some(at) => at,
                None => {
                    source.modules.push(ModuleSource {
                        dict: module,
                        role: Role::Fitted(kind),
                        model: format!("weapons/test/module_{module}"),
                        alt: None,
                        aimed: None,
                        attaches: slots(kind)
                            .into_iter()
                            .map(|slot| AttachSource {
                                eye: kind.sees().then_some(AttachEye::Own(Affine::IDENTITY)),
                                ..AttachSource::new(slot)
                            })
                            .collect(),
                        hand: (kind == Kind::Grip).then_some([Affine::IDENTITY; HAND_LOCATORS]),
                    });
                    source.modules.len() - 1
                }
            };
            let gun = match source.guns.iter().position(|made| made.dict == weapon) {
                Some(gun) => gun,
                None => {
                    source.guns.push(GunSource {
                        dict: weapon,
                        bones: 1,
                        aim_bone: 0,
                        bare: format!("weapons/test/gun_{weapon}_bare"),
                        slots: Kind::ALL.into_iter().flat_map(slots).map(|slot| (slot, 0, Affine::IDENTITY)).collect(),
                        pose: vec![(0, Affine::IDENTITY)],
                        ..GunSource::default()
                    });
                    source.guns.len() - 1
                }
            };
            source.guns[gun].fits.push(at);
        }
        Modules::parse(encode(&source).expect("the made-up table is one")).expect("and parses")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `weapon-modules` crate's own sample (its guns and dict ids are
    /// 0.69d0's): the AK-74M 3, its premium 2079, the FN FNC 820, the HK416
    /// 763, the P90 1220; the DTK-3 1415, the MIKI grip 1438, the PSO-1
    /// 1755, the EOTech 1771, a P90 brake 1421 and flash hider 1395, the
    /// DeltaPoint Pro 2112 (the HK416's scope and second sight).
    fn sample() -> Modules {
        Modules::parse(include_bytes!("../../../weapon-modules/tests/sample-v4.srpm").to_vec()).unwrap()
    }

    const AK: u16 = 3;
    const P90: u16 = 1220;
    const HK416: u16 = 763;
    const DTK3: u16 = 1415;
    const MIKI: u16 = 1438;
    const PSO1: u16 = 1755;
    const DPPRO: u16 = 2112;

    #[test]
    fn a_module_goes_in_the_slot_of_its_kind() {
        let modules = sample();
        assert_eq!(modules.kinds(AK, DTK3), [Kind::Muzzle]);
        assert_eq!(modules.kinds(AK, MIKI), [Kind::Grip]);
        assert_eq!(modules.kinds(AK, PSO1), [Kind::Scope]);
        assert_eq!(modules.kinds(HK416, DPPRO), [Kind::Scope, Kind::SecondSight]);
        assert!(modules.takes(HK416, DPPRO, Kind::SecondSight) && !modules.takes(AK, DPPRO, Kind::SecondSight));
        assert!(modules.kinds(HK416, DTK3).is_empty());
        assert!(modules.kinds(999, DTK3).is_empty());
        assert_eq!(Kind::ALL.map(slot_of), [3, 4, 5, 6]);
        assert_eq!(kind_of_slot(6), Some(Kind::SecondSight));
        assert_eq!((side_of(1), side_of(0), side_of(2)), (Some(Side::Right), Some(Side::Left), None));
        assert_eq!(kind_of_slot(4), Some(Kind::Muzzle));
        assert_eq!(kind_of_slot(2), None);
        assert!(modules.fits(AK).contains(&(DTK3, Kind::Muzzle)));
        assert!(modules.fits(999).is_empty());
    }

    #[test]
    fn the_check_is_the_guns_own_rules() {
        let modules = sample();
        let all = Fitted { scope: Some((PSO1, Mount::default())), muzzle: Some(DTK3), grip: Some(MIKI), second_sight: None };
        assert_eq!(modules.check(AK, all), Ok(()));
        assert_eq!(modules.check(AK, Fitted::default()), Ok(()));
        // nothing fitted is fine on anything; something on no gun is not
        assert_eq!(modules.check(999, Fitted::default()), Ok(()));
        assert_eq!(modules.check(999, all), Err(Unfit::NoGun));
        assert_eq!(
            modules.check(HK416, Fitted::default().with(Kind::Muzzle, Some(DTK3))),
            Err(Unfit::Refused(Refusal::NoFit(Kind::Muzzle)))
        );
        // a grip in the muzzle's slot
        assert_eq!(
            modules.check(AK, Fitted::default().with(Kind::Muzzle, Some(MIKI))),
            Err(Unfit::Refused(Refusal::NoFit(Kind::Muzzle)))
        );
        // the P90: a device of the barrel's own thread and a grip, whose
        // forend takes that thread, do not go together; each alone does
        let (hider, grip) = (Fitted::default().with(Kind::Muzzle, Some(1395)), Fitted::default().with(Kind::Grip, Some(MIKI)));
        assert_eq!(modules.check(P90, hider), Ok(()));
        assert_eq!(modules.check(P90, grip), Ok(()));
        assert!(matches!(
            modules.check(P90, hider.with(Kind::Grip, Some(MIKI))),
            Err(Unfit::Refused(Refusal::Taken(_)))
        ));
        assert_eq!(modules.check(AK, Fitted::default().with(Kind::Scope, Some(0))), Err(Unfit::Refused(Refusal::NoFit(Kind::Scope))));
    }

    /// The HK416 of the sample: a DeltaPoint Pro on its top rail, a second
    /// on the offset rail at either side, both at once; the AK-74M has no
    /// angled rail.
    #[test]
    fn a_second_sight_goes_on_either_side() {
        let modules = sample();
        let side = |side| Fitted::default().with(Kind::SecondSight, Some(DPPRO)).on_side(side);
        assert_eq!(modules.check(HK416, side(Side::Left)), Ok(()));
        assert_eq!(modules.check(HK416, side(Side::Right).with(Kind::Scope, Some(DPPRO))), Ok(()));
        assert_eq!(side(Side::Right).with(Kind::SecondSight, Some(DPPRO)).second_sight, Some((DPPRO, Side::Right)));
        assert_eq!(modules.check(AK, side(Side::Left)), Err(Unfit::Refused(Refusal::NoFit(Kind::SecondSight))));
        // a made-up table: a fit of each kind, the second sight on both sides
        let made = fixtures::modules(&[(63, 2112, Kind::Scope), (63, 2112, Kind::SecondSight)]);
        assert_eq!(made.kinds(63, 2112), [Kind::Scope, Kind::SecondSight]);
        assert_eq!(made.check(63, side(Side::Right).with(Kind::Scope, Some(2112))), Ok(()));
    }

    #[test]
    fn a_made_up_table_fits_what_it_is_told() {
        let modules = fixtures::modules(&[(63, 2601, Kind::Muzzle), (63, 2660, Kind::Grip), (3, 2101, Kind::Scope)]);
        assert_eq!(modules.kinds(63, 2601), [Kind::Muzzle]);
        assert_eq!(modules.check(63, Fitted { scope: None, muzzle: Some(2601), grip: Some(2660), second_sight: None }), Ok(()));
        assert!(modules.kinds(3, 2601).is_empty());
        assert!(modules.is_module(2101) && modules.is_module(2660) && !modules.is_module(63));
    }

    #[test]
    fn game_data_without_the_table_fits_nothing() {
        let none = Modules::NONE;
        assert!(!none.is_gun(AK));
        assert!(none.kinds(AK, DTK3).is_empty());
        assert_eq!(none.check(AK, Fitted::default().with(Kind::Muzzle, Some(DTK3))), Err(Unfit::NoGun));
        assert!(Modules::parse(b"SRPX".to_vec()).is_err());
        // the test archive has none: the shared one of tests is empty
        assert!(Modules::of(game_data::shared()).is_ok_and(|modules| modules.table.is_none()));
    }
}
