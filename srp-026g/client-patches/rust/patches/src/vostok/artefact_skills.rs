//! The artefact tree's native sites (patch 109, artefact-skills.md): the kit's packed skills
//! (modifier 40) of the player a site's object belongs to.
use super::{Context, medical::kit_modifiers};
use crate::{
    artefact_skills::{HitKind, Skills},
    raw,
};

/// The packed skills of `player`'s current kit; none when the payload is not valid.
fn skills(player: raw::Player) -> Skills {
    kit_modifiers(player)
        // SAFETY: the generated modifier field belongs to the live current kit.
        .and_then(|m| Skills::parse(unsafe { *m.artefact_ranks() }))
        .unwrap_or_default()
}

/// Resistance: XMM0 the part's summed armour for the hit's type, before the piercing's
/// subtraction (body_part_parameters::hit_by_type 0x32A083).
pub fn isolation(ctx: &mut Context<'_>) {
    use raw::site::artefact_isolation as site;
    // SAFETY: hit_by_type's pool, its model and the model's player live for the hit; EDI is
    // the hit's type there.
    let (kind, player) = unsafe {
        let kind = HitKind::parse(site::kind(ctx));
        let player = site::pool(ctx).and_then(|pool| pool.model()).and_then(|model| model.owner());
        (kind, player)
    };
    let (Some(kind), Some(player)) = (kind, player) else {
        return;
    };
    let armour = skills(player).armour(ctx.xmm_f32(0), kind);
    ctx.set_xmm_f32(0, armour);
}
