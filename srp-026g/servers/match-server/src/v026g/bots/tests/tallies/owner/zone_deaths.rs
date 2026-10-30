//! Who dies of the damage zones, and how the stretch in the zone began (register #27, zone entry): the spells of a match by the team
//! of the body, with the flags that tell the causes apart (`ZoneSpell`): a floor only a crouched body fits (the restored floors of
//! the exact erosion), the zone came on under the body, the zones never switch (no schedule to read), a zone the bots' marks miss, and
//! the motives of the bot over the spell (`Bot::bench_why`). Keys are `zd_<name>`, and `zd_by_team` with the same under "0" and "1",
//! so that a head-to-head run reads the ON team's counts as the ON team's.
//!
//! A death counts in the pool that ran out; a spell that ended otherwise (the body left, or an enemy killed it in the zone) is a spell.

use super::*;
use crate::v026g::bots::why;
use crate::v026g::bots::tests::{ZoneNote, ZoneSpell};

/// The motives of `Bot::bench_why`, by bit, as the keys name them.
const WHY: [(u16, &str); 11] = [
    (why::PLAIN, "plain"),
    (why::ESCAPE, "escape"),
    (why::CROSS, "cross"),
    (why::RING, "ring"),
    (why::NONE, "none"),
    (why::WALK_ON, "walk_on"),
    (why::LEGS, "legs"),
    (why::ZONE_OUT, "zone_out"),
    (why::ZONE_OK, "zone_ok"),
    (why::STRAIGHT, "straight"),
    (why::POCKET, "pocket"),
];

/// The names of the pools that kill, in the order of `ZoneNote::died`.
const POOLS: [&str; 4] = ["body", "head", "infection", "pain"];

#[derive(Default)]
pub(super) struct ZoneDeaths;

/// One team's counts.
#[derive(Default)]
struct Side {
    spells: u64,
    /// Health the zones took, in thousandths.
    lost_milli: u64,
    deaths: u64,
    deaths_by_pool: [u64; 4],
    /// Spells, and deaths among them, with each flag set.
    restored: [u64; 2],
    came_on: [u64; 2],
    fixed: [u64; 2],
    unmarked: [u64; 2],
    fight: [u64; 2],
    /// Spells and deaths over which a motive was on.
    why: [[u64; 2]; WHY.len()],
}

impl Side {
    fn add(&mut self, note: &ZoneNote) {
        let died = usize::from(note.died.is_some());
        self.spells += 1;
        self.lost_milli += (f64::from(note.spell.lost) * 1000.0).round() as u64;
        if let Some(pool) = note.died {
            self.deaths += 1;
            self.deaths_by_pool[usize::from(pool).min(3)] += 1;
        }
        let spell = &note.spell;
        for (flags, on) in [
            (&mut self.restored, spell.restored),
            (&mut self.came_on, spell.came_on),
            (&mut self.fixed, spell.fixed),
            (&mut self.unmarked, spell.unmarked),
            (&mut self.fight, note.fight),
        ] {
            if on {
                flags[0] += 1;
                flags[1] += died as u64;
            }
        }
        for (counts, (bit, _)) in self.why.iter_mut().zip(WHY) {
            if spell.why & bit != 0 {
                counts[0] += 1;
                counts[1] += died as u64;
            }
        }
    }

    /// The keys of one side: `<name>` is the spells with it, `<name>_deaths` the deaths among them.
    fn keyed(&self) -> serde_json::Value {
        let mut json = serde_json::json!({
            "zd_spells": self.spells,
            "zd_lost_milli": self.lost_milli,
            "zd_deaths": self.deaths,
        });
        for (pool, n) in POOLS.iter().zip(self.deaths_by_pool) {
            json[format!("zd_deaths_{pool}")] = n.into();
        }
        for (name, flags) in [
            ("restored", &self.restored),
            ("came_on", &self.came_on),
            ("fixed", &self.fixed),
            ("unmarked", &self.unmarked),
            ("fight", &self.fight),
        ] {
            json[format!("zd_{name}")] = flags[0].into();
            json[format!("zd_{name}_deaths")] = flags[1].into();
        }
        for ((_, name), counts) in WHY.iter().zip(&self.why) {
            json[format!("zd_why_{name}")] = counts[0].into();
            json[format!("zd_why_{name}_deaths")] = counts[1].into();
        }
        json
    }

    fn line(&self) -> String {
        format!(
            "{} spells, {} deaths (restored {}, came on {}, fixed {}, unmarked {}, fight {})",
            self.spells, self.deaths, self.restored[1], self.came_on[1], self.fixed[1], self.unmarked[1], self.fight[1]
        )
    }
}

impl Topic for ZoneDeaths {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn parts(&mut self, _run: &Run, game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        let mut sides = [Side::default(), Side::default()];
        let mut total = Side::default();
        // Spells under way when the match ends are spells too.
        let open = game.bodies.iter().filter_map(|b| b.zone_spell.map(|spell| ZoneNote::of(&spell, b, None)));
        for note in game.zone_notes.iter().copied().chain(open) {
            sides[usize::from(note.team.min(1))].add(&note);
            total.add(&note);
        }
        let mut json = total.keyed();
        json["zd_by_team"] = serde_json::json!({ "0": sides[0].keyed(), "1": sides[1].keyed() });
        let text = format!(
            "owner-test zone deaths: {}; team 0: {}; team 1: {}",
            total.line(),
            sides[0].line(),
            sides[1].line()
        );
        vec![("zone_deaths", Part::new(json, text))]
    }
}

/// A made-up spell: the flags, the motives and how it ended are filed under the body's team and counted once.
#[test]
pub(crate) fn a_zone_spell_is_filed_under_its_team_with_its_flags_and_motives() {
    let (mut game, mut tally) = made_up(&[0, 1]);
    let spell = |why: u16, restored: bool, came_on: bool, fixed: bool| ZoneSpell {
        start: 1000,
        health: 1.0,
        lost: 0.5,
        last: 2000,
        why,
        restored,
        unmarked: false,
        came_on,
        fixed,
    };
    let note = |team: u8, s: ZoneSpell, died: Option<u8>| ZoneNote { team, spell: s, died, fight: false };
    game.zone_notes.push(note(0, spell(why::CROSS, true, false, true), Some(0)));
    game.zone_notes.push(note(0, spell(why::PLAIN, false, true, false), None));
    game.zone_notes.push(note(1, spell(why::PLAIN | why::ZONE_OK, false, true, false), Some(2)));
    let (_, json) = tally.finish(&game);
    assert_eq!(json["zd_spells"], 3, "{json}");
    assert_eq!(json["zd_deaths"], 2, "{json}");
    assert_eq!(json["zd_by_team"]["0"]["zd_deaths"], 1);
    assert_eq!(json["zd_by_team"]["0"]["zd_restored_deaths"], 1);
    assert_eq!(json["zd_by_team"]["0"]["zd_fixed_deaths"], 1);
    assert_eq!(json["zd_by_team"]["0"]["zd_came_on"], 1);
    assert_eq!(json["zd_by_team"]["0"]["zd_came_on_deaths"], 0);
    assert_eq!(json["zd_by_team"]["0"]["zd_why_cross_deaths"], 1);
    assert_eq!(json["zd_by_team"]["1"]["zd_deaths_infection"], 1);
    assert_eq!(json["zd_by_team"]["1"]["zd_why_zone_ok_deaths"], 1);
    assert_eq!(json["zd_why_plain"], 2);
    assert_eq!(json["zd_lost_milli"], 1500);
}
