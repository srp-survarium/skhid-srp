use crate::v026g::bots::arc;
use crate::v026g::bots::tests::standin_grenade;
use crate::v026g::bots::view::{eye_height, still_eye};

use super::*;

/// The grenade leaves the hand this long after the key goes down, ms
/// (`arc::LAUNCH_DELAY_S`: the throw animation's event, LIKELY +-200 ms).
const LAUNCH_MS: u32 = (arc::LAUNCH_DELAY_S * 1000.0) as u32;

/// The bot is to hold its look, and not fire, this long after the press, ms:
/// the launch and the 300 ms of `CombatTuning::grenade_look_after_ms`.
const HELD_MS: u32 = LAUNCH_MS + 300;

/// The fuse from the press, ms (`arc::FUSE_S`).
const FUSE_MS: u32 = (arc::FUSE_S * 1000.0) as u32;

/// A first contact nearer than this to a thrower who aimed beyond it is a
/// short throw, metres (the blast's 10 m and 2 m more:
/// `CombatTuning::grenade_land_m`).
const SHORT_M: f32 = 12.0;

/// A grenade pressed: the thrower, the place it was aimed at, the press.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Throw {
    pub(crate) thrower: Place,
    pub(crate) aim: Vec3,
    pub(crate) press: u32,
}

/// The grenades counts: counted as the match runs, kept for [`OwnerTally::finish`].
#[derive(Default)]
pub(super) struct GrenadeCounts {
    pub(super) blasts: Vec<Blast>,
    /// The throws noted, not yet at their launch ([`LAUNCH_MS`] after the
    /// press): the blast is where the thrower's view then sends it.
    pub(super) throws: Vec<Throw>,
    /// By bot: the tick of the press whose hold (to [`HELD_MS`]) is on.
    pub(super) holds: BTreeMap<Place, u32>,
    /// Throws whose first contact fell within [`SHORT_M`] of a thrower who
    /// aimed beyond it, from his view at the launch.
    pub(super) short: u32,
    /// The thrower died between the press and the launch.
    pub(super) died_throwing: u32,
    /// A fire press between the press and the end of the hold.
    pub(super) fired_after_held: u32,
    /// The throws the bot weighed and did not make: the thrower in the
    /// blast (what the arc hits within it, a wall and a ceiling too), a
    /// steep wall beyond it, a teammate in the blast, any other
    /// (out of reach, nowhere to land, off the place).
    pub(super) refused: [u32; 4],
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

/// "holds a grenade back at (x, y, z): <reason>": the reason.
pub(crate) fn held_back(text: &str) -> Option<&str> {
    let rest = text.strip_prefix("holds a grenade back at (")?;
    rest.split_once("): ").map(|(_, why)| why)
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
    play(&mut game, &mut tally, 100, |_, game| {
        put(game, 0, 10.0, 10.0, 0);
        put(game, 1, 12.0, 10.0, 0);
        put(game, 2, 30.0, 10.0, 0);
        put(game, 3, 30.0, 14.0, 0);
        // The thrower looks steeply down: the grenade lands a metre or so ahead.
        game.bodies[0].pitch = -arc::PITCH_LIMIT;
        vec![bot_now(); 4]
    });
    // Pressed at 0 ms, noted late, gone off at 4000 ms already: once more ticks run.
    let note = Note {
        place: 0,
        time: game.tick,
        text: "throws a grenade at (11, 0, 10), blast at 4000".to_owned(),
    };
    tally.note(&note, &game);
    // Bot 3, a mate far off at the throw, walks in before the blast.
    game.bodies[3].feet = Vec3::new(13.0, 0.0, 10.0);
    game.tick += TICK_MS;
    tally.step(&mut game, &[bot_now(), bot_now(), bot_now(), bot_now()]);
    let (_, json) = tally.finish(&game);
    assert_eq!(json["grenades"], 1);
    // The blast is at the rest point, 2 m on from the first contact a metre ahead: the thrower is near
    // enough to be all but killed (0.96 of his health), not quite.
    assert!(json["grenade_self_dmg"].as_f64().unwrap() > 0.9, "{json}");
    assert!(json["grenade_mate_dmg"].as_f64().unwrap() > 0.3);
    assert_eq!(json["grenade_enemy_dmg"], 0.0);
    assert_eq!(json["grenade_in_radius"], serde_json::json!([1, 2, 0]));
    assert_eq!(json["grenade_walk_ins"], serde_json::json!([0, 1, 0]));
}

#[test]
pub(crate) fn the_blast_falls_off_to_its_minimum_at_ten_metres() {
    assert!((blast_damage(0.0) - BLAST_MAX).abs() < 1e-6);
    assert!((blast_damage(BLAST_M) - BLAST_MIN).abs() < 1e-6);
    assert_eq!(blast_damage(10.5), 0.0);
}

/// A thrower at (10, 10) with a view of `pitch` facing +z, a throw noted at 20 m: the tally's counts a
/// second and a half on.
fn thrown_with_pitch(pitch: f32) -> serde_json::Value {
    let (mut game, mut tally) = made_up(&[0, 0, 1]);
    play(&mut game, &mut tally, 40, |_, game| {
        put(game, 0, 10.0, 10.0, 0);
        put(game, 1, 14.0, 10.0, 0);
        put(game, 2, 30.0, 40.0, 0);
        game.bodies[0].yaw = 0.0;
        game.bodies[0].pitch = pitch;
        vec![bot_now(); 3]
    });
    let note = Note {
        place: 0,
        time: game.tick,
        text: "throws a grenade at (10.0, 0.0, 30.0), blast at 1000".to_owned(),
    };
    tally.note(&note, &game);
    game.tick += TICK_MS;
    tally.step(&mut game, &[bot_now(), bot_now(), bot_now()]);
    tally.finish(&game).1
}

#[test]
pub(crate) fn a_throw_that_lands_near_the_thrower_though_aimed_far_is_short() {
    // The flat throw: a level view lands 8 m out (the hand 0.3 m under the eye).
    let level = thrown_with_pitch(0.0);
    assert_eq!(level["grenade_short"], 1, "{level}");
    // The pitch that lands it at 20 m: not short.
    let pitch = arc::pitch_for(20.0, still_eye(Vec3::ZERO, false).y - arc::HAND_DOWN_M).unwrap();
    let aimed = thrown_with_pitch(pitch);
    assert_eq!(aimed["grenade_short"], 0, "{aimed}");
}

#[test]
pub(crate) fn the_thrower_dying_before_the_launch_and_a_shot_in_the_hold_are_counted() {
    let run = |die_at: usize, fire_at: usize| {
        let (mut game, mut tally) = made_up(&[0, 1]);
        play(&mut game, &mut tally, 60, |k, game| {
            // The key goes down at tick 5 and stays 12 ticks.
            let grenade = if (5..17).contains(&k) { key::GRENADE } else { 0 };
            let fire = if k == fire_at { key::FIRE } else { 0 };
            put(game, 0, 10.0, 10.0, grenade | fire);
            put(game, 1, 40.0, 40.0, 0);
            game.bodies[0].alive = k < die_at;
            vec![bot_now(); 2]
        });
        tally.finish(&game).1
    };
    // Dies at tick 20 (15 ticks after the press, 750 ms: before the launch); fires at tick 10.
    let json = run(20, 10);
    assert_eq!(json["grenade_died_throwing"], 1, "{json}");
    assert_eq!(json["grenade_fired_after_held"], 1, "{json}");
    // Dies at tick 40 (after the launch and the hold) and fires at tick 50: neither.
    let json = run(40, 50);
    assert_eq!(json["grenade_died_throwing"], 0, "{json}");
    assert_eq!(json["grenade_fired_after_held"], 0, "{json}");
    // Dies between the launch and the end of the hold (at 1.25 s = tick 30): not before the launch.
    let json = run(30, 0);
    assert_eq!(json["grenade_died_throwing"], 0, "{json}");
}

#[test]
pub(crate) fn a_shot_after_the_throw_was_given_up_is_not_a_shot_in_the_hold() {
    // The key goes down at tick 5; a mate walks in and the bot notes the throw held back at tick 8 (the key
    // let go); it fires at tick 12 (the stand-in's key). Fired at tick 7 it was in the hold.
    let run = |fire_at: usize, given_up: bool| {
        let (mut game, mut tally) = made_up(&[0, 1]);
        for k in 0..60usize {
            game.tick = (k as u32 + 1) * TICK_MS;
            if given_up && k == 8 {
                let note = Note {
                    place: 0,
                    time: game.tick,
                    text: "holds a grenade back at (1.0, 0.0, 2.0): teammate 3".to_owned(),
                };
                tally.note(&note, &game);
            }
            let grenade = if (5..8).contains(&k) { key::GRENADE } else { 0 };
            let fire = if k == fire_at { key::FIRE } else { 0 };
            put(&mut game, 0, 10.0, 10.0, grenade | fire);
            put(&mut game, 1, 40.0, 40.0, 0);
            tally.step(&mut game, &[bot_now(), bot_now()]);
            game.hit_log.clear();
        }
        tally.finish(&game).1
    };
    assert_eq!(run(12, true)["grenade_fired_after_held"], 0);
    assert_eq!(run(7, true)["grenade_fired_after_held"], 1);
    assert_eq!(run(12, false)["grenade_fired_after_held"], 1);
}

#[test]
pub(crate) fn the_refusals_are_counted_by_their_reason() {
    let (game, mut tally) = made_up(&[0, 1]);
    for why in [
        "lands in the thrower's blast",
        "hits a wall near the thrower",
        "teammate 3",
        "teammate 4",
        "out of reach",
        "lands off the place",
    ] {
        let note = Note {
            place: 0,
            time: 1000,
            text: format!("holds a grenade back at (1.0, 0.0, 2.0): {why}"),
        };
        tally.note(&note, &game);
    }
    // An old-form line (a teammate; no reason after the place) is read as well.
    assert_eq!(held_back("holds a grenade back at (1.0, 0.0, 2.0): teammate 7"), Some("teammate 7"));
    assert_eq!(held_back("throws a grenade at (1.0, 0.0, 2.0), blast at 5"), None);
    let (_, json) = tally.finish(&game);
    assert_eq!(json["grenade_refused"], serde_json::json!([1, 1, 2, 2]));
    assert_eq!(json["grenades"], 0);
}

#[test]
pub(crate) fn the_launch_constants_are_the_arcs() {
    assert_eq!(LAUNCH_MS, 1100);
    assert_eq!(FUSE_MS, 4000);
    assert_eq!(HELD_MS, 1400);
}

impl OwnerTally {
    /// A throw noted at its let-go: the blast is [`FUSE_MS`] after the press; where it lands is worked out at
    /// the launch ([`Self::grenade_launches`]).
    pub(super) fn grenade_thrown(&mut self, thrower: Place, aim: Vec3, blast: u32) {
        self.grenades.grenades += 1;
        self.grenades.throws.push(Throw {
            thrower,
            aim,
            press: blast.saturating_sub(FUSE_MS),
        });
    }

    /// A throw the bot weighed and did not make, by the reason it noted (`reflex.rs`, `Refuse::why`).
    pub(super) fn grenade_refused(&mut self, why: &str) {
        let at = if why.contains("thrower's blast") {
            0
        } else if why.contains("wall") {
            1
        } else if why.starts_with("teammate") {
            2
        } else {
            3
        };
        self.grenades.refused[at] += 1;
    }

    /// The bot let go of the key and gave the throw up: no hold is open for it.
    pub(super) fn grenade_given_up(&mut self, thrower: Place) {
        self.grenades.holds.remove(&thrower);
    }

    /// A bot's grenade key at this tick: a press opens a hold of [`HELD_MS`]; in it, the thrower dying before
    /// the launch and a fire press are counted. From the stand-in's keys, not the bot's word.
    pub(super) fn grenades_keys(&mut self, place: Place, tick: u32, body: &super::super::Body, prev: &Rec) {
        let g = &mut self.grenades;
        if body.keys & key::GRENADE != 0 && prev.keys & key::GRENADE == 0 {
            g.holds.entry(place).or_insert(tick);
        }
        let Some(&press) = g.holds.get(&place) else {
            return;
        };
        if tick >= press + HELD_MS {
            g.holds.remove(&place);
        } else if !body.alive {
            g.died_throwing += u32::from(tick < press + LAUNCH_MS);
            g.holds.remove(&place);
        } else if body.keys & key::FIRE != 0 && prev.keys & key::FIRE == 0 {
            g.fired_after_held += 1;
        }
    }

    /// The throws at their launch: the grenade leaves [`LAUNCH_MS`] after the press along the thrower's view
    /// then (his record), and comes down on the bullet mesh where `arc` says; the blast is there, at the rest
    /// point, [`FUSE_MS`] after the press. Who is inside is by the bodies' places at the launch.
    fn grenade_launches(&mut self, game: &Game, tick: u32) {
        let (due, later): (Vec<Throw>, Vec<Throw>) = std::mem::take(&mut self.grenades.throws)
            .into_iter()
            .partition(|t| tick >= t.press + LAUNCH_MS);
        self.grenades.throws = later;
        for throw in due {
            let k = self.index(throw.press + LAUNCH_MS);
            let bodies: Vec<Option<Rec>> = self.recs.iter().map(|r| r.get(k).or(r.last()).copied()).collect();
            let Some(rec) = bodies.get(throw.thrower).copied().flatten().filter(|r| r.alive) else {
                continue;
            };
            let up = eye_height(rec.keys & key::CROUCH != 0, rec.speed, rec.pitch);
            let eye = rec.feet + Vec3::new(0.0, up, 0.0);
            let Some(landing) =
                arc::Flight::thrown(eye, rec.yaw, rec.pitch).landing(&arc::Mesh(&game.world.sight))
            else {
                continue;
            };
            if throw.aim.distance_xz(rec.feet) >= SHORT_M && landing.contact.point.distance_xz(rec.feet) < SHORT_M {
                self.grenades.short += 1;
            }
            let inside = bodies
                .iter()
                .map(|r| {
                    r.is_some_and(|r| {
                        let chest = r.feet + Vec3::new(0.0, if r.keys & key::CROUCH != 0 { 0.8 } else { 1.3 }, 0.0);
                        r.alive && (chest - landing.rest).length() <= BLAST_M
                    })
                })
                .collect();
            self.grenades.blasts.push(Blast {
                thrower: throw.thrower,
                at: landing.rest,
                time: throw.press + FUSE_MS,
                inside,
            });
        }
    }

    /// The grenades going off: who is inside, who walked in, the damage it would do (the stand-in has none).
    pub(super) fn grenades_off(&mut self, game: &mut Game, sight: &mapdata::RayTree, rays: &mut u64, tick: u32) {
        self.grenade_launches(game, tick);
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
            if game.damage.blasts_hurt() {
                // The client's blast on the bodies ([`standin_grenade`]).
                for hurt in standin_grenade::explode(game, blast.thrower, blast.at, tick) {
                    let who = if hurt.place == blast.thrower {
                        0
                    } else if game.bodies[hurt.place].team == team {
                        1
                    } else {
                        2
                    };
                    self.grenades.blast_in[who] += 1;
                    self.grenades.walked_in[who] +=
                        u32::from(!blast.inside.get(hurt.place).copied().unwrap_or(false));
                    self.grenades.blast_dmg[who] += hurt.body;
                    self.grenades.blast_kills[who] += u32::from(hurt.killed);
                }
                continue;
            }
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
            "grenade_damage_applied": game.damage.blasts_hurt(),
            "grenade_self_dmg": (100.0 * self.grenades.blast_dmg[0]).round() / 100.0,
            "grenade_mate_dmg": (100.0 * self.grenades.blast_dmg[1]).round() / 100.0,
            "grenade_enemy_dmg": (100.0 * self.grenades.blast_dmg[2]).round() / 100.0,
            "grenade_kills": self.grenades.blast_kills,
            "grenade_in_radius": self.grenades.blast_in,
            "grenade_walk_ins": self.grenades.walked_in,
            "grenade_short": self.grenades.short,
            "grenade_died_throwing": self.grenades.died_throwing,
            "grenade_fired_after_held": self.grenades.fired_after_held,
            "grenade_refused": self.grenades.refused,
            "team_kills_gun": game.team_kills,
            "team_hits": game.team_hits,
        });
        let text = format!(
            "owner-test other: grenades {} ({}) {} self {:.2} mates {:.2} enemies {:.2}, {} {:?}, inside at the blast {:?} walked in {:?}; short {} (aimed beyond {SHORT_M} m, first contact within), thrower died in the press to launch {}, fire pressed in the hold {}, refused (thrower's blast, wall, teammate, other) {:?}; team kills by gunfire {}, team hits {}",
            self.grenades.grenades,
            if !game.damage.blasts_hurt() { "no damage in the stand-in" } else { "the client's blast" },
            if !game.damage.blasts_hurt() { "would damage" } else { "damaged" },
            self.grenades.blast_dmg[0],
            self.grenades.blast_dmg[1],
            self.grenades.blast_dmg[2],
            if !game.damage.blasts_hurt() { "would kill" } else { "killed" },
            self.grenades.blast_kills,
            self.grenades.blast_in,
            self.grenades.walked_in,
            self.grenades.short,
            self.grenades.died_throwing,
            self.grenades.fired_after_held,
            self.grenades.refused,
            game.team_kills,
            game.team_hits,
        );
        Part::new(json, text)
    }
}
