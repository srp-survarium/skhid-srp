//! The owner counts of the crouch flicker (register C5, 17.7; `fact-crouch-flicker.md`): how often a body toggles the crouch key in a fight,
//! and how long its spells of one stance last, on the definition the human recordings were read by (`scripts\crouch\crouch_spells.py`):
//!
//! - a tick is *alive* while the body lives; a *fight tick* is an alive tick within [`NEAR_TICKS`] (1 s) of a tick with FIRE held;
//! - a *toggle* is a tick whose CROUCH differs from the one before (both alive, the same life);
//! - a *spell* is a run of ticks with one CROUCH value inside one life; only complete ones count (not the first of a life, not the one cut
//!   by a death or the end of the match); its context is that of its first tick.
//!
//! Counts only: nothing decides by them. The tick the counts are made of is [`NEAR_TICKS`] ticks behind the game's, so that a fight
//! tick can look [`NEAR_TICKS`] ticks ahead as well as back.
//!
//! Keys of the `owner` object `crouch`: `alive_ticks`, `fight_ticks`, `toggles` (`fight`, `out`), `spells` (`stand` / `crouch` ->
//! `fight` / `out` -> `all`, `short` (standing: up to 2 ticks, 0.1 s; crouched: up to 10 ticks, 0.5 s), `six` (exactly 6 ticks, 0.3 s: the
//! length of the sight-lapse clock)), `fire_ticks` (`all`, `crouched`), and the rates `toggles_per_fight_min`, `stand_blips_per_fight_min`,
//! `crouch_short_per_fight_min`, `six_tick_share_pct` and `fire_crouched_pct`.
//!
//! The humans' figures (402 live matches, the best players, `fact-crouch-flicker.md`): 9.6 toggles a fight minute (q10-q90 5.8-13.7), 0.11
//! standing spells of 0.1 s or less a fight minute, 4.6 % of the in-fight crouch spells exactly 6 ticks, about 19 % of the fire ticks
//! crouched. The bots before the sight fix: 25.1, 1.39, 21.6 %.

use std::collections::VecDeque;

use super::*;

/// One second in ticks: how far a fight tick looks for a tick with FIRE held.
const NEAR_TICKS: usize = 20;
/// A standing spell this short or shorter is a blip (0.1 s); a crouch spell this short or shorter is short (0.5 s); the sight-lapse
/// clock's length (0.3 s).
const BLIP_TICKS: u32 = 2;
const SHORT_TICKS: u32 = 10;
const SIX_TICKS: u32 = 6;

/// One body's tick.
#[derive(Clone, Copy)]
struct Seen {
    alive: bool,
    crouch: bool,
    fire: bool,
}

/// The spell a body is in.
#[derive(Clone, Copy)]
struct Spell {
    crouched: bool,
    ticks: u32,
    /// It began in a fight.
    fight: bool,
    /// It began with a toggle of this life (so that its length is whole), not with the life.
    follows_toggle: bool,
}

/// A body's stance: not alive, or in a spell.
#[derive(Clone, Copy)]
enum Stance {
    Dead,
    In(Spell),
}

/// The counts of one stance in one context (a fight or not): spells whole, short, and six ticks long.
#[derive(Clone, Copy, Default)]
struct Spells {
    all: u32,
    short: u32,
    six: u32,
}

#[derive(Default)]
struct Totals {
    alive_ticks: u32,
    fight_ticks: u32,
    /// In a fight, out of one.
    toggles: [u32; 2],
    /// By stance (stand, crouch) and context (fight, out).
    spells: [[Spells; 2]; 2],
    fire_ticks: u32,
    fire_crouched: u32,
}

pub(super) struct CrouchCounts {
    /// Each body's last ticks, the oldest first: [`NEAR_TICKS`] behind, [`NEAR_TICKS`] ahead of the one being counted.
    rings: Vec<VecDeque<Seen>>,
    stance: Vec<Stance>,
    totals: Totals,
}

impl CrouchCounts {
    pub(super) fn new(n: usize) -> Self {
        Self { rings: vec![VecDeque::new(); n], stance: vec![Stance::Dead; n], totals: Totals::default() }
    }

    /// A body's tick: the tick [`NEAR_TICKS`] back is counted once the ring holds the ticks after it.
    fn tick(&mut self, place: usize, seen: Seen) {
        let ring = &mut self.rings[place];
        ring.push_back(seen);
        if ring.len() <= 2 * NEAR_TICKS {
            return;
        }
        let fight = ring.iter().any(|s| s.fire);
        let centre = ring[NEAR_TICKS];
        ring.pop_front();
        self.count(place, centre, fight);
    }

    /// The tick `centre`, which is `fight` or not.
    fn count(&mut self, place: usize, centre: Seen, fight: bool) {
        if !centre.alive {
            self.stance[place] = Stance::Dead;
            return;
        }
        let t = &mut self.totals;
        t.alive_ticks += 1;
        t.fight_ticks += u32::from(fight);
        if centre.fire {
            t.fire_ticks += 1;
            t.fire_crouched += u32::from(centre.crouch);
        }
        self.stance[place] = match self.stance[place] {
            Stance::In(mut spell) if spell.crouched == centre.crouch => {
                spell.ticks += 1;
                Stance::In(spell)
            }
            Stance::In(ended) => {
                t.toggles[usize::from(!fight)] += 1;
                if ended.follows_toggle {
                    let cell = &mut t.spells[usize::from(ended.crouched)][usize::from(!ended.fight)];
                    cell.all += 1;
                    let short = if ended.crouched { SHORT_TICKS } else { BLIP_TICKS };
                    cell.short += u32::from(ended.ticks <= short);
                    cell.six += u32::from(ended.ticks == SIX_TICKS);
                }
                Stance::In(Spell { crouched: centre.crouch, ticks: 1, fight, follows_toggle: true })
            }
            Stance::Dead => Stance::In(Spell { crouched: centre.crouch, ticks: 1, fight, follows_toggle: false }),
        };
    }

    fn part(&self) -> Part {
        let t = &self.totals;
        let fight_min = f64::from(t.fight_ticks) * f64::from(TICK_S) / 60.0;
        let per_min = |n: u32| if fight_min > 0.0 { (10.0 * f64::from(n) / fight_min).round() / 10.0 } else { 0.0 };
        let pct = |n: u32, of: u32| if of > 0 { (1000.0 * f64::from(n) / f64::from(of)).round() / 10.0 } else { 0.0 };
        let spells = |cell: Spells| serde_json::json!({ "all": cell.all, "short": cell.short, "six": cell.six });
        let [stand, crouch] = &t.spells;
        let toggles_per_min = per_min(t.toggles[0]);
        let blips = per_min(stand[0].short);
        let short = per_min(crouch[0].short);
        let six = pct(crouch[0].six, crouch[0].all);
        let fire = pct(t.fire_crouched, t.fire_ticks);
        let json = serde_json::json!({ "crouch": {
            "alive_ticks": t.alive_ticks,
            "fight_ticks": t.fight_ticks,
            "toggles": { "fight": t.toggles[0], "out": t.toggles[1] },
            "spells": {
                "stand": { "fight": spells(stand[0]), "out": spells(stand[1]) },
                "crouch": { "fight": spells(crouch[0]), "out": spells(crouch[1]) },
            },
            "fire_ticks": { "all": t.fire_ticks, "crouched": t.fire_crouched },
            "toggles_per_fight_min": toggles_per_min,
            "stand_blips_per_fight_min": blips,
            "crouch_short_per_fight_min": short,
            "six_tick_share_pct": six,
            "fire_crouched_pct": fire,
        } });
        let text = format!(
            "owner-test crouch: in a fight {toggles_per_min} toggles a fight minute (humans 9.6, q10-q90 5.8-13.7), {blips} standing blips of 0.1 s or less a fight minute (humans 0.11), {short} crouch spells of 0.5 s or less (humans: 3.5 or fewer), {six} % of the in-fight crouch spells exactly 6 ticks (humans 4.6 %), fire ticks crouched {fire} % (humans about 19 %); {} fight ticks of {} alive",
            t.fight_ticks, t.alive_ticks
        );
        Part::new(json, text)
    }
}

impl Topic for CrouchCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn body(&mut self, _run: &Run, at: &BodyAt<'_>, _rays: &mut u64) {
        if at.place >= self.rings.len() {
            return;
        }
        let body = &at.game.bodies[at.place];
        self.tick(
            at.place,
            Seen { alive: body.alive, crouch: body.keys & key::CROUCH != 0, fire: body.alive && body.keys & key::FIRE != 0 },
        );
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("crouch", self.part())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feeds one body the (crouch, fire) ticks, alive, then padding that gives the last of them its ticks ahead and its place in the ring.
    fn feed(topic: &mut CrouchCounts, ticks: &[(bool, bool)]) {
        for &(crouch, fire) in ticks.iter().chain(&[(false, false); 2 * NEAR_TICKS]) {
            topic.tick(0, Seen { alive: true, crouch, fire });
        }
    }

    /// Crouch for 6 ticks and stand for 1, three times, then stand for 30 and crouch again, all with FIRE held: in a fight, the flicker
    /// counts as toggles, whole spells, standing blips and six-tick crouch spells; the first spell of the life is not whole.
    #[test]
    fn a_flicker_in_a_fight_counts_toggles_blips_and_six_tick_spells() {
        let mut topic = CrouchCounts::new(1);
        let mut ticks = vec![(false, true); 10]; // the first standing spell of the life (not whole)
        for _ in 0..3 {
            ticks.extend([(true, true); 6]);
            ticks.push((false, true));
        }
        ticks.extend([(false, true); 29]); // with the blip before it, 30 standing ticks
        ticks.push((true, true));
        feed(&mut topic, &ticks);
        let t = &topic.totals;
        assert_eq!(t.toggles, [7, 0], "in and out three times, and in again");
        let [stand, crouch] = &t.spells;
        assert_eq!((crouch[0].all, crouch[0].six, crouch[0].short), (3, 3, 3), "three whole crouch spells of 6 ticks");
        assert_eq!((stand[0].all, stand[0].short), (3, 2), "three whole standing spells: two blips of 1 tick, then the long one");
        assert_eq!((t.fire_ticks, t.fire_crouched), (ticks.len() as u32, 19));
    }

    /// Out of a fight (no FIRE within a second) the toggles are counted apart, and a death cuts the spell without counting it.
    #[test]
    fn toggles_out_of_a_fight_are_apart_and_a_death_cuts_the_spell() {
        let mut topic = CrouchCounts::new(1);
        let mut ticks = vec![(false, false); 30];
        ticks.extend([(true, false); 30]);
        ticks.extend([(false, false); 30]);
        feed(&mut topic, &ticks);
        assert_eq!(topic.totals.toggles, [0, 2]);
        assert_eq!(topic.totals.spells[1][1].all, 1, "the one whole crouch spell, out of a fight");
        assert_eq!(topic.totals.fight_ticks, 0);
        let mut topic = CrouchCounts::new(1);
        for _ in 0..30 {
            topic.tick(0, Seen { alive: true, crouch: true, fire: false });
        }
        for _ in 0..(2 * NEAR_TICKS + 1) {
            topic.tick(0, Seen { alive: false, crouch: false, fire: false });
        }
        for _ in 0..(3 * NEAR_TICKS) {
            topic.tick(0, Seen { alive: true, crouch: false, fire: false });
        }
        assert_eq!(topic.totals.toggles, [0, 0]);
        assert_eq!(topic.totals.spells[1][1].all, 0, "the crouch spell cut by the death is not whole");
    }
}
