//! PLAY's route selection runs in the updater, before creating the game.
//! No driver, administrator access, credentials, or game-memory hooks.
use crate::{exit, install::Layout, leftover, log, message_box, patches, retry_box, route, waiting_box, win};
use relay_client::{Config, Selection};
use updater_logic::diagnosis::Diagnosis;
use updater_logic::route::Choice;

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
        let address = loop {
            let selection = match answering(config) {
                Ok(address) => break address,
                Err(selection) => selection,
            };
            let diagnosis = route::diagnose_game(config, &selection);
            let language = win::documents().as_deref().and_then(win::game_language);
            let (caption, message) = route::words(diagnosis, language);
            let again = match diagnosis {
                // The row turns to auto: the relay answers, so on through
                // it, after the player has read why (quiet: at once).
                Diagnosis::Blocked => {
                    route::blocked();
                    quiet || retry_box(&caption, &message)
                }
                // Back in a minute: the box probes every few seconds and
                // goes on by itself once the server answers.
                Diagnosis::Restarting => !quiet && waiting_box(&caption, &message, language, || answering(config).is_ok()),
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
    // The launcher's PLAY (not the scripts' `wait`) first closes what an
    // earlier start of this folder left running; a replay is watched
    // beside a running game.
    let replay = arguments.iter().any(|a| a.starts_with("-match_replay="));
    if !wait && !replay {
        while !leftover::close(&game) {
            let language = win::documents().as_deref().and_then(win::game_language);
            if quiet || !retry_box(crate::text::TITLE, updater_logic::leftover::cannot_close(language)) {
                return Err(Ended::Told("an earlier game of this folder still runs: not started".into()));
            }
            log::log!("play: trying again");
        }
    }
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

/// PLAY's route by the player's choice: the relay unprobed; direct, or
/// (Automatic) the first that answers, probed. `Err`: what the probe found
/// when that is no route of the choice's (`route::diagnose_game` tells why).
fn answering(config: &Config) -> Result<String, Selection> {
    let choice = route::choice();
    if choice == Choice::Relay {
        let address = config.relay.address();
        log::log!("play: the relay {address}, as chosen");
        return Ok(address);
    }
    let selection = relay_client::select(config);
    match (choice, &selection) {
        (_, Selection::Direct(address)) => {
            log::log!("play: direct route {address} answered TCP, lobby UDP and match UDP");
            Ok(address.to_string())
        }
        (Choice::Automatic, Selection::Relay(address)) => {
            log::log!("play: direct route unavailable; using relay {address}");
            Ok(address.to_string())
        }
        _ => {
            log::log!("play: no route by {}: {selection:?}", choice.word());
            Err(selection)
        }
    }
}
