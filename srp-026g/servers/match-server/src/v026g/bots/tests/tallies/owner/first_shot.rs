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
}

impl FirstShotCounts {
    pub(super) fn new(n: usize) -> Self {
        Self { had: vec![false; n], ..Self::default() }
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
        )
    }
}

impl Topic for FirstShotCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn end(&mut self, run: &Run, game: &mut Game, _sight: &mapdata::RayTree, _rays: &mut u64) {
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
            let hit_k = t as usize / tick_ms;
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
            self.open.push(Contact { bot, start: (start_k * tick_ms) as u32, hit_first, fired_first, ready });
        }
        self.hits_seen = run.hits.len();
        let deaths: Vec<(u32, Place)> = run.kills[self.kills_seen.min(run.kills.len())..].iter().map(|&(t, _, v)| (t, v)).collect();
        self.kills_seen = run.kills.len();
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
        });
        vec![("first_shot", Part::new(json, self.line()))]
    }
}
