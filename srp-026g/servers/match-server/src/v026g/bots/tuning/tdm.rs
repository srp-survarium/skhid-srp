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
    /// [v4] A lone mover does not walk into a known enemy: it holds short until a mate covers it ([`Approach`]).
    pub approach: Option<Approach>,
}

/// [A297 v4] The pre-contact pair. A moving bot (not holding an angle, not in a fight, not a carrier) whose own memory has an
/// enemy within `near_m` that it sensed itself or was told of within `fresh_s`, and that it does not see now, is "approaching"
/// him: the commander gives it an answerer as it gives a fight one (the nearest free mover, to a cover spot round it), and the
/// bot holds short, its view on his place, until that answerer stands within the cover ring off its line to him, or `hold_s`
/// passes; then both go. The v3 bench could not move the hunter (he kills inside the reaction window, A287: 65 % of his
/// victims dead within ~1 s), so the second bot must be set before contact. Holders keep their angles.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Approach {
    pub near_m: Metres,
    pub fresh_s: Positive,
    pub hold_s: Positive,
}

impl Default for Approach {
    fn default() -> Self {
        // near: his kills at p75 22 m and median 15.5 m (A287); fresh: the think notes' 5 s and a little; hold: a mate within
        // `answer_m` (30 m) runs ~6 m/s, so 3 s brings most; longer is a lone bot standing (GUESS).
        Self { near_m: Metres::of(30.0), fresh_s: Positive::of(8.0), hold_s: Positive::of(3.0) }
    }
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
            answer: Answer::NearestMover,
            dead_m: Metres::of(8.0),
            dead_s: Positive::of(20.0),
            linger_s: Positive::of(2.0),
            max_s: Positive::of(8.0),
            walk_m: Metres::of(10.0),
            approach: Some(Approach::default()),
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
    /// nearest pairs first (`group::answerers`, the commander's): 74 % of the owner's victims had a mate within 20 m (A287),
    /// mostly not the partner. Its bench (v2, 168 pairs vs the hunter): bot vs bot ON deaths -6 % (z -3.5), but the hunter died
    /// less vs ON (z -2.43): the covers pulled bots off the angles that kill him.
    Nearest,
    /// As `Nearest`, but a bot holding an angle (`group::holds`: Crossfire, HoldExit, Cut, Ambush, GuardLead) keeps it: only a
    /// moving bot answers, as squads split a holder and a mover (F.E.A.R.'s cover-while-advancing; Killzone's position picking).
    #[default]
    NearestMover,
}

/// [A297 item 2] Hold, not feed, when behind, in TDM only: the team behind on points (`MatchSignals::behind`, 0 level to 1
/// from 30 tickets down in TDM) weighs the angle holds that kill the human up and the long cut-offs down. Measured (PROVEN, 168 matches
/// vs the hunter stand-in, the tip's bots): his deaths per 10 bot-minutes of the killer's play were HoldExit 1.66, Crossfire
/// 1.24, Roam 0.72, Cut 0.42, Buddy 0.05; Cut held 32 % of the bots' time for 14 % of his deaths.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BehindHolds {
    /// Crossfire and HoldExit scores times `1 + hold_gain * behind`. GUESS (the bench decides).
    pub hold_gain: f32,
    /// Cut scores times `1 - cut_cut * behind`, `[0, 1]`. GUESS.
    pub cut_cut: f32,
}

impl Default for BehindHolds {
    fn default() -> Self {
        Self { hold_gain: 1.0, cut_cut: 0.75 }
    }
}

impl BehindHolds {
    /// The factor on a candidate of `kind` for a team `behind` (`[0, 1]`); `None` for the plays it leaves as they are.
    pub fn factor(&self, kind: PlayKind, behind: f32) -> Option<f32> {
        match kind {
            PlayKind::Crossfire | PlayKind::HoldExit => Some(1.0 + self.hold_gain * behind),
            PlayKind::Cut => Some(1.0 - self.cut_cut * behind),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TdmTuning {
    pub group: Option<GroupUp>,
    pub behind_holds: Option<BehindHolds>,
}

impl TdmTuning {
    pub(super) fn check(&self) -> Result<(), String> {
        if let Some(g) = self.group
            && (g.min_m.get() > g.max_m.get() || !(0.0..=90.0).contains(&g.bearing_min_deg) || g.linger_s.get() > g.max_s.get())
        {
            return Err("tdm.group".into());
        }
        if let Some(b) = self.behind_holds
            && (!(0.0..=4.0).contains(&b.hold_gain) || !(0.0..=1.0).contains(&b.cut_cut))
        {
            return Err("tdm.behind_holds".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Level: no change; behind: the holds up and Cut down by the share behind; other plays untouched.
    #[test]
    fn behind_holds_weighs_the_holds_up_and_cut_down_by_the_share_behind() {
        let rule = BehindHolds::default();
        assert_eq!(rule.factor(PlayKind::Crossfire, 0.0), Some(1.0));
        assert_eq!(rule.factor(PlayKind::HoldExit, 0.5), Some(1.5));
        assert_eq!(rule.factor(PlayKind::Cut, 1.0), Some(0.25));
        assert_eq!(rule.factor(PlayKind::Roam, 1.0), None);
        let bad = TdmTuning { behind_holds: Some(BehindHolds { hold_gain: 0.5, cut_cut: 1.5 }), ..Default::default() };
        assert!(bad.check().is_err());
        assert!(TdmTuning::default().behind_holds.is_none(), "off by default");
    }
}
