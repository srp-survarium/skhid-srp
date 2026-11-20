//! The game's `<Documents>\survarium\user.cfg`: saved console commands,
//! `<name> <value>` a line. The launcher keeps "Allow torrent" there so it
//! stays as the player left it, whatever is installed or replaced.

/// Beside user.cfg (both are the Windows user's), made by the launcher once it
/// has reset the original default `enable_torrent_seeding on`: until then the
/// line is not the player's choice.
pub const RESET_FILE: &str = "launcher_skhid.torrent_reset";

/// The first word after `name` on its first line, as the launcher reads it.
pub fn setting<'a>(text: &'a [u8], name: &[u8]) -> Option<&'a [u8]> {
    let line = text.split(|&byte| byte == b'\n').find_map(|line| {
        let rest = line.strip_prefix(name)?;
        rest.first().filter(|&&byte| byte == b' ' || byte == b'\t')?;
        Some(rest)
    })?;
    line.split(|&byte| byte <= b' ').find(|word| !word.is_empty())
}

/// The launcher's "Allow torrent"; off without the line.
pub fn torrent_allowed(text: &[u8]) -> bool {
    setting(text, b"enable_torrent_seeding") == Some(b"on")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allow_torrent_is_the_enable_torrent_seeding_line() {
        let of = |text: &str| torrent_allowed(text.as_bytes());
        assert!(of("localization polish\r\nenable_torrent_seeding on\r\nr_fullscreen on\r\n"));
        assert!(of("enable_torrent_seeding\ton"));
        assert!(!of("enable_torrent_seeding off\r\n"));
        assert!(!of("enable_torrent_seeding on2\r\n"));
        assert!(!of("enable_torrent_seedings on\r\n# enable_torrent_seeding on\r\n"));
        assert!(!of("r_fullscreen on\r\n"));
        assert!(!of(""));
    }
}
