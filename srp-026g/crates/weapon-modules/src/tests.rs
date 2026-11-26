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
const MPR45: usize = 19;
const DPPRO_SIDE: usize = 20;

/// The second sight's `aim`, above its base: on the rail's two quarter turns
/// its line of sight runs along the gun's.
const SIDE_EYE: Affine = Affine::translation(0.0, 0.03, 0.01);

// guns, by their dict id in 0.69d0
const AK_74M: u16 = 3;
const AK_74M_VICTORY: u16 = 2079;
const FN_FNC: u16 = 820;
const HK416: u16 = 763;
const FN_P90: u16 = 1220;

/// The gun's `offset` bone, which every slot hangs on (as on the stock AK-74M).
const OFFSET_BONE: u16 = 1;

/// Every sample gun's own eye, on the offset bone.
const BODY_AIM: Affine = Affine::translation(0.0, 0.06, -0.12);

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
    AttachSource { eye: Some(AttachEye::Own(locator(n))), alters: SIGHTS.to_vec(), closes: SCOPE_SLOTS.to_vec(), ..AttachSource::new(slot) }
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
                vec![AttachSource { eye: Some(AttachEye::InGunPlane(locator(14))), ..AttachSource::new(S::PicatinnyIronsightRear) }],
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
        // the 45-degree offset rail: on either angled rail, the collimator's slot beside it
        module(
            1886,
            Role::Carrier,
            "weapons/modules/picatinnyrails/ncstar_mpr45_picatinnyrail",
            vec![
                AttachSource {
                    adds: vec![(S::AngledCollimShortL, locator(23))],
                    closes: vec![S::AngledPicatinnyrailR],
                    ..AttachSource::new(S::AngledPicatinnyrailL)
                },
                AttachSource {
                    adds: vec![(S::AngledCollimShortR, locator(24))],
                    closes: vec![S::AngledPicatinnyrailL],
                    ..AttachSource::new(S::AngledPicatinnyrailR)
                },
            ],
        ),
        // a DeltaPoint Pro as a second sight (its own item, ours): 0.69d's
        // attach_params_3, `aim_4` and `reticle_3` at its `aim`, at the gun's eye plane
        ModuleSource {
            aimed: Some(String::from("weapons/modules/collimators/leupold_dppro_collimator/leupold_dppro_collimator_aim")),
            ..module(
                2672,
                Role::Fitted(Kind::SecondSight),
                "weapons/modules/collimators/leupold_dppro_collimator/leupold_dppro_collimator",
                vec![
                    AttachSource {
                        eye: Some(AttachEye::InGunPlane(SIDE_EYE)),
                        adds: vec![(S::Reticle2, SIDE_EYE)],
                        ..AttachSource::new(S::AngledCollimShortL)
                    },
                    AttachSource {
                        eye: Some(AttachEye::InGunPlane(SIDE_EYE)),
                        adds: vec![(S::Reticle3, SIDE_EYE)],
                        ..AttachSource::new(S::AngledCollimShortR)
                    },
                ],
            )
        },
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
        body_aim: (OFFSET_BONE, BODY_AIM),
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
            // a second sight the gun names, with no angled rail to bring its slot
            fits: vec![MIKI, EOTECH, DPPRO_SIDE],
            ..gun(FN_FNC, &[(S::I05x28Muzzle, 33), (S::PicatinnyScopeMedium, 34), (S::GripPicatinnyrail, 35)])
        },
        GunSource {
            stock: vec![(S::PicatinnyIronsightRear, HK_REAR), (S::Hk416a5IronsightFront, HK_FRONT), (S::I05x28Muzzle, HK_BRAKE)],
            carriers: vec![
                (S::AngledCollimShortL, vec![(MPR45, S::AngledPicatinnyrailL)]),
                (S::AngledCollimShortR, vec![(MPR45, S::AngledPicatinnyrailR)]),
            ],
            fits: vec![MIKI, EOTECH, DPPRO_SIDE],
            ..gun(
                HK416,
                &[
                    (S::I05x28Muzzle, 36),
                    (S::PicatinnyGrip, 37),
                    (S::PicatinnyScopeLong, 38),
                    (S::PicatinnyIronsightRear, 39),
                    (S::Hk416a5IronsightFront, 40),
                    (S::AngledPicatinnyrailL, 43),
                    (S::AngledPicatinnyrailR, 44),
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

fn right(module: usize) -> Option<(DictId, Side)> {
    dict(module).map(|dict| (dict, Side::Right))
}

fn left(module: usize) -> Option<(DictId, Side)> {
    dict(module).map(|dict| (dict, Side::Left))
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

/// The PKP's own grip, whose frame is a quarter turn on its gun (its rail is on the gun's
/// side): the hand stands at the grip's place in the gun's axes, as on a grip that is not
/// turned; the fingers are the grip's own either way.
#[test]
fn a_turned_grip_keeps_the_hand_in_the_guns_axes() {
    let turned = on_offset(locator(8));
    let [x, y, z] = turned.offset.t;
    let straight = on_offset(Affine::translation(x, y, z));
    let hand_on = |grip: &Placement| Hand::new(grip, &hand()).unwrap();
    assert_eq!(hand_on(&turned), hand_on(&straight));
    assert_eq!(hand_on(&turned).wrist, straight.child(&hand()[15]));
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
    let mut data = source();
    let hk = data.guns.iter_mut().find(|g| g.dict == HK416).unwrap();
    let donor_eye = Affine::translation(0.0, 0.1, 0.17);
    hk.slots.push((S::BodyAim, OFFSET_BONE, donor_eye));
    let bytes = encode(&data).unwrap();
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
        // an open sight: its own axes and its line of sight, at the plane of
        // the gun's own eye (0.69d's plane_slot)
        let (own, eye) = (on_offset(locator(39)).child(&locator(14)), bare.eye.unwrap().placed(EyeAnchor::Optic));
        assert_eq!((eye.bone, eye.offset.x, eye.offset.y, eye.offset.z), (own.bone, own.offset.x, own.offset.y, own.offset.z));
        let frame = |at: &Placement| locator_frame(&Affine::IDENTITY, at, &pose()[1].1, &Affine::IDENTITY);
        let gun_eye = Placement { bone: BoneIndex(OFFSET_BONE), offset: donor_eye };
        let (line, moved, plane) = (frame(&own), frame(&eye), frame(&gun_eye));
        let off = |a: [f32; 3], b: [f32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        assert!(dot(off(moved.t, plane.t), plane.z).abs() < 1e-5);
        let (step, along) = (off(moved.t, line.t), dot(off(moved.t, line.t), line.z));
        assert!(along.abs() > 0.1 && (0..3).all(|k| (step[k] - along * line.z[k]).abs() < 1e-5));
        assert!(bare.optic.is_none());
    });
}

/// 0.69d's two cases of `plane_slot` that leave the eye whole: a line of
/// sight along the plane, and a plane more than 2 m behind the eye.
#[test]
fn an_eye_stays_whole_off_its_plane() {
    use crate::resolve::slide_to_plane;
    let plane = Affine::translation(0.0, 0.06, -0.12);
    let at = |z: f32, k: [f32; 3]| Affine { z: k, t: [0.0, 0.07, z], ..Affine::IDENTITY };
    let step = |z: f32, k: [f32; 3]| slide_to_plane(&at(z, k), &plane).map(|step| step.t);
    let ahead = [0.0, 0.0, 1.0];
    assert!(step(0.1, ahead).is_some_and(|[x, y, z]| x == 0.0 && y == 0.0 && (z + 0.22).abs() < 1e-6));
    // the plane 1.92 m behind: moved; 2.02 m behind: whole
    assert!(step(1.8, ahead).is_some());
    assert_eq!(step(1.9, ahead), None);
    // a line along the plane
    assert_eq!(step(0.1, [1.0, 0.0, 0.0]), None);
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
    let all = Wanted { scope: dict(EOTECH), muzzle: dict(DTK3), grip: dict(MIKI), second_sight: None };
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

/// A gun whose suppressor is built in (the design's decision 3; 0.69d0's
/// OTs-14 chain `ots144a_groza_lock_suppressor > barrel_bracket_01_picatinnyrail`,
/// fits69.tsv): the suppressor is the gun's own part in its muzzle slot and
/// brings the bracket's slot, the bracket is the grip's carrier. The table
/// as it is says it: a grip goes on through the bracket, the suppressor
/// stays, and with no muzzle fit nothing takes its place.
#[test]
fn a_grip_hangs_on_a_built_in_suppressor_through_its_bracket() {
    let mut source = source();
    let (suppressor, bracket) = (source.modules.len(), source.modules.len() + 1);
    source.modules.push(module(
        1601,
        Role::Stock,
        "weapons/modules/suppressors/ots144a_groza_lock_suppressor",
        vec![AttachSource {
            adds: vec![(S::OnbarrelTacticalPicatinnyrail, locator(60))],
            fire_point: Some(locator(61)),
            ..AttachSource::new(S::GrozaLockMuzzle)
        }],
    ));
    source.modules.push(module(
        1602,
        Role::Carrier,
        "weapons/modules/picatinnyrails/barrel_bracket_01_picatinnyrail",
        vec![AttachSource { adds: vec![(S::PicatinnyGrip, locator(62))], ..AttachSource::new(S::OnbarrelTacticalPicatinnyrail) }],
    ));
    const OTS14: u16 = 900;
    source.guns.push(GunSource {
        stock: vec![(S::GrozaLockMuzzle, suppressor)],
        carriers: vec![(S::PicatinnyGrip, vec![(bracket, S::OnbarrelTacticalPicatinnyrail)])],
        fits: vec![MIKI],
        ..gun(OTS14, &[(S::GrozaLockMuzzle, 63)])
    });
    let bytes = encode(&source).unwrap();
    let table = Table::parse(&bytes).unwrap();
    with(&table, OTS14, Wanted { grip: dict(MIKI), ..Wanted::default() }, |bare| {
        assert_eq!(models(&bare), ["ots144a_groza_lock_suppressor", "barrel_bracket_01_picatinnyrail", "fabdefence_miki_foregrip"]);
        let on_suppressor = on_offset(locator(63)).child(&locator(60));
        let grip = bare.parts().nth(2).unwrap();
        assert_eq!(grip.at, on_suppressor.child(&locator(62)));
        // the fire point stays the suppressor's
        assert_eq!(bare.fire_point, Some(on_offset(locator(63)).child(&locator(61))));
        assert!(bare.left_hand.is_some());
    });
    assert_eq!(refusal(&table, OTS14, Wanted { muzzle: dict(DTK3), ..Wanted::default() }), Refusal::NoFit(Kind::Muzzle));
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

/// Patch 165's rule for the aimed view (`docs/notes/aim-pose.md`): on an
/// aimed frame at rest the aim record leaves its bone for the gun's model at
/// that pose (the record's offset times the bone, on no bone), and the
/// game's alignment reads that from then on. The optic before such an eye
/// (`calculate_locator` takes no bone's matrix for a record on none).
fn optic_before_the_posed_eye(frame: &Placement, eye: &Placement, rest: &impl Fn(BoneIndex) -> Affine, bones: &impl Fn(BoneIndex) -> Affine) -> Affine {
    let world = Affine::translation(10.0, 1.5, -4.0);
    let aim_offset = Affine::translation(0.0, 0.0, 0.05);
    let posed = eye.offset.then(&rest(eye.bone));
    let frame = locator_frame(&Affine::IDENTITY, frame, &bones(frame.bone), &world);
    let eye = aim_offset.then(&Affine::HALF_TURN).then(&posed).then(&world);
    frame.then(&eye.inverse().unwrap())
}

/// The two anchors beside patch 165, on a gun whose own `aim` is on the
/// root (every stock gun) and on one whose `aim` is on the bone the fire
/// clip kicks (the ten ported guns the patch was made for):
///
/// - alone, the optic's anchor never shows the kick; the idle pose's shows
///   it where the gun's own `aim` is on a still bone, and not on the ten;
/// - under the patch both anchors give one eye: centred at rest, the whole
///   kick shown, on every gun. So the kick is seen with either setting once
///   the patch is in, and without it only with the idle pose's.
#[test]
fn the_idle_pose_anchor_shows_the_kick_and_agrees_with_patch_165() {
    let kick = Affine::translation(0.0, 0.01, -0.03);
    let moved = |a: &Affine, b: &Affine| (a.t[1] - b.t[1]).abs() > 0.005 || (a.t[2] - b.t[2]).abs() > 0.005;
    for aim_bone in [0, OFFSET_BONE] {
        let mut source = source();
        source.guns[0].aim_bone = aim_bone;
        let bytes = encode(&source).unwrap();
        let table = Table::parse(&bytes).unwrap();
        with(&table, AK_74M, Wanted { scope: dict(EOTECH), ..Wanted::default() }, |bare| {
            let (frame, eye) = (bare.optic.unwrap().frame, bare.eye.unwrap());
            let (rest, kicked) = (bones(&Affine::IDENTITY, &Affine::IDENTITY), bones(&Affine::IDENTITY, &kick));
            let (on_optic, on_idle) = (eye.placed(EyeAnchor::Optic), eye.placed(EyeAnchor::GunIdle));
            let centred = optic_before_the_eye(&frame, &on_optic, &rest);
            // alone
            assert!(near(&centred, &optic_before_the_eye(&frame, &on_optic, &kicked)));
            assert!(near(&centred, &optic_before_the_eye(&frame, &on_idle, &rest)));
            let alone = optic_before_the_eye(&frame, &on_idle, &kicked);
            assert_eq!(moved(&alone, &centred), aim_bone != OFFSET_BONE);
            // under patch 165
            for anchor in [&on_optic, &on_idle] {
                assert!(near(&centred, &optic_before_the_posed_eye(&frame, anchor, &rest, &rest)));
                let shown = optic_before_the_posed_eye(&frame, anchor, &rest, &kicked);
                assert!(moved(&shown, &centred));
                assert!(near(&shown, &optic_before_the_posed_eye(&frame, &on_optic, &rest, &kicked)));
            }
            // and what it shows is what the idle pose's anchor shows alone on a gun aimed from the root
            if aim_bone != OFFSET_BONE {
                assert!(near(&alone, &optic_before_the_posed_eye(&frame, &on_optic, &rest, &kicked)));
            }
        });
    }
}

#[test]
fn the_fingers_are_each_joint_in_its_parents_frame() {
    let bytes = encode(&source()).unwrap();
    let table = Table::parse(&bytes).unwrap();
    with(&table, HK416, Wanted { grip: dict(MIKI), ..Wanted::default() }, |bare| {
        let (hand, locators) = (bare.left_hand.unwrap(), hand());
        let whole = hand.fingers();
        // thumb 1 in the hand's frame, thumb 2 in thumb 1's, index 1 in the hand's
        assert!(near(&whole[0], &locators[0].then(&locators[15].inverse().unwrap())));
        assert!(near(&whole[1], &locators[1].then(&locators[0].inverse().unwrap())));
        assert!(near(&whole[3], &locators[3].then(&locators[15].inverse().unwrap())));
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
    for role in [Role::Carrier, Role::Stock].into_iter().chain(Kind::ALL.map(Role::Fitted)) {
        assert_eq!(Role::from_byte(role.byte()), Some(role));
    }
    assert_eq!(Role::from_byte(6), None);
}

fn sample() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/sample-v4.srpm")
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
    assert!(kept == bytes, "tests/sample-v4.srpm is not what the cases encode to");
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
                let dict = Some(module.dict());
                let wanted = Wanted { scope: dict, muzzle: dict, grip: dict, second_sight: dict.map(|dict| (dict, Side::Right)) };
                let _ = resolve(gun, wanted);
            }
        }
    }
    let mut other = bytes.clone();
    other[4] = 5;
    assert_eq!(Table::parse(&other).err(), Some(Malformed::Version(5)));
    // a version 3 table has no second sights
    other[4] = 3;
    assert_eq!(Table::parse(&other).err(), Some(Malformed::Version(3)));
    assert_eq!(Table::parse(b"SRPX").err(), Some(Malformed::Magic));
}

fn row(out: &mut String, head: &str, bone: Option<BoneIndex>, matrices: &[Affine]) {
    use core::fmt::Write;
    out.push_str(head);
    if let Some(bone) = bone {
        write!(out, "\t{}", bone.0).unwrap();
    }
    for float in matrices.iter().flat_map(Affine::floats) {
        write!(out, "\t{float:?}").unwrap();
    }
    out.push('\n');
}

fn placed(out: &mut String, head: &str, at: &Placement) {
    row(out, head, Some(at.bone), &[at.offset]);
}

/// What `resolve` says of the sample table's guns, pinned in a checked-in
/// file: the client's Unicorn check (`check_attachments_026g.py`) runs the
/// game's code over `tests/sample-v4.srpm` and compares what the patch
/// wrote with these rows, so it needs no second `resolve`.
/// `SRP_WRITE_SAMPLE=1 cargo test -p weapon-modules` writes it.
///
/// A case is `case <gun> <scope> <muzzle> <grip> <second sight> <side>`
/// (dict ids, 0 none; the side left, right or -) and then `whole`, `refused <why>`, or `bare <body> <bones>` with a
/// row a thing: `part <model> <bone> <12 floats>`, `optic`, `eye
/// optic|gun_idle`, `second` (the second sight's frame), `eye2
/// optic|gun_idle`, `fire`, `wrist` (each a bone and twelve floats),
/// `fingers` (fifteen matrices), `effect <path>`.
#[test]
fn the_samples_loadouts_are_pinned() {
    use core::fmt::Write;
    let source = source();
    let bytes = encode(&source).unwrap();
    let table = Table::parse(&bytes).unwrap();
    let dict = |module: usize| DictId::new(source.modules[module].dict);
    let cases = [
        (AK_74M, None, None, None, None),
        (AK_74M, None, dict(DTK3), None, None),
        (AK_74M, None, None, dict(MIKI), None),
        (AK_74M, dict(PSO1), None, None, None),
        (AK_74M, dict(EOTECH), None, None, None),
        (AK_74M, dict(PSO1), dict(DTK3), dict(MIKI), None),
        (AK_74M_VICTORY, None, dict(DTK3), dict(MIKI), None),
        (FN_FNC, dict(EOTECH), None, dict(MIKI), None),
        (HK416, dict(EOTECH), None, dict(MIKI), None),
        (FN_P90, None, dict(TBRAKE), dict(MIKI), None),
        (FN_P90, None, dict(AEK_HIDER), dict(MIKI), None),
        (HK416, None, dict(DTK3), None, None),
        (HK416, None, None, None, right(DPPRO_SIDE)),
        (HK416, None, None, None, left(DPPRO_SIDE)),
        (HK416, dict(EOTECH), None, dict(MIKI), right(DPPRO_SIDE)),
        (FN_FNC, None, None, None, right(DPPRO_SIDE)),
    ];
    let mut out = String::new();
    for (gun, scope, muzzle, grip, second_sight) in cases {
        let id = |module: Option<DictId>| module.map_or(0, DictId::get);
        let side = match second_sight {
            Some((_, Side::Right)) => "right",
            Some((_, Side::Left)) => "left",
            None => "-",
        };
        writeln!(out, "case\t{gun}\t{}\t{}\t{}\t{}\t{side}", id(scope), id(muzzle), id(grip), id(second_sight.map(|(dict, _)| dict))).unwrap();
        let gun = table.gun(DictId::new(gun).unwrap()).unwrap();
        let bare = match resolve(gun, Wanted { scope, muzzle, grip, second_sight }) {
            Ok(Loadout::Whole) => {
                out.push_str("whole\n");
                continue;
            }
            Err(refusal) => {
                writeln!(out, "refused\t{refusal:?}").unwrap();
                continue;
            }
            Ok(Loadout::Bare(bare)) => bare,
        };
        writeln!(out, "bare\t{}\t{}", gun.bare().to_str().unwrap(), gun.bones()).unwrap();
        for part in bare.parts() {
            placed(&mut out, &std::format!("part\t{}", part.model().to_str().unwrap()), &part.at);
        }
        if let Some(optic) = &bare.optic {
            placed(&mut out, "optic", &optic.frame);
        }
        if let Some(eye) = &bare.eye {
            placed(&mut out, "eye\toptic", &eye.placed(EyeAnchor::Optic));
            placed(&mut out, "eye\tgun_idle", &eye.placed(EyeAnchor::GunIdle));
        }
        if let Some(second) = &bare.second {
            placed(&mut out, "second", &second.at);
            placed(&mut out, "eye2\toptic", &second.eye.placed(EyeAnchor::Optic));
            placed(&mut out, "eye2\tgun_idle", &second.eye.placed(EyeAnchor::GunIdle));
        }
        if let Some(fire_point) = &bare.fire_point {
            placed(&mut out, "fire", fire_point);
        }
        if let Some(hand) = &bare.left_hand {
            placed(&mut out, "wrist", &hand.wrist);
            row(&mut out, "fingers", None, &hand.fingers());
        }
        if let Some(effect) = bare.shot_effect {
            writeln!(out, "effect\t{}", effect.to_str().unwrap()).unwrap();
        }
    }
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/sample-v4.loadouts.tsv");
    if std::env::var_os("SRP_WRITE_SAMPLE").is_some() {
        std::fs::write(&path, &out).unwrap();
    }
    assert!(std::fs::read_to_string(&path).unwrap() == out, "tests/sample-v4.loadouts.tsv is not what resolve gives");
}

/// 0.69d0: the HK416's angled rails take the NcStar MPR45, whose
/// `angled_collim_short_r` takes a DeltaPoint Pro as a second sight
/// (`attach_params_3`: `aim_4` at its `aim`, `plane_slot: body_aim`). The
/// rail comes with the fit; the sight's eye is the second one, at the gun's
/// eye plane, and the gun's own eye stays the eye.
#[test]
fn a_second_sight_brings_its_offset_rail_and_a_second_eye() {
    let mut data = source();
    let hk = data.guns.iter_mut().find(|g| g.dict == HK416).unwrap();
    let donor_eye = Affine::translation(0.0, 0.1, 0.17);
    hk.slots.push((S::BodyAim, OFFSET_BONE, donor_eye));
    let bytes = encode(&data).unwrap();
    let table = Table::parse(&bytes).unwrap();
    let side = Wanted { second_sight: right(DPPRO_SIDE), ..Wanted::default() };
    let mut own_eye = None;
    with(&table, HK416, Wanted { grip: dict(MIKI), ..Wanted::default() }, |bare| own_eye = bare.eye);
    with(&table, HK416, side, |bare| {
        // nothing folds: the sights stand, the rail on the right angled rail, the sight on it
        assert_eq!(
            models(&bare),
            ["hk416_a5_rear_ironsight_on", "hk416_a5_front_ironsight_on", "hk416a5_i05x28_muzzlebrake", "ncstar_mpr45_picatinnyrail", "leupold_dppro_collimator"]
        );
        let [.., rail, sight] = bare.parts().collect::<Vec<_>>()[..] else { panic!() };
        assert_eq!(rail.at, on_offset(locator(44)));
        assert_eq!(sight.at, rail.at.child(&locator(24)));
        assert!(bare.optic.is_none());
        // the eye is the gun's own, as with nothing on the top rail
        assert_eq!(bare.eye, own_eye);
        let second = bare.second.unwrap();
        assert_eq!(second.at, sight.at);
        assert_eq!(second.module.aimed().unwrap().to_str().unwrap().rsplit('/').next(), Some("leupold_dppro_collimator_aim"));
        // its axes and its line of sight, at the plane of the gun's own eye
        let (line, eye) = (sight.at.child(&SIDE_EYE), second.eye.placed(EyeAnchor::Optic));
        assert_eq!((eye.bone, eye.offset.x, eye.offset.y, eye.offset.z), (line.bone, line.offset.x, line.offset.y, line.offset.z));
        let frame = |at: &Placement| locator_frame(&Affine::IDENTITY, at, &pose()[1].1, &Affine::IDENTITY);
        let plane = frame(&Placement { bone: BoneIndex(OFFSET_BONE), offset: donor_eye });
        let (moved, before) = (frame(&eye), frame(&line));
        let off = [moved.t[0] - plane.t[0], moved.t[1] - plane.t[1], moved.t[2] - plane.t[2]];
        assert!((off[0] * plane.z[0] + off[1] * plane.z[1] + off[2] * plane.z[2]).abs() < 1e-5);
        // moved along its own line, and by more than a little
        let step = [moved.t[0] - before.t[0], moved.t[1] - before.t[1], moved.t[2] - before.t[2]];
        let along = step[0] * before.z[0] + step[1] * before.z[1] + step[2] * before.z[2];
        assert!(along.abs() > 0.05 && (0..3).all(|k| (step[k] - along * before.z[k]).abs() < 1e-5));
    });
    // on the left: the rail on the left angled rail, the sight in its slot there
    with(&table, HK416, Wanted { second_sight: left(DPPRO_SIDE), ..Wanted::default() }, |bare| {
        let [.., rail, sight] = bare.parts().collect::<Vec<_>>()[..] else { panic!() };
        assert_eq!(rail.at, on_offset(locator(43)));
        assert_eq!(sight.at, rail.at.child(&locator(23)));
        assert_eq!(bare.second.unwrap().at, sight.at);
    });
    // with a scope on top: the scope's eye is the eye, the second sight stands where it stood
    let mut alone = None;
    with(&table, HK416, side, |bare| alone = bare.second.map(|second| (second.at, second.eye)));
    with(&table, HK416, Wanted { scope: dict(EOTECH), grip: dict(MIKI), ..side }, |bare| {
        let optic = bare.optic.unwrap();
        assert_eq!(bare.eye.unwrap().placed(EyeAnchor::Optic), optic.frame.child(&locator(12)));
        assert_eq!(bare.second.map(|second| (second.at, second.eye)), alone);
        assert_eq!(models(&bare).iter().filter(|model| model.starts_with("ncstar_mpr45")).count(), 1);
    });
}

#[test]
fn a_second_sight_needs_its_angled_rail() {
    let bytes = encode(&source()).unwrap();
    let table = Table::parse(&bytes).unwrap();
    let side = Wanted { second_sight: right(DPPRO_SIDE), ..Wanted::default() };
    // the FN FNC names the fit but has no angled rail
    assert_eq!(refusal(&table, FN_FNC, side), Refusal::NoSlot(Kind::SecondSight));
    // the AK-74M takes none
    assert_eq!(refusal(&table, AK_74M, side), Refusal::NoFit(Kind::SecondSight));
    // a second sight is no scope, and a scope no second sight
    assert_eq!(refusal(&table, HK416, Wanted { scope: dict(DPPRO_SIDE), ..Wanted::default() }), Refusal::NoFit(Kind::Scope));
    assert_eq!(refusal(&table, HK416, Wanted { second_sight: right(EOTECH), ..Wanted::default() }), Refusal::NoFit(Kind::SecondSight));
    // a second sight without its eye is no table
    let module = |fault| Malformed::Module { module: DPPRO_SIDE as u16, fault };
    assert_eq!(refused(|s| s.modules[DPPRO_SIDE].attaches[0].eye = None), module(ModuleFault::NoEye(S::AngledCollimShortL)));
}

/// A version 3 table (`tests/sample-v3.srpm`, the format before second
/// sights) is still read, as one without any.
#[test]
fn a_version_3_table_is_read() {
    let kept = std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/sample-v3.srpm")).unwrap();
    let table = Table::parse(&kept).unwrap();
    let hk = table.gun(DictId::new(HK416).unwrap()).unwrap();
    assert_eq!(hk.fits().count(), 2);
    assert!(table.guns().flat_map(|gun| gun.fits()).all(|module| module.role() != Role::Fitted(Kind::SecondSight)));
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
