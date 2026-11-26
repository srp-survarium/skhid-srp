//! local-match (0.69d): Training-with-bots / tutorial match options and results.
//!
//! Pure functions, no sockets and no clock (pass the match id; [`fresh_match_id`] gives a clock-based one).
//!
//! ```ignore
//! let gd = game_data::GameData::from_dir(data_dir)?;
//! let (opts, roster) = local_match::training_options(&gd, &TrainingParams::new(map, mode, weather, id), &human)?;
//! let payload = local_match::pack_5b(1, &opts);            // after the request id of the 83 5B reply
//! let tut = local_match::pack_5b(2, &local_match::tutorial_options(&gd, &human, id)?);
//! // when the client's 53 flush arrives:
//! let (msg97, rewards) = local_match::results(&gd, &roster, &events, &progress, roster.match_id)?;
//! ```
//!
//! The layouts equal those of the earlier reference implementation, which worked with the real client (the tests
//! compare bytes with reference vectors). The options are unpacked bytes; `pack_5b` uses stored LZMA2 chunks, which
//! the client's decoder accepts (a compressing encoder gives other packed bytes; the unpacked bytes are what counts).

pub mod gunrace;
mod mt;
pub mod options;
pub mod results;
pub mod start_objects;

pub use options::{
    fresh_match_id, online_options, online_options_bots, online_options_row, pick_online_bots, pve_options, OnlineBot, shooting_range_options, training_options, tutorial_human, tutorial_options, HumanLoadout, HumanProfile, OnlineParams, OnlinePlayer,
    TrainingParams,
};
pub use gunrace::gunrace_options;
pub use start_objects::{pve_roster_spec, pvp_roster_spec, roster_spec};
pub use results::{outcome, results, results_with_bonus, results_with_bonuses, HumanProgress, Person, Rewards};

/// A bot of the match (kept for the results screen).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosterBot {
    pub k: u32,
    pub team: u8,
    /// index into the 36 AI players
    pub ai: u32,
    /// st_bot_1st_name / 2nd_name list indices
    pub first: u32,
    pub second: u32,
    /// fallback name ("Bot N")
    pub name: String,
}

/// Who plays a local match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Roster {
    pub match_id: u32,
    pub map_id: u32,
    pub mode_id: u32,
    pub human_name: String,
    pub bots: Vec<RosterBot>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("no game mode file for map {map} mode {mode} variation {variation}")]
    NoMode { map: u32, mode: u32, variation: u32 },
    #[error("limit exceeded: {0}")]
    Limit(&'static str),
    #[error("invalid value: {0}")]
    Invalid(&'static str),
    #[error(transparent)]
    Protocol(#[from] protocol::Error),
}

/// `83 5B` payload after the request id: `u8 sub, u32 packed size, raw LZMA2` (sub 1 = training, 2 = tutorial, 3 = shooting range).
pub fn pack_5b(sub: u8, options: &[u8]) -> Vec<u8> {
    let blob = protocol::lzma2::compress(options);
    let mut out = Vec::with_capacity(blob.len() + 5);
    out.push(sub);
    out.extend_from_slice(&(blob.len() as u32).to_le_bytes());
    out.extend_from_slice(&blob);
    out
}
