//! `srp-bot`: scripted 0.26g clients that take places in a match, so that
//! one real client gets one. **For testing only.** What a bot does and
//! does not do: `srp::bot`. The server must be told which accounts are
//! bots: `SRP_BOT_ACCOUNTS=bot-1` (see `docs/SRP-026G.md`,
//! "A match alone").

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{SystemTime, UNIX_EPOCH};

use srp::bot::{self, Config, RoomTarget};

const DEFAULT_SERVER: &str = "127.0.0.1:1234";
const DEFAULT_NAME: &str = "bot-1";
/// The password of a bot nobody gave one: good for a test on one PC and
/// nothing else.
const DEFAULT_PASSWORD: &str = "srp-local-test-bot";

const USAGE: &str = "\
srp-bot: scripted Survarium 0.26g clients for testing srp alone.

Each bot signs in (a new name registers itself), presses Play, takes its
place in the match, stays to the end without playing, takes the results
and presses Play again.

  srp-bot [--server HOST:PORT] [--name NAME] [--password PASSWORD]
          [--count N] [--once] [--kit ID] [--drive] [--revision N]
          [--room KEY] [--pin PIN]

  --server    the login server, as the game's -server= (default
              127.0.0.1:1234; or SRP_BOT_SERVER)
  --name      the first bot's user name (default bot-1; or SRP_BOT_NAME)
  --password  the bots' password (or SRP_BOT_PASSWORD; a built-in one for
              local tests when neither is given). Never printed.
  --count N   N bots: bot-1 .. bot-N (default 1). The first waits in a
              room; the others join once somebody else is in it.
  --room KEY  join this existing room instead of Play (all bots join at once)
  --pin PIN   the room PIN, with --room; never printed
  --check-data  validate SRP_GAME_DATA and its dictionary, then exit without connecting
  --once      stop after one match
  --revision N  the patch revision the sign-in says (default: the newest,
              the late join's); only 5 or later plays in rooms
  --kit ID    press Play with this kit id (default: the kit the account
              plays)
  --drive     in a match, send inputs: a fixed script (walk, turn a quarter,
              walk, sprint, crouch, fire, reload, weapon 2, turn back),
              again and again; each input is printed (srp::drive)

The server must name the bots' accounts: SRP_BOT_ACCOUNTS=bot-1,bot-2
(testing only: never on a public server).
";

fn fail(text: &str) -> ! {
    eprintln!("srp-bot: {text}\n\n{USAGE}");
    std::process::exit(2);
}

/// `HH:MM:SS` of the day, UTC: the servers' logs are in UTC too.
fn clock() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
        % 86_400;
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

fn main() {
    let env = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
    let mut server = env("SRP_BOT_SERVER").unwrap_or_else(|| DEFAULT_SERVER.to_owned());
    let mut name = env("SRP_BOT_NAME").unwrap_or_else(|| DEFAULT_NAME.to_owned());
    let mut password = env("SRP_BOT_PASSWORD");
    let mut count = 1_usize;
    let mut once = false;
    let mut kit = None;
    let mut drive = false;
    let mut check_data = false;
    let mut room_key = None;
    let mut room_pin = None;
    let mut revision = survarium::v026g::match_msgs::late_join::PATCH_REVISION;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        // `--name=value` and `--name value`.
        let (flag, inline) = match arg.split_once('=') {
            Some((flag, value)) => (flag.to_owned(), Some(value.to_owned())),
            None => (arg.clone(), None),
        };
        let mut value = |what: &str| {
            inline
                .clone()
                .or_else(|| args.next())
                .unwrap_or_else(|| fail(&format!("{flag} needs {what}")))
        };
        match flag.as_str() {
            "--server" => server = value("a host:port"),
            "--name" => name = value("a user name"),
            "--password" => password = Some(value("a password")),
            "--count" => {
                count = value("a number")
                    .parse()
                    .ok()
                    .filter(|count| (1..=19).contains(count))
                    .unwrap_or_else(|| fail("--count must be 1 to 19"));
            }
            "--kit" => {
                kit = Some(
                    value("a kit id")
                        .parse()
                        .unwrap_or_else(|_| fail("--kit must be a number")),
                );
            }
            "--revision" => {
                revision = value("a number")
                    .parse()
                    .unwrap_or_else(|_| fail("--revision must be 0 to 255"));
            }
            "--room" => {
                room_key = Some(
                    value("a room key")
                        .parse::<u32>()
                        .unwrap_or_else(|_| fail("--room must be 1 to 99999")),
                )
            }
            "--pin" => {
                room_pin = Some(
                    value("a PIN")
                        .parse::<u16>()
                        .unwrap_or_else(|_| fail("--pin must be 0 to 9999")),
                )
            }
            "--check-data" => check_data = true,
            "--once" => once = true,
            "--drive" => drive = true,
            "--help" | "-h" => {
                print!("{USAGE}");
                return;
            }
            other => fail(&format!("unknown argument `{other}`")),
        }
    }
    if name.is_empty() || name.len() > 255 {
        fail("the name must be 1 to 255 bytes");
    }
    let password = password.unwrap_or_else(|| DEFAULT_PASSWORD.to_owned());

    // Tools otherwise silently use the embedded test archive. Live local
    // accounts contain ported items that only the server's current DB knows.
    if std::env::var_os(survarium::v026g::game_data::GAME_DATA_ENV).is_some() {
        survarium::v026g::game_data::init().unwrap_or_else(|error| fail(&error.to_string()));
    } else if check_data {
        fail("--check-data needs SRP_GAME_DATA");
    }
    if check_data {
        let dictionary = survarium::v026g::dictionary::Dictionary::shared();
        println!("game data loaded: {} items, {} maps", dictionary.items.len(), dictionary.maps.len());
        return;
    }
    let room = match (room_key, room_pin) {
        (Some(key), pin) => Some(
            RoomTarget::new(key, pin)
                .unwrap_or_else(|| fail("--room must be 1 to 99999 and --pin 0 to 9999")),
        ),
        (None, Some(_)) => fail("--pin requires --room"),
        (None, None) => None,
    };
    let names = bot::names(&name, count);
    println!(
        "{} srp-bot: {} at {server}{}. The server must have SRP_BOT_ACCOUNTS={}. Ctrl+C stops.",
        clock(),
        names.join(", "),
        if once { ", one match" } else { "" },
        names.join(",")
    );
    let configs = names
        .into_iter()
        .map(|name| Config {
            server: server.clone(),
            name,
            password: password.clone(),
            once,
            kit,
            drive,
            revision,
            room,
        })
        .collect();
    let print: bot::Print = Arc::new(|line| println!("{} {line}", clock()));
    // Ctrl+C ends the process; the lobby ends the sessions of links that
    // went silent.
    bot::run(configs, print, &AtomicBool::new(false));
}
