//! "Player vs Bots": the settings of the mode that more than one server
//! reads (the lobby, the match server, the rewards). The mode itself is the
//! lobby's (`lobby_server::v026g::pvb`, which documents every setting).
//!
//! - `SRP_PVB` (default off): whether the mode is offered at all;
//! - `SRP_PVB_MAX_MATCHES` (default 5, 1 to 20): the most matches against
//!   the server's bots at once; the match server keeps that many bots'
//!   maps in memory at most when the mode is on;
//! - `SRP_PVB_PAY` (default 0.5, 0 to 1): what a match against bots pays of
//!   a match's silver and experience.

/// `SRP_PVB_MAX_MATCHES` when unset.
pub const DEFAULT_MAX_MATCHES: usize = 5;
/// `SRP_PVB_PAY` when unset: half.
pub const DEFAULT_PAY: f64 = 0.5;
/// The most `SRP_PVB_MAX_MATCHES` may be.
pub const MOST_MATCHES: usize = 20;

/// `SRP_PVB`: `on`/`off` (and 1/0, true/false, yes/no); anything else is
/// an error in the log and off.
pub fn enabled_from(value: Option<&str>) -> bool {
    match value
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        None | Some("") => false,
        Some("on" | "1" | "true" | "yes") => true,
        Some("off" | "0" | "false" | "no") => false,
        Some(other) => {
            log::error!("SRP_PVB is `{other}`; it must be on or off. Using off");
            false
        }
    }
}

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

/// [`enabled_from`] of the environment.
pub fn enabled() -> bool {
    enabled_from(var("SRP_PVB").as_deref())
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
/// `SRP_MAPDATA_AT_ONCE` asks for, and at least one for each match with
/// bots that may run (`SRP_PVB_MAX_MATCHES`: against bots and rooms' with
/// bots together, whether Player vs Bots is on or not: any room may have
/// bots).
pub fn maps_at_once(at_once: usize, max_matches: usize) -> usize {
    at_once.max(max_matches)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_settings_read_as_documented() {
        assert!(!enabled_from(None));
        assert!(enabled_from(Some(" ON ")));
        assert!(!enabled_from(Some("maybe")));
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
        assert_eq!(maps_at_once(1, 5), 5);
        assert_eq!(maps_at_once(8, 5), 8);
    }
}
