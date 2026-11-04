//! A297: team tactics in Team Deathmatch (`traces-20261028/fx-team-a297-design.md`). Each part is off by default until its
//! by-side bench against the hunter stand-in decides.
//!
//! Root cause of `group` (PROVEN on the owner's 8 matches of 2026-11-03): the bots stand near each other (74 % of his
//! victims had a mate within 20 m) but not where they see what their mate fights (19 % of those mates had a line to him at
//! the death; he was traded within 3 s after 6 % of his kills). The trade partner was paired by place order, a median 34 m
//! off at the fight start, and the trade walked onto the body's spot after the death.

use super::*;

/// [A297 item 1] Group up and trade, in TDM only: pairs by position, and the partner covers its mate's fight from a second
/// bearing before the mate dies (a trade with a cover goal; F.E.A.R.'s cover-while-advancing, Killzone's position picking
/// by line of fire to the threat). Never onto the spot a mate just died on, never in file behind the mate.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GroupUp {
    /// A pair is kept while both live and stand within this, m; else the commander pairs the team's bots again, nearest
    /// first. GUESS: the near ring of the cover goal (`answer_m`) plus a little.
    pub keep_m: Metres,
    /// The partner answers its mate's fight within this of the mate, m (the live humans' Breach comers stood within 35 m of
    /// the camper at his death, `fx-gear-breach-human-note.md`).
    pub answer_m: Metres,
    /// The cover spot lies this far from the mate, m: at least, at most (the humans' nearest pair at a camper's death: P25
    /// 7 m, median 12 m).
    pub min_m: Metres,
    pub max_m: Metres,
    /// Seen from the enemy, the cover spot's bearing differs from the mate's by this at least, degrees: a second angle, no
    /// file (84 % of the humans' pushes came from two or more 45-degree sectors).
    pub bearing_min_deg: f32,
    /// ... or lies this far to the side of the mate's line from the enemy, m (a far enemy sees a 15 m offset under 30 degrees;
    /// the bench's file guard is 6 m). GUESS.
    pub lateral_m: Metres,
    /// Who answers a mate's fight ([`Answer`]).
    pub answer: Answer,
    /// No cover spot within this of a teammate's death in the last `dead_s`, m: the lane that just killed is not re-entered.
    pub dead_m: Metres,
    pub dead_s: Positive,
    /// The cover lasts while the mate's fight lasts and this long after, s; and this long at most in all.
    pub linger_s: Positive,
    pub max_s: Positive,
    /// It runs to the spot until this near, then walks with the sights on the enemy's known place, m.
    pub walk_m: Metres,
}

impl Default for GroupUp {
    fn default() -> Self {
        Self {
            keep_m: Metres::of(35.0),
            answer_m: Metres::of(30.0),
            min_m: Metres::of(6.0),
            max_m: Metres::of(15.0),
            bearing_min_deg: 30.0,
            lateral_m: Metres::of(5.0),
            answer: Answer::Nearest,
            dead_m: Metres::of(8.0),
            dead_s: Positive::of(20.0),
            linger_s: Positive::of(2.0),
            max_s: Positive::of(8.0),
            walk_m: Metres::of(10.0),
        }
    }
}

/// Who answers a mate's fight.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Answer {
    /// Its partner only (pairs by position), within `answer_m`. The first bench (168 pairs vs the hunter, half read): the
    /// partner was over `answer_m` off in 14,645 polls against 1,595 covers taken; pairs drift apart on their own plays.
    Partner,
    /// The nearest free bot of the team within `answer_m` (not in a fight itself, not a carrier), each fight one answerer, the
    /// nearest pairs first (`group::answerers`): 74 % of the owner's victims had a mate within 20 m (A287), mostly not the partner.
    #[default]
    Nearest,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TdmTuning {
    pub group: Option<GroupUp>,
}

impl TdmTuning {
    pub(super) fn check(&self) -> Result<(), String> {
        if let Some(g) = self.group
            && (g.min_m.get() > g.max_m.get() || !(0.0..=90.0).contains(&g.bearing_min_deg) || g.linger_s.get() > g.max_s.get())
        {
            return Err("tdm.group".into());
        }
        Ok(())
    }
}
