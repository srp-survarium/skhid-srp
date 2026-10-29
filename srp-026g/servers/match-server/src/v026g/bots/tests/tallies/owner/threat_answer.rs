//! The owner counts of the threat answer (P1, `threat.rs`; `an-anticamper.md` K1-K3): for each switch, how
//! often it fired, was refused or fell back, so that a 0 in the ON arm where a fix should fire shows.
//!
//! - K1 A2 (`whole_route`, `lane_weighted`): routes checked and exposed; searches round the lanes by how they
//!   ended (a way round, no way, too long or no less exposed, out of floors), the floors they expanded, and the
//!   exposed metres of the routes asked for and of the routes kept.
//! - K1 A3 (`replan`): ways ahead looked at against new cover, found exposed (a replan) or clear.
//! - K1 A4 (`recast`): lanes cast again.
//! - K2 (`job`): roles given, let go (not free, no play taken), jobs ended (unheld, their time, no bot, a holder
//!   down); and by team (`threat::JOB_TALLIES`): each end's reason (unheld split into its shooter died, not sensed
//!   lately, faded, cleared), whom a holder that went down died to (the spot's shooter, another enemy, not known),
//!   roles given on a spot whose shooter the team had not sensed for 15 s, and the jobs' lives (count, median, p90).
//! - K3: trades refused, PairUp teammates refused and join / wait spots out of sight found, Crossfire spots left
//!   out.
//!
//! The counts are the threads' (`threat::counted`, `searches`, `checks`, `spent`), read as the difference from
//! the topic's start: a match runs on one thread.

use super::*;
use crate::v026g::bots::threat;

/// The threat answer's counts at the start of the match, for the difference at its end.
pub(super) struct ThreatAnswerCounts {
    counted: [u64; 23],
    searches: [u64; 4],
    checks: [u64; 2],
    spent: [u64; 3],
    jobs: [[u64; 11]; 2],
    lives_from: usize,
    by_tag: [[u64; 3]; 4],
    offers: std::collections::BTreeMap<String, u64>,
}

impl Default for ThreatAnswerCounts {
    fn default() -> Self {
        Self {
            counted: threat::counted(),
            searches: threat::searches(),
            checks: threat::checks(),
            spent: threat::spents(),
            jobs: threat::job_tallied(),
            lives_from: threat::job_lives(0).len(),
            by_tag: crate::v026g::bots::path::tag::counted(),
            offers: threat::offer_counts(),
        }
    }
}

impl Topic for ThreatAnswerCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("threat_answer", self.part())]
    }
}

impl ThreatAnswerCounts {
    fn part(&self) -> Part {
        let d = |now: &[u64], then: &[u64], i: usize| now[i] - then[i];
        let (counted, searches, checks, spent) = (threat::counted(), threat::searches(), threat::checks(), threat::spents());
        let mut json = serde_json::Map::new();
        json.insert("k1_checked".into(), d(&checks, &self.checks, 0).into());
        json.insert("k1_exposed".into(), d(&checks, &self.checks, 1).into());
        for (i, name) in ["k1_round", "k1_no_way", "k1_too_long", "k1_budget"].iter().enumerate() {
            json.insert((*name).into(), d(&searches, &self.searches, i).into());
        }
        let runs: u64 = (0..4).map(|i| d(&searches, &self.searches, i)).sum();
        let floors = d(&spent, &self.spent, 0);
        json.insert("k1_search_floors".into(), floors.into());
        json.insert("k1_floors_per_search".into(), floors.checked_div(runs).unwrap_or(0).into());
        json.insert("k1_exposed_asked_m".into(), (d(&spent, &self.spent, 1) as f64 / 10.0).into());
        json.insert("k1_exposed_kept_m".into(), (d(&spent, &self.spent, 2) as f64 / 10.0).into());
        for (i, (_, name)) in threat::COUNTS.iter().enumerate() {
            json.insert((*name).into(), d(&counted, &self.counted, i).into());
        }
        // K2 by team: each end's reason, whom a holder died to, roles on a spot not sensed lately, the lives.
        let jobs = threat::job_tallied();
        let lives = threat::job_lives(self.lives_from);
        let mut by_team = Vec::new();
        for team in 0..2u8 {
            let t = usize::from(team);
            let mut team_json = serde_json::Map::new();
            for (i, (_, name)) in threat::JOB_TALLIES.iter().enumerate() {
                team_json.insert((*name).into(), (jobs[t][i] - self.jobs[t][i]).into());
            }
            let mut ms: Vec<u32> = lives.iter().filter(|(lt, _)| *lt == team).map(|&(_, ms)| ms).collect();
            ms.sort_unstable();
            let q = |p: f64| ms.get(((ms.len() as f64 * p) as usize).min(ms.len().saturating_sub(1))).map_or(0.0, |&v| f64::from(v) / 1000.0);
            let (median, p90) = (q(0.5), q(0.9));
            team_json.insert("jobs".into(), ms.len().into());
            team_json.insert("life_median_s".into(), median.into());
            team_json.insert("life_p90_s".into(), p90.into());
            let tn = |key: &str| team_json.get(key).and_then(serde_json::Value::as_u64).unwrap_or(0);
            by_team.push(format!(
                "team {team}: jobs {} (life median {median:.1} s, p90 {p90:.1} s), ended shooter died {} / unseen {} / faded {} / cleared {} / holder down {} (to the shooter {}, another {}, not known {}) / time {} / no bot {}, roles on a spot not sensed 15 s {}",
                ms.len(),
                tn("ended_shooter_died"),
                tn("ended_unseen"),
                tn("ended_faded"),
                tn("ended_cleared"),
                tn("ended_holder_down"),
                tn("down_by_shooter"),
                tn("down_by_other"),
                tn("down_by_unknown"),
                tn("ended_time"),
                tn("ended_nobody"),
                tn("given_unsensed"),
            ));
            json.insert(format!("jobs_team{team}"), serde_json::Value::Object(team_json));
        }
        // The job roles' offers by poll, and the roles let go unplayed by class (their last offer; the play run instead).
        let mut offers = serde_json::Map::new();
        let mut offer_text = Vec::new();
        for (name, n) in threat::offer_counts() {
            let n = n - self.offers.get(&name).copied().unwrap_or(0);
            if n > 0 {
                offer_text.push(format!("{name} {n}"));
                offers.insert(name, n.into());
            }
        }
        json.insert("job_offers".into(), serde_json::Value::Object(offers));
        // The grid searches by why they ran: count, failed, floors.
        let by_tag = crate::v026g::bots::path::tag::counted();
        let mut work = Vec::new();
        for (i, name) in crate::v026g::bots::path::tag::NAMES.iter().enumerate() {
            let [s, f, e] = [0, 1, 2].map(|k| by_tag[i][k] - self.by_tag[i][k]);
            json.insert(format!("searches_{name}"), serde_json::json!({"searches": s, "failed": f, "floors": e}));
            work.push(format!("{name} {s} (failed {f}, floors {e})"));
        }
        let n = |key: &str| json.get(key).and_then(serde_json::Value::as_u64).unwrap_or(0);
        let text = format!(
            "owner-test threat answer: K1 routes checked {} exposed {}; searches round the lanes {} (round {}, no way {}, too long or no less exposed {}, out of floors {}), {} floors a search, exposed m asked {:.1} kept {:.1}; replans asked {} (clear {}; next route a new way {}, the same {}, none {}; new ways less exposed {}, out of the lanes {}); recasts {}; K2 roles given {}, let go not free {} / no play {}, jobs ended unheld {} / time {} / no bot {} / a holder down {}, holds left without a job for no free bot {} spot-ticks; {}; K3 trades refused {}, PairUp mates refused {}, join spots hidden {} (none {}), wait spots hidden {}, Crossfire spots left out {}, chokes on a held spot not theirs {}; grid searches by why: {}; K2 role offers: {}",
            n("k1_checked"),
            n("k1_exposed"),
            runs,
            n("k1_round"),
            n("k1_no_way"),
            n("k1_too_long"),
            n("k1_budget"),
            n("k1_floors_per_search"),
            d(&spent, &self.spent, 1) as f64 / 10.0,
            d(&spent, &self.spent, 2) as f64 / 10.0,
            n("replan_asked"),
            n("replan_clear"),
            n("replan_new"),
            n("replan_same"),
            n("replan_no_route"),
            n("replan_less"),
            n("replan_out"),
            n("recast"),
            n("job_given"),
            n("job_passed_not_free"),
            n("job_passed_no_play"),
            n("job_ended_unheld"),
            n("job_ended_time"),
            n("job_ended_nobody"),
            n("job_ended_down"),
            n("job_not_opened"),
            by_team.join("; "),
            n("trade_refused"),
            n("pair_up_mate_refused"),
            n("pair_up_join_hidden"),
            n("pair_up_join_none"),
            n("pair_up_wait_hidden"),
            n("crossfire_spot_refused"),
            n("crossfire_choke_on_spot"),
            work.join(", "),
            offer_text.join(", "),
        );
        Part::new(serde_json::Value::Object(json), text)
    }
}
