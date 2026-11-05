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

/// Whether one of `player`'s artefact containers holds an active Onyx (state 4, its virtual
/// table's enable_active Onyx's).
fn active_onyx(player: raw::Player) -> bool {
    // SAFETY: the landing's live player owns its inventory and the container array it points at
    // (`artefact_slot_count` entries); each item there is an artefact.
    unsafe {
        let Some(inventory) = player.inventory() else {
            return false;
        };
        let Some(first) = inventory.artefact_slots_begin() else {
            return false;
        };
        (0..usize::from(*inventory.artefact_slot_count())).any(|index| {
            let Some(item) = first.at(index).item() else {
                return false;
            };
            let artefact = item.artefact();
            *artefact.state() == 4
                && artefact
                    .vtable()
                    .is_some_and(|table| *table.enable_active() == raw::image::ONYX_ACTIVE)
        })
    }
}

/// Onyx II: the landing of a player with the skill and an active Onyx does no damage.
pub fn lands_unhurt(ctx: &Context<'_>) -> bool {
    // SAFETY: the landing's ECX is its live player.
    let Some(player) = (unsafe { raw::site::artefact_landing::player(ctx) }) else {
        return false;
    };
    skills(player).onyx() && active_onyx(player)
}

/// Larkspur II: a Larkspur's actual activation (never a restore) fills its holder's stamina.
pub fn larkspur(ctx: &Context<'_>) {
    use raw::site::medical_artefact as site;
    // SAFETY: the actual activation owns this item, its inventory and holder; the holder's
    // stamina is embedded in it.
    unsafe {
        let Some(item) = site::item(ctx) else {
            return;
        };
        if !item.vtable().is_some_and(|table| *table.enable_active() == raw::image::LIFEBONE_ACTIVE) {
            return;
        }
        let Some(holder) = item.item().inventory().and_then(|inventory| inventory.holder()) else {
            return;
        };
        let player = holder.player();
        if skills(player).larkspur() {
            let stamina = player.stamina();
            *stamina.current() = *stamina.maximum();
        }
    }
}

/// Spring II at a Spring's passive link (patch 54's entry): its two reload nodes' values from
/// its holder's skills, before the passive's goes in.
pub fn spring_reload(ctx: &Context<'_>) {
    use raw::site::artefact_spring_reload as site;
    // SAFETY: the linking Spring, its inventory and holder are live for the link; the two
    // nodes are its own (patch 54 grew the object to 3F8h).
    unsafe {
        let Some(spring) = site::frame(ctx).and_then(|frame| frame.spring()) else {
            return;
        };
        let Some(holder) = spring.item().inventory().and_then(|inventory| inventory.holder()) else {
            return;
        };
        let (passive, active) = skills(holder.player()).spring_reload();
        *spring.passive_reload() = passive;
        *spring.active_reload() = active;
    }
}
