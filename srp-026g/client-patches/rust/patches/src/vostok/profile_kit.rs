//! The kit the lobby shows (patch 199, crate::profile_kit): lobby_menu::on_profile_changed
//! (0x222330) gives the movie the kit's items (root.fill_player_profile_items, 0x222A23), then,
//! once it has worked them out, its counts (root.update_profile_slots_count, 0x222CD6).
use super::Context;
use super::ui::{movie_of, set_uint_member};
use crate::raw;

/// The kit's items value given the movie at `lobby_menu.profile_counts` (0x222CA1), in one
/// root.fill_player_profile_items, with its counts in it: `item_slots` (the pockets) and
/// `artifact_slots` (the containers). Whether the counts went in; when they did not (no AS3
/// object), the items are given alone and the stock counts' call is the caller's to make.
pub fn give_whole(ctx: &Context<'_>) -> bool {
    use raw::site::lobby_menu_profile_counts as site;
    // SAFETY: the site is on_profile_changed's own, on the lobby's thread: ESI the lobby menu, ESP
    // its frame, the kit's value made for the items (0x222A15) and held until the function's end,
    // its counts made (0x222AE5..0x222B5A).
    unsafe {
        let (Some(menu), Some(frame)) = (site::menu(ctx), site::frame(ctx)) else { return false };
        let Some(movie) = movie_of(menu.movie()) else { return false };
        let kit = frame.kit();
        let counted = set_uint_member(kit, c"item_slots", *frame.pockets())
            && set_uint_member(kit, c"artifact_slots", u32::from(*frame.containers()));
        movie.invoke_value(c"root.fill_player_profile_items", kit);
        counted
    }
}
