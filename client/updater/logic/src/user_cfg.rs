//! The game's `<Documents>\survarium\user.cfg`: its saved console commands,
//! a line each (`<name> <value>`). The launcher keeps two of the player's
//! choices there, as the game's Settings do: the language (`localization`)
//! and "Allow torrent" (the game's own `enable_torrent_seeding`), so that
//! they stay as the player left them, whatever is installed or replaced.

/// The file the launcher makes beside itself once it has reset the original
/// game's `enable_torrent_seeding on` (its default, saved in every player's
/// user.cfg) to `off` (launcher patch 15): until then the line is not the
/// player's choice.
pub const RESET_FILE: &str = "launcher_skhid.torrent_reset";

/// The value of a saved command: the first word after the name of its
/// first line (`name`, a blank, the value), as the launcher reads it.
pub fn setting<'a>(text: &'a [u8], name: &[u8]) -> Option<&'a [u8]> {
    let line = text.split(|&byte| byte == b'\n').find_map(|line| {
        let rest = line.strip_prefix(name)?;
        rest.first().filter(|&&byte| byte == b' ' || byte == b'\t')?;
        Some(rest)
    })?;
    line.split(|&byte| byte <= b' ').find(|word| !word.is_empty())
}

/// The launcher's "Allow torrent": `enable_torrent_seeding on`, as the
/// launcher writes it (and the game); off without the line or with any
/// other value.
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
