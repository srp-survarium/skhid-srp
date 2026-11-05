//! A302: who shoots first in a contact between a bot and the stand-in human. On the farming camper bench none of the four A302
//! fixes beat him, and the shared cause was LIKELY the first shot: he kills inside the bots' notice and reaction
//! (`traces-20261028/fx-fights-a302-first-shot-plan.md`). This tally measures it per contact.
//!
//! A **contact** is the first hit between the human and an enemy bot in that bot's life, either way. In the window before it
//! ([`WINDOW_MS`]):
//! - each side's first **aimed shot** at the other: the fire key down with the view within [`AIMED_DEG`] of the bearing to the
//!   other;
//! - **who fired first** (by those shots) and **who hit first** (the contact's own hit);
//! - whether the bot's view was on him ([`READY_DEG`]) at the contact's start (the first aimed shot of either, else the hit);
//! - the outcome within [`OUTCOME_MS`] of the start: the bot dead, the human dead, or neither.
//!
//! The same counts can be read off the live recordings (human against human, `fx-fights-a302-firstshot-baseline.md`).
//!
//! And **each of his kills** of a bot (the owner, 2026-11-05: "they keep crossing lines near me without even watching at me while
//! they should have known I'm there and at least throw grenade or smoke and flank/push"):
//! - did the victim's **team know his spot** (one of its held lines with him its shooter covers the victim's feet);
//! - did the **victim** know him (its own or its team's senses placed him within [`KNOWN_MS`], `Track::own`);
//! - was it **crossing** his lane (walking in it at his first hit, or at the death when he did not hit it first);
//! - what it **did** in the [`OUTCOME_MS`] before its death: looked at him ([`READY_DEG`]), fired aimed at him, threw a grenade,
//!   threw a smoke, or none of these.

use super::*;
use crate::v026g::bots::kin;

/// The window before a contact's first hit in which the aimed shots are looked for, ms.
const WINDOW_MS: u32 = 3_000;
/// A shot is aimed at the other within this of the bearing to him, degrees.
const AIMED_DEG: f32 = 10.0;
/// The bot's view is on him within this, degrees.
const READY_DEG: f32 = 15.0;
/// The outcome is read this long after the contact's start, ms.
const OUTCOME_MS: u32 = 5_000;
/// A place the senses gave this recently counts as known, ms (as `look.rs`).
const KNOWN_MS: u32 = 10_000;
/// A walk: over this, m/s, in a step with a goal (as `look.rs`).
const WALK_MPS: f32 = 0.5;

/// His kills of bots, by what the victim's team and the victim knew, whether it crossed his lane, and what it did.
#[derive(Clone, Copy, Debug, Default)]
struct Kills {
    kills: u64,
    team_knew: u64,
    victim_knew: u64,
    walking: u64,
    crossing: u64,
    crossing_team_knew: u64,
    looked: u64,
    fired: u64,
    grenade: u64,
    smoke: u64,
    nothing: u64,
}

/// The angle between the view's yaw and the bearing from `from` to `to`, degrees, 0 to 180.
fn off_deg(yaw: f32, from: Vec3, to: Vec3) -> f32 {
    let bearing = kin::yaw_of([to.x - from.x, to.z - from.z]);
    let d = (yaw - bearing + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    d.abs().to_degrees()
}

/// Who did something first in a contact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum First {
    Bot,
    Human,
}

/// One contact, open until its outcome is read.
#[derive(Clone, Copy, Debug)]
struct Contact {
    bot: Place,
    start: u32,
    hit_first: First,
    /// By aimed shots in the window; none if neither shot aimed.
    fired_first: Option<First>,
    ready: bool,
}

#[derive(Default)]
pub(super) struct FirstShotCounts {
    human: Option<Place>,
    hits_seen: usize,
    kills_seen: usize,
    /// Whether each bot has had its contact in this life (reset at its death).
    had: Vec<bool>,
    open: Vec<Contact>,
    contacts: u64,
    bot_hit_first: u64,
    bot_fired_first: u64,
    human_fired_first: u64,
    ready: u64,
    /// Of the contacts with the bot ready (its view on him): the bot fired first.
    ready_bot_fired_first: u64,
    bot_died: u64,
    human_died: u64,
    /// The bot fired first and died in the contact anyway.
    bot_first_and_died: u64,
    /// [fatal exchanges] He fired first and the bot died; the bot fired first and he died.
    human_first_and_bot_died: u64,
    bot_first_and_human_died: u64,
    /// Each team's held lines at the last commander picture read (as `look.rs`).
    held: [Vec<crate::v026g::bots::threat::HeldLine>; 2],
    /// The enemies each bot's own or its team's senses placed within [`KNOWN_MS`], at its last read.
    placed: Vec<Vec<Place>>,
    /// Each place's last grenade and smoke throw, ms.
    grenade_at: Vec<Option<u32>>,
    smoke_at: Vec<Option<u32>>,
    /// Each bot's tick of his first hit on it in this life (for "crossing at his first hit").
    hit_at: Vec<Option<u32>>,
    his_kills: Kills,
    /// [A302 `threat.check_spot`] The checks of known lines by team since the match began (`reflex::check_tally`: begun, then
    /// ended by fight, hit, crossed, cap), the counter at the match's start, and each team's bots' deaths and bots (for checks
    /// a life). Counted with or without a stand-in human.
    checks: [[u64; 5]; 2],
    checks_from: Option<[[u64; 5]; 2]>,
    team_deaths: [u64; 2],
    team_bots: [u64; 2],
    deaths_seen: usize,
}

impl FirstShotCounts {
    pub(super) fn new(n: usize) -> Self {
        Self {
            had: vec![false; n],
            placed: vec![Vec::new(); n],
            grenade_at: vec![None; n],
            smoke_at: vec![None; n],
            hit_at: vec![None; n],
            ..Self::default()
        }
    }

    pub(super) fn set_human(&mut self, human: Option<Place>) {
        self.human = human;
    }

    /// The first tick in `from..=to` at which `shooter` fired aimed at `at` (each tick's own feet), ms.
    fn aimed_shot(run: &Run, shooter: Place, other: Place, from: usize, to: usize) -> Option<usize> {
        let (s, o) = (run.recs.get(shooter)?, run.recs.get(other)?);
        (from..=to.min(s.len().saturating_sub(1))).find(|&k| {
            let (Some(r), Some(t)) = (s.get(k), o.get(k)) else {
                return false;
            };
            r.alive && r.keys & key::FIRE != 0 && off_deg(r.yaw, r.feet, t.feet) <= AIMED_DEG
        })
    }

    fn close(&mut self, c: Contact, bot_died: bool, human_died: bool) {
        self.contacts += 1;
        self.bot_hit_first += u64::from(c.hit_first == First::Bot);
        self.bot_fired_first += u64::from(c.fired_first == Some(First::Bot));
        self.human_fired_first += u64::from(c.fired_first == Some(First::Human));
        self.ready += u64::from(c.ready);
        self.ready_bot_fired_first += u64::from(c.ready && c.fired_first == Some(First::Bot));
        self.bot_died += u64::from(bot_died);
        self.human_died += u64::from(human_died);
        self.bot_first_and_died += u64::from(bot_died && c.fired_first == Some(First::Bot));
        self.human_first_and_bot_died += u64::from(bot_died && c.fired_first == Some(First::Human));
        self.bot_first_and_human_died += u64::from(human_died && c.fired_first == Some(First::Bot));
    }

    /// One of his kills: what the victim's team and the victim knew, whether it crossed his lane, and what it did.
    fn his_kill(&mut self, run: &Run, game: &Game, human: Place, victim: Place, t: u32) {
        let (Some(recs), Some(his)) = (run.recs.get(victim), run.recs.get(human)) else {
            return;
        };
        let end = run.index(t).min(recs.len().saturating_sub(1));
        let Some(at_death) = recs.get(end) else {
            return;
        };
        let team = usize::from(game.bodies[victim].team).min(1);
        let lane = |feet: Vec3| self.held[team].iter().any(|l| l.shooter == human && l.covers(feet));
        let team_knew = lane(at_death.feet);
        let victim_knew = self.placed.get(victim).is_some_and(|p| p.contains(&human));
        // Crossing: walking in his lane at his first hit on it (or at the death if he hit it first only then).
        let at_hit = self.hit_at.get(victim).copied().flatten().map_or(end, |h| run.index(h).min(end));
        let walking = recs.get(at_hit).is_some_and(|r| r.travel && r.speed > WALK_MPS);
        let crossing = walking && recs.get(at_hit).is_some_and(|r| lane(r.feet));
        let from = end.saturating_sub(OUTCOME_MS as usize / TICK_MS as usize);
        let (mut looked, mut fired) = (false, false);
        for k in from..=end {
            let (Some(r), Some(h)) = (recs.get(k), his.get(k)) else {
                continue;
            };
            let off = off_deg(r.yaw, r.feet, h.feet);
            looked |= r.alive && off <= READY_DEG;
            fired |= r.alive && r.keys & key::FIRE != 0 && off <= AIMED_DEG;
        }
        let since = |at: Option<u32>| at.is_some_and(|a| t.saturating_sub(a) <= OUTCOME_MS);
        let grenade = since(self.grenade_at.get(victim).copied().flatten());
        let smoke = since(self.smoke_at.get(victim).copied().flatten());
        let k = &mut self.his_kills;
        k.kills += 1;
        k.team_knew += u64::from(team_knew);
        k.victim_knew += u64::from(victim_knew);
        k.walking += u64::from(walking);
        k.crossing += u64::from(crossing);
        k.crossing_team_knew += u64::from(crossing && team_knew);
        k.looked += u64::from(looked);
        k.fired += u64::from(fired);
        k.grenade += u64::from(grenade);
        k.smoke += u64::from(smoke);
        k.nothing += u64::from(!(looked || fired || grenade || smoke));
    }

    fn kills_line(&self) -> String {
        let k = &self.his_kills;
        let pct = |n: u64| 100.0 * n as f64 / k.kills.max(1) as f64;
        format!(
            "  his kills of bots (A302): {}; the victim's team knew his spot {} ({:.1} %), the victim knew him {} ({:.1} %); walking at his first hit {} ({:.1} %), crossing his lane {} ({:.1} %, with the team knowing {}); in the last {} s it looked at him {} ({:.1} %), fired aimed at him {} ({:.1} %), threw a grenade {}, a smoke {}, none of these {} ({:.1} %)",
            k.kills,
            k.team_knew,
            pct(k.team_knew),
            k.victim_knew,
            pct(k.victim_knew),
            k.walking,
            pct(k.walking),
            k.crossing,
            pct(k.crossing),
            k.crossing_team_knew,
            OUTCOME_MS / 1000,
            k.looked,
            pct(k.looked),
            k.fired,
            pct(k.fired),
            k.grenade,
            k.smoke,
            k.nothing,
            pct(k.nothing),
        )
    }

    fn line(&self) -> String {
        let pct = |n: u64| 100.0 * n as f64 / self.contacts.max(1) as f64;
        format!(
            "  first shot (A302) bot vs the stand-in human: contacts {}; the bot hit first {} ({:.1} %), fired first {} ({:.1} %), he fired first {} ({:.1} %); the bot ready (view on him) {} ({:.1} %), of them fired first {}; in {} s the bot died {} ({:.1} %), he died {} ({:.1} %); the bot fired first and died {}",
            self.contacts,
            self.bot_hit_first,
            pct(self.bot_hit_first),
            self.bot_fired_first,
            pct(self.bot_fired_first),
            self.human_fired_first,
            pct(self.human_fired_first),
            self.ready,
            pct(self.ready),
            self.ready_bot_fired_first,
            OUTCOME_MS / 1000,
            self.bot_died,
            pct(self.bot_died),
            self.human_died,
            pct(self.human_died),
            self.bot_first_and_died,
        ) + &self.fatal_line()
            + &self.checks_line()
    }

    /// [A302 `threat.check_spot`] The checks a team's bots began, how they ended, and per bot life (its bots plus their deaths).
    fn checks_line(&self) -> String {
        let team = |t: usize| {
            let c = self.checks[t];
            let lives = self.team_bots[t] + self.team_deaths[t];
            format!(
                "team {t}: checks {} (ended fight {} hit {} crossed {} cap {}), bot lives {lives}, a life {:.2}",
                c[0],
                c[1],
                c[2],
                c[3],
                c[4],
                c[0] as f64 / lives.max(1) as f64
            )
        };
        format!("\n  check_spot (A302): {}; {}", team(0), team(1))
    }

    /// [fatal exchanges] The same contacts counted as the live baseline counts them (`fx-fights-a302-firstshot-baseline.md`:
    /// only exchanges that ended in a death within the window, by the killer): the share the killer fired first in. Every
    /// contact (the line above) and fatal exchanges only (this line) are two ways of counting; they are not comparable.
    fn fatal_line(&self) -> String {
        let pct = |n: u64, of: u64| 100.0 * n as f64 / of.max(1) as f64;
        format!(
            "
  first shot (A302) fatal exchanges only (the live baseline's count): the bot died {}, he fired first {} ({:.1} %); he died {}, the bot fired first {} ({:.1} %)",
            self.bot_died,
            self.human_first_and_bot_died,
            pct(self.human_first_and_bot_died, self.bot_died),
            self.human_died,
            self.bot_first_and_human_died,
            pct(self.bot_first_and_human_died, self.human_died),
        )
    }
}

impl Topic for FirstShotCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn read(&mut self, place: Place, bot: Option<&Bot>, tick: u32) {
        if place >= self.placed.len() {
            self.placed.resize(place + 1, Vec::new());
        }
        let placed = &mut self.placed[place];
        placed.clear();
        if let Some(bot) = bot {
            placed.extend(
                bot.memory.tracks().iter().filter(|t| t.own.is_some_and(|(_, when)| tick.wrapping_sub(when) <= KNOWN_MS)).map(|t| t.who),
            );
        }
    }

    fn bots(&mut self, bots: &Bots, _game: &Game) {
        for (team, held) in self.held.iter_mut().enumerate() {
            *held = bots.teams.get(team).map(|state| state.knowledge.threat.held.clone()).unwrap_or_default();
        }
    }

    fn note(&mut self, _run: &Run, note: &Note, kind: &NoteKind<'_>, _game: &Game) {
        let slot = if matches!(kind, NoteKind::Thrown { .. }) {
            &mut self.grenade_at
        } else if note.text.starts_with("throws a smoke") {
            &mut self.smoke_at
        } else {
            return;
        };
        if note.place >= slot.len() {
            slot.resize(note.place + 1, None);
        }
        slot[note.place] = Some(note.time);
    }

    fn end(&mut self, run: &Run, game: &mut Game, _sight: &mapdata::RayTree, _rays: &mut u64) {
        // [A302 `threat.check_spot`] The checks, and the bots' lives they are counted over.
        let counted = crate::v026g::bots::reflex::check_tally::counted();
        let from = *self.checks_from.get_or_insert(counted);
        for (t, team) in self.checks.iter_mut().enumerate() {
            for (k, n) in team.iter_mut().enumerate() {
                *n = counted[t][k] - from[t][k];
            }
        }
        if self.team_bots == [0, 0] {
            for (p, b) in game.bodies.iter().enumerate() {
                if run.is_bot.get(p).copied().unwrap_or(false) {
                    self.team_bots[usize::from(b.team).min(1)] += 1;
                }
            }
        }
        for &(_, _, victim) in &run.kills[self.deaths_seen.min(run.kills.len())..] {
            if run.is_bot.get(victim).copied().unwrap_or(false)
                && let Some(b) = game.bodies.get(victim)
            {
                self.team_deaths[usize::from(b.team).min(1)] += 1;
            }
        }
        self.deaths_seen = run.kills.len();
        let Some(human) = self.human else {
            return;
        };
        let tick_ms = TICK_MS as usize;
        for &(t, shooter, victim) in &run.hits[self.hits_seen.min(run.hits.len())..] {
            let bot = if shooter == human { victim } else if victim == human { shooter } else { continue };
            let (Some(b), Some(h)) = (game.bodies.get(bot), game.bodies.get(human)) else {
                continue;
            };
            if b.team == h.team || !run.is_bot.get(bot).copied().unwrap_or(false) || self.had.get(bot).copied().unwrap_or(true) {
                continue;
            }
            self.had[bot] = true;
            let hit_k = run.index(t);
            let from = hit_k.saturating_sub(WINDOW_MS as usize / tick_ms);
            let bot_shot = Self::aimed_shot(run, bot, human, from, hit_k);
            let human_shot = Self::aimed_shot(run, human, bot, from, hit_k);
            let fired_first = match (bot_shot, human_shot) {
                (Some(b), Some(h)) => Some(if b < h { First::Bot } else { First::Human }),
                (Some(_), None) => Some(First::Bot),
                (None, Some(_)) => Some(First::Human),
                (None, None) => None,
            };
            let start_k = bot_shot.into_iter().chain(human_shot).min().unwrap_or(hit_k);
            let ready = match (run.recs[bot].get(start_k), run.recs[human].get(start_k)) {
                (Some(br), Some(hr)) => off_deg(br.yaw, br.feet, hr.feet) <= READY_DEG,
                _ => false,
            };
            let hit_first = if shooter == bot { First::Bot } else { First::Human };
            if hit_first == First::Human {
                self.hit_at[bot] = Some(t);
            }
            self.open.push(Contact { bot, start: run.time_of(start_k), hit_first, fired_first, ready });
        }
        self.hits_seen = run.hits.len();
        let new_kills: Vec<(u32, Place, Place)> = run.kills[self.kills_seen.min(run.kills.len())..].to_vec();
        self.kills_seen = run.kills.len();
        for &(t, killer, victim) in &new_kills {
            if killer == human && victim != human && run.is_bot.get(victim).copied().unwrap_or(false) {
                self.his_kill(run, game, human, victim, t);
            }
        }
        let deaths: Vec<(u32, Place)> = new_kills.iter().map(|&(t, _, v)| (t, v)).collect();
        for (t, victim) in deaths {
            let mut i = 0;
            while i < self.open.len() {
                let c = self.open[i];
                let within = t.saturating_sub(c.start) <= OUTCOME_MS;
                if within && (victim == c.bot || victim == human) {
                    self.open.remove(i);
                    self.close(c, victim == c.bot, victim == human);
                    continue;
                }
                i += 1;
            }
            if let Some(had) = self.had.get_mut(victim) {
                *had = false;
            }
            if let Some(at) = self.hit_at.get_mut(victim) {
                *at = None;
            }
        }
        // A contact past its outcome window with nobody dead: neither.
        let now = game.tick;
        let (done, open): (Vec<Contact>, Vec<Contact>) = self.open.drain(..).partition(|c| now.saturating_sub(c.start) > OUTCOME_MS);
        self.open = open;
        for c in done {
            self.close(c, false, false);
        }
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        for c in std::mem::take(&mut self.open) {
            self.close(c, false, false);
        }
        let json = serde_json::json!({
            "contacts": self.contacts,
            "bot_hit_first": self.bot_hit_first,
            "bot_fired_first": self.bot_fired_first,
            "human_fired_first": self.human_fired_first,
            "ready": self.ready,
            "ready_bot_fired_first": self.ready_bot_fired_first,
            "bot_died": self.bot_died,
            "human_died": self.human_died,
            "bot_first_and_died": self.bot_first_and_died,
            "human_first_and_bot_died": self.human_first_and_bot_died,
            "bot_first_and_human_died": self.bot_first_and_human_died,
            "check_spot": { "checks": self.checks, "team_bots": self.team_bots, "team_deaths": self.team_deaths },
            "his_kills": {
                "kills": self.his_kills.kills, "team_knew": self.his_kills.team_knew, "victim_knew": self.his_kills.victim_knew,
                "walking": self.his_kills.walking, "crossing": self.his_kills.crossing, "crossing_team_knew": self.his_kills.crossing_team_knew,
                "looked": self.his_kills.looked, "fired": self.his_kills.fired, "grenade": self.his_kills.grenade,
                "smoke": self.his_kills.smoke, "nothing": self.his_kills.nothing,
            },
        });
        vec![("first_shot", Part::new(json, format!("{}
{}", self.line(), self.kills_line())))]
    }
}
