//! Upload validation before a snapshot enters any server state or relay;
//! the same check of recorded worlds offline (`srp-world-check`,
//! [`super::world_check`]), so a release checks real worlds with the
//! server's own code.
use std::fmt;
use std::mem::Discriminant;
use std::num::NonZeroU32;

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

/// TEMPORARY (the fall-back, [`Refusals::judge`]): `body` checked as the
/// players' games check a world before they load it (patch 87,
/// `client-patches/rust/patches/src/world_check.rs`, `Match::context`):
/// the match's places, mode, batteries and respawn points, and no profiles,
/// artefacts, objects or inventory (as `srp-validate/examples/drift_dump.rs`
/// builds it). The games check every slot they have, the 20 places of a
/// growing roster's world (every live match's); a fixed roster's are fewer,
/// as the server's own check has them. Every game that receives a world
/// runs this check anyway, so a world relayed on it passing is one no
/// receiving game refuses.
pub(super) fn game_check(
    body: &[u8],
    time: u32,
    places: usize,
    map: MatchMap,
) -> Result<(), Refusal> {
    let map = map.map();
    let context = Context {
        time,
        places,
        game_mode: map.mode.rules().number(),
        batteries: map.victory_items_count as usize,
        respawn_points: Some(map.respawn_points),
        profiles: None,
        artefacts: None,
        objects: None,
        inventory: None,
    };
    world::validate(body, &context)
}

/// A match's refused uploads, by kind: the part (a player block of any
/// place is one part) and the reason, not where or what value. A kind is in
/// doubt, more likely the check's fault (or a server data that differs from
/// the games') than a broken or forged game, once uploads from two players
/// of the match were refused for it: every honest game writes the same
/// world, so a wrong check refuses every uploader's, while a cheater is one
/// place and cannot put a kind in doubt alone, however often it uploads.
/// [`Refusals::judge`] says so once a kind, for an error line; while it is,
/// its uploads fall back to the games' own check.
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
    /// The uploads the check refused for it, the fall-back's relays among
    /// them.
    uploads: u32,
    standing: Standing,
}

/// Where a kind of refusal stands in its match.
#[derive(Clone, Copy, Debug)]
enum Standing {
    /// Its uploads are all from one place: a broken or forged game is as
    /// likely as a wrong check. Refused.
    Refusing { place: usize },
    /// From two places or more (bit i: place i). TEMPORARY:
    /// each upload refused for it is checked again as the games check it
    /// ([`game_check`]); none has passed yet.
    InDoubt { places: u32 },
    /// In doubt, and `relayed` of its uploads passed the games' check and
    /// were relayed like any other upload.
    FallingBack { places: u32, relayed: NonZeroU32 },
}

/// What an upload the check refused gets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Answer {
    /// Sent to nobody, as every refused upload before the fall-back.
    Refuse,
    /// TEMPORARY: its kind is in doubt and it passes the games' own check:
    /// relayed like any other upload.
    Relay,
}

/// [`Refusals::judge`]'s verdict on a refused upload.
#[derive(Debug)]
pub(super) struct Judged {
    pub answer: Answer,
    /// The line to log at error level when this upload put its kind in
    /// doubt (once a kind).
    pub doubt: Option<String>,
}

fn bit(place: usize) -> u32 {
    u32::try_from(place)
        .ok()
        .and_then(|place| 1u32.checked_shl(place))
        .unwrap_or(0)
}

impl Standing {
    /// An upload of `place` the check refused for this kind: the next
    /// standing and what the upload gets. `passes` is the games' own check
    /// of it, run only in doubt.
    fn refused(self, place: usize, passes: impl FnOnce() -> bool) -> (Self, Answer) {
        let (places, relayed) = match self {
            Self::Refusing { place: only } if only == place => return (self, Answer::Refuse),
            Self::Refusing { place: only } => (bit(only) | bit(place), None),
            Self::InDoubt { places } => (places | bit(place), None),
            Self::FallingBack { places, relayed } => (places | bit(place), Some(relayed)),
        };
        // TEMPORARY (owner, 2026-11-07; packet-validation.md, "The fall-back
        // to the games' own check"): the fall-back. Removing it means
        // deleting the fall-back (this match becomes
        // `(Self::InDoubt { places }, Answer::Refuse)`; `FallingBack`,
        // `Answer::Relay`, `game_check` and the relay arm of
        // `game.rs::uploaded` go) and keeping the doubt detector
        // (`Refusing`, `InDoubt`, the `WORLD CHECK IN DOUBT` line).
        match (passes(), relayed) {
            (true, relayed) => (
                Self::FallingBack {
                    places,
                    relayed: relayed.map_or(NonZeroU32::MIN, |n| n.saturating_add(1)),
                },
                Answer::Relay,
            ),
            (false, Some(relayed)) => (Self::FallingBack { places, relayed }, Answer::Refuse),
            (false, None) => (Self::InDoubt { places }, Answer::Refuse),
        }
    }

    fn places(self) -> u32 {
        match self {
            Self::Refusing { place } => bit(place),
            Self::InDoubt { places } | Self::FallingBack { places, .. } => places,
        }
    }

    fn relayed(self) -> u32 {
        match self {
            Self::FallingBack { relayed, .. } => relayed.get(),
            Self::Refusing { .. } | Self::InDoubt { .. } => 0,
        }
    }
}

impl Refusals {
    /// `refused`, of `place`'s upload: counted with its kind, and judged.
    /// `game_check` is the games' own check of the upload ([`game_check`]),
    /// run only when its kind is in doubt.
    pub fn judge(
        &mut self,
        place: usize,
        refused: &Refused,
        game_check: impl FnOnce() -> Result<(), Refusal>,
    ) -> Judged {
        let part = refused.refusal.part.name().trim_end_matches('\0');
        let reason = std::mem::discriminant(&refused.refusal.reason);
        let Some(kind) = self
            .kinds
            .iter_mut()
            .find(|k| k.part == part && k.reason == reason)
        else {
            self.kinds.push(Kind {
                part,
                reason,
                first: refused.to_string(),
                uploads: 1,
                standing: Standing::Refusing { place },
            });
            return Judged {
                answer: Answer::Refuse,
                doubt: None,
            };
        };
        kind.uploads += 1;
        let before = kind.standing;
        let (standing, answer) = before.refused(place, || game_check().is_ok());
        kind.standing = standing;
        let doubt = matches!(
            (before, standing),
            (
                Standing::Refusing { .. },
                Standing::InDoubt { .. } | Standing::FallingBack { .. }
            )
        )
        .then(|| {
            format!(
                "WORLD CHECK IN DOUBT: {} uploads from {} players refused as {}; the check or \
                 the server's game data is a likelier fault than the games: until the match \
                 ends, an upload refused so is checked again as the players' games check it \
                 and relayed if it passes (WORLD CHECK FALLBACK, temporary; check the match's \
                 recording with srp-world-check)",
                kind.uploads,
                standing.places().count_ones(),
                kind.first
            )
        });
        Judged { answer, doubt }
    }

    /// The uploads the fall-back relayed in the match.
    pub fn relayed(&self) -> u32 {
        self.kinds.iter().map(|k| k.standing.relayed()).sum()
    }

    /// For the match's end line: the uploads refused, those the fall-back
    /// relayed, and of each kind in doubt its count, first refusal and
    /// relays.
    pub fn summary(&self) -> String {
        let uploads: u32 = self.kinds.iter().map(|k| k.uploads).sum();
        if uploads == 0 {
            return "no upload refused".into();
        }
        let places = self
            .kinds
            .iter()
            .fold(0, |m, k| m | k.standing.places())
            .count_ones();
        let relayed = match self.relayed() {
            0 => String::new(),
            relayed => format!(", {relayed} of them relayed by the fall-back"),
        };
        let doubted: Vec<String> = self
            .kinds
            .iter()
            .filter(|k| !matches!(k.standing, Standing::Refusing { .. }))
            .map(|k| {
                format!(
                    "{} x {} ({} players, {} relayed)",
                    k.uploads,
                    k.first,
                    k.standing.places().count_ones(),
                    k.standing.relayed()
                )
            })
            .collect();
        if doubted.is_empty() {
            format!("{uploads} uploads refused from {places} players{relayed}")
        } else {
            format!(
                "{uploads} uploads refused from {places} players{relayed}, IN DOUBT: {}",
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

    /// One kind of refusal from two places is said once and falls back to
    /// the games' check; one place's, however often, is refused and never
    /// checked again; other kinds are counted apart, and the end line has
    /// them.
    #[test]
    fn a_kind_two_places_share_falls_back_to_the_games_check() {
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
        let passes = || Ok(());
        let fails = || Err(Refusal::new(Part::Effects, 0x67e9, Reason::PickupRecord));
        let unrun = || -> Result<(), Refusal> { panic!("the games' check out of doubt") };
        let mut refusals = Refusals::default();
        assert_eq!(refusals.summary(), "no upload refused");
        // A forged or broken world of one game, however often: refused,
        // not in doubt, not checked again.
        for _ in 0..5 {
            let judged = refusals.judge(3, &effects(12), unrun);
            assert_eq!((judged.answer, judged.doubt), (Answer::Refuse, None));
        }
        assert_eq!(refusals.judge(3, &pickup(3), unrun).answer, Answer::Refuse);
        assert_eq!(refusals.summary(), "6 uploads refused from 1 players");
        // The same kind (at another offset of it) from another place: in
        // doubt, said once; this one fails the games' check too.
        let judged = refusals.judge(5, &effects(13), fails);
        assert_eq!(judged.answer, Answer::Refuse);
        let doubt = judged.doubt.expect("in doubt");
        assert!(
            doubt.starts_with("WORLD CHECK IN DOUBT: 6 uploads from 2 players refused as effects at 0x67e9: the closing part at 12 is not every world's"),
            "{doubt}"
        );
        // In doubt: what passes the games' check is relayed, what fails it
        // refused, by any place; the doubt is not said again.
        let judged = refusals.judge(3, &effects(12), passes);
        assert_eq!((judged.answer, judged.doubt), (Answer::Relay, None));
        assert_eq!(
            refusals.judge(6, &effects(12), fails).answer,
            Answer::Refuse
        );
        assert_eq!(
            refusals.judge(6, &effects(12), passes).answer,
            Answer::Relay
        );
        assert_eq!(refusals.relayed(), 2);
        // Another kind is not in doubt for it: refused unchecked.
        let physics = refused(Part::Physics, 0, Reason::PhysicsBytes { len: 1 });
        assert_eq!(refusals.judge(5, &physics, unrun).answer, Answer::Refuse);
        // A player block of any place is one part: from places 3 and 7.
        let judged = refusals.judge(7, &pickup(7), fails);
        assert_eq!(judged.answer, Answer::Refuse);
        assert!(judged.doubt.is_some());
        let summary = refusals.summary();
        assert!(
            summary.starts_with(
                "12 uploads refused from 4 players, 2 of them relayed by the fall-back, \
                 IN DOUBT: 9 x effects"
            ),
            "{summary}"
        );
        assert!(
            summary.contains("(3 players, 2 relayed); 2 x player 3 at 0x9"),
            "{summary}"
        );
        assert!(summary.ends_with("(2 players, 0 relayed)"), "{summary}");
    }
}
