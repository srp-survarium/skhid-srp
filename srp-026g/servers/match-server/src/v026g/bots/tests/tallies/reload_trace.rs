//! [reload trace] [measure] Event rows only (no count, no decision): what a bot did with each magazine and where it was while
//! it reloaded, for the split of the deaths in a reload by their cause.
//!
//! `"mag": [bot, class, how it ended, magazine, rounds left, shots, shots that hit, kills, ms from its first shot to its end
//! (null: no shot), the target's distance at its shots (least, median, most; null: none had a target), targets shot at,
//! health at the first shot, health at the end]`: one row for each magazine a bot fired from, at the reload that ends it
//! (`tactical`, `empty_in_view`, `empty`) or at its death (`death`).
//!
//! `"rl": [bot, class, why, length ms, enemies with a line at the start, the nearest enemy m, the nearest with a line m, its
//! target's distance m, ms since its last shot, ms since an enemy last had a line (0: one has it now; null: none in the
//! life), health, friends alive within 15 m, cover state, ms to the first sample with no line (null: none, or no line at
//! the start), ms to the first sample with a line (null: none), samples, samples in a line, the way walked m, how far from
//! the start m at the end, ticks watched, ticks with the sprint, aim and crouch keys, ticks moving, ticks of them forward
//! alone, ticks hiding, how much nearer the enemy nearest at the start came m, died after ms, the killer, the killer's
//! distance m, the killer was its target at the start, the killer had a line at the start]`: one row for each reload,
//! watched for [`WATCH_MS`] from its start or to a second after its end, whichever is later, to the death or to the next
//! reload. A line is the owner tally's (`RELOAD_LINE_M`, either way), sampled every [`SAMPLE_TICKS`] ticks.

use mapdata::Vec3;

use super::super::{BenchEvents, Bot, Game, TICK_MS};
use super::owner;
use crate::v026g::bots::reflex::ReloadWhy;
use crate::v026g::bots::view::Place;
use crate::v026g::drive::key;

/// A reload is watched at least this long from its start, ms: the window the players' reloads are read in.
pub(crate) const WATCH_MS: u32 = 3_000;
/// A line is asked every this many ticks.
pub(crate) const SAMPLE_TICKS: u32 = 2;

#[derive(Clone, Default)]
struct Mag {
    shots: u32,
    hits: u32,
    kills: u32,
    first: Option<(u32, f32)>,
    distances: Vec<f32>,
    targets: Vec<Place>,
}

#[derive(Clone)]
struct Walk {
    at: u32,
    until: u32,
    head: serde_json::Value,
    line_at_start: bool,
    nearest: Option<(Place, f32)>,
    target: Option<Place>,
    lined: Vec<Place>,
    clear_ms: Option<u32>,
    line_ms: Option<u32>,
    samples: u32,
    in_line: u32,
    way_m: f32,
    from: Vec3,
    last: Vec3,
    ticks: u32,
    keys: [u32; 6],
    hiding: u32,
}

#[derive(Clone, Default)]
struct One {
    alive: bool,
    mag: Mag,
    walk: Option<Walk>,
    reload_at: Option<u32>,
    last_shot: Option<u32>,
    last_line: Option<u32>,
}

#[derive(Default)]
pub(crate) struct ReloadTrace {
    by_place: Vec<One>,
}

fn round(x: f32, to: f64) -> f64 {
    (f64::from(x) * to).round() / to
}

impl ReloadTrace {
    /// After a game step, for one bot: its shots, hits and kills of the step.
    pub(crate) fn tick(&mut self, game: &Game, bot: &Bot, (shots, hits, kills): (u32, u32, u32), killer: Option<Place>, events: &mut BenchEvents) {
        if !events.on() {
            return;
        }
        let place = bot.place;
        if self.by_place.len() <= place {
            self.by_place.resize(place + 1, One::default());
        }
        let now = game.tick;
        let me = &game.bodies[place];
        let class = me.gun.class();
        let one = &mut self.by_place[place];
        let enemies = || game.bodies.iter().enumerate().filter(|(_, b)| b.alive && b.team != me.team);
        let eye = Game::eye(me);
        let lined = || -> Vec<(Place, f32)> {
            enemies()
                .map(|(p, b)| (p, b.feet.distance_xz(me.feet), b))
                .filter(|(_, m, _)| *m <= owner::RELOAD_LINE_M)
                .filter(|(_, _, b)| game.world.sight.visible(eye, Game::chest(b)) || game.world.sight.visible(Game::eye(b), Game::chest(me)))
                .map(|(p, m, _)| (p, m))
                .collect()
        };
        let mag_row = |mag: &Mag, how: &str, left: u32, magazine: u32| {
            let mut d = mag.distances.clone();
            d.sort_by(f32::total_cmp);
            let dist = (!d.is_empty()).then(|| [round(d[0], 10.0), round(d[d.len() / 2], 10.0), round(d[d.len() - 1], 10.0)]);
            serde_json::json!({
                "t": now,
                "mag": [place, class, how, magazine, left, mag.shots, mag.hits, mag.kills, mag.first.map(|(t, _)| now.saturating_sub(t)),
                    dist, mag.targets.len(), mag.first.map(|(_, h)| round(h, 100.0)), round(me.health.max(0.0), 100.0)],
            })
        };
        // The death: the magazine it held, and the reload it was in.
        if !me.alive {
            if one.alive {
                if one.mag.shots > 0 {
                    events.line(mag_row(&one.mag, "death", 0, me.gun.magazine));
                }
                if let Some(walk) = one.walk.take() {
                    events.line(Self::walk_row(game, place, class, walk, Some(now), killer));
                }
            }
            *one = One::default();
            return;
        }
        one.alive = true;
        // The magazine's shots.
        one.mag.kills += kills;
        if shots > 0 {
            one.last_shot = Some(now);
            one.mag.shots += shots;
            one.mag.hits += hits;
            one.mag.first.get_or_insert((now, me.health));
            if let Some(target) = bot.reflex.target() {
                one.mag.distances.push(game.bodies[target].feet.distance_xz(me.feet));
                if !one.mag.targets.contains(&target) {
                    one.mag.targets.push(target);
                }
            }
        }
        let note = bot.reflex.reload_note().filter(|note| now < note.until);
        let sample = (now / TICK_MS + place as u32).is_multiple_of(SAMPLE_TICKS);
        let new = note.filter(|note| one.reload_at != Some(note.at));
        let in_line = (sample || new.is_some() || one.walk.is_some()).then(lined);
        if let Some(lines) = in_line.as_ref()
            && !lines.is_empty()
        {
            one.last_line = Some(now);
        }
        // The reload under watch.
        if let Some(mut walk) = one.walk.take() {
            if let Some(note) = note.filter(|note| note.at == walk.at) {
                walk.until = note.until;
            }
            if new.is_some() || now > (walk.until + owner::RELOAD_AFTER_MS).max(walk.at + WATCH_MS) {
                events.line(Self::walk_row(game, place, class, walk, None, None));
            } else {
                walk.ticks += 1;
                walk.way_m += me.feet.distance_xz(walk.last);
                walk.last = me.feet;
                let moving = me.keys & (key::FORWARD | key::BACK | key::LEFT | key::RIGHT);
                for (count, on) in walk.keys.iter_mut().zip([
                    me.keys & key::SPRINT != 0,
                    me.keys & key::AIM != 0,
                    me.keys & key::CROUCH != 0,
                    moving != 0,
                    moving == key::FORWARD,
                    me.keys & key::FIRE != 0,
                ]) {
                    *count += u32::from(on);
                }
                walk.hiding += u32::from(bot.reflex.cover_state() == "hiding");
                if (now.saturating_sub(walk.at) / TICK_MS).is_multiple_of(SAMPLE_TICKS)
                    && let Some(lines) = in_line.as_ref()
                {
                    walk.samples += 1;
                    let after = now.saturating_sub(walk.at);
                    if lines.is_empty() {
                        if walk.line_at_start {
                            walk.clear_ms.get_or_insert(after);
                        }
                    } else {
                        walk.in_line += 1;
                        walk.line_ms.get_or_insert(after);
                    }
                }
                one.walk = Some(walk);
            }
        }
        // A new reload: the magazine's row, and the reload watched from here.
        if let Some(note) = new {
            one.reload_at = Some(note.at);
            if one.mag.shots > 0 {
                let how = match note.why {
                    ReloadWhy::EmptyInView => "empty_in_view",
                    ReloadWhy::Empty => "empty",
                    ReloadWhy::Tactical => "tactical",
                };
                events.line(mag_row(&one.mag, how, note.left, note.magazine));
            }
            let why = match note.why {
                ReloadWhy::EmptyInView => "empty_in_view",
                ReloadWhy::Empty => "empty",
                ReloadWhy::Tactical => "tactical",
            };
            one.mag = Mag::default();
            let lines = in_line.clone().unwrap_or_default();
            let nearest = enemies().map(|(p, b)| (p, b.feet.distance_xz(me.feet))).min_by(|a, b| a.1.total_cmp(&b.1));
            let nearest_lined = lines.iter().map(|l| l.1).min_by(f32::total_cmp);
            let target = bot.reflex.target();
            let friends = game
                .bodies
                .iter()
                .enumerate()
                .filter(|(p, b)| *p != place && b.alive && b.team == me.team && b.feet.distance_xz(me.feet) <= 15.0)
                .count();
            let since_line = if lines.is_empty() { one.last_line.map(|t| now.saturating_sub(t)) } else { Some(0) };
            let head = serde_json::json!([
                why,
                note.until - note.at,
                lines.len(),
                nearest.map(|n| round(n.1, 10.0)),
                nearest_lined.map(|m| round(m, 10.0)),
                target.map(|t| round(game.bodies[t].feet.distance_xz(me.feet), 10.0)),
                one.last_shot.map(|t| now.saturating_sub(t)),
                since_line,
                round(me.health, 100.0),
                friends,
                bot.reflex.cover_state(),
            ]);
            one.walk = Some(Walk {
                at: note.at,
                until: note.until,
                head,
                line_at_start: !lines.is_empty(),
                nearest,
                target,
                lined: lines.iter().map(|l| l.0).collect(),
                clear_ms: None,
                line_ms: (!lines.is_empty()).then_some(0),
                samples: 1,
                in_line: u32::from(!lines.is_empty()),
                way_m: 0.0,
                from: me.feet,
                last: me.feet,
                ticks: 0,
                keys: [0; 6],
                hiding: 0,
            });
        }
    }

    fn walk_row(game: &Game, place: Place, class: &str, walk: Walk, died: Option<u32>, killer: Option<Place>) -> serde_json::Value {
        let me = &game.bodies[place];
        let nearer = walk.nearest.map(|(p, m)| round(m - game.bodies[p].feet.distance_xz(me.feet), 10.0));
        let mut row = vec![serde_json::json!(place), serde_json::json!(class)];
        if let serde_json::Value::Array(head) = walk.head {
            row.extend(head);
        }
        row.extend([
            serde_json::json!(walk.clear_ms),
            serde_json::json!(walk.line_ms),
            serde_json::json!(walk.samples),
            serde_json::json!(walk.in_line),
            serde_json::json!(round(walk.way_m, 10.0)),
            serde_json::json!(round(me.feet.distance_xz(walk.from), 10.0)),
            serde_json::json!(walk.ticks),
            serde_json::json!(walk.keys),
            serde_json::json!(walk.hiding),
            serde_json::json!(nearer),
            serde_json::json!(died.map(|t| t.saturating_sub(walk.at))),
            serde_json::json!(killer),
            serde_json::json!(killer.map(|k| round(game.bodies[k].feet.distance_xz(me.feet), 10.0))),
            serde_json::json!(killer.map(|k| walk.target == Some(k))),
            serde_json::json!(killer.map(|k| walk.lined.contains(&k))),
        ]);
        serde_json::json!({"t": walk.at, "rl": row})
    }
}
