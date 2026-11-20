//! A local replay selected in Explorer. No shell command is ever evaluated.
use alloc::{format, string::String};

pub const PROG_ID: &str = "Skhid.Replay.1";

/// Select the local-file entry point, including Open With / dragging onto the exe.
pub fn command<S: AsRef<str>>(line: &[S]) -> Option<(&[S], bool)> {
    let rest = match line {
        [_, action, rest @ ..] if action.as_ref() == "open-replay" => rest,
        [_, path, ..] if path.as_ref().to_ascii_lowercase().ends_with(".sur") => &line[1..],
        _ => return None,
    };
    Some(match rest {
        [args @ .., last] if last.as_ref() == "quiet" => (args, true),
        _ => (rest, false),
    })
}

/// Exactly one absolute Windows .sur path. Encoding/length and readability
/// are checked at the Windows boundary before constructing the game command.
pub struct LocalReplay<'a>(&'a str);

impl<'a> LocalReplay<'a> {
    pub fn parse<S: AsRef<str>>(args: &'a [S]) -> Option<Self> {
        let [path] = args else { return None };
        let path = path.as_ref();
        let drive = path.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
            && path.get(1..3) == Some(":\\");
        let unc = path.starts_with("\\\\")
            && !path.starts_with("\\\\?\\")
            && !path.starts_with("\\\\.\\")
            && path[2..].split('\\').filter(|s| !s.is_empty()).count() >= 3;
        (path.to_ascii_lowercase().ends_with(".sur")
            && (drive || unc)
            && !path.chars().any(|c| c == '"' || c.is_control()))
        .then_some(Self(path))
    }

    pub fn path(&self) -> &str {
        self.0
    }
}

/// Fully quoted command registered with Explorer. %1 is substituted by Windows.
pub fn association_command(updater: &str) -> Option<String> {
    if updater.is_empty()
        || updater.ends_with('\\')
        || updater.chars().any(|c| c == '"' || c.is_control())
    {
        return None;
    }
    Some(format!("\"{updater}\" open-replay \"%1\""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{args, replay};

    #[test]
    fn explorer_and_drop_commands_keep_one_path_and_game_memory_defaults() {
        let updater = r"C:\My Games\game\binaries\x86\updater_skhid.exe";
        let file = r"D:\Downloaded replays\матч 1.SUR";
        let registered = association_command(updater).unwrap().replace("%1", file);
        for text in [registered, format!("\"{updater}\" \"{file}\"")] {
            let args = args::split(&text);
            let (args, quiet) = command(&args).unwrap();
            assert!(!quiet);
            let local = LocalReplay::parse(args).unwrap();
            assert_eq!(local.path(), file);
            let game = replay::game_command("C:\\game.exe", local.path()).unwrap();
            assert!(!game.contains("allocator="));
            assert!(!game.contains("resources_memory_size="));
            assert!(game.ends_with(&format!("-match_replay=\"{file}\"")));
        }
    }

    #[test]
    fn malformed_paths_and_extra_arguments_never_start_a_game() {
        for path in [
            "x.sur",
            "https://host/x.sur",
            r"C:\x.exe",
            "C:\\x\".sur",
            "C:\\x\n.sur",
            r"\\?\C:\x.sur",
            r"\\.\x.sur",
            r"1:\x.sur",
        ] {
            assert!(LocalReplay::parse(&[path]).is_none(), "{path}");
        }
        assert!(LocalReplay::parse(&[r"\\server\share\match.sur"]).is_some());
        assert!(LocalReplay::parse(&[r"C:\x.sur", "-bad"]).is_none());
        assert!(LocalReplay::parse::<&str>(&[]).is_none());
        assert!(command(&["updater", "replay", "link"]).is_none());
        assert!(command(&["updater", "check", "quiet"]).is_none());
        let (args, quiet) = command(&["updater", "open-replay", r"C:\x.sur", "quiet"]).unwrap();
        assert!(quiet);
        assert_eq!(LocalReplay::parse(args).unwrap().path(), r"C:\x.sur");
    }
}
