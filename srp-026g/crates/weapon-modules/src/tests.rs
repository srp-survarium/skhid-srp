//! Cases from 0.69d0's own data: the guns, modules, slots, rules and dict
//! ids are its `game.options`' (the rows of the design's `fits69.tsv` named
//! at each test). The locators are made up, one distinct matrix each: the
//! real ones are the models', which the porter reads.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use std::path::PathBuf;

use crate::encode::{AttachSource, EncodeError, GunSource, ModuleSource, Source, encode};
use crate::*;

use SlotId as S;

// modules, by their row
const AK_BRAKE: usize = 0;
const AK_FOREND: usize = 1;
const DTK3: usize = 2;
const MIKI: usize = 3;
const MIDWEST: usize = 4;
const BRACKET: usize = 5;
const PSO1: usize = 6;
const DOVETAIL_RAIL: usize = 7;
const EOTECH: usize = 8;
const FNC_BRAKE: usize = 9;
const HK_REAR: usize = 10;
const HK_FRONT: usize = 11;
const HK_BRAKE: usize = 12;
const AK_BRAKE_VICTORY: usize = 13;
const CHOKE: usize = 14;
const P90_BRAKE: usize = 15;
const P90_ACM: usize = 16;
const TBRAKE: usize = 17;
const AEK_HIDER: usize = 18;

// guns, by their dict id in 0.69d0
const AK_74M: u16 = 3;
const AK_74M_VICTORY: u16 = 2079;
const FN_FNC: u16 = 820;
const HK416: u16 = 763;
const FN_P90: u16 = 1220;

/// The gun's `offset` bone, which every slot hangs on (as on the stock AK-74M).
const OFFSET_BONE: u16 = 1;

/// A made-up locator: a quarter turn about y and a place of its own.
fn locator(n: u8) -> Affine {
    let n = f32::from(n);
    Affine { x: [0.0, 0.0, -1.0], y: [0.0, 1.0, 0.0], z: [1.0, 0.0, 0.0], t: [0.01 * n, 0.002 * n, 0.1 + 0.03 * n] }
}

fn hand() -> [Affine; HAND_LOCATORS] {
    core::array::from_fn(|n| locator(100 + n as u8))
}

const SIGHTS: [SlotId; 9] = [
    S::PicatinnyIronsightRear,
    S::TroyIronsightFront,
    S::Hk416a5IronsightFront,
    S::Hkmp7IronsightFront,
    S::ScarIronsightFront,
    S::IronsightRear,
    S::Vhs2IronsightFront,
    S::IronsightFront,
    S::M4a1FrontIronsight,
];
const SCOPE_SLOTS: [SlotId; 8] = [
    S::PicatinnyScopeShort,
    S::PicatinnyScopeMedium,
    S::PicatinnyScopeLong,
    S::PicatinnyCollimShort,
    S::PicatinnyCollimMedium,
    S::PicatinnyScopeHandguard,
    S::ScopeDovetail,
    S::ScopePicatinnyrail,
];

fn module(dict: u16, role: Role, model: &str, attaches: Vec<AttachSource>) -> ModuleSource {
    ModuleSource { dict, role, model: String::from(model), alt: None, aimed: None, attaches, hand: None }
}

fn muzzle(slot: SlotId, n: u8, effect: &str) -> AttachSource {
    AttachSource { fire_point: Some(locator(n)), shot_effect: Some(String::from(effect)), ..AttachSource::new(slot) }
}

fn optic(slot: SlotId, n: u8) -> AttachSource {
    AttachSource { eye: Some(locator(n)), alters: SIGHTS.to_vec(), closes: SCOPE_SLOTS.to_vec(), ..AttachSource::new(slot) }
}

fn modules() -> Vec<ModuleSource> {
    let fx = "fx/weapons/attach/";
    let brakes = "weapons/modules/muzzlebrakes/";
    vec![
        module(1334, Role::Stock, "weapons/modules/muzzlebrakes/ak74_m24x15_muzzlebrake", vec![muzzle(S::M24x15Muzzle, 1, "fx/weapons/ak_74")]),
        module(1333, Role::Stock, "weapons/modules/forends_handguards/ak74_plastic_forend", vec![AttachSource::new(S::Forend)]),
        module(1415, Role::Fitted(Kind::Muzzle), &[brakes, "zenit_dtk3_m24x15_muzzlebrake"].concat(), vec![muzzle(S::M24x15Muzzle, 2, &[fx, "dtk1_2"].concat())]),
        ModuleSource {
            hand: Some(hand()),
            ..module(1438, Role::Fitted(Kind::Grip), "weapons/modules/foregrips/fabdefence_miki_foregrip", vec![AttachSource::new(S::PicatinnyGrip)])
        },
        module(
            1669,
            Role::Carrier,
            "weapons/modules/forends_handguards/ak_midwest_extended_tactical_forend",
            vec![AttachSource {
                adds: vec![
                    (S::PicatinnyTacticalL, locator(3)),
                    (S::PicatinnyTacticalR, locator(4)),
                    (S::PicatinnyGrip, locator(5)),
                    (S::PicatinnyScopeHandguard, locator(6)),
                ],
                ..AttachSource::new(S::Forend)
            }],
        ),
        module(
            1867,
            Role::Carrier,
            "weapons/modules/picatinnyrails/medium_picatinnyrail",
            vec![
                AttachSource { adds: vec![(S::PicatinnyScopeMedium, locator(7))], ..AttachSource::new(S::ScopePicatinnyrail) },
                AttachSource { adds: vec![(S::PicatinnyTactical, locator(7))], ..AttachSource::new(S::TacticalPicatinnyrail) },
                AttachSource { adds: vec![(S::PicatinnyGrip, locator(8))], ..AttachSource::new(S::GripPicatinnyrail) },
            ],
        ),
        ModuleSource {
            aimed: Some(String::from("weapons/modules/scopes/pso_1_scope/pso_1_scope_aim")),
            ..module(1755, Role::Fitted(Kind::Scope), "weapons/modules/scopes/pso_1_scope/pso_1_scope", vec![optic(S::ScopeDovetail, 9)])
        },
        module(
            1703,
            Role::Carrier,
            "weapons/modules/picatinnyrails/custom_dovetail_01_picatinnyrail",
            vec![AttachSource { adds: vec![(S::PicatinnyScopeMedium, locator(10))], ..optic(S::ScopeDovetail, 11) }],
        ),
        ModuleSource {
            aimed: Some(String::from("weapons/modules/collimators/eotech_552_collimator/eotech_552_collimator_aim")),
            ..module(
                1771,
                Role::Fitted(Kind::Scope),
                "weapons/modules/collimators/eotech_552_collimator/eotech_552_collimator",
                [S::PicatinnyScopeMedium, S::PicatinnyScopeLong, S::PicatinnyScopeHandguard]
                    .map(|slot| AttachSource { adds: vec![(S::Reticle, locator(12))], ..optic(slot, 12) })
                    .to_vec(),
            )
        },
        module(1674, Role::Stock, &[brakes, "fnfnc_i05x28_muzzlebrake"].concat(), vec![muzzle(S::I05x28Muzzle, 13, "fx/weapons/ak_74")]),
        ModuleSource {
            alt: Some(String::from("weapons/modules/ironsights/hk416_a5_rear_ironsight_off")),
            ..module(
                1678,
                Role::Stock,
                "weapons/modules/ironsights/hk416_a5_rear_ironsight_on",
                vec![AttachSource { eye: Some(locator(14)), ..AttachSource::new(S::PicatinnyIronsightRear) }],
            )
        },
        ModuleSource {
            alt: Some(String::from("weapons/modules/ironsights/hk416_a5_front_ironsight_off")),
            ..module(1677, Role::Stock, "weapons/modules/ironsights/hk416_a5_front_ironsight_on", vec![AttachSource::new(S::Hk416a5IronsightFront)])
        },
        module(1676, Role::Stock, &[brakes, "hk416a5_i05x28_muzzlebrake"].concat(), vec![muzzle(S::I05x28Muzzle, 15, "fx/weapons/ak_74")]),
        module(1334, Role::Stock, &[brakes, "ak74_m24x15_victoryday2020_muzzlebrake"].concat(), vec![muzzle(S::M24x15Muzzle, 1, "fx/weapons/ak_74")]),
        module(1715, Role::Fitted(Kind::Muzzle), "weapons/modules/chokes/polychoke_ventedcollar_m22x075_choke", vec![muzzle(S::M22x075Muzzle, 16, "fx/weapons/attach/duckbill")]),
        module(1462, Role::Stock, &[brakes, "fnp90_m12x1l_muzzlebrake"].concat(), vec![muzzle(S::M12x1lMuzzle, 17, "fx/weapons/uzi")]),
        module(
            1464,
            Role::Carrier,
            "weapons/modules/forends_handguards/p90_acm_forend",
            vec![AttachSource {
                adds: vec![(S::PicatinnyGrip, locator(18)), (S::I05x28Muzzle, locator(19))],
                fire_point: Some(locator(20)),
                ..AttachSource::new(S::M12x1lMuzzle)
            }],
        ),
        module(1421, Role::Fitted(Kind::Muzzle), &[brakes, "diamondhead_tbrake_i05x28_muzzlebrake"].concat(), vec![muzzle(S::I05x28Muzzle, 21, &[fx, "dtk4_6"].concat())]),
        module(1395, Role::Fitted(Kind::Muzzle), &[brakes, "aek919k_m12x1l_muzzlebrake"].concat(), vec![muzzle(S::M12x1lMuzzle, 22, &[fx, "pp_arrest1"].concat())]),
    ]
}

/// The gun's bones in its aimed idle pose: the root, and `offset` a little
/// off it.
fn pose() -> Vec<(u16, Affine)> {
    vec![(0, Affine::IDENTITY), (OFFSET_BONE, Affine::translation(0.0, 0.004, -0.02))]
}

fn gun(dict: u16, slots: &[(SlotId, u8)]) -> GunSource {
    GunSource {
        dict,
        bones: 4,
        aim_bone: 0,
        bare: std::format!("weapons/gun_{dict}/bare"),
        slots: slots.iter().map(|(slot, n)| (*slot, OFFSET_BONE, locator(*n))).collect(),
        pose: pose(),
        ..GunSource::default()
    }
}

fn source() -> Source {
    let ak_slots = [(S::ScopeDovetail, 30), (S::Forend, 31), (S::M24x15Muzzle, 32)];
    let ak_carriers =
        vec![(S::PicatinnyGrip, vec![(MIDWEST, S::Forend)]), (S::PicatinnyScopeMedium, vec![(DOVETAIL_RAIL, S::ScopeDovetail)])];
    let guns = vec![
        GunSource {
            stock: vec![(S::M24x15Muzzle, AK_BRAKE), (S::Forend, AK_FOREND)],
            carriers: ak_carriers.clone(),
            fits: vec![DTK3, MIKI, PSO1, EOTECH],
            ..gun(AK_74M, &ak_slots)
        },
        GunSource {
            fits_of: Some(0),
            stock: vec![(S::M24x15Muzzle, AK_BRAKE_VICTORY), (S::Forend, AK_FOREND)],
            carriers: ak_carriers,
            ..gun(AK_74M_VICTORY, &ak_slots)
        },
        GunSource {
            stock: vec![(S::I05x28Muzzle, FNC_BRAKE)],
            carriers: vec![(S::PicatinnyGrip, vec![(BRACKET, S::GripPicatinnyrail)])],
            fits: vec![MIKI, EOTECH],
            ..gun(FN_FNC, &[(S::I05x28Muzzle, 33), (S::PicatinnyScopeMedium, 34), (S::GripPicatinnyrail, 35)])
        },
        GunSource {
            stock: vec![(S::PicatinnyIronsightRear, HK_REAR), (S::Hk416a5IronsightFront, HK_FRONT), (S::I05x28Muzzle, HK_BRAKE)],
            fits: vec![MIKI, EOTECH],
            ..gun(
                HK416,
                &[
                    (S::I05x28Muzzle, 36),
                    (S::PicatinnyGrip, 37),
                    (S::PicatinnyScopeLong, 38),
                    (S::PicatinnyIronsightRear, 39),
                    (S::Hk416a5IronsightFront, 40),
                ],
            )
        },
        GunSource {
            stock: vec![(S::M12x1lMuzzle, P90_BRAKE)],
            carriers: vec![(S::PicatinnyGrip, vec![(P90_ACM, S::M12x1lMuzzle)]), (S::I05x28Muzzle, vec![(P90_ACM, S::M12x1lMuzzle)])],
            fits: vec![MIKI, TBRAKE, AEK_HIDER],
            ..gun(FN_P90, &[(S::M12x1lMuzzle, 41), (S::PicatinnyScopeMedium, 42)])
        },
    ];
    Source { guns, modules: modules() }
}

fn dict(module: usize) -> Option<DictId> {
    DictId::new(modules()[module].dict)
}

fn with(table: &Table<'_>, gun: u16, wanted: Wanted, check: impl FnOnce(Bare<'_>)) {
    let gun = table.gun(DictId::new(gun).unwrap()).unwrap();
    match resolve(gun, wanted).unwrap() {
        Loadout::Bare(bare) => check(bare),
        Loadout::Whole => panic!("the whole gun"),
    }
}

fn refusal(table: &Table<'_>, gun: u16, wanted: Wanted) -> Refusal {
    resolve(table.gun(DictId::new(gun).unwrap()).unwrap(), wanted).err().unwrap()
}

fn models<'t>(bare: &Bare<'t>) -> Vec<&'t str> {
    bare.parts().map(|part| part.model().to_str().unwrap().rsplit('/').next().unwrap()).collect()
}

fn on_offset(offset: Affine) -> Placement {
    Placement { bone: BoneIndex(OFFSET_BONE), offset }
}

#[test]
fn a_bare_gun_is_drawn_whole() {
    let bytes = encode(&source()).unwrap();
    let table = Table::parse(&bytes).unwrap();
    for gun in table.guns() {
        assert!(matches!(resolve(gun, Wanted::default()), Ok(Loadout::Whole)));
    }
    assert_eq!(table.guns().count(), 5);
}

/// fits69.tsv: `ak_74m muzzle zenit_dtk3_m24x15_muzzlebrake direct`, over
/// `ak74_m24x15_muzzlebrake stock (default module)`.
#[test]
fn a_device_takes_the_guns_own_brakes_place() {
    let bytes = encode(&source()).unwrap();
    let table = Table::parse(&bytes).unwrap();
    with(&table, AK_74M, Wanted { muzzle: dict(DTK3), ..Wanted::default() }, |bare| {
        assert_eq!(models(&bare), ["ak74_plastic_forend", "zenit_dtk3_m24x15_muzzlebrake"]);
        let device = bare.parts().last().unwrap();
        assert_eq!(device.at, on_offset(locator(32)));
        assert_eq!(bare.fire_point, Some(device.at.child(&locator(2))));
        assert_eq!(bare.shot_effect, Some(c"fx/weapons/attach/dtk1_2"));
        assert!(bare.optic.is_none() && bare.eye.is_none() && bare.left_hand.is_none());
    });
}

/// fits69.tsv: `fn_fnc grip fabdefence_miki_foregrip bracket medium_picatinnyrail`.
#[test]
fn a_grip_brings_its_bracket() {
    let bytes = encode(&source()).unwrap();
    let table = Table::parse(&bytes).unwrap();
    with(&table, FN_FNC, Wanted { grip: dict(MIKI), ..Wanted::default() }, |bare| {
        assert_eq!(models(&bare), ["fnfnc_i05x28_muzzlebrake", "medium_picatinnyrail", "fabdefence_miki_foregrip"]);
        let [brake, bracket, grip] = bare.parts().collect::<Vec<_>>()[..] else { panic!() };
        assert_eq!(bracket.at, on_offset(locator(35)));
        // the grip on the slot the bracket brings in its third attach
        assert_eq!(grip.at, bracket.at.child(&locator(8)));
        assert_eq!(bare.left_hand.unwrap().wrist, grip.at.child(&hand()[15]));
        // the gun's own brake stays, with its fire point and effect
        assert_eq!(bare.fire_point, Some(brake.at.child(&locator(13))));
        assert_eq!(bare.shot_effect, Some(c"fx/weapons/ak_74"));
    });
}

/// fits69.tsv: `ak_74m grip fabdefence_miki_foregrip forend ak_midwest_extended_tactical_forend`.
#[test]
fn a_grip_swaps_the_forend() {
    let bytes = encode(&source()).unwrap();
    let table = Table::parse(&bytes).unwrap();
    with(&table, AK_74M, Wanted { grip: dict(MIKI), ..Wanted::default() }, |bare| {
        assert_eq!(models(&bare), ["ak74_m24x15_muzzlebrake", "ak_midwest_extended_tactical_forend", "fabdefence_miki_foregrip"]);
        let forend = bare.parts().nth(1).unwrap();
        assert_eq!(forend.at, on_offset(locator(31)));
        assert_eq!(bare.parts().last().unwrap().at, forend.at.child(&locator(5)));
    });
}

/// 0.69d0: the HK416's own sights are modules with a folded shape, the rear
/// one sets the eye; an optic alters both and sets its own.
#[test]
fn a_scope_folds_the_sights() {
    let bytes = encode(&source()).unwrap();
    let table = Table::parse(&bytes).unwrap();
    with(&table, HK416, Wanted { scope: dict(EOTECH), ..Wanted::default() }, |bare| {
        assert_eq!(models(&bare), ["hk416_a5_rear_ironsight_off", "hk416_a5_front_ironsight_off", "hk416a5_i05x28_muzzlebrake"]);
        // on the gun's own long rail, the second slot the optic names
        let optic = bare.optic.unwrap();
        assert_eq!(optic.frame, on_offset(locator(38)));
        assert_eq!(optic.module.aimed().unwrap().to_str().unwrap().rsplit('/').next(), Some("eotech_552_collimator_aim"));
        assert_eq!(bare.eye.unwrap().placed(EyeAnchor::Optic), optic.frame.child(&locator(12)));
    });
    // with a grip alone the sights stand, and the eye is the rear sight's
    with(&table, HK416, Wanted { grip: dict(MIKI), ..Wanted::default() }, |bare| {
        assert_eq!(&models(&bare)[..2], ["hk416_a5_rear_ironsight_on", "hk416_a5_front_ironsight_on"]);
        assert_eq!(bare.eye.unwrap().placed(EyeAnchor::Optic), on_offset(locator(39)).child(&locator(14)));
        assert!(bare.optic.is_none());
    });
}

/// 0.69d0: a rail optic on the AK-74M stands on a dovetail rail, which the
/// optic's own closes name (`scope_dovetail`) and which stays.
#[test]
fn a_rail_optic_brings_its_rail_and_never_moves_for_a_grip() {
    let bytes = encode(&source()).unwrap();
    let table = Table::parse(&bytes).unwrap();
    let mut alone = None;
    with(&table, AK_74M, Wanted { scope: dict(EOTECH), ..Wanted::default() }, |bare| {
        assert_eq!(models(&bare), ["ak74_m24x15_muzzlebrake", "ak74_plastic_forend", "custom_dovetail_01_picatinnyrail"]);
        let rail = bare.parts().last().unwrap();
        assert_eq!(bare.optic.unwrap().frame, rail.at.child(&locator(10)));
        // the optic's eye over the rail's own
        assert_eq!(bare.eye.unwrap().placed(EyeAnchor::Optic), bare.optic.unwrap().frame.child(&locator(12)));
        alone = bare.optic.map(|optic| optic.frame);
    });
    // everything at once: the forend the grip brings has a scope rail of its
    // own, and the optic stays where it was
    let all = Wanted { scope: dict(EOTECH), muzzle: dict(DTK3), grip: dict(MIKI) };
    with(&table, AK_74M, all, |bare| {
        assert_eq!(
            models(&bare),
            ["custom_dovetail_01_picatinnyrail", "ak_midwest_extended_tactical_forend", "zenit_dtk3_m24x15_muzzlebrake", "fabdefence_miki_foregrip"]
        );
        assert_eq!(bare.optic.map(|optic| optic.frame), alone);
    });
    // a dovetail scope needs no rail
    with(&table, AK_74M, Wanted { scope: dict(PSO1), ..Wanted::default() }, |bare| {
        assert_eq!(bare.optic.unwrap().frame, on_offset(locator(30)));
        assert_eq!(bare.parts().count(), 2);
    });
}

/// The design's decision 5: `ak_74m_victoryday2020` has no links of its own
/// in 0.69d0; its own brake is the same module with another model.
#[test]
fn a_premium_item_takes_its_base_guns_fits() {
    let bytes = encode(&source()).unwrap();
    let table = Table::parse(&bytes).unwrap();
    with(&table, AK_74M_VICTORY, Wanted { grip: dict(MIKI), ..Wanted::default() }, |bare| {
        assert_eq!(models(&bare), ["ak74_m24x15_victoryday2020_muzzlebrake", "ak_midwest_extended_tactical_forend", "fabdefence_miki_foregrip"]);
    });
    with(&table, AK_74M_VICTORY, Wanted { muzzle: dict(DTK3), ..Wanted::default() }, |bare| {
        assert_eq!(models(&bare), ["ak74_plastic_forend", "zenit_dtk3_m24x15_muzzlebrake"]);
    });
    let (base, premium) = (table.gun(DictId::new(AK_74M).unwrap()).unwrap(), table.gun(DictId::new(AK_74M_VICTORY).unwrap()).unwrap());
    assert!(premium.fits().map(|module| module.index()).eq(base.fits().map(|module| module.index())));
}

#[test]
fn an_unfit_pair_is_refused() {
    let bytes = encode(&source()).unwrap();
    let table = Table::parse(&bytes).unwrap();
    // a choke of the M22x0.75 thread: 0.69d0 links it to no AK
    assert_eq!(refusal(&table, AK_74M, Wanted { muzzle: dict(CHOKE), ..Wanted::default() }), Refusal::NoFit(Kind::Muzzle));
    // a grip is no scope
    assert_eq!(refusal(&table, AK_74M, Wanted { scope: dict(MIKI), ..Wanted::default() }), Refusal::NoFit(Kind::Scope));
    // the gun's own brake is not fitted by a player
    assert_eq!(refusal(&table, AK_74M, Wanted { muzzle: dict(AK_BRAKE), ..Wanted::default() }), Refusal::NoFit(Kind::Muzzle));
    assert_eq!(refusal(&table, HK416, Wanted { muzzle: dict(DTK3), ..Wanted::default() }), Refusal::NoFit(Kind::Muzzle));
    assert!(table.gun(DictId::new(9999).unwrap()).is_none());
}

/// fits69.tsv, `fn_p90`: the ACM forend goes on the barrel's thread in the
/// brake's place and brings the grip's rail and another thread; a device of
/// the gun's own thread wants the slot the forend is in.
#[test]
fn one_carrier_serves_two_kinds_and_a_taken_slot_is_refused() {
    let bytes = encode(&source()).unwrap();
    let table = Table::parse(&bytes).unwrap();
    with(&table, FN_P90, Wanted { grip: dict(MIKI), ..Wanted::default() }, |bare| {
        assert_eq!(models(&bare), ["p90_acm_forend", "fabdefence_miki_foregrip"]);
        // the forend's fire point, in the brake's place
        assert_eq!(bare.fire_point, Some(on_offset(locator(41)).child(&locator(20))));
        assert_eq!(bare.shot_effect, None);
    });
    with(&table, FN_P90, Wanted { muzzle: dict(TBRAKE), grip: dict(MIKI), ..Wanted::default() }, |bare| {
        assert_eq!(models(&bare), ["p90_acm_forend", "diamondhead_tbrake_i05x28_muzzlebrake", "fabdefence_miki_foregrip"]);
        let device = bare.parts().nth(1).unwrap();
        assert_eq!(bare.fire_point, Some(device.at.child(&locator(21))));
        assert_eq!(bare.shot_effect, Some(c"fx/weapons/attach/dtk4_6"));
    });
    with(&table, FN_P90, Wanted { muzzle: dict(AEK_HIDER), ..Wanted::default() }, |bare| {
        assert_eq!(models(&bare), ["aek919k_m12x1l_muzzlebrake"]);
    });
    let both = Wanted { muzzle: dict(AEK_HIDER), grip: dict(MIKI), ..Wanted::default() };
    assert_eq!(refusal(&table, FN_P90, both), Refusal::Taken(S::M12x1lMuzzle));
}

/// The bones of a frame: the root where `root` puts it, `offset` moved by
/// `kick` from its idle pose.
fn bones(root: &Affine, kick: &Affine) -> impl Fn(BoneIndex) -> Affine + use<> {
    let (root, offset) = (*root, pose()[1].1.then(kick).then(root));
    move |bone| if bone.0 == OFFSET_BONE { offset } else { root }
}

fn near(a: &Affine, b: &Affine) -> bool {
    a.floats().iter().zip(b.floats()).all(|(a, b)| (a - b).abs() < 1e-5)
}

/// The engine's own two formulas (`calculate_locator` 0x243BC0,
/// `calculate_aim_alignment` 0x244D40): the optic's frame times the inverse
/// of the eye's is where the optic stands before the camera.
fn optic_before_the_eye(frame: &Placement, eye: &Placement, bones: &impl Fn(BoneIndex) -> Affine) -> Affine {
    let world = Affine::translation(10.0, 1.5, -4.0);
    let aim_offset = Affine::translation(0.0, 0.0, 0.05);
    let frame = locator_frame(&Affine::IDENTITY, frame, &bones(frame.bone), &world);
    let eye = locator_frame(&aim_offset, eye, &bones(eye.bone), &world);
    frame.then(&eye.inverse().unwrap())
}

#[test]
fn the_eye_is_fixed_to_the_optic_or_to_the_guns_idle_pose() {
    let bytes = encode(&source()).unwrap();
    let table = Table::parse(&bytes).unwrap();
    with(&table, AK_74M, Wanted { scope: dict(EOTECH), ..Wanted::default() }, |bare| {
        let (frame, eye) = (bare.optic.unwrap().frame, bare.eye.unwrap());
        let idle = bones(&Affine::IDENTITY, &Affine::IDENTITY);
        let kicked = bones(&locator(50), &Affine::translation(0.0, 0.01, -0.03));
        let centred = optic_before_the_eye(&frame, &eye.placed(EyeAnchor::Optic), &idle);
        // on the optic: the same whatever the bones do
        assert!(near(&centred, &optic_before_the_eye(&frame, &eye.placed(EyeAnchor::Optic), &kicked)));
        // on the gun's idle pose: the same while the gun is in it, wherever
        // its root is, and the kick shows
        let swayed = bones(&locator(50), &Affine::IDENTITY);
        assert_eq!(eye.placed(EyeAnchor::GunIdle).bone, BoneIndex(0));
        assert!(near(&centred, &optic_before_the_eye(&frame, &eye.placed(EyeAnchor::GunIdle), &idle)));
        assert!(near(&centred, &optic_before_the_eye(&frame, &eye.placed(EyeAnchor::GunIdle), &swayed)));
        let shown = optic_before_the_eye(&frame, &eye.placed(EyeAnchor::GunIdle), &kicked);
        assert!((shown.t[1] - centred.t[1]).abs() > 0.005 || (shown.t[2] - centred.t[2]).abs() > 0.005);
    });
}

#[test]
fn the_fingers_are_each_joint_in_its_parents_frame() {
    let bytes = encode(&source()).unwrap();
    let table = Table::parse(&bytes).unwrap();
    with(&table, HK416, Wanted { grip: dict(MIKI), ..Wanted::default() }, |bare| {
        let (hand, locators) = (bare.left_hand.unwrap(), hand());
        let own = [Affine::translation(1.0, 2.0, 3.0); FINGERS];
        let whole = hand.fingers(FingerJoints::Grip, &own);
        // thumb 1 in the hand's frame, thumb 2 in thumb 1's, index 1 in the hand's
        assert!(near(&whole[0], &locators[0].then(&locators[15].inverse().unwrap())));
        assert!(near(&whole[1], &locators[1].then(&locators[0].inverse().unwrap())));
        assert!(near(&whole[3], &locators[3].then(&locators[15].inverse().unwrap())));
        // the gun's joints: the grip's turn, the gun's own place
        let kept = hand.fingers(FingerJoints::Gun, &own);
        assert!(kept.iter().zip(&whole).all(|(kept, whole)| kept.x == whole.x && kept.z == whole.z && kept.t == [1.0, 2.0, 3.0]));
    });
}

#[test]
fn the_inverse_and_the_engines_matrix() {
    let m = locator(7).then(&Affine::HALF_TURN).then(&Affine::translation(0.3, -0.2, 0.9));
    assert!(near(&m.then(&m.inverse().unwrap()), &Affine::IDENTITY));
    assert_eq!(Affine::from_matrix(&m.matrix()), m);
    assert_eq!(Affine::HALF_TURN.matrix()[..12], [-1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0]);
    assert!(Affine { z: [0.0; 3], ..Affine::IDENTITY }.inverse().is_none());
}

#[test]
fn the_slot_ids_are_their_numbers() {
    assert_eq!(SlotId::ALL.len(), 94);
    for (number, slot) in SlotId::ALL.iter().enumerate() {
        assert_eq!(usize::from(slot.byte()), number);
        assert_eq!(SlotId::from_byte(slot.byte()), Some(*slot));
        assert_eq!(SlotId::from_name(slot.name()), Some(*slot));
    }
    assert_eq!(SlotId::from_byte(94), None);
    assert_eq!((S::M24x15Muzzle.name(), S::NoneSlot.name()), ("m24x15_muzzle", "none"));
    for role in [Role::Fitted(Kind::Scope), Role::Fitted(Kind::Muzzle), Role::Fitted(Kind::Grip), Role::Carrier, Role::Stock] {
        assert_eq!(Role::from_byte(role.byte()), Some(role));
    }
    assert_eq!(Role::from_byte(5), None);
}

fn sample() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/sample-v2.srpm")
}

/// The format, pinned: the checked-in table is what these cases encode to,
/// and it parses. A change of the layout changes [`VERSION`] and the file's
/// name; `SRP_WRITE_SAMPLE=1 cargo test -p weapon-modules` writes it.
#[test]
fn the_checked_in_table_is_the_format() {
    let bytes = encode(&source()).unwrap();
    if std::env::var_os("SRP_WRITE_SAMPLE").is_some() {
        std::fs::write(sample(), &bytes).unwrap();
    }
    let kept = std::fs::read(sample()).unwrap();
    assert!(kept == bytes, "tests/sample-v2.srpm is not what the cases encode to");
    let table = Table::parse(&kept).unwrap();
    let ak = table.gun(DictId::new(AK_74M).unwrap()).unwrap();
    assert_eq!((ak.bones(), ak.aim_bone(), ak.slots().count(), ak.stock().count(), ak.fits().count()), (4, BoneIndex(0), 3, 2, 4));
    assert_eq!(ak.slot(S::Forend), Some(on_offset(locator(31))));
    let chain: Vec<_> = ak.carrier(S::PicatinnyGrip).unwrap().map(|(module, slot)| (module.dict().get(), slot)).collect();
    assert_eq!(chain, [(1669, S::Forend)]);
    let bracket = ak.fits().find(|module| module.dict().get() == 1771).unwrap();
    assert_eq!(bracket.attaches().map(|attach| attach.slot()).collect::<Vec<_>>(), [S::PicatinnyScopeMedium, S::PicatinnyScopeLong, S::PicatinnyScopeHandguard]);
    assert_eq!(bracket.attach(S::PicatinnyScopeLong).unwrap().closes().collect::<Vec<_>>(), SCOPE_SLOTS);
}

#[test]
fn no_bytes_panic_the_parse() {
    let bytes = encode(&source()).unwrap();
    for len in 0..bytes.len() {
        assert!(Table::parse(&bytes[..len]).is_err(), "a table cut at {len}");
    }
    assert_eq!(Table::parse(&[&bytes[..], &[0]].concat()).err(), Some(Malformed::Size));
    // every byte changed in turn: refused or another table, never a panic,
    // and whatever parses resolves without one
    for at in 0..bytes.len() {
        let mut changed = bytes.clone();
        changed[at] ^= 0x55;
        let Ok(table) = Table::parse(&changed) else { continue };
        for gun in table.guns() {
            for module in gun.fits() {
                let wanted = Wanted { scope: Some(module.dict()), muzzle: Some(module.dict()), grip: Some(module.dict()) };
                let _ = resolve(gun, wanted);
            }
        }
    }
    let mut other = bytes.clone();
    other[4] = 3;
    assert_eq!(Table::parse(&other).err(), Some(Malformed::Version(3)));
    assert_eq!(Table::parse(b"SRPX").err(), Some(Malformed::Magic));
}

fn refused(change: impl FnOnce(&mut Source)) -> Malformed {
    let mut source = source();
    change(&mut source);
    match encode(&source) {
        Err(EncodeError::Refused(malformed)) => malformed,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_table_that_breaks_a_rule_is_refused() {
    let gun = |fault| Malformed::Gun { gun: 0, fault };
    let module = |module: usize, fault| Malformed::Module { module: module as u16, fault };
    assert_eq!(refused(|s| s.modules[MIKI].hand = None), module(MIKI, ModuleFault::Hand));
    assert_eq!(refused(|s| s.modules[DTK3].hand = Some(hand())), module(DTK3, ModuleFault::Hand));
    assert_eq!(refused(|s| s.modules[PSO1].attaches[0].eye = None), module(PSO1, ModuleFault::NoEye(S::ScopeDovetail)));
    assert_eq!(refused(|s| s.modules[DTK3].attaches.clear()), module(DTK3, ModuleFault::NoAttach));
    assert_eq!(refused(|s| s.modules[EOTECH].attaches[1].slot = S::PicatinnyScopeMedium), module(EOTECH, ModuleFault::AttachTwice(S::PicatinnyScopeMedium)));
    assert_eq!(refused(|s| s.modules[DTK3].dict = 0), module(DTK3, ModuleFault::Record));
    assert_eq!(refused(|s| s.modules[DTK3].attaches[0].fire_point = Some(Affine::translation(f32::NAN, 0.0, 0.0))), module(DTK3, ModuleFault::Attach));
    assert_eq!(refused(|s| s.guns[1].dict = AK_74M), Malformed::Gun { gun: 1, fault: GunFault::DictTwice });
    assert_eq!(refused(|s| s.guns[0].slots[0].1 = 4), gun(GunFault::Slot));
    assert_eq!(refused(|s| s.guns[0].slots[0].1 = 2), gun(GunFault::PoseMissing(2)));
    assert_eq!(refused(|s| s.guns[0].fits_of = Some(1)), gun(GunFault::Base));
    assert_eq!(refused(|s| s.guns[0].stock[0].1 = DTK3), gun(GunFault::Stock));
    assert_eq!(refused(|s| s.guns[0].stock.push((S::Forend, AK_FOREND))), gun(GunFault::StockTwice(S::Forend)));
    assert_eq!(refused(|s| s.guns[0].slots.truncate(1)), gun(GunFault::StockAdrift));
    assert_eq!(refused(|s| s.guns[0].carriers[0].1[0].0 = MIKI), gun(GunFault::Carrier(S::PicatinnyGrip)));
    assert_eq!(refused(|s| s.guns[0].carriers[0].0 = S::Mag), gun(GunFault::Carrier(S::Mag)));
    assert_eq!(refused(|s| s.guns[0].fits.push(MIDWEST)), gun(GunFault::Fit));
    assert_eq!(refused(|s| s.guns[0].fits.push(DTK3)), gun(GunFault::FitTwice(DictId::new(1415).unwrap())));
    assert_eq!(refused(|s| s.guns[0].fits.push(99)), gun(GunFault::Fit));
}

/// What the shell effect needs when the scope's frame moves: the old frame
/// as one matrix times the new, in any pose.
#[test]
fn a_frame_over_another_on_its_bone() {
    let (was, now) = (on_offset(locator(30)), on_offset(locator(38)).child(&locator(12)));
    let over = was.frame_over(&now).unwrap();
    for (bone, world) in [(Affine::IDENTITY, Affine::IDENTITY), (locator(50), locator(51))] {
        let frame = |at: &Placement| locator_frame(&Affine::IDENTITY, at, &bone, &world);
        assert!(near(&over.then(&frame(&now)), &frame(&was)));
    }
    assert!(was.frame_over(&Placement { bone: BoneIndex(0), ..now }).is_none());
}
