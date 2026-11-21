//! Roster places, teams and sets of places.

/// Roster places a world keeps (the late join's roster: always 20).
pub const PLAYERS: usize = 20;

/// A team of the two.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Team {
    Zero = 0,
    One = 1,
}

impl Team {
    pub const BOTH: [Team; 2] = [Team::Zero, Team::One];

    /// A team value of the game's (a profile's): 0 or 1, anything else none
    /// (a free-for-all's side, a spectator).
    pub const fn from_raw(raw: u32) -> Option<Team> {
        match raw {
            0 => Some(Team::Zero),
            1 => Some(Team::One),
            _ => None,
        }
    }

    pub const fn index(self) -> usize {
        self as usize
    }

    pub const fn other(self) -> Team {
        match self {
            Team::Zero => Team::One,
            Team::One => Team::Zero,
        }
    }
}

/// A roster place, 0..20.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place(u8);

impl Place {
    pub const fn new(raw: u8) -> Option<Place> {
        if (raw as usize) < PLAYERS {
            Some(Place(raw))
        } else {
            None
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// A set of roster places (bit i: place i; no bit from 20 up).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Roster(u32);

impl Roster {
    pub const EMPTY: Roster = Roster(0);
    const ALL: u32 = (1 << PLAYERS) - 1;

    /// `None` for a bit that is no roster place.
    pub const fn from_bits(bits: u32) -> Option<Roster> {
        if bits & !Roster::ALL == 0 {
            Some(Roster(bits))
        } else {
            None
        }
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn has(self, place: Place) -> bool {
        self.0 >> place.0 & 1 != 0
    }

    pub const fn with(self, place: Place) -> Roster {
        Roster(self.0 | 1 << place.0)
    }

    pub const fn without(self, place: Place) -> Roster {
        Roster(self.0 & !(1 << place.0))
    }

    pub const fn minus(self, other: Roster) -> Roster {
        Roster(self.0 & !other.0)
    }

    pub const fn within(self, other: Roster) -> bool {
        self.0 & !other.0 == 0
    }

    pub const fn len(self) -> u8 {
        self.0.count_ones() as u8
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The places of the set, lowest first.
    pub fn places(self) -> impl Iterator<Item = Place> {
        (0..PLAYERS as u8)
            .map(Place)
            .filter(move |&place| self.has(place))
    }
}

/// Who is in the match (entered and not left), by team. A place is of one
/// team at most.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sides([Roster; 2]);

impl Sides {
    pub const EMPTY: Sides = Sides([Roster::EMPTY; 2]);

    /// `None` when a place is of both teams.
    pub const fn new(team_0: Roster, team_1: Roster) -> Option<Sides> {
        if team_0.0 & team_1.0 == 0 {
            Some(Sides([team_0, team_1]))
        } else {
            None
        }
    }

    pub const fn of(&self, team: Team) -> Roster {
        self.0[team.index()]
    }

    pub const fn all(&self) -> Roster {
        Roster(self.0[0].0 | self.0[1].0)
    }

    pub const fn team_of(&self, place: Place) -> Option<Team> {
        if self.0[0].has(place) {
            Some(Team::Zero)
        } else if self.0[1].has(place) {
            Some(Team::One)
        } else {
            None
        }
    }

    /// The sides with `place` of `team` (and of no other).
    pub const fn entered(&self, place: Place, team: Team) -> Sides {
        let mut sides = self.left(place);
        sides.0[team.index()] = sides.0[team.index()].with(place);
        sides
    }

    /// The sides without `place`.
    pub const fn left(&self, place: Place) -> Sides {
        Sides([self.0[0].without(place), self.0[1].without(place)])
    }
}
