use super::*;

/// The grenades counts: counted as the match runs, kept for [`OwnerTally::finish`].
#[derive(Default)]
pub(super) struct GrenadeCounts {
    pub(super) blasts: Vec<Blast>,
    pub(super) grenades: u32,
    /// Blast damage to the thrower, to his teammates, to enemies; the
    /// bodies it would have killed of each; the bodies inside the radius
    /// when it went off, and of them those outside at the throw.
    pub(super) blast_dmg: [f32; 3],
    pub(super) blast_kills: [u32; 3],
    pub(super) blast_in: [u32; 3],
    pub(super) walked_in: [u32; 3],
}

/// Grenades: the RGD-5's blast (`rgd-5.options`: `splash_radius` 10 m,
/// `splash_max_damage` 1.3 to `splash_min_damage` 0.05; linear between:
/// GUESS), on a body with a line from the landing.
pub(crate) const BLAST_M: f32 = 10.0;

pub(crate) const BLAST_MAX: f32 = 1.3;

pub(crate) const BLAST_MIN: f32 = 0.05;

/// A grenade let go: the thrower, where it lands, when it goes off, and
/// the bodies inside its radius at the throw.
#[derive(Clone, Debug)]
pub(crate) struct Blast {
    pub(crate) thrower: Place,
    pub(crate) at: Vec3,
    pub(crate) time: u32,
    pub(crate) inside: Vec<bool>,
}

/// "throws a grenade at (x, y, z), blast at <ms>": the place and the time.
pub(crate) fn thrown_at(text: &str) -> Option<(Vec3, u32)> {
    let rest = text.strip_prefix("throws a grenade at (")?;
    let (xyz, rest) = rest.split_once("), blast at ")?;
    let mut v = xyz.split(", ").map(|n| n.parse::<f32>().ok());
    let at = Vec3::new(v.next()??, v.next()??, v.next()??);
    Some((at, rest.trim().parse().ok()?))
}

/// Where a grenade thrown from `eye` at `aim` comes down: the first point
/// the bullet mesh stops its arc at, else `aim`. The arc as two lines,
/// up to a top over the middle a quarter of the way's length higher (a
/// lob; GUESS: the throw's speed and the bounces are not modelled).
pub(crate) fn grenade_lands(sight: &mapdata::RayTree, eye: Vec3, aim: Vec3) -> Vec3 {
    let d = eye.distance_xz(aim);
    let top = (eye + aim) * 0.5 + Vec3::new(0.0, d * 0.25, 0.0);
    for (a, b) in [(eye, top), (top, aim)] {
        let way = b - a;
        let len = way.length();
        if len < 1e-3 {
            continue;
        }
        let dir = way * (1.0 / len);
        if let Some(t) = sight.ray_cast(a, dir, len) {
            return a + dir * (t - 0.05).max(0.0);
        }
    }
    aim
}

/// The blast's damage at `d` m (linear, [`BLAST_MAX`] to [`BLAST_MIN`]).
pub(crate) fn blast_damage(d: f32) -> f32 {
    if d > BLAST_M {
        0.0
    } else {
        BLAST_MAX - (BLAST_MAX - BLAST_MIN) * d / BLAST_M
    }
}

#[test]
pub(crate) fn a_grenade_at_ones_feet_hurts_the_thrower_and_mates_and_one_walked_in() {
    let (mut game, mut tally) = made_up(&[0, 0, 1, 0]);
    play(&mut game, &mut tally, 30, |_, game| {
        put(game, 0, 10.0, 10.0, 0);
        put(game, 1, 12.0, 10.0, 0);
        put(game, 2, 30.0, 10.0, 0);
        put(game, 3, 30.0, 14.0, 0);
        vec![bot_now(); 4]
    });
    // Thrown at tick 30, gone off at 1000 ms already: once more ticks run.
    let note = Note {
        place: 0,
        time: game.tick,
        text: "throws a grenade at (11, 0, 10), blast at 1000".to_owned(),
    };
    tally.note(&note, &game);
    // Bot 3, a mate far off at the throw, walks in before the blast.
    game.bodies[3].feet = Vec3::new(13.0, 0.0, 10.0);
    game.tick += TICK_MS;
    tally.step(&mut game, &[bot_now(), bot_now(), bot_now(), bot_now()]);
    let (_, json) = tally.finish(&game);
    assert_eq!(json["grenades"], 1);
    assert!(json["grenade_self_dmg"].as_f64().unwrap() > 0.5, "{json}");
    assert!(json["grenade_mate_dmg"].as_f64().unwrap() > 0.3);
    assert_eq!(json["grenade_enemy_dmg"], 0.0);
    assert_eq!(json["grenade_kills"][0], 1);
    assert_eq!(json["grenade_in_radius"], serde_json::json!([1, 2, 0]));
    assert_eq!(json["grenade_walk_ins"], serde_json::json!([0, 1, 0]));
}

#[test]
pub(crate) fn the_blast_falls_off_to_its_minimum_at_ten_metres() {
    assert!((blast_damage(0.0) - BLAST_MAX).abs() < 1e-6);
    assert!((blast_damage(BLAST_M) - BLAST_MIN).abs() < 1e-6);
    assert_eq!(blast_damage(10.5), 0.0);
}

impl OwnerTally {
    /// The grenades going off: who is inside, who walked in, the damage it would do (the stand-in has none).
    pub(super) fn grenades_off(&mut self, game: &Game, sight: &mapdata::RayTree, rays: &mut u64, tick: u32) {
        // The grenades going off: who is inside, who walked in, the damage
        // it would do (the stand-in has none).
        let mut gone = Vec::new();
        self.grenades.blasts.retain(|b| {
            let off = tick >= b.time;
            if off {
                gone.push(b.clone());
            }
            !off
        });
        for blast in gone {
            let Some(team) = game.bodies.get(blast.thrower).map(|b| b.team) else {
                continue;
            };
            for (place, body) in game.bodies.iter().enumerate() {
                let d = (Game::chest(body) - blast.at).length();
                if !body.alive || d > BLAST_M {
                    continue;
                }
                let who = if place == blast.thrower {
                    0
                } else if body.team == team {
                    1
                } else {
                    2
                };
                self.grenades.blast_in[who] += 1;
                self.grenades.walked_in[who] += u32::from(!blast.inside.get(place).copied().unwrap_or(false));
                *rays += 1;
                if !sight.visible(blast.at + Vec3::new(0.0, 0.3, 0.0), Game::chest(body)) {
                    continue;
                }
                let dmg = blast_damage(d);
                self.grenades.blast_dmg[who] += dmg;
                self.grenades.blast_kills[who] += u32::from(dmg >= body.health);
            }
        }
    }

    /// The grenades counts at the end: their keys of the `owner` object and their lines of the owner-test text.
    pub(super) fn grenades_part(&self, game: &Game) -> Part {
        let json = serde_json::json!({
            "grenades": self.grenades.grenades,
            "grenade_damage_applied": false,
            "grenade_self_dmg": (100.0 * self.grenades.blast_dmg[0]).round() / 100.0,
            "grenade_mate_dmg": (100.0 * self.grenades.blast_dmg[1]).round() / 100.0,
            "grenade_enemy_dmg": (100.0 * self.grenades.blast_dmg[2]).round() / 100.0,
            "grenade_kills": self.grenades.blast_kills,
            "grenade_in_radius": self.grenades.blast_in,
            "grenade_walk_ins": self.grenades.walked_in,
            "team_kills_gun": game.team_kills,
            "team_hits": game.team_hits,
        });
        let text = format!(
            "owner-test other: grenades {} (no damage in the stand-in) would damage self {:.2} mates {:.2} enemies {:.2}, would kill {:?}, inside at the blast {:?} walked in {:?}; team kills by gunfire {}, team hits {}",
            self.grenades.grenades,
            self.grenades.blast_dmg[0],
            self.grenades.blast_dmg[1],
            self.grenades.blast_dmg[2],
            self.grenades.blast_kills,
            self.grenades.blast_in,
            self.grenades.walked_in,
            game.team_kills,
            game.team_hits,
        );
        Part::new(json, text)
    }
}
