//! Upload validation before a snapshot enters any server state or relay.
use std::fmt;

use super::registry::Formed;
use srp_validate::{
    KITS, MAX_PLAYERS, Refusal,
    world::{self, Context, Profile},
};
use survarium::v026g::match_msgs::static_info::KIT_USABLE;

/// A refused upload, with the match it was checked as: the clients take
/// the mode, batteries and respawn points from their own rule objects, so
/// a line naming them shows a server whose game data differs from theirs.
pub(super) struct Refused {
    refusal: Refusal,
    game_mode: u8,
    batteries: usize,
    respawn_points: usize,
    places: usize,
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (checked as mode {}, {} batteries, {} respawn points, {} places)",
            self.refusal, self.game_mode, self.batteries, self.respawn_points, self.places
        )
    }
}

pub(super) fn check(body: &[u8], time: u32, places: usize, formed: &Formed) -> Result<(), Refused> {
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
    let map = formed.played.map();
    let context = Context {
        time,
        places,
        game_mode: map.mode.rules().number(),
        batteries: map.victory_items_count as usize,
        respawn_points: Some(map.respawn_points),
        profiles: Some(&profiles[..known]),
        // Current server game data does not expose these native tables. Do
        // not invent bounds or reject valid data using an assumed layout.
        artefacts: None,
        objects: None,
        inventory: None,
    };
    world::validate(body, &context).map_err(|refusal| Refused {
        refusal,
        game_mode: context.game_mode,
        batteries: context.batteries,
        respawn_points: map.respawn_points,
        places,
    })
}
