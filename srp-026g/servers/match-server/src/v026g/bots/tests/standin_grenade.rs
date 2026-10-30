//! A grenade going off among the stand-in's bodies, as the 0.26g client
//! works the blast out (`survarium::v026g::damage`, module doc, step 5): the
//! RGD-5's `splash_*` numbers from the game data, in place of "the stand-in
//! has no grenade damage". Where and when it goes off is the owner tally's
//! ([`super::tallies::owner`]: the throw the bot notes, `arc`'s landing, the
//! fuse); this is what happens to the bodies then.
//!
//! - Who: every living body whose feet are within `splash_radius` of the
//!   blast, the thrower, his mates and enemies alike (the client's blast
//!   function tests no team: PROVEN, 0x339630).
//! - How much: linear in the distance, `splash_max_damage` at the blast to
//!   `splash_min_damage` at the radius (PROVEN).
//! - Which parts: the `splash_affected_bodies` of the grenade, one hit each
//!   with armour piercing 1.0, so armour does not hold (PROVEN), every one
//!   cut by the walls between the part and the blast. **GUESS:** a part is
//!   reached only along a clear line on the bullet mesh (the mesh keeps no
//!   material per face; the blast's power `splash_pierce` is 0.15, which a
//!   wall of any resistance above 0.15 stops outright, and a material of
//!   resistance 0 passes it whole: the stand-in has none of those), and a
//!   part's place is a fixed height over the feet ([`part_point`]).
//! - What it takes: the body, pain and leg pools by [`damage::BodyParams::hits_taken`],
//!   the largest hit to each pool; the head pool is not among the
//!   grenade's parts. A leg pool at 0 breaks the leg as a bullet's does.
//!
//! Not modelled: the stagger and stamina damage of the blast
//! (`stopping_power`, `stamina_damage`: the call at 0x2DA5B0), the blast's
//! sound, and a second grenade of one thrower inside 8 s counting only its
//! increase over the first on a pool (the same initiator, LIKELY).

use mapdata::Vec3;
use survarium::v026g::damage::{self, SplashParams};
use survarium::v026g::loadouts::BOT_GRENADE;

use super::standin_damage::Pool;
use super::{Body, Game, Kill, Place, STAND_IN_LEG_BROKEN_MS};

/// The RGD-5's `stopping_power` (the grenade block's, PROVEN, sprint-jump RE `FINDINGS.md` §1.1: 0.1).
const GRENADE_STOPPING_POWER: f32 = 0.1;

/// What one kit holds of fragmentation grenades: the units a spawn takes (`count`, the pockets' units) and the profile's store
/// (`stored`, the item's `amount_in_storage`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct KitGrenades {
    pub(crate) count: u32,
    pub(crate) stored: u32,
}

/// A body's frag grenades, as the client keeps them (PROVEN, `loadouts.rs`: `inventory::setup_from_profile` 0x2FCB70 at every
/// spawn: a pocket gets `min(its units, the item's storage)` and the storage loses that; `unload_to_profile` 0x2FD2D0 puts what
/// is left in the hand back into the storage). A bot's store is its count today (once per kit and match: a grenade thrown is
/// gone), and the profile's `stored` is what A144's store changes, so this follows the profile, not a per-life refill. The
/// stand-in explodes a throw only when the body has a grenade in hand ([`Self::throw`]); the tally only counts.
///
/// Moves to `KitPockets` (`live_grenades.rs`) when that reads the profile's store for the live pockets too.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Stock {
    /// Not modelled (a made-up match, or a place with no profile): every throw is carried.
    Unlimited,
    /// The three kits' grenades, the kit in hand and the units in the hand.
    Kits { kits: [KitGrenades; 3], kit: usize, next: usize, hand: u32 },
}

impl Stock {
    pub(crate) fn of(kits: [KitGrenades; 3]) -> Self {
        Self::Kits { kits, kit: 0, next: 0, hand: 0 }
    }

    /// The kit the next spawn uses (the bot's choice for its next life).
    pub(crate) fn choose(&mut self, kit: usize) {
        if let Self::Kits { next, .. } = self {
            *next = kit.min(2);
        }
    }

    /// A spawn: the last life's leftovers back in the store, the new kit's pockets filled from it.
    pub(crate) fn spawn(&mut self) {
        if let Self::Kits { kits, kit, next, hand } = self {
            kits[*kit].stored += std::mem::take(hand);
            *kit = *next;
            let pockets = &mut kits[*kit];
            *hand = pockets.count.min(pockets.stored);
            pockets.stored -= *hand;
        }
    }

    /// A throw: whether there was a grenade in the hand (taken).
    pub(crate) fn throw(&mut self) -> bool {
        match self {
            Self::Unlimited => true,
            Self::Kits { hand, .. } => {
                let had = *hand > 0;
                *hand = hand.saturating_sub(1);
                had
            }
        }
    }

    /// Whether the run models the pockets (else every throw is carried and the carried counts are not given).
    pub(crate) fn is_modelled(&self) -> bool {
        matches!(self, Self::Kits { .. })
    }
}

/// What one body took from one blast.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Hurt {
    pub(crate) place: Place,
    /// Its feet's distance from the blast, m.
    pub(crate) distance: f32,
    /// What the body pool lost (a share of its health).
    pub(crate) body: f32,
    pub(crate) pain: f32,
    pub(crate) killed: bool,
}

/// The bots' grenade's blast (`loadouts::BOT_GRENADE`).
pub(crate) fn splash() -> &'static SplashParams {
    damage::splash_named(BOT_GRENADE.name).expect("the bots' grenade has a blast")
}

/// Where a body part is, for the line from the blast. GUESS: the client
/// takes each part's physics body; the stand-in has the chest, the legs'
/// middle and the feet at fixed heights over the feet.
pub(crate) fn part_point(body: &Body, part: &str) -> Vec3 {
    let up = match part {
        "right_leg" | "left_leg" | "right_hip" | "left_hip" => 0.5,
        "right_foot" | "left_foot" => 0.15,
        _ if body.keys & super::key::CROUCH != 0 => 0.8,
        _ => 1.3,
    };
    body.feet + Vec3::new(0.0, up, 0.0)
}

/// The blast of `thrower`'s grenade at `at` at `tick`: every body in its
/// radius loses what the client would take off it, the dead are killed
/// (`Game::kill`), the rest remember the hit (their regeneration counts
/// from it). The bodies it hurt, in place order.
pub(crate) fn explode(game: &mut Game, thrower: Place, at: Vec3, tick: u32) -> Vec<Hurt> {
    let splash = splash();
    let human = damage::human();
    // Up from the ground a little: the line is not to start in the floor.
    let origin = at + Vec3::new(0.0, 0.3, 0.0);
    let mut hurt = Vec::new();
    let mut deaths = Vec::new();
    for place in 0..game.bodies.len() {
        let body = &game.bodies[place];
        if !body.alive {
            continue;
        }
        let distance = (body.feet - at).length();
        let Some(amount) = splash.amount_at(distance) else {
            continue;
        };
        game.metric_rays += splash.affected.len() as u64;
        let hits: Vec<(usize, f32)> = splash
            .affected
            .iter()
            .map(|&index| {
                let part = &human.parts[index].name;
                let reached = game.world.sight.visible(origin, part_point(body, part));
                (index, if reached { amount } else { 0.0 })
            })
            .collect();
        let taken = human.hits_taken(&hits, 1.0, &|_| 0.0);
        let body = &mut game.bodies[place];
        let (mut on_body, mut on_pain) = (0.0, 0.0);
        for (pool, lost) in taken {
            if lost <= 0.0 {
                continue;
            }
            match pool.as_str() {
                "body" => {
                    body.health -= lost;
                    on_body = lost;
                }
                "pain" => {
                    body.pools.hurt(Pool::Pain, lost, tick);
                    on_pain = lost;
                }
                "head" => body.pools.hurt(Pool::Head, lost, tick),
                "right_leg" | "left_leg" => {
                    let leg = usize::from(pool == "left_leg");
                    body.legs[leg] = (body.legs[leg] - lost).max(0.0);
                    if body.legs[leg] <= 0.0 && body.leg_broken_until == 0 {
                        body.leg_broken_until = tick + STAND_IN_LEG_BROKEN_MS;
                    }
                }
                _ => {}
            }
        }
        if on_body > 0.0 || on_pain > 0.0 {
            body.last_hit = tick;
            body.stagger.hit(tick, GRENADE_STOPPING_POWER);
        }
        let killed = body.health <= 1e-3 || body.pools.dead();
        if killed {
            body.health = body.health.min(0.0);
            let kind = if place == thrower {
                Kill::Own
            } else if body.team == game.bodies[thrower].team {
                Kill::Mate
            } else {
                Kill::Enemy
            };
            deaths.push((place, kind));
        }
        hurt.push(Hurt {
            place,
            distance,
            body: on_body,
            pain: on_pain,
            killed,
        });
    }
    for (place, kind) in deaths {
        game.kill_by_frag = true;
        game.kill(place, thrower, kind, tick);
        game.kill_by_frag = false;
    }
    hurt
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::tests::standin_damage::Pools;

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 2e-3
    }

    /// A game on the soak world's open ground with bodies of `teams` put
    /// at the given places (x, z), alive.
    fn field(teams: &[u8], at: &[(f32, f32)]) -> Game {
        let mut game = Game::new(super::super::soak_world(), teams, 1, None, None);
        for (body, &(x, z)) in game.bodies.iter_mut().zip(at) {
            body.alive = true;
            body.feet = Vec3::new(x, 0.0, z);
        }
        game
    }

    fn blast(game: &mut Game, thrower: Place, x: f32, z: f32) -> Vec<Hurt> {
        explode(game, thrower, Vec3::new(x, 0.0, z), 1_000)
    }

    /// The RGD-5 at one's feet is the full 1.3: the body pool takes all of
    /// it, the thrower dies; at the radius it is 0.05; beyond it nothing.
    #[test]
    fn the_blast_is_linear_from_the_max_to_the_min() {
        let mut game = field(&[0, 0, 0], &[(10.0, 10.0), (15.0, 10.0), (21.0, 10.0)]);
        let hurt = blast(&mut game, 0, 10.0, 10.0);
        assert_eq!(hurt.len(), 2, "{hurt:?}");
        assert!(near(hurt[0].body, 1.3) && hurt[0].killed && !game.bodies[0].alive);
        // 5 m: 0.675 of the body pool, pain as much (the back's coefficient 1): alive.
        assert!(near(hurt[1].body, 0.675) && near(hurt[1].pain, 0.675) && !hurt[1].killed);
        assert!(near(game.bodies[1].health, 1.0 - 0.675));
        assert!(game.bodies[2].alive && game.bodies[2].health == 1.0);
        // The thrower's own death is no kill credit and no mate kill.
        assert_eq!((game.team_kills, game.kill_log.len(), game.stats[0].1), (0, 0, 1));
    }

    /// A pool takes the largest hit of one blast, not their sum: a body at 3 m (1.0 of the
    /// 1.3 .. 0.05 line is 0.87 at 3.3 m) loses `amount` once from the body pool though two
    /// parts feed it.
    #[test]
    fn one_blast_takes_a_pool_once() {
        let mut game = field(&[0, 1], &[(10.0, 10.0), (13.0, 10.0)]);
        let want = splash().amount_at(3.0).unwrap();
        let hurt = blast(&mut game, 0, 10.0, 10.0);
        let b = hurt.iter().find(|h| h.place == 1).unwrap();
        assert!(near(b.body, want) && near(b.pain, want), "{b:?} {want}");
        assert!(near(game.bodies[1].health, 1.0 - want));
        assert!(near(game.bodies[1].pools.pain, 1.0 - want));
    }

    /// A kill of an enemy is the thrower's kill; of a mate, a team kill.
    #[test]
    fn a_blast_kills_enemies_for_the_thrower_and_mates_as_team_kills() {
        let mut game = field(&[0, 0, 1], &[(10.0, 10.0), (11.0, 10.0), (10.0, 11.0)]);
        let hurt = blast(&mut game, 0, 10.0, 10.0);
        assert!(hurt.iter().all(|h| h.killed), "{hurt:?}");
        assert_eq!(game.team_kills, 1);
        assert_eq!(game.stats[0].0, 1, "one enemy kill");
        assert_eq!(game.kill_log.len(), 2);
        assert!(game.bodies.iter().all(|b| !b.alive));
    }

    /// The dead are not hurt again.
    #[test]
    fn the_dead_are_not_hurt() {
        let mut game = field(&[0, 1], &[(10.0, 10.0), (11.0, 10.0)]);
        game.bodies[1].alive = false;
        let hurt = blast(&mut game, 0, 10.0, 10.0);
        assert_eq!(hurt.len(), 1);
    }

    /// A leg pool at 0 breaks the leg: at 1 m of the blast both legs' pools (0.5) are emptied.
    #[test]
    fn a_blast_close_by_breaks_the_legs() {
        let mut game = field(&[0, 1], &[(10.0, 10.0), (12.0, 10.0)]);
        let hurt = blast(&mut game, 0, 10.0, 10.0);
        assert!(hurt.iter().any(|h| h.place == 1));
        assert_eq!(game.bodies[1].legs, [0.0, 0.0]);
        assert!(game.bodies[1].leg_broken_until > 0);
    }

    /// A blast's pain can kill a body whose body pool survives it: the pain pool's death is the
    /// client's (`thresholds`), and the body keeps what is left.
    #[test]
    fn pain_alone_kills() {
        let mut game = field(&[0, 1], &[(10.0, 10.0), (14.0, 10.0)]);
        game.bodies[1].pools = Pools::default();
        game.bodies[1].pools.pain = 0.1;
        let hurt = blast(&mut game, 0, 10.0, 10.0);
        let h = hurt.iter().find(|h| h.place == 1).unwrap();
        assert!(h.killed && !game.bodies[1].alive);
    }
}
