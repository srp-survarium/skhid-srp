//! A short-lived sample of public chat, to find the slang the glossary
//! should have (`docs/notes/chat-translation.md`, "Collecting slang").
//! **Off by default**: the owner turns it on, after players are told.
//!
//! - `SRP_CHAT_SAMPLE_DIR`: the folder (on the server's private data
//!   volume, never a repository); unset or `off`: nothing is kept.
//! - `SRP_CHAT_SAMPLE_KEEP_DAYS` (default 14, 1 to 90): older files go.
//! - **What is kept:** the lines of the public channels only (general,
//!   match, team, a room's), one row each: the UTC hour, the writer's game
//!   language, the language the line was guessed to be in, the channel and
//!   the text. **No account, name, id or address**; private, squad and clan
//!   lines are never kept. A file a UTC day, `chat-sample-<day>.tsv` (`<day>`
//!   counted from 1970-01-01).
//! - Read with `srp-chat-glossary slang`.

use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use survarium::v026g::lobby::channel;

use super::translate::Lang;

const DAY: u64 = 86_400;
const DEFAULT_KEEP_DAYS: u64 = 14;
const PREFIX: &str = "chat-sample-";

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub dir: PathBuf,
    pub keep_days: u64,
}

impl Config {
    pub fn from_env() -> Option<Self> {
        let dir = std::env::var("SRP_CHAT_SAMPLE_DIR").ok()?;
        let dir = dir.trim();
        if dir.is_empty() || dir == "off" {
            return None;
        }
        let keep_days = match std::env::var("SRP_CHAT_SAMPLE_KEEP_DAYS") {
            Err(_) => DEFAULT_KEEP_DAYS,
            Ok(value) => match value.trim().parse::<u64>() {
                Ok(days @ 1..=90) => days,
                _ => {
                    log::error!(
                        "SRP_CHAT_SAMPLE_KEEP_DAYS is `{value}`; it must be 1 to 90. Using \
                         {DEFAULT_KEEP_DAYS}"
                    );
                    DEFAULT_KEEP_DAYS
                }
            },
        };
        log::info!(
            "Public chat lines are kept in {dir} for {keep_days} days, without names \
             (SRP_CHAT_SAMPLE_DIR)"
        );
        Some(Self {
            dir: PathBuf::from(dir),
            keep_days,
        })
    }
}

/// Whether lines of `channel` may be kept: the public ones.
pub fn is_public(line_channel: u8) -> bool {
    matches!(
        line_channel,
        channel::GENERAL | channel::MATCH | channel::TEAM_1 | channel::TEAM_2 | channel::ROOM
    )
}

pub struct ChatSample {
    config: Config,
    /// The day whose old files were last cleared.
    cleared: Option<u64>,
}

fn code(lang: Option<Lang>) -> &'static str {
    lang.map_or("-", Lang::code)
}

/// A row: tabs and line breaks of the text are spaces.
pub fn row(
    hour: u64,
    game: Option<Lang>,
    guessed: Option<Lang>,
    line_channel: u8,
    text: &str,
) -> String {
    let text: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    format!(
        "{hour:02}\t{}\t{}\t{line_channel}\t{text}\n",
        code(game),
        code(guessed)
    )
}

impl ChatSample {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            cleared: None,
        }
    }

    /// Keep a line (public channels only; see the module).
    pub fn keep(
        &mut self,
        game: Option<Lang>,
        guessed: Option<Lang>,
        line_channel: u8,
        text: &str,
    ) {
        if !is_public(line_channel) {
            return;
        }
        let unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_secs());
        let day = unix / DAY;
        if self.cleared != Some(day) {
            self.cleared = Some(day);
            self.clear_before(day.saturating_sub(self.config.keep_days - 1));
        }
        let path = self.config.dir.join(format!("{PREFIX}{day}.tsv"));
        let written = std::fs::create_dir_all(&self.config.dir).and_then(|()| {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)?
                .write_all(row(unix % DAY / 3600, game, guessed, line_channel, text).as_bytes())
        });
        if let Err(error) = written {
            log::warn!(
                "A chat sample line was not kept in {}: {error}",
                path.display()
            );
        }
    }

    /// Remove the files of days before `first`.
    fn clear_before(&self, first: u64) {
        let Ok(entries) = std::fs::read_dir(&self.config.dir) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let day = name
                .to_str()
                .and_then(|name| name.strip_prefix(PREFIX))
                .and_then(|name| name.strip_suffix(".tsv"))
                .and_then(|day| day.parse::<u64>().ok());
            if day.is_some_and(|day| day < first)
                && let Err(error) = std::fs::remove_file(entry.path())
            {
                log::warn!("{}: not removed: {error}", entry.path().display());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_public_lines_are_kept_with_no_name_and_old_days_go() {
        let dir = std::env::temp_dir().join(format!("srp-chat-sample-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // A file of long ago, and one of another tool.
        std::fs::write(dir.join("chat-sample-1.tsv"), "old").unwrap();
        std::fs::write(dir.join("notes.txt"), "kept").unwrap();
        let mut sample = ChatSample::new(Config {
            dir: dir.clone(),
            keep_days: 14,
        });
        sample.keep(
            Some(Lang::Ru),
            Some(Lang::Ru),
            channel::GENERAL,
            "го на\tбазу",
        );
        sample.keep(Some(Lang::En), Some(Lang::En), channel::PRIVATE, "secret");
        sample.keep(Some(Lang::En), Some(Lang::En), channel::SQUAD, "squad");
        sample.keep(None, None, channel::ROOM, "gg");
        let mut files: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        files.sort();
        assert_eq!(files.len(), 2, "{files:?}");
        assert!(files.contains(&"notes.txt".to_owned()));
        let today = files.iter().find(|name| name.starts_with(PREFIX)).unwrap();
        let kept = std::fs::read_to_string(dir.join(today)).unwrap();
        let rows: Vec<Vec<&str>> = kept.lines().map(|row| row.split('\t').collect()).collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][1..], ["ru", "ru", "1", "го на базу"]);
        assert_eq!(rows[1][1..], ["-", "-", "10", "gg"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
