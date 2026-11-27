//! `srp`: the 0.69d server. Settings from the config file (`config/srp.example.toml`, docs/notes/config.md); operator
//! guide `docs/guide-server.md`.
//!
//! ```text
//! srp [--config FILE] setup [--game DIR] [--address IP]   check this computer, write the config, database, login key
//! srp [--config FILE] [run]          the world host (when none runs yet) + login + lobby + match server
//! srp [--config FILE] check          check the config, print the effective settings, exit
//! srp extract [--game DIR] --out DIR      build the game tables from a game install into DIR (setup does this too)
//! srp [--config FILE] selftest [PROJECT]   start a world host of its own, build one start world, play a bot match
//! srp [--config FILE] world-host     the world host daemon alone, in the foreground
//! srp [--config FILE] account add|password <name> <password>
//! srp [--config FILE] ban|unban|comments ...   admin commands on the account database
//! ```
//! Without `--config` the file named by `SRP_CONFIG`, else `./srp.toml`, else the defaults.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

const USAGE: &str = "usage: srp [--config FILE] [setup [--game DIR] [--address IP] | extract [--game DIR] --out DIR | run | check | selftest [PROJECT] | world-host | account add|password <name> <password> | ban <account> <ban|suspend> <reason 0..15> <minutes|0> | unban <account> | comments [account [limit]]]";

fn fail(e: impl std::fmt::Display, code: i32) -> ! {
    eprintln!("srp: {e}");
    std::process::exit(code);
}

/// `--name VALUE` out of `args` (removed), or None.
fn take(args: &mut Vec<String>, name: &str) -> Option<String> {
    let i = args.iter().position(|a| a == name)?;
    let Some(v) = args.get(i + 1).cloned() else { fail(USAGE, 2) };
    args.drain(i..i + 2);
    Some(v)
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut path: Option<PathBuf> = std::env::var_os("SRP_CONFIG").filter(|p| !p.is_empty()).map(PathBuf::from);
    if let Some(p) = take(&mut args, "--config") {
        path = Some(PathBuf::from(p));
    }
    if args.first().map(String::as_str) == Some("extract") {
        // no config needed: the tables of a game install into a folder (tests: SRP_TEST_DATA or the workspace's data/)
        let game = take(&mut args, "--game").map(PathBuf::from);
        let out = take(&mut args, "--out").map(PathBuf::from);
        let (Some(out), 1) = (out, args.len()) else { fail(USAGE, 2) };
        let g = match game {
            Some(p) => srp::setup::game_at(&p).unwrap_or_else(|| fail(format!("no survarium.exe in {}", p.display()), 2)),
            None => srp::setup::search_game().unwrap_or_else(|| fail("no Survarium install found: pass --game <game folder>", 2)),
        };
        if let Err(e) = srp::setup::check_exe(&g.exe) {
            fail(e, 2);
        }
        match srp::setup::extract_tables(&g, &out) {
            Ok((n, bytes, secs)) => println!("{n} game tables ({:.1} MB) from {} written to {} in {secs:.1} s", bytes as f64 / 1e6, g.root.display(), out.display()),
            Err(e) => fail(e, 2),
        }
        return;
    }
    if args.first().map(String::as_str) == Some("setup") {
        // before the config is loaded: setup writes it when there is none
        let game = take(&mut args, "--game").map(PathBuf::from);
        let address = take(&mut args, "--address").map(|a| a.parse().unwrap_or_else(|_| fail(format!("--address {a}: an IPv4 address like 203.0.113.20"), 2)));
        if args.len() > 1 {
            fail(USAGE, 2);
        }
        let config = path.unwrap_or_else(|| PathBuf::from(srp::settings::DEFAULT_FILE));
        std::process::exit(srp::setup::run(&srp::setup::Options { config, game, address }));
    }
    let (mut config, file) = match srp::Config::load(path.as_deref()) {
        Ok(c) => c,
        Err(e) => fail(e, 2),
    };
    // debug only, never in the config file: record every online match's input stream
    config.record_dir = std::env::var_os("SRP_RECORD_DIR").filter(|d| !d.is_empty()).map(PathBuf::from);
    let cmd = args.first().map(String::as_str).unwrap_or("run");
    match cmd {
        "run" if args.len() <= 1 => {}
        "check" => std::process::exit(check(&config, file.as_deref())),
        "selftest" if args.len() <= 2 => std::process::exit(srp::world_host::selftest(&config, args.get(1).map(String::as_str))),
        "world-host" => match srp::world_host::run_foreground(&config) {
            Ok(code) => std::process::exit(code),
            Err(e) => fail(e, 2),
        },
        "help" | "--help" | "-h" => {
            println!("{USAGE}");
            return;
        }
        "account" => {
            let store = match accounts::Store::open(&config.db_path) {
                Ok(s) => Arc::new(s),
                Err(e) => fail(format!("database {}: {e}", config.db_path.display()), 2),
            };
            let data = srp::load_tables(&config).unwrap_or_else(|e| fail(e, 2));
            match srp::account_admin(&args, &config, store, Arc::new(data)) {
                Ok(line) => println!("{line}"),
                Err(e) => fail(e, 2),
            }
            return;
        }
        _ => {
            let store = match accounts::Store::open(&config.db_path) {
                Ok(s) => s,
                Err(e) => fail(format!("database {}: {e}", config.db_path.display()), 2),
            };
            match srp::admin(&args, &store) {
                Ok(line) => println!("{line}"),
                Err(e) => fail(e, 2),
            }
            return;
        }
    }
    // SRP_* environment variables are not settings: say so instead of silently starting with defaults
    let old: Vec<String> = std::env::vars().map(|(k, _)| k).filter(|k| k.starts_with("SRP_") && k != "SRP_CONFIG" && k != "SRP_RECORD_DIR").collect();
    if !old.is_empty() {
        log::warn!("ignored environment {}: every setting comes from the config file now (docs/notes/config.md)", old.join(", "));
    }
    match &file {
        Some(f) => log::info!("config {}", f.display()),
        None => log::info!("no config file (--config, SRP_CONFIG or ./srp.toml): every setting is its default"),
    }
    // the first start makes the login key (srp setup normally did already); every start says how players connect
    match srp::setup::ensure_key(&config.key_path) {
        Ok((_, created)) => {
            if created {
                log::warn!("generated a new login key {} (keep it secret: it decrypts the players' passwords)", config.key_path.display());
            }
            log::info!("players start the game with: {}", srp::setup::launch_line(&config));
        }
        Err(e) => fail(e, 2),
    }
    let parts = match srp::open_parts(&config) {
        Ok(p) => p,
        Err(e) => fail(e, 2),
    };
    // the world host first (a cold daemon needs a few seconds); it lives as long as `_daemon`
    let _daemon = match srp::world_host::start(&config) {
        Ok(d) => d,
        Err(e) => fail(e, 2),
    };
    let stop = AtomicBool::new(false);
    if let Err(e) = srp::run(&config, parts, srp::default_matches(), &stop) {
        fail(e, 1);
    }
}

/// `srp check`: the update check address and what Steam players run (docs/notes/steam.md).
fn steam_lines(c: &srp::Config) {
    println!("update     tcp {} (the Steam launcher's update check: \"latest version\", so its Play button turns on)", c.update_addr);
    let host = c.player_host();
    let shown = host.clone().unwrap_or_else(|| "<server address>".into());
    let mut cmd = format!("steam-play-setup.cmd -Server {shown}");
    if c.login_addr.port() != 65000 {
        cmd.push_str(&format!(" -Port {}", c.login_addr.port()));
    }
    if c.update_addr.port() != srp::update::DEFAULT_PORT {
        cmd.push_str(&format!(" -UpdatePort {}", c.update_addr.port()));
    }
    println!("steam      players run: {cmd}   (scripts/steam-play; the game takes the language set in Steam, -Language russian etc. overrides it)");
    println!("start file players without Steam run: {}   (scripts/player; writes Skhid.cmd on their desktop)", srp::setup::skhid_setup_command(c));
    println!("           it fetches http://{shown}:{}{} (login address and port)", c.update_addr.port(), srp::update::CLIENT_TXT_PATH);
    let Some(host) = host else {
        println!("           (every address here is 0.0.0.0: tell players the address they reach this server at)");
        return;
    };
    match srp::update::launcher_cfg_lines(&host, c.login_addr.port(), &host, c.update_addr.port(), None) {
        Ok(lines) => {
            println!("           it writes %LOCALAPPDATA%\\Survarium-Steam\\launcher.cfg (UTF-16 LE):");
            for l in lines {
                println!("             {l}");
            }
        }
        Err(e) => println!("           {e}"),
    }
}

/// `srp check`: the effective settings and what is missing. 0 = ready to start, 2 = something needed is missing.
fn check(c: &srp::Config, file: Option<&std::path::Path>) -> i32 {
    let mut bad = 0;
    let mut need = |ok: bool, what: String| {
        println!("{} {what}", if ok { "ok     " } else { "MISSING" });
        if !ok {
            bad += 1;
        }
    };
    println!("config     {}", file.map_or("(none: defaults)".to_string(), |f| f.display().to_string()));
    println!("login      tcp {} (players: -server={}:{})", c.login_addr, srp::setup::player_host(c), c.login_addr.port());
    println!("lobby      udp {} (clients are told {})", c.lobby_bind, c.lobby_public);
    println!("matches    udp {} (clients are told {})", c.online.bind, c.online.public_host);
    steam_lines(c);
    println!("database   {}", c.db_path.display());
    println!("data       {}", c.data_dir.as_ref().map_or("(not set)".to_string(), |d| d.display().to_string()));
    println!("logs       {}", c.log_dir.display());
    println!("run        {}", c.run_dir.display());
    match login_server::LoginKey::from_file(&c.key_path) {
        Ok(_) => {
            need(true, format!("login key {}", c.key_path.display()));
            println!("launch     {}", srp::setup::launch_line(c));
        }
        Err(e) if c.key_path.exists() => need(false, format!("login key {}: {e}", c.key_path.display())),
        Err(_) => println!("ok      login key {} is made at the first start (or by srp setup)", c.key_path.display()),
    }
    match srp::load_tables(c) {
        Ok(_) => need(true, format!("game tables in {}", c.data_dir.as_ref().map_or(String::new(), |d| d.display().to_string()))),
        Err(e) => need(false, e),
    }
    let socket = &c.world_host.socket;
    match srp::world_host::projects(socket, std::time::Duration::from_secs(2)) {
        Ok(p) => println!("ok      world host running on {} ({} projects)", socket.display(), p.len()),
        Err(_) => match &c.world_host.daemon {
            None => need(false, format!("world host: none answers on {} and world_host.exe is not set", socket.display())),
            Some(d) => {
                need(d.exe.is_file(), format!("game exe {} (as {})", d.exe.display(), d.exe_win));
                if d.exe.is_file() {
                    match srp::setup::check_exe(&d.exe) {
                        Ok(()) => need(true, "game exe is the 0.69d build".to_string()),
                        Err(e) => need(false, e),
                    }
                }
                match srp::world_host::binary(d) {
                    Ok(b) => {
                        need(b.is_file(), format!("world host binary {}", b.display()));
                        let pw = srp::world_host::prewarm_dir(c, d);
                        println!("prewarm    {}", pw.as_ref().map_or("none".to_string(), |p| p.display().to_string()));
                        println!("--- generated daemon config ({}) ---", c.run_dir.join("daemon.cfg").display());
                        print!("{}", srp::world_host::daemon_config(c, d, pw.as_deref()));
                        println!("---");
                    }
                    Err(e) => need(false, e),
                }
            }
        },
    }
    let l = &c.lobby;
    println!(
        "queue      min {} players, AI players after {}, report grace {} s",
        l.online_min_players,
        l.online_bot_wait.map_or("never".to_string(), |d| format!("{} s", d.as_secs())),
        l.online_report_grace.as_secs()
    );
    println!("economy    equipment use costs {:?}, match silver factors {:?}", l.equipment_use_costs, l.match_silver_factors);
    println!("accounts   auto register {}, client version {}, start money {:?}", c.auto_register, c.required_version.as_deref().unwrap_or("any"), c.start_money);
    println!("league     {}", if l.league.is_monthly() { "monthly seasons".to_string() } else { format!("{} configured season(s)", l.league.seasons.len()) });
    println!("battles    {}", l.faction_battles.as_ref().map_or("none".to_string(), |s| format!("{} scheduled", s.intervals.len())));
    println!("servers    {}", l.game_servers.iter().map(|g| format!("{} ({})", g.label, g.host)).collect::<Vec<_>>().join(", "));
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    for line in srp::settings::event_lines(&l.events, now) {
        println!("{line}");
    }
    if bad == 0 {
        println!("ready");
        0
    } else {
        println!("{bad} thing(s) missing");
        2
    }
}
