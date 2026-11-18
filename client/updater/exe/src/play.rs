//! PLAY's route selection runs in the updater, before creating the game.
//! No driver, administrator access, credentials, or game-memory hooks.
use crate::{exit, install::Layout, log, message_box, patches};
use relay_client::{Config, Selection};

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
    let game_arguments: Vec<_> = arguments
        .iter()
        .filter(|a| a.as_str() != "quiet")
        .cloned()
        .collect();
    let result = launch(&game_arguments);
    match result {
        Ok(()) => exit::OK,
        Err(message) => {
            log::log!("play: {message}");
            if !quiet {
                message_box(&message);
            }
            exit::FAILED
        }
    }
}

fn launch(arguments: &[String]) -> Result<(), String> {
    let layout = Layout::find().map_err(|path| crate::text::wrong_folder(&path))?;
    log::start(&layout);
    patches::ready(&layout)?;
    let mut arguments = arguments.to_vec();
    // Explicit test/local servers and replays retain the caller's choice.
    let explicit = arguments
        .iter()
        .any(|a| a.starts_with("-server=") || a.starts_with("-match_replay="));
    if !explicit && let Some(config) = config()? {
        let address = match relay_client::select(config) {
            Selection::Direct(address) => {
                log::log!("play: direct route {address} answered TCP, lobby UDP and match UDP");
                address
            }
            Selection::Relay(address) => {
                log::log!("play: direct route unavailable; using relay {address}");
                address
            }
            Selection::Unreachable => return Err(
                "Cannot reach the game server directly or through the relay. Please try again shortly.".into()),
        };
        arguments.push(format!("-server={address}"));
    }
    let game = format!("{}\\survarium_skhid.exe", layout.exe_dir);
    // Use Rust's Windows argument quoting; never assemble an unchecked command
    // line from user-supplied strings. The helper itself runs beside the game.
    std::process::Command::new(&game)
        .args(&arguments)
        .current_dir(&layout.exe_dir)
        .spawn()
        .map_err(|e| format!("Cannot start the game: {e}"))?;
    Ok(())
}
