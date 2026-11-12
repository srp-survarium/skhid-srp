//! A bot's own health pools, as the HUD shows a player: the body, the head,
//! the legs, the hands, the pain and the infection of the world's
//! `Damage::part_health` (the body table's order, `fact-health-recovery.md`
//! §1.1: body 0, head 3, legs 6 and 7, hands 16 and 17, pain 18, infection
//! 19). The plays used the body alone (`PlaceView::health`, part 0); the
//! consumables (`plays/heal.rs`) treat the others too.
//!
//! Kept by the bot ([`super::Bot`]) from each trusted world ([`Pools::of_world`],
//! `game/driven.rs`), like its hurts ([`super::motor::Hurt`]): it is its own
//! state and nobody else's.

/// The pools' most (`human_hit_params.options`, PROVEN).
pub const BODY_MAX: f32 = 1.0;
pub const HEAD_MAX: f32 = 0.4;
pub const LIMB_MAX: f32 = 0.5;
pub const PAIN_MAX: f32 = 1.0;
pub const INFECTION_MAX: f32 = 1.0;

/// The world's `part_health` indexes.
const BODY: usize = 0;
const HEAD: usize = 3;
const LEGS: [usize; 2] = [6, 7];
const HANDS: [usize; 2] = [16, 17];
const PAIN: usize = 18;
const INFECTION: usize = 19;
/// The pools the body table has.
const PARTS: usize = 20;

/// A limb pool at or under this is broken (the worlds show 0, and 0.01-0.03
/// just after it mends, `motor::Hurt`).
pub const LIMB_BROKEN: f32 = 0.005;

/// What a consumable treats; a limb group is both limbs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pool {
    Body,
    Head,
    Legs,
    Hands,
    Pain,
    Infection,
}

impl Pool {
    pub fn max(self) -> f32 {
        match self {
            Self::Body => BODY_MAX,
            Self::Head => HEAD_MAX,
            Self::Legs | Self::Hands => LIMB_MAX,
            Self::Pain => PAIN_MAX,
            Self::Infection => INFECTION_MAX,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pools {
    pub body: f32,
    pub head: f32,
    pub legs: [f32; 2],
    pub hands: [f32; 2],
    pub pain: f32,
    pub infection: f32,
}

impl Pools {
    pub const FULL: Self = Self {
        body: BODY_MAX,
        head: HEAD_MAX,
        legs: [LIMB_MAX; 2],
        hands: [LIMB_MAX; 2],
        pain: PAIN_MAX,
        infection: INFECTION_MAX,
    };

    /// The body pool alone known (a bot whose world gave no more): the
    /// rest whole.
    pub fn of_body(body: f32) -> Self {
        Self { body, ..Self::FULL }
    }

    /// From a world's `part_health`; none if it has not the pools.
    pub fn of_world(part_health: &[f32]) -> Option<Self> {
        if part_health.len() < PARTS {
            return None;
        }
        Some(Self {
            body: part_health[BODY],
            head: part_health[HEAD],
            legs: LEGS.map(|i| part_health[i]),
            hands: HANDS.map(|i| part_health[i]),
            pain: part_health[PAIN],
            infection: part_health[INFECTION],
        })
    }

    /// The missing share of a pool, `[0, 1]`: of a pair of limbs, the worse
    /// one's.
    pub fn missing(&self, pool: Pool) -> f32 {
        let value = match pool {
            Pool::Body => self.body,
            Pool::Head => self.head,
            Pool::Legs => self.legs[0].min(self.legs[1]),
            Pool::Hands => self.hands[0].min(self.hands[1]),
            Pool::Pain => self.pain,
            Pool::Infection => self.infection,
        };
        (1.0 - value / pool.max()).clamp(0.0, 1.0)
    }

    /// A limb of the pair is broken (its pool at 0).
    pub fn limb_broken(&self) -> bool {
        self.legs
            .iter()
            .chain(&self.hands)
            .any(|&pool| pool <= LIMB_BROKEN)
    }

    /// `amount` added to a pool (each limb of a pair), up to its most.
    pub fn add(&mut self, pool: Pool, amount: f32) {
        let up = |value: &mut f32| *value = (*value + amount).clamp(0.0, pool.max());
        match pool {
            Pool::Body => up(&mut self.body),
            Pool::Head => up(&mut self.head),
            Pool::Legs => self.legs.iter_mut().for_each(up),
            Pool::Hands => self.hands.iter_mut().for_each(up),
            Pool::Pain => up(&mut self.pain),
            Pool::Infection => up(&mut self.infection),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_world_gives_the_pools_by_the_body_tables_order() {
        let mut parts = vec![1.0; PARTS];
        parts[BODY] = 0.6;
        parts[HEAD] = 0.1;
        parts[6] = 0.0;
        parts[7] = 0.25;
        parts[17] = 0.4;
        parts[PAIN] = 0.5;
        parts[INFECTION] = 0.9;
        let pools = Pools::of_world(&parts).unwrap();
        assert_eq!(pools.body, 0.6);
        assert!((pools.missing(Pool::Body) - 0.4).abs() < 1e-6);
        assert!((pools.missing(Pool::Head) - 0.75).abs() < 1e-6);
        assert_eq!(pools.missing(Pool::Legs), 1.0, "the worse leg");
        assert!((pools.missing(Pool::Hands) - 0.2).abs() < 1e-6);
        assert!(pools.limb_broken());
        assert!((pools.missing(Pool::Pain) - 0.5).abs() < 1e-6);
        assert!((pools.missing(Pool::Infection) - 0.1).abs() < 1e-6);
        assert_eq!(Pools::of_world(&parts[..19]), None);
        assert!(!Pools::FULL.limb_broken());
        assert_eq!(Pools::FULL.missing(Pool::Body), 0.0);
    }

    #[test]
    fn adding_stops_at_the_pools_most() {
        let mut pools = Pools::of_body(0.4);
        pools.legs = [0.1, 0.5];
        pools.add(Pool::Body, 0.9);
        pools.add(Pool::Legs, 0.3);
        pools.add(Pool::Head, 0.2);
        assert_eq!(pools.body, 1.0);
        assert_eq!(pools.legs, [0.4, 0.5]);
        assert_eq!(pools.head, HEAD_MAX);
    }
}
