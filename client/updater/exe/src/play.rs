//! PLAY's route selection runs in the updater, before creating the game.
//! No driver, administrator access, credentials, or game-memory hooks.
use crate::text::TITLE;
use crate::{exit, install::Layout, log, message_box, patches, retry_box, route, waiting_box, win, working_box};
use relay_client::{Config, Selection};
use updater_logic::diagnosis::{self, Diagnosis, Via};
use updater_logic::route::{Choice, Learned};

pub(crate) fn config() -> Result<Option<&'static Config>, &'static str> {
    static CONFIG: std::sync::OnceLock<Result<Option<Config>, String>> = std::sync::OnceLock::new();
    CONFIG
        .get_or_init(read_config)
        .as_ref()
        .map(Option::as_ref)
        .map_err(String::as_str)
}

fn read_config() -> Result<Option<Config>, String> {
    let path = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .with_file_name("updater_skhid.relay");
    match std::fs::read_to_string(path) {
        Ok(text) => Config::parse(&text)
            .map(Some)
            .ok_or_else(|| "Invalid updater_skhid.relay. Run the launcher to repair it.".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("Cannot read updater_skhid.relay: {error}")),
    }
}

pub fn run(arguments: &[String]) -> u32 {
    let quiet = arguments.iter().any(|a| a == "quiet");
    // The scripts' way: stay until the game ends, so its end and exit status
    // are theirs to see, as when they started the exe themselves.
    let wait = arguments.iter().any(|a| a == "wait");
    let game_arguments: Vec<_> = arguments
        .iter()
        .filter(|a| !matches!(a.as_str(), "quiet" | "wait"))
        .cloned()
        .collect();
    match launch(&game_arguments, wait, quiet) {
        Ok(()) => exit::OK,
        Err(Ended::Told(message)) => {
            log::log!("play: {message}");
            exit::FAILED
        }
        Err(Ended::Failed(message)) => {
            log::log!("play: {message}");
            if !quiet {
                message_box(&message);
            }
            exit::FAILED
        }
    }
}

/// Why PLAY started no game.
enum Ended {
    /// The player was told in a box of the route's (or nothing, quiet).
    Told(String),
    Failed(String),
}

impl From<String> for Ended {
    fn from(message: String) -> Self {
        Self::Failed(message)
    }
}

impl From<&str> for Ended {
    fn from(message: &str) -> Self {
        Self::Failed(message.into())
    }
}

fn launch(arguments: &[String], wait: bool, quiet: bool) -> Result<(), Ended> {
    let layout = Layout::find().map_err(|path| crate::text::wrong_folder(&path))?;
    log::start(&layout);
    patches::ready(&layout)?;
    let mut arguments = arguments.to_vec();
    // Explicit test/local servers and replays retain the caller's choice.
    let explicit = arguments
        .iter()
        .any(|a| a.starts_with("-server=") || a.starts_with("-match_replay="));
    if !explicit && let Some(config) = config()? {
        let language = win::documents().as_deref().and_then(win::game_language);
        let address = loop {
            let selection = match answering(config, quiet, language) {
                Ok(address) => break address,
                Err(selection) => selection,
            };
            let diagnosis = route::diagnose_game(config, &selection);
            let (caption, message) = route::words(diagnosis, language);
            let again = match diagnosis {
                // Back in a minute: the box probes every few seconds and
                // goes on by itself once the server answers.
                Diagnosis::Restarting => !quiet && waiting_box(&caption, &message, language, || route::game_answers(config)),
                _ => !quiet && retry_box(&caption, &message),
            };
            if !again {
                return Err(Ended::Told(format!("{}: {}", diagnosis.title(0), diagnosis.advice(0))));
            }
            log::log!("play: trying again");
        };
        arguments.push(format!("-server={address}"));
    }
    let game = format!("{}\\survarium_skhid.exe", layout.exe_dir);
    // Use Rust's Windows argument quoting; never assemble an unchecked command
    // line from user-supplied strings. The helper itself runs beside the game.
    let mut game = std::process::Command::new(&game)
        .args(&arguments)
        .current_dir(&layout.exe_dir)
        .spawn()
        .map_err(|e| format!("Cannot start the game: {e}"))?;
    if wait {
        let status = game
            .wait()
            .map_err(|e| format!("Cannot wait for the game: {e}"))?;
        if !status.success() {
            return Err(format!("The game ended with an error ({status}).").into());
        }
    }
    Ok(())
}

/// PLAY's route by the player's choice, at once: the relay; direct (the
/// launcher's check only tells of a block: the choice is the player's); Automatic,
/// the route found (`route::found`: the launcher's check, or a lost direct
/// link). Only with none found does PLAY check, in a box that says so
/// (`quiet`: none). `Err`: that check found no route (`route::diagnose_game`
/// tells why).
fn answering(config: &Config, quiet: bool, language: Option<u32>) -> Result<String, Selection> {
    let address = |via| match via {
        Via::Direct => config.direct.address(),
        Via::Relay => config.relay.address(),
    };
    let (via, why) = match route::choice() {
        Choice::Relay => (Via::Relay, "chosen"),
        Choice::Direct => (Via::Direct, "chosen"),
        Choice::Automatic => match route::found() {
            Some(found) => {
                route::learn(Learned::Played);
                (found.via(), found.word())
            }
            None => return checked(config, quiet, language),
        },
    };
    let address = address(via);
    log::log!("play: {via:?} {address} ({why}), unchecked");
    Ok(address)
}

/// PLAY's own check of the routes, in Automatic with none found before.
fn checked(config: &Config, quiet: bool, language: Option<u32>) -> Result<String, Selection> {
    log::log!("play: no route found yet: PLAY checks");
    let check = || route::check(config, "play");
    let selection = if quiet {
        check()
    } else {
        working_box(TITLE, diagnosis::checking(language.unwrap_or(0)), check)
    };
    match selection {
        Selection::Direct(address) | Selection::Relay(address) => Ok(address.to_string()),
        Selection::Unreachable => Err(selection),
    }
}
