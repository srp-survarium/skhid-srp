//! Patch 161: a weapon's fitted modules (`docs/notes/attachments.md`, "The
//! client's runtime"). A weapon config names its gun and what is fitted to
//! it; `weapon_modules::resolve` says what stands where; the game is handed
//! that in its own terms:
//!
//! - `on_dictionary_requests`, `on_dictionary_configs` (the items
//!   dictionary cook's query and its callback): the modules table is asked
//!   for with the dictionary's configs and kept, before any weapon is made;
//! - `on_scope_dict`, `on_scope_result` (the weapon cook and its callback,
//!   where each asks the config for the weapon's scope): the scope item is
//!   the one `data/srp_scope` names, so a scope's modification is one add to
//!   `data` like a device's and a grip's, and `addons/rifle_scope_dict_id`
//!   stays the gun's own default optic;
//! - `on_request_room`, `on_asked` (the weapon cook, as it asks for the
//!   weapon's own resources): the bare body in place of the weapon's model,
//!   and each part's model;
//! - `on_ready`, `on_loaded` (the cook's callback, its entry and after
//!   load_weapon): the parts' models kept with their placements; the fire
//!   point, the scope's frame and the eye, three locator records, and the
//!   left hand's fingers, all data the game filled once from the weapon's
//!   model and written again here;
//! - `on_draw`, `on_culled`, `on_hidden` (weapon::draw, a culled player's
//!   weapon, on_hide): the parts in the scene with the weapon's model;
//! - `on_hands` (weapon::activate, after it filled the hand IK's records):
//!   the left hand's place;
//! - `on_fingers_view` (the game refills the fingers when the user's view
//!   changes): the fingers again;
//! - `on_shells`: the shells keep the frame the scope had before it moved;
//! - `on_release`: the parts' models given back, the weapon forgotten.
//!
//! A grip carries one set of hand locators, the idle one; the lobby's clips
//! ask for the lobby set, which gets the same.
//!
//! What a match computes does not change: every value written is read by
//! drawing alone (the models, the first view's gun before the camera, the
//! arms' IK, an effect's place).

#![forbid(unsafe_code)]

pub(crate) mod view;
pub use view::{on_foreground, on_view_begin, on_view_camera, on_view_end, on_view_frame, on_view_no_gun, on_view_submit};

use weapon_modules::{Bare, EyeAnchor, Gun, Hand, Loadout, Placement, resolve};

use crate::vostok::attachments::{self, Cook, FingerCorrector, HandSet, Leaving, Named, Records};
use crate::vostok::{
    AttachmentsOnScopeDictExit, AttachmentsOnScopeResultExit, Context, Flow, NoExit, OrigFingersSetWeaponModel, Thread,
};

/// What the eye is fixed to while aiming through an optic: the gun's aimed
/// idle pose, so the reticle is centred at rest and the shot's kick of the
/// optic's bone is seen (the owner's ask behind patch 165). The optic's own
/// anchor never shows it. Under patch 165 both give one eye
/// (weapon-modules' test of the two beside its rule; attachments.md, "The
/// two switches").
pub const EYE: EyeAnchor = EyeAnchor::GunIdle;

const HAND_SETS: [HandSet; 2] = [HandSet::Idle, HandSet::Lobby];

/// The gun `named` is and its bare loadout: nothing for a gun the table
/// lacks, a fit it refuses, or nothing fitted (the game's own weapon stays
/// as it is).
fn bare(thread: &Thread, named: Named, with: impl FnOnce(Gun<'static>, &Bare<'static>)) {
    let Some(gun) = attachments::modules(thread).and_then(|table| table.gun(named.gun)) else { return };
    if let Ok(Loadout::Bare(bare)) = resolve(gun, named.wanted) {
        with(gun, &bare);
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
        weapon.set_left_fingers(set, &hand.fingers());
    }
}

/// items_dictionary_cook::translate_query before its query (0x2C8963).
pub fn on_dictionary_requests(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if attachments::ask(ctx) { Flow::Skip } else { Flow::Continue }
}

/// items_dictionary_cook::on_configs_loaded's entry (0x2C8AA0).
pub fn on_dictionary_configs(ctx: &mut Context<'_>) -> Flow<NoExit> {
    crate::vostok::weapon_view::load_profiles(ctx);
    if let Err(why) = attachments::arrived(ctx) {
        attachments::refused(why);
    }
    Flow::Continue
}

/// on_weapon_config_loaded at its scope (0x24AA15).
pub fn on_scope_dict(ctx: &mut Context<'_>) -> Flow<AttachmentsOnScopeDictExit> {
    if attachments::scope_request(ctx) { Flow::Exit(AttachmentsOnScopeDictExit::Scoped) } else { Flow::Continue }
}

/// on_weapon_subresources_ready before it asks whether the weapon has a
/// scope (0x24AD13).
pub fn on_scope_result(ctx: &mut Context<'_>) -> Flow<AttachmentsOnScopeResultExit> {
    if attachments::scope_result(ctx) { Flow::Exit(AttachmentsOnScopeResultExit::Scoped) } else { Flow::Continue }
}

/// on_weapon_config_loaded before it sizes its requests (0x24AAB4).
pub fn on_request_room(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(named) = attachments::cooking(ctx, Cook::Room) {
        let mut parts = 0;
        bare(ctx.thread(), named, |_, bare| parts = bare.parts().count());
        attachments::more_requests(ctx, parts);
    }
    Flow::Continue
}

/// on_weapon_config_loaded with its weapon made and its requests pushed
/// (0x24AC30).
pub fn on_asked(ctx: &mut Context<'_>) -> Flow<NoExit> {
    attachments::fresh(ctx);
    if let Some(named) = attachments::cooking(ctx, Cook::Asked) {
        bare(ctx.thread(), named, |gun, bare| {
            attachments::request_parts(ctx, named, gun.bare(), bare.parts().map(|part| part.model()));
        });
    }
    Flow::Continue
}

/// on_weapon_subresources_ready's entry (0x24ACF0).
pub fn on_ready(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(asked) = attachments::asked(ctx) {
        let named = asked.named();
        bare(ctx.thread(), named, |_, bare| asked.hold(bare.parts().map(|part| part.at)));
    }
    Flow::Continue
}

/// on_weapon_subresources_ready after load_weapon (0x24B0E1).
pub fn on_loaded(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(mut weapon) = Records::loaded(ctx) else { return Flow::Continue };
    let Some(kept) = attachments::kept(weapon.thread(), weapon.id()) else { return Flow::Continue };
    bare(weapon.thread(), kept.named, |gun, bare| {
        // every placement of this gun names a bone below the table's count
        if weapon.bones().is_none_or(|bones| usize::from(gun.bones()) > bones) {
            return attachments::misfit(weapon.thread(), weapon.id(), kept.named);
        }
        let was = weapon.scope();
        if let Some(optic) = &bare.optic {
            weapon.set_scope(&optic.frame);
        }
        if let Some(eye) = &bare.eye {
            weapon.set_eye(&eye.placed(EYE));
        }
        if let Some(fire_point) = &bare.fire_point {
            weapon.set_fire_point(fire_point);
        }
        if let Some(hand) = &bare.left_hand {
            fingers(&mut weapon, hand);
        }
        let shells = bare.optic.as_ref().and_then(|optic| shells(&was, &optic.frame));
        attachments::set_shells(weapon.thread(), weapon.id(), shells);
    });
    Flow::Continue
}

/// weapon::draw after the weapon model's update (0x245017).
pub fn on_draw(ctx: &mut Context<'_>) -> Flow<NoExit> {
    attachments::draw(ctx);
    Flow::Continue
}

/// weapon::on_culled_player_draw (0x245353).
pub fn on_culled(ctx: &mut Context<'_>) -> Flow<NoExit> {
    attachments::leave(ctx, Leaving::Culled);
    Flow::Continue
}

/// weapon::on_hide (0x244017).
pub fn on_hidden(ctx: &mut Context<'_>) -> Flow<NoExit> {
    attachments::leave(ctx, Leaving::Hidden);
    Flow::Continue
}

/// weapon::activate after set_weapon_locators (0x244649).
pub fn on_hands(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(mut weapon) = Records::activated(ctx) else { return Flow::Continue };
    let Some(kept) = attachments::kept(weapon.thread(), weapon.id()) else { return Flow::Continue };
    bare(weapon.thread(), kept.named, |_, bare| {
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
    bare(weapon.thread(), kept.named, |_, bare| {
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
    attachments::released(ctx);
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
