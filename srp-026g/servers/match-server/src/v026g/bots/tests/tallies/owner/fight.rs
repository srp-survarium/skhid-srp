use super::*;

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
        self.reloads_in_line += 1;
        let recs = &self.recs[r.place];
        let i0 = self.index(r.at);
        let i1 = self.index(r.at + RELOAD_DIED_MS).min(recs.len().saturating_sub(1));
        let died = (i0.max(1)..=i1).any(|i| recs[i - 1].alive && !recs[i].alive);
        self.reloads_in_line_died += u32::from(died);
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
