//! Keeper of the Artifacts' kinds (docs/notes/artefact-skills.md "Keeper of the Artifacts"):
//! the one table srp draws a match's kind from and the game takes it by.
//!
//! - srp draws one [`Kind`] a match, uniformly over [`KINDS`], and writes its [`Kind::draw`]
//!   into modifier 40 bits 9..11 of every kit of the match (exe patch 109 parses it).
//! - Every map has the hidden Keeper anomaly (content edit list `artefact-keeper-1.json`): no
//!   container, a pool of [`POOL`] dormant artefacts of each kind, by [`Kind::dict_id`].
//! - A Keeper takes its place's one of the match's kind from that pool into its first empty
//!   container once a match, from the match's start (exe patch 109).

/// A kind Keeper gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Larkspur,
    Onyx,
    Spring,
}

/// Every kind, in draw order (draw 1 is the first).
pub const KINDS: [Kind; 3] = [Kind::Larkspur, Kind::Onyx, Kind::Spring];

/// Dormant artefacts of each kind in the hidden pool: one a place (20 places, a gift only in a
/// place's first life; one that comes back is dormant again).
pub const POOL: u8 = 20;

impl Kind {
    /// The kind of a draw (modifier 40 bits 9..11); none for 0 or a draw past [`KINDS`].
    pub fn of_draw(draw: u8) -> Option<Kind> {
        KINDS.get(usize::from(draw).checked_sub(1)?).copied()
    }

    /// Its draw, 1..=3.
    pub fn draw(self) -> u8 {
        match self {
            Kind::Larkspur => 1,
            Kind::Onyx => 2,
            Kind::Spring => 3,
        }
    }

    /// Its item in `gameplay/db_static_dictionaries`'s items_dict.
    pub fn dict_id(self) -> u16 {
        match self {
            Kind::Larkspur => 54,
            Kind::Onyx => 76,
            Kind::Spring => 74,
        }
    }

    /// Its item's dictionary name (srp checks the dictionary has `dict_id` by it).
    pub fn item_name(self) -> &'static str {
        match self {
            Kind::Larkspur => "artefact_lifebone",
            Kind::Onyx => "artefact_onyx",
            Kind::Spring => "artefact_spring",
        }
    }

    /// For the logs.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Larkspur => "Larkspur",
            Kind::Onyx => "Onyx",
            Kind::Spring => "Spring",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_name_each_kind_once() {
        for (n, kind) in KINDS.into_iter().enumerate() {
            assert_eq!(usize::from(kind.draw()), n + 1);
            assert_eq!(Kind::of_draw(kind.draw()), Some(kind));
        }
        assert_eq!(Kind::of_draw(0), None);
        assert_eq!(Kind::of_draw(4), None);
        assert_eq!(Kind::of_draw(6), None);
        let mut ids = KINDS.map(Kind::dict_id);
        ids.sort();
        assert!(ids.windows(2).all(|w| w[0] != w[1]));
    }
}
