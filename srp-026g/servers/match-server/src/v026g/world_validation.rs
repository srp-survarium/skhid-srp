//! Upload validation before a snapshot enters any server state or relay.
use std::fmt;

use super::registry::Formed;
use srp_validate::{
    KITS, MAX_PLAYERS, Part, Refusal,
    world::{self, Context, Held, Inventory, Item, Objects, Profile},
};
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::item_params;
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
    // Each place's kits as its game makes their objects: a player's state
    // is read with them to its exact end. A profile the game would make an
    // object of nothing for leaves the states read to their inventories.
    let dictionary = Dictionary::shared();
    let built: Result<Vec<[Vec<Item<'static>>; KITS]>, String> = formed.profiles[..known]
        .iter()
        .map(|profile| {
            Ok([
                profile.inventory(0, dictionary)?,
                profile.inventory(1, dictionary)?,
                profile.inventory(2, dictionary)?,
            ])
        })
        .collect();
    let built = built
        .inspect_err(|why| {
            log::warn!(
                "Match {:#x}: its players' states are read to their inventories only: {why}",
                formed.match_id
            );
        })
        .ok();
    let kits: Option<Vec<[&[Item]; KITS]>> = built
        .as_ref()
        .map(|built| built.iter().map(|[a, b, c]| [&a[..], &b[..], &c[..]]).collect());
    let inventory = kits.as_ref().map(|kits| Inventory {
        kits,
        body_parts: item_params::body_parts(),
    });
    let refused = |refusal| Refused {
        refusal,
        game_mode: map.mode.rules().number(),
        batteries: map.victory_items_count as usize,
        respawn_points: map.respawn_points,
        places,
    };
    // The world's game objects begin with the grenades and traps of every
    // place its game has bound, the newest first (`world::objects`): the
    // first roster's from the start, then each late joiner's once that
    // game installed it, in the order they joined. Which of the late
    // joiners the uploading game has installed the server does not know,
    // so a world is one of a game that installed the first `bound` of them.
    let mut first = None;
    for bound in (formed.initial_players.min(known)..=known).rev() {
        let held: Vec<Held> = formed.profiles[..bound]
            .iter()
            .rev()
            .flat_map(|profile| profile.held())
            .collect();
        let objects = Objects {
            held: &held,
            map: map.world_objects,
        };
        let context = Context {
            time,
            places,
            game_mode: map.mode.rules().number(),
            batteries: map.victory_items_count as usize,
            respawn_points: Some(map.respawn_points),
            profiles: Some(&profiles[..known]),
            // The artefacts table is the game's live one; the inventory's
            // artefact ids are bounded by its own check (patch 115).
            artefacts: None,
            objects: Some(&objects),
            inventory: inventory.as_ref(),
        };
        match world::validate(body, &context) {
            Ok(()) => return Ok(()),
            // Another count of installed places changes the game objects only.
            Err(refusal) if refusal.part == Part::Objects => {
                first.get_or_insert(refusal);
            }
            Err(refusal) => return Err(refused(first.unwrap_or(refusal))),
        }
    }
    Err(refused(first.expect("the first roster is no more than the known places")))
}
