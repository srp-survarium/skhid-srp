use super::*;

/// The batteries' carries as the bots saw them (the audit of the owner's
/// matches of 2026-10-23, #2, #6, #7): the carrier the view believed
/// against the stand-in's, world by world; the put-ins and takes that
/// failed; the intercepts and the interceptors killed. With
/// `SRP_BENCH_NOTES=<file>` every note and death is also appended there
/// (`<run> <place> <ms> <text>`), for the churn counts.
#[derive(Clone, Debug, Default)]
pub(crate) struct CarryTally {
    pub(crate) run: String,
    /// World-seconds of a carried battery: its carrier believed right,
    /// believed someone else, not known.
    pub(crate) right: u32,
    pub(crate) wrong: u32,
    pub(crate) unknown: u32,
    pub(crate) put_in_failed: u32,
    pub(crate) take_failed: u32,
    pub(crate) stands_failed: u32,
    pub(crate) collect_takes: u32,
    pub(crate) intercept_takes: u32,
    pub(crate) intercept_deaths: u32,
    /// Beliefs the view dropped or moved (its notes).
    pub(crate) beliefs_moved: u32,
    /// Each carrier's first world within [`ARRIVED_M`] of his container on
    /// this carry, and the put-ins seen; the put-ins' waits from there, ms.
    pub(crate) arrived: Vec<Option<u32>>,
    pub(crate) put_ins_seen: usize,
    pub(crate) put_in_waits: Vec<u32>,
    /// Every put-in (the carrier's team, the carrier, the time, ms) and the
    /// run's last world time: how often two carriers of a team put in
    /// together ([`put_in_pairs`]).
    pub(crate) put_in_times: Vec<(u8, usize, u32)>,
    pub(crate) span_ms: u32,
    /// The put-ins that began (a press whose ray met the box) by the length
    /// of use they needed: to 1.5 m, to 1.8 m (the client's reach with
    /// startup.cfg), over 1.8 m (up to the stand-in's 2.0).
    pub(crate) put_reach: [u32; 3],
    /// The uses at the containers ([`Game::put_presses`]) counted so far;
    /// of them: all, those that missed the box (no put began, none going
    /// on), those whose ray passed it by over [`FAR_MISS_M`] (the bot
    /// judges those missed at once), of these the ones that began a put all
    /// the same (a put the bot cut short) and the ones whose carrier's next
    /// use from the same stand began a put; and the waits from the
    /// arrival to the carry's first use there, ms.
    pub(crate) presses_seen: usize,
    pub(crate) presses: u32,
    pub(crate) presses_missed: u32,
    pub(crate) presses_far: u32,
    pub(crate) far_began: u32,
    pub(crate) far_then_in: u32,
    pub(crate) pressed: Vec<bool>,
    pub(crate) first_seen: usize,
    pub(crate) first_press_waits: Vec<u32>,
    /// Cologne CG-11 (`collect::POCKET_BLOCK`): by battery, the bots whose
    /// candidates leave it out for its pocket (the take-block notes), since
    /// when one does; the batteries ever so, and the ms one bot or more did
    /// (to the run's end if no note says otherwise). Ambush plays taken.
    pub(crate) pocket_holders: Vec<Vec<usize>>,
    pub(crate) pocket_since: Vec<Option<u32>>,
    pub(crate) pocket_batteries: Vec<bool>,
    pub(crate) pocket_ms: u32,
    pub(crate) ambush_takes: u32,
    pub(crate) last_time: u32,
}

/// A carrier is at his container within this, metres (xz).
pub(crate) const ARRIVED_M: f32 = 3.0;

/// The bot's rule for a use that surely missed the box (`collect`'s
/// `PUT_MISS_M`), metres; the next use "from the same stand" is within
/// [`SAME_STAND_M`] and [`NEXT_PRESS_MS`] of it.
pub(crate) const FAR_MISS_M: f32 = 1.0;

pub(crate) const SAME_STAND_M: f32 = 0.3;

pub(crate) const NEXT_PRESS_MS: u32 = 3_000;

/// Two put-ins of a team by different carriers this close are a pair: the
/// carriers put in together at the team's one container.
pub(crate) const PAIR_WINDOW_MS: u32 = 6_000;

/// The pairs of put-ins by two carriers of a team within [`PAIR_WINDOW_MS`],
/// and how many pairs independent arrivals would make by chance: `n` put-ins
/// of a team at uniform times over `span_ms` make `n (n - 1) / 2` pairs, each
/// within the window with probability `2 w / T - (w / T)^2`. Humans: 2 of 51
/// put-ins in a pair at about 0.15 put-ins a container-minute
/// (`human_putin_pairs.py`).
pub(crate) fn put_in_pairs(put_ins: &[(u8, usize, u32)], span_ms: u32) -> (u32, f64) {
    let mut observed = 0;
    let mut expected = 0.0;
    for team in 0..2u8 {
        let mut times: Vec<(usize, u32)> = put_ins
            .iter()
            .filter(|p| p.0 == team)
            .map(|p| (p.1, p.2))
            .collect();
        times.sort_by_key(|&(_, tick)| tick);
        for (i, &(place, tick)) in times.iter().enumerate() {
            observed += times[i + 1..]
                .iter()
                .take_while(|&&(_, later)| later - tick <= PAIR_WINDOW_MS)
                .filter(|&&(other, _)| other != place)
                .count() as u32;
        }
        let n = times.len() as f64;
        if span_ms > 0 && n > 1.0 {
            let ratio = f64::from(PAIR_WINDOW_MS) / f64::from(span_ms);
            expected += n * (n - 1.0) / 2.0 * (2.0 * ratio - ratio * ratio).clamp(0.0, 1.0);
        }
    }
    (observed, expected)
}


impl CarryTally {
    pub(crate) fn new(run: String) -> Self {
        Self {
            run,
            ..Self::default()
        }
    }

    /// A world: each carried battery's believed carrier against the truth;
    /// the carriers at their containers, and how long the put-ins took
    /// from there.
    pub(crate) fn world(&mut self, game: &Game, bots: &Bots) {
        self.arrived.resize(game.bodies.len(), None);
        self.pressed.resize(game.bodies.len(), false);
        // The uses old enough that the next one from the same stand is in.
        let presses = &game.put_presses;
        while let Some(press) = presses.get(self.presses_seen)
            && press.tick + NEXT_PRESS_MS <= game.tick
        {
            self.presses += 1;
            if press.began
                && let Some(reach) = press.reach
            {
                self.put_reach[usize::from(reach > 1.5) + usize::from(reach > 1.8)] += 1;
            }
            self.presses_missed += u32::from(!press.began && !press.putting);
            if press.miss > FAR_MISS_M {
                self.presses_far += 1;
                self.far_began += u32::from(press.began);
                let next = presses[self.presses_seen + 1..].iter().find(|p| {
                    p.place == press.place
                        && p.tick <= press.tick + NEXT_PRESS_MS
                        && p.feet.distance_xz(press.feet) <= SAME_STAND_M
                });
                self.far_then_in += u32::from(next.is_some_and(|p| p.began));
            }
            self.presses_seen += 1;
        }
        // The carry's first use there, from its arrival.
        for press in &presses[self.first_seen.min(presses.len())..] {
            if let Some(at) = self.arrived.get(press.place).copied().flatten()
                && !self.pressed[press.place]
                && press.tick >= at
            {
                self.pressed[press.place] = true;
                self.first_press_waits.push(press.tick - at);
            }
        }
        self.first_seen = presses.len();
        // The put-ins since, after the uses: a carry's first use and its
        // put-in may fall between the same two worlds.
        let log = &game.put_in_log;
        self.span_ms = self.span_ms.max(game.tick);
        for &(place, tick) in &log[self.put_ins_seen.min(log.len())..] {
            self.put_in_times.push((game.bodies[place].team, place, tick));
            if let Some(at) = self.arrived[place].take() {
                self.put_in_waits.push(tick.saturating_sub(at));
            }
        }
        self.put_ins_seen = log.len();
        for (place, body) in game.bodies.iter().enumerate() {
            let home = game.containers.iter().find(|(t, _)| *t == body.team);
            if !body.alive || body.carrying.is_none() {
                self.arrived[place] = None;
                self.pressed[place] = false;
            } else if self.arrived[place].is_none()
                && home.is_some_and(|(_, at)| at.distance_xz(body.feet) <= ARRIVED_M)
            {
                self.arrived[place] = Some(game.tick);
            }
        }
        for (battery, lying) in game.batteries.iter().enumerate() {
            if let Lying::Carried(place) = *lying {
                match bots.tracker().carrier(battery) {
                    Some(p) if p == place => self.right += 1,
                    Some(_) => self.wrong += 1,
                    None => self.unknown += 1,
                }
            }
        }
    }

    /// The deaths of a step (the statistics `before` it), by the play.
    pub(crate) fn deaths(&mut self, before: &[(u32, u32, u32)], game: &Game, bots: &Bots) {
        for (place, (now, was)) in game.stats.iter().zip(before).enumerate() {
            if now.1 <= was.1 {
                continue;
            }
            let play = bots.bots[place]
                .as_ref()
                .and_then(|b| b.assignment.as_ref())
                .map(|a| a.kind);
            if play == Some(super::plays::PlayKind::Intercept) {
                self.intercept_deaths += 1;
            }
            self.dump(place, game.tick, &format!("died (on {play:?})"));
        }
    }

    pub(crate) fn note(&mut self, note: &Note) {
        let text = note.text.as_str();
        if text.starts_with("ends Collect: could not put it in") {
            self.put_in_failed += 1;
        } else if text.starts_with("ends Collect: could not take it") {
            self.take_failed += 1;
        } else if text.starts_with("took no battery") {
            self.stands_failed += 1;
        } else if text.starts_with("takes Collect Battery") {
            self.collect_takes += 1;
        } else if text.starts_with("takes Intercept") {
            self.intercept_takes += 1;
        } else if text.contains("believed to carry battery") {
            self.beliefs_moved += 1;
        } else if text.starts_with("takes Ambush") {
            self.ambush_takes += 1;
        } else if let Some((battery, why)) = text
            .strip_prefix("battery ")
            .and_then(|rest| rest.split_once(' '))
            .and_then(|(n, rest)| Some((n.parse::<usize>().ok()?, rest)))
            && why.contains("Collect candidate")
        {
            self.pocket_note(
                battery,
                note.place,
                note.time,
                why.ends_with(super::plays::collect::POCKET_BLOCK),
            );
        }
        self.last_time = self.last_time.max(note.time);
        self.dump(note.place, note.time, text);
    }

    /// A take-block note of bot `place` on `battery`: left out for its
    /// pocket (`pocket`), or for another reason or back as a candidate.
    pub(crate) fn pocket_note(&mut self, battery: usize, place: usize, time: u32, pocket: bool) {
        if self.pocket_holders.len() <= battery {
            self.pocket_holders.resize(battery + 1, Vec::new());
            self.pocket_since.resize(battery + 1, None);
            self.pocket_batteries.resize(battery + 1, false);
        }
        let holders = &mut self.pocket_holders[battery];
        holders.retain(|&p| p != place);
        if pocket {
            holders.push(place);
            self.pocket_batteries[battery] = true;
            self.pocket_since[battery].get_or_insert(time);
        } else if holders.is_empty()
            && let Some(since) = self.pocket_since[battery].take()
        {
            self.pocket_ms += time.saturating_sub(since);
        }
    }

    /// The batteries left out for their pockets, and the seconds one bot
    /// or more did, to the run's last note.
    pub(crate) fn pocket_counts(&self) -> (usize, f32) {
        let open: u32 = self
            .pocket_since
            .iter()
            .flatten()
            .map(|&since| self.last_time.saturating_sub(since))
            .sum();
        let batteries = self.pocket_batteries.iter().filter(|&&b| b).count();
        (batteries, (self.pocket_ms + open) as f32 / 1000.0)
    }

    pub(crate) fn dump(&self, place: usize, time: u32, text: &str) {
        if let Some(path) = bench_settings().var_os("SRP_BENCH_NOTES") {
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .unwrap();
            writeln!(file, "{} {} {time} {text}", self.run, place as i64).unwrap();
        }
    }

    pub(crate) fn line(&self, taken: u32) -> String {
        let pocket = self.pocket_counts();
        let pairs = put_in_pairs(&self.put_in_times, self.span_ms);
        format!(
            "carrier believed right {} wrong {} unknown {} world-s; put-ins failed {}; takes failed {}, stands failed {}; collect takes {} for {taken} pickups; intercepts {}, interceptors killed {}; beliefs moved {}; put-ins from arrival {} took {} ms; uses at home {}, missed {}, far {} (began a put {}), far then in {}; first uses from arrival {} took {} ms; pocket blocked {} batteries {:.1} s; ambush takes {}; put-ins {}, pairs of carriers within {} s {} (by chance {:.1}); put-ins by the use ray they needed: to 1.5 m {}, to 1.8 m {}, over 1.8 m {}",
            self.right,
            self.wrong,
            self.unknown,
            self.put_in_failed,
            self.take_failed,
            self.stands_failed,
            self.collect_takes,
            self.intercept_takes,
            self.intercept_deaths,
            self.beliefs_moved,
            self.put_in_waits.len(),
            self.put_in_waits
                .iter()
                .map(|&ms| u64::from(ms))
                .sum::<u64>(),
            self.presses,
            self.presses_missed,
            self.presses_far,
            self.far_began,
            self.far_then_in,
            self.first_press_waits.len(),
            self.first_press_waits
                .iter()
                .map(|&ms| u64::from(ms))
                .sum::<u64>(),
            pocket.0,
            pocket.1,
            self.ambush_takes,
            self.put_in_times.len(),
            PAIR_WINDOW_MS / 1_000,
            pairs.0,
            pairs.1,
            self.put_reach[0],
            self.put_reach[1],
            self.put_reach[2],
        )
    }
}

#[cfg(test)]
mod put_in_pairs_tests {
    use super::*;

    #[test]
    fn two_carriers_of_a_team_within_the_window_are_a_pair_and_the_chance_follows_the_count() {
        // Team 0: places 1 and 2 at 100 s and 104 s (a pair), place 1 again at 150 s; team 1: one put-in.
        let put_ins = [(0, 1, 100_000), (0, 2, 104_000), (0, 1, 150_000), (1, 7, 101_000)];
        let (observed, expected) = put_in_pairs(&put_ins, 180_000);
        assert_eq!(observed, 1);
        // Team 0: 3 put-ins make 3 pairs, each within 6 s with 2 x 6/180 - (6/180)^2.
        let p = 2.0 * (6.0 / 180.0) - (6.0 / 180.0) * (6.0 / 180.0);
        assert!((expected - 3.0 * p).abs() < 1e-9, "{expected}");
        // The same carrier twice in the window is no pair.
        assert_eq!(put_in_pairs(&[(0, 1, 100_000), (0, 1, 103_000)], 180_000).0, 0);
        assert_eq!(put_in_pairs(&[], 0), (0, 0.0));
    }
}
