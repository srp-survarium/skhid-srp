//! The unified server for the **0.26g** client: the 0.26g login server (TLS
//! sign-in, TCP 1234), lobby (UDP 1235) and match server (UDP 1236) of
//! `srp::v026g`, and the web page (HTTP 8088), in **one process** sharing a
//! single [`SessionStore`], so a client signed in at the login server is
//! recognised by the lobby and the match server. The 0.26g client is
//! started with `-server=<host>:1234`. The web page is the 0.26g one
//! (`web_server::run_026g`): its admin acts on the 0.26g accounts and
//! tells the lobby.
//!
//! The retained 0.21d servers (login, presence, lobby,
//! match) are kept and tested but no longer started here: the two lobbies
//! share the store's queue and squads, and the two login servers one port.
//!
//! Each server runs on its own supervised thread: a panic or error in one is
//! logged and the server is restarted, so no single server can take down the
//! whole process. Accounts are kept in `<SRP_DATA_DIR>/srp.db` (see
//! `session::persist` and `session::v026g::persist`); saver threads write
//! changes every 2 s, and Ctrl+C (or closing the console) writes the rest
//! before exiting.

mod logger;

use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use log::LevelFilter;
use session::SessionStore;
use session::v026g::GameAccounts;
use srp::v026g::{Options, Stack};

/// Delay before restarting a server that exited, to avoid a hot crash loop.
const RESTART_DELAY: Duration = Duration::from_secs(1);

type Run = Box<dyn Fn(Arc<SessionStore>) -> std::io::Result<()> + Send>;

fn main() {
    // A single level from RUST_LOG (e.g. `RUST_LOG=debug`), defaulting to info.
    let level = std::env::var("RUST_LOG")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(LevelFilter::Info);
    logger::init(level).expect("failed to install logger");
    log_panics();
    // Before any server: a bad setting would restart it forever.
    if let Err(error) = vostok::config::load() {
        log::error!("srp: {error}");
        std::process::exit(1);
    }
    // A minimum revision it can't read stops it, rather than let older games in.
    let min_patch_revision = match lobby_server::v026g::min_patch_revision_from_env() {
        Ok(min) => min,
        Err(error) => {
            log::error!("srp: {error}");
            std::process::exit(1);
        }
    };
    // The players' game files, before anything reads an item or a map:
    // the accounts are checked against their dictionary when they load,
    // the code's items and the minimum revision here.
    let data = match survarium::v026g::game_data::init() {
        Ok(data) => data,
        Err(error) => {
            log::error!("srp: {error}");
            std::process::exit(1);
        }
    };
    if let Err(error) = srp::v026g::check_game_data(min_patch_revision, data) {
        log::error!("srp: {error}");
        std::process::exit(1);
    }

    // Accounts are loaded before any server can sign one in. (The store's
    // queue is the 0.21d lobby's, which srp does not run: its window is
    // the default.)
    let store = match SessionStore::from_env(session::DEFAULT_GATHER_WINDOW) {
        Ok(store) => Arc::new(store),
        Err(error) => {
            log::error!("srp: cannot load the accounts: {error}");
            std::process::exit(1);
        }
    };
    store.spawn_saver();
    // What a 0.26g account owns: the `g26_*` tables of the same database.
    let accounts = match GameAccounts::from_env() {
        Ok(accounts) => Arc::new(accounts),
        Err(error) => {
            log::error!("srp: cannot load the 0.26g accounts: {error}");
            std::process::exit(1);
        }
    };
    accounts.spawn_saver();
    let on_exit = (store.clone(), accounts.clone());
    let stop = ctrlc::set_handler(move || {
        log::info!("srp: stopping; saving the accounts");
        on_exit.0.flush();
        on_exit.1.flush();
        log::info!("srp: stopped");
        std::process::exit(0);
    });
    if let Err(error) = stop {
        log::warn!("srp: no Ctrl+C handler ({error}); changes of the last seconds may be lost");
    }

    // The 0.26g login server, lobby and match server. An address in use
    // stops srp here: nothing would restart into a free one.
    let stack = match Stack::start(store.clone(), accounts, Options::from_env()) {
        Ok(stack) => stack,
        Err(error) => {
            log::error!(
                "srp: cannot start the 0.26g servers: {error} (if it is not about SRP_BOT_TUNING: \
                 is another srp running, or a port of SRP_LOGIN_PORT / SRP_LOBBY_PORT / \
                 SRP_MATCH_PORT taken?)"
            );
            std::process::exit(1);
        }
    };

    // The web page over the 0.26g accounts; it tells the lobby what it
    // changes. (`web_server::run` is the 0.21d page: kept, not started.)
    let game = stack.web_game();
    let servers: [(&str, Run); 1] = [(
        "web",
        Box::new(move |store| web_server::run_026g(store, game.clone())),
    )];

    let handles: Vec<_> = servers
        .into_iter()
        .map(|(name, run)| {
            let store = store.clone();
            thread::Builder::new()
                .name(name.to_string())
                .spawn(move || supervise(name, store, run))
                .expect("failed to spawn server thread")
        })
        .collect();

    log::info!(
        "srp: started the 0.26g login ({}), lobby ({}) and match ({}) servers and the web page",
        stack.login_addr,
        stack.lobby_addr,
        stack.match_addr
    );

    for handle in handles {
        let _ = handle.join();
    }
}

/// Record every panic's message and location in the originating server's log.
/// Connection handlers and the supervisor contain panics with `catch_unwind`,
/// which otherwise leaves only a "panicked" line and loses the reason to stderr.
fn log_panics() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
            .unwrap_or("<non-string panic payload>");
        let location = info
            .location()
            .map(|location| location.to_string())
            .unwrap_or_default();
        // Route by the source crate, since connection threads are unnamed.
        let target = [
            ("login-server", "login_server"),
            ("lobby-server", "lobby_server"),
            ("match-server", "match_server"),
        ]
        .into_iter()
        .find(|(directory, _)| location.contains(directory))
        .map_or("srp", |(_, target)| target);
        let thread = thread::current();
        let thread = thread.name().unwrap_or("<unnamed>");
        log::error!(target: target, "panic in thread '{thread}' at {location}: {message}");
        default_hook(info);
    }));
}

/// Run one server forever, restarting it (after a short delay) if it returns or
/// panics. Keeps a crashing server from bringing down the others.
fn supervise(name: &str, store: Arc<SessionStore>, run: Run) {
    loop {
        let store = store.clone();
        match std::panic::catch_unwind(AssertUnwindSafe(|| (*run)(store))) {
            Ok(Ok(())) => log::warn!("{name} server exited; restarting"),
            Ok(Err(error)) => log::error!("{name} server failed: {error}; restarting"),
            Err(_) => log::error!("{name} server panicked; restarting"),
        }
        thread::sleep(RESTART_DELAY);
    }
}
