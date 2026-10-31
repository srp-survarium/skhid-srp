use crate::v026g::bots::arc;
use crate::v026g::bots::grenade_threat::AvoidStats;
use crate::v026g::bots::live_grenades::{LiveGrenade, Phase, ThrowCounts};
use crate::v026g::bots::reflex::GrenadeStats;
use crate::v026g::bots::tests::standin_grenade;
use crate::v026g::bots::view::{eye_height, still_eye};

use super::*;

/// The grenade leaves the hand this long after the key goes down, ms
/// (`arc::LAUNCH_DELAY_S`: the throw animation's event, LIKELY +-200 ms).
const LAUNCH_MS: u32 = (arc::LAUNCH_DELAY_S * 1000.0) as u32;

/// The bot is to hold its look, and not fire, this long after the press, ms:
/// the launch and the 300 ms of `CombatTuning::grenade_look_after_ms`.
const HELD_MS: u32 = LAUNCH_MS + 300;

/// The fuse from the press, ms: the bots' grenade's (`arc::Grenade::bots`, the game data's `ttl_ms`).
fn fuse_ms() -> u32 {
    arc::Grenade::bots().fuse_ms()
}

/// A first contact nearer than this to a thrower who aimed beyond it is a
/// short throw, metres (the blast's 10 m and 2 m more:
/// `CombatTuning::grenade_land_m`).
const SHORT_M: f32 = 12.0;

/// Two throws of one team at one spot: within these radii (metres, counted in the nearest) and this long
/// apart, ms (`CombatTuning::grenade_claim_m` and `_ms`; the humans' figures: `grenades-record.md` 9).
const SAME_SPOT_M: [f32; 3] = [3.0, 5.0, 8.0];
const SAME_SPOT_MS: u32 = 6_000;

/// A grenade pressed: the thrower, the place it was aimed at, the press.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Throw {
    pub(crate) thrower: Place,
    pub(crate) team: u8,
    pub(crate) aim: Vec3,
    pub(crate) press: u32,
}

/// What the bots' grenade logic counted (`reflex::GrenadeStats`, one reflex a life), added up over the
/// match: the readings are taken as the bots think, the change from the last reading of the same reflex
/// is added, a younger reflex is a new life.
#[derive(Default)]
pub(super) struct GrenadeLogic {
    last: BTreeMap<Place, GrenadeStats>,
    sum: GrenadeStats,
    /// Lives of bots whose reflex thought at least once.
    lives: u32,
}

/// The grenades counts: counted as the match runs, kept for [`OwnerTally::finish`].
#[derive(Default)]
pub(super) struct GrenadeCounts {
    /// By place: what the bot's grenade logic counted at this tick's reading (none: not a bot).
    pub(super) reading: Vec<Option<GrenadeStats>>,
    pub(super) logic: GrenadeLogic,
    pub(super) close: CloseBlasts,
    /// [diagnostic] The blasts gone off since the last drain: (time, thrower, where it went off, the place aimed at, contact point).
    pub(crate) gone_log: Vec<(u32, Place, Vec3, Vec3, (f32, f32, Vec3, Vec3, [Vec3; 2]))>,
    /// Throws as (thrower, team, the place, press) for [`SAME_SPOT_M`], and the pairs found: another bot
    /// of the team threw at a place within 3, 3-5, 5-8 m of this one within [`SAME_SPOT_MS`] before.
    pub(super) same_spot_log: Vec<(Place, u8, Vec3, u32)>,
    pub(super) same_spot: [u32; 3],
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
    /// The server's list of live grenades at this tick (`Tracker::live_grenades`) and what its throw keys
    /// came to; `None` until a bench tick has read them (the made-up matches never do).
    pub(super) live: Vec<LiveGrenade>,
    pub(super) listed: Option<ThrowCounts>,
    pub(super) live_blasts: LiveBlasts,
    /// Bodies inside the radius at the launch, by [thrower, his teammates, enemies] (bench runs only): the
    /// base of "left the radius by the blast" (`blast_in` less `walked_in` stayed in).
    pub(super) in_launch: [u32; 3],
    /// By bot: what its avoidance of live grenades has counted this match (the last reading).
    pub(super) avoid: BTreeMap<Place, AvoidStats>,
    /// The bots' alive time, ms, by the nearest alive enemy (`< 8 m`, `8-30 m`, farther, none alive), and the
    /// throws let go by the same bins at the let-go: the rate of a bot's throws per alive minute and per minute
    /// with an enemy 8-30 m away, counted as the humans' is (`grenades-record/scripts/rate_human.py`: the
    /// players' worlds, an enemy any alive player of the other team).
    pub(super) alive_by_enemy_ms: [u64; 4],
    pub(super) thrown_by_enemy: [u32; 4],
    /// Throws made with a grenade in the hand, and throws with none (phantom: the reflex's own count of two a life, no pockets).
    /// The grenades are the stand-in's (`standin_grenade::Stock`, per body): it explodes only a throw that had one.
    pub(super) carried_throws: u32,
    pub(super) phantom_throws: u32,
    /// The same by the thrower's team: `[carried, phantom]` of team 0 and of team 1.
    carry_by_team: [[u32; 2]; 2],
}

/// The bin of the nearest alive enemy of `place`: 0 under 8 m, 1 from 8 to 30 m, 2 farther, 3 none alive.
fn enemy_bin(game: &Game, place: Place) -> usize {
    let me = &game.bodies[place];
    let nearest = game
        .bodies
        .iter()
        .enumerate()
        .filter(|(other, body)| *other != place && body.alive && body.team != me.team)
        .map(|(_, body)| body.feet.distance_xz(me.feet))
        .fold(f32::INFINITY, f32::min);
    if nearest < 8.0 {
        0
    } else if nearest <= 30.0 {
        1
    } else if nearest.is_finite() {
        2
    } else {
        3
    }
}

/// The tally's blasts against the list's entries for the same throw: found with a landing, found still in the
/// hand (not launched by then), not found; and for those with a landing how far its rest point is from the
/// tally's own, by `<= 1 m`, `<= 2 m`, `<= 4 m` and more.
#[derive(Default)]
pub(super) struct LiveBlasts {
    pub(super) matched: u32,
    pub(super) pending: u32,
    pub(super) missing: u32,
    pub(super) err: [u32; 4],
}

/// Grenades: the bots' grenade's blast, from the game data (the RGD-5's `rgd-5.options`: `splash_radius` 10 m,
/// `splash_max_damage` 1.3 to `splash_min_damage` 0.05; linear between: GUESS), on a body with a line from the
/// landing.
pub(crate) fn blast_m() -> f32 {
    arc::Grenade::bots().splash_m
}

fn blast_params() -> survarium::v026g::weapons::GrenadeParams {
    survarium::v026g::weapons::grenade_named(survarium::v026g::loadouts::BOT_GRENADE.name)
        .expect("the game data has the bots' grenade")
}

pub(crate) fn blast_max() -> f32 {
    blast_params().splash_max_damage
}

pub(crate) fn blast_min() -> f32 {
    blast_params().splash_min_damage
}

/// A grenade let go: the thrower, where it lands, when it goes off, and
/// the bodies inside its radius at the throw.
#[derive(Clone, Debug)]
pub(crate) struct Blast {
    pub(crate) thrower: Place,
    /// [diagnostic] The place the throw was aimed at.
    pub(crate) aim: Vec3,
    /// [diagnostic] The view and eye the tally threw from at the launch, and where the arc first touched.
    pub(crate) launch: (f32, f32, Vec3, Vec3, [Vec3; 2]),
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

/// "throws a close grenade, blast at <ms>" (`reflex::start_close`, noted with the throw): the time of the blast.
pub(crate) fn closed_at(text: &str) -> Option<u32> {
    text.strip_prefix("throws a close grenade, blast at ")?.trim().parse().ok()
}

/// What the close throws (`grenade_close`) did to their own side: counted at their blasts.
#[derive(Clone, Debug, Default)]
pub(super) struct CloseBlasts {
    /// The blasts of throws the bot decided as close ones: (thrower, time).
    pub(super) noted: Vec<(Place, u32)>,
    /// Close blasts gone off, and of them those that hurt the thrower or a mate.
    pub(super) gone: u32,
    pub(super) leaks: u32,
    /// Damage and kills to [thrower, his teammates, enemies], and the bodies inside the radius.
    pub(super) dmg: [f32; 3],
    pub(super) kills: [u32; 3],
    pub(super) inside: [u32; 3],
    /// The least distance of the thrower, and of a teammate, to the blast, m.
    pub(super) least_m: [f32; 2],
    /// The first leaks: (thrower's distance, nearest mate's distance, the thrower's damage, the mates').
    pub(super) cases: Vec<[f32; 4]>,
}

/// "holds a grenade back at (x, y, z): <reason>": the reason.
pub(crate) fn held_back(text: &str) -> Option<&str> {
    let rest = text.strip_prefix("holds a grenade back at (")?;
    rest.split_once("): ").map(|(_, why)| why)
}

/// The blast's damage at `d` m (linear, `blast_max()` to `blast_min()`).
pub(crate) fn blast_damage(d: f32) -> f32 {
    if d > blast_m() {
        0.0
    } else {
        blast_max() - (blast_max() - blast_min()) * d / blast_m()
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
    tally.step_with(&mut game, &[bot_now(), bot_now(), bot_now(), bot_now()]);
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

/// The bodies inside the radius at the launch are counted by group (thrower, his teammates, enemies) for the share
/// that left by the blast: here the thrower, one mate and one enemy are near, a second mate is far and walks in (not
/// inside at the launch, inside at the blast). The key is only where a bench tick read the list.
#[test]
pub(crate) fn the_bodies_inside_the_radius_at_the_launch_are_counted_by_group() {
    let run = |bench: bool| {
        let (mut game, mut tally) = made_up(&[0, 0, 1, 0]);
        if bench {
            tally.topic::<GrenadeCounts>().listed = Some(ThrowCounts::default());
        }
        play(&mut game, &mut tally, 100, |_, game| {
            put(game, 0, 10.0, 10.0, 0);
            put(game, 1, 12.0, 10.0, 0);
            put(game, 2, 14.0, 10.0, 0);
            put(game, 3, 30.0, 14.0, 0);
            game.bodies[0].pitch = -arc::PITCH_LIMIT;
            vec![bot_now(); 4]
        });
        let note = Note {
            place: 0,
            time: game.tick,
            text: "throws a grenade at (11, 0, 10), blast at 4000".to_owned(),
        };
        tally.note(&note, &game);
        game.bodies[3].feet = Vec3::new(13.0, 0.0, 10.0);
        game.tick += TICK_MS;
        tally.step_with(&mut game, &[bot_now(), bot_now(), bot_now(), bot_now()]);
        tally.finish(&game).1
    };
    let json = run(true);
    assert_eq!(json["grenade_live"]["inside_at_launch"], serde_json::json!([1, 1, 1]), "{json}");
    // At the blast the walked-in mate is inside too: the share that left is read against the launch's.
    assert_eq!(json["grenade_in_radius"], serde_json::json!([1, 2, 1]), "{json}");
    assert_eq!(json["grenade_walk_ins"], serde_json::json!([0, 1, 0]), "{json}");
    assert!(run(false).get("grenade_live").is_none());
}

/// The stand-in explodes only the throws a thrower had a grenade in the hand for: the store is the client's (a spawn fills the
/// hand from the kit's store and what is left goes back), so a store of the count (today's bots) gives a grenade once a kit and
/// match, and a big store (A144) gives the count every life. No stock modelled: every throw is carried and none split.
#[test]
pub(crate) fn only_a_throw_with_a_grenade_in_hand_goes_off() {
    use crate::v026g::bots::tests::standin_grenade::{KitGrenades, Stock};
    let at = Vec3::new(10.0, 0.0, 10.0);
    let kit = |count, stored| KitGrenades { count, stored };
    let (mut game, _) = made_up(&[0, 1]);
    // Bot 0: kit 0 two grenades with a store of two (once a match); bot 1: kit 0 none.
    game.bodies[0].grenades = Stock::of([kit(2, 2), kit(0, 0), kit(0, 0)]);
    game.bodies[0].grenades.spawn();
    game.bodies[1].grenades = Stock::of([kit(0, 0); 3]);
    game.bodies[1].grenades.spawn();
    let mut counts = GrenadeCounts::default();
    let throws: Vec<bool> = (0..3).map(|_| counts.carries(&mut game, 0, 0)).collect();
    assert_eq!(throws, [true, true, false]);
    assert_eq!((counts.carried_throws, counts.phantom_throws), (2, 1));
    // The next life: the store is empty, a grenade thrown is gone for the match.
    game.bodies[0].grenades.spawn();
    assert!(!counts.carries(&mut game, 0, 0));
    // A bot whose kit holds none, by its team.
    assert!(!counts.carries(&mut game, 1, 1));
    assert_eq!((counts.carried_throws, counts.phantom_throws), (2, 3));
    assert_eq!(counts.carry_by_team, [[2, 2], [0, 1]]);
    // A store of 100 (A144): the count every life, the leftovers of an unthrown one back in the store.
    game.bodies[0].grenades = Stock::of([kit(2, 100), kit(0, 0), kit(0, 0)]);
    game.bodies[0].grenades.spawn();
    assert!(counts.carries(&mut game, 0, 0) && counts.carries(&mut game, 0, 0) && !counts.carries(&mut game, 0, 0));
    game.bodies[0].grenades.spawn();
    assert!(counts.carries(&mut game, 0, 0), "a new life has them again");
    // No stock modelled: every throw is carried and nothing is split.
    let mut free = GrenadeCounts::default();
    let (mut open, _) = made_up(&[0]);
    assert!((0..3).all(|_| free.carries(&mut open, 0, 0)));
    assert_eq!((free.carried_throws, free.phantom_throws), (0, 0), "unchecked throws are not split");
    // A throw is pending until its launch.
    free.grenade_thrown(0, 0, at, 5_000);
    assert_eq!(free.throws.len(), 1);
}

/// The bots' grenade logic counts are added up by the change since a bot's last reading, and a reflex
/// younger than the last reading is a new life; nothing is said where no bot thought.
#[test]
pub(crate) fn the_bots_grenade_logic_is_added_up_by_reading_and_by_life() {
    let (mut game, mut tally) = made_up(&[0, 1]);
    let reading = |thinks: u32, rolls: u32, first_throws: u32| {
        let mut g = GrenadeStats {
            thinks,
            rolls,
            ..GrenadeStats::default()
        };
        g.places[0] = rolls;
        g.nth[0] = first_throws;
        Some(BotNow {
            play: "Roam".to_owned(),
            grenades: g,
            ..BotNow::default()
        })
    };
    // One bot: a life read at 5 and 9 thinks, a second life read at 2 and 6.
    for (k, (thinks, rolls, throws)) in [(5, 1, 0), (9, 2, 1), (2, 1, 0), (6, 1, 0)].into_iter().enumerate() {
        game.tick = (k as u32 + 1) * TICK_MS;
        tally.step_with(&mut game, &[reading(thinks, rolls, throws), None]);
    }
    let (_, json) = tally.finish(&game);
    let logic = &json["grenade_logic"];
    assert_eq!(logic["bot_lives"], 2, "{json}");
    assert_eq!(logic["rolls"], 3, "{json}");
    assert_eq!(logic["places"], serde_json::json!([3, 0, 0, 0, 0]), "{json}");
    assert_eq!(logic["nth"], serde_json::json!([1, 0, 0]), "{json}");
    assert_eq!(logic["volley"], serde_json::json!([0, 0, 0, 0]), "{json}");
    assert_eq!(logic["near"], serde_json::json!([0, 0, 0]), "{json}");
    // No bot thought: no key.
    let (mut game, mut tally) = made_up(&[0, 1]);
    game.tick = TICK_MS;
    tally.step_with(&mut game, &[bot_now(), bot_now()]);
    let (_, json) = tally.finish(&game);
    assert!(json.get("grenade_logic").is_none(), "{json}");
}

/// Throws of one team at one spot: counted by radius for each earlier throw of another bot of the team
/// within the window; the bot's own volley and the other team's are not.
#[test]
pub(crate) fn throws_of_one_team_at_one_spot_are_counted_by_radius() {
    let (mut game, mut tally) = made_up(&[0, 0, 1]);
    let note = |place: Place, text: &str| Note {
        place,
        time: 0,
        text: text.to_owned(),
    };
    tally.note(&note(0, "throws a grenade at (10.0, 0.0, 20.0), blast at 8000"), &game);
    tally.note(&note(1, "throws a grenade at (12.0, 0.0, 20.0), blast at 9000"), &game);
    tally.note(&note(1, "throws a grenade at (16.0, 0.0, 20.0), blast at 11000"), &game);
    tally.note(&note(2, "throws a grenade at (10.0, 0.0, 20.0), blast at 9000"), &game);
    tally.note(&note(0, "throws a grenade at (10.0, 0.0, 20.0), blast at 30000"), &game);
    game.tick = TICK_MS;
    let thinking = Some(BotNow {
        play: "Roam".to_owned(),
        grenades: GrenadeStats {
            thinks: 1,
            ..GrenadeStats::default()
        },
        ..BotNow::default()
    });
    tally.step_with(&mut game, &[thinking, None, None]);
    let (_, json) = tally.finish(&game);
    assert_eq!(json["grenade_team_same_spot"], serde_json::json!([1, 0, 1]), "{json}");
}

#[test]
pub(crate) fn the_blast_falls_off_to_its_minimum_at_ten_metres() {
    assert!((blast_damage(0.0) - blast_max()).abs() < 1e-6);
    assert!((blast_damage(blast_m()) - blast_min()).abs() < 1e-6);
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
    tally.step_with(&mut game, &[bot_now(), bot_now(), bot_now()]);
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
            tally.step_with(&mut game, &[bot_now(), bot_now()]);
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

/// A bot's alive time is counted by the nearest alive enemy (under 8 m, 8-30 m, farther, none alive) and a throw it
/// lets go by the same bins, so the bots' rate can be set beside the humans' (`rate_human.py`).
#[test]
pub(crate) fn a_bots_alive_time_and_its_throws_are_counted_by_its_nearest_enemy() {
    let (mut game, mut tally) = made_up(&[0, 1]);
    game.bodies[0].feet = Vec3::ZERO;
    game.bodies[1].feet = Vec3::new(20.0, 0.0, 0.0);
    let bot = || {
        Some(BotNow {
            play: "Roam".to_owned(),
            grenades: GrenadeStats {
                thinks: 1,
                ..GrenadeStats::default()
            },
            ..BotNow::default()
        })
    };
    // Place 0 is the bot; the enemy (place 1) is no bot, and its own time is not counted.
    for _ in 0..4 {
        game.tick += TICK_MS;
        tally.step_with(&mut game, &[bot(), None]);
    }
    // A throw at 20 m from the enemy: the 8-30 m bin.
    let note = |time: u32, text: &str| Note {
        place: 0,
        time,
        text: text.to_owned(),
    };
    tally.note(&note(game.tick, "throws a grenade at (1.0, 0.0, 2.0), blast at 5000"), &game);
    // The enemy closes to 5 m: one tick under 8 m, and a throw there; dead, none is alive: the last bin.
    game.bodies[1].feet = Vec3::new(5.0, 0.0, 0.0);
    game.tick += TICK_MS;
    tally.step_with(&mut game, &[bot(), None]);
    tally.note(&note(game.tick, "throws a grenade at (1.0, 0.0, 2.0), blast at 9000"), &game);
    game.bodies[1].alive = false;
    game.tick += TICK_MS;
    tally.step_with(&mut game, &[bot(), None]);
    let (_, json) = tally.finish(&game);
    let rate = &json["grenade_rate"];
    assert_eq!(
        rate["alive_ms_by_enemy"],
        serde_json::json!([TICK_MS, 4 * TICK_MS, 0, TICK_MS]),
        "{json}"
    );
    assert_eq!(rate["thrown_by_enemy"], serde_json::json!([1, 1, 0, 0]), "{json}");
}

#[test]
pub(crate) fn the_launch_constants_are_the_arcs() {
    assert_eq!(LAUNCH_MS, 1100);
    assert_eq!(fuse_ms(), blast_params().ttl_ms);
    assert_eq!(HELD_MS, 1400);
}

impl GrenadeCounts {
    fn read_stats(&mut self, place: Place, stats: Option<GrenadeStats>) {
        if self.reading.len() <= place {
            self.reading.resize(place + 1, None);
        }
        self.reading[place] = stats;
    }
}

impl Topic for GrenadeCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn bots(&mut self, bots: &Bots, _game: &Game) {
        self.listed = Some(bots.tracker.throw_counts());
        self.live = bots.tracker.live_grenades().to_vec();
    }

    fn read(&mut self, place: Place, bot: Option<&Bot>, _tick: u32) {
        self.read_stats(place, bot.map(|bot| bot.reflex.grenade_stats()));
        if let Some(bot) = bot {
            self.avoid.insert(place, bot.features.avoid.stats);
        }
    }

    #[cfg(test)]
    fn inject(&mut self, place: Place, now: Option<&BotNow>) {
        self.read_stats(place, now.map(|now| now.grenades));
        if let Some(now) = now {
            self.avoid.insert(place, now.avoid);
        }
    }

    /// A throw noted at its let-go, or one the bot weighed and did not make (and the throw it gave up).
    fn note(&mut self, _run: &Run, note: &Note, kind: &NoteKind<'_>, game: &Game) {
        match *kind {
            NoteKind::Thrown { aim, blast } => {
                let team = game.bodies.get(note.place).map_or(0, |b| b.team);
                self.grenade_thrown(note.place, team, aim, blast);
                if note.place < game.bodies.len() {
                    self.thrown_by_enemy[enemy_bin(game, note.place)] += 1;
                }
            }
            NoteKind::Closed { blast } => self.close.noted.push((note.place, blast)),
            NoteKind::HeldBack(why) => {
                self.grenade_refused(why);
                // A throw given up in its hold (a mate walked in) is over: the shots after it are the bot's own.
                self.grenade_given_up(note.place);
            }
            _ => {}
        }
    }

    fn body(&mut self, _run: &Run, at: &BodyAt<'_>, _rays: &mut u64) {
        let Some(stats) = self.reading.get(at.place).copied().flatten() else {
            return;
        };
        self.grenades_keys(at.place, at.tick, &at.game.bodies[at.place], at.prev);
        self.grenades_logic(at.place, &stats);
        if at.game.bodies[at.place].alive {
            self.alive_by_enemy_ms[enemy_bin(at.game, at.place)] += u64::from(TICK_MS);
        }
    }

    fn end(&mut self, run: &Run, game: &mut Game, sight: &mapdata::RayTree, rays: &mut u64) {
        let tick = game.tick;
        self.grenades_off(run, game, sight, rays, tick);
    }

    fn parts(&mut self, _run: &Run, game: &Game, _scale: &Scale, shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("grenades", self.part(game, shared))]
    }
}

impl GrenadeCounts {
    /// A throw noted at its let-go: the blast is the fuse after the press; where it lands is worked out at
    /// the launch ([`Self::grenade_launches`]).
    pub(super) fn grenade_thrown(&mut self, thrower: Place, team: u8, aim: Vec3, blast: u32) {
        let press = blast.saturating_sub(fuse_ms());
        // Teammates' throws at one spot, a throw counted once for each radius it lands within of an
        // earlier throw of another bot of its team in the last SAME_SPOT_MS (the bot's own volley is not).
        for earlier in &self.same_spot_log {
            if earlier.0 != thrower
                && earlier.1 == team
                && press.abs_diff(earlier.3) <= SAME_SPOT_MS
            {
                let d = earlier.2.distance_xz(aim);
                for (k, radius) in SAME_SPOT_M.iter().enumerate() {
                    if d < *radius {
                        self.same_spot[k] += 1;
                        break;
                    }
                }
            }
        }
        self.same_spot_log.push((thrower, team, aim, press));
        self.grenades += 1;
        self.throws.push(Throw { thrower, team, aim, press });
    }

    /// A throw is let go: whether the body had a grenade in the hand (taken from it, [`standin_grenade::Stock`]). The client throws
    /// nothing from an empty hand, so a throw that had none goes off nowhere and is counted apart (phantom). A match that does not
    /// model the pockets carries every throw and splits none.
    fn carries(&mut self, game: &mut Game, thrower: Place, team: u8) -> bool {
        let Some(body) = game.bodies.get_mut(thrower) else {
            return true;
        };
        let modelled = body.grenades.is_modelled();
        let had = body.grenades.throw();
        let by_team = &mut self.carry_by_team[usize::from(team.min(1))];
        match (modelled, had) {
            (false, _) => {}
            (true, true) => {
                self.carried_throws += 1;
                by_team[0] += 1;
            }
            (true, false) => {
                self.phantom_throws += 1;
                by_team[1] += 1;
            }
        }
        had
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
        self.refused[at] += 1;
    }

    /// A bot's grenade logic counts at this tick (`Reflex::grenade_stats`): the change since its last reading
    /// is added; a reflex that has not thought yet is not read.
    pub(super) fn grenades_logic(&mut self, place: Place, now: &GrenadeStats) {
        if now.thinks == 0 {
            return;
        }
        let logic = &mut self.logic;
        let (change, new_life) = now.since(logic.last.get(&place));
        logic.last.insert(place, *now);
        logic.lives += u32::from(new_life);
        logic.sum.add(&change);
    }

    /// The bot let go of the key and gave the throw up: no hold is open for it.
    pub(super) fn grenade_given_up(&mut self, thrower: Place) {
        self.holds.remove(&thrower);
    }

    /// A bot's grenade key at this tick: a press opens a hold of [`HELD_MS`]; in it, the thrower dying before
    /// the launch and a fire press are counted. From the stand-in's keys, not the bot's word.
    pub(super) fn grenades_keys(&mut self, place: Place, tick: u32, body: &super::super::Body, prev: &Rec) {
        let g = &mut *self;
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
    /// point, the fuse after the press. Who is inside is by the bodies' places at the launch.
    fn grenade_launches(&mut self, run: &Run, game: &mut Game, tick: u32) {
        let (due, later): (Vec<Throw>, Vec<Throw>) = std::mem::take(&mut self.throws)
            .into_iter()
            .partition(|t| tick >= t.press + LAUNCH_MS);
        self.throws = later;
        for throw in due {
            let k = run.index(throw.press + LAUNCH_MS);
            let bodies: Vec<Option<Rec>> = run.recs.iter().map(|r| r.get(k).or(r.last()).copied()).collect();
            let Some(rec) = bodies.get(throw.thrower).copied().flatten().filter(|r| r.alive) else {
                continue;
            };
            if !self.carries(game, throw.thrower, throw.team) {
                continue;
            }
            let up = eye_height(rec.keys & key::CROUCH != 0, rec.speed, rec.pitch);
            let eye = rec.feet + Vec3::new(0.0, up, 0.0);
            let Some(landing) =
                arc::Flight::thrown(eye, rec.yaw, rec.pitch).landing(&arc::Mesh(&game.world.sight))
            else {
                continue;
            };
            if throw.aim.distance_xz(rec.feet) >= SHORT_M && landing.contact.point.distance_xz(rec.feet) < SHORT_M {
                self.short += 1;
            }
            let inside = bodies
                .iter()
                .map(|r| {
                    r.is_some_and(|r| {
                        let chest = r.feet + Vec3::new(0.0, if r.keys & key::CROUCH != 0 { 0.8 } else { 1.3 }, 0.0);
                        r.alive && (chest - landing.rest).length() <= blast_m()
                    })
                })
                .collect();
            self.blasts.push(Blast {
                thrower: throw.thrower,
                aim: throw.aim,
                launch: (
                    rec.yaw,
                    rec.pitch,
                    eye,
                    landing.contact.point,
                    [0.03f32, 0.05].map(|up| {
                        arc::Flight::thrown(eye, rec.yaw, rec.pitch + up)
                            .landing(&arc::Mesh(&game.world.sight))
                            .map_or(Vec3::ZERO, |l| l.contact.point)
                    }),
                ),
                at: landing.rest,
                time: throw.press + fuse_ms(),
                inside,
            });
        }
    }

    /// A blast against the server's list: the entry of the same thrower and press, and where it put the rest
    /// point (only in a bench run, which reads the list).
    fn live_blast(&mut self, blast: &Blast) {
        if self.listed.is_none() {
            return;
        }
        let press = blast.time.saturating_sub(fuse_ms());
        let found = self
            .live
            .iter()
            .find(|g| g.by == blast.thrower && g.press.abs_diff(press) <= 100);
        let live = &mut self.live_blasts;
        match found.map(|g| g.phase) {
            None => live.missing += 1,
            Some(Phase::Pressed) => live.pending += 1,
            Some(Phase::Flying { rest, .. }) => {
                live.matched += 1;
                let d = rest.distance_xz(blast.at);
                live.err[usize::from(d > 1.0) + usize::from(d > 2.0) + usize::from(d > 4.0)] += 1;
            }
        }
    }

    /// The grenades going off: who is inside, who walked in, the damage it would do (the stand-in has none).
    pub(super) fn grenades_off(&mut self, run: &Run, game: &mut Game, sight: &mapdata::RayTree, rays: &mut u64, tick: u32) {
        self.grenade_launches(run, game, tick);
        // The grenades going off: who is inside, who walked in, the damage
        // it would do (the stand-in has none).
        let mut gone = Vec::new();
        self.blasts.retain(|b| {
            let off = tick >= b.time;
            if off {
                gone.push(b.clone());
            }
            !off
        });
        for blast in gone {
            self.gone_log.push((blast.time, blast.thrower, blast.at, blast.aim, blast.launch));
            let before = (self.blast_dmg, self.blast_kills, self.blast_in);
            self.live_blast(&blast);
            let Some(team) = game.bodies.get(blast.thrower).map(|b| b.team) else {
                continue;
            };
            for (place, &inside) in blast.inside.iter().enumerate() {
                if let (true, Some(body)) = (inside, game.bodies.get(place)) {
                    let who = if place == blast.thrower {
                        0
                    } else if body.team == team {
                        1
                    } else {
                        2
                    };
                    self.in_launch[who] += 1;
                }
            }
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
                    self.blast_in[who] += 1;
                    self.walked_in[who] +=
                        u32::from(!blast.inside.get(hurt.place).copied().unwrap_or(false));
                    self.blast_dmg[who] += hurt.body;
                    self.blast_kills[who] += u32::from(hurt.killed);
                }
                self.close_blast(game, &blast, before);
                continue;
            }
            for (place, body) in game.bodies.iter().enumerate() {
                let d = (Game::chest(body) - blast.at).length();
                if !body.alive || d > blast_m() {
                    continue;
                }
                let who = if place == blast.thrower {
                    0
                } else if body.team == team {
                    1
                } else {
                    2
                };
                self.blast_in[who] += 1;
                self.walked_in[who] += u32::from(!blast.inside.get(place).copied().unwrap_or(false));
                *rays += 1;
                if !sight.visible(blast.at + Vec3::new(0.0, 0.3, 0.0), Game::chest(body)) {
                    continue;
                }
                let dmg = blast_damage(d);
                self.blast_dmg[who] += dmg;
                self.blast_kills[who] += u32::from(dmg >= body.health);
            }
            self.close_blast(game, &blast, before);
        }
    }

    /// A blast of a throw the bot decided as a close one: what it did to its own side, from the change in the
    /// totals since `before` (damage, kills, bodies inside the radius, by [thrower, mates, enemies]).
    fn close_blast(&mut self, game: &Game, blast: &Blast, before: ([f32; 3], [u32; 3], [u32; 3])) {
        let Some(at) = self.close.noted.iter().position(|&(thrower, time)| thrower == blast.thrower && time == blast.time) else {
            return;
        };
        self.close.noted.swap_remove(at);
        let Some(team) = game.bodies.get(blast.thrower).map(|b| b.team) else {
            return;
        };
        let dist = |place: Place| game.bodies.get(place).map_or(f32::INFINITY, |body| (Game::chest(body) - blast.at).length());
        let mates = game
            .bodies
            .iter()
            .enumerate()
            .filter(|(place, body)| *place != blast.thrower && body.alive && body.team == team)
            .map(|(place, _)| dist(place))
            .fold(f32::INFINITY, f32::min);
        let close = &mut self.close;
        close.gone += 1;
        let delta = |now: f32, was: f32| now - was;
        let dmg = [0, 1, 2].map(|k| delta(self.blast_dmg[k], before.0[k]));
        for k in 0..3 {
            close.dmg[k] += dmg[k];
            close.kills[k] += self.blast_kills[k] - before.1[k];
            close.inside[k] += self.blast_in[k] - before.2[k];
        }
        let thrower = dist(blast.thrower);
        close.least_m = if close.gone == 1 {
            [thrower, mates]
        } else {
            [close.least_m[0].min(thrower), close.least_m[1].min(mates)]
        };
        if dmg[0] > 0.0 || dmg[1] > 0.0 {
            close.leaks += 1;
            if close.cases.len() < 12 {
                close.cases.push([thrower, mates, dmg[0], dmg[1]]);
            }
        }
    }

    /// The grenades counts at the end: their keys of the `owner` object and their lines of the owner-test text.
    fn part(&self, game: &Game, shared: &Shared) -> Part {
        let modelled = game.bodies.iter().any(|body| body.grenades.is_modelled());
        // The fight topic's count of fire at a hidden target in a throw's hold and after one.
        let hidden_throw: [u32; 2] = shared
            .get("fire_hidden_throw")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let mut json = serde_json::json!({
            "grenades": self.grenades,
            "grenade_damage_applied": game.damage.blasts_hurt(),
            "grenade_self_dmg": (100.0 * self.blast_dmg[0]).round() / 100.0,
            "grenade_mate_dmg": (100.0 * self.blast_dmg[1]).round() / 100.0,
            "grenade_enemy_dmg": (100.0 * self.blast_dmg[2]).round() / 100.0,
            "grenade_kills": self.blast_kills,
            "grenade_in_radius": self.blast_in,
            "grenade_walk_ins": self.walked_in,
            "grenade_short": self.short,
            "grenade_died_throwing": self.died_throwing,
            "grenade_fired_after_held": self.fired_after_held,
            "grenade_refused": self.refused,
            "team_kills_gun": game.team_kills,
            "team_hits": game.team_hits,
        });
        let logic = &self.logic;
        // Only where bots thought: the made-up match has none, and its golden stays as it was.
        if logic.lives > 0 {
            let s = &logic.sum;
            json["grenade_team_same_spot"] = serde_json::json!(self.same_spot);
            json["grenade_rate"] = serde_json::json!({
                "alive_ms_by_enemy": self.alive_by_enemy_ms,
                "thrown_by_enemy": self.thrown_by_enemy,
            });
            json["grenade_logic"] = serde_json::json!({
                "bot_lives": logic.lives,
                "claimed": s.claimed,
                "rolls": s.rolls,
                "chance_no": s.chance_no,
                "places": s.places,
                "weighed": s.weighed,
                "refused": s.refused,
                "passed": s.passed,
                "given_up": s.given_up,
                "nth": s.nth,
                "timeouts": s.timeouts,
                "rechecks": s.rechecks,
                "thrown_from": s.thrown_from,
                "pushes_skipped": s.pushes_skipped,
                "volley": s.volley,
                "near": s.near,
                "fire_hidden_throw": hidden_throw,
            });
        }
        let counted = logic.sum.close != [0; 7] || self.close.gone > 0;
        if counted {
            let close = &self.close;
            json["grenade_close"] = serde_json::json!({
                "stats": logic.sum.close,
                "blasts": close.gone,
                "leaks": close.leaks,
                "dmg": close.dmg.map(|d| (100.0 * d).round() / 100.0),
                "kills": close.kills,
                "inside": close.inside,
                "least_m": close.least_m.map(|d| (10.0 * d).round() / 10.0),
                "cases": close.cases.iter().map(|c| c.map(|d| (10.0 * d).round() / 10.0)).collect::<Vec<_>>(),
            });
        }
        if let Some(listed) = self.listed {
            let live = &self.live_blasts;
            json["grenade_live"] = serde_json::json!({
                "frag": listed.frag,
                "smoke": listed.smoke,
                "no_pockets": listed.no_pockets,
                "launched": listed.launched,
                "unplaced": listed.unplaced,
                "died_throwing": listed.died_throwing,
                "blasts_matched": live.matched,
                "blasts_pending": live.pending,
                "blasts_unlisted": live.missing,
                "rest_error_m": live.err,
                "inside_at_launch": self.in_launch,
            });
            let mut avoid = AvoidStats::default();
            for stats in self.avoid.values() {
                avoid.add(stats);
            }
            json["grenade_avoid"] = serde_json::json!({
                "noticed": avoid.noticed,
                "stepped": avoid.stepped,
                "held": avoid.held,
                "out_at_act": avoid.out_at_act,
                "no_way": avoid.no_way,
                "retried": avoid.retried,
                "escaped": avoid.escaped,
                "caught": avoid.caught,
                "react_ms": avoid.react,
            });
        }
        if modelled {
            // The throws the stand-in blasts: only a grenade the thrower's kit had this life (an-gear-moves 3.1: the reflex
            // throws twice a life whatever the pockets hold, and ~80 % of the logged throws had none).
            json["grenade_carry"] = serde_json::json!({
                "carried_throws": self.carried_throws,
                "phantom_throws": self.phantom_throws,
                "phantom_share": Scale::rate(self.phantom_throws, self.carried_throws + self.phantom_throws),
                "by_team": {
                    "0": { "carried_throws": self.carry_by_team[0][0], "phantom_throws": self.carry_by_team[0][1] },
                    "1": { "carried_throws": self.carry_by_team[1][0], "phantom_throws": self.carry_by_team[1][1] },
                },
            });
        }
        let mut text = format!(
            "owner-test other: grenades {} ({}) {} self {:.2} mates {:.2} enemies {:.2}, {} {:?}, inside at the blast {:?} walked in {:?}; short {} (aimed beyond {SHORT_M} m, first contact within), thrower died in the press to launch {}, fire pressed in the hold {}, refused (thrower's blast, wall, teammate, other) {:?}; team kills by gunfire {}, team hits {}",
            self.grenades,
            if !game.damage.blasts_hurt() { "no damage in the stand-in" } else { "the client's blast" },
            if !game.damage.blasts_hurt() { "would damage" } else { "damaged" },
            self.blast_dmg[0],
            self.blast_dmg[1],
            self.blast_dmg[2],
            if !game.damage.blasts_hurt() { "would kill" } else { "killed" },
            self.blast_kills,
            self.blast_in,
            self.walked_in,
            self.short,
            self.died_throwing,
            self.fired_after_held,
            self.refused,
            game.team_kills,
            game.team_hits,
        );
        if modelled {
            text.push_str(&format!(
                "; carried: throws with a grenade in the kit {}, phantom throws with none {} (not blasted)",
                self.carried_throws, self.phantom_throws
            ));
        }
        if logic.lives > 0 {
            let s = &logic.sum;
            text.push_str(&format!(
                "; grenade logic over {} bot lives: rolls {} (chance said no {}), places by [held, camp, lost, heard, held spot] {:?}, weighed {:?}, refused by [thrower's blast, wall, teammate, out of reach, nowhere, off the place] {:?}, passed by arc [flat, pitch, yaw] {:?}, given up in the hold {:?}, throws as the 1st/2nd/3rd+ {:?}, view timeouts [no target, target] x [flat, pitch, yaw] {:?}, rechecks searched again [found, none] {:?}, thrown by source {:?}, pushes left out {}, alive minutes by the nearest enemy [<8 m, 8-30 m, farther, none] {:.1?}, thrown by the same {:?}, volley [windows, rolls, chance said no, second grenades] {:?}, near floor [weighed again, way out found, thrown] {:?}, held spots by think [with a spot, sensed lately, in range, outside his lanes, no mate or other place, unclaimed, a frag in the kit] {:?}, stopped before the roll with a sensed spot [throw under way, target in sight, all thrown, lockout] {:?}, throws given up for a dropped spot [before the key, in the hold] {:?}, fire at a hidden target in a throw's hold / within 3 s after one {:?}",
                logic.lives, s.rolls, s.chance_no, s.places, s.weighed, s.refused, s.passed, s.given_up, s.nth, s.timeouts, s.rechecks, s.thrown_from, s.pushes_skipped, self.alive_by_enemy_ms.map(|ms| ms as f64 / 60_000.0), self.thrown_by_enemy, s.volley, s.near, s.held_funnel, s.held_busy, s.held_dropped, hidden_throw,
            ));
        }
        if counted {
            let close = &self.close;
            text.push_str(&format!(
                "; close throws [rolls with a close place, weighed, refused by the arc or a mate's blast, refused for a mate not seen lately, refused for no way out, decided, let go] {:?}, blasts {} (leaks: the thrower or a mate hurt {}), damage [thrower, mates, enemies] {:.2?}, killed {:?}, inside {:?}, nearest [thrower, mate] to the blast {:.1?} m, leak cases [thrower m, nearest mate m, thrower dmg, mates dmg] {:.2?}",
                logic.sum.close, close.gone, close.leaks, close.dmg, close.kills, close.inside, close.least_m, close.cases,
            ));
        }
        if let Some(listed) = self.listed {
            let live = &self.live_blasts;
            text.push_str(&format!(
                "; live grenade list: frags {} (smokes left out {}, human throw keys with no pockets known {}), launched with a landing {}, unplaced {}, thrower dead at the launch {}; at the blasts: listed with a landing {}, still in the hand {}, not listed {}, rest point off the tally's by [<=1, <=2, <=4, more] m {:?}, inside the radius at the launch [thrower, mates, enemies] {:?}",
                listed.frag,
                listed.smoke,
                listed.no_pockets,
                listed.launched,
                listed.unplaced,
                listed.died_throwing,
                live.matched,
                live.pending,
                live.missing,
                live.err,
                self.in_launch,
            ));
            let mut avoid = AvoidStats::default();
            for stats in self.avoid.values() {
                avoid.add(stats);
            }
            text.push_str(&format!(
                "; avoiding live grenades by [told, saw, heard]: noticed {:?}, stepped out {:?}, held at the edge {:?}, already out at the act {:?}, no floor in reach {:?}, started again {:?}, at the blast out {:?} caught {:?}, reaction to the step (<=0.5, 1, 1.5, 2 s, more) {:?}",
                avoid.noticed,
                avoid.stepped,
                avoid.held,
                avoid.out_at_act,
                avoid.no_way,
                avoid.retried,
                avoid.escaped,
                avoid.caught,
                avoid.react,
            ));
        }
        Part::new(json, text)
    }
}
