//! The 0.26g stack of srp: the login server (TLS sign-in), the lobby (one
//! UDP link a client) and the match server (UDP), sharing one
//! [`SessionStore`], the 0.26g accounts ([`GameAccounts`]) and the finished
//! matches' [`Results`].
//!
//! The servers know nothing of each other; what joins them is here:
//!
//! - [`RegistryHost`]: the lobby's [`MatchHost`] over the match server's
//!   [`MatchRegistry`]. The lobby makes a match there and tells its players
//!   the ticket's address.
//! - [`Settler`]: the match server's results sink. A finished match's
//!   [`match_server::v026g::MatchResult`] is settled by
//!   `session::v026g::rewards` (the accounts are changed and written), and
//!   the lobby is told: the players get `85 00`, what the match changed and
//!   the results screen.
//! - [`WebLink`]: what the web page tells the lobby (`web_server::LobbyLink`
//!   over the lobby's [`Inbox`]): an account it changed, the operator's
//!   text. [`Stack::web_game`] is what `web_server::run_026g` is given.
//! - [`Stack`]: the three servers and the session collector, each on a
//!   thread that is started again if it fails.
//!
//! The retained 0.21d servers (login, presence, lobby,
//! match) are not started with it: the two lobbies share the store's queue
//! and squads, and the two login servers one port.

use std::io;
use std::net::{SocketAddr, TcpListener, UdpSocket};
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use lobby_server::v026g::{self as lobby, Inbox, Lobby, MatchFinished, MatchHost};
use login_server::v026g::{self as login, Endpoint, RevisionGate};
use match_server::v026g::{self as matches, MatchRegistry};
use session::LockExt;
use session::SessionStore;
use session::v026g::replays::{ReplaySettings, Replays};
use session::v026g::rewards::{self, Results, Terms};
use session::v026g::{GameAccounts, quests, wire};
use survarium::v026g::drops::Rng;

/// Delay before restarting a server that exited, to avoid a hot crash loop.
const RESTART_DELAY: Duration = Duration::from_secs(1);

/// The match server's registry as the lobby's match host: each struct is
/// moved over field for field.
pub struct RegistryHost(pub Arc<MatchRegistry>);

/// The patches of `resources.db` the web page offers the players' updater
/// (`GET /patches`): those the server's game data of this process was made
/// of (`survarium::v026g::game_data`), in the game's order.
pub fn offered_patches() -> Vec<web_server::OfferedPatch> {
    survarium::v026g::game_data::shared()
        .provenance
        .offered()
        .map(|(name, sha256, urls)| web_server::OfferedPatch {
            name,
            sha256: sha256.to_owned(),
            urls: urls.to_vec(),
        })
        .collect()
}

/// What srp checks of the server's game data `data` once it is read
/// (`game_data::init`), before it loads an account: every item, id and
/// award the lobby's code names is in it (`session::v026g::known::
/// check_code`), and `min_patch_revision` (`SRP_MIN_PATCH_REVISION`) keeps
/// out every game without its newest patch
/// (`lobby_server::v026g::check_min_patch_revision`). An error stops srp.
pub fn check_game_data(
    min_patch_revision: u8,
    data: &survarium::v026g::game_data::GameData,
) -> Result<(), String> {
    session::v026g::known::check_code().map_err(|error| error.to_string())?;
    lobby::check_min_patch_revision(min_patch_revision, data)
}

/// How far a match is, as the launcher's page has it
/// (`session::v026g::matches_now`).
fn phase(phase: matches::LivePhase) -> session::v026g::matches_now::Phase {
    use session::v026g::matches_now::Phase;
    match phase {
        matches::LivePhase::Gathering => Phase::Gathering,
        matches::LivePhase::Countdown => Phase::Countdown,
        matches::LivePhase::Playing => Phase::Playing,
        matches::LivePhase::Over => Phase::Over,
    }
}

impl web_server::MatchCounter for RegistryHost {
    fn active_matches(&self) -> usize {
        self.0.match_count()
    }

    fn live_matches(&self) -> Vec<web_server::LiveMatch> {
        self.0
            .live_matches(std::time::Instant::now())
            .into_iter()
            .map(|live| web_server::LiveMatch {
                map_id: live.map_id,
                phase: phase(live.phase),
                players: live.players,
                played: live.played,
                time_left: live.time_left,
                standing: live.standing.map(|standing| web_server::LiveStanding {
                    points: standing.points,
                    leader: (standing.winner != matches::DRAW)
                        .then_some(usize::from(standing.winner)),
                    age: standing.age,
                }),
            })
            .collect()
    }
}

impl MatchHost for RegistryHost {
    fn create(&self, setup: lobby::MatchSetup) -> Result<lobby::MatchTicket, String> {
        let pool = setup.pool;
        let setup = matches::MatchSetup {
            map_id: setup.map_id,
            match_level: setup.match_level,
            players: setup
                .players
                .into_iter()
                .map(|player| matches::MatchPlayer {
                    session_id: player.session_id,
                    account_id: player.account_id,
                    profile: player.profile,
                    bot: player.bot,
                    driven: player.driven,
                })
                .collect(),
            // The game's own weights: the ones the rewards are counted
            // with (`rewards::Terms::at`).
            score_weights: None,
            vs_bots: setup.vs_bots,
        };
        let ticket = match pool {
            session::QueuePool::Standard => self.0.create(setup),
            session::QueuePool::GrowingRoster => self.0.create_growing(setup),
        }
        .map_err(|error| error.to_string())?;
        Ok(lobby::MatchTicket {
            match_id: ticket.match_id,
            host: ticket.host,
            port: ticket.port,
        })
    }

    fn join_running(&self, match_id: u32, players: Vec<lobby::MatchPlayer>) -> bool {
        let players = players
            .into_iter()
            .map(|player| matches::MatchPlayer {
                session_id: player.session_id,
                account_id: player.account_id,
                profile: player.profile,
                bot: player.bot,
                driven: player.driven,
            })
            .collect();
        self.0.join_running_group(match_id, players).is_ok()
    }

    fn admits(&self, match_id: u32, now: std::time::Instant) -> bool {
        self.0.admits(match_id, now)
    }

    fn closed(
        &self,
        match_id: u32,
        now: std::time::Instant,
    ) -> Option<session::matchmaking_policy::Closed> {
        self.0.closed(match_id, now)
    }

    fn live(&self, match_id: u32, now: std::time::Instant) -> Option<lobby::LiveView> {
        let live = self.0.live_match(match_id, now)?;
        Some(lobby::LiveView {
            present: live.present,
            points: live.standing.map(|standing| standing.points),
            kills: live.kills,
            played: live
                .played
                .filter(|_| live.phase == matches::LivePhase::Playing),
            // Nothing is left of a match that has its result.
            time_left: if live.phase == matches::LivePhase::Over {
                Some(Duration::ZERO)
            } else {
                live.time_left
            },
        })
    }

    fn is_open(&self, match_id: u32) -> bool {
        self.0.is_open(match_id)
    }

    fn reseat(&self, match_id: u32, account_id: u32, session_id: u32) -> bool {
        self.0.reseat(match_id, account_id, session_id).is_some()
    }
}

/// The match server's result as the rewards take it: the same fields.
pub fn match_result(result: matches::MatchResult) -> rewards::MatchResult {
    rewards::MatchResult {
        match_id: result.match_id,
        map_id: result.map_id,
        game_mode: result.game_mode,
        match_level: result.match_level,
        end: match result.end {
            matches::MatchEnd::ModeRule => rewards::MatchEnd::ModeRule,
            matches::MatchEnd::TimeLimit => rewards::MatchEnd::TimeLimit,
            matches::MatchEnd::Deserted => rewards::MatchEnd::Deserted,
            matches::MatchEnd::NoShow => rewards::MatchEnd::NoShow,
        },
        winner: result.winner,
        team_points: result.team_points,
        duration_ms: result.duration_ms,
        played_ms: result.played_ms,
        trusted_snapshot_ms: result.trusted_snapshot_ms,
        trusted_by: result.trusted_by,
        vs_bots: result.vs_bots,
        players: result
            .players
            .into_iter()
            .map(|player| rewards::PlayerResult {
                place: player.place,
                session_id: player.session_id,
                account_id: player.account_id,
                name: player.name,
                team: player.team,
                driven: player.driven,
                entered: player.entered,
                present_at_end: player.present_at_end,
                left: player.left,
                time_in_match_ms: player.time_in_match_ms,
                statistics: player.statistics,
                score: player.score,
                kills: player.kills,
                deaths: player.deaths,
                longest_killstreak: player.longest_killstreak,
                kit_changes: player.kit_changes,
                supplies: player.supplies,
            })
            .collect(),
    }
}

/// Where the match server's finished matches go: the rewards, the
/// database, and the lobby.
pub struct Settler {
    store: Arc<SessionStore>,
    accounts: Arc<GameAccounts>,
    results: Arc<Results>,
    inbox: Arc<Inbox>,
    rng: Rng,
}

impl Settler {
    pub fn new(
        store: Arc<SessionStore>,
        accounts: Arc<GameAccounts>,
        results: Arc<Results>,
        inbox: Arc<Inbox>,
    ) -> Self {
        Self {
            store,
            accounts,
            results,
            inbox,
            rng: Rng::from_time(),
        }
    }

    /// One finished match: called on the match server's thread, before its
    /// clients hear the end.
    pub fn match_ended(&mut self, result: matches::MatchResult) {
        // Each account's latest place: one it left and came back from is
        // nobody's (the rewards take the same, `settle`).
        let result = match_result(result).latest_places();
        let match_id = result.match_id;
        let terms = Terms::at(quests::now());
        match self
            .results
            .settle(&result, &self.accounts, &terms, &mut self.rng)
        {
            Some(report) => {
                log::info!(
                    "Match {match_id:#x} settled ({:?}, winner {:?}, {} players): {}",
                    result.end,
                    result.winner,
                    report.players.len(),
                    report
                        .players
                        .iter()
                        .zip(&result.players)
                        .map(|(player, place)| format!(
                            "{} +{} exp +{} silver",
                            place.name, player.reward.experience, player.reward.money
                        ))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            None => log::info!("Match {match_id:#x}: nobody came; nothing to settle"),
        }
        // Written before anyone is told: what a player is shown is kept.
        self.accounts.flush();
        for player in &result.players {
            if self.store.is_online(player.account_id) {
                self.inbox.account_changed(player.account_id);
            }
        }
        self.inbox.match_finished(MatchFinished { match_id });
    }
}

/// The web page's word to the lobby.
pub struct WebLink {
    accounts: Arc<GameAccounts>,
    inbox: Arc<Inbox>,
}

impl web_server::LobbyLink for WebLink {
    /// The client is sent its account again (`Inbox::account_changed`:
    /// inventory, account, money, level and skills, reputations, quests),
    /// then its services (0x90: the premium time), which that does not
    /// carry and the admin page can change.
    fn account_changed(&self, account_id: u32) {
        self.inbox.account_changed(account_id);
        if let Some(account) = self.accounts.get(account_id) {
            let services = wire::services(&account.locked(), quests::now());
            self.inbox.push(account_id, vec![services]);
        }
    }

    fn set_admin_text(&self, first: &str, second: &str) {
        self.inbox.set_admin_text(first, second);
    }

    fn clear_admin_text(&self) {
        self.inbox.clear_admin_text();
    }

    fn admin_text(&self) -> Option<(String, String)> {
        self.inbox.admin_text()
    }
}

/// What the stack is started with.
#[derive(Clone, Debug)]
pub struct Options {
    /// `host:port` of the login server's TCP listener.
    pub login_bind: String,
    /// The login server's public host (see [`Endpoint::public_host`]).
    pub login_public_host: String,
    /// The lobby: its UDP address, its public host and the rest.
    pub lobby: lobby::Settings,
    /// `host:port` of the match server's UDP socket, and the host its
    /// clients are told.
    pub match_bind: String,
    pub match_public_host: String,
    /// The folder with `survarium_login_server.key` and `.crt`.
    pub certs: PathBuf,
    /// **Testing only** (`SRP_BOT_ACCOUNTS`, `session::v026g::bots`): the
    /// names of the accounts that are test bots (`srp-bot`). Empty, the
    /// default: nothing about bots is on.
    pub bots: Vec<String>,
    /// "Watch replay" (`SRP_REPLAY_DIR`, `SRP_GAME_MODES_DIR`; `None`:
    /// off): the folders the match server converts recordings into, the
    /// lobby gives tokens for and the web page serves from.
    pub replays: Option<ReplaySettings>,
}

impl Options {
    /// What the environment asks for (`vostok::config` and the lobby's own
    /// settings), with the certificates of `./certs`: srp is run from the
    /// repository's root. By default everything is on 127.0.0.1: the login
    /// server on TCP 1234, the lobby on UDP 1235, the match server on UDP
    /// 1236.
    pub fn from_env() -> Self {
        let config = vostok::config::get();
        Self {
            login_bind: config.login_server.bind_addr(),
            login_public_host: config.login_server.public_host.clone(),
            lobby: lobby::Settings::from_env(),
            match_bind: config.match_server.bind_addr(),
            match_public_host: config.match_server.public_host.clone(),
            certs: PathBuf::from("./certs"),
            bots: session::v026g::bots::from_env(),
            replays: ReplaySettings::from_env(),
        }
    }

    /// Everything on 127.0.0.1, on free ports: for the tests.
    pub fn local(certs: &Path) -> Self {
        Self {
            login_bind: "127.0.0.1:0".to_owned(),
            login_public_host: "127.0.0.1".to_owned(),
            lobby: lobby::Settings::local("127.0.0.1:0"),
            match_bind: "127.0.0.1:0".to_owned(),
            match_public_host: "127.0.0.1".to_owned(),
            certs: certs.to_owned(),
            bots: Vec::new(),
            replays: None,
        }
    }
}

/// The running 0.26g servers.
pub struct Stack {
    pub store: Arc<SessionStore>,
    pub accounts: Arc<GameAccounts>,
    pub results: Arc<Results>,
    pub registry: Arc<MatchRegistry>,
    pub inbox: Arc<Inbox>,
    /// "Watch replay": what the match server's conversions, the lobby's
    /// asks and the web page's downloads share (off when not set).
    pub replays: Arc<Replays>,
    /// Where each server listens (the ports given, for a port of 0).
    pub login_addr: SocketAddr,
    pub lobby_addr: SocketAddr,
    pub match_addr: SocketAddr,
    stop: Arc<AtomicBool>,
    /// The lobby's and the match server's threads: the ones [`Stack::stop`]
    /// waits for.
    threads: Vec<JoinHandle<()>>,
}

impl Stack {
    /// Bind the three servers and start them, with the collector that ends
    /// sessions nothing was seen of. An address that cannot be bound is an
    /// error here; a server that fails later is logged and started again.
    pub fn start(
        store: Arc<SessionStore>,
        accounts: Arc<GameAccounts>,
        options: Options,
    ) -> io::Result<Self> {
        // Kept in the accounts' repository: a restart forgets neither a
        // results screen not shown yet nor an account's play period.
        let results = Results::attached(&accounts);
        let inbox = Inbox::new();
        let stop = Arc::new(AtomicBool::new(false));
        // The lobby (who is a bot when a match is made), the rewards and
        // the ranking read it from the accounts.
        accounts.set_bots(&options.bots);
        if !options.bots.is_empty() {
            log::warn!(
                "srp: SRP_BOT_ACCOUNTS is set: the accounts named {} are test bots. A match of \
                 one real client and bots trusts that client's world on its word alone. For \
                 testing only: never on a public server",
                options.bots.join(", ")
            );
        }

        // The match server first: the registry tells the port it got.
        let match_socket = UdpSocket::bind(&options.match_bind)?;
        let match_addr = match_socket.local_addr()?;
        let registry = Arc::new(MatchRegistry::new(
            &options.match_public_host,
            match_addr.port(),
        ));

        // One for the process: the match server converts, the lobby gives
        // tokens, the web page takes them.
        let replays = Arc::new(matches::replays_of(options.replays.clone()));

        let host: Arc<dyn MatchHost> = Arc::new(RegistryHost(registry.clone()));
        let bind_lobby = {
            let (store, accounts, results, inbox, replays) = (
                store.clone(),
                accounts.clone(),
                results.clone(),
                inbox.clone(),
                replays.clone(),
            );
            move |settings: lobby::Settings| {
                Lobby::bind(
                    settings,
                    store.clone(),
                    accounts.clone(),
                    results.clone(),
                    host.clone(),
                    inbox.clone(),
                )
                .map(|lobby| lobby.with_replays(replays.clone()))
            }
        };
        let first_lobby = bind_lobby(options.lobby.clone())?;
        let lobby_addr = first_lobby.local_addr()?;
        // Started again, it is on the address it got.
        let lobby_settings = lobby::Settings {
            bind: lobby_addr.to_string(),
            ..options.lobby.clone()
        };

        let listener = TcpListener::bind(&options.login_bind)?;
        let login_addr = listener.local_addr()?;
        // Here, not on every sign-in: srp started from another folder
        // would refuse them all.
        for file in ["survarium_login_server.key", "survarium_login_server.crt"] {
            let path = options.certs.join(file);
            if !path.is_file() {
                return Err(io::Error::other(format!(
                    "the login server's {} is missing (run srp from the repository's root)",
                    path.display()
                )));
            }
        }
        let endpoint = Endpoint {
            public_host: options.login_public_host.clone(),
            lobby_host: options.lobby.public_host.clone(),
            lobby_port: lobby_addr.port(),
            local_peers: options.lobby.local_peers,
            key: options.certs.join("survarium_login_server.key"),
            certificate: options.certs.join("survarium_login_server.crt"),
            // Older games are kept out from their sign-in on; the lobby
            // refuses the sessions they had before (`patch_check`).
            revision_gate: RevisionGate {
                min: options.lobby.min_patch_revision,
                exempt: options.bots.clone(),
            },
        };

        let mut threads = Vec::new();
        match matches::replay::start(replays.clone()) {
            Some(Ok(thread)) => threads.push(thread),
            Some(Err(error)) => log::error!("Replays: the conversion thread failed: {error}"),
            None => log::info!("Replays are off (SRP_REPLAY_DIR, SRP_GAME_MODES_DIR)"),
        }
        {
            let (registry, store, accounts, results, inbox, stop, replays) = (
                registry.clone(),
                store.clone(),
                accounts.clone(),
                results.clone(),
                inbox.clone(),
                stop.clone(),
                replays.clone(),
            );
            let mut socket = Some(match_socket);
            let flag = stop.clone();
            threads.push(supervise("match", stop, move || {
                let socket = match socket.take() {
                    Some(socket) => socket,
                    None => UdpSocket::bind(match_addr)?,
                };
                let mut settler = Settler::new(
                    store.clone(),
                    accounts.clone(),
                    results.clone(),
                    inbox.clone(),
                );
                matches::run_on_with_replays(
                    &socket,
                    registry.clone(),
                    store.clone(),
                    move |result| settler.match_ended(result),
                    &flag,
                    Some(replays.clone()),
                )
            }));
        }
        {
            let mut first = Some(first_lobby);
            threads.push(supervise("lobby", stop.clone(), move || {
                match first.take() {
                    Some(lobby) => lobby,
                    None => bind_lobby(lobby_settings.clone())?,
                }
                .run()
            }));
        }
        {
            let store = store.clone();
            let mut first = Some(listener);
            // Never joined: it waits in `accept`.
            supervise("login", stop.clone(), move || {
                let listener = match first.take() {
                    Some(listener) => listener,
                    None => TcpListener::bind(login_addr)?,
                };
                login::serve(&listener, store.clone(), &endpoint)
            });
        }
        {
            let store = store.clone();
            supervise("collector", stop.clone(), move || {
                login::run_collector(store.clone());
                Ok(())
            });
        }
        log::info!(
            "srp: 0.26g servers started: login TCP {login_addr}, lobby UDP {lobby_addr}, match \
             UDP {match_addr}"
        );
        Ok(Self {
            store,
            accounts,
            results,
            registry,
            inbox,
            replays,
            login_addr,
            lobby_addr,
            match_addr,
            stop,
            threads,
        })
    }

    /// What makes the web page a 0.26g one (`web_server::run_026g`): these
    /// accounts, and the lobby told of what the page changes.
    pub fn web_game(&self) -> web_server::Game {
        web_server::Game {
            accounts: self.accounts.clone(),
            matches: Arc::new(RegistryHost(self.registry.clone())),
            lobby: Arc::new(WebLink {
                accounts: self.accounts.clone(),
                inbox: self.inbox.clone(),
            }),
            patches: offered_patches(),
            replays: Some(self.replays.clone()),
        }
    }

    /// Stop the lobby and the match server and wait for them; what the
    /// accounts and the store hold is written. (The login server's thread
    /// waits in `accept` and ends with the process.)
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.inbox.stop();
        // A conversion under way ends first (about two seconds at most).
        self.replays.stop();
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
        self.accounts.flush();
        self.store.flush();
    }
}

/// Run one server on a thread of its own, starting it again (after a short
/// delay) if it returns or panics, until `stop` is set: a failing server
/// must not take the others down.
fn supervise(
    name: &'static str,
    stop: Arc<AtomicBool>,
    mut run: impl FnMut() -> io::Result<()> + Send + 'static,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name(name.to_owned())
        .spawn(move || {
            loop {
                let outcome = std::panic::catch_unwind(AssertUnwindSafe(&mut run));
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                match outcome {
                    Ok(Ok(())) => log::warn!("{name} server exited; restarting"),
                    Ok(Err(error)) => log::error!("{name} server failed: {error}; restarting"),
                    Err(_) => log::error!("{name} server panicked; restarting"),
                }
                thread::sleep(RESTART_DELAY);
            }
        })
        .expect("failed to spawn server thread")
}
