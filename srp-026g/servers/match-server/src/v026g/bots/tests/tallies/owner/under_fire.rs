//! A230 (the owner, 2026-11-02): do bots under fire switch to the fight, or keep their team play? A220 (the live bots, the
//! recorded PvB matches a human ended with KD 10 or more): the bots he killed died in PairUp 22 %, Roam 20 %, Collect 11 %,
//! Buddy 10 %, HoldExit 9 %, Trade 7 %, a play a median 3 s old, and 26 % fired nothing in their last 5 s (A222: 68 % of
//! those within about 1 s of his first hit). Our tip fights in the reflex whatever the play (it aims and fires at its
//! target), so a team play kept under fire is not by itself no fight. Two counts, the stand-in human's hits and kills apart:
//!
//! - **the first enemy hit of a bot's life**, by the play at the hit: followed to the bot's death or [`FOLLOW_MS`]: the view
//!   off the shooter at the hit (bands as `look.rs`), whether its own sight of him (`Track::sight_since`) began before the
//!   hit, and the times from the hit until its reflex targets him, it presses fire, it fires with him the target, and its
//!   play changes (and to what); the time from its own first sight of him to its first shot at him; whether it died within
//!   1 s, died in the window, and died in the play it was hit in;
//! - **each bot's death**, by the play at the death: whether it pressed fire in its last 5 s (A220's measure) and whether its
//!   first hit of the life came within 1 s of the death.
//!
//! The times are in [`BIN_MS`] bins up to [`FOLLOW_MS`] (the last bin: never in the window). Nothing decides by these counts.

use super::*;
use crate::v026g::bots::kin;
use crate::v026g::bots::know::Source;

/// A first hit is followed this long, ms.
const FOLLOW_MS: u32 = 10_000;
/// A death this soon after the first hit, ms (A222's window).
const QUICK_MS: u32 = 1_000;
/// A220's window: a shot this long before the death, ms.
const LAST_MS: u32 = 5_000;
/// The times' bin, ms.
const BIN_MS: u32 = 250;
/// The bins: up to [`FOLLOW_MS`], then one for never.
const BINS: usize = (FOLLOW_MS / BIN_MS) as usize + 1;
/// The view bands off the shooter at the hit, degrees: up to each, then past the last (as `look.rs`).
const BANDS_DEG: [f32; 3] = [15.0, 45.0, 90.0];

/// The bin of a time `ms` after its start; none: never in the window.
fn bin(ms: Option<u32>) -> usize {
    ms.map_or(BINS - 1, |ms| ((ms / BIN_MS) as usize).min(BINS - 2))
}

/// The band of the view `yaw` at `from` off the bearing to `to`.
fn band(yaw: f32, from: Vec3, to: Vec3) -> usize {
    let bearing = kin::yaw_of([to.x - from.x, to.z - from.z]);
    let off = ((yaw - bearing + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI).abs().to_degrees();
    BANDS_DEG.iter().position(|&b| off <= b).unwrap_or(BANDS_DEG.len())
}

/// A first hit of a life, followed to the death or [`FOLLOW_MS`]: when each answer first came (none: not yet).
#[derive(Clone, Copy, Debug)]
struct Open {
    at: u32,
    shooter: Place,
    play: u16,
    band: usize,
    /// When its own sight of him began, if it had begun at the hit.
    seen_before: Option<u32>,
    /// When its own sight of him first began after the hit.
    seen_after: Option<u32>,
    target_at: Option<u32>,
    fire_at: Option<u32>,
    fire_him_at: Option<u32>,
    /// The first other play, and when.
    switch: Option<(u32, u16)>,
}

/// The first hits of one play (the play at the hit), by the stand-in human's or a bot's.
#[derive(Clone, Debug)]
struct ByPlay {
    hits: u64,
    by_band: [u64; BANDS_DEG.len() + 1],
    seen_before: u64,
    died_1s: u64,
    died: u64,
    /// Died in the window in the play it was hit in.
    died_same_play: u64,
    targeted: u64,
    fired: u64,
    fired_him: u64,
    switched: u64,
    switched_to: BTreeMap<u16, u64>,
    /// From the hit: to the reflex's target on him, to a fire key, to a shot at him, to another play; from its own first sight
    /// of him (before the hit or after) to a shot at him.
    to_target: [u64; BINS],
    to_fire: [u64; BINS],
    to_fire_him: [u64; BINS],
    to_switch: [u64; BINS],
    sight_to_fire_him: [u64; BINS],
}

impl Default for ByPlay {
    fn default() -> Self {
        Self {
            hits: 0,
            by_band: [0; BANDS_DEG.len() + 1],
            seen_before: 0,
            died_1s: 0,
            died: 0,
            died_same_play: 0,
            targeted: 0,
            fired: 0,
            fired_him: 0,
            switched: 0,
            switched_to: BTreeMap::new(),
            to_target: [0; BINS],
            to_fire: [0; BINS],
            to_fire_him: [0; BINS],
            to_switch: [0; BINS],
            sight_to_fire_him: [0; BINS],
        }
    }
}

impl ByPlay {
    fn add(&mut self, hit: &Open, died_at: Option<u32>) {
        let after = |t: Option<u32>| t.map(|t| t.saturating_sub(hit.at));
        self.hits += 1;
        self.by_band[hit.band] += 1;
        self.seen_before += u64::from(hit.seen_before.is_some());
        self.died_1s += u64::from(died_at.is_some_and(|t| t.saturating_sub(hit.at) <= QUICK_MS));
        self.died += u64::from(died_at.is_some());
        self.died_same_play += u64::from(died_at.is_some() && hit.switch.is_none());
        self.targeted += u64::from(hit.target_at.is_some());
        self.fired += u64::from(hit.fire_at.is_some());
        self.fired_him += u64::from(hit.fire_him_at.is_some());
        if let Some((_, to)) = hit.switch {
            self.switched += 1;
            *self.switched_to.entry(to).or_default() += 1;
        }
        self.to_target[bin(after(hit.target_at))] += 1;
        self.to_fire[bin(after(hit.fire_at))] += 1;
        self.to_fire_him[bin(after(hit.fire_him_at))] += 1;
        self.to_switch[bin(after(hit.switch.map(|s| s.0)))] += 1;
        let seen = hit.seen_before.or(hit.seen_after);
        self.sight_to_fire_him[bin(seen.zip(hit.fire_him_at).map(|(s, f)| f.saturating_sub(s)))] += 1;
    }
}

/// The deaths of one play (the play at the death): fired in the last [`LAST_MS`], the life's first hit within [`QUICK_MS`].
#[derive(Clone, Copy, Debug, Default)]
struct Deaths {
    deaths: u64,
    fired_5s: u64,
    first_hit_1s: u64,
}

#[derive(Default)]
pub(super) struct UnderFireCounts {
    human: Option<Place>,
    /// Each place's reflex target and its own sights (who, since), at this tick's read.
    target: Vec<Option<Place>>,
    sights: Vec<Vec<(Place, u32)>>,
    /// The open first hit of each place's life; whether it has had one; its last fire key; its life's first hit.
    open: Vec<Option<Open>>,
    hit_this_life: Vec<bool>,
    last_fire: Vec<Option<u32>>,
    first_hit: Vec<Option<u32>>,
    hits_seen: usize,
    kills_seen: usize,
    /// By the stand-in human's ([1]) or a bot's ([0]) hit, the play at the hit.
    by_play: [BTreeMap<u16, ByPlay>; 2],
    /// By the stand-in human's ([1]) or a bot's ([0]) kill, the play at the death.
    deaths: [BTreeMap<u16, Deaths>; 2],
}

impl UnderFireCounts {
    pub(super) fn new(n: usize) -> Self {
        Self {
            target: vec![None; n],
            sights: vec![Vec::new(); n],
            open: vec![None; n],
            hit_this_life: vec![false; n],
            last_fire: vec![None; n],
            first_hit: vec![None; n],
            ..Self::default()
        }
    }

    pub(super) fn set_human(&mut self, human: Option<Place>) {
        self.human = human;
    }

    fn grow(&mut self, n: usize) {
        if self.open.len() < n {
            self.target.resize(n, None);
            self.sights.resize(n, Vec::new());
            self.open.resize(n, None);
            self.hit_this_life.resize(n, false);
            self.last_fire.resize(n, None);
            self.first_hit.resize(n, None);
        }
    }

    fn close(&mut self, place: Place, died_at: Option<u32>) {
        let Some(hit) = self.open.get_mut(place).and_then(Option::take) else {
            return;
        };
        let by_human = usize::from(self.human == Some(hit.shooter));
        self.by_play[by_human].entry(hit.play).or_default().add(&hit, died_at);
    }

    /// Whether `place`'s own sight of `who` is up now, and since when.
    fn sight_of(&self, place: Place, who: Place) -> Option<u32> {
        self.sights.get(place)?.iter().find(|(w, _)| *w == who).map(|&(_, since)| since)
    }
}

impl Topic for UnderFireCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn read(&mut self, place: Place, bot: Option<&Bot>, _tick: u32) {
        self.grow(place + 1);
        self.target[place] = bot.and_then(|b| b.reflex.target());
        let sights = &mut self.sights[place];
        sights.clear();
        if let Some(bot) = bot {
            sights.extend(
                bot.memory
                    .tracks()
                    .iter()
                    .filter(|t| t.source == Source::Seen)
                    .filter_map(|t| t.sight_since.map(|since| (t.who, since))),
            );
        }
    }

    fn body(&mut self, run: &Run, at: &BodyAt<'_>, _rays: &mut u64) {
        let (rec, place, tick) = (at.rec, at.place, at.tick);
        self.grow(place + 1);
        if rec.alive && !at.prev.alive {
            self.hit_this_life[place] = false;
            self.first_hit[place] = None;
            self.last_fire[place] = None;
        }
        if !rec.alive || !run.is_bot.get(place).copied().unwrap_or(false) || self.human == Some(place) {
            return;
        }
        let firing = rec.keys & key::FIRE != 0;
        if firing {
            self.last_fire[place] = Some(tick);
        }
        let target = self.target[place];
        let sight = self.open[place].and_then(|h| self.sight_of(place, h.shooter));
        let Some(hit) = self.open[place].as_mut() else {
            return;
        };
        if tick <= hit.at {
            return;
        }
        if hit.seen_before.is_none() && hit.seen_after.is_none() {
            hit.seen_after = sight;
        }
        if hit.target_at.is_none() && target == Some(hit.shooter) {
            hit.target_at = Some(tick);
        }
        if firing {
            hit.fire_at.get_or_insert(tick);
            if target == Some(hit.shooter) {
                hit.fire_him_at.get_or_insert(tick);
            }
        }
        if hit.switch.is_none() && rec.play != hit.play {
            hit.switch = Some((tick, rec.play));
        }
    }

    fn end(&mut self, run: &Run, game: &mut Game, _sight: &mapdata::RayTree, _rays: &mut u64) {
        self.grow(game.bodies.len());
        for &(t, shooter, victim) in &run.hits[self.hits_seen.min(run.hits.len())..] {
            let (Some(v), Some(s)) = (game.bodies.get(victim), game.bodies.get(shooter)) else {
                continue;
            };
            if v.team == s.team
                || !run.is_bot.get(victim).copied().unwrap_or(false)
                || self.human == Some(victim)
                || self.hit_this_life[victim]
            {
                continue;
            }
            let (Some(vr), Some(sr)) = (run.recs[victim].last(), run.recs[shooter].last()) else {
                continue;
            };
            self.hit_this_life[victim] = true;
            self.first_hit[victim] = Some(t);
            let firing = vr.keys & key::FIRE != 0;
            let targets = self.target[victim] == Some(shooter);
            self.open[victim] = Some(Open {
                at: t,
                shooter,
                play: vr.play,
                band: band(vr.yaw, vr.feet, sr.feet),
                seen_before: self.sight_of(victim, shooter),
                seen_after: None,
                target_at: targets.then_some(t),
                fire_at: firing.then_some(t),
                fire_him_at: (firing && targets).then_some(t),
                switch: None,
            });
        }
        self.hits_seen = run.hits.len();
        let deaths: Vec<(u32, Place, Place)> = run.kills[self.kills_seen.min(run.kills.len())..].to_vec();
        self.kills_seen = run.kills.len();
        for (t, killer, victim) in deaths {
            if run.is_bot.get(victim).copied().unwrap_or(false) && self.human != Some(victim) {
                let play = run.recs[victim].last().map_or(0, |r| r.play);
                let by_human = usize::from(self.human == Some(killer));
                let d = self.deaths[by_human].entry(play).or_default();
                d.deaths += 1;
                d.fired_5s += u64::from(self.last_fire[victim].is_some_and(|f| t.saturating_sub(f) <= LAST_MS));
                d.first_hit_1s += u64::from(self.first_hit[victim].is_some_and(|h| t.saturating_sub(h) <= QUICK_MS));
            }
            self.close(victim, Some(t));
        }
        let now = game.tick;
        let stale: Vec<Place> =
            (0..self.open.len()).filter(|&p| self.open[p].is_some_and(|h| now.saturating_sub(h.at) > FOLLOW_MS)).collect();
        for place in stale {
            self.close(place, None);
        }
    }

    fn parts(&mut self, run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        let open: Vec<Place> = (0..self.open.len()).filter(|&p| self.open[p].is_some()).collect();
        for place in open {
            self.close(place, None);
        }
        let name = |play: u16| run.plays.get(usize::from(play)).cloned().unwrap_or_else(|| "none".to_owned());
        let mut lines = vec![format!(
            "owner-test under fire (A230: the first enemy hit of a bot's life by the play at the hit, followed to its death or \
             {FOLLOW_MS} ms: view bands up to {BANDS_DEG:?} deg; times in {BIN_MS} ms bins, the last never; each death by its play: \
             fired in the last {LAST_MS} ms, its first hit within {QUICK_MS} ms; the stand-in human's apart)"
        )];
        let mut json = serde_json::Map::new();
        for (who, (by_play, deaths)) in ["by_bot", "by_human"].iter().zip(self.by_play.iter().zip(&self.deaths)) {
            let mut hits = serde_json::Map::new();
            for (play, c) in by_play {
                let to: serde_json::Map<String, serde_json::Value> =
                    c.switched_to.iter().map(|(p, n)| (name(*p), serde_json::Value::from(*n))).collect();
                hits.insert(
                    name(*play),
                    serde_json::json!({
                        "hits": c.hits, "by_band": c.by_band, "seen_before": c.seen_before, "died_1s": c.died_1s, "died": c.died,
                        "died_same_play": c.died_same_play, "targeted": c.targeted, "fired": c.fired, "fired_him": c.fired_him,
                        "switched": c.switched, "switched_to": to, "to_target": c.to_target.to_vec(), "to_fire": c.to_fire.to_vec(),
                        "to_fire_him": c.to_fire_him.to_vec(), "to_switch": c.to_switch.to_vec(),
                        "sight_to_fire_him": c.sight_to_fire_him.to_vec(),
                    }),
                );
                lines.push(format!(
                    "  {who} {}: first hits {} {:?}, seen before {}, died within 1 s {}, died {} ({} in that play), targeted him {}, \
                     fired {}, at him {}, switched {}",
                    name(*play),
                    c.hits,
                    c.by_band,
                    c.seen_before,
                    c.died_1s,
                    c.died,
                    c.died_same_play,
                    c.targeted,
                    c.fired,
                    c.fired_him,
                    c.switched
                ));
            }
            let dead: serde_json::Map<String, serde_json::Value> = deaths
                .iter()
                .map(|(play, d)| (name(*play), serde_json::json!({"deaths": d.deaths, "fired_5s": d.fired_5s, "first_hit_1s": d.first_hit_1s})))
                .collect();
            lines.push(format!(
                "  {who} deaths by play: {}",
                deaths.iter().map(|(p, d)| format!("{} {} (fired 5 s {}, hit 1 s {})", name(*p), d.deaths, d.fired_5s, d.first_hit_1s)).collect::<Vec<_>>().join(", ")
            ));
            json.insert((*who).to_owned(), serde_json::json!({ "first_hits": hits, "deaths": dead }));
        }
        vec![("under_fire", Part::new(serde_json::json!({ "under_fire": json }), lines.join("\n")))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_falls_in_its_bin_and_never_in_the_last() {
        assert_eq!([bin(Some(0)), bin(Some(249)), bin(Some(250)), bin(Some(9_999))], [0, 0, 1, BINS - 2]);
        assert_eq!(bin(Some(FOLLOW_MS + 5_000)), BINS - 2, "late but in the window's last bin");
        assert_eq!(bin(None), BINS - 1);
    }

    #[test]
    fn a_first_hit_counts_its_answers_from_the_hit_and_its_death_in_the_same_play() {
        let hit = Open {
            at: 1_000,
            shooter: 7,
            play: 2,
            band: 1,
            seen_before: Some(400),
            seen_after: None,
            target_at: Some(1_300),
            fire_at: Some(1_600),
            fire_him_at: Some(1_600),
            switch: None,
        };
        let mut c = ByPlay::default();
        c.add(&hit, Some(1_800));
        assert_eq!((c.hits, c.died_1s, c.died, c.died_same_play, c.fired_him, c.switched), (1, 1, 1, 1, 1, 0));
        assert_eq!(c.to_target[1], 1, "300 ms after the hit");
        assert_eq!(c.to_fire_him[2], 1, "600 ms after the hit");
        assert_eq!(c.sight_to_fire_him[4], 1, "1.2 s from the first sight");
        assert_eq!(c.to_switch[BINS - 1], 1, "never switched");
        let switched = Open { switch: Some((3_500, 5)), seen_before: None, ..hit };
        c.add(&switched, None);
        assert_eq!((c.hits, c.died, c.died_same_play, c.switched, c.switched_to.get(&5)), (2, 1, 1, 1, Some(&1)));
        assert_eq!(c.to_switch[10], 1, "2.5 s after the hit");
    }

    #[test]
    fn the_view_band_off_the_shooter_wraps_and_ignores_height() {
        let at = Vec3::new(0.0, 0.0, 0.0);
        let yaw = kin::yaw_of([0.0, 1.0]);
        assert_eq!(band(yaw, at, Vec3::new(0.0, 4.0, 10.0)), 0);
        assert_eq!(band(yaw, at, Vec3::new(10.0, 0.0, 0.0)), 2);
        assert_eq!(band(yaw, at, Vec3::new(0.0, 0.0, -10.0)), 3);
    }
}
