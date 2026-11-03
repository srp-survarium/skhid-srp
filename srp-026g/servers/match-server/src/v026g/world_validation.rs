//! Upload validation before a snapshot enters any server state or relay.
use super::registry::Formed;
use srp_validate::{
    KITS, MAX_PLAYERS, Refusal,
    world::{self, Context, Profile},
};
use survarium::v026g::match_msgs::static_info::KIT_USABLE;

pub(super) fn check(body: &[u8], time: u32, places: usize, formed: &Formed) -> Result<(), Refusal> {
    let mut profiles = [Profile::default(); MAX_PLAYERS];
    for (target, source) in profiles.iter_mut().zip(&formed.profiles) {
        target.items = source.items.len();
        target.usable_kits = source
            .kits
            .iter()
            .take(KITS)
            .enumerate()
            .filter(|(_, kit)| kit.state == KIT_USABLE)
            .fold(0, |mask, (kit, _)| mask | 1 << kit);
    }
    let known = formed.profiles.len().min(MAX_PLAYERS);
    let context = Context {
        time,
        places,
        game_mode: formed.map.mode,
        batteries: formed.map.victory_items_count as usize,
        respawn_points: Some(formed.map.respawn_points),
        profiles: Some(&profiles[..known]),
        // Current server game data does not expose these native tables. Do
        // not invent bounds or reject valid data using an assumed layout.
        artefacts: None,
        objects: None,
        inventory: None,
    };
    world::validate(body, &context)
}
