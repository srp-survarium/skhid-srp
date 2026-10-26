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

#[derive(Default)]
pub(super) struct ResupplyCounts {
    /// By place: the holds the bot counted so far, whether this life had
    /// its nearly dry moment, and the event open.
    holds_seen: Vec<u32>,
    life_done: Vec<bool>,
    open: Vec<Option<Event>>,
    holds: u32,
    events: Vec<Event>,
}

impl ResupplyCounts {
    pub(super) fn new(n: usize) -> Self {
        Self {
            holds_seen: vec![0; n],
            life_done: vec![false; n],
            open: vec![None; n],
            ..Self::default()
        }
    }
}

impl OwnerTally {
    /// One bot's tick: its holds, and its first nearly dry moment of a life.
    /// Called after the death count, before the ammunition's (a death's
    /// dryness is the last tick's).
    pub(super) fn resupply_step(&mut self, place: Place, tick: u32, prev: &Rec, body: &super::super::Body, now: &BotNow) {
        let counts = &mut self.resupply;
        counts.holds += now.resupplies.saturating_sub(counts.holds_seen[place]);
        counts.holds_seen[place] = now.resupplies;
        if prev.alive && !body.alive {
            if let Some(mut event) = counts.open[place].take() {
                event.end = tick;
                event.died = true;
                event.died_dry = self.fight.dry[place];
                counts.events.push(event);
            }
            counts.life_done[place] = false;
        }
        if body.alive && !counts.life_done[place] && now.nearly != Nearly::Fine {
            counts.life_done[place] = true;
            counts.open[place] = Some(Event {
                place,
                at: tick,
                nearly: now.nearly,
                end: tick,
                died: false,
                died_dry: false,
            });
        }
    }

    pub(super) fn resupply_part(&mut self, game: &Game, scale: &Scale) -> Part {
        let ticks = self.recs.first().map_or(0, Vec::len);
        let end = self.time_of(ticks);
        let mut events = std::mem::take(&mut self.resupply.events);
        for open in std::mem::take(&mut self.resupply.open).into_iter().flatten() {
            events.push(Event { end, ..open });
        }
        // Lives of the bots: each rise from dead (or the first tick) to alive.
        let lives: u32 = (0..self.recs.len())
            .filter(|&p| self.is_bot[p])
            .map(|p| {
                let r = &self.recs[p];
                (0..r.len()).filter(|&k| r[k].alive && (k == 0 || !r[k - 1].alive)).count() as u32
            })
            .sum();
        // The play's own runs: each rise of the bot's play to Resupply, and the seconds in it.
        let id = self.play_index.get("Resupply").copied();
        let (mut runs, mut run_ticks) = (0u32, 0u32);
        if let Some(id) = id {
            for p in (0..self.recs.len()).filter(|&p| self.is_bot[p]) {
                let r = &self.recs[p];
                for k in 0..r.len() {
                    if r[k].alive && r[k].play == id {
                        run_ticks += 1;
                        runs += u32::from(k == 0 || r[k - 1].play != id || !r[k - 1].alive);
                    }
                }
            }
        }
        let kills_of = |e: &Event| {
            game.kill_log
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
            "resupply_holds": self.resupply.holds,
            "resupply_lives": lives,
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
            self.resupply.holds,
            scale.per_min(f64::from(self.resupply.holds)),
            Scale::rate(self.resupply.holds, lives),
            lives,
            all[0], all[1], all[2], all[3], all[4],
            obj[0], obj[1], obj[2], obj[3], obj[4],
            NEAR_EXTRA_M,
            nr[0], nr[1], nr[2], nr[3], nr[4],
        );
        Part::new(json, text)
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
