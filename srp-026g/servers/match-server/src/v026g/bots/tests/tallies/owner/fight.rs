use super::*;

/// The fight counts: counted as the match runs, kept for [`OwnerTally::finish`].
#[derive(Default)]
pub(super) struct FightCounts {
    /// Bot state of the last tick: dry, the last reload's (at, until).
    pub(super) dry: Vec<bool>,
    pub(super) last_reload: Vec<Option<(u32, u32)>>,
    pub(super) reloads: Vec<Reload>,
    pub(super) reloads_n: u32,
    pub(super) reloads_in_line: u32,
    pub(super) reloads_in_line_died: u32,
    pub(super) dry_ticks: u64,
    pub(super) reload_ticks: u64,
    pub(super) deaths_dry: u32,
    pub(super) deaths_reloading: u32,
    /// Fire ticks by weapon class and band; hits by the same.
    pub(super) fire: BTreeMap<&'static str, [u32; 5]>,
    pub(super) hits_band: BTreeMap<&'static str, [u32; 5]>,
    /// By weapon class with a players' range ([`CLASS_RANGE_M`]): fire
    /// ticks with an enemy in the view and those of them past the range;
    /// hits and those from past it; the ticks the range alone held the
    /// shot.
    pub(super) fire_range: BTreeMap<&'static str, [u32; 2]>,
    /// The same by the distance of the bot's own target (what its range gate
    /// cuts), where the enemy nearest the view may be another, farther one.
    pub(super) fire_range_target: BTreeMap<&'static str, [u32; 2]>,
    /// By place: the reflex tick whose held shot was counted last.
    pub(super) held_at: Vec<u32>,
    pub(super) hits_range: BTreeMap<&'static str, [u32; 2]>,
    pub(super) range_held: BTreeMap<&'static str, u32>,
    /// Fire ticks and hits by the shooter's movement ([`MOVES`]).
    pub(super) fire_move: [u32; 5],
    pub(super) hits_move: [u32; 5],
    /// Fire ticks with an enemy target: of them, those with no line from
    /// the eye to his chest or head; with none.
    pub(super) fire_on_target: u32,
    pub(super) fire_hidden: u32,
    /// The bot's grenade state at the tick being counted: its view held for a
    /// throw, and ms since its last throw (`fire_hidden_throw`).
    pub(super) throw_state: (bool, Option<u32>),
    /// Fire at a target out of sight, in a throw's hold, and within 3 s after
    /// a throw (the grenade logic's own reading of the +5.9 % of look 2).
    pub(super) fire_hidden_throw: [u32; 2],
    pub(super) fire_no_target: u32,
    /// By place: the last tick's target, and the enemies who hit it (who,
    /// when).
    pub(super) last_target: Vec<Option<Place>>,
    pub(super) hurt_by: Vec<Vec<(Place, u32)>>,
    pub(super) switches: u32,
    pub(super) switches_off_hurter: u32,
    pub(super) hits: Vec<(u32, Place, Place)>,
}

impl FightCounts {
    pub(super) fn new(n: usize) -> Self {
        Self {
            dry: vec![false; n],
            last_reload: vec![None; n],
            last_target: vec![None; n],
            held_at: vec![0; n],
            hurt_by: vec![Vec::new(); n],
            ..Self::default()
        }
    }
}

/// Calm (`gait.py`): no aim or fire key held and no living enemy within
/// this, m.
pub(crate) const CALM_ENEMY_M: f32 = 30.0;

/// Reloads in an enemy's line (`behaviour.py` D): within this, m; and a
/// death this soon after the reload's start counts, ms.
pub(crate) const RELOAD_LINE_M: f32 = 40.0;

pub(crate) const RELOAD_DIED_MS: u32 = 4_000;

/// A death this soon after a reload's end counts as while reloading, ms.
pub(crate) const RELOAD_AFTER_MS: u32 = 1_000;

/// Fire by range (`fire_stats.py`): the enemy nearest the view within this
/// many degrees of yaw, at 2-80 m; the bands' upper bounds, m.
pub(crate) const FIRE_ON_DEG: f32 = 10.0;

pub(crate) const FIRE_BANDS: [f32; 5] = [10.0, 20.0, 35.0, 50.0, 80.0];

pub(crate) const FIRE_MIN_M: f32 = 2.0;

/// Fire past the weapon class's range (X1, `x1_hit_by_distance.py`,
/// `range_cap`): the enemy nearest the view within [`FIRE_ON_DEG`], at
/// [`FIRE_MIN_M`] to this many metres (past the bolt's range and the
/// ceilings of `choose`), is the fire's distance.
pub(crate) const FIRE_FAR_M: f32 = 130.0;

/// The players' range by weapon class, m: the p95 of the distance of the
/// hit runs of the best players of the 402 live matches with 12 or more
/// people (`player_bots_weapons.md`). The yardstick of the counts, whatever
/// the bots' tuning is (`CombatTuning::range_floor_*_m` is that tuning);
/// the pistols and machine guns have none.
pub(crate) const CLASS_RANGE_M: [(&str, f32); 5] = [
    ("rifle", 65.0),
    ("smg", 58.0),
    ("shotgun", 49.0),
    ("marksman", 74.0),
    ("bolt", 105.0),
];

/// The players' range of a weapon class ([`CLASS_RANGE_M`]).
pub(crate) fn class_range_m(class: &str) -> Option<f32> {
    CLASS_RANGE_M
        .iter()
        .find(|(name, _)| *name == class)
        .map(|&(_, range)| range)
}

/// Turned on the shooter (`turn_hit.py`): hits this far apart start a
/// case, ms; from more than this off the view, degrees; turned when the
/// view is within this of him, degrees, within the time, ms.
pub(crate) const HIT_APART_MS: u32 = 3_000;

pub(crate) const HIT_SIDE_DEG: f32 = 60.0;

pub(crate) const TURNED_DEG: f32 = 15.0;

pub(crate) const TURN_MS: u32 = 3_000;

/// The shooter's movement at a fire tick or a hit: in the air (a jump
/// under way), still (under [`STILL_MPS`] or no walk key), sprinting
/// (forward and sprint), strafing (a side key, no forward or back),
/// walking.
pub(crate) const MOVES: [&str; 5] = ["still", "walk", "sprint", "strafe", "air"];

pub(crate) const STILL_MPS: f32 = 0.3;

/// Target switches: an enemy who hit the bot this recently is hurting it,
/// ms.
pub(crate) const HURT_MS: u32 = 3_000;

/// A reload under way: its place, start and end, an enemy's line seen
/// during it, the bot died within [`RELOAD_DIED_MS`].
#[derive(Clone, Copy, Debug)]
pub(crate) struct Reload {
    pub(crate) place: Place,
    pub(crate) at: u32,
    pub(crate) until: u32,
    pub(crate) in_line: bool,
}

/// How far a view at `yaw` from `from` is off the way to `to`, degrees.
pub(crate) fn off_deg(yaw: f32, from: Vec3, to: Vec3) -> f32 {
    wrap(yaw - yaw_of([to.x - from.x, to.z - from.z]))
        .abs()
        .to_degrees()
}

/// The range band of `d` m ([`FIRE_BANDS`]; past the last: the last).
pub(crate) fn band_of(d: f32) -> usize {
    FIRE_BANDS.iter().position(|&b| d <= b).unwrap_or(4)
}

/// The body's movement at a record ([`MOVES`]).
pub(crate) fn move_of(r: &Rec) -> usize {
    if r.air {
        4
    } else if r.speed < STILL_MPS || r.keys & WALK_KEYS == 0 {
        0
    } else if r.keys & key::SPRINT != 0 && r.keys & key::FORWARD != 0 {
        2
    } else if r.keys & (key::FORWARD | key::BACK) == 0 {
        3
    } else {
        1
    }
}

/// Whether some of `him` shows from `me`'s eye within `reach` m (his chest
/// or his head), the rays cast added to `rays`.
pub(crate) fn in_sight(
    sight: &mapdata::RayTree,
    me: &super::super::Body,
    him: &super::super::Body,
    reach: f32,
    rays: &mut u64,
) -> bool {
    let eye = Game::eye(me);
    if (Game::chest(him) - eye).length() > reach {
        return false;
    }
    *rays += 1;
    if sight.visible(eye, Game::chest(him)) {
        return true;
    }
    *rays += 1;
    sight.visible(eye, Game::eye(him))
}

impl OwnerTally {
    /// A reload over (and the 4 s after its start): in a line, died.
    pub(crate) fn reload_done(&mut self, r: &Reload) {
        if !r.in_line {
            return;
        }
        self.fight.reloads_in_line += 1;
        let recs = &self.recs[r.place];
        let i0 = self.index(r.at);
        let i1 = self.index(r.at + RELOAD_DIED_MS).min(recs.len().saturating_sub(1));
        let died = (i0.max(1)..=i1).any(|i| recs[i - 1].alive && !recs[i].alive);
        self.fight.reloads_in_line_died += u32::from(died);
    }
}

#[test]
pub(crate) fn a_bot_hit_from_behind_is_counted_turned_or_not() {
    // Bot 0 hit from behind turns within 2 s; bot 2 hit from behind
    // never turns; bot 4 hit from ahead (the near miss) is no side hit.
    let (mut game, mut tally) = made_up(&[0, 1, 0, 1, 0, 1]);
    play(&mut game, &mut tally, 120, |k, game| {
        put(game, 0, 10.0, 20.0, 0);
        put(game, 1, 10.0, 10.0, 0);
        put(game, 2, 20.0, 20.0, 0);
        put(game, 3, 20.0, 10.0, 0);
        put(game, 4, 30.0, 10.0, 0);
        put(game, 5, 30.0, 20.0, 0);
        // Facing +z (yaw 0): 1 and 3 are behind, 5 ahead.
        for p in [0, 2, 4] {
            game.bodies[p].yaw = 0.0;
        }
        if k >= 40 {
            game.bodies[0].yaw = std::f32::consts::PI;
        }
        if k == 20 {
            game.hit_log.extend([(game.tick, 1, 0), (game.tick, 3, 2), (game.tick, 5, 4)]);
        }
        vec![bot_now(); 6]
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["hits_first"], 3);
    assert_eq!(json["hits_side"], 2, "{json}");
    assert_eq!(json["turned_3s"], 1);
}

#[test]
pub(crate) fn fire_and_hits_count_by_the_shooters_movement_and_range() {
    let (mut game, mut tally) = made_up(&[0, 1, 0, 1]);
    play(&mut game, &mut tally, 10, |k, game| {
        // Bot 0 fires standing at an enemy 15 m ahead and hits him once;
        // bot 2 (the near miss) fires sprinting and hits nothing.
        put(game, 0, 10.0, 10.0, key::FIRE);
        put(game, 1, 10.0, 25.0, 0);
        put(game, 2, 20.0, 10.0, key::FIRE | key::FORWARD | key::SPRINT);
        game.bodies[2].step_m = 0.3;
        put(game, 3, 20.0, 25.0, 0);
        game.bodies[0].yaw = 0.0;
        game.bodies[2].yaw = 0.0;
        if k == 5 {
            game.hit_log.push((game.tick, 0, 1));
        }
        vec![bot_now(); 4]
    });
    let class = game.bodies[0].gun.class();
    let (_, json) = tally.finish(&game);
    assert_eq!(json["fire_by_move"]["still"], 10, "{json}");
    assert_eq!(json["fire_by_move"]["sprint"], 10);
    assert_eq!(json["hits_by_move"]["still"], 1);
    assert_eq!(json["hits_by_move"]["sprint"], 0);
    assert_eq!(json["hits_by_class"][class][1], 1);
}

#[test]
pub(crate) fn fire_at_a_target_behind_a_wall_is_counted_and_in_line_is_not() {
    // The soak field's wall stands at x = 40 for z < 25.
    let (mut game, mut tally) = made_up(&[0, 1, 0, 1]);
    play(&mut game, &mut tally, 10, |_, game| {
        put(game, 0, 30.0, 10.0, key::FIRE);
        put(game, 1, 50.0, 10.0, 0);
        put(game, 2, 30.0, 32.0, key::FIRE);
        put(game, 3, 50.0, 32.0, 0);
        game.bodies[0].target = Some(1);
        game.bodies[2].target = Some(3);
        vec![bot_now(); 4]
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["fire_on_target"], 20, "{json}");
    assert_eq!(json["fire_hidden"], 10);
}

#[test]
pub(crate) fn a_target_switch_away_while_the_hurter_is_in_line_is_counted() {
    let (mut game, mut tally) = made_up(&[0, 1, 1, 1, 0]);
    play(&mut game, &mut tally, 30, |k, game| {
        put(game, 0, 10.0, 30.0, 0);
        put(game, 4, 10.0, 35.0, 0);
        put(game, 1, 30.0, 30.0, 0);
        put(game, 2, 30.0, 35.0, 0);
        put(game, 3, 30.0, 38.0, 0);
        if k == 5 {
            game.hit_log.extend([(game.tick, 1, 0), (game.tick, 1, 4)]);
        }
        // Bot 0 goes from 2 to 3 while 1 hurts it; bot 4 (the near miss)
        // from 2 to 1, the one hurting it.
        game.bodies[0].target = Some(if k < 10 { 2 } else { 3 });
        game.bodies[4].target = Some(if k < 10 { 2 } else { 1 });
        vec![bot_now(); 5]
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["target_switches"], 2, "{json}");
    assert_eq!(json["switches_off_hurter"], 1);
}

#[test]
pub(crate) fn deaths_dry_or_reloading_are_counted() {
    let (mut game, mut tally) = made_up(&[0, 0, 0, 0]);
    play(&mut game, &mut tally, 100, |k, game| {
        for p in 0..4 {
            put(game, p, 10.0 + 5.0 * p as f32, 10.0, 0);
        }
        // Bot 0 dry, dies at 40; bot 1 (near miss) not dry, dies at 40.
        // Bot 2 dies 0.5 s after its reload; bot 3 (near miss) 2 s after.
        let dead = |at: usize| k >= at;
        game.bodies[0].alive = !dead(40);
        game.bodies[1].alive = !dead(40);
        game.bodies[2].alive = !dead(70);
        game.bodies[3].alive = !dead(90);
        let mut nows = vec![bot_now(); 4];
        nows[0].as_mut().unwrap().dry = true;
        for p in [2, 3] {
            nows[p].as_mut().unwrap().reload = Some((20 * TICK_MS, 60 * TICK_MS));
        }
        nows
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["deaths_dry"], 1, "{json}");
    assert_eq!(json["deaths_reloading"], 1);
    assert_eq!(json["reloads"], 2);
    assert_eq!(json["dry_s"], 2.0);
}

#[test]
pub(crate) fn a_reload_in_an_enemys_line_is_counted_and_one_behind_a_wall_is_not() {
    // The soak field's wall stands at x = 40 for z < 25.
    let (mut game, mut tally) = made_up(&[0, 1, 0]);
    play(&mut game, &mut tally, 160, |k, game| {
        put(game, 0, 10.0, 10.0, 0);
        put(game, 1, 30.0, 10.0, 0);
        put(game, 2, 60.0, 10.0, 0);
        // Bot 0 dies 3 s into its reload.
        game.bodies[0].alive = k < 60;
        let mut nows = vec![bot_now(); 3];
        for p in [0, 2] {
            nows[p].as_mut().unwrap().reload = Some((TICK_MS, 40 * TICK_MS));
        }
        nows
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["reloads"], 2);
    assert_eq!(json["reloads_in_line"], 1, "{json}");
    assert_eq!(json["reloads_in_line_died"], 1);
}

#[test]
pub(crate) fn fire_counts_by_the_range_of_the_enemy_in_view() {
    let (mut game, mut tally) = made_up(&[0, 1, 1]);
    play(&mut game, &mut tally, 10, |_, game| {
        // Bot 0 fires facing +z: the enemy ahead 28 m; one to its side.
        put(game, 0, 5.0, 5.0, key::FIRE);
        game.bodies[0].yaw = 0.0;
        put(game, 1, 5.0, 33.0, 0);
        put(game, 2, 25.0, 5.0, 0);
        vec![bot_now(); 3]
    });
    let class = game.bodies[0].gun.class();
    let (_, json) = tally.finish(&game);
    assert_eq!(json["fire_by_class"][class], serde_json::json!([0, 0, 10, 0, 0]), "{json}");
}

#[test]
pub(crate) fn fire_past_the_class_range_is_counted_with_its_hits_and_the_shots_the_range_held() {
    let (mut game, mut tally) = made_up(&[0, 1, 0, 1]);
    let class = game.bodies[0].gun.class();
    let range = class_range_m(class).expect("the stand-in's weapon has a players' range");
    play(&mut game, &mut tally, 10, |k, game| {
        // Bot 0 fires facing +z at an enemy 5 m inside the class's range,
        // bot 2 at one 5 m past it; one hit each; bot 2's shot is held for
        // the range alone on 3 of the ticks.
        put(game, 0, 5.0, 5.0, key::FIRE);
        put(game, 1, 5.0, 5.0 + range - 5.0, 0);
        put(game, 2, 25.0, 5.0, key::FIRE);
        put(game, 3, 25.0, 5.0 + range + 5.0, 0);
        game.bodies[0].yaw = 0.0;
        game.bodies[2].yaw = 0.0;
        game.bodies[0].target = Some(1);
        game.bodies[2].target = Some(3);
        if k == 5 {
            game.hit_log.extend([(game.tick, 0, 1), (game.tick, 2, 3)]);
        }
        let mut nows = vec![bot_now(); 4];
        nows[2].as_mut().unwrap().range_held = (k < 3).then_some(game.tick);
        nows
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["fire_past_class_range"][class], serde_json::json!([20, 10]), "{json}");
    assert_eq!(json["fire_past_class_range_at_target"][class], serde_json::json!([20, 10]));
    assert_eq!(json["hits_past_class_range"][class], serde_json::json!([2, 1]));
    assert_eq!(json["fire_past_class_range_pct"], 50.0);
    assert_eq!(json["hit_rate_in_class_range"], 0.1);
    assert_eq!(json["hit_rate_past_class_range"], 0.1);
    assert_eq!(json["hit_rate_by_class_range"][class], serde_json::json!([0.1, 0.1]));
    assert_eq!(json["fire_held_by_class_range"][class], 3);
}

/// The counts' yardstick is the numbers the tuning defaults to; a bench
/// variant of the tuning (the shotgun at 35 m) leaves the yardstick alone.
#[test]
pub(crate) fn the_counts_class_ranges_are_the_tunings_defaults() {
    let tuning = crate::v026g::bots::reflex::CombatTuning::default();
    for (class, range) in CLASS_RANGE_M {
        assert_eq!(tuning.range_floor_m(class), range, "{class}");
    }
    for class in ["pistol", "mg"] {
        assert_eq!(class_range_m(class), None);
        assert!(tuning.range_floor_m(class) >= crate::v026g::bots::reflex::NO_FLOOR_M);
    }
}

/// The living enemy of the body at `place` (of `team`) that `t` is, if it is one.
pub(super) fn enemy_of(game: &Game, place: Place, team: u8, t: Place) -> Option<&super::super::Body> {
    game.bodies
        .get(t)
        .filter(|b| t != place && b.alive && b.team != team)
}

impl OwnerTally {
    /// The game's hits this tick are kept, and the enemies who hurt each victim lately (for the target switches).
    pub(super) fn fight_hits_taken(&mut self, game: &Game, tick: u32) {
        self.fight.hits.extend(game.hit_log.iter().copied());
        for &(t, shooter, victim) in &game.hit_log {
            if let Some(hurt) = self.fight.hurt_by.get_mut(victim) {
                hurt.retain(|&(who, _)| who != shooter);
                hurt.push((shooter, t));
            }
        }
        for hurt in &mut self.fight.hurt_by {
            hurt.retain(|&(_, t)| tick <= t + HURT_MS);
        }
    }

    /// A death: dry or reloading at it.
    pub(super) fn fight_death(&mut self, place: Place, tick: u32, prev: &Rec, body: &super::super::Body) {
        // A death: dry or reloading at it.
        if prev.alive && !body.alive {
            self.fight.deaths_dry += u32::from(self.fight.dry[place]);
            self.fight.deaths_reloading += u32::from(self.fight.last_reload[place].is_some_and(
                |(at, until)| at <= tick && tick <= until + RELOAD_AFTER_MS,
            ));
        }
    }

    /// The bot's ammunition and reloads.
    pub(super) fn fight_ammo(&mut self, place: Place, tick: u32, body: &super::super::Body, now: &BotNow) {
        // The bot's ammunition and reloads.
        self.fight.dry[place] = body.alive && now.dry;
        if self.fight.dry[place] {
            self.fight.dry_ticks += 1;
        }
        if let Some((at, until)) = now.reload {
            if self.fight.last_reload[place].is_none_or(|(was, _)| was != at) {
                self.fight.reloads_n += 1;
                self.fight.reloads.push(Reload {
                    place,
                    at,
                    until,
                    in_line: false,
                });
            }
            self.fight.last_reload[place] = Some((at, until));
            if body.alive && at <= tick && tick < until {
                self.fight.reload_ticks += 1;
            }
        }
    }

    /// Fire this tick: by movement, at a target in line or not, by range.
    pub(super) fn fight_fire(&mut self, game: &Game, place: Place, body: &super::super::Body, rec: &Rec, sight: &mapdata::RayTree, rays: &mut u64) {
        if body.alive && body.keys & key::FIRE != 0 {
            self.fight.fire_move[move_of(rec)] += 1;
            // At the target: with a line to him or not.
            match body.target.and_then(|t| enemy_of(game, place, body.team, t)) {
                Some(him) => {
                    self.fight.fire_on_target += 1;
                    if !in_sight(sight, body, him, f32::INFINITY, rays) {
                        self.fight.fire_hidden += 1;
                        let (held, ago) = self.fight.throw_state;
                        if held {
                            self.fight.fire_hidden_throw[0] += 1;
                        } else if ago.is_some_and(|ago| ago <= 3_000) {
                            self.fight.fire_hidden_throw[1] += 1;
                        }
                    }
                }
                None => self.fight.fire_no_target += 1,
            }
            // By range: the enemy nearest the view.
            let in_view = |reach: f32| {
                game.bodies
                    .iter()
                    .filter(|b| b.alive && b.team != body.team)
                    .filter_map(|b| {
                        let d = b.feet.distance_xz(body.feet);
                        let off = off_deg(body.yaw, body.feet, b.feet);
                        ((FIRE_MIN_M..=reach).contains(&d) && off <= FIRE_ON_DEG)
                            .then_some((off, d))
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0))
            };
            if let Some((_, d)) = in_view(FIRE_BANDS[4]) {
                self.fight.fire.entry(body.gun.class()).or_insert([0; 5])[band_of(d)] += 1;
            }
            // And against the class's range, as far as it reaches.
            if let (Some(range), Some((_, d))) = (class_range_m(body.gun.class()), in_view(FIRE_FAR_M)) {
                let count = self.fight.fire_range.entry(body.gun.class()).or_default();
                count[0] += 1;
                count[1] += u32::from(d > range);
            }
            // ... and by the distance of its target.
            let at_target = body
                .target
                .and_then(|t| enemy_of(game, place, body.team, t))
                .map(|him| him.feet.distance_xz(body.feet))
                .filter(|d| (FIRE_MIN_M..=FIRE_FAR_M).contains(d));
            if let (Some(range), Some(d)) = (class_range_m(body.gun.class()), at_target) {
                let count = self.fight.fire_range_target.entry(body.gun.class()).or_default();
                count[0] += 1;
                count[1] += u32::from(d > range);
            }
        }
    }

    /// The shot the range alone held back this tick (fire key or not).
    pub(super) fn fight_range_held(&mut self, place: Place, body: &super::super::Body, now: &BotNow) {
        // The reflex decides ticks ahead of the game's: each of its ticks once.
        let Some(at) = now.range_held.filter(|&at| at != self.fight.held_at[place]) else {
            return;
        };
        self.fight.held_at[place] = at;
        if body.alive && class_range_m(body.gun.class()).is_some() {
            *self.fight.range_held.entry(body.gun.class()).or_default() += 1;
        }
    }

    /// A target switch while an enemy who hurt the bot is in its line, not the new target.
    pub(super) fn fight_switch(&mut self, game: &Game, place: Place, body: &super::super::Body, sight: &mapdata::RayTree, rays: &mut u64) {
        // A target switch while an enemy who hurt it is in its line,
        // not the new target.
        let target = body.target.filter(|_| body.alive);
        if let (Some(was), Some(new)) = (self.fight.last_target[place], target)
            && was != new
        {
            self.fight.switches += 1;
            let hurter_in_line = self.fight.hurt_by[place].iter().any(|&(who, _)| {
                who != new
                    && enemy_of(game, place, body.team, who).is_some_and(|him| {
                        in_sight(sight, him, body, f32::INFINITY, rays)
                    })
            });
            self.fight.switches_off_hurter += u32::from(hurter_in_line);
        }
        self.fight.last_target[place] = target;
    }

    /// The game's hits this tick, by the shooter's movement, the range and his weapon class.
    pub(super) fn fight_hits_scored(&mut self, game: &Game) {
        // Hits by the shooter's movement, the range and his weapon class.
        for &(_, shooter, victim) in &game.hit_log {
            let (Some(s), Some(v)) = (game.bodies.get(shooter), game.bodies.get(victim)) else {
                continue;
            };
            self.fight.hits_move[self.recs[shooter].last().map_or(0, move_of)] += 1;
            let band = band_of(s.feet.distance_xz(v.feet));
            self.fight.hits_band.entry(s.gun.class()).or_insert([0; 5])[band] += 1;
            if let Some(range) = class_range_m(s.gun.class()) {
                let count = self.fight.hits_range.entry(s.gun.class()).or_default();
                count[0] += 1;
                count[1] += u32::from(s.feet.distance_xz(v.feet) > range);
            }
        }
    }

    /// Reloads in an enemy's line (once a reload, every 4th tick), and the reloads that are over.
    pub(super) fn fight_reloads(&mut self, game: &Game, sight: &mapdata::RayTree, rays: &mut u64, tick: u32) {
        // Reloads in an enemy's line: once a reload, every 4th tick.
        if (tick / TICK_MS).is_multiple_of(4) {
            for reload in &mut self.fight.reloads {
                if reload.in_line || tick < reload.at || tick >= reload.until {
                    continue;
                }
                let me = &game.bodies[reload.place];
                if !me.alive {
                    continue;
                }
                reload.in_line = game.bodies.iter().any(|him| {
                    if !him.alive
                        || him.team == me.team
                        || him.feet.distance_xz(me.feet) > RELOAD_LINE_M
                    {
                        return false;
                    }
                    *rays += 2;
                    sight.visible(Game::eye(me), Game::chest(him))
                        || sight.visible(Game::eye(him), Game::chest(me))
                });
            }
        }
        let mut done = Vec::new();
        self.fight.reloads.retain(|r| {
            let over = tick > r.at.max(r.until) + RELOAD_DIED_MS;
            if over {
                done.push(*r);
            }
            !over
        });
        for r in done {
            self.reload_done(&r);
        }
    }

    /// The fight counts at the end: their keys of the `owner` object and their lines of the owner-test text.
    pub(super) fn fight_part(&self, scale: &Scale) -> Part {
        let n = self.recs.len();
        let bot = |place: Place| place < n;
        let tick_s = scale.tick_s;
        let pct = Scale::pct;
        let rate = Scale::rate;
        // Turned on the shooter.
        let (mut first_hits, mut hits_side, mut turned, mut died_unturned) = (0u32, 0u32, 0u32, 0u32);
        let mut last_hit: BTreeMap<Place, u32> = BTreeMap::new();
        for &(t, shooter, victim) in &self.fight.hits {
            let apart = last_hit.get(&victim).is_none_or(|&l| t >= l + HIT_APART_MS);
            last_hit.insert(victim, t);
            if !apart || !bot(victim) {
                continue;
            }
            let i = self.index(t);
            let recs = &self.recs[victim];
            let Some(at) = recs.get(i) else {
                continue;
            };
            if !at.alive {
                continue;
            }
            first_hits += 1;
            let from = |k: usize| self.recs[shooter].get(k).map_or(Vec3::ZERO, |r| r.feet);
            if off_deg(at.yaw, at.feet, from(i)) <= HIT_SIDE_DEG {
                continue;
            }
            hits_side += 1;
            let end = (i + (TURN_MS / TICK_MS) as usize).min(recs.len() - 1);
            let mut outcome = None;
            for k in i..=end {
                if !recs[k].alive {
                    outcome = Some(false);
                    break;
                }
                if off_deg(recs[k].yaw, recs[k].feet, from(k)) <= TURNED_DEG {
                    outcome = Some(true);
                    break;
                }
            }
            match outcome {
                Some(true) => turned += 1,
                Some(false) => died_unturned += 1,
                None => {}
            }
        }

        let fire_far: BTreeMap<&str, [u32; 2]> = self
            .fight.fire
            .iter()
            .map(|(class, bands)| (*class, [bands.iter().sum(), bands[3] + bands[4]]))
            .collect();
        let fire_near_class: [u32; 2] = self
            .fight.fire
            .iter()
            .filter(|(class, _)| matches!(**class, "shotgun" | "smg"))
            .fold([0, 0], |acc, (_, b)| [acc[0] + b.iter().sum::<u32>(), acc[1] + b[3] + b[4]]);
        let by_move = |v: &[u32; 5]| -> BTreeMap<&str, u32> { MOVES.iter().copied().zip(v.iter().copied()).collect() };
        let hit_rate_move: BTreeMap<&str, f64> = MOVES
            .iter()
            .enumerate()
            .map(|(i, m)| (*m, rate(self.fight.hits_move[i], self.fight.fire_move[i])))
            .collect();
        let hit_rate_class: BTreeMap<&str, Vec<f64>> = self
            .fight.fire
            .iter()
            .map(|(class, fire)| {
                let hits = self.fight.hits_band.get(class).copied().unwrap_or_default();
                (*class, (0..5).map(|b| rate(hits[b], fire[b])).collect())
            })
            .collect();
        // X1: hits a fire tick within the class's range and past it.
        let [fire_all, fire_past] = self.fight.fire_range.values().fold([0, 0], |a, v| [a[0] + v[0], a[1] + v[1]]);
        let [hits_all, hits_past] = self.fight.hits_range.values().fold([0, 0], |a, v| [a[0] + v[0], a[1] + v[1]]);
        let [target_all, target_past] = self.fight.fire_range_target.values().fold([0, 0], |a, v| [a[0] + v[0], a[1] + v[1]]);
        let rate_in = rate(hits_all - hits_past, fire_all - fire_past);
        let rate_past = rate(hits_past, fire_past);
        let rate_range_class: BTreeMap<&str, [f64; 2]> = self
            .fight
            .fire_range
            .iter()
            .map(|(class, fire)| {
                let hits = self.fight.hits_range.get(class).copied().unwrap_or_default();
                (*class, [rate(hits[0] - hits[1], fire[0] - fire[1]), rate(hits[1], fire[1])])
            })
            .collect();
        let held: u32 = self.fight.range_held.values().sum();
        let json = serde_json::json!({
            "dry_s": (self.fight.dry_ticks as f64 * tick_s).round(),
            "reload_s": (self.fight.reload_ticks as f64 * tick_s).round(),
            "deaths_dry": self.fight.deaths_dry,
            "deaths_reloading": self.fight.deaths_reloading,
            "reloads": self.fight.reloads_n,
            "reloads_in_line": self.fight.reloads_in_line,
            "reloads_in_line_died": self.fight.reloads_in_line_died,
            "fire_by_class": self.fight.fire,
            "fire_far_by_class": fire_far,
            "fire_far_close_class_pct": pct(fire_near_class[1].into(), fire_near_class[0].into()),
            "fire_past_class_range": self.fight.fire_range,
            "fire_past_class_range_pct": pct(fire_past.into(), fire_all.into()),
            "fire_past_class_range_at_target": self.fight.fire_range_target,
            "fire_past_class_range_at_target_pct": pct(target_past.into(), target_all.into()),
            "hits_past_class_range": self.fight.hits_range,
            "hit_rate_in_class_range": rate_in,
            "hit_rate_past_class_range": rate_past,
            "hit_rate_by_class_range": rate_range_class,
            "fire_held_by_class_range": self.fight.range_held,
            "hits_by_class": self.fight.hits_band,
            "hit_rate_by_class": hit_rate_class,
            "fire_by_move": by_move(&self.fight.fire_move),
            "hits_by_move": by_move(&self.fight.hits_move),
            "hit_rate_by_move": hit_rate_move,
            "fire_on_target": self.fight.fire_on_target,
            "fire_hidden": self.fight.fire_hidden,
            "fire_hidden_pct": pct(self.fight.fire_hidden.into(), self.fight.fire_on_target.into()),
            "fire_no_target": self.fight.fire_no_target,
            "target_switches": self.fight.switches,
            "switches_off_hurter": self.fight.switches_off_hurter,
            "hits_first": first_hits,
            "hits_side": hits_side,
            "turned_3s": turned,
            "turned_3s_pct": pct(turned.into(), hits_side.into()),
            "died_unturned": died_unturned,
        });
        let text = format!(
            "owner-test: dry {} s (deaths {}), reloading {} s (deaths {}), reloads {} in a line {} died {}; fire >=35 m by class {}; side hits {} turned in 3 s {} died unturned {}\n  owner-test fight: fire/hits by move {}; hits a fire tick by class and band {}; fire past the class range {} of {} ({:.1} %), at its target {} of {} ({:.1} %), hits a tick within {} past {}, held by the range {}; fire at a target {} out of line {} ({:.1} %), no target {}; target switches {} with the hurter in line {}",
            json["dry_s"],
            self.fight.deaths_dry,
            json["reload_s"],
            self.fight.deaths_reloading,
            self.fight.reloads_n,
            self.fight.reloads_in_line,
            self.fight.reloads_in_line_died,
            fire_far.iter().map(|(c, v)| format!("{c} {}/{}", v[1], v[0])).collect::<Vec<_>>().join(" "),
            hits_side,
            turned,
            died_unturned,
            MOVES.iter().enumerate().map(|(i, m)| format!("{m} {}/{}", self.fight.fire_move[i], self.fight.hits_move[i])).collect::<Vec<_>>().join(" "),
            json["hit_rate_by_class"],
            fire_past,
            fire_all,
            json["fire_past_class_range_pct"].as_f64().unwrap_or(0.0),
            target_past,
            target_all,
            json["fire_past_class_range_at_target_pct"].as_f64().unwrap_or(0.0),
            rate_in,
            rate_past,
            held,
            self.fight.fire_on_target,
            self.fight.fire_hidden,
            json["fire_hidden_pct"].as_f64().unwrap_or(0.0),
            self.fight.fire_no_target,
            self.fight.switches,
            self.fight.switches_off_hurter,
        );
        Part::new(json, text)
    }
}
