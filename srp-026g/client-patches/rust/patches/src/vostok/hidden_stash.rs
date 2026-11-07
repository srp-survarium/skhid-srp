//! Hidden Stash's native sites (patch 109, crate::hidden_stash): the anomaly's take, can_use's
//! container test and the two death entries. The run's reads and writes are the inventory's
//! (vostok::inventory: `stash`, `open_for_anomaly`, `empty_stash`), so every take of this crate
//! (Keeper's, patch 78's) opens the stash the same way.
use super::inventory::{ContainerSpace, Inventory};
use super::log::{Piece::{Dec, Text}, line};
use super::world::read;
use super::{Context, GameCell};
use crate::raw;

/// Which death entry a hook is at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Death {
    /// base_player::on_player_death (0x2D77C0).
    Base,
    /// The client player's override (0x1F28E0).
    Client,
}

/// use_execute before transfer_artefact (`artefact_container.transfer`): EBX the user.
pub fn open_for_anomaly(ctx: &Context<'_>) {
    use raw::site::artefact_container_transfer as site;
    // SAFETY: use_execute's user, alive on the game's thread through the take.
    let Some(user) = (unsafe { site::user(ctx) }) else { return };
    if let Some(mut inventory) = Inventory::of_player(user, ctx.thread()) {
        inventory.open_for_anomaly();
    }
}

/// can_use's container test (`hidden_stash.can_use`): true when the user has no container but
/// a stash to open, so the anomaly is usable (the next test, no carried item, still applies).
pub fn can_use_with_stash(ctx: &Context<'_>) -> bool {
    use raw::site::hidden_stash_can_use as site;
    // SAFETY: can_use's user (ESI) and its inventory (EAX), alive on the game's thread.
    let (Some(user), Some(inventory)) = (unsafe { site::player(ctx) }, unsafe { site::inventory(ctx) }) else {
        return false;
    };
    // SAFETY: the live inventory's container count.
    if unsafe { read(inventory.artefact_slot_count()) } != 0 {
        return false;
    }
    Inventory::of_player(user, ctx.thread()).is_some_and(|i| i.container_space() == ContainerSpace::Available)
}

/// The HUD's positions the last loop filled, by its ui (presentation only: nothing a match
/// computes reads it).
static SHOWN: GameCell<Option<(u32, u8)>> = GameCell::new(None);

/// update_local_player_state after its loop (`hud.quick_slots_end`): ESI the ui, EDI the
/// positions filled. Each position a longer list left is shown empty by the game's own
/// update_quick_slot and its cached description zeroed, as clear_player leaves one, so the next
/// item shown there is drawn whatever it is.
pub fn clear_stale_positions(ctx: &Context<'_>) {
    use raw::site::hud_quick_slots_end as site;
    // SAFETY: the loop's ui (ESI), alive through the hook; EDI its count.
    let (Some(ui), filled) = (unsafe { site::ui(ctx) }, unsafe { site::filled(ctx) }) else { return };
    let filled = u8::try_from(filled).unwrap_or(u8::MAX);
    let before = SHOWN.get(ctx.thread()).filter(|&(at, _)| at == ui.address()).map_or(0, |(_, n)| n);
    for position in crate::hidden_stash::stale(filled, before) {
        // SAFETY: the live ui, the hud's own function for one of its six positions (ESI the ui,
        // the position pushed), then that position's cache, inside the ui.
        unsafe {
            raw::engine_update_quick_slot(Some(ui), u32::from(position));
            let cache = ui.quick_slots().at(usize::from(position));
            cache.low().write_volatile(0);
            cache.high().write_volatile(0);
        }
    }
    SHOWN.set(ctx.thread(), Some((ui.address(), filled.min(6))));
}

/// A death entry: the dying player's open stash goes back.
pub fn empty_at_death(ctx: &Context<'_>, death: Death) {
    // SAFETY: the death entry's ECX, the live dying player, before its inventory's death.
    let player = unsafe {
        match death {
            Death::Base => raw::site::carry_death::player(ctx),
            Death::Client => raw::site::carry_client_death::player(ctx),
        }
    };
    let Some(player) = player else { return };
    let Some(mut inventory) = Inventory::of_player(player, ctx.thread()) else { return };
    if let Some(held) = inventory.empty_stash() {
        // SAFETY: the live player's place byte.
        let place = unsafe { read(player.place()) };
        line(&[Text("hidden stash: place "), Dec(place.into()), Text(if held { " loses its artefact" } else { " closes empty" })]);
    }
}
