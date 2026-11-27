//! Results of a local match: message 97 and the human's rewards. The client reports no kills / deaths / score to the
//! lobby, so the per-player numbers are SYNTHETIC (seeded by the match id through a Python-compatible
//! `random.Random(match_id)` stream, [`crate::mt`]); roster, names, teams, winner and the human's rewards are real.

use game_data::GameData;
use protocol::s2c::results::{MatchResult, ResultFull, ResultPlayer};
use protocol::Encode;

use crate::mt::PyRandom;
use crate::{Error, Roster};

/// Where the human stands before the match (the lobby's account state).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HumanProgress {
    /// account experience before the match
    pub xp: u32,
    /// reputation points of the active faction before the match
    pub faction_points: u32,
    /// active faction 1..4
    pub faction: u8,
    /// the account has not been paid the `played_first_game` event yet
    pub first_game: bool,
}

/// One row of the synthetic statistics.
#[derive(Debug, Clone, PartialEq)]
pub struct Person {
    pub name: String,
    pub team: u8,
    /// public id: 0 for the human, bot number + 1 for bots (ids below 2 * players + 2 are bots on the screen)
    pub pid: u64,
    pub human: bool,
    pub kills: u32,
    pub deaths: u32,
    pub score: u32,
}

/// What the match pays the human. Apply through pushes 8B (money), 8C (level + exp), 8E (reputation); the 97
/// screen is display only.
#[derive(Debug, Clone, PartialEq)]
pub struct Rewards {
    pub won: bool,
    /// a won/lost marker (53 events 5/6 or 0x3D/0x3E) was seen; a match left early pays nothing
    pub finished: bool,
    pub exp: u32,
    pub silver: u32,
    pub rep: u32,
    pub people: Vec<Person>,
    pub team0_score: u32,
    pub team1_score: u32,
}

/// `(won, finished)` from the 53 event ids: 5 / 0x3D = victory, 6 / 0x3E = defeat (B8 enables them).
pub fn outcome(events: &[u16]) -> (bool, bool) {
    let won = events.iter().any(|e| matches!(e, 5 | 0x3D));
    let finished = events.iter().any(|e| matches!(e, 5 | 6 | 0x3D | 0x3E));
    (won, finished)
}

/// Synthetic kills / deaths / score: the winner team gets 4..6 kills per player, the loser
/// 45..90 % of that; every kill is one death of the other team; shares by random weights.
fn synth_stats(gd: &GameData, roster: &Roster, won: bool, seed: u32) -> (Vec<Person>, u32, u32) {
    let mut rnd = PyRandom::new(seed);
    let mut people = vec![Person { name: roster.human_name.clone(), team: 0, pid: 0, human: true, kills: 0, deaths: 0, score: 0 }];
    let mut weights = vec![1.0f64];
    for b in &roster.bots {
        let name = gd.matches.bot_name(b.first as usize, b.second as usize, &b.name);
        people.push(Person { name, team: b.team, pid: b.k as u64 + 1, human: false, kills: 0, deaths: 0, score: 0 });
        weights.push(rnd.uniform(0.3, 1.7));
    }
    let wt = if won { 0usize } else { 1 };
    let team_idx = |t: usize| -> Vec<usize> { (0..people.len()).filter(|&i| people[i].team as usize == t).collect() };
    let teams = [team_idx(0), team_idx(1)];
    let mut tk = [0u32; 2];
    tk[wt] = rnd.randint(4, 6) * (teams[wt].len().max(1) as u32);
    tk[1 - wt] = (tk[wt] as f64 * rnd.uniform(0.45, 0.9)) as u32;
    for t in 0..2 {
        if teams[t].is_empty() {
            tk[t] = 0; // nobody to score (tutorial: the human is alone)
        }
    }
    for t in 0..2 {
        if !teams[t].is_empty() {
            let w: Vec<f64> = teams[t].iter().map(|&i| weights[i]).collect();
            for c in rnd.choices(&w, tk[t] as usize) {
                people[teams[t][c]].kills += 1;
            }
        }
        if !teams[1 - t].is_empty() {
            let w: Vec<f64> = teams[1 - t].iter().map(|&i| weights[i]).collect();
            for c in rnd.choices(&w, tk[t] as usize) {
                people[teams[1 - t][c]].deaths += 1;
            }
        }
    }
    for p in &mut people {
        p.score = p.kills * 100 + rnd.randint(0, 60);
    }
    (people, tk[0], tk[1])
}

/// Message 97 (payload after `97 x`) and the rewards of a finished local match.
/// `events` = the event ids of the client's `53` flush; `match_id` seeds the synthetic numbers (0 counts as 1).
pub fn results(gd: &GameData, roster: &Roster, events: &[u16], human: &HumanProgress, match_id: u32) -> Result<(Vec<u8>, Rewards), Error> {
    results_with_bonus(gd, roster, events, human, match_id, 0)
}

/// [`results`] with a percentage added to the human's experience, silver and reputation (the premium account's
/// +50 %, shown in the 97 the same as it is paid).
pub fn results_with_bonus(
    gd: &GameData,
    roster: &Roster,
    events: &[u16],
    human: &HumanProgress,
    match_id: u32,
    bonus_percent: u32,
) -> Result<(Vec<u8>, Rewards), Error> {
    results_with_bonuses(gd, roster, events, human, match_id, [bonus_percent; 3])
}

/// [`results_with_bonus`] with one percentage per component `[experience, silver, reputation]` (premium account and
/// the 24 h boosters add up per component).
pub fn results_with_bonuses(
    gd: &GameData,
    roster: &Roster,
    events: &[u16],
    human: &HumanProgress,
    match_id: u32,
    bonus_percent: [u32; 3],
) -> Result<(Vec<u8>, Rewards), Error> {
    let (won, finished) = outcome(events);
    let mid = if match_id == 0 { 1 } else { match_id };
    let (people, t0, t1) = synth_stats(gd, roster, won, mid);
    // the data's reward (game_data::progression): the synthetic kills / score of the screen are NOT facts, so only the
    // drop_after_combat silver roll is paid; the numbers on the screen are the paid ones
    let mut facts = game_data::MatchFacts::local(roster.map_id, roster.mode_id, won, finished, human.faction);
    facts.first_game = human.first_game;
    let r = if game_data::rewards::LOCAL_MATCH_PAYS {
        gd.match_reward(&facts, game_data::rewards::LOCAL_MATCH_LEVEL, &mut game_data::SplitMix(u64::from(mid)))
    } else {
        Default::default() // the bots mode awards nothing (client hint st_match_settings_bots_hint): the screen shows zeros
    };
    let (exp, silver, rep) = (r.exp, r.silver, r.rep);
    let up = |v: u32, pct: u32| v + (v as u64 * pct as u64 / 100) as u32;
    let (exp, silver, rep) = (up(exp, bonus_percent[0]), up(silver, bonus_percent[1]), up(rep, bonus_percent[2]));
    // the place byte is 1: a local match's screen takes the bot rows from the client and a small value is safe there
    // (docs/notes/match-stats.md round D); online results use protocol::s2c::set_places
    let players: Vec<ResultPlayer> = people
        .iter()
        .map(|p| ResultPlayer::new(&p.name, p.pid, i16::try_from(p.score).unwrap_or(i16::MAX), p.kills as u16, p.deaths as u16, p.team))
        .collect();
    let msg = MatchResult {
        match_id: mid,
        map_id: roster.map_id,
        winner_team: if won { 0 } else { 1 },
        // the screen draws the two u8 scores (the u32 params stay 0)
        team0_score: t0.min(255) as u8,
        team1_score: t1.min(255) as u8,
        mode: roster.mode_id as u8,
        match_level: game_data::rewards::LOCAL_MATCH_LEVEL as u8,
        players,
        full: ResultFull::rewards(
            0,
            human.xp.wrapping_add(exp),
            exp as u16,
            human.faction_points.wrapping_add(rep),
            rep as u16,
            human.faction,
            silver,
            exp as u16,
            rep as u16,
        ),
        ..Default::default()
    };
    let bytes = msg.to_bytes()?;
    Ok((bytes, Rewards { won, finished, exp, silver, rep, people, team0_score: t0, team1_score: t1 }))
}
