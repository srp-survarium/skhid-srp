//! Upload validation before a snapshot enters any server state or relay;
//! the same check of recorded worlds offline (`srp-world-check`,
//! [`super::world_check`]), so a release checks real worlds with the
//! server's own code.
use std::fmt;
use std::mem::Discriminant;

use super::registry::Formed;
use srp_validate::{
    KITS, MAX_PLAYERS, Part, Reason, Refusal,
    world::{self, Context, Held, Inventory, Item, Objects, Profile},
};
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::item_params;
use survarium::v026g::maps::MatchMap;
use survarium::v026g::match_msgs::static_info::{KIT_USABLE, MatchProfile};

/// The match a world is checked as: the server's own ([`Roster::of`]), or
/// the one a recording's 0xE4 gave its clients.
#[derive(Clone, Copy, Debug)]
pub struct Roster<'a> {
    pub map: MatchMap,
    /// Every place's profile known so far, by place.
    pub profiles: &'a [MatchProfile],
    /// The places of the first roster; the late joiners come after them.
    pub initial_players: usize,
}

impl<'a> Roster<'a> {
    pub(super) fn of(formed: &'a Formed) -> Self {
        Self {
            map: formed.played,
            profiles: &formed.profiles,
            initial_players: formed.initial_players,
        }
    }
}

/// A refused upload, with the match it was checked as: the clients take
/// the mode, batteries and respawn points from their own rule objects, so
/// a line naming them shows a server whose game data differs from theirs.
#[derive(Clone, Copy, Debug)]
pub struct Refused {
    refusal: Refusal,
    game_mode: u8,
    batteries: usize,
    respawn_points: usize,
    places: usize,
}

impl Refused {
    pub fn refusal(&self) -> &Refusal {
        &self.refusal
    }
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

/// Each place's kits as its game makes their objects
/// ([`MatchProfile::inventory`]), so that a player's state is read with them
/// to its exact end. An error names the first place whose kit holds an item
/// the game makes an object of that no one has traced: the match's states
/// are then read to their inventories only (the frame, the heads, the other
/// sections and the objects still exactly). That is a kit of the lobby's, not
/// the game data's (an item class the cook does not know stops the data at
/// start), so it is no reason to refuse every world of the match; the match
/// server says it once, at error level, when the place is formed
/// ([`unread_kits`]).
pub fn kits(profiles: &[MatchProfile]) -> Result<Vec<[Vec<Item<'static>>; KITS]>, String> {
    let dictionary = Dictionary::shared();
    profiles
        .iter()
        .take(MAX_PLAYERS)
        .enumerate()
        .map(|(place, profile)| place_kits(place, profile, dictionary))
        .collect()
}

/// Why the states of `profiles[from..]` would be read to their inventories
/// only ([`kits`]): what the match logs at error level as those places come.
pub(super) fn unread_kits(profiles: &[MatchProfile], from: usize) -> Option<String> {
    let dictionary = Dictionary::shared();
    profiles
        .iter()
        .take(MAX_PLAYERS)
        .enumerate()
        .skip(from)
        .find_map(|(place, profile)| place_kits(place, profile, dictionary).err())
}

fn place_kits(
    place: usize,
    profile: &MatchProfile,
    dictionary: &Dictionary,
) -> Result<[Vec<Item<'static>>; KITS], String> {
    let kit = |kit| {
        profile
            .inventory(kit, dictionary)
            .map_err(|why| format!("place {place} kit {kit}: {why}"))
    };
    Ok([kit(0)?, kit(1)?, kit(2)?])
}

pub fn check(body: &[u8], time: u32, places: usize, roster: Roster) -> Result<(), Refused> {
    let mut profiles = [Profile::default(); MAX_PLAYERS];
    for (target, source) in profiles.iter_mut().zip(roster.profiles) {
        target.items = source.items.len();
        target.usable_kits = source
            .kits
            .iter()
            .take(KITS)
            .enumerate()
            .filter(|(_, kit)| kit.state == KIT_USABLE)
            .fold(0, |mask, (kit, _)| mask | 1 << kit);
    }
    let known = roster.profiles.len().min(MAX_PLAYERS);
    let map = roster.map.map();
    let built = kits(roster.profiles).ok();
    let kits: Option<Vec<[&[Item]; KITS]>> = built.as_ref().map(|built| {
        built
            .iter()
            .map(|[a, b, c]| [&a[..], &b[..], &c[..]])
            .collect()
    });
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
    for bound in (roster.initial_players.min(known)..=known).rev() {
        let held: Vec<Held> = roster.profiles[..bound]
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
    Err(refused(first.expect(
        "the first roster is no more than the known places",
    )))
}

/// When a kind of refusal ([`Refusals`]) is more likely the check's fault
/// than the games': from this many places, or this many uploads.
const DOUBT_PLACES: u32 = 2;
const DOUBT_UPLOADS: u32 = 3;

/// A match's refused uploads, by kind: the part (a player block of any
/// place is one part) and the reason, not where or what value. One kind
/// from two places, or three times, is more likely a check or a server data
/// that differs from the games' than a broken or forged game: worlds the
/// players' games wrote are going to nobody, and late joiners and resyncs
/// get none. [`Refusals::count`] says so once a kind, for an error line.
#[derive(Debug, Default)]
pub(super) struct Refusals {
    kinds: Vec<Kind>,
}

#[derive(Debug)]
struct Kind {
    part: &'static str,
    reason: Discriminant<Reason>,
    /// Its first refusal, as logged.
    first: String,
    uploads: u32,
    /// Bit i: place i uploaded one.
    places: u32,
}

impl Kind {
    fn doubted(&self) -> bool {
        self.places.count_ones() >= DOUBT_PLACES || self.uploads >= DOUBT_UPLOADS
    }
}

impl Refusals {
    /// Counts `refused`, of `place`'s upload. When that makes its kind
    /// doubtful, the line to log at error level (once a kind).
    pub fn count(&mut self, place: usize, refused: &Refused) -> Option<String> {
        let part = refused.refusal.part.name().trim_end_matches('\0');
        let reason = std::mem::discriminant(&refused.refusal.reason);
        let at = match self
            .kinds
            .iter()
            .position(|k| k.part == part && k.reason == reason)
        {
            Some(at) => at,
            None => {
                self.kinds.push(Kind {
                    part,
                    reason,
                    first: refused.to_string(),
                    uploads: 0,
                    places: 0,
                });
                self.kinds.len() - 1
            }
        };
        let kind = &mut self.kinds[at];
        let before = kind.doubted();
        kind.uploads += 1;
        kind.places |= 1u32.checked_shl(place as u32).unwrap_or(0);
        (!before && kind.doubted()).then(|| {
            format!(
                "WORLD CHECK IN DOUBT: {} uploads from {} players refused as {}; the check or \
                 the server's game data is a likelier fault than the games: their worlds go to \
                 nobody (check the match's recording with srp-world-check)",
                kind.uploads,
                kind.places.count_ones(),
                kind.first
            )
        })
    }

    /// For the match's end line: the uploads refused, and of each kind in
    /// doubt its count and first refusal.
    pub fn summary(&self) -> String {
        let uploads: u32 = self.kinds.iter().map(|k| k.uploads).sum();
        if uploads == 0 {
            return "no upload refused".into();
        }
        let places = self.kinds.iter().fold(0, |m, k| m | k.places).count_ones();
        let doubted: Vec<String> = self
            .kinds
            .iter()
            .filter(|k| k.doubted())
            .map(|k| {
                format!(
                    "{} x {} ({} players)",
                    k.uploads,
                    k.first,
                    k.places.count_ones()
                )
            })
            .collect();
        if doubted.is_empty() {
            format!("{uploads} uploads refused from {places} players")
        } else {
            format!(
                "{uploads} uploads refused from {places} players, IN DOUBT: {}",
                doubted.join("; ")
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refused(part: Part, offset: usize, reason: Reason) -> Refused {
        Refused {
            refusal: Refusal::new(part, offset, reason),
            game_mode: 2,
            batteries: 10,
            respawn_points: 30,
            places: 20,
        }
    }

    /// One kind of refusal from two places, or three times from one, is
    /// said once; other kinds are counted apart, and the end line has them.
    #[test]
    fn a_kind_of_refusal_many_uploads_share_is_said_once() {
        let effects = |at| {
            refused(
                Part::Effects,
                0x67e9,
                Reason::EffectsBytes {
                    at,
                    what: "the closing part",
                },
            )
        };
        let pickup = |place| refused(Part::Player(place), 9, Reason::PickupRecord);
        let mut refusals = Refusals::default();
        assert_eq!(refusals.summary(), "no upload refused");
        // A forged or broken world of one game: counted, not in doubt.
        assert!(refusals.count(3, &effects(12)).is_none());
        assert!(refusals.count(3, &pickup(3)).is_none());
        assert_eq!(refusals.summary(), "2 uploads refused from 1 players");
        // The same kind (at another offset of it) from another place: in doubt.
        let doubt = refusals.count(5, &effects(13)).expect("in doubt");
        assert!(
            doubt.starts_with("WORLD CHECK IN DOUBT: 2 uploads from 2 players"),
            "{doubt}"
        );
        assert!(
            doubt.contains("effects at 0x67e9: the closing part at 12 is not every world's"),
            "{doubt}"
        );
        assert!(refusals.count(6, &effects(12)).is_none());
        // A player block of any place is one part: the third from place 3.
        assert!(refusals.count(3, &pickup(3)).is_none());
        assert!(refusals.count(3, &pickup(7)).is_some());
        let summary = refusals.summary();
        assert!(
            summary.starts_with("6 uploads refused from 3 players, IN DOUBT: 3 x effects"),
            "{summary}"
        );
        assert!(summary.contains("; 3 x player 3 at 0x9"), "{summary}");
    }
}
