//! The server's whole upload check ([`super::world_validation::check`],
//! late-joiner retries included) of a match as its 0xE4 gave it, for
//! `examples/world_check_bench.rs`: a recording's uploads timed and judged
//! offline as the match thread does.
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::match_msgs::static_info::{MatchOptions, StaticInfo, StaticInfoError};

use super::registry::Formed;

/// A match as the server holds it when an upload comes: its roster then,
/// and how many places its first roster had.
pub struct Match(Formed);

impl Match {
    /// `options`: the newest 0xE4 the match had sent; `initial_players`: the
    /// places of its first.
    pub fn new(options: &MatchOptions, initial_players: usize) -> Result<Self, StaticInfoError> {
        let info = StaticInfo::new(options, Dictionary::shared())?;
        Ok(Self(Formed {
            match_id: options.match_id,
            played: options.map,
            match_level: options.match_level,
            places: Vec::new(),
            profiles: options.players.clone(),
            score_weights: options.score_weights,
            info,
            formed_at: std::time::Instant::now(),
            growing_roster: false,
            initial_players,
            vs_bots: false,
            private: false,
            room_minimum: None,
        }))
    }

    /// The upload `body` of item time `time`, the world of `places` places
    /// (20 in a match that lets players in late): `Err` with the line the
    /// server logs of a refusal.
    pub fn check(&self, body: &[u8], time: u32, places: usize) -> Result<(), String> {
        super::world_validation::check(body, time, places, &self.0).map_err(|r| r.to_string())
    }
}
