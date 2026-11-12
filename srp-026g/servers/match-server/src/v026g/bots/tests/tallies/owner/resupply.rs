//! The resupply counts (X2, `batteries-record.md`): the bots' box holds
//! (the human number is in the record: holds of 2.5 s or more within 4 m of
//! a box, a human life), and the question of whether the play needs the
//! commander's route: how often a bot bound for an objective is nearly dry
//! (`resupply::NUMBERS` `dry_share`) while its team's box lies within
//! [`NEAR_EXTRA_M`] of extra walk from the way to its goal (straight lines
//! here: the extra is `bot -> box -> goal` less `bot -> goal`), and what its
//! life is after that: its seconds, kills and whether it died dry.

use super::*;

/// The extra walk to the box that counts as on the way, m.
pub(crate) const NEAR_EXTRA_M: f32 = 10.0;

/// How near dry a bot is, and what it was bound for.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) enum Nearly {
    /// Rounds above the share, or nothing a resupply could add.
    #[default]
    Fine,
    /// Nearly dry and bound for an objective; the extra walk to its team's
    /// box on the way to its goal (`None`: no box).
    Objective { extra_m: Option<f32> },
    /// Nearly dry, any other play.
    Other,
}

/// A bot's first moment nearly dry in a life, until the life ends.
#[derive(Clone, Copy, Debug)]
struct Event {
    place: Place,
    at: u32,
    nearly: Nearly,
    end: u32,
    died: bool,
    died_dry: bool,
}

/// A bot's life as the ammunition sees it: the rounds it fired (the drops of
/// what it holds), against the kit's count for a life.
#[derive(Clone, Copy, Debug)]
struct Life {
    place: Place,
    at: u32,
    end: u32,
    whole: u32,
    have: u32,
    fired: u32,
    died: bool,
}

#[derive(Default)]
pub(super) struct ResupplyCounts {
    /// By place: the life going on, and the lives done.
    life: Vec<Option<Life>>,
    lives_done: Vec<Life>,
    /// By place: the holds the bot counted so far, whether this life had
    /// its nearly dry moment, and the event open.
    holds_seen: Vec<u32>,
    life_done: Vec<bool>,
    open: Vec<Option<Event>>,
    holds: u32,
    events: Vec<Event>,
    /// By place: what its bot says at this tick's reading (none: not a bot), and whether it was dry the tick
    /// before (a death's dryness is the last tick's).
    now: Vec<Option<ResupplyNow>>,
    dry_last: Vec<bool>,
}

/// What the resupply counts read of a bot at a tick.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ResupplyNow {
    resupplies: u32,
    nearly: Nearly,
    ammo: crate::v026g::bots::guns::Ammo,
    dry: bool,
}

/// How near dry the bot is: its rounds at or under the resupply play's
/// `dry_share` of the kit's count with something to refill, and then
/// whether it is bound for an objective (its play's rank) and how much
/// farther its goal is by its team's box.
pub(super) fn nearly_dry(bots: &Bots, bot: &super::super::Bot, feet: Vec3) -> Nearly {
    use crate::v026g::bots::plays::{PlayKind, Rank};
    let ammo = bot.ammo();
    if ammo.missing() == 0 || ammo.share() > bots.tunings.of(bot.team).number(PlayKind::Resupply, "dry_share") {
        return Nearly::Fine;
    }
    let bound = bot
        .assignment
        .as_ref()
        .and_then(|a| bots.tunings.of(bot.team).plays.get(&a.kind))
        .is_some_and(|play| play.rank == Rank::Objective);
    if !bound {
        return Nearly::Other;
    }
    let goal = match bot.intent.go {
        Go::To { at, .. } => at,
        Go::Stay => feet,
    };
    let extra_m = bot.side.and_then(|side| {
        let world = &bots.tactics.as_ref()?.world;
        world
            .map
            .zones
            .iter()
            .filter(|z| z.kind == mapdata::ZoneKind::Resupply && z.team == side)
            .filter_map(|z| z.shapes.first())
            .map(|shape| {
                feet.distance_xz(shape.center) + shape.center.distance_xz(goal) - feet.distance_xz(goal)
            })
            .min_by(f32::total_cmp)
    });
    Nearly::Objective { extra_m }
}

impl ResupplyCounts {
    pub(super) fn new(n: usize) -> Self {
        Self {
            holds_seen: vec![0; n],
            life_done: vec![false; n],
            open: vec![None; n],
            life: vec![None; n],
            now: vec![None; n],
            dry_last: vec![false; n],
            ..Self::default()
        }
    }
}

impl Topic for ResupplyCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    /// The holds, the rounds and how near dry each bot is, from the bots and where the game has them.
    fn bots(&mut self, bots: &Bots, game: &Game) {
        for place in 0..self.now.len() {
            self.now[place] = bots.bots.get(place).and_then(Option::as_ref).map(|bot| ResupplyNow {
                resupplies: bot.resupplies,
                nearly: nearly_dry(bots, bot, game.bodies[place].feet),
                ammo: bot.ammo(),
                dry: bot.reflex.dry(),
            });
        }
    }

    #[cfg(test)]
    fn inject(&mut self, place: Place, now: Option<&BotNow>) {
        self.now[place] = now.map(|n| ResupplyNow { resupplies: n.resupplies, nearly: n.nearly, ammo: n.ammo, dry: n.dry });
    }

    fn body(&mut self, _run: &Run, at: &BodyAt<'_>, _rays: &mut u64) {
        if let Some(now) = self.now[at.place] {
            self.resupply_step(at.place, at.tick, at.prev, &at.game.bodies[at.place], &now);
        }
    }

    fn parts(&mut self, run: &Run, game: &Game, scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("resupply", self.part(run, game, scale))]
    }
}

impl ResupplyCounts {
    /// One bot's tick: its holds, and its first nearly dry moment of a life.
    /// Called after the death count, before the ammunition's (a death's
    /// dryness is the last tick's).
    fn resupply_step(&mut self, place: Place, tick: u32, prev: &Rec, body: &super::super::Body, now: &ResupplyNow) {
        if prev.alive
            && !body.alive
            && let Some(mut life) = self.life[place].take()
        {
            life.end = tick;
            life.died = true;
            self.lives_done.push(life);
        }
        if body.alive {
            let life = self.life[place].get_or_insert(Life {
                place,
                at: tick,
                end: tick,
                whole: now.ammo.whole,
                have: now.ammo.have,
                fired: 0,
                died: false,
            });
            life.fired += life.have.saturating_sub(now.ammo.have);
            life.have = now.ammo.have;
        }
        self.holds += now.resupplies.saturating_sub(self.holds_seen[place]);
        self.holds_seen[place] = now.resupplies;
        if prev.alive && !body.alive {
            if let Some(mut event) = self.open[place].take() {
                event.end = tick;
                event.died = true;
                event.died_dry = self.dry_last[place];
                self.events.push(event);
            }
            self.life_done[place] = false;
        }
        if body.alive && !self.life_done[place] && now.nearly != Nearly::Fine {
            self.life_done[place] = true;
            self.open[place] = Some(Event {
                place,
                at: tick,
                nearly: now.nearly,
                end: tick,
                died: false,
                died_dry: false,
            });
        }
        self.dry_last[place] = body.alive && now.dry;
    }

    fn part(&mut self, run: &Run, game: &Game, scale: &Scale) -> Part {
        let ticks = run.recs.first().map_or(0, Vec::len);
        let end = run.time_of(ticks);
        let mut events = std::mem::take(&mut self.events);
        for open in std::mem::take(&mut self.open).into_iter().flatten() {
            events.push(Event { end, ..open });
        }
        // Lives of the bots: each rise from dead (or the first tick) to alive.
        let lives: u32 = (0..run.recs.len())
            .filter(|&p| run.is_bot[p])
            .map(|p| {
                let r = &run.recs[p];
                (0..r.len()).filter(|&k| r[k].alive && (k == 0 || !r[k - 1].alive)).count() as u32
            })
            .sum();
        // The play's own runs: each rise of the bot's play to Resupply, and the seconds in it.
        let id = run.play_index.get("Resupply").copied();
        let (mut runs, mut run_ticks) = (0u32, 0u32);
        if let Some(id) = id {
            for p in (0..run.recs.len()).filter(|&p| run.is_bot[p]) {
                let r = &run.recs[p];
                for k in 0..r.len() {
                    if r[k].alive && r[k].play == id {
                        run_ticks += 1;
                        runs += u32::from(k == 0 || r[k - 1].play != id || !r[k - 1].alive);
                    }
                }
            }
        }
        // The ammunition of the lives: as the humans' (`human_suicide.py`, D): life length, rounds fired,
        // rounds per kill, and what is left of the kit's count at a death.
        let mut lives_v = std::mem::take(&mut self.lives_done);
        for open in std::mem::take(&mut self.life).into_iter().flatten() {
            lives_v.push(Life { end, ..open });
        }
        let kills_in = |l: &Life| {
            run.kills
                .iter()
                .filter(|&&(t, shooter, victim)| {
                    shooter == l.place
                        && t >= l.at
                        && t <= l.end
                        && game.bodies.get(victim).zip(game.bodies.get(shooter)).is_some_and(|(v, s)| v.team != s.team)
                })
                .count() as u32
        };
        let lives_a: Vec<Life> = lives_v.into_iter().filter(|l| l.whole > 0 && (l.end - l.at) >= 5_000).collect();
        let (mut fired_all, mut kills_all, mut ms_all) = (0u32, 0u32, 0u64);
        let mut seconds: Vec<u32> = Vec::new();
        let mut fired_each: Vec<u32> = Vec::new();
        let mut left = [0u32; 4];
        let mut died = 0u32;
        for l in &lives_a {
            fired_all += l.fired;
            kills_all += kills_in(l);
            ms_all += u64::from(l.end - l.at);
            seconds.push((l.end - l.at) / 1000);
            fired_each.push(l.fired);
            if l.died {
                died += 1;
                let share = 1.0 - f64::from(l.fired.min(l.whole)) / f64::from(l.whole);
                left[match share {
                    s if s > 0.5 => 0,
                    s if s >= 0.1 => 1,
                    s if s > 0.0 => 2,
                    _ => 3,
                }] += 1;
            }
        }
        let median = |v: &mut Vec<u32>| {
            v.sort_unstable();
            v.get(v.len() / 2).copied().unwrap_or(0)
        };
        let (life_s, fired_med) = (median(&mut seconds), median(&mut fired_each));
        let min_all = ms_all as f64 / 60_000.0;
        let kill_pm = if min_all > 0.0 { f64::from(kills_all) / min_all } else { 0.0 };
        let shots_pm = if min_all > 0.0 { f64::from(fired_all) / min_all } else { 0.0 };
        let per_kill = if kills_all > 0 { f64::from(fired_all) / f64::from(kills_all) } else { 0.0 };
        let pct = |n: u32| if died > 0 { (1000.0 * f64::from(n) / f64::from(died)).round() / 10.0 } else { 0.0 };
        let ammo_text = format!(
            "owner-test ammo lives (bots, lives of 5 s or more): {} (ended by death {}); life s median {}; rounds fired per life median {} (kit count median {}); rounds per kill {:.1}; kills per bot-minute {:.2}; rounds per bot-minute {:.1}; rounds left of the kit's count at death: over 50 % {} %, 10-50 % {} %, under 10 % {} %, none {} % (totals: rounds {}, kills {}, minutes {:.2}, died over 50 {}, 10-50 {}, under 10 {}, none {})",
            lives_a.len(),
            died,
            life_s,
            fired_med,
            {
                let mut w: Vec<u32> = lives_a.iter().map(|l| l.whole).collect();
                median(&mut w)
            },
            per_kill,
            kill_pm,
            shots_pm,
            pct(left[0]),
            pct(left[1]),
            pct(left[2]),
            pct(left[3]),
            fired_all,
            kills_all,
            min_all,
            left[0],
            left[1],
            left[2],
            left[3],
        );
        let kills_of = |e: &Event| {
            run.kills
                .iter()
                .filter(|&&(t, shooter, victim)| {
                    shooter == e.place
                        && t >= e.at
                        && t <= e.end
                        && game.bodies.get(victim).zip(game.bodies.get(shooter)).is_some_and(|(v, s)| v.team != s.team)
                })
                .count() as u32
        };
        let near = |e: &Event| matches!(e.nearly, Nearly::Objective { extra_m: Some(m) } if m <= NEAR_EXTRA_M);
        let objective = |e: &Event| matches!(e.nearly, Nearly::Objective { .. });
        // [events, life seconds, kills, died, died dry] of a set.
        let sum = |keep: &dyn Fn(&Event) -> bool| {
            let mut s = [0u32; 5];
            for e in events.iter().filter(|e| keep(e)) {
                s[0] += 1;
                s[1] += (e.end - e.at) / 1000;
                s[2] += kills_of(e);
                s[3] += u32::from(e.died);
                s[4] += u32::from(e.died_dry);
            }
            s
        };
        let all = sum(&|_| true);
        let obj = sum(&objective);
        let nr = sum(&near);
        let json = serde_json::json!({
            "resupply_holds": self.holds,
            "resupply_lives": lives,
            "ammo_lives": lives_a.len(),
            "ammo_lives_died": died,
            "ammo_life_s_median": life_s,
            "ammo_fired_median": fired_med,
            "ammo_rounds_per_kill": (per_kill * 10.0).round() / 10.0,
            "ammo_kills_per_min": (kill_pm * 100.0).round() / 100.0,
            "ammo_rounds_per_min": (shots_pm * 10.0).round() / 10.0,
            "ammo_left_over_50": left[0],
            "ammo_left_10_50": left[1],
            "ammo_left_under_10": left[2],
            "ammo_left_none": left[3],
            "resupply_runs": runs,
            "resupply_run_s": (f64::from(run_ticks) * scale.tick_s).round(),
            "nearly_dry": all[0],
            "nearly_dry_life_s": all[1],
            "nearly_dry_kills": all[2],
            "nearly_dry_died": all[3],
            "nearly_dry_died_dry": all[4],
            "nearly_dry_objective": obj[0],
            "nearly_dry_objective_life_s": obj[1],
            "nearly_dry_objective_kills": obj[2],
            "nearly_dry_objective_died": obj[3],
            "nearly_dry_objective_died_dry": obj[4],
            "nearly_dry_objective_near": nr[0],
            "nearly_dry_objective_near_life_s": nr[1],
            "nearly_dry_objective_near_kills": nr[2],
            "nearly_dry_objective_near_died": nr[3],
            "nearly_dry_objective_near_died_dry": nr[4],
        });
        let text = format!(
            "owner-test resupply: runs of the play {} ({:.0} s); box holds {} ({:.3} a bot-minute, {:.3} a life of {}); nearly dry (a life's first moment at or under dry_share, with something to refill) {} (life after {} s, kills {}, died {} dry {}); of them bound for an objective {} (life {} s, kills {}, died {} dry {}), and with the box within {} m of extra walk {} (life {} s, kills {}, died {} dry {})",
            runs,
            f64::from(run_ticks) * scale.tick_s,
            self.holds,
            scale.per_min(f64::from(self.holds)),
            Scale::rate(self.holds, lives),
            lives,
            all[0], all[1], all[2], all[3], all[4],
            obj[0], obj[1], obj[2], obj[3], obj[4],
            NEAR_EXTRA_M,
            nr[0], nr[1], nr[2], nr[3], nr[4],
        );
        Part::new(json, format!("{text}\n  {ammo_text}"))
    }
}

#[test]
pub(crate) fn holds_and_the_nearly_dry_lives_are_counted() {
    let (mut game, mut tally) = made_up(&[0, 1, 0]);
    play(&mut game, &mut tally, 200, |k, game| {
        for p in 0..3 {
            put(game, p, 10.0 + 5.0 * p as f32, 10.0, 0);
        }
        // Bot 0 is nearly dry bound for an objective, the box 5 m of extra
        // walk, from tick 20; dry from 60; a hold at 40 (1); it kills
        // bot 1 at 80 and dies dry at 120. Bot 2 is nearly dry on another
        // play from 30 and lives on.
        game.bodies[0].alive = k < 120;
        game.bodies[1].alive = k < 80;
        if k == 80 {
            game.kill_log.push((game.tick, 0, 1));
        }
        let mut nows = vec![bot_now(); 3];
        let n0 = nows[0].as_mut().unwrap();
        if k >= 20 {
            n0.nearly = Nearly::Objective { extra_m: Some(5.0) };
        }
        n0.dry = k >= 60;
        n0.resupplies = u32::from(k >= 40);
        if k >= 30 {
            nows[2].as_mut().unwrap().nearly = Nearly::Other;
        }
        nows
    });
    let (text, json) = tally.finish(&game);
    assert_eq!(json["resupply_holds"], 1, "{json}");
    assert_eq!(json["nearly_dry"], 2);
    assert_eq!(json["nearly_dry_objective"], 1);
    assert_eq!(json["nearly_dry_objective_near"], 1);
    assert_eq!(json["nearly_dry_objective_near_kills"], 1);
    assert_eq!(json["nearly_dry_objective_near_died"], 1);
    assert_eq!(json["nearly_dry_objective_near_died_dry"], 1, "{json}");
    assert_eq!(json["nearly_dry_died"], 1, "bot 2 lives on");
    assert!(text.contains("box holds 1 ("), "{text}");
}

#[test]
pub(crate) fn the_ammunition_of_a_life_is_counted_to_its_death() {
    use crate::v026g::bots::guns::Ammo;
    let (mut game, mut tally) = made_up(&[0, 1]);
    play(&mut game, &mut tally, 400, |k, game| {
        for p in 0..2 {
            put(game, p, 10.0 + 5.0 * p as f32, 10.0, 0);
        }
        // Bot 0 holds 100 of its kit's 100, fires 60 by tick 100 (a 1 round drop each tick
        // from 20 to 80), kills bot 1 at 90, and dies at tick 300 (15 s): a life of 15 s,
        // 60 rounds fired, 1 kill, 40 % left. Bot 1 lives on (400 ticks = 20 s).
        game.bodies[0].alive = k < 300;
        if k == 90 {
            game.kill_log.push((game.tick, 0, 1));
        }
        let mut nows = vec![bot_now(); 2];
        nows[0].as_mut().unwrap().ammo = Ammo { have: 100 - (k.clamp(20, 80) - 20) as u32, whole: 100, storage: 500 };
        nows[1].as_mut().unwrap().ammo = Ammo { have: 100, whole: 100, storage: 500 };
        nows
    });
    let (text, json) = tally.finish(&game);
    assert_eq!(json["ammo_lives"], 2, "{json}");
    assert_eq!(json["ammo_lives_died"], 1);
    assert_eq!(json["ammo_left_10_50"], 1, "{json}");
    assert_eq!(json["ammo_rounds_per_kill"], 60.0, "60 rounds for one kill: {json}");
    assert!(text.contains("owner-test ammo lives"), "{text}");
}
