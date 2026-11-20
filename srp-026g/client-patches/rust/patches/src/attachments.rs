//! Patch 161: a weapon's scope, eye, left hand and shells stand where its
//! fitted modules put them (`docs/notes/attachments.md`, "The client's
//! runtime"). A weapon config names its gun and what is fitted to it;
//! `weapon_modules::resolve` says where everything is; the game keeps all of
//! it as data it fills once from the weapon's model, and here that data is
//! written again after the game filled it:
//!
//! - `on_dictionary_requests`, `on_dictionary_configs` (the items
//!   dictionary cook's query and its callback): the modules table is asked
//!   for with the dictionary's configs and kept, before any weapon is made;
//! - `on_ready`, `on_loaded` (the cook's callback, its entry and after
//!   load_weapon): the scope's frame and the eye, two locator records, and
//!   the left hand's fingers; the weapon is remembered;
//! - `on_hands` (weapon::activate, after it filled the hand IK's records):
//!   the left hand's place;
//! - `on_fingers_view` (the game refills the fingers when the user's view
//!   changes): the fingers again;
//! - `on_shells`: the shells keep the frame the scope had before it moved;
//! - `on_release`: the weapon is forgotten.
//!
//! A grip carries one set of hand locators, the idle one; the lobby's clips
//! ask for the lobby set, which gets the same.
//!
//! What a match computes does not change: every value written is read by
//! drawing alone (the scope's models, the first view's gun before the
//! camera, the arms' IK, an effect's place).

#![forbid(unsafe_code)]

use weapon_modules::{Bare, EyeAnchor, FingerJoints, Hand, Loadout, Placement, resolve};

use crate::vostok::attachments::{self, FingerCorrector, HandSet, Kept, Named, Records};
use crate::vostok::{Context, Flow, NoExit, OrigFingersSetWeaponModel, Thread};

/// What the eye is fixed to while aiming through an optic: the optic (its
/// reticle centred at every moment; the shot's kick of the optic's bone not
/// seen while aiming), or the gun's aimed idle pose (the kick seen).
pub const EYE: EyeAnchor = EyeAnchor::Optic;

/// Whose finger joints the left hand keeps on a grip: the gun's own (the
/// 0.26g hand's, the grip turning each finger), or the grip's whole (0.69d's
/// hand).
pub const FINGER_JOINTS: FingerJoints = FingerJoints::Gun;

const HAND_SETS: [HandSet; 2] = [HandSet::Idle, HandSet::Lobby];

/// The bare gun `named` resolves to: None for a gun the table lacks, a fit
/// it refuses, or nothing fitted (the game's own weapon stays as it is).
fn bare(thread: &Thread, named: Named, with: impl FnOnce(&Bare<'_>)) {
    let Some(table) = attachments::modules(thread) else { return };
    if let Some(Ok(Loadout::Bare(bare))) = table.gun(named.gun).map(|gun| resolve(gun, named.wanted)) {
        with(&bare);
    }
}

/// The shells' frame as a matrix times the scope's new one: where the scope
/// was, when both records hang on one bone; else the shells go with the
/// scope.
pub fn shells(was: &Placement, now: &Placement) -> Option<weapon_modules::Affine> {
    was.frame_over(now)
}

fn fingers(weapon: &mut Records<'_>, hand: &Hand) {
    for set in HAND_SETS {
        let own = weapon.left_fingers(set);
        weapon.set_left_fingers(set, &hand.fingers(FINGER_JOINTS, &own));
    }
}

/// items_dictionary_cook::translate_query before its query (0x2C8963).
pub fn on_dictionary_requests(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if attachments::ask(ctx) { Flow::Skip } else { Flow::Continue }
}

/// items_dictionary_cook::on_configs_loaded's entry (0x2C8AA0).
pub fn on_dictionary_configs(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Err(why) = attachments::arrived(ctx) {
        attachments::refused(why);
    }
    Flow::Continue
}

/// on_weapon_subresources_ready's entry (0x24ACF0).
pub fn on_ready(ctx: &mut Context<'_>) -> Flow<NoExit> {
    attachments::park(ctx);
    Flow::Continue
}

/// on_weapon_subresources_ready after load_weapon (0x24B0E1).
pub fn on_loaded(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(mut weapon) = Records::loaded(ctx) else { return Flow::Continue };
    // a weapon made anew at an address keeps nothing of the one before it
    attachments::forget(weapon.thread(), weapon.id());
    let Some(named) = weapon.parked() else { return Flow::Continue };
    bare(weapon.thread(), named, |bare| {
        let was = weapon.scope();
        if let Some(optic) = &bare.optic {
            weapon.set_scope(&optic.frame);
        }
        if let Some(eye) = &bare.eye {
            weapon.set_eye(&eye.placed(EYE));
        }
        if let Some(hand) = &bare.left_hand {
            fingers(&mut weapon, hand);
        }
        let shells = bare.optic.as_ref().and_then(|optic| shells(&was, &optic.frame));
        attachments::remember(weapon.thread(), weapon.id(), Kept { named, shells });
    });
    Flow::Continue
}

/// weapon::activate after set_weapon_locators (0x244649).
pub fn on_hands(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(mut weapon) = Records::activated(ctx) else { return Flow::Continue };
    let Some(kept) = attachments::kept(weapon.thread(), weapon.id()) else { return Flow::Continue };
    bare(weapon.thread(), kept.named, |bare| {
        if let Some(hand) = &bare.left_hand {
            for set in HAND_SETS {
                weapon.set_left_hand(set, &hand.wrist);
            }
        }
    });
    Flow::Continue
}

/// correct_fingers' refill of the corrector at a change of view (0x298664).
pub fn on_fingers_view(refill: &OrigFingersSetWeaponModel, corrector: &mut FingerCorrector<'_>, instance: u32, first_view: u32) {
    refill.call(corrector, instance, first_view);
    let mut weapon = corrector.weapon();
    let Some(kept) = attachments::kept(weapon.thread(), weapon.id()) else { return };
    bare(weapon.thread(), kept.named, |bare| {
        if let Some(hand) = &bare.left_hand {
            fingers(&mut weapon, hand);
        }
    });
}

/// play_weapon_shell_pfx at its read of the scope's frame (0x2443EE).
pub fn on_shells(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(weapon) = Records::ejecting(ctx) else { return Flow::Continue };
    let Some(shells) = attachments::kept(weapon.thread(), weapon.id()).and_then(|kept| kept.shells) else {
        return Flow::Continue;
    };
    let frame = shells.then(&weapon.scope_frame());
    attachments::eject_at(ctx, &frame);
    Flow::Skip
}

/// weapon_cook::delete_resource's entry (0x24B190).
pub fn on_release(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(weapon) = attachments::released(ctx) {
        attachments::forget(ctx.thread(), weapon);
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use weapon_modules::{Affine, BoneIndex, locator_frame};

    use super::*;

    fn turned(x: f32, y: f32, z: f32) -> Affine {
        Affine { x: [0.0, 0.0, -1.0], y: [0.0, 1.0, 0.0], z: [1.0, 0.0, 0.0], t: [x, y, z] }
    }

    /// The engine's own formula for a record's frame (calculate_locator
    /// 0x243BC0): the shells' matrix times the moved scope's frame is the
    /// frame the scope had, for any bone and any place of the gun.
    #[test]
    fn the_shells_stay_where_the_scope_was() {
        let was = Placement { bone: BoneIndex(1), offset: Affine::translation(0.0, 0.06, 0.02) };
        let now = Placement { bone: BoneIndex(1), offset: turned(0.01, 0.09, 0.11) };
        let fix = shells(&was, &now).unwrap();
        for (bone, world) in [(Affine::IDENTITY, Affine::IDENTITY), (turned(0.0, 0.01, -0.03), turned(10.0, 1.5, -4.0))] {
            let frame = |at: &Placement| locator_frame(&Affine::IDENTITY, at, &bone, &world);
            let (kept, old) = (fix.then(&frame(&now)), frame(&was));
            assert!(kept.floats().iter().zip(old.floats()).all(|(a, b)| (a - b).abs() < 1e-5));
        }
        // on another bone the two frames move apart: the shells go with the scope
        assert!(shells(&was, &Placement { bone: BoneIndex(2), ..now }).is_none());
    }
}
