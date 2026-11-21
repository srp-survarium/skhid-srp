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
//!   3, 4 and 5 ([`Fitted`]), to `Ok` or why not ([`Unfit`]). Every fit
//!   check (a scope's, a muzzle device's, a grip's; an attach, an account's
//!   load, a shop's list) calls this and nothing else;
//! - [`Modules::kind`]: the kind a module is fitted as on a weapon, which is
//!   the slot its record goes in ([`slot_of`]);
//! - [`Modules::fits`]: what a weapon takes, for a list.
//!
//! Game data without the table has no gun: nothing is fitted to anything
//! through it, whatever the server's code.

use std::sync::OnceLock;

use weapon_modules::{DictId, Malformed, Table, Wanted, resolve};
pub use weapon_modules::{Kind, Refusal};

use super::game_data::{self, GameData};
use super::scopes::SCOPE_SLOT;
use super::weapon_kit::{GRIP_SLOT, MUZZLE_SLOT};

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
    }
}

/// The kind a weapon's kit slot holds, for the three that hold a module.
pub fn kind_of_slot(slot: u8) -> Option<Kind> {
    Kind::ALL.into_iter().find(|&kind| slot_of(kind) == slot)
}

/// What a weapon holds, or would hold, in its kit's slots 3, 4 and 5: each
/// a module's dict id (the record's own, what its item's modification sets
/// `srp_scope`, `srp_muzzle` or `srp_grip` to).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fitted {
    pub scope: Option<u16>,
    pub muzzle: Option<u16>,
    pub grip: Option<u16>,
}

impl Fitted {
    pub fn of(&self, kind: Kind) -> Option<u16> {
        match kind {
            Kind::Scope => self.scope,
            Kind::Muzzle => self.muzzle,
            Kind::Grip => self.grip,
        }
    }

    /// This, with `module` (or nothing) as its `kind`.
    pub fn with(mut self, kind: Kind, module: Option<u16>) -> Fitted {
        match kind {
            Kind::Scope => self.scope = module,
            Kind::Muzzle => self.muzzle = module,
            Kind::Grip => self.grip = module,
        }
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

    /// The kind `module` is fitted as on `weapon`; None when the weapon
    /// takes no such module.
    pub fn kind(&self, weapon: u16, module: u16) -> Option<Kind> {
        self.gun_fits(weapon)?
            .into_iter()
            .find(|&(dict, _)| dict == module)
            .map(|(_, kind)| kind)
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
                Kind::Scope => wanted.scope = Some(dict),
                Kind::Muzzle => wanted.muzzle = Some(dict),
                Kind::Grip => wanted.grip = Some(dict),
            }
        }
        resolve(gun, wanted).map(|_| ()).map_err(Unfit::Refused)
    }
}

/// Tables made up for tests, this crate's and the crates' that use it.
#[cfg(any(test, feature = "fixtures"))]
pub mod fixtures {
    use weapon_modules::encode::{AttachSource, GunSource, ModuleSource, Source, encode};
    use weapon_modules::{Affine, HAND_LOCATORS, Role, SlotId};

    use super::{Kind, Modules};

    /// A table in which each `(weapon, module, kind)` is a fit: every gun
    /// has one slot a kind and nothing of its own, every module goes in its
    /// kind's slot. No pair is refused in it.
    pub fn modules(fits: &[(u16, u16, Kind)]) -> Modules {
        let slot = |kind| match kind {
            Kind::Scope => SlotId::ScopeDovetail,
            Kind::Muzzle => SlotId::M24x15Muzzle,
            Kind::Grip => SlotId::PicatinnyGrip,
        };
        let mut source = Source::default();
        for &(weapon, module, kind) in fits {
            let at = match source.modules.iter().position(|made| made.dict == module) {
                Some(at) => at,
                None => {
                    source.modules.push(ModuleSource {
                        dict: module,
                        role: Role::Fitted(kind),
                        model: format!("weapons/test/module_{module}"),
                        alt: None,
                        aimed: None,
                        attaches: vec![AttachSource {
                            eye: (kind == Kind::Scope).then_some(Affine::IDENTITY),
                            ..AttachSource::new(slot(kind))
                        }],
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
                        slots: Kind::ALL.into_iter().map(|kind| (slot(kind), 0, Affine::IDENTITY)).collect(),
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
    /// 1755, the EOTech 1771, a P90 brake 1421 and flash hider 1395.
    fn sample() -> Modules {
        Modules::parse(include_bytes!("../../../weapon-modules/tests/sample-v2.srpm").to_vec()).unwrap()
    }

    const AK: u16 = 3;
    const P90: u16 = 1220;
    const HK416: u16 = 763;
    const DTK3: u16 = 1415;
    const MIKI: u16 = 1438;
    const PSO1: u16 = 1755;

    #[test]
    fn a_module_goes_in_the_slot_of_its_kind() {
        let modules = sample();
        assert_eq!(modules.kind(AK, DTK3), Some(Kind::Muzzle));
        assert_eq!(modules.kind(AK, MIKI), Some(Kind::Grip));
        assert_eq!(modules.kind(AK, PSO1), Some(Kind::Scope));
        assert_eq!(modules.kind(HK416, DTK3), None);
        assert_eq!(modules.kind(999, DTK3), None);
        assert_eq!((slot_of(Kind::Scope), slot_of(Kind::Muzzle), slot_of(Kind::Grip)), (3, 4, 5));
        assert_eq!(kind_of_slot(4), Some(Kind::Muzzle));
        assert_eq!(kind_of_slot(2), None);
        assert!(modules.fits(AK).contains(&(DTK3, Kind::Muzzle)));
        assert!(modules.fits(999).is_empty());
    }

    #[test]
    fn the_check_is_the_guns_own_rules() {
        let modules = sample();
        let all = Fitted { scope: Some(PSO1), muzzle: Some(DTK3), grip: Some(MIKI) };
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

    #[test]
    fn a_made_up_table_fits_what_it_is_told() {
        let modules = fixtures::modules(&[(63, 2601, Kind::Muzzle), (63, 2660, Kind::Grip), (3, 2101, Kind::Scope)]);
        assert_eq!(modules.kind(63, 2601), Some(Kind::Muzzle));
        assert_eq!(modules.check(63, Fitted { scope: None, muzzle: Some(2601), grip: Some(2660) }), Ok(()));
        assert_eq!(modules.kind(3, 2601), None);
        assert!(modules.is_module(2101) && modules.is_module(2660) && !modules.is_module(63));
    }

    #[test]
    fn game_data_without_the_table_fits_nothing() {
        let none = Modules::NONE;
        assert!(!none.is_gun(AK));
        assert_eq!(none.kind(AK, DTK3), None);
        assert_eq!(none.check(AK, Fitted::default().with(Kind::Muzzle, Some(DTK3))), Err(Unfit::NoGun));
        assert!(Modules::parse(b"SRPX".to_vec()).is_err());
        // the test archive has none: the shared one of tests is empty
        assert!(Modules::of(game_data::shared()).is_ok_and(|modules| modules.table.is_none()));
    }
}
