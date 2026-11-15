//! The one upload cap of an updater's run
//! (`srp-026g/docs/notes/torrent-downloads.md`, "The upload cap"): all the
//! sharing together (every torrent, the DHT) stays within [`CAP`] bytes a
//! second. It is the libtorrent session's upload limit, every peer's (those
//! of the local network too); the launcher's `1E` may lower it, never raise
//! it.

/// The fixed cap of all sharing: 1 MiB a second ("1 MB/s").
pub const CAP: u64 = 1 << 20;

/// The rate for the launcher's upload limit (`1E`, bytes a second; `None`
/// or 0: none): the lower of it and [`CAP`].
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
