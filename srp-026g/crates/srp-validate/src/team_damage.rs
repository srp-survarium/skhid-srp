//! Team damage turned back on its dealer past a threshold, as 0.28a had it
//! (exe patch 63, `docs/notes/team-damage.md`): the two formats it adds,
//! each with one owner here.
//!
//! - **The rule**, srp's options entry (tag [`TAG`] of the block after the
//!   squads, "SRPX"): three f32, `max_amount`, `max_ratio`, `return_ratio`
//!   (0.28a2's `teamdamage_params` in `gameplay/players/default.player`). The
//!   match server writes it ([`Rule::entry`]), the game reads it
//!   ([`Rule::of_entry`]), the server's check of the options takes it.
//! - **The counts**, a record of a world after its effects ([`Counts`]):
//!   what each place's hits took off other places' players, all of it and of
//!   it teammates'. The game writes it with every world and reads it with
//!   every world it loads, so a rollback, a late joiner's world and a
//!   replay's seek count as the games that played; the server's world check
//!   reads it as the game does.
//!
//! On the bits, as the rest of the crate: no float code in the game's build.
//! For numbers of no sign (finite, the sign bit clear) the bits order as the
//! numbers do, so every bound is an integer compare.

use crate::{MAX_PLAYERS, finite};

/// The tag of the rule's entry in srp's block after the options.
pub const TAG: u8 = 1;

/// The sign bit of an f32.
const SIGN: u32 = 0x8000_0000;

/// Whether `bits` are a number of no sign that is at most `most` (bits of
/// such a number).
const fn at_most(bits: u32, most: u32) -> bool {
    finite(bits) && bits & SIGN == 0 && bits <= most
}

/// The server's numbers: past both thresholds a dealer's hits on teammates
/// are turned on him.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule {
    max_amount: u32,
    max_ratio: u32,
    return_ratio: u32,
}

impl Rule {
    /// 0.28a2's, 0.28d0's and 0.31a2's `teamdamage_params`: `max_amount 3`,
    /// `max_ratio 0.67`, `return_ratio 2`.
    pub const ORIGINAL: Rule = Rule {
        max_amount: 0x4040_0000,
        max_ratio: 0x3F2B_851F,
        return_ratio: 0x4000_0000,
    };

    /// The entry's length: three f32.
    pub const ENTRY_LEN: usize = 12;

    /// The most a `max_amount` is (a hundred full health bars).
    const MOST_AMOUNT: u32 = 0x42C8_0000;
    /// The most a `max_ratio` is: 1.
    const MOST_RATIO: u32 = 0x3F80_0000;
    /// The most a `return_ratio` is: 16.
    const MOST_RETURN: u32 = 0x4180_0000;

    /// The rule of these f32 bits: `max_amount` 0 to 100 (health taken off
    /// teammates, 1 a full bar), `max_ratio` 0 to 1 (of all the dealer's),
    /// `return_ratio` 0 to 16 (the turned hit's damage, times the hit's).
    pub const fn of_bits(max_amount: u32, max_ratio: u32, return_ratio: u32) -> Option<Rule> {
        if at_most(max_amount, Self::MOST_AMOUNT)
            && at_most(max_ratio, Self::MOST_RATIO)
            && at_most(return_ratio, Self::MOST_RETURN)
        {
            Some(Rule { max_amount, max_ratio, return_ratio })
        } else {
            None
        }
    }

    /// The rule an entry's bytes are, if they are one.
    pub fn of_entry(bytes: &[u8]) -> Option<Rule> {
        let [a0, a1, a2, a3, r0, r1, r2, r3, t0, t1, t2, t3] = *bytes else {
            return None;
        };
        Rule::of_bits(
            u32::from_le_bytes([a0, a1, a2, a3]),
            u32::from_le_bytes([r0, r1, r2, r3]),
            u32::from_le_bytes([t0, t1, t2, t3]),
        )
    }

    /// The entry's bytes (after its tag and length).
    pub fn entry(&self) -> [u8; Self::ENTRY_LEN] {
        let [a, r, t] = [self.max_amount, self.max_ratio, self.return_ratio].map(u32::to_le_bytes);
        [a[0], a[1], a[2], a[3], r[0], r[1], r[2], r[3], t[0], t[1], t[2], t[3]]
    }

    pub fn max_amount(&self) -> f32 {
        f32::from_bits(self.max_amount)
    }

    pub fn max_ratio(&self) -> f32 {
        f32::from_bits(self.max_ratio)
    }

    pub fn return_ratio(&self) -> f32 {
        f32::from_bits(self.return_ratio)
    }
}

/// What one place's hits took off other places' players: all of it, and of
/// it its teammates'. Both numbers of no sign, the teammates' at most all.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Dealt {
    all: u32,
    team: u32,
}

impl Dealt {
    pub const NONE: Dealt = Dealt { all: 0, team: 0 };

    /// The counts of these f32 bits, if they are counts.
    pub const fn of_bits(all: u32, team: u32) -> Option<Dealt> {
        if at_most(all, 0x7F7F_FFFF) && at_most(team, all) {
            Some(Dealt { all, team })
        } else {
            None
        }
    }

    /// The counts after a hit that took `lost` (a number of no sign) off
    /// another place's player, a teammate's or not; none when a sum is no
    /// number (it never is: each is at most a few hundred).
    pub fn with(self, lost: f32, teammate: bool) -> Option<Dealt> {
        let all = f32::from_bits(self.all) + lost;
        let team = if teammate { f32::from_bits(self.team) + lost } else { f32::from_bits(self.team) };
        Dealt::of_bits(all.to_bits(), team.to_bits())
    }

    pub fn all(&self) -> f32 {
        f32::from_bits(self.all)
    }

    pub fn team(&self) -> f32 {
        f32::from_bits(self.team)
    }

    /// Nothing dealt (0 is the only number whose bits are 0).
    pub const fn is_none(&self) -> bool {
        self.all == 0
    }
}

/// Why bytes after a world's effects are not a counts record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CountsFault {
    /// Not the record's magic.
    Magic,
    /// Fewer bytes than its places' counts, or more after them.
    Length { len: usize },
    /// A count of places of none, or past the world's.
    Places { places: usize, most: usize },
    /// A place's counts are no counts (a number with a sign, or no number,
    /// or more of teammates' than of all).
    Counts { place: usize },
    /// The last place's counts are none: the writer stops at the last place
    /// that dealt any.
    Trailing,
}

impl core::fmt::Display for CountsFault {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CountsFault::Magic => write!(f, "the team damage record's magic is another"),
            CountsFault::Length { len } => write!(f, "a team damage record of {len} bytes"),
            CountsFault::Places { places, most } => {
                write!(f, "a team damage record of {places} places (1 to {most})")
            }
            CountsFault::Counts { place } => write!(f, "place {place}'s team damage counts are none"),
            CountsFault::Trailing => write!(f, "the team damage record ends with a place that dealt none"),
        }
    }
}

/// Each place's [`Dealt`] in a match, as a world carries them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Counts([Dealt; MAX_PLAYERS]);

impl Default for Counts {
    fn default() -> Self {
        Counts::NONE
    }
}

impl Counts {
    pub const NONE: Counts = Counts([Dealt::NONE; MAX_PLAYERS]);

    /// The record's magic.
    pub const MAGIC: [u8; 4] = *b"SRTD";
    /// The longest record: the magic, the places, two f32 a place.
    pub const MAX_RECORD: usize = 4 + 1 + 8 * MAX_PLAYERS;

    /// Place `place`'s counts (none past the 20).
    pub fn get(&self, place: usize) -> Dealt {
        self.0.get(place).copied().unwrap_or(Dealt::NONE)
    }

    /// The counts with place `place`'s `dealt` (the same past the 20).
    pub fn with(mut self, place: usize, dealt: Dealt) -> Counts {
        if let Some(at) = self.0.get_mut(place) {
            *at = dealt;
        }
        self
    }

    /// The record of these counts into `out`, and its length: the places up
    /// to the last that dealt any; none (0) when no place did.
    pub fn record(&self, out: &mut [u8; Self::MAX_RECORD]) -> usize {
        let places = self.0.iter().rposition(|dealt| !dealt.is_none()).map_or(0, |last| last.saturating_add(1));
        if places == 0 {
            return 0;
        }
        let mut len = 0;
        let mut put = |bytes: &[u8]| {
            for &byte in bytes {
                if let Some(at) = out.get_mut(len) {
                    *at = byte;
                    len = len.saturating_add(1);
                }
            }
        };
        put(&Self::MAGIC);
        put(&[places as u8]);
        for dealt in self.0.iter().take(places) {
            put(&dealt.all.to_le_bytes());
            put(&dealt.team.to_le_bytes());
        }
        len
    }

    /// The record that is all of `bytes`, of a world of `places` places.
    pub fn read(bytes: &[u8], places: usize) -> Result<Counts, CountsFault> {
        let (head, rest) = bytes.split_at_checked(5).ok_or(CountsFault::Length { len: bytes.len() })?;
        let [m0, m1, m2, m3, count] = *head else {
            return Err(CountsFault::Length { len: bytes.len() });
        };
        if [m0, m1, m2, m3] != Self::MAGIC {
            return Err(CountsFault::Magic);
        }
        let count = usize::from(count);
        let most = places.min(MAX_PLAYERS);
        if count == 0 || count > most {
            return Err(CountsFault::Places { places: count, most });
        }
        let (words, tail) = rest.as_chunks::<4>();
        if !tail.is_empty() || words.len() != count.saturating_mul(2) {
            return Err(CountsFault::Length { len: bytes.len() });
        }
        let mut counts = Counts::NONE;
        for (place, pair) in words.chunks_exact(2).enumerate() {
            let [all, team] = [0, 1].map(|i| pair.get(i).map_or(u32::MAX, |word| u32::from_le_bytes(*word)));
            let dealt = Dealt::of_bits(all, team).ok_or(CountsFault::Counts { place })?;
            counts = counts.with(place, dealt);
        }
        if counts.get(count.saturating_sub(1)).is_none() {
            return Err(CountsFault::Trailing);
        }
        Ok(counts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_original_rule() {
        let rule = Rule::ORIGINAL;
        assert_eq!((rule.max_amount(), rule.max_ratio(), rule.return_ratio()), (3.0, 0.67, 2.0));
        assert_eq!(Rule::of_entry(&rule.entry()), Some(rule));
        assert_eq!(rule.entry(), *b"\0\0\x40\x40\x1f\x85\x2b\x3f\0\0\0\x40");
    }

    #[test]
    fn rule_domains() {
        let of = |a: f32, r: f32, t: f32| Rule::of_bits(a.to_bits(), r.to_bits(), t.to_bits());
        assert!(of(0.0, 0.0, 0.0).is_some());
        assert!(of(100.0, 1.0, 16.0).is_some());
        assert!(of(-1.0, 0.5, 2.0).is_none());
        assert!(of(-0.0, 0.5, 2.0).is_none());
        assert!(of(3.0, 1.01, 2.0).is_none());
        assert!(of(3.0, 0.5, 16.5).is_none());
        assert!(of(f32::NAN, 0.5, 2.0).is_none());
        assert!(of(f32::INFINITY, 0.5, 2.0).is_none());
        assert!(of(100.5, 0.5, 2.0).is_none());
        assert!(Rule::of_entry(&[0; 11]).is_none());
        assert!(Rule::of_entry(&[0; 13]).is_none());
    }

    #[test]
    fn dealt_adds_up() {
        let dealt = Dealt::NONE.with(0.5, false).unwrap().with(0.25, true).unwrap();
        assert_eq!((dealt.all(), dealt.team()), (0.75, 0.25));
        assert!(Dealt::of_bits(1.0f32.to_bits(), 2.0f32.to_bits()).is_none());
        assert!(Dealt::of_bits((-1.0f32).to_bits(), 0).is_none());
        assert!(Dealt::NONE.with(f32::INFINITY, false).is_none());
    }

    fn record(counts: &Counts) -> std::vec::Vec<u8> {
        let mut out = [0; Counts::MAX_RECORD];
        let len = counts.record(&mut out);
        out[..len].to_vec()
    }

    #[test]
    fn records() {
        assert!(record(&Counts::NONE).is_empty());
        let counts = Counts::NONE.with(2, Dealt::NONE.with(4.0, true).unwrap());
        let bytes = record(&counts);
        assert_eq!(bytes.len(), 5 + 3 * 8);
        assert_eq!(&bytes[..5], b"SRTD\x03");
        assert_eq!(Counts::read(&bytes, 4), Ok(counts));
        assert_eq!(Counts::read(&bytes, 2), Err(CountsFault::Places { places: 3, most: 2 }));
        let mut full = Counts::NONE;
        for place in 0..MAX_PLAYERS {
            full = full.with(place, Dealt::NONE.with(1.0, place % 2 == 0).unwrap());
        }
        let bytes = record(&full);
        assert_eq!(bytes.len(), Counts::MAX_RECORD);
        assert_eq!(Counts::read(&bytes, MAX_PLAYERS), Ok(full));
    }

    #[test]
    fn refusals() {
        let counts = Counts::NONE.with(0, Dealt::NONE.with(1.0, true).unwrap());
        let bytes = record(&counts);
        let read = |bytes: &[u8]| Counts::read(bytes, 20);
        assert_eq!(read(b""), Err(CountsFault::Length { len: 0 }));
        assert_eq!(read(b"SRTX\x01\0\0\x80\x3f\0\0\x80\x3f"), Err(CountsFault::Magic));
        assert_eq!(read(b"SRTD\x00"), Err(CountsFault::Places { places: 0, most: 20 }));
        assert_eq!(read(&bytes[..bytes.len() - 1]), Err(CountsFault::Length { len: bytes.len() - 1 }));
        assert_eq!(read(&[&bytes[..], b"x"].concat()), Err(CountsFault::Length { len: bytes.len() + 1 }));
        // more of teammates' than of all
        assert_eq!(read(b"SRTD\x01\0\0\x80\x3f\0\0\0\x40"), Err(CountsFault::Counts { place: 0 }));
        // a last place that dealt none
        assert_eq!(read(b"SRTD\x02\0\0\x80\x3f\0\0\0\0\0\0\0\0\0\0\0\0"), Err(CountsFault::Trailing));
    }
}
