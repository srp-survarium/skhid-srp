//! The one upload cap of an updater's run, for all its sharing together
//! (`srp-026g/docs/notes/torrent-downloads.md`, "The upload cap").

/// Bytes a second: "1 MB/s".
pub const CAP: u64 = 1 << 20;

/// The launcher's `1E` (bytes a second; `None` or 0: none asked) may lower
/// the cap, never raise it.
pub fn rate(asked: Option<u64>) -> u64 {
    asked.filter(|&asked| asked > 0).map_or(CAP, |asked| asked.min(CAP))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_launcher_may_lower_it_only() {
        assert_eq!(rate(Some(CAP * 10)), CAP);
        assert_eq!(rate(Some(1000)), 1000);
        assert_eq!(rate(None), CAP);
        assert_eq!(rate(Some(0)), CAP);
    }
}
