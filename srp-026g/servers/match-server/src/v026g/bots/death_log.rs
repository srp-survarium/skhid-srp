//! [fix-team logging] One record per death, a bot's or a human's, for the match log and the bot trace: who died and who killed,
//! the range, the kill's kind and the killer's weapon, each side's posture and pace, whether a bot victim had its killer in its
//! senses, and who took the first hit. Logging only: no decision reads it.
//!
//! Why: in 14 local matches the human outscored every bot (1st of his team 12 of 14, 1.88x the best bot mate), and nothing
//! recorded who killed whom, so his fights could not be split by aim, movement or position
//! (`owner-final-local-matches-20261122.md` section 6, row 1).
//!
//! Every field is the server's own, read from the worlds:
//! - the killer: the one enemy whose statistics' kills rose with the death (the kill feed's pairing; none when two rose);
//! - the kind: which of the statistics' kill events rose with it for the killer (`HEADSHOT_KILL`, `MELEE_KILL`,
//!   `GRENADE_KILL`, `SILENT_KILL`; none of them: a plain kill);
//! - the weapon: the kind of the weapon the worlds show in each side's hands (`MatchFeatures::weapons`);
//! - the posture and pace: each body's keys (crouch, sprint) and speed as last reckoned, the victim's from the last world
//!   it was alive in;
//! - the senses: a bot victim's memory of its killer (last sensed, how), none for a human victim;
//! - the hits: each side's hits taken this life (`m_recent_hits` of its worlds), from when both were alive.

use mapdata::Vec3;

use super::know::Source;
use super::reflex::WeaponKind;
use super::view::{Place, PlaceView};
use crate::v026g::drive::key;

/// How the statistics booked the kill (the killer's kill events that rose with the death).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KillKind {
    Headshot,
    Melee,
    Grenade,
    Silent,
    /// A kill and none of the kinds above.
    Plain,
}

impl KillKind {
    /// The order of [`KillCounts`]' fields: headshot, melee, grenade, silent.
    pub fn of(rose: [bool; 4]) -> KillKind {
        match rose {
            [true, ..] => KillKind::Headshot,
            [_, true, ..] => KillKind::Melee,
            [_, _, true, _] => KillKind::Grenade,
            [.., true] => KillKind::Silent,
            _ => KillKind::Plain,
        }
    }

    fn name(self) -> &'static str {
        match self {
            KillKind::Headshot => "headshot",
            KillKind::Melee => "melee",
            KillKind::Grenade => "grenade",
            KillKind::Silent => "silent",
            KillKind::Plain => "plain",
        }
    }
}

/// By place, the statistics' kill events of the kinds [`KillKind`] reads: headshot, melee, grenade, silent kills.
pub type KillCounts = [u32; 4];

/// One side of a death, as the worlds last showed it.
#[derive(Clone, Debug, PartialEq)]
pub struct Side {
    pub place: Place,
    pub bot: bool,
    pub at: Vec3,
    pub speed: f32,
    pub crouched: bool,
    pub sprint_key: bool,
    pub weapon: Option<WeaponKind>,
    /// The hits it took this life from when both were alive (the worlds' ms), oldest first.
    pub hits: Vec<u32>,
}

impl Side {
    pub fn of(p: &PlaceView, weapon: Option<WeaponKind>, hits: Vec<u32>) -> Side {
        Side {
            place: p.place,
            bot: p.bot,
            at: p.feet,
            speed: p.speed(),
            crouched: p.keys & key::CROUCH != 0,
            sprint_key: p.keys & key::SPRINT != 0,
            weapon,
            hits,
        }
    }

    fn who(&self) -> String {
        format!("{} {}", if self.bot { "bot" } else { "human" }, self.place)
    }

    fn pose(&self) -> String {
        format!(
            "{}{}, {:.1} m/s, {}",
            if self.crouched { "crouched" } else { "standing" },
            if self.sprint_key { ", sprint key" } else { "" },
            self.speed,
            self.weapon.map_or_else(|| "weapon unknown".to_string(), |w| format!("{w:?}"))
        )
    }

    fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "place": self.place,
            "bot": self.bot,
            "at": [self.at.x, self.at.y, self.at.z],
            "speed": self.speed,
            "crouched": self.crouched,
            "sprint_key": self.sprint_key,
            "weapon": self.weapon.map(|w| format!("{w:?}")),
            "hits": self.hits,
        })
    }
}

/// What a bot victim's senses held of its killer at the death.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Senses {
    /// The victim is not a bot: no memory to read.
    Human,
    /// The bot had no track of him.
    Unknown,
    /// Its track of him: last sensed at `time`, by `source`; `sight_since`: in sight without a break since then.
    Track { time: u32, source: Source, sight_since: Option<u32> },
}

/// Who took the first hit, from each side's hits taken since both were alive (the killer's hits are those it took from
/// anyone: who hit it is not in the worlds).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirstHit {
    /// The victim was hit first (or the killer was never hit): the killer opened.
    Killer,
    /// The killer was hit before the victim was.
    Victim,
    /// Neither was hit (a one-shot kill the worlds did not list, a grenade's blast).
    Neither,
}

impl FirstHit {
    pub fn of(victim: &[u32], killer: &[u32]) -> FirstHit {
        match (victim.first(), killer.first()) {
            (None, None) => FirstHit::Neither,
            (Some(_), None) => FirstHit::Killer,
            (None, Some(_)) => FirstHit::Victim,
            (Some(v), Some(k)) => {
                if v <= k {
                    FirstHit::Killer
                } else {
                    FirstHit::Victim
                }
            }
        }
    }
}

/// One death's record.
#[derive(Clone, Debug, PartialEq)]
pub struct Death {
    pub time: u32,
    pub victim: Side,
    /// The paired killer; none when the kill feed did not pair one (`killers` then lists whose kills rose).
    pub killer: Option<Side>,
    pub killers: Vec<Place>,
    pub kind: Option<KillKind>,
    pub senses: Senses,
}

impl Death {
    pub fn range_m(&self) -> Option<f32> {
        self.killer.as_ref().map(|k| (k.at - self.victim.at).length())
    }

    pub fn first_hit(&self) -> Option<FirstHit> {
        self.killer.as_ref().map(|k| FirstHit::of(&self.victim.hits, &k.hits))
    }

    fn senses_text(&self) -> String {
        match self.senses {
            Senses::Human => "a human: senses not recorded".to_string(),
            Senses::Unknown => "the victim had no track of its killer".to_string(),
            Senses::Track { time, source, sight_since } => format!(
                "the victim last sensed its killer {:.1} s before ({source:?}{})",
                self.time.saturating_sub(time) as f32 / 1000.0,
                sight_since.map_or_else(String::new, |since| format!(", in sight for {:.1} s", time.saturating_sub(since) as f32 / 1000.0))
            ),
        }
    }

    /// The match log's line (after "the bots at T ms: ").
    pub fn line(&self) -> String {
        let hits = |s: &Side| match (s.hits.first(), s.hits.last()) {
            (Some(a), Some(b)) => format!("{} hits taken {:.1}-{:.1} s before", s.hits.len(), (self.time.saturating_sub(*a)) as f32 / 1000.0, (self.time.saturating_sub(*b)) as f32 / 1000.0),
            _ => "no hit taken".to_string(),
        };
        match &self.killer {
            Some(k) => format!(
                "death: {} killed by {} at {:.1} m, {} kill; victim {}, {}; killer {}, {}; first hit {:?}; {}",
                self.victim.who(),
                k.who(),
                self.range_m().unwrap_or(0.0),
                self.kind.map_or("kind not read", KillKind::name),
                self.victim.pose(),
                hits(&self.victim),
                k.pose(),
                hits(k),
                self.first_hit().unwrap_or(FirstHit::Neither),
                self.senses_text()
            ),
            None => format!(
                "death: {} killed, no paired killer (kills rose for {:?}); victim {}, {}; {}",
                self.victim.who(),
                self.killers,
                self.victim.pose(),
                hits(&self.victim),
                self.senses_text()
            ),
        }
    }

    /// The bot trace's line.
    pub fn json(&self) -> serde_json::Value {
        let senses = match self.senses {
            Senses::Human => serde_json::Value::Null,
            Senses::Unknown => serde_json::json!({"track": false}),
            Senses::Track { time, source, sight_since } => {
                serde_json::json!({"track": true, "last_sensed": time, "source": format!("{source:?}"), "sight_since": sight_since})
            }
        };
        serde_json::json!({
            "t": self.time,
            "death": {
                "victim": self.victim.json(),
                "killer": self.killer.as_ref().map(Side::json),
                "killers": self.killers,
                "range_m": self.range_m(),
                "kind": self.kind.map(KillKind::name),
                "first_hit": self.first_hit().map(|f| format!("{f:?}")),
                "senses": senses,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn side(place: Place, bot: bool, x: f32, hits: Vec<u32>) -> Side {
        Side {
            place,
            bot,
            at: Vec3::new(x, 0.0, 0.0),
            speed: 3.5,
            crouched: false,
            sprint_key: false,
            weapon: Some(WeaponKind::Rifle),
            hits,
        }
    }

    /// The kinds by the statistics' events that rose, and who was hit first by each side's hits taken.
    #[test]
    fn the_kill_kind_and_the_first_hit_come_from_the_worlds() {
        assert_eq!(KillKind::of([true, false, false, false]), KillKind::Headshot);
        assert_eq!(KillKind::of([false, false, true, false]), KillKind::Grenade);
        assert_eq!(KillKind::of([false; 4]), KillKind::Plain);
        assert_eq!(FirstHit::of(&[1_000, 1_200], &[1_100]), FirstHit::Killer);
        assert_eq!(FirstHit::of(&[1_000], &[]), FirstHit::Killer);
        assert_eq!(FirstHit::of(&[1_300], &[1_100]), FirstHit::Victim);
        assert_eq!(FirstHit::of(&[], &[]), FirstHit::Neither);
    }

    /// A bot shot by a human 20 m off: the line and the trace's record carry both sides, the range, the kind, the first hit
    /// and what the bot sensed of him.
    #[test]
    fn a_death_writes_its_line_and_its_trace_record() {
        let death = Death {
            time: 10_000,
            victim: side(3, true, 0.0, vec![9_000, 9_400]),
            killer: Some(side(9, false, 20.0, vec![])),
            killers: vec![9],
            kind: Some(KillKind::Headshot),
            senses: Senses::Track { time: 9_500, source: Source::Seen, sight_since: Some(9_000) },
        };
        assert_eq!(
            death.line(),
            "death: bot 3 killed by human 9 at 20.0 m, headshot kill; victim standing, 3.5 m/s, Rifle, 2 hits taken 1.0-0.6 s \
             before; killer standing, 3.5 m/s, Rifle, no hit taken; first hit Killer; the victim last sensed its killer 0.5 s \
             before (Seen, in sight for 0.5 s)"
        );
        let json = death.json();
        assert_eq!(json["death"]["range_m"], 20.0);
        assert_eq!(json["death"]["kind"], "headshot");
        assert_eq!(json["death"]["killer"]["bot"], false);
        assert_eq!(json["death"]["first_hit"], "Killer");
        let unpaired = Death { killer: None, killers: vec![9, 11], kind: None, senses: Senses::Human, ..death };
        assert!(unpaired.line().starts_with("death: bot 3 killed, no paired killer (kills rose for [9, 11])"), "{}", unpaired.line());
    }
}
