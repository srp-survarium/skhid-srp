use super::super::standin_heal::{DrugPress, Pressed};
use super::*;

/// The consumable counts (`fighting-record.md` §8.8, the table at its end;
/// the humans' side from `fact-health-recovery.md`): counted as the match
/// runs, worked out at its end ([`OwnerTally::heal_part`]) from a record of
/// every bot's body pool and medkits each tick and the stand-in's log of
/// the quick-use presses.
///
/// What the stand-in lets these show (`standin_heal.rs`): the damage is
/// 0.2 of the body a hit, no armour, so a body is hurt in steps of 0.2 and
/// dies on the fifth hit; the head, hands, pain and infection lose
/// nothing; the kits are the bots' (two medkits a kit, three kits). The
/// counts are of the body pool and the medkit.
#[derive(Default)]
pub(super) struct HealCounts {
    /// By place, each tick (the same indexes as [`OwnerTally::recs`]).
    pub(super) recs: Vec<Vec<HealRec>>,
}

impl HealCounts {
    pub(super) fn new(n: usize) -> Self {
        Self {
            recs: vec![Vec::new(); n],
        }
    }
}

/// One body at one tick.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct HealRec {
    alive: bool,
    /// The body pool.
    body: f32,
    /// Medkits in the slot of the kit in use.
    hand: u16,
    infection: f32,
    /// The bot has a target (the reflex's).
    target: bool,
    /// An enemy within 40 m has a clear line to the body, or the body to him.
    line: bool,
    /// A heal cover walk was begun in the last 1.6 s.
    walking: bool,
    /// The cover walks begun so far (a bot, this match).
    walks: u32,
}

/// An enemy this near with a clear line is "in an enemy's line", m (the situations).
const LINE_M: f32 = 40.0;
/// A heal cover walk begun this many ticks after a press (0.8 s) belongs to it.
const WALK_AFTER_TICKS: usize = 16;
/// A hurt body: under this (the record's 0.995).
const HURT: f32 = 0.995;
/// Healed: from this (the record's 0.99).
const HEALED: f32 = 0.99;
/// The presses a death counts after, ms.
const DEAD_AFTER_MS: u32 = 5_000;
/// The fatal fight's hits are one burst while they come this often, ms
/// (the killer's last burst: the record cuts the fight's start there).
const BURST_GAP_MS: u32 = 1_500;
/// A killing hit is the fatal fight's when it came this soon before the
/// death, ms.
const KILLED_BY_MS: u32 = 1_000;
/// "Older than 4 s": a hurt at the fight's start that old could have been
/// healed.
const OLD_MS: u32 = 4_000;
/// The window after a press the body is looked at, ticks of 3 s to 9 s.
const AFTER_FROM_S: u32 = 3;
const AFTER_TO_S: u32 = 9;

/// A hurt episode of a body: from its first loss to its heal or its death.
struct Episode {
    start: usize,
    lowest: f32,
    /// A medkit in the slot at its first loss.
    stock: bool,
    /// The tick of the first press in it.
    pressed: Option<u32>,
}

impl OwnerTally {
    /// One tick: every bot's body pool and medkits, as the game has them.
    pub(super) fn heal_tick(&mut self, game: &Game, nows: &[Option<BotNow>], sight: &mapdata::RayTree) {
        for (place, body) in game.bodies.iter().enumerate() {
            let (hand, _) = body.heal.medkits();
            let now = nows.get(place).and_then(Option::as_ref);
            // The rays only for a hurt bot: the line is asked of the presses on a hurt body.
            let line = body.alive
                && now.is_some()
                && body.health < HURT
                && game.bodies.iter().any(|him| {
                    him.alive
                        && him.team != body.team
                        && him.feet.distance_xz(body.feet) <= LINE_M
                        && (sight.visible(Game::eye(body), Game::chest(him))
                            || sight.visible(Game::eye(him), Game::chest(body)))
                });
            self.heal.recs[place].push(HealRec {
                alive: body.alive,
                body: body.health,
                hand,
                infection: body.rest.infection,
                target: now.is_some_and(|n| n.target),
                line,
                walking: now.is_some_and(|n| n.heal_walking),
                walks: now.map_or(0, |n| n.heal_walks),
            });
        }
    }

    /// The consumable counts at the end: their keys of the `owner` object and their line of the owner-test text.
    pub(super) fn heal_part(&self, game: &Game, scale: &Scale) -> Part {
        let tick_ms = TICK_MS;
        let ticks_of = |ms: u32| (ms / tick_ms) as usize;
        let mut episodes: Vec<Episode> = Vec::new();
        // Alive ticks of lives that began with and without a medkit; hurt
        // ticks of the former; deaths of the former, and of them those with
        // a medkit still in the slot and with no press in the life.
        let (mut alive_stock, mut alive_none, mut hurt_stock) = (0u64, 0u64, 0u64);
        let (mut deaths_stock, mut deaths_unused, mut deaths_no_press) = (0u32, 0u32, 0u32);
        let mut infection_deaths = 0u32;
        let mut deaths: Vec<(Place, usize)> = Vec::new();
        for place in (0..self.recs.len()).filter(|&p| self.is_bot[p]) {
            let recs = &self.heal.recs[place];
            let presses: Vec<u32> = game
                .drug_log
                .iter()
                .filter(|p| p.1 == place && matches!(p.2, Pressed::Used(_)))
                .map(|p| p.0)
                .collect();
            let mut life: Option<(bool, usize)> = None;
            let mut episode: Option<Episode> = None;
            for (k, rec) in recs.iter().enumerate() {
                let tick = self.time_of(k);
                let prev_alive = k > 0 && recs[k - 1].alive;
                if rec.alive && !prev_alive {
                    life = Some((rec.hand > 0, k));
                }
                if rec.alive {
                    let stock = life.is_some_and(|(stock, _)| stock);
                    if stock {
                        alive_stock += 1;
                        hurt_stock += u64::from(rec.body < HURT);
                    } else {
                        alive_none += 1;
                    }
                    let hurt = rec.body < HURT;
                    match (&mut episode, hurt) {
                        (None, true) => {
                            episode = Some(Episode {
                                start: k,
                                lowest: rec.body,
                                stock: rec.hand > 0,
                                pressed: None,
                            });
                        }
                        (Some(e), true) => e.lowest = e.lowest.min(rec.body),
                        (Some(_), false) => episodes.extend(episode.take()),
                        (None, false) => {}
                    }
                    if let Some(e) = &mut episode
                        && e.pressed.is_none()
                    {
                        let from = self.time_of(e.start);
                        e.pressed = presses.iter().copied().find(|&t| t >= from && t <= tick);
                    }
                }
                let next_alive = recs.get(k + 1).is_some_and(|r| r.alive);
                if rec.alive && !next_alive && k + 1 < recs.len() {
                    // A death (the run's last tick is no death).
                    episodes.extend(episode.take());
                    deaths.push((place, k));
                    infection_deaths += u32::from(rec.infection <= 0.1);
                    if let Some((stock, began)) = life
                        && stock
                    {
                        deaths_stock += 1;
                        deaths_unused += u32::from(rec.hand > 0);
                        let from = self.time_of(began);
                        deaths_no_press +=
                            u32::from(!presses.iter().any(|&t| t >= from && t <= tick));
                    }
                }
            }
            episodes.extend(episode.take());
        }
        // Episodes of a medkit's holders: pressed, and when after the first loss.
        let stocked: Vec<&Episode> = episodes.iter().filter(|e| e.stock).collect();
        let pressed_n = stocked.iter().filter(|e| e.pressed.is_some()).count();
        let depth = |lo: f32, hi: f32| {
            let of: Vec<_> = stocked
                .iter()
                .filter(|e| e.lowest >= lo && e.lowest < hi)
                .collect();
            (
                of.iter().filter(|e| e.pressed.is_some()).count() as f64,
                of.len() as f64,
            )
        };
        let bands = [depth(0.8, 2.0), depth(0.6, 0.8), depth(0.0, 0.6)];
        let mut delays: Vec<f64> = stocked
            .iter()
            .filter_map(|e| {
                e.pressed
                    .map(|t| f64::from(t - self.time_of(e.start)) / 1000.0)
            })
            .collect();
        delays.sort_by(f64::total_cmp);
        let median = delays.get(delays.len() / 2).copied();
        // Presses: used, on a hurt body, wasted; the heals that took the body full.
        let used: Vec<&DrugPress> = game
            .drug_log
            .iter()
            .filter(|p| matches!(p.2, Pressed::Used(_)) && self.is_bot[p.1])
            .collect();
        let hurt_used = used.iter().filter(|p| p.3 < HURT).count();
        let empty = game
            .drug_log
            .iter()
            .filter(|p| p.2 == Pressed::Empty && self.is_bot[p.1])
            .count();
        let active = game
            .drug_log
            .iter()
            .filter(|p| matches!(p.2, Pressed::Active(_)) && self.is_bot[p.1])
            .count();
        let (mut full_of, mut full_ok) = (0u32, 0u32);
        for p in used.iter().filter(|p| p.3 < HURT) {
            let k = self.index(p.0);
            let recs = &self.heal.recs[p.1];
            let (from, to) = (
                k + ticks_of(AFTER_FROM_S * 1000),
                k + ticks_of(AFTER_TO_S * 1000),
            );
            if recs.get(from).is_none_or(|r| !r.alive) {
                continue;
            }
            full_of += 1;
            full_ok += u32::from(
                recs[from..=to.min(recs.len() - 1)]
                    .iter()
                    .any(|r| r.alive && r.body >= HEALED),
            );
        }
        let dead_soon = deaths
            .iter()
            .filter(|&&(place, k)| {
                let tick = self.time_of(k);
                used.iter()
                    .any(|p| p.1 == place && p.0 <= tick && tick - p.0 <= DEAD_AFTER_MS)
            })
            .count();
        // The fatal fights: hurt at the start of the killer's last burst.
        let (mut fights, mut hurt_start, mut hurt_old) = (0u32, 0u32, 0u32);
        for &(place, k) in &deaths {
            let tick = self.time_of(k);
            let hits: Vec<(u32, Place)> = self
                .fight
                .hits
                .iter()
                .filter(|&&(t, _, victim)| victim == place && t <= tick)
                .map(|&(t, shooter, _)| (t, shooter))
                .collect();
            let Some(&(last, killer)) = hits.last() else {
                continue;
            };
            if tick - last > KILLED_BY_MS {
                continue;
            }
            let mut start = last;
            for &(t, shooter) in hits.iter().rev() {
                if shooter == killer && start - t <= BURST_GAP_MS {
                    start = t;
                }
            }
            fights += 1;
            let before = self.index(start).saturating_sub(1);
            let recs = &self.heal.recs[place];
            let hurt = recs.get(before).is_some_and(|r| r.body < HURT);
            hurt_start += u32::from(hurt);
            if hurt {
                // The age: since the last tick the body lost anything before the start.
                let lost = (1..=before)
                    .rev()
                    .find(|&i| {
                        recs[i].alive && recs[i - 1].alive && recs[i].body < recs[i - 1].body
                    })
                    .map_or(0, |i| self.time_of(i));
                hurt_old += u32::from(start.saturating_sub(lost) > OLD_MS);
            }
        }
        let pct = |a: u32, b: u32| Scale::pct(f64::from(a), f64::from(b));
        let alive_all = alive_stock + alive_none;
        let share = |a: u64, b: u64| Scale::pct(a as f64, b as f64);
        let per_100_min = if scale.bot_min > 0.0 {
            (10.0 * 100.0 * hurt_used as f64 / scale.bot_min).round() / 10.0
        } else {
            0.0
        };
        let heals_per_100 = if used.is_empty() {
            0.0
        } else {
            (10.0 * 100.0 * dead_soon as f64 / used.len() as f64).round() / 10.0
        };
        // The situation of every press on a hurt body, to be read with the deaths soon after
        // it: bits target 1, a loss in the last 2 s 2, an enemy's line 4, a heal cover walk
        // on at the press or begun within 0.8 s after it 8; each key [presses, of them dead within 5 s].
        let mut situations: BTreeMap<u32, [u32; 2]> = BTreeMap::new();
        for p in used.iter().filter(|p| p.3 < HURT) {
            let k = self.index(p.0);
            let recs = &self.heal.recs[p.1];
            let Some(rec) = recs.get(k) else { continue };
            let lost = (k.saturating_sub(40) + 1..=k.min(recs.len() - 1))
                .any(|i| i > 0 && recs[i].alive && recs[i - 1].alive && recs[i].body < recs[i - 1].body);
            // The walk is begun at the think after the press, so a walk counts when it is on at the
            // press or begins within `WALK_AFTER_TICKS` after it.
            let walk = rec.walking || recs[(k + WALK_AFTER_TICKS).min(recs.len() - 1)].walks > rec.walks;
            let bits = u32::from(rec.target)
                | u32::from(lost) << 1
                | u32::from(rec.line) << 2
                | u32::from(walk) << 3;
            let died = deaths
                .iter()
                .any(|&(place, dk)| place == p.1 && dk >= k && self.time_of(dk) - p.0 <= DEAD_AFTER_MS);
            let e = situations.entry(bits).or_default();
            e[0] += 1;
            e[1] += u32::from(died);
        }
        // The cover walks begun (the last record of each bot), and the heal plays taken: with the
        // habit on, none (a take of a Heal play displaces the running play).
        let walks: u32 = self.heal.recs.iter().filter_map(|r| r.last()).map(|r| r.walks).sum();
        let heal_index = self.play_index.get("Heal").copied();
        let displaced: u32 = self
            .recs
            .iter()
            .map(|recs| {
                recs.windows(2)
                    .filter(|w| Some(w[1].play) == heal_index && w[0].play != w[1].play)
                    .count() as u32
            })
            .sum();
        let by_depth: Vec<f64> = bands.iter().map(|&(a, b)| Scale::pct(a, b)).collect();
        let json = serde_json::json!({
            "heal_fatal_fights": fights,
            "heal_hurt_at_fatal_start_pct": pct(hurt_start, fights),
            "heal_hurt_over_4s_at_fatal_start_pct": pct(hurt_old, fights),
            "heal_hurt_share_pct": share(hurt_stock, alive_stock),
            "heal_hurt_episodes": stocked.len(),
            "heal_episodes_pressed_pct": pct(pressed_n as u32, stocked.len() as u32),
            "heal_episodes_pressed_by_depth_pct": by_depth,
            "heal_first_press_median_s": median.map(|m| (m * 10.0).round() / 10.0),
            "heal_presses": used.len(),
            "heal_presses_hurt_per_100_min": per_100_min,
            "heal_empty_presses": empty,
            "heal_active_presses": active,
            "heal_no_stock_alive_pct": share(alive_none, alive_all),
            "heal_dead_within_5s_per_100_heals": heals_per_100,
            "heal_deaths_with_stock": deaths_stock,
            "heal_dead_with_medkit_pct": pct(deaths_unused, deaths_stock),
            "heal_dead_no_press_pct": pct(deaths_no_press, deaths_stock),
            "heal_full_3_9s_after_pct": pct(full_ok, full_of),
            "heal_infection_deaths": infection_deaths,
            "heal_cover_walks": walks,
            "heal_plays_displaced": displaced,
            "heal_situations": situations,
        });
        let text = format!(
            "owner-test heal: hurt at the fatal fight's start {:.1} % ({:.1} % over 4 s old) of {} fights; hurt {:.1} % of alive time with stock; \
             episodes with a press {:.1} % of {} (depth >=0.8 / 0.6-0.8 / <0.6: {:.0} / {:.0} / {:.0} %), first press after {} s; \
             {} presses, {:.1} per 100 bot minutes on a hurt body, {} on an empty slot, {} on an active medkit; no stock at the spawn {:.1} % of alive time; \
             dead within 5 s of a press {:.1} per 100; dead with a medkit unused {:.1} % (no press in the life {:.1} %) of {}; full 3-9 s after {:.1} %; infection <= 0.1 at {} deaths; cover walks {}, heal plays taken {}",
            pct(hurt_start, fights),
            pct(hurt_old, fights),
            fights,
            share(hurt_stock, alive_stock),
            pct(pressed_n as u32, stocked.len() as u32),
            stocked.len(),
            by_depth[0],
            by_depth[1],
            by_depth[2],
            median.map_or("-".to_owned(), |m| format!("{m:.1}")),
            used.len(),
            per_100_min,
            empty,
            active,
            share(alive_none, alive_all),
            heals_per_100,
            pct(deaths_unused, deaths_stock),
            pct(deaths_no_press, deaths_stock),
            deaths_stock,
            pct(full_ok, full_of),
            infection_deaths,
            walks,
            displaced,
        );
        Part::new(json, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::stock::Drug;

    /// A body hurt to 0.6 at 1 s, pressed at 3 s (the medkit acts from 5 s)
    /// and full from 7.5 s; later hit to death by one killer from 20 s.
    #[test]
    fn a_hurt_a_press_a_heal_and_a_fatal_fight_are_counted() {
        let (mut game, mut tally) = made_up(&[0, 1]);
        let hold = |g: &mut Game| {
            for place in 0..2 {
                g.bodies[place].alive = true;
                g.bodies[place].step_m = 0.0;
            }
        };
        for k in 0..600usize {
            game.tick = (k as u32 + 1) * TICK_MS;
            let ms = game.tick;
            hold(&mut game);
            game.hit_log.clear();
            game.bodies[0].health = if ms <= 1_000 {
                1.0
            } else if ms <= 7_000 {
                0.6
            } else if ms <= 7_500 {
                0.6 + (ms - 7_000) as f32 / 500.0 * 0.4
            } else if ms <= 20_000 {
                1.0
            } else {
                // Five hits of 0.2 from 20 s on, one every 200 ms.
                (1.0 - 0.2 * ((ms - 20_000) / 200 + 1) as f32).max(0.0)
            };
            if ms == 3_000 {
                game.drug_log
                    .push((ms, 0, Pressed::Used(Drug::Medkit), 0.6));
            }
            if ms >= 20_000 && (ms - 20_000) % 200 == 0 && (ms - 20_000) / 200 < 5 {
                game.hit_log.push((ms, 1, 0));
            }
            if ms >= 21_000 {
                game.bodies[0].alive = false;
            }
            tally.step(&mut game, &[bot_now(), bot_now()]);
        }
        game.drug_log.push((21_500, 1, Pressed::Empty, 1.0));
        let (text, json) = tally.finish(&game);
        // Two episodes: the heal, and the fight that killed it (no press).
        assert_eq!(json["heal_hurt_episodes"], 2, "{json}");
        assert_eq!(json["heal_episodes_pressed_pct"], 50.0);
        assert_eq!(
            json["heal_episodes_pressed_by_depth_pct"],
            serde_json::json!([0.0, 100.0, 0.0])
        );
        // The first loss at 1.05 s, the press at 3 s.
        let delay = json["heal_first_press_median_s"].as_f64().unwrap();
        assert!((delay - 2.0).abs() <= 0.1, "{delay}");
        assert_eq!(json["heal_presses"], 1);
        assert_eq!(json["heal_empty_presses"], 1);
        // The press found the body hurt, and it was full 3-9 s later.
        assert_eq!(json["heal_full_3_9s_after_pct"], 100.0);
        // One fatal fight, the killer's burst from full health: not hurt at its start.
        assert_eq!(json["heal_fatal_fights"], 1, "{json}");
        assert_eq!(json["heal_hurt_at_fatal_start_pct"], 0.0);
        assert!(text.contains("owner-test heal:"), "{text}");
        // The life had a medkit at its start and died with it unused and
        // one press (the first life: no death before 20 s).
        assert_eq!(json["heal_deaths_with_stock"], 1);
    }

    /// A cover walk begun after the press (the reflex begins it at its next think) belongs to the press.
    #[test]
    fn a_walk_begun_just_after_a_press_is_in_its_situation() {
        let (mut game, mut tally) = made_up(&[0, 1]);
        for k in 0..400usize {
            game.tick = (k as u32 + 1) * TICK_MS;
            let ms = game.tick;
            for place in 0..2 {
                game.bodies[place].alive = true;
                game.bodies[place].step_m = 0.0;
            }
            game.hit_log.clear();
            game.bodies[0].health = if ms <= 1_000 { 1.0 } else { 0.6 };
            if ms == 3_000 {
                game.drug_log.push((ms, 0, Pressed::Used(Drug::Medkit), 0.6));
            }
            // The walk begins at 3.3 s: the count of walks goes up and the bot walks.
            let begun = ms >= 3_300;
            let now = Some(BotNow {
                play: "Roam".to_owned(),
                target: true,
                heal_walking: begun && ms < 3_300 + 1_600,
                heal_walks: u32::from(begun),
                ..BotNow::default()
            });
            tally.step(&mut game, &[now, bot_now()]);
        }
        let (_, json) = tally.finish(&game);
        let situations = json["heal_situations"].as_object().expect("situations");
        // Target (1) and a cover walk (8); no loss in the last 2 s? the loss came at 1.05 s, 2 s before the press.
        assert!(situations.keys().any(|k| k.parse::<u32>().unwrap() & 9 == 9), "{situations:?}");
        assert_eq!(json["heal_cover_walks"], 1);
    }
}
