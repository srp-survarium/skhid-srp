//! "Player vs Bots": the settings of the mode that more than one server
//! reads (the lobby, the match server, the rewards). The mode itself is the
//! lobby's (`lobby_server::v026g::pvb`, which documents every setting).
//!
//! - `SRP_PVB` (default off): whether the mode is offered at all;
//! - `SRP_PVB_MAX_MATCHES` (default 5, 1 to 20): the most matches against
//!   the server's bots at once; the match server keeps that many bots'
//!   maps in memory at most when the mode is on;
//! - `SRP_PVB_PAY` (default 0.5, 0 to 1): what a match against bots pays of
//!   a match's silver and experience;
//! - PvE reward multipliers and silver rates use at most tier 7, and its
//!   end-of-match drop at most match level 7.

/// `SRP_PVB_MAX_MATCHES` when unset.
pub const DEFAULT_MAX_MATCHES: usize = 5;
/// Highest reward-table level for PvE; equipment and bot difficulty still
/// use their normal levels. Owner's choice, 2026-10-28.
pub const MAX_REWARD_LEVEL: u16 = 7;
/// Highest drop-table level for PvE; equipment and bot difficulty still
/// use their normal levels. Owner's choice, 2026-11-01.
pub const MAX_DROP_LEVEL: u16 = 7;

/// The match level a match's drops are rolled at: the end-of-match drop and
/// a drop card used on its results screen. At most [`MAX_DROP_LEVEL`]
/// against bots.
pub fn drop_level(match_level: u16, vs_bots: bool) -> u16 {
    if vs_bots {
        match_level.min(MAX_DROP_LEVEL)
    } else {
        match_level
    }
}
/// `SRP_PVB_PAY` when unset: half.
pub const DEFAULT_PAY: f64 = 0.5;
/// The most `SRP_PVB_MAX_MATCHES` may be.
pub const MOST_MATCHES: usize = 20;

/// `SRP_PVB_MAX_MATCHES`: 1 to [`MOST_MATCHES`].
pub fn max_matches_from(value: Option<&str>) -> usize {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return DEFAULT_MAX_MATCHES;
    };
    match value.parse::<usize>() {
        Ok(count) if (1..=MOST_MATCHES).contains(&count) => count,
        _ => {
            log::error!(
                "SRP_PVB_MAX_MATCHES is `{value}`; it must be 1 to {MOST_MATCHES}. Using \
                 {DEFAULT_MAX_MATCHES}"
            );
            DEFAULT_MAX_MATCHES
        }
    }
}

/// `SRP_PVB_PAY`: 0 to 1.
pub fn pay_from(value: Option<&str>) -> f64 {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return DEFAULT_PAY;
    };
    match value.parse::<f64>() {
        Ok(pay) if pay.is_finite() && (0.0..=1.0).contains(&pay) => pay,
        _ => {
            log::error!("SRP_PVB_PAY is `{value}`; it must be 0 to 1. Using {DEFAULT_PAY}");
            DEFAULT_PAY
        }
    }
}

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// `SRP_PVB`: off unless `on`.
pub fn enabled() -> bool {
    crate::switch("SRP_PVB", false)
}

/// [`max_matches_from`] of the environment.
pub fn max_matches() -> usize {
    max_matches_from(var("SRP_PVB_MAX_MATCHES").as_deref())
}

/// [`pay_from`] of the environment, read once.
pub fn pay() -> f64 {
    static PAY: std::sync::OnceLock<f64> = std::sync::OnceLock::new();
    *PAY.get_or_init(|| pay_from(var("SRP_PVB_PAY").as_deref()))
}

/// How many bots' maps the match server may keep in memory at once: what
/// `SRP_MAPDATA_AT_ONCE` asks for, and with the mode on at least one for
/// each match against bots that may run (`SRP_PVB_MAX_MATCHES`).
pub fn maps_at_once(at_once: usize, enabled: bool, max_matches: usize) -> usize {
    if enabled {
        at_once.max(max_matches)
    } else {
        at_once
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_settings_read_as_documented() {
        assert_eq!(max_matches_from(None), 5);
        assert_eq!(max_matches_from(Some("3")), 3);
        assert_eq!(max_matches_from(Some("0")), 5);
        assert_eq!(max_matches_from(Some("21")), 5);
        assert_eq!(pay_from(None), 0.5);
        assert_eq!(pay_from(Some("0.25")), 0.25);
        assert_eq!(pay_from(Some("0")), 0.0);
        assert_eq!(pay_from(Some("2")), 0.5);
        assert_eq!(pay_from(Some("NaN")), 0.5);
        // The maps follow the cap only with the mode on.
        assert_eq!(maps_at_once(1, true, 5), 5);
        assert_eq!(maps_at_once(1, false, 5), 1);
        assert_eq!(maps_at_once(8, true, 5), 8);
    }
}
