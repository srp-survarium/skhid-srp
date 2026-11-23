//! Whose part of the way to the base a delivery was.
//!
//! 0.46c2's `victory_item_event_manager` (0x7B2700): where the artefact is
//! between the two bases is one number, d0 / (d0 + d1) (the distances to
//! team 0's and team 1's base). Each team has the best the artefact came to
//! its base since the other team last touched it; a player who takes, drops
//! or puts it is counted the part by which he bettered his team's best. At a
//! delivery the team's players share by their parts (0x7B29F0). Here the same
//! on integers: distances in mm, the number in 65536ths.

use crate::geometry::Position;
use crate::params::Params;
use crate::roster::{PLAYERS, Place, Sides, Team};

/// The whole way between the bases.
pub const WHOLE: u32 = 1 << 16;
/// The most way one place is counted: 128 times between the bases.
pub const MAX_WAY: u32 = 128 * WHOLE;

/// An artefact's journey since it appeared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Way {
    /// By team: the least way left to its base (0..=WHOLE).
    pub(crate) left: [u32; 2],
    /// By roster place: the way it carried the artefact to its own base
    /// (0..=MAX_WAY).
    pub(crate) carried: [u32; PLAYERS],
}

/// How much of the way between the bases is left from `at` to each team's
/// base: `[d0, d1] x WHOLE / (d0 + d1)`, the two adding up to WHOLE.
fn left_from(params: &Params, at: Position) -> [u32; 2] {
    let d0 = at.distance(params.base(Team::Zero));
    let d1 = at.distance(params.base(Team::One));
    // The bases are a metre apart at least, so d0 + d1 is not 0 (each is
    // rounded down by less than a millimetre), and it is below 2^31.
    let sum = d0 + d1;
    if sum == 0 || d1 == 0 {
        return [WHOLE, 0];
    }
    // d0 x WHOLE / sum without a 64-bit division: one bit a turn, the
    // remainder below sum.
    let mut rest = d0;
    let mut share = 0u32;
    for _ in 0..16 {
        rest <<= 1;
        share <<= 1;
        if rest >= sum {
            rest -= sum;
            share |= 1;
        }
    }
    [share, WHOLE - share]
}

impl Way {
    /// The artefact appeared at `at` (0.46c2's event 0): nobody carried it.
    pub fn begin(params: &Params, at: Position) -> Way {
        Way {
            left: left_from(params, at),
            carried: [0; PLAYERS],
        }
    }

    /// A player of `team` took, dropped or put the artefact at `at`: his
    /// team's best and his part of it, and the other team's way starts
    /// again from here.
    pub(crate) fn touched(
        mut self,
        params: &Params,
        place: Place,
        team: Team,
        at: Position,
    ) -> Way {
        let now = left_from(params, at);
        let other = team.other();
        self.left[other.index()] = now[other.index()];
        let best = self.left[team.index()].min(now[team.index()]);
        let gained = self.left[team.index()] - best;
        self.left[team.index()] = best;
        let carried = &mut self.carried[place.index()];
        *carried = carried.saturating_add(gained).min(MAX_WAY);
        self
    }

    /// The way without `place`'s part (it left the match).
    pub(crate) fn forgetting(mut self, place: Place) -> Way {
        self.carried[place.index()] = 0;
        self
    }

    /// Each place of `team` with a part of the way, and its share of the
    /// whole in hundredths (rounded as 0.46c2: + 0.5, down); nothing below
    /// half a hundredth.
    pub fn shares(&self, sides: &Sides, team: Team) -> impl Iterator<Item = (Place, u16)> + '_ {
        let members = sides.of(team);
        let total: u32 = members
            .places()
            .map(|place| self.carried[place.index()])
            .sum();
        members.places().filter_map(move |place| {
            let part = self.carried[place.index()];
            // part <= MAX_WAY = 2^23 and total <= 20 x 2^23: both products
            // fit 32 bits.
            let hundredths = (part * 200).checked_add(total)?.checked_div(2 * total)?;
            (hundredths != 0).then_some((place, hundredths as u16))
        })
    }

    /// By team: the least way left to its base, of [`WHOLE`].
    pub fn left(&self) -> [u32; 2] {
        self.left
    }

    /// By roster place: the way it carried the artefact.
    pub fn carried(&self) -> &[u32; PLAYERS] {
        &self.carried
    }

    /// The wire's form; `None` out of the domains.
    pub(crate) fn from_parts(left: [u32; 2], carried: [u32; PLAYERS]) -> Option<Way> {
        let fits = left.iter().all(|&l| l <= WHOLE) && carried.iter().all(|&c| c <= MAX_WAY);
        fits.then_some(Way { left, carried })
    }
}
