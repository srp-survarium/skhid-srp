//! Upload validation before a snapshot enters any server state or relay;
//! the same check of recorded worlds offline (`srp-world-check`,
//! [`super::world_check`]), so a release checks real worlds with the
//! server's own code.
use std::fmt;

use super::registry::Formed;
use srp_validate::{
    KITS, MAX_PLAYERS, Part, Refusal,
    world::{
        self, Affect, CarriedBattery, Context, Held, Inventory, Item, ItemBody, ItemsAndRest, JumpType, Objects, PlayerState,
        Profile, Stamina, WeaponState,
    },
};
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::item_params;
use survarium::v026g::maps::MatchMap;
use survarium::v026g::match_msgs::motion::Motion;
use survarium::v026g::match_msgs::players::Supplies;
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

/// A world that passed [`check_kept`]: each place's state as the check read it, owned, so that it outlives the upload's
/// bytes and reaches the bots with the world once it is trusted (one parse, at the boundary: the world format's one
/// definition, `srp_validate`). By place; `None` for a place with no block in the world.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Checked {
    pub places: Vec<Option<PlaceWorld>>,
}

/// One place's state in a checked world: what the bots read ([`Self::motion`], [`Self::supplies`], [`Self::stamina`]),
/// and the further fields of the state, kept for their readers to come.
#[derive(Clone, Debug, PartialEq)]
pub struct PlaceWorld {
    /// The body, the controller block, the damage model and the recent hits ([`Motion::of_world`]); `None` when the
    /// match's kits are not known (`kits`), as the items, and so the rest of the state, are not read then.
    pub motion: Option<Motion>,
    /// The kit in use, what is left of each item, the weapon in hands ([`Supplies::of_world`]).
    pub supplies: Supplies,
    /// The stamina record (`player_stamina::serialize` 0x329310).
    pub stamina: Stamina,
    /// `m_current_active_object` and `m_target_active_object`: the held object's and the pending one's ids.
    pub active_objects: (Option<u8>, Option<u8>),
    /// The battery the player carries (its user is the place: `player_tail_kept`).
    pub carried: Option<CarriedBattery>,
    /// The artefact slots' first slot and each slot's artefact id.
    pub artefact_slots: Option<(u8, Vec<Option<u8>>)>,
    /// Each weapon item's state by its slot: with a user (the gun in hands) its state machine's state, aim, selector
    /// (the jump state) and portable object (`WeaponState::in_hand`).
    pub guns: Vec<(u8, WeaponState)>,
    /// `m_jump_type`; `None` when `motion` is.
    pub jump_type: Option<JumpType>,
    /// Each body part's affects (a hand wound is `AffectKind::HandDamage`), in part order; empty when `motion` is
    /// `None`.
    pub part_affects: Vec<Vec<Affect>>,
    /// Each body part's `m_last_hit_time` (part +B0h: the last hit that took health, 0x32A29E), in part order; empty when
    /// `motion` is `None`.
    pub part_last_hits: Vec<u32>,
    /// The damage model's `m_invulnerability_time_in_ms` (the spawn protection); `None` when `motion` is, or when none
    /// is set.
    pub invulnerability_time_ms: Option<u32>,
}

impl PlaceWorld {
    fn of(
        place: u8,
        state: &PlayerState<'_>,
        items: Option<&ItemsAndRest<'_, '_>>,
        profile: &MatchProfile,
        dictionary: &Dictionary,
    ) -> Self {
        let rest = items.map(ItemsAndRest::rest);
        Self {
            motion: rest.map(|rest| Motion::of_world(state, rest)),
            supplies: Supplies::of_world(place, state, items, profile, dictionary),
            stamina: state.stamina(),
            active_objects: (state.current_active_object(), state.target_active_object()),
            carried: state.inventory().carried(),
            artefact_slots: state
                .inventory()
                .artefact_slots()
                .map(|slots| (slots.begin().get(), slots.ids().collect())),
            guns: items.map_or_else(Vec::new, |items| {
                items
                    .by_slot()
                    .filter_map(|(slot, item)| match item.body() {
                        ItemBody::Weapon(gun) => Some((slot, gun)),
                        _ => None,
                    })
                    .collect()
            }),
            jump_type: rest.map(|rest| rest.jump_type()),
            part_affects: rest.map_or_else(Vec::new, |rest| {
                rest.damage_model()
                    .body_parts()
                    .map(|part| part.affects().iter().collect())
                    .collect()
            }),
            part_last_hits: rest.map_or_else(Vec::new, |rest| {
                rest.damage_model().body_parts().map(|part| part.last_hit_time_ms()).collect()
            }),
            invulnerability_time_ms: rest.and_then(|rest| rest.damage_model().invulnerability_time_ms()),
        }
    }
}

/// [`check_kept`] for a reader that needs only the verdict (the world check over recordings, the bench).
pub fn check(body: &[u8], time: u32, places: usize, roster: Roster) -> Result<(), Refused> {
    check_with(body, time, places, roster, Keep::Nothing).map(drop)
}

/// The world check of an upload ([`check`]), keeping each place's state as it read it ([`Checked`]): the match's.
pub fn check_kept(body: &[u8], time: u32, places: usize, roster: Roster) -> Result<Checked, Refused> {
    check_with(body, time, places, roster, Keep::Places)
}

/// What a check keeps of the world it read.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Keep {
    Nothing,
    Places,
}

fn check_with(body: &[u8], time: u32, places: usize, roster: Roster, keep: Keep) -> Result<Checked, Refused> {
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
        let mut kept: Vec<Option<PlaceWorld>> = Vec::new();
        let read = match keep {
            Keep::Nothing => world::validate(body, &context),
            Keep::Places => {
                kept = vec![None; known];
                let dictionary = Dictionary::shared();
                world::validate_kept(body, &context, &mut |place, state, items| {
                    if let (Some(slot), Some(profile)) =
                        (kept.get_mut(usize::from(place)), roster.profiles.get(usize::from(place)))
                    {
                        *slot = Some(PlaceWorld::of(place, state, items, profile, dictionary));
                    }
                })
            }
        };
        match read {
            Ok(()) => return Ok(Checked { places: kept }),
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
