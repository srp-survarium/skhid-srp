//! Patch 199: the lobby is given the kit it shows whole, in one call (docs/notes/client_patch.md
//! "The kit shown, in one call").
//!
//! lobby_menu::on_profile_changed gives the movie a kit in two calls: its items
//! (root.fill_player_profile_items, the kit named), then its counts
//! (root.update_profile_slots_count, no kit named). A movie that drew the items before the counts
//! laid them on the last kit's counts (revision 72: the pockets the last kit lacked sent their
//! items back). The items' call is not made; at the counts' call the items value is given the
//! counts, `item_slots` and `artifact_slots` (the stock paper doll opens its slots from them,
//! PlayerProfileBase.fillProfileItems), and given the movie in its place. Only the lobby: nothing
//! a match computes.
#![forbid(unsafe_code)]

use crate::vostok::{Context, Flow, ProfileKitOnCountsExit, ProfileKitOnItemsExit, profile_kit};

/// on_profile_changed's root.fill_player_profile_items: not made (the kit is given at its counts).
pub fn on_items(_ctx: &mut Context<'_>) -> Flow<ProfileKitOnItemsExit> {
    Flow::Exit(ProfileKitOnItemsExit::Skipped)
}

/// on_profile_changed's root.update_profile_slots_count: the whole kit given in its place (the
/// stock call after the items when the counts could not go in them).
pub fn on_counts(ctx: &mut Context<'_>) -> Flow<ProfileKitOnCountsExit> {
    if profile_kit::give_whole(ctx) {
        Flow::Exit(ProfileKitOnCountsExit::Given)
    } else {
        Flow::Continue
    }
}
